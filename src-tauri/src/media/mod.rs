//! The permanent media archive: content-addressed originals (never modified),
//! plus disposable thumbnails and browser-friendly previews.

pub mod ffmpeg;
pub mod protocol;
pub mod worker;

use crate::db::new_id;
use crate::db::repo::media::{self as repo, MediaRef, MediaRow, NewMedia};
use crate::error::{AppError, AppResult};
use image::{DynamicImage, ImageDecoder, ImageReader};
use rusqlite::Connection;
use serde::Serialize;
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const THUMB_SIZE: u32 = 480;
const PREVIEW_SIZE: u32 = 2048;

#[derive(Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(data_dir: &Path) -> AppResult<Store> {
        let root = data_dir.join("media");
        for d in ["originals", "thumbs", "previews", "incoming"] {
            fs::create_dir_all(root.join(d))?;
        }
        fs::create_dir_all(data_dir.join("imports"))?;
        let s = Store { root };
        s.clean_imports()?;
        Ok(s)
    }
    fn imports(&self) -> PathBuf {
        self.root.parent().unwrap().join("imports")
    }
    pub fn original(&self, hash: &str, ext: &str) -> PathBuf {
        self.root.join("originals").join(&hash[0..2]).join(&hash[2..4]).join(format!("{hash}.{ext}"))
    }
    pub fn thumb(&self, id: &str) -> PathBuf {
        self.root.join("thumbs").join(format!("{id}.jpg"))
    }
    pub fn preview_image(&self, id: &str) -> PathBuf {
        self.root.join("previews").join(format!("{id}.jpg"))
    }
    /// Partial download from the partner (resumable). Never shown in the UI.
    pub fn incoming(&self, hash: &str) -> PathBuf {
        self.root.join("incoming").join(format!("{hash}.part"))
    }
    pub fn preview_video(&self, id: &str) -> PathBuf {
        self.root.join("previews").join(format!("{id}.mp4"))
    }
    /// Leftovers from an interrupted import are never part of the archive; safe to remove.
    fn clean_imports(&self) -> AppResult<()> {
        for e in fs::read_dir(self.imports())?.flatten() {
            let _ = fs::remove_file(e.path());
        }
        Ok(())
    }
}

/// Recognised from file contents (not the file name).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    Jpeg,
    Png,
    Webp,
    Gif,
    Avif,
    Heic,
    Mp4,
    M4v,
    Mov,
    Webm,
    Mkv,
    Avi,
}

impl Format {
    pub fn from_mime(mime: &str) -> Option<Format> {
        Some(match mime {
            "image/jpeg" => Format::Jpeg,
            "image/png" => Format::Png,
            "image/webp" => Format::Webp,
            "image/gif" => Format::Gif,
            "image/avif" => Format::Avif,
            "image/heif" | "image/heic" => Format::Heic,
            "video/mp4" => Format::Mp4,
            "video/x-m4v" => Format::M4v,
            "video/quicktime" => Format::Mov,
            "video/webm" => Format::Webm,
            "video/x-matroska" => Format::Mkv,
            "video/x-msvideo" => Format::Avi,
            _ => return None,
        })
    }
    pub fn ext(self) -> &'static str {
        match self {
            Format::Jpeg => "jpg",
            Format::Png => "png",
            Format::Webp => "webp",
            Format::Gif => "gif",
            Format::Avif => "avif",
            Format::Heic => "heic",
            Format::Mp4 => "mp4",
            Format::M4v => "m4v",
            Format::Mov => "mov",
            Format::Webm => "webm",
            Format::Mkv => "mkv",
            Format::Avi => "avi",
        }
    }
    pub fn mime(self) -> &'static str {
        match self {
            Format::Jpeg => "image/jpeg",
            Format::Png => "image/png",
            Format::Webp => "image/webp",
            Format::Gif => "image/gif",
            Format::Avif => "image/avif",
            Format::Heic => "image/heic",
            Format::Mp4 => "video/mp4",
            Format::M4v => "video/mp4",
            Format::Mov => "video/quicktime",
            Format::Webm => "video/webm",
            Format::Mkv => "video/x-matroska",
            Format::Avi => "video/x-msvideo",
        }
    }
    pub fn is_video(self) -> bool {
        matches!(self, Format::Mp4 | Format::M4v | Format::Mov | Format::Webm | Format::Mkv | Format::Avi)
    }
    /// Formats the `image` crate decodes (for thumbnails without ffmpeg).
    fn decodable(self) -> bool {
        matches!(self, Format::Jpeg | Format::Png | Format::Webp | Format::Gif)
    }
}

pub fn sniff(head: &[u8]) -> Option<Format> {
    let t = infer::get(head)?;
    Format::from_mime(t.mime_type())
}

struct Staged {
    part: PathBuf,
    hash: String,
    size: i64,
    format: Format,
}

/// Copies the source into the archive's staging area while hashing it.
fn stage(store: &Store, src: &Path) -> AppResult<Staged> {
    let meta = fs::metadata(src)?;
    if !meta.is_file() {
        return Err(AppError::invalid("not a file"));
    }
    let mut input = BufReader::with_capacity(1 << 20, File::open(src)?);
    let mut head = vec![0u8; 8192];
    let n = read_full(&mut input, &mut head)?;
    head.truncate(n);
    let format = sniff(&head).ok_or_else(|| AppError::invalid("not a photo or video Dear Us can keep"))?;

    let part = store.imports().join(format!("{}.part", new_id()));
    let mut out = File::create(&part)?;
    let mut hasher = blake3::Hasher::new();
    let mut size = head.len() as u64;
    hasher.update(&head);
    out.write_all(&head)?;
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n])?;
        size += n as u64;
    }
    out.sync_all()?;
    Ok(Staged { part, hash: hasher.finalize().to_hex().to_string(), size: size as i64, format })
}

fn read_full(r: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..])? {
            0 => break,
            k => n += k,
        }
    }
    Ok(n)
}

/// Moves a staged file to its permanent content-addressed home (or drops it if identical exists).
fn commit_file(store: &Store, s: &Staged) -> AppResult<PathBuf> {
    let dest = store.original(&s.hash, s.format.ext());
    if dest.exists() {
        fs::remove_file(&s.part)?;
    } else {
        fs::create_dir_all(dest.parent().unwrap())?;
        fs::rename(&s.part, &dest)?;
    }
    Ok(dest)
}

fn exif_taken_at(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let exif = exif::Reader::new().read_from_container(&mut BufReader::new(file)).ok()?;
    let f = exif
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .or_else(|| exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY))?;
    let raw = match &f.value {
        exif::Value::Ascii(v) => String::from_utf8_lossy(v.first()?).to_string(),
        _ => return None,
    };
    let dt = chrono::NaiveDateTime::parse_from_str(raw.trim(), "%Y:%m:%d %H:%M:%S").ok()?;
    Some(dt.format("%Y-%m-%dT%H:%M:%S").to_string())
}

/// Decodes with EXIF orientation applied.
fn decode_oriented(path: &Path) -> Option<DynamicImage> {
    let reader = ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    let mut decoder = reader.into_decoder().ok()?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).ok()?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    Some(img)
}

fn save_jpeg(img: &DynamicImage, max: u32, out: &Path) -> bool {
    let small = if img.width() > max || img.height() > max { img.thumbnail(max, max) } else { img.clone() };
    let rgb = DynamicImage::ImageRgb8(small.to_rgb8());
    let tmp = out.with_extension("tmp");
    let ok = File::create(&tmp)
        .ok()
        .and_then(|f| {
            let mut w = std::io::BufWriter::new(f);
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut w, 84).encode_image(&rgb).ok()?;
            w.flush().ok()
        })
        .is_some();
    ok && fs::rename(&tmp, out).is_ok()
}

pub struct Derived {
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    pub has_thumb: bool,
    pub preview: &'static str,
}

/// Thumbnails/previews for a file that arrived from the partner.
pub fn derive_for(store: &Store, id: &str, original: &Path, mime: &str) -> Derived {
    match Format::from_mime(mime) {
        Some(f) => derive(store, id, original, f),
        None => Derived { width: None, height: None, duration_ms: None, has_thumb: false, preview: "failed" },
    }
}

/// Builds thumbnails/previews. Never touches the original. Video transcodes are deferred
/// to the background worker (preview = "needed").
fn derive(store: &Store, id: &str, original: &Path, f: Format) -> Derived {
    let thumb = store.thumb(id);
    let mut d = Derived { width: None, height: None, duration_ms: None, has_thumb: false, preview: "none" };
    if f.decodable() {
        if let Some(img) = decode_oriented(original) {
            d.width = Some(img.width() as i64);
            d.height = Some(img.height() as i64);
            d.has_thumb = save_jpeg(&img, THUMB_SIZE, &thumb);
            if (img.width() > 2600 || img.height() > 2600) && save_jpeg(&img, PREVIEW_SIZE, &store.preview_image(id)) {
                d.preview = "image";
            }
        }
    } else if f == Format::Heic || f == Format::Avif {
        // AVIF displays natively; HEIC needs a converted copy.
        let preview = store.preview_image(id);
        if ffmpeg::still_jpeg(original, &preview, None) {
            if let Some(img) = decode_oriented(&preview) {
                d.width = Some(img.width() as i64);
                d.height = Some(img.height() as i64);
                d.has_thumb = save_jpeg(&img, THUMB_SIZE, &thumb);
            }
            d.preview = if f == Format::Heic { "image" } else { "none" };
            if f == Format::Avif {
                let _ = fs::remove_file(&preview);
            }
        } else if f == Format::Heic {
            d.preview = "failed";
        }
    } else if f.is_video() {
        let probe = ffmpeg::probe(original);
        if let Some(p) = &probe {
            d.width = p.width;
            d.height = p.height;
            d.duration_ms = p.duration_ms;
        }
        let seek = d.duration_ms.map(|ms| if ms > 2000 { 1.0 } else { 0.0 });
        let poster = store.imports().join(format!("{id}-poster.jpg"));
        if ffmpeg::still_jpeg(original, &poster, seek) {
            if let Some(img) = decode_oriented(&poster) {
                d.has_thumb = save_jpeg(&img, THUMB_SIZE, &thumb);
            }
            let _ = fs::remove_file(&poster);
        }
        let codec_ok = probe
            .as_ref()
            .and_then(|p| p.video_codec.as_deref())
            .map(|c| matches!(c, "h264" | "vp8" | "vp9" | "av1"))
            .unwrap_or(false);
        let container_ok = matches!(f, Format::Mp4 | Format::M4v | Format::Webm);
        d.preview = if container_ok && (codec_ok || probe.is_none()) {
            "none" // plays as-is (or we can't tell without ffmpeg — try the original)
        } else if ffmpeg::tools().is_some() {
            "needed"
        } else {
            "failed"
        };
    }
    d
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub path: String,
    pub media: Option<MediaRef>,
    pub error: Option<String>,
    /// true when this exact file was already in the archive (stored once).
    pub duplicate: bool,
}

/// Imports one file. Heavy work happens without holding the DB lock.
pub fn import_one(store: &Store, db: &Mutex<Connection>, src: &Path, created_by: Option<String>) -> AppResult<(MediaRow, bool)> {
    let staged = stage(store, src)?;
    if let Some(existing) = repo::find_by_hash(&db.lock().unwrap(), &staged.hash)? {
        let _ = fs::remove_file(&staged.part);
        return Ok((existing, true));
    }
    let original = commit_file(store, &staged)?;
    let f = staged.format;
    let row = repo::insert(
        &db.lock().unwrap(),
        &NewMedia {
            hash: staged.hash.clone(),
            kind: if f.is_video() { "video" } else { "photo" }.into(),
            mime: f.mime().into(),
            ext: f.ext().into(),
            size: staged.size,
            width: None,
            height: None,
            duration_ms: None,
            taken_at: if f.is_video() { None } else { exif_taken_at(&original) },
            original_name: src.file_name().map(|n| n.to_string_lossy().chars().take(255).collect()),
            preview: "none".into(),
            created_by,
        },
    )?;
    let d = derive(store, &row.r.id, &original, f);
    let conn = db.lock().unwrap();
    repo::set_dimensions(&conn, &row.r.id, d.width, d.height, d.duration_ms)?;
    repo::set_thumb(&conn, &row.r.id, d.has_thumb)?;
    repo::set_preview(&conn, &row.r.id, d.preview)?;
    Ok((repo::get(&conn, &row.r.id)?.expect("exists"), false))
}

/// Removes media nothing references (after the grace period). Returns count removed.
pub fn collect_garbage(store: &Store, conn: &Connection, grace_hours: i64) -> AppResult<usize> {
    let before = (chrono::Utc::now() - chrono::Duration::hours(grace_hours)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let rows = repo::unreferenced(conn, &before)?;
    for m in &rows {
        repo::delete_row(conn, &m.r.id)?;
        let _ = fs::remove_file(store.thumb(&m.r.id));
        let _ = fs::remove_file(store.preview_image(&m.r.id));
        let _ = fs::remove_file(store.preview_video(&m.r.id));
        let _ = fs::remove_file(store.original(&m.hash, &m.ext));
    }
    Ok(rows.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_store() -> (tempfile::TempDir, Store, Mutex<Connection>) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path()).unwrap();
        let conn = crate::db::open(&dir.path().join("t.db")).unwrap();
        (dir, store, Mutex::new(conn))
    }

    fn write_png(path: &Path, w: u32, h: u32) {
        let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 255) as u8, (y % 255) as u8, 120]));
        img.save(path).unwrap();
    }

    #[test]
    fn import_keeps_original_byte_identical_and_dedupes() {
        let (dir, store, db) = tmp_store();
        let src = dir.path().join("photo.png");
        write_png(&src, 900, 600);
        let (a, dup_a) = import_one(&store, &db, &src, None).unwrap();
        let copy = dir.path().join("same-photo-renamed.png");
        fs::copy(&src, &copy).unwrap();
        let (b, dup_b) = import_one(&store, &db, &copy, None).unwrap();
        assert!(!dup_a && dup_b);
        assert_eq!(a.r.id, b.r.id, "identical file stored once");
        let stored = store.original(&a.hash, &a.ext);
        assert_eq!(fs::read(&stored).unwrap(), fs::read(&src).unwrap());
        assert_eq!(blake3::hash(&fs::read(&stored).unwrap()).to_hex().to_string(), a.hash);
        assert!(a.r.has_thumb && store.thumb(&a.r.id).exists());
        assert_eq!((a.r.width, a.r.height), (Some(900), Some(600)));
        assert_eq!(a.r.preview, "none");
        assert_eq!(fs::read_dir(store.imports()).unwrap().count(), 0, "staging area is clean");
    }

    #[test]
    fn large_images_get_a_preview() {
        let (dir, store, db) = tmp_store();
        let src = dir.path().join("big.png");
        write_png(&src, 3000, 1000);
        let (m, _) = import_one(&store, &db, &src, None).unwrap();
        assert_eq!(m.r.preview, "image");
        let p = image::open(store.preview_image(&m.r.id)).unwrap();
        assert_eq!(p.width(), PREVIEW_SIZE);
    }

    #[test]
    fn rejects_non_media_by_content_not_name() {
        let (dir, store, db) = tmp_store();
        let fake = dir.path().join("evil.jpg");
        fs::write(&fake, b"MZ this is not a photo at all").unwrap();
        assert!(import_one(&store, &db, &fake, None).is_err());
        assert_eq!(fs::read_dir(store.imports()).unwrap().count(), 0);
    }

    #[test]
    fn garbage_collection_spares_referenced_media() {
        let (dir, store, db) = tmp_store();
        let a = dir.path().join("a.png");
        let b = dir.path().join("b.png");
        write_png(&a, 50, 50);
        write_png(&b, 60, 60);
        let (ma, _) = import_one(&store, &db, &a, None).unwrap();
        let (mb, _) = import_one(&store, &db, &b, None).unwrap();
        let conn = db.lock().unwrap();
        conn.execute("INSERT INTO users(id, display_name, is_self, created_at, updated_at) VALUES ('me','Jo',1,'t','t')", []).unwrap();
        conn.execute(
            "INSERT INTO journal_entries(id, author_id, title, entry_date, created_at, updated_at, cover_media_id)
             VALUES ('e','me','x','2025-01-01','t','t',?1)",
            [&ma.r.id],
        )
        .unwrap();
        assert_eq!(collect_garbage(&store, &conn, -1).unwrap(), 1);
        assert!(store.original(&ma.hash, &ma.ext).exists());
        assert!(!store.original(&mb.hash, &mb.ext).exists());
    }

    #[test]
    fn heic_and_hevc_via_ffmpeg_when_available() {
        if ffmpeg::tools().is_none() {
            eprintln!("ffmpeg not found — skipping");
            return;
        }
        let (dir, store, db) = tmp_store();
        let t = ffmpeg::tools().unwrap();
        // HEVC .mov like an iPhone video
        let mov = dir.path().join("iphone.mov");
        let ok = std::process::Command::new(&t.ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc=duration=2:size=640x360:rate=24",
                   "-c:v", "libx265", "-tag:v", "hvc1", "-pix_fmt", "yuv420p"])
            .arg(&mov)
            .status()
            .unwrap()
            .success();
        assert!(ok, "ffmpeg could not create the HEVC sample");
        let (m, _) = import_one(&store, &db, &mov, None).unwrap();
        assert_eq!(m.r.kind, "video");
        assert_eq!(m.r.preview, "needed");
        assert!(m.r.has_thumb);
        assert_eq!((m.r.width, m.r.height), (Some(640), Some(360)));
        assert!(ffmpeg::video_proxy(&store.original(&m.hash, &m.ext), &store.preview_video(&m.r.id)));
        let p = ffmpeg::probe(&store.preview_video(&m.r.id)).unwrap();
        assert_eq!(p.video_codec.as_deref(), Some("h264"));

        // HEIC photo (if this ffmpeg build can write HEIF)
        let heic = dir.path().join("photo.heic");
        let made = std::process::Command::new(&t.ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc=size=800x600", "-frames:v", "1",
                   "-c:v", "libx265", "-f", "heif"])
            .arg(&heic)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if made {
            let (h, _) = import_one(&store, &db, &heic, None).unwrap();
            assert_eq!(h.r.kind, "photo");
            assert_eq!(h.r.preview, "image");
            assert!(h.r.has_thumb);
        } else {
            eprintln!("this ffmpeg can't write HEIF — HEIC import covered by manual test");
        }
    }
}
