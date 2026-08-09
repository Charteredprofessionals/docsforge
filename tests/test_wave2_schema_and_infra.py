"""test_wave2_schema_and_infra.py — Contract & Schema tests for Wave 2 tasks."""

import os
import sqlite3

PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def test_wave2_outputs_exist():
    # TASK-002 output
    docx_engine_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "docx_engine.rs")
    assert os.path.exists(docx_engine_rs), "core/docx_engine.rs must exist"

    # TASK-006 outputs
    schema_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "schema.rs")
    migrations_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "migrations.rs")
    assert os.path.exists(schema_rs), "schema.rs must exist"
    assert os.path.exists(migrations_rs), "migrations.rs must exist"

    # TASK-008 outputs
    infra_mod_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "infra", "mod.rs")
    infra_crypto_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "infra", "crypto.rs")
    assert os.path.exists(infra_mod_rs), "infra/mod.rs must exist"
    assert os.path.exists(infra_crypto_rs), "infra/crypto.rs must exist"


def test_migrations_contain_all_13_tables():
    migrations_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "migrations.rs")
    with open(migrations_rs, "r", encoding="utf-8") as f:
        content = f.read()

    expected_tables = [
        "schema_version",
        "orgs",
        "users",
        "templates",
        "template_versions",
        "generation_log",
        "licenses",
        "license_seats",
        "devices",
        "license_files",
        "telemetry_consent",
        "policy_config",
        "webhook_subscriptions",
    ]

    for table in expected_tables:
        assert f"TABLE IF NOT EXISTS {table}" in content, f"Migration must define table '{table}'"

    assert "view_audit_export" in content, "Migration must define view_audit_export"
    assert "prevent_generation_log_update" in content, "Migration must include generation_log update trigger"
    assert "prevent_generation_log_delete" in content, "Migration must include generation_log delete trigger"


def test_docx_engine_validation_safety():
    docx_engine_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "docx_engine.rs")
    with open(docx_engine_rs, "r", encoding="utf-8") as f:
        content = f.read()

    assert "PK\\x03\\x04" in content, "docx_engine must check magic bytes"
    assert "MAX_UNCOMPRESSED_SIZE" in content, "docx_engine must cap uncompressed size"
    assert "MAX_ZIP_ENTRIES" in content, "docx_engine must cap entry count"
    assert "MAX_COMPRESSION_RATIO" in content, "docx_engine must cap compression ratio"
    assert "<!DOCTYPE" in content or "<!ENTITY" in content, "docx_engine must guard against XXE DTDs"
