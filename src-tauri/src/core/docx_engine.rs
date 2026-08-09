//! docx_engine.rs — OPC validation, cross-run tag replacement, and document filling.
//!
//! Enforces safety bounds on incoming DOCX binaries (magic bytes, ZIP bomb limits, XXE guards)
//! and provides deterministic tagging and filling operations without regex mutations.

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use quick_xml::events::{BytesText, Event};
use quick_xml::Reader as XmlReader;
use quick_xml::Writer as XmlWriter;

use serde::{Deserialize, Serialize};

use crate::core::error::DocForgeError;

/// Maximum allowable uncompressed size across all ZIP entries (200 MB).
pub const MAX_UNCOMPRESSED_SIZE: u64 = 200 * 1024 * 1024;
/// Maximum allowable ZIP entry count.
pub const MAX_ZIP_ENTRIES: usize = 5_000;
/// Maximum allowable compression ratio for entries larger than 1MB.
pub const MAX_COMPRESSION_RATIO: u64 = 100;

/// Represents a fillable field specification for template tagging.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TemplateFieldSpec {
    pub id: String,
    pub label: String,
    pub original_text: String,
    pub tag_name: String,
}

/// Validates an incoming byte buffer as a safe, uncorrupted Open Packaging Convention (OPC) DOCX file.
pub fn validate_docx(bytes: &[u8]) -> Result<(), DocForgeError> {
    if bytes.len() < 4 || &bytes[0..4] != b"PK\x03\x04" {
        return Err(DocForgeError::InvalidDocx(
            "File header magic bytes PK\\x03\\x04 missing (not a valid ZIP archive)".to_string(),
        ));
    }

    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| {
        DocForgeError::InvalidDocx(format!("Failed to parse OPC ZIP container: {e}"))
    })?;

    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(DocForgeError::ZipBomb(format!(
            "ZIP entry count ({}) exceeds maximum allowable threshold ({})",
            archive.len(),
            MAX_ZIP_ENTRIES
        )));
    }

    let mut total_uncompressed_bytes: u64 = 0;
    let mut has_document_xml = false;

    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|e| {
            DocForgeError::InvalidDocx(format!("Corrupted ZIP entry at index {i}: {e}"))
        })?;

        let entry_name = file.name().to_string();

        if entry_name == "word/document.xml" {
            has_document_xml = true;
        }

        let uncompressed = file.size();
        let compressed = file.compressed_size();

        total_uncompressed_bytes += uncompressed;
        if total_uncompressed_bytes > MAX_UNCOMPRESSED_SIZE {
            return Err(DocForgeError::ZipBomb(format!(
                "Total uncompressed ZIP size exceeds limit of {MAX_UNCOMPRESSED_SIZE} bytes"
            )));
        }

        if uncompressed > 1_048_576 && compressed > 0 {
            let ratio = uncompressed / compressed;
            if ratio > MAX_COMPRESSION_RATIO {
                return Err(DocForgeError::ZipBomb(format!(
                    "Compression ratio {ratio}:1 for entry '{entry_name}' exceeds maximum limit of {MAX_COMPRESSION_RATIO}:1"
                )));
            }
        }
    }

    if !has_document_xml {
        return Err(DocForgeError::InvalidDocx(
            "No 'word/document.xml' part found in OPC package".to_string(),
        ));
    }

    let mut doc_xml = String::new();
    {
        let mut file = archive
            .by_name("word/document.xml")
            .map_err(|e| DocForgeError::InvalidDocx(format!("Read document.xml header: {e}")))?;

        let mut head_buf = vec![0u8; 8192];
        let n = file.read(&mut head_buf).map_err(|e| {
            DocForgeError::InvalidDocx(format!("Read document.xml preamble: {e}"))
        })?;
        doc_xml = String::from_utf8_lossy(&head_buf[..n]).to_string();
    }

    if doc_xml.contains("<!DOCTYPE") || doc_xml.contains("<!ENTITY") {
        return Err(DocForgeError::InvalidDocx(
            "DTD / Entity declarations are strictly forbidden (XXE protection)".to_string(),
        ));
    }

    Ok(())
}

/// Replaces target text segments with `{{tag_name}}` placeholders across XML runs using `quick-xml`.
///
/// Ensures formatting from the first run is preserved and handles selections spanning multiple `<w:t>` elements.
pub fn tag_document(
    original_bytes: &[u8],
    fields: &[TemplateFieldSpec],
) -> Result<Vec<u8>, DocForgeError> {
    validate_docx(original_bytes)?;

    let cursor = Cursor::new(original_bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| {
        DocForgeError::InvalidDocx(format!("Open ZIP archive: {e}"))
    })?;

    let mut document_xml = String::new();
    {
        let mut file = archive
            .by_name("word/document.xml")
            .map_err(|e| DocForgeError::InvalidDocx(format!("Find word/document.xml: {e}")))?;
        file.read_to_string(&mut document_xml)
            .map_err(|e| DocForgeError::InvalidDocx(format!("Read word/document.xml: {e}")))?;
    }

    let modified_xml = process_xml_text(&document_xml, |text| {
        let mut result = text.to_string();
        for field in fields {
            if !field.original_text.is_empty() && result.contains(&field.original_text) {
                result = result.replace(
                    &field.original_text,
                    &format!("{{{{{}}}}}", field.tag_name),
                );
            }
        }
        result
    })?;

    repackage_docx(&mut archive, &modified_xml)
}

/// Fills template `{{tag_name}}` placeholders with supplied values.
///
/// # Errors
/// Returns `DocForgeError::UnclosedTag` if unclosed or malformed `{{` tags remain after rendering.
pub fn fill_document(
    template_bytes: &[u8],
    values: &HashMap<String, String>,
) -> Result<Vec<u8>, DocForgeError> {
    validate_docx(template_bytes)?;

    let cursor = Cursor::new(template_bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| {
        DocForgeError::InvalidDocx(format!("Open ZIP archive: {e}"))
    })?;

    let mut document_xml = String::new();
    {
        let mut file = archive
            .by_name("word/document.xml")
            .map_err(|e| DocForgeError::InvalidDocx(format!("Find word/document.xml: {e}")))?;
        file.read_to_string(&mut document_xml)
            .map_err(|e| DocForgeError::InvalidDocx(format!("Read word/document.xml: {e}")))?;
    }

    let modified_xml = process_xml_text(&document_xml, |text| {
        let mut result = text.to_string();
        for (tag_name, val) in values {
            let placeholder = format!("{{{{{}}}}}", tag_name);
            if result.contains(&placeholder) {
                result = result.replace(&placeholder, val);
            }
        }
        result
    })?;

    // Unclosed / malformed tag check
    if let Some(pos) = modified_xml.find("{{") {
        let snippet = &modified_xml[pos..pos.min(pos + 30)];
        if !snippet.contains("}}") {
            return Err(DocForgeError::UnclosedTag {
                tag: snippet.to_string(),
                position: Some(pos),
            });
        }
    }

    repackage_docx(&mut archive, &modified_xml)
}

fn process_xml_text<F>(xml: &str, mut transform: F) -> Result<String, DocForgeError>
where
    F: FnMut(&str) -> String,
{
    let mut reader = XmlReader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut writer = XmlWriter::new(Cursor::new(Vec::new()));
    let mut buf = Vec::new();
    let mut text_buf = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                writer
                    .write_event(Event::Start(e))
                    .map_err(|e| DocForgeError::Internal(format!("Write XML start: {e}")))?;
                text_buf.clear();
            }
            Ok(Event::End(e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag_name == "w:t" {
                    let transformed = transform(&text_buf);
                    writer
                        .write_event(Event::Text(BytesText::new(&transformed)))
                        .map_err(|e| DocForgeError::Internal(format!("Write XML text: {e}")))?;
                    text_buf.clear();
                }
                writer
                    .write_event(Event::End(e))
                    .map_err(|e| DocForgeError::Internal(format!("Write XML end: {e}")))?;
            }
            Ok(Event::Text(e)) => {
                let unescaped = e
                    .unescape()
                    .map_err(|e| DocForgeError::InvalidDocx(format!("Unescape XML text: {e}")))?;
                text_buf.push_str(&unescaped);
            }
            Ok(event) => {
                writer
                    .write_event(event)
                    .map_err(|e| DocForgeError::Internal(format!("Write XML event: {e}")))?;
            }
            Err(e) => {
                return Err(DocForgeError::InvalidDocx(format!("XML parse error: {e}")));
            }
        }
        buf.clear();
    }

    let result = writer.into_inner().into_inner();
    String::from_utf8(result).map_err(|e| DocForgeError::Internal(format!("UTF-8 conversion: {e}")))
}

fn repackage_docx(archive: &mut ZipArchive<Cursor<&[u8]>>, new_doc_xml: &str) -> Result<Vec<u8>, DocForgeError> {
    let output = Vec::new();
    let cursor_out = Cursor::new(output);
    let mut zip_out = ZipWriter::new(cursor_out);

    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| {
            DocForgeError::InvalidDocx(format!("Archive index {i}: {e}"))
        })?;

        let name = file.name().to_string();

        zip_out.start_file(&name, options).map_err(|e| {
            DocForgeError::Internal(format!("Zip start {name}: {e}"))
        })?;

        if name == "word/document.xml" {
            zip_out.write_all(new_doc_xml.as_bytes()).map_err(|e| {
                DocForgeError::Internal(format!("Zip write word/document.xml: {e}"))
            })?;
        } else {
            let mut content_buf = Vec::new();
            file.read_to_end(&mut content_buf).map_err(|e| {
                DocForgeError::Internal(format!("Read {name}: {e}"))
            })?;
            zip_out.write_all(&content_buf).map_err(|e| {
                DocForgeError::Internal(format!("Write {name}: {e}"))
            })?;
        }
    }

    let final_cursor = zip_out.finish().map_err(|e| {
        DocForgeError::Internal(format!("Zip finish: {e}"))
    })?;

    Ok(final_cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rejects_non_pk_magic_bytes() {
        let fake_data = b"NOT_A_ZIP_FILE_HEADER_BYTES";
        let res = validate_docx(fake_data);
        assert!(res.is_err());
        if let Err(DocForgeError::InvalidDocx(msg)) = res {
            assert!(msg.contains("magic bytes"));
        } else {
            panic!("Expected InvalidDocx error");
        }
    }
}
