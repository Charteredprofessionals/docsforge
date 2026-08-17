//! matter/form.rs — Grouped matter form renderer (TASK-111, REQ-031 / REQ-032).
//!
//! Builds a structured form view of a Matter: fields are grouped by their
//! `field_groups` membership (with an ungrouped fallback), and each field
//! carries its current value plus any validation error. This is the data
//! shape the v2 UI consumes to render an input form (REQ-032).

use rusqlite::Connection;
use serde_json::Value;

use crate::core::error::DocForgeError;
use crate::core::field_mapping::groups::FieldGroupDetail;
use crate::core::field_mapping::registry::list_fields;
use crate::core::field_mapping::schema::{validate_value, FieldDef, FieldType};
use crate::core::field_mapping::groups::list_field_groups;
use crate::core::matter::matter::{get_matter, Matter};
use crate::core::matter::matter_values::{list_matter_values, set_matter_value};

/// A field rendered in a form, with its current value and validation status.
#[derive(Debug, Clone, PartialEq)]
pub struct FormField {
    pub field: FieldDef,
    pub value: Option<Value>,
    pub error: Option<String>,
}

/// A group of fields rendered together in the form.
#[derive(Debug, Clone, PartialEq)]
pub struct FormGroup {
    pub group: FieldGroupDetail,
    pub fields: Vec<FormField>,
}

/// Full grouped form view of a Matter.
#[derive(Debug, Clone, PartialEq)]
pub struct MatterForm {
    pub matter: Matter,
    pub groups: Vec<FormGroup>,
    pub ungrouped_fields: Vec<FormField>,
}

/// Builds the complete grouped form for a matter (REQ-032).
pub fn render_matter_form(conn: &Connection, matter_id: &str) -> Result<MatterForm, DocForgeError> {
    let matter = get_matter(conn, matter_id)?
        .ok_or_else(|| DocForgeError::StorageMissing(format!("Matter '{matter_id}' not found")))?;

    let fields = list_fields(conn, &matter.bundle_version_id)?;
    let groups = list_field_groups(conn, Some(&matter.bundle_version_id), None)?;

    // Build a lookup of matter values keyed by field_id.
    let values = list_matter_values(conn, matter_id)?;
    let value_map: std::collections::HashMap<String, Value> = values
        .into_iter()
        .filter_map(|v| serde_json::from_str::<Value>(&v.value_json).ok().map(|parsed| (v.canonical_field_id, parsed)))
        .collect();

    // Build a FormField for each field, validating its current value.
    let mut form_fields: Vec<FormField> = Vec::with_capacity(fields.len());
    for field in &fields {
        let value = value_map.get(&field.field_id).cloned();
        let error = match &value {
            Some(v) => validate_value(field.field_type, v, field.required).err().map(|e| e.to_string()),
            None => None,
        };
        form_fields.push(FormField {
            field: field.clone(),
            value,
            error,
        });
    }

    // Partition fields into groups vs. ungrouped by group_id.
    let mut form_groups: Vec<FormGroup> = Vec::with_capacity(groups.len());
    for group in &groups {
        let mut grouped: Vec<FormField> = form_fields
            .iter()
            .filter(|ff| ff.field.group_id.as_deref() == Some(&group.id))
            .cloned()
            .collect();
        grouped.sort_by_key(|ff| ff.field.position);
        let group_fields: Vec<FieldDef> = grouped.iter().map(|ff| ff.field.clone()).collect();
        form_groups.push(FormGroup {
            group: FieldGroupDetail {
                group: group.clone(),
                fields: group_fields,
            },
            fields: grouped,
        });
    }

    let ungrouped_fields: Vec<FormField> = form_fields
        .into_iter()
        .filter(|ff| ff.field.group_id.is_none())
        .collect();

    Ok(MatterForm {
        matter,
        groups: form_groups,
        ungrouped_fields,
    })
}

/// Parses a raw user input string, stores it, and returns the updated form field.
///
/// Text fields wrap the raw string as a JSON string; other types attempt a
/// strict JSON parse. The result carries the validation status (REQ-031).
pub fn populate_matter_field(
    conn: &Connection,
    matter_id: &str,
    field_id: &str,
    raw_value: &str,
) -> Result<FormField, DocForgeError> {
    let matter = get_matter(conn, matter_id)?
        .ok_or_else(|| DocForgeError::StorageMissing(format!("Matter '{matter_id}' not found")))?;

    let fields = list_fields(conn, &matter.bundle_version_id)?;
    let field = fields
        .iter()
        .find(|f| f.field_id == field_id)
        .ok_or_else(|| DocForgeError::InvalidInput(format!("field '{field_id}' not found")))?
        .clone();

    let parsed: Value = if field.field_type == FieldType::Text {
        Value::String(raw_value.to_string())
    } else {
        serde_json::from_str(raw_value)
            .map_err(|e| DocForgeError::InvalidInput(format!("parse value for '{}': {e}", field.field_id)))?
    };

    set_matter_value(conn, matter_id, field_id, &parsed)?;

    let error = validate_value(field.field_type, &parsed, field.required)
        .err()
        .map(|e| e.to_string());

    Ok(FormField {
        field,
        value: Some(parsed),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::field_mapping::groups::create_group;
    use crate::core::field_mapping::registry::create_field;
    use crate::core::matter::matter::create_matter;
    use crate::core::field_mapping::schema::FieldType;
    use crate::schema::init_memory_db;

    fn setup() -> (Connection, String, String) {
        let conn = init_memory_db().expect("memory db");
        let bundle = crate::core::bundle::manifest::create_bundle(&conn, "Form Test", None, None).expect("bundle");
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

    fn field(_bv_id: &str, field_id: &str, group_id: Option<String>) -> FieldDef {
        FieldDef {
            id: String::new(),
            field_id: field_id.to_string(),
            label: field_id.to_string(),
            description: None,
            field_type: FieldType::Text,
            required: false,
            default: None,
            validation: None,
            group_id,
            options: Vec::new(),
            format: None,
            position: 0,
        }
    }

    #[test]
    fn test_render_matter_form_groups_fields() {
        let (conn, matter_id, bv) = setup();
        let group = create_group(&conn, None, "Group A", GroupScope::Shared, None).expect("group");
        create_field(&conn, &bv, &field(&bv, "a", Some(group.id.clone()))).expect("f a");
        create_field(&conn, &bv, &field(&bv, "b", Some(group.id.clone()))).expect("f b");
        create_field(&conn, &bv, &field(&bv, "loose", None)).expect("f loose");

        let form = render_matter_form(&conn, &matter_id).expect("render");
        assert_eq!(form.groups.len(), 1, "one group");
        assert_eq!(form.groups[0].fields.len(), 2, "two grouped fields");
        assert_eq!(form.ungrouped_fields.len(), 1, "one ungrouped field");
        assert_eq!(form.ungrouped_fields[0].field.field_id, "loose");
    }

    #[test]
    fn test_render_matter_form_validates_values() {
        let (conn, matter_id, bv) = setup();
        // Number field, set to a string -> validation error surfaced.
        let num_field = FieldDef {
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
        create_field(&conn, &bv, &num_field).expect("f num");
        set_matter_value(&conn, &matter_id, "shares", &Value::String("not-a-number".to_string())).expect("set bad");
        let form = render_matter_form(&conn, &matter_id).expect("render");
        let ff = form.ungrouped_fields.iter().find(|f| f.field.field_id == "shares").expect("found");
        assert!(ff.error.is_some(), "type mismatch must surface error");
    }

    #[test]
    fn test_populate_matter_field_accepts_json() {
        let (conn, matter_id, bv) = setup();
        let num_field = FieldDef {
            id: String::new(),
            field_id: "age".to_string(),
            label: "Age".to_string(),
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
        create_field(&conn, &bv, &num_field).expect("f num");
        let ff = populate_matter_field(&conn, &matter_id, "age", "42").expect("populate");
        assert_eq!(ff.value, Some(Value::from(42)));
        assert!(ff.error.is_none());
    }
}
