//! export/dfpkg.rs — `.dfpkg` template archive package exporter/importer.
//!
//! Bundles DOCX template binary, fields definition, metadata, and versioning info into a single portable zip archive.

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::core::docx_engine::TemplateFieldSpec;
use crate::core::error::DocForgeError;
use crate::core::template::TemplateRecord;

/// Exports a template, its fields, and versioning metadata into a portable `.dfpkg` bundle.
pub fn export_dfpkg(
    record: &TemplateRecord,
    docx_bytes: &[u8],
) -> Result<Vec<u8>, DocForgeError> {
    let output = Vec::new();
    let cursor = Cursor::new(output);
    let mut zip_out = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    zip_out
        .start_file("template.docx", options)
        .map_err(|e| DocForgeError::Internal(format!("Zip start template.docx: {e}")))?;
    zip_out
        .write_all(docx_bytes)
        .map_err(|e| DocForgeError::Internal(format!("Zip write template.docx: {e}")))?;

    let meta_json = serde_json::to_string_pretty(record)
        .map_err(|e| DocForgeError::Internal(format!("Serialize record: {e}")))?;

    zip_out
        .start_file("manifest.json", options)
        .map_err(|e| DocForgeError::Internal(format!("Zip start manifest.json: {e}")))?;
    zip_out
        .write_all(meta_json.as_bytes())
        .map_err(|e| DocForgeError::Internal(format!("Zip write manifest.json: {e}")))?;

    let final_cursor = zip_out
        .finish()
        .map_err(|e| DocForgeError::Internal(format!("Zip finish dfpkg: {e}")))?;

    Ok(final_cursor.into_inner())
}

/// Imports a `.dfpkg` bundle, extracting the template metadata and docx bytes.
pub fn import_dfpkg(bundle_bytes: &[u8]) -> Result<(TemplateRecord, Vec<u8>), DocForgeError> {
    let cursor = Cursor::new(bundle_bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| {
        DocForgeError::InvalidDocx(format!("Failed to open .dfpkg bundle archive: {e}"))
    })?;

    let mut docx_bytes = Vec::new();
    {
        let mut file = archive.by_name("template.docx").map_err(|e| {
            DocForgeError::InvalidDocx(format!("Missing template.docx in .dfpkg: {e}"))
        })?;
        file.read_to_end(&mut docx_bytes).map_err(|e| {
            DocForgeError::InvalidDocx(format!("Read template.docx from .dfpkg: {e}"))
        })?;
    }

    let mut manifest_json = String::new();
    {
        let mut file = archive.by_name("manifest.json").map_err(|e| {
            DocForgeError::InvalidDocx(format!("Missing manifest.json in .dfpkg: {e}"))
        })?;
        file.read_to_string(&mut manifest_json).map_err(|e| {
            DocForgeError::InvalidDocx(format!("Read manifest.json from .dfpkg: {e}"))
        })?;
    }

    let record: TemplateRecord = serde_json::from_str(&manifest_json).map_err(|e| {
        DocForgeError::InvalidDocx(format!("Invalid manifest.json in .dfpkg: {e}"))
    })?;

    Ok((record, docx_bytes))
}
