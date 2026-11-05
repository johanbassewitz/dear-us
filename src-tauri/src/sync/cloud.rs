//! Store edition: the Dear Us Cloud mailbox (`cloud/` in this repository) instead of the
//! couple's own Supabase project. Same shape as `supabase.rs`, so pairing and the sync engine
//! don't change: everything sent is already sealed with the space key on this PC.
//!
//! There's nothing to configure: the server address is built in (`DEARUS_CLOUD_URL` at build
//! time overrides it, e.g. for a staging server).

use super::relay::{BlobMeta, Device, Envelope, Relay};
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Read;
use std::sync::Mutex;
use std::time::Duration;

pub const CLOUD_URL: &str = match option_env!("DEARUS_CLOUD_URL") {
    Some(u) => u,
    None => "https://api.dearus.app",
};

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

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Turns server answers into messages a person can act on. Plan limits come back as
/// "[<code>] message" so the UI can offer the right next step.
fn http_err(e: ureq::Error) -> AppError {
    match e {
        ureq::Error::Status(code, resp) => {
            let v: Value = resp.into_json().unwrap_or(Value::Null);
            let msg = v["error"].as_str().unwrap_or("The Dear Us cloud didn't accept that.").chars().take(300).collect::<String>();
            let kind = v["code"].as_str().unwrap_or("");
            match (code, kind) {
                (401, _) => AppError::invalid("signed-out: please sign in to sync again"),
                (402, k) => AppError::invalid(format!("[{k}] {msg}")),
                (502..=504, _) => AppError::invalid("offline: the Dear Us cloud is taking a short break. Your memories are safe on this PC."),
                _ => AppError::invalid(msg),
            }
        }
        ureq::Error::Transport(t) => AppError::invalid(format!("offline: {t}")),
    }
}

pub fn is_offline(e: &AppError) -> bool {
    e.to_string().starts_with("offline:")
}

/// The Store edition always talks to its own server; whatever URL was passed is ignored.
pub fn normalize_url(_url: &str) -> AppResult<String> {
    Ok(CLOUD_URL.trim_end_matches('/').to_string())
}

pub fn check_project(url: &str, _anon: &str) -> AppResult<i64> {
    let v: Value = agent().get(&format!("{url}/v1/health")).call().map_err(http_err)?.into_json().map_err(|e| AppError::invalid(e.to_string()))?;
    v["version"].as_i64().ok_or_else(|| AppError::invalid("Unexpected answer from the Dear Us cloud."))
}

fn session_from(t: TokenResp) -> Session {
    Session { access_token: t.access_token, refresh_token: t.refresh_token, expires_at: now_secs() + t.expires_in - 60, user_id: t.user.id }
}

fn token_call(url: &str, path: &str, body: Value, on_err: impl Fn(ureq::Error) -> AppError) -> AppResult<Session> {
    let t: TokenResp = agent().post(&format!("{url}{path}")).send_json(body).map_err(on_err)?.into_json().map_err(|e| AppError::invalid(e.to_string()))?;
    Ok(session_from(t))
}

/// Always Some: the cloud doesn't ask for an email confirmation before the first sign-in.
pub fn sign_up(url: &str, _anon: &str, email: &str, password: &str) -> AppResult<Option<Session>> {
    token_call(url, "/v1/auth/signup", json!({ "email": email, "password": password }), http_err).map(Some)
}

pub fn sign_in(url: &str, _anon: &str, email: &str, password: &str) -> AppResult<Session> {
    token_call(url, "/v1/auth/signin", json!({ "email": email, "password": password }), |e| match e {
        ureq::Error::Status(401, _) => AppError::invalid("Email or password isn't right."),
        other => http_err(other),
    })
}

fn refresh(url: &str, refresh_token: &str) -> AppResult<Session> {
    token_call(url, "/v1/auth/refresh", json!({ "refresh_token": refresh_token }), http_err)
}

pub fn redeem_invite(url: &str, _anon: &str, session: &Session, secret: &str) -> AppResult<String> {
    let v: Value = agent()
        .post(&format!("{url}/v1/invites/redeem"))
        .set("Authorization", &format!("Bearer {}", session.access_token))
        .send_json(json!({ "secret": secret }))
        .map_err(http_err)?
        .into_json()
        .map_err(|e| AppError::invalid(e.to_string()))?;
    v["space_id"].as_str().map(String::from).ok_or_else(|| AppError::invalid("Unexpected answer while joining"))
}

/// The couple's plan, as decided by the server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct Plan {
    /// solo | trial | premium | free
    pub tier: String,
    pub trial_ends_at: Option<String>,
    pub premium_until: Option<String>,
    pub period: String,
    pub media_cap_bytes: Option<i64>,
    pub media_used_bytes: i64,
    pub cupid_allowance: i32,
    pub cupid_used: i32,
    pub cupid_bonus: i32,
    pub cupid_remaining: i32,
    pub billing_available: bool,
}

impl Plan {
    /// The cap the sync engine compares cloud uploads against.
    pub fn engine_cap(&self) -> i64 {
        self.media_cap_bytes.unwrap_or(i64::MAX / 4)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StoreProducts {
    pub premium_monthly: String,
    pub premium_yearly: String,
    pub arrows_100: String,
    pub arrows_300: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StoreTickets {
    pub collections_ticket: String,
    pub purchase_ticket: String,
    pub publisher_user_id: String,
    pub products: StoreProducts,
}

pub struct CloudRelay {
    url: String,
    space: String,
    session: Mutex<Session>,
    on_refresh: Box<dyn Fn(&str) + Send + Sync>,
    http: ureq::Agent,
}

impl CloudRelay {
    pub fn new(url: &str, _anon: &str, space: &str, session: Session, on_refresh: Box<dyn Fn(&str) + Send + Sync>) -> CloudRelay {
        CloudRelay { url: url.into(), space: space.into(), session: Mutex::new(session), on_refresh, http: agent() }
    }

    pub fn from_refresh(url: &str, _anon: &str, space: &str, refresh_token: &str, on_refresh: Box<dyn Fn(&str) + Send + Sync>) -> AppResult<CloudRelay> {
        let s = refresh(url, refresh_token)?;
        on_refresh(&s.refresh_token);
        Ok(CloudRelay::new(url, "", space, s, on_refresh))
    }

    pub fn user_id(&self) -> String {
        self.session.lock().unwrap().user_id.clone()
    }

    fn token(&self) -> AppResult<String> {
        let mut s = self.session.lock().unwrap();
        if now_secs() >= s.expires_at {
            let n = refresh(&self.url, &s.refresh_token)?;
            (self.on_refresh)(&n.refresh_token);
            *s = n;
        }
        Ok(s.access_token.clone())
    }

    fn req(&self, method: &str, path: &str) -> AppResult<ureq::Request> {
        Ok(self.http.request(method, &format!("{}{path}", self.url)).set("Authorization", &format!("Bearer {}", self.token()?)))
    }

    fn sp(&self, rest: &str) -> String {
        format!("/v1/spaces/{}{rest}", self.space)
    }

    fn json<T: serde::de::DeserializeOwned>(&self, method: &str, path: &str, body: Option<Value>) -> AppResult<T> {
        let r = self.req(method, path)?;
        let r = match body {
            Some(b) => r.send_json(b),
            None => r.call(),
        }
        .map_err(http_err)?;
        r.into_json().map_err(|e| AppError::invalid(format!("Unexpected answer from the Dear Us cloud: {e}")))
    }

    // ---- pairing (same names as SupabaseRelay) ----
    pub fn create_space(&self) -> AppResult<()> {
        self.json::<Value>("POST", "/v1/spaces", Some(json!({ "id": self.space }))).map(|_| ())
    }
    pub fn register_device(&self, device: &str, name: &str) -> AppResult<()> {
        self.json::<Value>("POST", &self.sp("/devices"), Some(json!({ "id": device, "name": name }))).map(|_| ())
    }
    pub fn create_invite(&self, secret_hash: &str) -> AppResult<()> {
        self.json::<Value>("POST", &self.sp("/invites"), Some(json!({ "secret_hash": secret_hash }))).map(|_| ())
    }
    pub fn revoke_device(&self, device: &str) -> AppResult<()> {
        self.json::<Value>("POST", &self.sp(&format!("/devices/{device}/revoke")), Some(json!({}))).map(|_| ())
    }

    // ---- plan, Cupid, purchases (Store edition only) ----
    pub fn plan(&self) -> AppResult<Plan> {
        self.json("GET", &self.sp("/plan"), None)
    }

    /// One Cupid request through our Gemini gateway. Returns the reply and the plan after it.
    pub fn cupid(&self, kind: &str, system: &str, turns: &[(String, String)], json_reply: bool) -> AppResult<(String, Plan)> {
        let turns: Vec<Value> = turns.iter().map(|(role, text)| json!({ "role": role, "text": text })).collect();
        let v: Value = self.json("POST", &self.sp("/cupid"), Some(json!({ "kind": kind, "system": system, "turns": turns, "json": json_reply })))?;
        let reply = v["reply"].as_str().unwrap_or_default().to_string();
        let plan: Plan = serde_json::from_value(v["plan"].clone()).map_err(|e| AppError::invalid(e.to_string()))?;
        Ok((reply, plan))
    }

    pub fn store_tickets(&self) -> AppResult<StoreTickets> {
        self.json("POST", "/v1/billing/microsoft/tickets", Some(json!({})))
    }

    pub fn store_sync(&self, collections_id_key: &str, purchase_id_key: &str) -> AppResult<Plan> {
        self.json("POST", &self.sp("/billing/microsoft/sync"), Some(json!({ "collections_id_key": collections_id_key, "purchase_id_key": purchase_id_key })))
    }

    fn link(&self, path: &str) -> AppResult<String> {
        let v: Value = self.json("POST", path, Some(json!({})))?;
        v["url"].as_str().map(String::from).ok_or_else(|| AppError::invalid("Unexpected answer from the Dear Us cloud"))
    }
}

impl Relay for CloudRelay {
    fn devices(&self) -> AppResult<Vec<Device>> {
        let rows: Vec<Value> = self.json("GET", &self.sp("/devices"), None)?;
        Ok(rows
            .into_iter()
            .map(|v| Device {
                id: v["id"].as_str().unwrap_or_default().into(),
                user_id: v["user_id"].as_str().unwrap_or_default().into(),
                name: v["name"].as_str().unwrap_or_default().into(),
                revoked: v["revoked"].as_bool().unwrap_or(false),
                endpoint: v["lan_endpoint"].as_str().map(String::from),
            })
            .collect())
    }
    fn put_envelope(&self, id: &str, sender: &str, recipients: &[String], ciphertext: &str) -> AppResult<()> {
        self.json::<Value>("POST", &self.sp("/envelopes"), Some(json!({ "id": id, "sender_device": sender, "recipients": recipients, "ciphertext": ciphertext })))
            .map(|_| ())
    }
    fn fetch_envelopes(&self, me: &str, after: i64, limit: usize) -> AppResult<Vec<Envelope>> {
        let rows: Vec<Value> = self.json("GET", &self.sp(&format!("/envelopes?device={me}&after={after}&limit={limit}")), None)?;
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
        self.json::<Value>("POST", &self.sp(&format!("/envelopes/{id}/ack")), Some(json!({ "device": me }))).map(|_| ())
    }
    fn envelope_delivered(&self, id: &str) -> AppResult<bool> {
        self.json("GET", &self.sp(&format!("/envelopes/{id}/delivered")), None)
    }
    fn cleanup_envelope(&self, id: &str) -> AppResult<bool> {
        self.json("DELETE", &self.sp(&format!("/envelopes/{id}")), None)
    }
    fn blob_meta(&self, hash: &str) -> AppResult<Option<BlobMeta>> {
        let v: Value = self.json("GET", &self.sp(&format!("/blobs/{hash}")), None)?;
        if v.is_null() {
            return Ok(None);
        }
        Ok(Some(BlobMeta {
            hash: v["hash"].as_str().unwrap_or_default().into(),
            size: v["size"].as_i64().unwrap_or(0),
            chunk_size: v["chunk_size"].as_i64().unwrap_or(0),
            chunk_count: v["chunk_count"].as_i64().unwrap_or(0),
            sender_device: v["sender_device"].as_str().unwrap_or_default().into(),
        }))
    }
    fn put_blob_meta(&self, m: &BlobMeta, recipients: &[String]) -> AppResult<()> {
        self.json::<Value>(
            "PUT",
            &self.sp(&format!("/blobs/{}", m.hash)),
            Some(json!({ "size": m.size, "chunk_size": m.chunk_size, "chunk_count": m.chunk_count, "sender_device": m.sender_device, "recipients": recipients })),
        )
        .map(|_| ())
    }
    fn put_chunk(&self, hash: &str, n: i64, data: &[u8]) -> AppResult<()> {
        let url = self.link(&self.sp(&format!("/blobs/{hash}/chunks/{n}/upload")))?;
        self.http.put(&url).set("Content-Type", "application/octet-stream").send_bytes(data).map_err(http_err)?;
        Ok(())
    }
    fn get_chunk(&self, hash: &str, n: i64) -> AppResult<Vec<u8>> {
        let url = self.link(&self.sp(&format!("/blobs/{hash}/chunks/{n}/download")))?;
        let r = self.http.get(&url).call().map_err(|e| match e {
            ureq::Error::Status(404, _) => AppError::invalid("chunk not uploaded yet"),
            other => http_err(other),
        })?;
        let mut buf = Vec::new();
        r.into_reader().take(64 * 1024 * 1024).read_to_end(&mut buf)?;
        Ok(buf)
    }
    fn ack_blob(&self, hash: &str, me: &str) -> AppResult<()> {
        self.json::<Value>("POST", &self.sp(&format!("/blobs/{hash}/ack")), Some(json!({ "device": me }))).map(|_| ())
    }
    fn blob_delivered(&self, hash: &str) -> AppResult<bool> {
        self.json("GET", &self.sp(&format!("/blobs/{hash}/delivered")), None)
    }
    fn cleanup_blob(&self, hash: &str) -> AppResult<bool> {
        self.json("DELETE", &self.sp(&format!("/blobs/{hash}")), None)
    }
    fn set_endpoint(&self, device: &str, endpoint: Option<&str>) -> AppResult<()> {
        self.json::<Value>("PATCH", &self.sp(&format!("/devices/{device}")), Some(json!({ "lan_endpoint": endpoint }))).map(|_| ())
    }
    /// For the Store edition: this month's cloud media charged against the plan's cap.
    fn used_bytes(&self) -> AppResult<i64> {
        self.json("GET", &self.sp("/usage"), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_limits_reach_the_engine() {
        let mut p = Plan {
            tier: "free".into(),
            trial_ends_at: None,
            premium_until: None,
            period: "2026-11".into(),
            media_cap_bytes: Some(100),
            media_used_bytes: 0,
            cupid_allowance: 10,
            cupid_used: 0,
            cupid_bonus: 0,
            cupid_remaining: 10,
            billing_available: true,
        };
        assert_eq!(p.engine_cap(), 100);
        p.media_cap_bytes = None;
        assert!(p.engine_cap() > 1 << 50, "unlimited, without overflowing used + size");
    }

    #[test]
    fn the_cloud_address_is_built_in() {
        assert_eq!(normalize_url("https://someone-elses.supabase.co").unwrap(), CLOUD_URL.trim_end_matches('/'));
    }

    /// Full two-PC sync through a running Dear Us Cloud server:
    ///   DEARUS_CLOUD_TEST_URL=http://127.0.0.1:8080 cargo test --features store cloud_e2e -- --ignored
    #[test]
    #[ignore]
    fn cloud_e2e_envelopes_and_files() {
        let url = std::env::var("DEARUS_CLOUD_TEST_URL").expect("DEARUS_CLOUD_TEST_URL");
        let email = |n: &str| format!("{n}-{}@e2e.dearus", uuid::Uuid::now_v7());
        let a = sign_up(&url, "", &email("a"), "password-a").unwrap().unwrap();
        let b = sign_up(&url, "", &email("b"), "password-b").unwrap().unwrap();
        let space = uuid::Uuid::now_v7().to_string();
        let ra = CloudRelay::new(&url, "", &space, a, Box::new(|_| {}));
        ra.create_space().unwrap();
        let (da, db) = (uuid::Uuid::now_v7().to_string(), uuid::Uuid::now_v7().to_string());
        ra.register_device(&da, "A").unwrap();
        let secret = "e2e-secret";
        let secret = format!("{secret}-{space}");
        ra.create_invite(&super::super::crypto::sha256_hex(&secret)).unwrap();
        assert_eq!(redeem_invite(&url, "", &b, &secret).unwrap(), space);
        let rb = CloudRelay::new(&url, "", &space, b, Box::new(|_| {}));
        rb.register_device(&db, "B").unwrap();
        assert_eq!(ra.plan().unwrap().tier, "trial");
        assert_eq!(ra.devices().unwrap().len(), 2);

        let env = uuid::Uuid::now_v7().to_string();
        ra.put_envelope(&env, &da, &[db.clone()], "sealed").unwrap();
        let got = rb.fetch_envelopes(&db, 0, 10).unwrap();
        assert_eq!(got[0].ciphertext, "sealed");
        rb.ack_envelope(&env, &db).unwrap();
        assert!(ra.cleanup_envelope(&env).unwrap());

        let hash = super::super::crypto::sha256_hex("photo");
        ra.put_blob_meta(&BlobMeta { hash: hash.clone(), size: 5, chunk_size: 4, chunk_count: 2, sender_device: da.clone() }, &[db.clone()]).unwrap();
        ra.put_chunk(&hash, 0, b"abcd").unwrap();
        ra.put_chunk(&hash, 1, b"e").unwrap();
        assert_eq!(rb.get_chunk(&hash, 0).unwrap(), b"abcd");
        assert_eq!(rb.blob_meta(&hash).unwrap().unwrap().chunk_count, 2);
        rb.ack_blob(&hash, &db).unwrap();
        assert!(ra.cleanup_blob(&hash).unwrap());
        assert!(rb.blob_meta(&hash).unwrap().is_none());
        assert_eq!(ra.used_bytes().unwrap(), 5);
    }
}
