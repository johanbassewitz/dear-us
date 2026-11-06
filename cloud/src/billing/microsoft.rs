//! Microsoft Store purchases, verified server-side (the app is never trusted to say "I paid").
//!
//! Flow (Microsoft "Store services" B2B APIs):
//! 1. The app asks us for two short-lived Azure AD tickets (`/tickets`).
//! 2. The app calls `StoreContext.GetCustomerCollectionsIdAsync` / `GetCustomerPurchaseIdAsync`
//!    with them and sends us the resulting Microsoft Store ID keys (`/sync`).
//! 3. We query the subscription (recurrence) and consumable (collections) APIs with those keys,
//!    update the couple's plan, credit arrow packs exactly once and report them as fulfilled.
//!
//! Endpoints/field names follow Microsoft's docs ("Manage products from your services");
//! verify them against a sandbox before launch (see docs/STORE_RELEASE.md).

use crate::auth::AuthUser;
use crate::config::MicrosoftStore;
use crate::error::{ApiError, ApiResult};
use crate::plan::{self, Plan};
use crate::spaces::require_member;
use crate::AppState;
use axum::extract::{Path, State};
use axum::Json;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const AUD_SERVICE: &str = "https://onestore.microsoft.com";
const AUD_COLLECTIONS: &str = "https://onestore.microsoft.com/b2b/keys/create/collections";
const AUD_PURCHASE: &str = "https://onestore.microsoft.com/b2b/keys/create/purchase";
const COLLECTIONS_QUERY: &str = "https://collections.mp.microsoft.com/v6.0/collections/query";
const COLLECTIONS_CONSUME: &str = "https://collections.mp.microsoft.com/v6.0/collections/consume";
const RECURRENCES_QUERY: &str = "https://purchase.mp.microsoft.com/v8.0/b2b/recurrences/query";
/// A renewal can take a moment to show up: keep Premium a few days past the paid period.
const GRACE_DAYS: i64 = 3;

#[derive(Default)]
pub struct TokenCache(Mutex<HashMap<String, (String, DateTime<Utc>)>>);

fn ms(state: &AppState) -> ApiResult<&MicrosoftStore> {
    state.cfg.microsoft.as_ref().ok_or_else(|| ApiError::unavailable("Purchases aren't available yet."))
}

async fn aad_token(state: &AppState, audience: &str) -> ApiResult<String> {
    if let Some((t, exp)) = state.ms_tokens.0.lock().unwrap().get(audience) {
        if *exp > Utc::now() + Duration::minutes(5) {
            return Ok(t.clone());
        }
    }
    let m = ms(state)?;
    let r = state
        .http
        .post(format!("https://login.microsoftonline.com/{}/oauth2/token", m.tenant_id))
        .form(&[("grant_type", "client_credentials"), ("client_id", &m.client_id), ("client_secret", &m.client_secret), ("resource", audience)])
        .send()
        .await
        .map_err(ApiError::internal)?;
    if !r.status().is_success() {
        return Err(ApiError::internal(format!("Azure AD token request failed: {}", r.status())));
    }
    let v: Value = r.json().await.map_err(ApiError::internal)?;
    let token = v["access_token"].as_str().ok_or_else(|| ApiError::internal("no access_token from Azure AD"))?.to_string();
    let secs = v["expires_in"].as_str().and_then(|s| s.parse().ok()).or_else(|| v["expires_in"].as_i64()).unwrap_or(3600);
    state.ms_tokens.0.lock().unwrap().insert(audience.into(), (token.clone(), Utc::now() + Duration::seconds(secs)));
    Ok(token)
}

#[derive(Serialize)]
pub struct Products {
    pub premium_monthly: String,
    pub premium_yearly: String,
    pub arrows_100: String,
    pub arrows_300: Option<String>,
}

#[derive(Serialize)]
pub struct Tickets {
    pub collections_ticket: String,
    pub purchase_ticket: String,
    pub publisher_user_id: String,
    pub products: Products,
}

pub async fn tickets(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser) -> ApiResult<Json<Tickets>> {
    let m = ms(&state)?.clone();
    Ok(Json(Tickets {
        collections_ticket: aad_token(&state, AUD_COLLECTIONS).await?,
        purchase_ticket: aad_token(&state, AUD_PURCHASE).await?,
        publisher_user_id: user.to_string(),
        products: Products { premium_monthly: m.premium_monthly, premium_yearly: m.premium_yearly, arrows_100: m.arrows_100, arrows_300: m.arrows_300 },
    }))
}

#[derive(Deserialize)]
pub struct SyncIn {
    pub collections_id_key: String,
    pub purchase_id_key: String,
}

/// What a product id means for us.
pub enum Product {
    Premium(&'static str),
    Arrows(&'static str, i32),
}

pub fn classify(m: &MicrosoftStore, product_id: &str) -> Option<Product> {
    let p = product_id.trim();
    if p.eq_ignore_ascii_case(&m.premium_monthly) {
        Some(Product::Premium("premium_monthly"))
    } else if p.eq_ignore_ascii_case(&m.premium_yearly) {
        Some(Product::Premium("premium_yearly"))
    } else if p.eq_ignore_ascii_case(&m.arrows_100) {
        Some(Product::Arrows("arrows_100", 100))
    } else if m.arrows_300.as_deref().is_some_and(|a| p.eq_ignore_ascii_case(a)) {
        Some(Product::Arrows("arrows_300", 300))
    } else {
        None
    }
}

/// Subscriptions that count: still paid for (cancelled ones run until they expire).
pub fn subscription_counts(state_str: &str) -> bool {
    !matches!(state_str.to_ascii_lowercase().as_str(), "refunded" | "revoked" | "chargeback")
}

pub async fn sync(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser, Path(space): Path<Uuid>, Json(b): Json<SyncIn>) -> ApiResult<Json<Plan>> {
    require_member(&state.db, space, user).await?;
    let m = ms(&state)?.clone();
    let token = aad_token(&state, AUD_SERVICE).await?;

    // ---- subscriptions
    let r = state.http.post(RECURRENCES_QUERY).bearer_auth(&token).json(&json!({ "b2bKey": b.purchase_id_key })).send().await.map_err(ApiError::internal)?;
    if !r.status().is_success() {
        return Err(ApiError::new(axum::http::StatusCode::BAD_GATEWAY, "store_error", "The Microsoft Store couldn't confirm your purchases right now. Try again in a bit."));
    }
    let v: Value = r.json().await.map_err(ApiError::internal)?;
    for item in v["items"].as_array().cloned().unwrap_or_default() {
        let Some(Product::Premium(product)) = item["productId"].as_str().and_then(|p| classify(&m, p)) else { continue };
        let (Some(id), Some(exp)) = (item["id"].as_str(), item["expirationTime"].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok())) else { continue };
        let st = item["recurrenceState"].as_str().unwrap_or("Unknown");
        sqlx::query(
            "insert into subscriptions(id, store, space_id, user_id, product, state, expires_at) values ($1, 'microsoft', $2, $3, $4, $5, $6)
             on conflict (id) do update set space_id = excluded.space_id, user_id = excluded.user_id, state = excluded.state,
               expires_at = excluded.expires_at, updated_at = now()",
        )
        .bind(id)
        .bind(space)
        .bind(user)
        .bind(product)
        .bind(st)
        .bind(exp.with_timezone(&Utc))
        .execute(&state.db)
        .await?;
    }
    refresh_premium(&state, space).await?;

    // ---- arrow packs (developer-managed consumables)
    let beneficiary = json!({ "identitytype": "b2b", "identityValue": b.collections_id_key, "localTicketReference": user.to_string() });
    let r = state
        .http
        .post(COLLECTIONS_QUERY)
        .bearer_auth(&token)
        .json(&json!({ "beneficiaries": [beneficiary], "productTypes": ["UnmanagedConsumable"], "validityType": "Valid" }))
        .send()
        .await
        .map_err(ApiError::internal)?;
    if r.status().is_success() {
        let v: Value = r.json().await.map_err(ApiError::internal)?;
        for item in v["items"].as_array().cloned().unwrap_or_default() {
            let Some(product_id) = item["productId"].as_str() else { continue };
            let Some(Product::Arrows(product, arrows)) = classify(&m, product_id) else { continue };
            let Some(tx_id) = item["transactionId"].as_str().or_else(|| item["orderId"].as_str()) else { continue };
            let qty = item["quantity"].as_i64().unwrap_or(1).clamp(1, 100) as i32;
            // credit exactly once per store transaction, then tell the Store it's fulfilled
            let mut tx = state.db.begin().await?;
            let inserted = sqlx::query(
                "insert into purchases(transaction_id, store, space_id, user_id, product, arrows) values ($1, 'microsoft', $2, $3, $4, $5) on conflict do nothing",
            )
            .bind(tx_id)
            .bind(space)
            .bind(user)
            .bind(product)
            .bind(arrows * qty)
            .execute(&mut *tx)
            .await?
            .rows_affected()
                == 1;
            if inserted {
                sqlx::query("update spaces set cupid_bonus = cupid_bonus + $2 where id = $1").bind(space).bind(arrows * qty).execute(&mut *tx).await?;
            }
            tx.commit().await?;
            let consumed = state
                .http
                .post(COLLECTIONS_CONSUME)
                .bearer_auth(&token)
                .json(&json!({ "beneficiary": beneficiary, "itemId": item["id"], "trackingId": Uuid::new_v4().to_string(), "productId": product_id }))
                .send()
                .await;
            if let Err(e) = consumed {
                tracing::warn!("couldn't report fulfilment of {tx_id}: {e} (will retry next sync; arrows aren't credited twice)");
            }
        }
    } else {
        tracing::warn!("collections query failed: {}", r.status());
    }
    Ok(Json(plan::current(&state, space).await?))
}

/// premium_until = the latest paid-through date of any counting subscription for the space.
pub async fn refresh_premium(state: &AppState, space: Uuid) -> ApiResult<()> {
    let rows: Vec<(String, String, DateTime<Utc>)> = sqlx::query_as("select store, state, expires_at from subscriptions where space_id = $1").bind(space).fetch_all(&state.db).await?;
    let until = rows
        .into_iter()
        .filter_map(|(store, st, e)| match store.as_str() {
            // Paddle: active/trialing/past_due run to the end of the paid period (+1 day for a renewal to land)
            "paddle" => matches!(st.as_str(), "active" | "trialing" | "past_due").then_some(e + Duration::days(1)),
            _ => subscription_counts(&st).then_some(e + Duration::days(GRACE_DAYS)),
        })
        .max();
    sqlx::query("update spaces set premium_until = $2 where id = $1").bind(space).bind(until).execute(&state.db).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> MicrosoftStore {
        MicrosoftStore {
            tenant_id: "t".into(),
            client_id: "c".into(),
            client_secret: "s".into(),
            premium_monthly: "9NPREMIUMMONTH".into(),
            premium_yearly: "9NPREMIUMYEAR".into(),
            arrows_100: "9NARROWS100".into(),
            arrows_300: None,
        }
    }

    #[test]
    fn products_map_to_entitlements() {
        let m = cfg();
        assert!(matches!(classify(&m, "9npremiummonth"), Some(Product::Premium("premium_monthly"))));
        assert!(matches!(classify(&m, "9NARROWS100"), Some(Product::Arrows(_, 100))));
        assert!(classify(&m, "9NSOMETHINGELSE").is_none());
    }

    #[test]
    fn cancelled_subscriptions_run_until_expiry() {
        assert!(subscription_counts("Active"));
        assert!(subscription_counts("Canceled"));
        assert!(!subscription_counts("Refunded"));
    }
}
