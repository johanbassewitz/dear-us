//! Bringing a newly linked device of the same person up to date ("Link my phone / other PC").
//!
//! The mailbox only holds what's in transit, so a device that joins later can't replay the
//! history from it. Instead, the device that made the link code sends ONE encrypted snapshot of
//! its archive (sealed with the space key, like everything else) addressed only to the new
//! device; photos and videos follow as ordinary files (over the home network when possible).
//!
//! The new device merges the snapshot before it applies anything from the mailbox, so rows
//! keep their per-field clocks and later changes merge exactly as on any other device.

use super::crypto::sha256_hex;
use super::engine::Ctx;
use super::relay::BlobMeta;
use crate::backup::{self, MediaRows};
use crate::db::now_iso;
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

/// The snapshot's name in the mailbox: derived from the receiving device, so the new device
/// knows what to look for without any other message.
pub fn snapshot_name(device: &str) -> String {
    sha256_hex(&format!("dearus-bootstrap|{device}"))
}

fn meta(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT value FROM local_meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

/// A copy of the archive without this PC's own bookkeeping (sync state, Cupid chat).
pub fn build_snapshot(conn: &Connection, dest: &Path) -> AppResult<()> {
    if dest.exists() {
        fs::remove_file(dest)?;
    }
    conn.execute("VACUUM INTO ?1", [dest.to_string_lossy()])?;
    let snap = Connection::open(dest)?;
    for t in backup::local_only_tables() {
        let exists: bool = snap.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)", [t], |r| r.get(0))?;
        if exists && !t.contains("fts") {
            snap.execute(&format!("DELETE FROM \"{t}\""), [])?;
        }
    }
    snap.execute_batch("VACUUM")?;
    Ok(())
}

/// Sender: snapshot + every photo/video, addressed only to `to`.
pub fn send(ctx: &Ctx, to: &str) -> AppResult<()> {
    let name = snapshot_name(to);
    let tmp = ctx.store.incoming(&format!("{name}.snapshot"));
    if let Some(dir) = tmp.parent() {
        fs::create_dir_all(dir)?;
    }
    build_snapshot(&ctx.db.lock().unwrap(), &tmp)?;
    let result = (|| -> AppResult<()> {
        let size = fs::metadata(&tmp)?.len() as i64;
        let count = ((size + ctx.chunk_size - 1) / ctx.chunk_size).max(1);
        ctx.relay.put_bootstrap_meta(&BlobMeta { hash: name.clone(), size, chunk_size: ctx.chunk_size, chunk_count: count, sender_device: ctx.device.clone() }, to)?;
        let mut f = fs::File::open(&tmp)?;
        let mut buf = vec![0u8; ctx.chunk_size as usize];
        for n in 0..count {
            let mut filled = 0;
            while filled < buf.len() {
                let k = f.read(&mut buf[filled..])?;
                if k == 0 {
                    break;
                }
                filled += k;
            }
            let sealed = ctx.key.seal(&buf[..filled], format!("{name}:{n}").as_bytes());
            ctx.relay.put_chunk(&name, n, &sealed)?;
        }
        Ok(())
    })();
    let _ = fs::remove_file(&tmp);
    result?;
    queue_media_for(&ctx.db.lock().unwrap(), to, ctx.chunk_size)?;
    Ok(())
}

/// Queues every local photo/video to be sent to `to` only. Files already on their way to
/// everyone are left as they are (they include the new device anyway).
pub fn queue_media_for(conn: &Connection, to: &str, chunk_size: i64) -> AppResult<usize> {
    let recipients = serde_json::to_string(&[to])?;
    let mut s = conn.prepare("SELECT id, hash, size FROM media WHERE availability = 'local' AND hash NOT LIKE 'pending:%'")?;
    let rows: Vec<(String, String, i64)> = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
    let mut n = 0;
    for (id, hash, size) in rows {
        let count = ((size + chunk_size - 1) / chunk_size).max(1);
        n += conn.execute(
            "INSERT INTO sync_blobs(hash, direction, media_id, size, chunk_count, state, updated_at, recipients)
             VALUES (?1, 'up', ?2, ?3, ?4, 'queued', ?5, ?6)
             ON CONFLICT(hash, direction) DO UPDATE SET state = 'queued', chunks_done = 0, attempts = 0, last_error = NULL,
               direct_since = NULL, recipients = excluded.recipients, updated_at = excluded.updated_at
             WHERE sync_blobs.state IN ('delivered', 'failed')",
            params![hash, id, size, count, now_iso(), recipients],
        )?;
    }
    Ok(n)
}

/// Receiver: Ok(true) once the snapshot is in. Ok(false) while it hasn't arrived yet.
pub fn receive(ctx: &Ctx) -> AppResult<bool> {
    let name = snapshot_name(&ctx.device);
    let Some(m) = ctx.relay.blob_meta(&name)? else { return Ok(false) };
    let tmp = ctx.store.incoming(&format!("{name}.snapshot"));
    if let Some(dir) = tmp.parent() {
        fs::create_dir_all(dir)?;
    }
    {
        let mut f = fs::File::create(&tmp)?;
        for n in 0..m.chunk_count {
            let sealed = match ctx.relay.get_chunk(&name, n) {
                Ok(c) => c,
                Err(_) => return Ok(false), // still uploading
            };
            f.write_all(&ctx.key.open(&sealed, format!("{name}:{n}").as_bytes())?)?;
        }
        f.sync_all()?;
    }
    let result = apply_snapshot(ctx.db, ctx.store, &tmp, &ctx.device);
    let _ = fs::remove_file(&tmp);
    result?;
    ctx.relay.ack_blob(&name, &ctx.device)?;
    let _ = ctx.relay.cleanup_blob(&name);
    Ok(true)
}

/// Merges a snapshot into this (new) device's archive and queues every photo to download.
pub fn apply_snapshot(db: &std::sync::Mutex<Connection>, store: &crate::media::Store, snap: &Path, device: &str) -> AppResult<()> {
    {
        let check = Connection::open_with_flags(snap, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let ok: String = check.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if ok != "ok" {
            return Err(AppError::invalid("the snapshot from your other device arrived damaged"));
        }
    }
    let mut conn = db.lock().unwrap();
    backup::merge_snapshot(&mut conn, snap, MediaRows::All)?;
    let tx = conn.transaction()?;
    // photos: here only once their file arrives
    let rows: Vec<(String, String, String, i64)> = {
        let mut s = tx.prepare("SELECT id, hash, ext, size FROM media WHERE hash NOT LIKE 'pending:%'")?;
        let r = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        r.collect::<Result<_, _>>()?
    };
    for (id, hash, ext, size) in rows {
        if store.original(&hash, &ext).exists() {
            continue;
        }
        tx.execute("UPDATE media SET availability = 'pending', has_thumb = 0 WHERE id = ?1", [&id])?;
        tx.execute(
            "INSERT OR IGNORE INTO sync_blobs(hash, direction, media_id, size, chunk_count, state, updated_at) VALUES (?1, 'down', ?2, ?3, 0, 'queued', ?4)",
            params![hash, id, size, now_iso()],
        )?;
    }
    // this device's clock starts after everything it now holds, so its own edits win as usual
    if let Some(max) = max_field_clock(&tx)? {
        super::hlc::observe(&tx, &max, device)?;
    }
    tx.execute("DELETE FROM local_meta WHERE key IN ('bootstrap.pending', 'bootstrap.from')", [])?;
    tx.commit()?;
    Ok(())
}

fn max_field_clock(conn: &Connection) -> AppResult<Option<String>> {
    let tables: Vec<String> = {
        let mut s = conn.prepare("SELECT m.name FROM sqlite_master m WHERE m.type = 'table' AND EXISTS (SELECT 1 FROM pragma_table_info(m.name) WHERE name = 'field_clock')")?;
        let r = s.query_map([], |r| r.get(0))?;
        r.collect::<Result<_, _>>()?
    };
    let mut max: Option<String> = None;
    for t in tables {
        let mut s = conn.prepare(&format!("SELECT field_clock FROM \"{t}\" WHERE field_clock IS NOT NULL AND field_clock <> '{{}}'"))?;
        let clocks: Vec<String> = s.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        for c in clocks {
            let v: serde_json::Value = serde_json::from_str(&c).unwrap_or_default();
            for h in v.as_object().into_iter().flat_map(|o| o.values()).filter_map(|x| x.as_str()) {
                if max.as_deref().is_none_or(|m| h > m) {
                    max = Some(h.to_string());
                }
            }
        }
    }
    Ok(max)
}

// ------------------------------------------------------------------ bookkeeping

/// This device is new and waiting for its snapshot (set when linking).
pub fn pending(conn: &Connection) -> bool {
    meta(conn, "bootstrap.pending").ok().flatten().as_deref() == Some("1")
}

pub fn mark_pending(conn: &Connection, from: &str) -> AppResult<()> {
    super::apply::meta_set(conn, "bootstrap.pending", "1")?;
    super::apply::meta_set(conn, "bootstrap.from", from)
}

/// The device that made a link code waits (one day at most) for the new device to appear.
pub fn mark_link_created(conn: &Connection) -> AppResult<()> {
    super::apply::meta_set(conn, "link.pending", &now_iso())
}

fn link_open(conn: &Connection) -> bool {
    meta(conn, "link.pending")
        .ok()
        .flatten()
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(&t).ok())
        .is_some_and(|t| chrono::Utc::now().signed_duration_since(t) < chrono::Duration::days(1))
}

/// Sender side, after a normal sync pass: sends the snapshot to any new device of mine.
/// Returns how many devices were brought up to date.
pub fn serve_new_devices(ctx: &Ctx, my_user: &str) -> AppResult<usize> {
    if !link_open(&ctx.db.lock().unwrap()) {
        return Ok(0);
    }
    let mut sent = 0;
    for d in ctx.relay.devices()? {
        if d.revoked || d.id == ctx.device || d.user_id != my_user {
            continue;
        }
        let key = format!("bootstrap.sent:{}", d.id);
        if meta(&ctx.db.lock().unwrap(), &key)?.is_some() {
            continue;
        }
        // devices that existed before this link code are already in sync
        let known = meta(&ctx.db.lock().unwrap(), &format!("device.known:{}", d.id))?.is_some();
        if known {
            continue;
        }
        send(ctx, &d.id)?;
        let conn = ctx.db.lock().unwrap();
        super::apply::meta_set(&conn, &key, &now_iso())?;
        conn.execute("DELETE FROM local_meta WHERE key = 'link.pending'", [])?;
        sent += 1;
    }
    Ok(sent)
}

/// Remembers the devices that exist when a link code is made (they need no snapshot).
pub fn remember_devices(conn: &Connection, ids: &[String]) -> AppResult<()> {
    for id in ids {
        super::apply::meta_set(conn, &format!("device.known:{id}"), "1")?;
    }
    Ok(())
}
