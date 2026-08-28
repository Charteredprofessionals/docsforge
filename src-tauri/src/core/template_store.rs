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
use crate::infra::crypto::{decrypt_at_rest, encrypt_at_rest};

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

    // Encrypt at rest (DPAPI on Windows) and write atomically (temp + rename).
    let protected = encrypt_at_rest(docx_bytes)
        .map_err(|e| DocForgeError::StorageIo(format!("Encrypt template: {e}")))?;
    atomic_write(&file_path, &protected)?;

    let storage_path = file_path.to_string_lossy().to_string();
    let status_str = TemplateStatus::Draft.to_string();

    // Wrap DB inserts in a transaction so the filesystem file and DB records are consistent.
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| DocForgeError::StorageIo(format!("Begin save_template transaction: {e}")))?;

    tx.execute(
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
    tx.execute(
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

    tx.commit()
        .map_err(|e| DocForgeError::StorageIo(format!("Commit save_template transaction: {e}")))?;

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

    let raw = fs::read(&path)
        .map_err(|e| DocForgeError::StorageIo(format!("Read template file: {e}")))?;
    let bytes = decrypt_at_rest(&raw)
        .map_err(|e| DocForgeError::StorageIo(format!("Decrypt template: {e}")))?;

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
        match r {
            Ok(record) => result.push(record),
            Err(e) => {
                eprintln!("[template_store] Warning: skipped malformed template row: {e}");
            }
        }
    }
    Ok(result)
}

/// Deletes a template from disk and SQLite.
pub fn delete_template(conn: &Connection, template_id: &str) -> Result<(), DocForgeError> {
    let _record = load_template_meta(conn, template_id)?;

    let base_dir = get_templates_dir().join(template_id);
    if base_dir.exists() {
        fs::remove_dir_all(&base_dir).map_err(|e| {
            DocForgeError::StorageIo(format!("Failed to remove template directory '{}': {e}", base_dir.display()))
        })?;
    }

    let affected = conn.execute("DELETE FROM templates WHERE id = ?1", params![template_id])
        .map_err(|e| DocForgeError::StorageIo(format!("DB delete: {e}")))?;

    if affected == 0 {
        return Err(DocForgeError::StorageMissing(format!(
            "Template '{template_id}' not found in database"
        )));
    }

    Ok(())
}

/// Updates an existing template's metadata and/or DOCX content.
///
/// Only non-empty fields are updated; `None` means "keep existing".
pub fn update_template(
    conn: &Connection,
    template_id: &str,
    name: Option<&str>,
    category: Option<&str>,
    description: Option<&str>,
    fields: Option<&[TemplateFieldSpec]>,
    docx_bytes: Option<&[u8]>,
) -> Result<TemplateRecord, DocForgeError> {
    let mut record = load_template_meta(conn, template_id)?;

    // Update metadata if provided.
    if let Some(n) = name {
        if !n.trim().is_empty() {
            record.name = n.to_string();
        }
    }
    if let Some(c) = category {
        if !c.trim().is_empty() {
            record.category = c.to_string();
        }
    }
    if let Some(d) = description {
        record.description = d.to_string();
    }

    // Update fields if provided.
    if let Some(new_fields) = fields {
        record.fields = new_fields.to_vec();
    }

    // Update DOCX content if provided.
    if let Some(bytes) = docx_bytes {
        let new_sha256 = compute_sha256(bytes);
        let new_version = record.current_version + 1;

        // Create new version directory.
        let base_dir = get_templates_dir();
        let new_version_dir = base_dir.join(template_id).join(format!("v{new_version}"));
        fs::create_dir_all(&new_version_dir).map_err(|e| {
            DocForgeError::StorageIo(format!("Failed to create version directory: {e}"))
        })?;

        let new_file_path = new_version_dir.join("template.docx");
        let protected = encrypt_at_rest(bytes)
            .map_err(|e| DocForgeError::StorageIo(format!("Encrypt template: {e}")))?;
        atomic_write(&new_file_path, &protected)?;

        let new_storage_path = new_file_path.to_string_lossy().to_string();
        let fields_json = serde_json::to_string(&record.fields)
            .map_err(|e| DocForgeError::Internal(format!("Serialize fields: {e}")))?;

        // Update template row to point to new version.
        conn.execute(
            "UPDATE templates SET
                name = ?1, category = ?2, description = ?3, current_version = ?4,
                storage_path = ?5, fields_json = ?6, content_sha256 = ?7, updated_at = datetime('now')
             WHERE id = ?8",
            params![
                &record.name,
                &record.category,
                &record.description,
                new_version,
                new_storage_path,
                fields_json,
                new_sha256,
                template_id,
            ],
        )
        .map_err(|e| DocForgeError::StorageIo(format!("DB update templates: {e}")))?;

        // Insert new version row.
        let version_id = format!("ver_{}", Uuid::new_v4());
        conn.execute(
            "INSERT INTO template_versions (
                id, template_id, version, status, storage_path, fields_json, content_sha256, note, created_by
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                version_id,
                template_id,
                new_version,
                TemplateStatus::Draft.to_string(),
                new_storage_path,
                fields_json,
                new_sha256,
                "Updated via edit",
                record.created_by,
            ],
        )
        .map_err(|e| DocForgeError::StorageIo(format!("DB insert template_versions: {e}")))?;

        record.current_version = new_version;
        record.storage_path = new_storage_path;
        record.content_sha256 = new_sha256;
        record.updated_at = chrono::Utc::now().to_rfc3339();
    } else {
        // Only metadata changed; update the row directly.
        let fields_json = serde_json::to_string(&record.fields)
            .map_err(|e| DocForgeError::Internal(format!("Serialize fields: {e}")))?;
        conn.execute(
            "UPDATE templates SET
                name = ?1, category = ?2, description = ?3, fields_json = ?4, updated_at = datetime('now')
             WHERE id = ?5",
            params![
                &record.name,
                &record.category,
                &record.description,
                fields_json,
                template_id,
            ],
        )
        .map_err(|e| DocForgeError::StorageIo(format!("DB update templates: {e}")))?;
        record.updated_at = chrono::Utc::now().to_rfc3339();
    }

    Ok(record)
}

/// Writes `data` to `path` atomically: a temp file is written then renamed into place,
/// so a crash mid-write cannot leave a half-written template file behind.
pub(crate) fn atomic_write(path: &Path, data: &[u8]) -> Result<(), DocForgeError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = dir.join(format!(".docforge_tmp_{}", Uuid::new_v4()));
    fs::write(&tmp, data).map_err(|e| DocForgeError::StorageIo(format!("Write temp file: {e}")))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        DocForgeError::StorageIo(format!("Rename temp file: {e}"))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::init_memory_db;

    #[test]
    fn test_save_load_encrypted_template_roundtrip() {
        let conn = init_memory_db().expect("init memory db");
        let fake_docx = b"PK\x03\x04_mock_docx_binary_content_123456789";
        let fields = vec![TemplateFieldSpec {
            id: "f1".to_string(),
            label: "Test Field".to_string(),
            original_text: "Sample".to_string(),
            tag_name: "test_field".to_string(),
        }];

        let record = save_template(
            &conn,
            "Test Template",
            "general",
            "A test description",
            &fields,
            fake_docx,
            None,
            None,
        )
        .expect("save_template should succeed");

        assert_eq!(record.name, "Test Template");
        assert_eq!(record.fields.len(), 1);

        let (loaded_record, loaded_bytes) = load_template_file(&conn, &record.id)
            .expect("load_template_file should succeed");

        assert_eq!(loaded_record.id, record.id);
        assert_eq!(&loaded_bytes[..], &fake_docx[..]);

        let list = list_templates(&conn, None).expect("list_templates should succeed");
        assert_eq!(list.len(), 1);

        delete_template(&conn, &record.id).expect("delete_template should succeed");
        let list_after = list_templates(&conn, None).expect("list_templates after delete");
        assert_eq!(list_after.len(), 0);
    }
}
