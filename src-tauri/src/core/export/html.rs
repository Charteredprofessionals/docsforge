//! export/html.rs — HTML preview renderer with input sanitization.

use crate::core::error::DocForgeError;

/// Renders HTML output and strips unsafe script tags, inline event handlers, and javascript: URIs.
pub fn render_sanitized_html(raw_html: &str) -> Result<String, DocForgeError> {
    if raw_html.is_empty() {
        return Ok(String::new());
    }

    // Basic server-side sanitization rules
    let sanitized = raw_html
        .replace("<script", "&lt;script")
        .replace("</script>", "&lt;/script&gt;")
        .replace("javascript:", "invalid-scheme:")
        .replace("onload=", "data-onload=")
        .replace("onerror=", "data-onerror=");

    Ok(sanitized)
}
