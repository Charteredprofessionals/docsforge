pub mod core;
pub mod infra;
pub mod migrations;
pub mod services;
mod commands;
mod schema;

use rusqlite::Connection;
use std::sync::Mutex;

pub struct AppState {
    pub db: Mutex<Connection>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            db: Mutex::new(schema::init_db().expect("Failed to initialize database")),
        })
        .invoke_handler(tauri::generate_handler![
            commands::upload_docx,
            commands::save_template,
            commands::list_templates,
            commands::get_template,
            commands::delete_template,
            commands::export_to_pdf,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
