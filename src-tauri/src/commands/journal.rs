use crate::db::repo::journal::{self, EntryDetail, EntryInput, EntryQuery, EntrySummary, TagCount, TrashItem};
use crate::db::repo::milestones::{self, MilestoneInput, TimelineItem};
use crate::error::{AppError, AppResult};
use crate::sync::oplog::Writer;
use crate::AppState;
use rusqlite::Connection;
use tauri::State;

/// Who is writing, from which device.
pub fn ctx(conn: &Connection) -> AppResult<(Writer, String)> {
    let device_id = crate::sync::device_id(conn)?;
    let me: String = conn
        .query_row("SELECT id FROM users WHERE is_self = 1", [], |r| r.get(0))
        .map_err(|_| AppError::invalid("set up your profile first"))?;
    Ok((Writer { device_id }, me))
}

#[tauri::command]
pub fn journal_list(state: State<AppState>, query: EntryQuery) -> AppResult<Vec<EntrySummary>> {
    journal::list(&state.db.lock().unwrap(), &query)
}

#[tauri::command]
pub fn journal_get(state: State<AppState>, id: String) -> AppResult<EntryDetail> {
    journal::get(&state.db.lock().unwrap(), &id)
}

#[tauri::command]
pub fn journal_create(state: State<AppState>, input: EntryInput) -> AppResult<EntryDetail> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    journal::create(&mut conn, &w, &me, &input)
}

#[tauri::command]
pub fn journal_update(state: State<AppState>, id: String, input: EntryInput) -> AppResult<EntryDetail> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    journal::update(&mut conn, &w, &id, &input)
}

#[tauri::command]
pub fn journal_delete(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    journal::delete(&mut conn, &w, &id)
}

#[tauri::command]
pub fn journal_restore(state: State<AppState>, id: String) -> AppResult<EntryDetail> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    journal::restore(&mut conn, &w, &id)
}

#[tauri::command]
pub fn journal_purge(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    journal::purge(&mut conn, &w, &id)
}

#[tauri::command]
pub fn journal_trash(state: State<AppState>) -> AppResult<Vec<TrashItem>> {
    journal::trash(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn journal_tags(state: State<AppState>) -> AppResult<Vec<TagCount>> {
    journal::tags(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn journal_on_this_day(state: State<AppState>, today: String) -> AppResult<Vec<EntrySummary>> {
    journal::on_this_day(&state.db.lock().unwrap(), &today)
}

#[tauri::command]
pub fn timeline(state: State<AppState>, major_only: bool) -> AppResult<Vec<TimelineItem>> {
    milestones::timeline(&state.db.lock().unwrap(), major_only)
}

#[tauri::command]
pub fn milestone_create(state: State<AppState>, input: MilestoneInput) -> AppResult<String> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    milestones::create(&mut conn, &w, &me, &input)
}

#[tauri::command]
pub fn milestone_update(state: State<AppState>, id: String, input: MilestoneInput) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    milestones::update(&mut conn, &w, &id, &input)
}

#[tauri::command]
pub fn milestone_delete(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    milestones::delete(&mut conn, &w, &id)
}
