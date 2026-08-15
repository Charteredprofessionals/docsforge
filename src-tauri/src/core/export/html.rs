//! export/html.rs — HTML preview renderer with input sanitization.
//!
//! Defense-in-depth HTML sanitizer used for any HTML preview surface. It uses a
//! strict allowlist of tags and attributes and strips everything else, including
//! event-handler attributes and dangerous URI schemes. (The React preview layer
//! should additionally sanitize with DOMPurify before `dangerouslySetInnerHTML`.)

use crate::core::error::DocForgeError;

/// Tags permitted in sanitized preview HTML.
const ALLOWED_TAGS: &[&str] = &[
    "p", "br", "b", "strong", "i", "em", "u", "ul", "ol", "li", "h1", "h2", "h3", "h4", "h5", "h6",
    "table", "thead", "tbody", "tr", "td", "th", "blockquote", "code", "pre", "span", "a", "img",
    "hr", "div", "section", "sub", "sup",
];

/// Tags whose entire content must be dropped (not just the tag).
const DROP_WITH_CONTENT: &[&str] = &["script", "style", "iframe", "object", "embed", "link", "base", "meta"];

/// Attributes permitted on specific tags (tag -> allowed attr).
fn allowed_attribute(tag: &str, attr: &str) -> bool {
    match (tag, attr) {
        ("a", "href") => true,
        ("a", "target") => true,
        ("a", "rel") => true,
        ("img", "src") => true,
        ("img", "alt") => true,
        ("img", "width") => true,
        ("img", "height") => true,
        _ => false,
    }
}

/// Returns true only for http(s)/mailto or safe data: URIs (images only).
fn is_safe_uri(value: &str) -> bool {
    let v = value.trim().to_ascii_lowercase();
    if v.starts_with("http://") || v.starts_with("https://") || v.starts_with("mailto:") {
        return true;
    }
    if v.starts_with("data:") {
        return v.starts_with("data:image/");
    }
    false
}

pub fn render_sanitized_html(raw_html: &str) -> Result<String, DocForgeError> {
    if raw_html.is_empty() {
        return Ok(String::new());
    }
    let mut out = String::with_capacity(raw_html.len());
    let bytes = raw_html.as_bytes();
    let mut i = 0;
    let n = bytes.len();
    let mut drop_depth = 0usize; // >0 while inside a dropped-content tag

    while i < n {
        if bytes[i] == b'<' {
            // Parse a tag: <tag ...> or </tag> or <!-- ... -->
            let tag_end = match raw_html[i..].find('>') {
                Some(off) => i + off,
                None => {
                    // Dangling '<' — emit literally and stop.
                    out.push('<');
                    i += 1;
                    continue;
                }
            };
            let tag_text = &raw_html[i + 1..tag_end];
            if tag_text.starts_with("!--") {
                // Comment: drop it.
                i = tag_end + 1;
                continue;
            }
            let is_close = tag_text.starts_with('/');
            let inner = if is_close { &tag_text[1..] } else { tag_text };
            let name_end = inner.find(|c: char| c == ' ' || c == '/' || c == '>').unwrap_or(inner.len());
            let raw_name = inner[..name_end].to_ascii_lowercase();
            let name = raw_name.trim();

            if is_close {
                if drop_depth > 0 && DROP_WITH_CONTENT.contains(&name) {
                    drop_depth -= 1;
                } else if drop_depth == 0 {
                    if ALLOWED_TAGS.contains(&name) {
                        out.push_str(&format!("</{}>", name));
                    }
                }
            } else if let Some(rest) = DROP_WITH_CONTENT.iter().find(|&&t| t == name) {
                // Opening of a drop-content tag.
                let _ = rest;
                drop_depth += 1;
            } else if ALLOWED_TAGS.contains(&name) {
                // Opening of an allowed tag: re-emit with sanitized attributes.
                // Self-closing?
                let self_close = inner.trim_end().ends_with('/');
                let attrs = parse_attributes(&inner[name_end..]);
                let mut emitted = format!("<{}", name);
                for (k, v) in attrs {
                    let kl = k.to_ascii_lowercase();
                    if allowed_attribute(name, &kl) {
                        if kl == "href" || kl == "src" {
                            if is_safe_uri(&v) {
                                emitted.push_str(&format!(" {}=\"{}\"", kl, escape_attr(&v)));
                            }
                        } else {
                            emitted.push_str(&format!(" {}=\"{}\"", kl, escape_attr(&v)));
                        }
                    }
                    // All other attributes (on*, style, etc.) are dropped.
                }
                emitted.push_str(if self_close { "/>" } else { ">" });
                out.push_str(&emitted);
            }
            // Unknown tags: silently dropped, content preserved.
            i = tag_end + 1;
        } else {
            // Text node — copy verbatim.
            let next = raw_html[i..].find('<').unwrap_or(n - i);
            out.push_str(&raw_html[i..i + next]);
            i += next;
        }
    }

    Ok(out)
}

/// Minimal attribute parser tolerant of unquoted/quoted values.
fn parse_attributes(s: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let mut it = s.trim_start().chars().peekable();
    let mut buf = String::new();
    while it.peek().is_some() {
        buf.clear();
        // Read attribute name.
        while let Some(&c) = it.peek() {
            if c.is_whitespace() || c == '=' || c == '/' || c == '>' {
                break;
            }
            buf.push(c);
            it.next();
        }
        let name = buf.trim().to_string();
        if name.is_empty() {
            // Skip separators.
            if let Some(&c) = it.peek() {
                if c == '/' || c == '>' {
                    break;
                }
            }
            it.next();
            continue;
        }
        // Skip whitespace.
        while let Some(&c) = it.peek() {
            if !c.is_whitespace() {
                break;
            }
            it.next();
        }
        let mut value = String::new();
        if let Some(&c) = it.peek() {
            if c == '=' {
                it.next();
                while let Some(&c) = it.peek() {
                    if c.is_whitespace() {
                        it.next();
                    } else {
                        break;
                    }
                }
                if let Some(&c) = it.peek() {
                    if c == '"' || c == '\'' {
                        let q = c;
                        it.next();
                        while let Some(c) = it.next() {
                            if c == q {
                                break;
                            }
                            value.push(c);
                        }
                    } else {
                        while let Some(&c) = it.peek() {
                            if c.is_whitespace() || c == '/' || c == '>' {
                                break;
                            }
                            value.push(c);
                            it.next();
                        }
                    }
                }
            }
        }
        attrs.push((name, value));
    }
    attrs
}

fn escape_attr(s: &str) -> String {
    s.replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_script_and_event_handlers() {
        let dirty = r#"<p onclick="x()">Hi</p><script>alert(1)</script><img src="x" onerror="alert(2)">"#;
        let clean = render_sanitized_html(dirty).unwrap();
        assert!(!clean.to_lowercase().contains("script"));
        assert!(!clean.to_lowercase().contains("onclick"));
        assert!(!clean.to_lowercase().contains("onerror"));
        assert!(clean.contains("<p>Hi</p>"));
    }

    #[test]
    fn neutralizes_javascript_uri() {
        let dirty = r#"<a href="javascript:alert(1)">click</a>"#;
        let clean = render_sanitized_html(dirty).unwrap();
        assert!(!clean.to_lowercase().contains("javascript:"));
    }

    #[test]
    fn keeps_safe_links_and_basic_formatting() {
        let ok = r#"<p>Hello <b>world</b> <a href="https://example.com">link</a></p>"#;
        let clean = render_sanitized_html(ok).unwrap();
        assert!(clean.contains("<b>world</b>"));
        assert!(clean.contains(r#"href="https://example.com""#));
    }
}
