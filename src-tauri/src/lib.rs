mod backup;
#[cfg(feature = "store")]
mod billing;
mod commands;
mod cupid;
mod db;
mod error;
mod media;
#[allow(dead_code)]
mod secrets;
mod sync;
mod timelock;

use commands::{bucket as bc, journal as jc, media as mc, planner as pc, special as sc};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub data_dir: PathBuf,
    pub store: media::Store,
}

/// `DEARUS_DATA_DIR` lets two instances run side by side on one PC (sync testing).
fn resolve_data_dir(app: &tauri::App) -> PathBuf {
    match std::env::var_os("DEARUS_DATA_DIR") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => app.path().app_data_dir().expect("no app data dir available"),
    }
}

/// Recovery-bin expiry and orphaned-media cleanup. Failures here never block startup.
fn maintenance(conn: &mut Connection, store: &media::Store) {
    let result = (|| -> error::AppResult<()> {
        let (w, _) = match jc::ctx(conn) {
            Ok(c) => c,
            Err(_) => return Ok(()), // no profile yet
        };
        let days = db::repo::journal::DEFAULT_RETENTION_DAYS;
        db::repo::journal::purge_expired(conn, &w, days)?;
        db::repo::milestones::purge_expired(conn, &w, days)?;
        db::repo::bucket::purge_expired(conn, &w, days)?;
        db::repo::special::purge_expired(conn, &w, days)?;
        db::repo::planner::purge_expired(conn, &w, days)?;
        db::repo::questions::purge_expired(conn, &w, days)?;
        db::repo::letters::purge_expired(conn, &w, days)?;
        db::repo::engagement::purge_expired(conn, &w, days)?;
        // 24 h grace so photos added to a memory that's still being written aren't touched.
        media::collect_garbage(store, conn, 24)?;
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("maintenance skipped: {e}");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        // Remembers window size/position between launches.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init());
    // Personal edition: signed auto-updates from GitHub Releases. The Store edition has no
    // self-updater: the Microsoft Store updates it.
    #[cfg(feature = "updater")]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build()).plugin(tauri_plugin_process::init());
    // Store edition: checkout and the subscription page open in the person's browser.
    #[cfg(feature = "store")]
    let builder = builder.plugin(tauri_plugin_opener::init());
    builder
        .register_asynchronous_uri_scheme_protocol("media", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            std::thread::spawn(move || responder.respond(media::protocol::handle(&app, &request)));
        })
        .setup(|app| {
            let data_dir = resolve_data_dir(app);
            std::fs::create_dir_all(&data_dir)?;
            media::ffmpeg::set_tools_dir(data_dir.join("tools"));
            let mut conn = db::open(&data_dir.join(db::DB_FILE))?;
            sync::device_id(&conn)?;
            let store = media::Store::new(&data_dir)?;
            maintenance(&mut conn, &store);
            let pending = db::repo::media::needing_preview(&conn)?;
            app.manage(AppState { db: Mutex::new(conn), data_dir, store });
            let queue = media::worker::start(app.handle().clone());
            for m in pending {
                queue.push(m.r.id); // resume conversions interrupted last time
            }
            app.manage(queue);
            app.manage(sync::service::start(app.handle().clone()));
            app.state::<sync::service::SyncService>().kick();
            commands::backup::start_scheduler(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::settings_get_all,
            commands::settings_set,
            commands::profile_get,
            commands::profile_save,
            commands::dashboard_summary,
            commands::open_data_dir,
            jc::journal_list,
            jc::journal_get,
            jc::journal_create,
            jc::journal_update,
            jc::journal_delete,
            jc::journal_restore,
            jc::journal_purge,
            jc::journal_trash,
            jc::journal_tags,
            jc::journal_on_this_day,
            jc::timeline,
            commands::questions::question_custom_list,
            commands::questions::question_custom_create,
            commands::questions::question_custom_delete,
            commands::questions::question_custom_restore,
            commands::questions::question_answers,
            commands::questions::question_answer_save,
            commands::questions::question_answer_delete,
            commands::questions::question_answer_restore,
            commands::questions::question_daily,
            commands::questions::mood_list,
            commands::questions::mood_save,
            commands::questions::mood_delete,
            jc::milestone_create,
            jc::milestone_update,
            jc::milestone_delete,
            bc::bucket_list,
            bc::bucket_create,
            bc::bucket_update,
            bc::bucket_set_status,
            bc::bucket_link_entry,
            bc::bucket_delete,
            bc::bucket_restore,
            bc::bucket_trash,
            sc::special_list,
            sc::special_create,
            sc::special_update,
            sc::special_delete,
            sc::special_restore,
            pc::planner_list,
            pc::planner_create,
            pc::planner_update,
            pc::planner_set_status,
            pc::planner_toggle_check,
            pc::planner_link_entry,
            pc::planner_delete,
            pc::planner_restore,
            pc::idea_list,
            pc::idea_create,
            pc::idea_delete,
            pc::idea_restore,
            commands::letters::letter_list,
            commands::letters::letter_get,
            commands::letters::letter_save_draft,
            commands::letters::letter_send,
            commands::letters::letter_delete,
            commands::letters::letter_restore,
            commands::letters::letter_unlock,
            commands::engagement::reaction_list,
            commands::engagement::reaction_toggle,
            commands::engagement::comment_list,
            commands::engagement::comment_add,
            commands::engagement::comment_update,
            commands::engagement::comment_delete,
            commands::engagement::comment_restore,
            commands::sync::sync_status,
            commands::sync::sync_check_project,
            commands::sync::sync_connect_account,
            commands::sync::sync_create_space,
            commands::sync::sync_create_invite,
            commands::sync::sync_join,
            commands::sync::sync_sign_in,
            commands::sync::sync_now,
            commands::sync::sync_recovery_phrase,
            commands::sync::sync_revoke_device,
            commands::sync::sync_disconnect,
            commands::sync::sync_conflicts,
            commands::sync::sync_resolve_conflict,
            commands::sync::sync_set_direct,
            commands::sync::sync_forgot_password,
            commands::sync::sync_reset_password,
            commands::sync::sync_create_device_link,
            commands::sync::sync_read_device_link,
            commands::sync::sync_link_device,
            commands::sync::sync_delete_account,
            commands::sync::sync_leave_space,
            commands::backup::backup_status,
            commands::backup::backup_set_folder,
            commands::backup::backup_set_auto,
            commands::backup::backup_run,
            commands::backup::backup_inspect,
            commands::backup::backup_verify,
            commands::backup::backup_cloud_folders,
            commands::backup::archive_verify,
            commands::backup::backup_restore,
            commands::backup::bin_list,
            commands::backup::bin_restore,
            commands::backup::bin_purge,
            commands::plan::plan_get,
            commands::plan::billing_purchase,
            commands::plan::billing_restore,
            commands::plan::billing_manage,
            commands::cupid::cupid_status,
            commands::cupid::cupid_set_option,
            commands::cupid::cupid_set_key,
            commands::cupid::cupid_remove_key,
            commands::cupid::cupid_history,
            commands::cupid::cupid_clear_chat,
            commands::cupid::cupid_chat,
            commands::cupid::question_daily_smart,
            commands::cupid::cupid_date_ideas,
            commands::cupid::cupid_report,
            mc::media_import,
            mc::media_capabilities,
            mc::media_install_converter,
            mc::media_get,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Dear Us");
}
