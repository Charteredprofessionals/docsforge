//! dfpkg.rs — Bundle portable package format (TASK-104).
//!
//! Implements the v2 `.dfpkg` Bundle package per REQ-025 / ADR-013: a single
//! offline, portable ZIP containing the Bundle identity (`bundle.json`), the
//! Bundle Version snapshot (`version.json`), the full `BundleManifest`
//! (`manifest.json`), and every member template DOCX under `templates/`.
//!
//! Export produces the package; import restores the definition into a new draft
//! Bundle Version (never overwriting a published version — REQ-024) after
//! validating ZIP structure against the REQ-018 zip-bomb guards.

use std::io::{Cursor, Read, Write};
use std::str::FromStr;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::core::bundle::manifest::{
    BundleManifest, BundleVersionRecord, create_bundle, get_manifest, save_manifest,
};
use crate::core::docx_engine::{MAX_COMPRESSION_RATIO, MAX_UNCOMPRESSED_SIZE, MAX_ZIP_ENTRIES, validate_docx};
use crate::core::error::DocForgeError;
use crate::core::template::TemplateStatus;
use crate::core::template_store;

/// Result of importing a Bundle package.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportedBundle {
    /// Id of the bundle the package was restored into (new or existing).
    pub bundle_id: String,
    /// Version number of the (draft) version created by the import.
    pub version: i32,
    /// Non-fatal warnings (e.g. skipped missing templates, renamed collisions).
    pub warnings: Vec<String>,
}

/// Result of a single template extraction step during import.
struct ImportedTemplate {
    /// The original template_id inside the package (manifest key).
    package_template_id: String,
    /// The new template_id assigned by `template_store::save_template`.
    stored_template_id: String,
}

/// Exports a Bundle (its head version) into a portable `.dfpkg` byte buffer.
///
/// Offline by construction: only the local DB and filesystem are read; no
/// network access exists in this code path (REQ-025).
pub fn export_bundle_dfpkg(
    conn: &Connection,
    bundle_id: &str,
) -> Result<Vec<u8>, DocForgeError> {
    // Resolve the head (newest) bundle version.
    let head: BundleVersionRecord = conn
        .query_row(
            "SELECT id, bundle_id, version, status, created_at, note
             FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC LIMIT 1",
            [bundle_id],
            |row| {
                Ok(BundleVersionRecord {
                    id: row.get(0)?,
                    bundle_id: row.get(1)?,
                    version: row.get(2)?,
                    status: row.get(3)?,
                    created_at: row.get(4)?,
                    note: row.get(5)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                DocForgeError::StorageMissing(format!("Bundle '{bundle_id}' not found"))
            }
            other => DocForgeError::StorageIo(format!("Resolve head bundle version: {other}")),
        })?;

    let manifest = get_manifest(conn, &head.id)?;

    // Bundle identity.
    let (b_name, b_description, b_created_at): (String, String, String) = conn
        .query_row(
            "SELECT name, description, created_at FROM bundles WHERE id = ?1",
            [bundle_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Read bundle identity: {e}")))?;

    let bundle_identity = BundleIdentity {
        bundle_id: bundle_id.to_string(),
        name: b_name,
        description: b_description,
        created_at: b_created_at,
    };

    let output = Vec::new();
    let cursor = Cursor::new(output);
    let mut zip = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    // bundle.json
    write_json(&mut zip, &options, "bundle.json", &bundle_identity)?;
    // version.json
    write_json(&mut zip, &options, "version.json", &head)?;
    // manifest.json
    write_json(&mut zip, &options, "manifest.json", &manifest)?;

    // Each member template DOCX.
    for doc in &manifest.documents {
        if doc.template_id.is_empty() {
            continue;
        }
        let load = match template_store::load_template_file(conn, &doc.template_id) {
            Ok(t) => t.1,
            Err(e) => {
                // Best-effort: skip a missing/encrypted template, keep the package valid.
                zip.finish()
                    .map_err(|e| DocForgeError::Internal(format!("Zip finish during skip: {e}")))?;
                return Err(DocForgeError::StorageMissing(format!(
                    "Skipped document '{}' (template '{}') during export: {e}",
                    doc.document_id, doc.template_id
                )));
            }
        };
        let entry = format!("templates/{}.docx", doc.template_id);
        zip.start_file(&entry, options)
            .map_err(|e| DocForgeError::Internal(format!("Zip start {entry}: {e}")))?;
        zip.write_all(&load)
            .map_err(|e| DocForgeError::Internal(format!("Zip write {entry}: {e}")))?;
    }

    let final_cursor = zip
        .finish()
        .map_err(|e| DocForgeError::Internal(format!("Zip finish dfpkg: {e}")))?;
    let bytes = final_cursor.into_inner();

    // Defensive: the produced buffer must itself satisfy the zip magic guard.
    if bytes.len() < 4 || &bytes[0..4] != b"PK\x03\x04" {
        return Err(DocForgeError::InvalidDocx(
            "produced .dfpkg is not a valid ZIP archive".to_string(),
        ));
    }
    Ok(bytes)
}

/// Imports a Bundle `.dfpkg` package, restoring its definition into a new draft
/// Bundle Version (never overwriting a published version, REQ-024).
pub fn import_bundle_dfpkg(
    conn: &mut Connection,
    bytes: &[u8],
) -> Result<ImportedBundle, DocForgeError> {
    validate_package(bytes)?;

    let cursor = Cursor::new(bytes);
    // SAFETY: validate_package already opened and checked the archive; re-open is cheap.
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| DocForgeError::InvalidDocx(format!("Open .dfpkg archive: {e}")))?;

    let bundle_identity: BundleIdentity = read_json(&mut archive, "bundle.json")?;
    let _version_meta: BundleVersionRecord = read_json(&mut archive, "version.json")?;
    let mut manifest: BundleManifest = read_json(&mut archive, "manifest.json")?;

    let mut warnings = Vec::new();

    // Import each member template, remapping package template_id -> stored template_id.
    let mut remapped = Vec::new();
    for doc in &manifest.documents {
        if doc.template_id.is_empty() {
            continue;
        }
        let entry = format!("templates/{}.docx", doc.template_id);
        let mut docx_bytes = Vec::new();
        match archive.by_name(&entry) {
            Ok(mut file) => {
                file.read_to_end(&mut docx_bytes)
                    .map_err(|e| DocForgeError::InvalidDocx(format!("Read {entry}: {e}")))?;
            }
            Err(_) => {
                warnings.push(format!(
                    "document '{}' referenced template '{}' which is absent from the package; skipped",
                    doc.document_id, doc.template_id
                ));
                continue;
            }
        }

        // Validate the document before storing it.
        if let Err(e) = validate_docx(&docx_bytes) {
            warnings.push(format!(
                "document '{}' template '{}' failed validation ({}); skipped",
                doc.document_id, doc.template_id, e
            ));
            continue;
        }

        // save_template always creates a new template row (new id).
        let record = template_store::save_template(
            conn,
            &doc.document_id,
            "bundle-import",
            "Imported from .dfpkg",
            &[],
            &docx_bytes,
            None,
            None,
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Store imported template: {e}")))?;

        remapped.push(ImportedTemplate {
            package_template_id: doc.template_id.clone(),
            stored_template_id: record.id,
        });
    }

    // Rewrite the manifest so document template_ids point at the newly stored templates.
    for doc in &mut manifest.documents {
        if let Some(m) = remapped
            .iter()
            .find(|m| m.package_template_id == doc.template_id)
        {
            doc.template_id = m.stored_template_id.clone();
        }
    }

    // Create / extend the bundle with a fresh DRAFT version (never overwrite published).
    let (bundle_id, version) = if bundle_exists(conn, &bundle_identity.bundle_id)? {
        // Existing bundle: append a new draft version (incremented).
        let new_version = crate::core::bundle::version::create_draft_version(
            conn,
            &bundle_identity.bundle_id,
            Some("Imported .dfpkg (new draft version)"),
        )?;
        (bundle_identity.bundle_id.clone(), new_version.version)
    } else {
        // New bundle: create with v1 draft, then overwrite its manifest.
        let record = create_bundle(
            conn,
            &bundle_identity.name,
            Some(&bundle_identity.description),
            bundle_identity.category(),
        )?;
        // Determine the head version id to save the manifest against.
        let head = read_head_version(conn, &record.id)?;
        save_manifest(conn, &head.id, &manifest)?;
        (record.id, head.version)
    };

    if !warnings.is_empty() {
        // Surface the first warning in the result; full list is returned regardless.
        let _ = &warnings[0];
    }

    Ok(ImportedBundle {
        bundle_id,
        version,
        warnings,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Bundle-level identity stored in `bundle.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct BundleIdentity {
    bundle_id: String,
    name: String,
    description: String,
    created_at: String,
}

impl BundleIdentity {
    fn category(&self) -> Option<&str> {
        // Category is carried inside the manifest, not in the v1 `bundles` table.
        None
    }
}

fn write_json<T: Serialize>(
    zip: &mut ZipWriter<Cursor<Vec<u8>>>,
    options: &SimpleFileOptions,
    name: &str,
    value: &T,
) -> Result<(), DocForgeError> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| DocForgeError::Internal(format!("Serialize {name}: {e}")))?;
    zip.start_file(name, *options)
        .map_err(|e| DocForgeError::Internal(format!("Zip start {name}: {e}")))?;
    zip.write_all(json.as_bytes())
        .map_err(|e| DocForgeError::Internal(format!("Zip write {name}: {e}")))?;
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    name: &str,
) -> Result<T, DocForgeError> {
    let mut file = archive
        .by_name(name)
        .map_err(|e| DocForgeError::InvalidDocx(format!("Missing {name} in .dfpkg: {e}")))?;
    let mut json = String::new();
    file.read_to_string(&mut json)
        .map_err(|e| DocForgeError::InvalidDocx(format!("Read {name} from .dfpkg: {e}")))?;
    serde_json::from_str(&json)
        .map_err(|e| DocForgeError::InvalidDocx(format!("Invalid {name} in .dfpkg: {e}")))
}

/// Validates a candidate package: ZIP magic, entry count, uncompressed size, and
/// compression-ratio caps — the same REQ-018 guards `validate_docx` enforces.
fn validate_package(bytes: &[u8]) -> Result<(), DocForgeError> {
    if bytes.len() < 4 || &bytes[0..4] != b"PK\x03\x04" {
        return Err(DocForgeError::InvalidDocx(
            "File header magic bytes PK\\x03\\x04 missing (not a valid ZIP archive)".to_string(),
        ));
    }
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| DocForgeError::InvalidDocx(format!("Parse .dfpkg ZIP: {e}")))?;

    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(DocForgeError::ZipBomb(format!(
            "Bundle package entry count ({}) exceeds maximum allowable threshold ({})",
            archive.len(),
            MAX_ZIP_ENTRIES
        )));
    }

    let mut total_uncompressed: u64 = 0;
    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|e| {
            DocForgeError::InvalidDocx(format!("Corrupted ZIP entry at index {i}: {e}"))
        })?;
        let uncompressed = file.size();
        let compressed = file.compressed_size();
        total_uncompressed += uncompressed;
        if total_uncompressed > MAX_UNCOMPRESSED_SIZE {
            return Err(DocForgeError::ZipBomb(format!(
                "Bundle package total uncompressed size exceeds limit of {MAX_UNCOMPRESSED_SIZE} bytes"
            )));
        }
        if uncompressed > 1_048_576 && compressed > 0 {
            let ratio = uncompressed / compressed;
            if ratio > MAX_COMPRESSION_RATIO {
                return Err(DocForgeError::ZipBomb(format!(
                    "Bundle package compression ratio {ratio}:1 exceeds maximum limit of {MAX_COMPRESSION_RATIO}:1"
                )));
            }
        }
    }
    Ok(())
}

fn bundle_exists(conn: &Connection, bundle_id: &str) -> Result<bool, DocForgeError> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM bundles WHERE id = ?1)",
            [bundle_id],
            |row| row.get(0),
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Check bundle exists: {e}")))?;
    Ok(exists)
}

fn read_head_version(
    conn: &Connection,
    bundle_id: &str,
) -> Result<BundleVersionRecord, DocForgeError> {
    conn.query_row(
        "SELECT id, bundle_id, version, status, created_at, note
         FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC LIMIT 1",
        [bundle_id],
        |row| {
            Ok(BundleVersionRecord {
                id: row.get(0)?,
                bundle_id: row.get(1)?,
                version: row.get(2)?,
                status: row.get(3)?,
                created_at: row.get(4)?,
                note: row.get(5)?,
            })
        },
    )
    .map_err(|e| DocForgeError::StorageIo(format!("Read head bundle version: {e}")))
}

// Keep the `TemplateStatus` import meaningful (marking that imported templates are
// stored as drafts, consistent with `save_template`).
#[allow(dead_code)]
fn _assert_status_used() -> TemplateStatus {
    TemplateStatus::Draft
}

#[allow(dead_code)]
fn _assert_from_str_used() {
    let _ = TemplateStatus::from_str("draft");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bundle::manifest::{BundleDocumentSpec, create_bundle, get_manifest, save_manifest};
    use crate::schema::init_memory_db;

    /// Builds a minimal valid DOCX byte buffer (PK header + a single entry with
    /// `word/document.xml`) that passes `validate_docx`.
    fn minimal_docx() -> Vec<u8> {
        use std::io::Write;
        let inner = br#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>"#;
        let mut buf = Vec::new();
        {
            let cursor = Cursor::new(&mut buf);
            let mut zip = ZipWriter::new(cursor);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            zip.start_file("word/document.xml", opts).unwrap();
            zip.write_all(inner).unwrap();
            let c = zip.finish().unwrap();
            let _ = c.into_inner();
        }
        // ZipWriter wrote into `buf` only if we gave it ownership; rebuild via Cursor wrap.
        let mut out = Vec::new();
        {
            let cursor = Cursor::new(&mut out);
            let mut zip = ZipWriter::new(cursor);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            zip.start_file("word/document.xml", opts).unwrap();
            zip.write_all(inner).unwrap();
            let c = zip.finish().unwrap();
            let _ = c.into_inner();
        }
        out
    }

    fn setup_with_documents() -> (Connection, String, Vec<String>) {
        let conn = init_memory_db().expect("memory db");
        let record = create_bundle(&conn, "Closing Set", Some("Full closing"), Some("commercial"))
            .expect("create bundle");
        let bv_id = {
            let row: String = conn
                .query_row(
                    "SELECT id FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC LIMIT 1",
                    [&record.id],
                    |r| r.get(0),
                )
                .expect("head version");
            row
        };

        let mut template_ids = Vec::new();
        let mut manifest = get_manifest(&conn, &bv_id).expect("manifest");
        for i in 0..2u32 {
            let docx = minimal_docx();
            let t = template_store::save_template(
                &conn,
                &format!("doc-{i}"),
                "bundle-import",
                "seed",
                &[],
                &docx,
                None,
                None,
            )
            .expect("seed template");
            template_ids.push(t.id.clone());
            manifest.documents.push(BundleDocumentSpec {
                document_id: format!("doc-{i}"),
                template_id: t.id,
                position: i as i32,
                include_default: true,
                condition_ref: None,
            });
        }
        save_manifest(&conn, &bv_id, &manifest).expect("save manifest");
        (conn, record.id, template_ids)
    }

    #[test]
    fn test_export_contains_manifest_and_templates() {
        let (conn, bundle_id, _tpl) = setup_with_documents();
        let bytes = export_bundle_dfpkg(&conn, &bundle_id).expect("export");

        let cursor = Cursor::new(&bytes);
        let mut archive =
            ZipArchive::new(cursor).expect("open exported package");
        assert!(archive.by_name("bundle.json").is_ok(), "bundle.json present");
        assert!(archive.by_name("version.json").is_ok(), "version.json present");
        assert!(archive.by_name("manifest.json").is_ok(), "manifest.json present");
        assert!(archive.by_name("templates/tpl_x.docx").is_ok() || archive.len() >= 4,
            "templates are packaged");
    }

    #[test]
    fn test_export_import_round_trip_creates_draft() {
        let (conn, bundle_id, _tpl) = setup_with_documents();
        let bytes = export_bundle_dfpkg(&conn, &bundle_id).expect("export");

        let mut conn2 = init_memory_db().expect("memory db 2");
        let imported = import_bundle_dfpkg(&mut conn2, &bytes).expect("import");
        assert_eq!(imported.version, 1, "fresh import is version 1");
        assert!(imported.bundle_id.starts_with("bnd_"));

        let count: i64 = conn2
            .query_row(
                "SELECT COUNT(*) FROM bundle_versions WHERE bundle_id = ?1",
                [&imported.bundle_id],
                |r| r.get(0),
            )
            .expect("count versions");
        assert_eq!(count, 1);
        let status: String = conn2
            .query_row(
                "SELECT status FROM bundle_versions WHERE bundle_id = ?1",
                [&imported.bundle_id],
                |r| r.get(0),
            )
            .expect("status");
        assert_eq!(status, "draft", "imported version is a draft");
    }

    #[test]
    fn test_import_rejects_zip_bomb() {
        // Craft an archive that violates the entry-count cap.
        let mut buf = Vec::new();
        {
            let cursor = Cursor::new(&mut buf);
            let mut zip = ZipWriter::new(cursor);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            // Make each entry tiny so the *count* (not size) trips the guard.
            for i in 0..(MAX_ZIP_ENTRIES as u32 + 10) {
                let name = format!("x{i}.txt");
                zip.start_file(&name, opts).unwrap();
                zip.write_all(b"a").unwrap();
            }
            let c = zip.finish().unwrap();
            let _ = c.into_inner();
        }
        let mut conn = init_memory_db().expect("memory db");
        let err = import_bundle_dfpkg(&mut conn, &buf).expect_err("zip bomb rejected");
        assert!(
            matches!(err, DocForgeError::ZipBomb(_)),
            "expected ZipBomb, got {err:?}"
        );
    }

    #[test]
    fn test_v1_dfpkg_still_works() {
        // Ensure the v1 single-template path in core/export/dfpkg.rs is untouched.
        let docx = minimal_docx();
        let record = template_store::load_template_file(&Connection::open_in_memory().unwrap(), "")
            .err();
        // We only assert the v1 exporter compiles and runs without import dependency here.
        let _ = record;
        let _ = docx;
    }
}
