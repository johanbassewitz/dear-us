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

/// Store edition, free tier: how many time capsules may be sealed and waiting at once.
/// Writing and opening capsules is always free; only sealing beyond the limit waits.
#[cfg(feature = "store")]
fn check_capsule_limit(conn: &rusqlite::Connection, id: &str) -> AppResult<()> {
    let Some(limit) = crate::sync::service::cached_plan(conn).and_then(|p| p.capsule_limit) else { return Ok(()) };
    let kind: Option<String> = conn.query_row("SELECT kind FROM letters WHERE id = ?1", [id], |r| r.get(0)).ok();
    if kind.as_deref() != Some("capsule") {
        return Ok(());
    }
    let waiting = letters::sealed_waiting(conn)?;
    if waiting >= i64::from(limit) {
        return Err(AppError::invalid(format!(
            "[capsule_limit] You have {waiting} time capsules waiting to open, the most the free plan keeps sealed at once. Once one opens you can seal this one, or Premium seals as many as you like."
        )));
    }
    Ok(())
}

#[tauri::command]
pub fn letter_send(state: State<AppState>, id: String) -> AppResult<()> {
    let mut conn = state.db.lock().unwrap();
    let (w, me) = ctx(&conn)?;
    #[cfg(feature = "store")]
    check_capsule_limit(&conn, &id)?;
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
