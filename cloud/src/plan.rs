//! The couple's plan, decided here (never by the app):
//!
//! - `solo`    : not paired yet. Free limits.
//! - `trial`   : 30 days from the moment the partner joins. Everything unlimited, 100 Cupid arrows/month.
//! - `premium` : an active subscription (bought by either partner, covers both).
//! - `free`    : after the trial without a subscription. Memories are never touched: text syncs
//!               without limit, cloud media up to 100 MB per month (Wi-Fi transfers don't count),
//!               10 Cupid arrows per month.
//!
//! Top-up arrows never expire and are used after the monthly allowance.

use crate::error::{ApiError, ApiResult};
use crate::AppState;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

pub fn period(now: DateTime<Utc>) -> String {
    now.format("%Y-%m").to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Solo,
    Trial,
    Premium,
    Free,
}

pub fn tier(now: DateTime<Utc>, trial_ends_at: Option<DateTime<Utc>>, premium_until: Option<DateTime<Utc>>) -> Tier {
    if premium_until.is_some_and(|p| p > now) {
        Tier::Premium
    } else if let Some(t) = trial_ends_at {
        if t > now {
            Tier::Trial
        } else {
            Tier::Free
        }
    } else {
        Tier::Solo
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub tier: Tier,
    pub trial_ends_at: Option<DateTime<Utc>>,
    pub premium_until: Option<DateTime<Utc>>,
    pub period: String,
    /// None = unlimited.
    pub media_cap_bytes: Option<i64>,
    pub media_used_bytes: i64,
    pub cupid_allowance: i32,
    pub cupid_used: i32,
    pub cupid_bonus: i32,
    pub cupid_remaining: i32,
    /// Time capsules that can be sealed and waiting at once (None = unlimited).
    pub capsule_limit: Option<i32>,
    /// Premium and arrows can be bought (billing is configured on the server).
    pub billing_available: bool,
    /// "paddle" (checkout in the browser) or "microsoft" (Store purchase dialog).
    pub billing_provider: Option<&'static str>,
}

#[derive(sqlx::FromRow)]
pub struct SpaceRow {
    pub trial_ends_at: Option<DateTime<Utc>>,
    pub premium_until: Option<DateTime<Utc>>,
    pub media_period: String,
    pub media_bytes: i64,
    pub cupid_period: String,
    pub cupid_used: i32,
    pub cupid_bonus: i32,
}

pub fn compute(cfg: &crate::config::Config, row: &SpaceRow, now: DateTime<Utc>) -> Plan {
    let t = tier(now, row.trial_ends_at, row.premium_until);
    let p = period(now);
    let unlimited = matches!(t, Tier::Trial | Tier::Premium);
    let allowance = if unlimited { cfg.premium_cupid } else { cfg.free_cupid };
    let media_used = if row.media_period == p { row.media_bytes } else { 0 };
    let cupid_used = if row.cupid_period == p { row.cupid_used } else { 0 };
    Plan {
        tier: t,
        trial_ends_at: row.trial_ends_at,
        premium_until: row.premium_until,
        period: p,
        media_cap_bytes: (!unlimited).then_some(cfg.free_media_bytes),
        media_used_bytes: media_used,
        cupid_allowance: allowance,
        cupid_used,
        cupid_bonus: row.cupid_bonus,
        cupid_remaining: (allowance - cupid_used).max(0) + row.cupid_bonus,
        capsule_limit: (!unlimited).then_some(cfg.free_capsules),
        billing_available: cfg.paddle.is_some() || cfg.microsoft.is_some(),
        billing_provider: if cfg.paddle.is_some() {
            Some("paddle")
        } else if cfg.microsoft.is_some() {
            Some("microsoft")
        } else {
            None
        },
    }
}

const SPACE_COLS: &str = "trial_ends_at, premium_until, media_period, media_bytes, cupid_period, cupid_used, cupid_bonus";

/// Reads the space row with a row lock (inside a transaction), so counters can't race.
pub async fn lock(tx: &mut Transaction<'_, Postgres>, space: Uuid) -> ApiResult<SpaceRow> {
    sqlx::query_as::<_, SpaceRow>(&format!("select {SPACE_COLS} from spaces where id = $1 for update"))
        .bind(space)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| ApiError::not_found("space"))
}

pub async fn load(conn: &mut PgConnection, space: Uuid) -> ApiResult<SpaceRow> {
    sqlx::query_as::<_, SpaceRow>(&format!("select {SPACE_COLS} from spaces where id = $1"))
        .bind(space)
        .fetch_optional(conn)
        .await?
        .ok_or_else(|| ApiError::not_found("space"))
}

pub async fn current(state: &AppState, space: Uuid) -> ApiResult<Plan> {
    let mut conn = state.db.acquire().await?;
    let row = load(&mut conn, space).await?;
    Ok(compute(&state.cfg, &row, Utc::now()))
}

/// Charges a file to this month's cloud media allowance (once per file). 402 when it doesn't fit.
pub async fn charge_media(tx: &mut Transaction<'_, Postgres>, cfg: &crate::config::Config, space: Uuid, bytes: i64) -> ApiResult<()> {
    let row = lock(tx, space).await?;
    let now = Utc::now();
    let plan = compute(cfg, &row, now);
    if let Some(cap) = plan.media_cap_bytes {
        if plan.media_used_bytes + bytes > cap {
            return Err(ApiError::quota(
                "media_quota",
                "This month's free cloud space for photos and videos is used up. They'll go over your home Wi-Fi, or through the cloud next month — or with Premium right away.",
            ));
        }
    }
    sqlx::query("update spaces set media_period = $2, media_bytes = $3 where id = $1")
        .bind(space)
        .bind(&plan.period)
        .bind(plan.media_used_bytes + bytes)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Where a Cupid arrow came from (so a failed request can give it back to the right place).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowSource {
    Monthly,
    TopUp,
}

/// Takes one Cupid arrow: the monthly allowance first, then top-up arrows. 402 when none left.
pub async fn take_arrow(tx: &mut Transaction<'_, Postgres>, cfg: &crate::config::Config, space: Uuid) -> ApiResult<(Plan, ArrowSource)> {
    let row = lock(tx, space).await?;
    let now = Utc::now();
    let plan = compute(cfg, &row, now);
    let (used, bonus, source) = if plan.cupid_used < plan.cupid_allowance {
        (plan.cupid_used + 1, plan.cupid_bonus, ArrowSource::Monthly)
    } else if plan.cupid_bonus > 0 {
        (plan.cupid_used, plan.cupid_bonus - 1, ArrowSource::TopUp)
    } else {
        return Err(ApiError::quota("cupid_quota", "Cupid is out of arrows for this month. They refill on the 1st — or top up his quiver."));
    };
    sqlx::query("update spaces set cupid_period = $2, cupid_used = $3, cupid_bonus = $4 where id = $1")
        .bind(space)
        .bind(&plan.period)
        .bind(used)
        .bind(bonus)
        .execute(&mut **tx)
        .await?;
    let row = SpaceRow { cupid_period: plan.period.clone(), cupid_used: used, cupid_bonus: bonus, ..row };
    Ok((compute(cfg, &row, now), source))
}

/// Gives an arrow back when Gemini couldn't answer (so failures never cost anything).
pub async fn refund_arrow(state: &AppState, space: Uuid, taken_in: &str, source: ArrowSource) -> ApiResult<()> {
    let mut tx = state.db.begin().await?;
    let row = lock(&mut tx, space).await?;
    let now = Utc::now();
    let plan = compute(&state.cfg, &row, now);
    let (used, bonus) = match source {
        ArrowSource::TopUp => (plan.cupid_used, plan.cupid_bonus + 1),
        // the month rolled over meanwhile: the new month starts at zero anyway
        ArrowSource::Monthly if plan.period != taken_in => return Ok(()),
        ArrowSource::Monthly => ((plan.cupid_used - 1).max(0), plan.cupid_bonus),
    };
    sqlx::query("update spaces set cupid_period = $2, cupid_used = $3, cupid_bonus = $4 where id = $1")
        .bind(space)
        .bind(&plan.period)
        .bind(used)
        .bind(bonus)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn row() -> SpaceRow {
        SpaceRow { trial_ends_at: None, premium_until: None, media_period: String::new(), media_bytes: 0, cupid_period: String::new(), cupid_used: 0, cupid_bonus: 0 }
    }

    #[test]
    fn tiers_follow_the_funnel() {
        let now = Utc::now();
        assert_eq!(tier(now, None, None), Tier::Solo);
        assert_eq!(tier(now, Some(now + Duration::days(3)), None), Tier::Trial);
        assert_eq!(tier(now, Some(now - Duration::days(1)), None), Tier::Free);
        assert_eq!(tier(now, Some(now - Duration::days(1)), Some(now + Duration::days(20))), Tier::Premium);
        assert_eq!(tier(now, Some(now - Duration::days(1)), Some(now - Duration::days(2))), Tier::Free, "lapsed subscription falls back gracefully");
    }

    #[test]
    fn counters_reset_each_month_and_bonus_stays() {
        let cfg = crate::config::Config::for_tests("", "", "");
        let now = Utc::now();
        let r = SpaceRow { trial_ends_at: Some(now - Duration::days(1)), media_period: "2000-01".into(), media_bytes: 99, cupid_period: "2000-01".into(), cupid_used: 10, cupid_bonus: 7, ..row() };
        let p = compute(&cfg, &r, now);
        assert_eq!(p.tier, Tier::Free);
        assert_eq!(p.media_used_bytes, 0);
        assert_eq!(p.cupid_used, 0);
        assert_eq!(p.cupid_remaining, 10 + 7);
        assert_eq!(p.media_cap_bytes, Some(100 * 1024 * 1024));
    }

    #[test]
    fn trial_and_premium_are_unlimited_media() {
        let cfg = crate::config::Config::for_tests("", "", "");
        let now = Utc::now();
        let p = compute(&cfg, &SpaceRow { trial_ends_at: Some(now + Duration::days(1)), ..row() }, now);
        assert_eq!(p.media_cap_bytes, None);
        assert_eq!(p.cupid_allowance, 100);
    }
}
