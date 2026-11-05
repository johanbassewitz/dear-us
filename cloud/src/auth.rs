//! Accounts: email + password (Argon2id), short-lived signed access tokens (1 h) and rotating
//! refresh tokens (90 days, stored only as SHA-256). One account = one person; both partners
//! have their own.

use crate::error::{ApiError, ApiResult};
use crate::AppState;
use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::Json;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub const ACCESS_TTL_SECS: i64 = 3600;
const REFRESH_TTL_DAYS: i64 = 90;

type HmacSha256 = Hmac<Sha256>;

pub fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

pub fn random_token() -> String {
    let mut b = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}

pub fn mac(secret: &[u8], msg: &str) -> String {
    let mut m = HmacSha256::new_from_slice(secret).expect("hmac key");
    m.update(msg.as_bytes());
    URL_SAFE_NO_PAD.encode(m.finalize().into_bytes())
}

pub fn mac_ok(secret: &[u8], msg: &str, sig: &str) -> bool {
    let Ok(raw) = URL_SAFE_NO_PAD.decode(sig) else { return false };
    let mut m = HmacSha256::new_from_slice(secret).expect("hmac key");
    m.update(msg.as_bytes());
    m.verify_slice(&raw).is_ok()
}

/// `v1.<user>.<exp>.<mac>`
pub fn sign_access(secret: &[u8], user: Uuid, exp: i64) -> String {
    let body = format!("v1.{user}.{exp}");
    let sig = mac(secret, &body);
    format!("{body}.{sig}")
}

pub fn verify_access(secret: &[u8], token: &str, now: i64) -> Option<Uuid> {
    let mut parts = token.rsplitn(2, '.');
    let sig = parts.next()?;
    let body = parts.next()?;
    if !mac_ok(secret, body, sig) {
        return None;
    }
    let mut f = body.split('.');
    if f.next()? != "v1" {
        return None;
    }
    let user = Uuid::parse_str(f.next()?).ok()?;
    let exp: i64 = f.next()?.parse().ok()?;
    (exp > now).then_some(user)
}

/// The signed-in person, from `Authorization: Bearer <access token>`.
pub struct AuthUser(pub Uuid);

impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        let h = parts.headers.get("authorization").and_then(|v| v.to_str().ok()).ok_or_else(ApiError::unauthorized)?;
        let token = h.strip_prefix("Bearer ").ok_or_else(ApiError::unauthorized)?;
        verify_access(&state.cfg.secret, token.trim(), chrono::Utc::now().timestamp()).map(AuthUser).ok_or_else(ApiError::unauthorized)
    }
}

/// Small in-memory brake on password guessing (per email and per client address).
#[derive(Default)]
pub struct Limiter(Mutex<HashMap<String, (Instant, u32)>>);

impl Limiter {
    pub fn hit(&self, key: &str, max: u32, window: Duration) -> bool {
        let mut m = self.0.lock().unwrap();
        let now = Instant::now();
        if m.len() > 50_000 {
            m.retain(|_, (t, _)| now.duration_since(*t) < window);
        }
        let e = m.entry(key.to_string()).or_insert((now, 0));
        if now.duration_since(e.0) >= window {
            *e = (now, 0);
        }
        e.1 += 1;
        e.1 <= max
    }
}

fn client_key(parts_headers: &axum::http::HeaderMap) -> String {
    parts_headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "direct".into())
}

#[derive(Deserialize)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct UserOut {
    pub id: Uuid,
}

#[derive(Serialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub user: UserOut,
}

fn check_email(e: &str) -> ApiResult<String> {
    let e = e.trim().to_lowercase();
    let ok = e.len() >= 5 && e.len() <= 200 && e.contains('@') && !e.starts_with('@') && !e.ends_with('@') && !e.contains(char::is_whitespace);
    ok.then_some(e).ok_or_else(|| ApiError::bad("Please enter a valid email address."))
}

fn check_password(p: &str) -> ApiResult<()> {
    let n = p.chars().count();
    if n < 8 {
        return Err(ApiError::bad("Use at least 8 characters for the password."));
    }
    if n > 200 {
        return Err(ApiError::bad("That password is too long."));
    }
    Ok(())
}

pub async fn hash_password(p: String) -> ApiResult<String> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default().hash_password(p.as_bytes(), &salt).map(|h| h.to_string())
    })
    .await
    .map_err(ApiError::internal)?
    .map_err(ApiError::internal)
}

async fn verify_password(p: String, hash: String) -> bool {
    tokio::task::spawn_blocking(move || PasswordHash::new(&hash).map(|h| Argon2::default().verify_password(p.as_bytes(), &h).is_ok()).unwrap_or(false))
        .await
        .unwrap_or(false)
}

async fn issue(state: &AppState, user: Uuid) -> ApiResult<Tokens> {
    let refresh = random_token();
    sqlx::query("insert into refresh_tokens(token_hash, user_id, expires_at) values ($1, $2, now() + make_interval(days => $3))")
        .bind(sha256_hex(&refresh))
        .bind(user)
        .bind(REFRESH_TTL_DAYS as i32)
        .execute(&state.db)
        .await?;
    let exp = chrono::Utc::now().timestamp() + ACCESS_TTL_SECS;
    Ok(Tokens { access_token: sign_access(&state.cfg.secret, user, exp), refresh_token: refresh, expires_in: ACCESS_TTL_SECS, user: UserOut { id: user } })
}

pub async fn signup(State(state): State<Arc<AppState>>, headers: axum::http::HeaderMap, Json(c): Json<Credentials>) -> ApiResult<Json<Tokens>> {
    let email = check_email(&c.email)?;
    check_password(&c.password)?;
    if !state.limiter.hit(&format!("signup:{}", client_key(&headers)), 20, Duration::from_secs(3600)) {
        return Err(ApiError::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "Too many new accounts from here. Try again in an hour."));
    }
    let hash = hash_password(c.password).await?;
    let id = Uuid::new_v4();
    let r = sqlx::query("insert into users(id, email, password_hash) values ($1, $2, $3)").bind(id).bind(&email).bind(hash).execute(&state.db).await;
    match r {
        Ok(_) => {}
        Err(sqlx::Error::Database(d)) if d.code().as_deref() == Some("23505") => {
            return Err(ApiError::new(StatusCode::CONFLICT, "email_taken", "There's already an account with this email. Sign in instead."))
        }
        Err(e) => return Err(e.into()),
    }
    Ok(Json(issue(&state, id).await?))
}

pub async fn signin(State(state): State<Arc<AppState>>, headers: axum::http::HeaderMap, Json(c): Json<Credentials>) -> ApiResult<Json<Tokens>> {
    let email = check_email(&c.email)?;
    let wait = || ApiError::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "Too many tries. Wait 15 minutes and try again.");
    if !state.limiter.hit(&format!("signin:{email}"), 10, Duration::from_secs(900)) || !state.limiter.hit(&format!("signin-ip:{}", client_key(&headers)), 60, Duration::from_secs(900)) {
        return Err(wait());
    }
    let row: Option<(Uuid, String)> = sqlx::query_as("select id, password_hash from users where email = $1").bind(&email).fetch_optional(&state.db).await?;
    let wrong = || ApiError::new(StatusCode::UNAUTHORIZED, "wrong_password", "Email or password isn't right.");
    let Some((id, hash)) = row else {
        // same work either way, so timing doesn't reveal which emails exist
        let _ = verify_password(c.password, state.dummy_hash.clone()).await;
        return Err(wrong());
    };
    if !verify_password(c.password, hash).await {
        return Err(wrong());
    }
    Ok(Json(issue(&state, id).await?))
}

#[derive(Deserialize)]
pub struct RefreshIn {
    pub refresh_token: String,
}

/// Rotates the refresh token: the old one stops working.
pub async fn refresh(State(state): State<Arc<AppState>>, Json(r): Json<RefreshIn>) -> ApiResult<Json<Tokens>> {
    let user: Option<(Uuid,)> = sqlx::query_as("delete from refresh_tokens where token_hash = $1 and expires_at > now() returning user_id")
        .bind(sha256_hex(r.refresh_token.trim()))
        .fetch_optional(&state.db)
        .await?;
    let (user,) = user.ok_or_else(ApiError::unauthorized)?;
    Ok(Json(issue(&state, user).await?))
}

pub async fn signout(State(state): State<Arc<AppState>>, Json(r): Json<RefreshIn>) -> ApiResult<Json<bool>> {
    sqlx::query("delete from refresh_tokens where token_hash = $1").bind(sha256_hex(r.refresh_token.trim())).execute(&state.db).await?;
    Ok(Json(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_tokens_verify_and_expire() {
        let s = b"0123456789abcdef0123456789abcdef";
        let u = Uuid::new_v4();
        let t = sign_access(s, u, 1000);
        assert_eq!(verify_access(s, &t, 999), Some(u));
        assert_eq!(verify_access(s, &t, 1000), None, "expired");
        assert_eq!(verify_access(b"another-secret-another-secret-xx", &t, 1), None, "wrong secret");
        let forged = t.replace(&u.to_string(), &Uuid::new_v4().to_string());
        assert_eq!(verify_access(s, &forged, 1), None, "tampered");
    }

    #[test]
    fn limiter_brakes_then_resets() {
        let l = Limiter::default();
        assert!(l.hit("a", 2, Duration::from_secs(60)));
        assert!(l.hit("a", 2, Duration::from_secs(60)));
        assert!(!l.hit("a", 2, Duration::from_secs(60)));
        assert!(l.hit("b", 2, Duration::from_secs(60)));
        assert!(l.hit("a", 2, Duration::from_millis(0)), "new window");
    }

    #[test]
    fn emails_are_normalized() {
        assert_eq!(check_email("  Me@Example.COM ").unwrap(), "me@example.com");
        assert!(check_email("nope").is_err());
        assert!(check_email("a b@c.d").is_err());
    }
}
