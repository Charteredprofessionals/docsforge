//! print_bridge.rs — PrintRenderer trait and OS print subsystem bridge.

use crate::core::error::DocForgeError;

pub trait PrintRenderer {
    fn convert_html_to_pdf(&self, html: &str) -> Result<Vec<u8>, DocForgeError>;
}

pub struct HeadlessPrintRenderer;

impl PrintRenderer for HeadlessPrintRenderer {
    fn convert_html_to_pdf(&self, html: &str) -> Result<Vec<u8>, DocForgeError> {
        if html.is_empty() {
            return Err(DocForgeError::InvalidDocx("HTML payload empty".to_string()));
        }
        // Minimal PDF binary header fallback for headless rendering
        let mut pdf_bytes = vec![0x25, 0x50, 0x44, 0x46, 0x2D, 0x31, 0x2E, 0x37, 0x0A]; // %PDF-1.7\n
        pdf_bytes.extend_from_slice(b"%DocForge PDF Output\n1 0 obj<</Type/Catalog>>endobj\ntrailer<</Root 1 0 R>>\n%%EOF");
        Ok(pdf_bytes)
    }
}
