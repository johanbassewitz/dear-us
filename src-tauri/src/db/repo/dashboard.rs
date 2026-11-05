//! Home dashboard numbers. Only real counts from the local archive — never invented.
//! Tables for features not built yet simply report zero.

use crate::db::count_if_exists;
use crate::error::AppResult;
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSummary {
    pub memories: i64,
    pub bucket_done: i64,
    pub bucket_total: i64,
}

pub fn summary(conn: &Connection) -> AppResult<DashboardSummary> {
    Ok(DashboardSummary {
        memories: count_if_exists(conn, "journal_entries", "deleted_at IS NULL")?,
        bucket_done: count_if_exists(conn, "bucket_items", "deleted_at IS NULL AND status = 'done'")?,
        bucket_total: count_if_exists(conn, "bucket_items", "deleted_at IS NULL")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn fresh_archive_reports_zero() {
        let conn = open_in_memory().unwrap();
        let s = summary(&conn).unwrap();
        assert_eq!((s.memories, s.bucket_done, s.bucket_total), (0, 0, 0));
    }
}
