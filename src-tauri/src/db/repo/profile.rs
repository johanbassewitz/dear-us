//! The relationship space and the two people in it.

use crate::db::{new_id, now_iso};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Space {
    pub id: String,
    pub name: String,
    pub start_date: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub space: Option<Space>,
    pub me: Option<Person>,
    pub partner: Option<Person>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInput {
    pub my_name: String,
    pub partner_name: Option<String>,
    pub space_name: String,
    pub start_date: Option<String>,
}

fn clean(s: &str, field: &str, max: usize) -> AppResult<String> {
    let t = s.trim();
    if t.is_empty() {
        return Err(AppError::invalid(format!("{field} can't be empty")));
    }
    if t.chars().count() > max {
        return Err(AppError::invalid(format!("{field} is too long")));
    }
    Ok(t.to_string())
}

fn clean_opt(s: &Option<String>, field: &str, max: usize) -> AppResult<Option<String>> {
    match s.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(v) => clean(v, field, max).map(Some),
    }
}

fn validate_date(d: &Option<String>) -> AppResult<Option<String>> {
    match d.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(v) => chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d")
            .map(|_| Some(v.to_string()))
            .map_err(|_| AppError::invalid("start date must be YYYY-MM-DD")),
    }
}

pub fn get(conn: &Connection) -> AppResult<Profile> {
    let space = conn
        .query_row("SELECT id, name, start_date FROM space LIMIT 1", [], |r| {
            Ok(Space { id: r.get(0)?, name: r.get(1)?, start_date: r.get(2)? })
        })
        .optional()?;
    let person = |is_self: i64| {
        conn.query_row(
            "SELECT id, display_name FROM users WHERE is_self = ?1 ORDER BY created_at LIMIT 1",
            [is_self],
            |r| Ok(Person { id: r.get(0)?, display_name: r.get(1)? }),
        )
        .optional()
    };
    Ok(Profile { space, me: person(1)?, partner: person(0)? })
}

/// Creates or updates the space and both people in one transaction.
pub fn save(conn: &mut Connection, input: &ProfileInput) -> AppResult<Profile> {
    let my_name = clean(&input.my_name, "Your name", 60)?;
    let partner_name = clean_opt(&input.partner_name, "Partner's name", 60)?;
    let space_name = clean(&input.space_name, "Space name", 80)?;
    let start_date = validate_date(&input.start_date)?;
    let now = now_iso();

    let tx = conn.transaction()?;
    let current = get(&tx)?;

    match current.space {
        Some(s) => tx.execute(
            "UPDATE space SET name=?1, start_date=?2, updated_at=?3 WHERE id=?4",
            params![space_name, start_date, now, s.id],
        )?,
        None => tx.execute(
            "INSERT INTO space(id, name, start_date, created_at, updated_at) VALUES (?1,?2,?3,?4,?4)",
            params![new_id(), space_name, start_date, now],
        )?,
    };

    match current.me {
        Some(p) => tx.execute(
            "UPDATE users SET display_name=?1, updated_at=?2 WHERE id=?3",
            params![my_name, now, p.id],
        )?,
        None => tx.execute(
            "INSERT INTO users(id, display_name, is_self, created_at, updated_at) VALUES (?1,?2,1,?3,?3)",
            params![new_id(), my_name, now],
        )?,
    };

    match (current.partner, partner_name) {
        (Some(p), Some(name)) => {
            tx.execute(
                "UPDATE users SET display_name=?1, updated_at=?2 WHERE id=?3",
                params![name, now, p.id],
            )?;
        }
        (None, Some(name)) => {
            tx.execute(
                "INSERT INTO users(id, display_name, is_self, created_at, updated_at) VALUES (?1,?2,0,?3,?3)",
                params![new_id(), name, now],
            )?;
        }
        // Clearing the partner's name keeps the row (it will matter once content references it).
        (_, None) => {}
    }

    tx.commit()?;
    get(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    fn input(me: &str, partner: Option<&str>, date: Option<&str>) -> ProfileInput {
        ProfileInput {
            my_name: me.into(),
            partner_name: partner.map(Into::into),
            space_name: "Our little world".into(),
            start_date: date.map(Into::into),
        }
    }

    #[test]
    fn empty_profile_on_fresh_install() {
        let conn = open_in_memory().unwrap();
        let p = get(&conn).unwrap();
        assert!(p.space.is_none() && p.me.is_none() && p.partner.is_none());
    }

    #[test]
    fn create_then_update_keeps_ids() {
        let mut conn = open_in_memory().unwrap();
        let a = save(&mut conn, &input("Jo", Some("Sam"), Some("2023-05-14"))).unwrap();
        let b = save(&mut conn, &input("  Johan ", Some("Sam"), None)).unwrap();
        assert_eq!(a.me.as_ref().unwrap().id, b.me.as_ref().unwrap().id);
        assert_eq!(b.me.unwrap().display_name, "Johan");
        assert_eq!(b.space.unwrap().start_date, None);
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn validates_input() {
        let mut conn = open_in_memory().unwrap();
        assert!(save(&mut conn, &input("  ", None, None)).is_err());
        assert!(save(&mut conn, &input("Jo", None, Some("14/05/2023"))).is_err());
        // nothing half-written
        assert!(get(&conn).unwrap().space.is_none());
    }
}
