//! Special dates: anniversaries, birthdays, trips. The countdown maths lives in the frontend.

use super::journal::validate_date;
use crate::db::{new_id, now_iso};
use crate::error::{AppError, AppResult};
use crate::sync::oplog::{stamp_fields, Writer};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const ENTITY: &str = "special_date";

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpecialInput {
    pub title: String,
    pub date: String,
    pub time: Option<String>,
    #[serde(default)]
    pub description: String,
    pub recurrence: String,
    pub remind_days_before: Option<i64>,
    pub linked_entry_id: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpecialDate {
    pub id: String,
    pub title: String,
    pub date: String,
    pub time: Option<String>,
    pub description: String,
    pub recurrence: String,
    pub remind_days_before: Option<i64>,
    pub linked_entry_id: Option<String>,
    pub linked_entry_title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, PartialEq, Clone)]
struct Clean {
    title: String,
    date: String,
    time: Option<String>,
    description: String,
    recurrence: String,
    remind: Option<i64>,
    linked: Option<String>,
}

fn clean(conn: &Connection, i: &SpecialInput) -> AppResult<Clean> {
    let title = i.title.trim().to_string();
    if title.is_empty() || title.chars().count() > 200 {
        return Err(AppError::invalid("A special date needs a title (200 characters max)"));
    }
    let description = i.description.trim().to_string();
    if description.chars().count() > 2000 {
        return Err(AppError::invalid("description is too long"));
    }
    let time = match i.time.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(v) => Some(
            chrono::NaiveTime::parse_from_str(v, "%H:%M")
                .map(|_| v.to_string())
                .map_err(|_| AppError::invalid("time must be HH:MM"))?,
        ),
    };
    if !["none", "yearly", "monthly"].contains(&i.recurrence.as_str()) {
        return Err(AppError::invalid("pick how often it repeats: never, yearly or monthly"));
    }
    if let Some(d) = i.remind_days_before {
        if !(0..=60).contains(&d) {
            return Err(AppError::invalid("reminders can be 0 to 60 days before"));
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
    Ok(Clean {
        title,
        date: validate_date(&i.date)?,
        time,
        description,
        recurrence: i.recurrence.clone(),
        remind: i.remind_days_before,
        linked,
    })
}

fn fields(c: &Clean) -> Vec<(&'static str, Value)> {
    vec![
        ("title", json!(c.title)),
        ("date", json!(c.date)),
        ("time", json!(c.time)),
        ("description", json!(c.description)),
        ("recurrence", json!(c.recurrence)),
        ("remind_days_before", json!(c.remind)),
        ("linked_entry_id", json!(c.linked)),
    ]
}

pub fn create(conn: &mut Connection, w: &Writer, author: &str, i: &SpecialInput) -> AppResult<String> {
    let c = clean(conn, i)?;
    let id = new_id();
    let now = now_iso();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO special_dates(id, title, date, time, description, recurrence, remind_days_before, linked_entry_id, created_by, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10)",
        params![id, c.title, c.date, c.time, c.description, c.recurrence, c.remind, c.linked, author, now],
    )?;
    let f = fields(&c);
    let mut ch: serde_json::Map<String, Value> = f.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    ch.insert("created_by".into(), json!(author));
    let hlc = w.record(&tx, ENTITY, &id, "upsert", Value::Object(ch))?;
    let clock = stamp_fields("{}", f.iter().map(|(k, _)| k.to_string()), &hlc);
    tx.execute("UPDATE special_dates SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
    tx.commit()?;
    Ok(id)
}

pub fn update(conn: &mut Connection, w: &Writer, id: &str, i: &SpecialInput) -> AppResult<()> {
    let next = clean(conn, i)?;
    let cur = conn
        .query_row(
            "SELECT title, date, time, description, recurrence, remind_days_before, linked_entry_id, field_clock
             FROM special_dates WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |r| {
                Ok((
                    Clean {
                        title: r.get(0)?,
                        date: r.get(1)?,
                        time: r.get(2)?,
                        description: r.get(3)?,
                        recurrence: r.get(4)?,
                        remind: r.get(5)?,
                        linked: r.get(6)?,
                    },
                    r.get::<_, String>(7)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::invalid("this special date doesn't exist anymore"))?;
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
        "UPDATE special_dates SET title=?2, date=?3, time=?4, description=?5, recurrence=?6, remind_days_before=?7, linked_entry_id=?8, updated_at=?9 WHERE id=?1",
        params![id, next.title, next.date, next.time, next.description, next.recurrence, next.remind, next.linked, now_iso()],
    )?;
    let ch: serde_json::Map<String, Value> = changed.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    let hlc = w.record(&tx, ENTITY, id, "upsert", Value::Object(ch))?;
    let clock = stamp_fields(&cur.1, changed.iter().map(|(k, _)| k.to_string()), &hlc);
    tx.execute("UPDATE special_dates SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
    tx.commit()?;
    Ok(())
}

pub fn delete(conn: &mut Connection, w: &Writer, id: &str) -> AppResult<()> {
    let now = now_iso();
    let tx = conn.transaction()?;
    let n = tx.execute(
        "UPDATE special_dates SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, now],
    )?;
    if n == 0 {
        return Err(AppError::invalid("this special date doesn't exist anymore"));
    }
    w.record(&tx, ENTITY, id, "delete", json!({ "deleted_at": now }))?;
    tx.commit()?;
    Ok(())
}

pub fn restore(conn: &mut Connection, w: &Writer, id: &str) -> AppResult<()> {
    let now = now_iso();
    let tx = conn.transaction()?;
    let n = tx.execute(
        "UPDATE special_dates SET deleted_at = NULL, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NOT NULL",
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
        let mut s = conn.prepare("SELECT id FROM special_dates WHERE deleted_at IS NOT NULL AND deleted_at < ?1")?;
        let r = s.query_map([cutoff], |r| r.get(0))?;
        r.collect::<Result<_, _>>()?
    };
    for id in &ids {
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM special_dates WHERE id = ?1", [id])?;
        w.record(&tx, ENTITY, id, "purge", json!({}))?;
        tx.commit()?;
    }
    Ok(ids.len())
}

/// All live special dates by stored date. The page works out occurrences.
pub fn list(conn: &Connection) -> AppResult<Vec<SpecialDate>> {
    let mut s = conn.prepare(
        "SELECT s.id, s.title, s.date, s.time, s.description, s.recurrence, s.remind_days_before,
                s.linked_entry_id, j.title, s.created_at, s.updated_at
         FROM special_dates s
         LEFT JOIN journal_entries j ON j.id = s.linked_entry_id AND j.deleted_at IS NULL
         WHERE s.deleted_at IS NULL ORDER BY s.date, s.id",
    )?;
    let rows = s.query_map([], |r| {
        let linked: Option<String> = r.get(7)?;
        let ltitle: Option<String> = r.get(8)?;
        Ok(SpecialDate {
            id: r.get(0)?,
            title: r.get(1)?,
            date: r.get(2)?,
            time: r.get(3)?,
            description: r.get(4)?,
            recurrence: r.get(5)?,
            remind_days_before: r.get(6)?,
            linked_entry_id: if ltitle.is_some() { linked } else { None },
            linked_entry_title: ltitle,
            created_at: r.get(9)?,
            updated_at: r.get(10)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::journal::{self, tests as jt};

    fn input(title: &str, date: &str) -> SpecialInput {
        SpecialInput {
            title: title.into(),
            date: date.into(),
            time: None,
            description: "".into(),
            recurrence: "yearly".into(),
            remind_days_before: Some(7),
            linked_entry_id: None,
        }
    }

    fn oplog_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM oplog WHERE entity = ?1", [ENTITY], |r| r.get(0)).unwrap()
    }

    #[test]
    fn validates_input() {
        let (mut conn, w, me) = jt::setup();
        assert!(create(&mut conn, &w, &me, &input("", "2025-01-01")).is_err());
        assert!(create(&mut conn, &w, &me, &input("x", "nope")).is_err());
        let mut bad = input("x", "2025-01-01");
        bad.recurrence = "weekly".into();
        assert!(create(&mut conn, &w, &me, &bad).is_err());
        bad = input("x", "2025-01-01");
        bad.remind_days_before = Some(61);
        assert!(create(&mut conn, &w, &me, &bad).is_err());
        bad = input("x", "2025-01-01");
        bad.time = Some("25:99".into());
        assert!(create(&mut conn, &w, &me, &bad).is_err());
        bad = input("x", "2025-01-01");
        bad.linked_entry_id = Some("ghost".into());
        assert!(create(&mut conn, &w, &me, &bad).is_err());
        assert_eq!(oplog_count(&conn), 0);
    }

    #[test]
    fn create_update_delete_restore_write_oplog() {
        let (mut conn, w, me) = jt::setup();
        let e = journal::create(&mut conn, &w, &me, &jt::input("Beach", "2025-07-12")).unwrap();
        let id = create(&mut conn, &w, &me, &input("Anniversary", "2024-07-03")).unwrap();
        assert_eq!(oplog_count(&conn), 1);
        let mut next = input("Our anniversary", "2024-07-03");
        next.time = Some("19:30".into());
        next.linked_entry_id = Some(e.summary.id.clone());
        update(&mut conn, &w, &id, &next).unwrap();
        assert_eq!(oplog_count(&conn), 2);
        update(&mut conn, &w, &id, &next).unwrap();
        assert_eq!(oplog_count(&conn), 2);
        let l = list(&conn).unwrap();
        assert_eq!(l[0].title, "Our anniversary");
        assert_eq!(l[0].time.as_deref(), Some("19:30"));
        assert_eq!(l[0].linked_entry_title.as_deref(), Some("Beach"));
        delete(&mut conn, &w, &id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
        assert!(delete(&mut conn, &w, &id).is_err());
        restore(&mut conn, &w, &id).unwrap();
        assert_eq!(list(&conn).unwrap().len(), 1);
        assert!(restore(&mut conn, &w, &id).is_err());
        assert_eq!(oplog_count(&conn), 4);
        let clock: String =
            conn.query_row("SELECT field_clock FROM special_dates WHERE id=?1", [&id], |r| r.get(0)).unwrap();
        assert!(clock.contains("title") && clock.contains("time"));
    }

    #[test]
    fn purges_only_old_deleted() {
        let (mut conn, w, me) = jt::setup();
        let a = create(&mut conn, &w, &me, &input("a", "2025-01-01")).unwrap();
        let b = create(&mut conn, &w, &me, &input("b", "2025-01-02")).unwrap();
        delete(&mut conn, &w, &a).unwrap();
        assert_eq!(purge_expired(&mut conn, &w, 30).unwrap(), 0);
        conn.execute("UPDATE special_dates SET deleted_at = '2000-01-01T00:00:00.000Z' WHERE id = ?1", [&a]).unwrap();
        assert_eq!(purge_expired(&mut conn, &w, 30).unwrap(), 1);
        assert_eq!(list(&conn).unwrap()[0].id, b);
    }
}
