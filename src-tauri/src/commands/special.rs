use crate::commands::journal::ctx;
use crate::db::repo::special::{self, SpecialDate, SpecialInput};
use crate::error::AppResult;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn special_list(state: State<AppState>) -> AppResult<Vec<SpecialDate>> {
    special::list(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn special_create(state: State<AppState>, input: SpecialInput) -> AppResult<String> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    special::create(&mut conn, &w, &me, &input)
}

#[tauri::command]
pub fn special_update(state: State<AppState>, id: String, input: SpecialInput) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    special::update(&mut conn, &w, &id, &input)
}

#[tauri::command]
pub fn special_delete(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    special::delete(&mut conn, &w, &id)
}

#[tauri::command]
pub fn special_restore(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    special::restore(&mut conn, &w, &id)
}
