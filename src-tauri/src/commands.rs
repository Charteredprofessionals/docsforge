use base64::{engine::general_purpose, Engine as _};
use quick_xml::events::{BytesText, Event};
use quick_xml::Reader as XmlReader;
use quick_xml::Writer;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read, Write};
use tauri::State;
use uuid::Uuid;
use zip::ZipArchive;

use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateField {
    pub id: String,
    pub label: String,
    pub original_text: String,
    pub tag_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateMeta {
    pub id: String,
    pub name: String,
    pub fields_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateFull {
    pub id: String,
    pub name: String,
    pub fields: Vec<TemplateField>,
    pub template_docx_b64: String,
    pub created_at: String,
}

#[tauri::command]
pub fn upload_docx(file_path: String) -> Result<String, String> {
    let bytes = std::fs::read(&file_path).map_err(|e| format!("Failed to read file: {e}"))?;

    let content = extract_docx_text(&bytes)?;

    let b64 = general_purpose::STANDARD.encode(&bytes);

    Ok(serde_json::json!({
        "filename": std::path::Path::new(&file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy(),
        "base64": b64,
        "textContent": content,
    })
    .to_string())
}

fn extract_docx_text(data: &[u8]) -> Result<String, String> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = ZipArchive::new(cursor).map_err(|e| format!("Invalid docx zip: {e}"))?;

    let mut xml_content = String::new();
    {
        let mut file = archive
            .by_name("word/document.xml")
            .map_err(|e| format!("No word/document.xml found: {e}"))?;
        file.read_to_string(&mut xml_content)
            .map_err(|e| format!("Failed to read document.xml: {e}"))?;
    }

    Ok(xml_content)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveTemplateRequest {
    pub name: String,
    pub original_docx_b64: String,
    pub fields: Vec<TemplateField>,
}

#[tauri::command]
pub fn save_template(
    state: State<AppState>,
    request: SaveTemplateRequest,
) -> Result<String, String> {
    let template_id = Uuid::new_v4().to_string();

    let original_bytes = general_purpose::STANDARD
        .decode(&request.original_docx_b64)
        .map_err(|e| format!("Invalid base64: {e}"))?;

    let template_bytes = replace_text_with_tags(&original_bytes, &request.fields)?;

    let fields_json =
        serde_json::to_string(&request.fields).map_err(|e| format!("Serialize fields: {e}"))?;

    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;

    db.execute(
        "INSERT INTO templates (id, name, original_docx, template_docx, fields_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            template_id,
            request.name,
            original_bytes,
            template_bytes,
            fields_json,
        ],
    )
    .map_err(|e| format!("DB insert: {e}"))?;

    Ok(serde_json::json!({ "id": template_id, "success": true }).to_string())
}

fn replace_text_with_tags(
    original_bytes: &[u8],
    fields: &[TemplateField],
) -> Result<Vec<u8>, String> {
    let cursor = std::io::Cursor::new(original_bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| format!("Invalid docx zip: {e}"))?;

    let mut document_xml = String::new();
    {
        let mut file = archive
            .by_name("word/document.xml")
            .map_err(|e| format!("No word/document.xml: {e}"))?;
        file.read_to_string(&mut document_xml)
            .map_err(|e| format!("Read document.xml: {e}"))?;
    }

    // Use quick-xml for safe parsing and replacement
    document_xml = xml_replace_with_tags(&document_xml, fields)?;

    let output = Vec::new();
    let cursor_out = std::io::Cursor::new(output);
    let mut zip_out = zip::ZipWriter::new(cursor_out);

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Archive index {i}: {e}"))?;

        let name = file.name().to_string();

        zip_out
            .start_file(&name, options)
            .map_err(|e| format!("Zip start {name}: {e}"))?;

        if name == "word/document.xml" {
            zip_out
                .write_all(document_xml.as_bytes())
                .map_err(|e| format!("Zip write document.xml: {e}"))?;
        } else {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)
                .map_err(|e| format!("Read {name}: {e}"))?;
            zip_out
                .write_all(&buf)
                .map_err(|e| format!("Write {name}: {e}"))?;
        }
    }

    let cursor_final = zip_out.finish().map_err(|e| format!("Zip finish: {e}"))?;

    Ok(cursor_final.into_inner())
}

/// Safely replace text in XML using quick-xml parser.
/// This preserves XML structure and only replaces text content within elements.
fn xml_replace_with_tags(xml: &str, fields: &[TemplateField]) -> Result<String, String> {
    let mut reader = XmlReader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let mut buf = Vec::new();
    let mut text_buf = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                writer
                    .write_event(Event::Start(e))
                    .map_err(|e| format!("Write start: {e}"))?;
                text_buf.clear();
            }
            Ok(Event::End(e)) => {
                // Check if this is a </w:t> closing tag - that's where we replace
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag_name == "w:t" {
                    let replaced = replace_in_text(&text_buf, fields);
                    writer
                        .write_event(Event::Text(BytesText::new(&replaced)))
                        .map_err(|e| format!("Write text: {e}"))?;
                    text_buf.clear();
                }
                writer
                    .write_event(Event::End(e))
                    .map_err(|e| format!("Write end: {e}"))?;
            }
            Ok(Event::Empty(e)) => {
                writer
                    .write_event(Event::Empty(e))
                    .map_err(|e| format!("Write empty: {e}"))?;
            }
            Ok(Event::Text(e)) => {
                text_buf.push_str(
                    &e.unescape()
                        .map_err(|e| format!("Unescape: {e}"))?
                        .to_string(),
                );
            }
            Ok(Event::CData(e)) => {
                writer
                    .write_event(Event::CData(e))
                    .map_err(|e| format!("Write cdata: {e}"))?;
            }
            Ok(Event::Comment(e)) => {
                writer
                    .write_event(Event::Comment(e))
                    .map_err(|e| format!("Write comment: {e}"))?;
            }
            Ok(Event::Decl(e)) => {
                writer
                    .write_event(Event::Decl(e))
                    .map_err(|e| format!("Write decl: {e}"))?;
            }
            Ok(Event::PI(e)) => {
                writer
                    .write_event(Event::PI(e))
                    .map_err(|e| format!("Write pi: {e}"))?;
            }
            Ok(Event::DocType(e)) => {
                writer
                    .write_event(Event::DocType(e))
                    .map_err(|e| format!("Write doctype: {e}"))?;
            }
            Err(e) => {
                return Err(format!(
                    "XML parse error at position {}: {e}",
                    reader.error_position()
                ));
            }
        }
        buf.clear();
    }

    let result = writer.into_inner().into_inner();
    String::from_utf8(result).map_err(|e| format!("UTF-8: {e}"))
}

/// Replace text content if it matches any field's original_text.
fn replace_in_text(text: &str, fields: &[TemplateField]) -> String {
    // Try exact match first
    for field in fields {
        if text == field.original_text {
            return format!("{{{{{}}}}}", field.tag_name);
        }
    }

    // Try substring replacement
    let mut result = text.to_string();
    for field in fields {
        if result.contains(&field.original_text) {
            result = result.replace(&field.original_text, &format!("{{{{{}}}}}", field.tag_name));
        }
    }

    result
}

#[tauri::command]
pub fn list_templates(state: State<AppState>) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;

    let mut stmt = db
        .prepare("SELECT id, name, fields_json, created_at, updated_at FROM templates ORDER BY created_at DESC")
        .map_err(|e| format!("DB prepare: {e}"))?;

    let templates: Vec<TemplateMeta> = stmt
        .query_map([], |row| {
            Ok(TemplateMeta {
                id: row.get(0)?,
                name: row.get(1)?,
                fields_json: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(|e| format!("DB query: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    serde_json::to_string(&templates).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn get_template(state: State<AppState>, template_id: String) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;

    let (name, fields_json, template_docx, created_at): (String, String, Vec<u8>, String) = db
        .query_row(
            "SELECT name, fields_json, template_docx, created_at FROM templates WHERE id = ?1",
            params![template_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| format!("Template not found: {e}"))?;

    let fields: Vec<TemplateField> =
        serde_json::from_str(&fields_json).map_err(|e| format!("Parse fields: {e}"))?;

    let b64 = general_purpose::STANDARD.encode(&template_docx);

    let full = TemplateFull {
        id: template_id,
        name,
        fields,
        template_docx_b64: b64,
        created_at,
    };

    serde_json::to_string(&full).map_err(|e| format!("Serialize: {e}"))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FillTemplateRequest {
    pub template_id: String,
    pub values: std::collections::HashMap<String, String>,
}

#[tauri::command]
pub fn fill_template(
    state: State<AppState>,
    request: FillTemplateRequest,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;

    let template_docx: Vec<u8> = db
        .query_row(
            "SELECT template_docx FROM templates WHERE id = ?1",
            params![request.template_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Template not found: {e}"))?;

    let filled = fill_docx_with_values(&template_docx, &request.values)?;

    let b64 = general_purpose::STANDARD.encode(&filled);

    Ok(serde_json::json!({ "docx_base64": b64 }).to_string())
}

/// Replace {{tag}} placeholders inside word/document.xml with provided values.
fn fill_docx_with_values(
    template_bytes: &[u8],
    values: &std::collections::HashMap<String, String>,
) -> Result<Vec<u8>, String> {
    let cursor = std::io::Cursor::new(template_bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| format!("Invalid docx zip: {e}"))?;

    let mut document_xml = String::new();
    {
        let mut file = archive
            .by_name("word/document.xml")
            .map_err(|e| format!("No word/document.xml: {e}"))?;
        file.read_to_string(&mut document_xml)
            .map_err(|e| format!("Read document.xml: {e}"))?;
    }

    document_xml = xml_fill_values(&document_xml, values)?;

    let output = Vec::new();
    let cursor_out = std::io::Cursor::new(output);
    let mut zip_out = zip::ZipWriter::new(cursor_out);

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Archive index {i}: {e}"))?;

        let name = file.name().to_string();

        zip_out
            .start_file(&name, options)
            .map_err(|e| format!("Zip start {name}: {e}"))?;

        if name == "word/document.xml" {
            zip_out
                .write_all(document_xml.as_bytes())
                .map_err(|e| format!("Zip write document.xml: {e}"))?;
        } else {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)
                .map_err(|e| format!("Read {name}: {e}"))?;
            zip_out
                .write_all(&buf)
                .map_err(|e| format!("Write {name}: {e}"))?;
        }
    }

    let cursor_final = zip_out.finish().map_err(|e| format!("Zip finish: {e}"))?;

    Ok(cursor_final.into_inner())
}

fn xml_fill_values(
    xml: &str,
    values: &std::collections::HashMap<String, String>,
) -> Result<String, String> {
    let mut result = xml.to_string();
    for (tag, value) in values {
        let placeholder = format!("{{{{{}}}}}", tag);
        result = result.replace(&placeholder, value);
    }
    Ok(result)
}

#[tauri::command]
pub fn delete_template(state: State<AppState>, template_id: String) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;

    let affected = db
        .execute("DELETE FROM templates WHERE id = ?1", params![template_id])
        .map_err(|e| format!("DB delete: {e}"))?;

    if affected == 0 {
        return Err("Template not found".to_string());
    }

    Ok(serde_json::json!({ "success": true }).to_string())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportPdfRequest {
    pub docx_base64: String,
    pub output_filename: String,
}

/// PDF export using LibreOffice headless.
/// Returns a clear error if LibreOffice is not installed.
#[tauri::command]
pub fn export_to_pdf(request: ExportPdfRequest) -> Result<String, String> {
    let bytes = general_purpose::STANDARD
        .decode(&request.docx_base64)
        .map_err(|e| format!("Invalid base64: {e}"))?;

    let temp_dir = std::env::temp_dir().join("docforge");
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("Create temp dir: {e}"))?;

    let docx_path = temp_dir.join(format!("{}.docx", Uuid::new_v4()));

    std::fs::write(&docx_path, &bytes).map_err(|e| format!("Write temp docx: {e}"))?;

    // Check if LibreOffice is available first
    let soffice_check = std::process::Command::new("soffice")
        .arg("--version")
        .output();

    if soffice_check.is_err() {
        return Err(
            "LibreOffice not found. Please install LibreOffice to enable PDF export.".to_string(),
        );
    }

    let output = std::process::Command::new("soffice")
        .args([
            "--headless",
            "--convert-to",
            "pdf",
            "--outdir",
            temp_dir.to_str().unwrap_or("."),
            docx_path.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| format!("Failed to run LibreOffice: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PDF conversion failed: {stderr}"));
    }

    let generated_pdf = temp_dir.join(
        docx_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
            + ".pdf",
    );

    let pdf_bytes =
        std::fs::read(&generated_pdf).map_err(|e| format!("Read generated PDF: {e}"))?;

    let pdf_b64 = general_purpose::STANDARD.encode(&pdf_bytes);

    let _ = std::fs::remove_file(&docx_path);
    let _ = std::fs::remove_file(&generated_pdf);

    Ok(serde_json::json!({
        "pdf_base64": pdf_b64,
        "filename": format!("{}.pdf", request.output_filename),
    })
    .to_string())
}
