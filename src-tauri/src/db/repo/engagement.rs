//! Reactions and comments on journal entries. Both are shared records and are op-logged.

use crate::db::{new_id, now_iso};
use crate::error::{AppError, AppResult};
use crate::sync::oplog::{stamp_fields, Writer};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::json;

pub const REACTION_ENTITY: &str = "reaction";
pub const COMMENT_ENTITY: &str = "comment";
pub const REACTION_TYPES: [&str; 5] = ["love", "emotional", "funny", "adorable", "special"];
const MAX_COMMENT: usize = 2000;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReactionCount {
    #[serde(rename = "type")]
    pub kind: String,
    pub count: i64,
    pub mine: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub entry_id: String,
    pub author_id: String,
    pub author_name: Option<String>,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
    pub edited: bool,
}

fn require_entry(conn: &Connection, entry_id: &str) -> AppResult<()> {
    let ok: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM journal_entries WHERE id = ?1 AND deleted_at IS NULL)",
        [entry_id],
        |r| r.get(0),
    )?;
    if ok {
        Ok(())
    } else {
        Err(AppError::invalid("this memory doesn't exist anymore"))
    }
}

// ---------- reactions ----------

/// One row per reaction type, with how many people used it and whether I did.
pub fn reactions(conn: &Connection, entry_id: &str, me: &str) -> AppResult<Vec<ReactionCount>> {
    REACTION_TYPES
        .iter()
        .map(|t| {
            let (count, mine): (i64, i64) = conn.query_row(
                "SELECT COUNT(*), COALESCE(SUM(user_id = ?3), 0) FROM reactions
                 WHERE entry_id = ?1 AND type = ?2 AND deleted_at IS NULL",
                params![entry_id, t, me],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            Ok(ReactionCount { kind: t.to_string(), count, mine: mine > 0 })
        })
        .collect()
}

/// Adds my reaction, or removes it if I already had it.
pub fn toggle_reaction(
    conn: &mut Connection,
    w: &Writer,
    entry_id: &str,
    me: &str,
    kind: &str,
) -> AppResult<Vec<ReactionCount>> {
    if !REACTION_TYPES.contains(&kind) {
        return Err(AppError::invalid("unknown reaction"));
    }
    require_entry(conn, entry_id)?;
    let now = now_iso();
    let tx = conn.transaction()?;
    let existing: Option<String> = tx
        .query_row(
            "SELECT id FROM reactions WHERE entry_id = ?1 AND user_id = ?2 AND type = ?3 AND deleted_at IS NULL",
            params![entry_id, me, kind],
            |r| r.get(0),
        )
        .optional()?;
    match existing {
        Some(id) => {
            tx.execute("UPDATE reactions SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1", params![id, now])?;
            w.record(&tx, REACTION_ENTITY, &id, "delete", json!({ "deleted_at": now }))?;
        }
        None => {
            let id = new_id();
            tx.execute(
                "INSERT INTO reactions(id, entry_id, user_id, type, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?5)",
                params![id, entry_id, me, kind, now],
            )?;
            let hlc = w.record(
                &tx,
                REACTION_ENTITY,
                &id,
                "upsert",
                json!({ "entry_id": entry_id, "user_id": me, "type": kind, "created_at": now }),
            )?;
            let clock = stamp_fields("{}", ["entry_id", "user_id", "type"].map(String::from), &hlc);
            tx.execute("UPDATE reactions SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
        }
    }
    tx.commit()?;
    reactions(conn, entry_id, me)
}

// ---------- comments ----------

fn clean_body(body: &str) -> AppResult<String> {
    let body = body.trim();
    if body.is_empty() {
        return Err(AppError::invalid("Write a few words first ♡"));
    }
    if body.chars().count() > MAX_COMMENT {
        return Err(AppError::invalid("that comment is too long (2000 characters max)"));
    }
    Ok(body.to_string())
}

fn get_comment(conn: &Connection, id: &str) -> AppResult<Comment> {
    conn.query_row(
        "SELECT k.id, k.entry_id, k.author_id, u.display_name, k.body, k.created_at, k.updated_at
         FROM comments k LEFT JOIN users u ON u.id = k.author_id
         WHERE k.id = ?1 AND k.deleted_at IS NULL",
        [id],
        from_row,
    )
    .optional()?
    .ok_or_else(|| AppError::invalid("this comment doesn't exist anymore"))
}

fn from_row(r: &rusqlite::Row) -> rusqlite::Result<Comment> {
    let created_at: String = r.get(5)?;
    let updated_at: String = r.get(6)?;
    Ok(Comment {
        id: r.get(0)?,
        entry_id: r.get(1)?,
        author_id: r.get(2)?,
        author_name: r.get(3)?,
        body: r.get(4)?,
        edited: updated_at != created_at,
        created_at,
        updated_at,
    })
}

pub fn comments(conn: &Connection, entry_id: &str) -> AppResult<Vec<Comment>> {
    let mut stmt = conn.prepare(
        "SELECT k.id, k.entry_id, k.author_id, u.display_name, k.body, k.created_at, k.updated_at
         FROM comments k LEFT JOIN users u ON u.id = k.author_id
         WHERE k.entry_id = ?1 AND k.deleted_at IS NULL ORDER BY k.created_at, k.id",
    )?;
    let rows = stmt.query_map([entry_id], from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn add_comment(conn: &mut Connection, w: &Writer, entry_id: &str, me: &str, body: &str) -> AppResult<Comment> {
    let body = clean_body(body)?;
    require_entry(conn, entry_id)?;
    let id = new_id();
    let now = now_iso();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO comments(id, entry_id, author_id, body, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?5)",
        params![id, entry_id, me, body, now],
    )?;
    let hlc = w.record(
        &tx,
        COMMENT_ENTITY,
        &id,
        "upsert",
        json!({ "entry_id": entry_id, "author_id": me, "body": body, "created_at": now }),
    )?;
    let clock = stamp_fields("{}", ["entry_id", "author_id", "body"].map(String::from), &hlc);
    tx.execute("UPDATE comments SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
    tx.commit()?;
    get_comment(conn, &id)
}

fn require_author(conn: &Connection, id: &str, me: &str) -> AppResult<(String, String)> {
    let (author, clock): (String, String) = conn
        .query_row("SELECT author_id, field_clock FROM comments WHERE id = ?1 AND deleted_at IS NULL", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?
        .ok_or_else(|| AppError::invalid("this comment doesn't exist anymore"))?;
    if author != me {
        return Err(AppError::invalid("You can only change your own comments"));
    }
    Ok((author, clock))
}

pub fn update_comment(conn: &mut Connection, w: &Writer, id: &str, me: &str, body: &str) -> AppResult<Comment> {
    let body = clean_body(body)?;
    let (_, clock) = require_author(conn, id, me)?;
    let current = get_comment(conn, id)?;
    if current.body == body {
        return Ok(current);
    }
    let now = now_iso();
    let tx = conn.transaction()?;
    tx.execute("UPDATE comments SET body = ?2, updated_at = ?3 WHERE id = ?1", params![id, body, now])?;
    let hlc = w.record(&tx, COMMENT_ENTITY, id, "upsert", json!({ "body": body }))?;
    let clock = stamp_fields(&clock, ["body".to_string()], &hlc);
    tx.execute("UPDATE comments SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
    tx.commit()?;
    get_comment(conn, id)
}

pub fn delete_comment(conn: &mut Connection, w: &Writer, id: &str, me: &str) -> AppResult<()> {
    require_author(conn, id, me)?;
    let now = now_iso();
    let tx = conn.transaction()?;
    tx.execute("UPDATE comments SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1", params![id, now])?;
    w.record(&tx, COMMENT_ENTITY, id, "delete", json!({ "deleted_at": now }))?;
    tx.commit()?;
    Ok(())
}

pub fn restore_comment(conn: &mut Connection, w: &Writer, id: &str, me: &str) -> AppResult<Comment> {
    let now = now_iso();
    let tx = conn.transaction()?;
    let n = tx.execute(
        "UPDATE comments SET deleted_at = NULL, updated_at = ?3 WHERE id = ?1 AND author_id = ?2 AND deleted_at IS NOT NULL",
        params![id, me, now],
    )?;
    if n == 0 {
        return Err(AppError::invalid("nothing to restore"));
    }
    w.record(&tx, COMMENT_ENTITY, id, "restore", json!({}))?;
    tx.commit()?;
    get_comment(conn, id)
}

/// Permanently removes soft-deleted reactions and comments older than the retention period.
pub fn purge_expired(conn: &mut Connection, w: &Writer, retention_days: i64) -> AppResult<usize> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(retention_days))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let mut total = 0;
    for (table, entity) in [("reactions", REACTION_ENTITY), ("comments", COMMENT_ENTITY)] {
        let ids: Vec<String> = {
            let mut s =
                conn.prepare(&format!("SELECT id FROM {table} WHERE deleted_at IS NOT NULL AND deleted_at < ?1"))?;
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
    use crate::db::repo::journal::{self, tests::{input, photo, setup}, EntryMediaInput};

    fn op_kinds(conn: &Connection, entity: &str) -> Vec<String> {
        let mut s = conn.prepare("SELECT kind FROM oplog WHERE entity = ?1 ORDER BY hlc").unwrap();
        s.query_map([entity], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
    }

    fn entry(conn: &mut Connection, w: &Writer, me: &str) -> String {
        journal::create(conn, w, me, &input("Beach day", "2025-07-12")).unwrap().summary.id
    }

    #[test]
    fn toggle_adds_then_removes_and_logs() {
        let (mut conn, w, me) = setup();
        let e = entry(&mut conn, &w, &me);
        let r = toggle_reaction(&mut conn, &w, &e, &me, "love").unwrap();
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], ReactionCount { kind: "love".into(), count: 1, mine: true });
        let r = toggle_reaction(&mut conn, &w, &e, &me, "love").unwrap();
        assert_eq!(r[0].count, 0);
        assert!(!r[0].mine);
        // toggling again creates a fresh active row; only one active at a time
        toggle_reaction(&mut conn, &w, &e, &me, "love").unwrap();
        let active: i64 = conn
            .query_row("SELECT COUNT(*) FROM reactions WHERE deleted_at IS NULL", [], |r| r.get(0))
            .unwrap();
        assert_eq!(active, 1);
        assert_eq!(op_kinds(&conn, REACTION_ENTITY), vec!["upsert", "delete", "upsert"]);
        assert_eq!(journal::get(&conn, &e).unwrap().summary.reaction_count, 1);
    }

    #[test]
    fn reaction_validation() {
        let (mut conn, w, me) = setup();
        let e = entry(&mut conn, &w, &me);
        assert!(toggle_reaction(&mut conn, &w, &e, &me, "angry").is_err());
        assert!(toggle_reaction(&mut conn, &w, "nope", &me, "love").is_err());
        assert!(op_kinds(&conn, REACTION_ENTITY).is_empty());
    }

    #[test]
    fn other_users_reactions_count_but_are_not_mine() {
        let (mut conn, w, me) = setup();
        let e = entry(&mut conn, &w, &me);
        toggle_reaction(&mut conn, &w, &e, "partner", "funny").unwrap();
        let r = toggle_reaction(&mut conn, &w, &e, &me, "funny").unwrap();
        assert_eq!(r[2].count, 2);
        assert!(r[2].mine);
        let r = reactions(&conn, &e, "partner").unwrap();
        assert!(r[2].mine);
    }

    #[test]
    fn comment_add_edit_delete_restore() {
        let (mut conn, w, me) = setup();
        let e = entry(&mut conn, &w, &me);
        assert!(add_comment(&mut conn, &w, &e, &me, "   ").is_err());
        assert!(add_comment(&mut conn, &w, &e, &me, &"x".repeat(2001)).is_err());
        let c = add_comment(&mut conn, &w, &e, &me, " best day ").unwrap();
        assert_eq!(c.body, "best day");
        assert_eq!(c.author_name.as_deref(), Some("Jo"));
        assert!(!c.edited);
        std::thread::sleep(std::time::Duration::from_millis(3));
        let c2 = update_comment(&mut conn, &w, &c.id, &me, "best day ever").unwrap();
        assert!(c2.edited);
        // unchanged text logs nothing
        update_comment(&mut conn, &w, &c.id, &me, "best day ever").unwrap();
        assert_eq!(journal::get(&conn, &e).unwrap().summary.comment_count, 1);
        delete_comment(&mut conn, &w, &c.id, &me).unwrap();
        assert!(comments(&conn, &e).unwrap().is_empty());
        assert_eq!(journal::get(&conn, &e).unwrap().summary.comment_count, 0);
        restore_comment(&mut conn, &w, &c.id, &me).unwrap();
        assert_eq!(comments(&conn, &e).unwrap().len(), 1);
        assert_eq!(op_kinds(&conn, COMMENT_ENTITY), vec!["upsert", "upsert", "delete", "restore"]);
    }

    #[test]
    fn only_the_author_can_edit_or_delete() {
        let (mut conn, w, me) = setup();
        let e = entry(&mut conn, &w, &me);
        let c = add_comment(&mut conn, &w, &e, "partner", "hi love").unwrap();
        assert!(update_comment(&mut conn, &w, &c.id, &me, "hijack").is_err());
        assert!(delete_comment(&mut conn, &w, &c.id, &me).is_err());
        assert_eq!(comments(&conn, &e).unwrap()[0].body, "hi love");
        assert_eq!(op_kinds(&conn, COMMENT_ENTITY).len(), 1);
    }

    #[test]
    fn purge_expired_removes_old_soft_deleted_rows() {
        let (mut conn, w, me) = setup();
        let e = entry(&mut conn, &w, &me);
        let c = add_comment(&mut conn, &w, &e, &me, "old").unwrap();
        delete_comment(&mut conn, &w, &c.id, &me).unwrap();
        toggle_reaction(&mut conn, &w, &e, &me, "love").unwrap();
        toggle_reaction(&mut conn, &w, &e, &me, "love").unwrap();
        assert_eq!(purge_expired(&mut conn, &w, 30).unwrap(), 0);
        conn.execute("UPDATE comments SET deleted_at = '2000-01-01T00:00:00.000Z'", []).unwrap();
        conn.execute("UPDATE reactions SET deleted_at = '2000-01-01T00:00:00.000Z' WHERE deleted_at IS NOT NULL", [])
            .unwrap();
        assert_eq!(purge_expired(&mut conn, &w, 30).unwrap(), 2);
    }

    #[test]
    fn purging_an_entry_removes_its_reactions_and_comments() {
        let (mut conn, w, me) = setup();
        let e = entry(&mut conn, &w, &me);
        add_comment(&mut conn, &w, &e, &me, "hi").unwrap();
        toggle_reaction(&mut conn, &w, &e, &me, "love").unwrap();
        journal::delete(&mut conn, &w, &e).unwrap();
        journal::purge(&mut conn, &w, &e).unwrap();
        let n: i64 = conn.query_row("SELECT (SELECT COUNT(*) FROM comments)+(SELECT COUNT(*) FROM reactions)", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn scrapbook_styles_round_trip_validate_and_log() {
        let (mut conn, w, me) = setup();
        let p = photo(&conn, "h1");
        let mut i = input("Styled", "2025-07-12");
        i.media = vec![EntryMediaInput { id: p, caption: None }];
        i.layout = "collage".into();
        i.frame = "tape".into();
        i.paper = "kraft".into();
        let e = journal::create(&mut conn, &w, &me, &i).unwrap();
        assert_eq!((e.layout.as_str(), e.frame.as_str(), e.paper.as_str()), ("collage", "tape", "kraft"));
        i.paper = "dots".into();
        let e2 = journal::update(&mut conn, &w, &e.summary.id, &i).unwrap();
        assert_eq!(e2.paper, "dots");
        let last: String = conn.query_row("SELECT changes_json FROM oplog ORDER BY hlc DESC LIMIT 1", [], |r| r.get(0)).unwrap();
        assert_eq!(last, r#"{"paper":"dots"}"#);
        i.layout = "weird".into();
        assert!(journal::create(&mut conn, &w, &me, &i).is_err());
        i.layout = "auto".into();
        i.frame = "glitter".into();
        assert!(journal::update(&mut conn, &w, &e.summary.id, &i).is_err());
    }
}
