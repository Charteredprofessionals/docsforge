//! fidelity_gate.rs — real DOCX tag-fidelity corpus runner (TASK-118 / F-008).
//!
//! Replaces the previously fabricated harness (which returned hardcoded 100%) with
//! an actual round-trip: for each fixture it builds a valid DOCX, runs
//! `tag_document` to inject `{{tag}}` placeholders, then `fill_document` to
//! substitute values, and asserts the rendered output matches expectations.

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

use docforge::core::docx_engine::{fill_document, tag_document, TemplateFieldSpec};

pub struct CorpusResult {
    pub total: usize,
    pub passed: usize,
    pub fidelity_percentage: f64,
}

/// Builds a minimal but valid DOCX (PK zip containing `word/document.xml`).
fn make_docx(body: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut z = zip::ZipWriter::new(Cursor::new(&mut buf));
        z.start_file(
            "word/document.xml",
            zip::write::FileOptions::<()>::default(),
        )
        .expect("start document.xml");
        z.write_all(body.as_bytes()).expect("write document.xml");
        z.finish().expect("finish zip");
    }
    buf
}

fn extract_document_xml(bytes: &[u8]) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("open generated docx");
    let mut doc_xml = String::new();
    {
        let mut f = archive
            .by_name("word/document.xml")
            .expect("find document.xml");
        f.read_to_string(&mut doc_xml).expect("read document.xml");
    }
    doc_xml
}

struct Fixture {
    name: &'static str,
    doc: &'static str,
    original: &'static str,
    tag: &'static str,
    value: &'static str,
}

fn corpus() -> Vec<Fixture> {
    vec![
        Fixture {
            name: "plain_text",
            doc: "<w:document><w:body><w:p><w:r><w:t>Hello Company, welcome.</w:t></w:r></w:p></w:body></w:document>",
            original: "Company",
            tag: "company",
            value: "Globex",
        },
        // Cross-run: selection spans two <w:t> runs (bold "Acme").
        Fixture {
            name: "cross_run",
            doc: "<w:document><w:body><w:p><w:r><w:t>Dear </w:t></w:r><w:r w:rPr=\"bold\"><w:t>Acme</w:t></w:r></w:p></w:body></w:document>",
            original: "Dear Acme",
            tag: "client",
            value: "Initech",
        },
        // Multiple fields in one paragraph.
        Fixture {
            name: "multi_field",
            doc: "<w:document><w:body><w:p><w:r><w:t>Ref RefNum for Party</w:t></w:r></w:p></w:body></w:document>",
            original: "RefNum",
            tag: "ref",
            value: "REF-42",
        },
        // Multi-paragraph document.
        Fixture {
            name: "multi_para",
            doc: "<w:document><w:body><w:p><w:r><w:t>Header Title</w:t></w:r></w:p><w:p><w:r><w:t>Body Text</w:t></w:r></w:p></w:body></w:document>",
            original: "Title",
            tag: "title",
            value: "Annual Report",
        },
        // Unicode content.
        Fixture {
            name: "unicode",
            doc: "<w:document><w:body><w:p><w:r><w:t>Café Münchën 名前</w:t></w:r></w:p></w:body></w:document>",
            original: "Münchën",
            tag: "city",
            value: "Berlin",
        },
        // Field at paragraph start.
        Fixture {
            name: "leading_field",
            doc: "<w:document><w:body><w:p><w:r><w:t>Name signed the doc.</w:t></w:r></w:p></w:body></w:document>",
            original: "Name",
            tag: "signatory",
            value: "Jane Doe",
        },
        // Cross-run with three runs (bold middle).
        Fixture {
            name: "cross_run_three",
            doc: "<w:document><w:body><w:p><w:r><w:t>Start </w:t></w:r><w:r w:rPr=\"bold\"><w:t>Middle</w:t></w:r><w:r><w:t> End</w:t></w:r></w:p></w:body></w:document>",
            original: "Start Middle End",
            tag: "phrase",
            value: "Complete",
        },
        // Italic cross-run.
        Fixture {
            name: "cross_run_italic",
            doc: "<w:document><w:body><w:p><w:r><w:t>Important: </w:t></w:r><w:r w:rPr=\"italic\"><w:t>Review</w:t></w:r><w:r><w:t> this</w:t></w:r></w:p></w:body></w:document>",
            original: "Review this",
            tag: "action",
            value: "Approve",
        },
        // Nested formatting (bold + italic).
        Fixture {
            name: "nested_formatting",
            doc: "<w:document><w:body><w:p><w:r><w:t>Total: </w:t></w:r><w:r w:rPr=\"bold\"><w:t>$</w:t></w:r><w:r w:rPr=\"italic\"><w:t>100</w:t></w:r></w:p></w:body></w:document>",
            original: "$100",
            tag: "amount",
            value: "$500",
        },
        // Repeated placeholder in same paragraph.
        Fixture {
            name: "repeated_placeholder",
            doc: "<w:document><w:body><w:p><w:r><w:t>Name: Client, Client, Client</w:t></w:r></w:p></w:body></w:document>",
            original: "Client",
            tag: "client_name",
            value: "Acme Corp",
        },
        // Number field.
        Fixture {
            name: "number_field",
            doc: "<w:document><w:body><w:p><w:r><w:t>Quantity: 42 items</w:t></w:r></w:p></w:body></w:document>",
            original: "42",
            tag: "quantity",
            value: "100",
        },
        // Date field.
        Fixture {
            name: "date_field",
            doc: "<w:document><w:body><w:p><w:r><w:t>Date: 2024-01-15</w:t></w:r></w:p></w:body></w:document>",
            original: "2024-01-15",
            tag: "date",
            value: "2025-06-30",
        },
        // Email field.
        Fixture {
            name: "email_field",
            doc: "<w:document><w:body><w:p><w:r><w:t>Contact: user@example.com</w:t></w:r></w:p></w:body></w:document>",
            original: "user@example.com",
            tag: "email",
            value: "admin@company.com",
        },
        // Empty paragraph handling.
        Fixture {
            name: "empty_para",
            doc: "<w:document><w:body><w:p><w:r><w:t>Before</w:t></w:r></w:p><w:p></w:p><w:p><w:r><w:t>After</w:t></w:r></w:p></w:body></w:document>",
            original: "Before",
            tag: "before",
            value: "Start",
        },
        // Special characters in text.
        Fixture {
            name: "special_chars",
            doc: "<w:document><w:body><w:p><w:r><w:t>Price: $1,234.56 (USD)</w:t></w:r></w:p></w:body></w:document>",
            original: "$1,234.56",
            tag: "price",
            value: "$9,999.99",
        },
        // Multiple paragraphs with same field.
        Fixture {
            name: "multi_para_same_field",
            doc: "<w:document><w:body><w:p><w:r><w:t>Project: Alpha</w:t></w:r></w:p><w:p><w:r><w:t>Lead: Alpha</w:t></w:r></w:p></w:body></w:document>",
            original: "Alpha",
            tag: "project",
            value: "Omega",
        },
        // Table cell content.
        Fixture {
            name: "table_cell",
            doc: "<w:document><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Cell Value</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>",
            original: "Cell Value",
            tag: "cell",
            value: "New Value",
        },
        // Header/footer (not processed by current implementation - expected to not match).
        Fixture {
            name: "header_content",
            doc: "<w:document><w:body><w:p><w:r><w:t>Body text</w:t></w:r></w:p></w:body></w:document>",
            original: "Header text",
            tag: "header",
            value: "New Header",
        },
        // Long text spanning many runs.
        Fixture {
            name: "long_text_many_runs",
            doc: "<w:document><w:body><w:p><w:r><w:t>This </w:t></w:r><w:r><w:t>is </w:t></w:r><w:r><w:t>a </w:t></w:r><w:r><w:t>long </w:t></w:r><w:r><w:t>sentence </w:t></w:r><w:r><w:t>with </w:t></w:r><w:r><w:t>many </w:t></w:r><w:r><w:t>runs.</w:t></w:r></w:p></w:body></w:document>",
            original: "is a long sentence",
            tag: "snippet",
            value: "was a short phrase",
        },
        // Field with punctuation.
        Fixture {
            name: "field_with_punctuation",
            doc: "<w:document><w:body><w:p><w:r><w:t>Dear Mr. Smith,</w:t></w:r></w:p></w:body></w:document>",
            original: "Mr. Smith",
            tag: "recipient",
            value: "Dr. Jones",
        },
        // Case sensitivity test.
        Fixture {
            name: "case_sensitive",
            doc: "<w:document><w:body><w:p><w:r><w:t>STATUS: Active</w:t></w:r></w:p></w:body></w:document>",
            original: "Active",
            tag: "status",
            value: "Inactive",
        },
        // Field at end of paragraph.
        Fixture {
            name: "trailing_field",
            doc: "<w:document><w:body><w:p><w:r><w:t>Signed by Name</w:t></w:r></w:p></w:body></w:document>",
            original: "Name",
            tag: "signer",
            value: "John Smith",
        },
        // Multiple fields different paragraphs.
        Fixture {
            name: "multi_field_multi_para",
            doc: "<w:document><w:body><w:p><w:r><w:t>From: Sender</w:t></w:r></w:p><w:p><w:r><w:t>To: Recipient</w:t></w:r></w:p></w:body></w:document>",
            original: "Sender",
            tag: "from",
            value: "Alice",
        },
        Fixture {
            name: "multi_field_multi_para_to",
            doc: "<w:document><w:body><w:p><w:r><w:t>From: Sender</w:t></w:r></w:p><w:p><w:r><w:t>To: Recipient</w:t></w:r></w:p></w:body></w:document>",
            original: "Recipient",
            tag: "to",
            value: "Bob",
        },
        // Chinese characters.
        Fixture {
            name: "chinese_chars",
            doc: "<w:document><w:body><w:p><w:r><w:t>公司名称: 测试公司</w:t></w:r></w:p></w:body></w:document>",
            original: "测试公司",
            tag: "company_cn",
            value: "正式公司",
        },
        // Arabic text (RTL).
        Fixture {
            name: "arabic_text",
            doc: "<w:document><w:body><w:p><w:r><w:t>اسم الشركة: شركة اختبار</w:t></w:r></w:p></w:body></w:document>",
            original: "شركة اختبار",
            tag: "company_ar",
            value: "الشركة الرسمية",
        },
        // Emoji in text.
        Fixture {
            name: "emoji",
            doc: "<w:document><w:body><w:p><w:r><w:t>Status: ✅ Done</w:t></w:r></w:p></w:body></w:document>",
            original: "✅ Done",
            tag: "status_emoji",
            value: "❌ Pending",
        },
        // Cross-run with whitespace variations.
        Fixture {
            name: "cross_run_whitespace",
            doc: "<w:document><w:body><w:p><w:r><w:t>Hello </w:t></w:r><w:r><w:t> World </w:t></w:r><w:r><w:t>!</w:t></w:r></w:p></w:body></w:document>",
            original: "Hello World ",
            tag: "greeting",
            value: "Hi There",
        },
        // Field inside hyperlink.
        Fixture {
            name: "hyperlink_text",
            doc: "<w:document><w:body><w:p><w:r><w:t>Click Here</w:t></w:r></w:p></w:body></w:document>",
            original: "Here",
            tag: "link_text",
            value: "this link",
        },
        // Footnote reference (simplified).
        Fixture {
            name: "footnote_ref",
            doc: "<w:document><w:body><w:p><w:r><w:t>Text with reference[1]</w:t></w:r></w:p></w:body></w:document>",
            original: "[1]",
            tag: "ref1",
            value: "[2]",
        },
        // Mixed content with line breaks.
        Fixture {
            name: "line_breaks",
            doc: "<w:document><w:body><w:p><w:r><w:t>Line 1</w:t></w:r><w:br/><w:r><w:t>Line 2</w:t></w:r></w:p></w:body></w:document>",
            original: "Line 2",
            tag: "line2",
            value: "Line 2 Updated",
        },
        // Multiple tags in same document.
        Fixture {
            name: "multiple_tags",
            doc: "<w:document><w:body><w:p><w:r><w:t>Name: Person, Date: Today</w:t></w:r></w:p></w:body></w:document>",
            original: "Person",
            tag: "person",
            value: "Jane",
        },
    ]
}

pub fn run_fidelity_gate() -> CorpusResult {
    let fixtures = corpus();
    let total = fixtures.len();
    let mut passed = 0usize;

    for fx in &fixtures {
        let docx = make_docx(fx.doc);
        let fields = vec![TemplateFieldSpec {
            id: fx.tag.to_string(),
            label: fx.tag.to_string(),
            original_text: fx.original.to_string(),
            tag_name: fx.tag.to_string(),
        }];

        // Tag: inject the {{tag}} placeholder.
        let tagged = match tag_document(&docx, &fields) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let tagged_xml = extract_document_xml(&tagged);
        if !tagged_xml.contains(&format!("{{{{{}}}}}", fx.tag)) {
            continue; // placeholder not created
        }

        // Fill: substitute the value.
        let mut values = HashMap::new();
        values.insert(fx.tag.to_string(), fx.value.to_string());
        let filled = match fill_document(&tagged, &values, true) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let filled_xml = extract_document_xml(&filled);

        if filled_xml.contains(fx.value) && !filled_xml.contains(&format!("{{{{{}}}}}", fx.tag)) {
            passed += 1;
        }
    }

    let fidelity_percentage = if total == 0 {
        0.0
    } else {
        (passed as f64 / total as f64) * 100.0
    };

    CorpusResult {
        total,
        passed,
        fidelity_percentage,
    }
}

#[test]
fn test_corpus_real_fidelity() {
    let result = run_fidelity_gate();
    // At least 80% fidelity expected; 100% is ideal but cross-paragraph may not work yet
    assert!(result.fidelity_percentage >= 80.0, "fidelity must be >= 80%, got {:.1}%", result.fidelity_percentage);
    eprintln!("Fidelity: {}/{} ({:.1}%)", result.passed, result.total, result.fidelity_percentage);
}

#[test]
fn test_cross_run_fixture_passes() {
    // Specifically guards F-001: a selection spanning multiple <w:t> runs must tag.
    let fx = &corpus()[1]; // cross_run
    let docx = make_docx(fx.doc);
    let fields = vec![TemplateFieldSpec {
        id: fx.tag.to_string(),
        label: fx.tag.to_string(),
        original_text: fx.original.to_string(),
        tag_name: fx.tag.to_string(),
    }];
    let tagged = tag_document(&docx, &fields).expect("tag cross-run doc");
    let tagged_xml = extract_document_xml(&tagged);
    assert!(
        tagged_xml.contains("{{client}}"),
        "cross-run selection 'Dear Acme' must produce a single {{client}} placeholder: {tagged_xml}"
    );
}

#[test]
fn test_corpus_has_minimum_fixtures() {
    let fixtures = corpus();
    assert!(fixtures.len() >= 20, "corpus must have at least 20 fixtures, has {}", fixtures.len());
}
