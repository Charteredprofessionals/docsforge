//! export/pdf.rs — Native PDF conversion engine (no LibreOffice dependency).
//!
//! Adopted from the `templatebuilder` sibling project (high-impact feature): renders a
//! filled DOCX to PDF entirely in Rust via `docx-rs` (parse) + `printpdf` (layout).
//! Used as a fallback when LibreOffice is unavailable, so PDF export works out of the box.

use crate::core::error::DocForgeError;
use docx_rs::*;
use printpdf::{BuiltinFont, Mm, PdfDocument};
use std::io::BufWriter;

const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN: f32 = 20.0;
const FONT_SIZE: f32 = 11.0;
const LINE_H: f32 = 5.5;

/// Render plain-text lines onto a multi-page A4 PDF using `printpdf`.
fn render_lines_to_pdf(lines: &[String]) -> Result<Vec<u8>, DocForgeError> {
    let (doc, page1, layer1) =
        PdfDocument::new("DocForge Export", Mm(PAGE_W), Mm(PAGE_H), "Layer 1");
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| DocForgeError::Internal(format!("PDF font error: {e}")))?;

    let mut current_layer = doc.get_page(page1).get_layer(layer1);
    let mut y = PAGE_H - MARGIN;

    for line in lines {
        if y < MARGIN {
            let (new_page, new_layer) = doc.add_page(Mm(PAGE_W), Mm(PAGE_H), "Layer 2");
            current_layer = doc.get_page(new_page).get_layer(new_layer);
            y = PAGE_H - MARGIN;
        }
        if !line.is_empty() {
            current_layer.use_text(line.clone(), FONT_SIZE, Mm(MARGIN), Mm(y), &font);
        }
        y -= LINE_H;
    }

    let mut buf: Vec<u8> = Vec::new();
    doc.save(&mut BufWriter::new(&mut buf))
        .map_err(|e| DocForgeError::Internal(format!("PDF save error: {e}")))?;
    Ok(buf)
}

/// Extract paragraph text from a DOCX byte vector using `docx-rs`.
fn docx_to_lines(docx_bytes: &[u8]) -> Result<Vec<String>, DocForgeError> {
    let doc = read_docx(docx_bytes)
        .map_err(|e| DocForgeError::InvalidDocx(format!("docx-rs parse failed: {e}")))?;

    let mut lines = Vec::new();
    for child in doc.document.children {
        if let DocumentChild::Paragraph(p) = child {
            let mut para = String::new();
            for r in p.children {
                if let ParagraphChild::Run(run) = r {
                    for c in run.children {
                        if let RunChild::Text(t) = c {
                            para.push_str(&t.text);
                        }
                    }
                }
            }
            lines.push(para);
        }
    }
    Ok(lines)
}

/// Native DOCX -> PDF. No external process required. Used as a fallback when
/// LibreOffice is not installed, so PDF export works out of the box.
pub fn export_pdf_from_docx(docx_bytes: &[u8]) -> Result<Vec<u8>, DocForgeError> {
    let lines = docx_to_lines(docx_bytes)?;
    render_lines_to_pdf(&lines)
}

/// Native HTML -> PDF (used by the REST bridge). Strips tags and renders text.
pub fn export_pdf(html_content: &str) -> Result<Vec<u8>, DocForgeError> {
    let text = strip_html(html_content);
    let lines: Vec<String> = text.split('\n').map(|s| s.trim_end().to_string()).collect();
    render_lines_to_pdf(&lines)
}

/// Minimal HTML tag stripper for the native fallback (no layout fidelity).
fn strip_html(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_pdf_marks_valid_header() {
        // export_pdf strips HTML and renders plain text to a valid PDF.
        let pdf = export_pdf("<p>Hello <b>World</b></p>").expect("pdf");
        assert!(pdf.starts_with(b"%PDF"), "output must be a PDF document");
    }

    #[test]
    fn test_strip_html_removes_tags() {
        assert_eq!(strip_html("<p>Hi</p>"), "Hi");
        assert_eq!(strip_html("a&amp;b"), "a&b");
    }
}
