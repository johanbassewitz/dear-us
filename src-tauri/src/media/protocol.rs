//! `media://` protocol (http://media.localhost/<id>/<variant> on Windows).
//! Serves archive files to the UI by id — never by raw path — with HTTP range support
//! so videos can seek.
//!
//! Variants: thumb | display (best viewable: converted preview, else original) | original

use crate::db::repo::media as repo;
use crate::AppState;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{AppHandle, Manager};

const CHUNK: u64 = 8 * 1024 * 1024;
const WHOLE_FILE_LIMIT: u64 = 32 * 1024 * 1024;

fn status(code: StatusCode) -> Response<Vec<u8>> {
    Response::builder().status(code).body(Vec::new()).unwrap()
}

/// The UI builds URLs with `convertFileSrc`, which encodes "/" as %2F. Only that escape
/// is accepted; everything else is validated strictly below.
pub fn split_path(path: &str) -> Option<(String, String)> {
    let p = path.replace("%2F", "/").replace("%2f", "/");
    let mut parts = p.trim_matches('/').split('/');
    let (id, variant) = (parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    Some((id.to_string(), variant.to_string()))
}

pub fn handle(app: &AppHandle, req: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let Some((id, variant)) = split_path(req.uri().path()) else {
        return status(StatusCode::BAD_REQUEST);
    };
    let (id, variant) = (id.as_str(), variant.as_str());
    // ids are UUIDs: refuse anything else outright
    if id.len() != 36 || !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return status(StatusCode::BAD_REQUEST);
    }
    let state = app.state::<AppState>();
    let row = match repo::get(&state.db.lock().unwrap(), id) {
        Ok(Some(r)) => r,
        Ok(None) => return status(StatusCode::NOT_FOUND),
        Err(_) => return status(StatusCode::INTERNAL_SERVER_ERROR),
    };
    let store = &state.store;
    let original = || (store.original(&row.hash, &row.ext), row.r.mime.clone());
    let (path, mime, immutable): (PathBuf, String, bool) = match variant {
        "thumb" if row.r.has_thumb => (store.thumb(id), "image/jpeg".into(), false),
        "original" => {
            let (p, m) = original();
            (p, m, true)
        }
        "display" => match row.r.preview.as_str() {
            "image" => (store.preview_image(id), "image/jpeg".into(), false),
            "video" => (store.preview_video(id), "video/mp4".into(), false),
            _ => {
                let (p, m) = original();
                (p, m, false)
            }
        },
        _ => return status(StatusCode::NOT_FOUND),
    };
    let range = req.headers().get(header::RANGE).and_then(|v| v.to_str().ok()).map(String::from);
    serve(&path, &mime, range.as_deref(), immutable).unwrap_or_else(|_| status(StatusCode::NOT_FOUND))
}

/// Parses a single `bytes=` range. Returns (start, end inclusive) or None if unsatisfiable.
pub fn parse_range(h: &str, len: u64) -> Option<(u64, u64)> {
    let spec = h.strip_prefix("bytes=")?.split(',').next()?.trim();
    let (a, b) = spec.split_once('-')?;
    if len == 0 {
        return None;
    }
    let (start, end) = if a.is_empty() {
        let suffix: u64 = b.parse().ok()?;
        (len.saturating_sub(suffix), len - 1)
    } else {
        let start: u64 = a.parse().ok()?;
        let end = if b.is_empty() { len - 1 } else { b.parse::<u64>().ok()?.min(len - 1) };
        (start, end)
    };
    (start <= end && start < len).then_some((start, end))
}

fn serve(path: &PathBuf, mime: &str, range: Option<&str>, immutable: bool) -> std::io::Result<Response<Vec<u8>>> {
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    let cache = if immutable { "private, max-age=31536000, immutable" } else { "no-cache" };
    let base = Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, cache)
        .header("X-Content-Type-Options", "nosniff");

    let wanted = match range {
        Some(h) => match parse_range(h, len) {
            Some(r) => Some(r),
            None => {
                return Ok(Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(header::CONTENT_RANGE, format!("bytes */{len}"))
                    .body(Vec::new())
                    .unwrap())
            }
        },
        None if len > WHOLE_FILE_LIMIT => Some((0, len - 1)),
        None => None,
    };

    match wanted {
        None => {
            let mut body = Vec::with_capacity(len as usize);
            f.read_to_end(&mut body)?;
            Ok(base.status(StatusCode::OK).header(header::CONTENT_LENGTH, len).body(body).unwrap())
        }
        Some((start, end)) => {
            let end = end.min(start + CHUNK - 1);
            let n = end - start + 1;
            f.seek(SeekFrom::Start(start))?;
            let mut body = vec![0u8; n as usize];
            f.read_exact(&mut body)?;
            Ok(base
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"))
                .header(header::CONTENT_LENGTH, n)
                .body(body)
                .unwrap())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        let id = "01a1a368-f71d-74a1-82a2-758ad4800fe3";
        assert_eq!(split_path(&format!("/{id}%2Fthumb")), Some((id.into(), "thumb".into())));
        assert_eq!(split_path(&format!("/{id}/display")), Some((id.into(), "display".into())));
        assert_eq!(split_path("/a/b/c"), None);
        assert_eq!(split_path("/only"), None);
    }

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-", 100), Some((0, 99)));
        assert_eq!(parse_range("bytes=10-19", 100), Some((10, 19)));
        assert_eq!(parse_range("bytes=90-500", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=-10", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=100-", 100), None);
        assert_eq!(parse_range("bytes=20-10", 100), None);
        assert_eq!(parse_range("items=0-1", 100), None);
    }
}
