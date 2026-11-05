use crate::error::{AppError, AppResult};
use crate::media::{self, ffmpeg, worker::PreviewQueue, ImportResult};
use crate::AppState;
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Imports files chosen by the user (file picker or drag & drop).
#[tauri::command]
pub async fn media_import(app: AppHandle, paths: Vec<String>) -> AppResult<Vec<ImportResult>> {
    if paths.len() > 500 {
        return Err(AppError::invalid("please add at most 500 files at once"));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let me: Option<String> = state
            .db
            .lock()
            .unwrap()
            .query_row("SELECT id FROM users WHERE is_self = 1", [], |r| r.get(0))
            .ok();
        let queue = app.state::<PreviewQueue>();
        paths
            .into_iter()
            .map(|p| match media::import_one(&state.store, &state.db, &PathBuf::from(&p), me.clone()) {
                Ok((row, duplicate)) => {
                    if row.r.preview == "needed" {
                        queue.push(row.r.id.clone());
                    }
                    ImportResult { path: p, media: Some(row.r), error: None, duplicate }
                }
                Err(e) => ImportResult { path: p, media: None, error: Some(e.to_string()), duplicate: false },
            })
            .collect()
    })
    .await
    .map_err(|e| AppError::invalid(format!("import stopped unexpectedly: {e}")))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaCapabilities {
    /// ffmpeg found: iPhone HEIC photos and HEVC/.mov videos can be previewed.
    pub converter: bool,
}

#[tauri::command]
pub fn media_capabilities() -> MediaCapabilities {
    MediaCapabilities { converter: ffmpeg::tools().is_some() }
}

/// One-click converter install: downloads the pinned ffmpeg build into `<data>/tools`,
/// then makes previews for photos/videos that couldn't be shown before.
/// Emits `converter-progress` with { done, total } while downloading.
#[tauri::command]
pub async fn media_install_converter(app: tauri::AppHandle) -> AppResult<MediaCapabilities> {
    use tauri::Emitter;
    let a = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        ffmpeg::install(|done, total| {
            let _ = a.emit("converter-progress", serde_json::json!({ "done": done, "total": total }));
        })
    })
    .await
    .map_err(|e| AppError::invalid(e.to_string()))?
    .map_err(AppError::invalid)?;
    let a = app.clone();
    std::thread::spawn(move || {
        crate::commands::backup::regenerate_previews(&a);
        let _ = tauri::Emitter::emit(&a, "data-changed", ());
    });
    Ok(MediaCapabilities { converter: true })
}

#[tauri::command]
pub fn media_get(state: tauri::State<AppState>, id: String) -> AppResult<crate::db::repo::media::MediaRef> {
    crate::db::repo::media::get(&state.db.lock().unwrap(), &id)?
        .map(|r| r.r)
        .ok_or_else(|| AppError::invalid("not found"))
}
