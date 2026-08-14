//! core/mod.rs — Root module definition for docforge-core.
//!
//! Exposes domain models, error types, and core engine interfaces for document processing,
//! template storage, governance, licensing, and multi-format exports.

pub mod bundle;
pub mod bundles;
pub mod bug_book;
pub mod docx_engine;
pub mod error;
pub mod export;
pub mod field_mapping;
pub mod matter;
pub mod rules;
pub mod generation_run;
pub mod governance;
pub mod licensing;
pub mod template;
pub mod template_store;
pub mod versioning;

pub use bundles::{add_template_to_bundle, create_bundle, delete_bundle, get_bundle_templates, list_bundles, remove_template_from_bundle};
// v2 Bundle domain. `create_bundle`/`list_bundles`/`delete_bundle` collide with
// the v1 re-exports above, so the v2 versions live under the `bundle::` namespace;
// only non-colliding v2 names are re-exported at the core root.
pub use bundle::{
    BundleDetail, BundleDocumentSpec, BundleManifest, BundleRecord, BundleSchema, BundleSummary,
    BundleVersionRecord, OutputConfig, OutputFormat, get_bundle, get_manifest, save_manifest,
};
pub use bug_book::{
    add_attachment, create_bug, export_bugs_csv, export_bugs_pdf, get_bug, list_bugs, record_crash,
    update_bug_status, BugAttachment, BugEntry, BugFilter, NewBug, SEVERITIES, STATUSES,
};
pub use docx_engine::{fill_document, tag_document, validate_docx, TemplateFieldSpec};
pub use error::{DocForgeError, ErrorResponse};
pub use export::{export_dfpkg, export_docx, export_pdf_from_docx, import_dfpkg, render_sanitized_html};
pub use matter::{
    create_matter, delete_matter, get_matter, list_matters, matter_to_json, populate_matter_field,
    render_matter_form, update_matter_status, validate_matter, FormField, FormGroup, Matter, MatterForm,
    MatterStatus, MatterValue, ValidationIssue, ValidationLevel, ValidationReport,
};
pub use field_mapping::{
    FieldDef, FieldGroup, FieldType, GroupScope, create_field, create_field_group, list_field_groups,
    list_fields, remove_field, update_field, validate_field_schema, validate_value,
};
pub use generation_run::{
    compute_input_hash, create_run, execute_run, get_run, list_runs, resolve_document_values, GeneratedDocument,
    ExecuteResult, GenerationRun, RunStatus, ENGINE_VERSION,
};
pub use governance::{authorize, generate_usage_report, record_generation, transition_template_status, Action, UserRole};
pub use licensing::{activate_offline_license_file, evaluate_entitlement, get_active_license, Feature, LicenseInfo, LicenseTier};
pub use template::{TemplateRecord, TemplateStatus};
pub use template_store::{delete_template, list_templates, load_template_file, load_template_meta, save_template};
pub use rules::{
    add_rule, collect_field_refs, evaluate, evaluate_preview, evaluate_rules, list_rules, parse,
    remove_rule, validate_rule_expression, BinOp, DocumentDecision, Expr, Literal, Rule, RulesPreview,
    SkippedDocument, UnaryOp,
};
pub use versioning::{create_template_version, rollback_template_version, TemplateVersionRecord};
