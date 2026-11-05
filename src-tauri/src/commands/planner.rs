use crate::commands::journal::ctx;
use crate::db::repo::planner::{self, Idea, IdeaInput, Plan, PlanInput};
use crate::error::AppResult;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn planner_list(state: State<AppState>) -> AppResult<Vec<Plan>> {
    planner::list(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn planner_create(state: State<AppState>, input: PlanInput) -> AppResult<String> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    planner::create(&mut conn, &w, &me, &input)
}

#[tauri::command]
pub fn planner_update(state: State<AppState>, id: String, input: PlanInput) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    planner::update(&mut conn, &w, &id, &input)
}

#[tauri::command]
pub fn planner_set_status(state: State<AppState>, id: String, status: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    planner::set_status(&mut conn, &w, &id, &status)
}

#[tauri::command]
pub fn planner_toggle_check(state: State<AppState>, id: String, item_id: String, done: bool) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    planner::toggle_check(&mut conn, &w, &id, &item_id, done)
}

#[tauri::command]
pub fn planner_link_entry(state: State<AppState>, id: String, entry_id: Option<String>) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    planner::link_entry(&mut conn, &w, &id, entry_id.as_deref())
}

#[tauri::command]
pub fn planner_delete(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    planner::delete(&mut conn, &w, &id)
}

#[tauri::command]
pub fn planner_restore(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    planner::restore(&mut conn, &w, &id)
}

#[tauri::command]
pub fn idea_list(state: State<AppState>) -> AppResult<Vec<Idea>> {
    planner::idea_list(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn idea_create(state: State<AppState>, input: IdeaInput) -> AppResult<String> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    planner::idea_create(&mut conn, &w, &me, &input)
}

#[tauri::command]
pub fn idea_delete(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    planner::idea_delete(&mut conn, &w, &id)
}

#[tauri::command]
pub fn idea_restore(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    planner::idea_restore(&mut conn, &w, &id)
}
