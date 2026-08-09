"""test_wave5_core.py — Verification tests for Wave 5 tasks."""

import os
import json

PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def test_wave5_outputs_exist():
    # TASK-005
    manifest_json = os.path.join(PROJECT_ROOT, "src-tauri", "tests", "fixtures", "manifest.json")
    fidelity_rs = os.path.join(PROJECT_ROOT, "src-tauri", "tests", "fidelity_gate.rs")
    assert os.path.exists(manifest_json)
    assert os.path.exists(fidelity_rs)

    # TASK-011
    pdf_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "export", "pdf.rs")
    print_bridge_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "infra", "print_bridge.rs")
    assert os.path.exists(pdf_rs)
    assert os.path.exists(print_bridge_rs)

    # TASK-013
    tauri_conf = os.path.join(PROJECT_ROOT, "src-tauri", "tauri.conf.json")
    sanitized_preview_tsx = os.path.join(PROJECT_ROOT, "src", "components", "SanitizedPreview.tsx")
    assert os.path.exists(tauri_conf)
    assert os.path.exists(sanitized_preview_tsx)

    # TASK-015
    fields_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "fields.rs")
    types_ts = os.path.join(PROJECT_ROOT, "src", "lib", "types.ts")
    assert os.path.exists(fields_rs)
    assert os.path.exists(types_ts)

    # TASK-014
    services_mod_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "services", "mod.rs")
    assert os.path.exists(services_mod_rs)


def test_csp_is_strict():
    tauri_conf_path = os.path.join(PROJECT_ROOT, "src-tauri", "tauri.conf.json")
    with open(tauri_conf_path, "r", encoding="utf-8") as f:
        config = json.load(f)

    csp = config.get("app", {}).get("security", {}).get("csp")
    assert csp is not None, "CSP must not be null"
    assert "default-src 'self'" in csp, "CSP must enforce default-src 'self'"


def test_corpus_manifest_valid():
    manifest_path = os.path.join(PROJECT_ROOT, "src-tauri", "tests", "fixtures", "manifest.json")
    with open(manifest_path, "r", encoding="utf-8") as f:
        manifest = json.load(f)

    assert manifest["fixtures_count"] == 50
    assert manifest["target_fidelity"] == "100%"


def test_field_types_schema():
    fields_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "fields.rs")
    with open(fields_rs, "r", encoding="utf-8") as f:
        content = f.read()

    assert "enum FieldType" in content
    assert "Text" in content
    assert "Date" in content
    assert "Dropdown" in content
    assert "Checkbox" in content
    assert "Signature" in content
