//! manifest.rs — BundleManifest model and bundle persistence (TASK-102).
//!
//! A Bundle is a reusable generation definition. Its identity lives in the v1
//! `bundles` table; the full definition (documents, canonical field schema,
//! mappings, rules, output configuration) is snapshotted into
//! `bundle_versions.manifest_json`. Every bundle is created with a draft
//! Bundle Version v1 so it always starts with a version (architecture §6.1).
//!
//! Published Bundle Versions are immutable (REQ-024): the `bundle_versions`
//! append-only trigger rejects writes, surfaced here as
//! `DocForgeError::PublishedBundleImmutable`.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::error::DocForgeError;

/// The JSON payload stored in `bundle_versions.manifest_json`.
///
/// A complete snapshot of a Bundle Version's definition. Optional sections are
/// `#[serde(default)]` so manifests written by earlier or migrated snapshots
/// (`'{}'` from the v5 promotion) still parse without schema rewrite.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BundleManifest {
    /// Display name of the bundle (duplicated from `bundles.name` for snapshot self-containment).
    pub name: String,
    /// Optional human description of the bundle.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional business category used by the Bundles screen.
    #[serde(default)]
    pub category: Option<String>,
    /// Output naming/format configuration (single source of truth; behaviors land in TASK-105).
    pub output_config: OutputConfig,
    /// Ordered document membership of this bundle version.
    #[serde(default)]
    pub documents: Vec<BundleDocumentSpec>,
    /// Canonical field schema (fields/groups/mappings/rules are populated by the F1 tasks).
    #[serde(default)]
    pub schema: BundleSchema,
}

impl Default for BundleManifest {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: None,
            category: None,
            output_config: OutputConfig::default(),
            documents: Vec::new(),
            schema: BundleSchema::default(),
        }
    }
}

/// One ordered document inside a bundle version.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleDocumentSpec {
    /// Stable logical document identity (e.g. "agreement").
    pub document_id: String,
    /// Id of the bound template, or empty when rule-driven / not yet bound.
    pub template_id: String,
    /// Display/order position within the bundle.
    #[serde(default)]
    pub position: i32,
    /// Whether the document is included by default (rules may override).
    #[serde(default = "default_include_document")]
    pub include_default: bool,
    /// Optional reference to a rules row deciding conditional inclusion.
    #[serde(default)]
    pub condition_ref: Option<String>,
}

fn default_include_document() -> bool {
    true
}

/// Output format produced by a generation run for a bundle version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    /// Produce DOCX only.
    Docx,
    /// Produce PDF only.
    Pdf,
    /// Produce both DOCX and PDF.
    DocxAndPdf,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Docx
    }
}

/// Bundle output configuration snapshot.
///
/// Kept here (not in `output_config.rs`) as the single source of truth for the
/// persisted shape; TASK-105 adds naming/folder-policy behaviors on top of it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutputConfig {
    /// Optional filename pattern (e.g. "{matter_name}_{document_id}"). None = engine default.
    #[serde(default)]
    pub filename_template: Option<String>,
    /// Which output format(s) a run produces.
    #[serde(default)]
    pub output_format: OutputFormat,
    /// Optional output folder; None = app-data default output tree.
    #[serde(default)]
    pub output_folder: Option<String>,
    /// Whether a run additionally packages outputs into a single zip.
    #[serde(default)]
    pub zip_output: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            filename_template: None,
            output_format: OutputFormat::default(),
            output_folder: None,
            zip_output: false,
        }
    }
}

/// Canonical field schema placeholder within the manifest.
///
/// The F1 tasks (field schema, groups, mappings, rules) populate these vectors.
/// They are kept as raw `serde_json::Value` with serde defaults so future
/// schema migration is non-breaking for previously stored manifests.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BundleSchema {
    /// Canonical field definitions.
    #[serde(default)]
    pub fields: Vec<serde_json::Value>,
    /// Field group definitions (shared vs document-specific).
    #[serde(default)]
    pub groups: Vec<serde_json::Value>,
    /// Placeholder-to-field mappings.
    #[serde(default)]
    pub mappings: Vec<serde_json::Value>,
    /// Conditional-document rule definitions.
    #[serde(default)]
    pub rules: Vec<serde_json::Value>,
}

/// Identity record of a bundle (mirrors the v1 `bundles` row plus head category).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: Option<String>,
    pub created_at: String,
}

/// Bundle summary with head-version metadata for the Bundles screen.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: Option<String>,
    pub head_version: Option<i32>,
    pub status: Option<String>,
}

/// One `bundle_versions` row.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleVersionRecord {
    pub id: String,
    pub bundle_id: String,
    pub version: i32,
    pub status: String,
    pub created_at: String,
    pub note: Option<String>,
}

/// Full bundle detail: identity plus its complete version history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleDetail {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: Option<String>,
    pub created_at: String,
    pub versions: Vec<BundleVersionRecord>,
}

/// Creates a bundle with an initial draft Bundle Version v1 carrying an empty
/// (name/category populated) manifest, so every bundle starts with a version.
pub fn create_bundle(
    conn: &Connection,
    name: &str,
    description: Option<&str>,
    category: Option<&str>,
) -> Result<BundleRecord, DocForgeError> {
    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return Err(DocForgeError::InvalidInput(
            "bundle name must not be empty".to_string(),
        ));
    }
    let description = description.map(str::trim).filter(|s| !s.is_empty());
    let category = category.map(str::trim).filter(|s| !s.is_empty());

    let bundle_id = format!("bnd_{}", Uuid::new_v4());
    let version_id = format!("bv_{}", Uuid::new_v4());

    let manifest = BundleManifest {
        name: trimmed_name.to_string(),
        description: description.map(String::from),
        category: category.clone().map(String::from),
        output_config: OutputConfig::default(),
        documents: Vec::new(),
        schema: BundleSchema::default(),
    };
    let manifest_json = serde_json::to_string(&manifest).map_err(|e| {
        DocForgeError::Internal(format!("Serialize initial bundle manifest: {e}"))
    })?;

    let tx = conn
        .unchecked_transaction()
        .map_err(|e| map_rusqlite_error(e, "Begin bundle creation transaction"))?;

    tx.execute(
        "INSERT INTO bundles (id, name, description) VALUES (?1, ?2, ?3)",
        params![bundle_id, trimmed_name, description.unwrap_or("")],
    )
    .map_err(|e| map_rusqlite_error(e, "Insert bundle"))?;

    tx.execute(
        "INSERT INTO bundle_versions (id, bundle_id, version, status, manifest_json, note)
         VALUES (?1, ?2, 1, 'draft', ?3, ?4)",
        params![version_id, bundle_id, manifest_json, "Initial draft"],
    )
    .map_err(|e| map_rusqlite_error(e, "Insert initial bundle version"))?;

    tx.commit()
        .map_err(|e| map_rusqlite_error(e, "Commit bundle creation"))?;

    let created_at: String = conn
        .query_row(
            "SELECT created_at FROM bundles WHERE id = ?1",
            params![bundle_id],
            |row| row.get(0),
        )
        .map_err(|e| map_rusqlite_error(e, "Read created bundle"))?;

    Ok(BundleRecord {
        id: bundle_id,
        name: trimmed_name.to_string(),
        description: description.unwrap_or("").to_string(),
        category: category.map(String::from),
        created_at,
    })
}

/// Lists bundles with their head version (highest `bundle_versions.version`).
pub fn list_bundles(conn: &Connection) -> Result<Vec<BundleSummary>, DocForgeError> {
    let mut stmt = conn
        .prepare(
            "SELECT b.id, b.name, b.description, b.created_at,
                    bv.version, bv.status, bv.manifest_json
             FROM bundles b
             LEFT JOIN bundle_versions bv
               ON bv.bundle_id = b.id
              AND bv.version = (
                    SELECT MAX(version) FROM bundle_versions WHERE bundle_id = b.id
                  )
             ORDER BY b.created_at DESC",
        )
        .map_err(|e| map_rusqlite_error(e, "Prepare bundle list query"))?;

    let rows = stmt
        .query_map([], |row| {
            let manifest_json: Option<String> = row.get(6)?;
            Ok(BundleSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                category: manifest_category(manifest_json.as_deref()),
                head_version: row.get(4)?,
                status: row.get(5)?,
            })
        })
        .map_err(|e| map_rusqlite_error(e, "Query bundle list"))?;

    let mut summaries = Vec::new();
    for row in rows {
        summaries.push(row.map_err(|e| map_rusqlite_error(e, "Map bundle list row"))?);
    }
    Ok(summaries)
}

/// Loads a bundle identity plus its full version history (newest first).
pub fn get_bundle(conn: &Connection, bundle_id: &str) -> Result<BundleDetail, DocForgeError> {
    let base = conn
        .query_row(
            "SELECT id, name, description, created_at FROM bundles WHERE id = ?1",
            params![bundle_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DocForgeError::StorageMissing(format!(
                "Bundle '{bundle_id}' not found"
            )),
            other => map_rusqlite_error(other, "Query bundle"),
        })?;

    let mut stmt = conn
        .prepare(
            "SELECT id, bundle_id, version, status, created_at, note
             FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC",
        )
        .map_err(|e| map_rusqlite_error(e, "Prepare bundle versions query"))?;
    let rows = stmt
        .query_map(params![bundle_id], |row| {
            Ok(BundleVersionRecord {
                id: row.get(0)?,
                bundle_id: row.get(1)?,
                version: row.get(2)?,
                status: row.get(3)?,
                created_at: row.get(4)?,
                note: row.get(5)?,
            })
        })
        .map_err(|e| map_rusqlite_error(e, "Query bundle versions"))?;

    let mut versions = Vec::new();
    for row in rows {
        versions.push(row.map_err(|e| map_rusqlite_error(e, "Map bundle version row"))?);
    }

    let category = versions
        .iter()
        .max_by_key(|v| v.version)
        .and_then(|v| {
            conn.query_row(
                "SELECT manifest_json FROM bundle_versions WHERE id = ?1",
                params![v.id],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .as_deref()
            .and_then(parse_manifest_category)
        });

    Ok(BundleDetail {
        id: base.0,
        name: base.1,
        description: base.2,
        category,
        created_at: base.3,
        versions,
    })
}

/// Deletes a bundle and its versions. Child rows (`bundle_documents`, `fields`,
/// etc.) cascade from `bundle_versions`. Published versions and bundles still
/// referenced by matters/generation runs are refused by triggers/FKs.
pub fn delete_bundle(conn: &Connection, bundle_id: &str) -> Result<(), DocForgeError> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM bundles WHERE id = ?1)",
            params![bundle_id],
            |row| row.get(0),
        )
        .map_err(|e| map_rusqlite_error(e, "Check bundle exists"))?;
    if !exists {
        return Err(DocForgeError::StorageMissing(format!(
            "Bundle '{bundle_id}' not found"
        )));
    }

    conn.execute(
        "DELETE FROM bundle_versions WHERE bundle_id = ?1",
        params![bundle_id],
    )
    .map_err(|e| map_rusqlite_error(e, "Delete bundle versions"))?;

    conn.execute("DELETE FROM bundles WHERE id = ?1", params![bundle_id])
        .map_err(|e| map_rusqlite_error(e, "Delete bundle"))?;

    Ok(())
}

/// Reads and parses the manifest snapshot of a bundle version.
pub fn get_manifest(
    conn: &Connection,
    bundle_version_id: &str,
) -> Result<BundleManifest, DocForgeError> {
    let manifest_json: String = conn
        .query_row(
            "SELECT manifest_json FROM bundle_versions WHERE id = ?1",
            params![bundle_version_id],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DocForgeError::StorageMissing(format!(
                "Bundle version '{bundle_version_id}' not found"
            )),
            other => map_rusqlite_error(other, "Query bundle version manifest"),
        })?;

    serde_json::from_str(&manifest_json).map_err(|e| {
        DocForgeError::Internal(format!(
            "Manifest of bundle version '{bundle_version_id}' is not valid JSON: {e}"
        ))
    })
}

/// Persists a manifest snapshot. Only draft versions are writable: published
/// versions are immutable (REQ-024) and their UPDATE is rejected by the
/// append-only trigger, surfaced as `PublishedBundleImmutable`.
pub fn save_manifest(
    conn: &Connection,
    bundle_version_id: &str,
    manifest: &BundleManifest,
) -> Result<(), DocForgeError> {
    let manifest_json = serde_json::to_string(manifest).map_err(|e| {
        DocForgeError::Internal(format!("Serialize bundle manifest: {e}"))
    })?;

    let affected = conn
        .execute(
            "UPDATE bundle_versions SET manifest_json = ?1 WHERE id = ?2",
            params![manifest_json, bundle_version_id],
        )
        .map_err(|e| map_rusqlite_error(e, "Update bundle version manifest"))?;

    if affected == 0 {
        return Err(DocForgeError::StorageMissing(format!(
            "Bundle version '{bundle_version_id}' not found"
        )));
    }
    Ok(())
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

/// Tolerantly extracts `category` from a manifest JSON string.
///
/// Migrated v1 bundles carry `manifest_json = '{}'`; any unparseable or
/// category-less manifest simply yields `None` so listing never breaks.
fn parse_manifest_category(manifest_json: &str) -> Option<String> {
    serde_json::from_str::<BundleManifest>(manifest_json)
        .ok()
        .and_then(|m| m.category)
}

/// `Option<String>` column helper for list rows.
fn manifest_category(manifest_json: Option<&str>) -> Option<String> {
    manifest_json.and_then(parse_manifest_category)
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn test_create_bundle_creates_draft_version_v1() {
        let conn = test_conn();
        let record = create_bundle(&conn, "Closing Set", Some("Full closing"), Some("commercial"))
            .expect("create bundle");
        assert!(record.id.starts_with("bnd_"), "bundle id prefix");
        assert_eq!(record.name, "Closing Set");
        assert_eq!(record.category.as_deref(), Some("commercial"));

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM bundle_versions WHERE bundle_id = ?1",
                params![record.id],
                |row| row.get(0),
            )
            .expect("count bundle versions");
        assert_eq!(count, 1, "exactly one version row");

        let (version, status): (i32, String) = conn
            .query_row(
                "SELECT version, status FROM bundle_versions WHERE bundle_id = ?1",
                params![record.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read bundle version row");
        assert_eq!(version, 1);
        assert_eq!(status, "draft");
    }

    #[test]
    fn test_manifest_round_trip() {
        let conn = test_conn();
        let record = create_bundle(&conn, "Closing Set", None, None).expect("create bundle");
        let bv_id = head_version_id(&conn, &record.id);

        let mut manifest = get_manifest(&conn, &bv_id).expect("read initial manifest");
        manifest.documents = vec![BundleDocumentSpec {
            document_id: "doc-agreement".to_string(),
            template_id: "tpl-1".to_string(),
            position: 0,
            include_default: true,
            condition_ref: None,
        }];
        manifest.output_config.output_format = OutputFormat::DocxAndPdf;
        manifest.output_config.zip_output = true;
        manifest.output_config.filename_template =
            Some("{matter_name}_{document_id}".to_string());
        save_manifest(&conn, &bv_id, &manifest).expect("save manifest");

        let loaded = get_manifest(&conn, &bv_id).expect("read saved manifest");
        assert_eq!(loaded, manifest, "manifest must round-trip losslessly");
        assert_eq!(loaded.documents.len(), 1);
        assert_eq!(loaded.documents[0].document_id, "doc-agreement");
        assert_eq!(loaded.output_config.output_format, OutputFormat::DocxAndPdf);
        assert!(loaded.output_config.zip_output);
    }

    #[test]
    fn test_manifest_serializes_empty_by_default() {
        let conn = test_conn();
        let record = create_bundle(&conn, "Empty Set", None, None).expect("create bundle");
        let bv_id = head_version_id(&conn, &record.id);

        let manifest = get_manifest(&conn, &bv_id).expect("read fresh manifest");
        assert!(manifest.documents.is_empty());
        assert!(manifest.schema.fields.is_empty(), "empty schema fields");
        assert!(manifest.schema.groups.is_empty(), "empty schema groups");
        assert!(manifest.schema.mappings.is_empty(), "empty schema mappings");
        assert!(manifest.schema.rules.is_empty(), "empty schema rules");
        assert_eq!(
            manifest.output_config.output_format,
            OutputFormat::Docx,
            "default output format is docx"
        );
    }

    #[test]
    fn test_save_manifest_on_published_returns_error() {
        let conn = test_conn();
        let record = create_bundle(&conn, "Sealed Set", None, None).expect("create bundle");
        let bv_id = head_version_id(&conn, &record.id);

        conn.execute(
            "UPDATE bundle_versions SET status = 'published' WHERE id = ?1",
            params![bv_id],
        )
        .expect("publish draft version");

        let err = save_manifest(&conn, &bv_id, &BundleManifest::default())
            .expect_err("published bundle versions are immutable");
        assert!(
            matches!(err, DocForgeError::PublishedBundleImmutable(_)),
            "expected PublishedBundleImmutable, got {err:?}"
        );
    }

    #[test]
    fn test_create_bundle_rejects_empty_name() {
        let conn = test_conn();
        let err = create_bundle(&conn, "   ", None, None)
            .expect_err("empty name must be rejected");
        assert!(matches!(err, DocForgeError::InvalidInput(_)));
    }

    #[test]
    fn test_delete_bundle_removes_bundle_and_versions() {
        let conn = test_conn();
        let record = create_bundle(&conn, "Temp Set", None, None).expect("create bundle");
        delete_bundle(&conn, &record.id).expect("delete bundle");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM bundles WHERE id = ?1",
                params![record.id],
                |row| row.get(0),
            )
            .expect("count bundles");
        assert_eq!(count, 0);
        let version_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM bundle_versions WHERE bundle_id = ?1",
                params![record.id],
                |row| row.get(0),
            )
            .expect("count versions");
        assert_eq!(version_count, 0, "versions must be removed with the bundle");
    }
}
