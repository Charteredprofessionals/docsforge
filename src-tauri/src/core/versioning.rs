//! versioning.rs — Template version creation and rollback management.
//!
//! Provides transactional snapshot versioning and non-destructive rollbacks for templates.

use std::fs;
use std::path::PathBuf;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::core::docx_engine::TemplateFieldSpec;
use crate::core::error::DocForgeError;
use crate::core::template::{TemplateRecord, TemplateStatus};
use crate::core::template_store::{compute_sha256, get_templates_dir, load_template_meta};

/// Represents a version snapshot of a template.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TemplateVersionRecord {
    pub id: String,
    pub template_id: String,
    pub version: i32,
    pub status: TemplateStatus,
    pub storage_path: String,
    pub fields: Vec<TemplateFieldSpec>,
    pub content_sha256: String,
    pub note: String,
    pub created_by: Option<String>,
    pub created_at: String,
}

/// Creates a new version snapshot for an existing template.
pub fn create_template_version(
    conn: &Connection,
    template_id: &str,
    note: &str,
    docx_bytes: &[u8],
    fields: &[TemplateFieldSpec],
    user_id: Option<&str>,
) -> Result<TemplateVersionRecord, DocForgeError> {
    let current_meta = load_template_meta(conn, template_id)?;
    let new_version = current_meta.current_version + 1;

    let base_dir = get_templates_dir();
    let version_dir = base_dir.join(template_id).join(format!("v{new_version}"));
    fs::create_dir_all(&version_dir).map_err(|e| {
        DocForgeError::StorageIo(format!("Create version directory: {e}"))
    })?;

    let file_path = version_dir.join("template.docx");
    fs::write(&file_path, docx_bytes).map_err(|e| {
        DocForgeError::StorageIo(format!("Write version DOCX file: {e}"))
    })?;

    let storage_path = file_path.to_string_lossy().to_string();
    let sha256_hash = compute_sha256(docx_bytes);
    let fields_json = serde_json::to_string(fields)
        .map_err(|e| DocForgeError::Internal(format!("Serialize fields: {e}")))?;

    let version_id = format!("ver_{}", Uuid::new_v4());
    let status_str = current_meta.status.to_string();

    conn.execute(
        "INSERT INTO template_versions (
            id, template_id, version, status, storage_path, fields_json, content_sha256, note, created_by
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            version_id,
            template_id,
            new_version,
            status_str,
            storage_path,
            fields_json,
            sha256_hash,
            note,
            user_id,
        ],
    )
    .map_err(|e| DocForgeError::StorageIo(format!("Insert template_version: {e}")))?;

    conn.execute(
        "UPDATE templates SET current_version = ?1, storage_path = ?2, fields_json = ?3,
                              content_sha256 = ?4, updated_at = datetime('now')
         WHERE id = ?5",
        params![new_version, storage_path, fields_json, sha256_hash, template_id],
    )
    .map_err(|e| DocForgeError::StorageIo(format!("Update template head version: {e}")))?;

    Ok(TemplateVersionRecord {
        id: version_id,
        template_id: template_id.to_string(),
        version: new_version,
        status: current_meta.status,
        storage_path,
        fields: fields.to_vec(),
        content_sha256: sha256_hash,
        note: note.to_string(),
        created_by: user_id.map(|s| s.to_string()),
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// Rolls back a template to a prior version by creating a new head version copy of the target snapshot.
pub fn rollback_template_version(
    conn: &Connection,
    template_id: &str,
    target_version: i32,
    user_id: Option<&str>,
) -> Result<TemplateRecord, DocForgeError> {
    let (target_storage_path, fields_json, sha256_hash): (String, String, String) = conn
        .query_row(
            "SELECT storage_path, fields_json, content_sha256 FROM template_versions
             WHERE template_id = ?1 AND version = ?2",
            params![template_id, target_version],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| {
            DocForgeError::StorageMissing(format!(
                "Version {target_version} for template '{template_id}' not found: {e}"
            ))
        })?;

    let bytes = fs::read(&target_storage_path).map_err(|e| {
        DocForgeError::StorageIo(format!("Read target version DOCX file: {e}"))
    })?;

    let fields: Vec<TemplateFieldSpec> =
        serde_json::from_str(&fields_json).unwrap_or_default();

    let rollback_note = format!("Rollback to version {target_version}");
    create_template_version(conn, template_id, &rollback_note, &bytes, &fields, user_id)?;

    load_template_meta(conn, template_id)
}
