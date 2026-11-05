//! Journal entries. Every write = row change + op-log record in one transaction.

use super::media::{self as media_repo, MediaRef, COLS as MEDIA_COLS};
use crate::db::{new_id, now_iso};
use crate::error::{AppError, AppResult};
use crate::sync::oplog::{stamp_fields, Writer};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const ENTITY: &str = "journal_entry";
pub const DEFAULT_RETENTION_DAYS: i64 = 30;
const MAX_BODY_BYTES: usize = 512 * 1024;
const MAX_MEDIA: usize = 300;
const MAX_TAGS: usize = 20;

// ---------- input ----------

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EntryMediaInput {
    pub id: String,
    pub caption: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EntryInput {
    pub title: String,
    pub body_json: String,
    pub entry_date: String,
    pub entry_time: Option<String>,
    pub location: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub media: Vec<EntryMediaInput>,
    pub cover_media_id: Option<String>,
    #[serde(default)]
    pub highlight: bool,
    #[serde(default = "d_auto")]
    pub layout: String,
    #[serde(default = "d_none")]
    pub frame: String,
    #[serde(default = "d_none")]
    pub paper: String,
}

fn d_auto() -> String {
    "auto".into()
}
fn d_none() -> String {
    "none".into()
}

pub const LAYOUTS: [&str; 5] = ["auto", "polaroid", "collage", "magazine", "minimal"];
pub const FRAMES: [&str; 5] = ["none", "polaroid", "tape", "rounded", "vintage"];
pub const PAPERS: [&str; 4] = ["none", "dots", "lines", "kraft"];

fn pick(v: &str, allowed: &[&str], what: &str) -> AppResult<String> {
    let v = v.trim();
    if allowed.contains(&v) {
        Ok(v.to_string())
    } else {
        Err(AppError::invalid(format!("unknown {what} style")))
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Clean {
    title: String,
    body_json: String,
    body_text: String,
    entry_date: String,
    entry_time: Option<String>,
    location: Option<String>,
    tags: Vec<String>,
    media: Vec<(String, Option<String>)>,
    cover: Option<String>,
    highlight: bool,
    layout: String,
    frame: String,
    paper: String,
}

fn opt_trim(s: &Option<String>, max: usize, field: &str) -> AppResult<Option<String>> {
    match s.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(v) if v.chars().count() > max => Err(AppError::invalid(format!("{field} is too long"))),
        Some(v) => Ok(Some(v.to_string())),
    }
}

pub fn validate_date(d: &str) -> AppResult<String> {
    let d = d.trim();
    chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
        .map(|_| d.to_string())
        .map_err(|_| AppError::invalid("date must be YYYY-MM-DD"))
}

fn validate_time(t: &Option<String>) -> AppResult<Option<String>> {
    match t.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(v) => chrono::NaiveTime::parse_from_str(v, "%H:%M")
            .map(|_| Some(v.to_string()))
            .map_err(|_| AppError::invalid("time must be HH:MM")),
    }
}

pub fn normalize_tags(tags: &[String]) -> AppResult<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let t = t.trim().trim_start_matches('#').trim().to_lowercase();
        if t.is_empty() {
            continue;
        }
        if t.chars().count() > 40 {
            return Err(AppError::invalid("a tag is too long (40 characters max)"));
        }
        if !out.contains(&t) {
            out.push(t);
        }
    }
    if out.len() > MAX_TAGS {
        return Err(AppError::invalid("too many tags (20 max)"));
    }
    Ok(out)
}

/// Plain text from TipTap/ProseMirror JSON, for search and previews.
pub fn body_text(body_json: &str) -> AppResult<String> {
    if body_json.trim().is_empty() {
        return Ok(String::new());
    }
    if body_json.len() > MAX_BODY_BYTES {
        return Err(AppError::invalid("the text is too long"));
    }
    let v: Value = serde_json::from_str(body_json).map_err(|_| AppError::invalid("invalid text format"))?;
    if v.get("type").and_then(Value::as_str) != Some("doc") {
        return Err(AppError::invalid("invalid text format"));
    }
    fn walk(v: &Value, out: &mut String) {
        if let Some(t) = v.get("text").and_then(Value::as_str) {
            out.push_str(t);
        }
        if v.get("type").and_then(Value::as_str) == Some("hardBreak") {
            out.push('\n');
        }
        if let Some(children) = v.get("content").and_then(Value::as_array) {
            for c in children {
                walk(c, out);
            }
            let block = matches!(
                v.get("type").and_then(Value::as_str),
                Some("paragraph" | "heading" | "listItem" | "blockquote")
            );
            if block && !out.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    let mut out = String::new();
    walk(&v, &mut out);
    Ok(out.trim().to_string())
}

fn clean(conn: &Connection, i: &EntryInput) -> AppResult<Clean> {
    let title = i.title.trim().to_string();
    if title.is_empty() {
        return Err(AppError::invalid("Give this memory a title ♡"));
    }
    if title.chars().count() > 200 {
        return Err(AppError::invalid("title is too long (200 characters max)"));
    }
    let body_text = body_text(&i.body_json)?;
    let body_json = if body_text.is_empty() { String::new() } else { i.body_json.clone() };
    if i.media.len() > MAX_MEDIA {
        return Err(AppError::invalid("too many photos and videos in one memory"));
    }
    let mut media: Vec<(String, Option<String>)> = Vec::new();
    for m in &i.media {
        if media.iter().any(|(id, _)| id == &m.id) {
            continue;
        }
        media.push((m.id.clone(), opt_trim(&m.caption, 500, "caption")?));
    }
    let ids: Vec<String> = media.iter().map(|(id, _)| id.clone()).collect();
    if !media_repo::exists_all(conn, &ids)? {
        return Err(AppError::invalid("a photo or video is missing from the archive"));
    }
    let cover = match &i.cover_media_id {
        Some(c) if ids.contains(c) => Some(c.clone()),
        _ => ids.first().cloned(),
    };
    Ok(Clean {
        title,
        body_json,
        body_text,
        entry_date: validate_date(&i.entry_date)?,
        entry_time: validate_time(&i.entry_time)?,
        location: opt_trim(&i.location, 200, "location")?,
        tags: normalize_tags(&i.tags)?,
        media,
        cover,
        highlight: i.highlight,
        layout: pick(&i.layout, &LAYOUTS, "layout")?,
        frame: pick(&i.frame, &FRAMES, "frame")?,
        paper: pick(&i.paper, &PAPERS, "paper")?,
    })
}

// ---------- output ----------

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EntrySummary {
    pub id: String,
    pub title: String,
    pub excerpt: String,
    pub entry_date: String,
    pub entry_time: Option<String>,
    pub location: Option<String>,
    pub tags: Vec<String>,
    pub author_id: String,
    pub author_name: Option<String>,
    pub highlight: bool,
    pub cover: Option<MediaRef>,
    pub photo_count: i64,
    pub video_count: i64,
    pub reaction_count: i64,
    pub comment_count: i64,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryMedia {
    #[serde(flatten)]
    pub media: MediaRef,
    pub caption: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryDetail {
    #[serde(flatten)]
    pub summary: EntrySummary,
    pub body_json: String,
    pub media: Vec<EntryMedia>,
    pub cover_media_id: Option<String>,
    pub created_at: String,
    pub layout: String,
    pub frame: String,
    pub paper: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashItem {
    pub id: String,
    pub title: String,
    pub entry_date: String,
    pub deleted_at: String,
    pub cover: Option<MediaRef>,
}

#[derive(Debug, Serialize)]
pub struct TagCount {
    pub tag: String,
    pub count: i64,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EntryQuery {
    pub text: Option<String>,
    pub tag: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    #[serde(default)]
    pub oldest_first: bool,
    #[serde(default)]
    pub highlights_only: bool,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

fn excerpt(text: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= 240 {
        flat
    } else {
        format!("{}…", flat.chars().take(239).collect::<String>().trim_end())
    }
}

/// FTS5 query from free text: every word must match as a prefix. Strips FTS syntax.
pub fn fts_query(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split_whitespace()
        .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\"*"))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

const SUMMARY_SELECT: &str = "
    SELECT e.id, e.title, e.body_text, e.entry_date, e.entry_time, e.location, e.tags_json,
           e.author_id, u.display_name, e.highlight, e.updated_at,
           (SELECT COUNT(*) FROM entry_media em JOIN media m ON m.id = em.media_id WHERE em.entry_id = e.id AND m.kind = 'photo'),
           (SELECT COUNT(*) FROM entry_media em JOIN media m ON m.id = em.media_id WHERE em.entry_id = e.id AND m.kind = 'video'),
           (SELECT COUNT(*) FROM reactions x WHERE x.entry_id = e.id AND x.deleted_at IS NULL),
           (SELECT COUNT(*) FROM comments k WHERE k.entry_id = e.id AND k.deleted_at IS NULL),
           c.id, c.kind, c.mime, c.width, c.height, c.duration_ms, c.taken_at, c.has_thumb, c.preview,
           c.availability, c.original_name, c.size, c.hash, c.ext
    FROM journal_entries e
    LEFT JOIN users u ON u.id = e.author_id
    LEFT JOIN media c ON c.id = COALESCE(e.cover_media_id,
        (SELECT media_id FROM entry_media WHERE entry_id = e.id ORDER BY position LIMIT 1))";

fn summary_from_row(r: &rusqlite::Row) -> rusqlite::Result<EntrySummary> {
    let tags_json: String = r.get(6)?;
    let body_text: String = r.get(2)?;
    let cover_id: Option<String> = r.get(15)?;
    let cover = match cover_id {
        Some(_) => Some(media_repo::from_row(r, 15)?.r),
        None => None,
    };
    Ok(EntrySummary {
        id: r.get(0)?,
        title: r.get(1)?,
        excerpt: excerpt(&body_text),
        entry_date: r.get(3)?,
        entry_time: r.get(4)?,
        location: r.get(5)?,
        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        author_id: r.get(7)?,
        author_name: r.get(8)?,
        highlight: r.get::<_, i64>(9)? != 0,
        updated_at: r.get(10)?,
        photo_count: r.get(11)?,
        video_count: r.get(12)?,
        reaction_count: r.get(13)?,
        comment_count: r.get(14)?,
        cover,
    })
}

pub fn list(conn: &Connection, q: &EntryQuery) -> AppResult<Vec<EntrySummary>> {
    let mut sql = format!("{SUMMARY_SELECT} WHERE e.deleted_at IS NULL");
    let mut args: Vec<String> = Vec::new();
    if let Some(m) = q.text.as_deref().and_then(fts_query) {
        args.push(m);
        sql += &format!(" AND e.id IN (SELECT entry_id FROM journal_fts WHERE journal_fts MATCH ?{})", args.len());
    }
    if let Some(tag) = q.tag.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        args.push(tag.to_lowercase());
        sql += &format!(" AND EXISTS (SELECT 1 FROM json_each(e.tags_json) WHERE value = ?{})", args.len());
    }
    if let Some(from) = q.from.as_deref().filter(|s| !s.is_empty()) {
        args.push(validate_date(from)?);
        sql += &format!(" AND e.entry_date >= ?{}", args.len());
    }
    if let Some(to) = q.to.as_deref().filter(|s| !s.is_empty()) {
        args.push(validate_date(to)?);
        sql += &format!(" AND e.entry_date <= ?{}", args.len());
    }
    if q.highlights_only {
        sql += " AND e.highlight = 1";
    }
    let dir = if q.oldest_first { "ASC" } else { "DESC" };
    sql += &format!(
        " ORDER BY e.entry_date {dir}, COALESCE(e.entry_time, '') {dir}, e.created_at {dir} LIMIT {} OFFSET {}",
        q.limit.unwrap_or(500).clamp(1, 5000),
        q.offset.unwrap_or(0).max(0)
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(args.iter()), summary_from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<EntryDetail> {
    let summary = conn
        .query_row(&format!("{SUMMARY_SELECT} WHERE e.id = ?1 AND e.deleted_at IS NULL"), [id], summary_from_row)
        .optional()?
        .ok_or_else(|| AppError::invalid("this memory doesn't exist anymore"))?;
    let (body_json, cover_media_id, created_at, layout, frame, paper): (String, Option<String>, String, String, String, String) =
        conn.query_row(
            "SELECT body_json, cover_media_id, created_at, layout, frame, paper FROM journal_entries WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )?;
    let prefixed: String = MEDIA_COLS.split(", ").map(|c| format!("m.{c}")).collect::<Vec<_>>().join(", ");
    let mut stmt = conn.prepare(&format!(
        "SELECT {prefixed}, em.caption FROM entry_media em JOIN media m ON m.id = em.media_id
         WHERE em.entry_id = ?1 ORDER BY em.position"
    ))?;
    let media = stmt
        .query_map([id], |r| Ok(EntryMedia { media: media_repo::from_row(r, 0)?.r, caption: r.get(14)? }))?
        .collect::<Result<_, _>>()?;
    Ok(EntryDetail { summary, body_json, media, cover_media_id, created_at, layout, frame, paper })
}

fn write_media(tx: &Transaction, id: &str, media: &[(String, Option<String>)]) -> AppResult<()> {
    tx.execute("DELETE FROM entry_media WHERE entry_id = ?1", [id])?;
    for (pos, (mid, caption)) in media.iter().enumerate() {
        tx.execute(
            "INSERT INTO entry_media(entry_id, media_id, position, caption) VALUES (?1,?2,?3,?4)",
            params![id, mid, pos as i64, caption],
        )?;
    }
    Ok(())
}

fn write_fts(tx: &Transaction, id: &str, c: Option<&Clean>) -> AppResult<()> {
    tx.execute("DELETE FROM journal_fts WHERE entry_id = ?1", [id])?;
    if let Some(c) = c {
        tx.execute(
            "INSERT INTO journal_fts(entry_id, title, body, tags, location) VALUES (?1,?2,?3,?4,?5)",
            params![id, c.title, c.body_text, c.tags.join(" "), c.location.clone().unwrap_or_default()],
        )?;
    }
    Ok(())
}

fn media_json(media: &[(String, Option<String>)]) -> Value {
    Value::Array(media.iter().map(|(id, cap)| json!({ "id": id, "caption": cap })).collect())
}

fn fields_json(c: &Clean) -> Vec<(&'static str, Value)> {
    vec![
        ("title", json!(c.title)),
        ("body_json", json!(c.body_json)),
        ("entry_date", json!(c.entry_date)),
        ("entry_time", json!(c.entry_time)),
        ("location", json!(c.location)),
        ("tags", json!(c.tags)),
        ("media", media_json(&c.media)),
        ("cover_media_id", json!(c.cover)),
        ("highlight", json!(c.highlight)),
        ("layout", json!(c.layout)),
        ("frame", json!(c.frame)),
        ("paper", json!(c.paper)),
    ]
}

pub fn create(conn: &mut Connection, w: &Writer, author_id: &str, input: &EntryInput) -> AppResult<EntryDetail> {
    let c = clean(conn, input)?;
    let id = new_id();
    let now = now_iso();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO journal_entries(id, author_id, title, body_json, body_text, entry_date, entry_time,
                                     location, tags_json, cover_media_id, highlight, layout, frame, paper,
                                     created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?15)",
        params![
            id, author_id, c.title, c.body_json, c.body_text, c.entry_date, c.entry_time, c.location,
            json!(c.tags).to_string(), c.cover, c.highlight as i64, c.layout, c.frame, c.paper, now
        ],
    )?;
    write_media(&tx, &id, &c.media)?;
    write_fts(&tx, &id, Some(&c))?;
    let fields = fields_json(&c);
    let mut changes = serde_json::Map::new();
    for (k, v) in &fields {
        changes.insert(k.to_string(), v.clone());
    }
    changes.insert("author_id".into(), json!(author_id));
    changes.insert("created_at".into(), json!(now));
    let hlc = w.record(&tx, ENTITY, &id, "upsert", Value::Object(changes))?;
    let clock = stamp_fields("{}", fields.iter().map(|(k, _)| k.to_string()), &hlc);
    tx.execute("UPDATE journal_entries SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
    tx.commit()?;
    get(conn, &id)
}

fn load_clean(conn: &Connection, id: &str) -> AppResult<(Clean, String)> {
    let row = conn
        .query_row(
            "SELECT title, body_json, body_text, entry_date, entry_time, location, tags_json, cover_media_id,
                    highlight, layout, frame, paper, field_clock
             FROM journal_entries WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, i64>(8)?,
                    r.get::<_, String>(9)?,
                    r.get::<_, String>(10)?,
                    r.get::<_, String>(11)?,
                    r.get::<_, String>(12)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::invalid("this memory doesn't exist anymore"))?;
    let mut stmt = conn.prepare("SELECT media_id, caption FROM entry_media WHERE entry_id = ?1 ORDER BY position")?;
    let media = stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<Vec<_>, _>>()?;
    Ok((
        Clean {
            title: row.0,
            body_json: row.1,
            body_text: row.2,
            entry_date: row.3,
            entry_time: row.4,
            location: row.5,
            tags: serde_json::from_str(&row.6).unwrap_or_default(),
            cover: row.7,
            highlight: row.8 != 0,
            layout: row.9,
            frame: row.10,
            paper: row.11,
            media,
        },
        row.12,
    ))
}

pub fn update(conn: &mut Connection, w: &Writer, id: &str, input: &EntryInput) -> AppResult<EntryDetail> {
    let next = clean(conn, input)?;
    let (cur, clock) = load_clean(conn, id)?;
    let before = fields_json(&cur);
    let after = fields_json(&next);
    let changed: Vec<(&str, Value)> = after
        .into_iter()
        .zip(before)
        .filter(|((_, a), (_, b))| a != b)
        .map(|((k, a), _)| (k, a))
        .collect();
    if changed.is_empty() {
        return get(conn, id);
    }
    let now = now_iso();
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE journal_entries SET title=?2, body_json=?3, body_text=?4, entry_date=?5, entry_time=?6,
             location=?7, tags_json=?8, cover_media_id=?9, highlight=?10, layout=?11, frame=?12, paper=?13,
             updated_at=?14 WHERE id=?1",
        params![
            id, next.title, next.body_json, next.body_text, next.entry_date, next.entry_time, next.location,
            json!(next.tags).to_string(), next.cover, next.highlight as i64, next.layout, next.frame, next.paper, now
        ],
    )?;
    if cur.media != next.media {
        write_media(&tx, id, &next.media)?;
    }
    write_fts(&tx, id, Some(&next))?;
    let mut changes = serde_json::Map::new();
    for (k, v) in &changed {
        changes.insert(k.to_string(), v.clone());
    }
    let hlc = w.record(&tx, ENTITY, id, "upsert", Value::Object(changes))?;
    let clock = stamp_fields(&clock, changed.iter().map(|(k, _)| k.to_string()), &hlc);
    tx.execute("UPDATE journal_entries SET field_clock = ?2 WHERE id = ?1", params![id, clock])?;
    tx.commit()?;
    get(conn, id)
}

/// Moves to the recovery bin (restorable for the retention period).
pub fn delete(conn: &mut Connection, w: &Writer, id: &str) -> AppResult<()> {
    let now = now_iso();
    let tx = conn.transaction()?;
    let n = tx.execute(
        "UPDATE journal_entries SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, now],
    )?;
    if n == 0 {
        return Err(AppError::invalid("this memory doesn't exist anymore"));
    }
    write_fts(&tx, id, None)?;
    w.record(&tx, ENTITY, id, "delete", json!({ "deleted_at": now }))?;
    tx.commit()?;
    Ok(())
}

pub fn restore(conn: &mut Connection, w: &Writer, id: &str) -> AppResult<EntryDetail> {
    let now = now_iso();
    let tx = conn.transaction()?;
    let n = tx.execute(
        "UPDATE journal_entries SET deleted_at = NULL, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NOT NULL",
        params![id, now],
    )?;
    if n == 0 {
        return Err(AppError::invalid("nothing to restore"));
    }
    w.record(&tx, ENTITY, id, "restore", json!({}))?;
    tx.commit()?;
    let (c, _) = load_clean(conn, id)?;
    let tx = conn.transaction()?;
    write_fts(&tx, id, Some(&c))?;
    tx.commit()?;
    get(conn, id)
}

/// Permanently removes an entry that is already in the recovery bin.
pub fn purge(conn: &mut Connection, w: &Writer, id: &str) -> AppResult<()> {
    let tx = conn.transaction()?;
    tx.execute("UPDATE milestones SET linked_entry_id = NULL WHERE linked_entry_id = ?1", [id])?;
    tx.execute("UPDATE bucket_items SET linked_entry_id = NULL WHERE linked_entry_id = ?1", [id])?;
    tx.execute("DELETE FROM reactions WHERE entry_id = ?1", [id])?;
    tx.execute("DELETE FROM comments WHERE entry_id = ?1", [id])?;
    let n = tx.execute("DELETE FROM journal_entries WHERE id = ?1 AND deleted_at IS NOT NULL", [id])?;
    if n == 0 {
        return Err(AppError::invalid("only memories in the recovery bin can be removed forever"));
    }
    write_fts(&tx, id, None)?;
    w.record(&tx, ENTITY, id, "purge", json!({}))?;
    tx.commit()?;
    Ok(())
}

/// Purges bin items older than the retention period. Returns how many.
pub fn purge_expired(conn: &mut Connection, w: &Writer, retention_days: i64) -> AppResult<usize> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(retention_days))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let ids: Vec<String> = {
        let mut stmt = conn.prepare("SELECT id FROM journal_entries WHERE deleted_at IS NOT NULL AND deleted_at < ?1")?;
        let rows = stmt.query_map([cutoff], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    for id in &ids {
        purge(conn, w, id)?;
    }
    Ok(ids.len())
}

pub fn trash(conn: &Connection) -> AppResult<Vec<TrashItem>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT e.id, e.title, e.entry_date, e.deleted_at, {}
         FROM journal_entries e
         LEFT JOIN media c ON c.id = COALESCE(e.cover_media_id,
            (SELECT media_id FROM entry_media WHERE entry_id = e.id ORDER BY position LIMIT 1))
         WHERE e.deleted_at IS NOT NULL ORDER BY e.deleted_at DESC",
        MEDIA_COLS.split(", ").map(|c| format!("c.{c}")).collect::<Vec<_>>().join(", ")
    ))?;
    let rows = stmt.query_map([], |r| {
        let cover_id: Option<String> = r.get(4)?;
        Ok(TrashItem {
            id: r.get(0)?,
            title: r.get(1)?,
            entry_date: r.get(2)?,
            deleted_at: r.get(3)?,
            cover: if cover_id.is_some() { Some(media_repo::from_row(r, 4)?.r) } else { None },
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn tags(conn: &Connection) -> AppResult<Vec<TagCount>> {
    let mut stmt = conn.prepare(
        "SELECT t.value, COUNT(*) FROM journal_entries e, json_each(e.tags_json) t
         WHERE e.deleted_at IS NULL GROUP BY t.value ORDER BY COUNT(*) DESC, t.value",
    )?;
    let rows = stmt.query_map([], |r| Ok(TagCount { tag: r.get(0)?, count: r.get(1)? }))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Entries from this calendar day in earlier years.
pub fn on_this_day(conn: &Connection, today: &str) -> AppResult<Vec<EntrySummary>> {
    let today = validate_date(today)?;
    let mut stmt = conn.prepare(&format!(
        "{SUMMARY_SELECT} WHERE e.deleted_at IS NULL
           AND substr(e.entry_date, 6, 5) = substr(?1, 6, 5)
           AND substr(e.entry_date, 1, 4) < substr(?1, 1, 4)
         ORDER BY e.entry_date DESC LIMIT 20"
    ))?;
    let rows = stmt.query_map([today], summary_from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::db::repo::media::{insert as insert_media, NewMedia};

    pub fn setup() -> (Connection, Writer, String) {
        let conn = open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO users(id, display_name, is_self, created_at, updated_at) VALUES ('me','Jo',1,'t','t')",
            [],
        )
        .unwrap();
        let w = Writer { device_id: crate::sync::device_id(&conn).unwrap() };
        (conn, w, "me".into())
    }

    pub fn photo(conn: &Connection, hash: &str) -> String {
        insert_media(
            conn,
            &NewMedia {
                hash: hash.into(),
                kind: "photo".into(),
                mime: "image/jpeg".into(),
                ext: "jpg".into(),
                size: 10,
                width: None,
                height: None,
                duration_ms: None,
                taken_at: None,
                original_name: None,
                preview: "none".into(),
                created_by: None,
            },
        )
        .unwrap()
        .r
        .id
    }

    pub fn input(title: &str, date: &str) -> EntryInput {
        EntryInput {
            title: title.into(),
            body_json: r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Sunset at the beach with ice cream"}]}]}"#.into(),
            entry_date: date.into(),
            entry_time: None,
            location: Some("Byblos".into()),
            tags: vec!["Beach".into(), "#summer".into(), "beach".into()],
            media: vec![],
            cover_media_id: None,
            highlight: false,
            layout: "auto".into(),
            frame: "none".into(),
            paper: "none".into(),
        }
    }

    fn ops(conn: &Connection) -> Vec<(String, String)> {
        let mut s = conn.prepare("SELECT kind, changes_json FROM oplog ORDER BY hlc").unwrap();
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(Result::unwrap).collect()
    }

    #[test]
    fn create_writes_entry_media_fts_and_op_atomically() {
        let (mut conn, w, me) = setup();
        let p1 = photo(&conn, "h1");
        let p2 = photo(&conn, "h2");
        let mut i = input("Beach day", "2025-07-12");
        i.media = vec![
            EntryMediaInput { id: p1.clone(), caption: Some(" us ".into()) },
            EntryMediaInput { id: p2.clone(), caption: None },
        ];
        i.cover_media_id = Some(p2.clone());
        let e = create(&mut conn, &w, &me, &i).unwrap();
        assert_eq!(e.summary.tags, vec!["beach", "summer"]);
        assert_eq!(e.summary.excerpt, "Sunset at the beach with ice cream");
        assert_eq!(e.media.len(), 2);
        assert_eq!(e.media[0].caption.as_deref(), Some("us"));
        assert_eq!(e.summary.cover.as_ref().unwrap().id, p2);
        assert_eq!(e.summary.photo_count, 2);
        assert_eq!(e.summary.author_name.as_deref(), Some("Jo"));
        let o = ops(&conn);
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].0, "upsert");
        assert!(o[0].1.contains("\"author_id\":\"me\""));
    }

    #[test]
    fn rejects_unknown_media_and_writes_nothing() {
        let (mut conn, w, me) = setup();
        let mut i = input("x", "2025-01-01");
        i.media = vec![EntryMediaInput { id: "nope".into(), caption: None }];
        assert!(create(&mut conn, &w, &me, &i).is_err());
        assert!(list(&conn, &EntryQuery::default()).unwrap().is_empty());
        assert!(ops(&conn).is_empty());
    }

    #[test]
    fn validation() {
        let (mut conn, w, me) = setup();
        assert!(create(&mut conn, &w, &me, &input("  ", "2025-01-01")).is_err());
        assert!(create(&mut conn, &w, &me, &input("x", "2025-13-01")).is_err());
        let mut i = input("x", "2025-01-01");
        i.body_json = "{\"type\":\"script\"}".into();
        assert!(create(&mut conn, &w, &me, &i).is_err());
        let mut i = input("x", "2025-01-01");
        i.entry_time = Some("25:00".into());
        assert!(create(&mut conn, &w, &me, &i).is_err());
    }

    #[test]
    fn update_logs_only_changed_fields() {
        let (mut conn, w, me) = setup();
        let e = create(&mut conn, &w, &me, &input("Beach day", "2025-07-12")).unwrap();
        let mut i = input("Beach day ♡", "2025-07-12");
        i.highlight = true;
        update(&mut conn, &w, &e.summary.id, &i).unwrap();
        // no-op update logs nothing
        update(&mut conn, &w, &e.summary.id, &i).unwrap();
        let o = ops(&conn);
        assert_eq!(o.len(), 2);
        let ch: Value = serde_json::from_str(&o[1].1).unwrap();
        let keys: Vec<&String> = ch.as_object().unwrap().keys().collect();
        assert_eq!(keys.len(), 2);
        assert_eq!(ch["title"], "Beach day ♡");
        assert_eq!(ch["highlight"], true);
    }

    #[test]
    fn search_tag_date_filters_and_sort() {
        let (mut conn, w, me) = setup();
        create(&mut conn, &w, &me, &input("Beach day", "2025-07-12")).unwrap();
        let mut i = input("Cinema night", "2024-02-01");
        i.body_json = String::new();
        i.tags = vec!["movies".into()];
        i.location = None;
        create(&mut conn, &w, &me, &i).unwrap();

        let q = |text: &str| list(&conn, &EntryQuery { text: Some(text.into()), ..Default::default() }).unwrap().len();
        assert_eq!(q("sun"), 1); // prefix match in body
        assert_eq!(q("byblos"), 1); // location
        assert_eq!(q("cinema"), 1);
        assert_eq!(q("\"; DROP TABLE"), 0); // FTS syntax stripped, no error
        assert_eq!(q("   "), 2);

        let by_tag = list(&conn, &EntryQuery { tag: Some("Movies".into()), ..Default::default() }).unwrap();
        assert_eq!(by_tag.len(), 1);
        let ranged = list(&conn, &EntryQuery { from: Some("2025-01-01".into()), ..Default::default() }).unwrap();
        assert_eq!(ranged[0].title, "Beach day");
        let newest = list(&conn, &EntryQuery::default()).unwrap();
        let oldest = list(&conn, &EntryQuery { oldest_first: true, ..Default::default() }).unwrap();
        assert_eq!(newest[0].title, "Beach day");
        assert_eq!(oldest[0].title, "Cinema night");
        let t = tags(&conn).unwrap();
        assert_eq!(t.len(), 3);
    }

    #[test]
    fn delete_restore_purge_cycle() {
        let (mut conn, w, me) = setup();
        let e = create(&mut conn, &w, &me, &input("Beach day", "2025-07-12")).unwrap();
        let id = e.summary.id;
        assert!(purge(&mut conn, &w, &id).is_err(), "can't purge a live entry");
        delete(&mut conn, &w, &id).unwrap();
        assert!(list(&conn, &EntryQuery::default()).unwrap().is_empty());
        assert!(list(&conn, &EntryQuery { text: Some("beach".into()), ..Default::default() }).unwrap().is_empty());
        assert_eq!(trash(&conn).unwrap().len(), 1);
        restore(&mut conn, &w, &id).unwrap();
        assert_eq!(list(&conn, &EntryQuery { text: Some("beach".into()), ..Default::default() }).unwrap().len(), 1);
        delete(&mut conn, &w, &id).unwrap();
        assert_eq!(purge_expired(&mut conn, &w, 30).unwrap(), 0, "too recent to auto-purge");
        purge(&mut conn, &w, &id).unwrap();
        assert!(trash(&conn).unwrap().is_empty());
        let kinds: Vec<String> = ops(&conn).into_iter().map(|o| o.0).collect();
        assert_eq!(kinds, ["upsert", "delete", "restore", "delete", "purge"]);
    }

    #[test]
    fn on_this_day_only_past_years() {
        let (mut conn, w, me) = setup();
        create(&mut conn, &w, &me, &input("A", "2024-11-03")).unwrap();
        create(&mut conn, &w, &me, &input("B", "2026-11-03")).unwrap();
        create(&mut conn, &w, &me, &input("C", "2025-11-04")).unwrap();
        let r = on_this_day(&conn, "2026-11-03").unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "A");
    }

    #[test]
    fn body_text_extraction() {
        let j = r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Hi"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"one"}]}]}]}]}"#;
        assert_eq!(body_text(j).unwrap(), "Hi\none");
        assert_eq!(body_text("").unwrap(), "");
        assert!(body_text("not json").is_err());
    }
}
