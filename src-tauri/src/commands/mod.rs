//! Thin IPC layer: validate → call repo → return. No business logic here.

use crate::db::repo::{dashboard, profile, settings};
use crate::error::AppResult;
use crate::AppState;
use serde::Serialize;
use std::collections::HashMap;
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    data_dir: String,
    schema_version: i64,
    /// "personal" (bring-your-own Supabase, GitHub updates) or "store" (Microsoft Store).
    edition: &'static str,
}

#[tauri::command]
pub fn app_info(state: State<AppState>, app: tauri::AppHandle) -> AppResult<AppInfo> {
    let conn = state.db.lock().unwrap();
    let schema_version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    Ok(AppInfo {
        version: app.package_info().version.to_string(),
        data_dir: state.data_dir.display().to_string(),
        schema_version,
        edition: crate::sync::backend::EDITION,
    })
}

#[tauri::command]
pub fn settings_get_all(state: State<AppState>) -> AppResult<HashMap<String, String>> {
    settings::get_all(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn settings_set(state: State<AppState>, key: String, value: String) -> AppResult<()> {
    settings::set(&state.db.lock().unwrap(), &key, &value)
}

#[tauri::command]
pub fn profile_get(state: State<AppState>) -> AppResult<profile::Profile> {
    profile::get(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn profile_save(state: State<AppState>, app: tauri::AppHandle, input: profile::ProfileInput) -> AppResult<profile::Profile> {
    let mut conn = state.db.lock().unwrap();
    let p = profile::save(&mut conn, &input)?;
    // names and the start date are shared with the partner
    crate::sync::identity::record_profile_ops(&mut conn)?;
    drop(conn);
    use tauri::Manager;
    if let Some(s) = app.try_state::<crate::sync::service::SyncService>() {
        s.kick();
    }
    Ok(p)
}

#[tauri::command]
pub fn dashboard_summary(state: State<AppState>) -> AppResult<dashboard::DashboardSummary> {
    dashboard::summary(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn open_data_dir(state: State<AppState>) -> AppResult<()> {
    std::process::Command::new("explorer").arg(&state.data_dir).spawn()?;
    Ok(())
}

pub mod bucket;
pub mod engagement;
pub mod journal;
pub mod letters;
pub mod media;
pub mod plan;
pub mod planner;
pub mod questions;
pub mod special;
pub mod sync;
pub mod backup;
pub mod cupid;
