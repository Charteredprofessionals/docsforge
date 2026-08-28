use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tauri::State;
use uuid::Uuid;
use zip::ZipArchive;

use crate::AppState;
use crate::core::docx_engine::{fill_document, tag_document, TemplateFieldSpec};
use crate::core::export::export_pdf_from_docx;
use crate::core::field_mapping::extraction::extract_placeholders_from_docx;
use sha2::{Digest, Sha256};
use crate::core::governance::{
    authorize, get_current_user as get_db_current_user, get_current_user_role, set_current_user_role,
    Action,
};
use crate::core::governance::record_generation as log_generation;
use crate::core::template_store;
use crate::core::bundles::{
    add_template_to_bundle, create_bundle, delete_bundle, get_bundle_templates, list_bundles,
    remove_template_from_bundle,
};
use crate::core::bundle::{
    create_bundle as create_bundle_v2, list_bundles as list_bundles_v2, get_bundle as get_bundle_v2,
    get_manifest, save_manifest,
    BundleManifest,
};
use crate::core::bundle::version::{
    create_draft_version, publish_version, review_version, archive_version, list_versions,
};
use crate::core::bundle::dfpkg::{
    export_bundle_dfpkg, import_bundle_dfpkg,
};
use crate::core::field_mapping::{
    create_field, update_field, list_fields, remove_field, create_field_group,
    FieldDef, FieldGroup, GroupScope,
};
use crate::core::field_mapping::groups::{
    create_group, list_groups_with_shared_first, assign_field_to_group, group_summary,
    list_field_groups,
};
use crate::core::field_mapping::mapping::{
    set_mapping, list_mappings,
};
use crate::core::field_mapping::extraction::find_unmapped_placeholders;
use crate::core::matter::{
    create_matter, get_matter, list_matters, update_matter_status, delete_matter,
    set_matter_value, get_matter_value, list_matter_values, matter_to_json,
    render_matter_form, populate_matter_field, validate_matter,
    MatterStatus,
};
use crate::core::rules::{
    add_rule, remove_rule, list_rules, evaluate_rules, evaluate_preview, validate_rule_expression,
};
use crate::core::generation_run::{
    execute_run, create_run, get_run, list_runs,
    RunStatus,
};
use crate::schema::{get_db_path, init_db};

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
    pub fields: Vec<TemplateFieldSpec>,
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
#[serde(rename_all = "camelCase")]
pub struct SaveTemplateRequest {
    pub name: String,
    pub original_docx_b64: String,
    pub fields: Vec<TemplateFieldSpec>,
}

#[tauri::command]
pub fn save_template(
    state: State<AppState>,
    request: SaveTemplateRequest,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;

    // RBAC: Creator or Admin required
    authorize(get_current_user_role(&db)?, Action::CreateTemplate)
        .map_err(|e| e.to_string())?;

    let original_bytes = general_purpose::STANDARD
        .decode(&request.original_docx_b64)
        .map_err(|e| format!("Invalid base64: {e}"))?;

    // Turn the original document into a template by inserting {{tag}} placeholders.
    let template_bytes = tag_document(&original_bytes, &request.fields)?;

    let record = template_store::save_template(
        &db,
        &request.name,
        "general",
        "",
        &request.fields,
        &template_bytes,
        None,
        None,
    )?;

    Ok(serde_json::json!({ "id": record.id, "success": true }).to_string())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTemplateRequest {
    pub template_id: String,
    pub name: Option<String>,
    pub category: Option<String>,
    pub description: Option<String>,
    pub fields: Option<Vec<TemplateFieldSpec>>,
    pub original_docx_b64: Option<String>,
}

#[tauri::command]
pub fn update_template(
    state: State<AppState>,
    request: UpdateTemplateRequest,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;

    authorize(get_current_user_role(&db)?, Action::CreateTemplate)
        .map_err(|e| e.to_string())?;

    let docx_bytes = if let Some(b64) = request.original_docx_b64 {
        Some(general_purpose::STANDARD.decode(&b64).map_err(|e| format!("Invalid base64: {e}"))?)
    } else {
        None
    };

    let record = template_store::update_template(
        &db,
        &request.template_id,
        request.name.as_deref(),
        request.category.as_deref(),
        request.description.as_deref(),
        request.fields.as_deref(),
        docx_bytes.as_deref(),
    )?;

    Ok(serde_json::json!({ "id": record.id, "success": true }).to_string())
}

/// Builds a minimal but valid DOCX whose body contains the literal marker words
/// that `seed_sample_template` tags into `{{...}}` placeholders. Kept dependency-free
/// (plain ZIP + XML) so it passes `validate_docx` without extra crates.
fn build_sample_template_docx() -> Vec<u8> {
    let body = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>
<w:p><w:r><w:t>Dear recipient_name,</w:t></w:r></w:p>
<w:p><w:r><w:t>Welcome to company_name. This letter is issued on document_date to confirm the details we discussed.</w:t></w:r></w:p>
<w:p><w:r><w:t>Please review the attached materials and contact sender_name with any questions.</w:t></w:r></w:p>
<w:p><w:r><w:t>Sincerely,</w:t></w:r></w:p>
<w:p><w:r><w:t>sender_name</w:t></w:r></w:p>
</w:body>
</w:document>"#;

    let mut out = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut out);
        let mut zip = zip::ZipWriter::new(cursor);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("word/document.xml", opts)
            .expect("write sample document.xml");
        zip.write_all(body.as_bytes())
            .expect("write sample body");
        let _ = zip.finish().expect("finish sample zip").into_inner();
    }
    out
}

const SAMPLE_TEMPLATE_NAME: &str = "Sample Welcome Letter";

/// Seeds a starter template so first-time users have a working example to learn
/// DocForge with. Idempotent: returns the existing sample id if already seeded.
#[tauri::command]
pub fn seed_sample_template(state: State<AppState>) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    authorize(get_current_user_role(&db)?, Action::CreateTemplate)
        .map_err(|e| e.to_string())?;

    let templates = template_store::list_templates(&db, None).map_err(|e| e.to_string())?;
    if let Some(existing) = templates
        .iter()
        .find(|t| t.name == SAMPLE_TEMPLATE_NAME)
    {
        return serde_json::to_string(&serde_json::json!({
            "id": existing.id,
            "already_exists": true
        }))
        .map_err(|e| format!("Serialize: {e}"));
    }

    let docx_bytes = build_sample_template_docx();
    let fields = vec![
        TemplateFieldSpec {
            id: "f_recipient".to_string(),
            label: "Recipient Name".to_string(),
            original_text: "recipient_name".to_string(),
            tag_name: "recipient_name".to_string(),
        },
        TemplateFieldSpec {
            id: "f_company".to_string(),
            label: "Company Name".to_string(),
            original_text: "company_name".to_string(),
            tag_name: "company_name".to_string(),
        },
        TemplateFieldSpec {
            id: "f_date".to_string(),
            label: "Document Date".to_string(),
            original_text: "document_date".to_string(),
            tag_name: "document_date".to_string(),
        },
        TemplateFieldSpec {
            id: "f_sender".to_string(),
            label: "Sender Name".to_string(),
            original_text: "sender_name".to_string(),
            tag_name: "sender_name".to_string(),
        },
    ];

    let record = template_store::save_template(
        &db,
        SAMPLE_TEMPLATE_NAME,
        "general",
        "A starter template to learn DocForge. Edit or delete it any time.",
        &fields,
        &docx_bytes,
        None,
        None,
    )
    .map_err(|e| e.to_string())?;

    serde_json::to_string(&serde_json::json!({ "id": record.id, "success": true }))
        .map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn list_templates(state: State<AppState>) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;

    let records = template_store::list_templates(&db, None)?;

    let metas: Vec<TemplateMeta> = records
        .into_iter()
        .map(|r| TemplateMeta {
            id: r.id,
            name: r.name,
            fields_json: serde_json::to_string(&r.fields).unwrap_or_else(|_| "[]".to_string()),
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect();

    serde_json::to_string(&metas).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn get_template(state: State<AppState>, template_id: String) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;

    // RBAC: ViewTemplate allowed for all authenticated users
    authorize(get_current_user_role(&db)?, Action::ViewTemplate)
        .map_err(|e| e.to_string())?;

    let (record, bytes) = template_store::load_template_file(&db, &template_id)?;

    let b64 = general_purpose::STANDARD.encode(&bytes);

    let full = TemplateFull {
        id: template_id,
        name: record.name,
        fields: record.fields,
        template_docx_b64: b64,
        created_at: record.created_at,
    };

    serde_json::to_string(&full).map_err(|e| format!("Serialize: {e}"))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTemplateFieldsRequest {
    pub template_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTemplateFieldsCsvRequest {
    pub template_id: String,
}

/// Extracts the unique `{{field}}` placeholder names embedded in a saved template's
/// DOCX and returns them as a clean list (braces stripped) for verification and
/// downstream CSV bulk-entry (Phase A of the mail-merge feature).
#[tauri::command]
pub fn get_template_fields(
    state: State<AppState>,
    request: GetTemplateFieldsRequest,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    authorize(get_current_user_role(&db)?, Action::ViewTemplate).map_err(|e| e.to_string())?;

    let (_record, bytes) =
        template_store::load_template_file(&db, &request.template_id).map_err(|e| e.to_string())?;

    let placeholders = extract_placeholders_from_docx(&bytes).map_err(|e| e.to_string())?;
    let fields: Vec<String> = placeholders
        .iter()
        .map(|p| p.trim_start_matches("{{").trim_end_matches("}}").to_string())
        .collect();

    serde_json::to_string(&serde_json::json!({ "fields": fields }))
        .map_err(|e| format!("Serialize: {e}"))
}

/// Produces a downloadable CSV whose header row matches the template's extracted
/// `{{field}}` placeholder names plus a reserved `output_filename` column, ready
/// for bulk data entry (one output document per data row).
#[tauri::command]
pub fn export_template_fields_csv(
    state: State<AppState>,
    request: ExportTemplateFieldsCsvRequest,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    authorize(get_current_user_role(&db)?, Action::ViewTemplate).map_err(|e| e.to_string())?;

    let (_record, bytes) =
        template_store::load_template_file(&db, &request.template_id).map_err(|e| e.to_string())?;

    let placeholders = extract_placeholders_from_docx(&bytes).map_err(|e| e.to_string())?;
    let fields: Vec<String> = placeholders
        .iter()
        .map(|p| p.trim_start_matches("{{").trim_end_matches("}}").to_string())
        .collect();

    let mut headers = fields.clone();
    headers.push("output_filename".to_string());

    let header_row = headers
        .iter()
        .map(|h| crate::core::bug_book::csv_escape(h))
        .collect::<Vec<_>>()
        .join(",");

    let csv = format!("{header_row}\n");
    serde_json::to_string(&serde_json::json!({ "csv": csv })).map_err(|e| format!("Serialize: {e}"))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FillTemplateRequest {
    pub template_id: String,
    pub values: HashMap<String, String>,
    pub replace_all: bool,
}

#[tauri::command]
pub fn fill_template(state: State<AppState>, request: FillTemplateRequest) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;

    // RBAC: Filler or above required
    authorize(get_current_user_role(&db)?, Action::FillTemplate)
        .map_err(|e| e.to_string())?;

    let (record, bytes) = template_store::load_template_file(&db, &request.template_id)?;

    // Field presence validation: fail fast if any template placeholder lacks a value.
    let placeholders = crate::core::field_mapping::extraction::extract_placeholders_from_docx(&bytes)
        .map_err(|e| format!("Extract placeholders: {e}"))?;
    let missing: Vec<String> = placeholders
        .iter()
        .filter(|p| !request.values.contains_key(*p))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "Missing required fields: {}",
            missing.join(", ")
        ));
    }

    let filled = fill_document(&bytes, &request.values, request.replace_all)?;

    // Get user/machine IDs for audit
    let user_id = get_db_current_user(&db).ok().map(|user| user.0);
    let role = get_current_user_role(&db).ok();

    // Best-effort audit log; failure must not block document generation.
    let _ = log_generation(
        &db,
        &request.template_id,
        record.current_version,
        &record.name,
        "docx",
        user_id.as_deref(),
        role.map(|r| r.to_string()).as_deref(),
    );

    let b64 = general_purpose::STANDARD.encode(&filled);

    Ok(serde_json::json!({ "docx_base64": b64 }).to_string())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchFillFromCsvRequest {
    pub template_id: String,
    pub csv: String,
    pub output_dir: String,
    pub formats: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchGeneratedFile {
    pub row: usize,
    pub filename: String,
    pub path: String,
    pub sha256: String,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchFillResult {
    pub generated: Vec<BatchGeneratedFile>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[tauri::command]
pub fn batch_fill_from_csv(
    state: State<AppState>,
    request: BatchFillFromCsvRequest,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
    authorize(get_current_user_role(&db)?, Action::FillTemplate).map_err(|e| e.to_string())?;

    let (_record, template_bytes) =
        template_store::load_template_file(&db, &request.template_id).map_err(|e| e.to_string())?;

    let mut reader = csv::Reader::from_reader(request.csv.as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| format!("CSV header parse: {e}"))?
        .iter()
        .map(|h| h.trim().to_string())
        .collect();

    let output_filename_idx = headers.iter().position(|h| h == "output_filename");
    let field_headers: Vec<String> = headers
        .iter()
        .filter(|h| **h != "output_filename")
        .cloned()
        .collect();

    std::fs::create_dir_all(&request.output_dir)
        .map_err(|e| format!("Create output dir: {e}"))?;

    let mut generated = Vec::new();
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let mut row_index = 0usize;

    for result in reader.records() {
        row_index += 1;
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("Row {row_index}: CSV parse error: {e}"));
                continue;
            }
        };

        let mut values = HashMap::new();
        for (i, header) in field_headers.iter().enumerate() {
            if let Some(value) = record.get(i) {
                values.insert(header.clone(), value.to_string());
            }
        }

        let filename = if let Some(idx) = output_filename_idx {
            if let Some(value) = record.get(idx) {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    format!("{}_row_{}.docx", request.template_id, row_index)
                } else {
                    format!("{trimmed}.docx")
                }
            } else {
                format!("{}_row_{}.docx", request.template_id, row_index)
            }
        } else {
            format!("{}_row_{}.docx", request.template_id, row_index)
        };

        let filled = match fill_document(&template_bytes, &values, true) {
            Ok(b) => b,
            Err(e) => {
                errors.push(format!("Row {row_index}: fill error: {e}"));
                continue;
            }
        };

        let docx_path = std::path::PathBuf::from(&request.output_dir).join(&filename);
        let sha256 = {
            let mut hasher = Sha256::new();
            hasher.update(&filled);
            let result = hasher.finalize();
            result.iter().map(|b| format!("{b:02x}")).collect()
        };

        if let Err(e) = std::fs::write(&docx_path, &filled) {
            errors.push(format!("Row {row_index}: write error: {e}"));
            continue;
        }

        let gen = BatchGeneratedFile {
            row: row_index,
            filename: filename.clone(),
            path: docx_path.to_string_lossy().to_string(),
            sha256,
            status: "success".to_string(),
            error: None,
        };

        if request.formats.contains(&"pdf".to_string()) {
            match export_pdf_from_docx(&filled) {
                Ok(pdf_bytes) => {
                    let pdf_filename = filename.replace(".docx", ".pdf");
                    let pdf_path = std::path::PathBuf::from(&request.output_dir).join(&pdf_filename);
                    if let Err(e) = std::fs::write(&pdf_path, &pdf_bytes) {
                        warnings.push(format!("Row {row_index}: PDF write error: {e}"));
                    } else {
                        warnings.push(format!("Row {row_index}: PDF exported to {}", pdf_path.display()));
                    }
                }
                Err(e) => {
                    warnings.push(format!("Row {row_index}: Native PDF export failed: {e}"));
                }
            }
        }

        generated.push(gen);
    }

    let result = BatchFillResult {
        generated,
        warnings,
        errors,
    };

    serde_json::to_string(&result).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn delete_template(state: State<AppState>, template_id: String) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;

    // RBAC: Admin only
    authorize(get_current_user_role(&db)?, Action::DeleteTemplate)
        .map_err(|e| e.to_string())?;

    template_store::delete_template(&db, &template_id)?;

    Ok(serde_json::json!({ "success": true }).to_string())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPdfRequest {
    pub docx_base64: String,
    pub output_filename: String,
}

/// PDF export. Uses the native Rust converter (`export_pdf_from_docx`) as the primary
/// engine so PDF export works with zero external dependencies. Optionally uses
/// LibreOffice for higher-fidelity layout if available and explicitly requested.
#[tauri::command]
pub fn export_to_pdf(request: ExportPdfRequest) -> Result<String, String> {
    let bytes = general_purpose::STANDARD
        .decode(&request.docx_base64)
        .map_err(|e| format!("Invalid base64: {e}"))?;

    // Primary: native Rust PDF engine (docx-rs + printpdf) - works offline, no dependencies
    match crate::core::export::export_pdf_from_docx(&bytes) {
        Ok(pdf_bytes) => {
            let pdf_b64 = general_purpose::STANDARD.encode(&pdf_bytes);
            Ok(serde_json::json!({
                "pdf_base64": pdf_b64,
                "filename": format!("{}.pdf", request.output_filename),
                "engine": "native",
                "note": "Native Rust PDF engine (plain text layout). For higher fidelity, install LibreOffice and enable the 'prefer_libreoffice' option."
            })
            .to_string())
        }
        Err(e) => {
            // Fallback: try LibreOffice if native engine fails (should be rare)
            let temp_dir = std::env::temp_dir().join("docforge");
            std::fs::create_dir_all(&temp_dir).map_err(|e| format!("Create temp dir: {e}"))?;

            let docx_path = temp_dir.join(format!("{}.docx", Uuid::new_v4()));
            std::fs::write(&docx_path, &bytes).map_err(|e| format!("Write temp docx: {e}"))?;

            let result = match find_soffice() {
                Some(soffice) => {
                    let output = run_with_timeout(
                        &soffice,
                        &[
                            "--headless",
                            "--convert-to",
                            "pdf",
                            "--outdir",
                            temp_dir.to_str().unwrap_or("."),
                            docx_path.to_str().unwrap_or(""),
                        ],
                        Duration::from_secs(120),
                    );
                    match output {
                        Ok(status) if status.success() => {
                            let generated_pdf = temp_dir.join(
                                docx_path
                                    .file_stem()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string()
                                    + ".pdf",
                            );
                            match std::fs::read(&generated_pdf) {
                                Ok(pdf_bytes) => {
                                    let pdf_b64 = general_purpose::STANDARD.encode(&pdf_bytes);
                                    let _ = std::fs::remove_file(&generated_pdf);
                                    let _ = std::fs::remove_file(&docx_path);
                                    Ok(serde_json::json!({
                                        "pdf_base64": pdf_b64,
                                        "filename": format!("{}.pdf", request.output_filename),
                                        "engine": "libreoffice",
                                    })
                                    .to_string())
                                }
                                Err(e) => Err(format!("LibreOffice fallback failed to read PDF: {e}")),
                            }
                        }
                        _ => Err("LibreOffice fallback conversion failed".to_string()),
                    }
                }
                None => Err(format!(
                    "Native PDF engine failed: {e}. LibreOffice not installed. Install LibreOffice for fallback."
                )),
            };

            let _ = std::fs::remove_file(&docx_path);
            result
        }
}
}

/// Prefer known absolute LibreOffice path to avoid PATH-based executable hijacking.
fn find_soffice() -> Option<String> {
    let candidates: [String; 3] = [
        "C:\\Program Files\\LibreOffice\\program\\soffice.exe".to_string(),
        "C:\\Program Files (x86)\\LibreOffice\\program\\soffice.exe".to_string(),
        "soffice".to_string(),
    ];
    for c in candidates {
        if c == "soffice" {
            if std::process::Command::new(&c)
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
            {
                return Some(c);
            }
        } else if std::path::Path::new(&c).exists() {
            return Some(c);
        }
    }
    // No PATH-based fallback to avoid executable hijacking
    None
}

// â”€â”€ Database backup / restore â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Copies the active SQLite database file to a user-chosen backup location.
#[tauri::command]
pub fn backup_database(state: State<AppState>, target_path: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // RBAC: Admin only
    authorize(get_current_user_role(&db)?, Action::BackupDatabase)
        .map_err(|e| e.to_string())?;

    let db_path = get_db_path();
    if !db_path.exists() {
        return Err("Database file not found".to_string());
    }
    std::fs::copy(&db_path, &target_path).map_err(|e| format!("Backup failed: {e}"))?;
    Ok(())
}

/// Replaces the active database with a previously created backup, then re-opens it.
#[tauri::command]
pub fn restore_database(state: State<AppState>, source_path: String) -> Result<(), String> {
    // RBAC: Admin only — check FIRST using the current (legitimate) DB.
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        authorize(get_current_user_role(&db)?, Action::RestoreDatabase)
            .map_err(|e| e.to_string())?;
    } // release lock before I/O

    if !std::path::Path::new(&source_path).exists() {
        return Err("Source backup file not found".to_string());
    }

    // Validate SQLite magic bytes (first 6 bytes = "SQLite").
    let header = std::fs::read(&source_path)
        .map_err(|e| format!("Cannot read backup file: {e}"))?;
    if !header.starts_with(b"SQLite") {
        return Err("Invalid SQLite backup file (bad magic bytes)".to_string());
    }

    let db_path = get_db_path();
    std::fs::copy(&source_path, &db_path).map_err(|e| format!("Restore failed: {e}"))?;
    let new_conn = init_db().map_err(|e| format!("Re-init failed: {e}"))?;
    *state.db.lock().map_err(|e| e.to_string())? = new_conn;

    Ok(())
}

/// Deletes the active database (dangerous operation).
#[tauri::command]
pub fn delete_database(state: State<AppState>) -> Result<(), String> {
    // RBAC: Admin only — check FIRST before any destructive I/O.
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        authorize(get_current_user_role(&db)?, Action::DeleteDatabase)
            .map_err(|e| e.to_string())?;
    } // release lock before file deletion

    let db_path = get_db_path();
    if db_path.exists() {
        std::fs::remove_file(&db_path).map_err(|e| format!("Delete failed: {e}"))?;
    }

    Ok(())
}

// â”€â”€ Template Bundles â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBundleRequest {
    pub name: String,
    pub description: Option<String>,
    pub template_ids: Vec<String>,
}

#[tauri::command]
pub fn create_bundle_cmd(
    state: State<AppState>,
    request: CreateBundleRequest,
) -> Result<String, String> {
    if request.name.trim().is_empty() {
        return Err("Bundle name cannot be empty".to_string());
    }
    if request.template_ids.is_empty() {
        return Err("Bundle must contain at least one template".to_string());
    }

    let db = state.db.lock().map_err(|e| e.to_string())?;

    // RBAC: Creator or Admin required
    authorize(get_current_user_role(&db)?, Action::CreateBundle)
        .map_err(|e| e.to_string())?;

    create_bundle(&db, &request.name, request.description.as_deref(), &request.template_ids)
        .map_err(|e| format!("Create bundle failed: {e}"))
}

#[tauri::command]
pub fn list_bundles_cmd(state: State<AppState>) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let bundles = list_bundles(&db).map_err(|e| format!("List bundles failed: {e}"))?;
    serde_json::to_string(&bundles).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn get_bundle_templates_cmd(state: State<AppState>, bundle_id: String) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let ids = get_bundle_templates(&db, &bundle_id).map_err(|e| format!("Get bundle failed: {e}"))?;
    serde_json::to_string(&ids).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn delete_bundle_cmd(state: State<AppState>, bundle_id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // RBAC: Admin only
    authorize(get_current_user_role(&db)?, Action::DeleteBundle)
        .map_err(|e| e.to_string())?;

    delete_bundle(&db, &bundle_id).map_err(|e| format!("Delete bundle failed: {e}"))
}

#[tauri::command]
pub fn add_template_to_bundle_cmd(
    state: State<AppState>,
    bundle_id: String,
    template_id: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // RBAC: Creator or Admin required
    authorize(get_current_user_role(&db)?, Action::CreateTemplate)
        .map_err(|e| e.to_string())?;

    add_template_to_bundle(&db, &bundle_id, &template_id)
        .map_err(|e| format!("Add to bundle failed: {e}"))
}

#[tauri::command]
pub fn remove_template_from_bundle_cmd(
    state: State<AppState>,
    bundle_id: String,
    template_id: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // RBAC: Creator or Admin required
    authorize(get_current_user_role(&db)?, Action::CreateTemplate)
        .map_err(|e| e.to_string())?;

    remove_template_from_bundle(&db, &bundle_id, &template_id)
        .map_err(|e| format!("Remove from bundle failed: {e}"))
}

// ============================================================================
// v2 Bundle + Matter domain commands
// ============================================================================

// --- Bundle v2 ---

#[tauri::command]
pub fn create_bundle_v2_cmd(
    state: State<AppState>,
    request: CreateBundleRequest,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::CreateBundle)
        .map_err(|e| e.to_string())?;
    let record = create_bundle_v2(&db, &request.name, request.description.as_deref(), None)
        .map_err(|e| format!("Create bundle v2 failed: {e}"))?;
    serde_json::to_string(&record).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn list_bundles_v2_cmd(state: State<AppState>) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let bundles = list_bundles_v2(&db).map_err(|e| format!("List bundles v2 failed: {e}"))?;
    serde_json::to_string(&bundles).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn get_bundle_v2_cmd(state: State<AppState>, bundle_id: String) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let detail = get_bundle_v2(&db, &bundle_id).map_err(|e| format!("Get bundle v2 failed: {e}"))?;
    serde_json::to_string(&detail).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn create_draft_version_cmd(
    state: State<AppState>,
    bundle_id: String,
    note: Option<String>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::CreateTemplate)
        .map_err(|e| e.to_string())?;
    let version = create_draft_version(&db, &bundle_id, note.as_deref())
        .map_err(|e| format!("Create draft version failed: {e}"))?;
    serde_json::to_string(&version).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn publish_version_cmd(
    state: State<AppState>,
    bundle_version_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::ApproveTemplate)
        .map_err(|e| e.to_string())?;
    publish_version(&db, &bundle_version_id)
        .map_err(|e| format!("Publish version failed: {e}"))?;
    Ok(serde_json::json!({"success": true}).to_string())
}

#[tauri::command]
pub fn review_version_cmd(
    state: State<AppState>,
    bundle_version_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::ApproveTemplate)
        .map_err(|e| e.to_string())?;
    review_version(&db, &bundle_version_id)
        .map_err(|e| format!("Review version failed: {e}"))?;
    Ok(serde_json::json!({"success": true}).to_string())
}

#[tauri::command]
pub fn archive_version_cmd(
    state: State<AppState>,
    bundle_version_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::ApproveTemplate)
        .map_err(|e| e.to_string())?;
    archive_version(&db, &bundle_version_id)
        .map_err(|e| format!("Archive version failed: {e}"))?;
    Ok(serde_json::json!({"success": true}).to_string())
}

#[tauri::command]
pub fn list_versions_cmd(
    state: State<AppState>,
    bundle_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let versions = list_versions(&db, &bundle_id).map_err(|e| format!("List versions failed: {e}"))?;
    serde_json::to_string(&versions).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn get_manifest_cmd(
    state: State<AppState>,
    bundle_version_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let manifest = get_manifest(&db, &bundle_version_id)
        .map_err(|e| format!("Get manifest failed: {e}"))?;
    serde_json::to_string(&manifest).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn save_manifest_cmd(
    state: State<AppState>,
    bundle_version_id: String,
    manifest: BundleManifest,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::CreateTemplate)
        .map_err(|e| e.to_string())?;
    save_manifest(&db, &bundle_version_id, &manifest)
        .map_err(|e| format!("Save manifest failed: {e}"))?;
    Ok(serde_json::json!({"success": true}).to_string())
}

#[tauri::command]
pub fn export_bundle_dfpkg_cmd(
    state: State<AppState>,
    bundle_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::CreateBundle)
        .map_err(|e| e.to_string())?;
    let bytes = export_bundle_dfpkg(&db, &bundle_id)
        .map_err(|e| format!("Export bundle dfpkg failed: {e}"))?;
    let b64 = general_purpose::STANDARD.encode(&bytes);
    Ok(serde_json::json!({"dfpkg_base64": b64, "filename": format!("{bundle_id}.dfpkg")}).to_string())
}

#[tauri::command]
pub fn import_bundle_dfpkg_cmd(
    state: State<AppState>,
    dfpkg_base64: String,
) -> Result<String, String> {
    let mut db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::CreateBundle)
        .map_err(|e| e.to_string())?;
    let bytes = general_purpose::STANDARD.decode(&dfpkg_base64)
        .map_err(|e| format!("Invalid base64: {e}"))?;
    let result = import_bundle_dfpkg(&mut db, &bytes)
        .map_err(|e| format!("Import bundle dfpkg failed: {e}"))?;
    serde_json::to_string(&result).map_err(|e| format!("Serialize: {e}"))
}

// --- Field Mapping ---

#[tauri::command]
pub fn create_field_cmd(
    state: State<AppState>,
    bundle_version_id: String,
    field: FieldDef,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::CreateTemplate)
        .map_err(|e| e.to_string())?;
    let created = create_field(&db, &bundle_version_id, &field)
        .map_err(|e| format!("Create field failed: {e}"))?;
    serde_json::to_string(&created).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn update_field_cmd(
    state: State<AppState>,
    field_db_id: String,
    field: FieldDef,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::CreateTemplate)
        .map_err(|e| e.to_string())?;
    let updated = update_field(&db, &field_db_id, &field)
        .map_err(|e| format!("Update field failed: {e}"))?;
    serde_json::to_string(&updated).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn list_fields_cmd(
    state: State<AppState>,
    bundle_version_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let fields = list_fields(&db, &bundle_version_id)
        .map_err(|e| format!("List fields failed: {e}"))?;
    serde_json::to_string(&fields).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn remove_field_cmd(
    state: State<AppState>,
    field_db_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::CreateTemplate)
        .map_err(|e| e.to_string())?;
    remove_field(&db, &field_db_id)
        .map_err(|e| format!("Remove field failed: {e}"))?;
    Ok(serde_json::json!({"success": true}).to_string())
}

#[tauri::command]
pub fn create_field_group_cmd(
    state: State<AppState>,
    bundle_version_id: Option<String>,
    group: FieldGroup,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::CreateTemplate)
        .map_err(|e| e.to_string())?;
    let created = create_field_group(&db, bundle_version_id.as_deref(), &group)
        .map_err(|e| format!("Create field group failed: {e}"))?;
    serde_json::to_string(&created).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn list_field_groups_cmd(
    state: State<AppState>,
    bundle_version_id: Option<String>,
    scope: Option<String>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let group_scope = scope.as_deref().and_then(|s| s.parse::<GroupScope>().ok());
    let groups = list_field_groups(&db, bundle_version_id.as_deref(), group_scope)
        .map_err(|e| format!("List field groups failed: {e}"))?;
    serde_json::to_string(&groups).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn create_group_cmd(
    state: State<AppState>,
    bundle_version_id: Option<String>,
    name: String,
    scope: String,
    description: Option<String>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::CreateTemplate)
        .map_err(|e| e.to_string())?;
    let group_scope: GroupScope = scope.parse().map_err(|e| format!("Invalid scope: {e}"))?;
    let created = create_group(&db, bundle_version_id.as_deref(), &name, group_scope, description.as_deref())
        .map_err(|e| format!("Create group failed: {e}"))?;
    serde_json::to_string(&created).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn list_groups_shared_first_cmd(
    state: State<AppState>,
    bundle_version_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let groups = list_groups_with_shared_first(&db, &bundle_version_id)
        .map_err(|e| format!("List groups failed: {e}"))?;
    serde_json::to_string(&groups).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn assign_field_to_group_cmd(
    state: State<AppState>,
    field_db_id: String,
    group_id: Option<String>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::CreateTemplate)
        .map_err(|e| e.to_string())?;
    assign_field_to_group(&db, &field_db_id, group_id.as_deref())
        .map_err(|e| format!("Assign field to group failed: {e}"))?;
    Ok(serde_json::json!({"success": true}).to_string())
}

#[tauri::command]
pub fn group_summary_cmd(
    state: State<AppState>,
    bundle_version_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let summary = group_summary(&db, &bundle_version_id)
        .map_err(|e| format!("Group summary failed: {e}"))?;
    serde_json::to_string(&summary).map_err(|e| format!("Serialize: {e}"))
}

// --- Field Mappings ---

#[tauri::command]
pub fn set_mapping_cmd(
    state: State<AppState>,
    bundle_version_id: String,
    document_id: String,
    placeholder: String,
    canonical_field_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::CreateTemplate)
        .map_err(|e| e.to_string())?;
    let mapping = set_mapping(&db, &bundle_version_id, &document_id, &placeholder, &canonical_field_id)
        .map_err(|e| format!("Set mapping failed: {e}"))?;
    serde_json::to_string(&mapping).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn list_mappings_cmd(
    state: State<AppState>,
    bundle_version_id: String,
    document_id: Option<String>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mappings = list_mappings(&db, &bundle_version_id, document_id.as_deref())
        .map_err(|e| format!("List mappings failed: {e}"))?;
    serde_json::to_string(&mappings).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn find_unmapped_placeholders_cmd(
    state: State<AppState>,
    bundle_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    
    // Get the latest/active version for this bundle
    let versions = list_versions(&db, &bundle_id)
        .map_err(|e| format!("List versions failed: {e}"))?;
    
    let bundle_version_id = versions
        .into_iter()
        .find(|v| v.status == "published" || v.status == "draft")
        .map(|v| v.id)
        .ok_or_else(|| "No active bundle version found".to_string())?;
    
    let unmapped = find_unmapped_placeholders(&db, &bundle_version_id)
        .map_err(|e| format!("Find unmapped placeholders failed: {e}"))?;
    serde_json::to_string(&unmapped).map_err(|e| format!("Serialize: {e}"))
}

// --- Matter ---

#[tauri::command]
pub fn create_matter_cmd(
    state: State<AppState>,
    bundle_id: String,
    bundle_version_id: String,
    name: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::CreateTemplate)
        .map_err(|e| e.to_string())?;
    let matter = create_matter(&db, &bundle_id, &bundle_version_id, &name, None, None)
        .map_err(|e| format!("Create matter failed: {e}"))?;
    serde_json::to_string(&matter).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn get_matter_cmd(
    state: State<AppState>,
    matter_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let matter = get_matter(&db, &matter_id)
        .map_err(|e| format!("Get matter failed: {e}"))?;
    serde_json::to_string(&matter).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn list_matters_cmd(
    state: State<AppState>,
    bundle_version_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let matters = list_matters(&db, &bundle_version_id)
        .map_err(|e| format!("List matters failed: {e}"))?;
    serde_json::to_string(&matters).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn update_matter_status_cmd(
    state: State<AppState>,
    matter_id: String,
    status: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let matter_status: MatterStatus = status.parse().map_err(|e| format!("Invalid status: {e}"))?;
    let matter = update_matter_status(&db, &matter_id, matter_status)
        .map_err(|e| format!("Update matter status failed: {e}"))?;
    serde_json::to_string(&matter).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn delete_matter_cmd(
    state: State<AppState>,
    matter_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::DeleteTemplate)
        .map_err(|e| e.to_string())?;
    delete_matter(&db, &matter_id)
        .map_err(|e| format!("Delete matter failed: {e}"))?;
    Ok(serde_json::json!({"success": true}).to_string())
}

// --- Matter Values ---

#[tauri::command]
pub fn set_matter_value_cmd(
    state: State<AppState>,
    matter_id: String,
    canonical_field_id: String,
    value: serde_json::Value,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mv = set_matter_value(&db, &matter_id, &canonical_field_id, &value)
        .map_err(|e| format!("Set matter value failed: {e}"))?;
    serde_json::to_string(&mv).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn get_matter_value_cmd(
    state: State<AppState>,
    matter_id: String,
    canonical_field_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mv = get_matter_value(&db, &matter_id, &canonical_field_id)
        .map_err(|e| format!("Get matter value failed: {e}"))?;
    serde_json::to_string(&mv).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn list_matter_values_cmd(
    state: State<AppState>,
    matter_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let values = list_matter_values(&db, &matter_id)
        .map_err(|e| format!("List matter values failed: {e}"))?;
    serde_json::to_string(&values).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn matter_to_json_cmd(
    state: State<AppState>,
    matter_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let json = matter_to_json(&db, &matter_id)
        .map_err(|e| format!("Matter to json failed: {e}"))?;
    serde_json::to_string(&json).map_err(|e| format!("Serialize: {e}"))
}

// --- Matter Form ---

#[tauri::command]
pub fn render_matter_form_cmd(
    state: State<AppState>,
    matter_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let form = render_matter_form(&db, &matter_id)
        .map_err(|e| format!("Render matter form failed: {e}"))?;
    serde_json::to_string(&form).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn populate_matter_field_cmd(
    state: State<AppState>,
    matter_id: String,
    field_id: String,
    raw_value: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let ff = populate_matter_field(&db, &matter_id, &field_id, &raw_value)
        .map_err(|e| format!("Populate matter field failed: {e}"))?;
    serde_json::to_string(&ff).map_err(|e| format!("Serialize: {e}"))
}

// --- Matter Validation ---

#[tauri::command]
pub fn validate_matter_cmd(
    state: State<AppState>,
    matter_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let report = validate_matter(&db, &matter_id)
        .map_err(|e| format!("Validate matter failed: {e}"))?;
    serde_json::to_string(&report).map_err(|e| format!("Serialize: {e}"))
}

// --- Rules ---

#[tauri::command]
pub fn add_rule_cmd(
    state: State<AppState>,
    bundle_version_id: String,
    document_id: String,
    condition_expr: String,
    description: Option<String>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::CreateTemplate)
        .map_err(|e| e.to_string())?;
    let rule = add_rule(&db, &bundle_version_id, &document_id, &condition_expr, description.as_deref())
        .map_err(|e| format!("Add rule failed: {e}"))?;
    serde_json::to_string(&rule).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn remove_rule_cmd(
    state: State<AppState>,
    rule_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::CreateTemplate)
        .map_err(|e| e.to_string())?;
    remove_rule(&db, &rule_id)
        .map_err(|e| format!("Remove rule failed: {e}"))?;
    Ok(serde_json::json!({"success": true}).to_string())
}

#[tauri::command]
pub fn list_rules_cmd(
    state: State<AppState>,
    bundle_version_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let rules = list_rules(&db, &bundle_version_id)
        .map_err(|e| format!("List rules failed: {e}"))?;
    serde_json::to_string(&rules).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn evaluate_rules_cmd(
    state: State<AppState>,
    bundle_version_id: String,
    matter_data: serde_json::Value,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let decisions = evaluate_rules(&db, &bundle_version_id, &matter_data)
        .map_err(|e| format!("Evaluate rules failed: {e}"))?;
    serde_json::to_string(&decisions).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn evaluate_preview_cmd(
    state: State<AppState>,
    matter_id: String,
    _document_ids: Option<Vec<String>>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    
    // Get matter to retrieve bundle_version_id
    let matter = get_matter(&db, &matter_id)
        .map_err(|e| format!("Get matter failed: {e}"))?
        .ok_or_else(|| format!("Matter '{}' not found", matter_id))?;
    
    // Get matter data as JSON
    let matter_data = matter_to_json(&db, &matter_id)
        .map_err(|e| format!("Get matter data failed: {e}"))?;
    
    let preview = evaluate_preview(&db, &matter.bundle_version_id, &matter_data)
        .map_err(|e| format!("Evaluate preview failed: {e}"))?;
    
    // TODO: Apply document_ids filtering if needed in future
    serde_json::to_string(&preview).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn validate_rule_expression_cmd(
    state: State<AppState>,
    bundle_version_id: String,
    expression: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let refs = validate_rule_expression(&db, &bundle_version_id, &expression)
        .map_err(|e| format!("Validate rule expression failed: {e}"))?;
    serde_json::to_string(&refs).map_err(|e| format!("Serialize: {e}"))
}

// --- Generation Run ---

#[tauri::command]
pub fn execute_run_cmd(
    state: State<AppState>,
    matter_id: String,
    _document_ids: Option<Vec<String>>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    authorize(get_current_user_role(&db)?, Action::FillTemplate)
        .map_err(|e| e.to_string())?;
    
    // Use temp directory for output (frontend will handle downloads)
    let output_root = std::env::temp_dir().join("docforge_generation");
    std::fs::create_dir_all(&output_root).map_err(|e| format!("Create output dir: {e}"))?;
    
    // TODO: Pass document_ids filtering to execute_run when supported
    let result = execute_run(&db, &matter_id, &output_root, None)
        .map_err(|e| format!("Execute run failed: {e}"))?;
    serde_json::to_string(&result).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn create_run_cmd(
    state: State<AppState>,
    matter_id: String,
    bundle_id: String,
    bundle_version_id: String,
    status: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let run_status: RunStatus = status.parse().map_err(|e| format!("Invalid status: {e}"))?;
    let run = create_run(&db, &matter_id, &bundle_id, &bundle_version_id, None, None, None, run_status)
        .map_err(|e| format!("Create run failed: {e}"))?;
    serde_json::to_string(&run).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn get_run_cmd(
    state: State<AppState>,
    run_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let run = get_run(&db, &run_id)
        .map_err(|e| format!("Get run failed: {e}"))?;
    serde_json::to_string(&run).map_err(|e| format!("Serialize: {e}"))
}

#[tauri::command]
pub fn list_runs_cmd(
    state: State<AppState>,
    matter_id: String,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let runs = list_runs(&db, &matter_id)
        .map_err(|e| format!("List runs failed: {e}"))?;
    serde_json::to_string(&runs).map_err(|e| format!("Serialize: {e}"))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogBugRequest {
    pub error_type: String,
    pub message: String,
    pub stack_trace: String,
    pub severity: Option<String>,
    pub context: Option<String>,
    pub category: Option<String>,
    pub source: Option<String>,
}

/// Records an automatically captured runtime error/crash. Defaults severity to
/// `high`, source to `auto`. Critical entries trigger team notifications.
#[tauri::command]
pub fn log_bug(state: State<AppState>, request: LogBugRequest) -> Result<String, String> {
    let severity = request.severity.unwrap_or_else(|| "high".to_string());
    let source = request.source.unwrap_or_else(|| "auto".to_string());

    let db = state.db.lock().map_err(|e| e.to_string())?;

    // RBAC: Anyone can log bugs, but critical ones need approval for webhook
    authorize(get_current_user_role(&db)?, Action::ManageBugs)
        .map_err(|e| e.to_string())?;

    let entry = crate::core::bug_book::create_bug(
        &db,
        &crate::core::bug_book::NewBug {
            error_type: request.error_type,
            severity,
            status: "open".to_string(),
            context: request.context.unwrap_or_default(),
            message: request.message,
            stack_trace: request.stack_trace,
            source,
            category: request.category.unwrap_or_else(|| "uncategorized".to_string()),
            keywords: String::new(),
        },
    )
    .map_err(|e| e.to_string())?;

    if entry.severity == "critical" {
        let payload = serde_json::to_string(&entry).unwrap_or_default();
        let _ = crate::services::webhook::dispatch_webhook_event(&db, "bug.critical", &payload);
    }

    Ok(serde_json::json!({ "id": entry.id, "success": true }).to_string())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBugRequest {
    pub error_type: String,
    pub message: String,
    pub severity: String,
    pub status: Option<String>,
    pub context: Option<String>,
    pub stack_trace: Option<String>,
    pub category: Option<String>,
    pub keywords: Option<String>,
}

/// Creates a manual bug entry from the Admin Console.
#[tauri::command]
pub fn create_bug_entry(state: State<AppState>, request: CreateBugRequest) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // RBAC: Approver or Admin
    authorize(get_current_user_role(&db)?, Action::ManageBugs)
        .map_err(|e| e.to_string())?;

    let entry = crate::core::bug_book::create_bug(
        &db,
        &crate::core::bug_book::NewBug {
            error_type: request.error_type,
            severity: request.severity,
            status: request.status.unwrap_or_else(|| "open".to_string()),
            context: request.context.unwrap_or_default(),
            message: request.message,
            stack_trace: request.stack_trace.unwrap_or_default(),
            source: "manual".to_string(),
            category: request.category.unwrap_or_else(|| "uncategorized".to_string()),
            keywords: request.keywords.unwrap_or_default(),
        },
    )
    .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "id": entry.id, "success": true }).to_string())
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListBugsRequest {
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub severity: Option<String>,
    pub status: Option<String>,
    pub keyword: Option<String>,
    pub sort_by: Option<String>,
    pub sort_dir: Option<String>,
    pub limit: Option<u32>,
}

/// Lists bug entries matching the supplied filters/sort criteria.
#[tauri::command]
pub fn list_bugs(state: State<AppState>, request: ListBugsRequest) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // RBAC: Approver or Admin for listing bugs
    authorize(get_current_user_role(&db)?, Action::ManageBugs)
        .map_err(|e| e.to_string())?;

    let filter = crate::core::bug_book::BugFilter {
        date_from: request.date_from,
        date_to: request.date_to,
        severity: request.severity,
        status: request.status,
        keyword: request.keyword,
        sort_by: request.sort_by.unwrap_or_else(|| "created_at".to_string()),
        sort_dir: request.sort_dir.unwrap_or_else(|| "desc".to_string()),
        limit: request.limit,
    };
    let bugs = crate::core::bug_book::list_bugs(&db, &filter).map_err(|e| e.to_string())?;
    serde_json::to_string(&bugs).map_err(|e| e.to_string())
}

/// Fetches a single bug entry (with attachments) by id.
#[tauri::command]
pub fn get_bug(state: State<AppState>, bug_id: String) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // RBAC: Approver or Admin
    authorize(get_current_user_role(&db)?, Action::ManageBugs)
        .map_err(|e| e.to_string())?;

    let entry = crate::core::bug_book::get_bug(&db, &bug_id).map_err(|e| e.to_string())?;
    serde_json::to_string(&entry).map_err(|e| e.to_string())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBugStatusRequest {
    pub bug_id: String,
    pub status: String,
    pub resolved_by: Option<String>,
}

/// Updates a bug entry's lifecycle status (open / in_progress / resolved / wont_fix).
#[tauri::command]
pub fn update_bug_status(
    state: State<AppState>,
    request: UpdateBugStatusRequest,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // RBAC: Approver or Admin
    authorize(get_current_user_role(&db)?, Action::ManageBugs)
        .map_err(|e| e.to_string())?;

    crate::core::bug_book::update_bug_status(&db, &request.bug_id, &request.status, request.resolved_by)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "success": true }).to_string())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddBugAttachmentRequest {
    pub bug_id: String,
    pub filename: String,
    pub mime_type: String,
    pub data_b64: String,
}

/// Attaches a supplementary log/screenshot to a bug entry.
#[tauri::command]
pub fn add_bug_attachment(
    state: State<AppState>,
    request: AddBugAttachmentRequest,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // RBAC: Approver or Admin
    authorize(get_current_user_role(&db)?, Action::ManageBugs)
        .map_err(|e| e.to_string())?;

    let att = crate::core::bug_book::add_attachment(
        &db,
        &request.bug_id,
        &request.filename,
        &request.mime_type,
        &request.data_b64,
    )
    .map_err(|e| e.to_string())?;
    serde_json::to_string(&att).map_err(|e| e.to_string())
}

/// Exports the filtered bug list to CSV.
#[tauri::command]
pub fn export_bugs_csv(state: State<AppState>, request: ListBugsRequest) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // RBAC: Approver or Admin for export
    authorize(get_current_user_role(&db)?, Action::ExportBugs)
        .map_err(|e| e.to_string())?;

    let filter = crate::core::bug_book::BugFilter {
        date_from: request.date_from,
        date_to: request.date_to,
        severity: request.severity,
        status: request.status,
        keyword: request.keyword,
        sort_by: request.sort_by.unwrap_or_else(|| "created_at".to_string()),
        sort_dir: request.sort_dir.unwrap_or_else(|| "desc".to_string()),
        limit: request.limit,
    };
    let csv = crate::core::bug_book::export_bugs_csv(&db, &filter).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "csv": csv }).to_string())
}

/// Exports the filtered bug list to a PDF report (native, no external deps).
#[tauri::command]
pub fn export_bugs_pdf(state: State<AppState>, request: ListBugsRequest) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // RBAC: Approver or Admin for export
    authorize(get_current_user_role(&db)?, Action::ExportBugs)
        .map_err(|e| e.to_string())?;

    let filter = crate::core::bug_book::BugFilter {
        date_from: request.date_from,
        date_to: request.date_to,
        severity: request.severity,
        status: request.status,
        keyword: request.keyword,
        sort_by: request.sort_by.unwrap_or_else(|| "created_at".to_string()),
        sort_dir: request.sort_dir.unwrap_or_else(|| "desc".to_string()),
        limit: request.limit,
    };
    let pdf = crate::core::bug_book::export_bugs_pdf(&db, &filter).map_err(|e| e.to_string())?;
    let pdf_b64 = general_purpose::STANDARD.encode(&pdf);
    Ok(serde_json::json!({ "pdf_base64": pdf_b64, "filename": "bug-book-report.pdf" }).to_string())
}

/// Gets the current user info.
#[tauri::command]
pub fn get_current_user(state: State<AppState>) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let user_info = get_db_current_user(&db).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "id": user_info.0,
        "role": user_info.1,
        "name": user_info.2,
        "email": user_info.3
    }).to_string())
}

/// Sets the current user's role (Admin only).
#[tauri::command]
pub fn set_user_role(state: State<AppState>, role: String) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // RBAC: Admin only for setting roles
    authorize(get_current_user_role(&db)?, Action::ManageUsers)
        .map_err(|e| e.to_string())?;

    let new_role = role.parse().map_err(|e| format!("Invalid role: {e}"))?;
    set_current_user_role(&db, new_role).map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "success": true }).to_string())
}

#[tauri::command]
pub fn get_telemetry_consent(state: State<AppState>) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let consent = crate::services::telemetry::TelemetryService::get_consent(&db)
        .map_err(|e| e.to_string())?;
    serde_json::to_string(&consent).map_err(|e| e.to_string())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetTelemetryConsentRequest {
    pub opt_in: bool,
    pub crash_reports: bool,
}

#[tauri::command]
pub fn set_telemetry_consent(
    state: State<AppState>,
    request: SetTelemetryConsentRequest,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    crate::services::telemetry::TelemetryService::set_consent(
        &db,
        request.opt_in,
        request.crash_reports,
    )
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "success": true }).to_string())
}

/// Runs a command and returns its exit status, or an error if it exceeds `timeout`.
fn run_with_timeout(
    program: &str,
    args: &[&str],
    timeout: Duration,
) -> Result<std::process::ExitStatus, String> {
    let mut child = std::process::Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to launch {program}: {e}"))?;

    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let res = child.wait();
        let _ = tx.send(res);
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(status)) => Ok(status),
        Ok(Err(e)) => Err(format!("Process error: {e}")),
        Err(_) => {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F", "/T"])
                .output();
            Err("PDF conversion timed out".to_string())
        }
    }
}
