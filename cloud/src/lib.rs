//! Dear Us cloud: accounts, the encrypted ephemeral mailbox, plans/quotas, the Cupid gateway
//! and Store purchase verification. It never sees a memory: the couple's space key only exists
//! on their PCs.

pub mod auth;
pub mod billing;
pub mod config;
pub mod cupid;
pub mod error;
pub mod plan;
pub mod relay;
pub mod spaces;
pub mod storage;

use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;

pub struct AppState {
    pub db: PgPool,
    pub cfg: config::Config,
    pub storage: storage::Storage,
    pub http: reqwest::Client,
    pub limiter: auth::Limiter,
    pub ms_tokens: billing::microsoft::TokenCache,
    /// Verified against when an email doesn't exist, so sign-in timing reveals nothing.
    pub dummy_hash: String,
}

pub async fn build_state(cfg: config::Config) -> Result<Arc<AppState>, String> {
    let db = PgPoolOptions::new().max_connections(20).acquire_timeout(Duration::from_secs(10)).connect(&cfg.database_url).await.map_err(|e| format!("database: {e}"))?;
    sqlx::migrate!("./migrations").run(&db).await.map_err(|e| format!("migrations: {e}"))?;
    let storage = storage::Storage::from_config(&cfg)?;
    let http = reqwest::Client::builder().timeout(Duration::from_secs(90)).connect_timeout(Duration::from_secs(15)).build().map_err(|e| e.to_string())?;
    let dummy_hash = auth::hash_password(auth::random_token()).await.map_err(|e| e.message)?;
    Ok(Arc::new(AppState { db, cfg, storage, http, limiter: Default::default(), ms_tokens: Default::default(), dummy_hash }))
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "service": "dear-us-cloud", "version": 1 }))
}

pub fn router(state: Arc<AppState>) -> Router {
    use relay as r;
    use spaces as s;
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/auth/signup", post(auth::signup))
        .route("/v1/auth/signin", post(auth::signin))
        .route("/v1/auth/refresh", post(auth::refresh))
        .route("/v1/auth/signout", post(auth::signout))
        .route("/v1/spaces", post(s::create))
        .route("/v1/invites/redeem", post(s::redeem_invite))
        .route("/v1/spaces/{space}/devices", get(s::devices).post(s::register_device))
        .route("/v1/spaces/{space}/devices/{device}", axum::routing::patch(s::set_endpoint))
        .route("/v1/spaces/{space}/devices/{device}/revoke", post(s::revoke_device))
        .route("/v1/spaces/{space}/invites", post(s::create_invite))
        .route("/v1/spaces/{space}/plan", get(s::get_plan))
        .route("/v1/spaces/{space}/usage", get(r::usage))
        .route("/v1/spaces/{space}/envelopes", get(r::fetch_envelopes).post(r::put_envelope))
        .route("/v1/spaces/{space}/envelopes/{id}", delete(r::cleanup_envelope))
        .route("/v1/spaces/{space}/envelopes/{id}/ack", post(r::ack_envelope))
        .route("/v1/spaces/{space}/envelopes/{id}/delivered", get(r::envelope_delivered))
        .route("/v1/spaces/{space}/blobs/{hash}", get(r::get_blob).put(r::put_blob).delete(r::cleanup_blob))
        .route("/v1/spaces/{space}/blobs/{hash}/ack", post(r::ack_blob))
        .route("/v1/spaces/{space}/blobs/{hash}/delivered", get(r::blob_delivered))
        .route("/v1/spaces/{space}/blobs/{hash}/chunks/{n}/upload", post(r::chunk_upload))
        .route("/v1/spaces/{space}/blobs/{hash}/chunks/{n}/download", post(r::chunk_download))
        .route("/v1/spaces/{space}/cupid", post(cupid::ask))
        .route("/v1/billing/microsoft/tickets", post(billing::microsoft::tickets))
        .route("/v1/spaces/{space}/billing/microsoft/sync", post(billing::microsoft::sync))
        .route("/v1/local-chunks/{*key}", put(storage::local_put).get(storage::local_get))
        // envelopes can be large (a batch of ops); chunks go to R2 directly in production
        .layer(DefaultBodyLimit::max(24 * 1024 * 1024))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state)
}

/// Removes mailbox items nobody picked up within `MAILBOX_TTL_DAYS`, expired invites and
/// refresh tokens. The sender's PC still has every original; it re-sends if ever needed.
pub async fn sweep(state: &AppState) -> Result<(), error::ApiError> {
    let days = state.cfg.mailbox_ttl_days as i32;
    sqlx::query("delete from envelopes where created_at < now() - make_interval(days => $1)").bind(days).execute(&state.db).await?;
    let old: Vec<(uuid::Uuid, String)> =
        sqlx::query_as("select space_id, hash from blobs where created_at < now() - make_interval(days => $1) limit 500").bind(days).fetch_all(&state.db).await?;
    for (space, hash) in old {
        relay::forget_blob(state, space, &hash).await?;
    }
    sqlx::query("delete from invites where expires_at < now() - interval '7 days'").execute(&state.db).await?;
    sqlx::query("delete from refresh_tokens where expires_at < now()").execute(&state.db).await?;
    Ok(())
}
