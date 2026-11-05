//! The couple's OWN Supabase project as the relay. Nothing about it is built into the app:
//! URL and public (anon) key come from Settings or the invite code. The anon key is designed
//! to be public; access is enforced by the row-level security in supabase/setup.sql.

use super::relay::{BlobMeta, Device, Envelope, Relay};
use crate::error::{AppError, AppResult};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Read;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
    pub user_id: String,
}

#[derive(Deserialize)]
struct TokenResp {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
    user: UserResp,
}
#[derive(Deserialize)]
struct UserResp {
    id: String,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(15)).timeout(Duration::from_secs(120)).build()
}

/// Turns HTTP problems into messages a person can act on.
fn http_err(e: ureq::Error) -> AppError {
    match e {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            let msg = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| {
                    ["msg", "message", "error_description", "error"].iter().find_map(|k| v.get(*k).and_then(Value::as_str).map(String::from))
                })
                .unwrap_or(body);
            let msg = msg.chars().take(300).collect::<String>();
            if code == 404 && msg.contains("function") {
                AppError::invalid("Your Supabase project isn't set up yet: run the setup script in its SQL Editor.")
            } else if code == 540 || msg.to_lowercase().contains("paused") {
                AppError::invalid("Your Supabase project is paused. Open supabase.com and restore it. Your memories are safe on this PC.")
            } else {
                AppError::invalid(format!("Supabase said: {msg} ({code})"))
            }
        }
        ureq::Error::Transport(t) => AppError::invalid(format!("offline: {t}")),
    }
}

pub fn is_offline(e: &AppError) -> bool {
    e.to_string().starts_with("offline:")
}

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

fn session_from(t: TokenResp) -> Session {
    Session { access_token: t.access_token, refresh_token: t.refresh_token, expires_at: now_secs() + t.expires_in - 60, user_id: t.user.id }
}

pub fn normalize_url(url: &str) -> AppResult<String> {
    let u = url.trim().trim_end_matches('/').to_string();
    if !(u.starts_with("https://") || u.starts_with("http://localhost") || u.starts_with("http://127.0.0.1")) || u.len() > 200 {
        return Err(AppError::invalid("The project URL should look like https://abcdefgh.supabase.co"));
    }
    Ok(u)
}

/// Checks the URL/key and that setup.sql has been run.
pub fn check_project(url: &str, anon: &str) -> AppResult<i64> {
    let r = agent()
        .post(&format!("{url}/rest/v1/rpc/dear_us_version"))
        .set("apikey", anon)
        .set("Authorization", &format!("Bearer {anon}"))
        .send_json(json!({}))
        .map_err(http_err)?;
    r.into_json::<i64>().map_err(|_| AppError::invalid("Unexpected answer from Supabase. Is the URL right?"))
}

/// Ok(None) means: account created, but Supabase wants the email confirmed first.
pub fn sign_up(url: &str, anon: &str, email: &str, password: &str) -> AppResult<Option<Session>> {
    let r = agent().post(&format!("{url}/auth/v1/signup")).set("apikey", anon).send_json(json!({ "email": email, "password": password })).map_err(http_err)?;
    let v: Value = r.into_json().map_err(|e| AppError::invalid(e.to_string()))?;
    if v.get("access_token").is_some() {
        let t: TokenResp = serde_json::from_value(v).map_err(|e| AppError::invalid(e.to_string()))?;
        Ok(Some(session_from(t)))
    } else {
        Ok(None)
    }
}

pub fn sign_in(url: &str, anon: &str, email: &str, password: &str) -> AppResult<Session> {
    let r = agent()
        .post(&format!("{url}/auth/v1/token?grant_type=password"))
        .set("apikey", anon)
        .send_json(json!({ "email": email, "password": password }))
        .map_err(|e| match e {
            ureq::Error::Status(400, _) => AppError::invalid("Email or password isn't right, or the email isn't confirmed yet."),
            other => http_err(other),
        })?;
    Ok(session_from(r.into_json().map_err(|e| AppError::invalid(e.to_string()))?))
}

fn refresh(url: &str, anon: &str, refresh_token: &str) -> AppResult<Session> {
    let r = agent()
        .post(&format!("{url}/auth/v1/token?grant_type=refresh_token"))
        .set("apikey", anon)
        .send_json(json!({ "refresh_token": refresh_token }))
        .map_err(|e| match e {
            ureq::Error::Status(400 | 401, _) => AppError::invalid("signed-out: please sign in to sync again"),
            other => http_err(other),
        })?;
    Ok(session_from(r.into_json().map_err(|e| AppError::invalid(e.to_string()))?))
}

pub struct SupabaseRelay {
    url: String,
    anon: String,
    space: String,
    session: Mutex<Session>,
    /// Called whenever the refresh token rotates, so it can be saved securely.
    on_refresh: Box<dyn Fn(&str) + Send + Sync>,
    http: ureq::Agent,
}

impl SupabaseRelay {
    pub fn new(url: &str, anon: &str, space: &str, session: Session, on_refresh: Box<dyn Fn(&str) + Send + Sync>) -> SupabaseRelay {
        SupabaseRelay { url: url.into(), anon: anon.into(), space: space.into(), session: Mutex::new(session), on_refresh, http: agent() }
    }

    pub fn from_refresh(url: &str, anon: &str, space: &str, refresh_token: &str, on_refresh: Box<dyn Fn(&str) + Send + Sync>) -> AppResult<SupabaseRelay> {
        let s = refresh(url, anon, refresh_token)?;
        on_refresh(&s.refresh_token);
        Ok(SupabaseRelay::new(url, anon, space, s, on_refresh))
    }

    pub fn user_id(&self) -> String {
        self.session.lock().unwrap().user_id.clone()
    }

    fn token(&self) -> AppResult<String> {
        let mut s = self.session.lock().unwrap();
        if now_secs() >= s.expires_at {
            let n = refresh(&self.url, &self.anon, &s.refresh_token)?;
            (self.on_refresh)(&n.refresh_token);
            *s = n;
        }
        Ok(s.access_token.clone())
    }

    fn req(&self, method: &str, path: &str) -> AppResult<ureq::Request> {
        Ok(self
            .http
            .request(method, &format!("{}{path}", self.url))
            .set("apikey", &self.anon)
            .set("Authorization", &format!("Bearer {}", self.token()?)))
    }

    fn rpc(&self, func: &str, args: Value) -> AppResult<Value> {
        let r = self.req("POST", &format!("/rest/v1/rpc/{func}"))?.send_json(args).map_err(http_err)?;
        let body = r.into_string().map_err(|e| AppError::invalid(e.to_string()))?;
        Ok(if body.trim().is_empty() { Value::Null } else { serde_json::from_str(&body).unwrap_or(Value::Null) })
    }

    fn rpc_bool(&self, func: &str, args: Value) -> AppResult<bool> {
        Ok(self.rpc(func, args)?.as_bool().unwrap_or(false))
    }

    fn insert(&self, table: &str, row: Value) -> AppResult<()> {
        self.req("POST", &format!("/rest/v1/{table}"))?
            .set("Prefer", "resolution=ignore-duplicates,return=minimal")
            .send_json(row)
            .map_err(http_err)?;
        Ok(())
    }

    fn select(&self, path_and_query: &str) -> AppResult<Vec<Value>> {
        let r = self.req("GET", &format!("/rest/v1/{path_and_query}"))?.call().map_err(http_err)?;
        r.into_json().map_err(|e| AppError::invalid(e.to_string()))
    }

    // ---- pairing ----
    pub fn create_space(&self) -> AppResult<()> {
        self.rpc("create_space", json!({ "p_space": self.space }))?;
        Ok(())
    }
    pub fn register_device(&self, device: &str, name: &str) -> AppResult<()> {
        self.rpc("register_device", json!({ "p_device": device, "p_space": self.space, "p_name": name }))?;
        Ok(())
    }
    pub fn create_invite(&self, secret_hash: &str) -> AppResult<()> {
        self.rpc("create_invite", json!({ "p_space": self.space, "p_secret_hash": secret_hash }))?;
        Ok(())
    }
    pub fn revoke_device(&self, device: &str) -> AppResult<()> {
        self.rpc("revoke_device", json!({ "p_device": device }))?;
        Ok(())
    }
    fn chunk_path(&self, hash: &str, n: i64) -> String {
        format!("{}/{hash}/{n}", self.space)
    }
}

/// Redeems an invite with a fresh session (before we know the space). Returns the space id.
pub fn redeem_invite(url: &str, anon: &str, session: &Session, secret: &str) -> AppResult<String> {
    let r = agent()
        .post(&format!("{url}/rest/v1/rpc/redeem_invite"))
        .set("apikey", anon)
        .set("Authorization", &format!("Bearer {}", session.access_token))
        .send_json(json!({ "p_secret": secret }))
        .map_err(http_err)?;
    r.into_json::<String>().map_err(|_| AppError::invalid("Unexpected answer while joining"))
}

impl Relay for SupabaseRelay {
    fn devices(&self) -> AppResult<Vec<Device>> {
        let rows = self.select(&format!("devices?select=id,user_id,name,revoked_at,lan_endpoint&space_id=eq.{}", self.space))?;
        Ok(rows
            .into_iter()
            .map(|v| Device {
                id: v["id"].as_str().unwrap_or_default().into(),
                user_id: v["user_id"].as_str().unwrap_or_default().into(),
                name: v["name"].as_str().unwrap_or_default().into(),
                revoked: !v["revoked_at"].is_null(),
                endpoint: v["lan_endpoint"].as_str().map(String::from),
            })
            .collect())
    }
    fn put_envelope(&self, id: &str, sender: &str, recipients: &[String], ciphertext: &str) -> AppResult<()> {
        self.insert("envelopes", json!({ "id": id, "space_id": self.space, "sender_device": sender, "recipients": recipients, "ciphertext": ciphertext }))
    }
    fn fetch_envelopes(&self, me: &str, after: i64, limit: usize) -> AppResult<Vec<Envelope>> {
        let rows = self.select(&format!(
            "envelopes?select=id,seq,sender_device,ciphertext&space_id=eq.{}&seq=gt.{after}&recipients=cs.%7B{me}%7D&order=seq.asc&limit={limit}",
            self.space
        ))?;
        Ok(rows
            .into_iter()
            .map(|v| Envelope {
                id: v["id"].as_str().unwrap_or_default().into(),
                seq: v["seq"].as_i64().unwrap_or(0),
                sender_device: v["sender_device"].as_str().unwrap_or_default().into(),
                ciphertext: v["ciphertext"].as_str().unwrap_or_default().into(),
            })
            .collect())
    }
    fn ack_envelope(&self, id: &str, me: &str) -> AppResult<()> {
        self.insert("envelope_acks", json!({ "envelope_id": id, "device_id": me }))
    }
    fn envelope_delivered(&self, id: &str) -> AppResult<bool> {
        self.rpc_bool("envelope_delivered", json!({ "p_id": id }))
    }
    fn cleanup_envelope(&self, id: &str) -> AppResult<bool> {
        self.rpc_bool("cleanup_envelope", json!({ "p_id": id }))
    }
    fn blob_meta(&self, hash: &str) -> AppResult<Option<BlobMeta>> {
        let rows = self.select(&format!("blobs?select=hash,size,chunk_size,chunk_count,sender_device&space_id=eq.{}&hash=eq.{hash}", self.space))?;
        Ok(rows.into_iter().next().map(|v| BlobMeta {
            hash: v["hash"].as_str().unwrap_or_default().into(),
            size: v["size"].as_i64().unwrap_or(0),
            chunk_size: v["chunk_size"].as_i64().unwrap_or(0),
            chunk_count: v["chunk_count"].as_i64().unwrap_or(0),
            sender_device: v["sender_device"].as_str().unwrap_or_default().into(),
        }))
    }
    fn put_blob_meta(&self, m: &BlobMeta, recipients: &[String]) -> AppResult<()> {
        self.insert(
            "blobs",
            json!({ "space_id": self.space, "hash": m.hash, "size": m.size, "chunk_size": m.chunk_size, "chunk_count": m.chunk_count,
                    "sender_device": m.sender_device, "recipients": recipients }),
        )
    }
    fn put_chunk(&self, hash: &str, n: i64, data: &[u8]) -> AppResult<()> {
        self.req("POST", &format!("/storage/v1/object/relay/{}", self.chunk_path(hash, n)))?
            .set("x-upsert", "true")
            .set("Content-Type", "application/octet-stream")
            .send_bytes(data)
            .map_err(http_err)?;
        Ok(())
    }
    fn get_chunk(&self, hash: &str, n: i64) -> AppResult<Vec<u8>> {
        let r = self.req("GET", &format!("/storage/v1/object/authenticated/relay/{}", self.chunk_path(hash, n)))?.call().map_err(http_err)?;
        let mut buf = Vec::new();
        r.into_reader().take(64 * 1024 * 1024).read_to_end(&mut buf)?;
        Ok(buf)
    }
    fn ack_blob(&self, hash: &str, me: &str) -> AppResult<()> {
        self.insert("blob_acks", json!({ "space_id": self.space, "hash": hash, "device_id": me }))
    }
    fn blob_delivered(&self, hash: &str) -> AppResult<bool> {
        self.rpc_bool("blob_delivered", json!({ "p_space": self.space, "p_hash": hash }))
    }
    fn cleanup_blob(&self, hash: &str) -> AppResult<bool> {
        let Some(meta) = self.blob_meta(hash)? else { return Ok(true) };
        if !self.blob_delivered(hash)? {
            return Ok(false);
        }
        // Delete the stored chunks first (through the Storage API, so the space is really freed),
        // then the record.
        let names: Vec<String> = (0..meta.chunk_count).map(|n| self.chunk_path(hash, n)).collect();
        for batch in names.chunks(100) {
            self.req("DELETE", "/storage/v1/object/relay")?.send_json(json!({ "prefixes": batch })).map_err(http_err)?;
        }
        self.rpc_bool("forget_blob", json!({ "p_space": self.space, "p_hash": hash }))
    }
    fn set_endpoint(&self, device: &str, endpoint: Option<&str>) -> AppResult<()> {
        self.req("PATCH", &format!("/rest/v1/devices?id=eq.{device}"))?
            .set("Prefer", "return=minimal")
            .send_json(json!({ "lan_endpoint": endpoint, "last_seen": chrono::Utc::now().to_rfc3339() }))
            .map_err(http_err)?;
        Ok(())
    }
    fn used_bytes(&self) -> AppResult<i64> {
        Ok(self.rpc("space_usage", json!({ "p_space": self.space }))?.as_i64().unwrap_or(0))
    }
}
