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
            commands::update_template,
            commands::seed_sample_template,
            commands::list_templates,
            commands::get_template,
            commands::get_template_fields,
            commands::export_template_fields_csv,
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
            // v2 Bundle + Matter commands
            commands::create_bundle_v2_cmd,
            commands::list_bundles_v2_cmd,
            commands::get_bundle_v2_cmd,
            commands::create_draft_version_cmd,
            commands::publish_version_cmd,
            commands::review_version_cmd,
            commands::archive_version_cmd,
            commands::list_versions_cmd,
            commands::get_manifest_cmd,
            commands::save_manifest_cmd,
            commands::export_bundle_dfpkg_cmd,
            commands::import_bundle_dfpkg_cmd,
            commands::create_field_cmd,
            commands::update_field_cmd,
            commands::list_fields_cmd,
            commands::remove_field_cmd,
            commands::create_field_group_cmd,
            commands::list_field_groups_cmd,
            commands::create_group_cmd,
            commands::list_groups_shared_first_cmd,
            commands::assign_field_to_group_cmd,
            commands::group_summary_cmd,
            commands::set_mapping_cmd,
            commands::list_mappings_cmd,
            commands::find_unmapped_placeholders_cmd,
            commands::create_matter_cmd,
            commands::get_matter_cmd,
            commands::list_matters_cmd,
            commands::update_matter_status_cmd,
            commands::delete_matter_cmd,
            commands::set_matter_value_cmd,
            commands::get_matter_value_cmd,
            commands::list_matter_values_cmd,
            commands::matter_to_json_cmd,
            commands::render_matter_form_cmd,
            commands::populate_matter_field_cmd,
            commands::validate_matter_cmd,
            commands::add_rule_cmd,
            commands::remove_rule_cmd,
            commands::list_rules_cmd,
            commands::evaluate_rules_cmd,
            commands::evaluate_preview_cmd,
            commands::validate_rule_expression_cmd,
            commands::execute_run_cmd,
            commands::create_run_cmd,
            commands::get_run_cmd,
            commands::list_runs_cmd,
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
            commands::get_telemetry_consent,
            commands::set_telemetry_consent,
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
