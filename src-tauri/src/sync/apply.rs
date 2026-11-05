//! Applies a partner's changes to the local archive.
//!
//! Rules (docs/PLAN.md §6.2):
//! - per-field last-writer-wins by HLC (field_clock), so independent fields merge cleanly;
//! - long text edited on both PCs while apart: the newer one wins AND the other version is
//!   kept in `sync_conflicts` for the couple to look at — nothing is silently lost;
//! - deletes are soft (recovery bin); purges only remove rows already in the bin;
//! - every op is applied at most once (`sync_applied`), so re-delivery is harmless;
//! - field names are checked against the real table columns — a hostile payload can't
//!   inject SQL or touch columns that don't exist.
//!
//! Applying remote ops never writes to the op-log, so nothing echoes back.

use crate::db::{new_id, now_iso};
use crate::error::{AppError, AppResult};
use rusqlite::types::Value as Sql;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Op {
    pub op_id: String,
    pub hlc: String,
    pub device_id: String,
    pub entity: String,
    pub entity_id: String,
    pub kind: String,
    pub changes: Value,
}

#[derive(Clone, Copy)]
enum List {
    /// `[{id, caption}]` → entry_media
    EntryMedia,
    /// `[id, …]` → (table, parent column)
    Ids(&'static str, &'static str),
}

struct Spec {
    entity: &'static str,
    table: &'static str,
    lists: &'static [(&'static str, List)],
    /// change key → JSON column
    json_cols: &'static [(&'static str, &'static str)],
    long_text: &'static [&'static str],
}

const SPECS: &[Spec] = &[
    Spec { entity: "journal_entry", table: "journal_entries", lists: &[("media", List::EntryMedia)], json_cols: &[("tags", "tags_json")], long_text: &["body_json"] },
    Spec { entity: "milestone", table: "milestones", lists: &[], json_cols: &[], long_text: &["description"] },
    Spec { entity: "bucket_item", table: "bucket_items", lists: &[("media", List::Ids("bucket_media", "item_id"))], json_cols: &[], long_text: &["description"] },
    Spec { entity: "special_date", table: "special_dates", lists: &[], json_cols: &[], long_text: &["description"] },
    Spec { entity: "planned_date", table: "planned_dates", lists: &[], json_cols: &[], long_text: &["description"] },
    Spec { entity: "date_idea", table: "date_ideas", lists: &[], json_cols: &[], long_text: &[] },
    Spec { entity: "custom_question", table: "custom_questions", lists: &[], json_cols: &[], long_text: &[] },
    Spec { entity: "answer", table: "answers", lists: &[], json_cols: &[], long_text: &["body"] },
    Spec { entity: "letter", table: "letters", lists: &[("media", List::Ids("letter_media", "letter_id"))], json_cols: &[], long_text: &[] },
    Spec { entity: "reaction", table: "reactions", lists: &[], json_cols: &[], long_text: &[] },
    Spec { entity: "comment", table: "comments", lists: &[], json_cols: &[], long_text: &["body"] },
];

fn spec(entity: &str) -> Option<&'static Spec> {
    SPECS.iter().find(|s| s.entity == entity)
}

/// Columns of a table (whitelist for incoming field names).
fn columns(conn: &Connection, table: &str) -> AppResult<HashSet<String>> {
    let mut s = conn.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
    let cols = s.query_map([], |r| r.get::<_, String>(1))?.collect::<Result<_, _>>()?;
    Ok(cols)
}

fn to_sql(v: &Value) -> Sql {
    match v {
        Value::Null => Sql::Null,
        Value::Bool(b) => Sql::Integer(*b as i64),
        Value::Number(n) => n.as_i64().map(Sql::Integer).unwrap_or_else(|| Sql::Real(n.as_f64().unwrap_or(0.0))),
        Value::String(s) => Sql::Text(s.clone()),
        other => Sql::Text(other.to_string()),
    }
}

fn hlc_device(hlc: &str) -> &str {
    hlc.splitn(3, '-').nth(2).unwrap_or("")
}

/// Was our local write of a field still unconfirmed by the partner? Then the incoming
/// change was made without seeing it — a genuine concurrent edit.
fn local_unconfirmed(conn: &Connection, local_hlc: &str, my_device: &str) -> AppResult<bool> {
    if hlc_device(local_hlc) != my_device {
        return Ok(false);
    }
    let state: Option<String> = conn
        .query_row("SELECT state FROM oplog WHERE hlc = ?1", [local_hlc], |r| r.get(0))
        .optional()?;
    Ok(state.map(|s| s != "acked").unwrap_or(false))
}

fn record_conflict(conn: &Connection, op: &Op, field: &str, kept: &Value, other: &Value, other_device: &str) -> AppResult<()> {
    let txt = |v: &Value| match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        o => o.to_string(),
    };
    conn.execute(
        "INSERT INTO sync_conflicts(id, entity, entity_id, field, kept_value, other_value, other_device, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![new_id(), op.entity, op.entity_id, field, txt(kept), txt(other), other_device, now_iso()],
    )?;
    Ok(())
}

fn current_value(conn: &Connection, table: &str, col: &str, id: &str) -> AppResult<Value> {
    let v: Sql = conn.query_row(&format!("SELECT \"{col}\" FROM \"{table}\" WHERE id = ?1"), [id], |r| r.get(0))?;
    Ok(match v {
        Sql::Null => Value::Null,
        Sql::Integer(i) => Value::from(i),
        Sql::Real(f) => Value::from(f),
        Sql::Text(s) => Value::String(s),
        Sql::Blob(_) => Value::Null,
    })
}

fn write_list(conn: &Connection, list: List, parent: &str, v: &Value) -> AppResult<()> {
    let items = v.as_array().cloned().unwrap_or_default();
    match list {
        List::EntryMedia => {
            conn.execute("DELETE FROM entry_media WHERE entry_id = ?1", [parent])?;
            for (pos, it) in items.iter().enumerate() {
                let Some(mid) = it.get("id").and_then(Value::as_str) else { continue };
                ensure_media_stub(conn, mid)?;
                conn.execute(
                    "INSERT OR IGNORE INTO entry_media(entry_id, media_id, position, caption) VALUES (?1,?2,?3,?4)",
                    params![parent, mid, pos as i64, it.get("caption").and_then(Value::as_str)],
                )?;
            }
        }
        List::Ids(table, col) => {
            conn.execute(&format!("DELETE FROM {table} WHERE {col} = ?1"), [parent])?;
            for (pos, it) in items.iter().enumerate() {
                let Some(mid) = it.as_str() else { continue };
                ensure_media_stub(conn, mid)?;
                conn.execute(
                    &format!("INSERT OR IGNORE INTO {table}({col}, media_id, position) VALUES (?1,?2,?3)"),
                    params![parent, mid, pos as i64],
                )?;
            }
        }
    }
    Ok(())
}

/// A record may reference a photo whose details haven't arrived yet: keep the reference
/// valid with a placeholder row that the media manifest fills in later.
fn ensure_media_stub(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute(
        "INSERT OR IGNORE INTO media(id, hash, kind, mime, ext, size, availability, created_at)
         VALUES (?1, 'pending:' || ?1, 'photo', 'application/octet-stream', 'bin', 0, 'pending', ?2)",
        params![id, now_iso()],
    )?;
    Ok(())
}

/// Journal entries keep derived search text; rebuild it after a remote change.
fn after_journal(conn: &Connection, id: &str) -> AppResult<()> {
    let row: Option<(String, String, String, Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT title, body_json, tags_json, location, deleted_at FROM journal_entries WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    conn.execute("DELETE FROM journal_fts WHERE entry_id = ?1", [id])?;
    let Some((title, body_json, tags_json, location, deleted)) = row else { return Ok(()) };
    let text = crate::db::repo::journal::body_text(&body_json).unwrap_or_default();
    conn.execute("UPDATE journal_entries SET body_text = ?2 WHERE id = ?1", params![id, text])?;
    if deleted.is_none() {
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        conn.execute(
            "INSERT INTO journal_fts(entry_id, title, body, tags, location) VALUES (?1,?2,?3,?4,?5)",
            params![id, title, text, tags.join(" "), location.unwrap_or_default()],
        )?;
    }
    Ok(())
}

fn upsert(conn: &Connection, s: &Spec, op: &Op, my_device: &str) -> AppResult<()> {
    let changes = op.changes.as_object().ok_or_else(|| AppError::invalid("changes must be an object"))?;
    let cols = columns(conn, s.table)?;
    let existing: Option<String> = conn
        .query_row(&format!("SELECT field_clock FROM \"{}\" WHERE id = ?1", s.table), [&op.entity_id], |r| r.get(0))
        .optional()?;
    let mut clock: Map<String, Value> = existing.as_deref().and_then(|c| serde_json::from_str(c).ok()).unwrap_or_default();
    let now = now_iso();

    // Map each change key to what it writes.
    let mut plain: Vec<(String, Value)> = Vec::new();
    let mut lists: Vec<(List, Value, String)> = Vec::new();
    for (k, v) in changes {
        if let Some((_, l)) = s.lists.iter().find(|(key, _)| key == k) {
            lists.push((*l, v.clone(), k.clone()));
        } else if let Some((_, col)) = s.json_cols.iter().find(|(key, _)| key == k) {
            plain.push((col.to_string(), Value::String(v.to_string())));
        } else if cols.contains(k) && !matches!(k.as_str(), "id" | "field_clock" | "updated_at" | "deleted_at") {
            plain.push((k.clone(), v.clone()));
        } // unknown keys are ignored (e.g. a newer app version's field)
    }

    if existing.is_none() {
        let mut names = vec!["id".to_string()];
        let mut vals = vec![Sql::Text(op.entity_id.clone())];
        for (c, v) in &plain {
            names.push(c.clone());
            vals.push(to_sql(v));
        }
        for (c, v) in [("created_at", &now), ("updated_at", &now)] {
            if cols.contains(c) && !names.iter().any(|n| n == c) {
                names.push(c.into());
                vals.push(Sql::Text(v.clone()));
            }
        }
        let quoted: Vec<String> = names.iter().map(|n| format!("\"{n}\"")).collect();
        let marks: Vec<String> = (1..=names.len()).map(|i| format!("?{i}")).collect();
        conn.execute(
            &format!("INSERT INTO \"{}\"({}) VALUES ({})", s.table, quoted.join(","), marks.join(",")),
            params_from_iter(vals),
        )?;
        for (l, v, _) in &lists {
            write_list(conn, *l, &op.entity_id, v)?;
        }
        for k in changes.keys() {
            clock.insert(k.clone(), Value::String(op.hlc.clone()));
        }
    } else {
        let wins = |key: &str, clock: &Map<String, Value>| match clock.get(key).and_then(Value::as_str) {
            Some(local) => op.hlc.as_str() > local,
            None => true,
        };
        let key_of = |col: &str| -> String {
            s.json_cols.iter().find(|(_, c)| *c == col).map(|(k, _)| k.to_string()).unwrap_or_else(|| col.to_string())
        };
        let mut sets: Vec<(String, Value)> = Vec::new();
        for (col, v) in &plain {
            let key = key_of(col);
            let local_hlc = clock.get(&key).and_then(Value::as_str).map(String::from);
            let is_long = s.long_text.contains(&key.as_str());
            if wins(&key, &clock) {
                if is_long {
                    if let Some(lh) = &local_hlc {
                        let cur = current_value(conn, s.table, col, &op.entity_id)?;
                        if &cur != v && local_unconfirmed(conn, lh, my_device)? {
                            record_conflict(conn, op, &key, v, &cur, my_device)?;
                        }
                    }
                }
                sets.push((col.clone(), v.clone()));
                clock.insert(key, Value::String(op.hlc.clone()));
            } else if is_long {
                let cur = current_value(conn, s.table, col, &op.entity_id)?;
                if &cur != v {
                    // Ours is newer, but theirs was written without seeing ours: keep both.
                    record_conflict(conn, op, &key, &cur, v, &op.device_id)?;
                }
            }
        }
        if !sets.is_empty() {
            let assigns: Vec<String> = sets.iter().enumerate().map(|(i, (c, _))| format!("\"{c}\" = ?{}", i + 2)).collect();
            let mut vals = vec![Sql::Text(op.entity_id.clone())];
            vals.extend(sets.iter().map(|(_, v)| to_sql(v)));
            conn.execute(&format!("UPDATE \"{}\" SET {} WHERE id = ?1", s.table, assigns.join(", ")), params_from_iter(vals))?;
        }
        for (l, v, key) in &lists {
            if wins(key, &clock) {
                write_list(conn, *l, &op.entity_id, v)?;
                clock.insert(key.clone(), Value::String(op.hlc.clone()));
            }
        }
    }
    if cols.contains("updated_at") {
        conn.execute(&format!("UPDATE \"{}\" SET updated_at = ?2 WHERE id = ?1", s.table), params![op.entity_id, now])?;
    }
    conn.execute(
        &format!("UPDATE \"{}\" SET field_clock = ?2 WHERE id = ?1", s.table),
        params![op.entity_id, Value::Object(clock).to_string()],
    )?;
    Ok(())
}

fn set_deleted(conn: &Connection, s: &Spec, op: &Op, deleted: Option<String>) -> AppResult<()> {
    let existing: Option<String> = conn
        .query_row(&format!("SELECT field_clock FROM \"{}\" WHERE id = ?1", s.table), [&op.entity_id], |r| r.get(0))
        .optional()?;
    let Some(existing) = existing else { return Ok(()) }; // never had it: nothing to delete/restore
    let mut clock: Map<String, Value> = serde_json::from_str(&existing).unwrap_or_default();
    if clock.get("deleted_at").and_then(Value::as_str).is_some_and(|l| l >= op.hlc.as_str()) {
        return Ok(()); // a newer delete/restore already happened here
    }
    clock.insert("deleted_at".into(), Value::String(op.hlc.clone()));
    conn.execute(
        &format!("UPDATE \"{}\" SET deleted_at = ?2, field_clock = ?3 WHERE id = ?1", s.table),
        params![op.entity_id, deleted, Value::Object(clock).to_string()],
    )?;
    Ok(())
}

/// Permanently removes a record — only if it's already in the bin here too.
fn purge(conn: &Connection, s: &Spec, op: &Op) -> AppResult<()> {
    let deleted: Option<Option<String>> = conn
        .query_row(&format!("SELECT deleted_at FROM \"{}\" WHERE id = ?1", s.table), [&op.entity_id], |r| r.get(0))
        .optional()?;
    if !matches!(deleted, Some(Some(_))) {
        return Ok(()); // restored or edited here since: keep it
    }
    conn.execute_batch("SAVEPOINT purge_one")?;
    let res = (|| -> AppResult<()> {
        let id = &op.entity_id;
        if s.entity == "journal_entry" {
            for sql in [
                "DELETE FROM reactions WHERE entry_id = ?1",
                "DELETE FROM comments WHERE entry_id = ?1",
                "DELETE FROM journal_fts WHERE entry_id = ?1",
                "UPDATE milestones SET linked_entry_id = NULL WHERE linked_entry_id = ?1",
                "UPDATE bucket_items SET linked_entry_id = NULL WHERE linked_entry_id = ?1",
                "UPDATE planned_dates SET linked_entry_id = NULL WHERE linked_entry_id = ?1",
                "UPDATE special_dates SET linked_entry_id = NULL WHERE linked_entry_id = ?1",
            ] {
                conn.execute(sql, [id])?;
            }
        }
        for (_, l) in s.lists {
            match l {
                List::EntryMedia => conn.execute("DELETE FROM entry_media WHERE entry_id = ?1", [id])?,
                List::Ids(t, c) => conn.execute(&format!("DELETE FROM {t} WHERE {c} = ?1"), [id])?,
            };
        }
        conn.execute(&format!("DELETE FROM \"{}\" WHERE id = ?1", s.table), [id])?;
        Ok(())
    })();
    match res {
        Ok(()) => conn.execute_batch("RELEASE purge_one")?,
        // Something still points at it: leaving it in the bin is the safe choice.
        Err(_) => conn.execute_batch("ROLLBACK TO purge_one; RELEASE purge_one")?,
    }
    Ok(())
}

// ---------- special entities ----------

fn meta_get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT value FROM local_meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}
pub fn meta_set(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO local_meta(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// LWW for small records without a field_clock column (clock kept in local_meta).
fn lww_meta(conn: &Connection, scope: &str, field: &str, hlc: &str) -> AppResult<bool> {
    let key = format!("clock:{scope}:{field}");
    if meta_get(conn, &key)?.is_some_and(|l| l.as_str() >= hlc) {
        return Ok(false);
    }
    meta_set(conn, &key, hlc)?;
    Ok(true)
}

/// Columns that hold a user id, everywhere — used to re-point the "partner" placeholder.
const USER_COLUMNS: &[&str] = &["author_id", "created_by", "user_id", "sender_id", "recipient_id"];

fn apply_user(conn: &Connection, op: &Op) -> AppResult<()> {
    let me: Option<String> = conn.query_row("SELECT id FROM users WHERE is_self = 1", [], |r| r.get(0)).optional()?;
    if me.as_deref() == Some(op.entity_id.as_str()) {
        return Ok(());
    }
    let name = op.changes.get("display_name").and_then(Value::as_str).unwrap_or("").trim().to_string();
    let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM users WHERE id = ?1)", [&op.entity_id], |r| r.get(0))?;
    let now = now_iso();
    if !exists {
        // Replace the local "partner" placeholder (typed during setup) with the real person.
        let placeholder: Option<String> = conn
            .query_row("SELECT id FROM users WHERE is_self = 0 AND id <> ?1 ORDER BY created_at LIMIT 1", [&op.entity_id], |r| r.get(0))
            .optional()?;
        conn.execute(
            "INSERT INTO users(id, display_name, is_self, created_at, updated_at) VALUES (?1, ?2, 0, ?3, ?3)",
            params![op.entity_id, if name.is_empty() { "Your partner" } else { &name }, now],
        )?;
        if let Some(ph) = placeholder {
            let tables: Vec<String> = {
                let mut s = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '%_fts%'")?;
                let r = s.query_map([], |r| r.get(0))?;
                r.collect::<Result<_, _>>()?
            };
            for t in tables {
                let cols = columns(conn, &t)?;
                for c in USER_COLUMNS.iter().filter(|c| cols.contains(**c)) {
                    conn.execute(&format!("UPDATE \"{t}\" SET \"{c}\" = ?1 WHERE \"{c}\" = ?2"), params![op.entity_id, ph])?;
                }
            }
            conn.execute("DELETE FROM users WHERE id = ?1", [ph])?;
        }
        lww_meta(conn, &format!("user:{}", op.entity_id), "display_name", &op.hlc)?;
    } else if !name.is_empty() && lww_meta(conn, &format!("user:{}", op.entity_id), "display_name", &op.hlc)? {
        conn.execute("UPDATE users SET display_name = ?2, updated_at = ?3 WHERE id = ?1", params![op.entity_id, name, now])?;
    }
    Ok(())
}

fn apply_space(conn: &Connection, op: &Op) -> AppResult<()> {
    let Some(ch) = op.changes.as_object() else { return Ok(()) };
    let local: Option<String> = conn.query_row("SELECT id FROM space LIMIT 1", [], |r| r.get(0)).optional()?;
    let now = now_iso();
    if local.is_none() {
        conn.execute(
            "INSERT INTO space(id, name, start_date, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![op.entity_id, ch.get("name").and_then(Value::as_str).unwrap_or("Our little world"), ch.get("start_date").and_then(Value::as_str), now],
        )?;
        return Ok(());
    }
    for field in ["name", "start_date"] {
        if let Some(v) = ch.get(field) {
            if lww_meta(conn, "space", field, &op.hlc)? {
                conn.execute(&format!("UPDATE space SET {field} = ?1, updated_at = ?2"), params![to_sql(v), now])?;
            }
        }
    }
    Ok(())
}

/// First pick of the day wins (lowest HLC), so both PCs settle on the same question.
fn apply_daily(conn: &Connection, op: &Op) -> AppResult<()> {
    let qid = op.changes.get("question_id").and_then(Value::as_str).unwrap_or_default();
    if qid.is_empty() {
        return Ok(());
    }
    let key = format!("clock:daily:{}", op.entity_id);
    // our own pick's time comes from the op-log if we picked it here
    let local_clock = match meta_get(conn, &key)? {
        Some(c) => Some(c),
        None => conn
            .query_row(
                "SELECT hlc FROM oplog WHERE entity = 'daily_question' AND entity_id = ?1 ORDER BY hlc LIMIT 1",
                [&op.entity_id],
                |r| r.get::<_, String>(0),
            )
            .optional()?,
    };
    let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM daily_questions WHERE id = ?1)", [&op.entity_id], |r| r.get(0))?;
    let replace = !exists || local_clock.as_deref().is_some_and(|l| op.hlc.as_str() < l);
    if replace {
        conn.execute(
            "INSERT INTO daily_questions(id, question_id, picked_by_device, created_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET question_id = excluded.question_id, picked_by_device = excluded.picked_by_device",
            params![op.entity_id, qid, op.device_id, now_iso()],
        )?;
        meta_set(conn, &key, &op.hlc)?;
    }
    Ok(())
}

/// Media details from the partner. The bytes follow separately (see engine).
/// Returns the media id that now needs downloading, if any.
fn apply_media(conn: &Connection, op: &Op) -> AppResult<Option<String>> {
    let c = &op.changes;
    let hash = c.get("hash").and_then(Value::as_str).unwrap_or_default();
    if hash.len() != 64 || !hash.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(AppError::invalid("bad media hash"));
    }
    let kind = match c.get("kind").and_then(Value::as_str) {
        Some("video") => "video",
        _ => "photo",
    };
    let mime = c.get("mime").and_then(Value::as_str).unwrap_or("application/octet-stream");
    let ext = c.get("ext").and_then(Value::as_str).filter(|e| e.len() <= 5 && e.chars().all(|x| x.is_ascii_alphanumeric())).unwrap_or("bin");
    if let Some(local_id) = conn.query_row("SELECT id FROM media WHERE hash = ?1", [hash], |r| r.get::<_, String>(0)).optional()? {
        if local_id != op.entity_id {
            conn.execute("INSERT OR REPLACE INTO media_alias(remote_id, local_id) VALUES (?1, ?2)", params![op.entity_id, local_id])?;
            conn.execute("DELETE FROM media WHERE id = ?1 AND availability = 'pending' AND hash LIKE 'pending:%'", [&op.entity_id])
                .ok();
        }
        return Ok(None);
    }
    let size = c.get("size").and_then(Value::as_i64).unwrap_or(0);
    let stub: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM media WHERE id = ?1)", [&op.entity_id], |r| r.get(0))?;
    if stub {
        conn.execute(
            "UPDATE media SET hash=?2, kind=?3, mime=?4, ext=?5, size=?6, width=?7, height=?8, duration_ms=?9,
                    taken_at=?10, original_name=?11, availability='pending' WHERE id = ?1",
            params![op.entity_id, hash, kind, mime, ext, size, c.get("width").and_then(Value::as_i64), c.get("height").and_then(Value::as_i64),
                    c.get("duration_ms").and_then(Value::as_i64), c.get("taken_at").and_then(Value::as_str), c.get("original_name").and_then(Value::as_str)],
        )?;
    } else {
        conn.execute(
            "INSERT INTO media(id, hash, kind, mime, ext, size, width, height, duration_ms, taken_at, original_name, availability, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'pending',?12)",
            params![op.entity_id, hash, kind, mime, ext, size, c.get("width").and_then(Value::as_i64), c.get("height").and_then(Value::as_i64),
                    c.get("duration_ms").and_then(Value::as_i64), c.get("taken_at").and_then(Value::as_str), c.get("original_name").and_then(Value::as_str), now_iso()],
        )?;
    }
    Ok(Some(op.entity_id.clone()))
}

/// Replace media ids the partner uses for files we already had under another id.
fn translate_aliases(v: &mut Value, aliases: &HashMap<String, String>) {
    match v {
        Value::String(s) => {
            if let Some(l) = aliases.get(s.as_str()) {
                *s = l.clone();
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| translate_aliases(x, aliases)),
        Value::Object(o) => o.values_mut().for_each(|x| translate_aliases(x, aliases)),
        _ => {}
    }
}

fn validate(op: &Op) -> AppResult<()> {
    let ok_id = !op.entity_id.is_empty() && op.entity_id.len() <= 64 && op.entity_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if !ok_id || op.op_id.is_empty() || op.op_id.len() > 64 || op.hlc.len() > 128 {
        return Err(AppError::invalid("malformed op"));
    }
    if !matches!(op.kind.as_str(), "upsert" | "delete" | "restore" | "purge") {
        return Err(AppError::invalid("unknown op kind"));
    }
    Ok(())
}

#[derive(Debug, Default)]
pub struct Outcome {
    pub applied: usize,
    pub skipped_duplicates: usize,
    pub failed: Vec<String>,
    pub media_to_fetch: Vec<String>,
    pub touched_journal: bool,
}

/// Applies a batch of remote ops (caller provides the transaction). Each op is isolated
/// in a savepoint: one bad op never blocks the others.
pub fn apply_batch(conn: &Connection, ops: &[Op], my_device: &str) -> AppResult<Outcome> {
    let mut out = Outcome::default();
    let aliases: HashMap<String, String> = {
        let mut s = conn.prepare("SELECT remote_id, local_id FROM media_alias")?;
        let r = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        r.collect::<Result<_, _>>()?
    };
    let mut sorted: Vec<&Op> = ops.iter().collect();
    // media details first, so records can reference them; then causal (HLC) order
    sorted.sort_by(|a, b| (a.entity != "media").cmp(&(b.entity != "media")).then_with(|| a.hlc.cmp(&b.hlc)));
    for op in sorted {
        let done: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sync_applied WHERE op_id = ?1)", [&op.op_id], |r| r.get(0))?;
        if done {
            out.skipped_duplicates += 1;
            continue;
        }
        let mut op = op.clone();
        if !aliases.is_empty() && op.entity != "media" {
            translate_aliases(&mut op.changes, &aliases);
        }
        conn.execute_batch("SAVEPOINT op_apply")?;
        let res = (|| -> AppResult<()> {
            validate(&op)?;
            match op.entity.as_str() {
                "user" => apply_user(conn, &op)?,
                "space" => apply_space(conn, &op)?,
                "daily_question" => apply_daily(conn, &op)?,
                "media" => {
                    if let Some(id) = apply_media(conn, &op)? {
                        out.media_to_fetch.push(id);
                    }
                }
                e => {
                    let s = spec(e).ok_or_else(|| AppError::invalid(format!("unknown record type {e}")))?;
                    match op.kind.as_str() {
                        "upsert" => upsert(conn, s, &op, my_device)?,
                        "delete" => set_deleted(conn, s, &op, Some(op.changes.get("deleted_at").and_then(Value::as_str).map(String::from).unwrap_or_else(now_iso)))?,
                        "restore" => set_deleted(conn, s, &op, None)?,
                        _ => purge(conn, s, &op)?,
                    }
                    if e == "journal_entry" {
                        after_journal(conn, &op.entity_id)?;
                        out.touched_journal = true;
                    }
                    if e == "letter" {
                        // Written before we paired, it was addressed to the sender's stand-in
                        // for us: on this PC, "the partner" is me.
                        conn.execute(
                            "UPDATE letters SET recipient_id = (SELECT id FROM users WHERE is_self = 1)
                             WHERE id = ?1 AND sender_id <> (SELECT id FROM users WHERE is_self = 1)
                               AND (recipient_id IS NULL OR recipient_id NOT IN (SELECT id FROM users))",
                            [&op.entity_id],
                        )?;
                    }
                }
            }
            conn.execute("INSERT INTO sync_applied(op_id, applied_at) VALUES (?1, ?2)", params![op.op_id, now_iso()])?;
            Ok(())
        })();
        match res {
            Ok(()) => {
                conn.execute_batch("RELEASE op_apply")?;
                out.applied += 1;
            }
            Err(e) => {
                conn.execute_batch("ROLLBACK TO op_apply; RELEASE op_apply")?;
                out.failed.push(format!("{} {}: {e}", op.entity, op.entity_id));
            }
        }
    }
    Ok(out)
}

/// Keep the other version of a conflicting text: write it as a normal local change
/// (op-logged, so the partner gets the decision too).
pub fn resolve_conflict(conn: &mut Connection, w: &crate::sync::oplog::Writer, conflict_id: &str, use_other: bool) -> AppResult<()> {
    let (entity, entity_id, field, other): (String, String, String, Option<String>) = conn
        .query_row(
            "SELECT entity, entity_id, field, other_value FROM sync_conflicts WHERE id = ?1 AND resolved_at IS NULL",
            [conflict_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::invalid("this conflict was already resolved"))?;
    let tx = conn.transaction()?;
    if use_other {
        let s = spec(&entity).ok_or_else(|| AppError::invalid("unknown record type"))?;
        if !s.long_text.contains(&field.as_str()) {
            return Err(AppError::invalid("unexpected conflict field"));
        }
        let value = other.unwrap_or_default();
        tx.execute(&format!("UPDATE \"{}\" SET \"{field}\" = ?2, updated_at = ?3 WHERE id = ?1", s.table), params![entity_id, value, now_iso()])?;
        let mut ch = Map::new();
        ch.insert(field.clone(), Value::String(value));
        let hlc = w.record(&tx, &entity, &entity_id, "upsert", Value::Object(ch))?;
        let fc: String = tx.query_row(&format!("SELECT field_clock FROM \"{}\" WHERE id = ?1", s.table), [&entity_id], |r| r.get(0))?;
        let fc = crate::sync::oplog::stamp_fields(&fc, [field.clone()], &hlc);
        tx.execute(&format!("UPDATE \"{}\" SET field_clock = ?2 WHERE id = ?1", s.table), params![entity_id, fc])?;
        if entity == "journal_entry" {
            after_journal(&tx, &entity_id)?;
        }
    }
    tx.execute("UPDATE sync_conflicts SET resolved_at = ?2 WHERE id = ?1", params![conflict_id, now_iso()])?;
    tx.commit()?;
    Ok(())
}
