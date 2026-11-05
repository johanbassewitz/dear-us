//! Backups & recovery (docs/PLAN.md §9).
//!
//! A backup is a plain folder anyone can copy (external drive, the couple's own OneDrive…):
//!
//!   DearUsBackup/
//!     manifest.json            what's inside + BLAKE3 hash of every file
//!     db/2026-11-04T16-00-00.db  consistent database snapshots (last 10 kept)
//!     media/ab/<hash>.<ext>    originals, copied once (content-addressed)
//!     thumbs/<media id>.jpg    small previews, so a restore looks right immediately
//!
//! A backup only counts as complete after every file in it was re-read and verified.
//! Restoring never overwrites what's already on this PC: it adds what's missing.
//! Secrets (sync sign-in, space key) are never in a backup.

use crate::db::now_iso;
use crate::error::{AppError, AppResult};
use crate::media::Store;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const FORMAT: u32 = 1;
const KEEP_DB_SNAPSHOTS: usize = 10;
const KEEP_LOCAL_SNAPSHOTS: usize = 7;
const DIR_NAME: &str = "DearUsBackup";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MediaFile {
    pub hash: String,
    pub ext: String,
    pub size: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: u32,
    pub app_version: String,
    pub schema_version: i64,
    pub created_at: String,
    pub space_name: Option<String>,
    pub db_file: String,
    pub db_hash: String,
    pub db_size: i64,
    pub media: Vec<MediaFile>,
    pub entries: i64,
}

#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct VerifyReport {
    pub ok: bool,
    pub checked_files: usize,
    pub missing: Vec<String>,
    pub damaged: Vec<String>,
    pub db_ok: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BackupResult {
    pub folder: String,
    pub created_at: String,
    pub copied_files: usize,
    pub copied_bytes: i64,
    pub total_files: usize,
    pub verify: VerifyReport,
}

pub fn blake3_file(path: &Path) -> AppResult<(String, i64)> {
    let mut f = File::open(path)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut n = 0i64;
    loop {
        let k = f.read(&mut buf)?;
        if k == 0 {
            break;
        }
        h.update(&buf[..k]);
        n += k as i64;
    }
    Ok((h.finalize().to_hex().to_string(), n))
}

/// `chosen` may be the backup folder itself or its parent.
pub fn backup_dir(chosen: &Path) -> PathBuf {
    if chosen.file_name().is_some_and(|n| n == DIR_NAME) || chosen.join("manifest.json").exists() {
        chosen.to_path_buf()
    } else {
        chosen.join(DIR_NAME)
    }
}

fn stamp() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S").to_string()
}

/// Consistent copy of the live database (safe while the app runs).
fn snapshot_db(db: &Mutex<Connection>, dest: &Path) -> AppResult<()> {
    if dest.exists() {
        fs::remove_file(dest)?;
    }
    let conn = db.lock().unwrap();
    conn.execute("VACUUM INTO ?1", [dest.to_string_lossy()])?;
    Ok(())
}

fn copy_verified(src: &Path, dest: &Path, expect_hash: Option<&str>) -> AppResult<i64> {
    fs::create_dir_all(dest.parent().unwrap())?;
    let tmp = dest.with_extension("copying");
    {
        let mut i = File::open(src)?;
        let mut o = File::create(&tmp)?;
        std::io::copy(&mut i, &mut o)?;
        o.flush()?;
        o.sync_all()?;
    }
    if let Some(h) = expect_hash {
        let (got, _) = blake3_file(&tmp)?;
        if got != h {
            let _ = fs::remove_file(&tmp);
            return Err(AppError::invalid(format!("a file changed while copying ({})", src.display())));
        }
    }
    fs::rename(&tmp, dest)?;
    Ok(fs::metadata(dest)?.len() as i64)
}

fn media_path(dir: &Path, m: &MediaFile) -> PathBuf {
    dir.join("media").join(&m.hash[0..2]).join(format!("{}.{}", m.hash, m.ext))
}

/// Writes (or updates) a backup in `chosen`. Media already in the backup isn't copied again.
pub fn run_backup(db: &Mutex<Connection>, store: &Store, chosen: &Path, app_version: &str, progress: &dyn Fn(usize, usize)) -> AppResult<BackupResult> {
    let dir = backup_dir(chosen);
    fs::create_dir_all(dir.join("db"))?;
    fs::create_dir_all(dir.join("media"))?;
    fs::create_dir_all(dir.join("thumbs"))?;

    let (media, thumbs, schema, space, entries) = {
        let conn = db.lock().unwrap();
        let mut s = conn.prepare("SELECT id, hash, ext, size, has_thumb FROM media WHERE availability = 'local' AND hash NOT LIKE 'pending:%'")?;
        let rows: Vec<(String, String, String, i64, bool)> =
            s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, i64>(4)? != 0)))?.collect::<Result<_, _>>()?;
        let schema: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let space: Option<String> = conn.query_row("SELECT name FROM space LIMIT 1", [], |r| r.get(0)).optional()?;
        let entries: i64 = conn.query_row("SELECT COUNT(*) FROM journal_entries WHERE deleted_at IS NULL", [], |r| r.get(0))?;
        let media: Vec<MediaFile> = rows.iter().map(|(_, h, e, s, _)| MediaFile { hash: h.clone(), ext: e.clone(), size: *s }).collect();
        let thumbs: Vec<String> = rows.iter().filter(|r| r.4).map(|r| r.0.clone()).collect();
        (media, thumbs, schema, space, entries)
    };

    let total = media.len() + 1;
    let mut copied = 0usize;
    let mut bytes = 0i64;
    for (i, m) in media.iter().enumerate() {
        progress(i, total);
        let dest = media_path(&dir, m);
        if dest.exists() && fs::metadata(&dest).map(|x| x.len() as i64 == m.size).unwrap_or(false) {
            continue; // copied by an earlier backup (verified below)
        }
        let src = store.original(&m.hash, &m.ext);
        if !src.exists() {
            return Err(AppError::invalid(format!("a photo/video is missing from this PC's archive ({}); run Verify", m.hash)));
        }
        bytes += copy_verified(&src, &dest, Some(&m.hash))?;
        copied += 1;
    }
    for id in &thumbs {
        let src = store.thumb(id);
        let dest = dir.join("thumbs").join(format!("{id}.jpg"));
        if src.exists() && !dest.exists() {
            let _ = copy_verified(&src, &dest, None);
        }
    }

    let created_at = now_iso();
    let db_file = format!("db/{}.db", stamp());
    snapshot_db(db, &dir.join(&db_file))?;
    let (db_hash, db_size) = blake3_file(&dir.join(&db_file))?;
    progress(total, total);

    let manifest = Manifest {
        format: FORMAT,
        app_version: app_version.into(),
        schema_version: schema,
        created_at: created_at.clone(),
        space_name: space,
        db_file,
        db_hash,
        db_size,
        media: media.clone(),
        entries,
    };
    // Not "complete" until every file has been read back and checked.
    let verify = verify_with(&dir, &manifest)?;
    if !verify.ok {
        return Err(AppError::invalid(format!(
            "The backup couldn't be verified ({} missing, {} damaged). Your memories on this PC are fine. Check the backup drive and try again.",
            verify.missing.len(),
            verify.damaged.len()
        )));
    }
    let tmp = dir.join("manifest.json.writing");
    fs::write(&tmp, serde_json::to_vec_pretty(&manifest)?)?;
    fs::rename(&tmp, dir.join("manifest.json"))?;
    prune_snapshots(&dir.join("db"), KEEP_DB_SNAPSHOTS, &manifest.db_file)?;

    Ok(BackupResult { folder: dir.display().to_string(), created_at, copied_files: copied, copied_bytes: bytes, total_files: media.len(), verify })
}

fn prune_snapshots(folder: &Path, keep: usize, current: &str) -> AppResult<()> {
    let mut snaps: Vec<PathBuf> = fs::read_dir(folder)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "db")).collect();
    snaps.sort();
    let current_name = Path::new(current).file_name().map(|n| n.to_os_string());
    while snaps.len() > keep {
        let old = snaps.remove(0);
        if Some(old.file_name().unwrap().to_os_string()) != current_name {
            let _ = fs::remove_file(old);
        }
    }
    Ok(())
}

pub fn read_manifest(chosen: &Path) -> AppResult<(PathBuf, Manifest)> {
    let dir = backup_dir(chosen);
    let raw = fs::read(dir.join("manifest.json")).map_err(|_| AppError::invalid("There's no Dear Us backup in that folder."))?;
    let m: Manifest = serde_json::from_slice(&raw).map_err(|_| AppError::invalid("The backup's manifest is damaged."))?;
    if m.format > FORMAT {
        return Err(AppError::invalid("This backup was made by a newer version of Dear Us. Update the app first."));
    }
    Ok((dir, m))
}

fn verify_with(dir: &Path, m: &Manifest) -> AppResult<VerifyReport> {
    let mut r = VerifyReport::default();
    let dbp = dir.join(&m.db_file);
    match blake3_file(&dbp) {
        Ok((h, _)) if h == m.db_hash => {
            let c = Connection::open_with_flags(&dbp, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            let ok: String = c.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
            r.db_ok = ok == "ok";
            if !r.db_ok {
                r.damaged.push(m.db_file.clone());
            }
        }
        Ok(_) => r.damaged.push(m.db_file.clone()),
        Err(_) => r.missing.push(m.db_file.clone()),
    }
    for f in &m.media {
        let p = media_path(dir, f);
        r.checked_files += 1;
        match blake3_file(&p) {
            Ok((h, n)) if h == f.hash && n == f.size => {}
            Ok(_) => r.damaged.push(f.hash.clone()),
            Err(_) => r.missing.push(f.hash.clone()),
        }
    }
    r.ok = r.missing.is_empty() && r.damaged.is_empty() && r.db_ok;
    Ok(r)
}

pub fn verify_backup(chosen: &Path) -> AppResult<VerifyReport> {
    let (dir, m) = read_manifest(chosen)?;
    verify_with(&dir, &m)
}

/// Checks this PC's own archive: database integrity + every original against its hash.
pub fn verify_archive(db: &Mutex<Connection>, store: &Store) -> AppResult<VerifyReport> {
    let (rows, db_ok) = {
        let conn = db.lock().unwrap();
        let ok: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        let mut s = conn.prepare("SELECT hash, ext FROM media WHERE availability = 'local' AND hash NOT LIKE 'pending:%'")?;
        let rows: Vec<(String, String)> = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        (rows, ok == "ok")
    };
    let mut r = VerifyReport { db_ok, ..Default::default() };
    for (hash, ext) in rows {
        r.checked_files += 1;
        match blake3_file(&store.original(&hash, &ext)) {
            Ok((h, _)) if h == hash => {}
            Ok(_) => r.damaged.push(hash),
            Err(_) => r.missing.push(hash),
        }
    }
    r.ok = r.db_ok && r.missing.is_empty() && r.damaged.is_empty();
    Ok(r)
}

#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    pub added_rows: i64,
    pub added_files: usize,
    pub skipped_existing_files: usize,
    pub tables: Vec<(String, i64)>,
}

/// Tables that are this install's own bookkeeping — never restored from a backup.
const LOCAL_ONLY: &[&str] = &["local_meta", "oplog", "sync_applied", "sync_inbox", "sync_outbox", "sync_blobs", "sync_conflicts", "media_alias", "journal_fts", "cupid_chats"];

/// Adds everything from the backup that this PC doesn't have. Never overwrites or deletes.
pub fn restore(db: &Mutex<Connection>, store: &Store, chosen: &Path, progress: &dyn Fn(usize, usize)) -> AppResult<RestoreReport> {
    let (dir, m) = read_manifest(chosen)?;
    let v = verify_with(&dir, &m)?;
    if !v.db_ok {
        return Err(AppError::invalid("The backup's database is damaged, so nothing was restored. Try an older backup folder or another copy."));
    }
    let app_schema: i64 = db.lock().unwrap().query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if m.schema_version > app_schema {
        return Err(AppError::invalid("This backup is from a newer version of Dear Us. Update the app first."));
    }
    let mut report = RestoreReport::default();

    // 1) files first, so restored records never point at missing photos
    let total = m.media.len();
    for (i, f) in m.media.iter().enumerate() {
        progress(i, total);
        if v.missing.contains(&f.hash) || v.damaged.contains(&f.hash) {
            continue; // reported; never import a damaged file
        }
        let dest = store.original(&f.hash, &f.ext);
        if dest.exists() {
            report.skipped_existing_files += 1;
            continue;
        }
        copy_verified(&media_path(&dir, f), &dest, Some(&f.hash))?;
        report.added_files += 1;
    }

    // 2) rows: insert only what's missing, column by column for older backups
    {
        let mut conn = db.lock().unwrap();
        let snap = dir.join(&m.db_file);
        conn.execute("ATTACH DATABASE ?1 AS bak", [snap.to_string_lossy()])?;
        let res = (|| -> AppResult<()> {
            let tx = conn.transaction()?;
            let tables: Vec<String> = {
                let mut s = tx.prepare("SELECT name FROM bak.sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '%fts%'")?;
                let r = s.query_map([], |r| r.get(0))?;
                r.collect::<Result<_, _>>()?
            };
            // parents before children (foreign keys)
            let order = ["users", "space", "media", "journal_entries"];
            let mut sorted = tables.clone();
            sorted.sort_by_key(|t| order.iter().position(|o| o == t).unwrap_or(order.len()));
            for t in sorted.iter().filter(|t| !LOCAL_ONLY.contains(&t.as_str())) {
                let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM main.sqlite_master WHERE type='table' AND name=?1)", [t], |r| r.get(0))?;
                if !exists {
                    continue;
                }
                let cols = |schema: &str| -> AppResult<Vec<String>> {
                    let mut s = tx.prepare(&format!("PRAGMA {schema}.table_info(\"{t}\")"))?;
                    let r = s.query_map([], |r| r.get::<_, String>(1))?;
                    Ok(r.collect::<Result<_, _>>()?)
                };
                let main_cols = cols("main")?;
                let common: Vec<String> = cols("bak")?.into_iter().filter(|c| main_cols.contains(c)).map(|c| format!("\"{c}\"")).collect();
                if common.is_empty() {
                    continue;
                }
                let list = common.join(",");
                // a restored photo row is only "here" if its file made it
                let extra = if t == "media" { " WHERE availability = 'local'" } else { "" };
                let n = tx.execute(&format!("INSERT OR IGNORE INTO main.\"{t}\"({list}) SELECT {list} FROM bak.\"{t}\"{extra}"), [])?;
                if t == "media" {
                    // files that were missing/damaged in the backup: honest placeholder, not "local"
                    for f in m.media.iter().filter(|f| (v.missing.contains(&f.hash) || v.damaged.contains(&f.hash)) && !store.original(&f.hash, &f.ext).exists()) {
                        tx.execute("UPDATE main.media SET availability = 'missing', has_thumb = 0 WHERE hash = ?1", [&f.hash])?;
                    }
                }
                if n > 0 {
                    report.tables.push((t.clone(), n as i64));
                    report.added_rows += n as i64;
                }
            }
            // search index for everything that's live
            tx.execute("DELETE FROM journal_fts", [])?;
            let mut s = tx.prepare("SELECT id, title, body_text, tags_json, COALESCE(location,'') FROM journal_entries WHERE deleted_at IS NULL")?;
            let rows: Vec<(String, String, String, String, String)> =
                s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<Result<_, _>>()?;
            drop(s);
            for (id, title, body, tags, loc) in rows {
                let tags: Vec<String> = serde_json::from_str(&tags).unwrap_or_default();
                tx.execute("INSERT INTO journal_fts(entry_id, title, body, tags, location) VALUES (?1,?2,?3,?4,?5)", params![id, title, body, tags.join(" "), loc])?;
            }
            tx.commit()?;
            Ok(())
        })();
        conn.execute("DETACH DATABASE bak", [])?;
        res?;
    }

    // 3) thumbnails come along (regenerated later if missing)
    if let Ok(rd) = fs::read_dir(dir.join("thumbs")) {
        for e in rd.flatten() {
            let dest = store.thumb(&e.path().file_stem().unwrap_or_default().to_string_lossy());
            if !dest.exists() {
                let _ = fs::copy(e.path(), dest);
            }
        }
    }
    progress(total, total);
    Ok(report)
}

/// Small rotating copies of the database inside the data folder (protects against a
/// damaged database even without a backup folder). Once a day.
pub fn local_snapshot(db: &Mutex<Connection>, data_dir: &Path) -> AppResult<Option<PathBuf>> {
    let folder = data_dir.join("backups");
    fs::create_dir_all(&folder)?;
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let exists = fs::read_dir(&folder)?.flatten().any(|e| e.file_name().to_string_lossy().starts_with(&today));
    if exists {
        return Ok(None);
    }
    let dest = folder.join(format!("{}.db", stamp()));
    snapshot_db(db, &dest)?;
    prune_snapshots(&folder, KEEP_LOCAL_SNAPSHOTS, "")?;
    Ok(Some(dest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::journal::{self, tests as jt, EntryMediaInput, EntryQuery};

    struct Pc {
        _dir: tempfile::TempDir,
        db: Mutex<Connection>,
        store: Store,
    }
    fn pc() -> Pc {
        let dir = tempfile::tempdir().unwrap();
        let conn = crate::db::open(&dir.path().join("t.db")).unwrap();
        conn.execute("INSERT INTO users(id, display_name, is_self, created_at, updated_at) VALUES ('me','Jo',1,'t','t')", []).unwrap();
        let store = Store::new(dir.path()).unwrap();
        Pc { _dir: dir, db: Mutex::new(conn), store }
    }
    fn add_entry(p: &Pc, title: &str, with_photo: bool) -> String {
        let mut i = jt::input(title, "2025-07-12");
        if with_photo {
            let src = p._dir.path().join(format!("{title}.png"));
            image::RgbImage::from_fn(120, 80, |x, y| image::Rgb([x as u8, y as u8, title.len() as u8 * 20])).save(&src).unwrap();
            let (m, _) = crate::media::import_one(&p.store, &p.db, &src, None).unwrap();
            i.media = vec![EntryMediaInput { id: m.r.id, caption: None }];
        }
        let w = crate::sync::oplog::Writer { device_id: "dev".into() };
        journal::create(&mut p.db.lock().unwrap(), &w, "me", &i).unwrap().summary.id
    }
    fn titles(p: &Pc) -> Vec<String> {
        let mut t: Vec<String> = journal::list(&p.db.lock().unwrap(), &EntryQuery::default()).unwrap().into_iter().map(|e| e.title).collect();
        t.sort();
        t
    }

    #[test]
    fn backup_is_verified_incremental_and_restores_on_a_new_pc() {
        let a = pc();
        add_entry(&a, "Beach", true);
        let target = tempfile::tempdir().unwrap();
        let r1 = run_backup(&a.db, &a.store, target.path(), "test", &|_, _| {}).unwrap();
        assert!(r1.verify.ok);
        assert_eq!(r1.copied_files, 1);
        // incremental: the same photo isn't copied twice
        add_entry(&a, "Cinema", false);
        let r2 = run_backup(&a.db, &a.store, target.path(), "test", &|_, _| {}).unwrap();
        assert_eq!(r2.copied_files, 0);
        assert!(verify_backup(target.path()).unwrap().ok);

        // brand-new install: everything comes back, photo byte-identical, search works
        let b = {
            let dir = tempfile::tempdir().unwrap();
            let conn = crate::db::open(&dir.path().join("t.db")).unwrap();
            let store = Store::new(dir.path()).unwrap();
            Pc { _dir: dir, db: Mutex::new(conn), store }
        };
        let rep = restore(&b.db, &b.store, target.path(), &|_, _| {}).unwrap();
        assert_eq!(rep.added_files, 1);
        assert_eq!(titles(&b), vec!["Beach", "Cinema"]);
        assert_eq!(journal::list(&b.db.lock().unwrap(), &EntryQuery { text: Some("cinem".into()), ..Default::default() }).unwrap().len(), 1);
        assert!(verify_archive(&b.db, &b.store).unwrap().ok);
        let self_user: String = b.db.lock().unwrap().query_row("SELECT display_name FROM users WHERE is_self = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(self_user, "Jo");
    }

    #[test]
    fn restoring_an_older_backup_never_overwrites_newer_changes() {
        let a = pc();
        let id = add_entry(&a, "Original title", false);
        let target = tempfile::tempdir().unwrap();
        run_backup(&a.db, &a.store, target.path(), "test", &|_, _| {}).unwrap();
        // newer work after the backup
        let w = crate::sync::oplog::Writer { device_id: "dev".into() };
        let mut i = jt::input("Edited after the backup", "2025-07-12");
        i.body_json = String::new();
        journal::update(&mut a.db.lock().unwrap(), &w, &id, &i).unwrap();
        add_entry(&a, "Newer memory", false);
        let rep = restore(&a.db, &a.store, target.path(), &|_, _| {}).unwrap();
        assert_eq!(rep.added_rows, 0, "nothing was missing");
        assert_eq!(titles(&a), vec!["Edited after the backup", "Newer memory"]);
    }

    #[test]
    fn damaged_backup_files_are_detected_and_never_imported() {
        let a = pc();
        add_entry(&a, "Photo", true);
        let target = tempfile::tempdir().unwrap();
        run_backup(&a.db, &a.store, target.path(), "test", &|_, _| {}).unwrap();
        let (dir, m) = read_manifest(target.path()).unwrap();
        let f = media_path(&dir, &m.media[0]);
        let mut bytes = fs::read(&f).unwrap();
        bytes[10] ^= 0xff;
        fs::write(&f, bytes).unwrap();
        let v = verify_backup(target.path()).unwrap();
        assert!(!v.ok && v.damaged.len() == 1);

        let fresh = {
            let dir = tempfile::tempdir().unwrap();
            let conn = crate::db::open(&dir.path().join("t.db")).unwrap();
            let store = Store::new(dir.path()).unwrap();
            Pc { _dir: dir, db: Mutex::new(conn), store }
        };
        let rep = restore(&fresh.db, &fresh.store, target.path(), &|_, _| {}).unwrap();
        assert_eq!(rep.added_files, 0, "damaged file not imported");
        // and the record isn't marked as having a local photo that doesn't exist
        assert!(verify_archive(&fresh.db, &fresh.store).unwrap().ok);
    }

    #[test]
    fn verify_archive_catches_a_damaged_original() {
        let a = pc();
        add_entry(&a, "Photo", true);
        assert!(verify_archive(&a.db, &a.store).unwrap().ok);
        let (hash, ext): (String, String) = a.db.lock().unwrap().query_row("SELECT hash, ext FROM media", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        fs::write(a.store.original(&hash, &ext), b"broken").unwrap();
        let v = verify_archive(&a.db, &a.store).unwrap();
        assert_eq!(v.damaged.len(), 1);
    }

    #[test]
    fn local_snapshots_once_a_day() {
        let a = pc();
        let dir = tempfile::tempdir().unwrap();
        assert!(local_snapshot(&a.db, dir.path()).unwrap().is_some());
        assert!(local_snapshot(&a.db, dir.path()).unwrap().is_none());
    }
}
