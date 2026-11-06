pub mod apply;
pub mod backend;
// linking more of a person's own devices (link codes are a Store-edition feature)
#[cfg_attr(not(feature = "store"), allow(dead_code))]
pub mod bootstrap;
#[cfg(feature = "store")]
pub mod cloud;
pub mod crypto;
pub mod direct;
pub mod engine;
pub mod hlc;
pub mod identity;
pub mod oplog;
pub mod relay;
// the personal edition's mailbox (kept compiled in the Store edition for its tests)
#[cfg_attr(feature = "store", allow(dead_code))]
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
