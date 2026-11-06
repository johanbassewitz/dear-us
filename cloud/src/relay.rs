//! The ephemeral mailbox. Everything here is already encrypted by the apps. Items are deleted as
//! soon as every (non-removed) partner device has confirmed it; the sweeper removes leftovers
//! after `MAILBOX_TTL_DAYS` (the sender always keeps the original).

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::spaces::{require_device, require_member};
use crate::storage::object_key;
use crate::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

fn check_hash(h: &str) -> ApiResult<()> {
    (h.len() == 64 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))).then_some(()).ok_or_else(|| ApiError::bad("bad file hash"))
}

// ------------------------------------------------------------------ envelopes

#[derive(Deserialize)]
pub struct PutEnvelope {
    pub id: Uuid,
    pub sender_device: Uuid,
    pub recipients: Vec<Uuid>,
    pub ciphertext: String,
}

/// Idempotent: the same id twice is a no-op.
pub async fn put_envelope(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path(space): Path<Uuid>, Json(b): Json<PutEnvelope>) -> ApiResult<Json<bool>> {
    require_member(&state.db, space, user).await?;
    require_device(&state.db, space, user, b.sender_device).await?;
    if b.recipients.len() > 50 {
        return Err(ApiError::bad("too many recipients"));
    }
    sqlx::query("insert into envelopes(id, space_id, sender_device, recipients, ciphertext) values ($1, $2, $3, $4, $5) on conflict (id) do nothing")
        .bind(b.id)
        .bind(space)
        .bind(b.sender_device)
        .bind(&b.recipients)
        .bind(&b.ciphertext)
        .execute(&state.db)
        .await?;
    Ok(Json(true))
}

#[derive(Deserialize)]
pub struct FetchQuery {
    pub device: Uuid,
    #[serde(default)]
    pub after: i64,
    #[serde(default = "default_limit")]
    pub limit: i64,
}
fn default_limit() -> i64 {
    20
}

#[derive(Serialize, sqlx::FromRow)]
pub struct EnvelopeOut {
    pub id: Uuid,
    pub seq: i64,
    pub sender_device: Uuid,
    pub ciphertext: String,
}

pub async fn fetch_envelopes(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path(space): Path<Uuid>, Query(q): Query<FetchQuery>) -> ApiResult<Json<Vec<EnvelopeOut>>> {
    require_member(&state.db, space, user).await?;
    require_device(&state.db, space, user, q.device).await?;
    let rows = sqlx::query_as::<_, EnvelopeOut>(
        "select id, seq, sender_device, ciphertext from envelopes
          where space_id = $1 and seq > $2 and $3 = any(recipients) order by seq asc limit $4",
    )
    .bind(space)
    .bind(q.after)
    .bind(q.device)
    .bind(q.limit.clamp(1, 100))
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct Ack {
    pub device: Uuid,
}

pub async fn ack_envelope(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path((space, id)): Path<(Uuid, Uuid)>, Json(b): Json<Ack>) -> ApiResult<Json<bool>> {
    require_member(&state.db, space, user).await?;
    require_device(&state.db, space, user, b.device).await?;
    sqlx::query(
        "insert into envelope_acks(envelope_id, device_id)
         select id, $3 from envelopes where id = $1 and space_id = $2 on conflict do nothing",
    )
    .bind(id)
    .bind(space)
    .bind(b.device)
    .execute(&state.db)
    .await?;
    Ok(Json(true))
}

async fn envelope_delivered_inner(state: &AppState, space: Uuid, id: Uuid) -> ApiResult<bool> {
    let v: Option<bool> = sqlx::query_scalar(
        "select all_acked(e.recipients, array(select device_id from envelope_acks a where a.envelope_id = e.id))
           from envelopes e where e.id = $1 and e.space_id = $2",
    )
    .bind(id)
    .bind(space)
    .fetch_optional(&state.db)
    .await?;
    Ok(v.unwrap_or(true)) // already gone = delivered
}

pub async fn envelope_delivered(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path((space, id)): Path<(Uuid, Uuid)>) -> ApiResult<Json<bool>> {
    require_member(&state.db, space, user).await?;
    Ok(Json(envelope_delivered_inner(&state, space, id).await?))
}

/// Deletes only if delivered. Returns true when it no longer exists.
pub async fn cleanup_envelope(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path((space, id)): Path<(Uuid, Uuid)>) -> ApiResult<Json<bool>> {
    require_member(&state.db, space, user).await?;
    if !envelope_delivered_inner(&state, space, id).await? {
        return Ok(Json(false));
    }
    sqlx::query("delete from envelopes where id = $1 and space_id = $2").bind(id).bind(space).execute(&state.db).await?;
    Ok(Json(true))
}

// ---------------------------------------------------------------------- blobs

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct BlobMeta {
    pub hash: String,
    pub size: i64,
    pub chunk_size: i64,
    pub chunk_count: i32,
    pub sender_device: Uuid,
}

pub async fn get_blob(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path((space, hash)): Path<(Uuid, String)>) -> ApiResult<Json<Option<BlobMeta>>> {
    require_member(&state.db, space, user).await?;
    check_hash(&hash)?;
    let m = sqlx::query_as::<_, BlobMeta>("select hash, size, chunk_size, chunk_count, sender_device from blobs where space_id = $1 and hash = $2")
        .bind(space)
        .bind(&hash)
        .fetch_optional(&state.db)
        .await?;
    Ok(Json(m))
}

#[derive(Deserialize)]
pub struct PutBlob {
    pub size: i64,
    pub chunk_size: i64,
    pub chunk_count: i32,
    pub sender_device: Uuid,
    pub recipients: Vec<Uuid>,
    /// "media" (default) or "bootstrap": the one-time snapshot for a newly linked device.
    #[serde(default)]
    pub purpose: Option<String>,
}

/// Biggest one-time snapshot accepted for a newly linked device (text only; photos follow
/// as normal files).
const BOOTSTRAP_MAX: i64 = 256 * 1024 * 1024;

/// Announces a file. Nothing is charged yet: files that end up going over Wi-Fi are free.
pub async fn put_blob(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path((space, hash)): Path<(Uuid, String)>, Json(b): Json<PutBlob>) -> ApiResult<Json<bool>> {
    require_member(&state.db, space, user).await?;
    require_device(&state.db, space, user, b.sender_device).await?;
    check_hash(&hash)?;
    let expected = if b.size == 0 { 0 } else { (b.size + b.chunk_size - 1) / b.chunk_size.max(1) };
    if b.size < 0 || b.chunk_size <= 0 || b.chunk_size > 16 * 1024 * 1024 || i64::from(b.chunk_count) != expected || b.recipients.len() > 50 {
        return Err(ApiError::bad("file description doesn't add up"));
    }
    let bootstrap = match b.purpose.as_deref().unwrap_or("media") {
        "media" => false,
        "bootstrap" => {
            // free, but only for devices that just joined, and only the sender's own
            let fresh: i64 = sqlx::query_scalar(
                "select count(*) from devices where space_id = $1 and id = any($2) and user_id = $3 and revoked_at is null
                   and registered_at > now() - interval '1 day'",
            )
            .bind(space)
            .bind(&b.recipients)
            .bind(user)
            .fetch_one(&state.db)
            .await?;
            if b.size > BOOTSTRAP_MAX || b.recipients.is_empty() || fresh != b.recipients.len() as i64 {
                return Err(ApiError::bad("A device snapshot can only go to your own newly linked device."));
            }
            true
        }
        _ => return Err(ApiError::bad("unknown file purpose")),
    };
    sqlx::query(
        "insert into blobs(space_id, hash, size, chunk_size, chunk_count, sender_device, recipients, purpose, counted) values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         on conflict (space_id, hash) do nothing",
    )
    .bind(space)
    .bind(&hash)
    .bind(b.size)
    .bind(b.chunk_size)
    .bind(b.chunk_count)
    .bind(b.sender_device)
    .bind(&b.recipients)
    .bind(if bootstrap { "bootstrap" } else { "media" })
    .bind(bootstrap) // a snapshot never counts against the photo allowance
    .execute(&state.db)
    .await?;
    Ok(Json(true))
}

#[derive(Serialize)]
pub struct Link {
    pub url: String,
}

/// A signed upload link for one chunk. The first upload of a file charges its size to this
/// month's media allowance (402 `media_quota` when it doesn't fit on the free tier).
pub async fn chunk_upload(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path((space, hash, n)): Path<(Uuid, String, i64)>) -> ApiResult<Json<Link>> {
    require_member(&state.db, space, user).await?;
    check_hash(&hash)?;
    let mut tx = state.db.begin().await?;
    let row: Option<(i64, i32, bool, Uuid)> = sqlx::query_as("select size, chunk_count, counted, sender_device from blobs where space_id = $1 and hash = $2 for update")
        .bind(space)
        .bind(&hash)
        .fetch_optional(&mut *tx)
        .await?;
    let (size, count, counted, sender) = row.ok_or_else(|| ApiError::not_found("file"))?;
    require_device(&mut *tx, space, user, sender).await?;
    if n < 0 || n >= i64::from(count) {
        return Err(ApiError::bad("no such chunk"));
    }
    if !counted {
        crate::plan::charge_media(&mut tx, &state.cfg, space, size).await?;
        sqlx::query("update blobs set counted = true where space_id = $1 and hash = $2").bind(space).bind(&hash).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(Json(Link { url: state.storage.upload_url(&object_key(&space, &hash, n)) }))
}

pub async fn chunk_download(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path((space, hash, n)): Path<(Uuid, String, i64)>) -> ApiResult<Json<Link>> {
    require_member(&state.db, space, user).await?;
    check_hash(&hash)?;
    let count: Option<i32> = sqlx::query_scalar("select chunk_count from blobs where space_id = $1 and hash = $2").bind(space).bind(&hash).fetch_optional(&state.db).await?;
    let count = count.ok_or_else(|| ApiError::not_found("file"))?;
    if n < 0 || n >= i64::from(count) {
        return Err(ApiError::bad("no such chunk"));
    }
    Ok(Json(Link { url: state.storage.download_url(&object_key(&space, &hash, n)) }))
}

pub async fn ack_blob(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path((space, hash)): Path<(Uuid, String)>, Json(b): Json<Ack>) -> ApiResult<Json<bool>> {
    require_member(&state.db, space, user).await?;
    require_device(&state.db, space, user, b.device).await?;
    check_hash(&hash)?;
    sqlx::query(
        "insert into blob_acks(space_id, hash, device_id)
         select space_id, hash, $3 from blobs where space_id = $1 and hash = $2 on conflict do nothing",
    )
    .bind(space)
    .bind(&hash)
    .bind(b.device)
    .execute(&state.db)
    .await?;
    Ok(Json(true))
}

async fn blob_delivered_inner(state: &AppState, space: Uuid, hash: &str) -> ApiResult<bool> {
    let v: Option<bool> = sqlx::query_scalar(
        "select all_acked(b.recipients, array(select device_id from blob_acks a where a.space_id = b.space_id and a.hash = b.hash))
           from blobs b where b.space_id = $1 and b.hash = $2",
    )
    .bind(space)
    .bind(hash)
    .fetch_optional(&state.db)
    .await?;
    Ok(v.unwrap_or(true))
}

pub async fn blob_delivered(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path((space, hash)): Path<(Uuid, String)>) -> ApiResult<Json<bool>> {
    require_member(&state.db, space, user).await?;
    check_hash(&hash)?;
    Ok(Json(blob_delivered_inner(&state, space, &hash).await?))
}

/// Deletes the stored chunks and the record, only once delivered. Returns true when gone.
pub async fn cleanup_blob(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path((space, hash)): Path<(Uuid, String)>) -> ApiResult<Json<bool>> {
    require_member(&state.db, space, user).await?;
    check_hash(&hash)?;
    if !blob_delivered_inner(&state, space, &hash).await? {
        return Ok(Json(false));
    }
    forget_blob(&state, space, &hash).await?;
    Ok(Json(true))
}

pub async fn forget_blob(state: &AppState, space: Uuid, hash: &str) -> ApiResult<()> {
    let count: Option<i32> = sqlx::query_scalar("select chunk_count from blobs where space_id = $1 and hash = $2").bind(space).bind(hash).fetch_optional(&state.db).await?;
    if let Some(count) = count {
        for n in 0..i64::from(count) {
            state.storage.delete(&state.http, &object_key(&space, hash, n)).await?;
        }
        sqlx::query("delete from blobs where space_id = $1 and hash = $2").bind(space).bind(hash).execute(&state.db).await?;
    }
    Ok(())
}

/// Bytes charged to this month's cloud media allowance (what the app compares to the cap).
pub async fn usage(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path(space): Path<Uuid>) -> ApiResult<Json<i64>> {
    require_member(&state.db, space, user).await?;
    Ok(Json(crate::plan::current(&state, space).await?.media_used_bytes))
}
