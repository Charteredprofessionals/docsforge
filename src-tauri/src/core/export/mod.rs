//! export/mod.rs — Multi-format document export module.
//!
//! Provides DOCX filling, sanitized HTML previews, PDF generation, and portable `.dfpkg` package bundling.

pub mod dfpkg;
pub mod docx;
pub mod html;
pub mod pdf;

pub use dfpkg::{export_dfpkg, import_dfpkg};
pub use docx::export_docx;
pub use html::render_sanitized_html;
pub use pdf::export_pdf;
