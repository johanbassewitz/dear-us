//! Time-lock encryption for capsules, using the public drand "quicknet" beacon and the
//! tlock scheme (age-armored). Locking needs no network (the chain's public parameters are
//! embedded below). Unlocking fetches one public signature from the drand relay once the
//! round has been published; before that, nobody on Earth can decrypt the text.

use crate::error::{AppError, AppResult};
use std::time::Duration;

/// drand quicknet (scheme bls-unchained-g1-rfc9380).
/// Source: https://api.drand.sh/52db9ba70e0cc0f6eaf7803dd07447a1f5477735fd3f661792ba94600c84e971/info
pub const CHAIN_HASH: &str = "52db9ba70e0cc0f6eaf7803dd07447a1f5477735fd3f661792ba94600c84e971";
pub const PUBLIC_KEY: &str = "83cf0f2896adee7eb8b5f01fcad3912212c437e0073e911fb90022d3e760183c8c4b450b6a0a6c3ac6a5776a2d1064510d1fec758c921cc22b0e17e63aaf4bcb5ed66304de9cf809bd274ca73bab4af5a6e9c76a4bc09e76eae8991ef5ece45a";
pub const GENESIS: i64 = 1_692_803_367;
pub const PERIOD: i64 = 3;
const RELAY: &str = "https://api.drand.sh";

/// First round whose beacon is published at or after `unlock_unix`.
/// Round r is released at `GENESIS + (r - 1) * PERIOD`.
pub fn round_for(unlock_unix: i64) -> u64 {
    if unlock_unix <= GENESIS {
        return 1;
    }
    let elapsed = unlock_unix - GENESIS;
    ((elapsed + PERIOD - 1) / PERIOD + 1) as u64
}

/// When a round becomes available (unix seconds).
#[allow(dead_code)]
pub fn round_time(round: u64) -> i64 {
    GENESIS + (round as i64 - 1) * PERIOD
}

fn cipher_err(_: impl std::fmt::Debug) -> AppError {
    AppError::invalid("couldn't seal this capsule")
}

/// Encrypts `plain` so it can only be opened once the beacon for `unlock_unix` exists.
/// Returns age-armored text (safe to store as TEXT and to send).
pub fn encrypt(plain: &[u8], unlock_unix: i64) -> AppResult<String> {
    let chain = hex::decode(CHAIN_HASH).map_err(cipher_err)?;
    let pk = hex::decode(PUBLIC_KEY).map_err(cipher_err)?;
    let mut armored = tlock_age::armor::ArmoredWriter::wrap_output(Vec::new()).map_err(cipher_err)?;
    tlock_age::encrypt(&mut armored, plain, &chain, &pk, round_for(unlock_unix)).map_err(cipher_err)?;
    let bytes = armored.finish().map_err(cipher_err)?;
    String::from_utf8(bytes).map_err(cipher_err)
}

/// The round a ciphertext is locked to (read from its header, no key needed).
pub fn ciphertext_round(armored: &str) -> AppResult<u64> {
    let h = tlock_age::decrypt_header(armored.as_bytes())
        .map_err(|_| AppError::invalid("this capsule's seal can't be read"))?;
    Ok(h.round())
}

/// Decrypts with the beacon signature (hex) of the locked round.
pub fn decrypt(armored: &str, signature_hex: &str) -> AppResult<Vec<u8>> {
    let chain = hex::decode(CHAIN_HASH).map_err(cipher_err)?;
    let sig = hex::decode(signature_hex.trim())
        .map_err(|_| AppError::invalid("the time-lock network sent something unexpected"))?;
    let not_yet = || AppError::invalid("this capsule can't be opened yet");
    // The tlock crate asserts (panics) when handed a signature that isn't the right one
    // (wrong round, or a bad relay). That must be an error, never a crash.
    let attempt = std::panic::catch_unwind(|| {
        let mut out = Vec::new();
        tlock_age::decrypt(&mut out, armored.as_bytes(), &chain, &sig).map(|_| out)
    });
    match attempt {
        Ok(Ok(out)) => Ok(out),
        _ => Err(not_yet()),
    }
}

/// Fetches the public beacon signature (hex) for a round. The only network call in the app.
pub fn fetch_signature(round: u64) -> AppResult<String> {
    let url = format!("{RELAY}/{CHAIN_HASH}/public/{round}");
    let res = ureq::get(&url).timeout(Duration::from_secs(12)).call();
    let body = match res {
        Ok(r) => r.into_string().map_err(|_| offline())?,
        Err(ureq::Error::Status(404, _)) => {
            return Err(AppError::invalid("Not quite time yet - the time-lock network hasn't released this moment."))
        }
        Err(_) => return Err(offline()),
    };
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|_| offline())?;
    v.get("signature")
        .and_then(|s| s.as_str())
        .map(String::from)
        .ok_or_else(offline)
}

fn offline() -> AppError {
    AppError::invalid("Couldn't reach the time-lock network. Try again when you're online.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_math_matches_the_chain() {
        assert_eq!(round_for(0), 1);
        assert_eq!(round_for(GENESIS), 1);
        assert_eq!(round_for(GENESIS + 1), 2);
        assert_eq!(round_for(GENESIS + 3), 2);
        assert_eq!(round_for(GENESIS + 4), 3);
        // Round 1000 is published at genesis + 999 * 3 (api.drand.sh/<chain>/public/1000).
        assert_eq!(round_time(1000), GENESIS + 2997);
        assert_eq!(round_for(round_time(1000)), 1000);
        assert_eq!(round_for(round_time(1000) + 1), 1001);
        // The chosen round is never earlier than the requested moment.
        for t in [GENESIS + 7, 1_900_000_000, 2_100_000_000] {
            assert!(round_time(round_for(t)) >= t);
            assert!(round_time(round_for(t) - 1) < t);
        }
    }

    #[test]
    fn locks_to_the_right_round_and_stays_shut_before_unlock() {
        let unlock = 4_000_000_000; // far future: the beacon for this round does not exist
        let ct = encrypt(b"I love you", unlock).unwrap();
        assert!(ct.starts_with("-----BEGIN AGE ENCRYPTED FILE-----"));
        assert!(!ct.contains("I love you"));
        assert_eq!(ciphertext_round(&ct).unwrap(), round_for(unlock));
        // A signature for some other round (1000) must not open it.
        let sig1000 = "b44679b9a59af2ec876b1a6b1ad52ea9b1615fc3982b19576350f93447cb1125e342b73a8dd2bacbe47e4b6b63ed5e39";
        assert!(decrypt(&ct, sig1000).is_err());
        assert!(decrypt(&ct, "not hex").is_err());
    }

    #[test]
    fn opens_with_the_published_signature_of_a_known_round() {
        // Offline check against a real beacon value (quicknet round 1000, from api.drand.sh).
        let ct = encrypt(b"hello later", round_time(1000)).unwrap();
        assert_eq!(ciphertext_round(&ct).unwrap(), 1000);
        let sig1000 = "b44679b9a59af2ec876b1a6b1ad52ea9b1615fc3982b19576350f93447cb1125e342b73a8dd2bacbe47e4b6b63ed5e39";
        assert_eq!(decrypt(&ct, sig1000).unwrap(), b"hello later");
    }

    /// Needs internet: locks to a moment that has already passed, fetches the live beacon, opens it.
    #[test]
    #[ignore]
    fn network_roundtrip_with_live_beacon() {
        let past = chrono::Utc::now().timestamp() - 120;
        let ct = encrypt("for later, with love".as_bytes(), past).unwrap();
        let sig = fetch_signature(ciphertext_round(&ct).unwrap()).unwrap();
        assert_eq!(decrypt(&ct, &sig).unwrap(), "for later, with love".as_bytes());
    }
}
