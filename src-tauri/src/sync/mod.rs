pub mod apply;
pub mod crypto;
pub mod direct;
pub mod engine;
pub mod hlc;
pub mod identity;
pub mod oplog;
pub mod relay;
pub mod supabase;
pub mod service;
#[cfg(test)]
mod tests;

use crate::db::new_id;
use crate::error::AppResult;
use rusqlite::{params, Connection, OptionalExtension};

/// This install's stable device id (created once, kept in `local_meta`).
pub fn device_id(conn: &Connection) -> AppResult<String> {
    if let Some(id) = conn
        .query_row("SELECT value FROM local_meta WHERE key = 'device_id'", [], |r| r.get(0))
        .optional()?
    {
        return Ok(id);
    }
    let id = new_id();
    conn.execute("INSERT INTO local_meta(key, value) VALUES ('device_id', ?1)", params![id])?;
    Ok(id)
}
