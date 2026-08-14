pub mod core;
pub mod infra;
pub mod migrations;
pub mod services;
pub mod commands;
pub mod schema;

use rusqlite::Connection;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Connection>,
}

pub fn run() {
    install_crash_hook();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            db: Mutex::new(schema::init_db().expect("Failed to initialize database")),
        })
        .setup(|app| {
            // Initialize the local user on first run
            let db = app.state::<AppState>();
            let conn = db.db.lock().expect("DB lock in setup");
            let _ = core::governance::initialize_local_user(&conn);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::upload_docx,
            commands::save_template,
            commands::list_templates,
            commands::get_template,
            commands::fill_template,
            commands::delete_template,
            commands::export_to_pdf,
            commands::backup_database,
            commands::restore_database,
            commands::delete_database,
            commands::create_bundle_cmd,
            commands::list_bundles_cmd,
            commands::get_bundle_templates_cmd,
            commands::delete_bundle_cmd,
            commands::add_template_to_bundle_cmd,
            commands::remove_template_from_bundle_cmd,
            commands::log_bug,
            commands::create_bug_entry,
            commands::list_bugs,
            commands::get_bug,
            commands::update_bug_status,
            commands::add_bug_attachment,
            commands::export_bugs_csv,
            commands::export_bugs_pdf,
            commands::get_current_user,
            commands::set_user_role,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Installs a global panic hook that records any uncaught Rust panic as a
/// `critical` bug entry in the Bug Book before the process aborts. Best-effort:
/// failures here must never interfere with the panic's normal abort behavior.
fn install_crash_hook() {
    std::panic::set_hook(Box::new(|info| {
        let payload = match info.payload().downcast_ref::<&str>() {
            Some(s) => (*s).to_string(),
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => s.clone(),
                None => "unknown panic payload".to_string(),
            },
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown location".to_string());

        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Ok(conn) = schema::init_db() {
                // Bound the wait so a contended DB lock can't delay the abort.
                let _ = conn.execute_batch("PRAGMA busy_timeout = 1000");
                let _ = core::bug_book::record_crash(&conn, &payload, &location);
            }
        }));
    }));
}
