//! matter/validation.rs — Three-level Matter validation (TASK-113, REQ-032).
//!
//! `validate_matter` runs three validation levels and returns a structured
//! report pinpointing the exact document + field for every diagnostic:
//!
//! 1. **Field-level** — each present value matches its declared type/format
//!    (`field_mapping::validate_value`).
//! 2. **Matter-level** — all required fields of the bundle version are present.
//! 3. **Bundle-level** — no unresolved placeholders, no missing templates, no
//!    mappings/rules pointing at unknown fields.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::core::error::DocForgeError;
use crate::core::field_mapping::extraction::find_unmapped_placeholders;
use crate::core::field_mapping::registry::list_fields;
use crate::core::field_mapping::schema::validate_value;
use crate::core::matter::matter::get_matter;
#[cfg(test)]
use crate::core::matter::matter::Matter;
use crate::core::matter::matter_values::list_matter_values;

/// Which validation level produced an issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationLevel {
    Field,
    Matter,
    Bundle,
}

/// A single validation diagnostic, always identifying exact document + field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub level: ValidationLevel,
    pub code: String,
    pub message: String,
    pub document_id: Option<String>,
    pub field_id: Option<String>,
}

/// Full validation report for a Matter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub matter_id: String,
    pub issues: Vec<ValidationIssue>,
    pub is_valid: bool,
}

fn issue(
    level: ValidationLevel,
    code: &str,
    message: impl Into<String>,
    document_id: Option<&str>,
    field_id: Option<&str>,
) -> ValidationIssue {
    ValidationIssue {
        level,
        code: code.to_string(),
        message: message.into(),
        document_id: document_id.map(str::to_string),
        field_id: field_id.map(str::to_string),
    }
}

/// Validates a Matter across the three levels (REQ-032).
pub fn validate_matter(conn: &Connection, matter_id: &str) -> Result<ValidationReport, DocForgeError> {
    let matter = get_matter(conn, matter_id)?
        .ok_or_else(|| DocForgeError::StorageMissing(format!("Matter '{matter_id}' not found")))?;

    let fields = list_fields(conn, &matter.bundle_version_id)?;
    let values = list_matter_values(conn, matter_id)?;
    let value_map: std::collections::HashMap<String, serde_json::Value> = values
        .into_iter()
        .filter_map(|v| {
            serde_json::from_str::<serde_json::Value>(&v.value_json)
                .ok()
                .map(|parsed| (v.canonical_field_id, parsed))
        })
        .collect();

    let mut issues: Vec<ValidationIssue> = Vec::new();

    // Level 1 — Field-level type/format validation.
    for field in &fields {
        match value_map.get(&field.field_id) {
            Some(value) => {
                if let Err(e) = validate_value(field.field_type, value, field.required) {
                    issues.push(issue(
                        ValidationLevel::Field,
                        "field_type_mismatch",
                        format!("Field '{}': {}", field.field_id, e),
                        None,
                        Some(&field.field_id),
                    ));
                }
            }
            None => {
                if field.required {
                    issues.push(issue(
                        ValidationLevel::Field,
                        "required_field_missing",
                        format!("Required field '{}' has no value", field.field_id),
                        None,
                        Some(&field.field_id),
                    ));
                }
            }
        }
    }

    // Level 2 — Matter-level: required fields present (rollup across levels).
    let missing_required = fields
        .iter()
        .filter(|f| f.required && !value_map.contains_key(&f.field_id))
        .count();
    if missing_required > 0 {
        issues.push(issue(
            ValidationLevel::Matter,
            "matter_incomplete",
            format!("{missing_required} required field(s) are missing values"),
            None,
            None,
        ));
    }

    // Level 3 — Bundle-level: unresolved placeholders, missing templates,
    // invalid mappings/rules.
    let unmapped = find_unmapped_placeholders(conn, &matter.bundle_version_id)?;
    for u in &unmapped {
        issues.push(issue(
            ValidationLevel::Bundle,
            "unmapped_placeholder",
            format!(
                "Placeholder '{}' in document '{}' has no mapping{}",
                u.placeholder,
                u.document_id,
                u.suggested_canonical_field_id
                    .as_ref()
                    .map(|s| format!(" (suggested field: {s})"))
                    .unwrap_or_default()
            ),
            Some(&u.document_id),
            u.suggested_canonical_field_id.as_deref(),
        ));
    }

    let missing_templates = count_missing_templates(conn, &matter.bundle_version_id)?;
    if missing_templates > 0 {
        issues.push(issue(
            ValidationLevel::Bundle,
            "missing_template",
            format!("{missing_templates} document(s) reference a missing template"),
            None,
            None,
        ));
    }

    let invalid_mappings = count_invalid_mappings(conn, &matter.bundle_version_id)?;
    if invalid_mappings > 0 {
        issues.push(issue(
            ValidationLevel::Bundle,
            "invalid_mapping",
            format!("{invalid_mappings} mapping(s) reference an unknown field"),
            None,
            None,
        ));
    }

    let is_valid = issues.is_empty();
    Ok(ValidationReport {
        matter_id: matter_id.to_string(),
        issues,
        is_valid,
    })
}

/// Counts `bundle_documents` whose `template_id` is absent from `templates`.
fn count_missing_templates(conn: &Connection, bundle_version_id: &str) -> Result<usize, DocForgeError> {
    let count: i32 = conn
        .query_row(
            "SELECT COUNT(1)
             FROM bundle_documents bd
             LEFT JOIN templates t ON t.id = bd.template_id
             WHERE bd.bundle_version_id = ?1 AND (bd.template_id IS NULL OR t.id IS NULL)",
            [bundle_version_id],
            |r| r.get(0),
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Count missing templates: {e}")))?;
    Ok(count as usize)
}

/// Counts `field_mappings` whose `canonical_field_id` is absent from `fields`
/// for the same bundle version.
fn count_invalid_mappings(conn: &Connection, bundle_version_id: &str) -> Result<usize, DocForgeError> {
    let count: i32 = conn
        .query_row(
            "SELECT COUNT(1)
             FROM field_mappings fm
             LEFT JOIN fields f ON f.field_id = fm.canonical_field_id AND f.bundle_version_id = fm.bundle_version_id
             WHERE fm.bundle_version_id = ?1 AND f.id IS NULL",
            [bundle_version_id],
            |r| r.get(0),
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Count invalid mappings: {e}")))?;
    Ok(count as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::field_mapping::registry::create_field;
    use crate::core::field_mapping::schema::FieldType;
    use crate::core::matter::matter::create_matter;
    use crate::core::matter::matter_values::set_matter_value;
    use crate::schema::init_memory_db;

    fn setup() -> (Connection, String, String) {
        let conn = init_memory_db().expect("memory db");
        let bundle = crate::core::bundle::manifest::create_bundle(&conn, "Validation Test", None, None).expect("bundle");
        let bv = conn
            .query_row(
                "SELECT id FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC LIMIT 1",
                [&bundle.id],
                |r| r.get::<_, String>(0),
            )
            .expect("bv");
        let matter = create_matter(&conn, &bundle.id, &bv, "M1", None, None).expect("matter");
        (conn, matter.id, bv)
    }

    fn field(_bv: &str, field_id: &str, required: bool) -> crate::core::field_mapping::schema::FieldDef {
        crate::core::field_mapping::schema::FieldDef {
            id: String::new(),
            field_id: field_id.to_string(),
            label: field_id.to_string(),
            description: None,
            field_type: FieldType::Text,
            required,
            default: None,
            validation: None,
            group_id: None,
            options: Vec::new(),
            format: None,
            position: 0,
        }
    }

    #[test]
    fn test_three_level_validation_clean() {
        let (conn, matter_id, bv) = setup();
        create_field(&conn, &bv, &field(&bv, "name", true)).expect("field");
        set_matter_value(&conn, &matter_id, "name", &serde_json::json!("Acme")).expect("set");
        let report = validate_matter(&conn, &matter_id).expect("validate");
        assert!(report.is_valid, "expected valid, got issues: {:?}", report.issues);
        assert!(report.issues.is_empty());
    }

    #[test]
    fn test_three_level_validation_required_missing() {
        let (conn, matter_id, bv) = setup();
        create_field(&conn, &bv, &field(&bv, "name", true)).expect("field");
        // No value set.
        let report = validate_matter(&conn, &matter_id).expect("validate");
        assert!(!report.is_valid);
        assert!(report.issues.iter().any(|i| i.code == "required_field_missing"));
        assert!(report.issues.iter().any(|i| i.level == ValidationLevel::Field));
        assert!(report.issues.iter().any(|i| i.level == ValidationLevel::Matter));
    }

    #[test]
    fn test_three_level_validation_type_mismatch() {
        let (conn, matter_id, bv) = setup();
        let num = crate::core::field_mapping::schema::FieldDef {
            id: String::new(),
            field_id: "shares".to_string(),
            label: "Shares".to_string(),
            description: None,
            field_type: FieldType::Number,
            required: false,
            default: None,
            validation: None,
            group_id: None,
            options: Vec::new(),
            format: None,
            position: 0,
        };
        create_field(&conn, &bv, &num).expect("field");
        set_matter_value(&conn, &matter_id, "shares", &serde_json::json!("abc")).expect("set");
        let report = validate_matter(&conn, &matter_id).expect("validate");
        assert!(!report.is_valid);
        assert!(report.issues.iter().any(|i| i.code == "field_type_mismatch"));
    }
}
