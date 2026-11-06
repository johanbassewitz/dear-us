//! Where encrypted media chunks wait: Cloudflare R2 (production) or this server's disk (dev).
//! The apps upload/download chunks directly with short-lived signed links, so file bytes don't
//! pass through this server when R2 is used.

use crate::auth::{mac, mac_ok};
use crate::config::Config;
use crate::error::{ApiError, ApiResult};
use crate::AppState;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const LINK_TTL: Duration = Duration::from_secs(15 * 60);
pub const MAX_CHUNK_BYTES: usize = 17 * 1024 * 1024; // 16 MiB chunk + encryption overhead

pub enum Storage {
    R2 { bucket: Bucket, creds: Credentials },
    Local { dir: PathBuf, public_url: String, secret: Vec<u8> },
}

pub fn object_key(space: &uuid::Uuid, hash: &str, n: i64) -> String {
    format!("{space}/{hash}/{n}")
}

impl Storage {
    pub fn from_config(cfg: &Config) -> Result<Storage, String> {
        match &cfg.r2 {
            Some(r) => {
                let url = r.endpoint.parse().map_err(|e| format!("R2_ENDPOINT: {e}"))?;
                let bucket = Bucket::new(url, UrlStyle::Path, r.bucket.clone(), "auto").map_err(|e| format!("R2 bucket: {e}"))?;
                Ok(Storage::R2 { bucket, creds: Credentials::new(r.access_key.clone(), r.secret_key.clone()) })
            }
            None => {
                std::fs::create_dir_all(&cfg.local_storage_dir).map_err(|e| format!("LOCAL_STORAGE_DIR: {e}"))?;
                Ok(Storage::Local { dir: cfg.local_storage_dir.clone().into(), public_url: cfg.public_url.clone(), secret: cfg.secret.clone() })
            }
        }
    }

    fn local_link(public_url: &str, secret: &[u8], key: &str, method: &str) -> String {
        let exp = chrono::Utc::now().timestamp() + LINK_TTL.as_secs() as i64;
        let sig = mac(secret, &format!("{method}|{key}|{exp}"));
        format!("{public_url}/v1/local-chunks/{key}?exp={exp}&sig={sig}")
    }

    pub fn upload_url(&self, key: &str) -> String {
        match self {
            Storage::R2 { bucket, creds } => bucket.put_object(Some(creds), key).sign(LINK_TTL).to_string(),
            Storage::Local { public_url, secret, .. } => Self::local_link(public_url, secret, key, "PUT"),
        }
    }

    pub fn download_url(&self, key: &str) -> String {
        match self {
            Storage::R2 { bucket, creds } => bucket.get_object(Some(creds), key).sign(LINK_TTL).to_string(),
            Storage::Local { public_url, secret, .. } => Self::local_link(public_url, secret, key, "GET"),
        }
    }

    pub async fn delete(&self, http: &reqwest::Client, key: &str) -> ApiResult<()> {
        match self {
            Storage::R2 { bucket, creds } => {
                let url = bucket.delete_object(Some(creds), key).sign(Duration::from_secs(60));
                let r = http.delete(url).send().await.map_err(ApiError::internal)?;
                if !(r.status().is_success() || r.status() == StatusCode::NOT_FOUND) {
                    return Err(ApiError::internal(format!("R2 delete failed: {}", r.status())));
                }
                Ok(())
            }
            Storage::Local { dir, .. } => {
                match tokio::fs::remove_file(local_path(dir, key)?).await {
                    Ok(()) => Ok(()),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(e) => Err(ApiError::internal(e)),
                }
            }
        }
    }
}

fn local_path(dir: &std::path::Path, key: &str) -> ApiResult<PathBuf> {
    // key = <uuid>/<64 hex>/<n>: nothing else is accepted, so no path tricks
    let parts: Vec<&str> = key.split('/').collect();
    let ok = parts.len() == 3
        && uuid::Uuid::parse_str(parts[0]).is_ok()
        && parts[1].len() == 64
        && parts[1].bytes().all(|b| b.is_ascii_hexdigit())
        && parts[2].parse::<u32>().is_ok();
    if !ok {
        return Err(ApiError::bad("bad chunk name"));
    }
    Ok(dir.join(parts[0]).join(parts[1]).join(parts[2]))
}

#[derive(Deserialize)]
pub struct LinkQuery {
    exp: i64,
    sig: String,
}

fn check_link(state: &AppState, method: &str, key: &str, q: &LinkQuery) -> ApiResult<PathBuf> {
    let Storage::Local { dir, secret, .. } = &state.storage else { return Err(ApiError::not_found("chunk")) };
    if q.exp < chrono::Utc::now().timestamp() || !mac_ok(secret, &format!("{method}|{key}|{}", q.exp), &q.sig) {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "link_expired", "This link expired."));
    }
    local_path(dir, key)
}

pub async fn local_put(State(state): State<Arc<AppState>>, Path(key): Path<String>, Query(q): Query<LinkQuery>, body: Bytes) -> ApiResult<StatusCode> {
    let path = check_link(&state, "PUT", &key, &q)?;
    if body.len() > MAX_CHUNK_BYTES {
        return Err(ApiError::bad("chunk too big"));
    }
    tokio::fs::create_dir_all(path.parent().unwrap()).await.map_err(ApiError::internal)?;
    let tmp = path.with_extension("part");
    tokio::fs::write(&tmp, &body).await.map_err(ApiError::internal)?;
    tokio::fs::rename(&tmp, &path).await.map_err(ApiError::internal)?;
    Ok(StatusCode::OK)
}

pub async fn local_get(State(state): State<Arc<AppState>>, Path(key): Path<String>, Query(q): Query<LinkQuery>) -> ApiResult<Vec<u8>> {
    let path = check_link(&state, "GET", &key, &q)?;
    tokio::fs::read(&path).await.map_err(|_| ApiError::not_found("chunk"))
}
