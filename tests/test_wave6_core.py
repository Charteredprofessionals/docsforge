"""test_wave6_core.py — Verification tests for Wave 6 tasks."""

import os
import json

PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def test_wave6_outputs_exist():
    # TASK-017 / package.json
    package_json = os.path.join(PROJECT_ROOT, "package.json")
    with open(package_json, "r", encoding="utf-8") as f:
        pkg = json.load(f)

    assert "docxtemplater" not in pkg.get("dependencies", {}), "docxtemplater MUST be removed from dependencies (AC-001)"
    assert "pizzip" not in pkg.get("dependencies", {}), "pizzip MUST be removed from dependencies (AC-001)"

    # TASK-021
    gov_service_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "services", "governance.rs")
    assert os.path.exists(gov_service_rs)

    # TASK-022
    telemetry_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "services", "telemetry.rs")
    consent_dialog_tsx = os.path.join(PROJECT_ROOT, "src", "components", "ConsentDialog.tsx")
    assert os.path.exists(telemetry_rs)
    assert os.path.exists(consent_dialog_tsx)

    # TASK-024
    cli_bin = os.path.join(PROJECT_ROOT, "src-tauri", "src", "tools", "docforge_cli.rs")
    if not os.path.exists(cli_bin):
        cli_bin = os.path.join(PROJECT_ROOT, "src-tauri", "src", "bin", "docforge.rs")
    assert os.path.exists(cli_bin)


def test_cli_binary_source():
    cli_bin = os.path.join(PROJECT_ROOT, "src-tauri", "src", "tools", "docforge_cli.rs")
    if not os.path.exists(cli_bin):
        cli_bin = os.path.join(PROJECT_ROOT, "src-tauri", "src", "bin", "docforge.rs")
    with open(cli_bin, "r", encoding="utf-8") as f:
        content = f.read()

    assert "docforge-core" in content
    assert "main()" in content


def test_telemetry_consent_defaults_to_opt_out():
    telemetry_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "services", "telemetry.rs")
    with open(telemetry_rs, "r", encoding="utf-8") as f:
        content = f.read()

    assert "opt_in: false" in content, "Telemetry consent MUST default to opt-out"
