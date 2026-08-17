//! matter/matter_values.rs — Matter value store (TASK-110, REQ-030).
//!
//! Row-per-value editable store. Values are aggregated into a JSON object
//! (`matter_to_json`) for consumption by the mapping layer's `resolve_value`.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::core::error::DocForgeError;
use crate::core::matter::matter::get_matter;
#[cfg(test)]
use crate::core::matter::matter::Matter;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// One row in the `matter_values` table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MatterValue {
    pub id: String,
    pub matter_id: String,
    pub canonical_field_id: String,
    pub value_json: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

/// Sets (inserts or replaces) a single field value for a matter.
///
/// Validates that the `canonical_field_id` exists in the `fields` table for
/// the same bundle version as the matter.
pub fn set_matter_value(
    conn: &Connection,
    matter_id: &str,
    canonical_field_id: &str,
    value: &Value,
) -> Result<MatterValue, DocForgeError> {
    let matter = get_matter(conn, matter_id)?
        .ok_or_else(|| DocForgeError::StorageMissing(format!("Matter '{matter_id}' not found")))?;

    // Validate field exists in this bundle version.
    let field_exists: i32 = conn
        .query_row(
            "SELECT COUNT(1) FROM fields WHERE field_id = ?1 AND bundle_version_id = ?2",
            rusqlite::params![canonical_field_id, matter.bundle_version_id],
            |r| r.get(0),
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Check field exists: {e}")))?;
    if field_exists == 0 {
        return Err(DocForgeError::InvalidInput(format!(
            "canonical_field_id '{}' does not exist in bundle version '{}'",
            canonical_field_id, matter.bundle_version_id
        )));
    }

    let id = format!("mv_{}", Uuid::new_v4());
    let now = chrono::Utc::now().to_rfc3339();
    let value_json = serde_json::to_string(value)
        .map_err(|e| DocForgeError::InvalidInput(format!("Serialize value: {e}")))?;

    conn.execute(
        "INSERT OR REPLACE INTO matter_values (id, matter_id, canonical_field_id, value_json, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, matter_id, canonical_field_id, value_json, now],
    )
    .map_err(|e| DocForgeError::StorageIo(format!("Upsert matter_value: {e}")))?;

    Ok(MatterValue {
        id,
        matter_id: matter_id.to_string(),
        canonical_field_id: canonical_field_id.to_string(),
        value_json,
        updated_at: now,
    })
}

/// Fetches a single matter value by field id.
pub fn get_matter_value(
    conn: &Connection,
    matter_id: &str,
    canonical_field_id: &str,
) -> Result<Option<MatterValue>, DocForgeError> {
    let mut stmt = conn
        .prepare("SELECT id, matter_id, canonical_field_id, value_json, updated_at FROM matter_values WHERE matter_id = ?1 AND canonical_field_id = ?2")
        .map_err(|e| DocForgeError::StorageIo(format!("Prepare get_matter_value: {e}")))?;

    let rows = stmt
        .query_map(rusqlite::params![matter_id, canonical_field_id], row_to_matter_value)
        .map_err(|e| DocForgeError::StorageIo(format!("Query matter_value: {e}")))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| DocForgeError::StorageIo(format!("Map matter_value row: {e}")))?);
    }
    Ok(out.into_iter().next())
}

/// Lists all values for a matter.
pub fn list_matter_values(
    conn: &Connection,
    matter_id: &str,
) -> Result<Vec<MatterValue>, DocForgeError> {
    let mut stmt = conn
        .prepare("SELECT id, matter_id, canonical_field_id, value_json, updated_at FROM matter_values WHERE matter_id = ?1 ORDER BY canonical_field_id")
        .map_err(|e| DocForgeError::StorageIo(format!("Prepare list_matter_values: {e}")))?;

    let rows = stmt
        .query_map([matter_id], row_to_matter_value)
        .map_err(|e| DocForgeError::StorageIo(format!("Query matter_values: {e}")))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| DocForgeError::StorageIo(format!("Map matter_value row: {e}")))?);
    }
    Ok(out)
}

/// Aggregates all matter_values into a single JSON object.
///
/// Returns `{ "canonical_field_id": <parsed_value>, ... }`.
pub fn matter_to_json(
    conn: &Connection,
    matter_id: &str,
) -> Result<Value, DocForgeError> {
    let values = list_matter_values(conn, matter_id)?;
    let mut obj = serde_json::Map::new();
    for v in values {
        let parsed: Value = serde_json::from_str(&v.value_json)
            .map_err(|e| DocForgeError::StorageIo(format!("Parse value_json for {}: {e}", v.canonical_field_id)))?;
        obj.insert(v.canonical_field_id, parsed);
    }
    Ok(Value::Object(obj))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn row_to_matter_value(row: &rusqlite::Row) -> Result<MatterValue, rusqlite::Error> {
    Ok(MatterValue {
        id: row.get(0)?,
        matter_id: row.get(1)?,
        canonical_field_id: row.get(2)?,
        value_json: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::field_mapping::schema::FieldDef;
    use crate::core::field_mapping::registry::create_field;
    use crate::core::matter::matter::create_matter;
    use crate::schema::init_memory_db;

    fn setup_matter() -> (Connection, String, Matter) {
        let conn = init_memory_db().expect("memory db");
        let bundle = crate::core::bundle::manifest::create_bundle(&conn, "Values Test", None, None).expect("create bundle");
        let bv_id = conn
            .query_row(
                "SELECT id FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC LIMIT 1",
                [&bundle.id],
                |r| r.get::<_, String>(0),
            )
            .expect("head version");
        let matter = create_matter(&conn, &bundle.id, &bv_id, "M1", None, None).expect("create matter");
        (conn, bv_id, matter)
    }

    fn make_field(field_id: &str) -> FieldDef {
        FieldDef {
            id: String::new(),
            field_id: field_id.to_string(),
            label: field_id.to_string(),
            description: None,
            field_type: crate::core::field_mapping::schema::FieldType::Text,
            required: false,
            default: None,
            validation: None,
            group_id: None,
            options: Vec::new(),
            format: None,
            position: 0,
        }
    }

    #[test]
    fn test_set_and_get_matter_value() {
        let (conn, bv_id, matter) = setup_matter();
        create_field(&conn, &bv_id, &make_field("company.name")).expect("field");
        set_matter_value(&conn, &matter.id, "company.name", &serde_json::json!("Acme")).expect("set");
        let got = get_matter_value(&conn, &matter.id, "company.name").expect("get").expect("present");
        assert_eq!(got.canonical_field_id, "company.name");
        assert_eq!(got.value_json, "\"Acme\"");
    }

    #[test]
    fn test_set_matter_value_rejects_unknown_field() {
        let (conn, _bv_id, matter) = setup_matter();
        let err = set_matter_value(&conn, &matter.id, "does.not.exist", &serde_json::json!(1))
            .expect_err("rejected");
        assert!(matches!(err, DocForgeError::InvalidInput(_)));
    }

    #[test]
    fn test_list_matter_values() {
        let (conn, bv_id, matter) = setup_matter();
        create_field(&conn, &bv_id, &make_field("f1")).expect("field f1");
        create_field(&conn, &bv_id, &make_field("f2")).expect("field f2");
        set_matter_value(&conn, &matter.id, "f1", &serde_json::json!(1)).expect("set f1");
        set_matter_value(&conn, &matter.id, "f2", &serde_json::json!(2)).expect("set f2");
        let list = list_matter_values(&conn, &matter.id).expect("list");
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_matter_to_json_round_trip() {
        let (conn, bv_id, matter) = setup_matter();
        create_field(&conn, &bv_id, &make_field("name")).expect("field name");
        create_field(&conn, &bv_id, &make_field("count")).expect("field count");
        set_matter_value(&conn, &matter.id, "name", &serde_json::json!("Alice")).expect("set name");
        set_matter_value(&conn, &matter.id, "count", &serde_json::json!(42)).expect("set count");
        let json = matter_to_json(&conn, &matter.id).expect("to json");
        assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("Alice"));
        assert_eq!(json.get("count").and_then(|v| v.as_i64()), Some(42));
    }
}
