//! regression_v2.rs — TASK-118 v1 template-flow regression gate (REQ-004 / REQ-010).
//!
//! Proves the v1 template path (template_store + docx_engine + versioning) still
//! works on a database that has been migrated to schema v5. Nothing in the v1
//! path is rewritten or deleted; this gate locks that guarantee in place.
//!
//! Exercises: template create/list/load, DOCX validation, placeholder fill,
//! template versioning + rollback, and a 50-fixture tag-fidelity gate.

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

use docforge::core::docx_engine::{fill_document, validate_docx, TemplateFieldSpec};
use docforge::core::template_store::{list_templates, load_template_file, save_template};
use docforge::core::versioning::{create_template_version, rollback_template_version};
use docforge::schema::init_memory_db;

/// Builds a minimal but valid DOCX (PK zip containing `word/document.xml`).
fn minimal_docx(body: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut z = zip::ZipWriter::new(Cursor::new(&mut buf));
        z.start_file("word/document.xml", zip::write::FileOptions::<()>::default())
            .expect("start document.xml");
        z.write_all(body.as_bytes()).expect("write document.xml");
        z.finish().expect("finish zip");
    }
    buf
}

/// Extracts `word/document.xml` text from a filled DOCX for assertion.
fn extract_document_xml(bytes: &[u8]) -> String {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).expect("open generated docx");
    let mut doc_xml = String::new();
    {
        let mut f = archive
            .by_name("word/document.xml")
            .expect("find document.xml");
        f.read_to_string(&mut doc_xml).expect("read document.xml");
    }
    doc_xml
}

#[test]
fn test_v1_template_create_list_load_on_v5_db() {
    let conn = init_memory_db().expect("v5 migration on in-memory db");

    let docx = minimal_docx("<w:document><w:body><w:p><w:r><w:t>Hello {{Company}}</w:t></w:r></w:p></w:body></w:document>");
    let rec = save_template(
        &conn,
        "Agreement",
        "legal",
        "Standard agreement",
        &[],
        &docx,
        None,
        None,
    )
    .expect("save template");

    // List shows exactly the one v1 template.
    let listed = list_templates(&conn, None).expect("list templates");
    assert_eq!(listed.len(), 1, "exactly one template listed");
    assert_eq!(listed[0].id, rec.id);

    // load_template_file round-trips the bytes through at-rest encryption.
    let (meta, bytes) = load_template_file(&conn, &rec.id).expect("load template file");
    assert_eq!(meta.id, rec.id);
    assert_eq!(bytes, docx, "bytes round-trip unchanged");
    validate_docx(&bytes).expect("loaded docx is valid");
}

#[test]
fn test_v1_template_fill_substitutes_placeholders() {
    let conn = init_memory_db().expect("v5 migration");
    let docx = minimal_docx(
        "<w:document><w:body><w:p><w:r><w:t>Dear {{Company}}, ref {{Ref}}</w:t></w:r></w:p></w:body></w:document>",
    );
    let rec = save_template(&conn, "Letter", "general", "Letter", &[], &docx, None, None)
        .expect("save template");

    let (_, bytes) = load_template_file(&conn, &rec.id).expect("load");
    let mut values = HashMap::new();
    values.insert("Company".to_string(), "Acme Pvt Ltd".to_string());
    values.insert("Ref".to_string(), "REF-2026-001".to_string());

    let filled = fill_document(&bytes, &values, true).expect("fill document");
    let xml = extract_document_xml(&filled);
    assert!(xml.contains("Acme Pvt Ltd"), "company substituted: {xml}");
    assert!(xml.contains("REF-2026-001"), "ref substituted: {xml}");
    assert!(!xml.contains("{{Company}}"), "placeholder removed: {xml}");
}

#[test]
fn test_v1_template_versioning_and_rollback() {
    let conn = init_memory_db().expect("v5 migration");
    let v1 = minimal_docx("<w:document><w:body><w:p><w:t>v1 {{X}}</w:t></w:p></w:body></w:document>");
    let rec = save_template(&conn, "Evolving", "general", "Evolving", &[], &v1, None, None)
        .expect("save v1");

    // Create a v2 with different body.
    let v2 = minimal_docx("<w:document><w:body><w:p><w:t>v2 {{X}}</w:t></w:p></w:body></w:document>");
    let ver = create_template_version(&conn, &rec.id, "bump to v2", &v2, &[], None)
        .expect("create version");
    assert_eq!(ver.version, 2);

    // Roll back to v1. Rollback is non-destructive: it appends a new version
    // (v3 here) that copies v1's content, so current_version advances rather
    // than resetting.
    let rolled = rollback_template_version(&conn, &rec.id, 1, None).expect("rollback");
    assert_eq!(rolled.current_version, ver.version + 1, "rollback appends a new current version");

    let (_, bytes) = load_template_file(&conn, &rec.id).expect("load after rollback");
    let xml = extract_document_xml(&bytes);
    assert!(xml.contains("v1 {{X}}"), "rolled back to v1 body: {xml}");
}

#[test]
fn test_v1_fifty_fixture_tag_fidelity_gate() {
    let conn = init_memory_db().expect("v5 migration");

    // Build a document with 50 distinct placeholders.
    let mut body = String::from("<w:document><w:body><w:p><w:r><w:t>");
    let mut expected = Vec::new();
    for i in 0..50 {
        let tag = format!("f{i}");
        body.push_str(&format!("{{{{{tag}}}}} "));
        expected.push(tag);
    }
    body.push_str("</w:t></w:r></w:p></w:body></w:document>");

    let docx = minimal_docx(&body);
    let rec = save_template(&conn, "FiftyFixtures", "general", "50 fixtures", &[], &docx, None, None)
        .expect("save template");
    let (_, bytes) = load_template_file(&conn, &rec.id).expect("load");

    // Fill every fixture with a distinct value.
    let mut values = HashMap::new();
    for (i, tag) in expected.iter().enumerate() {
        values.insert(tag.clone(), format!("val_{i}"));
    }
    let filled = fill_document(&bytes, &values).expect("fill 50 fixtures");
    let xml = extract_document_xml(&filled);

    // Every placeholder must be substituted (no `{{` remaining) and every value present.
    assert!(!xml.contains("{{"), "no unresolved placeholders remain: {xml}");
    for i in 0..50 {
        assert!(
            xml.contains(&format!("val_{i}")),
            "fixture f{i} value missing after fill"
        );
    }
}

#[test]
fn test_v1_tag_document_spec_deserializes() {
    // The v1 TemplateFieldSpec shape is preserved (camelCase contract).
    let json = r#"{"id":"t1","label":"Company","originalText":"{{Company}}","tagName":"Company"}"#;
    let spec: TemplateFieldSpec =
        serde_json::from_str(json).expect("TemplateFieldSpec deserializes camelCase");
    assert_eq!(spec.tag_name, "Company");
    assert_eq!(spec.original_text, "{{Company}}");
}
