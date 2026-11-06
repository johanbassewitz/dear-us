//! One sync pass: send local changes, confirm deliveries, receive the partner's changes,
//! move photos/videos in encrypted chunks, and clean the relay up — in that order.
//!
//! Safety rules (docs/PLAN.md §6):
//! - a payload is deleted from the relay only after every partner device confirmed it,
//!   and a device confirms only after the change is committed to its local database;
//! - files are verified by BLAKE3 hash before they become part of the archive;
//! - every step is resumable: crash or lose the network anywhere and the next pass
//!   continues (duplicates are ignored by op id / hash).
//!
//! The DB lock is never held across a network call.

use super::apply::{apply_batch, meta_set, Op};
use super::crypto::SpaceKey;
use super::relay::{BlobMeta, Relay};
use crate::db::repo::media as media_repo;
use crate::db::{new_id, now_iso};
use crate::error::{AppError, AppResult};
use crate::media::Store;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::Mutex;

pub const DEFAULT_CHUNK: i64 = 8 * 1024 * 1024;
/// Stay well under Supabase's free 1 GB storage (personal edition).
#[cfg_attr(feature = "store", allow(dead_code))]
pub const DEFAULT_QUOTA_CAP: i64 = 700 * 1024 * 1024;
const OPS_PER_ENVELOPE: i64 = 300;
/// How long a file waits for a direct transfer before it goes through the cloud anyway.
pub const DIRECT_GRACE_MINUTES: i64 = 15;

pub struct Ctx<'a> {
    pub db: &'a Mutex<Connection>,
    /// Direct PC-to-PC path on the same network (None = cloud only).
    pub direct: Option<&'a super::direct::DirectClient>,
    pub store: &'a Store,
    pub relay: &'a dyn Relay,
    pub key: &'a SpaceKey,
    pub device: String,
    pub chunk_size: i64,
    pub quota_cap: i64,
}

#[derive(Debug, Default, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub sent_ops: usize,
    pub received_ops: usize,
    pub uploaded_files: usize,
    pub downloaded_files: usize,
    pub waiting_for_partner_device: bool,
    pub waiting_for_space: bool,
    /// A newly linked device still waiting for the snapshot from the person's other device.
    pub waiting_for_link: bool,
    pub errors: Vec<String>,
    /// Media ids that became available (or changed) locally.
    pub media_ready: Vec<String>,
    /// Videos that still need a playable copy made.
    pub media_needing_preview: Vec<String>,
    pub changed: bool,
}

#[derive(Serialize, Deserialize)]
struct Wire {
    v: u32,
    ops: Vec<Op>,
}

fn meta(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT value FROM local_meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

impl Ctx<'_> {
    fn partners(&self) -> AppResult<Vec<String>> {
        Ok(self.relay.devices()?.into_iter().filter(|d| !d.revoked && d.id != self.device).map(|d| d.id).collect())
    }

    pub fn sync_once(&self) -> AppResult<Report> {
        let mut r = Report::default();
        let partners = self.partners()?;
        if partners.is_empty() {
            r.waiting_for_partner_device = true;
        } else {
            self.push(&partners, &mut r)?;
        }
        self.check_delivered()?;
        self.pull(&mut r)?;
        if !partners.is_empty() {
            self.upload(&partners, &mut r);
        }
        self.download(&mut r);
        Ok(r)
    }

    // ---------- outgoing ----------

    fn push(&self, partners: &[String], r: &mut Report) -> AppResult<()> {
        for _ in 0..20 {
            let (ops, manifests, new_blobs) = {
                let conn = self.db.lock().unwrap();
                let mut s = conn.prepare(
                    "SELECT op_id, hlc, device_id, entity, entity_id, kind, changes_json FROM oplog
                     WHERE state = 'pending' ORDER BY hlc LIMIT ?1",
                )?;
                let ops: Vec<Op> = s
                    .query_map([OPS_PER_ENVELOPE], |row| {
                        Ok(Op {
                            op_id: row.get(0)?,
                            hlc: row.get(1)?,
                            device_id: row.get(2)?,
                            entity: row.get(3)?,
                            entity_id: row.get(4)?,
                            kind: row.get(5)?,
                            changes: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or(Value::Null),
                        })
                    })?
                    .collect::<Result<_, _>>()?;
                if ops.is_empty() {
                    return Ok(());
                }
                let (manifests, new_blobs) = self.media_manifests(&conn, &ops)?;
                (ops, manifests, new_blobs)
            };
            let env_id = new_id();
            let mut all = manifests;
            all.extend(ops.iter().cloned());
            let plain = serde_json::to_vec(&Wire { v: 1, ops: all }).map_err(|e| AppError::invalid(e.to_string()))?;
            let ct = self.key.seal_b64(&plain, env_id.as_bytes());
            self.relay.put_envelope(&env_id, &self.device, partners, &ct)?;

            let mut conn = self.db.lock().unwrap();
            let tx = conn.transaction()?;
            for op in &ops {
                tx.execute("UPDATE oplog SET state = 'sent', envelope_id = ?2 WHERE op_id = ?1", params![op.op_id, env_id])?;
            }
            tx.execute(
                "INSERT INTO sync_outbox(envelope_id, op_count, size, state, created_at) VALUES (?1,?2,?3,'sent',?4)",
                params![env_id, ops.len() as i64, ct.len() as i64, now_iso()],
            )?;
            for (hash, media_id, size) in new_blobs {
                let count = ((size + self.chunk_size - 1) / self.chunk_size).max(1);
                tx.execute(
                    "INSERT OR IGNORE INTO sync_blobs(hash, direction, media_id, size, chunk_count, state, updated_at)
                     VALUES (?1, 'up', ?2, ?3, ?4, 'queued', ?5)",
                    params![hash, media_id, size, count, now_iso()],
                )?;
            }
            tx.commit()?;
            r.sent_ops += ops.len();
            r.changed = true;
        }
        Ok(())
    }

    /// Media details for any local photo/video these ops mention that the partner hasn't been told about.
    #[allow(clippy::type_complexity)]
    fn media_manifests(&self, conn: &Connection, ops: &[Op]) -> AppResult<(Vec<Op>, Vec<(String, String, i64)>)> {
        fn strings<'v>(v: &'v Value, out: &mut Vec<&'v str>) {
            match v {
                Value::String(s) if s.len() == 36 => out.push(s),
                Value::Array(a) => a.iter().for_each(|x| strings(x, out)),
                Value::Object(o) => o.values().for_each(|x| strings(x, out)),
                _ => {}
            }
        }
        let mut seen = HashSet::new();
        let mut manifests = Vec::new();
        let mut blobs = Vec::new();
        for op in ops {
            let mut ids = Vec::new();
            strings(&op.changes, &mut ids);
            for id in ids {
                if !seen.insert(id.to_string()) {
                    continue;
                }
                let Some(m) = media_repo::get(conn, id)? else { continue };
                if m.r.availability != "local" {
                    continue;
                }
                let told: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM sync_blobs WHERE hash = ?1 AND direction = 'up')",
                    [&m.hash],
                    |r| r.get(0),
                )?;
                if told {
                    continue;
                }
                manifests.push(Op {
                    op_id: new_id(),
                    hlc: String::new(),
                    device_id: self.device.clone(),
                    entity: "media".into(),
                    entity_id: m.r.id.clone(),
                    kind: "upsert".into(),
                    changes: json!({
                        "hash": m.hash, "kind": m.r.kind, "mime": m.r.mime, "ext": m.ext, "size": m.r.size,
                        "width": m.r.width, "height": m.r.height, "duration_ms": m.r.duration_ms,
                        "taken_at": m.r.taken_at, "original_name": m.r.original_name,
                    }),
                });
                blobs.push((m.hash.clone(), m.r.id.clone(), m.r.size));
            }
        }
        Ok((manifests, blobs))
    }

    fn check_delivered(&self) -> AppResult<()> {
        let pending: Vec<String> = {
            let conn = self.db.lock().unwrap();
            let mut s = conn.prepare("SELECT envelope_id FROM sync_outbox WHERE state = 'sent' ORDER BY created_at LIMIT 200")?;
            let r = s.query_map([], |r| r.get(0))?;
            r.collect::<Result<_, _>>()?
        };
        for id in pending {
            if !self.relay.envelope_delivered(&id)? {
                continue;
            }
            {
                let mut conn = self.db.lock().unwrap();
                let tx = conn.transaction()?;
                tx.execute("UPDATE oplog SET state = 'acked' WHERE envelope_id = ?1", [&id])?;
                tx.execute("UPDATE sync_outbox SET state = 'delivered' WHERE envelope_id = ?1", [&id])?;
                tx.commit()?;
            }
            self.relay.cleanup_envelope(&id)?;
        }
        Ok(())
    }

    // ---------- incoming ----------

    fn pull(&self, r: &mut Report) -> AppResult<()> {
        loop {
            let cursor: i64 = {
                let conn = self.db.lock().unwrap();
                meta(&conn, "sync.cursor")?.and_then(|c| c.parse().ok()).unwrap_or(0)
            };
            let batch = self.relay.fetch_envelopes(&self.device, cursor, 25)?;
            if batch.is_empty() {
                return Ok(());
            }
            for env in batch {
                let known: Option<String> = {
                    let conn = self.db.lock().unwrap();
                    conn.query_row("SELECT state FROM sync_inbox WHERE envelope_id = ?1", [&env.id], |r| r.get(0)).optional()?
                };
                if known.as_deref() != Some("failed") && known.is_some() {
                    // Applied before, confirmation may have been lost: confirm again.
                    self.relay.ack_envelope(&env.id, &self.device)?;
                } else if known.is_none() {
                    match self.open_envelope(&env.id, &env.ciphertext) {
                        Ok(wire) => {
                            self.apply_envelope(&env.id, env.seq, &wire.ops, r)?;
                            self.relay.ack_envelope(&env.id, &self.device)?;
                            let conn = self.db.lock().unwrap();
                            conn.execute("UPDATE sync_inbox SET state = 'acked', updated_at = ?2 WHERE envelope_id = ?1", params![env.id, now_iso()])?;
                        }
                        Err(e) => {
                            // Never confirm what we couldn't read: the sender's copy stays safe.
                            let conn = self.db.lock().unwrap();
                            conn.execute(
                                "INSERT OR REPLACE INTO sync_inbox(envelope_id, seq, state, error, updated_at) VALUES (?1,?2,'failed',?3,?4)",
                                params![env.id, env.seq, e.to_string(), now_iso()],
                            )?;
                            r.errors.push(format!("An update couldn't be opened: {e}"));
                        }
                    }
                }
                let _ = self.relay.cleanup_envelope(&env.id);
                let conn = self.db.lock().unwrap();
                meta_set(&conn, "sync.cursor", &env.seq.to_string())?;
            }
        }
    }

    fn open_envelope(&self, id: &str, ct: &str) -> AppResult<Wire> {
        let plain = self.key.open_b64(ct, id.as_bytes())?;
        let wire: Wire = serde_json::from_slice(&plain).map_err(|_| AppError::invalid("unreadable update format"))?;
        if wire.v != 1 {
            return Err(AppError::invalid("this update needs a newer version of Dear Us"));
        }
        Ok(wire)
    }

    fn apply_envelope(&self, id: &str, seq: i64, ops: &[Op], r: &mut Report) -> AppResult<()> {
        let mut conn = self.db.lock().unwrap();
        let tx = conn.transaction()?;
        if let Some(max) = ops.iter().filter(|o| !o.hlc.is_empty()).map(|o| o.hlc.as_str()).max() {
            super::hlc::observe(&tx, max, &self.device)?;
        }
        let out = apply_batch(&tx, ops, &self.device)?;
        for mid in &out.media_to_fetch {
            let m = media_repo::get(&tx, mid)?.ok_or_else(|| AppError::invalid("media vanished"))?;
            tx.execute(
                "INSERT OR IGNORE INTO sync_blobs(hash, direction, media_id, size, chunk_count, state, updated_at)
                 VALUES (?1, 'down', ?2, ?3, 0, 'queued', ?4)",
                params![m.hash, mid, m.r.size, now_iso()],
            )?;
        }
        tx.execute(
            "INSERT OR REPLACE INTO sync_inbox(envelope_id, seq, state, error, updated_at) VALUES (?1,?2,'applied',?3,?4)",
            params![id, seq, if out.failed.is_empty() { None } else { Some(out.failed.join("; ")) }, now_iso()],
        )?;
        tx.commit()?;
        r.received_ops += out.applied;
        r.changed |= out.applied > 0;
        r.errors.extend(out.failed.into_iter().map(|f| format!("Skipped one change ({f})")));
        Ok(())
    }

    // ---------- files ----------

    fn upload(&self, partners: &[String], r: &mut Report) {
        let direct_peer = self.direct.and_then(|d| d.reachable());
        #[allow(clippy::type_complexity)]
        let rows: Vec<(String, String, i64, i64, i64, String, Option<String>, Option<String>)> = {
            let conn = self.db.lock().unwrap();
            let mut s = match conn.prepare(
                "SELECT hash, media_id, size, chunk_count, chunks_done, state, direct_since, recipients FROM sync_blobs
                 WHERE direction = 'up' AND state IN ('queued','active','done') ORDER BY updated_at",
            ) {
                Ok(s) => s,
                Err(e) => return r.errors.push(e.to_string()),
            };
            s.query_map([], |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?, x.get(3)?, x.get(4)?, x.get(5)?, x.get(6)?, x.get(7)?)))
                .and_then(|m| m.collect())
                .unwrap_or_default()
        };
        let all_partners = partners;
        for (hash, media_id, size, count, done, state, direct_since, recipients) in rows {
            // A file meant only for some devices (e.g. a newly linked one) goes only to those
            // that are still active.
            let to: Vec<String> = match recipients.as_deref().and_then(|j| serde_json::from_str::<Vec<String>>(j).ok()) {
                Some(list) => list.into_iter().filter(|d| all_partners.contains(d)).collect(),
                None => all_partners.to_vec(),
            };
            if to.is_empty() {
                let _ = self.set_blob(&hash, "up", "delivered", None, None);
                continue;
            }
            let partners: &[String] = &to;
            let res = (|| -> AppResult<()> {
                if state == "done" {
                    if self.relay.blob_delivered(&hash)? && self.relay.cleanup_blob(&hash)? {
                        self.set_blob(&hash, "up", "delivered", None, None)?;
                    }
                    return Ok(());
                }
                // Same network? Let the partner fetch it directly, and skip the cloud copy.
                let in_grace = direct_since
                    .as_deref()
                    .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                    .map(|t| chrono::Utc::now().signed_duration_since(t) < chrono::Duration::minutes(DIRECT_GRACE_MINUTES))
                    .unwrap_or(true);
                // When the cloud allowance is used up, files keep going over the home network
                // whenever the partner's PC is reachable (free tier: unlimited Wi-Fi sync).
                let go_direct = in_grace && (direct_peer.is_some() || direct_since.is_some())
                    || direct_peer.is_some() && self.relay.used_bytes()? + size > self.quota_cap;
                if done == 0 && go_direct {
                    self.relay.put_blob_meta(
                        &BlobMeta { hash: hash.clone(), size, chunk_size: self.chunk_size, chunk_count: count, sender_device: self.device.clone() },
                        partners,
                    )?;
                    if direct_since.is_none() {
                        let conn = self.db.lock().unwrap();
                        conn.execute("UPDATE sync_blobs SET direct_since = ?2 WHERE hash = ?1 AND direction = 'up'", params![hash, now_iso()])?;
                    }
                    if self.relay.blob_delivered(&hash)? && self.relay.cleanup_blob(&hash)? {
                        self.set_blob(&hash, "up", "delivered", Some(count), None)?;
                        r.uploaded_files += 1;
                    }
                    return Ok(());
                }
                if done == 0 && self.relay.used_bytes()? + size > self.quota_cap {
                    r.waiting_for_space = true;
                    return Ok(()); // stays queued; the original is safe on this PC
                }
                let path = {
                    let conn = self.db.lock().unwrap();
                    let m = media_repo::get(&conn, &media_id)?.ok_or_else(|| AppError::invalid("photo no longer in the archive"))?;
                    self.store.original(&m.hash, &m.ext)
                };
                self.relay.put_blob_meta(
                    &BlobMeta { hash: hash.clone(), size, chunk_size: self.chunk_size, chunk_count: count, sender_device: self.device.clone() },
                    partners,
                )?;
                let mut f = File::open(&path)?;
                for n in done..count {
                    f.seek(SeekFrom::Start((n * self.chunk_size) as u64))?;
                    let mut buf = Vec::with_capacity(self.chunk_size as usize);
                    Read::by_ref(&mut f).take(self.chunk_size as u64).read_to_end(&mut buf)?;
                    let sealed = self.key.seal(&buf, format!("{hash}:{n}").as_bytes());
                    self.relay.put_chunk(&hash, n, &sealed)?;
                    self.set_blob(&hash, "up", "active", Some(n + 1), None)?;
                }
                self.set_blob(&hash, "up", "done", Some(count), None)?;
                r.uploaded_files += 1;
                Ok(())
            })();
            if let Err(e) = res {
                let _ = self.set_blob(&hash, "up", &state, None, Some(&e.to_string()));
                r.errors.push(format!("A photo/video upload paused: {e}"));
                break; // network trouble: try again next pass
            }
        }
    }

    fn download(&self, r: &mut Report) {
        let rows: Vec<(String, String, i64)> = {
            let conn = self.db.lock().unwrap();
            let mut s = match conn.prepare(
                "SELECT hash, media_id, chunks_done FROM sync_blobs WHERE direction = 'down' AND state IN ('queued','active') ORDER BY updated_at",
            ) {
                Ok(s) => s,
                Err(e) => return r.errors.push(e.to_string()),
            };
            s.query_map([], |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?))).and_then(|m| m.collect()).unwrap_or_default()
        };
        for (hash, media_id, done) in rows {
            match self.download_one(&hash, &media_id, done) {
                Ok(Some(preview_needed)) => {
                    r.downloaded_files += 1;
                    r.changed = true;
                    r.media_ready.push(media_id.clone());
                    if preview_needed {
                        r.media_needing_preview.push(media_id);
                    }
                }
                Ok(None) => {} // not uploaded yet
                Err(e) => {
                    let _ = self.set_blob(&hash, "down", "active", None, Some(&e.to_string()));
                    r.errors.push(format!("A photo/video download paused: {e}"));
                }
            }
        }
    }

    /// Ok(Some(needs_preview)) when finished, Ok(None) when the sender hasn't uploaded it yet.
    fn download_one(&self, hash: &str, media_id: &str, done: i64) -> AppResult<Option<bool>> {
        let Some(m) = self.relay.blob_meta(hash)? else { return Ok(None) };
        let part = self.store.incoming(hash);
        let mut f = OpenOptions::new().create(true).truncate(false).read(true).write(true).open(&part)?;
        let mut have = done;
        // Resume exactly at a chunk boundary (drop any half-written tail).
        f.set_len((have * m.chunk_size).min(m.size).max(0) as u64)?;
        f.seek(SeekFrom::End(0))?;
        while have < m.chunk_count {
            // Direct from the partner's PC when possible, otherwise from the cloud.
            let direct = self.direct.and_then(|d| d.chunk(&m.sender_device, hash, have).ok());
            let sealed = match direct {
                Some(c) => c,
                None => match self.relay.get_chunk(hash, have) {
                    Ok(c) => c,
                    Err(_) => return Ok(None), // not available yet (uploading, or waiting for a direct transfer)
                },
            };
            let plain = self.key.open(&sealed, format!("{hash}:{have}").as_bytes())?;
            f.write_all(&plain)?;
            have += 1;
            self.set_blob(hash, "down", "active", Some(have), None)?;
        }
        f.sync_all()?;
        drop(f);
        // Verify before it joins the archive.
        let bytes_ok = fs::metadata(&part)?.len() as i64 == m.size;
        let mut hasher = blake3::Hasher::new();
        let mut rd = File::open(&part)?;
        std::io::copy(&mut rd, &mut hasher)?;
        if !bytes_ok || hasher.finalize().to_hex().as_str() != hash {
            let _ = fs::remove_file(&part);
            self.set_blob(hash, "down", "queued", Some(0), Some("file failed its integrity check; downloading again"))?;
            return Err(AppError::invalid("a file arrived damaged and will be downloaded again"));
        }
        let (ext, mime) = {
            let conn = self.db.lock().unwrap();
            let row = media_repo::get(&conn, media_id)?.ok_or_else(|| AppError::invalid("media vanished"))?;
            (row.ext, row.r.mime)
        };
        let dest = self.store.original(hash, &ext);
        if dest.exists() {
            fs::remove_file(&part)?;
        } else {
            fs::create_dir_all(dest.parent().unwrap())?;
            fs::rename(&part, &dest)?;
        }
        let d = crate::media::derive_for(self.store, media_id, &dest, &mime);
        {
            let conn = self.db.lock().unwrap();
            media_repo::set_dimensions(&conn, media_id, d.width, d.height, d.duration_ms)?;
            media_repo::set_thumb(&conn, media_id, d.has_thumb)?;
            media_repo::set_preview(&conn, media_id, d.preview)?;
            conn.execute("UPDATE media SET availability = 'local' WHERE id = ?1", [media_id])?;
        }
        self.set_blob(hash, "down", "done", Some(m.chunk_count), None)?;
        self.relay.ack_blob(hash, &self.device)?;
        let _ = self.relay.cleanup_blob(hash);
        Ok(Some(d.preview == "needed"))
    }

    fn set_blob(&self, hash: &str, dir: &str, state: &str, chunks: Option<i64>, err: Option<&str>) -> AppResult<()> {
        let conn = self.db.lock().unwrap();
        conn.execute(
            "UPDATE sync_blobs SET state = ?3, chunks_done = COALESCE(?4, chunks_done), last_error = ?5,
                    attempts = attempts + CASE WHEN ?5 IS NULL THEN 0 ELSE 1 END, updated_at = ?6
             WHERE hash = ?1 AND direction = ?2",
            params![hash, dir, state, chunks, err, now_iso()],
        )?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub pending_ops: i64,
    pub awaiting_delivery: i64,
    pub uploads_pending: i64,
    pub downloads_pending: i64,
    pub failed_updates: i64,
    pub open_conflicts: i64,
}

pub fn counts(conn: &Connection) -> AppResult<Counts> {
    let q = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0));
    Ok(Counts {
        pending_ops: q("SELECT COUNT(*) FROM oplog WHERE state = 'pending'")?,
        awaiting_delivery: q("SELECT COUNT(*) FROM oplog WHERE state = 'sent'")?,
        uploads_pending: q("SELECT COUNT(*) FROM sync_blobs WHERE direction = 'up' AND state IN ('queued','active')")?,
        downloads_pending: q("SELECT COUNT(*) FROM sync_blobs WHERE direction = 'down' AND state IN ('queued','active')")?,
        failed_updates: q("SELECT COUNT(*) FROM sync_inbox WHERE state = 'failed'")?,
        open_conflicts: q("SELECT COUNT(*) FROM sync_conflicts WHERE resolved_at IS NULL")?,
    })
}

/// Serves one sealed chunk of a file this PC is currently sending (for direct transfers).
/// Only files queued for the partner are ever served.
pub fn serve_chunk(db: &Mutex<Connection>, store: &Store, key: &SpaceKey, hash: &str, n: i64, chunk_size: i64) -> AppResult<Vec<u8>> {
    let (ext, count): (String, i64) = {
        let conn = db.lock().unwrap();
        conn.query_row(
            "SELECT m.ext, b.chunk_count FROM sync_blobs b JOIN media m ON m.id = b.media_id
             WHERE b.hash = ?1 AND b.direction = 'up' AND b.state <> 'delivered'",
            [hash],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::invalid("not shared"))?
    };
    if n < 0 || n >= count {
        return Err(AppError::invalid("no such chunk"));
    }
    let mut f = File::open(store.original(hash, &ext))?;
    f.seek(SeekFrom::Start((n * chunk_size) as u64))?;
    let mut buf = Vec::with_capacity(chunk_size as usize);
    Read::by_ref(&mut f).take(chunk_size as u64).read_to_end(&mut buf)?;
    Ok(key.seal(&buf, format!("{hash}:{n}").as_bytes()))
}
