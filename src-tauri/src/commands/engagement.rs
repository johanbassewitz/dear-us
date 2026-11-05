use crate::commands::journal::ctx;
use crate::db::repo::engagement::{self, Comment, ReactionCount};
use crate::error::AppResult;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn reaction_list(state: State<AppState>, entry_id: String) -> AppResult<Vec<ReactionCount>> {
    let conn = state.db.lock().unwrap();
    let me: String = ctx(&conn)?.1;
    engagement::reactions(&conn, &entry_id, &me)
}

#[tauri::command]
pub fn reaction_toggle(state: State<AppState>, entry_id: String, kind: String) -> AppResult<Vec<ReactionCount>> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    engagement::toggle_reaction(&mut conn, &w, &entry_id, &me, &kind)
}

#[tauri::command]
pub fn comment_list(state: State<AppState>, entry_id: String) -> AppResult<Vec<Comment>> {
    engagement::comments(&state.db.lock().unwrap(), &entry_id)
}

#[tauri::command]
pub fn comment_add(state: State<AppState>, entry_id: String, body: String) -> AppResult<Comment> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    engagement::add_comment(&mut conn, &w, &entry_id, &me, &body)
}

#[tauri::command]
pub fn comment_update(state: State<AppState>, id: String, body: String) -> AppResult<Comment> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    engagement::update_comment(&mut conn, &w, &id, &me, &body)
}

#[tauri::command]
pub fn comment_delete(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    engagement::delete_comment(&mut conn, &w, &id, &me)
}

#[tauri::command]
pub fn comment_restore(state: State<AppState>, id: String) -> AppResult<Comment> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    engagement::restore_comment(&mut conn, &w, &id, &me)
}
