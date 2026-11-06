//! Encrypted backups (Store edition): the same folder idea as a plain backup, but every file is
//! sealed with the couple's space key, so a backup in OneDrive / Google Drive / iCloud Drive /
//! Dropbox is unreadable to the cloud company (and to us). Restoring needs the space key: it's
//! already on a paired PC, or it comes back from the 24-word recovery phrase.
//!
//!   DearUsBackup/
//!     manifest.json         {"format":2,"encrypted":true,"keyCheck":…,"sealed":…}
//!     db/<time>.dub         sealed database snapshots (last 10)
//!     media/ab/<name>.dub   sealed originals; names don't reveal the files' hashes
//!
//! Files are sealed in 4 MiB pieces (XChaCha20-Poly1305), each bound to the file and its
//! position, so pieces can't be swapped, reordered or cut off unnoticed.

use super::{blake3_file, backup_dir, BackupResult, RestoreReport, VerifyReport, MediaRows};
use crate::db::now_iso;
use crate::error::{AppError, AppResult};
use crate::media::Store;
use crate::sync::crypto::{sha256_hex, SpaceKey};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAGIC: &[u8; 4] = b"DUB1";
const PIECE: usize = 4 * 1024 * 1024;
const KEEP: usize = 10;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SealedMedia {
    pub hash: String,
    pub ext: String,
    pub size: i64,
    /// file name inside media/ (derived from the hash with the key)
    pub name: String,
    pub stored_hash: String,
    pub stored_size: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SealedManifest {
    pub app_version: String,
    pub schema_version: i64,
    pub created_at: String,
    pub space_name: Option<String>,
    pub entries: i64,
    pub db_file: String,
    pub db_hash: String,
    pub db_stored_hash: String,
    pub db_stored_size: i64,
    pub media: Vec<SealedMedia>,
}

fn key_check(key: &SpaceKey) -> String {
    key.mac(b"dearus-backup-key-check")
}

fn media_name(key: &SpaceKey, hash: &str) -> String {
    sha256_hex(&key.mac(format!("dearus-backup-name|{hash}").as_bytes()))[..40].to_string()
}

fn media_path(dir: &Path, name: &str) -> PathBuf {
    dir.join("media").join(&name[..2]).join(format!("{name}.dub"))
}

fn aad(label: &str, i: u64, last: bool) -> Vec<u8> {
    format!("dearus-backup|{label}|{i}|{}", last as u8).into_bytes()
}

fn read_full(f: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        let n = f.read(&mut buf[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    Ok(filled)
}

/// Seals `src` into `dest` (written atomically). Returns (BLAKE3 of the sealed file, its size).
pub fn seal_file(key: &SpaceKey, src: &Path, dest: &Path, label: &str) -> AppResult<(String, i64)> {
    fs::create_dir_all(dest.parent().unwrap())?;
    let tmp = dest.with_extension("sealing");
    {
        let mut i = File::open(src)?;
        let mut o = File::create(&tmp)?;
        o.write_all(MAGIC)?;
        let mut cur = vec![0u8; PIECE];
        let mut next = vec![0u8; PIECE];
        let mut n_cur = read_full(&mut i, &mut cur)?;
        let mut idx = 0u64;
        loop {
            let n_next = if n_cur == PIECE { read_full(&mut i, &mut next)? } else { 0 };
            let last = n_next == 0;
            let sealed = key.seal(&cur[..n_cur], &aad(label, idx, last));
            o.write_all(&(sealed.len() as u32).to_le_bytes())?;
            o.write_all(&sealed)?;
            if last {
                break;
            }
            std::mem::swap(&mut cur, &mut next);
            n_cur = n_next;
            idx += 1;
        }
        o.flush()?;
        o.sync_all()?;
    }
    fs::rename(&tmp, dest)?;
    blake3_file(dest)
}

/// Opens a sealed file into `dest`. Fails (writing nothing) if anything was changed or cut.
pub fn open_file(key: &SpaceKey, src: &Path, dest: &Path, label: &str) -> AppResult<()> {
    let damaged = || AppError::invalid("a backup file is damaged or doesn't belong to this space");
    fs::create_dir_all(dest.parent().unwrap())?;
    let tmp = dest.with_extension("opening");
    let res = (|| -> AppResult<()> {
        let mut i = File::open(src)?;
        let mut magic = [0u8; 4];
        i.read_exact(&mut magic).map_err(|_| damaged())?;
        if &magic != MAGIC {
            return Err(damaged());
        }
        let mut o = File::create(&tmp)?;
        let mut idx = 0u64;
        let mut len = [0u8; 4];
        i.read_exact(&mut len).map_err(|_| damaged())?;
        loop {
            let n = u32::from_le_bytes(len) as usize;
            if n > PIECE + 64 {
                return Err(damaged());
            }
            let mut sealed = vec![0u8; n];
            i.read_exact(&mut sealed).map_err(|_| damaged())?;
            // is there another piece after this one?
            let last = match i.read_exact(&mut len) {
                Ok(()) => false,
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => true,
                Err(e) => return Err(e.into()),
            };
            let plain = key.open(&sealed, &aad(label, idx, last)).map_err(|_| damaged())?;
            o.write_all(&plain)?;
            if last {
                break;
            }
            idx += 1;
        }
        o.sync_all()?;
        Ok(())
    })();
    match res {
        Ok(()) => Ok(fs::rename(&tmp, dest)?),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Is the backup in this folder an encrypted one?
pub fn is_sealed(chosen: &Path) -> bool {
    fs::read(backup_dir(chosen).join("manifest.json"))
        .ok()
        .and_then(|raw| serde_json::from_slice::<serde_json::Value>(&raw).ok())
        .is_some_and(|v| v["encrypted"] == true)
}

pub fn read_manifest(chosen: &Path, key: &SpaceKey) -> AppResult<(PathBuf, SealedManifest)> {
    let dir = backup_dir(chosen);
    let raw = fs::read(dir.join("manifest.json")).map_err(|_| AppError::invalid("There's no Dear Us backup in that folder."))?;
    let v: serde_json::Value = serde_json::from_slice(&raw).map_err(|_| AppError::invalid("The backup's manifest is damaged."))?;
    if v["encrypted"] != true {
        return Err(AppError::invalid("This backup isn't encrypted."));
    }
    if v["keyCheck"].as_str() != Some(key_check(key).as_str()) {
        return Err(AppError::invalid("[backup_wrong_key] This backup belongs to a different space. Check the recovery phrase."));
    }
    let sealed = STANDARD.decode(v["sealed"].as_str().unwrap_or_default()).map_err(|_| AppError::invalid("The backup's manifest is damaged."))?;
    let plain = key.open(&sealed, b"dearus-backup-manifest").map_err(|_| AppError::invalid("The backup's manifest is damaged."))?;
    let m: SealedManifest = serde_json::from_slice(&plain).map_err(|_| AppError::invalid("The backup's manifest is damaged."))?;
    Ok((dir, m))
}

fn write_manifest(dir: &Path, key: &SpaceKey, m: &SealedManifest) -> AppResult<()> {
    let sealed = key.seal(&serde_json::to_vec(m)?, b"dearus-backup-manifest");
    let wrapper = serde_json::json!({
        "format": 2,
        "encrypted": true,
        "keyCheck": key_check(key),
        "createdAt": m.created_at,
        "sealed": STANDARD.encode(sealed),
    });
    let tmp = dir.join("manifest.json.writing");
    fs::write(&tmp, serde_json::to_vec_pretty(&wrapper)?)?;
    fs::rename(&tmp, dir.join("manifest.json"))?;
    Ok(())
}

/// Checks every sealed file against the manifest (no key needed beyond reading it).
pub fn verify_with(dir: &Path, m: &SealedManifest) -> AppResult<VerifyReport> {
    let mut r = VerifyReport::default();
    match blake3_file(&dir.join(&m.db_file)) {
        Ok((h, n)) if h == m.db_stored_hash && n == m.db_stored_size => r.db_ok = true,
        Ok(_) => r.damaged.push(m.db_file.clone()),
        Err(_) => r.missing.push(m.db_file.clone()),
    }
    for f in &m.media {
        r.checked_files += 1;
        match blake3_file(&media_path(dir, &f.name)) {
            Ok((h, n)) if h == f.stored_hash && n == f.stored_size => {}
            Ok(_) => r.damaged.push(f.hash.clone()),
            Err(_) => r.missing.push(f.hash.clone()),
        }
    }
    r.ok = r.missing.is_empty() && r.damaged.is_empty() && r.db_ok;
    Ok(r)
}

pub fn verify_backup(chosen: &Path, key: &SpaceKey) -> AppResult<VerifyReport> {
    let (dir, m) = read_manifest(chosen, key)?;
    verify_with(&dir, &m)
}

fn stamp() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S").to_string()
}

/// Writes (or updates) an encrypted backup. Files already in it aren't sealed again.
pub fn run_backup(db: &Mutex<Connection>, store: &Store, chosen: &Path, app_version: &str, key: &SpaceKey, progress: &dyn Fn(usize, usize)) -> AppResult<BackupResult> {
    let dir = backup_dir(chosen);
    fs::create_dir_all(dir.join("db"))?;
    fs::create_dir_all(dir.join("media"))?;
    // what an earlier encrypted backup already holds (to skip re-sealing)
    let previous: HashMap<String, SealedMedia> = read_manifest(chosen, key).map(|(_, m)| m.media.into_iter().map(|f| (f.hash.clone(), f)).collect()).unwrap_or_default();
    // an earlier plain backup in the same folder is removed once this one is complete
    let plain_before = super::read_manifest(chosen).ok().map(|(_, m)| m);

    let (rows, schema, space, entries) = {
        let conn = db.lock().unwrap();
        let mut s = conn.prepare("SELECT hash, ext, size FROM media WHERE availability = 'local' AND hash NOT LIKE 'pending:%'")?;
        let rows: Vec<(String, String, i64)> = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
        let schema: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let space: Option<String> = conn.query_row("SELECT name FROM space LIMIT 1", [], |r| r.get(0)).optional()?;
        let entries: i64 = conn.query_row("SELECT COUNT(*) FROM journal_entries WHERE deleted_at IS NULL", [], |r| r.get(0))?;
        (rows, schema, space, entries)
    };
    let total = rows.len() + 1;
    let (mut copied, mut bytes) = (0usize, 0i64);
    let mut media = Vec::with_capacity(rows.len());
    for (i, (hash, ext, size)) in rows.into_iter().enumerate() {
        progress(i, total);
        let name = media_name(key, &hash);
        let dest = media_path(&dir, &name);
        if let Some(p) = previous.get(&hash) {
            if dest.exists() && fs::metadata(&dest).map(|x| x.len() as i64 == p.stored_size).unwrap_or(false) {
                media.push(p.clone());
                continue;
            }
        }
        let src = store.original(&hash, &ext);
        if !src.exists() {
            return Err(AppError::invalid(format!("a photo/video is missing from this PC's archive ({hash}); run Verify")));
        }
        if blake3_file(&src)?.0 != hash {
            return Err(AppError::invalid(format!("a photo/video on this PC is damaged ({hash}); run Verify")));
        }
        let (stored_hash, stored_size) = seal_file(key, &src, &dest, &hash)?;
        copied += 1;
        bytes += stored_size;
        media.push(SealedMedia { hash, ext, size, name, stored_hash, stored_size });
    }

    let created_at = now_iso();
    let db_file = format!("db/{}.dub", stamp());
    let tmp_db = dir.join("db").join(".snapshot.db.tmp");
    if tmp_db.exists() {
        fs::remove_file(&tmp_db)?;
    }
    db.lock().unwrap().execute("VACUUM INTO ?1", [tmp_db.to_string_lossy()])?;
    let sealed_db = (|| -> AppResult<(String, String, i64)> {
        let ok: String = Connection::open(&tmp_db)?.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if ok != "ok" {
            return Err(AppError::invalid("This PC's database didn't pass its check, so no backup was made. Run Verify in Settings."));
        }
        let (plain_hash, _) = blake3_file(&tmp_db)?;
        let (h, n) = seal_file(key, &tmp_db, &dir.join(&db_file), "db")?;
        Ok((plain_hash, h, n))
    })();
    let _ = fs::remove_file(&tmp_db);
    let (db_hash, db_stored_hash, db_stored_size) = sealed_db?;
    progress(total, total);

    let m = SealedManifest { app_version: app_version.into(), schema_version: schema, created_at: created_at.clone(), space_name: space, entries, db_file, db_hash, db_stored_hash, db_stored_size, media };
    let verify = verify_with(&dir, &m)?;
    if !verify.ok {
        return Err(AppError::invalid(format!(
            "The backup couldn't be verified ({} missing, {} damaged). Your memories on this PC are fine. Check the backup folder and try again.",
            verify.missing.len(),
            verify.damaged.len()
        )));
    }
    write_manifest(&dir, key, &m)?;
    prune(&dir.join("db"), &m.db_file)?;
    // sealed files no longer listed (photos deleted for good) are removed too
    let keep: std::collections::HashSet<String> = m.media.iter().map(|f| format!("{}.dub", f.name)).collect();
    if let Ok(groups) = fs::read_dir(dir.join("media")) {
        for g in groups.flatten() {
            for f in fs::read_dir(g.path()).into_iter().flatten().flatten() {
                let n = f.file_name().to_string_lossy().to_string();
                if n.ends_with(".dub") && !keep.contains(&n) {
                    let _ = fs::remove_file(f.path());
                }
            }
        }
    }
    // no readable copies left behind from before encryption was on
    if let Some(old) = plain_before {
        for f in &old.media {
            let _ = fs::remove_file(dir.join("media").join(&f.hash[0..2]).join(format!("{}.{}", f.hash, f.ext)));
        }
        for e in fs::read_dir(dir.join("db")).into_iter().flatten().flatten() {
            if e.path().extension().is_some_and(|x| x == "db") {
                let _ = fs::remove_file(e.path());
            }
        }
        let _ = fs::remove_dir_all(dir.join("thumbs"));
    }
    Ok(BackupResult { folder: dir.display().to_string(), created_at, copied_files: copied, copied_bytes: bytes, total_files: m.media.len(), verify })
}

fn prune(folder: &Path, current: &str) -> AppResult<()> {
    let mut snaps: Vec<PathBuf> = fs::read_dir(folder)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "dub")).collect();
    snaps.sort();
    let current = Path::new(current).file_name().map(|n| n.to_os_string());
    while snaps.len() > KEEP {
        let old = snaps.remove(0);
        if old.file_name().map(|n| n.to_os_string()) != current {
            let _ = fs::remove_file(old);
        }
    }
    Ok(())
}

/// Adds everything from an encrypted backup that this PC doesn't have. Never overwrites.
pub fn restore(db: &Mutex<Connection>, store: &Store, chosen: &Path, key: &SpaceKey, scratch: &Path, progress: &dyn Fn(usize, usize)) -> AppResult<RestoreReport> {
    let (dir, m) = read_manifest(chosen, key)?;
    let v = verify_with(&dir, &m)?;
    if !v.db_ok {
        return Err(AppError::invalid("The backup's database is damaged, so nothing was restored. Try another copy."));
    }
    let app_schema: i64 = db.lock().unwrap().query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if m.schema_version > app_schema {
        return Err(AppError::invalid("This backup is from a newer version of Dear Us. Update the app first."));
    }
    let mut report = RestoreReport::default();
    let total = m.media.len();
    let mut skipped: Vec<String> = Vec::new();
    for (i, f) in m.media.iter().enumerate() {
        progress(i, total);
        if v.missing.contains(&f.hash) || v.damaged.contains(&f.hash) {
            skipped.push(f.hash.clone());
            continue;
        }
        let dest = store.original(&f.hash, &f.ext);
        if dest.exists() {
            report.skipped_existing_files += 1;
            continue;
        }
        open_file(key, &media_path(&dir, &f.name), &dest, &f.hash)?;
        if blake3_file(&dest)?.0 != f.hash {
            let _ = fs::remove_file(&dest);
            skipped.push(f.hash.clone());
            continue;
        }
        report.added_files += 1;
    }
    fs::create_dir_all(scratch)?;
    let tmp_db = scratch.join("restore-snapshot.db");
    open_file(key, &dir.join(&m.db_file), &tmp_db, "db")?;
    let res = (|| -> AppResult<()> {
        if blake3_file(&tmp_db)?.0 != m.db_hash {
            return Err(AppError::invalid("The backup's database is damaged, so nothing was restored."));
        }
        let mut conn = db.lock().unwrap();
        let tables = super::merge_snapshot(&mut conn, &tmp_db, MediaRows::WithFile)?;
        for h in &skipped {
            conn.execute("UPDATE media SET availability = 'missing', has_thumb = 0 WHERE hash = ?1 AND availability <> 'local'", [h])?;
        }
        report.added_rows = tables.iter().map(|(_, n)| n).sum();
        report.tables = tables;
        Ok(())
    })();
    let _ = fs::remove_file(&tmp_db);
    res?;
    progress(total, total);
    Ok(report)
}

/// Where the cloud drives on this PC keep their synced folder (OneDrive, Google Drive,
/// iCloud Drive, Dropbox). Backing up into one of these puts the (encrypted) backup in that
/// person's own cloud, with nothing to sign in to inside Dear Us.
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CloudFolder {
    pub provider: String,
    pub path: String,
}

pub fn cloud_folders() -> Vec<CloudFolder> {
    let mut out: Vec<CloudFolder> = Vec::new();
    let mut add = |provider: &str, p: PathBuf| {
        if p.is_dir() && !out.iter().any(|c| c.path == p.display().to_string()) {
            out.push(CloudFolder { provider: provider.into(), path: p.display().to_string() });
        }
    };
    for var in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
        if let Some(p) = std::env::var_os(var) {
            add("OneDrive", PathBuf::from(p));
        }
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    if let Some(h) = &home {
        add("OneDrive", h.join("OneDrive"));
        add("iCloud Drive", h.join("iCloudDrive"));
        add("Google Drive", h.join("Google Drive"));
        add("Google Drive", h.join("My Drive"));
        add("Dropbox", h.join("Dropbox"));
    }
    // Google Drive for desktop mounts a drive letter (usually G:) with "My Drive" inside
    if cfg!(windows) {
        for letter in b'D'..=b'Z' {
            add("Google Drive", PathBuf::from(format!("{}:\\My Drive", letter as char)));
        }
    }
    // Dropbox can live elsewhere: its own info file says where
    if let Some(local) = std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("APPDATA")) {
        if let Ok(raw) = fs::read(PathBuf::from(local).join("Dropbox").join("info.json")) {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&raw) {
                for k in ["personal", "business"] {
                    if let Some(p) = v[k]["path"].as_str() {
                        add("Dropbox", PathBuf::from(p));
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_files_roundtrip_and_detect_tampering() {
        let dir = tempfile::tempdir().unwrap();
        let key = SpaceKey::generate();
        for size in [0usize, 10, PIECE, PIECE + 1, 2 * PIECE + 7] {
            let src = dir.path().join(format!("src{size}"));
            let data: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
            fs::write(&src, &data).unwrap();
            let sealed = dir.path().join(format!("s{size}.dub"));
            seal_file(&key, &src, &sealed, "label").unwrap();
            assert!(!fs::read(&sealed).unwrap().windows(10.min(size.max(1))).any(|w| size >= 10 && w == &data[..10]), "no plaintext");
            let out = dir.path().join(format!("o{size}"));
            open_file(&key, &sealed, &out, "label").unwrap();
            assert_eq!(fs::read(&out).unwrap(), data, "size {size}");
            assert!(open_file(&key, &sealed, &out, "another file").is_err(), "bound to its file");
            assert!(open_file(&SpaceKey::generate(), &sealed, &out, "label").is_err(), "wrong key");
        }
        // cutting the last piece off is detected
        let src = dir.path().join("big");
        fs::write(&src, vec![7u8; PIECE + 100]).unwrap();
        let sealed = dir.path().join("big.dub");
        seal_file(&key, &src, &sealed, "x").unwrap();
        let bytes = fs::read(&sealed).unwrap();
        let first_len = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        fs::write(&sealed, &bytes[..8 + first_len]).unwrap();
        assert!(open_file(&key, &sealed, &dir.path().join("cut"), "x").is_err(), "truncation detected");
    }
}
