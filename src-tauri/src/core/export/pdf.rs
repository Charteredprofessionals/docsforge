//! export/pdf.rs — PDF conversion engine.
//!
//! Converts DOCX to PDF using the headless print renderer without LibreOffice dependency.

use crate::core::error::DocForgeError;
use crate::infra::print_bridge::{HeadlessPrintRenderer, PrintRenderer};

pub fn export_pdf(html_content: &str) -> Result<Vec<u8>, DocForgeError> {
    let renderer = HeadlessPrintRenderer;
    renderer.convert_html_to_pdf(html_content)
}
