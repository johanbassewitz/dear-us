//! End-to-end encryption for everything that goes through the relay.
//! One 256-bit "space key" per couple, created on the first PC and handed over in the
//! invite code. Supabase only ever stores ciphertext.

use crate::error::{AppError, AppResult};
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};

#[derive(Clone)]
pub struct SpaceKey([u8; 32]);

impl std::fmt::Debug for SpaceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SpaceKey(…)") // never print the key
    }
}

impl SpaceKey {
    pub fn generate() -> SpaceKey {
        let k = XChaCha20Poly1305::generate_key(&mut OsRng);
        SpaceKey(k.into())
    }

    pub fn to_b64(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }

    pub fn from_b64(s: &str) -> AppResult<SpaceKey> {
        let v = URL_SAFE_NO_PAD.decode(s.trim()).map_err(|_| AppError::invalid("invalid key"))?;
        let a: [u8; 32] = v.try_into().map_err(|_| AppError::invalid("invalid key"))?;
        Ok(SpaceKey(a))
    }

    /// 24-word recovery phrase (BIP-39 English) — the human-friendly backup of the key.
    pub fn to_phrase(&self) -> String {
        bip39::Mnemonic::from_entropy(&self.0).expect("32 bytes is valid entropy").to_string()
    }

    #[allow(dead_code)] // used by restore-from-backup (Phase 5)
    pub fn from_phrase(phrase: &str) -> AppResult<SpaceKey> {
        let normalized = phrase.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
        let m = bip39::Mnemonic::parse_normalized(&normalized)
            .map_err(|_| AppError::invalid("That recovery phrase doesn't look right. Check each word."))?;
        let e = m.to_entropy();
        let a: [u8; 32] = e.try_into().map_err(|_| AppError::invalid("A recovery phrase has 24 words."))?;
        Ok(SpaceKey(a))
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new((&self.0).into())
    }

    /// Encrypts and authenticates. `aad` binds the ciphertext to its context (e.g. envelope id),
    /// so a payload can't be replayed under another name.
    pub fn seal(&self, plain: &[u8], aad: &[u8]) -> Vec<u8> {
        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        let mut out = nonce.to_vec();
        out.extend(self.cipher().encrypt(&nonce, Payload { msg: plain, aad }).expect("encryption cannot fail"));
        out
    }

    pub fn open(&self, sealed: &[u8], aad: &[u8]) -> AppResult<Vec<u8>> {
        if sealed.len() < 24 + 16 {
            return Err(AppError::invalid("damaged sync payload"));
        }
        let (nonce, ct) = sealed.split_at(24);
        self.cipher()
            .decrypt(XNonce::from_slice(nonce), Payload { msg: ct, aad })
            .map_err(|_| AppError::invalid("a sync payload failed its integrity check (damaged or not for this space)"))
    }

    /// Keyed BLAKE3 MAC (used to authenticate direct PC-to-PC requests).
    pub fn mac(&self, msg: &[u8]) -> String {
        let k = blake3::derive_key("dear-us direct transfer v1", &self.0);
        blake3::keyed_hash(&k, msg).to_hex().to_string()
    }

    pub fn seal_b64(&self, plain: &[u8], aad: &[u8]) -> String {
        STANDARD.encode(self.seal(plain, aad))
    }

    pub fn open_b64(&self, sealed: &str, aad: &[u8]) -> AppResult<Vec<u8>> {
        let raw = STANDARD.decode(sealed.trim()).map_err(|_| AppError::invalid("damaged sync payload"))?;
        self.open(&raw, aad)
    }
}

pub fn sha256_hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(s.as_bytes()))
}

pub fn random_token(bytes: usize) -> String {
    use chacha20poly1305::aead::rand_core::RngCore;
    let mut b = vec![0u8; bytes];
    OsRng.fill_bytes(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open_roundtrip_and_tamper_detection() {
        let k = SpaceKey::generate();
        let s = k.seal(b"hello", b"env-1");
        assert_eq!(k.open(&s, b"env-1").unwrap(), b"hello");
        assert!(k.open(&s, b"env-2").is_err(), "bound to its context");
        let mut bad = s.clone();
        *bad.last_mut().unwrap() ^= 1;
        assert!(k.open(&bad, b"env-1").is_err(), "tampering detected");
        assert!(SpaceKey::generate().open(&s, b"env-1").is_err(), "other couple's key can't read it");
    }

    #[test]
    fn phrase_roundtrip() {
        let k = SpaceKey::generate();
        let p = k.to_phrase();
        assert_eq!(p.split(' ').count(), 24);
        let back = SpaceKey::from_phrase(&format!("  {}  ", p.to_uppercase())).unwrap();
        assert_eq!(back.to_b64(), k.to_b64());
        assert!(SpaceKey::from_phrase("love love love").is_err());
        assert_eq!(SpaceKey::from_b64(&k.to_b64()).unwrap().0, k.0);
    }
}
