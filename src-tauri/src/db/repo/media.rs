//! Media rows. Files themselves are handled by `crate::media`.

use crate::db::{new_id, now_iso};
use crate::error::AppResult;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

/// What the UI needs to show a photo or video.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MediaRef {
    pub id: String,
    pub kind: String,
    pub mime: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    pub taken_at: Option<String>,
    pub has_thumb: bool,
    /// none = original is shown as-is; needed = still converting; image/video = converted copy;
    /// failed = conversion failed (original kept, safe).
    pub preview: String,
    pub availability: String,
    pub original_name: Option<String>,
    pub size: i64,
}

/// Internal row with storage details.
#[derive(Debug, Clone)]
pub struct MediaRow {
    pub r: MediaRef,
    pub hash: String,
    pub ext: String,
}

pub const COLS: &str = "id, kind, mime, width, height, duration_ms, taken_at, has_thumb, preview, availability, original_name, size, hash, ext";

pub fn from_row(row: &Row, offset: usize) -> rusqlite::Result<MediaRow> {
    let g = |i: usize| offset + i;
    Ok(MediaRow {
        r: MediaRef {
            id: row.get(g(0))?,
            kind: row.get(g(1))?,
            mime: row.get(g(2))?,
            width: row.get(g(3))?,
            height: row.get(g(4))?,
            duration_ms: row.get(g(5))?,
            taken_at: row.get(g(6))?,
            has_thumb: row.get::<_, i64>(g(7))? != 0,
            preview: row.get(g(8))?,
            availability: row.get(g(9))?,
            original_name: row.get(g(10))?,
            size: row.get(g(11))?,
        },
        hash: row.get(g(12))?,
        ext: row.get(g(13))?,
    })
}

pub struct NewMedia {
    pub hash: String,
    pub kind: String,
    pub mime: String,
    pub ext: String,
    pub size: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    pub taken_at: Option<String>,
    pub original_name: Option<String>,
    pub preview: String,
    pub created_by: Option<String>,
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Option<MediaRow>> {
    Ok(conn
        .query_row(&format!("SELECT {COLS} FROM media WHERE id = ?1"), [id], |r| from_row(r, 0))
        .optional()?)
}

pub fn find_by_hash(conn: &Connection, hash: &str) -> AppResult<Option<MediaRow>> {
    Ok(conn
        .query_row(&format!("SELECT {COLS} FROM media WHERE hash = ?1"), [hash], |r| from_row(r, 0))
        .optional()?)
}

pub fn insert(conn: &Connection, m: &NewMedia) -> AppResult<MediaRow> {
    let id = new_id();
    conn.execute(
        "INSERT INTO media(id, hash, kind, mime, ext, size, width, height, duration_ms, taken_at,
                           original_name, preview, created_by, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
        params![
            id, m.hash, m.kind, m.mime, m.ext, m.size, m.width, m.height, m.duration_ms, m.taken_at,
            m.original_name, m.preview, m.created_by, now_iso()
        ],
    )?;
    Ok(get(conn, &id)?.expect("just inserted"))
}

pub fn set_thumb(conn: &Connection, id: &str, has: bool) -> AppResult<()> {
    conn.execute("UPDATE media SET has_thumb = ?2 WHERE id = ?1", params![id, has as i64])?;
    Ok(())
}

pub fn set_preview(conn: &Connection, id: &str, state: &str) -> AppResult<()> {
    conn.execute("UPDATE media SET preview = ?2 WHERE id = ?1", params![id, state])?;
    Ok(())
}

pub fn set_dimensions(conn: &Connection, id: &str, w: Option<i64>, h: Option<i64>, dur: Option<i64>) -> AppResult<()> {
    conn.execute(
        "UPDATE media SET width = COALESCE(?2, width), height = COALESCE(?3, height),
                          duration_ms = COALESCE(?4, duration_ms) WHERE id = ?1",
        params![id, w, h, dur],
    )?;
    Ok(())
}

pub fn needing_preview(conn: &Connection) -> AppResult<Vec<MediaRow>> {
    let mut stmt = conn.prepare(&format!("SELECT {COLS} FROM media WHERE preview = 'needed'"))?;
    let rows = stmt.query_map([], |r| from_row(r, 0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Media nothing points to anymore, imported before `before` (grace period for open editors).
pub fn unreferenced(conn: &Connection, before: &str) -> AppResult<Vec<MediaRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM media m WHERE m.created_at < ?1
           AND NOT EXISTS (SELECT 1 FROM entry_media em WHERE em.media_id = m.id)
           AND NOT EXISTS (SELECT 1 FROM journal_entries j WHERE j.cover_media_id = m.id)
           AND NOT EXISTS (SELECT 1 FROM milestones s WHERE s.cover_media_id = m.id)
           AND NOT EXISTS (SELECT 1 FROM bucket_media bm WHERE bm.media_id = m.id)
           AND NOT EXISTS (SELECT 1 FROM bucket_items b WHERE b.cover_media_id = m.id)
           AND NOT EXISTS (SELECT 1 FROM letter_media lm WHERE lm.media_id = m.id)"
    ))?;
    let rows = stmt.query_map([before], |r| from_row(r, 0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn delete_row(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM media WHERE id = ?1", [id])?;
    Ok(())
}

pub fn exists_all(conn: &Connection, ids: &[String]) -> AppResult<bool> {
    for id in ids {
        let ok: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM media WHERE id = ?1)", [id], |r| r.get(0))?;
        if !ok {
            return Ok(false);
        }
    }
    Ok(true)
}
