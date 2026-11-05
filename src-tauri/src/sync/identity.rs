//! Who we are, shared with the partner: my display name and the space details travel as
//! ordinary ops ("user", "space"), so both PCs agree on names and the relationship start date.

use super::oplog::Writer;
use crate::error::AppResult;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

/// Records the current profile as ops (call after profile changes and when pairing).
pub fn record_profile_ops(conn: &mut Connection) -> AppResult<()> {
    let device_id = super::device_id(conn)?;
    let w = Writer { device_id };
    let me: Option<(String, String)> =
        conn.query_row("SELECT id, display_name FROM users WHERE is_self = 1", [], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
    let space: Option<(String, String, Option<String>)> =
        conn.query_row("SELECT id, name, start_date FROM space LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
    let tx = conn.transaction()?;
    if let Some((id, name)) = me {
        w.record(&tx, "user", &id, "upsert", json!({ "display_name": name }))?;
    }
    if let Some((id, name, start)) = space {
        let hlc = w.record(&tx, "space", &id, "upsert", json!({ "name": name, "start_date": start }))?;
        // our own clock for these fields, so an older remote value can't overwrite them
        for f in ["name", "start_date"] {
            super::apply::meta_set(&tx, &format!("clock:space:{f}"), &hlc)?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// The joining PC takes the shared space id (it seeds e.g. the daily question).
pub fn adopt_space_id(conn: &Connection, space_id: &str) -> AppResult<()> {
    conn.execute("UPDATE space SET id = ?1", params![space_id])?;
    Ok(())
}

pub fn my_user_id(conn: &Connection) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT id FROM users WHERE is_self = 1", [], |r| r.get(0)).optional()?)
}
