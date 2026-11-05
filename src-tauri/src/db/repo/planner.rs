//! Date planner: shared plans (idea -> planned -> done) with a checklist and a budget, plus the
//! couple's own date ideas (the curated library ships with the app and is not stored here).

use super::journal::validate_date;
use crate::db::{new_id, now_iso};
use crate::error::{AppError, AppResult};
use crate::sync::oplog::{stamp_fields, Writer};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const ENTITY: &str = "planned_date";
pub const IDEA_ENTITY: &str = "date_idea";
const MAX_CHECKLIST: usize = 50;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct ChecklistItem {
    #[serde(default)]
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub done: bool,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlanInput {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub date: Option<String>,
    pub time: Option<String>,
    #[serde(default)]
    pub location: String,
    pub budget_cents: Option<i64>,
    #[serde(default)]
    pub currency: String,
    pub status: String,
    #[serde(default)]
    pub checklist: Vec<ChecklistItem>,
    pub idea_ref: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    pub title: String,
    pub description: String,
    pub date: Option<String>,
    pub time: Option<String>,
    pub location: String,
    pub budget_cents: Option<i64>,
    pub currency: String,
    pub status: String,
    pub checklist: Vec<ChecklistItem>,
    pub idea_ref: Option<String>,
    pub linked_entry_id: Option<String>,
    pub linked_entry_title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IdeaInput {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub category: String,
    pub budget: String,
    pub duration: String,
    pub setting: String,
    pub mood: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Idea {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub budget: String,
    pub duration: String,
    pub setting: String,
    pub mood: String,
}

#[derive(Debug, Clone, PartialEq)]
struct Fields {
    title: String,
    description: String,
    date: Option<String>,
    time: Option<String>,
    location: String,
    budget_cents: Option<i64>,
    currency: String,
    status: String,
    checklist: Vec<ChecklistItem>,
    idea_ref: Option<String>,
    linked: Option<String>,
}

fn checklist_text(c: &[ChecklistItem]) -> String {
    serde_json::to_string(c).unwrap_or_else(|_| "[]".into())
}

fn to_json(f: &Fields) -> Vec<(&'static str, Value)> {
    vec![
        ("title", json!(f.title)),
        ("description", json!(f.description)),
        ("date", json!(f.date)),
        ("time", json!(f.time)),
        ("location", json!(f.location)),
        ("budget_cents", json!(f.budget_cents)),
        ("currency", json!(f.currency)),
        ("status", json!(f.status)),
        ("checklist_json", json!(checklist_text(&f.checklist))),
        ("idea_ref", json!(f.idea_ref)),
        ("linked_entry_id", json!(f.linked)),
    ]
}

fn check_status(s: &str) -> AppResult<()> {
    match s {
        "idea" | "planned" | "done" | "cancelled" => Ok(()),
        _ => Err(AppError::invalid("pick Idea, Planned, Done or Cancelled")),
    }
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

fn clean(i: &PlanInput, linked: Option<String>) -> AppResult<Fields> {
    let title = i.title.trim().to_string();
    if title.is_empty() || title.chars().count() > 200 {
        return Err(AppError::invalid("Give this date a name (200 characters max)"));
    }
    let description = i.description.trim().to_string();
    if description.chars().count() > 2000 {
        return Err(AppError::invalid("the description is too long"));
    }
    let location = i.location.trim().to_string();
    if location.chars().count() > 200 {
        return Err(AppError::invalid("the location is too long (200 characters max)"));
    }
    check_status(&i.status)?;
    let date = match i.date.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(d) => Some(validate_date(d)?),
    };
    if i.status == "planned" && date.is_none() {
        return Err(AppError::invalid("pick a day for a planned date, or keep it as an idea"));
    }
    let time = match i.time.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(v) => Some(
            chrono::NaiveTime::parse_from_str(v, "%H:%M")
                .map(|_| v.to_string())
                .map_err(|_| AppError::invalid("time must be HH:MM"))?,
        ),
    };
    if let Some(b) = i.budget_cents {
        if !(0..=100_000_000_000).contains(&b) {
            return Err(AppError::invalid("the budget must be a positive amount"));
        }
    }
    let currency = if i.currency.trim().is_empty() { "USD".to_string() } else { i.currency.trim().to_uppercase() };
    if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_uppercase()) {
        return Err(AppError::invalid("currency must be a 3-letter code like USD"));
    }
    if i.checklist.len() > MAX_CHECKLIST {
        return Err(AppError::invalid(format!("up to {MAX_CHECKLIST} checklist items")));
    }
    let mut checklist: Vec<ChecklistItem> = vec![];
    for c in &i.checklist {
        let text = c.text.trim().to_string();
        if text.is_empty() {
            continue;
        }
        if text.chars().count() > 200 {
            return Err(AppError::invalid("a checklist item is too long (200 characters max)"));
        }
        let mut id = if c.id.trim().is_empty() { new_id() } else { c.id.trim().to_string() };
        if checklist.iter().any(|x| x.id == id) {
            id = new_id();
        }
        checklist.push(ChecklistItem { id, text, done: c.done });
    }
    Ok(Fields {
        title,
        description,
        date,
        time,
        location,
        budget_cents: i.budget_cents,
        currency,
        status: i.status.clone(),
        checklist,
        idea_ref: i.idea_ref.clone().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        linked,
    })
}

fn load(conn: &Connection, id: &str) -> AppResult<(Fields, String)> {
    conn.query_row(
        "SELECT title, description, date, time, location, budget_cents, currency, status, checklist_json,
                idea_ref, linked_entry_id, field_clock
         FROM planned_dates WHERE id = ?1 AND deleted_at IS NULL",
        [id],
        |r| {
            let cl: String = r.get(8)?;
            Ok((
                Fields {
                    title: r.get(0)?,
                    description: r.get(1)?,
                    date: r.get(2)?,
                    time: r.get(3)?,
                    location: r.get(4)?,
                    budget_cents: r.get(5)?,
                    currency: r.get(6)?,
                    status: r.get(7)?,
                    checklist: serde_json::from_str(&cl).unwrap_or_default(),
                    idea_ref: r.get(9)?,
                    linked: r.get(10)?,
                },
                r.get::<_, String>(11)?,
            ))
        },
    )
    .optional()?
    .ok_or_else(|| AppError::invalid("this date doesn't exist anymore"))
}

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
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE planned_dates SET title=?2, description=?3, date=?4, time=?5, location=?6, budget_cents=?7, currency=?8,
             status=?9, checklist_json=?10, idea_ref=?11, linked_entry_id=?12, updated_at=?13 WHERE id=?1",
        params![
            id, next.title, next.description, next.date, next.time, next.location, next.budget_cents, next.currency,
            next.status, checklist_text(&next.checklist), next.idea_ref, next.linked, now_iso()
        ],
    )?;
    let ch: serde_json::Map<String, Value> = changed.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    let hlc = w.record(&tx, ENTITY, id, "upsert", Value::Object(ch))?;
    let clock = stamp_fields(clock, changed.iter().map(|(k, _)| k.to_string()), &hlc);
    tx.execute("UPDATE planned_dates SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
    tx.commit()?;
    Ok(())
}

pub fn create(conn: &mut Connection, w: &Writer, author: &str, i: &PlanInput) -> AppResult<String> {
    let c = clean(i, None)?;
    let id = new_id();
    let now = now_iso();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO planned_dates(id, title, description, date, time, location, budget_cents, currency, status,
                                   checklist_json, idea_ref, linked_entry_id, created_by, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,NULL,?12,?13,?13)",
        params![
            id, c.title, c.description, c.date, c.time, c.location, c.budget_cents, c.currency, c.status,
            checklist_text(&c.checklist), c.idea_ref, author, now
        ],
    )?;
    let f = to_json(&c);
    let mut ch: serde_json::Map<String, Value> = f.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    ch.insert("created_by".into(), json!(author));
    ch.insert("created_at".into(), json!(now));
    let hlc = w.record(&tx, ENTITY, &id, "upsert", Value::Object(ch))?;
    let clock = stamp_fields("{}", f.iter().map(|(k, _)| k.to_string()), &hlc);
    tx.execute("UPDATE planned_dates SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
    tx.commit()?;
    Ok(id)
}

pub fn update(conn: &mut Connection, w: &Writer, id: &str, i: &PlanInput) -> AppResult<()> {
    let (cur, clock) = load(conn, id)?;
    let next = clean(i, cur.linked.clone())?;
    save(conn, w, id, &cur, &clock, &next)
}

/// Quick status change from the card. Planned still needs a day.
pub fn set_status(conn: &mut Connection, w: &Writer, id: &str, status: &str) -> AppResult<()> {
    check_status(status)?;
    let (cur, clock) = load(conn, id)?;
    if status == "planned" && cur.date.is_none() {
        return Err(AppError::invalid("pick a day for a planned date, or keep it as an idea"));
    }
    let next = Fields { status: status.into(), ..cur.clone() };
    save(conn, w, id, &cur, &clock, &next)
}

/// Ticks (or unticks) one checklist item without touching anything else.
pub fn toggle_check(conn: &mut Connection, w: &Writer, id: &str, item_id: &str, done: bool) -> AppResult<()> {
    let (cur, clock) = load(conn, id)?;
    let mut next = cur.clone();
    let it = next
        .checklist
        .iter_mut()
        .find(|c| c.id == item_id)
        .ok_or_else(|| AppError::invalid("that checklist item doesn't exist anymore"))?;
    it.done = done;
    save(conn, w, id, &cur, &clock, &next)
}

/// Links (or, with None, unlinks) the journal memory written about this date.
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
    soft(conn, w, "planned_dates", ENTITY, id, true, "this date doesn't exist anymore")
}

pub fn restore(conn: &mut Connection, w: &Writer, id: &str) -> AppResult<()> {
    soft(conn, w, "planned_dates", ENTITY, id, false, "nothing to restore")
}

fn soft(conn: &mut Connection, w: &Writer, table: &str, entity: &str, id: &str, delete: bool, msg: &str) -> AppResult<()> {
    let now = now_iso();
    let tx = conn.transaction()?;
    let sql = if delete {
        format!("UPDATE {table} SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NULL")
    } else {
        format!("UPDATE {table} SET deleted_at = NULL, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NOT NULL")
    };
    if tx.execute(&sql, params![id, now])? == 0 {
        return Err(AppError::invalid(msg));
    }
    if delete {
        w.record(&tx, entity, id, "delete", json!({ "deleted_at": now }))?;
    } else {
        w.record(&tx, entity, id, "restore", json!({}))?;
    }
    tx.commit()?;
    Ok(())
}

pub fn purge_expired(conn: &mut Connection, w: &Writer, retention_days: i64) -> AppResult<usize> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(retention_days))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let mut total = 0;
    for (table, entity) in [("planned_dates", ENTITY), ("date_ideas", IDEA_ENTITY)] {
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

/// All live plans, soonest first; undated ideas last. The page groups them.
pub fn list(conn: &Connection) -> AppResult<Vec<Plan>> {
    let mut s = conn.prepare(
        "SELECT p.id, p.title, p.description, p.date, p.time, p.location, p.budget_cents, p.currency, p.status,
                p.checklist_json, p.idea_ref, p.linked_entry_id, j.title, p.created_at, p.updated_at
         FROM planned_dates p
         LEFT JOIN journal_entries j ON j.id = p.linked_entry_id AND j.deleted_at IS NULL
         WHERE p.deleted_at IS NULL
         ORDER BY p.date IS NULL, p.date, p.time IS NULL, p.time, p.created_at DESC, p.id DESC",
    )?;
    let rows = s.query_map([], |r| {
        let cl: String = r.get(9)?;
        let linked_id: Option<String> = r.get(11)?;
        let linked_title: Option<String> = r.get(12)?;
        Ok(Plan {
            id: r.get(0)?,
            title: r.get(1)?,
            description: r.get(2)?,
            date: r.get(3)?,
            time: r.get(4)?,
            location: r.get(5)?,
            budget_cents: r.get(6)?,
            currency: r.get(7)?,
            status: r.get(8)?,
            checklist: serde_json::from_str(&cl).unwrap_or_default(),
            idea_ref: r.get(10)?,
            // a memory in the recovery bin hides its link until it returns
            linked_entry_id: if linked_title.is_some() { linked_id } else { None },
            linked_entry_title: linked_title,
            created_at: r.get(13)?,
            updated_at: r.get(14)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

// ---- the couple's own ideas ----

fn one_of(v: &str, allowed: &[&str], msg: &str) -> AppResult<String> {
    if allowed.contains(&v) {
        Ok(v.to_string())
    } else {
        Err(AppError::invalid(msg))
    }
}

fn clean_idea(i: &IdeaInput) -> AppResult<IdeaInput> {
    let title = i.title.trim().to_string();
    if title.is_empty() || title.chars().count() > 200 {
        return Err(AppError::invalid("Give your idea a name (200 characters max)"));
    }
    let description = i.description.trim().to_string();
    if description.chars().count() > 2000 {
        return Err(AppError::invalid("the description is too long"));
    }
    Ok(IdeaInput {
        title,
        description,
        category: one_of(
            &i.category,
            &["at-home", "outdoors", "adventure", "food", "entertainment", "low-budget", "special-occasion", "romantic"],
            "pick a category",
        )?,
        budget: one_of(&i.budget, &["free", "low", "medium", "high"], "pick a budget")?,
        duration: one_of(&i.duration, &["short", "half-day", "full-day"], "pick how long it takes")?,
        setting: one_of(&i.setting, &["indoor", "outdoor", "either"], "pick indoor, outdoor or either")?,
        mood: one_of(&i.mood, &["cozy", "playful", "relaxed", "romantic", "adventurous"], "pick a mood")?,
    })
}

pub fn idea_create(conn: &mut Connection, w: &Writer, author: &str, i: &IdeaInput) -> AppResult<String> {
    let c = clean_idea(i)?;
    let id = new_id();
    let now = now_iso();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO date_ideas(id, title, description, category, budget, duration, setting, mood, created_by, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10)",
        params![id, c.title, c.description, c.category, c.budget, c.duration, c.setting, c.mood, author, now],
    )?;
    let ch = json!({
        "title": c.title, "description": c.description, "category": c.category, "budget": c.budget,
        "duration": c.duration, "setting": c.setting, "mood": c.mood, "created_by": author, "created_at": now
    });
    let keys: Vec<String> = ch.as_object().unwrap().keys().cloned().collect();
    let hlc = w.record(&tx, IDEA_ENTITY, &id, "upsert", ch)?;
    let clock = stamp_fields("{}", keys.into_iter(), &hlc);
    tx.execute("UPDATE date_ideas SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
    tx.commit()?;
    Ok(id)
}

pub fn idea_delete(conn: &mut Connection, w: &Writer, id: &str) -> AppResult<()> {
    soft(conn, w, "date_ideas", IDEA_ENTITY, id, true, "this idea doesn't exist anymore")
}

pub fn idea_restore(conn: &mut Connection, w: &Writer, id: &str) -> AppResult<()> {
    soft(conn, w, "date_ideas", IDEA_ENTITY, id, false, "nothing to restore")
}

pub fn idea_list(conn: &Connection) -> AppResult<Vec<Idea>> {
    let mut s = conn.prepare(
        "SELECT id, title, description, category, budget, duration, setting, mood FROM date_ideas
         WHERE deleted_at IS NULL ORDER BY created_at DESC, id DESC",
    )?;
    let rows = s.query_map([], |r| {
        Ok(Idea {
            id: r.get(0)?,
            title: r.get(1)?,
            description: r.get(2)?,
            category: r.get(3)?,
            budget: r.get(4)?,
            duration: r.get(5)?,
            setting: r.get(6)?,
            mood: r.get(7)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::journal::{self, tests as jt};

    fn input(title: &str, status: &str, date: Option<&str>) -> PlanInput {
        PlanInput {
            title: title.into(),
            description: "".into(),
            date: date.map(String::from),
            time: None,
            location: "".into(),
            budget_cents: None,
            currency: "USD".into(),
            status: status.into(),
            checklist: vec![],
            idea_ref: None,
        }
    }

    fn ops(conn: &Connection, entity: &str) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM oplog WHERE entity = ?1", [entity], |r| r.get(0)).unwrap()
    }

    #[test]
    fn validates_input() {
        let (mut conn, w, me) = jt::setup();
        assert!(create(&mut conn, &w, &me, &input("  ", "idea", None)).is_err());
        assert!(create(&mut conn, &w, &me, &input("x", "later", None)).is_err());
        assert!(create(&mut conn, &w, &me, &input("x", "planned", None)).is_err());
        assert!(create(&mut conn, &w, &me, &input("x", "idea", Some("2025-13-40"))).is_err());
        let mut i = input("x", "idea", None);
        i.time = Some("25:99".into());
        assert!(create(&mut conn, &w, &me, &i).is_err());
        i = input("x", "idea", None);
        i.budget_cents = Some(-5);
        assert!(create(&mut conn, &w, &me, &i).is_err());
        i = input("x", "idea", None);
        i.currency = "dollars".into();
        assert!(create(&mut conn, &w, &me, &i).is_err());
        assert_eq!(ops(&conn, ENTITY), 0);
    }

    #[test]
    fn create_update_cleans_checklist_and_logs_ops() {
        let (mut conn, w, me) = jt::setup();
        let mut i = input("Rooftop picnic", "planned", Some("2027-06-01"));
        i.budget_cents = Some(4500);
        i.currency = "eur".into();
        i.checklist = vec![
            ChecklistItem { id: "".into(), text: " Blanket ".into(), done: false },
            ChecklistItem { id: "".into(), text: "   ".into(), done: false },
            ChecklistItem { id: "".into(), text: "Snacks".into(), done: true },
        ];
        let id = create(&mut conn, &w, &me, &i).unwrap();
        assert_eq!(ops(&conn, ENTITY), 1);
        let p = list(&conn).unwrap().remove(0);
        assert_eq!((p.currency.as_str(), p.budget_cents), ("EUR", Some(4500)));
        assert_eq!(p.checklist.len(), 2);
        assert_eq!(p.checklist[0].text, "Blanket");
        assert!(!p.checklist[0].id.is_empty());

        let mut n = i.clone();
        n.checklist = p.checklist.clone();
        n.location = "Home".into();
        update(&mut conn, &w, &id, &n).unwrap();
        assert_eq!(ops(&conn, ENTITY), 2);
        update(&mut conn, &w, &id, &n).unwrap(); // no-op writes nothing
        assert_eq!(ops(&conn, ENTITY), 2);
        let clock: String = conn.query_row("SELECT field_clock FROM planned_dates WHERE id=?1", [&id], |r| r.get(0)).unwrap();
        assert!(clock.contains("location"));
    }

    #[test]
    fn status_and_checklist_toggle() {
        let (mut conn, w, me) = jt::setup();
        let mut i = input("Movie night", "idea", None);
        i.checklist = vec![ChecklistItem { id: "c1".into(), text: "Popcorn".into(), done: false }];
        let id = create(&mut conn, &w, &me, &i).unwrap();
        assert!(set_status(&mut conn, &w, &id, "planned").is_err()); // no day yet
        assert!(set_status(&mut conn, &w, &id, "nope").is_err());
        toggle_check(&mut conn, &w, &id, "c1", true).unwrap();
        assert!(list(&conn).unwrap()[0].checklist[0].done);
        assert!(toggle_check(&mut conn, &w, &id, "ghost", true).is_err());
        let mut n = i.clone();
        n.date = Some("2027-02-02".into());
        n.status = "planned".into();
        n.checklist[0].done = true;
        update(&mut conn, &w, &id, &n).unwrap();
        set_status(&mut conn, &w, &id, "done").unwrap();
        assert_eq!(list(&conn).unwrap()[0].status, "done");
    }

    #[test]
    fn sorted_soonest_first_undated_last() {
        let (mut conn, w, me) = jt::setup();
        create(&mut conn, &w, &me, &input("Idea", "idea", None)).unwrap();
        create(&mut conn, &w, &me, &input("Later", "planned", Some("2027-09-01"))).unwrap();
        create(&mut conn, &w, &me, &input("Sooner", "planned", Some("2027-03-01"))).unwrap();
        let t: Vec<String> = list(&conn).unwrap().into_iter().map(|p| p.title).collect();
        assert_eq!(t, ["Sooner", "Later", "Idea"]);
    }

    #[test]
    fn links_memory_and_hides_link_while_in_bin() {
        let (mut conn, w, me) = jt::setup();
        let id = create(&mut conn, &w, &me, &input("Hike", "idea", None)).unwrap();
        assert!(link_entry(&mut conn, &w, &id, Some("ghost")).is_err());
        let e = journal::create(&mut conn, &w, &me, &jt::input("The hike", "2025-07-12")).unwrap();
        link_entry(&mut conn, &w, &id, Some(&e.summary.id)).unwrap();
        let p = &list(&conn).unwrap()[0];
        assert_eq!(p.linked_entry_title.as_deref(), Some("The hike"));
        // editing the plan keeps the link
        update(&mut conn, &w, &id, &input("Hike 2", "idea", None)).unwrap();
        assert_eq!(list(&conn).unwrap()[0].linked_entry_id.as_deref(), Some(e.summary.id.as_str()));
        journal::delete(&mut conn, &w, &e.summary.id).unwrap();
        assert!(list(&conn).unwrap()[0].linked_entry_id.is_none());
        link_entry(&mut conn, &w, &id, None).unwrap();
    }

    #[test]
    fn soft_delete_restore_purge() {
        let (mut conn, w, me) = jt::setup();
        let a = create(&mut conn, &w, &me, &input("A", "idea", None)).unwrap();
        delete(&mut conn, &w, &a).unwrap();
        assert!(delete(&mut conn, &w, &a).is_err());
        assert!(list(&conn).unwrap().is_empty());
        restore(&mut conn, &w, &a).unwrap();
        assert!(restore(&mut conn, &w, &a).is_err());
        assert_eq!(list(&conn).unwrap().len(), 1);
        let kinds: Vec<String> = {
            let mut st = conn.prepare("SELECT kind FROM oplog WHERE entity='planned_date' AND entity_id=?1 ORDER BY hlc").unwrap();
            st.query_map([&a], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
        };
        assert_eq!(kinds, ["upsert", "delete", "restore"]);
        delete(&mut conn, &w, &a).unwrap();
        assert_eq!(purge_expired(&mut conn, &w, 30).unwrap(), 0);
        assert_eq!(purge_expired(&mut conn, &w, -1).unwrap(), 1);
        let purges: i64 = conn.query_row("SELECT COUNT(*) FROM oplog WHERE kind='purge'", [], |r| r.get(0)).unwrap();
        assert_eq!(purges, 1);
    }

    #[test]
    fn custom_ideas_validate_and_soft_delete() {
        let (mut conn, w, me) = jt::setup();
        let ok = IdeaInput {
            title: "Stargazing on the roof".into(),
            description: "".into(),
            category: "romantic".into(),
            budget: "free".into(),
            duration: "short".into(),
            setting: "outdoor".into(),
            mood: "cozy".into(),
        };
        let mut bad = ok.clone();
        bad.mood = "grumpy".into();
        assert!(idea_create(&mut conn, &w, &me, &bad).is_err());
        bad = ok.clone();
        bad.title = " ".into();
        assert!(idea_create(&mut conn, &w, &me, &bad).is_err());
        assert_eq!(ops(&conn, IDEA_ENTITY), 0);
        let id = idea_create(&mut conn, &w, &me, &ok).unwrap();
        assert_eq!(ops(&conn, IDEA_ENTITY), 1);
        assert_eq!(idea_list(&conn).unwrap().len(), 1);
        idea_delete(&mut conn, &w, &id).unwrap();
        assert!(idea_list(&conn).unwrap().is_empty());
        idea_restore(&mut conn, &w, &id).unwrap();
        assert_eq!(idea_list(&conn).unwrap().len(), 1);
        idea_delete(&mut conn, &w, &id).unwrap();
        assert_eq!(purge_expired(&mut conn, &w, -1).unwrap(), 1);
    }
}
