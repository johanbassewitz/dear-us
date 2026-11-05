//! Love letters and time capsules.
//!
//! Drafts are private to this device: they are never op-logged. Sending op-logs one upsert
//! with the full letter. For capsules that upsert carries the tlock ciphertext only - never the
//! plaintext body, the attachment list or (when hidden) the title; those travel inside the
//! ciphertext. The sender's own copy keeps the plaintext, because they wrote it.

use super::journal::body_text;
use super::media::{self as media_repo, MediaRef, COLS as MEDIA_COLS};
use crate::db::{new_id, now_iso};
use crate::error::{AppError, AppResult};
use crate::sync::oplog::{stamp_fields, Writer};
use crate::timelock;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const ENTITY: &str = "letter";
const MAX_PHOTOS: usize = 12;
/// Capsules can wait a long time, but not past the point where dates stop making sense.
const MAX_UNLOCK_YEAR: i32 = 2200;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LetterInput {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body_json: String,
    /// "letter" or "capsule"
    pub kind: String,
    /// RFC 3339 instant; only for capsules.
    pub unlock_at: Option<String>,
    #[serde(default = "yes")]
    pub show_title: bool,
    #[serde(default)]
    pub media_ids: Vec<String>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LetterSummary {
    pub id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub recipient_id: Option<String>,
    pub recipient_name: Option<String>,
    pub mine: bool,
    /// None when the sender chose to hide the title until the capsule opens.
    pub title: Option<String>,
    pub kind: String,
    pub status: String,
    pub sent_at: Option<String>,
    pub unlock_at: Option<String>,
    pub show_title: bool,
    pub lock_kind: String,
    /// Capsule that hasn't reached its moment yet.
    pub sealed: bool,
    /// Capsule received from your person whose moment has come but whose text isn't opened yet.
    pub needs_unlock: bool,
    pub unlocked_at: Option<String>,
    pub excerpt: String,
    pub photo_count: i64,
    pub cover: Option<MediaRef>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LetterDetail {
    #[serde(flatten)]
    pub summary: LetterSummary,
    /// Empty while the text is still sealed to the reader.
    pub body_json: String,
    pub media: Vec<MediaRef>,
}

/// What the unlock command needs before it goes to the network.
#[derive(Debug, Clone)]
pub struct UnlockPlan {
    pub round: u64,
}

struct Row {
    id: String,
    sender_id: String,
    recipient_id: Option<String>,
    title: String,
    body_json: String,
    kind: String,
    status: String,
    sent_at: Option<String>,
    unlock_at: Option<String>,
    show_title: bool,
    lock_kind: String,
    cipher: Option<String>,
    unlocked_at: Option<String>,
    created_at: String,
    updated_at: String,
    field_clock: String,
}

const ROW_COLS: &str = "id, sender_id, recipient_id, title, body_json, kind, status, sent_at, unlock_at, show_title,
     lock_kind, tlock_ciphertext, unlocked_at, created_at, updated_at, field_clock";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Row> {
    Ok(Row {
        id: r.get(0)?,
        sender_id: r.get(1)?,
        recipient_id: r.get(2)?,
        title: r.get(3)?,
        body_json: r.get(4)?,
        kind: r.get(5)?,
        status: r.get(6)?,
        sent_at: r.get(7)?,
        unlock_at: r.get(8)?,
        show_title: r.get::<_, i64>(9)? != 0,
        lock_kind: r.get(10)?,
        cipher: r.get(11)?,
        unlocked_at: r.get(12)?,
        created_at: r.get(13)?,
        updated_at: r.get(14)?,
        field_clock: r.get(15)?,
    })
}

fn load(conn: &Connection, id: &str) -> AppResult<Row> {
    conn.query_row(
        &format!("SELECT {ROW_COLS} FROM letters WHERE id = ?1 AND deleted_at IS NULL"),
        [id],
        row,
    )
    .optional()?
    .ok_or_else(|| AppError::invalid("this letter doesn't exist anymore"))
}

fn instant(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&chrono::Utc))
}

fn is_future(unlock_at: &str) -> bool {
    instant(unlock_at).is_some_and(|d| d > chrono::Utc::now())
}

fn normalize_instant(s: &str) -> AppResult<String> {
    use chrono::Datelike;
    let d = instant(s.trim()).ok_or_else(|| AppError::invalid("pick a valid date and time"))?;
    if d.year() > MAX_UNLOCK_YEAR {
        return Err(AppError::invalid("that date is too far away"));
    }
    Ok(d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

fn is_blank_doc(body_json: &str) -> AppResult<bool> {
    Ok(body_text(body_json)?.trim().is_empty())
}

struct Clean {
    title: String,
    body_json: String,
    kind: String,
    unlock_at: Option<String>,
    show_title: bool,
    media: Vec<String>,
}

fn clean(conn: &Connection, i: &LetterInput) -> AppResult<Clean> {
    let title = i.title.trim().to_string();
    if title.chars().count() > 200 {
        return Err(AppError::invalid("the title is too long (200 characters max)"));
    }
    if !matches!(i.kind.as_str(), "letter" | "capsule") {
        return Err(AppError::invalid("pick a letter or a time capsule"));
    }
    body_text(&i.body_json)?; // validates the format and size
    let unlock_at = match (i.kind.as_str(), i.unlock_at.as_deref().map(str::trim)) {
        ("capsule", Some(s)) if !s.is_empty() => Some(normalize_instant(s)?),
        _ => None,
    };
    let mut media: Vec<String> = vec![];
    for m in &i.media_ids {
        if !media.contains(m) {
            if media_repo::get(conn, m)?.is_none() {
                return Err(AppError::invalid("that photo isn't in the archive"));
            }
            media.push(m.clone());
        }
    }
    if media.len() > MAX_PHOTOS {
        return Err(AppError::invalid(format!("up to {MAX_PHOTOS} photos per letter")));
    }
    Ok(Clean {
        title,
        body_json: i.body_json.clone(),
        show_title: i.kind == "letter" || i.show_title,
        kind: i.kind.clone(),
        unlock_at,
        media,
    })
}

fn write_media(tx: &rusqlite::Transaction, id: &str, media: &[String]) -> AppResult<()> {
    tx.execute("DELETE FROM letter_media WHERE letter_id = ?1", [id])?;
    for (pos, m) in media.iter().enumerate() {
        tx.execute(
            "INSERT INTO letter_media(letter_id, media_id, position) VALUES (?1,?2,?3)",
            params![id, m, pos as i64],
        )?;
    }
    Ok(())
}

fn media_ids(conn: &Connection, id: &str) -> AppResult<Vec<String>> {
    let mut s = conn.prepare("SELECT media_id FROM letter_media WHERE letter_id = ?1 ORDER BY position")?;
    let ids: Vec<String> = s.query_map([id], |r| r.get(0))?.collect::<Result<_, _>>()?;
    Ok(ids)
}

/// Creates a draft (id None) or updates one. Drafts never touch the op-log.
pub fn save_draft(conn: &mut Connection, me: &str, id: Option<&str>, i: &LetterInput) -> AppResult<String> {
    let c = clean(conn, i)?;
    let now = now_iso();
    let tx = conn.transaction()?;
    let id = match id {
        Some(id) => {
            let n = tx.execute(
                "UPDATE letters SET title=?2, body_json=?3, kind=?4, unlock_at=?5, show_title=?6, updated_at=?7
                 WHERE id=?1 AND status='draft' AND sender_id=?8 AND deleted_at IS NULL",
                params![id, c.title, c.body_json, c.kind, c.unlock_at, c.show_title as i64, now, me],
            )?;
            if n == 0 {
                return Err(AppError::invalid("this draft isn't available to edit (sent letters can't change)"));
            }
            id.to_string()
        }
        None => {
            let id = new_id();
            tx.execute(
                "INSERT INTO letters(id, sender_id, title, body_json, kind, status, unlock_at, show_title, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,'draft',?6,?7,?8,?8)",
                params![id, me, c.title, c.body_json, c.kind, c.unlock_at, c.show_title as i64, now],
            )?;
            id
        }
    };
    write_media(&tx, &id, &c.media)?;
    tx.commit()?;
    Ok(id)
}

fn partner_id(conn: &Connection) -> AppResult<Option<String>> {
    Ok(conn
        .query_row("SELECT id FROM users WHERE is_self = 0 ORDER BY created_at LIMIT 1", [], |r| r.get(0))
        .optional()?)
}

/// Sends a draft: one op-log upsert. Capsules are sealed with tlock first.
pub fn send(conn: &mut Connection, w: &Writer, me: &str, id: &str) -> AppResult<()> {
    let cur = load(conn, id)?;
    if cur.sender_id != me || cur.status != "draft" {
        return Err(AppError::invalid("only your own drafts can be sent"));
    }
    let title = cur.title.trim().to_string();
    if title.is_empty() {
        return Err(AppError::invalid("Give it a title before sending"));
    }
    if is_blank_doc(&cur.body_json)? {
        return Err(AppError::invalid("Write a few words first - the page is still empty"));
    }
    let media = media_ids(conn, id)?;
    let recipient = partner_id(conn)?;
    let now = now_iso();
    let capsule = cur.kind == "capsule";

    let (lock_kind, cipher, unlock_at) = if capsule {
        let unlock = cur
            .unlock_at
            .clone()
            .filter(|u| is_future(u))
            .ok_or_else(|| AppError::invalid("Pick a moment in the future to open the capsule"))?;
        let unix = instant(&unlock).map(|d| d.timestamp()).unwrap_or_default();
        let payload = json!({ "v": 1, "title": title, "body": cur.body_json, "media": media });
        let ct = timelock::encrypt(payload.to_string().as_bytes(), unix)?;
        ("tlock", Some(ct), Some(unlock))
    } else {
        ("none", None, None)
    };

    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE letters SET status='sent', sent_at=?2, recipient_id=?3, lock_kind=?4, tlock_ciphertext=?5,
                unlock_at=?6, show_title=?7, updated_at=?2 WHERE id=?1",
        params![id, now, recipient, lock_kind, cipher, unlock_at, cur.show_title as i64],
    )?;
    let mut ch = serde_json::Map::new();
    ch.insert("sender_id".into(), json!(me));
    ch.insert("recipient_id".into(), json!(recipient));
    ch.insert("kind".into(), json!(cur.kind));
    ch.insert("status".into(), json!("sent"));
    ch.insert("sent_at".into(), json!(now));
    ch.insert("unlock_at".into(), json!(unlock_at));
    ch.insert("show_title".into(), json!(cur.show_title));
    ch.insert("lock_kind".into(), json!(lock_kind));
    ch.insert("created_at".into(), json!(cur.created_at));
    if capsule {
        // Everything private rides inside the ciphertext.
        ch.insert("tlock_ciphertext".into(), json!(cipher));
        ch.insert("title".into(), json!(if cur.show_title { title.clone() } else { String::new() }));
    } else {
        ch.insert("title".into(), json!(title));
        ch.insert("body_json".into(), json!(cur.body_json));
        ch.insert("media".into(), json!(media));
    }
    let keys: Vec<String> = ch.keys().cloned().collect();
    let hlc = w.record(&tx, ENTITY, id, "upsert", Value::Object(ch))?;
    let clock = stamp_fields(&cur.field_clock, keys, &hlc);
    tx.execute("UPDATE letters SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
    tx.commit()?;
    Ok(())
}

/// Soft delete. Only a sent letter you wrote is a shared record; drafts and your copy of
/// someone else's letter are local, so nothing is op-logged for them.
pub fn delete(conn: &mut Connection, w: &Writer, me: &str, id: &str) -> AppResult<()> {
    let cur = load(conn, id)?;
    let now = now_iso();
    let tx = conn.transaction()?;
    tx.execute("UPDATE letters SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1", params![id, now])?;
    if cur.status == "sent" && cur.sender_id == me {
        w.record(&tx, ENTITY, id, "delete", json!({ "deleted_at": now }))?;
    }
    tx.commit()?;
    Ok(())
}

pub fn restore(conn: &mut Connection, w: &Writer, me: &str, id: &str) -> AppResult<()> {
    let now = now_iso();
    let tx = conn.transaction()?;
    let cur: Option<(String, String)> = tx
        .query_row("SELECT status, sender_id FROM letters WHERE id = ?1 AND deleted_at IS NOT NULL", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?;
    let (status, sender) = cur.ok_or_else(|| AppError::invalid("nothing to restore"))?;
    tx.execute("UPDATE letters SET deleted_at = NULL, updated_at = ?2 WHERE id = ?1", params![id, now])?;
    if status == "sent" && sender == me {
        w.record(&tx, ENTITY, id, "restore", json!({}))?;
    }
    tx.commit()?;
    Ok(())
}

pub fn purge_expired(conn: &mut Connection, w: &Writer, retention_days: i64) -> AppResult<usize> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(retention_days))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let rows: Vec<(String, String, String)> = {
        let mut s = conn.prepare(
            "SELECT id, status, sender_id FROM letters WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
        )?;
        let r = s.query_map([cutoff], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        r.collect::<Result<_, _>>()?
    };
    let me: Option<String> = conn.query_row("SELECT id FROM users WHERE is_self = 1", [], |r| r.get(0)).optional()?;
    for (id, status, sender) in &rows {
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM letter_media WHERE letter_id = ?1", [id])?;
        tx.execute("DELETE FROM letters WHERE id = ?1", [id])?;
        if status == "sent" && Some(sender) == me.as_ref() {
            w.record(&tx, ENTITY, id, "purge", json!({}))?;
        }
        tx.commit()?;
    }
    Ok(rows.len())
}

// ---- reading -------------------------------------------------------------------------------

fn name_of(conn: &Connection, id: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT display_name FROM users WHERE id = ?1", [id], |r| r.get(0)).optional()?)
}

fn summarize(conn: &Connection, me: &str, r: &Row) -> AppResult<LetterSummary> {
    let mine = r.sender_id == me;
    let capsule = r.kind == "capsule";
    let sent = r.status == "sent";
    let moment_passed = r.unlock_at.as_deref().is_some_and(|u| !is_future(u));
    let sealed = capsule && sent && !moment_passed && r.unlocked_at.is_none();
    // Someone else's capsule stays closed until it has been opened on this device.
    let opened_here = mine || !capsule || r.unlocked_at.is_some();
    let needs_unlock = !mine && capsule && sent && moment_passed && !opened_here;
    let hidden = !opened_here;
    let title_visible = !hidden || r.show_title;
    let recipient_id = r.recipient_id.clone().or(if mine && !sent { partner_id(conn)? } else { None });
    let recipient_name = match &recipient_id {
        Some(id) => name_of(conn, id)?,
        None => None,
    };
    let (excerpt, photo_count, cover) = if hidden {
        (String::new(), 0, None)
    } else {
        let text = body_text(&r.body_json).unwrap_or_default();
        let mut ex: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if ex.chars().count() > 180 {
            ex = ex.chars().take(180).collect::<String>() + "...";
        }
        let media = media_for(conn, &r.id)?;
        (ex, media.len() as i64, media.first().cloned())
    };
    Ok(LetterSummary {
        id: r.id.clone(),
        sender_id: r.sender_id.clone(),
        sender_name: name_of(conn, &r.sender_id)?.unwrap_or_default(),
        recipient_id,
        recipient_name,
        mine,
        title: if title_visible && !(r.title.is_empty() && hidden) { Some(r.title.clone()) } else { None },
        kind: r.kind.clone(),
        status: r.status.clone(),
        sent_at: r.sent_at.clone(),
        unlock_at: r.unlock_at.clone(),
        show_title: r.show_title,
        lock_kind: r.lock_kind.clone(),
        sealed,
        needs_unlock,
        unlocked_at: r.unlocked_at.clone(),
        excerpt,
        photo_count,
        cover,
        created_at: r.created_at.clone(),
        updated_at: r.updated_at.clone(),
    })
}

fn media_for(conn: &Connection, id: &str) -> AppResult<Vec<MediaRef>> {
    let cols = MEDIA_COLS.split(", ").map(|c| format!("m.{c}")).collect::<Vec<_>>().join(", ");
    let mut s = conn.prepare(&format!(
        "SELECT {cols} FROM letter_media b JOIN media m ON m.id = b.media_id WHERE b.letter_id = ?1 ORDER BY b.position"
    ))?;
    let rows = s.query_map([id], |r| Ok(media_repo::from_row(r, 0)?.r))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Everything you may see: your own letters and drafts, plus what was sent to you.
/// Newest first (drafts by last edit, sent by sent date).
pub fn list(conn: &Connection, me: &str) -> AppResult<Vec<LetterSummary>> {
    let mut s = conn.prepare(&format!(
        "SELECT {ROW_COLS} FROM letters
         WHERE deleted_at IS NULL AND (sender_id = ?1 OR status = 'sent')
         ORDER BY COALESCE(sent_at, updated_at) DESC, id DESC"
    ))?;
    let rows: Vec<Row> = s.query_map([me], row)?.collect::<Result<_, _>>()?;
    rows.iter().map(|r| summarize(conn, me, r)).collect()
}

pub fn get(conn: &Connection, me: &str, id: &str) -> AppResult<LetterDetail> {
    let r = load(conn, id)?;
    if r.sender_id != me && r.status != "sent" {
        return Err(AppError::invalid("this letter doesn't exist anymore"));
    }
    let summary = summarize(conn, me, &r)?;
    let hidden = r.sender_id != me && r.kind == "capsule" && r.unlocked_at.is_none();
    Ok(LetterDetail {
        body_json: if hidden { String::new() } else { r.body_json.clone() },
        media: if hidden { vec![] } else { media_for(conn, id)? },
        summary,
    })
}

// ---- opening someone else's capsule --------------------------------------------------------

/// Checks that a capsule may be opened now and says which beacon round is needed.
/// Returns None when there is nothing to fetch (your own capsule, or already open).
pub fn unlock_plan(conn: &Connection, me: &str, id: &str) -> AppResult<Option<UnlockPlan>> {
    let r = load(conn, id)?;
    if r.kind != "capsule" || r.sender_id == me || r.unlocked_at.is_some() {
        return Ok(None);
    }
    if r.status != "sent" || r.lock_kind != "tlock" {
        return Err(AppError::invalid("this capsule can't be opened"));
    }
    if r.unlock_at.as_deref().is_none_or(is_future) {
        return Err(AppError::invalid("Not quite time yet - this capsule is still sealed."));
    }
    let ct = r.cipher.ok_or_else(|| AppError::invalid("this capsule hasn't fully arrived yet"))?;
    Ok(Some(UnlockPlan { round: timelock::ciphertext_round(&ct)? }))
}

/// Opens the capsule with the beacon signature for its round and keeps the text locally.
/// Local state only: nothing is op-logged, because it is derived from the shared ciphertext.
pub fn unlock_apply(conn: &mut Connection, me: &str, id: &str, signature_hex: &str) -> AppResult<()> {
    let r = load(conn, id)?;
    if r.kind != "capsule" || r.sender_id == me || r.unlocked_at.is_some() {
        return Ok(());
    }
    let ct = r.cipher.ok_or_else(|| AppError::invalid("this capsule hasn't fully arrived yet"))?;
    let plain = timelock::decrypt(&ct, signature_hex)?;
    let v: Value = serde_json::from_slice(&plain).map_err(|_| AppError::invalid("this capsule's contents can't be read"))?;
    let body = v.get("body").and_then(Value::as_str).unwrap_or_default().to_string();
    body_text(&body)?;
    let title = v.get("title").and_then(Value::as_str).unwrap_or_default().to_string();
    let media: Vec<String> = v
        .get("media")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|m| m.as_str().map(String::from)).collect())
        .unwrap_or_default();
    // Photos that haven't reached this device yet are simply left out.
    let mut have = vec![];
    for m in media {
        if media_repo::get(conn, &m)?.is_some() {
            have.push(m);
        }
    }
    let now = now_iso();
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE letters SET title = ?2, body_json = ?3, unlocked_at = ?4 WHERE id = ?1",
        params![id, title, body, now],
    )?;
    write_media(&tx, id, &have)?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::journal::tests as jt;

    const BODY: &str = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"You make ordinary days feel like holidays."}]}]}"#;

    fn future(hours: i64) -> String {
        (chrono::Utc::now() + chrono::Duration::hours(hours)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }

    fn inp(kind: &str) -> LetterInput {
        LetterInput {
            title: "For the road".into(),
            body_json: BODY.into(),
            kind: kind.into(),
            unlock_at: if kind == "capsule" { Some(future(48)) } else { None },
            show_title: true,
            media_ids: vec![],
        }
    }

    fn ops(conn: &Connection) -> Vec<(String, String, String)> {
        let mut s = conn.prepare("SELECT entity, kind, changes_json FROM oplog ORDER BY hlc").unwrap();
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect()
    }

    fn add_partner(conn: &Connection) {
        conn.execute(
            "INSERT INTO users(id, display_name, is_self, created_at, updated_at) VALUES ('her','Maya',0,'t','t')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn validates_input() {
        let (mut conn, _w, me) = jt::setup();
        let mut i = inp("letter");
        i.title = "x".repeat(201);
        assert!(save_draft(&mut conn, &me, None, &i).is_err());
        let mut i = inp("letter");
        i.kind = "postcard".into();
        assert!(save_draft(&mut conn, &me, None, &i).is_err());
        let mut i = inp("letter");
        i.body_json = "not json".into();
        assert!(save_draft(&mut conn, &me, None, &i).is_err());
        let mut i = inp("letter");
        i.media_ids = vec!["nope".into()];
        assert!(save_draft(&mut conn, &me, None, &i).is_err());
        let mut i = inp("capsule");
        i.unlock_at = Some("someday".into());
        assert!(save_draft(&mut conn, &me, None, &i).is_err());
    }

    #[test]
    fn drafts_are_private_and_never_op_logged() {
        let (mut conn, w, me) = jt::setup();
        let id = save_draft(&mut conn, &me, None, &inp("letter")).unwrap();
        let mut i = inp("letter");
        i.title = "Edited".into();
        save_draft(&mut conn, &me, Some(&id), &i).unwrap();
        delete(&mut conn, &w, &me, &id).unwrap();
        restore(&mut conn, &w, &me, &id).unwrap();
        assert!(ops(&conn).is_empty(), "drafts must never reach the op-log");
        let all = list(&conn, &me).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].title.as_deref(), Some("Edited"));
        // Someone else never sees my draft.
        assert!(list(&conn, "her").unwrap().is_empty());
        assert!(get(&conn, "her", &id).is_err());
    }

    #[test]
    fn send_requires_title_text_and_a_future_moment() {
        let (mut conn, w, me) = jt::setup();
        let mut i = inp("letter");
        i.title = "  ".into();
        let id = save_draft(&mut conn, &me, None, &i).unwrap();
        assert!(send(&mut conn, &w, &me, &id).unwrap_err().to_string().contains("title"));
        let mut i = inp("letter");
        i.body_json = r#"{"type":"doc","content":[{"type":"paragraph"}]}"#.into();
        let id = save_draft(&mut conn, &me, None, &i).unwrap();
        assert!(send(&mut conn, &w, &me, &id).is_err());
        let mut i = inp("capsule");
        i.unlock_at = Some((chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339());
        let id = save_draft(&mut conn, &me, None, &i).unwrap();
        assert!(send(&mut conn, &w, &me, &id).unwrap_err().to_string().contains("future"));
        let mut i = inp("capsule");
        i.unlock_at = None;
        let id = save_draft(&mut conn, &me, None, &i).unwrap();
        assert!(send(&mut conn, &w, &me, &id).is_err());
        assert!(ops(&conn).is_empty());
    }

    #[test]
    fn sending_a_letter_op_logs_one_full_upsert() {
        let (mut conn, w, me) = jt::setup();
        add_partner(&conn);
        let m = jt::photo(&conn, "h1");
        let mut i = inp("letter");
        i.media_ids = vec![m.clone()];
        let id = save_draft(&mut conn, &me, None, &i).unwrap();
        send(&mut conn, &w, &me, &id).unwrap();
        let o = ops(&conn);
        assert_eq!(o.len(), 1);
        assert_eq!((o[0].0.as_str(), o[0].1.as_str()), ("letter", "upsert"));
        let ch: Value = serde_json::from_str(&o[0].2).unwrap();
        assert_eq!(ch["title"], "For the road");
        assert_eq!(ch["recipient_id"], "her");
        assert_eq!(ch["status"], "sent");
        assert_eq!(ch["media"], json!([m]));
        assert!(ch["body_json"].as_str().unwrap().contains("holidays"));
        let clock: Value = serde_json::from_str(
            &conn.query_row("SELECT field_clock FROM letters WHERE id=?1", [&id], |r| r.get::<_, String>(0)).unwrap(),
        )
        .unwrap();
        assert!(clock.get("body_json").is_some() && clock.get("status").is_some());
        // Sent letters are final.
        assert!(save_draft(&mut conn, &me, Some(&id), &i).is_err());
        assert!(send(&mut conn, &w, &me, &id).is_err());
        let d = get(&conn, &me, &id).unwrap();
        assert_eq!(d.summary.recipient_name.as_deref(), Some("Maya"));
        assert_eq!(d.media.len(), 1);
    }

    #[test]
    fn sent_without_a_partner_has_no_recipient_yet() {
        let (mut conn, w, me) = jt::setup();
        let id = save_draft(&mut conn, &me, None, &inp("letter")).unwrap();
        send(&mut conn, &w, &me, &id).unwrap();
        let d = get(&conn, &me, &id).unwrap();
        assert!(d.summary.recipient_id.is_none() && d.summary.recipient_name.is_none());
    }

    #[test]
    fn capsule_op_log_has_ciphertext_but_no_plaintext() {
        let (mut conn, w, me) = jt::setup();
        let m = jt::photo(&conn, "h2");
        let mut i = inp("capsule");
        i.show_title = false;
        i.title = "Open on our anniversary".into();
        i.media_ids = vec![m.clone()];
        let id = save_draft(&mut conn, &me, None, &i).unwrap();
        send(&mut conn, &w, &me, &id).unwrap();
        let o = ops(&conn);
        assert_eq!(o.len(), 1);
        let raw = &o[0].2;
        assert!(!raw.contains("holidays"), "plaintext leaked into the op-log");
        assert!(!raw.contains("anniversary"), "hidden title leaked into the op-log");
        assert!(!raw.contains(&m), "attachment list leaked into the op-log");
        let ch: Value = serde_json::from_str(raw).unwrap();
        assert_eq!(ch["lock_kind"], "tlock");
        assert_eq!(ch["title"], "");
        assert!(ch.get("body_json").is_none() && ch.get("media").is_none());
        assert!(ch["tlock_ciphertext"].as_str().unwrap().starts_with("-----BEGIN AGE ENCRYPTED FILE-----"));
        // The sender's own copy still reads fine, labelled as sealed.
        let d = get(&conn, &me, &id).unwrap();
        assert!(d.summary.sealed && d.summary.mine);
        assert!(d.body_json.contains("holidays"));
        assert_eq!(d.summary.title.as_deref(), Some("Open on our anniversary"));
    }

    #[test]
    fn soft_delete_and_restore_of_a_sent_letter_are_logged() {
        let (mut conn, w, me) = jt::setup();
        let id = save_draft(&mut conn, &me, None, &inp("letter")).unwrap();
        send(&mut conn, &w, &me, &id).unwrap();
        delete(&mut conn, &w, &me, &id).unwrap();
        assert!(list(&conn, &me).unwrap().is_empty());
        assert!(delete(&mut conn, &w, &me, &id).is_err());
        restore(&mut conn, &w, &me, &id).unwrap();
        assert_eq!(list(&conn, &me).unwrap().len(), 1);
        let kinds: Vec<String> = ops(&conn).into_iter().map(|o| o.1).collect();
        assert_eq!(kinds, ["upsert", "delete", "restore"]);
        assert!(restore(&mut conn, &w, &me, &id).is_err());
    }

    #[test]
    fn purge_removes_expired_rows_and_logs_only_sent_ones() {
        let (mut conn, w, me) = jt::setup();
        let draft = save_draft(&mut conn, &me, None, &inp("letter")).unwrap();
        let sent = save_draft(&mut conn, &me, None, &inp("letter")).unwrap();
        send(&mut conn, &w, &me, &sent).unwrap();
        delete(&mut conn, &w, &me, &draft).unwrap();
        delete(&mut conn, &w, &me, &sent).unwrap();
        conn.execute("UPDATE letters SET deleted_at = '2000-01-01T00:00:00.000Z'", []).unwrap();
        assert_eq!(purge_expired(&mut conn, &w, 30).unwrap(), 2);
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM letters", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
        let kinds: Vec<String> = ops(&conn).into_iter().map(|o| o.1).collect();
        assert_eq!(kinds, ["upsert", "delete", "purge"]);
    }

    /// A capsule that arrived from the partner (Phase 4 will insert these via sync).
    fn received_capsule(conn: &mut Connection, show_title: bool, unlock: &str, unlock_unix: i64) -> String {
        let payload = json!({ "v": 1, "title": "Secret title", "body": BODY, "media": [] });
        let ct = timelock::encrypt(payload.to_string().as_bytes(), unlock_unix).unwrap();
        conn.execute(
            "INSERT INTO users(id, display_name, is_self, created_at, updated_at) VALUES ('her','Maya',0,'t','t')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO letters(id, sender_id, recipient_id, title, kind, status, sent_at, unlock_at, show_title,
                                 lock_kind, tlock_ciphertext, created_at, updated_at)
             VALUES ('c1','her','me',?1,'capsule','sent','2026-01-01T00:00:00.000Z',?2,?3,'tlock',?4,'t','t')",
            params![if show_title { "Visible title" } else { "" }, unlock, show_title as i64, ct],
        )
        .unwrap();
        "c1".into()
    }

    #[test]
    fn someone_elses_sealed_capsule_hides_text_and_respects_show_title() {
        let (mut conn, _w, me) = jt::setup();
        let id = received_capsule(&mut conn, false, &future(24), chrono::Utc::now().timestamp() + 86_400);
        let d = get(&conn, &me, &id).unwrap();
        assert!(d.summary.sealed && !d.summary.mine && !d.summary.needs_unlock);
        assert!(d.summary.title.is_none(), "title hidden by the sender");
        assert!(d.body_json.is_empty() && d.summary.excerpt.is_empty());
        assert_eq!(d.summary.sender_name, "Maya");
        assert!(unlock_plan(&conn, &me, &id).unwrap_err().to_string().contains("Not quite time"));
        conn.execute("UPDATE letters SET show_title = 1, title = 'Visible title' WHERE id='c1'", []).unwrap();
        assert_eq!(get(&conn, &me, &id).unwrap().summary.title.as_deref(), Some("Visible title"));
    }

    #[test]
    fn unlock_opens_with_the_right_signature_only() {
        let (mut conn, _w, me) = jt::setup();
        // Locked to quicknet round 1000, which exists (real, public signature).
        let past_unix = timelock::round_time(1000);
        let id = received_capsule(&mut conn, false, "2023-08-23T15:09:24.000Z", past_unix);
        let d = get(&conn, &me, &id).unwrap();
        assert!(d.summary.needs_unlock && !d.summary.sealed);
        let plan = unlock_plan(&conn, &me, &id).unwrap().unwrap();
        assert_eq!(plan.round, 1000);
        assert!(unlock_apply(&mut conn, &me, &id, "00ff").is_err());
        assert!(get(&conn, &me, &id).unwrap().body_json.is_empty());
        let sig = "b44679b9a59af2ec876b1a6b1ad52ea9b1615fc3982b19576350f93447cb1125e342b73a8dd2bacbe47e4b6b63ed5e39";
        unlock_apply(&mut conn, &me, &id, sig).unwrap();
        let d = get(&conn, &me, &id).unwrap();
        assert!(d.body_json.contains("holidays"));
        assert_eq!(d.summary.title.as_deref(), Some("Secret title"));
        assert!(d.summary.unlocked_at.is_some() && !d.summary.needs_unlock);
        assert!(unlock_plan(&conn, &me, &id).unwrap().is_none(), "already open");
        assert!(ops(&conn).is_empty(), "unlocking is local and never op-logged");
    }

    #[test]
    fn deleting_my_copy_of_a_received_letter_is_local_only() {
        let (mut conn, w, me) = jt::setup();
        let id = received_capsule(&mut conn, true, &future(24), chrono::Utc::now().timestamp() + 86_400);
        delete(&mut conn, &w, &me, &id).unwrap();
        assert!(ops(&conn).is_empty());
    }
}
