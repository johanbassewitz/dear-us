use crate::backup::{self, BackupResult, RestoreReport, VerifyReport};
use crate::db::repo::bin::{self, BinItem};
use crate::error::{AppError, AppResult};
use crate::sync::apply::meta_set;
use crate::AppState;
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use serde_json::json;
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

fn get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT value FROM local_meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupStatus {
    folder: Option<String>,
    auto: bool,
    last_at: Option<String>,
    last_ok: Option<bool>,
    last_error: Option<String>,
    last_copied: Option<i64>,
    last_total: Option<i64>,
    local_snapshots: usize,
}

#[tauri::command]
pub fn backup_status(state: State<AppState>) -> AppResult<BackupStatus> {
    let conn = state.db.lock().unwrap();
    let last: Option<serde_json::Value> = get(&conn, "backup.last")?.and_then(|s| serde_json::from_str(&s).ok());
    let snaps = std::fs::read_dir(state.data_dir.join("backups")).map(|r| r.flatten().count()).unwrap_or(0);
    Ok(BackupStatus {
        folder: get(&conn, "backup.folder")?,
        auto: get(&conn, "backup.auto")?.as_deref() == Some("1"),
        last_at: last.as_ref().and_then(|l| l["at"].as_str().map(String::from)),
        last_ok: last.as_ref().and_then(|l| l["ok"].as_bool()),
        last_error: last.as_ref().and_then(|l| l["error"].as_str().map(String::from)),
        last_copied: last.as_ref().and_then(|l| l["copied"].as_i64()),
        last_total: last.as_ref().and_then(|l| l["total"].as_i64()),
        local_snapshots: snaps,
    })
}

#[tauri::command]
pub fn backup_set_folder(state: State<AppState>, path: String) -> AppResult<()> {
    let p = PathBuf::from(&path);
    if !p.is_dir() {
        return Err(AppError::invalid("That folder doesn't exist."));
    }
    // can we write there?
    let probe = p.join(".dearus-write-test");
    std::fs::write(&probe, b"ok").map_err(|_| AppError::invalid("Dear Us can't write to that folder. Pick another one."))?;
    let _ = std::fs::remove_file(probe);
    let conn = state.db.lock().unwrap();
    meta_set(&conn, "backup.folder", &path)?;
    if get(&conn, "backup.auto")?.is_none() {
        meta_set(&conn, "backup.auto", "1")?; // daily backups on by default once a folder is chosen
    }
    Ok(())
}

#[tauri::command]
pub fn backup_set_auto(state: State<AppState>, on: bool) -> AppResult<()> {
    meta_set(&state.db.lock().unwrap(), "backup.auto", if on { "1" } else { "0" })
}

fn do_backup(app: &AppHandle) -> AppResult<BackupResult> {
    let state = app.state::<AppState>();
    let folder = get(&state.db.lock().unwrap(), "backup.folder")?.ok_or_else(|| AppError::invalid("Choose a backup folder first."))?;
    let version = app.package_info().version.to_string();
    let a = app.clone();
    let res = backup::run_backup(&state.db, &state.store, &PathBuf::from(&folder), &version, &move |done, total| {
        let _ = a.emit("backup-progress", json!({ "done": done, "total": total }));
    });
    let conn = state.db.lock().unwrap();
    let record = match &res {
        Ok(r) => json!({ "at": r.created_at, "ok": true, "copied": r.copied_files, "total": r.total_files }),
        Err(e) => json!({ "at": crate::db::now_iso(), "ok": false, "error": e.to_string() }),
    };
    meta_set(&conn, "backup.last", &record.to_string())?;
    drop(conn);
    let _ = app.emit("backup-status", ());
    res
}

#[tauri::command]
pub async fn backup_run(app: AppHandle) -> AppResult<BackupResult> {
    let a = app.clone();
    tauri::async_runtime::spawn_blocking(move || do_backup(&a)).await.map_err(|e| AppError::invalid(e.to_string()))?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    created_at: String,
    space_name: Option<String>,
    entries: i64,
    files: usize,
    app_version: String,
}

#[tauri::command]
pub fn backup_inspect(path: String) -> AppResult<BackupInfo> {
    let (_, m) = backup::read_manifest(&PathBuf::from(path))?;
    Ok(BackupInfo { created_at: m.created_at, space_name: m.space_name, entries: m.entries, files: m.media.len(), app_version: m.app_version })
}

#[tauri::command]
pub async fn backup_verify(path: String) -> AppResult<VerifyReport> {
    tauri::async_runtime::spawn_blocking(move || backup::verify_backup(&PathBuf::from(path))).await.map_err(|e| AppError::invalid(e.to_string()))?
}

#[tauri::command]
pub async fn archive_verify(app: AppHandle) -> AppResult<VerifyReport> {
    let a = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let s = a.state::<AppState>();
        backup::verify_archive(&s.db, &s.store)
    })
    .await
    .map_err(|e| AppError::invalid(e.to_string()))?
}

#[tauri::command]
pub async fn backup_restore(app: AppHandle, path: String) -> AppResult<RestoreReport> {
    let a = app.clone();
    let report = tauri::async_runtime::spawn_blocking(move || -> AppResult<RestoreReport> {
        let s = a.state::<AppState>();
        // a safety copy of the current database first, just in case
        let _ = backup::local_snapshot(&s.db, &s.data_dir);
        let b = a.clone();
        let rep = backup::restore(&s.db, &s.store, &PathBuf::from(path), &move |done, total| {
            let _ = b.emit("backup-progress", json!({ "done": done, "total": total }));
        })?;
        regenerate_previews(&a);
        Ok(rep)
    })
    .await
    .map_err(|e| AppError::invalid(e.to_string()))??;
    let _ = app.emit("data-changed", ());
    Ok(report)
}

/// Thumbnails/previews for restored photos that don't have them yet.
pub(crate) fn regenerate_previews(app: &AppHandle) {
    let s = app.state::<AppState>();
    let rows: Vec<(String, String, String, String)> = {
        let conn = s.db.lock().unwrap();
        let Ok(mut st) = conn.prepare("SELECT id, hash, ext, mime FROM media WHERE availability = 'local' AND (has_thumb = 0 OR preview IN ('image','video'))") else { return };
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).and_then(|m| m.collect()).unwrap_or_default()
    };
    for (id, hash, ext, mime) in rows {
        let thumb_ok = s.store.thumb(&id).exists();
        let preview_ok = s.store.preview_image(&id).exists() || s.store.preview_video(&id).exists();
        if thumb_ok && preview_ok {
            continue;
        }
        let d = crate::media::derive_for(&s.store, &id, &s.store.original(&hash, &ext), &mime);
        let conn = s.db.lock().unwrap();
        let _ = crate::db::repo::media::set_thumb(&conn, &id, d.has_thumb);
        let _ = crate::db::repo::media::set_preview(&conn, &id, d.preview);
        drop(conn);
        if d.preview == "needed" {
            if let Some(q) = app.try_state::<crate::media::worker::PreviewQueue>() {
                q.push(id.clone());
            }
        }
    }
}

/// Daily safety copy of the database + the automatic backup (if a folder is set).
pub fn start_scheduler(app: AppHandle) {
    std::thread::Builder::new()
        .name("backups".into())
        .spawn(move || {
            std::thread::sleep(Duration::from_secs(90)); // let the app settle first
            loop {
                let s = app.state::<AppState>();
                let _ = backup::local_snapshot(&s.db, &s.data_dir);
                let due = {
                    let conn = s.db.lock().unwrap();
                    let auto = get(&conn, "backup.auto").ok().flatten().as_deref() == Some("1");
                    let folder = get(&conn, "backup.folder").ok().flatten();
                    let last: Option<String> = get(&conn, "backup.last").ok().flatten().and_then(|l| {
                        serde_json::from_str::<serde_json::Value>(&l).ok().filter(|v| v["ok"] == true).and_then(|v| v["at"].as_str().map(String::from))
                    });
                    let stale = last
                        .and_then(|at| chrono::DateTime::parse_from_rfc3339(&at).ok())
                        .map(|t| chrono::Utc::now().signed_duration_since(t) > chrono::Duration::hours(24))
                        .unwrap_or(true);
                    auto && folder.is_some_and(|f| PathBuf::from(f).is_dir()) && stale
                };
                if due {
                    let _ = do_backup(&app);
                }
                std::thread::sleep(Duration::from_secs(30 * 60));
            }
        })
        .expect("could not start backups");
}

#[tauri::command]
pub fn bin_list(state: State<AppState>) -> AppResult<Vec<BinItem>> {
    let conn = state.db.lock().unwrap();
    let me = crate::sync::identity::my_user_id(&conn)?.unwrap_or_default();
    bin::list(&conn, &me)
}

#[tauri::command]
pub fn bin_restore(state: State<AppState>, entity: String, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = crate::commands::journal::ctx(&conn)?;
    bin::restore(&mut conn, &w, &entity, &id)
}

#[tauri::command]
pub fn bin_purge(state: State<AppState>, entity: String, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = crate::commands::journal::ctx(&conn)?;
    bin::purge(&mut conn, &w, &entity, &id)
}
