"""test_wave4_core.py — Verification tests for Wave 4 tasks."""

import os

PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def test_wave4_outputs_exist():
    # TASK-010 output
    ipc_ts = os.path.join(PROJECT_ROOT, "src", "lib", "ipc.ts")
    assert os.path.exists(ipc_ts), "src/lib/ipc.ts must exist"

    # TASK-012 outputs
    export_mod_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "export", "mod.rs")
    export_docx_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "export", "docx.rs")
    export_html_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "export", "html.rs")
    export_dfpkg_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "export", "dfpkg.rs")
    assert os.path.exists(export_mod_rs)
    assert os.path.exists(export_docx_rs)
    assert os.path.exists(export_html_rs)
    assert os.path.exists(export_dfpkg_rs)

    # TASK-016 output
    versioning_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "versioning.rs")
    assert os.path.exists(versioning_rs)


def test_ipc_ts_has_binary_and_typed_error_handling():
    ipc_ts = os.path.join(PROJECT_ROOT, "src", "lib", "ipc.ts")
    with open(ipc_ts, "r", encoding="utf-8") as f:
        content = f.read()

    assert "DocForgeError" in content, "ipc.ts must define DocForgeError class"
    assert "invokeApi" in content, "ipc.ts must define invokeApi"
    assert "invokeBinaryApi" in content, "ipc.ts must define invokeBinaryApi for >1MB payloads"


def test_export_module_features():
    html_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "export", "html.rs")
    with open(html_rs, "r", encoding="utf-8") as f:
        content = f.read()

    assert "render_sanitized_html" in content, "html.rs must sanitize preview HTML"

    dfpkg_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "export", "dfpkg.rs")
    with open(dfpkg_rs, "r", encoding="utf-8") as f:
        dfpkg_content = f.read()

    assert "export_dfpkg" in dfpkg_content, "dfpkg.rs must export dfpkg bundles"
    assert "import_dfpkg" in dfpkg_content, "dfpkg.rs must import dfpkg bundles"


def test_versioning_rollback_and_create():
    versioning_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "versioning.rs")
    with open(versioning_rs, "r", encoding="utf-8") as f:
        content = f.read()

    assert "create_template_version" in content, "versioning.rs must create template versions"
    assert "rollback_template_version" in content, "versioning.rs must support non-destructive rollback"
