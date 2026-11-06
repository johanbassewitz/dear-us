//! The temporary mailbox between the two PCs. Supabase implements it (`supabase.rs`);
//! tests use `MemoryRelay`. Everything passed in is already encrypted.

use crate::error::AppResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Device {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub revoked: bool,
    /// LAN address for direct transfers ("192.168.1.20:51234"), if published.
    #[serde(default)]
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Envelope {
    pub id: String,
    pub seq: i64,
    pub sender_device: String,
    pub ciphertext: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlobMeta {
    pub hash: String,
    pub size: i64,
    pub chunk_size: i64,
    pub chunk_count: i64,
    pub sender_device: String,
}

pub trait Relay: Send + Sync {
    fn devices(&self) -> AppResult<Vec<Device>>;
    /// Idempotent: putting the same id twice is a no-op.
    fn put_envelope(&self, id: &str, sender: &str, recipients: &[String], ciphertext: &str) -> AppResult<()>;
    /// Envelopes for `me` with seq > after, oldest first.
    fn fetch_envelopes(&self, me: &str, after: i64, limit: usize) -> AppResult<Vec<Envelope>>;
    fn ack_envelope(&self, id: &str, me: &str) -> AppResult<()>;
    /// True when every (non-revoked) recipient has confirmed, or it's already gone.
    fn envelope_delivered(&self, id: &str) -> AppResult<bool>;
    /// Deletes only if delivered. Returns true if it no longer exists.
    fn cleanup_envelope(&self, id: &str) -> AppResult<bool>;

    fn blob_meta(&self, hash: &str) -> AppResult<Option<BlobMeta>>;
    fn put_blob_meta(&self, meta: &BlobMeta, recipients: &[String]) -> AppResult<()>;
    fn put_chunk(&self, hash: &str, n: i64, data: &[u8]) -> AppResult<()>;
    fn get_chunk(&self, hash: &str, n: i64) -> AppResult<Vec<u8>>;
    fn ack_blob(&self, hash: &str, me: &str) -> AppResult<()>;
    fn blob_delivered(&self, hash: &str) -> AppResult<bool>;
    /// Deletes chunks + record only if delivered. Returns true if gone.
    fn cleanup_blob(&self, hash: &str) -> AppResult<bool>;
    /// Publishes this device's LAN address for direct transfers (None = stop).
    fn set_endpoint(&self, device: &str, endpoint: Option<&str>) -> AppResult<()>;
    /// Bytes this space currently occupies in the relay (for the free-tier cap).
    fn used_bytes(&self) -> AppResult<i64>;
    /// Announces the one-time encrypted snapshot for a newly linked device of mine.
    fn put_bootstrap_meta(&self, meta: &BlobMeta, recipient: &str) -> AppResult<()> {
        self.put_blob_meta(meta, &[recipient.to_string()])
    }
}

#[cfg(test)]
pub mod memory {
    use super::*;
    use crate::error::AppError;
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    pub struct State {
        pub devices: Vec<Device>,
        pub envelopes: Vec<(Envelope, Vec<String>)>,
        pub env_acks: HashMap<String, HashSet<String>>,
        pub blobs: HashMap<String, (BlobMeta, Vec<String>)>,
        pub chunks: HashMap<(String, i64), Vec<u8>>,
        pub blob_acks: HashMap<String, HashSet<String>>,
        pub seq: i64,
        /// test knob: fail the next N network calls
        pub fail_next: usize,
    }

    #[derive(Clone, Default)]
    pub struct MemoryRelay(pub Arc<Mutex<State>>);

    impl MemoryRelay {
        fn guard(&self) -> AppResult<std::sync::MutexGuard<'_, State>> {
            let mut s = self.0.lock().unwrap();
            if s.fail_next > 0 {
                s.fail_next -= 1;
                return Err(AppError::invalid("simulated network failure"));
            }
            Ok(s)
        }
        fn active_recipients(s: &State, rec: &[String]) -> Vec<String> {
            rec.iter().filter(|r| s.devices.iter().any(|d| &d.id == *r && !d.revoked)).cloned().collect()
        }
    }

    impl Relay for MemoryRelay {
        fn devices(&self) -> AppResult<Vec<Device>> {
            Ok(self.guard()?.devices.clone())
        }
        fn put_envelope(&self, id: &str, sender: &str, recipients: &[String], ciphertext: &str) -> AppResult<()> {
            let mut s = self.guard()?;
            if s.envelopes.iter().any(|(e, _)| e.id == id) {
                return Ok(());
            }
            s.seq += 1;
            let seq = s.seq;
            s.envelopes.push((Envelope { id: id.into(), seq, sender_device: sender.into(), ciphertext: ciphertext.into() }, recipients.to_vec()));
            Ok(())
        }
        fn fetch_envelopes(&self, me: &str, after: i64, limit: usize) -> AppResult<Vec<Envelope>> {
            let s = self.guard()?;
            Ok(s.envelopes.iter().filter(|(e, r)| e.seq > after && r.iter().any(|x| x == me)).take(limit).map(|(e, _)| e.clone()).collect())
        }
        fn ack_envelope(&self, id: &str, me: &str) -> AppResult<()> {
            self.guard()?.env_acks.entry(id.into()).or_default().insert(me.into());
            Ok(())
        }
        fn envelope_delivered(&self, id: &str) -> AppResult<bool> {
            let s = self.guard()?;
            let Some((_, rec)) = s.envelopes.iter().find(|(e, _)| e.id == id) else { return Ok(true) };
            let acks = s.env_acks.get(id).cloned().unwrap_or_default();
            Ok(Self::active_recipients(&s, rec).iter().all(|r| acks.contains(r)))
        }
        fn cleanup_envelope(&self, id: &str) -> AppResult<bool> {
            if !self.envelope_delivered(id)? {
                return Ok(false);
            }
            self.guard()?.envelopes.retain(|(e, _)| e.id != id);
            Ok(true)
        }
        fn blob_meta(&self, hash: &str) -> AppResult<Option<BlobMeta>> {
            Ok(self.guard()?.blobs.get(hash).map(|(m, _)| m.clone()))
        }
        fn put_blob_meta(&self, meta: &BlobMeta, recipients: &[String]) -> AppResult<()> {
            self.guard()?.blobs.entry(meta.hash.clone()).or_insert((meta.clone(), recipients.to_vec()));
            Ok(())
        }
        fn put_chunk(&self, hash: &str, n: i64, data: &[u8]) -> AppResult<()> {
            self.guard()?.chunks.insert((hash.into(), n), data.to_vec());
            Ok(())
        }
        fn get_chunk(&self, hash: &str, n: i64) -> AppResult<Vec<u8>> {
            self.guard()?.chunks.get(&(hash.into(), n)).cloned().ok_or_else(|| AppError::invalid("chunk not uploaded yet"))
        }
        fn ack_blob(&self, hash: &str, me: &str) -> AppResult<()> {
            self.guard()?.blob_acks.entry(hash.into()).or_default().insert(me.into());
            Ok(())
        }
        fn blob_delivered(&self, hash: &str) -> AppResult<bool> {
            let s = self.guard()?;
            let Some((_, rec)) = s.blobs.get(hash) else { return Ok(true) };
            let acks = s.blob_acks.get(hash).cloned().unwrap_or_default();
            Ok(Self::active_recipients(&s, rec).iter().all(|r| acks.contains(r)))
        }
        fn cleanup_blob(&self, hash: &str) -> AppResult<bool> {
            if !self.blob_delivered(hash)? {
                return Ok(false);
            }
            let mut s = self.guard()?;
            s.blobs.remove(hash);
            s.chunks.retain(|(h, _), _| h != hash);
            Ok(true)
        }
        fn set_endpoint(&self, device: &str, endpoint: Option<&str>) -> AppResult<()> {
            let mut s = self.guard()?;
            if let Some(d) = s.devices.iter_mut().find(|d| d.id == device) {
                d.endpoint = endpoint.map(String::from);
            }
            Ok(())
        }
        fn used_bytes(&self) -> AppResult<i64> {
            let s = self.guard()?;
            Ok(s.chunks.values().map(|c| c.len() as i64).sum::<i64>() + s.envelopes.iter().map(|(e, _)| e.ciphertext.len() as i64).sum::<i64>())
        }
    }
}
