# DocForge — System Architecture (v2)

> Owner: System Architect | Engine: Authr OS SDLC Studio (Phase 1–4 full re-approval)
> Status: **DRAFT — pending approval gate**
> Scope: Full Phase 1–4 re-approval run for **v2.0.0**. Evolves the RELEASED v1.0.0
> architecture — single-template tagging/filling — into the **Bundle + Matter**
> document-automation domain model, per the master spec. This document is a revision of
> the v1 `architecture.md`, not a from-scratch replacement: the layered architecture
> (L0/L1/L2/L3) and all working v1 decisions carry forward (ADR-001..ADR-009) unless
> explicitly superseded; ADR-010..ADR-013 are new. Section numbering has evolved from v1
> (v1 §18 "Post-Release Additions" content now lives in §21).
> Constraint: the existing React/Tauri shell structure (`src/`, `src-tauri/`) is retained;
> ALL document logic remains in the Rust core. No source code is delivered by this
> document. Items not yet implemented are marked **planned (v2.0.0)**.

---

## 1. System Overview

DocForge is a **deterministic, privacy-first, offline-first professional document-bundle
automation** product. Users bring their **own existing DOCX templates**, organize related
documents into a reusable **Bundle**, mark which values change per matter, enter
**Matter** data once, and DocForge generates all applicable final documents consistently —
Word and PDF — with no document content ever leaving the device unless the user opts in,
and with zero dependence on an LLM in the generation path.

Company Secretarial is a primary vertical, but the architecture is **profession-agnostic**
(lawyers, HR, accountants, real-estate, insurance, procurement). DocForge does **not**
provide professional or legal content: it automates the user's own documents.

The fundamental mental model (from the master spec, non-negotiable):

```
Workspace
 ├── Bundles (Documents, Fields, Mappings, Rules, Versions)
 └── Matters (Matter Data, Selected Bundle Version, Generation Runs, Generated Documents)
```

Core workflow (do not allow drift):

```
USER'S EXISTING FILES → CREATE BUNDLE → IDENTIFY/MARK FIELDS → MAP SHARED DATA
→ SAVE BUNDLE → CREATE NEW MATTER → ENTER DATA ONCE → VALIDATE → GENERATE → FINAL DOCX/PDF
```

The product plan mandates a **headless core**: a single Rust library (`docforge-core`)
owns every document operation, and three shells consume it — the Tauri 2 desktop GUI
(primary), a headless CLI, and an optional enterprise local REST bridge. The GUI must
**never** implement its own document-generation semantics (REQ-040); `docx_engine` in the
Rust core remains the single source of truth for every tag/fill/render operation.

### 1.1 Architectural Drivers

| Driver | Source | Implication |
|---|---|---|
| Kill dual engine | REQ-001, REQ-040, AC-001 | One Rust `docx_engine` owns tag + fill; zero generation logic in JS |
| Cross-run tagging | REQ-002, AC-002 | Run-aware XML replacement; formatting inherits from first run |
| Bundle + Matter domain | REQ-023..REQ-036 | Bundle is a reusable generation definition; Matter is one instance of a Bundle Version; one data entry → many documents |
| Offline-first, no ghost deps | REQ-006, REQ-103, AC-006 | Bundled PDF path; zero required installs on a clean VM |
| Data stays local | REQ-004, REQ-019, REQ-105 | FS-backed storage + SQLite index; DPAPI at rest; zero-knowledge telemetry |
| Enterprise additive | REQ-011..REQ-021 | RBAC, audit, SSO, on-prem, policy files layered on the same core |
| Determinism | REQ-003, REQ-037, out-of-scope AI | Structured tag/rule errors; no LLM and no unrestricted scripting anywhere in the generation path |
| Reproducibility | REQ-024, REQ-033, REQ-034 | Immutable published Bundle Versions; generation-run snapshots; never mutate historical documents |

### 1.2 Definition of Done (from `constraints.json`)

- 100% tag fidelity on the 50-fixture DOCX corpus (unchanged; re-run every v2 phase)
- PDF export on a clean Windows VM with no LibreOffice (unchanged)
- Signed binaries; SBOM per release; quality gate passed before any release candidate
- **planned (v2.0.0):** Bundle → Matter → Generate round-trip produces all applicable
  documents from a single matter data entry (REQ-030), and every run reproduces the exact
  same output given the same immutable Bundle Version + matter data (REQ-024, REQ-033)

---

## 2. Layered Architecture

Four layers, enforced as crate/module boundaries. Each layer depends only on the one
directly below it. The GUI shell retains its current directory (`src/`, `src-tauri/`)
but becomes a thin presentation layer over the core.

```
┌─────────────────────────────────────────────────────────────────────┐
│ L3 SHELL  ── thin presentation, no domain logic                      │
│                                                                     │
│  docforge-gui        docforge-cli         docforge-server (opt.)    │
│  (Tauri 2 + React 18) (Rust binary)       (Rust binary, enterprise) │
│  src/, src-tauri/     headless batch        local REST bridge,      │
│  preview only         bundle/matter/generate webhooks               │
├─────────────────────────────────────────────────────────────────────┤
│ L2 SERVICES ── application orchestration + enforcement              │
│                                                                     │
│  GenerationService   TemplateService   GovernanceService            │
│  BundleService       MatterService     MappingService               │
│  RuleService         GenerationRunService                           │
│  AuthService         LicenseService    TelemetryService             │
│  UpdateService       WebhookService                                 │
│  (RBAC checks · workflow transitions · audit writes · consent)      │
├─────────────────────────────────────────────────────────────────────┤
│ L1 CORE  ── docforge-core (library, no GUI/IO deps except IO ports) │
│                                                                     │
│  docx_engine   template_store   governance   licensing   export     │
│  bundle        field_mapping    matter       rules       generation_run
│  (pure domain: parse/tag/fill, storage, bundle definition,          │
│   canonical schema + mappings, matter data, deterministic rules,    │
│   run records, workflows, entitlement, formats)                     │
├─────────────────────────────────────────────────────────────────────┤
│ L0 INFRASTRUCTURE ── adapters behind interfaces (traits)            │
│                                                                     │
│  SQLite (rusqlite) · DPAPI/keychain crypto · FS template store      │
│  WebView2 print bridge · Sentry (opt-in) · license server client    │
│  (optional cloud) · webhook HTTP client · updater transport         │
└─────────────────────────────────────────────────────────────────────┘
```

**Layer rules (`/clean-code`):**
- Core modules are pure: they take byte slices/domain structs and return results; all
  I/O goes through ports (traits) implemented in L0. This is what lets the core compile
  as a library without GUI dependencies (`constraints.json`).
- Services call core and L0, enforce authorization/consent, and translate domain errors
  into typed transport errors.
- Shells never import `docx_engine` internals; they call services and render results.
- **v2 rule:** the GUI renders the Matter form and the Generation preview, but the
  decision of *which documents generate and how* is made exclusively in the core
  (`rules` → `docx_engine` → `export`), never in React (REQ-040).

---

## 3. Domain Model v2

The v1 model was *template-centric*: a template stands alone, has its own field schema,
and is filled independently. The v2 model is *bundle-centric*: a **Bundle** is a reusable
generation definition, and a **Matter** is one instance of a published Bundle Version.
This section defines the entities and their invariants. Status: **planned (v2.0.0)**;
the v1 model remains operational during the migration (§20).

```
┌───────────────────────────────────────────────────────────────────────────────┐
│ Workspace                                                                    │
│                                                                               │
│  Bundles                                                                      │
│   ├─ Bundle: identity · documents · fields · mappings · rules · output config │
│   ├─ BundleVersion (immutable when published)                                 │
│   │    └─ DocumentTemplate[] (tagged DOCX files)                              │
│   │    ├─ Field / FieldGroup (canonical schema, shared vs document-specific)  │
│   │    ├─ FieldMapping (document {{placeholder}} → canonical Field)           │
│   │    └─ Rule (deterministic conditional document inclusion)                 │
│                                                                               │
│  Matters                                                                      │
│   ├─ Matter (instance of one BundleVersion)                                   │
│   ├─ MatterData (canonical field values — entered ONCE)                       │
│   ├─ GenerationRun (input snapshot · engine version · warnings/errors/status) │
│   └─ GeneratedDocument (output DOCX/PDF, never mutated)                       │
└───────────────────────────────────────────────────────────────────────────────┘
```

### 3.1 Entities

| Entity | Definition | Backing |
|---|---|---|
| **Workspace** | Top-level container; the user's entire document-automation domain. Maps to the local app-data root and, in Business/Enterprise, to an `org` scope. | `orgs` + app-data root |
| **Bundle** | A reusable generation definition: identity (id, name, description), the set of documents it generates, the canonical field schema, the placeholder→field mappings, the conditional rules, the output configuration, and a version history. *Not* a folder of templates — it is the definition that makes "one data entry → many consistent documents" possible. | `bundles` |
| **BundleVersion** | An immutable snapshot of a Bundle's full definition at a point in time. Published versions can never be changed; a change produces a new version. Generation always references the exact version used. | `bundle_versions` |
| **DocumentTemplate** | A single tagged DOCX file owned by the Bundle (the v1 `template_store` entity, now nested under a Bundle). | `templates` / `bundle_documents` |
| **Field** | A canonical, Bundle-level data field: `id, label, description, type, required, default, validation, group, options, format`. Fields are the *shared* vocabulary across all documents in the Bundle (REQ-026). | `fields` |
| **FieldGroup** | A named grouping of fields with a scope: **shared** (same value used across all documents in the Bundle) or **document-specific** (value only relevant to one document). Groups drive the matter-entry form's visual separation (REQ-027). | `field_groups` |
| **FieldMapping** | The explicit, deterministic link between a document's literal placeholder (`{{security_type}}`) and a canonical Field (REQ-028). Mapping is a first-class, validated relation — never ad-hoc string replacement scattered in code. | `field_mappings` |
| **Rule** | A deterministic, Bundle-level conditional-document expression (e.g. `security_type == "equity"`) that decides whether a document is included in a run (REQ-036, REQ-037). | `rules` |
| **Matter** | One instance of a published Bundle Version: a named case/matter/profile for which documents are generated. A Matter is **always** bound to exactly one Bundle Version (REQ-029). | `matters` |
| **MatterData** | The canonical field values for a Matter, entered once and reused across every document in the Bundle (REQ-030). Stored separately from the Bundle definition — a Bundle never contains matter values. | `matter_data` |
| **GenerationRun** | The record of one generation operation: id, matter_id, bundle_id, exact bundle_version, timestamp, input snapshot/hash, engine version, requested documents, output files, warnings, errors, status (REQ-033). | `generation_runs` |
| **GeneratedDocument** | One output artifact of a run (DOCX or PDF). Historical generated documents are never mutated; a new run produces new artifacts (REQ-034). | `generated_documents` |

### 3.2 Invariants

1. **A Matter is an instance of a Bundle Version.** `matters.bundle_version_id` points at
   an immutable `bundle_versions` row; it is set at creation and cannot be silently
   re-pointed by a later publish (REQ-024, REQ-029).
2. **One matter data source → many generated documents.** All documents in a Bundle are
   filled from the same `MatterData`; the user never re-enters data per document
   (REQ-030). Document-specific fields are the only per-document additions, and they are
   still declared on the Bundle's canonical schema (REQ-027).
3. **Matter data is stored separately from the Bundle definition.** A Bundle is reusable
   across any number of matters; importing a Bundle (`.dfpkg`, REQ-025) carries no matter
   values. The same Bundle Version can therefore generate documents for many matters with
   byte-identical behavior (REQ-024, REQ-030).
4. **Published Bundle Versions are immutable.** Any change to documents, fields, mappings,
   rules, or output configuration creates a **new** Bundle Version. Historical Generation
   Runs keep referencing the exact version used, so an old run can always be explained or
   re-executed against the definition it actually used (REQ-024, REQ-033, REQ-034).

### 3.3 Professional Neutrality

The core domain model is **profession-agnostic**. No Company Secretarial, MCA, ROC,
Company-Act, or any other jurisdiction/vertical term is hard-coded into `bundle`,
`field_mapping`, `matter`, `rules`, `generation_run`, or `docx_engine`. Profession-specific
wording exists only as **starter bundles** (sample content shipped as data, not code) so
the same core serves lawyers, HR, accountants, real-estate, insurance, and procurement
without change (REQ-039). This is enforced by code review in the same way AC-001 is:
domain modules contain no legal/CS vocabulary.

---

## 4. Module Boundaries (the 11 approved modules)

Each module is a single-responsibility unit with a documented public surface. Six v1
modules carry forward unchanged in principle; five new modules are **planned (v2.0.0)**.

### 4.1 `docx_engine` (core, L1) — REQ-001, 002, 003, 040
- **Responsibility:** tag and fill DOCX documents; nothing else. The single source of
  truth for every document operation (REQ-040).
- Public surface (Rust):
  - `tag_document(docx: &[u8], selections: &[TagSelection]) -> Result<TaggedDocx, DocForgeError>`
    — replaces user-selected text with `{{tag_name}}` placeholders, merging across `<w:t>`
    runs and preserving formatting of the selection's first run (REQ-002).
  - `fill_document(docx: &[u8], values: &FieldValues, schema: &FieldSchema) -> Result<Vec<u8>, DocForgeError>`
    — replaces placeholders with values, cross-run aware, detecting leftover/unclosed tags
    and returning a structured error (REQ-003, AC-003).
  - `extract_plain_text(docx: &[u8]) -> Result<String, DocForgeError>` — for import/preview.
  - `validate_docx(docx: &[u8]) -> Result<DocxValidation, DocForgeError>` — magic bytes +
    zip structure + bomb/entity limits (REQ-018, AC-018).
  - **planned (v2.0.0):** `scan_placeholders(docx: &[u8]) -> Result<Vec<PlaceholderOccurrence>, DocForgeError>`
    — returns every `{{...}}` literal in a document with its offset and document identity,
    consumed by the mapping layer (REQ-028) and the Bundle Health Check (REQ-038). Filling
    still takes *resolved* canonical values; `docx_engine` never knows about fields,
    matters, or rules.
- **Explicitly out:** rendering HTML (`export::html`), storage, mapping decisions,
  conditional logic, matter data.

### 4.2 `template_store` (core, L1) — REQ-004, 010, 007
- **Responsibility:** persistence of DocumentTemplate files, their versions, and field
  schemas, now consumed by Bundles. v1 public surface is unchanged so existing stored
  templates keep working (REQ-004, §20).
- Public surface:
  - `save(meta, bytes) -> TemplateId`, `load(id, version) -> Vec<u8>`, `list()`,
    `delete(id)`, `create_version(...)`, `rollback(id, to_version)`,
    `resolve(storage_path)`.
- Files live under the app-data `templates/` tree; SQLite stores paths + metadata only
  (REQ-004, AC-004). Per-template lifecycle states (`draft|review|published|archived`)
  remain for standalone templates (REQ-010); **within a Bundle, publication is governed by
  the Bundle Version** (ADR-010). A `.dfpkg` bundle is both the portable unit (REQ-007,
  ADR-013) and a snapshot format for version rollback.

### 4.3 `governance` (core, L1) — REQ-011, 012, 013, 014, 021
- **Responsibility:** RBAC, approval workflow, immutable audit log, admin/usage reports.
  Unchanged from v1. Audit entries now carry `run_id` where a generation run produced them.
- Public surface (unchanged):
  - `authorize(user, role, action) -> Result<(), DocForgeError>` — RBAC matrix
    (viewer/filler/creator/approver/admin) (REQ-011, AC-011).
  - `transition_status(template_id, from, to, actor) -> Result<Status, ...>` (REQ-012).
  - `record_generation(entry: AuditEntry)` / `export_audit(filter) -> ExportableAudit`
    — append-only `generation_log` writer (REQ-013, AC-013).
  - `usage_report(org_id, period) -> AggregateReport` (REQ-014).
- The module remains the only writer of `generation_log`.

### 4.4 `licensing` (core, L1) — REQ-015, 019
- **Responsibility:** entitlement evaluation, device registration, grace windows,
  offline license files, revocation — all zero-knowledge. Unchanged from v1.
  **planned (v2.0.0):** entitlement surfaces for v2 capabilities (bundle count, field
  types, rules, PDF export) gate the same way template-count gates do today.
- Public surface (unchanged): `evaluate_entitlement`, `activate`, `grace_remaining`,
  `revoke`.
- Nothing about document content exists here (REQ-019, ADR-007).

### 4.5 `export` (core, L1) — REQ-006, 007
- **Responsibility:** produce output artifacts. DOCX (identity copy with embedded fields),
  PDF (ADR-005 + ADR-009 native fallback), HTML preview (mammoth → sanitized), `.dfpkg`
  bundle package.
- Public surface: `export_docx(...)`, `export_pdf(docx, renderer) -> Vec<u8>`,
  `render_html_preview(docx) -> SanitizedHtml`, `export_dfpkg(...)`, `import_dfpkg(...)`.
- **planned (v2.0.0):** `.dfpkg` expands from `{document.docx, fields.json, metadata.json,
  version}` to the full Bundle package `{documents/…, fields.json, field_groups.json,
  field_mappings.json, rules.json, output_config.json, metadata.json, version}` (REQ-025,
  ADR-013). Import validates every member (magic bytes, zip structure, schema conformance)
  before any row is written.

### 4.6 `bundle` (core, L1, **new**) — REQ-023, 024, 025, 038
- **Responsibility:** Bundle CRUD, bundle manifest, bundle versioning (immutable published
  versions), bundle import/export (`.dfpkg`), and output configuration. Owns the Bundle
  definition lifecycle end to end; does **not** own value entry or document rendering.
- Public surface (Rust, **planned (v2.0.0)**):
  - `create_bundle(req: NewBundle) -> Bundle`
  - `get_bundle(id: BundleId) -> Bundle` · `list_bundles() -> Vec<BundleSummary>`
  - `update_bundle(id, req: UpdateBundle) -> Bundle` — only on the draft head
  - `delete_bundle(id) -> Result<(), DocForgeError>` — refused while published versions
    are referenced by `generation_runs` (invariant 4)
  - `add_document(bundle_id, docx: &[u8], meta: DocumentMeta) -> DocumentTemplate`
  - `remove_document(bundle_id, template_id) -> Result<(), DocForgeError>`
  - `create_draft_version(bundle_id, note) -> BundleVersion` ·
    `publish_version(bundle_id, note) -> BundleVersion` — publish seals the snapshot
    (ADR-010); `get_version(bundle_id, version) -> BundleVersion` ·
    `list_versions(bundle_id) -> Vec<BundleVersionMeta>`
  - `set_output_config(bundle_id, cfg: OutputConfig) -> OutputConfig` — naming rules,
    default formats (docx/pdf), output folder policy
  - `export_dfpkg(bundle_id, version) -> Vec<u8>` ·
    `import_dfpkg(bytes: &[u8]) -> Bundle` (REQ-025)
  - `health_check(bundle_id) -> BundleHealthReport` (REQ-038, §11)
  - `validate_bundle(bundle_id) -> BundleValidation` (bundle-level validation, §10.3)
- **Explicitly out:** matter data (that is `matter`), placeholder→value replacement
  (`docx_engine`), condition evaluation (`rules`), field schema semantics
  (`field_mapping`).

### 4.7 `field_mapping` (core, L1, **new**) — REQ-026, 027, 028
- **Responsibility:** the canonical field schema (Field/FieldGroup), the explicit
  deterministic mapping layer (document `{{placeholder}}` → canonical field), schema
  validation, defaults, and field types. This is the layer that makes "enter once, reuse
  everywhere" possible — mappings are data, validated and queried, never scattered string
  replacement (ADR-011).
- Public surface (Rust, **planned (v2.0.0)**):
  - `create_field(bundle_version_id, req: NewField) -> Field` ·
    `update_field(id, req: UpdateField) -> Field` · `list_fields(bundle_version_id) -> Vec<Field>` ·
    `remove_field(id) -> Result<(), DocForgeError>` (refused while mapped in a published
    version)
  - `create_field_group(bundle_version_id, name, scope) -> FieldGroup` ·
    `list_field_groups(bundle_version_id) -> Vec<FieldGroup>`
  - `set_mapping(bundle_version_id, template_id, placeholder, field_id) -> FieldMapping`
    · `list_mappings(bundle_version_id) -> Vec<FieldMapping>` ·
    `list_unmapped_placeholders(bundle_version_id) -> Vec<UnmappedPlaceholder>`
    (drives health check, REQ-038)
  - `resolve_value(doc, placeholder, field, matter_data) -> FieldValue` — deterministic
    lookup; unknown/mismatched mapping yields a structured error, never a silent blank
  - `validate_field_schema(fields: &[Field]) -> Vec<SchemaIssue>` (REQ-026)
  - Field types (v2.0.0): `text, multiline_text, number, currency, percentage, date,
    datetime, boolean, email, phone, url, select, multiselect`; **planned (later)**:
    `address, person, company, table, file, signature`.
  - Field attributes (REQ-026): `id, label, description, type, required, default,
    validation, group, options, format`.
- **Explicitly out:** value entry UX (`matter` + `gui_shell`), conditional logic
  (`rules`), document parsing (`docx_engine`).

### 4.8 `matter` (core, L1, **new**) — REQ-029, 030, 031, 032
- **Responsibility:** Matter CRUD, matter data entry (grouped form: shared vs
  document-specific fields), and matter validation. Owns the *instance* side of the
  domain; never touches document bytes.
- Public surface (Rust, **planned (v2.0.0)**):
  - `create_matter(bundle_id, bundle_version_id, name) -> Matter` — binds the immutable
    Bundle Version (REQ-029)
  - `get_matter(id) -> Matter` · `list_matters(filter) -> Vec<MatterSummary>` ·
    `delete_matter(id) -> Result<(), DocForgeError>` (refused while runs exist)
  - `set_matter_data(matter_id, values: MatterValues) -> MatterData` — writes canonical
    values once; recomputes `input_hash` (REQ-030)
  - `get_matter_data(matter_id) -> MatterData`
  - `matter_form_schema(matter_id) -> GroupedFormSchema` — shared group + document-specific
    groups, for the form renderer (REQ-027, REQ-031)
  - `validate_matter(matter_id) -> MatterValidation` — field + matter + bundle levels
    (§10), with exact field identification (REQ-032)
- **Explicitly out:** generation (`generation_run`), mapping/schema editing
  (`field_mapping`), document fill (`docx_engine`).

### 4.9 `rules` (core, L1, **new**) — REQ-036, 037
- **Responsibility:** deterministic conditional documents. Bundle-level inclusion
  conditions expressed in a **safe, deterministic expression system** — comparisons and
  boolean logic over canonical field values only. **No unrestricted scripting** (REQ-037).
- Public surface (Rust, **planned (v2.0.0)**):
  - `add_rule(bundle_version_id, template_id, expression, label) -> Rule` ·
    `list_rules(bundle_version_id) -> Vec<Rule>` · `update_rule(id, …) -> Rule` ·
    `remove_rule(id)`
  - `validate_rule_expression(expr: &str) -> Result<(), RuleError>` — parse + type-check
    against the canonical schema before a rule is stored
  - `evaluate_rules(bundle_version_id, matter_data) -> RuleEvaluation` — per-document
    `Include`/`Exclude { reason }`, deterministic and side-effect-free
  - `evaluate_preview(bundle_version_id, matter_data, requested_documents) -> GenerationPreview`
    (REQ-036, §6.3)
- Expression grammar (v2.0.0): field references, string/number/date/boolean literals,
  `==`, `!=`, `<`, `<=`, `>`, `>=`, `&&`, `||`, `!`, parentheses. Examples:
  `security_type == "equity"`, `document_count >= 2 && fast_track == false`. Unknown
  fields, function calls, loops, and arbitrary code are rejected at parse time. The parser
  is a small hand-rolled deterministic evaluator owned by this module (ADR-011; §17).
- **Explicitly out:** value computation or mutation of data (conditions only), document
  content, cross-bundle rules.

### 4.10 `generation_run` (core, L1, **new**) — REQ-033, 034, 035
- **Responsibility:** generation run records and execution. A run is the unit of
  reproducibility: id, matter_id, bundle_id, exact bundle_version, timestamp, input
  snapshot/hash, engine version, requested vs produced documents, warnings, errors, status
  (REQ-033). Historical runs and their outputs are **never mutated** (REQ-034).
- Public surface (Rust, **planned (v2.0.0)**):
  - `create_run(matter_id, requested_documents: Option<Vec<DocumentId>>) -> GenerationRun`
  - `execute_run(run_id) -> GenerationRun` — orchestrates: load bundle version + matter
    data + mappings; evaluate rules; `docx_engine::fill_document` per included document;
    `export` to DOCX/PDF (REQ-035). Runs in the L2 `GenerationRunService` thread pool.
  - `preview_run(matter_id, requested_documents) -> GenerationPreview` (§6.3)
  - `get_run(run_id) -> GenerationRun` · `list_runs(matter_id) -> Vec<GenerationRun>`
  - `rerun(matter_id, run_id) -> GenerationRun` — creates a **new** run; the historical
    run and its documents are untouched (REQ-034)
  - `output_name(template, run, format) -> String` — deterministic naming rules from the
    Bundle's `OutputConfig` (REQ-035)
  - `run_warnings(run_id) -> Vec<RunWarning>` — e.g. placeholder left empty, optional
    field missing, skipped-by-rule (REQ-036)
- **Explicitly out:** field entry UX (`matter`), mapping schema (`field_mapping`),
  rendering (`docx_engine`/`export`).

### 4.11 `gui_shell` (shell, L3) — REQ-008, 009, 014, 016, 017, 022, 031, 035, 036
- **Responsibility:** the Tauri desktop shell: React 18 screens, Tauri command
  registration, binary IPC, preview rendering, update/consent UX. Registers commands that
  delegate to L2 services; it contains **no** zip/XML processing, no rule evaluation, and
  no generation semantics (AC-001, REQ-040).
- React screens (v1 + v2): Template Creator (REQ-008), Template Filler (REQ-009),
  **My Bundles / Bundle Builder** (REQ-023..028, REQ-031), **Matter Form** (grouped:
  shared vs document-specific, REQ-027), **Generation Preview** (count + skipped-with-
  reason, REQ-035, REQ-036), **Generated Documents**, Template List, Admin Console
  (REQ-014), Consent/Telemetry dialog (REQ-020), Licensing/Paywall (REQ-015).
- **UI/UX principles (v2):**
  1. **Primary journey:** My Bundles → Select Bundle → New Matter → Fill Once → Validate
     → Generate → Finished Documents. Every screen serves this path.
  2. **Navigation follows the mental model:** Dashboard, Bundles, Matters, Generated
     Documents (§3). Matters are always shown beneath the Bundle Version they instantiate.
  3. **Not a generic Word editor.** DocForge is a bundle-automation tool; document
     editing stays at template-authoring time inside the Bundle Builder. The Matter form
     is a data-entry form, not an editor.
  4. **Shared vs document-specific fields are visually distinct** in the Matter form —
     separate groups, clear labels, so users understand which values fan out to many
     documents (REQ-027, REQ-030).
  5. **Preview before generate:** the preview states the number of documents to produce
     and why others are skipped (REQ-036); validation failures are shown inline before any
     run starts.
- The same core is reused by `cli`/`server` shells (REQ-016, AC-016) — gui_shell is the
  reference shell.

---

## 5. Component Diagram (text)

```
                      ┌───────────────────────────────────────────────────┐
                      │            React 18 / TS (preview only)           │
                      │  Dashboard  Bundles  BundleBuilder  Matters       │
                      │  MatterForm  GenerationPreview  Admin  Licensing  │
                      │          (iframe sandbox + DOMPurify)             │
                      └───────────────────────┬───────────────────────────┘
                                              │ typed invoke (binary IPC)
                      ┌───────────────────────▼───────────────────────────┐
                      │              gui_shell (Tauri 2)                  │
                      │   Command Router · capability ACL · CSP           │
                      └───────────┬──────────┬─────────┬─────────┬────────┘
             ┌─────────┐ ┌─────────▼────┐ ┌──▼──────┐ ┌──▼──────┐ ┌──▼──────┐
             │BundleSvc│ │ MatterSvc    │ │MapSvc   │ │RuleSvc  │ │GenRunSvc│
             │ MapSvc  │ │ GenerationSvc│ │GovSvc   │ │AuthSvc  │ │Telemetry│
             └────┬────┘ └──────┬───────┘ └────┬─────┘ └────┬─────┘ └────┬────┘
                  └─────────────┴──────┬───────┴─────────────┴───────────┘
                         ┌─────────────▼──────────────────────────────┐
                         │              docforge-core                 │
                         │  docx_engine  template_store  bundle        │
                         │  field_mapping matter  rules  generation_run│
                         │  governance   licensing   export            │
                         └─────────────┬──────────────────────────────┘
             ┌─────────────────────────┼──────────────────────────┐
   ┌─────────▼────────┐ ┌──────────────▼───────┐ ┌────────────────▼─────────┐
   │ SQLite (rusqlite)│ │ FS template store    │ │ Print bridge             │
   │ WAL · FK · audit │ │ app-data/templates   │ │ (WebView2 headless)      │
   │                  │ │ DPAPI-encrypted      │ │ + native fallback        │
   └──────────────────┘ └──────────────────────┘ │ (ADR-005, ADR-009)       │
                                                 └──────────────────────────┘
   ┌──────────────────────┬──────────────────────┬──────────────────────┐
   │ Optional cloud:      │  Webhooks (ent.)     │  Sentry (opt-in)     │
   │ license issuance ·   │  / REST bridge       │  crash/aggregate     │
   │ seat mgmt (no docs)  │  localhost:PORT      │  consent-gated       │
   └──────────────────────┴──────────────────────┴──────────────────────┘
```

Data never flows from the bottom row back into the document path: the license/telemetry
cloud surfaces are zero-knowledge (ADR-007). The `rules` and `generation_run` modules are
reached only through services; the React layer never evaluates a rule or assembles a
document.

---

## 6. Data Flow

### 6.1 Bundle creation flow (7 steps, from the master spec)

1. **Bring existing files.** User imports their own DOCX files via the native dialog
   (`tauri-plugin-dialog`); only picker-confirmed paths enter the FS (REQ-018). Each file
   passes `docx_engine::validate_docx` (magic bytes, zip structure, bomb caps) and is
   stored by `template_store::save` as a DocumentTemplate (REQ-004).
2. **Create the Bundle.** `bundle::create_bundle` records identity + output configuration;
   documents are attached in order (`bundle::add_document`). Status: draft head.
3. **Identify/mark fields.** In the Bundle Builder, the user tags placeholders in each
   document via `docx_engine::tag_document` (cross-run aware, REQ-002). Placeholders are
   literal `{{...}}` spans.
4. **Map shared data.** The user defines canonical fields and field groups
   (`field_mapping::create_field`, `create_field_group`), then maps each placeholder to a
   canonical field (`field_mapping::set_mapping`) — the explicit deterministic mapping
   layer (ADR-011). Unmapped placeholders are surfaced immediately
   (`list_unmapped_placeholders`).
5. **Add rules (optional).** Bundle-level conditional documents via `rules::add_rule`
   (e.g. `security_type == "equity"`); expressions are validated at parse time (REQ-037).
6. **Save and validate the Bundle.** `bundle::validate_bundle` runs bundle-level
   validation (§10.3): unresolved placeholders, invalid mappings/rules, missing templates
   — each reported with exact document + field identity (REQ-032, REQ-038).
7. **Publish a Bundle Version.** `bundle::publish_version` seals the immutable snapshot
   (ADR-010). From this point the Bundle is reusable: any number of matters can be created
   against this exact version (REQ-024, REQ-029).

### 6.2 Matter flow

1. **Select Bundle (and Version).** `matter::create_matter(bundle_id, bundle_version_id,
   name)` instantiates the chosen published Bundle Version (REQ-029). The UI always shows
   which version a Matter uses.
2. **Grouped form.** `matter::matter_form_schema` returns the canonical fields grouped by
   scope — shared fields first (fan out to every document), then document-specific groups
   (REQ-027). Shared vs document-specific is visually distinct (UI principle 4).
3. **Enter data once.** `matter::set_matter_data` stores canonical values once; the
   `input_hash` is recomputed (REQ-030).
4. **Validate.** `matter::validate_matter` runs field + matter + bundle validation with
   inline, per-field messages (§10).
5. **Preview.** `generation_run::preview_run` returns the number of documents to generate
   and the skipped documents with their rule reasons (REQ-036, §6.3).
6. **Generate.** `generation_run::create_run` + `execute_run` produce the final DOCX/PDF
   artifacts (§6.3).

### 6.3 Generation flow

```
UI (Matter form / preview)
  → L2 GenerationRunService
    → domain: load Bundle Version (immutable) + MatterData + FieldMappings
    → rules::evaluate_rules          (include / exclude per document, with reason)
    → docx_engine::fill_document     (per included document; resolves canonical values)
    → export::export_docx / export_pdf
  → generation_run::execute_run      (records input snapshot, engine version, outputs,
                                      warnings, errors, status)
  → GeneratedDocument[] → Finished DOCX/PDF
```

- The React layer issues the *request* (matter + optional document selection) and renders
  the *result*; it never decides which documents generate or how a placeholder is filled
  (REQ-040).
- `docx_engine` receives *resolved* canonical values — mapping resolution already happened
  in `field_mapping` (ADR-011).
- **Generation preview semantics (REQ-036):** the preview reports (a) the **number of
  documents to generate**; (b) each **document skipped by a condition with its human
  reason** (rule label/expression); (c) validation status per document. The preview is
  computed by `evaluate_rules` + schema validation — the same code path the run uses, so
  the preview never lies.
- **Run immutability (REQ-033, REQ-034):** every run stores `bundle_version`, `input_hash`
  (SHA-256 of canonicalized matter data), and `engine_version`. Reruns create new runs;
  historical run records and their generated documents are never modified.

### 6.4 Export flow

1. `export` runs in the `GenerationRunService` thread pool (REQ-101: never on the UI
   thread; 10MB target tag/fill < 2s).
2. **DOCX:** byte-copy of the filled package (zip identity, metadata untouched).
3. **PDF:** WebView2 headless print-to-PDF primary (ADR-005) with native `printpdf` +
   `docx-rs` fallback (ADR-009); LibreOffice never required (REQ-006, AC-006).
4. **HTML preview:** mammoth HTML passes through DOMPurify and renders in a sandboxed
   iframe under strict CSP (REQ-017, ADR-006).
5. **.dfpkg (planned v2.0.0):** full Bundle package — documents, canonical schema, field
   groups, mappings, rules, output config, metadata, version (REQ-025, ADR-013).
6. Every export writes its audit entry with `format` recorded (REQ-013).

### 6.5 Governance flow

Unchanged from v1: `draft → review → published → archived`; only `approver`/`admin` may
publish; only published templates are fillable by `viewer`/`filler` (REQ-012, AC-012).
Within Bundles, publication is expressed by the immutable **Bundle Version** (ADR-010);
standalone v1 templates keep the per-template lifecycle during migration (§20).

---

## 7. API Boundaries

### 7.1 Tauri command surface (gui_shell, typed)

All commands return `Result<T, DocForgeError>` serialized as structured JSON (§9).
Payloads >1MB travel as raw bytes over binary IPC (ADR-004); no Base64 in the hot path
(REQ-005, AC-005). v1 commands are retained unchanged (backwards compatibility); v2
commands are **planned (v2.0.0)**.

| Command | Arguments | Returns | Module/Service |
|---|---|---|---|
| `upload_docx` | `path: string` (picker-confirmed) | `DocxPreview { text, size }` | GenerationService |
| `tag_template` | `bytes: Uint8Array`, `selections: TagSelection[]` | `TemplateDraft { id, fields }` | docx_engine + template_store |
| `save_template` / `list_templates` / `get_template` | `TemplateDraft` / filters / `{ id, version? }` | `TemplateId` / `TemplateMeta[]` / `TemplateDetail` | TemplateService |
| `update_template_status` / `create_template_version` / `rollback_template` / `delete_template` | lifecycle ops | `Status` / `Version` / `()` | GovernanceService / TemplateService |
| `fill_template` | `{ id, version?, values }` | `FilledResult { outPath }` | GenerationService |
| `export_document` / `render_preview` | `{ id, format, options }` / `{ id, version? }` | `ExportArtifact` / `SanitizedHtml` | ExportService |
| `create_bundle` | `{ name, description? }` | `Bundle` | BundleService |
| `list_bundles` / `get_bundle` / `delete_bundle` | `{}` / `{ id }` / `{ id }` | `BundleSummary[]` / `BundleDetail` / `()` | BundleService |
| `add_bundle_document` / `remove_bundle_document` | `{ bundleId, docx: Uint8Array, name }` / `{ bundleId, templateId }` | `DocumentTemplate` / `()` | BundleService |
| `create_draft_version` / `publish_bundle_version` / `list_bundle_versions` | `{ id, note? }` | `BundleVersion` / `BundleVersionMeta[]` | BundleService |
| `export_dfpkg` / `import_dfpkg` | `{ bundleId, version? }` / `bytes: Uint8Array` | `Vec<u8>` / `Bundle` | BundleService |
| `create_field` / `update_field` / `list_fields` | `{ bundleVersionId, … }` | `Field` / `Field[]` | MappingService |
| `create_field_group` / `list_field_groups` | `{ bundleVersionId, name, scope }` | `FieldGroup` / `FieldGroup[]` | MappingService |
| `set_field_mapping` / `list_unmapped_placeholders` | `{ bundleVersionId, templateId, placeholder, fieldId }` / `{ bundleVersionId }` | `FieldMapping` / `UnmappedPlaceholder[]` | MappingService |
| `add_rule` / `list_rules` / `validate_rule_expression` | `{ bundleVersionId, … }` | `Rule` / `Rule[]` / `()` | RuleService |
| `create_matter` | `{ bundleId, bundleVersionId, name }` | `Matter` | MatterService |
| `get_matter` / `list_matters` / `delete_matter` | `{ matterId }` / filters / `{ matterId }` | `Matter` / `MatterSummary[]` / `()` | MatterService |
| `set_matter_data` / `get_matter_data` | `{ matterId, values }` / `{ matterId }` | `MatterData` | MatterService |
| `matter_form_schema` | `{ matterId }` | `GroupedFormSchema` | MatterService |
| `validate_matter` | `{ matterId }` | `MatterValidation` | MatterService |
| `preview_generation` | `{ matterId, documentIds? }` | `GenerationPreview` | GenerationRunService |
| `generate` | `{ matterId, documentIds? }` | `GenerationRun` | GenerationRunService |
| `list_generation_runs` / `get_generation_run` | `{ matterId }` / `{ runId }` | `GenerationRun[]` / `GenerationRunDetail` | GenerationRunService |
| `bundle_health_check` | `{ bundleId }` | `BundleHealthReport` | BundleService |
| `export_generated_documents` | `{ runId, format }` | `ExportArtifact[]` | ExportService |
| `list_users` / `set_user_role` / `export_audit` / `usage_report` | admin | unchanged | GovernanceService |
| `activate_license` / `get_entitlement` | unchanged | unchanged | LicenseService |
| `set_telemetry_consent` / `authenticate` | unchanged | unchanged | TelemetryService / AuthService |

Capabilities (ACL) are declared in `src-tauri/capabilities/default.json`; every command
is registered under least privilege. RBAC enforcement is **server-side in Rust**, never
trusted from the renderer (REQ-011, AC-011).

### 7.2 CLI surface (`docforge`, headless — REQ-016)

```
docforge generate --template <id|path> --data data.json --out out.docx [--format docx|pdf|dfpkg]
docforge template list | import <file.docx> | export <id> --format dfpkg
docforge fill --template <id> --values values.json --out out.docx
docforge bundle create | list | show <id> | export <id> --format dfpkg | import <file.dfpkg>
docforge bundle health <id>                          # planned (v2.0.0)
docforge bundle publish <id> [--note "…"]           # planned (v2.0.0)
docforge matter create <bundle> <version> <name> | list | show <id> | validate <id>
docforge generate --matter <id> [--document <id> …] [--format docx|pdf]
docforge runs list --matter <id> | show <runId>
docforge audit export --org <id> --out audit.csv
docforge license activate <key|file> | status | deactivate
docforge config show | set <key> <value>            # policy-file overlay
docforge serve [--port 0]                           # optional enterprise REST bridge
```

Exit codes: `0` success, `2` usage error, `3` validation/tag error (structured JSON to
stderr for scripting), `4` license/entitlement error, `5` storage/IO error.

### 7.3 Optional local REST bridge (enterprise — REQ-016)

Bound to `127.0.0.1`, enabled only in Business/Enterprise tiers, bearer-token
authenticated with RBAC passthrough. v2 endpoints are **planned (v2.0.0)**.

| Method/Path | Purpose |
|---|---|
| `POST /v1/generate` | matter or template + JSON data → output artifact |
| `GET /v1/templates` / `POST /v1/templates` | library browse / import |
| `GET /v1/bundles` / `POST /v1/bundles` | bundle browse / create |
| `GET /v1/bundles/{id}` / `POST /v1/bundles/{id}/publish` | bundle detail / publish version |
| `GET /v1/matters` / `POST /v1/matters` / `GET /v1/matters/{id}` | matter browse / create / detail |
| `POST /v1/matters/{id}/validate` / `POST /v1/matters/{id}/generate` | validate / generate |
| `GET /v1/runs?matter=` / `GET /v1/runs/{id}` | run history / detail |
| `POST /v1/webhooks` | register generation-event webhook |
| `GET /v1/audit?since=` | pull audit trail (enterprise) |
| `GET /v1/health` | liveness (no document data) |

The bridge is compiled into `docforge-server` and, when enabled, is reachable from the
desktop shell — same core, same auth, same audit.

---

## 8. Security Model

| Concern | Mechanism | Requirement |
|---|---|---|
| Renderer policy | Strict CSP replacing `"csp": null` (ADR-006); `script-src 'self'`, no `unsafe-inline`/`unsafe-eval`; `frame-src` limited to sandboxed preview iframe | REQ-017, AC-017 |
| HTML preview | mammoth output → DOMPurify → sandboxed iframe; never `dangerouslySetInnerHTML` with unsanitized content | REQ-017 |
| DOCX validation | Magic bytes (`PK\x03\x04`), zip structure, entry count + compression-ratio caps (zip-bomb guard), XML entity/namespace limits, reject non-docx with precise errors | REQ-018, AC-018 |
| `.dfpkg` import (planned v2.0.0) | Same zip validation as DOCX, plus manifest schema conformance and placeholder/schema consistency checks; no path strings from the package are ever used for FS writes (REQ-025) | REQ-018, REQ-025 |
| Rule expressions | Deterministic DSL parsed/type-checked in `rules`; no function calls, no loops, no arbitrary code (REQ-037); evaluation is side-effect-free and bounded | REQ-037 |
| Path safety | Only picker-confirmed paths enter the FS; canonical-path + containment checks; no user string concatenated into paths | REQ-018 |
| At-rest data | Windows: DPAPI-encrypted template files (AC-019); macOS: Keychain; SQLite stores metadata only | REQ-019, REQ-004 |
| IPC | Binary IPC (ADR-004); Tauri capability ACL per command; arguments validated in Rust | REQ-005 |
| Licensing | Zero-knowledge: license checks carry activation facts only, never document bytes or field values (ADR-007) | REQ-019 |
| Telemetry | Consent-gated, aggregate-only, redaction pipeline; enterprise build disables entirely (ADR-007) | REQ-020, AC-020 |
| AuthN/AuthZ | Local identity + optional SAML/SSO token binding; RBAC enforced in services layer for every command and CLI/REST path | REQ-011, REQ-021 |
| Supply chain | EV code signing, signed updates, SBOM per release | REQ-022 |
| Secrets | None in config; API keys/tokens via DPAPI-protected settings store | — |

---

## 9. Error Handling — Structured Errors

A single typed error enum in `docforge-core` (`DocForgeError`) serializes to a stable
JSON contract shared by GUI, CLI, and REST:

```json
{ "code": "unclosed_tag", "message": "Template contains an unclosed tag",
  "detail": { "tag_name": "recipient_name", "offset": 14832 } }
```

| `code` | Meaning | Requirement |
|---|---|---|
| `invalid_docx` | magic bytes/zip/XML structure failed validation | REQ-018 |
| `zip_bomb` | entry count or compression ratio exceeded limits | REQ-018 |
| `unclosed_tag` | `{{` without `}}` after fill pass — no artifact produced | REQ-003, AC-003 |
| `unknown_tag` | value set for tag not present in schema | REQ-009 |
| `invalid_field_value` | `{ field_id, reason }` per-field validation failure | REQ-009, REQ-032 |
| `unknown_placeholder` / `unmapped_placeholder` | placeholder in a document has no mapping in the Bundle version | REQ-028, REQ-032 |
| `duplicate_mapping` | placeholder already mapped to another field in this version | REQ-028 |
| `invalid_rule_expression` | expression failed parse/type-check (`{ rule_id, reason }`) | REQ-037 |
| `rule_field_unknown` | rule references a field not in the canonical schema | REQ-037 |
| `invalid_field_schema` | `{ field_id, reason }` schema-level violation (REQ-026) | REQ-026 |
| `bundle_not_published` | attempted to create a Matter against an unpublished draft version | REQ-024, REQ-029 |
| `bundle_version_immutable` | attempted to edit a published version — create a new one | REQ-024 |
| `bundle_version_mismatch` | run/matter references a version inconsistent with the bundle head | REQ-024 |
| `matter_incomplete` | matter failed validation; `{ issues: [...] }` blocks generation | REQ-032 |
| `matter_data_hash_mismatch` | stored matter data hash disagrees on load (integrity) | REQ-033 |
| `missing_template` | a Bundle document's file is missing/unreadable at generation | REQ-038 |
| `storage_missing` / `storage_io` | template/version file missing or unreadable | REQ-004/010 |
| `forbidden` | RBAC violation (`{ required_role }`) | REQ-011 |
| `not_published` | non-creator attempted to fill a draft | REQ-012 |
| `license_*` | `not_entitled`, `device_limit`, `grace_expired`, `invalid_key` | REQ-015 |
| `internal` | invariant failure (bug); includes correlation id for telemetry | — |

Rules: fail fast, never emit a partially-filled artifact, never leak document content
into error text or logs. Frontend maps `code` → per-field inline messages (REQ-009).
Validation issues from §10 carry `{ document_id?, field_id?, code, message }` so the UI
can navigate to the exact document + field.

---

## 10. Validation & Diagnostics — Three Levels

Validation is intentionally **three levels**, each with a distinct scope and exact
identification in diagnostics (REQ-032). Status: **planned (v2.0.0)**.

### 10.1 Field validation (`field_mapping`)

Per-field, per-value, at schema time and at entry time.

- **Schema-time:** type is one of the supported field types; `required`/`default`/`options`
  conform to the type (e.g. a `select` without `options` is invalid); `validation`
  constraints are parseable (min/max/pattern/range); group references exist (REQ-026).
- **Value-time:** value matches the field type and format (email, phone, url, date,
  datetime, number, currency, percentage, boolean, select option membership, multiselect
  subset); required fields must be present; defaults applied for missing optional fields.
- Diagnostics: `{ field_id, label, code, message }` mapped to the exact form field.

### 10.2 Matter validation (`matter`)

Cross-field validation over a Matter's data, evaluated against the selected Bundle Version.

- All **required shared fields** across the Bundle are present (REQ-030).
- Cross-field consistency (type-safe only — no legal/domain rules in core, REQ-039):
  e.g. dates compare correctly, currency fields are numeric.
- Fields referenced by **any rule expression** must be present and well-typed, so rule
  evaluation never depends on missing data (REQ-037).
- Diagnostics: `{ field_id?, group_id, code, message }` — the Matter form highlights each
  failing group/field inline (REQ-031).

### 10.3 Bundle validation (`bundle`)

Definition-level validation, run at save/publish time and on every Matter validation.

- **Unresolved placeholders:** every `{{...}}` literal in every Bundle document has a
  mapping to a canonical field (`scan_placeholders` vs `field_mappings`); a placeholder
  that is *in* the document but *not* in the schema is reported (REQ-028).
- **Invalid mappings:** mapping targets a field that does not exist in this version, or a
  placeholder is mapped to a field of an incompatible type (e.g. a date field mapped into
  a placeholder inside a numeric cell).
- **Invalid rules:** expressions fail parse/type-check, or reference unknown fields
  (REQ-037).
- **Missing templates:** a Bundle document's template file is missing, unreadable, or its
  `content_sha256` disagrees (REQ-004, REQ-038).
- Diagnostics identify **exact document + field**: `{ document_id, document_name,
  field_id?, placeholder?, code, message }` (§9).

---

## 11. Bundle Health Check (first-class diagnostic)

`bundle::health_check` produces a single, exportable diagnostic report for a Bundle
(REQ-038). Status: **planned (v2.0.0)**.

```
bundle_health_check(bundle_id) -> BundleHealthReport {
  bundle_id, bundle_name, bundle_version (head), generated_at,
  document_count, field_count, field_group_count,
  mapped_placeholders, unmapped_placeholders, unknown_placeholders,
  invalid_rules, invalid_mappings, missing_templates,
  status: healthy | attention | unhealthy,
  documents: [
    { document_id, document_name, status, placeholder_count,
      mapped_placeholders, unmapped_placeholders,
      issues: [{ field_id?, placeholder?, code, message }] }
  ],
  fields: [
    { field_id, label, group, type, required, mapped_from: [ {document_id, placeholder} ],
      issues: [] }
  ],
  rules: [ { rule_id, template_id, expression, status, reason } ]
}
```

- Report items carry **exact document + field identification**, so the UI can jump from a
  diagnostic straight into the Bundle Builder at the offending placeholder (REQ-038).
- Health is checked: after every publish, on Matter creation against a version, and
  on-demand from the Bundles screen / `docforge bundle health <id>`.
- `status: healthy` — generation-ready; `attention` — warnings (e.g. unmapped
  placeholders) but generation possible; `unhealthy` — generation blocked (missing
  templates, invalid rules/mappings).

---

## 12. Observability

- **Consent:** first-run dialog explains what is collected (counts, timing, crash
  metadata) and what is never collected (document contents, field values, matter names).
  Opt-in only (REQ-020, AC-020).
- **Crash reporting:** Sentry, consent-gated, DSN stripped from enterprise builds.
- **Aggregate analytics:** events like `generation.completed {duration_ms, format,
  document_count, skipped_count}` with no PII/document identity; locally buffered, flushed
  in aggregate.
- **Enterprise:** telemetry + crash upload compiled out; policy file can force-disable
  (REQ-020).
- **Redaction pipeline:** any event payload passes a content-free allowlist before egress
  (ADR-007); verified via `code_review` (AC-019/AC-020).
- **planned (v2.0.0):** run-scoped counters (`rules.evaluated`, `documents.skipped`,
  `runs.rerun`) feed the existing aggregate pipeline; they are counts only — never values.

---

## 13. Scalability & the Headless Core

- **Single implementation, three shells:** `docforge-core` compiles without GUI deps
  (`constraints.json`). The CLI and local REST server reuse the identical
  bundle/matter/generate code — REQ-016/AC-016 prove the headless path (ADR-001).
- **Concurrency:** generation runs on a dedicated thread pool behind the services layer;
  the WebView never blocks (REQ-101). `rules` evaluation is pure and thread-safe.
- **Large files:** >10MB payloads use binary IPC + chunked/streaming save (REQ-005,
  ADR-004); quick-xml is streaming (constant memory on `document.xml`).
- **SQLite:** WAL mode, foreign keys, single-writer; metadata-only so the DB stays small;
  indexes on `template_versions.template_id`, `generation_log(generated_at)`,
  `generation_runs(matter_id, created_at)`, `bundle_versions(bundle_id)`.
- **Bundle reuse:** one published Bundle Version serves many matters without any copy —
  matters store values only, so storage cost scales with matter data, not documents
  (REQ-029, REQ-030).
- **Multi-user (Business/Enterprise):** org scoping via `org_id` on every query; shared
  library served from a local/networked shared store in later phases (connectors are
  post-GA per product plan; storage stays behind the `template_store` port).

---

## 14. Deployment Model

| Channel | Packaging | Notes |
|---|---|---|
| Consumer | Signed **MSIX** (Store + sideload), **EXE** (web), winget manifest | EV code signing; auto-update `stable` channel |
| Company | **MSI** for Intune/WSUS/GPO with silent install + JSON policy file | Policy file: telemetry, update channel, SSO IdP, license pool |
| Enterprise | MSI + offline update channel (no internet required) | On-prem/air-gapped; SBOM + compliance pack shipped per release |

- **Auto-update:** signed manifests, staged rollout (e.g. 5% → 25% → 100%), one-click
  rollback to the previous signed build, `stable`/`beta` channels (REQ-022, AC-022).
  Enterprise channel supports manual offline update payloads.
- **SBOM:** SPDX/CycloneDX generated by the SDLC Studio exporter and attached to every
  release manifest (REQ-022, release definition).
- **Signing:** EV code signing on Windows; notarization on macOS; all binaries and
  update payloads signature-verified before execution.
- **planned (v2.0.0):** `.dfpkg` bundles are fully offline-portable (REQ-025) — moving a
  Bundle between machines requires no cloud, only the package file.

---

## 15. Database Design (Data Model v3 — target schema v5)

SQLite via rusqlite (bundled). All existing v1 tables (`orgs`, `users`, `templates`,
`template_versions`, `generation_log`, `licenses`, `license_seats`, `devices`,
`license_files`, `telemetry_consent`, `policy_config`, `webhook_subscriptions`,
`schema_version`) remain unchanged. The v2 domain adds the tables below. The migration
from the current shipped schema (v4: Data Model v2 + template bundles + bug book) to the
**schema v5** target is specified in `schema.md`; the DDL here is the authoritative target
shape. Status: **planned (v2.0.0)**.

```sql
-- §3.1 Bundle — reusable generation definition (REQ-023)
CREATE TABLE bundles (
  id             TEXT PRIMARY KEY,
  org_id         TEXT NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
  name           TEXT NOT NULL COLLATE NOCASE,
  description    TEXT,
  output_config_json TEXT NOT NULL DEFAULT '{}',   -- naming rules, formats, folder policy
  head_version   INTEGER NOT NULL DEFAULT 0,       -- MAX(bundle_versions.version)
  created_by     TEXT NOT NULL REFERENCES users(id),
  created_at     TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at     TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE (org_id, name)
);

-- §3.1 BundleVersion — immutable once published (REQ-024, ADR-010)
CREATE TABLE bundle_versions (
  id               TEXT PRIMARY KEY,
  bundle_id        TEXT NOT NULL REFERENCES bundles(id) ON DELETE CASCADE,
  version          INTEGER NOT NULL,
  status           TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','published')),
  manifest_json    TEXT NOT NULL,                  -- full immutable snapshot of the definition
  note             TEXT,
  created_by       TEXT NOT NULL REFERENCES users(id),
  created_at       TEXT NOT NULL DEFAULT (datetime('now')),
  published_by     TEXT REFERENCES users(id),
  published_at     TEXT,
  UNIQUE (bundle_id, version)
);

-- §3.1 bundle membership: DocumentTemplate rows (REQ-023)
CREATE TABLE bundle_documents (
  id                 TEXT PRIMARY KEY,
  bundle_version_id  TEXT NOT NULL REFERENCES bundle_versions(id) ON DELETE CASCADE,
  template_id        TEXT NOT NULL REFERENCES templates(id),
  order_index        INTEGER NOT NULL DEFAULT 0,
  is_conditional     INTEGER NOT NULL DEFAULT 0 CHECK (is_conditional IN (0,1)),
  UNIQUE (bundle_version_id, template_id)
);

-- §3.1 FieldGroup — shared vs document-specific (REQ-027)
CREATE TABLE field_groups (
  id                 TEXT PRIMARY KEY,
  bundle_version_id  TEXT NOT NULL REFERENCES bundle_versions(id) ON DELETE CASCADE,
  name               TEXT NOT NULL,
  scope              TEXT NOT NULL CHECK (scope IN ('shared','document_specific')),
  sort_order         INTEGER NOT NULL DEFAULT 0,
  UNIQUE (bundle_version_id, name)
);

-- §3.1 Field — canonical schema (REQ-026)
CREATE TABLE fields (
  id                 TEXT PRIMARY KEY,
  bundle_version_id  TEXT NOT NULL REFERENCES bundle_versions(id) ON DELETE CASCADE,
  key                TEXT NOT NULL,                -- canonical field id, e.g. "security_type"
  label              TEXT NOT NULL,
  description        TEXT,
  field_type         TEXT NOT NULL CHECK (field_type IN
    ('text','multiline_text','number','currency','percentage','date','datetime',
     'boolean','email','phone','url','select','multiselect')),
  group_id           TEXT REFERENCES field_groups(id),
  required           INTEGER NOT NULL DEFAULT 0 CHECK (required IN (0,1)),
  default_value      TEXT,
  validation_json    TEXT NOT NULL DEFAULT '{}',   -- min/max/pattern/range per type
  options_json       TEXT,                         -- select/multiselect options
  format             TEXT,                         -- display format hint
  UNIQUE (bundle_version_id, key)
);

-- §3.1 FieldMapping — explicit placeholder → field (REQ-028, ADR-011)
CREATE TABLE field_mappings (
  id                 TEXT PRIMARY KEY,
  bundle_version_id  TEXT NOT NULL REFERENCES bundle_versions(id) ON DELETE CASCADE,
  template_id        TEXT NOT NULL REFERENCES templates(id),
  placeholder        TEXT NOT NULL,                -- literal, e.g. {{company_name}}
  field_id           TEXT NOT NULL REFERENCES fields(id),
  UNIQUE (bundle_version_id, template_id, placeholder)
);

-- §3.1 Rule — deterministic conditional document (REQ-036, REQ-037)
CREATE TABLE rules (
  id                 TEXT PRIMARY KEY,
  bundle_version_id  TEXT NOT NULL REFERENCES bundle_versions(id) ON DELETE CASCADE,
  template_id        TEXT NOT NULL REFERENCES templates(id),
  expression         TEXT NOT NULL,                -- DSL: security_type == "equity"
  label              TEXT,                         -- human reason for include/exclude
  UNIQUE (bundle_version_id, template_id)
);

-- §3.1 Matter — instance of a Bundle Version (REQ-029)
CREATE TABLE matters (
  id                 TEXT PRIMARY KEY,
  bundle_id          TEXT NOT NULL REFERENCES bundles(id) ON DELETE CASCADE,
  bundle_version_id  TEXT NOT NULL REFERENCES bundle_versions(id),  -- immutable binding
  name               TEXT NOT NULL COLLATE NOCASE,
  status             TEXT NOT NULL DEFAULT 'in_progress'
                     CHECK (status IN ('in_progress','ready','generating','complete')),
  created_by         TEXT NOT NULL REFERENCES users(id),
  created_at         TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at         TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE (bundle_id, name)
);

-- §3.1 MatterData — one data source, many documents (REQ-030)
CREATE TABLE matter_data (
  matter_id      TEXT PRIMARY KEY REFERENCES matters(id) ON DELETE CASCADE,
  values_json    TEXT NOT NULL,                    -- canonical field values
  input_hash     TEXT NOT NULL CHECK (length(input_hash) = 64),  -- sha256 canonicalized
  updated_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

-- §3.1 GenerationRun — reproducibility record (REQ-033)
CREATE TABLE generation_runs (
  id                    TEXT PRIMARY KEY,
  matter_id             TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
  bundle_id             TEXT NOT NULL REFERENCES bundles(id),
  bundle_version        INTEGER NOT NULL,          -- exact immutable version used
  input_hash            TEXT NOT NULL CHECK (length(input_hash) = 64),
  engine_version        TEXT NOT NULL,             -- docx_engine semver
  status                TEXT NOT NULL DEFAULT 'queued'
                        CHECK (status IN ('queued','running','completed','partial','failed','validation_error')),
  requested_documents_json TEXT NOT NULL,          -- template ids, or null = all
  warnings_json         TEXT NOT NULL DEFAULT '[]',
  errors_json           TEXT NOT NULL DEFAULT '[]',
  created_by            TEXT NOT NULL REFERENCES users(id),
  created_at            TEXT NOT NULL DEFAULT (datetime('now')),
  completed_at          TEXT
);
CREATE INDEX idx_generation_runs_matter_time ON generation_runs(matter_id, created_at);

-- §3.1 GeneratedDocument — never mutated (REQ-034)
CREATE TABLE generated_documents (
  id             TEXT PRIMARY KEY,
  run_id         TEXT NOT NULL REFERENCES generation_runs(id) ON DELETE CASCADE,
  template_id    TEXT NOT NULL REFERENCES templates(id),
  output_path    TEXT NOT NULL,                    -- FS path, NO BLOB (REQ-004)
  format         TEXT NOT NULL CHECK (format IN ('docx','pdf')),
  status         TEXT NOT NULL CHECK (status IN ('generated','skipped','failed')),
  skip_reason    TEXT,                             -- human reason when skipped by a rule
  content_sha256 TEXT NOT NULL CHECK (length(content_sha256) = 64)
);
CREATE INDEX idx_generated_documents_run ON generated_documents(run_id);
```

- WAL mode; `PRAGMA foreign_keys = ON`; migrations versioned in `schema.rs` with the
  `schema_version` ledger. Target `PRAGMA user_version = 5` (§20).
- `bundle_versions.manifest_json` is the single immutable snapshot; the child tables
  (`bundle_documents`, `fields`, `field_groups`, `field_mappings`, `rules`) hang off the
  version id so a published version's definition is fully reconstructible (ADR-010,
  ADR-012).

---

## 16. Auth / Authorization

- **Local identity (Free/Pro):** single-device profile; `users` row created locally.
  Consumer value is zero-friction — no forced login.
- **Business:** local user registry + org scoping; admin assigns RBAC roles; the
  `licensing` module tracks seat assignment (`license_seats`).
- **Enterprise SSO (SAML, REQ-021):** `AuthService.authenticate` accepts a SAML
  assertion from the configured IdP; the verified `external_sub` is mapped (JIT or
  admin-provisioned) to a local `users` row with an RBAC role. SSO is additive: local
  auth remains for air-gapped installs.
- **Enforcement points:** (1) every Tauri command in the services layer,
  (2) every CLI subcommand, (3) every REST bridge route — `governance::authorize`
  is called before any domain operation. The UI reflects but never enforces roles
  (AC-011).
- **v2 RBAC mapping (planned):** `creator` builds/publishes Bundles, `filler`/`viewer`
  enter Matter data and generate from published versions, `approver`/`admin` publish and
  govern. `bundle::publish_version` requires `approver`/`admin` in governed orgs; a
  personal workspace allows self-publish (REQ-012, REQ-024).
- **Policy files (enterprise):** `policy_config` overlays defaults — allowed roles,
  update channel, telemetry off, IdP endpoint, license pool (REQ-021, AC-021).

---

## 17. External Dependencies

| Dependency | Use | Rationale / Decision |
|---|---|---|
| `quick-xml` 0.37 | Streaming `document.xml` parse/serialize for tag + fill | Already adopted; streaming (constant memory); no regex mutation (constraint; ADR-002) |
| `zip` 2.x | OPC container read/write (docx, dfpkg) | Already adopted; handles arbitrary zip members with validation |
| `rusqlite` 0.32 (bundled) | SQLite metadata index + audit | Bundled — no native SQLite dependency on user machines |
| `mammoth` (JS) | DOCX → HTML preview | Renders faithfully; output MUST be DOMPurify-sanitized (ADR-006) |
| `DOMPurify` (JS) | Sanitize preview HTML | CSP complement; blocks embedded script/event-handler exfiltration |
| PDF engine | Print-to-PDF | ADR-005: WebView2 headless primary + native `printpdf`/`docx-rs` fallback (ADR-009) |
| `tauri-plugin-dialog` | Native file picker | Picker-confirmed paths only (REQ-018) |
| `serde`/`serde_json` | Typed IPC + REST contracts | Structured errors and DTOs (REQ-003) |
| `uuid` | Identifiers | Immutable audit ids |
| `dirs` | App-data location | Cross-platform data dir |
| Rule DSL parser (planned) | Deterministic expression parse/eval in `rules` | Small hand-rolled evaluator owned by the module (ADR-011); no third-party eval — keeps determinism and no-scripting guarantees (REQ-037) |
| Sentry (opt-in) | Crash reporting | Consent-gated; stripped in enterprise builds (REQ-020) |
| Paddle / license service (optional cloud) | Billing + license issuance | Zero-knowledge: activation facts only (REQ-019, ADR-007) |
| WebView2 runtime | Tauri webview + headless print | Ships with Win10/11; Tauri bootstrap installs if absent — keeps clean-VM promise |

**Removed:** `docxtemplater`, `pizzip` (JS doc generation) — killed by ADR-001;
Base64 bridge — replaced by binary IPC (ADR-004); `soffice` ghost dependency — replaced
by ADR-005/ADR-009.

---

## 18. Technology Decisions — Summary

| # | Decision | Rationale |
|---|---|---|
| D1 | Unified Rust `docx_engine` owns tag + fill (kill docxtemplater) | One parser/behavior; AC-001; ADR-001 |
| D2 | Cross-run XML replacement via quick-xml with run merging | REQ-002; no regex on `document.xml`; ADR-002 |
| D3 | FS-backed template storage + SQLite index, no BLOBs | REQ-004; small DB; DPAPI-encryptable files; ADR-003 |
| D4 | Binary IPC replaces Base64 for >1MB payloads | REQ-005; 33% payload reduction; ADR-004 |
| D5 | WebView2 headless print-to-PDF primary; guided fallback | REQ-006/AC-006 clean-VM; ADR-005 |
| D6 | Strict CSP + DOMPurify + sandboxed iframe previews | REQ-017/AC-017; ADR-006 |
| D7 | Zero-knowledge licensing/telemetry | REQ-019/020; GDPR/SOC 2 scope; ADR-007 |
| D8 | Deterministic generation only (no LLM in path) | Positioning + REQ-003 determinism; ADR-008 |
| D9 | Layered core/services/shell with port-based I/O | Headless reuse by CLI/server; REQ-016 |
| D10 | RBAC + audit enforced in Rust services, not UI | AC-011/013; tamper-resistant by construction |
| D11 | SQLite WAL metadata-only | Offline-first; instant startup; ADR-003 |
| D12 | Signed multi-channel distribution (MSIX/MSI/EXE) with SBOM | REQ-022; Intune/WSUS silent deploy |
| D13 | Bundle + Matter domain model; immutable published Bundle Versions | REQ-023/024/029; one data entry → many documents; ADR-010 |
| D14 | Explicit canonical mapping layer (placeholder → field) as data | REQ-028; no scattered string replacement; ADR-011 |
| D15 | Generation-run snapshots (input hash + bundle version + engine version) | REQ-033/034; reproducibility; ADR-012 |
| D16 | `.dfpkg` as portable Bundle package | REQ-025; offline portability; ADR-013 |
| D17 | Safe deterministic rule DSL (no unrestricted scripting) | REQ-036/037; conditions only, side-effect-free |

**New ADRs (planned files in `docs/adr/`):** ADR-010 immutable published bundle versions ·
ADR-011 explicit canonical mapping layer · ADR-012 generation-run snapshots ·
ADR-013 `.dfpkg` as portable bundle unit. Full texts are authored as ADR files alongside
this document; the index below (§19) includes them.

---

## 19. Requirement Traceability

Every requirement maps to a module (or an ADR where the decision is architectural).
v1 rows are unchanged; v2 rows (REQ-023..REQ-040) are **planned (v2.0.0)**.

| ID | Requirement (abridged) | Module / Artifact | AC |
|---|---|---|---|
| REQ-001 | Unified docx_engine, no frontend generation | `docx_engine` · ADR-001 | AC-001 |
| REQ-002 | Cross-run XML replacement, first-run formatting | `docx_engine` · ADR-002 | AC-002 |
| REQ-003 | Unclosed-tag structured error, no corruption | `docx_engine` · §9 error model | AC-003 |
| REQ-004 | FS-backed storage, SQLite index only | `template_store` · ADR-003 | AC-004 |
| REQ-005 | Binary IPC, no Base64 hot path | `gui_shell` · ADR-004 | AC-005 |
| REQ-006 | PDF export without LibreOffice | `export` · ADR-005/009 | AC-006 |
| REQ-007 | Formats: docx, pdf, html preview, dfpkg | `export` · §6.4 | AC-007 |
| REQ-008 | Template Creator (upload, field types, preview) | `gui_shell` + `template_store` · §4.1 | AC-008 |
| REQ-009 | Template Filler (validate, preview, export) | `gui_shell` + `docx_engine` · §9 | AC-009 |
| REQ-010 | Versioning: draft/review/published/archived + rollback | `template_store` · §6.5 | AC-010 |
| REQ-011 | RBAC: viewer/filler/creator/approver/admin | `governance` · §16 | AC-011 |
| REQ-012 | Approval workflow; published-only fillable | `governance` · §6.5 | AC-012 |
| REQ-013 | Immutable exportable audit log | `governance` · §15 `generation_log` | AC-013 |
| REQ-014 | Admin console: users/seats/licenses/audit/reports | `governance` + `gui_shell` · §4.11 | AC-014 |
| REQ-015 | Offline activation, device caps, grace, enterprise files | `licensing` · ADR-007 | AC-015 |
| REQ-016 | CLI + local REST bridge + webhooks | `docforge-core` reuse · §7.2/§7.3 | AC-016 |
| REQ-017 | Strict CSP + sanitized mammoth HTML | `gui_shell` + `export` · ADR-006 | AC-017 |
| REQ-018 | DOCX magic/zip validation + path traversal guard | `docx_engine` + `gui_shell` · §8 | AC-018 |
| REQ-019 | DPAPI storage encryption; zero-knowledge licensing/telemetry | `template_store` + `licensing` · ADR-007 | AC-019 |
| REQ-020 | Opt-in telemetry/crash, aggregate only, disable-able | `gui_shell` + `TelemetryService` · §12 | AC-020 |
| REQ-021 | SSO/SAML, on-prem/air-gapped, policy-file deploy | `governance` + `AuthService` · §16 | AC-021 |
| REQ-022 | Signed auto-update, staged rollout, rollback, SBOM | `gui_shell` + `UpdateService` · §14 | AC-022 |
| REQ-023 | Bundle = reusable generation definition (identity, documents, schema, mappings, rules, output config, versions) | `bundle` · §4.6 · ADR-010 | — |
| REQ-024 | Published Bundle Versions immutable; new version on change; generation references exact version | `bundle` + `generation_run` · ADR-010 | — |
| REQ-025 | Bundle import/export via `.dfpkg`, no cloud required | `bundle` + `export` · ADR-013 | — |
| REQ-026 | Canonical field schema: types + attributes (id, label, description, type, required, default, validation, group, options, format) | `field_mapping` · §4.7 | — |
| REQ-027 | Field groups (shared vs document-specific) with UI separation | `field_mapping` + `gui_shell` · §4.7/§4.11 | — |
| REQ-028 | Explicit deterministic mapping layer: document placeholders → canonical fields | `field_mapping` · ADR-011 | — |
| REQ-029 | Matter = instance of a Bundle Version; matter data stored separately | `matter` + `bundle` · §3.2 | — |
| REQ-030 | One matter data source → many generated documents (no per-document entry) | `matter` + `generation_run` · §6.2 | — |
| REQ-031 | Matter UX: select bundle, grouped form, validate, preview, generate | `gui_shell` + `matter` · §4.11/§6.2 | — |
| REQ-032 | Validation at three levels: field, matter, bundle | `field_mapping` + `matter` + `bundle` · §10 | — |
| REQ-033 | Generation Run record with input snapshot, engine version, warnings/errors/status | `generation_run` · §4.10 · ADR-012 | — |
| REQ-034 | Never mutate historical generated documents; new run per change | `generation_run` · §4.10/§6.3 | — |
| REQ-035 | Generate all / generate selected, output naming rules, DOCX/PDF | `generation_run` + `export` · §4.10/§6.3 | — |
| REQ-036 | Conditional documents (bundle-level) with preview explaining skipped docs | `rules` + `gui_shell` · §4.9/§6.3 | — |
| REQ-037 | Safe deterministic expression system (no unrestricted scripting) | `rules` · §4.9/§8 | — |
| REQ-038 | Bundle Health Check diagnostics identifying exact document+field | `bundle` + `field_mapping` · §11 | — |
| REQ-039 | Professional neutrality: no hard-coded legal/CS rules in core | domain model · §3.3 | — |
| REQ-040 | No frontend generation engine; Rust docx_engine remains single source of truth | `docx_engine` + `gui_shell` · ADR-001/§6.3 | — |
| REQ-101 | 10MB tag/fill < 2s; UI never blocks | `docx_engine` + thread pool · §13 | — |
| REQ-102 | 100% tag fidelity on 50-fixture corpus | `docx_engine` test gate · ADR-002 | — |
| REQ-103 | Win10/11 primary; clean-VM zero manual deps | §14, ADR-005 | — |
| REQ-104 | Unit/component/e2e coverage; CI gate | SDLC `verify_module.py` + CI | — |
| REQ-105 | GDPR local-first; SOC 2 scoped; DPA + whitepaper | §12, §14, ADR-007 | — |

**ADR index:** ADR-001 unified Rust engine · ADR-002 cross-run replacement · ADR-003
FS storage · ADR-004 binary IPC · ADR-005 PDF engine · ADR-006 CSP/previews · ADR-007
licensing/zero-knowledge · ADR-008 deterministic no-AI · ADR-009 templatebuilder
features (native PDF fallback, backup/restore, template bundles) · **ADR-010 immutable
published bundle versions** · **ADR-011 explicit canonical mapping layer** · **ADR-012
generation-run snapshots** · **ADR-013 `.dfpkg` as portable bundle unit**. All in
`docs/adr/`.

---

## 20. Migration Path (v1 → v2)

### 20.1 Data migration (existing schema → schema v5)

The shipped schema is v4 (Data Model v2 + template bundles + bug book). Migration to
**schema v5** (specified in `schema.md`, authoritative DDL in §15) is transactional and
one-time, preserving every existing user artifact:

1. **Templates keep working.** Existing `templates` / `template_versions` rows are
   untouched: standalone templates, their lifecycle states, rollback, and the fields they
   carry remain valid (REQ-004, REQ-010). Nothing is deleted or rewritten.
2. **v1 template bundles migrate into Bundle documents.** The v1 bundle-grouping tables
   (`bundles` + `bundle_templates`, ADR-009) are promoted to the v2 Bundle model: each
   existing group becomes a `bundles` row, its member templates become `bundle_documents`
   rows under a first `bundle_versions` snapshot, and the v1 group name becomes the Bundle
   name. The v2 draft version carries the existing members; fields/mappings/rules start
   empty and are filled in Phase 2 (REQ-023).
3. **New v2 tables** (`bundles`, `bundle_versions`, `bundle_documents`, `field_groups`,
   `fields`, `field_mappings`, `rules`, `matters`, `matter_data`, `generation_runs`,
   `generated_documents`) are created; `schema_version` records `5`.
4. **`generation_log` is preserved** with `template_version` intact; new run-based audit
   entries link `run_id` going forward (REQ-013, REQ-033).
5. **Verify:** `PRAGMA foreign_key_check;` count parity per legacy table; existing
   standalone-template fill still works end-to-end; migrated bundles appear on the Bundles
   screen with a `needs_setup` badge (Phase 2 pending).

### 20.2 Phased delivery (each phase leaves the app buildable and testable)

| Phase | Scope | Shipped modules | Exit gate |
|---|---|---|---|
| **Phase 1 — Bundle stabilize** | `bundle` CRUD + manifest + versioning; `.dfpkg` v2 container; bundle-level publish | `bundle`, `template_store` (adapt), `export` (dfpkg) | v1 standalone flows green on 50-fixture corpus; `docforge bundle list/create/publish` works |
| **Phase 2 — Fields & mappings** | Canonical schema + field types + groups; explicit mapping layer; bundle validation; Health Check | `field_mapping`, `bundle` (validate/health) | Bundle Health Check reports exact document+field; schema v5 applied |
| **Phase 3 — Matter** | Matter CRUD, grouped form, matter validation, matter_form_schema | `matter`, `gui_shell` (Matter form) | Create Matter against a published version; validate inline |
| **Phase 4 — Generation** | Rules DSL + evaluation; generation runs; preview; generated documents | `rules`, `generation_run`, `export` (run outputs), `gui_shell` (preview + Generated Documents) | Generate all/selected from one matter data entry; rerun creates new run; historical outputs untouched |

- Every phase re-runs the v1 regression gates (50-fixture corpus, clean-VM PDF, CLI smoke)
  so the app never regresses while the domain model evolves (REQ-104).
- v1 commands are kept (with no behavior change) until Phase 4 exits; removal is a
  separate post-v2.0.0 CR under the change policy.

---

## 21. Post-Release Governance (v2.0.0 full re-approval)

This document is the **v2.0.0 full re-approval run** per
`docs/governance/POST_RELEASE_CHANGE_POLICY.md`: the Bundle + Matter domain model is a
breaking change (data-model rewrite and new module boundaries), which the policy
classifies as requiring a full Phase 1–4 re-approval rather than a tiered CR. The prior
post-release records below remain valid project history and their artifacts stay in the
codebase.

Post-1.0 changes continue to be governed by `config.json` `changeGovernance`; any change
to architecture, schema, or command surface requires CR registration in
`docs/governance/CHANGELOG.md` before merge.

### CR-2026-001 — Bug Book Feature (v1, carried forward)

| Layer | Addition | Notes |
|---|---|---|
| **L1 Core** | `core/bug_book.rs` | Bug tracking module. Public types: `BugEntry`, `BugAttachment`, `NewBug`, `BugFilter`. Functions: `create_bug`, `get_bug`, `list_bugs`, `update_bug_status`, `add_attachment`, `export_bugs_csv`, `export_bugs_pdf`. |
| **L0 DB** | Schema migration **v4** | Adds `bug_book` and `bug_attachments` tables with indexes on `org_id`, `status`, `created_at`. |
| **L3 Shell** | 8 new Tauri commands | `log_bug`, `create_bug_entry`, `list_bugs`, `get_bug`, `update_bug_status`, `add_bug_attachment`, `export_bugs_csv`, `export_bugs_pdf`. Registered in `lib.rs` via `generate_handler!`. |
| **L3 Shell** | Panic-hook crash capture | `lib.rs` `run()` installs a panic hook that records unwinding payload into the bug book before exit. |
| **L2 Services** | `services/webhook.rs` | `dispatch_webhook_event` now POSTs a `bug.critical` event to `webhook_subscriptions` via `curl.exe` when a bug reaches critical severity. |
| **L3 Frontend** | Admin Console | `Bug Book` tab (`BugBook.tsx`). Global `errorCapture.ts` registers `window.onerror` and `window.onunhandledrejection` and forwards captures through IPC/types wrappers. |

### CR-2026-002 — save_template CamelCase Fix (v1, carried forward)

`TemplateFieldSpec` derives `#[serde(rename_all = "camelCase")]`, allowing the frontend
to send camelCase field names (matching the existing `TemplateField` type) without
triggering a `missing field original_text` runtime deserialization error. This aligns
stored `fields_json` with the frontend contract and eliminates the manual field rename
mapping previously required in the IPC boundary.

### Post-Release Governance

| Artifact | Purpose |
|---|---|
| `config.json` → `changeGovernance` | Enforces CR-gated merges for architecture, schema, and command-surface changes. |
| `docs/governance/CHANGELOG.md` | CR register; every merged change record links to its CR and affected modules/commands/tables. |
| `docs/governance/POST_RELEASE_CHANGE_POLICY.md` | Policy governing tiered CRs (additive modules, additive commands, additive schema migrations) vs. full Phase 1–4 re-approval (breaking IPC contracts, data model rewrites, layer boundary crossings — the v2.0.0 Bundle + Matter change is this category). |

