//! registry.rs — Canonical field & group persistence for DocForge (TASK-106).
//!
//! CRUD over the v5 `fields` / `field_groups` tables, bound to a
//! `bundle_version_id`. Writes are rejected on published bundle versions
//! (REQ-024 immutability), mirrored from `bundle::manifest` via the same
//! `published bundle_versions is immutable` trigger-message mapping.

use rusqlite::{params, Connection};
use std::str::FromStr;
use uuid::Uuid;

use crate::core::error::DocForgeError;
use crate::core::field_mapping::schema::{
    FieldDef, FieldGroup, FieldType, GroupScope, validate_field_schema,
};

/// Maps a rusqlite error to a precise `DocForgeError`.
///
/// Recognizes the published-version immutability trigger (REQ-024) and otherwise
/// reports storage failures as `StorageIo`.
fn map_rusqlite_error(e: rusqlite::Error, context: &str) -> DocForgeError {
    let message = e.to_string();
    if message.contains("published bundle_versions is immutable") {
        DocForgeError::PublishedBundleImmutable(context.to_string())
    } else {
        DocForgeError::StorageIo(format!("{context}: {message}"))
    }
}

/// Ensures the `fields` table carries an `options_json` column.
///
/// `schema.md` defines the v2.0.0 `fields` columns without `options`, yet REQ-026
/// lists `options` as a first-class attribute for `Select`/`Multiselect` fields.
/// Rather than edit `migrations.rs` (out of scope for this task), we add the column
/// idempotently at runtime so options round-trip cleanly. Existing v1–v5 migrations
/// are untouched.
fn ensure_fields_options_column(conn: &Connection) -> Result<(), DocForgeError> {
    let has_column: bool = conn
        .prepare("PRAGMA table_info(fields)")
        .map_err(|e| map_rusqlite_error(e, "Inspect fields schema"))?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| map_rusqlite_error(e, "Read fields columns"))?
        .filter_map(|r| r.ok())
        .any(|c| c == "options_json");
    if !has_column {
        conn.execute("ALTER TABLE fields ADD COLUMN options_json TEXT", [])
            .map_err(|e| map_rusqlite_error(e, "Add fields.options_json column"))?;
    }
    Ok(())
}

/// Reads a `FieldDef` from the current result-row position of a `fields` SELECT.
fn field_from_row(row: &rusqlite::Row) -> Result<FieldDef, rusqlite::Error> {
    let type_str: String = row.get(5)?;
    let field_type = FieldType::from_str(&type_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e)))?;
    let required: i32 = row.get(6)?;
    let default_json: Option<String> = row.get(7)?;
    let validation_json: Option<String> = row.get(8)?;
    let options_json: Option<String> = row.get(12)?;
    let options = match options_json {
        Some(s) => serde_json::from_str(&s).unwrap_or_default(),
        None => Vec::new(),
    };
    Ok(FieldDef {
        id: row.get(0)?,
        field_id: row.get(2)?,
        label: row.get(3)?,
        description: row.get(4)?,
        field_type,
        required: required != 0,
        default: default_json
            .and_then(|s| serde_json::from_str(&s).ok()),
        validation: validation_json
            .and_then(|s| serde_json::from_str(&s).ok()),
        options,
        format: row.get(9)?,
        group_id: row.get(10)?,
        position: row.get(11)?,
    })
}

/// Returns `Err(PublishedBundleImmutable)` if the bundle version is published.
///
/// Application-layer guard (REQ-024) so field writes never mutate a sealed
/// version even before the DB trigger would reject the statement.
fn ensure_version_writable(conn: &Connection, bundle_version_id: &str) -> Result<(), DocForgeError> {
    let status: String = conn
        .query_row(
            "SELECT status FROM bundle_versions WHERE id = ?1",
            params![bundle_version_id],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DocForgeError::StorageMissing(format!(
                "Bundle version '{bundle_version_id}' not found"
            )),
            other => map_rusqlite_error(other, "Read bundle version status"),
        })?;
    if status == "published" {
        return Err(DocForgeError::PublishedBundleImmutable(
            bundle_version_id.to_string(),
        ));
    }
    Ok(())
}

/// Creates a canonical field on a bundle version (REQ-026).
///
/// Validates the definition, then inserts into `fields`. The owning bundle
/// version must be a draft (published versions are immutable). Returns the
/// persisted `FieldDef` with its generated `id`.
pub fn create_field(
    conn: &Connection,
    bundle_version_id: &str,
    field: &FieldDef,
) -> Result<FieldDef, DocForgeError> {
    ensure_version_writable(conn, bundle_version_id)?;
    validate_field_schema(field)?;
    ensure_fields_options_column(conn)?;

    let id = Uuid::new_v4().to_string();
    let default_json = field
        .default
        .as_ref()
        .and_then(|v| serde_json::to_string(v).ok());
    let validation_json = field
        .validation
        .as_ref()
        .and_then(|v| serde_json::to_string(v).ok());
    let options_json = serde_json::to_string(&field.options).ok();

    conn.execute(
        "INSERT INTO fields
         (id, bundle_version_id, field_id, label, description, type, required,
          default_json, validation_json, format, group_id, position, options_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            id,
            bundle_version_id,
            field.field_id,
            field.label,
            field.description,
            field.field_type.as_db_str(),
            field.required as i32,
            default_json,
            validation_json,
            field.format,
            field.group_id,
            field.position,
            options_json,
        ],
    )
    .map_err(|e| map_rusqlite_error(e, "Insert field"))?;

    let mut persisted = field.clone();
    persisted.id = id;
    Ok(persisted)
}

/// Updates a canonical field definition (validated) by its database `id`.
///
/// Only draft bundle versions accept writes; a published owner yields
/// `PublishedBundleImmutable` (REQ-024).
pub fn update_field(
    conn: &Connection,
    field_db_id: &str,
    field: &FieldDef,
) -> Result<FieldDef, DocForgeError> {
    validate_field_schema(field)?;
    ensure_fields_options_column(conn)?;

    let bundle_version_id: String = conn
        .query_row(
            "SELECT bundle_version_id FROM fields WHERE id = ?1",
            params![field_db_id],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DocForgeError::StorageMissing(format!("Field '{field_db_id}' not found"))
            }
            other => map_rusqlite_error(other, "Read field owner"),
        })?;
    ensure_version_writable(conn, &bundle_version_id)?;

    let default_json = field
        .default
        .as_ref()
        .and_then(|v| serde_json::to_string(v).ok());
    let validation_json = field
        .validation
        .as_ref()
        .and_then(|v| serde_json::to_string(v).ok());
    let options_json = serde_json::to_string(&field.options).ok();

    let affected = conn
        .execute(
            "UPDATE fields SET
             field_id = ?2, label = ?3, description = ?4, type = ?5, required = ?6,
             default_json = ?7, validation_json = ?8, format = ?9, group_id = ?10,
             position = ?11, options_json = ?12
             WHERE id = ?1",
            params![
                field_db_id,
                field.field_id,
                field.label,
                field.description,
                field.field_type.as_db_str(),
                field.required as i32,
                default_json,
                validation_json,
                field.format,
                field.group_id,
                field.position,
                options_json,
            ],
        )
        .map_err(|e| map_rusqlite_error(e, "Update field"))?;

    if affected == 0 {
        return Err(DocForgeError::StorageMissing(format!(
            "Field '{field_db_id}' not found"
        )));
    }
    let mut persisted = field.clone();
    persisted.id = field_db_id.to_string();
    Ok(persisted)
}

/// Lists the canonical fields of a bundle version, ordered by `position`.
pub fn list_fields(conn: &Connection, bundle_version_id: &str) -> Result<Vec<FieldDef>, DocForgeError> {
    ensure_fields_options_column(conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, bundle_version_id, field_id, label, description, type, required,
                    default_json, validation_json, format, group_id, position, options_json
             FROM fields WHERE bundle_version_id = ?1
             ORDER BY position ASC, field_id ASC",
        )
        .map_err(|e| map_rusqlite_error(e, "Prepare field list"))?;
    let rows = stmt
        .query_map(params![bundle_version_id], field_from_row)
        .map_err(|e| map_rusqlite_error(e, "Query fields"))?;
    let mut fields = Vec::new();
    for row in rows {
        fields.push(row.map_err(|e| map_rusqlite_error(e, "Map field row"))?);
    }
    Ok(fields)
}

/// Removes a field by its database `id`, cascading to its `field_mappings` rows.
///
/// `field_mappings.canonical_field_id` is not declared as a FK in the v5 migration,
/// so the mapping rows are deleted explicitly to avoid orphans.
pub fn remove_field(conn: &Connection, field_db_id: &str) -> Result<(), DocForgeError> {
    conn.execute(
        "DELETE FROM field_mappings WHERE canonical_field_id = ?1",
        params![field_db_id],
    )
    .map_err(|e| map_rusqlite_error(e, "Delete field mappings"))?;
    let affected = conn
        .execute("DELETE FROM fields WHERE id = ?1", params![field_db_id])
        .map_err(|e| map_rusqlite_error(e, "Delete field"))?;
    if affected == 0 {
        return Err(DocForgeError::StorageMissing(format!(
            "Field '{field_db_id}' not found"
        )));
    }
    Ok(())
}

/// Creates a field group (REQ-027) on an optional bundle version.
///
/// `bundle_version_id` of `None` creates a global reusable group available to any
/// version. Returns the persisted `FieldGroup` with its generated `id`.
pub fn create_field_group(
    conn: &Connection,
    bundle_version_id: Option<&str>,
    group: &FieldGroup,
) -> Result<FieldGroup, DocForgeError> {
    if group.name.trim().is_empty() {
        return Err(DocForgeError::InvalidInput(
            "field group name must not be empty".to_string(),
        ));
    }
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO field_groups
         (id, bundle_version_id, name, description, scope, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id,
            bundle_version_id,
            group.name,
            group.description,
            group.scope.as_db_str(),
            group.sort_order,
        ],
    )
    .map_err(|e| map_rusqlite_error(e, "Insert field group"))?;

    let mut persisted = group.clone();
    persisted.id = id;
    persisted.bundle_version_id = bundle_version_id.map(str::to_string);
    Ok(persisted)
}

/// Reads a `FieldGroup` from the current result-row position of a `field_groups` SELECT.
fn group_from_row(row: &rusqlite::Row) -> Result<FieldGroup, rusqlite::Error> {
    let scope_str: String = row.get(4)?;
    let scope = GroupScope::from_str(&scope_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e)))?;
    Ok(FieldGroup {
        id: row.get(0)?,
        bundle_version_id: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        scope,
        sort_order: row.get(5)?,
    })
}

/// Lists field groups for a bundle version.
///
/// Returns groups owned by `bundle_version_id` plus global reusable groups
/// (`bundle_version_id IS NULL`). Passing `None` returns every group.
pub fn list_field_groups(
    conn: &Connection,
    bundle_version_id: Option<&str>,
) -> Result<Vec<FieldGroup>, DocForgeError> {
    let mut stmt = match bundle_version_id {
        Some(_) => conn
            .prepare(
                "SELECT id, bundle_version_id, name, description, scope, sort_order
                 FROM field_groups
                 WHERE bundle_version_id = ?1 OR bundle_version_id IS NULL
                 ORDER BY sort_order ASC, name ASC",
            )
            .map_err(|e| map_rusqlite_error(e, "Prepare version group list"))?,
        None => conn
            .prepare(
                "SELECT id, bundle_version_id, name, description, scope, sort_order
                 FROM field_groups ORDER BY sort_order ASC, name ASC",
            )
            .map_err(|e| map_rusqlite_error(e, "Prepare group list"))?,
    };
    let rows = match bundle_version_id {
        Some(bv) => stmt
            .query_map(params![bv], group_from_row)
            .map_err(|e| map_rusqlite_error(e, "Query version groups"))?,
        None => stmt
            .query_map([], group_from_row)
            .map_err(|e| map_rusqlite_error(e, "Query groups"))?,
    };
    let mut groups = Vec::new();
    for row in rows {
        groups.push(row.map_err(|e| map_rusqlite_error(e, "Map group row"))?);
    }
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bundle::create_bundle;
    use crate::schema::init_memory_db;
    use rusqlite::params;
    use serde_json::json;

    /// Creates a draft bundle version and returns its id.
    fn draft_version(conn: &Connection) -> String {
        let record = create_bundle(conn, "Test Bundle", None, None).expect("create bundle");
        conn.query_row(
            "SELECT id FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC LIMIT 1",
            params![record.id],
            |row| row.get(0),
        )
        .expect("read head version id")
    }

    fn sample_field(_bundle_version_id: &str, field_id: &str, field_type: FieldType) -> FieldDef {
        FieldDef {
            id: String::new(),
            field_id: field_id.to_string(),
            label: format!("Label {field_id}"),
            description: None,
            field_type,
            required: true,
            default: match field_type {
                FieldType::Boolean => Some(json!(false)),
                FieldType::Number | FieldType::Currency | FieldType::Percentage => Some(json!(0)),
                _ => Some(json!("x")),
            },
            validation: None,
            options: if matches!(field_type, FieldType::Select | FieldType::Multiselect) {
                vec!["one".to_string(), "two".to_string()]
            } else {
                Vec::new()
            },
            format: None,
            group_id: None,
            position: 0,
        }
    }

    #[test]
    fn test_create_and_list_field() {
        let conn = init_memory_db().expect("init");
        let bv = draft_version(&conn);
        let field = sample_field(&bv, "company.name", FieldType::Text);
        let created = create_field(&conn, &bv, &field).expect("create field");
        assert!(!created.id.is_empty());

        let listed = list_fields(&conn, &bv).expect("list fields");
        assert_eq!(listed.len(), 1);
        let got = &listed[0];
        assert_eq!(got.field_type, FieldType::Text);
        assert_eq!(got.label, "Label company.name");
        assert!(got.required);
        assert_eq!(got.options, Vec::<String>::new());
    }

    #[test]
    fn test_create_select_field_round_trips_options() {
        let conn = init_memory_db().expect("init");
        let bv = draft_version(&conn);
        let field = sample_field(&bv, "doc.status", FieldType::Select);
        let created = create_field(&conn, &bv, &field).expect("create select field");
        assert_eq!(created.options, vec!["one".to_string(), "two".to_string()]);

        let listed = list_fields(&conn, &bv).expect("list fields");
        assert_eq!(listed[0].options, vec!["one".to_string(), "two".to_string()]);
    }

    #[test]
    fn test_update_field_changes_attributes() {
        let conn = init_memory_db().expect("init");
        let bv = draft_version(&conn);
        let mut field = sample_field(&bv, "company.name", FieldType::Text);
        let created = create_field(&conn, &bv, &field).expect("create field");

        field.label = "Renamed".to_string();
        field.required = false;
        update_field(&conn, &created.id, &field).expect("update field");

        let listed = list_fields(&conn, &bv).expect("list fields");
        assert_eq!(listed[0].label, "Renamed");
        assert!(!listed[0].required);
    }

    #[test]
    fn test_remove_field_cascades_mappings() {
        let conn = init_memory_db().expect("init");
        let bv = draft_version(&conn);
        let field = sample_field(&bv, "company.name", FieldType::Text);
        let created = create_field(&conn, &bv, &field).expect("create field");

        conn.execute(
            "INSERT INTO bundle_documents (id, bundle_version_id, position)
             VALUES ('bd1', ?1, 0)",
            params![bv],
        )
        .expect("insert bundle document");
        conn.execute(
            "INSERT INTO field_mappings (id, bundle_version_id, document_id, placeholder, canonical_field_id)
             VALUES ('fm1', ?1, 'bd1', '{{company_name}}', ?2)",
            params![bv, created.id],
        )
        .expect("insert mapping");

        remove_field(&conn, &created.id).expect("remove field");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM field_mappings WHERE canonical_field_id = ?1",
                params![created.id],
                |row| row.get(0),
            )
            .expect("count mappings");
        assert_eq!(count, 0, "mappings must be removed with the field");
        assert!(list_fields(&conn, &bv).expect("list").is_empty());
    }

    #[test]
    fn test_create_field_on_published_version_is_immutable() {
        let conn = init_memory_db().expect("init");
        let bv = draft_version(&conn);
        conn.execute(
            "UPDATE bundle_versions SET status = 'published' WHERE id = ?1",
            params![bv],
        )
        .expect("publish version");
        let field = sample_field(&bv, "company.name", FieldType::Text);
        let err = create_field(&conn, &bv, &field).expect_err("published version rejects writes");
        assert!(matches!(err, DocForgeError::PublishedBundleImmutable(_)));
    }

    #[test]
    fn test_create_field_group_scopes() {
        let conn = init_memory_db().expect("init");
        let bv1 = draft_version(&conn);
        let bv2 = {
            let record = create_bundle(&conn, "Other Bundle", None, None).expect("create bundle");
            conn.query_row(
                "SELECT id FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC LIMIT 1",
                params![record.id],
                |row| row.get::<_, String>(0),
            )
            .expect("read head version id")
        };

        let shared = create_field_group(
            &conn,
            Some(&bv1),
            &FieldGroup {
                id: String::new(),
                bundle_version_id: None,
                name: "Company (shared)".to_string(),
                description: None,
                scope: GroupScope::Shared,
                sort_order: 0,
            },
        )
        .expect("create shared group");

        let doc_specific = create_field_group(
            &conn,
            Some(&bv2),
            &FieldGroup {
                id: String::new(),
                bundle_version_id: None,
                name: "Annex (doc-specific)".to_string(),
                description: None,
                scope: GroupScope::DocumentSpecific,
                sort_order: 1,
            },
        )
        .expect("create document-specific group");

        let global = create_field_group(
            &conn,
            None,
            &FieldGroup {
                id: String::new(),
                bundle_version_id: None,
                name: "Global".to_string(),
                description: None,
                scope: GroupScope::Shared,
                sort_order: 2,
            },
        )
        .expect("create global group");

        let bv1_groups = list_field_groups(&conn, Some(&bv1)).expect("list bv1 groups");
        assert_eq!(bv1_groups.len(), 2, "owned + global groups returned");
        assert!(bv1_groups.iter().any(|g| g.id == shared.id && g.scope == GroupScope::Shared));
        assert!(bv1_groups.iter().any(|g| g.id == global.id));

        let bv2_groups = list_field_groups(&conn, Some(&bv2)).expect("list bv2 groups");
        assert_eq!(bv2_groups.len(), 2);
        assert!(bv2_groups
            .iter()
            .any(|g| g.id == doc_specific.id && g.scope == GroupScope::DocumentSpecific));

        let all = list_field_groups(&conn, None).expect("list all groups");
        assert_eq!(all.len(), 3);
    }
}
