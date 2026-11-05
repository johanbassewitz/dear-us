//! Store billing. Each store verifies purchases on this server and updates the same per-couple
//! plan (`spaces.premium_until`, `spaces.cupid_bonus`), so buying on one platform covers both
//! partners everywhere. Microsoft Store first; Apple/Google come later (docs/MOBILE_PLAN.md).

pub mod microsoft;
