//! migrations.rs — Versioned database schema migration ledger.
//!
//! Applies incremental schema updates safely and idempotently up to schema v5
//! (Data Model v3: Bundle + Matter domain).

use rusqlite::{Connection, Result, Transaction};

pub const CURRENT_SCHEMA_VERSION: i32 = 5;

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

    let has_v2_templates: bool = conn
        .prepare("PRAGMA table_info(templates)")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(1))
                .map(|rows| rows.filter_map(|r| r.ok()).any(|col| col == "org_id"))
        })
        .unwrap_or(false);

    if current_version < 2 || !has_v2_templates {
        let tx = conn.transaction()?;
        migration_v2(&tx)?;
        tx.pragma_update(None, "user_version", 2)?;
        tx.commit()?;
    }

    let has_v3_bundles: bool = conn
        .prepare("PRAGMA table_info(bundles)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|col| col == "id");

    if current_version < 3 || !has_v3_bundles {
        let tx = conn.transaction()?;
        migration_v3(&tx)?;
        tx.pragma_update(None, "user_version", 3)?;
        tx.commit()?;
    }

    let has_v4_bug_book: bool = conn
        .prepare("PRAGMA table_info(bug_book)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|col| col == "id");

    if current_version < 4 || !has_v4_bug_book {
        let tx = conn.transaction()?;
        migration_v4(&tx)?;
        tx.pragma_update(None, "user_version", 4)?;
        tx.commit()?;
    }

    let has_v5_bundle_versions: bool = conn
        .prepare("PRAGMA table_info(bundle_versions)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|col| col == "id");

    if current_version < 5 || !has_v5_bundle_versions {
        let tx = conn.transaction()?;
        migration_v5(&tx)?;
        tx.pragma_update(None, "user_version", 5)?;
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
    // If an old `templates` table exists without `org_id` column, drop it so v2 table is created cleanly.
    let has_org_id: bool = tx
        .prepare("PRAGMA table_info(templates)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|col| col == "org_id");

    if !has_org_id {
        let _ = tx.execute_batch("DROP TABLE IF EXISTS templates;");
    }

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

/// Migration v3 — Template Bundles (group templates for batch operations).
/// Adopted from the `templatebuilder` sibling project (high-impact feature).
fn migration_v3(tx: &Transaction) -> Result<()> {
    tx.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS bundles (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS bundle_templates (
            id          TEXT PRIMARY KEY,
            bundle_id   TEXT NOT NULL REFERENCES bundles(id) ON DELETE CASCADE,
            template_id TEXT NOT NULL,
            position    INTEGER NOT NULL DEFAULT 0,
            UNIQUE(bundle_id, template_id)
        );
        ",
    )?;
    Ok(())
}

/// Migration v4 — Bug Book: persistent crash/error log for the Admin Console.
/// Records automatic (captured) and manual bug entries with severity, status,
/// context, stack trace, and attachments, plus a child table for supplementary files.
fn migration_v4(tx: &Transaction) -> Result<()> {
    tx.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS bug_book (
            id           TEXT PRIMARY KEY,
            created_at   TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
            error_type   TEXT NOT NULL DEFAULT 'runtime_error',
            severity     TEXT NOT NULL DEFAULT 'medium',
            status       TEXT NOT NULL DEFAULT 'open',
            context      TEXT NOT NULL DEFAULT '',
            message      TEXT NOT NULL DEFAULT '',
            stack_trace  TEXT NOT NULL DEFAULT '',
            source       TEXT NOT NULL DEFAULT 'auto',
            category     TEXT NOT NULL DEFAULT 'uncategorized',
            keywords     TEXT NOT NULL DEFAULT '',
            resolved_by  TEXT,
            resolved_at  TEXT
        );

        CREATE TABLE IF NOT EXISTS bug_attachments (
            id         TEXT PRIMARY KEY,
            bug_id     TEXT NOT NULL REFERENCES bug_book(id) ON DELETE CASCADE,
            filename   TEXT NOT NULL,
            mime_type  TEXT NOT NULL DEFAULT 'application/octet-stream',
            data_b64   TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_bug_book_created ON bug_book(created_at);
        CREATE INDEX IF NOT EXISTS idx_bug_book_severity ON bug_book(severity);
        CREATE INDEX IF NOT EXISTS idx_bug_book_status ON bug_book(status);
        ",
    )?;
    Ok(())
}

/// Migration v5 — Bundle + Matter domain (Data Model v3, schema v5).
/// Adds the Bundle/Matter tables (bundle_versions, bundle_documents, fields,
/// field_groups, field_mappings, rules, matters, matter_values, generation_runs,
/// generated_documents), append-only triggers for generation_runs and published
/// bundle_versions, and promotes existing v1 template groups (`bundles` +
/// `bundle_templates`) into first `bundle_versions` snapshots. All v1–v4 tables
/// are preserved exactly (REQ-023, REQ-024, REQ-033, REQ-034).
fn migration_v5(tx: &Transaction) -> Result<()> {
    tx.execute_batch(
        "
        -- 1. Bundle Versions (immutable snapshot of a reusable Bundle definition)
        CREATE TABLE IF NOT EXISTS bundle_versions (
            id            TEXT PRIMARY KEY,
            bundle_id     TEXT NOT NULL,
            version       INTEGER NOT NULL,
            status        TEXT NOT NULL DEFAULT 'draft'
                          CHECK (status IN ('draft','review','published','archived')),
            manifest_json TEXT NOT NULL DEFAULT '{}',
            created_by    TEXT,
            created_at    TEXT NOT NULL DEFAULT (datetime('now')),
            note          TEXT,
            UNIQUE(bundle_id, version)
        );

        -- 2. Bundle Documents (ordered membership inside one Bundle Version)
        CREATE TABLE IF NOT EXISTS bundle_documents (
            id                TEXT PRIMARY KEY,
            bundle_version_id TEXT NOT NULL REFERENCES bundle_versions(id) ON DELETE CASCADE,
            template_id       TEXT REFERENCES templates(id),
            position          INTEGER NOT NULL DEFAULT 0,
            include_default   INTEGER NOT NULL DEFAULT 1,
            condition_ref     TEXT,
            created_at        TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- 3. Field Groups (shared vs document-specific)
        CREATE TABLE IF NOT EXISTS field_groups (
            id                TEXT PRIMARY KEY,
            bundle_version_id TEXT REFERENCES bundle_versions(id) ON DELETE CASCADE,
            name              TEXT NOT NULL,
            description       TEXT,
            scope             TEXT NOT NULL DEFAULT 'shared'
                              CHECK (scope IN ('shared','document_specific')),
            sort_order        INTEGER NOT NULL DEFAULT 0,
            created_at        TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- 4. Fields (canonical Bundle-level field schema)
        CREATE TABLE IF NOT EXISTS fields (
            id                TEXT PRIMARY KEY,
            bundle_version_id TEXT NOT NULL REFERENCES bundle_versions(id) ON DELETE CASCADE,
            field_id          TEXT NOT NULL,
            label             TEXT NOT NULL,
            description       TEXT,
            type              TEXT NOT NULL
                              CHECK (type IN ('text','multiline_text','number','currency',
                                              'percentage','date','datetime','boolean',
                                              'email','phone','url','select','multiselect')),
            required          INTEGER NOT NULL DEFAULT 0,
            default_json      TEXT,
            validation_json   TEXT,
            format            TEXT,
            group_id          TEXT,
            position          INTEGER NOT NULL DEFAULT 0,
            created_at        TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(bundle_version_id, field_id)
        );

        -- 5. Field Mappings (explicit placeholder -> canonical field)
        CREATE TABLE IF NOT EXISTS field_mappings (
            id                 TEXT PRIMARY KEY,
            bundle_version_id  TEXT NOT NULL REFERENCES bundle_versions(id) ON DELETE CASCADE,
            document_id        TEXT NOT NULL REFERENCES bundle_documents(id) ON DELETE CASCADE,
            placeholder        TEXT NOT NULL,
            canonical_field_id TEXT NOT NULL,
            created_at         TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(bundle_version_id, document_id, placeholder)
        );

        -- 6. Rules (deterministic conditional-document expressions)
        CREATE TABLE IF NOT EXISTS rules (
            id                TEXT PRIMARY KEY,
            bundle_version_id TEXT NOT NULL REFERENCES bundle_versions(id) ON DELETE CASCADE,
            document_id       TEXT REFERENCES bundle_documents(id) ON DELETE CASCADE,
            field_id          TEXT,
            operator          TEXT,
            value_json        TEXT,
            condition_expr    TEXT,
            description       TEXT,
            enabled           INTEGER NOT NULL DEFAULT 1,
            created_at        TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- 7. Matters (instance of exactly one Bundle Version)
        CREATE TABLE IF NOT EXISTS matters (
            id                  TEXT PRIMARY KEY,
            name                TEXT NOT NULL,
            bundle_id           TEXT NOT NULL REFERENCES bundles(id),
            bundle_version_id   TEXT NOT NULL REFERENCES bundle_versions(id),
            org_id              TEXT REFERENCES orgs(id),
            status              TEXT NOT NULL DEFAULT 'draft'
                                CHECK (status IN ('draft','ready','generating','generated')),
            created_by          TEXT,
            created_at          TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at          TEXT NOT NULL DEFAULT (datetime('now')),
            input_snapshot_json TEXT,
            input_snapshot_hash TEXT
        );

        -- 8. Matter Values (row-per-value editable store)
        CREATE TABLE IF NOT EXISTS matter_values (
            id                 TEXT PRIMARY KEY,
            matter_id          TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
            canonical_field_id TEXT NOT NULL,
            value_json         TEXT NOT NULL,
            updated_at         TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(matter_id, canonical_field_id)
        );

        -- 9. Generation Runs (append-only run records)
        CREATE TABLE IF NOT EXISTS generation_runs (
            id                  TEXT PRIMARY KEY,
            matter_id           TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
            bundle_id           TEXT NOT NULL REFERENCES bundles(id),
            bundle_version_id   TEXT NOT NULL REFERENCES bundle_versions(id),
            input_snapshot_json TEXT,
            input_snapshot_hash TEXT,
            engine_version      TEXT,
            status              TEXT NOT NULL DEFAULT 'pending'
                                CHECK (status IN ('pending','running','succeeded','failed','partial')),
            warnings_json       TEXT,
            errors_json         TEXT,
            created_at          TEXT NOT NULL DEFAULT (datetime('now')),
            completed_at        TEXT
        );

        -- 10. Generated Documents (output artifacts, never mutated)
        CREATE TABLE IF NOT EXISTS generated_documents (
            id                 TEXT PRIMARY KEY,
            generation_run_id  TEXT NOT NULL REFERENCES generation_runs(id) ON DELETE CASCADE,
            bundle_document_id TEXT REFERENCES bundle_documents(id),
            document_name      TEXT NOT NULL,
            format             TEXT NOT NULL CHECK (format IN ('docx','pdf')),
            output_path        TEXT NOT NULL,
            content_sha256     TEXT,
            status             TEXT NOT NULL DEFAULT 'succeeded',
            created_at         TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(generation_run_id, bundle_document_id)
        );

        -- Indexes
        CREATE INDEX IF NOT EXISTS idx_bundle_documents_version ON bundle_documents(bundle_version_id);
        CREATE INDEX IF NOT EXISTS idx_field_groups_version ON field_groups(bundle_version_id);
        CREATE INDEX IF NOT EXISTS idx_fields_version ON fields(bundle_version_id);
        CREATE INDEX IF NOT EXISTS idx_field_mappings_field ON field_mappings(canonical_field_id);
        CREATE INDEX IF NOT EXISTS idx_matters_bundle_version ON matters(bundle_version_id);
        CREATE INDEX IF NOT EXISTS idx_matters_bundle ON matters(bundle_id);
        CREATE INDEX IF NOT EXISTS idx_matter_values_matter ON matter_values(matter_id);
        CREATE INDEX IF NOT EXISTS idx_generation_runs_matter ON generation_runs(matter_id);
        CREATE INDEX IF NOT EXISTS idx_generation_runs_version ON generation_runs(bundle_version_id);
        CREATE INDEX IF NOT EXISTS idx_generated_documents_run ON generated_documents(generation_run_id);

        -- Append-only trigger for generation_runs: PREVENT UPDATE
        CREATE TRIGGER IF NOT EXISTS prevent_generation_runs_update
        BEFORE UPDATE ON generation_runs
        BEGIN
            SELECT RAISE(FAIL, 'generation_runs table is append-only; UPDATE operations are forbidden');
        END;

        -- Append-only trigger for generation_runs: PREVENT DELETE
        CREATE TRIGGER IF NOT EXISTS prevent_generation_runs_delete
        BEFORE DELETE ON generation_runs
        BEGIN
            SELECT RAISE(FAIL, 'generation_runs table is append-only; DELETE operations are forbidden');
        END;

        -- Published Bundle Versions are immutable (REQ-024): PREVENT UPDATE
        CREATE TRIGGER IF NOT EXISTS prevent_bundle_versions_published_update
        BEFORE UPDATE ON bundle_versions
        FOR EACH ROW
        WHEN OLD.status = 'published'
        BEGIN
            SELECT RAISE(FAIL, 'published bundle_versions is immutable (REQ-024); UPDATE operations are forbidden');
        END;

        -- Published Bundle Versions are immutable (REQ-024): PREVENT DELETE
        CREATE TRIGGER IF NOT EXISTS prevent_bundle_versions_published_delete
        BEFORE DELETE ON bundle_versions
        FOR EACH ROW
        WHEN OLD.status = 'published'
        BEGIN
            SELECT RAISE(FAIL, 'published bundle_versions is immutable (REQ-024); DELETE operations are forbidden');
        END;

        -- Promote v1 template groups into first Bundle Version snapshots.
        -- Each existing `bundles` row becomes version 1 draft; its `bundle_templates`
        -- members are copied into `bundle_documents` (reusing the member row id).
        -- Orphan template references are skipped so FK checks stay clean.
        INSERT OR IGNORE INTO bundle_versions (id, bundle_id, version, status, manifest_json, created_by, note)
        SELECT hex(randomblob(16)), b.id, 1, 'draft', '{}', 'user-local', 'migrated from v1 bundle group'
        FROM bundles b;

        INSERT OR IGNORE INTO bundle_documents (id, bundle_version_id, template_id, position, include_default, condition_ref)
        SELECT bt.id, bv.id, bt.template_id, bt.position, 1, NULL
        FROM bundle_templates bt
        JOIN bundle_versions bv ON bv.bundle_id = bt.bundle_id AND bv.version = 1
        WHERE bt.template_id IN (SELECT id FROM templates);

        -- Record migration
        INSERT OR REPLACE INTO schema_version (version) VALUES (5);
        ",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::init_memory_db;
    use rusqlite::params;

    const V5_TABLES: [&str; 10] = [
        "bundle_versions",
        "bundle_documents",
        "field_groups",
        "fields",
        "field_mappings",
        "rules",
        "matters",
        "matter_values",
        "generation_runs",
        "generated_documents",
    ];

    /// Applies the v1..v4 migration chain only, leaving a DB at `user_version = 4`.
    fn apply_to_v4(conn: &mut Connection) {
        let tx = conn.transaction().expect("begin tx");
        migration_v1(&tx).expect("v1");
        migration_v2(&tx).expect("v2");
        migration_v3(&tx).expect("v3");
        migration_v4(&tx).expect("v4");
        tx.pragma_update(None, "user_version", 4).expect("set v4");
        tx.commit().expect("commit v4");
    }

    fn user_version(conn: &Connection) -> i32 {
        conn.query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read user_version")
    }

    fn insert_bundle_seed(conn: &Connection) {
        conn.execute(
            "INSERT INTO bundles (id, name, description) VALUES ('b1', 'Closing Set', '')",
            [],
        )
        .expect("insert bundle");
        conn.execute(
            "INSERT INTO bundle_versions (id, bundle_id, version, status) VALUES ('bv1', 'b1', 1, 'draft')",
            [],
        )
        .expect("insert bundle_version");
        conn.execute(
            "INSERT INTO matters (id, name, bundle_id, bundle_version_id) VALUES ('m1', 'Acme', 'b1', 'bv1')",
            [],
        )
        .expect("insert matter");
    }

    #[test]
    fn test_apply_migrations_twice_is_idempotent_and_lands_on_v5() {
        let db_path = std::env::temp_dir()
            .join(format!("docforge_migration_test_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));

        let mut conn = Connection::open(&db_path).expect("open temp db");
        conn.execute_batch("PRAGMA foreign_keys = ON;").expect("pragma");
        apply_migrations(&mut conn).expect("first apply");
        apply_migrations(&mut conn).expect("second apply");
        assert_eq!(user_version(&conn), 5);
        assert_eq!(user_version(&conn), CURRENT_SCHEMA_VERSION);

        drop(conn);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[test]
    fn test_v5_creates_all_bundle_and_matter_tables() {
        let conn = init_memory_db().expect("init");
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1")
            .expect("prepare sqlite_master query");
        for name in V5_TABLES {
            let found: bool = stmt
                .query_row(params![name], |row| row.get::<_, String>(0))
                .map(|found_name| found_name == name)
                .unwrap_or(false);
            assert!(found, "missing v5 table: {name}");
        }
    }

    #[test]
    fn test_generation_runs_rejects_update_and_delete() {
        let conn = init_memory_db().expect("init");
        insert_bundle_seed(&conn);
        conn.execute(
            "INSERT INTO generation_runs (id, matter_id, bundle_id, bundle_version_id, status)
             VALUES ('r1', 'm1', 'b1', 'bv1', 'pending')",
            [],
        )
        .expect("insert generation run");

        let update_err = conn.execute(
            "UPDATE generation_runs SET status = 'succeeded' WHERE id = 'r1'",
            [],
        );
        assert!(update_err.is_err(), "append-only generation_runs must reject UPDATE");

        let delete_err = conn.execute("DELETE FROM generation_runs WHERE id = 'r1'", []);
        assert!(delete_err.is_err(), "append-only generation_runs must reject DELETE");
    }

    #[test]
    fn test_published_bundle_version_is_immutable_but_draft_is_not() {
        let conn = init_memory_db().expect("init");
        conn.execute(
            "INSERT INTO bundles (id, name, description) VALUES ('b1', 'Closing Set', '')",
            [],
        )
        .expect("insert bundle");
        conn.execute(
            "INSERT INTO bundle_versions (id, bundle_id, version, status) VALUES ('bv-pub', 'b1', 1, 'published')",
            [],
        )
        .expect("insert published version");
        conn.execute(
            "INSERT INTO bundle_versions (id, bundle_id, version, status) VALUES ('bv-draft', 'b1', 2, 'draft')",
            [],
        )
        .expect("insert draft version");

        let update_err = conn.execute(
            "UPDATE bundle_versions SET note = 'x' WHERE id = 'bv-pub'",
            [],
        );
        assert!(update_err.is_err(), "published bundle_versions must reject UPDATE");

        let delete_err = conn.execute("DELETE FROM bundle_versions WHERE id = 'bv-pub'", []);
        assert!(delete_err.is_err(), "published bundle_versions must reject DELETE");

        conn.execute(
            "UPDATE bundle_versions SET note = 'editable' WHERE id = 'bv-draft'",
            [],
        )
        .expect("draft bundle_versions must allow UPDATE");
        let note: Option<String> = conn
            .query_row(
                "SELECT note FROM bundle_versions WHERE id = 'bv-draft'",
                [],
                |row| row.get(0),
            )
            .expect("read draft note");
        assert_eq!(note.as_deref(), Some("editable"));
    }

    #[test]
    fn test_v4_to_v5_promotes_bundles_to_versions() {
        let mut conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch("PRAGMA foreign_keys = ON;").expect("pragma");
        apply_to_v4(&mut conn);

        conn.execute(
            "INSERT INTO templates (id, name, storage_path) VALUES ('tpl-1', 'Agreement', '/tmp/agreement.docx')",
            [],
        )
        .expect("insert template 1");
        conn.execute(
            "INSERT INTO templates (id, name, storage_path) VALUES ('tpl-2', 'Annex', '/tmp/annex.docx')",
            [],
        )
        .expect("insert template 2");
        conn.execute(
            "INSERT INTO bundles (id, name, description) VALUES ('b1', 'Closing Set', '')",
            [],
        )
        .expect("insert bundle");
        conn.execute(
            "INSERT INTO bundle_templates (id, bundle_id, template_id, position) VALUES ('bt1', 'b1', 'tpl-1', 0)",
            [],
        )
        .expect("insert bundle_template 1");
        conn.execute(
            "INSERT INTO bundle_templates (id, bundle_id, template_id, position) VALUES ('bt2', 'b1', 'tpl-2', 1)",
            [],
        )
        .expect("insert bundle_template 2");

        apply_migrations(&mut conn).expect("apply v5");
        assert_eq!(user_version(&conn), 5);

        let (bv_id, bv_version, bv_status, bv_note): (String, i32, String, Option<String>) = conn
            .query_row(
                "SELECT id, version, status, note FROM bundle_versions WHERE bundle_id = 'b1'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                    ))
                },
            )
            .expect("read promoted bundle_version");
        assert_eq!(bv_version, 1);
        assert_eq!(bv_status, "draft");
        assert_eq!(bv_note.as_deref(), Some("migrated from v1 bundle group"));

        let doc_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM bundle_documents WHERE bundle_version_id = ?1",
                params![bv_id],
                |row| row.get(0),
            )
            .expect("count promoted documents");
        assert_eq!(doc_count, 2, "every bundle_template member must be copied");

        let (tpl, pos): (String, i32) = conn
            .query_row(
                "SELECT template_id, position FROM bundle_documents
                 WHERE bundle_version_id = ?1 AND template_id = 'tpl-2'",
                params![bv_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read promoted document");
        assert_eq!(tpl, "tpl-2");
        assert_eq!(pos, 1, "member position must be preserved");

        let bt_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM bundle_templates WHERE bundle_id = 'b1'",
                [],
                |row| row.get(0),
            )
            .expect("count v4 grouping rows");
        assert_eq!(bt_count, 2, "v1 grouping tables must remain untouched");

        let mut fk_stmt = conn.prepare("PRAGMA foreign_key_check").expect("prepare fk check");
        let mut fk_rows = fk_stmt.query([]).expect("run fk check");
        let fk_row = fk_rows.next().expect("read fk check row");
        assert!(
            fk_row.is_none(),
            "foreign_key_check must report zero rows"
        );
    }
}
