//! Partner sync commands: setup, pairing, status, conflicts, devices.

use crate::db::repo::profile::{self, ProfileInput};
use crate::error::{AppError, AppResult};
use crate::sync::crypto::{random_token, sha256_hex, SpaceKey};
use crate::sync::service::{self, Config, Status, SyncService};
use crate::sync::backend::{self, AccountRelay, Session};
use crate::sync::{apply, identity};
use crate::AppState;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

const INVITE_PREFIX: &str = "DEARUS1-";

fn check_email(e: &str) -> AppResult<String> {
    let e = e.trim().to_lowercase();
    if e.len() < 5 || !e.contains('@') || e.len() > 200 {
        return Err(AppError::invalid("Please enter a valid email address."));
    }
    Ok(e)
}
fn check_password(p: &str) -> AppResult<()> {
    if p.chars().count() < 8 {
        return Err(AppError::invalid("Use at least 8 characters for the password."));
    }
    Ok(())
}

fn device(state: &AppState) -> AppResult<String> {
    crate::sync::device_id(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn sync_status(app: AppHandle) -> Status {
    let cached = app.state::<SyncService>().status.lock().unwrap().clone();
    if cached.state.is_empty() {
        service::refresh_status(&app, None)
    } else {
        cached
    }
}

#[tauri::command]
pub async fn sync_check_project(url: String, anon_key: String) -> AppResult<i64> {
    let url = backend::normalize_url(&url)?;
    tauri::async_runtime::spawn_blocking(move || backend::check_project(&url, anon_key.trim()))
        .await
        .map_err(|e| AppError::invalid(e.to_string()))?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountResult {
    pub needs_email_confirmation: bool,
}

/// Saves the project and signs in (or creates the account). No space yet.
#[tauri::command]
pub async fn sync_connect_account(
    app: AppHandle,
    url: String,
    anon_key: String,
    email: String,
    password: String,
    create: bool,
) -> AppResult<AccountResult> {
    let url = backend::normalize_url(&url)?;
    let anon = anon_key.trim().to_string();
    let email = check_email(&email)?;
    check_password(&password)?;
    let session = {
        let (u, a, e) = (url.clone(), anon.clone(), email.clone());
        tauri::async_runtime::spawn_blocking(move || -> AppResult<Option<Session>> {
            backend::check_project(&u, &a)?;
            if create {
                backend::sign_up(&u, &a, &e, &password)
            } else {
                backend::sign_in(&u, &a, &e, &password).map(Some)
            }
        })
        .await
        .map_err(|e| AppError::invalid(e.to_string()))??
    };
    let state = app.state::<AppState>();
    let dev = device(&state)?;
    {
        let conn = state.db.lock().unwrap();
        service::save_config(&conn, &Config { url, anon, space: None, email: Some(email) })?;
    }
    match session {
        Some(s) => {
            service::store_refresh(&dev, &s.refresh_token)?;
            app.state::<SyncService>().reset();
            service::refresh_status(&app, None);
            Ok(AccountResult { needs_email_confirmation: false })
        }
        None => Ok(AccountResult { needs_email_confirmation: true }),
    }
}

fn session_relay(app: &AppHandle, space: &str) -> AppResult<Arc<AccountRelay>> {
    let state = app.state::<AppState>();
    let (cfg, dev) = {
        let conn = state.db.lock().unwrap();
        (service::load_config(&conn)?, crate::sync::device_id(&conn)?)
    };
    let cfg = cfg.ok_or_else(|| AppError::invalid("Connect your Supabase project first."))?;
    let token = crate::secrets::get(&service::secret_name(&dev, "refresh"))?.ok_or_else(|| AppError::invalid("Please sign in first."))?;
    Ok(Arc::new(AccountRelay::from_refresh(&cfg.url, &cfg.anon, space, &token, service::refresh_saver(&dev))?))
}

/// First PC: creates the shared space and its encryption key. Returns the recovery phrase.
#[tauri::command]
pub async fn sync_create_space(app: AppHandle) -> AppResult<String> {
    let a = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> AppResult<String> {
        let state = a.state::<AppState>();
        let (space, dev) = {
            let conn = state.db.lock().unwrap();
            let p = profile::get(&conn)?;
            let space = p.space.ok_or_else(|| AppError::invalid("Set up your space first."))?.id;
            (space, crate::sync::device_id(&conn)?)
        };
        let relay = session_relay(&a, &space)?;
        relay.create_space()?;
        relay.register_device(&dev, &service::device_name())?;
        let key = SpaceKey::generate();
        service::store_key(&dev, &key)?;
        {
            let mut conn = state.db.lock().unwrap();
            let mut cfg = service::load_config(&conn)?.unwrap_or_default();
            cfg.space = Some(space);
            service::save_config(&conn, &cfg)?;
            identity::record_profile_ops(&mut conn)?;
        }
        let svc = a.state::<SyncService>();
        svc.set_relay(relay);
        svc.kick();
        Ok(key.to_phrase())
    })
    .await
    .map_err(|e| AppError::invalid(e.to_string()))?
}

#[derive(Serialize, Deserialize)]
struct InvitePayload {
    v: u32,
    u: String,
    a: String,
    s: String,
    i: String,
    k: String,
}

/// A one-time code (valid 48 h) for the partner. It contains the space key: share it privately.
#[tauri::command]
pub async fn sync_create_invite(app: AppHandle) -> AppResult<String> {
    let a = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> AppResult<String> {
        let state = a.state::<AppState>();
        let (cfg, dev) = {
            let conn = state.db.lock().unwrap();
            (service::load_config(&conn)?, crate::sync::device_id(&conn)?)
        };
        let cfg = cfg.ok_or_else(|| AppError::invalid("Sync isn't set up."))?;
        let space = cfg.space.clone().ok_or_else(|| AppError::invalid("Create your shared space first."))?;
        let key = service::load_key(&dev)?.ok_or_else(|| AppError::invalid("The space key is missing on this PC."))?;
        let secret = random_token(24);
        service::connect(&a)?.create_invite(&sha256_hex(&secret))?;
        let payload = InvitePayload { v: 1, u: cfg.url, a: cfg.anon, s: space, i: secret, k: key.to_b64() };
        Ok(format!("{INVITE_PREFIX}{}", URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload)?)))
    })
    .await
    .map_err(|e| AppError::invalid(e.to_string()))?
}

fn parse_invite(code: &str) -> AppResult<InvitePayload> {
    let body = code.trim().strip_prefix(INVITE_PREFIX).ok_or_else(|| AppError::invalid("That doesn't look like a Dear Us invite code."))?;
    let raw = URL_SAFE_NO_PAD.decode(body.trim()).map_err(|_| AppError::invalid("The invite code seems cut off. Copy it again."))?;
    let p: InvitePayload = serde_json::from_slice(&raw).map_err(|_| AppError::invalid("The invite code seems damaged. Copy it again."))?;
    let _url = backend::normalize_url(&p.u)?;
    // The two editions use different clouds, so their invites don't mix.
    #[cfg(feature = "store")]
    if p.u.trim_end_matches('/') != _url {
        return Err(AppError::invalid("This invite is from the personal (self-hosted) edition of Dear Us. Ask your partner to install Dear Us from the Microsoft Store and send a new code."));
    }
    SpaceKey::from_b64(&p.k)?;
    Ok(p)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinInput {
    pub invite: String,
    pub email: String,
    pub password: String,
    pub create_account: bool,
    /// Needed when this PC has no profile yet.
    pub my_name: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinResult {
    pub needs_email_confirmation: bool,
    pub joined: bool,
}

/// Second PC: joins the partner's space with their invite code.
#[tauri::command]
pub async fn sync_join(app: AppHandle, input: JoinInput) -> AppResult<JoinResult> {
    let inv = parse_invite(&input.invite)?;
    let email = check_email(&input.email)?;
    check_password(&input.password)?;
    let a = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> AppResult<JoinResult> {
        let session = if input.create_account {
            match backend::sign_up(&inv.u, &inv.a, &email, &input.password)? {
                Some(s) => s,
                None => {
                    // remember the project so "I've confirmed my email" can sign in later
                    let state = a.state::<AppState>();
                    let conn = state.db.lock().unwrap();
                    service::save_config(&conn, &Config { url: inv.u.clone(), anon: inv.a.clone(), space: None, email: Some(email) })?;
                    return Ok(JoinResult { needs_email_confirmation: true, joined: false });
                }
            }
        } else {
            backend::sign_in(&inv.u, &inv.a, &email, &input.password)?
        };
        let space = backend::redeem_invite(&inv.u, &inv.a, &session, &inv.i)?;
        let state = a.state::<AppState>();
        let dev = device(&state)?;
        service::store_refresh(&dev, &session.refresh_token)?;
        let relay = Arc::new(AccountRelay::new(&inv.u, &inv.a, &space, session, service::refresh_saver(&dev)));
        relay.register_device(&dev, &service::device_name())?;
        service::store_key(&dev, &SpaceKey::from_b64(&inv.k)?)?;
        {
            let mut conn = state.db.lock().unwrap();
            let p = profile::get(&conn)?;
            if p.me.is_none() {
                let name = input.my_name.clone().unwrap_or_default();
                profile::save(&mut conn, &ProfileInput { my_name: name, partner_name: None, space_name: "Our little world".into(), start_date: None })?;
            }
            identity::adopt_space_id(&conn, &space)?;
            service::save_config(&conn, &Config { url: inv.u, anon: inv.a, space: Some(space), email: Some(email) })?;
            // Share my name only: the space details come from the partner's PC.
            let me = identity::my_user_id(&conn)?;
            let name: Option<String> = conn.query_row("SELECT display_name FROM users WHERE is_self = 1", [], |r| r.get(0)).ok();
            if let (Some(id), Some(name)) = (me, name) {
                let w = crate::sync::oplog::Writer { device_id: dev.clone() };
                let tx = conn.transaction()?;
                w.record(&tx, "user", &id, "upsert", json!({ "display_name": name }))?;
                tx.commit()?;
            }
        }
        let svc = a.state::<SyncService>();
        svc.set_relay(relay);
        svc.kick();
        Ok(JoinResult { needs_email_confirmation: false, joined: true })
    })
    .await
    .map_err(|e| AppError::invalid(e.to_string()))?
}

/// After confirming the email: sign in with the saved project.
#[tauri::command]
pub async fn sync_sign_in(app: AppHandle, password: String) -> AppResult<()> {
    let state = app.state::<AppState>();
    let (cfg, dev) = {
        let conn = state.db.lock().unwrap();
        (service::load_config(&conn)?, crate::sync::device_id(&conn)?)
    };
    let cfg = cfg.ok_or_else(|| AppError::invalid("Sync isn't set up."))?;
    let email = cfg.email.clone().ok_or_else(|| AppError::invalid("No email saved."))?;
    let s = tauri::async_runtime::spawn_blocking(move || backend::sign_in(&cfg.url, &cfg.anon, &email, &password))
        .await
        .map_err(|e| AppError::invalid(e.to_string()))??;
    service::store_refresh(&dev, &s.refresh_token)?;
    let svc = app.state::<SyncService>();
    svc.reset();
    svc.kick();
    Ok(())
}

#[tauri::command]
pub fn sync_now(app: AppHandle) {
    app.state::<SyncService>().kick();
}

#[tauri::command]
pub fn sync_recovery_phrase(state: State<AppState>) -> AppResult<String> {
    let dev = device(&state)?;
    Ok(service::load_key(&dev)?.ok_or_else(|| AppError::invalid("There's no space key on this PC yet."))?.to_phrase())
}

#[tauri::command]
pub async fn sync_revoke_device(app: AppHandle, device_id: String) -> AppResult<()> {
    let a = app.clone();
    tauri::async_runtime::spawn_blocking(move || service::connect(&a)?.revoke_device(&device_id))
        .await
        .map_err(|e| AppError::invalid(e.to_string()))??;
    app.state::<SyncService>().kick();
    Ok(())
}

/// Stops syncing on this PC. Memories stay here; nothing is deleted.
#[tauri::command]
pub fn sync_disconnect(app: AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    {
        let conn = state.db.lock().unwrap();
        let dev = crate::sync::device_id(&conn)?;
        service::clear_config(&conn, &dev)?;
    }
    app.state::<SyncService>().reset();
    service::refresh_status(&app, None);
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    id: String,
    entity: String,
    entity_id: String,
    field: String,
    kept_value: String,
    other_value: String,
    created_at: String,
    title: Option<String>,
}

#[tauri::command]
pub fn sync_conflicts(state: State<AppState>) -> AppResult<Vec<Conflict>> {
    let conn = state.db.lock().unwrap();
    let mut s = conn.prepare(
        "SELECT c.id, c.entity, c.entity_id, c.field, COALESCE(c.kept_value,''), COALESCE(c.other_value,''), c.created_at,
                (SELECT title FROM journal_entries WHERE id = c.entity_id)
         FROM sync_conflicts c WHERE c.resolved_at IS NULL ORDER BY c.created_at DESC",
    )?;
    let rows = s.query_map([], |r| {
        Ok(Conflict {
            id: r.get(0)?,
            entity: r.get(1)?,
            entity_id: r.get(2)?,
            field: r.get(3)?,
            kept_value: r.get(4)?,
            other_value: r.get(5)?,
            created_at: r.get(6)?,
            title: r.get(7)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[tauri::command]
pub fn sync_resolve_conflict(app: AppHandle, id: String, use_other: bool) -> AppResult<()> {
    let state = app.state::<AppState>();
    {
        let mut conn = state.db.lock().unwrap();
        let (w, _) = crate::commands::journal::ctx(&conn)?;
        apply::resolve_conflict(&mut conn, &w, &id, use_other)?;
    }
    app.state::<SyncService>().kick();
    Ok(())
}

/// Direct PC-to-PC transfers on the same network (on by default).
#[tauri::command]
pub fn sync_set_direct(app: AppHandle, on: bool) -> AppResult<()> {
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        apply::meta_set(&conn, "direct.enabled", if on { "1" } else { "0" })?;
    }
    app.state::<SyncService>().kick();
    service::refresh_status(&app, None);
    Ok(())
}

/// Store edition: emails a code for choosing a new password.
#[tauri::command]
pub async fn sync_forgot_password(email: String) -> AppResult<()> {
    let email = check_email(&email)?;
    #[cfg(feature = "store")]
    {
        let url = backend::normalize_url("")?;
        return tauri::async_runtime::spawn_blocking(move || crate::sync::cloud::forgot_password(&url, &email))
            .await
            .map_err(|e| AppError::invalid(e.to_string()))?;
    }
    #[cfg(not(feature = "store"))]
    {
        let _ = email;
        Err(AppError::invalid("Reset your password from your Supabase project's dashboard (Authentication → Users)."))
    }
}

/// Store edition: sets the new password with the emailed code and signs this PC in.
#[tauri::command]
pub async fn sync_reset_password(app: AppHandle, email: String, code: String, password: String) -> AppResult<()> {
    let email = check_email(&email)?;
    check_password(&password)?;
    #[cfg(feature = "store")]
    {
        let url = backend::normalize_url("")?;
        let (u, e) = (url.clone(), email.clone());
        let session = tauri::async_runtime::spawn_blocking(move || crate::sync::cloud::reset_password(&u, &e, code.trim(), &password))
            .await
            .map_err(|e| AppError::invalid(e.to_string()))??;
        let state = app.state::<AppState>();
        let dev = device(&state)?;
        {
            let conn = state.db.lock().unwrap();
            let mut cfg = service::load_config(&conn)?.unwrap_or_default();
            cfg.url = url;
            cfg.email = Some(email);
            service::save_config(&conn, &cfg)?;
        }
        service::store_refresh(&dev, &session.refresh_token)?;
        let svc = app.state::<SyncService>();
        svc.reset();
        svc.kick();
        service::refresh_status(&app, None);
        return Ok(());
    }
    #[cfg(not(feature = "store"))]
    {
        let _ = (app, code, password, email);
        Err(AppError::invalid("Reset your password from your Supabase project's dashboard."))
    }
}

// ------------------------------------------------- linking more of my own devices

const LINK_PREFIX: &str = "DEARUS-LINK1-";

#[derive(Serialize, Deserialize)]
struct LinkPayload {
    v: u32,
    /// space id
    s: String,
    /// one-time secret (only its hash is on the server)
    i: String,
    /// the space key: the code is as private as the recovery phrase
    k: String,
    /// the account to sign in with on the new device
    e: String,
    /// this device's name, to show "bringing your memories from …"
    f: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceLink {
    pub code: String,
    /// The same code as a QR code (SVG), for a phone's camera.
    pub qr_svg: String,
    pub expires_in_minutes: u32,
}

/// "Link my phone / another PC": a 15-minute code only this account can use.
#[tauri::command]
pub async fn sync_create_device_link(app: AppHandle) -> AppResult<DeviceLink> {
    #[cfg(feature = "store")]
    {
        let a = app.clone();
        return tauri::async_runtime::spawn_blocking(move || -> AppResult<DeviceLink> {
            let state = a.state::<AppState>();
            let (cfg, dev) = {
                let conn = state.db.lock().unwrap();
                (service::load_config(&conn)?, crate::sync::device_id(&conn)?)
            };
            let cfg = cfg.ok_or_else(|| AppError::invalid("Set up partner sync first."))?;
            let space = cfg.space.clone().ok_or_else(|| AppError::invalid("Create your shared space first."))?;
            let email = cfg.email.clone().unwrap_or_default();
            let key = service::load_key(&dev)?.ok_or_else(|| AppError::invalid("The space key is missing on this PC."))?;
            let relay = service::connect(&a)?;
            let existing: Vec<String> = crate::sync::relay::Relay::devices(relay.as_ref())?.into_iter().map(|d| d.id).collect();
            let secret = random_token(24);
            relay.create_device_link(&sha256_hex(&secret))?;
            {
                let conn = state.db.lock().unwrap();
                crate::sync::bootstrap::remember_devices(&conn, &existing)?;
                crate::sync::bootstrap::mark_link_created(&conn)?;
            }
            let payload = LinkPayload { v: 1, s: space, i: secret, k: key.to_b64(), e: email, f: service::device_name() };
            let code = format!("{LINK_PREFIX}{}", URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload)?));
            let qr = qrcode::QrCode::with_error_correction_level(code.as_bytes(), qrcode::EcLevel::L).map_err(|e| AppError::invalid(e.to_string()))?;
            let qr_svg = qr.render::<qrcode::render::svg::Color>().min_dimensions(220, 220).quiet_zone(true).build();
            Ok(DeviceLink { code, qr_svg, expires_in_minutes: 15 })
        })
        .await
        .map_err(|e| AppError::invalid(e.to_string()))?;
    }
    #[cfg(not(feature = "store"))]
    {
        let _ = app;
        Err(AppError::invalid("Linking devices is part of Dear Us from the Microsoft Store."))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkPreview {
    pub email: String,
    pub from_device: String,
}

/// Reads a link code (without using it), so the new device can show whose account it is.
#[tauri::command]
pub fn sync_read_device_link(code: String) -> AppResult<LinkPreview> {
    let p = parse_link(&code)?;
    Ok(LinkPreview { email: p.e, from_device: p.f })
}

fn parse_link(code: &str) -> AppResult<LinkPayload> {
    let body = code.trim().strip_prefix(LINK_PREFIX).ok_or_else(|| {
        if code.trim().starts_with(INVITE_PREFIX) {
            AppError::invalid("That's a partner invite. To link your own device, use \"Link a device\" on your other device.")
        } else {
            AppError::invalid("That doesn't look like a Dear Us link code.")
        }
    })?;
    let raw = URL_SAFE_NO_PAD.decode(body.trim()).map_err(|_| AppError::invalid("The link code seems cut off. Copy it again."))?;
    let p: LinkPayload = serde_json::from_slice(&raw).map_err(|_| AppError::invalid("The link code seems damaged. Copy it again."))?;
    SpaceKey::from_b64(&p.k)?;
    Ok(p)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(feature = "store"), allow(dead_code))]
pub struct LinkInput {
    pub code: String,
    pub password: String,
}

/// New device: joins with the link code from my other device, signed in as the SAME person.
/// Everything already written arrives as one snapshot, then photos follow.
#[tauri::command]
pub async fn sync_link_device(app: AppHandle, input: LinkInput) -> AppResult<()> {
    let p = parse_link(&input.code)?;
    let email = check_email(&p.e)?;
    #[cfg(feature = "store")]
    {
        let a = app.clone();
        tauri::async_runtime::spawn_blocking(move || -> AppResult<()> {
            let state = a.state::<AppState>();
            {
                let conn = state.db.lock().unwrap();
                crate::sync::bootstrap::ensure_fresh(&conn)?;
            }
            let url = backend::normalize_url("")?;
            let session = backend::sign_in(&url, "", &email, &input.password)?;
            let space = backend::redeem_invite(&url, "", &session, &p.i)?;
            if space != p.s {
                return Err(AppError::invalid("That link code doesn't match its space. Make a new one."));
            }
            let dev = device(&state)?;
            service::store_refresh(&dev, &session.refresh_token)?;
            let relay = Arc::new(AccountRelay::new(&url, "", &space, session, service::refresh_saver(&dev)));
            relay.register_device(&dev, &service::device_name())?;
            service::store_key(&dev, &SpaceKey::from_b64(&p.k)?)?;
            {
                let conn = state.db.lock().unwrap();
                crate::sync::bootstrap::clear_empty_profile(&conn)?;
                service::save_config(&conn, &Config { url, anon: String::new(), space: Some(space), email: Some(email) })?;
                crate::sync::bootstrap::mark_pending(&conn, &p.f)?;
            }
            let svc = a.state::<SyncService>();
            svc.set_relay(relay);
            svc.kick();
            Ok(())
        })
        .await
        .map_err(|e| AppError::invalid(e.to_string()))??;
        service::refresh_status(&app, None);
        return Ok(());
    }
    #[cfg(not(feature = "store"))]
    {
        let _ = (app, input, email);
        Err(AppError::invalid("Linking devices is part of Dear Us from the Microsoft Store."))
    }
}

/// Store edition: deletes this person's Dear Us account. Memories on this PC stay exactly as
/// they are (and keep working offline); the partner keeps the shared space and its plan.
#[tauri::command]
pub async fn sync_delete_account(app: AppHandle, password: String) -> AppResult<()> {
    #[cfg(feature = "store")]
    {
        let state = app.state::<AppState>();
        let dev = device(&state)?;
        let token = crate::secrets::get(&service::secret_name(&dev, "refresh"))?.ok_or_else(|| AppError::invalid("Sign in first (Settings → Partner sync), then delete the account."))?;
        let url = backend::normalize_url("")?;
        tauri::async_runtime::spawn_blocking(move || crate::sync::cloud::delete_account(&url, &token, &password))
            .await
            .map_err(|e| AppError::invalid(e.to_string()))??;
        {
            let conn = state.db.lock().unwrap();
            service::clear_config(&conn, &dev)?;
            conn.execute("DELETE FROM local_meta WHERE key LIKE 'plan.%' OR key LIKE 'billing.%' OR key LIKE 'link.%'", [])?;
        }
        app.state::<SyncService>().reset();
        service::refresh_status(&app, None);
        return Ok(());
    }
    #[cfg(not(feature = "store"))]
    {
        let _ = (app, password);
        Err(AppError::invalid("Accounts live in your own Supabase project in this edition."))
    }
}

/// Store edition: "Disconnect from my partner". This person leaves the shared space; nothing is
/// shared any more in either direction. Memories already on this PC (including what the partner
/// shared before) stay here, and the partner keeps the space and its plan.
#[tauri::command]
pub async fn sync_leave_space(app: AppHandle) -> AppResult<()> {
    #[cfg(feature = "store")]
    {
        let a = app.clone();
        tauri::async_runtime::spawn_blocking(move || service::connect(&a)?.leave_space())
            .await
            .map_err(|e| AppError::invalid(e.to_string()))??;
        let state = app.state::<AppState>();
        {
            let conn = state.db.lock().unwrap();
            let dev = crate::sync::device_id(&conn)?;
            service::clear_config(&conn, &dev)?;
            conn.execute("DELETE FROM local_meta WHERE key LIKE 'plan.%' OR key LIKE 'billing.%' OR key LIKE 'link.%'", [])?;
        }
        app.state::<SyncService>().reset();
        service::refresh_status(&app, None);
        return Ok(());
    }
    #[cfg(not(feature = "store"))]
    {
        let _ = app;
        Err(AppError::invalid("Use \"Stop syncing on this PC\" in this edition."))
    }
}
