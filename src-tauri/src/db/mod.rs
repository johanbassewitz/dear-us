pub mod migrations;
pub mod repo;

use crate::error::AppResult;
use rusqlite::Connection;
use std::path::Path;

pub const DB_FILE: &str = "dear_us.db";

/// Opens (or creates) the local archive database and brings its schema up to date.
pub fn open(path: &Path) -> AppResult<Connection> {
    let mut conn = Connection::open(path)?;
    configure(&conn)?;
    migrations::migrate(&mut conn)?;
    Ok(conn)
}

#[cfg(test)]
pub fn open_in_memory() -> AppResult<Connection> {
    let mut conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrations::migrate(&mut conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = FULL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )?;
    Ok(())
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// Returns 0 when the table doesn't exist yet (feature not built), never a fake number.
pub fn count_if_exists(conn: &Connection, table: &str, where_clause: &str) -> AppResult<i64> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |r| r.get(0),
    )?;
    if !exists {
        return Ok(0);
    }
    let sql = if where_clause.is_empty() {
        format!("SELECT COUNT(*) FROM \"{table}\"")
    } else {
        format!("SELECT COUNT(*) FROM \"{table}\" WHERE {where_clause}")
    };
    Ok(conn.query_row(&sql, [], |r| r.get(0))?)
}
