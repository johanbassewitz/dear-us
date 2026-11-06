//! Runs sync in the background while the app is open and keeps its settings.
//! Settings (project URL, public key, space id, email) live in `local_meta`; the space key
//! and the sign-in token live in Windows Credential Manager, scoped to this install's device id.

use super::apply::meta_set;
use super::crypto::SpaceKey;
use super::engine::{counts, Counts, Ctx, Report, DEFAULT_CHUNK};
#[cfg(not(feature = "store"))]
use super::engine::DEFAULT_QUOTA_CAP;
use super::relay::Relay;
use super::backend::{self, AccountRelay};
use crate::error::{AppError, AppResult};
use crate::secrets;
use crate::AppState;
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const INTERVAL: Duration = Duration::from_secs(45);

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub url: String,
    pub anon: String,
    pub space: Option<String>,
    pub email: Option<String>,
}

fn get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT value FROM local_meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
}

pub fn load_config(conn: &Connection) -> AppResult<Option<Config>> {
    let (Some(url), Some(anon)) = (get(conn, "sync.url")?, get(conn, "sync.anon")?) else { return Ok(None) };
    Ok(Some(Config { url, anon, space: get(conn, "sync.space")?, email: get(conn, "sync.email")? }))
}

pub fn save_config(conn: &Connection, c: &Config) -> AppResult<()> {
    meta_set(conn, "sync.url", &c.url)?;
    meta_set(conn, "sync.anon", &c.anon)?;
    if let Some(s) = &c.space {
        meta_set(conn, "sync.space", s)?;
    }
    if let Some(e) = &c.email {
        meta_set(conn, "sync.email", e)?;
    }
    Ok(())
}

pub fn clear_config(conn: &Connection, device: &str) -> AppResult<()> {
    conn.execute("DELETE FROM local_meta WHERE key LIKE 'sync.%'", [])?;
    secrets::delete(&secret_name(device, "refresh"))?;
    secrets::delete(&secret_name(device, "space-key"))?;
    Ok(())
}

pub fn secret_name(device: &str, what: &str) -> String {
    // device-scoped so two test instances on one PC don't share credentials
    format!("{}:{what}", &device[..device.len().min(36)])
}

pub fn store_key(device: &str, key: &SpaceKey) -> AppResult<()> {
    secrets::set(&secret_name(device, "space-key"), &key.to_b64())
}

pub fn load_key(device: &str) -> AppResult<Option<SpaceKey>> {
    secrets::get(&secret_name(device, "space-key"))?.map(|k| SpaceKey::from_b64(&k)).transpose()
}

pub fn store_refresh(device: &str, token: &str) -> AppResult<()> {
    secrets::set(&secret_name(device, "refresh"), token)
}

pub fn refresh_saver(device: &str) -> Box<dyn Fn(&str) + Send + Sync> {
    let d = device.to_string();
    Box::new(move |t: &str| {
        let _ = store_refresh(&d, t);
    })
}

#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    #[serde(skip)]
    pub endpoint: Option<String>,
    pub id: String,
    pub name: String,
    pub mine: bool,
    pub this_pc: bool,
    pub revoked: bool,
}

#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// off | setup | waiting_for_partner | syncing | up_to_date | pending | offline | waiting_for_space | error | signed_out
    pub state: String,
    pub configured: bool,
    pub paired: bool,
    /// A saved sign-in exists on this PC.
    pub signed_in: bool,
    pub email: Option<String>,
    pub project_url: Option<String>,
    pub last_sync_at: Option<String>,
    pub last_error: Option<String>,
    pub counts: CountsOut,
    pub devices: Vec<DeviceInfo>,
    pub this_device: String,
    pub relay_bytes: Option<i64>,
    pub direct_enabled: bool,
    /// The partner's PC is reachable on this network right now.
    pub direct_peer: bool,
    /// A newly linked device waiting for its snapshot from the person's other device.
    pub linking: bool,
    /// Store edition: the couple's plan (trial / premium / free), last known.
    #[cfg(feature = "store")]
    pub plan: Option<super::cloud::Plan>,
}

#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CountsOut {
    pub pending_ops: i64,
    pub awaiting_delivery: i64,
    pub uploads_pending: i64,
    pub downloads_pending: i64,
    pub failed_updates: i64,
    pub open_conflicts: i64,
}

impl From<Counts> for CountsOut {
    fn from(c: Counts) -> Self {
        CountsOut {
            pending_ops: c.pending_ops,
            awaiting_delivery: c.awaiting_delivery,
            uploads_pending: c.uploads_pending,
            downloads_pending: c.downloads_pending,
            failed_updates: c.failed_updates,
            open_conflicts: c.open_conflicts,
        }
    }
}

pub struct SyncService {
    trigger: Mutex<Sender<()>>,
    server_port: Mutex<Option<u16>>,
    published: Mutex<Option<String>>,
    pub status: Mutex<Status>,
    relay: Mutex<Option<Arc<AccountRelay>>>,
}

impl SyncService {
    pub fn kick(&self) {
        let _ = self.trigger.lock().unwrap().send(());
    }
    /// Drops the cached connection (after sign-in, joining, or disconnecting).
    pub fn reset(&self) {
        *self.relay.lock().unwrap() = None;
    }
    pub fn set_relay(&self, r: Arc<AccountRelay>) {
        *self.relay.lock().unwrap() = Some(r);
    }
}

pub fn device_name() -> String {
    std::env::var("COMPUTERNAME").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "This PC".into())
}

/// Connection to the relay for this install, from saved settings + saved sign-in.
pub fn connect(app: &AppHandle) -> AppResult<Arc<AccountRelay>> {
    let svc = app.state::<SyncService>();
    if let Some(r) = svc.relay.lock().unwrap().clone() {
        return Ok(r);
    }
    let state = app.state::<AppState>();
    let (cfg, device) = {
        let conn = state.db.lock().unwrap();
        (load_config(&conn)?, super::device_id(&conn)?)
    };
    let cfg = cfg.ok_or_else(|| AppError::invalid("sync isn't set up"))?;
    let space = cfg.space.ok_or_else(|| AppError::invalid("not connected to a space yet"))?;
    let token = secrets::get(&secret_name(&device, "refresh"))?.ok_or_else(|| AppError::invalid("signed-out: please sign in to sync again"))?;
    let r = Arc::new(AccountRelay::from_refresh(&cfg.url, &cfg.anon, &space, &token, refresh_saver(&device))?);
    svc.set_relay(r.clone());
    Ok(r)
}

fn base_status(app: &AppHandle) -> AppResult<Status> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().unwrap();
    let cfg = load_config(&conn)?;
    let device = super::device_id(&conn)?;
    let paired = cfg.as_ref().is_some_and(|c| c.space.is_some()) && load_key(&device).ok().flatten().is_some();
    Ok(Status {
        state: if cfg.is_none() { "off".into() } else if !paired { "setup".into() } else { "syncing".into() },
        configured: cfg.is_some(),
        signed_in: crate::secrets::get(&secret_name(&device, "refresh")).ok().flatten().is_some(),
        paired,
        email: cfg.as_ref().and_then(|c| c.email.clone()),
        project_url: cfg.as_ref().map(|c| c.url.clone()),
        last_sync_at: get(&conn, "sync.last_ok")?,
        counts: counts(&conn)?.into(),
        this_device: device,
        direct_enabled: direct_enabled(&conn),
        linking: super::bootstrap::pending(&conn),
        #[cfg(feature = "store")]
        plan: cached_plan(&conn),
        ..Default::default()
    })
}

/// One full pass. Returns the report (or why it couldn't run).
pub fn run_once(app: &AppHandle) -> AppResult<Option<Report>> {
    let st = base_status(app)?;
    if !st.paired {
        return Ok(None);
    }
    let relay = connect(app)?;
    let state = app.state::<AppState>();
    let key = load_key(&st.this_device)?.ok_or_else(|| AppError::invalid("the space key is missing on this PC"))?;
    let direct = direct_client(app, relay.as_ref(), &key, &st.this_device);
    let ctx = Ctx {
        direct: direct.as_ref(),
        db: &state.db,
        store: &state.store,
        relay: relay.as_ref(),
        key: &key,
        device: st.this_device.clone(),
        chunk_size: DEFAULT_CHUNK,
        quota_cap: quota_cap(app, relay.as_ref()),
    };
    // A newly linked device first takes in the snapshot from the person's other device.
    let mut fresh = false;
    if super::bootstrap::pending(&state.db.lock().unwrap()) {
        if !super::bootstrap::receive(&ctx)? {
            return Ok(Some(Report { waiting_for_link: true, ..Default::default() }));
        }
        fresh = true;
    }
    let mut report = ctx.sync_once()?;
    report.changed |= fresh;
    // ...and a device that made a link code brings the new one up to date.
    if let Err(e) = super::bootstrap::serve_new_devices(&ctx, &relay.user_id()) {
        report.errors.push(format!("Couldn't send your memories to your new device yet: {e}"));
    }
    {
        let conn = state.db.lock().unwrap();
        meta_set(&conn, "sync.last_ok", &crate::db::now_iso())?;
    }
    #[cfg(feature = "store")]
    crate::commands::plan::daily_billing_check(app);
    Ok(Some(report))
}

/// How many bytes may wait in the cloud. Personal edition: stay under Supabase's free 1 GB.
/// Store edition: the plan's monthly media allowance (the server enforces it too).
#[cfg(not(feature = "store"))]
fn quota_cap(_app: &AppHandle, _relay: &AccountRelay) -> i64 {
    DEFAULT_QUOTA_CAP
}

#[cfg(feature = "store")]
fn quota_cap(app: &AppHandle, relay: &AccountRelay) -> i64 {
    let state = app.state::<AppState>();
    match relay.plan() {
        Ok(p) => {
            let _ = save_plan(&state.db.lock().unwrap(), &p);
            p.engine_cap()
        }
        Err(_) => cached_plan(&state.db.lock().unwrap()).map(|p| p.engine_cap()).unwrap_or(i64::MAX / 4),
    }
}

#[cfg(feature = "store")]
pub fn cached_plan(conn: &Connection) -> Option<super::cloud::Plan> {
    get(conn, "plan.cache").ok().flatten().and_then(|v| serde_json::from_str(&v).ok())
}

#[cfg(feature = "store")]
pub fn save_plan(conn: &Connection, p: &super::cloud::Plan) -> AppResult<()> {
    // stored in the server's shape, so it reads back with the same Deserialize
    let v = serde_json::json!({
        "tier": p.tier, "trial_ends_at": p.trial_ends_at, "premium_until": p.premium_until, "period": p.period,
        "media_cap_bytes": p.media_cap_bytes, "media_used_bytes": p.media_used_bytes, "cupid_allowance": p.cupid_allowance,
        "cupid_used": p.cupid_used, "cupid_bonus": p.cupid_bonus, "cupid_remaining": p.cupid_remaining, "capsule_limit": p.capsule_limit,
        "billing_available": p.billing_available, "billing_provider": p.billing_provider,
    });
    meta_set(conn, "plan.cache", &v.to_string())
}

pub fn direct_enabled(conn: &Connection) -> bool {
    get(conn, "direct.enabled").ok().flatten().as_deref() != Some("0")
}

/// Starts the local transfer server once, publishes this PC's LAN address, and returns a
/// client for the partner's PC(s) that published theirs. Any failure = cloud only.
fn direct_client(app: &AppHandle, relay: &dyn Relay, key: &SpaceKey, device: &str) -> Option<super::direct::DirectClient> {
    let state = app.state::<AppState>();
    let enabled = direct_enabled(&state.db.lock().unwrap());
    let svc = app.state::<SyncService>();
    let published = svc.published.lock().unwrap().clone();
    if !enabled {
        if published.is_some() && relay.set_endpoint(device, None).is_ok() {
            *svc.published.lock().unwrap() = None;
        }
        return None;
    }
    let port = {
        let mut port = svc.server_port.lock().unwrap();
        if port.is_none() {
            let handle = app.clone();
            let k = key.clone();
            let provider: super::direct::ChunkProvider = Arc::new(move |hash: &str, n: i64| {
                let st = handle.state::<AppState>();
                super::engine::serve_chunk(&st.db, &st.store, &k, hash, n, DEFAULT_CHUNK)
            });
            *port = super::direct::serve("0.0.0.0:0", key.clone(), provider).ok();
        }
        (*port)?
    };
    let endpoint = super::direct::lan_ip().map(|ip| format!("{ip}:{port}"));
    if endpoint != published && relay.set_endpoint(device, endpoint.as_deref()).is_ok() {
        *svc.published.lock().unwrap() = endpoint;
    }
    let partners: std::collections::HashMap<String, String> = relay
        .devices()
        .ok()?
        .into_iter()
        .filter(|d| d.id != device && !d.revoked)
        .filter_map(|d| d.endpoint.map(|e| (d.id, e)))
        .collect();
    (!partners.is_empty()).then(|| super::direct::DirectClient::new(key.clone(), partners))
}

pub fn refresh_status(app: &AppHandle, outcome: Option<&AppResult<Option<Report>>>) -> Status {
    let mut st = base_status(app).unwrap_or_default();
    if st.paired {
        if let Ok(relay) = connect(app) {
            if let Ok(devs) = relay.devices() {
                let me_user = relay.user_id();
                st.devices = devs
                    .into_iter()
                    .map(|d| DeviceInfo { endpoint: d.endpoint, this_pc: d.id == st.this_device, mine: d.user_id == me_user, id: d.id, name: d.name, revoked: d.revoked })
                    .collect();
            }
            st.relay_bytes = relay.used_bytes().ok();
            if st.direct_enabled {
                if let Ok(Some(key)) = load_key(&st.this_device) {
                    let eps: std::collections::HashMap<String, String> = st
                        .devices
                        .iter()
                        .filter(|d| !d.this_pc && !d.revoked)
                        .filter_map(|d| d.endpoint.clone().map(|e| (d.id.clone(), e)))
                        .collect();
                    st.direct_peer = !eps.is_empty() && super::direct::DirectClient::new(key, eps).reachable().is_some();
                }
            }
        }
    }
    let svc = app.state::<SyncService>();
    let prev = svc.status.lock().unwrap().clone();
    match outcome {
        Some(Ok(Some(r))) => {
            st.state = if r.waiting_for_link {
                "linking".into()
            } else if r.waiting_for_partner_device {
                "waiting_for_partner".into()
            } else if r.waiting_for_space {
                "waiting_for_space".into()
            } else if !r.errors.is_empty() {
                st.last_error = r.errors.first().cloned();
                "error".into()
            } else if st.counts.pending_ops + st.counts.awaiting_delivery + st.counts.uploads_pending + st.counts.downloads_pending > 0 {
                "pending".into()
            } else {
                "up_to_date".into()
            };
        }
        Some(Ok(None)) => {}
        Some(Err(e)) => {
            st.last_error = Some(e.to_string());
            st.state = if backend::is_offline(e) {
                "offline".into()
            } else if e.to_string().starts_with("signed-out") {
                "signed_out".into()
            } else {
                "error".into()
            };
        }
        None => {
            if st.paired {
                st.state = prev.state.clone();
                st.last_error = prev.last_error.clone();
                if st.state.is_empty() {
                    st.state = "syncing".into();
                }
            }
        }
    }
    *svc.status.lock().unwrap() = st.clone();
    let _ = app.emit("sync-status", &st);
    st
}

pub fn start(app: AppHandle) -> SyncService {
    let (tx, rx) = channel::<()>();
    let handle = app.clone();
    std::thread::Builder::new()
        .name("partner-sync".into())
        .spawn(move || loop {
            match rx.recv_timeout(INTERVAL) {
                Ok(()) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            while rx.try_recv().is_ok() {} // collapse bursts of triggers
            let outcome = run_once(&handle);
            if let Ok(Some(r)) = &outcome {
                if r.changed {
                    let _ = handle.emit("data-changed", ());
                }
                if let Some(q) = handle.try_state::<crate::media::worker::PreviewQueue>() {
                    for id in &r.media_needing_preview {
                        q.push(id.clone());
                    }
                }
                for id in &r.media_ready {
                    let _ = handle.emit("media-updated", id);
                }
            }
            if let Err(e) = &outcome {
                if e.to_string().starts_with("signed-out") {
                    handle.state::<SyncService>().reset();
                }
            }
            refresh_status(&handle, Some(&outcome));
        })
        .expect("could not start sync");
    SyncService { trigger: Mutex::new(tx), server_port: Mutex::new(None), published: Mutex::new(None), status: Mutex::new(Status::default()), relay: Mutex::new(None) }
}
