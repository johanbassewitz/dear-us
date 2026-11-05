//! Records every shared change. Must be called inside the same transaction as the change
//! itself, so the archive and the log can never disagree.

use crate::db::{new_id, now_iso};
use crate::error::AppResult;
use rusqlite::{params, Connection};
use serde_json::{Map, Value};

pub struct Writer {
    pub device_id: String,
}

impl Writer {
    /// Logs the op and returns the HLC used, so the caller can stamp `field_clock`.
    pub fn record(&self, conn: &Connection, entity: &str, entity_id: &str, kind: &str, changes: Value) -> AppResult<String> {
        let hlc = super::hlc::next(conn, &self.device_id)?;
        conn.execute(
            "INSERT INTO oplog(op_id, hlc, device_id, entity, entity_id, kind, changes_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![new_id(), hlc, self.device_id, entity, entity_id, kind, changes.to_string(), now_iso()],
        )?;
        Ok(hlc)
    }
}

/// Updates a record's `field_clock` JSON: every changed field gets this op's HLC.
pub fn stamp_fields(existing: &str, fields: impl IntoIterator<Item = String>, hlc: &str) -> String {
    let mut map: Map<String, Value> = serde_json::from_str(existing).unwrap_or_default();
    for f in fields {
        map.insert(f, Value::String(hlc.to_string()));
    }
    Value::Object(map).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_only_changed_fields() {
        let a = stamp_fields("{}", ["title".to_string(), "body".to_string()], "h1");
        let b = stamp_fields(&a, ["title".to_string()], "h2");
        let v: Value = serde_json::from_str(&b).unwrap();
        assert_eq!(v["title"], "h2");
        assert_eq!(v["body"], "h1");
    }
}
