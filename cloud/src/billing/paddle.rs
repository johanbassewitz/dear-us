//! Paddle (merchant of record): the app opens Paddle's checkout in the browser; Paddle tells us
//! about payments with signed webhooks; we update the couple's plan. The app is never trusted
//! to say "I paid".
//!
//! Setup (Paddle dashboard): products/prices (env PADDLE_PRICE_*), an API key, a webhook
//! destination pointing at `/v1/billing/paddle/webhook` (its secret = PADDLE_WEBHOOK_SECRET),
//! and Checkout → default payment link = the website's checkout page (site/checkout.html).

use crate::auth::AuthUser;
use crate::config::Paddle;
use crate::error::{ApiError, ApiResult};
use crate::plan;
use crate::spaces::require_member;
use crate::AppState;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;
use std::sync::Arc;
use uuid::Uuid;

fn paddle(state: &AppState) -> ApiResult<&Paddle> {
    state.cfg.paddle.as_ref().ok_or_else(|| ApiError::unavailable("Purchases aren't available yet."))
}

pub enum Product {
    Premium(&'static str),
    Arrows(&'static str, i32),
}

pub fn classify(p: &Paddle, price_id: &str) -> Option<Product> {
    if price_id == p.price_premium_monthly {
        Some(Product::Premium("premium_monthly"))
    } else if price_id == p.price_premium_yearly {
        Some(Product::Premium("premium_yearly"))
    } else if price_id == p.price_arrows_100 {
        Some(Product::Arrows("arrows_100", 100))
    } else if p.price_arrows_300.as_deref() == Some(price_id) {
        Some(Product::Arrows("arrows_300", 300))
    } else {
        None
    }
}

fn price_for(p: &Paddle, product: &str) -> ApiResult<String> {
    match product {
        "premium_monthly" => Ok(p.price_premium_monthly.clone()),
        "premium_yearly" => Ok(p.price_premium_yearly.clone()),
        "arrows_100" => Ok(p.price_arrows_100.clone()),
        "arrows_300" => p.price_arrows_300.clone().ok_or_else(|| ApiError::bad("That pack isn't available.")),
        _ => Err(ApiError::bad("Unknown product.")),
    }
}

#[derive(Deserialize)]
pub struct CheckoutIn {
    pub product: String,
}

#[derive(Serialize)]
pub struct Link {
    pub url: String,
}

/// Creates a Paddle transaction tagged with the couple's space and returns its checkout URL
/// (the website's checkout page, where Paddle's overlay opens for this transaction).
pub async fn checkout(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path(space): Path<Uuid>, Json(b): Json<CheckoutIn>) -> ApiResult<Json<Link>> {
    require_member(&state.db, space, user).await?;
    let p = paddle(&state)?;
    let price = price_for(p, &b.product)?;
    if matches!(b.product.as_str(), "premium_monthly" | "premium_yearly") && plan::current(&state, space).await?.tier == plan::Tier::Premium {
        return Err(ApiError::bad("You already have Premium ♡ Manage it from Settings → Your plan."));
    }
    let r = state
        .http
        .post(format!("{}/transactions", p.api_base))
        .bearer_auth(&p.api_key)
        .json(&json!({
            "items": [{ "price_id": price, "quantity": 1 }],
            "custom_data": { "space_id": space, "user_id": user },
        }))
        .send()
        .await
        .map_err(ApiError::internal)?;
    let status = r.status();
    let v: Value = r.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        tracing::error!("paddle transaction failed ({status}): {}", v["error"]["detail"].as_str().unwrap_or_default());
        return Err(ApiError::new(StatusCode::BAD_GATEWAY, "billing_error", "Checkout couldn't start right now. Try again in a minute."));
    }
    let url = v["data"]["checkout"]["url"].as_str().ok_or_else(|| ApiError::internal("Paddle returned no checkout URL: set a default payment link in Paddle → Checkout settings"))?.to_string();
    Ok(Json(Link { url }))
}

/// "Manage subscription": a Paddle customer-portal link (cancel, change card, invoices).
pub async fn portal(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path(space): Path<Uuid>) -> ApiResult<Json<Link>> {
    require_member(&state.db, space, user).await?;
    let p = paddle(&state)?;
    let row: Option<(String, Option<String>)> =
        sqlx::query_as("select id, customer_id from subscriptions where space_id = $1 and store = 'paddle' and customer_id is not null order by expires_at desc limit 1")
            .bind(space)
            .fetch_optional(&state.db)
            .await?;
    let Some((sub, Some(customer))) = row else { return Err(ApiError::bad("There's no subscription to manage yet.")) };
    let r = state
        .http
        .post(format!("{}/customers/{customer}/portal-sessions", p.api_base))
        .bearer_auth(&p.api_key)
        .json(&json!({ "subscription_ids": [sub] }))
        .send()
        .await
        .map_err(ApiError::internal)?;
    let v: Value = r.json().await.map_err(ApiError::internal)?;
    let url = v["data"]["urls"]["general"]["overview"].as_str().ok_or_else(|| ApiError::new(StatusCode::BAD_GATEWAY, "billing_error", "The subscription page couldn't open right now. Try again in a minute."))?;
    Ok(Json(Link { url: url.to_string() }))
}

/// `Paddle-Signature: ts=<unix>;h1=<hex hmac-sha256 of "<ts>:<raw body>">`
pub fn signature_ok(secret: &str, header: &str, body: &[u8], now: i64) -> bool {
    let mut ts = None;
    let mut sigs = Vec::new();
    for part in header.split(';') {
        match part.split_once('=') {
            Some(("ts", v)) => ts = v.trim().parse::<i64>().ok(),
            Some(("h1", v)) => sigs.push(v.trim().to_string()),
            _ => {}
        }
    }
    let Some(ts) = ts else { return false };
    if (now - ts).abs() > 300 {
        return false; // replayed or badly delayed
    }
    let mut signed = format!("{ts}:").into_bytes();
    signed.extend_from_slice(body);
    sigs.iter().any(|s| {
        let Ok(raw) = hex::decode(s) else { return false };
        let mut m = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("hmac key");
        m.update(&signed);
        m.verify_slice(&raw).is_ok()
    })
}

fn custom(v: &Value, key: &str) -> Option<Uuid> {
    v["custom_data"][key].as_str().and_then(|s| Uuid::parse_str(s).ok())
}

/// Paddle's notifications. Answers 200 for anything we don't need, so Paddle doesn't retry.
pub async fn webhook(State(state): State<Arc<AppState>>, headers: HeaderMap, body: Bytes) -> ApiResult<StatusCode> {
    let p = paddle(&state)?;
    let sig = headers.get("paddle-signature").and_then(|h| h.to_str().ok()).unwrap_or_default();
    if !signature_ok(&p.webhook_secret, sig, &body, Utc::now().timestamp()) {
        return Err(ApiError::new(StatusCode::UNAUTHORIZED, "bad_signature", "bad signature"));
    }
    let v: Value = serde_json::from_slice(&body).map_err(|_| ApiError::bad("bad json"))?;
    let kind = v["event_type"].as_str().unwrap_or_default();
    let d = &v["data"];
    if kind.starts_with("subscription.") {
        subscription_event(&state, p, d).await?;
    } else if kind == "transaction.completed" {
        transaction_completed(&state, p, d).await?;
    }
    Ok(StatusCode::OK)
}

async fn subscription_event(state: &AppState, p: &Paddle, d: &Value) -> ApiResult<()> {
    let Some(id) = d["id"].as_str() else { return Ok(()) };
    let product = d["items"].as_array().into_iter().flatten().filter_map(|i| i["price"]["id"].as_str()).find_map(|price| match classify(p, price) {
        Some(Product::Premium(name)) => Some(name),
        _ => None,
    });
    let Some(product) = product else { return Ok(()) };
    // the space comes from the checkout's custom data (Paddle copies it to the subscription)
    let known: Option<(Uuid, Uuid)> = sqlx::query_as("select space_id, user_id from subscriptions where id = $1").bind(id).fetch_optional(&state.db).await?;
    let (space, user) = match (custom(d, "space_id"), custom(d, "user_id"), known) {
        (Some(s), Some(u), _) => (s, u),
        (_, _, Some(k)) => k,
        _ => {
            tracing::warn!("paddle subscription {id} without space: ignored");
            return Ok(());
        }
    };
    let status = d["status"].as_str().unwrap_or("unknown");
    let ends = d["current_billing_period"]["ends_at"].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&Utc));
    let active = matches!(status, "active" | "trialing" | "past_due");
    let expires = if active { ends.unwrap_or_else(Utc::now) } else { Utc::now() };
    let space_exists: bool = sqlx::query_scalar("select exists(select 1 from spaces where id = $1)").bind(space).fetch_one(&state.db).await?;
    let user_exists: bool = sqlx::query_scalar("select exists(select 1 from users where id = $1)").bind(user).fetch_one(&state.db).await?;
    if !space_exists || !user_exists {
        tracing::warn!("paddle subscription {id} for a space/account that no longer exists: ignored");
        return Ok(());
    }
    sqlx::query(
        "insert into subscriptions(id, store, space_id, user_id, product, state, expires_at, customer_id) values ($1, 'paddle', $2, $3, $4, $5, $6, $7)
         on conflict (id) do update set space_id = excluded.space_id, user_id = excluded.user_id, state = excluded.state,
           expires_at = excluded.expires_at, product = excluded.product,
           customer_id = coalesce(excluded.customer_id, subscriptions.customer_id), updated_at = now()",
    )
    .bind(id)
    .bind(space)
    .bind(user)
    .bind(product)
    .bind(status)
    .bind(expires)
    .bind(d["customer_id"].as_str())
    .execute(&state.db)
    .await?;
    if let Some((old, _)) = known.filter(|(old, _)| *old != space) {
        super::microsoft::refresh_premium(state, old).await?;
    }
    super::microsoft::refresh_premium(state, space).await
}

async fn transaction_completed(state: &AppState, p: &Paddle, d: &Value) -> ApiResult<()> {
    let Some(tx_id) = d["id"].as_str() else { return Ok(()) };
    let (Some(space), Some(user)) = (custom(d, "space_id"), custom(d, "user_id")) else { return Ok(()) };
    let mut arrows = 0;
    let mut product = "";
    for item in d["items"].as_array().into_iter().flatten() {
        if let Some(Product::Arrows(name, n)) = item["price"]["id"].as_str().and_then(|price| classify(p, price)) {
            arrows += n * item["quantity"].as_i64().unwrap_or(1).clamp(1, 100) as i32;
            product = name;
        }
    }
    if arrows == 0 {
        return Ok(()); // a subscription payment: handled by the subscription events
    }
    let mut tx = state.db.begin().await?;
    let exists: bool = sqlx::query_scalar("select exists(select 1 from spaces where id = $1)").bind(space).fetch_one(&mut *tx).await?;
    if !exists {
        return Ok(());
    }
    // exactly once per Paddle transaction, however often Paddle retries the webhook
    let new = sqlx::query("insert into purchases(transaction_id, store, space_id, user_id, product, arrows) values ($1, 'paddle', $2, $3, $4, $5) on conflict do nothing")
        .bind(tx_id)
        .bind(space)
        .bind(user)
        .bind(product)
        .bind(arrows)
        .execute(&mut *tx)
        .await?
        .rows_affected()
        == 1;
    if new {
        sqlx::query("update spaces set cupid_bonus = cupid_bonus + $2 where id = $1").bind(space).bind(arrows).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_paddle_signatures() {
        let body = br#"{"event_type":"transaction.completed"}"#;
        let ts = 1_700_000_000;
        let mut m = Hmac::<Sha256>::new_from_slice(b"pdl_ntfset_secret").unwrap();
        m.update(format!("{ts}:").as_bytes());
        m.update(body);
        let h1 = hex::encode(m.finalize().into_bytes());
        let header = format!("ts={ts};h1={h1}");
        assert!(signature_ok("pdl_ntfset_secret", &header, body, ts + 10));
        assert!(!signature_ok("another", &header, body, ts + 10), "wrong secret");
        assert!(!signature_ok("pdl_ntfset_secret", &header, b"{}", ts + 10), "changed body");
        assert!(!signature_ok("pdl_ntfset_secret", &header, body, ts + 3600), "too old");
    }
}
