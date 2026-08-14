//! matter/matter.rs — Matter CRUD (TASK-110, REQ-030).
//!
//! A Matter is a structured data instance bound to exactly one Bundle Version.
//! It holds the runtime values for the fields defined in that version, and is
//! the entry point for document generation (REQ-030).

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::error::DocForgeError;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Lifecycle status of a Matter, matching the DB CHECK constraint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MatterStatus {
    Draft,
    Ready,
    Generating,
    Generated,
}

impl std::fmt::Display for MatterStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl MatterStatus {
    fn as_str(&self) -> &str {
        match self {
            MatterStatus::Draft => "draft",
            MatterStatus::Ready => "ready",
            MatterStatus::Generating => "generating",
            MatterStatus::Generated => "generated",
        }
    }

    fn from_str(s: &str) -> Result<Self, DocForgeError> {
        match s {
            "draft" => Ok(MatterStatus::Draft),
            "ready" => Ok(MatterStatus::Ready),
            "generating" => Ok(MatterStatus::Generating),
            "generated" => Ok(MatterStatus::Generated),
            _ => Err(DocForgeError::InvalidInput(format!(
                "Unknown matter status: '{s}'"
            ))),
        }
    }
}

/// One row in the `matters` table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Matter {
    pub id: String,
    pub name: String,
    pub bundle_id: String,
    pub bundle_version_id: String,
    pub org_id: Option<String>,
    pub status: MatterStatus,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub input_snapshot_json: Option<String>,
    pub input_snapshot_hash: Option<String>,
}

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

/// Creates a new Matter in `draft` status for the given bundle version.
pub fn create_matter(
    conn: &Connection,
    bundle_id: &str,
    bundle_version_id: &str,
    name: &str,
    org_id: Option<&str>,
    created_by: Option<&str>,
) -> Result<Matter, DocForgeError> {
    if name.trim().is_empty() {
        return Err(DocForgeError::InvalidInput(
            "matter name must not be empty".to_string(),
        ));
    }

    // Validate bundle_version_id exists.
    let bv_exists: i32 = conn
        .query_row(
            "SELECT COUNT(1) FROM bundle_versions WHERE id = ?1",
            [bundle_version_id],
            |r| r.get(0),
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Check bundle_version exists: {e}")))?;
    if bv_exists == 0 {
        return Err(DocForgeError::InvalidInput(format!(
            "bundle_version_id '{bundle_version_id}' does not exist"
        )));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let id = format!("mat_{}", Uuid::new_v4());

    conn.execute(
        "INSERT INTO matters (id, name, bundle_id, bundle_version_id, org_id, status, created_by, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'draft', ?6, ?7, ?8)",
        rusqlite::params![id, name, bundle_id, bundle_version_id, org_id, created_by, now, now],
    )
    .map_err(|e| DocForgeError::StorageIo(format!("Insert matter: {e}")))?;

    Ok(Matter {
        id,
        name: name.to_string(),
        bundle_id: bundle_id.to_string(),
        bundle_version_id: bundle_version_id.to_string(),
        org_id: org_id.map(String::from),
        status: MatterStatus::Draft,
        created_by: created_by.map(String::from),
        created_at: now.clone(),
        updated_at: now,
        input_snapshot_json: None,
        input_snapshot_hash: None,
    })
}

/// Fetches a Matter by id, or None if not found.
pub fn get_matter(conn: &Connection, matter_id: &str) -> Result<Option<Matter>, DocForgeError> {
    let mut stmt = conn
        .prepare("SELECT id, name, bundle_id, bundle_version_id, org_id, status, created_by, created_at, updated_at, input_snapshot_json, input_snapshot_hash FROM matters WHERE id = ?1")
        .map_err(|e| DocForgeError::StorageIo(format!("Prepare get_matter: {e}")))?;

    let rows = stmt
        .query_map([matter_id], row_to_matter)
        .map_err(|e| DocForgeError::StorageIo(format!("Query matter: {e}")))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| DocForgeError::StorageIo(format!("Map matter row: {e}")))?);
    }
    Ok(out.into_iter().next())
}

/// Lists all Matters for a given bundle version.
pub fn list_matters(
    conn: &Connection,
    bundle_version_id: &str,
) -> Result<Vec<Matter>, DocForgeError> {
    let mut stmt = conn
        .prepare("SELECT id, name, bundle_id, bundle_version_id, org_id, status, created_by, created_at, updated_at, input_snapshot_json, input_snapshot_hash FROM matters WHERE bundle_version_id = ?1 ORDER BY updated_at DESC")
        .map_err(|e| DocForgeError::StorageIo(format!("Prepare list_matters: {e}")))?;

    let rows = stmt
        .query_map([bundle_version_id], row_to_matter)
        .map_err(|e| DocForgeError::StorageIo(format!("Query matters: {e}")))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| DocForgeError::StorageIo(format!("Map matter row: {e}")))?);
    }
    Ok(out)
}

/// Updates the status of a Matter.
pub fn update_matter_status(
    conn: &Connection,
    matter_id: &str,
    status: MatterStatus,
) -> Result<Matter, DocForgeError> {
    let now = chrono::Utc::now().to_rfc3339();
    let affected = conn
        .execute(
            "UPDATE matters SET status = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![status.as_str(), now, matter_id],
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Update matter status: {e}")))?;

    if affected == 0 {
        return Err(DocForgeError::StorageMissing(format!(
            "Matter '{matter_id}' not found"
        )));
    }

    let mut matter = get_matter(conn, matter_id)?
        .ok_or_else(|| DocForgeError::StorageMissing(format!("Matter '{matter_id}' vanished")))?;
    matter.status = status;
    matter.updated_at = now;
    Ok(matter)
}

/// Deletes a Matter (CASCADE removes matter_values).
pub fn delete_matter(conn: &Connection, matter_id: &str) -> Result<(), DocForgeError> {
    let affected = conn
        .execute("DELETE FROM matters WHERE id = ?1", [matter_id])
        .map_err(|e| DocForgeError::StorageIo(format!("Delete matter: {e}")))?;

    if affected == 0 {
        return Err(DocForgeError::StorageMissing(format!(
            "Matter '{matter_id}' not found"
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn row_to_matter(row: &rusqlite::Row) -> Result<Matter, rusqlite::Error> {
    let status_str: String = row.get(5)?;
    Ok(Matter {
        id: row.get(0)?,
        name: row.get(1)?,
        bundle_id: row.get(2)?,
        bundle_version_id: row.get(3)?,
        org_id: row.get(4)?,
        status: MatterStatus::from_str(&status_str)
            .map_err(|_| rusqlite::Error::InvalidColumnType(
                5,
                "status".to_string(),
                rusqlite::types::Type::Text,
            ))?,
        created_by: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        input_snapshot_json: row.get(9)?,
        input_snapshot_hash: row.get(10)?,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bundle::manifest::create_bundle;
    use crate::schema::init_memory_db;

    fn setup_bundle() -> (Connection, String, String) {
        let conn = init_memory_db().expect("memory db");
        let bundle = create_bundle(&conn, "Matter Test Bundle", None, None).expect("create bundle");
        let bv_id = conn
            .query_row(
                "SELECT id FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC LIMIT 1",
                [&bundle.id],
                |r| r.get::<_, String>(0),
            )
            .expect("head version");
        (conn, bundle.id, bv_id)
    }

    #[test]
    fn test_create_matter() {
        let (conn, bundle_id, bv_id) = setup_bundle();
        let m = create_matter(&conn, &bundle_id, &bv_id, "Acme Corp", None, None).expect("create matter");
        assert_eq!(m.name, "Acme Corp");
        assert_eq!(m.status, MatterStatus::Draft);
        assert_eq!(m.bundle_id, bundle_id);
        assert_eq!(m.bundle_version_id, bv_id);
    }

    #[test]
    fn test_create_matter_rejects_empty_name() {
        let (conn, bundle_id, bv_id) = setup_bundle();
        let err = create_matter(&conn, &bundle_id, &bv_id, "", None, None).expect_err("rejected");
        assert!(matches!(err, DocForgeError::InvalidInput(_)));
    }

    #[test]
    fn test_get_matter_not_found() {
        let (conn, _, _) = setup_bundle();
        let result = get_matter(&conn, "mat_nonexistent").expect("query ok");
        assert!(result.is_none());
    }

    #[test]
    fn test_list_matters_for_version() {
        let (conn, bundle_id, bv_id) = setup_bundle();
        create_matter(&conn, &bundle_id, &bv_id, "M1", None, None).expect("create M1");
        create_matter(&conn, &bundle_id, &bv_id, "M2", None, None).expect("create M2");
        let list = list_matters(&conn, &bv_id).expect("list");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "M2"); // newest first (ORDER BY updated_at DESC)
    }

    #[test]
    fn test_update_matter_status() {
        let (conn, bundle_id, bv_id) = setup_bundle();
        let m = create_matter(&conn, &bundle_id, &bv_id, "M1", None, None).expect("create M1");
        let updated = update_matter_status(&conn, &m.id, MatterStatus::Ready).expect("update");
        assert_eq!(updated.status, MatterStatus::Ready);
    }

    #[test]
    fn test_delete_matter_cascades_values() {
        let (conn, bundle_id, bv_id) = setup_bundle();
        let m = create_matter(&conn, &bundle_id, &bv_id, "M1", None, None).expect("create M1");
        delete_matter(&conn, &m.id).expect("delete");
        let result = get_matter(&conn, &m.id).expect("query");
        assert!(result.is_none());
        // matter_values should also be gone (ON DELETE CASCADE).
        let vals = crate::core::matter::matter_values::list_matter_values(&conn, &m.id)
            .expect("values");
        assert!(vals.is_empty());
    }
}
