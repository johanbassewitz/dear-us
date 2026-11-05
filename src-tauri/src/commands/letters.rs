use crate::commands::journal::ctx;
use crate::db::repo::letters::{self, LetterDetail, LetterInput, LetterSummary};
use crate::error::{AppError, AppResult};
use crate::timelock;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn letter_list(state: State<AppState>) -> AppResult<Vec<LetterSummary>> {
    let conn = state.db.lock().unwrap();
    let (_, me) = ctx(&conn)?;
    letters::list(&conn, &me)
}

#[tauri::command]
pub fn letter_get(state: State<AppState>, id: String) -> AppResult<LetterDetail> {
    let conn = state.db.lock().unwrap();
    let (_, me) = ctx(&conn)?;
    letters::get(&conn, &me, &id)
}

#[tauri::command]
pub fn letter_save_draft(state: State<AppState>, id: Option<String>, input: LetterInput) -> AppResult<String> {
    let mut conn = state.db.lock().unwrap();
    let (_, me) = ctx(&conn)?;
    letters::save_draft(&mut conn, &me, id.as_deref(), &input)
}

#[tauri::command]
pub fn letter_send(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    letters::send(&mut conn, &w, &me, &id)
}

#[tauri::command]
pub fn letter_delete(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    letters::delete(&mut conn, &w, &me, &id)
}

#[tauri::command]
pub fn letter_restore(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    letters::restore(&mut conn, &w, &me, &id)
}

/// Opens a capsule from your person once its moment has come. The only command that touches the
/// network (one public drand signature); the database lock is not held while waiting for it.
#[tauri::command]
pub async fn letter_unlock(state: State<'_, AppState>, id: String) -> AppResult<LetterDetail> {
    let plan = {
        let conn = state.db.lock().unwrap();
        let (_, me) = ctx(&conn)?;
        letters::unlock_plan(&conn, &me, &id)?
    };
    if let Some(plan) = plan {
        let sig = tauri::async_runtime::spawn_blocking(move || timelock::fetch_signature(plan.round))
            .await
            .map_err(|_| AppError::invalid("something went wrong while opening the capsule"))??;
        let mut conn = state.db.lock().unwrap();
        let (_, me) = ctx(&conn)?;
        letters::unlock_apply(&mut conn, &me, &id, &sig)?;
    }
    let conn = state.db.lock().unwrap();
    let (_, me) = ctx(&conn)?;
    letters::get(&conn, &me, &id)
}
