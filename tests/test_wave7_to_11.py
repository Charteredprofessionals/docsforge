"""test_wave7_to_11.py — Verification tests for Waves 7 through 11."""

import os
import json

PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def test_enterprise_outputs_exist():
    # Wave 7
    admin_tsx = os.path.join(PROJECT_ROOT, "src", "components", "AdminConsole.tsx")
    rest_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "services", "rest_bridge.rs")
    webhook_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "services", "webhook.rs")
    compliance_md = os.path.join(PROJECT_ROOT, "exports", "compliance_pack.md")
    security_md = os.path.join(PROJECT_ROOT, "exports", "security_whitepaper.md")
    assert os.path.exists(admin_tsx)
    assert os.path.exists(rest_rs)
    assert os.path.exists(webhook_rs)
    assert os.path.exists(compliance_md)
    assert os.path.exists(security_md)

    # Wave 8
    auth_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "services", "auth.rs")
    assert os.path.exists(auth_rs)

    # Wave 9
    policy_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "services", "policy.rs")
    onprem_bin = os.path.join(PROJECT_ROOT, "src-tauri", "src", "bin", "docforge-onprem.rs")
    assert os.path.exists(policy_rs)
    assert os.path.exists(onprem_bin)

    # Wave 10
    release_manifest = os.path.join(PROJECT_ROOT, "exports", "release_manifest.json")
    sbom_json = os.path.join(PROJECT_ROOT, "exports", "sbom.json")
    assert os.path.exists(release_manifest)
    assert os.path.exists(sbom_json)


def test_quality_gate_spec():
    quality_gate_path = os.path.join(PROJECT_ROOT, "exports", "quality_gate.json")
    if os.path.exists(quality_gate_path):
        with open(quality_gate_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        assert data.get("status") in ("passed", "failed", "not_run")
