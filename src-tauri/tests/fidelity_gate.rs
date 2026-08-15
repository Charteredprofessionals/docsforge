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
        let filled = match fill_document(&tagged, &values) {
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
    assert_eq!(result.passed, result.total, "all fidelity fixtures must pass");
    assert_eq!(result.fidelity_percentage, 100.0);
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
