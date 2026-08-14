//! version.rs — Bundle Version lifecycle (TASK-103, REQ-024).
//!
//! Draft/review/published/archived lifecycle for Bundle Versions, layered on
//! the v5 `bundle_versions` table. Publishing seals a snapshot: from that
//! moment the row is immutable (REQ-024) and the append-only trigger in
//! `migrations.rs` (v5) rejects every UPDATE/DELETE, surfaced here as
//! `DocForgeError::PublishedBundleImmutable`.
//!
//! The trigger guards `WHEN OLD.status = 'published'` with no transition
//! carve-out, so it fires on *any* UPDATE of a published row — including a
//! status change to `archived`. Published versions therefore cannot be edited
//! or archived in place: the only evolution is a *new* version (create → publish
//! supersedes the old head). `archive_version` refuses published rows with
//! `PublishedBundleImmutable` and only ever transitions draft/review rows.
//!
//! `BundleVersionRecord` (metadata only) is shared with `manifest.rs` (TASK-102)
//! so no duplicate record type exists here; the snapshot body is read via
//! `get_manifest`.

use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::core::bundle::manifest::{BundleManifest, BundleVersionRecord};
use crate::core::error::DocForgeError;

const STATUS_DRAFT: &str = "draft";
const STATUS_REVIEW: &str = "review";
const STATUS_PUBLISHED: &str = "published";
const STATUS_ARCHIVED: &str = "archived";

/// Creates the next draft Bundle Version of a bundle.
///
/// The version number is `max(version) + 1` (or 1 when the bundle has no
/// versions yet). The manifest snapshot is copied from the current head
/// version so the new draft continues the definition; a bundle without any
/// prior version starts from an empty manifest populated with the bundle
/// name. Unparseable head manifests (e.g. migrated `'{}'` rows) fall back to
/// that same empty manifest. The new row is inserted in `draft` status.
pub fn create_draft_version(
    conn: &Connection,
    bundle_id: &str,
    note: Option<&str>,
) -> Result<BundleVersionRecord, DocForgeError> {
    let note = note.map(str::trim).filter(|s| !s.is_empty());

    let tx = conn
        .unchecked_transaction()
        .map_err(|e| map_rusqlite_error(e, "Begin draft version transaction"))?;

    let bundle_name: String = tx
        .query_row(
            "SELECT name FROM bundles WHERE id = ?1",
            params![bundle_id],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DocForgeError::StorageMissing(format!("Bundle '{bundle_id}' not found"))
            }
            other => map_rusqlite_error(other, "Check bundle exists"),
        })?;

    let next_version: i32 = tx
        .query_row(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM bundle_versions WHERE bundle_id = ?1",
            params![bundle_id],
            |row| row.get(0),
        )
        .map_err(|e| map_rusqlite_error(e, "Compute next bundle version"))?;

    let head_manifest_json: Option<String> = tx
        .query_row(
            "SELECT manifest_json FROM bundle_versions WHERE bundle_id = ?1
             ORDER BY version DESC LIMIT 1",
            params![bundle_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| map_rusqlite_error(e, "Read head bundle version manifest"))?;

    let fallback_manifest =
        || BundleManifest { name: bundle_name.clone(), ..BundleManifest::default() };
    let manifest = match head_manifest_json.as_deref() {
        Some(json) => serde_json::from_str::<BundleManifest>(json)
            .unwrap_or_else(|_| fallback_manifest()),
        None => fallback_manifest(),
    };
    let manifest_json = serde_json::to_string(&manifest)
        .map_err(|e| DocForgeError::Internal(format!("Serialize bundle manifest: {e}")))?;

    let version_id = format!("bv_{}", Uuid::new_v4());

    tx.execute(
        "INSERT INTO bundle_versions (id, bundle_id, version, status, manifest_json, note)
         VALUES (?1, ?2, ?3, 'draft', ?4, ?5)",
        params![version_id, bundle_id, next_version, manifest_json, note],
    )
    .map_err(|e| map_rusqlite_error(e, "Insert bundle version"))?;

    tx.commit()
        .map_err(|e| map_rusqlite_error(e, "Commit bundle version creation"))?;

    let created_at: String = conn
        .query_row(
            "SELECT created_at FROM bundle_versions WHERE id = ?1",
            params![version_id],
            |row| row.get(0),
        )
        .map_err(|e| map_rusqlite_error(e, "Read created bundle version"))?;

    Ok(BundleVersionRecord {
        id: version_id,
        bundle_id: bundle_id.to_string(),
        version: next_version,
        status: STATUS_DRAFT.to_string(),
        created_at,
        note: note.map(String::from),
    })
}

/// Lists all Bundle Versions of a bundle, newest version first.
pub fn list_versions(
    conn: &Connection,
    bundle_id: &str,
) -> Result<Vec<BundleVersionRecord>, DocForgeError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, bundle_id, version, status, created_at, note
             FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC",
        )
        .map_err(|e| map_rusqlite_error(e, "Prepare bundle version list query"))?;

    let rows = stmt
        .query_map(params![bundle_id], map_version_row)
        .map_err(|e| map_rusqlite_error(e, "Query bundle version list"))?;

    let mut versions = Vec::new();
    for row in rows {
        versions.push(row.map_err(|e| map_rusqlite_error(e, "Map bundle version row"))?);
    }
    Ok(versions)
}

/// Loads one Bundle Version by id, with `StorageMissing` when it does not exist.
pub fn get_version(
    conn: &Connection,
    bundle_version_id: &str,
) -> Result<BundleVersionRecord, DocForgeError> {
    conn.query_row(
        "SELECT id, bundle_id, version, status, created_at, note
         FROM bundle_versions WHERE id = ?1",
        params![bundle_version_id],
        map_version_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => DocForgeError::StorageMissing(format!(
            "Bundle version '{bundle_version_id}' not found"
        )),
        other => map_rusqlite_error(other, "Query bundle version"),
    })
}

/// Loads the current head (highest version) Bundle Version of a bundle.
///
/// Generation runs and matter creation resolve the exact `bundle_version_id`
/// they must reference from here (REQ-024, REQ-033).
pub fn get_head_version(
    conn: &Connection,
    bundle_id: &str,
) -> Result<BundleVersionRecord, DocForgeError> {
    conn.query_row(
        "SELECT id, bundle_id, version, status, created_at, note
         FROM bundle_versions WHERE bundle_id = ?1
         ORDER BY version DESC LIMIT 1",
        params![bundle_id],
        map_version_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => DocForgeError::StorageMissing(format!(
            "Bundle '{bundle_id}' has no versions; create a draft version first"
        )),
        other => map_rusqlite_error(other, "Query head bundle version"),
    })
}

/// Seals a Bundle Version as `published` (REQ-024).
///
/// Only `draft`/`review` versions can be published. Publishing an
/// already-published version is rejected because the row is immutable; the
/// append-only trigger remains the SQL-level backstop, surfaced as
/// `PublishedBundleImmutable`.
pub fn publish_version(
    conn: &Connection,
    bundle_version_id: &str,
) -> Result<(), DocForgeError> {
    match load_status(conn, bundle_version_id)?.as_str() {
        STATUS_PUBLISHED => {
            return Err(DocForgeError::PublishedBundleImmutable(format!(
                "Bundle version '{bundle_version_id}' is already published"
            )));
        }
        STATUS_ARCHIVED => {
            return Err(DocForgeError::InvalidInput(format!(
                "Bundle version '{bundle_version_id}' is archived and cannot be published"
            )));
        }
        _ => {}
    }

    update_status(conn, bundle_version_id, STATUS_PUBLISHED, "Publish bundle version")
}

/// Moves a `draft` Bundle Version into `review` (lifecycle step before publish).
pub fn review_version(
    conn: &Connection,
    bundle_version_id: &str,
) -> Result<(), DocForgeError> {
    match load_status(conn, bundle_version_id)?.as_str() {
        STATUS_PUBLISHED => {
            return Err(DocForgeError::PublishedBundleImmutable(format!(
                "Bundle version '{bundle_version_id}' is published and cannot be reviewed"
            )));
        }
        STATUS_ARCHIVED => {
            return Err(DocForgeError::InvalidInput(format!(
                "Bundle version '{bundle_version_id}' is archived and cannot be reviewed"
            )));
        }
        STATUS_REVIEW => {
            return Err(DocForgeError::InvalidInput(format!(
                "Bundle version '{bundle_version_id}' is already in review"
            )));
        }
        _ => {}
    }

    update_status(conn, bundle_version_id, STATUS_REVIEW, "Move bundle version to review")
}

/// Archives a Bundle Version (lifecycle end state).
///
/// Published versions (REQ-024) are immutable in place: the append-only
/// trigger rejects *every* UPDATE of a published row, including a status
/// change to `archived`, so this function refuses them with
/// `PublishedBundleImmutable` — the supported retirement path for a published
/// version is to publish a newer one, which supersedes it as the head.
/// Draft/review rows transition to `archived` normally.
pub fn archive_version(
    conn: &Connection,
    bundle_version_id: &str,
) -> Result<(), DocForgeError> {
    if load_status(conn, bundle_version_id)?.as_str() == STATUS_PUBLISHED {
        return Err(DocForgeError::PublishedBundleImmutable(format!(
            "Bundle version '{bundle_version_id}' is published and cannot be archived in place; publish a new version to supersede it"
        )));
    }

    update_status(conn, bundle_version_id, STATUS_ARCHIVED, "Archive bundle version")
}

/// Reads the current `status` of a Bundle Version, mapping a missing row to
/// `StorageMissing`.
fn load_status(conn: &Connection, bundle_version_id: &str) -> Result<String, DocForgeError> {
    conn.query_row(
        "SELECT status FROM bundle_versions WHERE id = ?1",
        params![bundle_version_id],
        |row| row.get(0),
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => DocForgeError::StorageMissing(format!(
            "Bundle version '{bundle_version_id}' not found"
        )),
        other => map_rusqlite_error(other, "Query bundle version status"),
    })
}

/// Sets a new status on a Bundle Version, treating a zero-row UPDATE as a
/// missing version.
fn update_status(
    conn: &Connection,
    bundle_version_id: &str,
    status: &str,
    context: &str,
) -> Result<(), DocForgeError> {
    let affected = conn
        .execute(
            "UPDATE bundle_versions SET status = ?1 WHERE id = ?2",
            params![status, bundle_version_id],
        )
        .map_err(|e| map_rusqlite_error(e, context))?;

    if affected == 0 {
        return Err(DocForgeError::StorageMissing(format!(
            "Bundle version '{bundle_version_id}' not found"
        )));
    }
    Ok(())
}

/// Row mapper for the `BundleVersionRecord` metadata columns.
fn map_version_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BundleVersionRecord> {
    Ok(BundleVersionRecord {
        id: row.get(0)?,
        bundle_id: row.get(1)?,
        version: row.get(2)?,
        status: row.get(3)?,
        created_at: row.get(4)?,
        note: row.get(5)?,
    })
}

/// Maps a rusqlite error to a precise `DocForgeError`, recognizing the
/// published-version immutability trigger (REQ-024).
fn map_rusqlite_error(e: rusqlite::Error, context: &str) -> DocForgeError {
    let message = e.to_string();
    if message.contains("published bundle_versions is immutable") {
        DocForgeError::PublishedBundleImmutable(context.to_string())
    } else {
        DocForgeError::StorageIo(format!("{context}: {message}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bundle::manifest::{
        OutputFormat, create_bundle, get_manifest, save_manifest,
    };
    use crate::schema::init_memory_db;

    fn test_conn() -> Connection {
        init_memory_db().expect("init memory db")
    }

    fn head_version_id(conn: &Connection, bundle_id: &str) -> String {
        conn.query_row(
            "SELECT id FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC LIMIT 1",
            params![bundle_id],
            |row| row.get(0),
        )
        .expect("read head bundle version id")
    }

    fn insert_bare_bundle(conn: &Connection, id: &str, name: &str) {
        conn.execute(
            "INSERT INTO bundles (id, name, description) VALUES (?1, ?2, '')",
            params![id, name],
        )
        .expect("insert bare bundle");
    }

    #[test]
    fn test_create_draft_version_1_when_none() {
        let conn = test_conn();
        insert_bare_bundle(&conn, "b-bare", "Bare Bundle");
        let record = create_draft_version(&conn, "b-bare", Some("first draft"))
            .expect("create first draft version");
        assert_eq!(record.version, 1);
        assert_eq!(record.status, "draft");
        assert_eq!(record.bundle_id, "b-bare");
        assert_eq!(record.note.as_deref(), Some("first draft"));
        assert!(record.id.starts_with("bv_"), "version id prefix");
    }

    #[test]
    fn test_create_draft_version_increments() {
        let conn = test_conn();
        let bundle = create_bundle(&conn, "Closing Set", None, None).expect("create bundle");
        let v2 = create_draft_version(&conn, &bundle.id, None).expect("create draft v2");
        assert_eq!(v2.version, 2, "create_bundle already produced v1; next draft is v2");
        let v3 = create_draft_version(&conn, &bundle.id, Some("third")).expect("create draft v3");
        assert_eq!(v3.version, 3);
    }

    #[test]
    fn test_new_draft_copies_head_manifest() {
        let conn = test_conn();
        let bundle = create_bundle(&conn, "Closing Set", None, None).expect("create bundle");
        let v1_id = head_version_id(&conn, &bundle.id);
        let mut manifest = get_manifest(&conn, &v1_id).expect("read v1 manifest");
        manifest.output_config.output_format = OutputFormat::DocxAndPdf;
        save_manifest(&conn, &v1_id, &manifest).expect("edit v1 manifest");

        let v2 = create_draft_version(&conn, &bundle.id, None).expect("create draft v2");
        let v2_manifest = get_manifest(&conn, &v2.id).expect("read v2 manifest");
        assert_eq!(
            v2_manifest, manifest,
            "new draft must copy the head version definition"
        );
    }

    #[test]
    fn test_publish_version_then_edit_rejected() {
        let conn = test_conn();
        let bundle = create_bundle(&conn, "Sealed Set", None, None).expect("create bundle");
        let bv_id = head_version_id(&conn, &bundle.id);
        publish_version(&conn, &bv_id).expect("publish v1");

        let err = save_manifest(&conn, &bv_id, &BundleManifest::default())
            .expect_err("published version must reject manifest edits");
        assert!(
            matches!(err, DocForgeError::PublishedBundleImmutable(_)),
            "expected PublishedBundleImmutable, got {err:?}"
        );

        let republish = publish_version(&conn, &bv_id).expect_err("second publish must be rejected");
        assert!(
            matches!(republish, DocForgeError::PublishedBundleImmutable(_)),
            "expected PublishedBundleImmutable, got {republish:?}"
        );
    }

    #[test]
    fn test_new_version_after_publish() {
        let conn = test_conn();
        let bundle = create_bundle(&conn, "Growth Set", None, None).expect("create bundle");
        let v1 = get_version(&conn, &head_version_id(&conn, &bundle.id)).expect("read v1");
        publish_version(&conn, &v1.id).expect("publish v1");

        let v2 = create_draft_version(&conn, &bundle.id, Some("v2 changes")).expect("create v2");
        assert_eq!(v2.version, 2, "after v1 published, next draft is v2");

        let v1_after = get_version(&conn, &v1.id).expect("re-read v1");
        assert_eq!(v1_after.status, "published", "published v1 must remain unchanged");
        assert_eq!(v2.status, "draft", "new version starts as draft");
        assert_ne!(v2.id, v1.id, "each version has its own row");
    }

    #[test]
    fn test_versions_listed_desc() {
        let conn = test_conn();
        let bundle = create_bundle(&conn, "Sorted Set", None, None).expect("create bundle");
        create_draft_version(&conn, &bundle.id, None).expect("create v2");
        create_draft_version(&conn, &bundle.id, None).expect("create v3");

        let versions = list_versions(&conn, &bundle.id).expect("list versions");
        let numbers: Vec<i32> = versions.iter().map(|v| v.version).collect();
        assert_eq!(numbers, vec![3, 2, 1], "newest version must come first");
    }

    #[test]
    fn test_archive_behavior() {
        let conn = test_conn();
        let bundle = create_bundle(&conn, "Archive Set", None, None).expect("create bundle");
        let draft = get_version(&conn, &head_version_id(&conn, &bundle.id)).expect("read draft v1");

        archive_version(&conn, &draft.id).expect("archive draft");
        let archived = get_version(&conn, &draft.id).expect("re-read archived");
        assert_eq!(archived.status, "archived", "draft rows may be archived");

        let v2 = create_draft_version(&conn, &bundle.id, None).expect("create v2");
        publish_version(&conn, &v2.id).expect("publish v2");
        let err = archive_version(&conn, &v2.id)
            .expect_err("published rows must not be archived in place");
        assert!(
            matches!(err, DocForgeError::PublishedBundleImmutable(_)),
            "expected PublishedBundleImmutable, got {err:?}"
        );
        let still_published = get_version(&conn, &v2.id).expect("re-read published");
        assert_eq!(still_published.status, "published", "published row unchanged");
    }

    #[test]
    fn test_review_version_draft_to_review() {
        let conn = test_conn();
        let bundle = create_bundle(&conn, "Review Set", None, None).expect("create bundle");
        let bv_id = head_version_id(&conn, &bundle.id);

        review_version(&conn, &bv_id).expect("send to review");
        let record = get_version(&conn, &bv_id).expect("read reviewed version");
        assert_eq!(record.status, "review");

        publish_version(&conn, &bv_id).expect("publish from review");
        let published = get_version(&conn, &bv_id).expect("read published version");
        assert_eq!(published.status, "published");
    }

    #[test]
    fn test_get_head_version_resolves_max_version() {
        let conn = test_conn();
        let bundle = create_bundle(&conn, "Head Set", None, None).expect("create bundle");
        let v2 = create_draft_version(&conn, &bundle.id, None).expect("create v2");

        let head = get_head_version(&conn, &bundle.id).expect("resolve head version");
        assert_eq!(head.id, v2.id);
        assert_eq!(head.version, 2);

        let err = get_head_version(&conn, "b-missing").expect_err("missing bundle has no head");
        assert!(matches!(err, DocForgeError::StorageMissing(_)));
    }
}
