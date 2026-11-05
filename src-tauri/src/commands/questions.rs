use crate::commands::journal::ctx;
use crate::db::repo::questions::{self, Answer, CustomQuestion, DailyQuestion, Mood};
use crate::error::AppResult;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn question_custom_list(state: State<AppState>) -> AppResult<Vec<CustomQuestion>> {
    questions::custom_list(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn question_custom_create(state: State<AppState>, text: String, category: String) -> AppResult<CustomQuestion> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    questions::custom_create(&mut conn, &w, &me, &text, &category)
}

#[tauri::command]
pub fn question_custom_delete(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    questions::custom_delete(&mut conn, &w, &id)
}

#[tauri::command]
pub fn question_custom_restore(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    questions::custom_restore(&mut conn, &w, &id)
}

#[tauri::command]
pub fn question_answers(state: State<AppState>) -> AppResult<Vec<Answer>> {
    let conn = state.db.lock().unwrap();
    let (_, me) = ctx(&conn)?;
    questions::answer_list(&conn, &me)
}

#[tauri::command]
pub fn question_answer_save(state: State<AppState>, question_id: String, body: String, date: String) -> AppResult<Answer> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    questions::answer_save(&mut conn, &w, &me, &question_id, &body, &date)
}

#[tauri::command]
pub fn question_answer_delete(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    questions::answer_delete(&mut conn, &w, &me, &id)
}

#[tauri::command]
pub fn question_answer_restore(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    questions::answer_restore(&mut conn, &w, &me, &id)
}

#[tauri::command]
pub fn question_daily(state: State<AppState>, date: String, question_id: String) -> AppResult<DailyQuestion> {
    let mut conn = state.db.lock().unwrap();
    let (w, _) = ctx(&conn)?;
    questions::daily_ensure(&mut conn, &w, &date, &question_id)
}

#[tauri::command]
pub fn mood_list(state: State<AppState>, limit: Option<i64>) -> AppResult<Vec<Mood>> {
    questions::mood_list(&state.db.lock().unwrap(), limit.unwrap_or(60))
}

#[tauri::command]
pub fn mood_save(state: State<AppState>, date: String, mood: String, note: String) -> AppResult<Mood> {
    questions::mood_save(&state.db.lock().unwrap(), &date, &mood, &note)
}

#[tauri::command]
pub fn mood_delete(state: State<AppState>, date: String) -> AppResult<()> {
    questions::mood_delete(&state.db.lock().unwrap(), &date)
}
