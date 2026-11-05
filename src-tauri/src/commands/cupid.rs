use crate::cupid::{self, Candidate, ChatMessage, CupidStatus, IdeaSuggestion};
use crate::db::repo::questions;
use crate::error::{AppError, AppResult};
use crate::AppState;
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;
use tauri::{AppHandle, Manager, State};

fn key_and_model(conn: &Connection) -> AppResult<(String, String)> {
    let st = cupid::status(conn)?;
    if !st.enabled {
        return Err(AppError::invalid("Cupid's brain is off. Add your Gemini key in Settings → Cupid."));
    }
    let device = crate::sync::device_id(conn)?;
    let key = crate::secrets::get(&cupid::secret_name(&device))?.ok_or_else(|| AppError::invalid("No Gemini key on this PC."))?;
    Ok((key, st.model))
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> AppResult<T> + Send + 'static) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| AppError::invalid(e.to_string()))?
}

#[tauri::command]
pub fn cupid_status(state: State<AppState>) -> AppResult<CupidStatus> {
    cupid::status(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn cupid_set_option(state: State<AppState>, name: String, value: String) -> AppResult<CupidStatus> {
    let conn = state.db.lock().unwrap();
    cupid::set_option(&conn, &name, &value)?;
    cupid::status(&conn)
}

/// Checks the key with Google (without using a model), then stores it in Windows Credential Manager.
#[tauri::command]
pub async fn cupid_set_key(app: AppHandle, key: String) -> AppResult<CupidStatus> {
    let key = key.trim().to_string();
    if key.len() < 20 || key.len() > 200 || key.chars().any(char::is_whitespace) {
        return Err(AppError::invalid("That doesn't look like a Gemini API key."));
    }
    let device = {
        let s = app.state::<AppState>();
        let conn = s.db.lock().unwrap();
        crate::sync::device_id(&conn)?
    };
    let k = key.clone();
    // A rejected key is never saved. If Google is just busy or unreachable, the key is
    // saved anyway (it can't be judged right now) and the first chat will tell.
    blocking(move || cupid::check_key(&k)).await?;
    crate::secrets::set(&cupid::secret_name(&device), &key)?;
    let s = app.state::<AppState>();
    let conn = s.db.lock().unwrap();
    cupid::status(&conn)
}

#[tauri::command]
pub fn cupid_remove_key(state: State<AppState>) -> AppResult<CupidStatus> {
    let conn = state.db.lock().unwrap();
    let device = crate::sync::device_id(&conn)?;
    crate::secrets::delete(&cupid::secret_name(&device))?;
    cupid::status(&conn)
}

#[tauri::command]
pub fn cupid_history(state: State<AppState>) -> AppResult<Vec<ChatMessage>> {
    cupid::history(&state.db.lock().unwrap(), 60)
}

#[tauri::command]
pub fn cupid_clear_chat(state: State<AppState>) -> AppResult<()> {
    state.db.lock().unwrap().execute("DELETE FROM cupid_chats", [])?;
    Ok(())
}

#[tauri::command]
pub async fn cupid_chat(app: AppHandle, message: String) -> AppResult<ChatMessage> {
    let message = message.trim().to_string();
    if message.is_empty() || message.chars().count() > 2000 {
        return Err(AppError::invalid("Write a message (up to 2000 characters)."));
    }
    let (key, model, system, turns) = {
        let s = app.state::<AppState>();
        let conn = s.db.lock().unwrap();
        let (key, model) = key_and_model(&conn)?;
        let names = |self_: i64| conn.query_row("SELECT display_name FROM users WHERE is_self = ?1 LIMIT 1", [self_], |r| r.get::<_, String>(0)).optional();
        let me = names(1)?.unwrap_or_else(|| "someone".into());
        let partner = names(0)?;
        let mut turns: Vec<(String, String)> = cupid::history(&conn, 12)?.into_iter().map(|m| (m.role, m.content)).collect();
        turns.push(("user".into(), message.clone()));
        cupid::save_message(&conn, "user", &message)?;
        (key, model, cupid::chat_system(&me, partner.as_deref()), turns)
    };
    let reply = blocking(move || cupid::generate(&key, &model, &system, &turns, false)).await?;
    let s = app.state::<AppState>();
    let conn = s.db.lock().unwrap();
    cupid::save_message(&conn, "model", &reply)
}

/// The day's question: if nobody picked it yet and Cupid is on, Cupid chooses from a
/// shortlist of the library; otherwise the usual fixed pick. First pick wins on both PCs.
#[tauri::command]
pub async fn question_daily_smart(app: AppHandle, date: String, fallback_id: String, candidates: Vec<Candidate>) -> AppResult<questions::DailyQuestion> {
    let existing_or_ai = {
        let s = app.state::<AppState>();
        let conn = s.db.lock().unwrap();
        let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM daily_questions WHERE id = ?1)", [&date], |r| r.get(0))?;
        let st = cupid::status(&conn)?;
        if exists || !st.enabled || !st.daily_question || candidates.is_empty() {
            None
        } else {
            key_and_model(&conn).ok()
        }
    };
    let mut chosen = fallback_id.clone();
    if let Some((key, model)) = existing_or_ai {
        let (d, c) = (date.clone(), candidates.clone());
        let reply = blocking(move || cupid::generate(&key, &model, cupid::persona(), &[("user".into(), cupid::pick_question_prompt(&d, &c))], true)).await;
        if let Ok(r) = reply {
            if let Some(id) = cupid::parse_pick(&r, &candidates) {
                chosen = id;
            }
        } // any problem: the fixed pick is used, quietly
    }
    let s = app.state::<AppState>();
    let mut conn = s.db.lock().unwrap();
    let (w, _) = crate::commands::journal::ctx(&conn)?;
    questions::daily_ensure(&mut conn, &w, &date, &chosen)
}

#[tauri::command]
pub async fn cupid_date_ideas(app: AppHandle, filters: Value) -> AppResult<Vec<IdeaSuggestion>> {
    let (key, model, bucket) = {
        let s = app.state::<AppState>();
        let conn = s.db.lock().unwrap();
        let st = cupid::status(&conn)?;
        if !st.date_ideas {
            return Err(AppError::invalid("Date ideas from Cupid are turned off in Settings."));
        }
        let (key, model) = key_and_model(&conn)?;
        let bucket: Vec<String> = if st.share_bucket_titles {
            let mut q = conn.prepare("SELECT title FROM bucket_items WHERE deleted_at IS NULL AND status <> 'done' ORDER BY updated_at DESC LIMIT 10")?;
            let r = q.query_map([], |r| r.get(0))?;
            r.collect::<Result<_, _>>()?
        } else {
            Vec::new()
        };
        (key, model, bucket)
    };
    let prompt = cupid::ideas_prompt(&filters, &bucket);
    let reply = blocking(move || cupid::generate(&key, &model, cupid::persona(), &[("user".into(), prompt)], true)).await?;
    let ideas = cupid::parse_ideas(&reply);
    if ideas.is_empty() {
        return Err(AppError::invalid("Cupid couldn't think of anything this time. Try again?"));
    }
    Ok(ideas)
}
