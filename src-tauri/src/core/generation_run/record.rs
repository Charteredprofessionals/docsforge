//! generation_run/record.rs — Append-only generation run record (TASK-116, REQ-033 / REQ-034).
//!
//! A `GenerationRun` captures the immutable context of one document-generation
//! invocation: the exact matter, bundle version, input snapshot hash, and engine
//! version. Runs are append-only — a rerun always creates a new record and never
//! mutates historical ones (REQ-034).

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::error::DocForgeError;

/// Lifecycle status of a generation run, matching the DB CHECK constraint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Partial,
}

impl RunStatus {
    fn as_str(&self) -> &str {
        match self {
            RunStatus::Pending => "pending",
            RunStatus::Running => "running",
            RunStatus::Succeeded => "succeeded",
            RunStatus::Failed => "failed",
            RunStatus::Partial => "partial",
        }
    }

    fn from_str(s: &str) -> Result<Self, DocForgeError> {
        match s {
            "pending" => Ok(RunStatus::Pending),
            "running" => Ok(RunStatus::Running),
            "succeeded" => Ok(RunStatus::Succeeded),
            "failed" => Ok(RunStatus::Failed),
            "partial" => Ok(RunStatus::Partial),
            _ => Err(DocForgeError::InvalidInput(format!("Unknown run status: '{s}'"))),
        }
    }
}

/// One row in the `generation_runs` table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerationRun {
    pub id: String,
    pub matter_id: String,
    pub bundle_id: String,
    pub bundle_version_id: String,
    pub input_snapshot_json: Option<String>,
    pub input_snapshot_hash: Option<String>,
    pub engine_version: Option<String>,
    pub status: RunStatus,
    pub warnings_json: Option<String>,
    pub errors_json: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

/// The engine version stamped on every run record.
pub const ENGINE_VERSION: &str = "2.0.0";

/// Creates a new (append-only) generation run bound to a matter.
///
/// The `generation_runs` table is append-only (DB triggers reject UPDATE/DELETE
/// per REQ-034), so the run is written exactly once here with its known status.
/// The generation orchestrator (TASK-117) creates the run after execution with
/// the final status and inserts the per-document artifacts separately.
pub fn create_run(
    conn: &Connection,
    matter_id: &str,
    bundle_id: &str,
    bundle_version_id: &str,
    input_snapshot_json: Option<&str>,
    input_snapshot_hash: Option<&str>,
    engine_version: Option<&str>,
    status: RunStatus,
) -> Result<GenerationRun, DocForgeError> {
    let now = chrono::Utc::now().to_rfc3339();
    let id = format!("run_{}", Uuid::new_v4());
    let is_final = matches!(status, RunStatus::Succeeded | RunStatus::Failed | RunStatus::Partial);

    conn.execute(
        "INSERT INTO generation_runs
         (id, matter_id, bundle_id, bundle_version_id, input_snapshot_json, input_snapshot_hash, engine_version, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            id,
            matter_id,
            bundle_id,
            bundle_version_id,
            input_snapshot_json,
            input_snapshot_hash,
            engine_version.or(Some(ENGINE_VERSION)),
            status.as_str(),
            now
        ],
    )
    .map_err(|e| DocForgeError::StorageIo(format!("Insert generation_run: {e}")))?;

    Ok(GenerationRun {
        id,
        matter_id: matter_id.to_string(),
        bundle_id: bundle_id.to_string(),
        bundle_version_id: bundle_version_id.to_string(),
        input_snapshot_json: input_snapshot_json.map(str::to_string),
        input_snapshot_hash: input_snapshot_hash.map(str::to_string),
        engine_version: engine_version.map(str::to_string).or_else(|| Some(ENGINE_VERSION.to_string())),
        status,
        warnings_json: None,
        errors_json: None,
        created_at: now.clone(),
        completed_at: if is_final {
            Some(now)
        } else {
            None
        },
    })
}

/// Fetches a run by id.
pub fn get_run(conn: &Connection, run_id: &str) -> Result<Option<GenerationRun>, DocForgeError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, matter_id, bundle_id, bundle_version_id, input_snapshot_json,
                    input_snapshot_hash, engine_version, status, warnings_json, errors_json,
                    created_at, completed_at
             FROM generation_runs WHERE id = ?1",
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Prepare get_run: {e}")))?;
    let rows = stmt
        .query_map([run_id], row_to_run)
        .map_err(|e| DocForgeError::StorageIo(format!("Query run: {e}")))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| DocForgeError::StorageIo(format!("Map run row: {e}")))?);
    }
    Ok(out.into_iter().next())
}

/// Lists all runs for a matter, newest first.
pub fn list_runs(conn: &Connection, matter_id: &str) -> Result<Vec<GenerationRun>, DocForgeError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, matter_id, bundle_id, bundle_version_id, input_snapshot_json,
                    input_snapshot_hash, engine_version, status, warnings_json, errors_json,
                    created_at, completed_at
             FROM generation_runs WHERE matter_id = ?1 ORDER BY created_at DESC",
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Prepare list_runs: {e}")))?;
    let rows = stmt
        .query_map([matter_id], row_to_run)
        .map_err(|e| DocForgeError::StorageIo(format!("Query runs: {e}")))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| DocForgeError::StorageIo(format!("Map run row: {e}")))?);
    }
    Ok(out)
}

/// Computes the sha256 input hash of a canonicalized matter-data JSON blob.
pub fn compute_input_hash(snapshot_json: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(snapshot_json.as_bytes());
    let digest = hasher.finalize();
    format!("{digest:x}")
}

fn row_to_run(row: &rusqlite::Row) -> Result<GenerationRun, rusqlite::Error> {
    let status_str: String = row.get(7)?;
    Ok(GenerationRun {
        id: row.get(0)?,
        matter_id: row.get(1)?,
        bundle_id: row.get(2)?,
        bundle_version_id: row.get(3)?,
        input_snapshot_json: row.get(4)?,
        input_snapshot_hash: row.get(5)?,
        engine_version: row.get(6)?,
        status: RunStatus::from_str(&status_str).unwrap_or(RunStatus::Pending),
        warnings_json: row.get(8)?,
        errors_json: row.get(9)?,
        created_at: row.get(10)?,
        completed_at: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bundle::manifest::create_bundle;
    use crate::core::field_mapping::registry::create_field;
    use crate::core::field_mapping::schema::FieldType;
    use crate::core::matter::matter::create_matter;
    use crate::core::matter::matter_values::{matter_to_json, set_matter_value};
    use crate::schema::init_memory_db;

    fn setup() -> (Connection, String, String, String) {
        let conn = init_memory_db().expect("mem");
        let bundle = create_bundle(&conn, "Run Test", None, None).expect("bundle");
        let bv = conn
            .query_row(
                "SELECT id FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC LIMIT 1",
                [&bundle.id],
                |r| r.get::<_, String>(0),
            )
            .expect("bv");
        let matter = create_matter(&conn, &bundle.id, &bv, "M1", None, None).expect("matter");
        (conn, bundle.id, bv, matter.id)
    }

    #[test]
    fn test_run_captures_snapshot_append_only() {
        let (conn, bundle_id, bv, matter_id) = setup();
        create_field(
            &conn,
            &bv,
            &crate::core::field_mapping::schema::FieldDef {
                id: String::new(),
                field_id: "name".to_string(),
                label: "Name".to_string(),
                description: None,
                field_type: FieldType::Text,
                required: false,
                default: None,
                validation: None,
                group_id: None,
                options: Vec::new(),
                format: None,
                position: 0,
            },
        )
        .expect("field");
        set_matter_value(&conn, &matter_id, "name", &serde_json::json!("Acme")).expect("set");

        let snapshot = serde_json::to_string(&matter_to_json(&conn, &matter_id).expect("json")).expect("ser");
        let hash = compute_input_hash(&snapshot);

        let run = create_run(&conn, &matter_id, &bundle_id, &bv, Some(&snapshot), Some(&hash), None, RunStatus::Pending).expect("run");
        assert_eq!(run.status, RunStatus::Pending);
        assert_eq!(run.input_snapshot_hash.as_deref(), Some(hash.as_str()));
        assert_eq!(run.engine_version.as_deref(), Some(ENGINE_VERSION));

        // A second run for the same matter is a distinct append-only record.
        let run2 = create_run(&conn, &matter_id, &bundle_id, &bv, Some(&snapshot), Some(&hash), None, RunStatus::Succeeded).expect("run2");
        assert_ne!(run.id, run2.id);
        assert_eq!(run2.status, RunStatus::Succeeded);
        assert!(run2.completed_at.is_some());

        let runs = list_runs(&conn, &matter_id).expect("list");
        assert_eq!(runs.len(), 2);

        // The append-only guarantee is enforced at the DB layer: UPDATE must fail.
        let update = conn.execute(
            "UPDATE generation_runs SET status = 'running' WHERE id = ?1",
            [&run.id],
        );
        assert!(update.is_err(), "append-only generation_runs must reject UPDATE");
    }
}
