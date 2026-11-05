//! Local-only preferences (theme, last page, Cupid position…). Never synced.

use crate::db::now_iso;
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection};
use std::collections::HashMap;

const MAX_VALUE_LEN: usize = 16 * 1024;

fn validate_key(key: &str) -> AppResult<()> {
    let ok = !key.is_empty()
        && key.len() <= 64
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-');
    if ok {
        Ok(())
    } else {
        Err(AppError::invalid(format!("invalid setting key: {key}")))
    }
}

pub fn get_all(conn: &Connection) -> AppResult<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn set(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    validate_key(key)?;
    if value.len() > MAX_VALUE_LEN {
        return Err(AppError::invalid("setting value too large"));
    }
    conn.execute(
        "INSERT INTO settings(key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, now_iso()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn set_get_and_overwrite() {
        let conn = open_in_memory().unwrap();
        set(&conn, "ui.theme", "\"rose\"").unwrap();
        set(&conn, "ui.theme", "\"sage\"").unwrap();
        let all = get_all(&conn).unwrap();
        assert_eq!(all.get("ui.theme").unwrap(), "\"sage\"");
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn rejects_bad_keys() {
        let conn = open_in_memory().unwrap();
        assert!(set(&conn, "", "x").is_err());
        assert!(set(&conn, "a b", "x").is_err());
        assert!(set(&conn, "x'; DROP TABLE settings;--", "x").is_err());
    }
}
