//! core/mod.rs — Root module definition for docforge-core.
//!
//! Exposes domain models, error types, and core engine interfaces for document processing,
//! template storage, governance, licensing, and multi-format exports.

pub mod bundles;
pub mod docx_engine;
pub mod error;
pub mod export;
pub mod governance;
pub mod licensing;
pub mod template;
pub mod template_store;
pub mod versioning;

pub use bundles::{add_template_to_bundle, create_bundle, delete_bundle, get_bundle_templates, list_bundles, remove_template_from_bundle};
pub use docx_engine::{fill_document, tag_document, validate_docx, TemplateFieldSpec};
pub use error::{DocForgeError, ErrorResponse};
pub use export::{export_dfpkg, export_docx, export_pdf_from_docx, import_dfpkg, render_sanitized_html};
pub use governance::{authorize, generate_usage_report, record_generation, transition_template_status, Action, UserRole};
pub use licensing::{activate_offline_license_file, evaluate_entitlement, get_active_license, Feature, LicenseInfo, LicenseTier};
pub use template::{TemplateRecord, TemplateStatus};
pub use template_store::{delete_template, list_templates, load_template_file, load_template_meta, save_template};
pub use versioning::{create_template_version, rollback_template_version, TemplateVersionRecord};
