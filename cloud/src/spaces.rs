//! Spaces (one per couple), members (at most two people), devices and one-time invites.

use crate::auth::{sha256_hex, AuthUser};
use crate::error::{ApiError, ApiResult};
use crate::plan::{self, Plan};
use crate::AppState;
use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::PgExecutor;
use std::sync::Arc;
use uuid::Uuid;

pub async fn require_member<'e>(db: impl PgExecutor<'e>, space: Uuid, user: Uuid) -> ApiResult<()> {
    let ok: bool = sqlx::query_scalar("select exists(select 1 from members where space_id = $1 and user_id = $2)").bind(space).bind(user).fetch_one(db).await?;
    ok.then_some(()).ok_or_else(ApiError::forbidden)
}

/// The device belongs to this person, in this space, and hasn't been removed.
pub async fn require_device<'e>(db: impl PgExecutor<'e>, space: Uuid, user: Uuid, device: Uuid) -> ApiResult<()> {
    let ok: bool = sqlx::query_scalar("select exists(select 1 from devices where id = $1 and space_id = $2 and user_id = $3 and revoked_at is null)")
        .bind(device)
        .bind(space)
        .bind(user)
        .fetch_one(db)
        .await?;
    ok.then_some(()).ok_or_else(|| ApiError::new(axum::http::StatusCode::FORBIDDEN, "device_revoked", "This PC was removed from your space."))
}

#[derive(Deserialize)]
pub struct CreateSpace {
    pub id: Uuid,
}

pub async fn create(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Json(b): Json<CreateSpace>) -> ApiResult<Json<bool>> {
    let mut tx = state.db.begin().await?;
    let existing: Option<Uuid> = sqlx::query_scalar("select created_by from spaces where id = $1").bind(b.id).fetch_optional(&mut *tx).await?;
    match existing {
        Some(owner) if owner == user => {} // creating again (retry) is fine
        Some(_) => return Err(ApiError::forbidden()),
        None => {
            sqlx::query("insert into spaces(id, created_by) values ($1, $2)").bind(b.id).bind(user).execute(&mut *tx).await?;
        }
    }
    sqlx::query("insert into members(space_id, user_id) values ($1, $2) on conflict do nothing").bind(b.id).bind(user).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(true))
}

#[derive(Deserialize)]
pub struct RegisterDevice {
    pub id: Uuid,
    pub name: String,
}

pub async fn register_device(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path(space): Path<Uuid>, Json(b): Json<RegisterDevice>) -> ApiResult<Json<bool>> {
    require_member(&state.db, space, user).await?;
    let name: String = b.name.trim().chars().take(80).collect();
    let name = if name.is_empty() { "PC".to_string() } else { name };
    let r = sqlx::query(
        "insert into devices(id, space_id, user_id, name) values ($1, $2, $3, $4)
         on conflict (id) do update set name = excluded.name, last_seen = now()
           where devices.user_id = excluded.user_id and devices.space_id = excluded.space_id",
    )
    .bind(b.id)
    .bind(space)
    .bind(user)
    .bind(name)
    .execute(&state.db)
    .await?;
    if r.rows_affected() == 0 {
        return Err(ApiError::forbidden());
    }
    Ok(Json(true))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct DeviceOut {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub revoked: bool,
    pub lan_endpoint: Option<String>,
}

pub async fn devices(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path(space): Path<Uuid>) -> ApiResult<Json<Vec<DeviceOut>>> {
    require_member(&state.db, space, user).await?;
    let rows = sqlx::query_as::<_, DeviceOut>(
        "select id, user_id, name, revoked_at is not null as revoked, lan_endpoint from devices where space_id = $1 order by registered_at",
    )
    .bind(space)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

pub async fn revoke_device(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path((space, device)): Path<(Uuid, Uuid)>) -> ApiResult<Json<bool>> {
    require_member(&state.db, space, user).await?;
    sqlx::query("update devices set revoked_at = coalesce(revoked_at, now()) where id = $1 and space_id = $2").bind(device).bind(space).execute(&state.db).await?;
    Ok(Json(true))
}

#[derive(Deserialize)]
pub struct Endpoint {
    pub lan_endpoint: Option<String>,
}

/// Heartbeat + LAN address for direct PC-to-PC transfers.
pub async fn set_endpoint(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path((space, device)): Path<(Uuid, Uuid)>, Json(b): Json<Endpoint>) -> ApiResult<Json<bool>> {
    if b.lan_endpoint.as_ref().is_some_and(|e| e.len() > 60) {
        return Err(ApiError::bad("endpoint too long"));
    }
    let r = sqlx::query("update devices set lan_endpoint = $4, last_seen = now() where id = $1 and space_id = $2 and user_id = $3")
        .bind(device)
        .bind(space)
        .bind(user)
        .bind(b.lan_endpoint)
        .execute(&state.db)
        .await?;
    if r.rows_affected() == 0 {
        return Err(ApiError::forbidden());
    }
    Ok(Json(true))
}

#[derive(Deserialize)]
pub struct CreateInvite {
    pub secret_hash: String,
    /// "partner" (default): the other person, 48 h. "device": one more of your own devices,
    /// 15 minutes, and only your own account can use it.
    #[serde(default)]
    pub kind: Option<String>,
}

/// One-time, 48 hours. Only the hash of the secret is stored; the secret (and the space key)
/// travel in the invite code from PC to PC, never through this server.
pub async fn create_invite(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path(space): Path<Uuid>, Json(b): Json<CreateInvite>) -> ApiResult<Json<bool>> {
    require_member(&state.db, space, user).await?;
    if b.secret_hash.len() != 64 || !b.secret_hash.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(ApiError::bad("bad invite"));
    }
    let (kind, minutes) = match b.kind.as_deref().unwrap_or("partner") {
        "partner" => ("partner", 48 * 60),
        "device" => ("device", 15),
        _ => return Err(ApiError::bad("unknown invite kind")),
    };
    sqlx::query("insert into invites(secret_hash, space_id, created_by, expires_at, kind) values ($1, $2, $3, now() + make_interval(mins => $4), $5)")
        .bind(b.secret_hash.to_lowercase())
        .bind(space)
        .bind(user)
        .bind(minutes)
        .bind(kind)
        .execute(&state.db)
        .await?;
    Ok(Json(true))
}

#[derive(Deserialize)]
pub struct Redeem {
    pub secret: String,
}

#[derive(Serialize)]
pub struct Redeemed {
    pub space_id: Uuid,
    pub kind: String,
}

/// The partner joins. A space holds two people at most. Pairing starts the 30-day trial.
pub async fn redeem_invite(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Json(b): Json<Redeem>) -> ApiResult<Json<Redeemed>> {
    let mut tx = state.db.begin().await?;
    let found: Option<(Uuid, String, Uuid)> = sqlx::query_as(
        "update invites set used_by = $2, used_at = now()
          where secret_hash = $1 and used_by is null and expires_at > now()
          returning space_id, kind, created_by",
    )
    .bind(sha256_hex(b.secret.trim()))
    .bind(user)
    .fetch_optional(&mut *tx)
    .await?;
    let (space, kind, creator) = found.ok_or_else(|| ApiError::bad("This code is invalid, expired, or already used. Ask for a new one."))?;
    if kind == "device" {
        // linking one more of your own devices: never a way in for anyone else
        if creator != user {
            return Err(ApiError::bad("This is a device link code: sign in with the same account as on your other device."));
        }
        tx.commit().await?;
        return Ok(Json(Redeemed { space_id: space, kind }));
    }
    let others: i64 = sqlx::query_scalar("select count(*) from members where space_id = $1 and user_id <> $2").bind(space).bind(user).fetch_one(&mut *tx).await?;
    if others >= 2 {
        return Err(ApiError::bad("This space already has two people."));
    }
    sqlx::query("insert into members(space_id, user_id) values ($1, $2) on conflict do nothing").bind(space).bind(user).execute(&mut *tx).await?;
    let people: i64 = sqlx::query_scalar("select count(*) from members where space_id = $1").bind(space).fetch_one(&mut *tx).await?;
    if people >= 2 {
        sqlx::query("update spaces set trial_ends_at = coalesce(trial_ends_at, now() + make_interval(days => $2)) where id = $1")
            .bind(space)
            .bind(state.cfg.trial_days as i32)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(Redeemed { space_id: space, kind }))
}

pub async fn get_plan(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path(space): Path<Uuid>) -> ApiResult<Json<Plan>> {
    require_member(&state.db, space, user).await?;
    Ok(Json(plan::current(&state, space).await?))
}
