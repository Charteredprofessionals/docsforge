//! export/docx.rs — DOCX byte-identical exporter.

use crate::core::docx_engine::fill_document;
use crate::core::error::DocForgeError;
use std::collections::HashMap;

/// Exports a filled DOCX byte vector from template bytes and field values.
pub fn export_docx(
    template_bytes: &[u8],
    field_values: &HashMap<String, String>,
) -> Result<Vec<u8>, DocForgeError> {
    fill_document(template_bytes, field_values)
}
