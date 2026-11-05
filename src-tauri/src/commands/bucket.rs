use crate::commands::journal::ctx;
use crate::db::repo::bucket::{self, BucketInput, BucketItem, BucketTrashItem};
use crate::error::AppResult;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn bucket_list(state: State<AppState>) -> AppResult<Vec<BucketItem>> {
    bucket::list(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn bucket_create(state: State<AppState>, input: BucketInput) -> AppResult<String> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    bucket::create(&mut conn, &w, &me, &input)
}

#[tauri::command]
pub fn bucket_update(state: State<AppState>, id: String, input: BucketInput) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    bucket::update(&mut conn, &w, &id, &input)
}

#[tauri::command]
pub fn bucket_set_status(state: State<AppState>, id: String, status: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    bucket::set_status(&mut conn, &w, &id, &status)
}

#[tauri::command]
pub fn bucket_link_entry(state: State<AppState>, item_id: String, entry_id: Option<String>) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    bucket::link_entry(&mut conn, &w, &item_id, entry_id.as_deref())
}

#[tauri::command]
pub fn bucket_delete(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    bucket::delete(&mut conn, &w, &id)
}

#[tauri::command]
pub fn bucket_restore(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    bucket::restore(&mut conn, &w, &id)
}

#[tauri::command]
pub fn bucket_trash(state: State<AppState>) -> AppResult<Vec<BucketTrashItem>> {
    bucket::trash(&state.db.lock().unwrap())
}
