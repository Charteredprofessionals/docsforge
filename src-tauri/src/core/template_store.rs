//! template_store.rs — Filesystem-backed template repository with SQLite index.
//!
//! Stores actual DOCX files under app-data directory `templates/<id>/v<version>/template.docx`
//! and records metadata, fields_json, and content SHA-256 in SQLite (Data Model v2). Zero BLOBs in DB.

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::core::docx_engine::TemplateFieldSpec;
use crate::core::error::DocForgeError;
use crate::core::template::{TemplateRecord, TemplateStatus};

/// Returns the base directory path for template storage under app data.
pub fn get_templates_dir() -> PathBuf {
    let data_dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    let app_dir = data_dir.join("docforge").join("templates");
    fs::create_dir_all(&app_dir).expect("Failed to create templates storage directory");
    app_dir
}

/// Calculates hex-encoded SHA-256 digest of a byte slice.
pub fn compute_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Validates that a file path is safely contained within the designated base directory.
pub fn check_path_containment(base: &Path, target: &Path) -> Result<(), DocForgeError> {
    let canonical_base = base
        .canonicalize()
        .unwrap_or_else(|_| base.to_path_buf());
    
    // For target that may not exist yet, check its parent
    let target_parent = target.parent().unwrap_or(target);
    let canonical_target = target_parent
        .canonicalize()
        .map_err(|e| DocForgeError::StorageIo(format!("Path resolution failed: {e}")))?;

    if !canonical_target.starts_with(&canonical_base) {
        return Err(DocForgeError::Forbidden(format!(
            "Path traversal blocked: target '{}' outside base '{}'",
            target.display(),
            base.display()
        )));
    }
    Ok(())
}

/// Saves a template to filesystem storage and creates SQLite index records in Data Model v2.
pub fn save_template(
    conn: &Connection,
    name: &str,
    category: &str,
    description: &str,
    fields: &[TemplateFieldSpec],
    docx_bytes: &[u8],
    org_id: Option<&str>,
    created_by: Option<&str>,
) -> Result<TemplateRecord, DocForgeError> {
    let template_id = format!("tpl_{}", Uuid::new_v4());
    let version = 1;

    let base_dir = get_templates_dir();
    let version_dir = base_dir.join(&template_id).join(format!("v{version}"));
    fs::create_dir_all(&version_dir).map_err(|e| {
        DocForgeError::StorageIo(format!("Failed to create template directory: {e}"))
    })?;

    let file_path = version_dir.join("template.docx");

    let sha256_hash = compute_sha256(docx_bytes);
    let fields_json = serde_json::to_string(fields)
        .map_err(|e| DocForgeError::Internal(format!("Serialize fields: {e}")))?;

    fs::write(&file_path, docx_bytes)
        .map_err(|e| DocForgeError::StorageIo(format!("Failed to write DOCX file: {e}")))?;

    let storage_path = file_path.to_string_lossy().to_string();

    let status_str = TemplateStatus::Draft.to_string();

    conn.execute(
        "INSERT INTO templates (
            id, org_id, name, category, description, current_version, status,
            storage_path, fields_json, content_sha256, created_by
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            template_id,
            org_id,
            name,
            category,
            description,
            version,
            status_str,
            storage_path,
            fields_json,
            sha256_hash,
            created_by,
        ],
    )
    .map_err(|e| DocForgeError::StorageIo(format!("DB insert templates: {e}")))?;

    let version_id = format!("ver_{}", Uuid::new_v4());
    conn.execute(
        "INSERT INTO template_versions (
            id, template_id, version, status, storage_path, fields_json, content_sha256, note, created_by
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            version_id,
            template_id,
            version,
            status_str,
            storage_path,
            fields_json,
            sha256_hash,
            "Initial version",
            created_by,
        ],
    )
    .map_err(|e| DocForgeError::StorageIo(format!("DB insert template_versions: {e}")))?;

    load_template_meta(conn, &template_id)
}

/// Loads metadata for a template by ID.
pub fn load_template_meta(
    conn: &Connection,
    template_id: &str,
) -> Result<TemplateRecord, DocForgeError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, org_id, name, category, description, current_version, status,
                    storage_path, fields_json, content_sha256, created_by, created_at, updated_at
             FROM templates WHERE id = ?1",
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Prepare query: {e}")))?;

    let record = stmt
        .query_row(params![template_id], |row| {
            let fields_json: String = row.get(8)?;
            let fields: Vec<TemplateFieldSpec> =
                serde_json::from_str(&fields_json).unwrap_or_default();
            let status_str: String = row.get(6)?;

            Ok(TemplateRecord {
                id: row.get(0)?,
                org_id: row.get(1)?,
                name: row.get(2)?,
                category: row.get(3)?,
                description: row.get(4)?,
                current_version: row.get(5)?,
                status: TemplateStatus::from_str(&status_str).unwrap_or(TemplateStatus::Draft),
                storage_path: row.get(7)?,
                fields,
                content_sha256: row.get(9)?,
                created_by: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })
        .map_err(|e| DocForgeError::StorageMissing(format!("Template '{template_id}' not found: {e}")))?;

    Ok(record)
}

/// Loads template metadata and DOCX bytes from disk, verifying SHA-256 integrity.
pub fn load_template_file(
    conn: &Connection,
    template_id: &str,
) -> Result<(TemplateRecord, Vec<u8>), DocForgeError> {
    let record = load_template_meta(conn, template_id)?;

    let path = PathBuf::from(&record.storage_path);
    if !path.exists() {
        return Err(DocForgeError::StorageMissing(format!(
            "DOCX file missing from storage path: {}",
            record.storage_path
        )));
    }

    let bytes = fs::read(&path)
        .map_err(|e| DocForgeError::StorageIo(format!("Read template file: {e}")))?;

    let actual_sha256 = compute_sha256(&bytes);
    if actual_sha256 != record.content_sha256 {
        return Err(DocForgeError::StorageIo(format!(
            "SHA-256 integrity mismatch for template '{}': expected {}, got {}",
            template_id, record.content_sha256, actual_sha256
        )));
    }

    Ok((record, bytes))
}

/// Lists all templates.
pub fn list_templates(
    conn: &Connection,
    org_id: Option<&str>,
) -> Result<Vec<TemplateRecord>, DocForgeError> {
    let mut sql = "SELECT id, org_id, name, category, description, current_version, status,
                          storage_path, fields_json, content_sha256, created_by, created_at, updated_at
                   FROM templates".to_string();

    if org_id.is_some() {
        sql.push_str(" WHERE org_id = ?1");
    }
    sql.push_str(" ORDER BY created_at DESC");

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| DocForgeError::StorageIo(format!("Prepare list query: {e}")))?;

    let map_row = |row: &rusqlite::Row| {
        let fields_json: String = row.get(8)?;
        let fields: Vec<TemplateFieldSpec> =
            serde_json::from_str(&fields_json).unwrap_or_default();
        let status_str: String = row.get(6)?;

        Ok(TemplateRecord {
            id: row.get(0)?,
            org_id: row.get(1)?,
            name: row.get(2)?,
            category: row.get(3)?,
            description: row.get(4)?,
            current_version: row.get(5)?,
            status: TemplateStatus::from_str(&status_str).unwrap_or(TemplateStatus::Draft),
            storage_path: row.get(7)?,
            fields,
            content_sha256: row.get(9)?,
            created_by: row.get(10)?,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
        })
    };

    let rows = if let Some(oid) = org_id {
        stmt.query_map(params![oid], map_row)
    } else {
        stmt.query_map([], map_row)
    }
    .map_err(|e| DocForgeError::StorageIo(format!("Query list: {e}")))?;

    let mut result = Vec::new();
    for r in rows {
        if let Ok(record) = r {
            result.push(record);
        }
    }
    Ok(result)
}

/// Deletes a template from disk and SQLite.
pub fn delete_template(conn: &Connection, template_id: &str) -> Result<(), DocForgeError> {
    let record = load_template_meta(conn, template_id)?;

    let base_dir = get_templates_dir().join(template_id);
    if base_dir.exists() {
        let _ = fs::remove_dir_all(&base_dir);
    }

    conn.execute("DELETE FROM templates WHERE id = ?1", params![template_id])
        .map_err(|e| DocForgeError::StorageIo(format!("DB delete: {e}")))?;

    Ok(())
}
