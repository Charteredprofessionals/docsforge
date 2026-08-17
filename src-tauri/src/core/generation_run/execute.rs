//! generation_run/execute.rs — Run orchestration over the verified core (TASK-117, REQ-035).
//!
//! `execute_run` is the single composition point that drives document generation:
//! it loads the Matter data, evaluates conditional-document rules to get the
//! included set, fills each included document via `docx_engine::fill_document`
//! (the only text-substitution surface), renders deterministic output filenames
//! from `OutputConfig`, writes the artifacts, records immutable
//! `generated_documents` rows (with content SHA-256), and closes the run with an
//! append-only `generation_runs` record. Supports generate-all and
//! generate-selected.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use rusqlite::Connection;
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::core::bundle::manifest::{get_manifest, BundleDocumentSpec, OutputFormat};
use crate::core::docx_engine::fill_document;
use crate::core::error::DocForgeError;
use crate::core::export::export_pdf_from_docx;
use crate::core::field_mapping::mapping::{list_mappings, resolve_value};
use crate::core::generation_run::record::{compute_input_hash, create_run, GenerationRun, RunStatus};
use crate::core::matter::matter::get_matter;
use crate::core::matter::matter_values::matter_to_json;
use crate::core::rules::evaluate::evaluate_rules;
use crate::core::template_store::load_template_file;

/// One produced artifact for a single (document, format) pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedDocument {
    pub bundle_document_id: String,
    pub document_name: String,
    pub format: String,
    pub output_path: String,
    pub content_sha256: String,
}

/// Result of a full generation run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecuteResult {
    pub run_id: String,
    pub included_document_ids: Vec<String>,
    pub generated: Vec<GeneratedDocument>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// Strips `{{` / `}}` braces from a stored placeholder so it matches the tag
/// form `fill_document` expects.
fn strip_braces(placeholder: &str) -> &str {
    let s = placeholder.strip_prefix("{{").unwrap_or(placeholder);
    s.strip_suffix("}}").unwrap_or(s)
}

/// Renders a resolved JSON value into the string form `fill_document` consumes.
fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::Null => Some(String::new()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        other => Some(serde_json::to_string(other).unwrap_or_default()),
    }
}

/// Pure resolver: builds the `placeholder_tag -> value` map for one document.
///
/// Pulls the document's field mappings, resolves each against the Matter data,
/// and returns a map keyed by the placeholder tag (braces stripped). Exercised
/// directly by tests without touching the filesystem.
pub fn resolve_document_values(
    conn: &Connection,
    bundle_version_id: &str,
    document_id: &str,
    matter_data: &Value,
) -> Result<HashMap<String, String>, DocForgeError> {
    let mut values = HashMap::new();
    let mappings = list_mappings(conn, bundle_version_id, Some(document_id))?;
    for m in &mappings {
        let resolved = resolve_value(conn, bundle_version_id, document_id, &m.placeholder, matter_data)?;
        if let Some(v) = resolved.value {
            if let Some(s) = value_to_string(&v) {
                values.insert(strip_braces(&m.placeholder).to_string(), s);
            }
        }
    }
    Ok(values)
}

fn output_formats(fmt: OutputFormat) -> Vec<&'static str> {
    match fmt {
        OutputFormat::Docx => vec!["docx"],
        OutputFormat::Pdf => vec!["pdf"],
        OutputFormat::DocxAndPdf => vec!["docx", "pdf"],
    }
}

/// Executes a generation run for a Matter.
///
/// `selected` (when `Some`) narrows the rule-included set to the named
/// documents. Outputs are written under `output_root` following `OutputConfig`
/// naming/folder policy. Returns a structured result; the run record and
/// `generated_documents` rows are persisted (append-only).
pub fn execute_run(
    conn: &Connection,
    matter_id: &str,
    output_root: &Path,
    selected: Option<&[String]>,
) -> Result<ExecuteResult, DocForgeError> {
    let matter = get_matter(conn, matter_id)?
        .ok_or_else(|| DocForgeError::StorageMissing(format!("Matter '{matter_id}' not found")))?;
    let bundle_id = matter.bundle_id.clone();
    let bv = matter.bundle_version_id.clone();

    let matter_data = matter_to_json(conn, matter_id)?;
    let snapshot = serde_json::to_string(&matter_data)
        .map_err(|e| DocForgeError::Internal(format!("Serialize matter snapshot: {e}")))?;
    let input_hash = compute_input_hash(&snapshot);

    let manifest = get_manifest(conn, &bv)?;
    let output_config = &manifest.output_config;

    let decisions = evaluate_rules(conn, &bv, &matter_data)?;

    // Eligible = rule-included; optionally narrowed to `selected`.
    let mut eligible: Vec<_> = decisions.into_iter().filter(|d| d.included).collect();
    if let Some(sel) = selected {
        eligible.retain(|d| sel.contains(&d.document_id));
    }

    let mut generated = Vec::new();
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    for decision in &eligible {
        let doc: &BundleDocumentSpec = match manifest
            .documents
            .iter()
            .find(|d| d.document_id == decision.document_id)
        {
            Some(d) => d,
            None => {
                warnings.push(format!(
                    "Document '{}' present in rules but missing from manifest; skipped",
                    decision.document_id
                ));
                continue;
            }
        };
        if doc.template_id.trim().is_empty() {
            warnings.push(format!(
                "Document '{}' has no bound template; skipped",
                decision.document_id
            ));
            continue;
        }

        let (_, template_bytes) = match load_template_file(conn, &doc.template_id) {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!(
                    "Document '{}': failed to load template: {e}",
                    decision.document_id
                ));
                continue;
            }
        };

        let values = match resolve_document_values(conn, &bv, &decision.document_id, &matter_data) {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!(
                    "Document '{}': failed to resolve values: {e}",
                    decision.document_id
                ));
                continue;
            }
        };

        let filled_docx = match fill_document(&template_bytes, &values, true) {
            Ok(b) => b,
            Err(e) => {
                errors.push(format!(
                    "Document '{}': fill failed: {e}",
                    decision.document_id
                ));
                continue;
            }
        };

        for ext in output_formats(output_config.output_format) {
            let bytes = if ext == "pdf" {
                match export_pdf_from_docx(&filled_docx) {
                    Ok(b) => b,
                    Err(e) => {
                        errors.push(format!(
                            "Document '{}': PDF conversion failed: {e}",
                            decision.document_id
                        ));
                        continue;
                    }
                }
            } else {
                filled_docx.clone()
            };

            let filename = match crate::core::bundle::output_config::render_filename(
                output_config
                    .filename_template
                    .as_deref()
                    .unwrap_or(""),
                &values,
                &decision.document_name,
                ext,
            ) {
                Ok(f) => f,
                Err(e) => {
                    errors.push(format!(
                        "Document '{}': filename render failed: {e}",
                        decision.document_id
                    ));
                    continue;
                }
            };

            let path = match crate::core::bundle::output_config::resolve_output_path(
                output_root,
                output_config,
                &filename,
            ) {
                Ok(p) => p,
                Err(e) => {
                    errors.push(format!(
                        "Document '{}': output path resolution failed: {e}",
                        decision.document_id
                    ));
                    continue;
                }
            };

            if let Err(e) = fs::write(&path, &bytes) {
                errors.push(format!(
                    "Document '{}': failed to write '{}': {e}",
                    decision.document_id,
                    path.display()
                ));
                continue;
            }

            let content_sha256 = {
                let mut h = Sha256::new();
                h.update(&bytes);
                format!("{:x}", h.finalize())
            };

            generated.push(GeneratedDocument {
                bundle_document_id: decision.document_id.clone(),
                document_name: decision.document_name.clone(),
                format: (*ext).to_string(),
                output_path: path.to_string_lossy().to_string(),
                content_sha256,
            });
        }
    }

    let status = if generated.is_empty() && !errors.is_empty() {
        RunStatus::Failed
    } else if errors.is_empty() {
        RunStatus::Succeeded
    } else {
        RunStatus::Partial
    };

    // Append-only run record (status known at creation per REQ-034).
    let run: GenerationRun = create_run(
        conn,
        matter_id,
        &bundle_id,
        &bv,
        Some(&snapshot),
        Some(&input_hash),
        None,
        status,
    )?;

    // Persist immutable output artifacts.
    for g in &generated {
        let id = format!("gen_{}", Uuid::new_v4());
        conn.execute(
            "INSERT INTO generated_documents
             (id, generation_run_id, bundle_document_id, document_name, format, output_path, content_sha256, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'succeeded')",
            rusqlite::params![
                id,
                run.id,
                g.bundle_document_id,
                g.document_name,
                g.format,
                g.output_path,
                g.content_sha256
            ],
        )
        .map_err(|e| DocForgeError::StorageIo(format!("Insert generated_document: {e}")))?;
    }

    Ok(ExecuteResult {
        run_id: run.id,
        included_document_ids: eligible.iter().map(|d| d.document_id.clone()).collect(),
        generated,
        warnings,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bundle::manifest::{create_bundle, save_manifest, BundleDocumentSpec, BundleManifest};
    use crate::core::field_mapping::registry::create_field;
    use crate::core::field_mapping::schema::{FieldDef, FieldType};
    use crate::core::field_mapping::mapping::set_mapping;
    use crate::core::matter::matter::create_matter;
    use crate::core::matter::matter_values::set_matter_value;
    use crate::core::template_store::save_template;
    use crate::schema::init_memory_db;
    use std::io::Cursor;
    use zip::write::FileOptions;
    use zip::ZipWriter;

    fn minimal_docx(body: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut z = ZipWriter::new(Cursor::new(&mut buf));
            z.start_file("word/document.xml", FileOptions::<()>::default())
                .expect("start file");
            z.write_all(body.as_bytes()).expect("write xml");
            z.finish().expect("finish zip");
        }
        buf
    }

    fn setup() -> (Connection, String, String, String) {
        let conn = init_memory_db().expect("mem");
        let bundle = create_bundle(&conn, "Exec Test", None, None).expect("bundle");
        let bv = conn
            .query_row(
                "SELECT id FROM bundle_versions WHERE bundle_id = ?1 ORDER BY version DESC LIMIT 1",
                [&bundle.id],
                |r| r.get::<_, String>(0),
            )
            .expect("bv");

        // Field + mapping + matter value.
        create_field(
            &conn,
            &bv,
            &FieldDef {
                id: String::new(),
                field_id: "company".to_string(),
                label: "Company".to_string(),
                description: None,
                field_type: FieldType::Text,
                required: false,
                default: None,
                validation: None,
                group_id: None,
                options: Vec::new(),
                format: None,
                position: 0,
            },
        )
        .expect("field");

        // Bundle document + template binding.
        conn.execute(
            "INSERT OR IGNORE INTO bundle_documents (id, bundle_version_id, template_id, position, include_default)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params!["doc1", bv, None::<String>, 0i32, 1i32],
        )
        .expect("insert bundle_document");

        let docx = minimal_docx("<w:document><w:body><w:p><w:r><w:t>Hello {{Company}}</w:t></w:r></w:p></w:body></w:document>");
        let tpl = save_template(
            &conn,
            "Tpl",
            "general",
            "A template",
            &[],
            &docx,
            None,
            None,
        )
        .expect("save template");

        conn.execute(
            "UPDATE bundle_documents SET template_id = ?1 WHERE id = ?2",
            rusqlite::params![tpl.id, "doc1"],
        )
        .expect("bind template");

        // Persist a manifest whose documents list binds doc1 to the template so
        // execute_run can resolve the template from the snapshot.
        let manifest = BundleManifest {
            name: "Exec Test".to_string(),
            documents: vec![BundleDocumentSpec {
                document_id: "doc1".to_string(),
                template_id: tpl.id.clone(),
                position: 0,
                include_default: true,
                condition_ref: None,
            }],
            ..Default::default()
        };
        save_manifest(&conn, &bv, &manifest).expect("save manifest");

        set_mapping(&conn, &bv, "doc1", "{{Company}}", "company").expect("mapping");

        let matter = create_matter(&conn, &bundle.id, &bv, "M1", None, None).expect("matter");
        set_matter_value(&conn, &matter.id, "company", &json!("Acme Corp")).expect("value");

        (conn, bv, matter.id, tpl.id)
    }

    #[test]
    fn test_resolve_document_values_pure() {
        let (conn, bv, matter_id, _tpl) = setup();
        let matter_data = matter_to_json(&conn, &matter_id).expect("json");
        let values = resolve_document_values(&conn, &bv, "doc1", &matter_data).expect("resolve");
        assert_eq!(values.get("Company").map(String::as_str), Some("Acme Corp"));
    }

    #[test]
    fn test_execute_run_generates_docx_and_records() {
        let (conn, _bv, matter_id, _tpl) = setup();
        let out_root = std::env::temp_dir().join(format!("docforge_exec_{}", Uuid::new_v4()));
        fs::create_dir_all(&out_root).expect("mk output root");

        let result = execute_run(&conn, &matter_id, &out_root, None).expect("execute");
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert_eq!(result.generated.len(), 1, "one docx generated");
        let g = &result.generated[0];
        assert_eq!(g.bundle_document_id, "doc1");
        assert_eq!(g.format, "docx");

        let content = fs::read(&g.output_path).expect("read output");
        // Extract word/document.xml from the generated DOCX to verify substitution.
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&content)).expect("open generated docx");
        let mut doc_xml = String::new();
        {
            use std::io::Read;
            let mut f = archive.by_name("word/document.xml").expect("find document.xml");
            f.read_to_string(&mut doc_xml).expect("read document.xml");
        }
        assert!(doc_xml.contains("Acme Corp"), "placeholder substituted: {doc_xml}");
        assert!(!doc_xml.contains("{{Company}}"), "placeholder removed: {doc_xml}");

        // generated_documents row persisted and immutable (content_sha256 present).
        let count: i32 = conn
            .query_row(
                "SELECT COUNT(1) FROM generated_documents WHERE generation_run_id = ?1",
                [&result.run_id],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(count, 1);

        // Append-only guarantee: the run row exists.
        let run_exists: i32 = conn
            .query_row(
                "SELECT COUNT(1) FROM generation_runs WHERE id = ?1",
                [&result.run_id],
                |r| r.get(0),
            )
            .expect("run count");
        assert_eq!(run_exists, 1);

        let _ = fs::remove_dir_all(&out_root);
    }

    #[test]
    fn test_execute_run_respects_selected() {
        let (conn, _bv, matter_id, _tpl) = setup();
        let out_root = std::env::temp_dir().join(format!("docforge_exec_sel_{}", Uuid::new_v4()));
        fs::create_dir_all(&out_root).expect("mk output root");

        // Selecting a non-existent document yields nothing generated.
        let result = execute_run(&conn, &matter_id, &out_root, Some(&["nope".to_string()]))
            .expect("execute");
        assert!(result.generated.is_empty());
        assert!(result.included_document_ids.is_empty());

        let _ = fs::remove_dir_all(&out_root);
    }
}
