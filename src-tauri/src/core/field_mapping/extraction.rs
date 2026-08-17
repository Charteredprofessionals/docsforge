//! extraction.rs — Placeholder discovery and unmapped placeholder health check (REQ-029, REQ-038).
//!
//! Provides raw-text and DOCX placeholder extraction, plus a database-backed
//! health-check that identifies unmapped placeholders with optional field suggestions.

use std::collections::HashSet;
use std::io::{Cursor, Read};

use rusqlite::{params, Connection};
use zip::ZipArchive;

use crate::core::docx_engine::validate_docx;
use crate::core::error::DocForgeError;
use crate::core::field_mapping::mapping::{list_mappings, UnmappedPlaceholder};
use crate::core::field_mapping::registry::list_fields;
use crate::core::template_store::load_template_file;

/// Scans raw text for `{{...}}` patterns and returns unique placeholder strings,
/// deduplicated preserving first-seen order. Nested or unclosed `{{` sequences
/// are ignored.
pub fn extract_placeholders_from_text(text: &str) -> Vec<String> {
    let mut placeholders = Vec::new();
    let mut seen = HashSet::new();
    let bytes = text.as_bytes();
    let mut i = 0;

    while i + 1 < bytes.len() {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            let start = i;
            i += 2;
            let mut found = false;
            while i + 1 < bytes.len() {
                if bytes[i] == b'}' && bytes[i + 1] == b'}' {
                    let end = i + 2;
                    let placeholder = text[start..end].to_string();
                    if seen.insert(placeholder.clone()) {
                        placeholders.push(placeholder);
                    }
                    i = end;
                    found = true;
                    break;
                }
                i += 1;
            }
            if !found {
                break;
            }
        } else {
            i += 1;
        }
    }

    placeholders
}

/// Extracts placeholders from a DOCX byte buffer.
///
/// Validates the OPC structure via `docx_engine::validate_docx`, then reads
/// `word/document.xml` and strips XML tags to produce plain text before
/// scanning for `{{...}}` patterns.
pub fn extract_placeholders_from_docx(bytes: &[u8]) -> Result<Vec<String>, DocForgeError> {
    validate_docx(bytes)?;

    let cursor = Cursor::new(bytes);
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

    let plain_text = strip_xml_tags(&document_xml);
    Ok(extract_placeholders_from_text(&plain_text))
}

/// Identifies unmapped placeholders for a bundle version by scanning each
/// document's template and diffing against `field_mappings`.
///
/// For each unmapped placeholder, suggests a canonical field when a field's
/// label matches the placeholder text (case-insensitive, braces stripped).
pub fn find_unmapped_placeholders(
    conn: &Connection,
    bundle_version_id: &str,
) -> Result<Vec<UnmappedPlaceholder>, DocForgeError> {
    let mappings = list_mappings(conn, bundle_version_id, None)
        .map_err(|e| DocForgeError::StorageIo(format!("List mappings: {e}")))?;

    let fields = list_fields(conn, bundle_version_id)
        .map_err(|e| DocForgeError::StorageIo(format!("List fields: {e}")))?;

    let mut doc_stmt = conn
        .prepare("SELECT id, template_id FROM bundle_documents WHERE bundle_version_id = ?1")
        .map_err(|e| DocForgeError::StorageIo(format!("Prepare bundle documents query: {e}")))?;

    let doc_rows = doc_stmt
        .query_map(params![bundle_version_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| DocForgeError::StorageIo(format!("Query bundle documents: {e}")))?;

    let mut unmapped = Vec::new();

    for doc_row in doc_rows {
        let (document_id, template_id) =
            doc_row.map_err(|e| DocForgeError::StorageIo(format!("Map document row: {e}")))?;

        if template_id.is_empty() {
            continue;
        }

        let (_record, template_bytes) = load_template_file(conn, &template_id)
            .map_err(|e| DocForgeError::StorageIo(format!("Load template file: {e}")))?;

        let placeholders = extract_placeholders_from_docx(&template_bytes)
            .map_err(|e| DocForgeError::StorageIo(format!("Extract placeholders from docx: {e}")))?;

        for placeholder in placeholders {
            let is_mapped = mappings
                .iter()
                .any(|m| m.document_id == document_id && m.placeholder == placeholder);

            if !is_mapped {
                let suggested = fields.iter().find(|f| {
                    let label_lower = f.label.to_lowercase();
                    let ph_clean = placeholder
                        .trim_matches(|c| c == '{' || c == '}')
                        .replace('_', " ")
                        .to_lowercase();
                    label_lower == ph_clean
                }).map(|f| f.field_id.clone());

                unmapped.push(UnmappedPlaceholder {
                    document_id: document_id.clone(),
                    placeholder,
                    suggested_canonical_field_id: suggested,
                });
            }
        }
    }

    Ok(unmapped)
}

/// Removes XML tags from a string, preserving text content between elements.
fn strip_xml_tags(xml: &str) -> String {
    let mut plain = String::new();
    let mut in_tag = false;
    for ch in xml.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => plain.push(ch),
            _ => {}
        }
    }
    plain
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bundle::manifest::create_bundle;
    use crate::core::field_mapping::schema::{FieldDef, FieldType};
    use crate::core::field_mapping::registry::create_field;
    use crate::core::template_store::save_template;
    use crate::schema::init_memory_db;
    use rusqlite::params;
    use std::io::{Cursor, Write};
    use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

    // ---------------------------------------------------------------------------
    // Text extraction tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_extract_placeholders_from_text_basic() {
        let input = "Hello {{name}}, your {{email}} is ready";
        let result = extract_placeholders_from_text(input);
        assert_eq!(result, vec!["{{name}}", "{{email}}"]);
    }

    #[test]
    fn test_extract_placeholders_from_text_deduplicates() {
        let input = "{{x}} ... {{x}} ... {{y}}";
        let result = extract_placeholders_from_text(input);
        assert_eq!(result, vec!["{{x}}", "{{y}}"]);
    }

    #[test]
    fn test_extract_placeholders_from_text_no_placeholders() {
        let input = "plain text";
        let result = extract_placeholders_from_text(input);
        assert!(result.is_empty());
    }

    #[test]
    fn test_extract_placeholders_nested_braces_ignored() {
        let input = "{{a}} and {{b";
        let result = extract_placeholders_from_text(input);
        assert_eq!(result, vec!["{{a}}"]);
    }

    // ---------------------------------------------------------------------------
    // DOCX extraction test
    // ---------------------------------------------------------------------------

    #[test]
    fn test_extract_placeholders_from_docx_basic() {
        let docx = minimal_docx("Hello {{name}}, your {{email}} is ready");
        let result = extract_placeholders_from_docx(&docx).expect("extract from docx");
        assert_eq!(result, vec!["{{name}}", "{{email}}"]);
    }

    // ---------------------------------------------------------------------------
    // Unmapped placeholders tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_find_unmapped_placeholders_identifies_gaps() {
        let conn = init_memory_db().expect("memory db");
        let record = create_bundle(&conn, "Mapping Check", None, None).expect("create bundle");
        let bv_id = conn
            .query_row(
                "SELECT id FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC LIMIT 1",
                [&record.id],
                |r| r.get::<_, String>(0),
            )
            .expect("head version");

        let field = create_field(
            &conn,
            &bv_id,
            &FieldDef {
                id: String::new(),
                field_id: "company.name".to_string(),
                label: "Company Name".to_string(),
                description: None,
                field_type: FieldType::Text,
                required: true,
                default: None,
                validation: None,
                group_id: None,
                options: Vec::new(),
                format: None,
                position: 0,
            },
        )
        .expect("create field");

        let tpl1_bytes = minimal_docx("Hello {{company_name}}");
        let tpl1 = save_template(&conn, "Template 1", "general", "", &[], &tpl1_bytes, None, None)
            .expect("save template 1");

        let tpl2_bytes = minimal_docx("Goodbye {{director_name}}");
        let tpl2 = save_template(&conn, "Template 2", "general", "", &[], &tpl2_bytes, None, None)
            .expect("save template 2");

        let doc1 = &tpl1.id;
        let doc2 = &tpl2.id;

        conn.execute(
            "INSERT INTO bundle_documents (id, bundle_version_id, template_id, position, include_default)
             VALUES (?1, ?2, ?3, 0, 1)",
            params![doc1, bv_id, &tpl1.id],
        ).expect("insert bundle_document 1");
        conn.execute(
            "INSERT INTO bundle_documents (id, bundle_version_id, template_id, position, include_default)
             VALUES (?1, ?2, ?3, 1, 1)",
            params![doc2, bv_id, &tpl2.id],
        ).expect("insert bundle_document 2");

        conn.execute(
            "INSERT INTO field_mappings (id, bundle_version_id, document_id, placeholder, canonical_field_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params!["fm1", bv_id, doc1, "{{company_name}}", &field.id],
        )
        .expect("insert mapping");

        let unmapped = find_unmapped_placeholders(&conn, &bv_id).expect("find unmapped");
        assert_eq!(unmapped.len(), 1);
        assert_eq!(unmapped[0].document_id, *doc2);
        assert_eq!(unmapped[0].placeholder, "{{director_name}}");
        assert_eq!(unmapped[0].suggested_canonical_field_id, None);
    }

    #[test]
    fn test_find_unmapped_suggests_field() {
        let conn = init_memory_db().expect("memory db");
        let record = create_bundle(&conn, "Suggest Check", None, None).expect("create bundle");
        let bv_id = conn
            .query_row(
                "SELECT id FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC LIMIT 1",
                [&record.id],
                |r| r.get::<_, String>(0),
            )
            .expect("head version");

        let _field = create_field(
            &conn,
            &bv_id,
            &FieldDef {
                id: String::new(),
                field_id: "company.name".to_string(),
                label: "Company Name".to_string(),
                description: None,
                field_type: FieldType::Text,
                required: true,
                default: None,
                validation: None,
                group_id: None,
                options: Vec::new(),
                format: None,
                position: 0,
            },
        )
        .expect("create field");

        let tpl1_bytes = minimal_docx("Hello {{Company_Name}}");
        let tpl1 = save_template(&conn, "Template 1", "general", "", &[], &tpl1_bytes, None, None)
            .expect("save template");

        conn.execute(
            "INSERT INTO bundle_documents (id, bundle_version_id, template_id, position, include_default)
             VALUES (?1, ?2, ?3, 0, 1)",
            params![&tpl1.id, bv_id, &tpl1.id],
        )
        .expect("insert bundle_document");

        let unmapped = find_unmapped_placeholders(&conn, &bv_id).expect("find unmapped");
        assert_eq!(unmapped.len(), 1);
        assert_eq!(unmapped[0].suggested_canonical_field_id, Some("company.name".to_string()));
    }

    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------

    /// Builds a minimal valid DOCX byte buffer that passes `validate_docx`.
    fn minimal_docx(text: &str) -> Vec<u8> {
        let inner = format!(
            r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>{}</w:t></w:r></w:p></w:body></w:document>"#,
            text
        );
        let mut out = Vec::new();
        {
            let cursor = Cursor::new(&mut out);
            let mut zip = ZipWriter::new(cursor);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            zip.start_file("word/document.xml", opts).unwrap();
            zip.write_all(inner.as_bytes()).unwrap();
            let c = zip.finish().unwrap();
            let _ = c.into_inner();
        }
        out
    }
}
