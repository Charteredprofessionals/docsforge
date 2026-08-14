//! groups.rs — Field group scope semantics & matter-form grouping (TASK-107, REQ-027).
//!
//! Builds on the persistence surface from `registry` (TASK-106): groups are
//! already stored in `field_groups` with a `scope` of `shared` /
//! `document_specific` and an owning `bundle_version_id` (or `NULL` for a
//! global reusable group). This module adds the SCOPE-aware read/assignment
//! semantics that drive the Matter form's Shared-vs-DocumentSpecific split
//! (AC-027), plus the `GroupSummary` used by the later Bundle Health Check.

use rusqlite::{params, Connection, Error as RusqliteError};

use crate::core::error::DocForgeError;
use crate::core::field_mapping::registry;
use crate::core::field_mapping::schema::{FieldDef, FieldGroup, GroupScope};

/// Aggregated counts for a bundle version's field groups and fields.
///
/// Consumed by the Bundle Health Check (later wave): `shared_count` and
/// `document_specific_count` summarize the group scope split, while
/// `total_fields` reports how many canonical fields the version carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupSummary {
    /// Number of `Shared` groups owned by the bundle version.
    pub shared_count: i64,
    /// Number of `DocumentSpecific` groups owned by the bundle version.
    pub document_specific_count: i64,
    /// Total number of canonical fields on the bundle version.
    pub total_fields: i64,
}

/// A field group together with its member fields (REQ-027).
///
/// Returned by detail queries that need the full group + field roster.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldGroupDetail {
    /// The owning field group.
    pub group: FieldGroup,
    /// Canonical fields assigned to this group, ordered by `position`.
    pub fields: Vec<FieldDef>,
}

/// Result of assigning a field to a group (or clearing its group).
#[derive(Debug, Clone, PartialEq)]
pub struct GroupAssignmentResult {
    /// Database id of the field that was assigned.
    pub field_db_id: String,
    /// Target group id, or `None` if the field was ungrouped.
    pub group_id: Option<String>,
    /// Whether the assignment succeeded.
    pub assigned: bool,
}

/// Computes the next `sort_order` for a new group on the given bundle version.
///
/// Global groups (`bundle_version_id IS NULL`) get their own independent
/// sequence so a version's groups and the global pool do not collide.
fn next_sort_order(
    conn: &Connection,
    bundle_version_id: Option<&str>,
) -> Result<i64, DocForgeError> {
    let max: Option<i64> = match bundle_version_id {
        Some(bv) => conn
            .query_row(
                "SELECT MAX(sort_order) FROM field_groups WHERE bundle_version_id = ?1",
                params![bv],
                |row| row.get(0),
            )
            .ok(),
        None => conn
            .query_row(
                "SELECT MAX(sort_order) FROM field_groups WHERE bundle_version_id IS NULL",
                [],
                |row| row.get(0),
            )
            .ok(),
    };
    Ok(max.unwrap_or(-1) + 1)
}

/// Creates a field group with the given scope, delegating persistence to the registry.
///
/// This is the canonical convenience entry point for REQ-027 group creation:
/// it derives the next `sort_order` automatically and reuses
/// `registry::create_field_group` so no SQL is duplicated. A global group is
/// created when `bundle_version_id` is `None`.
pub fn create_group(
    conn: &Connection,
    bundle_version_id: Option<&str>,
    name: &str,
    scope: GroupScope,
    description: Option<&str>,
) -> Result<FieldGroup, DocForgeError> {
    let sort_order = next_sort_order(conn, bundle_version_id)?;
    let group = FieldGroup {
        id: String::new(),
        bundle_version_id: None,
        name: name.to_string(),
        description: description.map(str::to_string),
        scope,
        sort_order,
    };
    registry::create_field_group(conn, bundle_version_id, &group)
}

/// Creates a field group with scope-aware validation (REQ-027).
///
/// Delegates persistence to `registry::create_field_group` after enforcing
/// the invariant that `DocumentSpecific` groups must belong to a bundle
/// version. `Shared` groups may be global (`bundle_version_id` is `None`)
/// or version-scoped.
pub fn create_field_group(
    conn: &Connection,
    bundle_version_id: Option<&str>,
    group: &FieldGroup,
) -> Result<FieldGroup, DocForgeError> {
    if group.scope == GroupScope::DocumentSpecific && bundle_version_id.is_none() {
        return Err(DocForgeError::InvalidInput(
            "DocumentSpecific field groups must belong to a bundle version".to_string(),
        ));
    }
    registry::create_field_group(conn, bundle_version_id, group)
}

/// Lists field groups for a bundle version, optionally filtered by scope.
///
/// Passing `Some(GroupScope::Shared)` returns only shared groups; passing
/// `Some(GroupScope::DocumentSpecific)` returns only document-specific groups;
/// passing `None` returns every group the version can see (owned + global).
pub fn list_field_groups(
    conn: &Connection,
    bundle_version_id: Option<&str>,
    scope: Option<GroupScope>,
) -> Result<Vec<FieldGroup>, DocForgeError> {
    let groups = registry::list_field_groups(conn, bundle_version_id)?;
    match scope {
        Some(s) => Ok(groups.into_iter().filter(|g| g.scope == s).collect()),
        None => Ok(groups),
    }
}

/// Reorders fields within a group according to the provided id sequence.
///
/// Each field's `position` is set to its index in `field_ids_in_order`.
/// Fields not present in the sequence are left untouched. Returns
/// `StorageMissing` if any listed field does not belong to the group.
pub fn reorder_group_fields(
    conn: &Connection,
    group_id: &str,
    field_ids_in_order: &[String],
) -> Result<(), DocForgeError> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| map_rusqlite_error(e, "Begin reorder transaction"))?;

    for (position, field_id) in field_ids_in_order.iter().enumerate() {
        let affected = tx
            .execute(
                "UPDATE fields SET position = ?1 WHERE id = ?2 AND group_id = ?3",
                params![position as i64, field_id, group_id],
            )
            .map_err(|e| map_rusqlite_error(e, "Reorder field"))?;
        if affected == 0 {
            return Err(DocForgeError::StorageMissing(format!(
                "field '{field_id}' not found in group '{group_id}'"
            )));
        }
    }

    tx.commit()
        .map_err(|e| map_rusqlite_error(e, "Commit reorder"))?;
    Ok(())
}

/// Lists groups visible to a bundle version: those it owns plus global groups.
///
/// Ordered by `sort_order` then `name` (matching the registry's natural order).
/// This is the flat view used when scope separation is not required.
pub fn list_groups_for_version(
    conn: &Connection,
    bundle_version_id: &str,
) -> Result<Vec<FieldGroup>, DocForgeError> {
    registry::list_field_groups(conn, Some(bundle_version_id))
}

/// Lists all groups for a bundle version (owned + global), Shared first.
///
/// The ordering is the key REQ-027 contract for the Matter form UI: every
/// `Shared` group precedes every `DocumentSpecific` group, with ties broken by
/// `sort_order` then `name`. This drives the visual separation between
/// Bundle-wide and per-document field blocks.
pub fn list_groups_with_shared_first(
    conn: &Connection,
    bundle_version_id: &str,
) -> Result<Vec<FieldGroup>, DocForgeError> {
    let mut groups = registry::list_field_groups(conn, Some(bundle_version_id))?;
    groups.sort_by(|a, b| {
        let rank = |s: GroupScope| match s {
            GroupScope::Shared => 0i64,
            GroupScope::DocumentSpecific => 1i64,
        };
        rank(a.scope)
            .cmp(&rank(b.scope))
            .then(a.sort_order.cmp(&b.sort_order))
            .then(a.name.cmp(&b.name))
    });
    Ok(groups)
}

/// Assigns a canonical field to a field group (or clears it when `group_id` is `None`).
///
/// The target group must belong to the same `bundle_version_id` as the field,
/// or be a global group (`bundle_version_id IS NULL`). Otherwise an
/// `InvalidInput` error is returned, keeping cross-version group leakage
/// impossible. A missing field or group yields a precise `StorageMissing` /
/// `InvalidInput` error rather than a silent no-op.
pub fn assign_field_to_group(
    conn: &Connection,
    field_db_id: &str,
    group_id: Option<&str>,
) -> Result<(), DocForgeError> {
    match group_id {
        None => {
            conn.execute(
                "UPDATE fields SET group_id = NULL WHERE id = ?1",
                params![field_db_id],
            )
            .map_err(|e| map_rusqlite_error(e, "Clear field group"))?;
            Ok(())
        }
        Some(gid) => {
            let field_bv: String = conn
                .query_row(
                    "SELECT bundle_version_id FROM fields WHERE id = ?1",
                    params![field_db_id],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    RusqliteError::QueryReturnedNoRows => DocForgeError::StorageMissing(format!(
                        "field '{field_db_id}' not found"
                    )),
                    other => map_rusqlite_error(other, "Read field owner"),
                })?;

            let group_bv: Option<String> = conn
                .query_row(
                    "SELECT bundle_version_id FROM field_groups WHERE id = ?1",
                    params![gid],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    RusqliteError::QueryReturnedNoRows => DocForgeError::InvalidInput(format!(
                        "field group '{gid}' not found"
                    )),
                    other => map_rusqlite_error(other, "Read group owner"),
                })?;

            if let Some(gbv) = group_bv {
                if gbv != field_bv {
                    return Err(DocForgeError::InvalidInput(format!(
                        "field group '{gid}' belongs to bundle version '{gbv}' \
                         but field belongs to bundle version '{field_bv}'"
                    )));
                }
            }

            let affected = conn
                .execute(
                    "UPDATE fields SET group_id = ?1 WHERE id = ?2",
                    params![gid, field_db_id],
                )
                .map_err(|e| map_rusqlite_error(e, "Assign field to group"))?;
            if affected == 0 {
                return Err(DocForgeError::StorageMissing(format!(
                    "field '{field_db_id}' not found"
                )));
            }
            Ok(())
        }
    }
}

/// Returns the group scope split and field count for a bundle version.
///
/// Counts only groups owned by `bundle_version_id` (global reusable groups are
/// excluded from the split) plus every canonical field on that version.
pub fn group_summary(
    conn: &Connection,
    bundle_version_id: &str,
) -> Result<GroupSummary, DocForgeError> {
    let shared_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM field_groups
             WHERE bundle_version_id = ?1 AND scope = 'shared'",
            params![bundle_version_id],
            |row| row.get(0),
        )
        .map_err(|e| map_rusqlite_error(e, "Count shared groups"))?;
    let document_specific_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM field_groups
             WHERE bundle_version_id = ?1 AND scope = 'document_specific'",
            params![bundle_version_id],
            |row| row.get(0),
        )
        .map_err(|e| map_rusqlite_error(e, "Count document-specific groups"))?;
    let total_fields: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM fields WHERE bundle_version_id = ?1",
            params![bundle_version_id],
            |row| row.get(0),
        )
        .map_err(|e| map_rusqlite_error(e, "Count fields"))?;
    Ok(GroupSummary {
        shared_count,
        document_specific_count,
        total_fields,
    })
}

/// Maps a rusqlite error to a precise `DocForgeError` for this module.
fn map_rusqlite_error(e: RusqliteError, context: &str) -> DocForgeError {
    DocForgeError::StorageIo(format!("{context}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bundle::create_bundle;
    use crate::core::field_mapping::registry::create_field;
    use crate::core::field_mapping::schema::{FieldDef, FieldType, GroupScope};
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

    /// Builds a minimal valid `FieldDef` for insertion under `bundle_version_id`.
    fn sample_field(_bundle_version_id: &str, field_id: &str) -> FieldDef {
        FieldDef {
            id: String::new(),
            field_id: field_id.to_string(),
            label: format!("Label {field_id}"),
            description: None,
            field_type: FieldType::Text,
            required: true,
            default: Some(json!("x")),
            validation: None,
            options: Vec::new(),
            format: None,
            group_id: None,
            position: 0,
        }
    }

    #[test]
    fn test_create_group_with_scope() {
        let conn = init_memory_db().expect("init");
        let bv = draft_version(&conn);

        let shared = create_group(&conn, Some(&bv), "Company", GroupScope::Shared, None)
            .expect("create shared group");
        let doc_specific = create_group(
            &conn,
            Some(&bv),
            "Annex",
            GroupScope::DocumentSpecific,
            Some("Per-document annex"),
        )
        .expect("create document-specific group");

        assert_eq!(shared.scope, GroupScope::Shared);
        assert_eq!(doc_specific.scope, GroupScope::DocumentSpecific);
        assert_eq!(shared.bundle_version_id.as_deref(), Some(bv.as_str()));
        assert!(shared.sort_order < doc_specific.sort_order);
    }

    #[test]
    fn test_list_groups_shared_first() {
        let conn = init_memory_db().expect("init");
        let bv = draft_version(&conn);

        // Insert document-specific first to prove ordering is not by insertion.
        let _doc = create_group(
            &conn,
            Some(&bv),
            "Annex",
            GroupScope::DocumentSpecific,
            None,
        )
        .expect("create doc-specific group");
        let _shared = create_group(&conn, Some(&bv), "Company", GroupScope::Shared, None)
            .expect("create shared group");

        let ordered = list_groups_with_shared_first(&conn, &bv).expect("list shared first");
        assert!(ordered.len() >= 2);
        let first_shared_index = ordered
            .iter()
            .position(|g| g.scope == GroupScope::Shared)
            .expect("has a shared group");
        let first_doc_index = ordered
            .iter()
            .position(|g| g.scope == GroupScope::DocumentSpecific)
            .expect("has a document-specific group");
        assert!(
            first_shared_index < first_doc_index,
            "Shared groups must precede DocumentSpecific groups"
        );
    }

    #[test]
    fn test_assign_field_to_group() {
        let conn = init_memory_db().expect("init");
        let bv = draft_version(&conn);
        let other_bv = {
            let record = create_bundle(&conn, "Other Bundle", None, None).expect("create bundle");
            conn.query_row(
                "SELECT id FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC LIMIT 1",
                params![record.id],
                |row| row.get::<_, String>(0),
            )
            .expect("read head version id")
        };

        let field = create_field(&conn, &bv, &sample_field(&bv, "company.name"))
            .expect("create field");
        let shared = create_group(&conn, Some(&bv), "Company", GroupScope::Shared, None)
            .expect("create shared group");
        let other_group = create_group(
            &conn,
            Some(&other_bv),
            "Foreign",
            GroupScope::Shared,
            None,
        )
        .expect("create foreign group");

        assign_field_to_group(&conn, &field.id, Some(&shared.id)).expect("assign to shared group");
        let stored: Option<String> = conn
            .query_row(
                "SELECT group_id FROM fields WHERE id = ?1",
                params![field.id],
                |row| row.get(0),
            )
            .expect("read group id");
        assert_eq!(stored.as_deref(), Some(shared.id.as_str()));

        let err = assign_field_to_group(&conn, &field.id, Some(&other_group.id))
            .expect_err("cross-version assignment must fail");
        assert!(matches!(err, DocForgeError::InvalidInput(_)));

        assign_field_to_group(&conn, &field.id, None).expect("clear group");
        let cleared: Option<String> = conn
            .query_row(
                "SELECT group_id FROM fields WHERE id = ?1",
                params![field.id],
                |row| row.get(0),
            )
            .expect("read cleared group id");
        assert_eq!(cleared, None);
    }

    #[test]
    fn test_group_summary_counts() {
        let conn = init_memory_db().expect("init");
        let bv = draft_version(&conn);

        let _s1 =
            create_group(&conn, Some(&bv), "Company", GroupScope::Shared, None).expect("shared 1");
        let _s2 = create_group(&conn, Some(&bv), "Client", GroupScope::Shared, None)
            .expect("shared 2");
        let _d1 = create_group(
            &conn,
            Some(&bv),
            "Annex",
            GroupScope::DocumentSpecific,
            None,
        )
        .expect("doc-specific 1");

        let _f1 = create_field(&conn, &bv, &sample_field(&bv, "company.name")).expect("field 1");
        let _f2 = create_field(&conn, &bv, &sample_field(&bv, "client.name")).expect("field 2");
        let _f3 = create_field(&conn, &bv, &sample_field(&bv, "annex.note")).expect("field 3");

        let summary = group_summary(&conn, &bv).expect("group summary");
        assert_eq!(summary.shared_count, 2);
        assert_eq!(summary.document_specific_count, 1);
        assert_eq!(summary.total_fields, 3);
    }

    #[test]
    fn test_create_shared_group_without_bundle_version() {
        let conn = init_memory_db().expect("init");
        let group = FieldGroup {
            id: String::new(),
            bundle_version_id: None,
            name: "Global Shared".to_string(),
            description: None,
            scope: GroupScope::Shared,
            sort_order: 0,
        };
        let created = create_field_group(&conn, None, &group).expect("shared global group");
        assert_eq!(created.scope, GroupScope::Shared);
        assert_eq!(created.bundle_version_id, None);
    }

    #[test]
    fn test_create_document_specific_requires_bundle_version() {
        let conn = init_memory_db().expect("init");
        let group = FieldGroup {
            id: String::new(),
            bundle_version_id: None,
            name: "Annex".to_string(),
            description: None,
            scope: GroupScope::DocumentSpecific,
            sort_order: 0,
        };
        let err = create_field_group(&conn, None, &group).expect_err("doc-specific global rejected");
        assert!(matches!(err, DocForgeError::InvalidInput(_)));
    }

    #[test]
    fn test_list_field_groups_filters_by_scope() {
        let conn = init_memory_db().expect("init");
        let bv = draft_version(&conn);

        let shared = FieldGroup {
            id: String::new(),
            bundle_version_id: Some(bv.clone()),
            name: "Company".to_string(),
            description: None,
            scope: GroupScope::Shared,
            sort_order: 0,
        };
        let doc_specific = FieldGroup {
            id: String::new(),
            bundle_version_id: Some(bv.clone()),
            name: "Annex".to_string(),
            description: None,
            scope: GroupScope::DocumentSpecific,
            sort_order: 1,
        };
        create_field_group(&conn, Some(&bv), &shared).expect("create shared");
        create_field_group(&conn, Some(&bv), &doc_specific).expect("create doc-specific");

        let shared_only = list_field_groups(&conn, Some(&bv), Some(GroupScope::Shared))
            .expect("list shared");
        assert_eq!(shared_only.len(), 1);
        assert_eq!(shared_only[0].scope, GroupScope::Shared);

        let doc_only = list_field_groups(&conn, Some(&bv), Some(GroupScope::DocumentSpecific))
            .expect("list doc-specific");
        assert_eq!(doc_only.len(), 1);
        assert_eq!(doc_only[0].scope, GroupScope::DocumentSpecific);

        let all = list_field_groups(&conn, Some(&bv), None).expect("list all");
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_reorder_group_fields() {
        let conn = init_memory_db().expect("init");
        let bv = draft_version(&conn);
        let group = create_group(&conn, Some(&bv), "Company", GroupScope::Shared, None)
            .expect("create group");

        let f1 = create_field(&conn, &bv, &sample_field(&bv, "field.a")).expect("field 1");
        let f2 = create_field(&conn, &bv, &sample_field(&bv, "field.b")).expect("field 2");
        let f3 = create_field(&conn, &bv, &sample_field(&bv, "field.c")).expect("field 3");

        assign_field_to_group(&conn, &f1.id, Some(&group.id)).expect("assign 1");
        assign_field_to_group(&conn, &f2.id, Some(&group.id)).expect("assign 2");
        assign_field_to_group(&conn, &f3.id, Some(&group.id)).expect("assign 3");

        reorder_group_fields(&conn, &group.id, &[f2.id.clone(), f1.id.clone(), f3.id.clone()])
            .expect("reorder");

        let fields = registry::list_fields(&conn, &bv).expect("list fields");
        let grouped: Vec<_> = fields
            .into_iter()
            .filter(|f| f.group_id == Some(group.id.clone()))
            .collect();
        assert_eq!(grouped.len(), 3);
        assert_eq!(grouped[0].field_id, "field.b");
        assert_eq!(grouped[0].position, 0);
        assert_eq!(grouped[1].field_id, "field.a");
        assert_eq!(grouped[1].position, 1);
        assert_eq!(grouped[2].field_id, "field.c");
        assert_eq!(grouped[2].position, 2);
    }
}
