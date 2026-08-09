"""test_core_contract.py — Contract verification for TASK-001 (docforge-core scaffolding)."""

import os
import pytest

PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def test_core_module_files_exist():
    core_dir = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core")
    assert os.path.exists(core_dir), "src-tauri/src/core directory must exist"
    assert os.path.exists(os.path.join(core_dir, "mod.rs")), "core/mod.rs must exist"
    assert os.path.exists(os.path.join(core_dir, "error.rs")), "core/error.rs must exist"


def test_lib_rs_declares_core():
    lib_path = os.path.join(PROJECT_ROOT, "src-tauri", "src", "lib.rs")
    with open(lib_path, "r", encoding="utf-8") as f:
        content = f.read()
    assert "pub mod core;" in content, "lib.rs must declare pub mod core;"


def test_error_variants_present():
    error_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "error.rs")
    with open(error_rs, "r", encoding="utf-8") as f:
        content = f.read()

    expected_codes = [
        "invalid_docx",
        "zip_bomb",
        "unclosed_tag",
        "unknown_tag",
        "invalid_field_value",
        "storage_missing",
        "storage_io",
        "forbidden",
        "not_published",
        "license_invalid",
        "license_expired",
        "license_limit_exceeded",
        "internal",
    ]

    for code in expected_codes:
        assert f'"{code}"' in content, f"Error variant code '{code}' must be handled in error.rs"
