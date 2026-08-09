//! schema.rs — Database schema management for Data Model v2.
//!
//! Manages connection initialization, schema bootstrapping, triggers, and views.
//! Document files are stored on disk (`storage_path`); database stores metadata and SHA-256 hashes.

use rusqlite::{Connection, Result};
use std::path::PathBuf;

use crate::migrations::apply_migrations;

/// Returns the platform app data directory path for `docforge.db`.
pub fn get_db_path() -> PathBuf {
    let data_dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    let app_dir = data_dir.join("docforge");
    std::fs::create_dir_all(&app_dir).expect("Failed to create app data directory");
    app_dir.join("docforge.db")
}

/// Initializes the SQLite database connection, enabling WAL mode and foreign keys,
/// and applies all schema migrations up to current version.
pub fn init_db() -> Result<Connection> {
    let db_path = get_db_path();
    let mut conn = Connection::open(db_path)?;

    // Enable WAL mode & foreign key constraints
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;",
    )?;

    apply_migrations(&mut conn)?;

    Ok(conn)
}

/// Helper function to open an in-memory SQLite connection for testing schema DDL and queries.
pub fn init_memory_db() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;",
    )?;
    apply_migrations(&mut conn)?;
    Ok(conn)
}
