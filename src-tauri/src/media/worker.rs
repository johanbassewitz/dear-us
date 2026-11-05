//! Background video conversion (HEVC/.mov → H.264 MP4 preview). One at a time, so the PC
//! stays responsive. Interrupted jobs resume on next launch (state stays "needed").

use super::ffmpeg;
use crate::db::repo::media as repo;
use crate::AppState;
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

pub struct PreviewQueue {
    tx: Mutex<Sender<String>>,
}

impl PreviewQueue {
    pub fn push(&self, media_id: String) {
        let _ = self.tx.lock().unwrap().send(media_id);
    }
}

pub fn start(app: AppHandle) -> PreviewQueue {
    let (tx, rx) = channel::<String>();
    std::thread::Builder::new()
        .name("video-previews".into())
        .spawn(move || {
            for id in rx {
                let state = app.state::<AppState>();
                let row = match repo::get(&state.db.lock().unwrap(), &id) {
                    Ok(Some(r)) if r.r.preview == "needed" => r,
                    _ => continue,
                };
                let original = state.store.original(&row.hash, &row.ext);
                let out = state.store.preview_video(&id);
                let tmp = out.with_file_name(format!("{id}.converting.mp4"));
                let ok = ffmpeg::video_proxy(&original, &tmp) && std::fs::rename(&tmp, &out).is_ok();
                if !ok {
                    let _ = std::fs::remove_file(&tmp);
                }
                let _ = repo::set_preview(&state.db.lock().unwrap(), &id, if ok { "video" } else { "failed" });
                let _ = app.emit("media-updated", &id);
            }
        })
        .expect("could not start video worker");
    PreviewQueue { tx: Mutex::new(tx) }
}
