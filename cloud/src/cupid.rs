//! Cupid's gateway: the app sends the conversation (never journal text, letters or photos —
//! the app's privacy rules decide what goes in), we add our AI key, take one arrow from the
//! couple's quiver, and give it back if the AI couldn't answer.
//!
//! Provider: OpenRouter (OPENROUTER_API_KEY; model in OPENROUTER_MODEL, a cheap one by
//! default) or, without it, Google's Gemini API directly (GEMINI_API_KEY).

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::plan::{self, Plan};
use crate::spaces::require_member;
use crate::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

const ENDPOINT: &str = "https://generativelanguage.googleapis.com/v1beta/models";

#[derive(Deserialize, Clone)]
pub struct Turn {
    pub role: String,
    pub text: String,
}

#[derive(Deserialize)]
pub struct CupidIn {
    /// "chat" | "ideas" | "daily_pick" (one free pick of the day's question per space per day)
    pub kind: String,
    pub system: String,
    pub turns: Vec<Turn>,
    #[serde(default)]
    pub json: bool,
}

#[derive(Serialize)]
pub struct CupidOut {
    pub reply: String,
    pub plan: Plan,
}

/// OpenRouter (OpenAI-style chat completions). `data_collection: deny` keeps requests away from
/// providers that may store or train on them.
pub fn openrouter_body(model: &str, system: &str, turns: &[Turn], json_reply: bool) -> Value {
    let mut messages = vec![json!({ "role": "system", "content": system })];
    messages.extend(turns.iter().map(|t| json!({ "role": if t.role == "model" { "assistant" } else { "user" }, "content": t.text })));
    let mut body = json!({
        "model": model,
        "messages": messages,
        "temperature": 0.9,
        "max_tokens": 800,
        "provider": { "data_collection": "deny" },
    });
    if json_reply {
        body["response_format"] = json!({ "type": "json_object" });
    }
    body
}

pub fn parse_openrouter(v: &Value) -> Result<String, ApiError> {
    let choice = &v["choices"][0];
    if choice["finish_reason"].as_str() == Some("content_filter") {
        return Err(ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "cupid_declined", "Cupid can't help with that one."));
    }
    let text = choice["message"]["content"].as_str().unwrap_or_default();
    if text.trim().is_empty() {
        return Err(ApiError::new(StatusCode::BAD_GATEWAY, "cupid_empty", "Cupid didn't come up with anything this time. Try again?"));
    }
    Ok(text.trim().to_string())
}

async fn call_openrouter(state: &AppState, key: &str, system: &str, turns: &[Turn], json_reply: bool) -> ApiResult<String> {
    let busy = || ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "cupid_busy", "Cupid is very busy right now. Try again in a few minutes.");
    let cfg = &state.cfg;
    for m in [cfg.openrouter_model.as_str(), cfg.openrouter_fallback_model.as_str()] {
        let r = state
            .http
            .post(format!("{}/chat/completions", cfg.openrouter_base))
            .bearer_auth(key)
            .header("HTTP-Referer", "https://dearus.axianthq.com")
            .header("X-Title", "Dear Us")
            .timeout(Duration::from_secs(60))
            .json(&openrouter_body(m, system, turns, json_reply))
            .send()
            .await;
        match r {
            Ok(r) if r.status().is_success() => {
                let v: Value = r.json().await.map_err(ApiError::internal)?;
                // OpenRouter can answer 200 with an error object (e.g. the provider failed)
                if v.get("error").is_some() && v["choices"].is_null() {
                    tracing::warn!("openrouter {m}: {}", v["error"]["message"].as_str().unwrap_or("error"));
                    continue;
                }
                return parse_openrouter(&v);
            }
            Ok(r) if r.status().as_u16() == 403 => return Err(ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "cupid_declined", "Cupid can't help with that one.")),
            Ok(r) if r.status().as_u16() == 402 => {
                tracing::error!("openrouter: out of credits — top up at openrouter.ai/credits");
                return Err(ApiError::unavailable("Cupid is resting right now. Try again later."));
            }
            // busy, or (404) no provider for this model meets the data policy: try the fallback model
            Ok(r) if matches!(r.status().as_u16(), 404 | 408 | 429 | 500 | 502 | 503 | 504) => {
                let status = r.status();
                let text = r.text().await.unwrap_or_default();
                tracing::warn!("openrouter {m} said {status}: {}", text.chars().take(200).collect::<String>());
                continue;
            }
            Ok(r) => {
                let status = r.status();
                let text = r.text().await.unwrap_or_default();
                tracing::error!("openrouter {m} said {status}: {}", text.chars().take(300).collect::<String>());
                return Err(busy());
            }
            Err(e) => {
                tracing::warn!("openrouter {m} unreachable: {e}");
                continue;
            }
        }
    }
    Err(busy())
}

/// One answer from whichever AI provider is configured.
async fn call_llm(state: &AppState, system: &str, turns: &[Turn], json_reply: bool) -> ApiResult<String> {
    if let Some(key) = &state.cfg.openrouter_key {
        return call_openrouter(state, key, system, turns, json_reply).await;
    }
    call_gemini(state, &request_body(system, turns, json_reply)).await
}

pub fn request_body(system: &str, turns: &[Turn], json_reply: bool) -> Value {
    let contents: Vec<Value> = turns
        .iter()
        .map(|t| json!({ "role": if t.role == "model" { "model" } else { "user" }, "parts": [{ "text": t.text }] }))
        .collect();
    let mut body = json!({
        "systemInstruction": { "parts": [{ "text": system }] },
        "contents": contents,
        "generationConfig": { "temperature": 0.9, "maxOutputTokens": 800 },
    });
    if json_reply {
        body["generationConfig"]["responseMimeType"] = json!("application/json");
    }
    body
}

pub fn parse_reply(v: &Value) -> Result<String, ApiError> {
    if let Some(reason) = v["promptFeedback"]["blockReason"].as_str() {
        return Err(ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "cupid_declined", format!("Cupid can't help with that one ({reason}).")));
    }
    let text: String = v["candidates"][0]["content"]["parts"]
        .as_array()
        .map(|parts| parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join(""))
        .unwrap_or_default();
    if text.trim().is_empty() {
        return Err(ApiError::new(StatusCode::BAD_GATEWAY, "cupid_empty", "Cupid didn't come up with anything this time. Try again?"));
    }
    Ok(text.trim().to_string())
}

fn validate(b: &CupidIn) -> ApiResult<()> {
    let too_long = b.system.chars().count() > 6000
        || b.turns.is_empty()
        || b.turns.len() > 16
        || b.turns.iter().any(|t| t.text.chars().count() > 4000)
        || b.turns.iter().map(|t| t.text.len()).sum::<usize>() > 40_000;
    if too_long {
        return Err(ApiError::bad("That's a bit long for Cupid. Try a shorter message."));
    }
    if !matches!(b.kind.as_str(), "chat" | "ideas" | "daily_pick") {
        return Err(ApiError::bad("unknown Cupid request"));
    }
    Ok(())
}

async fn call_gemini(state: &AppState, body: &Value) -> ApiResult<String> {
    let key = state.cfg.gemini_key.as_ref().ok_or_else(|| ApiError::unavailable("Cupid is resting right now. Try again later."))?;
    let models = [state.cfg.gemini_model.as_str(), state.cfg.gemini_fallback_model.as_str()];
    let busy = || ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "cupid_busy", "Cupid is very busy right now. Try again in a few minutes.");
    for m in models {
        let r = state.http.post(format!("{ENDPOINT}/{m}:generateContent")).header("x-goog-api-key", key).timeout(Duration::from_secs(60)).json(body).send().await;
        match r {
            Ok(r) if r.status().is_success() => {
                let v: Value = r.json().await.map_err(ApiError::internal)?;
                return parse_reply(&v);
            }
            Ok(r) if matches!(r.status().as_u16(), 429 | 500 | 502 | 503 | 504) => continue, // try the lighter model
            Ok(r) => {
                let status = r.status();
                let text = r.text().await.unwrap_or_default();
                tracing::error!("gemini {m} said {status}: {}", text.chars().take(300).collect::<String>());
                return Err(busy());
            }
            Err(e) => {
                tracing::warn!("gemini {m} unreachable: {e}");
                continue;
            }
        }
    }
    Err(busy())
}

pub async fn ask(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path(space): Path<Uuid>, Json(b): Json<CupidIn>) -> ApiResult<Json<CupidOut>> {
    require_member(&state.db, space, user).await?;
    validate(&b)?;
    if b.kind == "daily_pick" {
        // free, once per space per day: the app quietly uses its fixed pick otherwise
        let r = sqlx::query("update spaces set daily_pick_on = current_date where id = $1 and daily_pick_on is distinct from current_date").bind(space).execute(&state.db).await?;
        if r.rows_affected() == 0 {
            return Err(ApiError::new(StatusCode::CONFLICT, "already_picked", "Today's question was already picked."));
        }
        let reply = call_llm(&state, &b.system, &b.turns, b.json).await?;
        return Ok(Json(CupidOut { reply, plan: plan::current(&state, space).await? }));
    }
    let mut tx = state.db.begin().await?;
    let (after, source) = plan::take_arrow(&mut tx, &state.cfg, space).await?;
    tx.commit().await?;
    match call_llm(&state, &b.system, &b.turns, b.json).await {
        Ok(reply) => Ok(Json(CupidOut { reply, plan: after })),
        Err(e) => {
            if let Err(re) = plan::refund_arrow(&state, space, &after.period, source).await {
                tracing::error!("couldn't refund a Cupid arrow: {}", re.message);
            }
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_and_parses() {
        let b = request_body("sys", &[Turn { role: "user".into(), text: "hi".into() }, Turn { role: "model".into(), text: "yo".into() }], true);
        assert_eq!(b["contents"][1]["role"], "model");
        assert_eq!(b["generationConfig"]["responseMimeType"], "application/json");
        assert_eq!(parse_reply(&json!({"candidates":[{"content":{"parts":[{"text":" hey "}]}}]})).unwrap(), "hey");
        assert_eq!(parse_reply(&json!({"promptFeedback":{"blockReason":"SAFETY"}})).unwrap_err().code, "cupid_declined");
    }

    #[test]
    fn openrouter_requests_and_replies() {
        let turns = [Turn { role: "user".into(), text: "hi".into() }, Turn { role: "model".into(), text: "yo".into() }];
        let b = openrouter_body("google/gemini-2.5-flash-lite", "be kind", &turns, true);
        assert_eq!(b["messages"][0]["role"], "system");
        assert_eq!(b["messages"][2]["role"], "assistant");
        assert_eq!(b["response_format"]["type"], "json_object");
        assert_eq!(b["provider"]["data_collection"], "deny", "no providers that store or train on prompts");
        assert_eq!(parse_openrouter(&json!({"choices":[{"message":{"content":" hello "}}]})).unwrap(), "hello");
        assert_eq!(parse_openrouter(&json!({"choices":[{"finish_reason":"content_filter","message":{"content":""}}]})).unwrap_err().code, "cupid_declined");
        assert_eq!(parse_openrouter(&json!({"choices":[]})).unwrap_err().code, "cupid_empty");
    }

    #[test]
    fn rejects_oversized_requests() {
        let big = CupidIn { kind: "chat".into(), system: "s".into(), turns: vec![Turn { role: "user".into(), text: "x".repeat(4001) }], json: false };
        assert!(validate(&big).is_err());
        let odd = CupidIn { kind: "essay".into(), system: "s".into(), turns: vec![Turn { role: "user".into(), text: "x".into() }], json: false };
        assert!(validate(&odd).is_err());
    }
}

#[derive(Deserialize)]
pub struct ReportIn {
    /// harmful | inappropriate | wrong | other
    pub reason: String,
    #[serde(default)]
    pub details: Option<String>,
    /// The answer being reported, only if the person chose to include it.
    #[serde(default)]
    pub answer: Option<String>,
}

/// "Report this answer": kept for review (see docs/STORE_RELEASE.md → Reviewing reports).
pub async fn report(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path(space): Path<Uuid>, Json(b): Json<ReportIn>) -> ApiResult<Json<bool>> {
    require_member(&state.db, space, user).await?;
    if !matches!(b.reason.as_str(), "harmful" | "inappropriate" | "wrong" | "other") {
        return Err(ApiError::bad("Pick a reason."));
    }
    if !state.limiter.hit(&format!("ai-report:{user}"), 30, Duration::from_secs(3600)) {
        return Err(ApiError::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "Thanks, we've got your reports. Try again later."));
    }
    let clip = |s: Option<String>, n: usize| s.map(|t| t.chars().take(n).collect::<String>()).filter(|t| !t.trim().is_empty());
    sqlx::query("insert into ai_reports(id, user_id, space_id, reason, details, answer) values ($1, $2, $3, $4, $5, $6)")
        .bind(Uuid::new_v4())
        .bind(user)
        .bind(space)
        .bind(&b.reason)
        .bind(clip(b.details, 2000))
        .bind(clip(b.answer, 8000))
        .execute(&state.db)
        .await?;
    tracing::warn!("new Cupid report ({}) — review with deploy/ai-reports.sh", b.reason);
    Ok(Json(true))
}
