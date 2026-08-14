//! output_config.rs — Bundle output configuration behaviors (TASK-105).
//!
//! Implements the naming/folder policy from architecture §6 (REQ-035) on top of
//! the persisted `OutputConfig` model defined in `manifest.rs` (single source of
//! truth — no duplicate type lives here).
//!
//! Design decisions (documented determinism, AC-035):
//! - **Unresolved placeholders stay as-is.** `{{field_id}}` tokens with no value
//!   in the supplied map are copied into the result verbatim instead of failing.
//!   Output naming must not block generation (architecture §6.4): a cosmetic
//!   placeholder in a filename template is surfaced by the health check, not by
//!   a hard error mid-run.
//! - **Sanitization removes** Windows-illegal `<>:"/\|?*` and control characters
//!   from the rendered name. Everything else (spaces, dots, `{}`, `&`, `+`, …)
//!   is preserved so business names read naturally in file names.
//! - **Extension is appended once**, case-insensitively, so `report` + `docx`
//!   yields `report.docx` and `report.docx` + `docx` stays `report.docx`.
//! - **Containment is lexical and checked twice**: `output_folder` rejects `..`
//!   and absolute paths during validation, and `resolve_output_path` re-validates
//!   plus verifies the final path is a sub-path of the output root. Generation can
//!   never silently write outside the output root (REQ-018, REQ-035).

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use crate::core::bundle::manifest::{OutputConfig, OutputFormat};
use crate::core::error::DocForgeError;

/// Returns the engine default output configuration: no filename template (engine
/// default naming), DOCX only, app-data default output tree, no zip packaging.
///
/// Mirrors `OutputConfig::default()` explicitly so call sites read the policy
/// without reaching into the persistence model's `Default` impl.
pub fn default_output_config() -> OutputConfig {
    OutputConfig {
        filename_template: None,
        output_format: OutputFormat::Docx,
        output_folder: None,
        zip_output: false,
    }
}

/// Renders a deterministic output filename from a `{{field_id}}` template.
///
/// `template` is rendered by replacing `{{field_id}}` placeholders with the
/// matching values from `values` (keys are trimmed before lookup). An empty or
/// whitespace-only template falls back to `document_name`. Unresolved
/// placeholders are kept verbatim (see module docs). The result is sanitized to
/// a safe single-path-component filename and `extension` is appended unless the
/// name already ends with it (case-insensitive). `extension` may be passed with
/// or without a leading dot (e.g. `"docx"` or `".pdf"`).
pub fn render_filename(
    template: &str,
    values: &HashMap<String, String>,
    document_name: &str,
    extension: &str,
) -> Result<String, DocForgeError> {
    let base = if template.trim().is_empty() {
        document_name.to_string()
    } else {
        render_placeholders(template, values)
    };

    let sanitized = sanitize_filename_component(&base);
    if sanitized.trim().is_empty() {
        return Err(DocForgeError::InvalidInput(
            "rendered output filename is empty after sanitization".to_string(),
        ));
    }

    Ok(append_extension_once(&sanitized, extension))
}

/// Validates an `OutputConfig` for naming and folder safety (REQ-035).
///
/// - `filename_template`, when set, must be non-empty and contain only
///   Windows-safe characters (no `<>:"/\|?*`, no control characters).
/// - `output_folder`, when set, must be a relative safe path: no absolute
///   prefix and no `..` traversal components.
/// - `zip_output` is orthogonal to `output_format` (a single zip may wrap DOCX,
///   PDF, or both), so the current model has no format/zip inconsistency to
///   reject; the check is reserved for future policy combinations.
pub fn validate_output_config(cfg: &OutputConfig) -> Result<(), DocForgeError> {
    if let Some(template) = &cfg.filename_template {
        if template.trim().is_empty() {
            return Err(DocForgeError::InvalidInput(
                "filename_template must not be empty when set".to_string(),
            ));
        }
        if let Some(illegal) = template
            .chars()
            .find(|c| !is_safe_filename_char(*c))
        {
            return Err(DocForgeError::InvalidInput(format!(
                "filename_template contains illegal character '{illegal}'"
            )));
        }
    }

    if let Some(folder) = &cfg.output_folder {
        validate_output_folder(folder)?;
    }

    Ok(())
}

/// Resolves the target file path for a generated document.
///
/// Joins `output_root` with `cfg.output_folder` (when set) and `document_name`,
/// then containment-checks the result: the joined path must remain inside
/// `output_root` (path traversal guard, REQ-018). `document_name` is sanitized
/// so path separators in a document name can never escape the root.
pub fn resolve_output_path(
    output_root: &Path,
    cfg: &OutputConfig,
    document_name: &str,
) -> Result<PathBuf, DocForgeError> {
    let mut target = output_root.to_path_buf();

    if let Some(folder) = &cfg.output_folder {
        validate_output_folder(folder)?;
        if !folder.is_empty() {
            target.push(folder);
        }
    }

    let filename = sanitize_filename_component(document_name);
    if filename.trim().is_empty() {
        return Err(DocForgeError::InvalidInput(
            "document_name yields an empty filename".to_string(),
        ));
    }
    target.push(filename);

    if !is_subpath_of(output_root, &target) {
        return Err(DocForgeError::InvalidInput(format!(
            "resolved output path escapes output_root '{}': {}",
            output_root.display(),
            target.display()
        )));
    }

    Ok(target)
}

/// Replaces every `{{key}}` placeholder with its mapped value; tokens with no
/// value are copied verbatim (non-fatal policy, see module docs).
fn render_placeholders(template: &str, values: &HashMap<String, String>) -> String {
    let mut rendered = String::with_capacity(template.len());
    let mut cursor = 0;

    while cursor < template.len() {
        match template[cursor..].find("{{") {
            Some(open_offset) => {
                let open = cursor + open_offset;
                let after_open = open + 2;
                match template[after_open..].find("}}") {
                    Some(close_offset) => {
                        let close = after_open + close_offset;
                        let key = template[after_open..close].trim();
                        rendered.push_str(&template[cursor..open]);
                        match values.get(key) {
                            Some(value) => rendered.push_str(value),
                            None => rendered.push_str(&template[open..close + 2]),
                        }
                        cursor = close + 2;
                    }
                    None => {
                        rendered.push_str(&template[cursor..]);
                        break;
                    }
                }
            }
            None => {
                rendered.push_str(&template[cursor..]);
                break;
            }
        }
    }

    rendered
}

/// Removes Windows-illegal filename characters (`<>:"/\|?*`) and control chars.
fn sanitize_filename_component(input: &str) -> String {
    input
        .chars()
        .filter(|c| is_safe_filename_char(*c))
        .collect()
}

/// A character is safe in a filename when it is not a Windows-illegal
/// character and not a control character.
fn is_safe_filename_char(c: char) -> bool {
    !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') && !c.is_control()
}

/// Appends `.extension` unless the name already ends with it (case-insensitive).
fn append_extension_once(name: &str, extension: &str) -> String {
    let extension = extension.trim().trim_start_matches('.');
    if extension.is_empty() {
        return name.to_string();
    }

    let dotted = format!(".{extension}");
    if name.to_lowercase().ends_with(&dotted.to_lowercase()) {
        name.to_string()
    } else {
        format!("{name}{dotted}")
    }
}

/// Validates an output folder string: relative only, no `..` traversal.
fn validate_output_folder(folder: &str) -> Result<(), DocForgeError> {
    if folder.is_empty() {
        return Ok(());
    }

    let path = Path::new(folder);
    for component in path.components() {
        match component {
            Component::ParentDir => {
                return Err(DocForgeError::InvalidInput(format!(
                    "output_folder must not contain '..' path traversal: '{folder}'"
                )));
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(DocForgeError::InvalidInput(format!(
                    "output_folder must be a relative path: '{folder}'"
                )));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    Ok(())
}

/// Lexical containment check: `candidate` is inside `root` when it shares
/// `root`'s full component prefix and none of its remaining components is a
/// parent-dir (`..`) traversal.
fn is_subpath_of(root: &Path, candidate: &Path) -> bool {
    let root_components: Vec<Component<'_>> = root.components().collect();
    let candidate_components: Vec<Component<'_>> = candidate.components().collect();

    if candidate_components.len() < root_components.len() {
        return false;
    }
    if root_components
        .iter()
        .zip(&candidate_components)
        .any(|(root_part, candidate_part)| root_part != candidate_part)
    {
        return false;
    }
    candidate_components[root_components.len()..]
        .iter()
        .all(|component| !matches!(component, Component::ParentDir))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn test_default_config_is_docx_flat() {
        let cfg = default_output_config();
        assert_eq!(cfg.filename_template, None);
        assert_eq!(cfg.output_format, OutputFormat::Docx);
        assert_eq!(cfg.output_folder, None);
        assert!(!cfg.zip_output);
    }

    #[test]
    fn test_render_filename_replaces_placeholders() {
        let template = "{{company_name}}_Board_Resolution{{date}}.docx";
        let values = values(&[("company_name", "ABC Pvt Ltd"), ("date", "2026-08-11")]);
        let rendered = render_filename(template, &values, "Resolution", "docx")
            .expect("render with known fields");
        assert_eq!(rendered, "ABC Pvt Ltd_Board_Resolution2026-08-11.docx");
    }

    #[test]
    fn test_render_filename_sanitizes_illegal_chars() {
        let template = "{{title}}_Report.docx";
        let values = values(&[("title", "C:\\Temp\\Q1:Final")]);
        let rendered = render_filename(template, &values, "Report", "docx")
            .expect("render with illegal characters");
        assert_eq!(rendered, "CTempQ1Final_Report.docx");
        assert!(!rendered.contains([':', '\\', '/']));
    }

    #[test]
    fn test_render_filename_appends_extension_once() {
        let values = values(&[("company_name", "ABC Pvt Ltd")]);

        let without_extension = render_filename("{{company_name}}", &values, "Report", "docx")
            .expect("render without extension");
        assert_eq!(without_extension, "ABC Pvt Ltd.docx");

        let with_extension =
            render_filename("{{company_name}}.docx", &values, "Report", "docx")
                .expect("render with extension already present");
        assert_eq!(with_extension, "ABC Pvt Ltd.docx", "extension appended exactly once");

        let uppercase =
            render_filename("{{company_name}}.DOCX", &values, "Report", "docx")
                .expect("render with uppercase extension");
        assert_eq!(uppercase, "ABC Pvt Ltd.DOCX", "case-insensitive dedupe");
    }

    #[test]
    fn test_render_filename_empty_template_uses_document_name() {
        let values = HashMap::new();
        let rendered = render_filename("", &values, "Agreement", ".pdf").expect("empty template");
        assert_eq!(rendered, "Agreement.pdf");
        let blank = render_filename("   ", &values, "Agreement", ".pdf").expect("blank template");
        assert_eq!(blank, "Agreement.pdf");
    }

    #[test]
    fn test_render_filename_keeps_unresolved_placeholder() {
        let values = HashMap::new();
        let rendered =
            render_filename("{{company_name}}_Report.docx", &values, "Report", "docx")
                .expect("unresolved placeholder must not fail");
        assert_eq!(
            rendered, "{{company_name}}_Report.docx",
            "unresolved placeholders are kept as-is so naming never blocks generation"
        );
    }

    #[test]
    fn test_validate_rejects_path_traversal() {
        let traversal = OutputConfig {
            output_folder: Some("../escape".to_string()),
            ..default_output_config()
        };
        let err = validate_output_config(&traversal).expect_err("traversal must be rejected");
        assert!(matches!(err, DocForgeError::InvalidInput(_)));
        assert!(err.to_string().contains("traversal"));

        let absolute = OutputConfig {
            output_folder: Some("C:\\escape".to_string()),
            ..default_output_config()
        };
        let err = validate_output_config(&absolute).expect_err("absolute must be rejected");
        assert!(matches!(err, DocForgeError::InvalidInput(_)));
    }

    #[test]
    fn test_validate_rejects_illegal_template_character() {
        let cfg = OutputConfig {
            filename_template: Some("Resolution/{{date}}".to_string()),
            ..default_output_config()
        };
        let err = validate_output_config(&cfg).expect_err("illegal template char rejected");
        assert!(matches!(err, DocForgeError::InvalidInput(_)));
        assert!(err.to_string().contains("'/'"));
    }

    #[test]
    fn test_validate_accepts_safe_config() {
        let cfg = OutputConfig {
            filename_template: Some("{{company_name}}_Resolution{{date}}.docx".to_string()),
            output_folder: Some("reports/2026".to_string()),
            output_format: OutputFormat::DocxAndPdf,
            zip_output: true,
        };
        assert!(validate_output_config(&cfg).is_ok());
    }

    #[test]
    fn test_resolve_output_path_stays_contained() {
        let root = Path::new("C:/output/root");

        let traversal = OutputConfig {
            output_folder: Some("../escape".to_string()),
            ..default_output_config()
        };
        let err = resolve_output_path(root, &traversal, "resolution.docx")
            .expect_err("traversal must be rejected at resolve time");
        assert!(matches!(err, DocForgeError::InvalidInput(_)));

        let nested = OutputConfig {
            output_folder: Some("runs/2026".to_string()),
            ..default_output_config()
        };
        let resolved = resolve_output_path(root, &nested, "resolution.docx")
            .expect("nested folder resolves inside root");
        assert_eq!(resolved, Path::new("C:/output/root/runs/2026/resolution.docx"));
        assert!(
            resolved.starts_with(root),
            "resolved path must remain inside the output root"
        );

        let flat = resolve_output_path(root, &default_output_config(), "resolution.docx")
            .expect("no output folder resolves to the root itself");
        assert_eq!(flat, Path::new("C:/output/root/resolution.docx"));
    }
}
