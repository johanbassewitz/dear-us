//! One recovery bin for everything that can be deleted. Restoring and deleting forever are
//! op-logged like any other change, so the partner's PC follows.

use super::journal;
use crate::db::now_iso;
use crate::error::{AppError, AppResult};
use crate::sync::oplog::{stamp_fields, Writer};
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::json;

struct Kind {
    entity: &'static str,
    table: &'static str,
    title_sql: &'static str,
    label: &'static str,
    /// child rows removed on "delete forever"
    children: &'static [(&'static str, &'static str)],
    /// extra condition (e.g. only my own comments/letters)
    only_mine: Option<&'static str>,
}

const KINDS: &[Kind] = &[
    Kind { entity: "journal_entry", table: "journal_entries", title_sql: "title", label: "Memory", children: &[], only_mine: None },
    Kind { entity: "milestone", table: "milestones", title_sql: "title", label: "Milestone", children: &[], only_mine: None },
    Kind { entity: "bucket_item", table: "bucket_items", title_sql: "title", label: "Bucket list", children: &[("bucket_media", "item_id")], only_mine: None },
    Kind { entity: "special_date", table: "special_dates", title_sql: "title", label: "Special date", children: &[], only_mine: None },
    Kind { entity: "planned_date", table: "planned_dates", title_sql: "title", label: "Date plan", children: &[], only_mine: None },
    Kind { entity: "date_idea", table: "date_ideas", title_sql: "title", label: "Date idea", children: &[], only_mine: None },
    Kind { entity: "custom_question", table: "custom_questions", title_sql: "text", label: "Question", children: &[], only_mine: None },
    Kind { entity: "answer", table: "answers", title_sql: "substr(body, 1, 80)", label: "Answer", children: &[], only_mine: Some("user_id") },
    Kind { entity: "comment", table: "comments", title_sql: "substr(body, 1, 80)", label: "Comment", children: &[], only_mine: Some("author_id") },
    Kind { entity: "letter", table: "letters", title_sql: "CASE WHEN title = '' THEN 'Untitled letter' ELSE title END", label: "Letter", children: &[("letter_media", "letter_id")], only_mine: Some("sender_id") },
];

fn kind(entity: &str) -> AppResult<&'static Kind> {
    KINDS.iter().find(|k| k.entity == entity).ok_or_else(|| AppError::invalid("unknown kind of item"))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BinItem {
    pub entity: String,
    pub id: String,
    pub label: String,
    pub title: String,
    pub deleted_at: String,
}

pub fn list(conn: &Connection, me: &str) -> AppResult<Vec<BinItem>> {
    let mut out = Vec::new();
    for k in KINDS {
        let mine = k.only_mine.map(|c| format!(" AND {c} = ?1")).unwrap_or_default();
        let sql = format!("SELECT id, {}, deleted_at FROM {} WHERE deleted_at IS NOT NULL{mine}", k.title_sql, k.table);
        let mut s = conn.prepare(&sql)?;
        let map = |r: &rusqlite::Row| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?));
        let rows: Vec<(String, Option<String>, String)> =
            if k.only_mine.is_some() { s.query_map([me], map)?.collect::<Result<_, _>>()? } else { s.query_map([], map)?.collect::<Result<_, _>>()? };
        out.extend(rows.into_iter().map(|(id, title, deleted_at)| BinItem {
            entity: k.entity.into(),
            id,
            label: k.label.into(),
            title: title.filter(|t| !t.trim().is_empty()).unwrap_or_else(|| "(untitled)".into()),
            deleted_at,
        }));
    }
    out.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at));
    Ok(out)
}

pub fn restore(conn: &mut Connection, w: &Writer, entity: &str, id: &str) -> AppResult<()> {
    if entity == "journal_entry" {
        journal::restore(conn, w, id)?;
        return Ok(());
    }
    let k = kind(entity)?;
    let tx = conn.transaction()?;
    let clock: Option<String> = tx
        .query_row(&format!("SELECT field_clock FROM {} WHERE id = ?1 AND deleted_at IS NOT NULL", k.table), [id], |r| r.get(0))
        .ok();
    let clock = clock.ok_or_else(|| AppError::invalid("nothing to restore"))?;
    tx.execute(&format!("UPDATE {} SET deleted_at = NULL, updated_at = ?2 WHERE id = ?1", k.table), params![id, now_iso()])?;
    let hlc = w.record(&tx, entity, id, "restore", json!({}))?;
    tx.execute(&format!("UPDATE {} SET field_clock = ?2 WHERE id = ?1", k.table), params![id, stamp_fields(&clock, ["deleted_at".to_string()], &hlc)])?;
    tx.commit()?;
    Ok(())
}

/// Permanently removes an item that's already in the bin.
pub fn purge(conn: &mut Connection, w: &Writer, entity: &str, id: &str) -> AppResult<()> {
    if entity == "journal_entry" {
        return journal::purge(conn, w, id);
    }
    let k = kind(entity)?;
    let tx = conn.transaction()?;
    let in_bin: bool = tx.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {} WHERE id = ?1 AND deleted_at IS NOT NULL)", k.table), [id], |r| r.get(0))?;
    if !in_bin {
        return Err(AppError::invalid("only items in the recovery bin can be removed forever"));
    }
    for (t, c) in k.children {
        tx.execute(&format!("DELETE FROM {t} WHERE {c} = ?1"), [id])?;
    }
    tx.execute(&format!("DELETE FROM {} WHERE id = ?1", k.table), [id])?;
    w.record(&tx, entity, id, "purge", json!({}))?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::{bucket, journal::tests as jt};

    #[test]
    fn one_bin_for_everything() {
        let (mut conn, w, me) = jt::setup();
        let e = journal::create(&mut conn, &w, &me, &jt::input("A memory", "2025-01-01")).unwrap();
        let b: bucket::BucketInput = serde_json::from_value(json!({"title":"Paris","priority":"low","status":"want","targetDate":null,"coverMediaId":null})).unwrap();
        let bid = bucket::create(&mut conn, &w, &me, &b).unwrap();
        journal::delete(&mut conn, &w, &e.summary.id).unwrap();
        bucket::delete(&mut conn, &w, &bid).unwrap();
        let items = list(&conn, &me).unwrap();
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|i| i.label == "Bucket list" && i.title == "Paris"));
        restore(&mut conn, &w, "bucket_item", &bid).unwrap();
        purge(&mut conn, &w, "journal_entry", &e.summary.id).unwrap();
        assert!(list(&conn, &me).unwrap().is_empty());
        assert!(purge(&mut conn, &w, "bucket_item", &bid).is_err(), "live items can't be purged");
        let kinds: Vec<String> = conn.prepare("SELECT kind FROM oplog WHERE entity='bucket_item' ORDER BY hlc").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
        assert_eq!(kinds, ["upsert", "delete", "restore"]);
    }
}
