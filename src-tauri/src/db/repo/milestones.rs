//! Manual milestones and the combined timeline (milestones + journal entries, no duplicates).

use super::journal::{validate_date, EntryQuery};
use super::media::{self as media_repo, MediaRef, COLS as MEDIA_COLS};
use crate::db::{new_id, now_iso};
use crate::error::{AppError, AppResult};
use crate::sync::oplog::{stamp_fields, Writer};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const ENTITY: &str = "milestone";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MilestoneInput {
    pub title: String,
    pub date: String,
    #[serde(default)]
    pub description: String,
    pub cover_media_id: Option<String>,
    pub linked_entry_id: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TimelineItem {
    /// "milestone" or "entry"
    pub kind: String,
    pub id: String,
    pub title: String,
    pub date: String,
    pub time: Option<String>,
    pub text: String,
    pub major: bool,
    pub linked_entry_id: Option<String>,
    pub cover: Option<MediaRef>,
}

struct Clean {
    title: String,
    date: String,
    description: String,
    cover: Option<String>,
    linked: Option<String>,
}

fn clean(conn: &Connection, i: &MilestoneInput) -> AppResult<Clean> {
    let title = i.title.trim().to_string();
    if title.is_empty() || title.chars().count() > 200 {
        return Err(AppError::invalid("A milestone needs a title (200 characters max)"));
    }
    let description = i.description.trim().to_string();
    if description.chars().count() > 2000 {
        return Err(AppError::invalid("description is too long"));
    }
    let cover = i.cover_media_id.clone().filter(|s| !s.is_empty());
    if let Some(c) = &cover {
        if media_repo::get(conn, c)?.is_none() {
            return Err(AppError::invalid("that photo isn't in the archive"));
        }
    }
    let linked = i.linked_entry_id.clone().filter(|s| !s.is_empty());
    if let Some(l) = &linked {
        let ok: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM journal_entries WHERE id = ?1 AND deleted_at IS NULL)",
            [l],
            |r| r.get(0),
        )?;
        if !ok {
            return Err(AppError::invalid("the linked memory doesn't exist"));
        }
    }
    Ok(Clean { title, date: validate_date(&i.date)?, description, cover, linked })
}

fn fields(c: &Clean) -> Vec<(&'static str, Value)> {
    vec![
        ("title", json!(c.title)),
        ("date", json!(c.date)),
        ("description", json!(c.description)),
        ("cover_media_id", json!(c.cover)),
        ("linked_entry_id", json!(c.linked)),
    ]
}

pub fn create(conn: &mut Connection, w: &Writer, author: &str, i: &MilestoneInput) -> AppResult<String> {
    let c = clean(conn, i)?;
    let id = new_id();
    let now = now_iso();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO milestones(id, title, date, description, cover_media_id, linked_entry_id, created_by, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?8)",
        params![id, c.title, c.date, c.description, c.cover, c.linked, author, now],
    )?;
    let f = fields(&c);
    let mut ch: serde_json::Map<String, Value> = f.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    ch.insert("created_by".into(), json!(author));
    let hlc = w.record(&tx, ENTITY, &id, "upsert", Value::Object(ch))?;
    let clock = stamp_fields("{}", f.iter().map(|(k, _)| k.to_string()), &hlc);
    tx.execute("UPDATE milestones SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
    tx.commit()?;
    Ok(id)
}

pub fn update(conn: &mut Connection, w: &Writer, id: &str, i: &MilestoneInput) -> AppResult<()> {
    let next = clean(conn, i)?;
    let cur = conn
        .query_row(
            "SELECT title, date, description, cover_media_id, linked_entry_id, field_clock FROM milestones
             WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |r| {
                Ok((
                    Clean { title: r.get(0)?, date: r.get(1)?, description: r.get(2)?, cover: r.get(3)?, linked: r.get(4)? },
                    r.get::<_, String>(5)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::invalid("this milestone doesn't exist anymore"))?;
    let changed: Vec<(&str, Value)> = fields(&next)
        .into_iter()
        .zip(fields(&cur.0))
        .filter(|((_, a), (_, b))| a != b)
        .map(|((k, a), _)| (k, a))
        .collect();
    if changed.is_empty() {
        return Ok(());
    }
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE milestones SET title=?2, date=?3, description=?4, cover_media_id=?5, linked_entry_id=?6, updated_at=?7 WHERE id=?1",
        params![id, next.title, next.date, next.description, next.cover, next.linked, now_iso()],
    )?;
    let ch: serde_json::Map<String, Value> = changed.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    let hlc = w.record(&tx, ENTITY, id, "upsert", Value::Object(ch))?;
    let clock = stamp_fields(&cur.1, changed.iter().map(|(k, _)| k.to_string()), &hlc);
    tx.execute("UPDATE milestones SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
    tx.commit()?;
    Ok(())
}

/// Soft delete; purged with the journal's retention period.
pub fn delete(conn: &mut Connection, w: &Writer, id: &str) -> AppResult<()> {
    let now = now_iso();
    let tx = conn.transaction()?;
    let n = tx.execute(
        "UPDATE milestones SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, now],
    )?;
    if n == 0 {
        return Err(AppError::invalid("this milestone doesn't exist anymore"));
    }
    w.record(&tx, ENTITY, id, "delete", json!({ "deleted_at": now }))?;
    tx.commit()?;
    Ok(())
}

pub fn purge_expired(conn: &mut Connection, w: &Writer, retention_days: i64) -> AppResult<usize> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(retention_days))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let ids: Vec<String> = {
        let mut s = conn.prepare("SELECT id FROM milestones WHERE deleted_at IS NOT NULL AND deleted_at < ?1")?;
        let r = s.query_map([cutoff], |r| r.get(0))?;
        r.collect::<Result<_, _>>()?
    };
    for id in &ids {
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM milestones WHERE id = ?1", [id])?;
        w.record(&tx, ENTITY, id, "purge", json!({}))?;
        tx.commit()?;
    }
    Ok(ids.len())
}

/// Milestones + journal entries in date order. `major_only` = milestones + highlighted entries.
pub fn timeline(conn: &Connection, major_only: bool) -> AppResult<Vec<TimelineItem>> {
    let mcols = MEDIA_COLS.split(", ").map(|c| format!("c.{c}")).collect::<Vec<_>>().join(", ");
    let mut stmt = conn.prepare(&format!(
        "SELECT s.id, s.title, s.date, s.description, s.linked_entry_id, {mcols}
         FROM milestones s
         LEFT JOIN media c ON c.id = COALESCE(s.cover_media_id,
             (SELECT j.cover_media_id FROM journal_entries j WHERE j.id = s.linked_entry_id))
         WHERE s.deleted_at IS NULL"
    ))?;
    let mut items: Vec<TimelineItem> = stmt
        .query_map([], |r| {
            let cover_id: Option<String> = r.get(5)?;
            Ok(TimelineItem {
                kind: "milestone".into(),
                id: r.get(0)?,
                title: r.get(1)?,
                date: r.get(2)?,
                time: None,
                text: r.get(3)?,
                major: true,
                linked_entry_id: r.get(4)?,
                cover: if cover_id.is_some() { Some(media_repo::from_row(r, 5)?.r) } else { None },
            })
        })?
        .collect::<Result<_, _>>()?;

    let entries = super::journal::list(
        conn,
        &EntryQuery { highlights_only: major_only, limit: Some(5000), ..Default::default() },
    )?;
    items.extend(entries.into_iter().map(|e| TimelineItem {
        kind: "entry".into(),
        id: e.id.clone(),
        title: e.title,
        date: e.entry_date,
        time: e.entry_time,
        text: e.excerpt,
        major: e.highlight,
        linked_entry_id: Some(e.id),
        cover: e.cover,
    }));
    items.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.time.cmp(&a.time)).then_with(|| a.kind.cmp(&b.kind)));
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::journal::{self, tests as jt};

    fn input(title: &str, date: &str, linked: Option<String>) -> MilestoneInput {
        MilestoneInput { title: title.into(), date: date.into(), description: "".into(), cover_media_id: None, linked_entry_id: linked }
    }

    #[test]
    fn timeline_combines_without_duplicates_and_filters_major() {
        let (mut conn, w, me) = jt::setup();
        let e = journal::create(&mut conn, &w, &me, &jt::input("Beach", "2025-07-12")).unwrap();
        let mut hi = jt::input("First trip", "2024-03-01");
        hi.highlight = true;
        journal::create(&mut conn, &w, &me, &hi).unwrap();
        create(&mut conn, &w, &me, &input("We met", "2023-05-14", None)).unwrap();
        create(&mut conn, &w, &me, &input("Beach milestone", "2025-07-12", Some(e.summary.id.clone()))).unwrap();

        let all = timeline(&conn, false).unwrap();
        assert_eq!(all.len(), 4);
        assert_eq!(all.last().unwrap().title, "We met");
        let major = timeline(&conn, true).unwrap();
        assert_eq!(major.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), ["Beach milestone", "First trip", "We met"]);
    }

    #[test]
    fn validates_and_updates() {
        let (mut conn, w, me) = jt::setup();
        assert!(create(&mut conn, &w, &me, &input("", "2024-01-01", None)).is_err());
        assert!(create(&mut conn, &w, &me, &input("x", "2024-01-01", Some("ghost".into()))).is_err());
        let id = create(&mut conn, &w, &me, &input("We met", "2023-05-14", None)).unwrap();
        update(&mut conn, &w, &id, &input("We met ♡", "2023-05-14", None)).unwrap();
        assert_eq!(timeline(&conn, false).unwrap()[0].title, "We met ♡");
        delete(&mut conn, &w, &id).unwrap();
        assert!(timeline(&conn, false).unwrap().is_empty());
    }
}
