//! Which cloud mailbox this build talks to. Both expose the same functions and a relay type
//! with the same pairing methods, so pairing, the background service and the sync engine are
//! shared by both editions.
//!
//! - personal edition (default): the couple's own Supabase project (`supabase.rs`).
//! - Store edition (`--features store`): Dear Us Cloud (`cloud.rs`).

#[cfg(not(feature = "store"))]
pub use super::supabase::{check_project, is_offline, normalize_url, redeem_invite, sign_in, sign_up, Session, SupabaseRelay as AccountRelay};

#[cfg(feature = "store")]
pub use super::cloud::{check_project, is_offline, normalize_url, redeem_invite, sign_in, sign_up, CloudRelay as AccountRelay, Session};

/// "personal" or "store" (shown to the UI, which hides what doesn't apply).
pub const EDITION: &str = if cfg!(feature = "store") { "store" } else { "personal" };
