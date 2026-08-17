//! mapping.rs — Explicit deterministic mapping layer (TASK-108, REQ-028).
//!
//! Bridges a document's placeholder text to a canonical Bundle field. This is
//! the ONLY place where placeholder-to-field resolution happens — no scattered
//! string replacement exists anywhere in the generation path (REQ-028, AC-028).

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::error::DocForgeError;
use crate::core::field_mapping::schema::validate_value;
#[cfg(test)]
use crate::core::field_mapping::schema::FieldDef;
use crate::core::field_mapping::registry::list_fields;

/// One row in the `field_mappings` table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldMapping {
    pub id: String,
    pub bundle_version_id: String,
    pub document_id: String,
    pub placeholder: String,
    pub canonical_field_id: String,
}

/// Result of resolving a single placeholder occurrence against matter data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedValue {
    /// The placeholder text as it appeared in the document (e.g. `{{Company_Name}}`).
    pub placeholder: String,
    /// The canonical field this placeholder maps to (e.g. `company.name`).
    pub canonical_field_id: String,
    /// The resolved value from matter data, if present and valid.
    pub value: Option<Value>,
    /// A structured error when resolution or validation fails — never silent blank.
    pub error: Option<String>,
}

/// An unmapped placeholder detected in a document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnmappedPlaceholder {
    /// The bundle_document this placeholder was found in.
    pub document_id: String,
    /// The raw placeholder text (e.g. `{{director_name}}`).
    pub placeholder: String,
    /// A suggested canonical field, if one with a matching label exists.
    pub suggested_canonical_field_id: Option<String>,
}

/// Maps a document placeholder to a canonical field (REQ-028).
///
/// Enforces uniqueness per (bundle_version_id, document_id, placeholder) at the
/// DB level; surface constraint violations as `InvalidInput`.
pub fn set_mapping(
    conn: &Connection,
    bundle_version_id: &str,
    document_id: &str,
    placeholder: &str,
    canonical_field_id: &str,
) -> Result<FieldMapping, DocForgeError> {
    if placeholder.trim().is_empty() {
        return Err(DocForgeError::InvalidInput(
            "placeholder must not be empty".to_string(),
        ));
    }

    let fields = list_fields(conn, bundle_version_id)
        .map_err(|e| DocForgeError::StorageIo(format!("List fields: {e}")))?;
    if !fields.iter().any(|f| f.field_id == canonical_field_id) {
        return Err(DocForgeError::InvalidInput(format!(
            "canonical_field_id '{}' does not exist in bundle version '{}'",
            canonical_field_id, bundle_version_id
        )));
    }

    let id = format!("fmp_{}", uuid::Uuid::new_v4());
    conn.execute(
        "INSERT INTO field_mappings (id, bundle_version_id, document_id, placeholder, canonical_field_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, bundle_version_id, document_id, placeholder, canonical_field_id],
    )
    .map_err(|e| map_rusqlite_error(e, "Insert field mapping"))?;

    Ok(FieldMapping {
        id,
        bundle_version_id: bundle_version_id.to_string(),
        document_id: document_id.to_string(),
        placeholder: placeholder.to_string(),
        canonical_field_id: canonical_field_id.to_string(),
    })
}

/// Lists all mappings for a bundle version, optionally filtered by document.
pub fn list_mappings(
    conn: &Connection,
    bundle_version_id: &str,
    document_id: Option<&str>,
) -> Result<Vec<FieldMapping>, DocForgeError> {
    let (sql, params) = if document_id.is_some() {
        (
            "SELECT id, bundle_version_id, document_id, placeholder, canonical_field_id
             FROM field_mappings
             WHERE bundle_version_id = ?1 AND document_id = ?2
             ORDER BY document_id, placeholder",
            rusqlite::params![bundle_version_id, document_id.unwrap()],
        )
    } else {
        (
            "SELECT id, bundle_version_id, document_id, placeholder, canonical_field_id
             FROM field_mappings
             WHERE bundle_version_id = ?1
             ORDER BY document_id, placeholder",
            rusqlite::params![bundle_version_id],
        )
    };

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| DocForgeError::StorageIo(format!("Prepare list mappings: {e}")))?;

    let mapped_rows = stmt
        .query_map(params, row_to_mapping)
        .map_err(|e| DocForgeError::StorageIo(format!("Query mappings: {e}")))?;

    let mut out = Vec::new();
    for row in mapped_rows {
        out.push(row.map_err(|e| DocForgeError::StorageIo(format!("Map mapping row: {e}")))?);
    }
    Ok(out)
}

/// Resolves a placeholder occurrence against matter data using the explicit
/// mapping table. Returns a structured `ResolvedValue` — **never silent blank**.
///
/// Resolution order:
/// 1. Look up the mapping for (document_id, placeholder) within the bundle_version.
/// 2. Read the canonical field definition to get the expected type.
/// 3. Extract the value from `matter_data` by `canonical_field_id`.
/// 4. Validate the extracted value against the field type (via `validate_value`).
pub fn resolve_value(
    conn: &Connection,
    bundle_version_id: &str,
    document_id: &str,
    placeholder: &str,
    matter_data: &Value,
) -> Result<ResolvedValue, DocForgeError> {
    let mappings = list_mappings(conn, bundle_version_id, Some(document_id))?;
    let mapping = mappings
        .iter()
        .find(|m| m.placeholder == placeholder)
        .ok_or_else(|| DocForgeError::InvalidInput(format!(
            "No mapping found for placeholder '{}' in document '{}'",
            placeholder, document_id
        )))?;

    let fields = list_fields(conn, bundle_version_id)
        .map_err(|e| DocForgeError::StorageIo(format!("List fields: {e}")))?;
    let field = fields
        .iter()
        .find(|f| f.field_id == mapping.canonical_field_id)
        .ok_or_else(|| DocForgeError::InvalidInput(format!(
            "Mapped canonical field '{}' not found in bundle version '{}'",
            mapping.canonical_field_id, bundle_version_id
        )))?;

    let raw_value = matter_data
        .get(&field.field_id)
        .cloned()
        .unwrap_or(Value::Null);

    let validation_err = validate_value(field.field_type, &raw_value, field.required);
    if let Err(e) = &validation_err {
        return Ok(ResolvedValue {
            placeholder: placeholder.to_string(),
            canonical_field_id: field.field_id.clone(),
            value: Some(raw_value),
            error: Some(e.to_string()),
        });
    }

    Ok(ResolvedValue {
        placeholder: placeholder.to_string(),
        canonical_field_id: field.field_id.clone(),
        value: Some(raw_value),
        error: None,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn row_to_mapping(row: &rusqlite::Row) -> Result<FieldMapping, rusqlite::Error> {
    Ok(FieldMapping {
        id: row.get(0)?,
        bundle_version_id: row.get(1)?,
        document_id: row.get(2)?,
        placeholder: row.get(3)?,
        canonical_field_id: row.get(4)?,
    })
}

fn map_rusqlite_error(e: rusqlite::Error, context: &str) -> DocForgeError {
    let message = e.to_string();
    if message.contains("UNIQUE constraint failed: field_mappings") {
        DocForgeError::InvalidInput(format!(
            "{context}: mapping already exists for this placeholder/document/version"
        ))
    } else if message.contains("published bundle_versions is immutable") {
        DocForgeError::PublishedBundleImmutable(context.to_string())
    } else {
        DocForgeError::StorageIo(format!("{context}: {message}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bundle::manifest::create_bundle;
    use crate::core::field_mapping::registry::create_field;
    use crate::schema::init_memory_db;

    fn setup_bundle_with_fields() -> (Connection, String, String, FieldDef, Vec<String>) {
        let conn = init_memory_db().expect("memory db");
        let record = create_bundle(&conn, "Mapping Set", None, None).expect("create bundle");
        let bv_id = conn
            .query_row(
                "SELECT id FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC LIMIT 1",
                [&record.id],
                |r| r.get::<_, String>(0),
            )
            .expect("head version");

        let field = create_field(
            &conn,
            &bv_id,
            &FieldDef {
                id: String::new(),
                field_id: "company.name".to_string(),
                label: "Company Name".to_string(),
                description: None,
                field_type: crate::core::field_mapping::schema::FieldType::Text,
                required: true,
                default: None,
                validation: None,
                group_id: None,
                options: Vec::new(),
                format: None,
                position: 0,
            },
        )
        .expect("create field");

        let doc_ids = vec!["doc-a".to_string(), "doc-b".to_string()];
        for (i, doc_id) in doc_ids.iter().enumerate() {
            let tpl_id = format!("tpl_{i}");
            conn.execute(
                "INSERT OR IGNORE INTO templates (id, name, storage_path) VALUES (?1, ?2, ?3)",
                rusqlite::params![tpl_id, format!("Template {i}"), format!("/tmp/{tpl_id}.docx")],
            )
            .expect("insert template");
            conn.execute(
                "INSERT INTO bundle_documents (id, bundle_version_id, template_id, position, include_default)
                 VALUES (?1, ?2, ?3, ?4, 1)",
                rusqlite::params![doc_id, bv_id, tpl_id, i as i32],
            )
            .expect("insert bundle_document");
        }

        (conn, record.id, bv_id, field, doc_ids)
    }

    #[test]
    fn test_four_placeholders_one_field() {
        let (conn, _bundle_id, bv_id, field, _doc_ids) = setup_bundle_with_fields();

        // Simulate two documents sharing one field via different placeholders.
        for (doc_id, placeholder) in [
            ("doc-a", "{{company_name}}"),
            ("doc-a", "{{name_of_company}}"),
            ("doc-b", "{{Company_Name}}"),
            ("doc-b", "{{company}}"),
        ] {
            set_mapping(&conn, &bv_id, doc_id, placeholder, &field.field_id)
                .expect("set mapping");
        }

        let matter_data = serde_json::json!({"company.name": "ABC Pvt Ltd"});

        for (doc_id, placeholder) in [
            ("doc-a", "{{company_name}}"),
            ("doc-a", "{{name_of_company}}"),
            ("doc-b", "{{Company_Name}}"),
            ("doc-b", "{{company}}"),
        ] {
            let resolved = resolve_value(&conn, &bv_id, doc_id, placeholder, &matter_data)
                .expect("resolve");
            assert_eq!(resolved.error, None);
            assert_eq!(
                resolved.value.unwrap(),
                serde_json::json!("ABC Pvt Ltd"),
                "placeholder '{}' in '{}'",
                placeholder,
                doc_id
            );
        }
    }

    #[test]
    fn test_set_mapping_rejects_duplicate() {
        let (conn, _bundle_id, bv_id, field, _doc_ids) = setup_bundle_with_fields();
        set_mapping(&conn, &bv_id, "doc-a", "{{x}}", &field.field_id).expect("first");
        let err = set_mapping(&conn, &bv_id, "doc-a", "{{x}}", &field.field_id)
            .expect_err("duplicate rejected");
        assert!(
            matches!(err, DocForgeError::InvalidInput(_)),
            "expected InvalidInput, got {err:?}"
        );
    }

    #[test]
    fn test_set_mapping_rejects_unknown_field() {
        let (conn, _bundle_id, bv_id, _field, _doc_ids) = setup_bundle_with_fields();
        let err = set_mapping(&conn, &bv_id, "doc-a", "{{x}}", "does.not.exist")
            .expect_err("unknown field rejected");
        assert!(
            matches!(err, DocForgeError::InvalidInput(_)),
            "expected InvalidInput, got {err:?}"
        );
    }

    #[test]
    fn test_resolve_value_type_mismatch() {
        let (conn, _bundle_id, bv_id, field, _doc_ids) = setup_bundle_with_fields();
        set_mapping(&conn, &bv_id, "doc-a", "{{shares}}", &field.field_id).expect("map");

        let matter_data = serde_json::json!({"company.name": 42});
        let resolved = resolve_value(&conn, &bv_id, "doc-a", "{{shares}}", &matter_data)
            .expect("resolve");
        assert!(resolved.error.is_some(), "type mismatch must produce error, not silent blank");
        assert!(resolved.value.is_some(), "original value retained for diagnostics");
    }

    #[test]
    fn test_list_mappings_filters_by_document() {
        let (conn, _bundle_id, bv_id, field, _doc_ids) = setup_bundle_with_fields();
        set_mapping(&conn, &bv_id, "doc-a", "{{a}}", &field.field_id).expect("a");
        set_mapping(&conn, &bv_id, "doc-b", "{{b}}", &field.field_id).expect("b");

        let all = list_mappings(&conn, &bv_id, None).expect("all");
        assert_eq!(all.len(), 2);

        let a_only = list_mappings(&conn, &bv_id, Some("doc-a")).expect("a only");
        assert_eq!(a_only.len(), 1);
        assert_eq!(a_only[0].document_id, "doc-a");
    }
}
