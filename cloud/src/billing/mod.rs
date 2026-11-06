//! Billing. Every provider updates the same per-couple plan (`spaces.premium_until`,
//! `spaces.cupid_bonus`), so a purchase by either partner covers both on every device.
//! - Paddle (current): checkout in the browser, confirmed by Paddle's signed webhooks.
//! - Microsoft Store purchases: kept, unused while Paddle is configured.

pub mod microsoft;
pub mod paddle;
