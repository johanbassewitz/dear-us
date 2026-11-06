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

// ------------------------------------------------------------- password reset

const RESET_MINUTES: i32 = 30;
const RESET_ATTEMPTS: i32 = 5;

fn reset_hash(user: Uuid, code: &str) -> String {
    sha256_hex(&format!("dearus-reset|{user}|{code}"))
}

#[derive(Deserialize)]
pub struct ForgotIn {
    pub email: String,
}

/// Emails a 6-digit code. Always answers the same way, so it can't be used to find out which
/// emails have an account.
pub async fn forgot(State(state): State<Arc<AppState>>, headers: axum::http::HeaderMap, Json(b): Json<ForgotIn>) -> ApiResult<Json<bool>> {
    let email = check_email(&b.email)?;
    if !state.limiter.hit(&format!("forgot:{email}"), 3, Duration::from_secs(3600)) || !state.limiter.hit(&format!("forgot-ip:{}", client_key(&headers)), 20, Duration::from_secs(3600)) {
        return Err(ApiError::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "A code was just sent. Check your inbox (and spam), or try again in an hour."));
    }
    let user: Option<Uuid> = sqlx::query_scalar("select id from users where email = $1").bind(&email).fetch_optional(&state.db).await?;
    if let Some(user) = user {
        let code = format!("{:06}", rand::random::<u32>() % 1_000_000);
        sqlx::query(
            "insert into password_resets(user_id, code_hash, expires_at) values ($1, $2, now() + make_interval(mins => $3))
             on conflict (user_id) do update set code_hash = excluded.code_hash, expires_at = excluded.expires_at, attempts = 0, created_at = now()",
        )
        .bind(user)
        .bind(reset_hash(user, &code))
        .bind(RESET_MINUTES)
        .execute(&state.db)
        .await?;
        let text = format!(
            "Hi,\n\nYour Dear Us code to choose a new password is:\n\n    {code}\n\nIt works for {RESET_MINUTES} minutes. If you didn't ask for it, you can ignore this email: your account and memories are safe.\n\nDear Us"
        );
        crate::mail::send(&state, &email, "Your Dear Us code", &text).await?;
    }
    Ok(Json(true))
}

#[derive(Deserialize)]
pub struct ResetIn {
    pub email: String,
    pub code: String,
    pub password: String,
}

/// Sets a new password with the emailed code, signs out every other device's session and signs
/// this one in. Memories aren't affected: they're locked with the space key, not the password.
pub async fn reset(State(state): State<Arc<AppState>>, Json(b): Json<ResetIn>) -> ApiResult<Json<Tokens>> {
    let email = check_email(&b.email)?;
    check_password(&b.password)?;
    let wrong = || ApiError::new(StatusCode::BAD_REQUEST, "wrong_code", "That code isn't right or has expired. Ask for a new one.");
    let row: Option<(Uuid, String, i32)> = sqlx::query_as(
        "select r.user_id, r.code_hash, r.attempts from password_resets r join users u on u.id = r.user_id
          where u.email = $1 and r.expires_at > now()",
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await?;
    let Some((user, hash, attempts)) = row else { return Err(wrong()) };
    if attempts >= RESET_ATTEMPTS {
        sqlx::query("delete from password_resets where user_id = $1").bind(user).execute(&state.db).await?;
        return Err(wrong());
    }
    if reset_hash(user, b.code.trim()) != hash {
        sqlx::query("update password_resets set attempts = attempts + 1 where user_id = $1").bind(user).execute(&state.db).await?;
        return Err(wrong());
    }
    let new_hash = hash_password(b.password).await?;
    let mut tx = state.db.begin().await?;
    sqlx::query("update users set password_hash = $2 where id = $1").bind(user).bind(new_hash).execute(&mut *tx).await?;
    sqlx::query("delete from password_resets where user_id = $1").bind(user).execute(&mut *tx).await?;
    sqlx::query("delete from refresh_tokens where user_id = $1").bind(user).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(issue(&state, user).await?))
}

// ------------------------------------------------------------- account deletion

#[derive(Deserialize)]
pub struct DeleteIn {
    pub password: String,
}

/// Deletes this person's account for good (asks for the password again). Spaces they share
/// with a partner are handed over to the partner, so the partner keeps syncing and keeps the
/// plan; spaces where they're alone are deleted with everything waiting in them.
/// Memories on the PCs are not touched: they live there, not here.
pub async fn delete_account(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Json(b): Json<DeleteIn>) -> ApiResult<Json<bool>> {
    let hash: Option<String> = sqlx::query_scalar("select password_hash from users where id = $1").bind(user).fetch_optional(&state.db).await?;
    let hash = hash.ok_or_else(ApiError::unauthorized)?;
    if !verify_password(b.password, hash).await {
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "wrong_password", "That password isn't right."));
    }
    let spaces: Vec<Uuid> = sqlx::query_scalar("select space_id from members where user_id = $1").bind(user).fetch_all(&state.db).await?;
    let mut alone: Vec<Uuid> = Vec::new();
    let mut tx = state.db.begin().await?;
    for space in spaces {
        let partner: Option<Uuid> = sqlx::query_scalar("select user_id from members where space_id = $1 and user_id <> $2 limit 1").bind(space).bind(user).fetch_optional(&mut *tx).await?;
        match partner {
            Some(p) => {
                sqlx::query("update spaces set created_by = $2 where id = $1 and created_by = $3").bind(space).bind(p).bind(user).execute(&mut *tx).await?;
                // their subscription rows and purchases move with the space, so Premium the
                // couple paid for stays until it runs out (cancel it in the Microsoft account)
                sqlx::query("update subscriptions set user_id = $2 where space_id = $1 and user_id = $3").bind(space).bind(p).bind(user).execute(&mut *tx).await?;
                sqlx::query("update purchases set user_id = $2 where space_id = $1 and user_id = $3").bind(space).bind(p).bind(user).execute(&mut *tx).await?;
                // invites the leaving person made would otherwise vanish with them: fine
            }
            None => alone.push(space),
        }
    }
    tx.commit().await?;
    // spaces with nobody left: files in storage first, then the rows (cascade)
    for space in &alone {
        let hashes: Vec<String> = sqlx::query_scalar("select hash from blobs where space_id = $1").bind(space).fetch_all(&state.db).await?;
        for h in hashes {
            crate::relay::forget_blob(&state, *space, &h).await?;
        }
        sqlx::query("delete from spaces where id = $1").bind(space).execute(&state.db).await?;
    }
    // removes sessions, memberships, devices (and with them anything addressed to them)
    sqlx::query("delete from users where id = $1").bind(user).execute(&state.db).await?;
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
