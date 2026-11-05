//! Optional ffmpeg integration for formats the built-in viewer can't show
//! (iPhone HEIC photos, HEVC/.mov videos) and for video thumbnails.
//! Lookup order: DEARUS_FFMPEG env → ffmpeg.exe next to the app → <data>/tools (the
//! one-click download from Settings) → PATH.
//! Without ffmpeg, originals are still stored safely; they just can't be previewed.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

fn command(p: &Path) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(p);
    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW);
    c.stdin(Stdio::null());
    c
}

fn works(p: &Path) -> bool {
    command(p).arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

fn find() -> Option<Tools> {
    let exe = if cfg!(windows) { ".exe" } else { "" };
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(p) = std::env::var_os("DEARUS_FFMPEG") {
        candidates.push(PathBuf::from(p));
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        candidates.push(dir.join(format!("ffmpeg{exe}")));
    }
    if let Some(dir) = TOOLS_DIR.get() {
        candidates.push(dir.join(format!("ffmpeg{exe}")));
    }
    candidates.push(PathBuf::from(format!("ffmpeg{exe}")));
    for ff in candidates {
        if works(&ff) {
            let probe = match ff.parent().filter(|p| !p.as_os_str().is_empty()) {
                Some(dir) => dir.join(format!("ffprobe{exe}")),
                None => PathBuf::from(format!("ffprobe{exe}")),
            };
            if works(&probe) {
                return Some(Tools { ffmpeg: ff, ffprobe: probe });
            }
        }
    }
    None
}

static TOOLS_DIR: OnceLock<PathBuf> = OnceLock::new();
static FOUND: Mutex<Option<Option<Arc<Tools>>>> = Mutex::new(None);

/// Where the one-click download puts ffmpeg (`<data>/tools`). Set once at startup.
pub fn set_tools_dir(dir: PathBuf) {
    let _ = TOOLS_DIR.set(dir);
}

pub fn tools() -> Option<Arc<Tools>> {
    let mut g = FOUND.lock().unwrap();
    g.get_or_insert_with(|| find().map(Arc::new)).clone()
}

/// Forget the cached lookup (after installing ffmpeg).
fn rescan() -> bool {
    *FOUND.lock().unwrap() = None;
    tools().is_some()
}

/// Pinned build: Gyan's ffmpeg 9.0.2 "essentials" (GPL), checked against this SHA-256.
const FFMPEG_URL: &str = "https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-essentials_build.zip";
const FFMPEG_SHA256: &str = "60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba";

/// Downloads the pinned ffmpeg build (~110 MB), verifies it and keeps only ffmpeg.exe and
/// ffprobe.exe in `<data>/tools`. `progress` gets (bytes so far, total if known).
pub fn install(progress: impl Fn(u64, Option<u64>)) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};
    let dir = TOOLS_DIR.get().ok_or("tools folder not set")?.clone();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let zip_path = dir.join("ffmpeg-download.zip");
    let resp = ureq::get(FFMPEG_URL)
        .timeout(std::time::Duration::from_secs(30 * 60))
        .call()
        .map_err(|e| format!("couldn't download the converter: {e}"))?;
    let total = resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok());
    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(&zip_path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    let mut last = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("download interrupted: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        if done - last >= 2 * 1024 * 1024 {
            last = done;
            progress(done, total);
        }
    }
    drop(file);
    let result = (|| {
        if hex::encode(hasher.finalize()) != FFMPEG_SHA256 {
            return Err("the download was damaged (checksum mismatch). Please try again.".to_string());
        }
        let f = std::fs::File::open(&zip_path).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
        let exe = if cfg!(windows) { ".exe" } else { "" };
        for name in ["ffmpeg", "ffprobe"] {
            let wanted = format!("/bin/{name}{exe}");
            let idx = (0..zip.len())
                .find(|&i| zip.by_index(i).map(|e| e.name().ends_with(&wanted)).unwrap_or(false))
                .ok_or_else(|| format!("{name} missing from the download"))?;
            let mut entry = zip.by_index(idx).map_err(|e| e.to_string())?;
            let tmp = dir.join(format!("{name}{exe}.part"));
            let mut out = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
            drop(out);
            std::fs::rename(&tmp, dir.join(format!("{name}{exe}"))).map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    let _ = std::fs::remove_file(&zip_path);
    result?;
    if rescan() {
        Ok(())
    } else {
        Err("the converter was downloaded but won't start on this PC".into())
    }
}

#[derive(Debug, Default)]
pub struct Probe {
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    pub video_codec: Option<String>,
    pub format: Option<String>,
}

pub fn probe(input: &Path) -> Option<Probe> {
    let t = tools()?;
    let out = command(&t.ffprobe)
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"])
        .arg(input)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let mut p = Probe::default();
    if let Some(s) = v["streams"].as_array().and_then(|a| a.iter().find(|s| s["codec_type"] == "video")) {
        p.width = s["width"].as_i64();
        p.height = s["height"].as_i64();
        p.video_codec = s["codec_name"].as_str().map(String::from);
        // Phone videos are often stored sideways with a rotation flag.
        let rotated = s["side_data_list"]
            .as_array()
            .and_then(|l| l.iter().find_map(|d| d["rotation"].as_i64()))
            .map(|r| r.abs() % 180 == 90)
            .unwrap_or(false);
        if rotated {
            std::mem::swap(&mut p.width, &mut p.height);
        }
    }
    p.duration_ms = v["format"]["duration"].as_str().and_then(|d| d.parse::<f64>().ok()).map(|d| (d * 1000.0) as i64);
    p.format = v["format"]["format_name"].as_str().map(String::from);
    Some(p)
}

fn run(args: Vec<OsString>) -> bool {
    let Some(t) = tools() else { return false };
    command(&t.ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn os(items: &[&str]) -> Vec<OsString> {
    items.iter().map(OsString::from).collect()
}

/// One still frame (photo conversion or video poster) as JPEG, max 2048px.
pub fn still_jpeg(input: &Path, out: &Path, seek_secs: Option<f64>) -> bool {
    let mut args: Vec<OsString> = Vec::new();
    if let Some(s) = seek_secs {
        args.extend(os(&["-ss", &format!("{s:.2}")]));
    }
    args.push("-i".into());
    args.push(input.into());
    args.extend(os(&[
        "-frames:v",
        "1",
        "-vf",
        "scale='min(2048,iw)':'min(2048,ih)':force_original_aspect_ratio=decrease",
        "-q:v",
        "3",
    ]));
    args.push(out.into());
    run(args) && out.exists()
}

/// H.264/AAC MP4 copy that WebView2 can always play. Original is untouched.
pub fn video_proxy(input: &Path, out: &Path) -> bool {
    let mut args: Vec<OsString> = vec!["-i".into(), input.into()];
    args.extend(os(&[
        "-map", "0:v:0", "-map", "0:a:0?", "-c:v", "libx264", "-preset", "veryfast", "-crf", "22",
        "-vf", "scale='min(1920,iw)':-2,format=yuv420p", "-c:a", "aac", "-b:a", "160k",
        "-movflags", "+faststart",
    ]));
    args.push(out.into());
    run(args) && out.exists()
}

#[cfg(test)]
mod install_tests {
    /// Real network download (~110 MB). Run with: cargo test install_downloads -- --ignored
    #[test]
    #[ignore]
    fn install_downloads_and_finds_ffmpeg() {
        let dir = tempfile::tempdir().unwrap();
        super::set_tools_dir(dir.path().join("tools"));
        std::env::remove_var("DEARUS_FFMPEG");
        super::install(|_, _| {}).expect("install");
        assert!(dir.path().join("tools").join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" }).exists());
        assert!(!dir.path().join("tools").join("ffmpeg-download.zip").exists());
        assert!(super::tools().is_some());
    }
}
