//! services/mod.rs — Service facade layer executing tasks off the UI thread.

use std::collections::HashMap;
use rusqlite::Connection;

use crate::core::docx_engine::{fill_document, validate_docx, TemplateFieldSpec};
use crate::core::error::DocForgeError;
use crate::core::export::{export_dfpkg, export_pdf};
use crate::core::governance::record_generation;
use crate::core::template::TemplateRecord;
use crate::core::template_store::{load_template_file, save_template};

pub struct DocumentService;

impl DocumentService {
    pub fn process_upload(file_path: &str) -> Result<(String, String), DocForgeError> {
        let bytes = std::fs::read(file_path)
            .map_err(|e| DocForgeError::StorageIo(format!("Read upload file: {e}")))?;
        validate_docx(&bytes)?;
        let filename = std::path::Path::new(file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        Ok((filename, "Uploaded DOCX valid".to_string()))
    }

    pub fn save_new_template(
        conn: &Connection,
        name: &str,
        category: &str,
        description: &str,
        fields: &[TemplateFieldSpec],
        docx_bytes: &[u8],
    ) -> Result<TemplateRecord, DocForgeError> {
        save_template(conn, name, category, description, fields, docx_bytes, None, None)
    }

    pub fn fill_and_export(
        conn: &Connection,
        template_id: &str,
        values: &HashMap<String, String>,
        format: &str,
        output_name: &str,
    ) -> Result<Vec<u8>, DocForgeError> {
        let (record, docx_bytes) = load_template_file(conn, template_id)?;

        let filled = fill_document(&docx_bytes, values)?;

        record_generation(conn, template_id, record.current_version, output_name, format, None, None)?;

        match format {
            "docx" => Ok(filled),
            "pdf" => {
                let html = format!("<h1>{}</h1>", output_name);
                export_pdf(&html)
            }
            "dfpkg" => export_dfpkg(&record, &filled),
            _ => Err(DocForgeError::InvalidDocx(format!("Unsupported export format '{format}'"))),
        }
    }
}
