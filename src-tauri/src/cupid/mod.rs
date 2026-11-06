//! Cupid's brain: Google Gemini.
//! - personal edition: each person's OWN free API key (set in Settings);
//! - Store edition: through the Dear Us Cloud gateway with our paid key, counted in "arrows"
//!   per couple (see cloud/src/cupid.rs). No key on the PC at all.
//!
//! Privacy rules (docs/PLAN.md §10):
//! - off until the person adds a key and acknowledges the free-tier data notice;
//! - the key lives in Windows Credential Manager, is never synced and never sent to the UI;
//! - Cupid only sees what's needed for the request: chat messages typed to him, a shortlist
//!   of library questions, or date-idea filters (+ open bucket-list titles if allowed).
//!   Never letters, capsules, journal text, photos or videos;
//! - no scores, judgments or "relationship analysis".
#![cfg_attr(feature = "store", allow(dead_code))]

use crate::error::{AppError, AppResult};
use crate::sync::apply::meta_set;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

pub const DEFAULT_MODEL: &str = "gemini-flash-latest";
const ENDPOINT: &str = "https://generativelanguage.googleapis.com/v1beta/models";

const PERSONA: &str = "You are Cupid, a small, warm, playful companion inside \"Dear Us\", a private journal app for a couple. \
Keep replies short (at most 120 words), kind and practical. Never rate, score or judge the relationship, never take sides, \
never diagnose. If someone seems in distress or unsafe, gently suggest talking to a trusted person or a professional. \
You can't see their journal, letters or photos — only what they write to you here.";

fn get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT value FROM local_meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

pub fn secret_name(device: &str) -> String {
    crate::sync::service::secret_name(device, "gemini")
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Arrows {
    /// What's left right now: this month's allowance + top-up arrows.
    pub remaining: i32,
    pub allowance: i32,
    pub used: i32,
    pub bonus: i32,
    pub tier: String,
    pub can_buy: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CupidStatus {
    pub enabled: bool,
    /// Store edition: Cupid runs through the Dear Us cloud (no personal key).
    pub cloud: bool,
    /// Store edition: the couple's arrows (None until the plan is known).
    pub arrows: Option<Arrows>,
    pub has_key: bool,
    pub privacy_ack: bool,
    pub model: String,
    pub daily_question: bool,
    pub date_ideas: bool,
    pub share_bucket_titles: bool,
}

#[cfg(feature = "store")]
pub fn arrows(p: &crate::sync::cloud::Plan) -> Arrows {
    Arrows { remaining: p.cupid_remaining, allowance: p.cupid_allowance, used: p.cupid_used, bonus: p.cupid_bonus, tier: p.tier.clone(), can_buy: p.billing_available }
}

pub fn status(conn: &Connection) -> AppResult<CupidStatus> {
    let on = |k: &str, default: bool| -> AppResult<bool> { Ok(get(conn, k)?.map(|v| v == "1").unwrap_or(default)) };
    let privacy_ack = on("cupid.privacy_ack", false)?;
    #[cfg(feature = "store")]
    let (has_key, cloud, arrows) = {
        // "has a brain" = connected to a shared space in the Dear Us cloud
        let connected = crate::sync::service::load_config(conn)?.and_then(|c| c.space).is_some();
        (connected, true, crate::sync::service::cached_plan(conn).map(|p| arrows(&p)))
    };
    #[cfg(not(feature = "store"))]
    let (has_key, cloud, arrows) = {
        let device = crate::sync::device_id(conn)?;
        (crate::secrets::get(&secret_name(&device))?.is_some(), false, None)
    };
    Ok(CupidStatus {
        enabled: has_key && privacy_ack,
        cloud,
        arrows,
        has_key,
        privacy_ack,
        model: get(conn, "cupid.model")?.unwrap_or_else(|| DEFAULT_MODEL.into()),
        daily_question: on("cupid.daily", true)?,
        date_ideas: on("cupid.ideas", true)?,
        share_bucket_titles: on("cupid.bucket_titles", false)?,
    })
}

pub fn set_option(conn: &Connection, name: &str, value: &str) -> AppResult<()> {
    let key = match name {
        "privacyAck" => "cupid.privacy_ack",
        "dailyQuestion" => "cupid.daily",
        "dateIdeas" => "cupid.ideas",
        "shareBucketTitles" => "cupid.bucket_titles",
        "model" => {
            let v = value.trim();
            if v.is_empty() || v.len() > 60 || !v.chars().all(|c| c.is_ascii_alphanumeric() || "-._".contains(c)) {
                return Err(AppError::invalid("That model name doesn't look right (e.g. gemini-flash-latest)."));
            }
            return meta_set(conn, "cupid.model", v);
        }
        _ => return Err(AppError::invalid("unknown Cupid setting")),
    };
    meta_set(conn, key, if value == "1" || value == "true" { "1" } else { "0" })
}

/// Tried after the chosen model when Google says it's overloaded (503) or failing (500).
const FALLBACK_MODEL: &str = "gemini-flash-lite-latest";

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(15)).timeout(Duration::from_secs(90)).build()
}

/// Turns a failed Gemini request into a message for people. `busy` = worth trying another model.
fn explain(e: ureq::Error) -> (AppError, bool) {
    match e {
        ureq::Error::Status(code, resp) => {
            let msg = resp.into_json::<Value>().ok().and_then(|v| v["error"]["message"].as_str().map(String::from)).unwrap_or_default();
            let low = msg.to_lowercase();
            match code {
                400 if low.contains("api key") => (AppError::invalid("Gemini didn't accept that API key."), false),
                400 if low.contains("location") => (AppError::invalid("Gemini's free tier isn't available in your country yet."), false),
                401 | 403 => (AppError::invalid("Gemini refused the key. Check it's copied completely, and that the Gemini API is enabled for it in Google AI Studio."), false),
                404 => (AppError::invalid("Gemini doesn't know that model name. Try gemini-flash-latest in Cupid's settings."), false),
                429 => (AppError::invalid("Cupid needs a little rest: the free Gemini limit was reached. Try again later."), false),
                500 | 502 | 503 | 504 => (AppError::invalid("Gemini is very busy right now (that's on Google's side). Try again in a few minutes."), true),
                _ => (AppError::invalid(format!("Gemini said: {} ({code})", msg.chars().take(200).collect::<String>())), false),
            }
        }
        ureq::Error::Transport(t) => {
            let detail = t.to_string();
            let msg = if detail.to_lowercase().contains("timed out") {
                "Gemini took too long to answer (it may be very busy). Try again in a few minutes.".to_string()
            } else {
                format!("Cupid can't reach Gemini right now. Are you online? ({})", detail.chars().take(120).collect::<String>())
            };
            (AppError::invalid(msg), true)
        }
    }
}

/// Checks a key without depending on any model being available: lists one model.
/// Ok(true) = accepted, Ok(false) = Google couldn't be asked right now (busy/offline), Err = rejected.
pub fn check_key(api_key: &str) -> AppResult<bool> {
    match agent().get(&format!("{ENDPOINT}?pageSize=1")).set("x-goog-api-key", api_key).call() {
        Ok(_) => Ok(true),
        Err(e) => match explain(e) {
            (_, true) => Ok(false),
            (err, false) => Err(err),
        },
    }
}

/// One Gemini call. `turns` = (role "user"|"model", text). `json` asks for a JSON reply.
/// If the model is overloaded, tries the lighter fallback model once.
pub fn generate(api_key: &str, model: &str, system: &str, turns: &[(String, String)], json_reply: bool) -> AppResult<String> {
    let body = request_body(system, turns, json_reply);
    let mut models = vec![model];
    if model != FALLBACK_MODEL {
        models.push(FALLBACK_MODEL);
    }
    let mut last = None;
    for m in models {
        match agent().post(&format!("{ENDPOINT}/{m}:generateContent")).set("x-goog-api-key", api_key).send_json(body.clone()) {
            Ok(r) => {
                let v: Value = r.into_json().map_err(|e| AppError::invalid(e.to_string()))?;
                return parse_reply(&v);
            }
            Err(e) => {
                let (err, busy) = explain(e);
                if !busy {
                    return Err(err);
                }
                last = Some(err);
            }
        }
    }
    Err(last.unwrap_or_else(|| AppError::invalid("Gemini didn't answer.")))
}

pub fn request_body(system: &str, turns: &[(String, String)], json_reply: bool) -> Value {
    let contents: Vec<Value> = turns
        .iter()
        .map(|(role, text)| json!({ "role": if role == "model" { "model" } else { "user" }, "parts": [{ "text": text }] }))
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

pub fn parse_reply(v: &Value) -> AppResult<String> {
    if let Some(reason) = v["promptFeedback"]["blockReason"].as_str() {
        return Err(AppError::invalid(format!("Gemini declined to answer that ({reason}).")));
    }
    let text: String = v["candidates"][0]["content"]["parts"]
        .as_array()
        .map(|parts| parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join(""))
        .unwrap_or_default();
    if text.trim().is_empty() {
        return Err(AppError::invalid("Cupid didn't come up with anything this time. Try again?"));
    }
    Ok(text.trim().to_string())
}

// ---------- chat ----------

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

pub fn history(conn: &Connection, limit: i64) -> AppResult<Vec<ChatMessage>> {
    let mut s = conn.prepare("SELECT id, role, content, created_at FROM cupid_chats ORDER BY created_at DESC, rowid DESC LIMIT ?1")?;
    let mut v: Vec<ChatMessage> =
        s.query_map([limit], |r| Ok(ChatMessage { id: r.get(0)?, role: r.get(1)?, content: r.get(2)?, created_at: r.get(3)? }))?.collect::<Result<_, _>>()?;
    v.reverse();
    Ok(v)
}

pub fn save_message(conn: &Connection, role: &str, content: &str) -> AppResult<ChatMessage> {
    let m = ChatMessage { id: crate::db::new_id(), role: role.into(), content: content.into(), created_at: crate::db::now_iso() };
    conn.execute("INSERT INTO cupid_chats(id, role, content, created_at) VALUES (?1,?2,?3,?4)", params![m.id, m.role, m.content, m.created_at])?;
    Ok(m)
}

pub fn chat_system(me: &str, partner: Option<&str>) -> String {
    format!("{PERSONA} You're talking with {me}{}.", partner.map(|p| format!(", whose partner is {p}")).unwrap_or_default())
}

// ---------- structured helpers ----------

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub text: String,
}

pub fn pick_question_prompt(date: &str, candidates: &[Candidate]) -> String {
    let list = candidates.iter().map(|c| format!("- {}: {}", c.id, c.text)).collect::<Vec<_>>().join("\n");
    format!(
        "Today is {date}. Pick the ONE gentle question below that would make the nicest conversation for a couple today \
         (consider the season and day of the week). Answer as JSON: {{\"id\": \"<the id>\"}}.\n\n{list}"
    )
}

/// The id Cupid picked, if it's one of the candidates (anything else is ignored).
pub fn parse_pick(reply: &str, candidates: &[Candidate]) -> Option<String> {
    let v: Value = serde_json::from_str(reply.trim().trim_start_matches("```json").trim_matches('`').trim()).ok()?;
    let id = v["id"].as_str()?;
    candidates.iter().any(|c| c.id == id).then(|| id.to_string())
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IdeaSuggestion {
    pub title: String,
    pub description: String,
    pub category: String,
    pub budget: String,
    pub duration: String,
    pub setting: String,
    pub mood: String,
}

const CATEGORIES: &[&str] = &["at-home", "outdoors", "romantic", "adventure", "food", "entertainment", "low-budget", "special-occasion"];
const BUDGETS: &[&str] = &["free", "low", "medium", "high"];
const DURATIONS: &[&str] = &["short", "half-day", "full-day"];
const SETTINGS: &[&str] = &["indoor", "outdoor", "either"];
const MOODS: &[&str] = &["cozy", "playful", "romantic", "adventurous", "relaxed"];

pub fn ideas_prompt(filters: &Value, bucket_titles: &[String]) -> String {
    let wishes = if bucket_titles.is_empty() { String::new() } else { format!("\nThings on their shared bucket list (for inspiration): {}.", bucket_titles.join("; ")) };
    format!(
        "Suggest 3 specific, charming date ideas for a couple. Preferences (any may be empty): {filters}.{wishes}\n\
         Reply as JSON: {{\"ideas\": [{{\"title\": short, \"description\": one warm sentence, \"category\": one of {CATEGORIES:?}, \
         \"budget\": one of {BUDGETS:?}, \"duration\": one of {DURATIONS:?}, \"setting\": one of {SETTINGS:?}, \"mood\": one of {MOODS:?}}}]}}. \
         Nothing risky, nothing sexual."
    )
}

/// Keeps only well-formed ideas with allowed values (the model's output is never trusted as-is).
pub fn parse_ideas(reply: &str) -> Vec<IdeaSuggestion> {
    let raw = reply.trim().trim_start_matches("```json").trim_matches('`').trim();
    let v: Value = serde_json::from_str(raw).unwrap_or(Value::Null);
    let list = v["ideas"].as_array().cloned().or_else(|| v.as_array().cloned()).unwrap_or_default();
    let pick = |x: &Value, allowed: &[&str], default: &str| -> String {
        x.as_str().map(|s| s.trim().to_lowercase()).filter(|s| allowed.contains(&s.as_str())).unwrap_or_else(|| default.into())
    };
    list.iter()
        .filter_map(|i| {
            let title = i["title"].as_str()?.trim().chars().take(80).collect::<String>();
            if title.is_empty() {
                return None;
            }
            Some(IdeaSuggestion {
                title,
                description: i["description"].as_str().unwrap_or("").trim().chars().take(240).collect(),
                category: pick(&i["category"], CATEGORIES, "romantic"),
                budget: pick(&i["budget"], BUDGETS, "low"),
                duration: pick(&i["duration"], DURATIONS, "short"),
                setting: pick(&i["setting"], SETTINGS, "either"),
                mood: pick(&i["mood"], MOODS, "cozy"),
            })
        })
        .take(5)
        .collect()
}

pub fn persona() -> &'static str {
    PERSONA
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_gemini_request() {
        let b = request_body("sys", &[("user".into(), "hi".into()), ("model".into(), "hello".into())], true);
        assert_eq!(b["contents"][1]["role"], "model");
        assert_eq!(b["systemInstruction"]["parts"][0]["text"], "sys");
        assert_eq!(b["generationConfig"]["responseMimeType"], "application/json");
    }

    #[test]
    fn parses_replies_and_blocks() {
        let ok = json!({"candidates":[{"content":{"parts":[{"text":"Hello "},{"text":"love"}]}}]});
        assert_eq!(parse_reply(&ok).unwrap(), "Hello love");
        assert!(parse_reply(&json!({"promptFeedback":{"blockReason":"SAFETY"}})).is_err());
        assert!(parse_reply(&json!({"candidates":[]})).is_err());
    }

    #[test]
    fn only_accepts_a_listed_question() {
        let c = vec![Candidate { id: "q1".into(), text: "a".into() }, Candidate { id: "q2".into(), text: "b".into() }];
        assert_eq!(parse_pick(r#"{"id":"q2"}"#, &c).as_deref(), Some("q2"));
        assert_eq!(parse_pick("```json\n{\"id\":\"q1\"}\n```", &c).as_deref(), Some("q1"));
        assert_eq!(parse_pick(r#"{"id":"made-up"}"#, &c), None);
        assert_eq!(parse_pick("not json", &c), None);
    }

    #[test]
    fn idea_output_is_sanitized() {
        let r = r#"{"ideas":[{"title":"Sunrise picnic","description":"Bring croissants.","category":"OUTDOORS","budget":"cheap!","duration":"short","setting":"outdoor","mood":"romantic"},{"title":"","description":"x"}]}"#;
        let ideas = parse_ideas(r);
        assert_eq!(ideas.len(), 1);
        assert_eq!(ideas[0].category, "outdoors");
        assert_eq!(ideas[0].budget, "low", "unknown values fall back to a safe default");
    }

    #[test]
    fn settings_validate() {
        let conn = crate::db::open_in_memory().unwrap();
        set_option(&conn, "model", "gemini-2.5-flash").unwrap();
        assert!(set_option(&conn, "model", "../etc").is_err());
        assert!(set_option(&conn, "nope", "1").is_err());
        let s = status(&conn).unwrap();
        assert_eq!(s.model, "gemini-2.5-flash");
        assert!(!s.enabled, "no key, no ack → off");
    }
}
