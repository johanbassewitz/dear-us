//! Everything comes from environment variables (see `.env.example`). Secrets are never logged.

use std::env;

#[derive(Clone)]
pub struct R2 {
    /// e.g. https://<account-id>.r2.cloudflarestorage.com
    pub endpoint: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
}

#[derive(Clone)]
pub struct Paddle {
    /// https://api.paddle.com (live) or https://sandbox-api.paddle.com (testing)
    pub api_base: String,
    pub api_key: String,
    /// "Secret key" of the webhook destination (Paddle → Developer tools → Notifications)
    pub webhook_secret: String,
    pub price_premium_monthly: String,
    pub price_premium_yearly: String,
    pub price_arrows_100: String,
    pub price_arrows_300: Option<String>,
}

#[derive(Clone)]
pub struct MicrosoftStore {
    pub tenant_id: String,
    pub client_id: String,
    pub client_secret: String,
    /// Store IDs of the add-ons, as set up in Partner Center.
    pub premium_monthly: String,
    pub premium_yearly: String,
    pub arrows_100: String,
    pub arrows_300: Option<String>,
}

/// How password-reset emails go out.
#[derive(Clone)]
pub enum Mail {
    /// Production: Resend (https://resend.com). `from` must be on a verified domain.
    Resend { api_key: String, from: String },
    /// No provider configured (development): the code is written to the server log.
    Log,
    /// Tests: kept in memory as (to, body).
    Memory(std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>),
}

#[derive(Clone)]
pub struct Config {
    pub bind: String,
    pub database_url: String,
    /// The URL the apps use to reach this server (also used for local-storage links).
    pub public_url: String,
    /// 32+ random bytes (hex or any string): signs access tokens and local-storage links.
    pub secret: Vec<u8>,
    pub r2: Option<R2>,
    /// Without R2, chunks are kept on this disk (development / tiny self-hosting).
    pub local_storage_dir: String,
    pub gemini_key: Option<String>,
    pub gemini_model: String,
    pub gemini_fallback_model: String,
    pub microsoft: Option<MicrosoftStore>,
    /// Payments through Paddle (checkout in the browser). Used instead of Microsoft Store
    /// purchases when set.
    pub paddle: Option<Paddle>,
    pub mail: Mail,
    pub trial_days: i64,
    /// Free tier: time capsules that can be sealed and waiting at the same time.
    pub free_capsules: i32,
    /// Free tier: cloud media per calendar month.
    pub free_media_bytes: i64,
    pub free_cupid: i32,
    pub premium_cupid: i32,
    /// Undelivered envelopes/files older than this are swept (the sender keeps the original).
    pub mailbox_ttl_days: i64,
}

fn opt(k: &str) -> Option<String> {
    env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn num<T: std::str::FromStr>(k: &str, default: T) -> T {
    opt(k).and_then(|v| v.parse().ok()).unwrap_or(default)
}

impl Config {
    pub fn from_env() -> Result<Config, String> {
        let secret = opt("SERVER_SECRET").ok_or("SERVER_SECRET is required (32+ random characters)")?;
        if secret.len() < 32 {
            return Err("SERVER_SECRET must be at least 32 characters".into());
        }
        let r2 = match (opt("R2_ENDPOINT"), opt("R2_BUCKET"), opt("R2_ACCESS_KEY_ID"), opt("R2_SECRET_ACCESS_KEY")) {
            (Some(endpoint), Some(bucket), Some(access_key), Some(secret_key)) => Some(R2 { endpoint, bucket, access_key, secret_key }),
            (None, None, None, None) => None,
            _ => return Err("set all of R2_ENDPOINT, R2_BUCKET, R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY (or none)".into()),
        };
        let microsoft = match (opt("MS_TENANT_ID"), opt("MS_CLIENT_ID"), opt("MS_CLIENT_SECRET")) {
            (Some(tenant_id), Some(client_id), Some(client_secret)) => Some(MicrosoftStore {
                tenant_id,
                client_id,
                client_secret,
                premium_monthly: opt("MS_PRODUCT_PREMIUM_MONTHLY").ok_or("MS_PRODUCT_PREMIUM_MONTHLY is required with Microsoft Store billing")?,
                premium_yearly: opt("MS_PRODUCT_PREMIUM_YEARLY").ok_or("MS_PRODUCT_PREMIUM_YEARLY is required with Microsoft Store billing")?,
                arrows_100: opt("MS_PRODUCT_ARROWS_100").ok_or("MS_PRODUCT_ARROWS_100 is required with Microsoft Store billing")?,
                arrows_300: opt("MS_PRODUCT_ARROWS_300"),
            }),
            (None, None, None) => None,
            _ => return Err("set all of MS_TENANT_ID, MS_CLIENT_ID, MS_CLIENT_SECRET (or none)".into()),
        };
        Ok(Config {
            // BIND, or the PORT a hosting platform (Render, Railway, …) hands us, or 8080
            bind: opt("BIND").unwrap_or_else(|| format!("0.0.0.0:{}", opt("PORT").unwrap_or_else(|| "8080".into()))),
            database_url: opt("DATABASE_URL").ok_or("DATABASE_URL is required")?,
            public_url: opt("PUBLIC_URL").ok_or("PUBLIC_URL is required (e.g. https://dearus-api.axianthq.com)")?.trim_end_matches('/').to_string(),
            secret: secret.into_bytes(),
            r2,
            local_storage_dir: opt("LOCAL_STORAGE_DIR").unwrap_or_else(|| "./data/chunks".into()),
            gemini_key: opt("GEMINI_API_KEY"),
            gemini_model: opt("GEMINI_MODEL").unwrap_or_else(|| "gemini-flash-latest".into()),
            gemini_fallback_model: opt("GEMINI_FALLBACK_MODEL").unwrap_or_else(|| "gemini-flash-lite-latest".into()),
            microsoft,
            paddle: match (opt("PADDLE_API_KEY"), opt("PADDLE_WEBHOOK_SECRET")) {
                (Some(api_key), Some(webhook_secret)) => Some(Paddle {
                    api_base: opt("PADDLE_API_BASE").unwrap_or_else(|| {
                        if opt("PADDLE_ENV").as_deref() == Some("sandbox") { "https://sandbox-api.paddle.com".into() } else { "https://api.paddle.com".into() }
                    }),
                    api_key,
                    webhook_secret,
                    price_premium_monthly: opt("PADDLE_PRICE_PREMIUM_MONTHLY").ok_or("PADDLE_PRICE_PREMIUM_MONTHLY is required with Paddle")?,
                    price_premium_yearly: opt("PADDLE_PRICE_PREMIUM_YEARLY").ok_or("PADDLE_PRICE_PREMIUM_YEARLY is required with Paddle")?,
                    price_arrows_100: opt("PADDLE_PRICE_ARROWS_100").ok_or("PADDLE_PRICE_ARROWS_100 is required with Paddle")?,
                    price_arrows_300: opt("PADDLE_PRICE_ARROWS_300"),
                }),
                (None, None) => None,
                _ => return Err("set both PADDLE_API_KEY and PADDLE_WEBHOOK_SECRET (or neither)".into()),
            },
            mail: match opt("RESEND_API_KEY") {
                Some(api_key) => Mail::Resend { api_key, from: opt("MAIL_FROM").ok_or("MAIL_FROM is required with RESEND_API_KEY (e.g. Dear Us <dearus-noreply@axianthq.com>)")? },
                None => Mail::Log,
            },
            trial_days: num("TRIAL_DAYS", 30),
            free_capsules: num("FREE_CAPSULES", 3),
            free_media_bytes: num("FREE_MEDIA_MB", 100i64) * 1024 * 1024,
            free_cupid: num("FREE_CUPID_PER_MONTH", 10),
            premium_cupid: num("PREMIUM_CUPID_PER_MONTH", 100),
            mailbox_ttl_days: num("MAILBOX_TTL_DAYS", 30),
        })
    }

    /// For tests: local storage, no external services.
    pub fn for_tests(database_url: &str, public_url: &str, dir: &str) -> Config {
        Config {
            bind: "127.0.0.1:0".into(),
            database_url: database_url.into(),
            public_url: public_url.into(),
            secret: b"test-secret-test-secret-test-secret-0123".to_vec(),
            r2: None,
            local_storage_dir: dir.into(),
            gemini_key: None,
            gemini_model: "gemini-flash-latest".into(),
            gemini_fallback_model: "gemini-flash-lite-latest".into(),
            microsoft: None,
            paddle: None,
            mail: Mail::Memory(Default::default()),
            trial_days: 30,
            free_capsules: 3,
            free_media_bytes: 100 * 1024 * 1024,
            free_cupid: 10,
            premium_cupid: 100,
            mailbox_ttl_days: 30,
        }
    }
}
