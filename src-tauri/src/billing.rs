//! Store edition: Microsoft Store purchases (Premium subscriptions, Cupid arrow packs).
//!
//! The app only shows the Store's own purchase dialog and hands Microsoft's proof to the Dear
//! Us cloud; the server checks it with Microsoft and updates the couple's plan
//! (cloud/src/billing/microsoft.rs). A purchase by either partner covers both.

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum Outcome {
    Purchased,
    AlreadyOwned,
    Cancelled,
    Failed(String),
}

impl Outcome {
    pub fn as_str(&self) -> &str {
        match self {
            Outcome::Purchased => "purchased",
            Outcome::AlreadyOwned => "already_owned",
            Outcome::Cancelled => "cancelled",
            Outcome::Failed(_) => "failed",
        }
    }
}

/// Microsoft Store ID keys for the signed-in Microsoft account (90-day proofs the server
/// redeems with Microsoft).
pub struct IdKeys {
    pub collections: String,
    pub purchase: String,
}

#[cfg(windows)]
mod windows_store {
    use super::{IdKeys, Outcome};
    use crate::error::{AppError, AppResult};
    use windows::core::{Interface, HSTRING};
    use windows::Services::Store::{StoreContext, StorePurchaseStatus};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::IInitializeWithWindow;

    fn err(e: windows::core::Error) -> AppError {
        AppError::invalid(format!("The Microsoft Store didn't respond: {}", e.message()))
    }

    /// A desktop app must tell the Store which window owns its dialogs.
    fn context(hwnd: isize) -> AppResult<StoreContext> {
        let ctx = StoreContext::GetDefault().map_err(err)?;
        let init: IInitializeWithWindow = ctx.cast().map_err(err)?;
        unsafe { init.Initialize(HWND(hwnd as *mut core::ffi::c_void)) }.map_err(err)?;
        Ok(ctx)
    }

    pub fn purchase(hwnd: isize, store_id: &str) -> AppResult<Outcome> {
        let ctx = context(hwnd)?;
        let result = ctx.RequestPurchaseAsync(&HSTRING::from(store_id)).map_err(err)?.get().map_err(err)?;
        let status = result.Status().map_err(err)?;
        Ok(match status {
            StorePurchaseStatus::Succeeded => Outcome::Purchased,
            StorePurchaseStatus::AlreadyPurchased => Outcome::AlreadyOwned,
            StorePurchaseStatus::NotPurchased => Outcome::Cancelled,
            StorePurchaseStatus::NetworkError => Outcome::Failed("The Store couldn't be reached. Check your connection and try again.".into()),
            _ => Outcome::Failed(result.ExtendedError().map(|e| format!("The Store reported a problem ({e:?}).")).unwrap_or_else(|_| "The Store reported a problem.".into())),
        })
    }

    pub fn id_keys(hwnd: isize, collections_ticket: &str, purchase_ticket: &str, user: &str) -> AppResult<IdKeys> {
        let ctx = context(hwnd)?;
        let user = HSTRING::from(user);
        let collections = ctx.GetCustomerCollectionsIdAsync(&HSTRING::from(collections_ticket), &user).map_err(err)?.get().map_err(err)?.to_string();
        let purchase = ctx.GetCustomerPurchaseIdAsync(&HSTRING::from(purchase_ticket), &user).map_err(err)?.get().map_err(err)?.to_string();
        if collections.is_empty() || purchase.is_empty() {
            return Err(AppError::invalid("Sign in to the Microsoft Store app with your Microsoft account, then try again."));
        }
        Ok(IdKeys { collections, purchase })
    }
}

/// Shows the Store's purchase dialog for this add-on (Store ID from Partner Center).
pub fn purchase(hwnd: isize, store_id: &str) -> AppResult<Outcome> {
    #[cfg(windows)]
    return windows_store::purchase(hwnd, store_id);
    #[cfg(not(windows))]
    {
        let _ = (hwnd, store_id);
        Err(AppError::invalid("Purchases work in the Microsoft Store version on Windows."))
    }
}

pub fn id_keys(hwnd: isize, collections_ticket: &str, purchase_ticket: &str, user: &str) -> AppResult<IdKeys> {
    #[cfg(windows)]
    return windows_store::id_keys(hwnd, collections_ticket, purchase_ticket, user);
    #[cfg(not(windows))]
    {
        let _ = (hwnd, collections_ticket, purchase_ticket, user);
        Err(AppError::invalid("Purchases work in the Microsoft Store version on Windows."))
    }
}

/// Our product names → the Store IDs the server knows.
pub fn store_id(products: &crate::sync::cloud::StoreProducts, product: &str) -> AppResult<String> {
    match product {
        "premium_monthly" => Ok(products.premium_monthly.clone()),
        "premium_yearly" => Ok(products.premium_yearly.clone()),
        "arrows_100" => Ok(products.arrows_100.clone()),
        "arrows_300" => products.arrows_300.clone().ok_or_else(|| AppError::invalid("That pack isn't available.")),
        _ => Err(AppError::invalid("Unknown product.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn products_map_to_store_ids() {
        let p = crate::sync::cloud::StoreProducts { premium_monthly: "9NM".into(), premium_yearly: "9NY".into(), arrows_100: "9NA".into(), arrows_300: None };
        assert_eq!(store_id(&p, "premium_yearly").unwrap(), "9NY");
        assert!(store_id(&p, "arrows_300").is_err());
        assert!(store_id(&p, "free_money").is_err());
        assert_eq!(Outcome::Cancelled.as_str(), "cancelled");
    }
}
