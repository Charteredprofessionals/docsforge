//! migrations.rs — Versioned database schema migration ledger.
//!
//! Applies incremental schema updates safely and idempotently up to Data Model v2.

use rusqlite::{Connection, Result, Transaction};

pub const CURRENT_SCHEMA_VERSION: i32 = 2;

/// Applies all pending schema migrations inside a transaction.
pub fn apply_migrations(conn: &mut Connection) -> Result<()> {
    let current_version: i32 = conn.query_row(
        "PRAGMA user_version",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    if current_version < 1 {
        let tx = conn.transaction()?;
        migration_v1(&tx)?;
        tx.pragma_update(None, "user_version", 1)?;
        tx.commit()?;
    }

    let current_version: i32 = conn.query_row(
        "PRAGMA user_version",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    if current_version < 2 {
        let tx = conn.transaction()?;
        migration_v2(&tx)?;
        tx.pragma_update(None, "user_version", 2)?;
        tx.commit()?;
    }

    Ok(())
}

fn migration_v1(tx: &Transaction) -> Result<()> {
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version     INTEGER PRIMARY KEY,
            applied_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS legacy_templates (
            id            TEXT PRIMARY KEY,
            name          TEXT NOT NULL,
            original_docx BLOB NOT NULL,
            template_docx BLOB NOT NULL,
            fields_json   TEXT NOT NULL,
            created_at    TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;
    Ok(())
}

fn migration_v2(tx: &Transaction) -> Result<()> {
    tx.execute_batch(
        "
        -- 1. Orgs
        CREATE TABLE IF NOT EXISTS orgs (
            id            TEXT PRIMARY KEY,
            name          TEXT NOT NULL,
            plan          TEXT NOT NULL DEFAULT 'free',
            settings_json TEXT NOT NULL DEFAULT '{}',
            created_at    TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- 2. Users
        CREATE TABLE IF NOT EXISTS users (
            id         TEXT PRIMARY KEY,
            org_id     TEXT REFERENCES orgs(id) ON DELETE CASCADE,
            name       TEXT NOT NULL,
            email      TEXT UNIQUE NOT NULL,
            role       TEXT NOT NULL DEFAULT 'viewer',
            active     INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- 3. Templates (FS-backed, metadata only)
        CREATE TABLE IF NOT EXISTS templates (
            id              TEXT PRIMARY KEY,
            org_id          TEXT REFERENCES orgs(id) ON DELETE CASCADE,
            name            TEXT NOT NULL,
            category        TEXT NOT NULL DEFAULT 'general',
            description     TEXT NOT NULL DEFAULT '',
            current_version INTEGER NOT NULL DEFAULT 1,
            status          TEXT NOT NULL DEFAULT 'draft',
            storage_path    TEXT NOT NULL,
            fields_json     TEXT NOT NULL DEFAULT '[]',
            content_sha256  TEXT NOT NULL DEFAULT '',
            created_by      TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- 4. Template Versions
        CREATE TABLE IF NOT EXISTS template_versions (
            id             TEXT PRIMARY KEY,
            template_id    TEXT NOT NULL REFERENCES templates(id) ON DELETE CASCADE,
            version        INTEGER NOT NULL,
            status         TEXT NOT NULL DEFAULT 'draft',
            storage_path   TEXT NOT NULL,
            fields_json    TEXT NOT NULL DEFAULT '[]',
            content_sha256 TEXT NOT NULL DEFAULT '',
            note           TEXT NOT NULL DEFAULT '',
            created_by     TEXT,
            created_at     TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(template_id, version)
        );

        -- 5. Generation Log (Immutable Audit)
        CREATE TABLE IF NOT EXISTS generation_log (
            id           TEXT PRIMARY KEY,
            template_id  TEXT NOT NULL REFERENCES templates(id) ON DELETE SET NULL,
            version      INTEGER NOT NULL DEFAULT 1,
            output_name  TEXT NOT NULL,
            format       TEXT NOT NULL,
            status       TEXT NOT NULL DEFAULT 'success',
            user_id      TEXT,
            machine_id   TEXT,
            generated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- Append-only trigger for generation_log: PREVENT UPDATE
        CREATE TRIGGER IF NOT EXISTS prevent_generation_log_update
        BEFORE UPDATE ON generation_log
        BEGIN
            SELECT RAISE(FAIL, 'generation_log table is append-only; UPDATE operations are forbidden');
        END;

        -- Append-only trigger for generation_log: PREVENT DELETE
        CREATE TRIGGER IF NOT EXISTS prevent_generation_log_delete
        BEFORE DELETE ON generation_log
        BEGIN
            SELECT RAISE(FAIL, 'generation_log table is append-only; DELETE operations are forbidden');
        END;

        -- 6. Licenses
        CREATE TABLE IF NOT EXISTS licenses (
            id         TEXT PRIMARY KEY,
            org_id     TEXT REFERENCES orgs(id) ON DELETE CASCADE,
            user_id    TEXT REFERENCES users(id) ON DELETE SET NULL,
            tier       TEXT NOT NULL DEFAULT 'free',
            seats      INTEGER NOT NULL DEFAULT 1,
            devices    INTEGER NOT NULL DEFAULT 2,
            status     TEXT NOT NULL DEFAULT 'active',
            issued_at  TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT
        );

        -- 7. License Seats
        CREATE TABLE IF NOT EXISTS license_seats (
            id          TEXT PRIMARY KEY,
            license_id  TEXT NOT NULL REFERENCES licenses(id) ON DELETE CASCADE,
            user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            assigned_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(license_id, user_id)
        );

        -- 8. Devices
        CREATE TABLE IF NOT EXISTS devices (
            id            TEXT PRIMARY KEY,
            license_id    TEXT NOT NULL REFERENCES licenses(id) ON DELETE CASCADE,
            machine_id    TEXT NOT NULL,
            name          TEXT NOT NULL,
            registered_at TEXT NOT NULL DEFAULT (datetime('now')),
            last_seen_at  TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(license_id, machine_id)
        );

        -- 9. License Files (air-gapped offline licensing)
        CREATE TABLE IF NOT EXISTS license_files (
            id             TEXT PRIMARY KEY,
            license_id     TEXT NOT NULL REFERENCES licenses(id) ON DELETE CASCADE,
            file_signature TEXT NOT NULL,
            payload_b64    TEXT NOT NULL,
            installed_at   TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- 10. Telemetry Consent
        CREATE TABLE IF NOT EXISTS telemetry_consent (
            id            TEXT PRIMARY KEY DEFAULT 'default',
            opt_in        INTEGER NOT NULL DEFAULT 0,
            crash_reports INTEGER NOT NULL DEFAULT 0,
            updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- 11. Policy Config (Silent enterprise policy overrides)
        CREATE TABLE IF NOT EXISTS policy_config (
            key        TEXT PRIMARY KEY,
            value_json TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- 12. Webhook Subscriptions
        CREATE TABLE IF NOT EXISTS webhook_subscriptions (
            id         TEXT PRIMARY KEY,
            event_type TEXT NOT NULL,
            target_url TEXT NOT NULL,
            secret     TEXT NOT NULL,
            active     INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- 13. Audit Export Projection View
        CREATE VIEW IF NOT EXISTS view_audit_export AS
        SELECT
            g.id AS log_id,
            g.template_id,
            t.name AS template_name,
            g.version,
            g.output_name,
            g.format,
            g.status,
            g.user_id,
            g.machine_id,
            g.generated_at
        FROM generation_log g
        LEFT JOIN templates t ON g.template_id = t.id;

        -- Record migration
        INSERT OR REPLACE INTO schema_version (version) VALUES (2);
        ",
    )?;

    Ok(())
}
