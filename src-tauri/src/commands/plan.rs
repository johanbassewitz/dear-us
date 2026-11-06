//! The couple's plan and Store purchases. Store edition only; in the personal edition these
//! commands exist but report that there's nothing to buy.

use crate::error::{AppError, AppResult};
use serde::Serialize;
use serde_json::Value;
use tauri::AppHandle;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseResult {
    /// purchased | already_owned | cancelled | failed | checkout_opened (Paddle, in the browser)
    pub status: String,
    pub message: Option<String>,
    pub plan: Option<Value>,
}

#[cfg_attr(not(feature = "store"), allow(dead_code))]
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> AppResult<T> + Send + 'static) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| AppError::invalid(e.to_string()))?
}

#[cfg(feature = "store")]
mod store {
    use super::*;
    use crate::sync::cloud::Plan;
    use crate::sync::service;
    use crate::AppState;
    use tauri::{Emitter, Manager};

    pub fn hwnd(app: &AppHandle) -> isize {
        #[cfg(windows)]
        {
            app.get_webview_window("main").and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize).unwrap_or(0)
        }
        #[cfg(not(windows))]
        {
            let _ = app;
            0
        }
    }

    pub fn publish(app: &AppHandle, plan: &Plan) -> Value {
        let state = app.state::<AppState>();
        let _ = service::save_plan(&state.db.lock().unwrap(), plan);
        let _ = app.emit("plan-changed", ());
        serde_json::to_value(plan).unwrap_or(Value::Null)
    }

    /// Opens a web page (Paddle checkout, the subscription page) in the person's browser.
    pub fn open_in_browser(app: &AppHandle, url: &str) -> AppResult<()> {
        use tauri_plugin_opener::OpenerExt;
        if !url.starts_with("https://") {
            return Err(AppError::invalid("unexpected link"));
        }
        app.opener().open_url(url, None::<&str>).map_err(|e| AppError::invalid(format!("Couldn't open your browser: {e}")))
    }

    pub fn paired(app: &AppHandle) -> bool {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        service::load_config(&conn).ok().flatten().and_then(|c| c.space).is_some()
    }

    /// Asks the Store for this Microsoft account's proofs and lets the server apply them.
    pub fn sync_entitlements(app: &AppHandle) -> AppResult<Plan> {
        let relay = service::connect(app)?;
        let t = relay.store_tickets()?;
        let keys = crate::billing::id_keys(hwnd(app), &t.collections_ticket, &t.purchase_ticket, &t.publisher_user_id)?;
        let plan = relay.store_sync(&keys.collections, &keys.purchase)?;
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        crate::sync::apply::meta_set(&conn, "billing.last_sync", &crate::db::now_iso())?;
        Ok(plan)
    }

    /// Renewals and cancellations: the PC that bought something re-checks once a day.
    pub fn daily(app: &AppHandle) {
        let due = {
            let state = app.state::<AppState>();
            let conn = state.db.lock().unwrap();
            let get = |k: &str| conn.query_row("SELECT value FROM local_meta WHERE key = ?1", [k], |r| r.get::<_, String>(0)).ok();
            get("billing.purchased").is_some()
                && get("billing.last_sync")
                    .and_then(|t| chrono::DateTime::parse_from_rfc3339(&t).ok())
                    .map(|t| chrono::Utc::now().signed_duration_since(t) > chrono::Duration::hours(24))
                    .unwrap_or(true)
        };
        if due {
            if let Ok(p) = sync_entitlements(app) {
                publish(app, &p);
            }
        }
    }
}

#[cfg(feature = "store")]
pub use store::daily as daily_billing_check;

/// The plan as last known; `refresh` asks the server first (when connected).
#[tauri::command]
pub async fn plan_get(app: AppHandle, refresh: bool) -> AppResult<Option<Value>> {
    #[cfg(feature = "store")]
    {
        use tauri::Manager;
        if refresh && store::paired(&app) {
            let a = app.clone();
            if let Ok(p) = blocking(move || crate::sync::service::connect(&a)?.plan()).await {
                return Ok(Some(store::publish(&app, &p)));
            }
        }
        let state = app.state::<crate::AppState>();
        let conn = state.db.lock().unwrap();
        Ok(crate::sync::service::cached_plan(&conn).and_then(|p| serde_json::to_value(p).ok()))
    }
    #[cfg(not(feature = "store"))]
    {
        let _ = (app, refresh);
        Ok(None)
    }
}

/// Opens the Store's purchase dialog for `product` (premium_monthly | premium_yearly |
/// arrows_100 | arrows_300), then has the server confirm it with Microsoft.
#[tauri::command]
pub async fn billing_purchase(app: AppHandle, product: String) -> AppResult<PurchaseResult> {
    #[cfg(feature = "store")]
    {
        if !store::paired(&app) {
            return Err(AppError::invalid("Connect with your partner first (Settings → Partner sync), so the purchase covers you both."));
        }
        let a = app.clone();
        blocking(move || {
            let relay = crate::sync::service::connect(&a)?;
            let plan = relay.plan()?;
            if plan.billing_provider.as_deref() == Some("paddle") {
                // Paddle: checkout in the browser; the server hears about the payment directly
                let url = relay.paddle_checkout(&product)?;
                store::open_in_browser(&a, &url)?;
                return Ok(PurchaseResult { status: "checkout_opened".into(), message: None, plan: Some(store::publish(&a, &plan)) });
            }
            let tickets = relay.store_tickets()?;
            let id = crate::billing::store_id(&tickets.products, &product)?;
            let outcome = crate::billing::purchase(store::hwnd(&a), &id)?;
            let mut res = PurchaseResult { status: outcome.as_str().into(), message: None, plan: None };
            match outcome {
                crate::billing::Outcome::Purchased | crate::billing::Outcome::AlreadyOwned => {
                    {
                        use tauri::Manager;
                        let state = a.state::<crate::AppState>();
                        crate::sync::apply::meta_set(&state.db.lock().unwrap(), "billing.purchased", "1")?;
                    }
                    let plan = store::sync_entitlements(&a)?;
                    res.plan = Some(store::publish(&a, &plan));
                }
                crate::billing::Outcome::Failed(m) => res.message = Some(m),
                crate::billing::Outcome::Cancelled => {}
            }
            Ok(res)
        })
        .await
    }
    #[cfg(not(feature = "store"))]
    {
        let _ = (app, product);
        Err(AppError::invalid("There's nothing to buy in this edition of Dear Us."))
    }
}

/// "Manage subscription" (Paddle): opens the page to cancel, change the card or see invoices.
#[tauri::command]
pub async fn billing_manage(app: AppHandle) -> AppResult<()> {
    #[cfg(feature = "store")]
    {
        let a = app.clone();
        return blocking(move || {
            let url = crate::sync::service::connect(&a)?.paddle_portal()?;
            store::open_in_browser(&a, &url)
        })
        .await;
    }
    #[cfg(not(feature = "store"))]
    {
        let _ = app;
        Err(AppError::invalid("There's nothing to manage in this edition of Dear Us."))
    }
}

/// "Restore purchases": re-reads this Microsoft account's subscriptions and packs.
#[tauri::command]
pub async fn billing_restore(app: AppHandle) -> AppResult<Option<Value>> {
    #[cfg(feature = "store")]
    {
        let a = app.clone();
        let plan = blocking(move || store::sync_entitlements(&a)).await?;
        {
            use tauri::Manager;
            let state = app.state::<crate::AppState>();
            crate::sync::apply::meta_set(&state.db.lock().unwrap(), "billing.purchased", "1")?;
        }
        Ok(Some(store::publish(&app, &plan)))
    }
    #[cfg(not(feature = "store"))]
    {
        let _ = app;
        Err(AppError::invalid("There's nothing to restore in this edition of Dear Us."))
    }
}
