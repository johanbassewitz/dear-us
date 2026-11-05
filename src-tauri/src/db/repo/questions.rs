//! Couple questions: custom questions, answers (revisited over time), the shared "question of the day"
//! and a strictly private mood check-in.
//!
//! The library itself lives in the frontend (`src/data/questions.json`); rows here only reference its ids.
//! A partner's answer is only returned once *I* have answered the same question for the same day.

use super::journal::validate_date;
use crate::db::{new_id, now_iso};
use crate::error::{AppError, AppResult};
use crate::sync::oplog::{stamp_fields, Writer};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};

pub const CUSTOM: &str = "custom_question";
pub const ANSWER: &str = "answer";
pub const DAILY: &str = "daily_question";

pub const CATEGORIES: [&str; 6] = ["favorite-moments", "appreciation", "communication", "plans", "goals", "reflection"];
pub const MOODS: [&str; 6] = ["glowing", "happy", "calm", "tired", "low", "stressed"];

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CustomQuestion {
    pub id: String,
    pub text: String,
    pub category: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    pub id: String,
    pub question_id: String,
    pub user_id: String,
    pub mine: bool,
    pub body: String,
    pub answered_for_date: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DailyQuestion {
    pub id: String,
    pub question_id: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Mood {
    pub date: String,
    pub mood: String,
    pub note: String,
}

// ---------- custom questions ----------

pub fn custom_list(conn: &Connection) -> AppResult<Vec<CustomQuestion>> {
    let mut s = conn.prepare(
        "SELECT id, text, category, created_at FROM custom_questions WHERE deleted_at IS NULL ORDER BY created_at DESC, id DESC",
    )?;
    let rows = s.query_map([], |r| {
        Ok(CustomQuestion { id: r.get(0)?, text: r.get(1)?, category: r.get(2)?, created_at: r.get(3)? })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn custom_create(conn: &mut Connection, w: &Writer, author: &str, text: &str, category: &str) -> AppResult<CustomQuestion> {
    let text = text.trim().to_string();
    if text.is_empty() || text.chars().count() > 300 {
        return Err(AppError::invalid("Write your question first (300 characters max)"));
    }
    if !CATEGORIES.contains(&category) {
        return Err(AppError::invalid("pick a category for your question"));
    }
    let id = new_id();
    let now = now_iso();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO custom_questions(id, text, category, created_by, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?5)",
        params![id, text, category, author, now],
    )?;
    let hlc = w.record(&tx, CUSTOM, &id, "upsert", json!({ "text": text, "category": category, "created_by": author }))?;
    let clock = stamp_fields("{}", ["text".to_string(), "category".to_string()], &hlc);
    tx.execute("UPDATE custom_questions SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
    tx.commit()?;
    Ok(CustomQuestion { id, text, category: category.into(), created_at: now })
}

pub fn custom_delete(conn: &mut Connection, w: &Writer, id: &str) -> AppResult<()> {
    soft_delete(conn, w, "custom_questions", CUSTOM, id, "this question doesn't exist anymore")
}

pub fn custom_restore(conn: &mut Connection, w: &Writer, id: &str) -> AppResult<()> {
    soft_restore(conn, w, "custom_questions", CUSTOM, id)
}

fn soft_delete(conn: &mut Connection, w: &Writer, table: &str, entity: &str, id: &str, msg: &str) -> AppResult<()> {
    let now = now_iso();
    let tx = conn.transaction()?;
    let n = tx.execute(
        &format!("UPDATE {table} SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NULL"),
        params![id, now],
    )?;
    if n == 0 {
        return Err(AppError::invalid(msg));
    }
    w.record(&tx, entity, id, "delete", json!({ "deleted_at": now }))?;
    tx.commit()?;
    Ok(())
}

fn soft_restore(conn: &mut Connection, w: &Writer, table: &str, entity: &str, id: &str) -> AppResult<()> {
    let tx = conn.transaction()?;
    let n = tx.execute(
        &format!("UPDATE {table} SET deleted_at = NULL, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NOT NULL"),
        params![id, now_iso()],
    )?;
    if n == 0 {
        return Err(AppError::invalid("there's nothing to restore"));
    }
    w.record(&tx, entity, id, "restore", json!({ "deleted_at": Value::Null }))?;
    tx.commit()?;
    Ok(())
}

// ---------- answers ----------

const ANSWER_COLS: &str = "id, question_id, user_id, body, answered_for_date, created_at, updated_at";

fn answer_row(r: &rusqlite::Row, me: &str) -> rusqlite::Result<Answer> {
    let user_id: String = r.get(2)?;
    Ok(Answer {
        id: r.get(0)?,
        question_id: r.get(1)?,
        mine: user_id == me,
        user_id,
        body: r.get(3)?,
        answered_for_date: r.get(4)?,
        created_at: r.get(5)?,
        updated_at: r.get(6)?,
    })
}

/// My answers, plus a partner's answer only where I have answered the same question for the same day.
pub fn answer_list(conn: &Connection, me: &str) -> AppResult<Vec<Answer>> {
    let mut s = conn.prepare(&format!(
        "SELECT {ANSWER_COLS} FROM answers a
         WHERE a.deleted_at IS NULL AND (a.user_id = ?1 OR EXISTS (
            SELECT 1 FROM answers m WHERE m.user_id = ?1 AND m.deleted_at IS NULL
              AND m.question_id = a.question_id AND m.answered_for_date = a.answered_for_date))
         ORDER BY a.answered_for_date DESC, a.created_at DESC, a.id DESC"
    ))?;
    let rows = s.query_map([me], |r| answer_row(r, me))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Saves my answer for a question on a day. Answering the same question on a later day adds a new answer;
/// saving again on the same day edits it.
pub fn answer_save(conn: &mut Connection, w: &Writer, me: &str, question_id: &str, body: &str, date: &str) -> AppResult<Answer> {
    let question_id = question_id.trim();
    if question_id.is_empty() || question_id.len() > 100 {
        return Err(AppError::invalid("pick a question to answer"));
    }
    let body = body.trim().to_string();
    if body.is_empty() || body.chars().count() > 5000 {
        return Err(AppError::invalid("Write a few words first (5000 characters max)"));
    }
    let date = validate_date(date)?;
    let existing: Option<(String, String, String)> = conn
        .query_row(
            "SELECT id, body, field_clock FROM answers
             WHERE question_id = ?1 AND user_id = ?2 AND answered_for_date = ?3 AND deleted_at IS NULL",
            params![question_id, me, date],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let tx = conn.transaction()?;
    let id = match existing {
        Some((id, old, _)) if old == body => id,
        Some((id, _, clock)) => {
            tx.execute("UPDATE answers SET body = ?2, updated_at = ?3 WHERE id = ?1", params![id, body, now_iso()])?;
            let hlc = w.record(&tx, ANSWER, &id, "upsert", json!({ "body": body }))?;
            let clock = stamp_fields(&clock, ["body".to_string()], &hlc);
            tx.execute("UPDATE answers SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
            id
        }
        None => {
            let id = new_id();
            let now = now_iso();
            tx.execute(
                "INSERT INTO answers(id, question_id, user_id, body, answered_for_date, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?6)",
                params![id, question_id, me, body, date, now],
            )?;
            let hlc = w.record(
                &tx,
                ANSWER,
                &id,
                "upsert",
                json!({ "question_id": question_id, "user_id": me, "body": body, "answered_for_date": date }),
            )?;
            let clock = stamp_fields(
                "{}",
                ["question_id", "user_id", "body", "answered_for_date"].map(String::from),
                &hlc,
            );
            tx.execute("UPDATE answers SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
            id
        }
    };
    tx.commit()?;
    Ok(conn.query_row(&format!("SELECT {ANSWER_COLS} FROM answers WHERE id = ?1"), [&id], |r| answer_row(r, me))?)
}

pub fn answer_delete(conn: &mut Connection, w: &Writer, me: &str, id: &str) -> AppResult<()> {
    let owner: Option<String> = conn
        .query_row("SELECT user_id FROM answers WHERE id = ?1 AND deleted_at IS NULL", [id], |r| r.get(0))
        .optional()?;
    if owner.as_deref() != Some(me) {
        return Err(AppError::invalid("you can only remove your own answers"));
    }
    soft_delete(conn, w, "answers", ANSWER, id, "this answer doesn't exist anymore")
}

pub fn answer_restore(conn: &mut Connection, w: &Writer, me: &str, id: &str) -> AppResult<()> {
    let row: Option<(String, String, String)> = conn
        .query_row(
            "SELECT user_id, question_id, answered_for_date FROM answers WHERE id = ?1 AND deleted_at IS NOT NULL",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((owner, q, d)) = row else { return Err(AppError::invalid("there's nothing to restore")) };
    if owner != me {
        return Err(AppError::invalid("you can only restore your own answers"));
    }
    let again: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM answers WHERE question_id=?1 AND user_id=?2 AND answered_for_date=?3 AND deleted_at IS NULL)",
        params![q, me, d],
        |r| r.get(0),
    )?;
    if again {
        return Err(AppError::invalid("you've already answered this question again"));
    }
    soft_restore(conn, w, "answers", ANSWER, id)
}

// ---------- question of the day ----------

/// Records the day's question the first time it is shown. The first writer wins (INSERT OR IGNORE),
/// so both partners end up with the same question even if their libraries ever differ.
pub fn daily_ensure(conn: &mut Connection, w: &Writer, date: &str, question_id: &str) -> AppResult<DailyQuestion> {
    let date = validate_date(date)?;
    let question_id = question_id.trim();
    if question_id.is_empty() || question_id.len() > 100 {
        return Err(AppError::invalid("pick a question"));
    }
    let tx = conn.transaction()?;
    let n = tx.execute(
        "INSERT OR IGNORE INTO daily_questions(id, question_id, picked_by_device, created_at) VALUES (?1,?2,?3,?4)",
        params![date, question_id, w.device_id, now_iso()],
    )?;
    if n == 1 {
        w.record(&tx, DAILY, &date, "upsert", json!({ "question_id": question_id, "picked_by_device": w.device_id }))?;
    }
    let q: String = tx.query_row("SELECT question_id FROM daily_questions WHERE id = ?1", [&date], |r| r.get(0))?;
    tx.commit()?;
    Ok(DailyQuestion { id: date, question_id: q })
}

// ---------- private moods (local only, never op-logged) ----------

pub fn mood_list(conn: &Connection, limit: i64) -> AppResult<Vec<Mood>> {
    let mut s = conn.prepare("SELECT date, mood, note FROM moods ORDER BY date DESC LIMIT ?1")?;
    let rows = s.query_map([limit.clamp(1, 400)], |r| Ok(Mood { date: r.get(0)?, mood: r.get(1)?, note: r.get(2)? }))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn mood_save(conn: &Connection, date: &str, mood: &str, note: &str) -> AppResult<Mood> {
    let date = validate_date(date)?;
    if !MOODS.contains(&mood) {
        return Err(AppError::invalid("pick how you're feeling"));
    }
    let note = note.trim().to_string();
    if note.chars().count() > 1000 {
        return Err(AppError::invalid("keep the note under 1000 characters"));
    }
    conn.execute(
        "INSERT INTO moods(id, date, mood, note, created_at) VALUES (?1,?2,?3,?4,?5)
         ON CONFLICT(date) DO UPDATE SET mood = excluded.mood, note = excluded.note",
        params![new_id(), date, mood, note, now_iso()],
    )?;
    Ok(Mood { date, mood: mood.into(), note })
}

pub fn mood_delete(conn: &Connection, date: &str) -> AppResult<()> {
    conn.execute("DELETE FROM moods WHERE date = ?1", [date])?;
    Ok(())
}

// ---------- retention ----------

pub fn purge_expired(conn: &mut Connection, w: &Writer, retention_days: i64) -> AppResult<usize> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(retention_days))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let mut total = 0;
    for (table, entity) in [("answers", ANSWER), ("custom_questions", CUSTOM)] {
        let ids: Vec<String> = {
            let mut s = conn.prepare(&format!("SELECT id FROM {table} WHERE deleted_at IS NOT NULL AND deleted_at < ?1"))?;
            let r = s.query_map([&cutoff], |r| r.get(0))?;
            r.collect::<Result<_, _>>()?
        };
        for id in &ids {
            let tx = conn.transaction()?;
            tx.execute(&format!("DELETE FROM {table} WHERE id = ?1"), [id])?;
            w.record(&tx, entity, id, "purge", json!({}))?;
            tx.commit()?;
        }
        total += ids.len();
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::journal::tests as jt;

    fn op_count(conn: &Connection, entity: &str) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM oplog WHERE entity = ?1", [entity], |r| r.get(0)).unwrap()
    }

    #[test]
    fn custom_questions_validate_log_and_soft_delete() {
        let (mut conn, w, me) = jt::setup();
        assert!(custom_create(&mut conn, &w, &me, "  ", "goals").is_err());
        assert!(custom_create(&mut conn, &w, &me, "Why us?", "nope").is_err());
        let q = custom_create(&mut conn, &w, &me, " What made you smile today? ", "reflection").unwrap();
        assert_eq!(q.text, "What made you smile today?");
        assert_eq!(op_count(&conn, CUSTOM), 1);
        custom_delete(&mut conn, &w, &q.id).unwrap();
        assert!(custom_list(&conn).unwrap().is_empty());
        assert!(custom_delete(&mut conn, &w, &q.id).is_err());
        custom_restore(&mut conn, &w, &q.id).unwrap();
        assert_eq!(custom_list(&conn).unwrap().len(), 1);
        assert_eq!(op_count(&conn, CUSTOM), 3);
    }

    #[test]
    fn answers_validate_edit_same_day_and_revisit_later() {
        let (mut conn, w, me) = jt::setup();
        assert!(answer_save(&mut conn, &w, &me, "q1", "   ", "2026-11-03").is_err());
        assert!(answer_save(&mut conn, &w, &me, "q1", "hi", "11/03").is_err());
        assert!(answer_save(&mut conn, &w, &me, "", "hi", "2026-11-03").is_err());
        let a = answer_save(&mut conn, &w, &me, "q1", "Our beach day", "2026-11-03").unwrap();
        let again = answer_save(&mut conn, &w, &me, "q1", "Our beach day, and the ice cream", "2026-11-03").unwrap();
        assert_eq!(a.id, again.id);
        let later = answer_save(&mut conn, &w, &me, "q1", "Now I'd say the road trip", "2026-12-01").unwrap();
        assert_ne!(a.id, later.id);
        let list = answer_list(&conn, &me).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].answered_for_date, "2026-12-01");
        assert!(list.iter().all(|a| a.mine));
        assert_eq!(op_count(&conn, ANSWER), 3); // create, edit, create
        // saving identical text writes nothing
        answer_save(&mut conn, &w, &me, "q1", "Now I'd say the road trip", "2026-12-01").unwrap();
        assert_eq!(op_count(&conn, ANSWER), 3);
    }

    #[test]
    fn answers_soft_delete_and_restore() {
        let (mut conn, w, me) = jt::setup();
        let a = answer_save(&mut conn, &w, &me, "q1", "x", "2026-11-03").unwrap();
        answer_delete(&mut conn, &w, &me, &a.id).unwrap();
        assert!(answer_list(&conn, &me).unwrap().is_empty());
        answer_restore(&mut conn, &w, &me, &a.id).unwrap();
        assert_eq!(answer_list(&conn, &me).unwrap().len(), 1);
        // not restorable once a fresh answer exists for the same day
        answer_delete(&mut conn, &w, &me, &a.id).unwrap();
        answer_save(&mut conn, &w, &me, "q1", "y", "2026-11-03").unwrap();
        assert!(answer_restore(&mut conn, &w, &me, &a.id).is_err());
    }

    #[test]
    fn partner_answer_is_hidden_until_i_answer_the_same_question_that_day() {
        let (mut conn, w, me) = jt::setup();
        conn.execute(
            "INSERT INTO users(id, display_name, is_self, created_at, updated_at) VALUES ('sam','Sam',0,'t','t')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO answers(id, question_id, user_id, body, answered_for_date, created_at, updated_at)
             VALUES ('p1','q1','sam','Her answer','2026-11-03','t','t')",
            [],
        )
        .unwrap();
        assert!(answer_list(&conn, &me).unwrap().is_empty());
        answer_save(&mut conn, &w, &me, "q1", "Mine", "2026-11-04").unwrap();
        assert_eq!(answer_list(&conn, &me).unwrap().len(), 1); // different day: still hidden
        answer_save(&mut conn, &w, &me, "q1", "Mine", "2026-11-03").unwrap();
        let list = answer_list(&conn, &me).unwrap();
        assert_eq!(list.len(), 3);
        assert!(list.iter().any(|a| !a.mine && a.body == "Her answer"));
        // the partner's answer can't be removed from here
        assert!(answer_delete(&mut conn, &w, &me, "p1").is_err());
    }

    #[test]
    fn daily_question_is_first_writer_wins_and_logged_once() {
        let (mut conn, w, _) = jt::setup();
        assert!(daily_ensure(&mut conn, &w, "today", "q1").is_err());
        let a = daily_ensure(&mut conn, &w, "2026-11-03", "favorite-moments-01").unwrap();
        let b = daily_ensure(&mut conn, &w, "2026-11-03", "plans-02").unwrap();
        assert_eq!(a.question_id, "favorite-moments-01");
        assert_eq!(b.question_id, "favorite-moments-01");
        assert_eq!(op_count(&conn, DAILY), 1);
    }

    #[test]
    fn moods_are_private_validated_and_never_op_logged() {
        let (conn, _, _) = jt::setup();
        assert!(mood_save(&conn, "2026-11-03", "furious", "").is_err());
        assert!(mood_save(&conn, "nope", "calm", "").is_err());
        mood_save(&conn, "2026-11-03", "calm", " quiet day ").unwrap();
        mood_save(&conn, "2026-11-03", "happy", "").unwrap();
        mood_save(&conn, "2026-11-04", "tired", "long one").unwrap();
        let list = mood_list(&conn, 30).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!((list[1].mood.as_str(), list[1].note.as_str()), ("happy", ""));
        mood_delete(&conn, "2026-11-04").unwrap();
        assert_eq!(mood_list(&conn, 30).unwrap().len(), 1);
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM oplog", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn purge_removes_only_expired_soft_deleted_rows() {
        let (mut conn, w, me) = jt::setup();
        let a = answer_save(&mut conn, &w, &me, "q1", "x", "2026-11-03").unwrap();
        let keep = answer_save(&mut conn, &w, &me, "q2", "y", "2026-11-03").unwrap();
        answer_delete(&mut conn, &w, &me, &a.id).unwrap();
        assert_eq!(purge_expired(&mut conn, &w, 30).unwrap(), 0);
        conn.execute("UPDATE answers SET deleted_at = '2000-01-01T00:00:00.000Z' WHERE id = ?1", [&a.id]).unwrap();
        assert_eq!(purge_expired(&mut conn, &w, 30).unwrap(), 1);
        let left = answer_list(&conn, &me).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, keep.id);
    }
}
