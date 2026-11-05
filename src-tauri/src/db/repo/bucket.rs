//! Shared bucket list: things to do together. Moves want -> planned -> done; finishing can link a memory.

use super::journal::validate_date;
use super::media::{self as media_repo, MediaRef, COLS as MEDIA_COLS};
use crate::db::{new_id, now_iso};
use crate::error::{AppError, AppResult};
use crate::sync::oplog::{stamp_fields, Writer};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const ENTITY: &str = "bucket_item";
const MAX_PHOTOS: usize = 12;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BucketInput {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: String,
    pub priority: String,
    pub status: String,
    pub target_date: Option<String>,
    pub cover_media_id: Option<String>,
    #[serde(default)]
    pub media_ids: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BucketItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub priority: String,
    pub status: String,
    pub target_date: Option<String>,
    pub completed_at: Option<String>,
    pub linked_entry_id: Option<String>,
    pub linked_entry_title: Option<String>,
    pub cover_media_id: Option<String>,
    pub cover: Option<MediaRef>,
    pub media: Vec<MediaRef>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BucketTrashItem {
    pub id: String,
    pub title: String,
    pub deleted_at: String,
}

/// The editable state of an item (what is op-logged field by field).
#[derive(Debug, Clone, PartialEq)]
struct Fields {
    title: String,
    description: String,
    category: String,
    priority: String,
    status: String,
    target_date: Option<String>,
    completed_at: Option<String>,
    linked: Option<String>,
    cover: Option<String>,
    media: Vec<String>,
}

fn to_json(f: &Fields) -> Vec<(&'static str, Value)> {
    vec![
        ("title", json!(f.title)),
        ("description", json!(f.description)),
        ("category", json!(f.category)),
        ("priority", json!(f.priority)),
        ("status", json!(f.status)),
        ("target_date", json!(f.target_date)),
        ("completed_at", json!(f.completed_at)),
        ("linked_entry_id", json!(f.linked)),
        ("cover_media_id", json!(f.cover)),
        ("media", json!(f.media)),
    ]
}

fn check_status(s: &str) -> AppResult<()> {
    match s {
        "want" | "planned" | "done" => Ok(()),
        _ => Err(AppError::invalid("pick Dreaming, Planned or Done")),
    }
}

fn media_exists(conn: &Connection, id: &str) -> AppResult<()> {
    if media_repo::get(conn, id)?.is_none() {
        return Err(AppError::invalid("that photo isn't in the archive"));
    }
    Ok(())
}

fn entry_exists(conn: &Connection, id: &str) -> AppResult<()> {
    let ok: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM journal_entries WHERE id = ?1 AND deleted_at IS NULL)",
        [id],
        |r| r.get(0),
    )?;
    if !ok {
        return Err(AppError::invalid("that memory doesn't exist"));
    }
    Ok(())
}

/// Completion timestamp follows the status: set when it becomes done, cleared when reopened.
fn completed_for(prev_status: &str, prev: &Option<String>, next_status: &str) -> Option<String> {
    if next_status != "done" {
        None
    } else if prev_status == "done" && prev.is_some() {
        prev.clone()
    } else {
        Some(now_iso())
    }
}

fn clean(conn: &Connection, i: &BucketInput, prev: Option<&Fields>) -> AppResult<Fields> {
    let title = i.title.trim().to_string();
    if title.is_empty() || title.chars().count() > 200 {
        return Err(AppError::invalid("Give this dream a title (200 characters max)"));
    }
    let description = i.description.trim().to_string();
    if description.chars().count() > 2000 {
        return Err(AppError::invalid("the description is too long"));
    }
    let category = i.category.trim().to_string();
    if category.chars().count() > 40 {
        return Err(AppError::invalid("the category is too long (40 characters max)"));
    }
    if !matches!(i.priority.as_str(), "low" | "medium" | "high") {
        return Err(AppError::invalid("pick a priority: low, medium or high"));
    }
    check_status(&i.status)?;
    let target_date = match i.target_date.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(d) => Some(validate_date(d)?),
    };
    let mut media: Vec<String> = vec![];
    for m in &i.media_ids {
        if !media.contains(m) {
            media_exists(conn, m)?;
            media.push(m.clone());
        }
    }
    if media.len() > MAX_PHOTOS {
        return Err(AppError::invalid(format!("up to {MAX_PHOTOS} photos per dream")));
    }
    let cover = i.cover_media_id.clone().filter(|s| !s.is_empty());
    if let Some(c) = &cover {
        if !media.contains(c) {
            return Err(AppError::invalid("the cover must be one of the photos"));
        }
    }
    let (ps, pc, pl) = match prev {
        Some(p) => (p.status.as_str(), p.completed_at.clone(), p.linked.clone()),
        None => ("want", None, None),
    };
    Ok(Fields {
        title,
        description,
        category,
        priority: i.priority.clone(),
        completed_at: completed_for(ps, &pc, &i.status),
        status: i.status.clone(),
        target_date,
        linked: pl,
        cover,
        media,
    })
}

fn write_media(tx: &rusqlite::Transaction, id: &str, media: &[String]) -> AppResult<()> {
    tx.execute("DELETE FROM bucket_media WHERE item_id = ?1", [id])?;
    for (pos, m) in media.iter().enumerate() {
        tx.execute(
            "INSERT INTO bucket_media(item_id, media_id, position) VALUES (?1,?2,?3)",
            params![id, m, pos as i64],
        )?;
    }
    Ok(())
}

fn load(conn: &Connection, id: &str) -> AppResult<(Fields, String)> {
    let row = conn
        .query_row(
            "SELECT title, description, category, priority, status, target_date, completed_at,
                    linked_entry_id, cover_media_id, field_clock
             FROM bucket_items WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |r| {
                Ok((
                    Fields {
                        title: r.get(0)?,
                        description: r.get(1)?,
                        category: r.get(2)?,
                        priority: r.get(3)?,
                        status: r.get(4)?,
                        target_date: r.get(5)?,
                        completed_at: r.get(6)?,
                        linked: r.get(7)?,
                        cover: r.get(8)?,
                        media: vec![],
                    },
                    r.get::<_, String>(9)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::invalid("this dream doesn't exist anymore"))?;
    let mut f = row.0;
    let mut s = conn.prepare("SELECT media_id FROM bucket_media WHERE item_id = ?1 ORDER BY position")?;
    f.media = s.query_map([id], |r| r.get(0))?.collect::<Result<_, _>>()?;
    Ok((f, row.1))
}

/// Writes only what changed, plus op-log, in one transaction.
fn save(conn: &mut Connection, w: &Writer, id: &str, cur: &Fields, clock: &str, next: &Fields) -> AppResult<()> {
    let changed: Vec<(&str, Value)> = to_json(next)
        .into_iter()
        .zip(to_json(cur))
        .filter(|((_, a), (_, b))| a != b)
        .map(|((k, a), _)| (k, a))
        .collect();
    if changed.is_empty() {
        return Ok(());
    }
    let now = now_iso();
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE bucket_items SET title=?2, description=?3, category=?4, priority=?5, status=?6, target_date=?7,
             completed_at=?8, linked_entry_id=?9, cover_media_id=?10, updated_at=?11 WHERE id=?1",
        params![
            id, next.title, next.description, next.category, next.priority, next.status, next.target_date,
            next.completed_at, next.linked, next.cover, now
        ],
    )?;
    if cur.media != next.media {
        write_media(&tx, id, &next.media)?;
    }
    let ch: serde_json::Map<String, Value> = changed.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    let hlc = w.record(&tx, ENTITY, id, "upsert", Value::Object(ch))?;
    let clock = stamp_fields(clock, changed.iter().map(|(k, _)| k.to_string()), &hlc);
    tx.execute("UPDATE bucket_items SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
    tx.commit()?;
    Ok(())
}

pub fn create(conn: &mut Connection, w: &Writer, author: &str, i: &BucketInput) -> AppResult<String> {
    let c = clean(conn, i, None)?;
    let id = new_id();
    let now = now_iso();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO bucket_items(id, title, description, category, priority, status, target_date, completed_at,
                                  linked_entry_id, cover_media_id, created_by, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,NULL,?9,?10,?11,?11)",
        params![id, c.title, c.description, c.category, c.priority, c.status, c.target_date, c.completed_at, c.cover, author, now],
    )?;
    write_media(&tx, &id, &c.media)?;
    let f = to_json(&c);
    let mut ch: serde_json::Map<String, Value> = f.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    ch.insert("created_by".into(), json!(author));
    ch.insert("created_at".into(), json!(now));
    let hlc = w.record(&tx, ENTITY, &id, "upsert", Value::Object(ch))?;
    let clock = stamp_fields("{}", f.iter().map(|(k, _)| k.to_string()), &hlc);
    tx.execute("UPDATE bucket_items SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
    tx.commit()?;
    Ok(id)
}

pub fn update(conn: &mut Connection, w: &Writer, id: &str, i: &BucketInput) -> AppResult<()> {
    let (cur, clock) = load(conn, id)?;
    let next = clean(conn, i, Some(&cur))?;
    save(conn, w, id, &cur, &clock, &next)
}

/// Moves between columns. Done stamps `completed_at`; moving back clears it.
pub fn set_status(conn: &mut Connection, w: &Writer, id: &str, status: &str) -> AppResult<()> {
    check_status(status)?;
    let (cur, clock) = load(conn, id)?;
    let next = Fields {
        status: status.into(),
        completed_at: completed_for(&cur.status, &cur.completed_at, status),
        ..cur.clone()
    };
    save(conn, w, id, &cur, &clock, &next)
}

/// Links (or, with None, unlinks) the journal memory that tells the story of this item.
pub fn link_entry(conn: &mut Connection, w: &Writer, id: &str, entry_id: Option<&str>) -> AppResult<()> {
    let (cur, clock) = load(conn, id)?;
    let entry = entry_id.filter(|s| !s.is_empty());
    if let Some(e) = entry {
        entry_exists(conn, e)?;
    }
    let next = Fields { linked: entry.map(String::from), ..cur.clone() };
    save(conn, w, id, &cur, &clock, &next)
}

pub fn delete(conn: &mut Connection, w: &Writer, id: &str) -> AppResult<()> {
    let now = now_iso();
    let tx = conn.transaction()?;
    let n = tx.execute(
        "UPDATE bucket_items SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, now],
    )?;
    if n == 0 {
        return Err(AppError::invalid("this dream doesn't exist anymore"));
    }
    w.record(&tx, ENTITY, id, "delete", json!({ "deleted_at": now }))?;
    tx.commit()?;
    Ok(())
}

pub fn restore(conn: &mut Connection, w: &Writer, id: &str) -> AppResult<()> {
    let now = now_iso();
    let tx = conn.transaction()?;
    let n = tx.execute(
        "UPDATE bucket_items SET deleted_at = NULL, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NOT NULL",
        params![id, now],
    )?;
    if n == 0 {
        return Err(AppError::invalid("nothing to restore"));
    }
    w.record(&tx, ENTITY, id, "restore", json!({}))?;
    tx.commit()?;
    Ok(())
}

pub fn purge_expired(conn: &mut Connection, w: &Writer, retention_days: i64) -> AppResult<usize> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(retention_days))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let ids: Vec<String> = {
        let mut s = conn.prepare("SELECT id FROM bucket_items WHERE deleted_at IS NOT NULL AND deleted_at < ?1")?;
        let r = s.query_map([cutoff], |r| r.get(0))?;
        r.collect::<Result<_, _>>()?
    };
    for id in &ids {
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM bucket_media WHERE item_id = ?1", [id])?;
        tx.execute("DELETE FROM bucket_items WHERE id = ?1", [id])?;
        w.record(&tx, ENTITY, id, "purge", json!({}))?;
        tx.commit()?;
    }
    Ok(ids.len())
}

pub fn trash(conn: &Connection) -> AppResult<Vec<BucketTrashItem>> {
    let mut s = conn.prepare("SELECT id, title, deleted_at FROM bucket_items WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC")?;
    let rows = s.query_map([], |r| Ok(BucketTrashItem { id: r.get(0)?, title: r.get(1)?, deleted_at: r.get(2)? }))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn media_for(conn: &Connection, id: &str) -> AppResult<Vec<MediaRef>> {
    let cols = MEDIA_COLS.split(", ").map(|c| format!("m.{c}")).collect::<Vec<_>>().join(", ");
    let mut s = conn.prepare(&format!(
        "SELECT {cols} FROM bucket_media b JOIN media m ON m.id = b.media_id WHERE b.item_id = ?1 ORDER BY b.position"
    ))?;
    let rows = s.query_map([id], |r| Ok(media_repo::from_row(r, 0)?.r))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// All live items, newest first. The page filters and groups them.
pub fn list(conn: &Connection) -> AppResult<Vec<BucketItem>> {
    let mut s = conn.prepare(
        "SELECT b.id, b.title, b.description, b.category, b.priority, b.status, b.target_date, b.completed_at,
                b.linked_entry_id, j.title, b.cover_media_id, b.created_at, b.updated_at
         FROM bucket_items b
         LEFT JOIN journal_entries j ON j.id = b.linked_entry_id AND j.deleted_at IS NULL
         WHERE b.deleted_at IS NULL ORDER BY b.created_at DESC, b.id DESC",
    )?;
    let mut items: Vec<BucketItem> = s
        .query_map([], |r| {
            let linked_id: Option<String> = r.get(8)?;
            let linked_title: Option<String> = r.get(9)?;
            Ok(BucketItem {
                id: r.get(0)?,
                title: r.get(1)?,
                description: r.get(2)?,
                category: r.get(3)?,
                priority: r.get(4)?,
                status: r.get(5)?,
                target_date: r.get(6)?,
                completed_at: r.get(7)?,
                // A link to a memory that is in the recovery bin is hidden until it returns.
                linked_entry_id: if linked_title.is_some() { linked_id } else { None },
                linked_entry_title: linked_title,
                cover_media_id: r.get(10)?,
                cover: None,
                media: vec![],
                created_at: r.get(11)?,
                updated_at: r.get(12)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    for it in &mut items {
        it.media = media_for(conn, &it.id)?;
        it.cover = match &it.cover_media_id {
            Some(c) => it.media.iter().find(|m| &m.id == c).cloned(),
            None => None,
        }
        .or_else(|| it.media.first().cloned());
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::dashboard;
    use crate::db::repo::journal::{self, tests as jt};

    fn input(title: &str) -> BucketInput {
        BucketInput {
            title: title.into(),
            description: "".into(),
            category: "Travel".into(),
            priority: "medium".into(),
            status: "want".into(),
            target_date: None,
            cover_media_id: None,
            media_ids: vec![],
        }
    }

    fn op_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM oplog WHERE entity = 'bucket_item'", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn validates_input() {
        let (mut conn, w, me) = jt::setup();
        assert!(create(&mut conn, &w, &me, &input("  ")).is_err());
        let mut i = input("x");
        i.priority = "urgent".into();
        assert!(create(&mut conn, &w, &me, &i).is_err());
        i = input("x");
        i.status = "later".into();
        assert!(create(&mut conn, &w, &me, &i).is_err());
        i = input("x");
        i.target_date = Some("2025-13-40".into());
        assert!(create(&mut conn, &w, &me, &i).is_err());
        i = input("x");
        i.media_ids = vec!["ghost".into()];
        assert!(create(&mut conn, &w, &me, &i).is_err());
        assert_eq!(op_count(&conn), 0);
    }

    #[test]
    fn create_update_writes_ops_and_photos() {
        let (mut conn, w, me) = jt::setup();
        let p1 = jt::photo(&conn, "h1");
        let p2 = jt::photo(&conn, "h2");
        let mut i = input("See the northern lights");
        i.media_ids = vec![p1.clone(), p2.clone()];
        i.cover_media_id = Some(p2.clone());
        i.target_date = Some("2027-01-10".into());
        let id = create(&mut conn, &w, &me, &i).unwrap();
        assert_eq!(op_count(&conn), 1);
        let it = &list(&conn).unwrap()[0];
        assert_eq!(it.media.len(), 2);
        assert_eq!(it.cover.as_ref().unwrap().id, p2);
        assert_eq!(it.status, "want");

        i.priority = "high".into();
        i.media_ids = vec![p1.clone()];
        i.cover_media_id = None;
        update(&mut conn, &w, &id, &i).unwrap();
        assert_eq!(op_count(&conn), 2);
        // no-op update writes nothing
        update(&mut conn, &w, &id, &i).unwrap();
        assert_eq!(op_count(&conn), 2);
        let it = &list(&conn).unwrap()[0];
        assert_eq!(it.priority, "high");
        assert_eq!(it.cover.as_ref().unwrap().id, p1);
        let clock: String = conn.query_row("SELECT field_clock FROM bucket_items WHERE id=?1", [&id], |r| r.get(0)).unwrap();
        assert!(clock.contains("priority"));
    }

    #[test]
    fn done_sets_completed_and_reopen_clears() {
        let (mut conn, w, me) = jt::setup();
        let id = create(&mut conn, &w, &me, &input("Learn to dance")).unwrap();
        set_status(&mut conn, &w, &id, "planned").unwrap();
        assert!(list(&conn).unwrap()[0].completed_at.is_none());
        set_status(&mut conn, &w, &id, "done").unwrap();
        let it = &list(&conn).unwrap()[0];
        assert_eq!(it.status, "done");
        let stamp = it.completed_at.clone().unwrap();
        // editing a finished item keeps the original completion time
        let mut i = input("Learn to dance");
        i.status = "done".into();
        i.description = "salsa!".into();
        update(&mut conn, &w, &id, &i).unwrap();
        assert_eq!(list(&conn).unwrap()[0].completed_at.clone().unwrap(), stamp);
        set_status(&mut conn, &w, &id, "want").unwrap();
        assert!(list(&conn).unwrap()[0].completed_at.is_none());
        assert!(set_status(&mut conn, &w, &id, "nope").is_err());
        assert!(set_status(&mut conn, &w, "ghost", "done").is_err());
    }

    #[test]
    fn links_memory_and_survives_its_purge() {
        let (mut conn, w, me) = jt::setup();
        let id = create(&mut conn, &w, &me, &input("Sunset hike")).unwrap();
        assert!(link_entry(&mut conn, &w, &id, Some("ghost")).is_err());
        let e = journal::create(&mut conn, &w, &me, &jt::input("The hike", "2025-07-12")).unwrap();
        let before = op_count(&conn);
        link_entry(&mut conn, &w, &id, Some(&e.summary.id)).unwrap();
        assert_eq!(op_count(&conn), before + 1);
        let it = &list(&conn).unwrap()[0];
        assert_eq!(it.linked_entry_id.as_deref(), Some(e.summary.id.as_str()));
        assert_eq!(it.linked_entry_title.as_deref(), Some("The hike"));
        // memory in the bin: link hidden; permanently removed: link cleared
        journal::delete(&mut conn, &w, &e.summary.id).unwrap();
        assert!(list(&conn).unwrap()[0].linked_entry_id.is_none());
        journal::purge(&mut conn, &w, &e.summary.id).unwrap();
        link_entry(&mut conn, &w, &id, None).unwrap();
        assert!(list(&conn).unwrap()[0].linked_entry_id.is_none());
    }

    #[test]
    fn soft_delete_restore_purge_and_dashboard() {
        let (mut conn, w, me) = jt::setup();
        let a = create(&mut conn, &w, &me, &input("A")).unwrap();
        let b = create(&mut conn, &w, &me, &input("B")).unwrap();
        set_status(&mut conn, &w, &a, "done").unwrap();
        let s = dashboard::summary(&conn).unwrap();
        assert_eq!((s.bucket_done, s.bucket_total), (1, 2));
        delete(&mut conn, &w, &a).unwrap();
        assert!(delete(&mut conn, &w, &a).is_err());
        let s = dashboard::summary(&conn).unwrap();
        assert_eq!((s.bucket_done, s.bucket_total), (0, 1));
        assert_eq!(trash(&conn).unwrap().len(), 1);
        restore(&mut conn, &w, &a).unwrap();
        assert_eq!(list(&conn).unwrap().len(), 2);
        assert!(restore(&mut conn, &w, &a).is_err());
        let kinds: Vec<String> = {
            let mut st = conn.prepare("SELECT kind FROM oplog WHERE entity='bucket_item' AND entity_id=?1 ORDER BY hlc").unwrap();
            st.query_map([&a], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
        };
        assert_eq!(kinds, ["upsert", "upsert", "delete", "restore"]);

        delete(&mut conn, &w, &b).unwrap();
        assert_eq!(purge_expired(&mut conn, &w, 30).unwrap(), 0);
        assert_eq!(purge_expired(&mut conn, &w, -1).unwrap(), 1);
        assert!(trash(&conn).unwrap().is_empty());
        let purges: i64 = conn.query_row("SELECT COUNT(*) FROM oplog WHERE kind='purge'", [], |r| r.get(0)).unwrap();
        assert_eq!(purges, 1);
    }
}
