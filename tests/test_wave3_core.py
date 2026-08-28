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

    # TASK-118: Fidelity gate test file exists
    fidelity_gate_rs = os.path.join(PROJECT_ROOT, "src-tauri", "tests", "fidelity_gate.rs")
    assert os.path.exists(fidelity_gate_rs), "fidelity_gate.rs must exist for TASK-118"


def test_docx_engine_has_tag_and_fill_functions():
    docx_engine_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "docx_engine.rs")
    with open(docx_engine_rs, "r", encoding="utf-8") as f:
        content = f.read()

    assert "pub fn tag_document" in content, "tag_document must be implemented in docx_engine.rs"
    assert "pub fn fill_document" in content, "fill_document must be implemented in docx_engine.rs"
    assert "UnclosedTag" in content, "fill_document must handle unclosed tag errors"
    # Cross-run matching uses paragraph-level text accumulation
    assert "para_text" in content, "docx_engine must accumulate paragraph text for cross-run matching"


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


def test_fidelity_gate_has_real_fixtures():
    """Verify the fidelity gate uses real DOCX fixtures and tests cross-run matching."""
    fidelity_gate_rs = os.path.join(PROJECT_ROOT, "src-tauri", "tests", "fidelity_gate.rs")
    with open(fidelity_gate_rs, "r", encoding="utf-8") as f:
        content = f.read()

    # Must have corpus function with fixtures
    assert "fn corpus()" in content, "fidelity_gate must have corpus() function"
    # Must test cross-run matching (F-001)
    assert "cross_run" in content, "fidelity_gate must test cross-run matching"
    # Must have fill_document round-trip
    assert "fill_document" in content, "fidelity_gate must test fill_document round-trip"
    # Must NOT have hardcoded 100% fidelity
    assert "CorpusResult { total: 50, passed: 50, fidelity_percentage: 100.0 }" not in content, "fidelity_gate must not have hardcoded results"
    # Must use actual tag_document and fill_document
    assert "tag_document(&docx, &fields)" in content, "fidelity_gate must call tag_document"
    assert "extract_document_xml" in content, "fidelity_gate must verify XML output"


def test_v2_bundle_module_exists():
    """Verify v2 Bundle module (TASK-102-105) files and exports exist."""
    bundle_dir = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "bundle")
    assert os.path.exists(bundle_dir), "bundle module directory must exist"
    for fname in ["mod.rs", "manifest.rs", "version.rs", "dfpkg.rs", "output_config.rs"]:
        assert os.path.exists(os.path.join(bundle_dir, fname)), f"bundle/{fname} must exist"


def test_v2_field_mapping_module_exists():
    """Verify v2 Field Mapping module (TASK-106-108) files and exports exist."""
    fm_dir = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "field_mapping")
    assert os.path.exists(fm_dir), "field_mapping module directory must exist"
    for fname in ["mod.rs", "schema.rs", "registry.rs", "groups.rs", "mapping.rs", "extraction.rs"]:
        assert os.path.exists(os.path.join(fm_dir, fname)), f"field_mapping/{fname} must exist"


def test_v2_matter_module_exists():
    """Verify v2 Matter module (TASK-110-113) files and exports exist."""
    matter_dir = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "matter")
    assert os.path.exists(matter_dir), "matter module directory must exist"
    for fname in ["mod.rs", "matter.rs", "matter_values.rs", "form.rs", "validation.rs"]:
        assert os.path.exists(os.path.join(matter_dir, fname)), f"matter/{fname} must exist"


def test_v2_rules_module_exists():
    """Verify v2 Rules module (TASK-114-115) files and exports exist."""
    rules_dir = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "rules")
    assert os.path.exists(rules_dir), "rules module directory must exist"
    for fname in ["mod.rs", "parser.rs", "evaluate.rs"]:
        assert os.path.exists(os.path.join(rules_dir, fname)), f"rules/{fname} must exist"


def test_v2_generation_run_module_exists():
    """Verify v2 Generation Run module (TASK-116-117) files and exports exist."""
    gen_dir = os.path.join(PROJECT_ROOT, "src-tauri", "src", "core", "generation_run")
    assert os.path.exists(gen_dir), "generation_run module directory must exist"
    for fname in ["mod.rs", "record.rs", "execute.rs"]:
        assert os.path.exists(os.path.join(gen_dir, fname)), f"generation_run/{fname} must exist"


def test_v2_tauri_commands_registered():
    """Verify v2 Bundle+Matter Tauri commands are registered in lib.rs."""
    lib_rs = os.path.join(PROJECT_ROOT, "src-tauri", "src", "lib.rs")
    with open(lib_rs, "r", encoding="utf-8") as f:
        content = f.read()

    v2_commands = [
        "commands::create_bundle_v2_cmd",
        "commands::list_bundles_v2_cmd",
        "commands::get_bundle_v2_cmd",
        "commands::create_draft_version_cmd",
        "commands::publish_version_cmd",
        "commands::review_version_cmd",
        "commands::archive_version_cmd",
        "commands::list_versions_cmd",
        "commands::get_manifest_cmd",
        "commands::save_manifest_cmd",
        "commands::export_bundle_dfpkg_cmd",
        "commands::import_bundle_dfpkg_cmd",
        "commands::create_field_cmd",
        "commands::update_field_cmd",
        "commands::list_fields_cmd",
        "commands::remove_field_cmd",
        "commands::create_field_group_cmd",
        "commands::list_field_groups_cmd",
        "commands::create_group_cmd",
        "commands::list_groups_shared_first_cmd",
        "commands::assign_field_to_group_cmd",
        "commands::group_summary_cmd",
        "commands::set_mapping_cmd",
        "commands::list_mappings_cmd",
        "commands::find_unmapped_placeholders_cmd",
        "commands::create_matter_cmd",
        "commands::get_matter_cmd",
        "commands::list_matters_cmd",
        "commands::update_matter_status_cmd",
        "commands::delete_matter_cmd",
        "commands::set_matter_value_cmd",
        "commands::get_matter_value_cmd",
        "commands::list_matter_values_cmd",
        "commands::matter_to_json_cmd",
        "commands::render_matter_form_cmd",
        "commands::populate_matter_field_cmd",
        "commands::validate_matter_cmd",
        "commands::add_rule_cmd",
        "commands::remove_rule_cmd",
        "commands::list_rules_cmd",
        "commands::evaluate_rules_cmd",
        "commands::evaluate_preview_cmd",
        "commands::validate_rule_expression_cmd",
        "commands::execute_run_cmd",
        "commands::create_run_cmd",
        "commands::get_run_cmd",
        "commands::list_runs_cmd",
    ]
    for cmd in v2_commands:
        assert cmd in content, f"Tauri command {cmd} must be registered in lib.rs"
