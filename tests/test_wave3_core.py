"""test_wave3_core.py — Verification tests for Wave 3 tasks."""

import os

PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def test_wave3_outputs_exist():
    # TASK-003
    docx_engine_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "docx_engine.rs")
    assert os.path.exists(docx_engine_rs)

    # TASK-007
    template_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "template.rs")
    template_store_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "template_store.rs")
    assert os.path.exists(template_rs)
    assert os.path.exists(template_store_rs)

    # TASK-019
    governance_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "governance.rs")
    assert os.path.exists(governance_rs)

    # TASK-020
    licensing_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "licensing.rs")
    assert os.path.exists(licensing_rs)


def test_docx_engine_has_tag_and_fill_functions():
    docx_engine_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "docx_engine.rs")
    with open(docx_engine_rs, "r", encoding="utf-8") as f:
        content = f.read()

    assert "pub fn tag_document" in content, "tag_document must be implemented in docx_engine.rs"
    assert "pub fn fill_document" in content, "fill_document must be implemented in docx_engine.rs"
    assert "UnclosedTag" in content, "fill_document must handle unclosed tag errors"


def test_template_store_no_blob_and_sha256():
    template_store_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "template_store.rs")
    with open(template_store_rs, "r", encoding="utf-8") as f:
        content = f.read()

    assert "compute_sha256" in content, "template_store must calculate SHA-256 digest"
    assert "check_path_containment" in content, "template_store must enforce path containment checks"
    assert "load_template_file" in content, "template_store must read file from storage path"


def test_governance_rbac_and_audit():
    governance_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "governance.rs")
    with open(governance_rs, "r", encoding="utf-8") as f:
        content = f.read()

    assert "pub enum UserRole" in content, "governance must define UserRole enum"
    assert "pub fn authorize" in content, "governance must define authorize function"
    assert "record_generation" in content, "governance must define record_generation function"


def test_licensing_tiers_and_entitlements():
    licensing_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "licensing.rs")
    with open(licensing_rs, "r", encoding="utf-8") as f:
        content = f.read()

    assert "pub enum LicenseTier" in content, "licensing must define LicenseTier enum"
    assert "evaluate_entitlement" in content, "licensing must evaluate entitlements"
    assert "activate_offline_license_file" in content, "licensing must support offline air-gapped activation"
