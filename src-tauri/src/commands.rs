use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tauri::State;
use uuid::Uuid;
use zip::ZipArchive;

use crate::AppState;
use crate::core::docx_engine::{fill_document, tag_document, TemplateFieldSpec};
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
pub struct FillTemplateRequest {
    pub template_id: String,
    pub values: HashMap<String, String>,
}

#[tauri::command]
pub fn fill_template(state: State<AppState>, request: FillTemplateRequest) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;

    // RBAC: Filler or above required
    authorize(get_current_user_role(&db)?, Action::FillTemplate)
        .map_err(|e| e.to_string())?;

    let (record, bytes) = template_store::load_template_file(&db, &request.template_id)?;

    let filled = fill_document(&bytes, &request.values)?;

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
pub struct ExportPdfRequest {
    pub docx_base64: String,
    pub output_filename: String,
}

/// PDF export. Prefers high-fidelity LibreOffice headless conversion; if LibreOffice
/// is unavailable, transparently falls back to the native Rust converter
/// (`export_pdf_from_docx`) so PDF export works with zero external dependencies.
#[tauri::command]
pub fn export_to_pdf(request: ExportPdfRequest) -> Result<String, String> {
    let bytes = general_purpose::STANDARD
        .decode(&request.docx_base64)
        .map_err(|e| format!("Invalid base64: {e}"))?;

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
                            Ok(serde_json::json!({
                                "pdf_base64": pdf_b64,
                                "filename": format!("{}.pdf", request.output_filename),
                                "engine": "libreoffice",
                            })
                            .to_string())
                        }
                        Err(e) => native_pdf_fallback(&bytes, &request.output_filename, &e.to_string()),
                    }
                }
                _ => native_pdf_fallback(&bytes, &request.output_filename, "LibreOffice conversion failed"),
            }
        }
        None => native_pdf_fallback(
            &bytes,
            &request.output_filename,
            "LibreOffice not found",
        ),
    };

    let _ = std::fs::remove_file(&docx_path);
    result
}

/// Converts the DOCX bytes to PDF natively (no LibreOffice). Returns a clear error
/// only if the native engine itself fails.
fn native_pdf_fallback(
    bytes: &[u8],
    output_filename: &str,
    reason: &str,
) -> Result<String, String> {
    match crate::core::export::export_pdf_from_docx(bytes) {
        Ok(pdf_bytes) => {
            let pdf_b64 = general_purpose::STANDARD.encode(&pdf_bytes);
            Ok(serde_json::json!({
                "pdf_base64": pdf_b64,
                "filename": format!("{}.pdf", output_filename),
                "engine": "native",
                "note": format!("Used native Rust PDF engine ({reason}). Layout is plain text.")
            })
            .to_string())
        }
        Err(e) => Err(format!(
            "LibreOffice unavailable ({reason}) and native PDF engine failed: {e}"
        )),
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

// ── Database backup / restore ────────────────────────────────────────────────

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
    let db_path = get_db_path();
    if !std::path::Path::new(&source_path).exists() {
        return Err("Source backup file not found".to_string());
    }

    // Validate that source is a valid SQLite file
    if std::fs::read_to_string(&source_path)
        .map(|content| content.starts_with("SQLite"))
        .unwrap_or(false)
    {
        return Err("Invalid SQLite backup file".to_string());
    }

    std::fs::copy(&source_path, &db_path).map_err(|e| format!("Restore failed: {e}"))?;
    let new_conn = init_db().map_err(|e| format!("Re-init failed: {e}"))?;
    *state.db.lock().map_err(|e| e.to_string())? = new_conn;

    let db = state.db.lock().map_err(|e| e.to_string())?;

    // RBAC: Admin only
    authorize(get_current_user_role(&db)?, Action::RestoreDatabase)
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Deletes the active database (dangerous operation).
#[tauri::command]
pub fn delete_database(state: State<AppState>) -> Result<(), String> {
    let db_path = get_db_path();
    if db_path.exists() {
        std::fs::remove_file(&db_path).map_err(|e| format!("Delete failed: {e}"))?;
    }

    let db = state.db.lock().map_err(|e| e.to_string())?;

    // RBAC: Admin only
    authorize(get_current_user_role(&db)?, Action::DeleteDatabase)
        .map_err(|e| e.to_string())?;

    Ok(())
}

// ── Template Bundles ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
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

// ── Bug Book (Admin Console crash/error log) ──────────────────────────────────

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