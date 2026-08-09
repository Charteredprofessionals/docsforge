# DocForge — System Architecture (v1)

> Owner: System Architect | Engine: Authr OS SDLC Studio (Phase 1)
> Status: **DRAFT — pending approval gate**
> Scope: Takeover + rebuild of the working prototype into a consumer/SMB/enterprise-ready
> product per `docs/company/product-plan.md` Section 5 and `requirements.md` v1.
> Constraint: the existing React/Tauri shell structure (`src/`, `src-tauri/`) is retained;
> ALL document logic moves into the Rust core. No source code is delivered by this document.

---

## 1. System Overview

DocForge is a **deterministic, privacy-first, offline-first document-automation** product.
Users tag placeholders in their own DOCX templates, fill field values through a governed
library, and export Word/PDF/HTML — with no document content ever leaving the device
unless the user opts in, and with zero dependence on an LLM in the generation path.

The product plan mandates a **headless core**: a single Rust library (`docforge-core`)
owns every document operation, and three shells consume it — the Tauri 2 desktop GUI
(primary), a headless CLI, and an optional enterprise local REST bridge. This kills the
current dual-engine drift (Rust quick-xml tagging + JS docxtemplater filling) and turns
one verified implementation into a single source of truth.

### 1.1 Architectural Drivers

| Driver | Source | Implication |
|---|---|---|
| Kill dual engine | REQ-001, AC-001 | One Rust `docx_engine` owns tag + fill; zero generation logic in JS |
| Cross-run tagging | REQ-002, AC-002 | Run-aware XML replacement; formatting inherits from first run |
| Offline-first, no ghost deps | REQ-006, REQ-103, AC-006 | Bundled PDF path; zero required installs on a clean VM |
| Data stays local | REQ-004, REQ-019, REQ-105 | FS-backed storage + SQLite index; DPAPI at rest; zero-knowledge telemetry |
| Enterprise additive | REQ-011..REQ-021 | RBAC, audit, SSO, on-prem, policy files layered on the same core |
| Determinism | REQ-003, out-of-scope AI | Structured tag errors; no LLM anywhere in the generation path |

### 1.2 Definition of Done (from `constraints.json`)

- 100% tag fidelity on the 50-fixture DOCX corpus
- PDF export on a clean Windows VM with no LibreOffice
- Signed binaries; SBOM per release; quality gate passed before any release candidate

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
│  preview only         generate/export       webhooks                │
├─────────────────────────────────────────────────────────────────────┤
│ L2 SERVICES ── application orchestration + enforcement              │
│                                                                     │
│  GenerationService   TemplateService   GovernanceService            │
│  AuthService         LicenseService    TelemetryService             │
│  UpdateService       WebhookService                                 │
│  (RBAC checks · workflow transitions · audit writes · consent)      │
├─────────────────────────────────────────────────────────────────────┤
│ L1 CORE  ── docforge-core (library, no GUI/IO deps except IO ports) │
│                                                                     │
│  docx_engine   template_store   governance   licensing   export     │
│  (pure domain: parse/tag/fill, storage, workflows, entitlement,     │
│   formats)                                                          │
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

---

## 3. Module Boundaries (the 6 approved modules)

Each module is a single-responsibility unit with a documented public surface.

### 3.1 `docx_engine` (core, L1) — REQ-001, 002, 003
- **Responsibility:** tag and fill DOCX documents; nothing else.
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
- **Explicitly out:** rendering HTML (that is `export::html`), storage, licensing.

### 3.2 `template_store` (core, L1) — REQ-004, 010, 007
- **Responsibility:** persistence of templates, their versions, and field schemas.
- Public surface:
  - `save(meta, bytes) -> TemplateId`, `load(id, version) -> Vec<u8>`, `list()`,
    `delete(id)`, `create_version(...)`, `rollback(id, to_version)`,
    `resolve(storage_path)`.
- Files live under the app-data `templates/` tree; SQLite stores paths + metadata only
  (REQ-004, AC-004). Versioning states: `draft | review | published | archived`
  (REQ-010, AC-010). A `.dfpkg` bundle is both the portable unit (REQ-007) and a
  snapshot format for version rollback.

### 3.3 `governance` (core, L1) — REQ-011, 012, 013, 014, 021
- **Responsibility:** RBAC, approval workflow, immutable audit log, admin/usage reports.
- Public surface:
  - `authorize(user, role, action) -> Result<(), DocForgeError>` — RBAC matrix
    (viewer/filler/creator/approver/admin) (REQ-011, AC-011).
  - `transition_status(template_id, from, to, actor) -> Result<Status, ...>` —
    draft → approve → publish; published-only fillable by non-creators (REQ-012, AC-012).
  - `record_generation(entry: AuditEntry)` / `export_audit(filter) -> ExportableAudit`
    — append-only `generation_log` writer (REQ-013, AC-013).
  - `usage_report(org_id, period) -> AggregateReport` — aggregates only, no content
    (REQ-014).
- Audit entries are written by the services layer through this module; the module itself
  is the only writer of `generation_log`.

### 3.4 `licensing` (core, L1) — REQ-015, 019
- **Responsibility:** entitlement evaluation, device registration, grace windows,
  offline license files, revocation — all zero-knowledge.
- Public surface:
  - `evaluate_entitlement(feature, context) -> Entitlement` — gates Free/Pro/Business/
    Enterprise capabilities (template count, field types, PDF export, team features).
  - `activate(key | license_file, device) -> ActivationReceipt` — offline-capable;
    Pro 2-device limit, Business per-seat pool, Enterprise offline-issued files with no
    phone-home after activation (REQ-015, AC-015).
  - `grace_remaining()` — 30/60/90-day windows (consumer/company/enterprise).
  - `revoke(device)` — admin-driven (REQ-014).
- Nothing about document content exists here; licensing payloads are activation facts
  only (REQ-019, ADR-007).

### 3.5 `export` (core, L1) — REQ-006, 007
- **Responsibility:** produce output artifacts. DOCX (identity copy with embedded fields),
  PDF (via print bridge; ADR-005), HTML preview (mammoth → sanitized), `.dfpkg` bundle
  (docx + fields + metadata + version) (REQ-007, AC-007).
- Public surface: `export_docx(...)`, `export_pdf(docx, renderer) -> Vec<u8>`,
  `render_html_preview(docx) -> SanitizedHtml`, `export_dfpkg(...)`, `import_dfpkg(...)`.
- HTML preview output must be sanitized before it reaches the renderer (REQ-017,
  ADR-006) — sanitization happens here, not in the UI.

### 3.6 `gui_shell` (shell, L3) — REQ-008, 009, 014, 016, 017, 022
- **Responsibility:** the Tauri desktop shell: React 18 screens, Tauri command
  registration, binary IPC, preview rendering, update/consent UX.
- React screens: Template Creator (REQ-008, AC-008), Template Filler (REQ-009,
  AC-009), Template List, Admin Console (REQ-014, AC-014), Consent/Telemetry dialog
  (REQ-020, AC-020), Licensing/Paywall surface (REQ-015).
- The shell registers commands that delegate to L2 services; it contains **no** zip/XML
  processing (AC-001). The same core is reused by `cli`/`server` shells (REQ-016,
  AC-016) — gui_shell is the reference shell; cli/server are thin sibling shells
  (see §5.2, §6, §10).

---

## 4. Component Diagram (text)

```
                      ┌────────────────────────────────────────────┐
                      │              React 18 / TS                 │
                      │  TemplateCreator  TemplateFiller  Admin    │
                      │  ConsentDialog   LicensingPane   Preview   │
                      │        (iframe sandbox + DOMPurify)        │
                      └───────────────────┬────────────────────────┘
                                          │ typed invoke (binary IPC)
                      ┌───────────────────▼────────────────────────┐
                      │           gui_shell (Tauri 2)              │
                      │  Command Router · capability ACL · CSP     │
                      └───────────────────┬────────────────────────┘
              ┌─────────────┬─────────────┼─────────────┬───────────┐
   ┌──────────▼──────┐ ┌─────▼─────┐ ┌─────▼──────┐ ┌───▼──────┐ ┌──▼───────────┐
   │ GenerationSvc   │ │ TemplateSvc│ │ Governance │ │ AuthSvc  │ │ TelemetrySvc │
   │  (thread pool)  │ │           │ │   Svc      │ │(RBAC/SSO)│ │ (opt-in)     │
   └──────────┬──────┘ └─────┬─────┘ └─────┬──────┘ └───┬──────┘ └──┬───────────┘
              └──────────────┴─────┬───────┴────────────┴───────────┘
                       ┌───────────▼──────────────────────────┐
                       │            docforge-core             │
                       │  docx_engine  template_store         │
                       │  governance   licensing   export     │
                       └───────────┬──────────────────────────┘
            ┌──────────────────────┼───────────────────────┐
  ┌─────────▼────────┐ ┌───────────▼────────┐ ┌────────────▼─────────┐
  │ SQLite (rusqlite)│ │ FS template store  │ │ Print bridge         │
  │ WAL · FK · audit │ │ app-data/templates │ │ (WebView2 headless)  │
  │                 │ │ DPAPI-encrypted    │ │ + guided LO fallback │
  └──────────────────┘ └────────────────────┘ └──────────────────────┘
  ┌──────────────────────┬──────────────────────┬────────────────────┐
  │ Optional cloud:      │  Webhooks (ent.)     │  Sentry (opt-in)   │
  │ license issuance ·   │  / REST bridge       │  crash/aggregate   │
  │ seat mgmt (no docs)  │  localhost:PORT      │  consent-gated     │
  └──────────────────────┴──────────────────────┴────────────────────┘
```

Data never flows from the bottom row back into the document path: the license/telemetry
cloud surfaces are zero-knowledge (ADR-007).

---

## 5. Data Flow

### 5.1 Tag flow (Template Creator)
1. User picks a DOCX via the native dialog (`tauri-plugin-dialog`). The shell receives a
   user-confirmed path — no user-supplied path string is ever concatenated into a
   filesystem action (REQ-018 path-traversal guard, §7).
2. `gui_shell` invokes `upload_docx` → `GenerationService.import_docx` →
   `docx_engine::validate_docx` (magic bytes `PK\x03\x04`, zip structure, entry-count +
   compression-ratio caps) and `extract_plain_text` for the preview.
3. The editor renders the extracted text; user selects spans → `TagSelection[]`.
4. `tag_document` streams `word/document.xml` through quick-xml, merging `<w:t>` runs so
   a selection spanning multiple runs becomes a single `{{tag}}` placeholder carrying the
   first run's `<w:rPr>` formatting (REQ-002, ADR-002).
5. `template_store::save` writes the tagged `.docx` to `app-data/templates/<id>/v1/`
   and an index row (no BLOB, REQ-004). Fields JSON (typed schema per REQ-008) stored in
   the row. Status begins at `draft` (REQ-010, REQ-012).

### 5.2 Fill flow (Template Filler)
1. `fill_template` resolves the template id → latest **published** version (RBAC: only
   creators/approvers may fill drafts; REQ-012).
2. Frontend sends field values; the service runs `docx_engine::fill_document`.
3. Placeholder → value replacement is run-aware; any unclosed `{{` remaining after the
   pass raises `DocForgeError::UnclosedTag { tag_name, offset }` — **no partially-filled
   file is ever returned** (REQ-003, AC-003).
4. Per-field type validation (text/date/dropdown/checkbox/signature, REQ-009) yields
   `InvalidFieldValue { field_id, reason }` errors mapped to inline UI messages.
5. On success, `governance::record_generation` appends an immutable audit entry
   (fields_hash, version, format, user, machine, timestamp — REQ-013) and the result is
   offered to the Export pipeline.

### 5.3 Export flow
1. `export` runs in the `GenerationService` thread pool (REQ-101: never on the UI
   thread; 10MB target tag/fill < 2s).
2. **DOCX:** byte-copy of the filled package (zip identity, metadata untouched).
3. **PDF:** sanitized HTML render path → WebView2 headless print-to-PDF (ADR-005),
   LibreOffice never required (REQ-006, AC-006); optional guided portable-LO fallback.
4. **HTML preview:** `mammoth` HTML passes through DOMPurify and is rendered in a
   sandboxed iframe under strict CSP (REQ-017, ADR-006).
5. **.dfpkg:** zip bundle of {document.docx, fields.json, metadata.json, version}.
6. Every export writes its audit entry with `format` recorded (REQ-013).

### 5.4 Governance flow
`draft → review → published → archived`; only `approver`/`admin` may publish; only
published templates are fillable by `viewer`/`filler` (REQ-012, AC-012). Rollback loads a
prior `template_versions` snapshot and marks a new version (REQ-010, AC-010).

---

## 6. API Boundaries

### 6.1 Tauri command surface (gui_shell, typed)
All commands return `Result<T, DocForgeError>` serialized as structured JSON
(§8). Payloads >1MB travel as raw bytes over binary IPC (ADR-004); no Base64 in the hot
path (REQ-005, AC-005).

| Command | Arguments | Returns | Module/Service |
|---|---|---|---|
| `upload_docx` | `path: string` (picker-confirmed) | `DocxPreview { text, size }` | GenerationService |
| `tag_template` | `bytes: Uint8Array`, `selections: TagSelection[]` | `TemplateDraft { id, fields }` | docx_engine + template_store |
| `save_template` | `TemplateDraft` | `TemplateId` | TemplateService |
| `list_templates` | `{ status?, orgId? }` | `TemplateMeta[]` | TemplateService |
| `get_template` | `{ id, version? }` | `TemplateDetail { bytes, fields, status }` | TemplateService |
| `update_template_status` | `{ id, to }` | `Status` | GovernanceService |
| `create_template_version` | `{ id, note }` | `Version` | TemplateService |
| `rollback_template` | `{ id, toVersion }` | `Version` | TemplateService |
| `delete_template` | `{ id }` | `()` | TemplateService |
| `fill_template` | `{ id, version?, values }` | `FilledResult { outPath }` | GenerationService |
| `export_document` | `{ id, format, options }` | `ExportArtifact` | ExportService |
| `render_preview` | `{ id, version? }` | `SanitizedHtml` | ExportService |
| `list_users` / `set_user_role` | `{ orgId }` / `{ userId, role }` | `User[]` / `()` | GovernanceService (admin) |
| `export_audit` | `{ filter, format }` | `AuditFile` | GovernanceService |
| `usage_report` | `{ period, groupBy }` | `AggregateReport` | GovernanceService |
| `activate_license` | `{ key \| filePath }` | `ActivationReceipt` | LicenseService |
| `get_entitlement` | `{ feature? }` | `Entitlement` | LicenseService |
| `set_telemetry_consent` | `{ consented: bool }` | `()` | TelemetryService |
| `authenticate` | `{ ssoToken? \| local }` | `Session` | AuthService |

Capabilities (ACL) are declared in `src-tauri/capabilities/default.json`; every command
is registered under least privilege. RBAC enforcement is **server-side in Rust**, never
trusted from the renderer (REQ-011, AC-011).

### 6.2 CLI surface (`docforge`, headless — REQ-016)
```
docforge generate --template <id|path> --data data.json --out out.docx [--format docx|pdf|dfpkg]
docforge template list | import <file.docx> | export <id> --format dfpkg
docforge fill --template <id> --values values.json --out out.docx
docforge audit export --org <id> --out audit.csv
docforge license activate <key|file> | status | deactivate
docforge config show | set <key> <value>      # policy-file overlay
docforge serve [--port 0]                     # optional enterprise REST bridge
```
Exit codes: `0` success, `2` usage error, `3` validation/tag error (structured JSON to
stderr for scripting), `4` license/entitlement error, `5` storage/IO error.

### 6.3 Optional local REST bridge (enterprise — REQ-016)
Bound to `127.0.0.1`, enabled only in Business/Enterprise tiers, bearer-token
authenticated with RBAC passthrough:

| Method/Path | Purpose |
|---|---|
| `POST /v1/generate` | template + JSON data → output artifact |
| `GET /v1/templates` / `POST /v1/templates` | library browse / import |
| `POST /v1/webhooks` | register generation-event webhook (success/failure) |
| `GET /v1/audit?since=` | pull audit trail (enterprise) |
| `GET /v1/health` | liveness (no document data) |

The bridge is compiled into `docforge-server` and, when enabled, is reachable from the
desktop shell — same core, same auth, same audit.

---

## 7. Security Model

| Concern | Mechanism | Requirement |
|---|---|---|
| Renderer policy | Strict CSP replacing `"csp": null` (ADR-006); `script-src 'self'`, no `unsafe-inline`/`unsafe-eval`; `frame-src` limited to sandboxed preview iframe | REQ-017, AC-017 |
| HTML preview | mammoth output → DOMPurify → sandboxed iframe; never `dangerouslySetInnerHTML` with unsanitized content | REQ-017 |
| DOCX validation | Magic bytes (`PK\x03\x04`), zip structure, entry count + compression-ratio caps (zip-bomb guard), XML entity/namespace limits, reject non-docx with precise errors | REQ-018, AC-018 |
| Path safety | Only picker-confirmed paths enter the FS; canonical-path + containment checks; no user string concatenated into paths | REQ-018 |
| At-rest data | Windows: DPAPI-encrypted template files (AC-019); macOS: Keychain; SQLite stores metadata only | REQ-019, REQ-004 |
| IPC | Binary IPC (ADR-004); Tauri capability ACL per command; arguments validated in Rust | REQ-005 |
| Licensing | Zero-knowledge: license checks carry activation facts only, never document bytes or field values (ADR-007) | REQ-019 |
| Telemetry | Consent-gated, aggregate-only, redaction pipeline; enterprise build disables entirely (ADR-007) | REQ-020, AC-020 |
| AuthN/AuthZ | Local identity + optional SAML/SSO token binding; RBAC enforced in services layer for every command and CLI/REST path | REQ-011, REQ-021 |
| Supply chain | EV code signing, signed updates, SBOM per release | REQ-022 |
| Secrets | None in config; API keys/tokens via DPAPI-protected settings store | — |

---

## 8. Error Handling — Structured Tag Errors

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
| `invalid_field_value` | `{ field_id, reason }` per-field validation failure | REQ-009 |
| `storage_missing` / `storage_io` | template/version file missing or unreadable | REQ-004/010 |
| `forbidden` | RBAC violation (`{ required_role }`) | REQ-011 |
| `not_published` | non-creator attempted to fill a draft | REQ-012 |
| `license_*` | `not_entitled`, `device_limit`, `grace_expired`, `invalid_key` | REQ-015 |
| `internal` | invariant failure (bug); includes correlation id for telemetry | — |

Rules: fail fast, never emit a partially-filled artifact, never leak document content
into error text or logs. Frontend maps `code` → per-field inline messages (REQ-009).

---

## 9. Observability

- **Consent:** first-run dialog explains what is collected (counts, timing, crash
  metadata) and what is never collected (document contents, field values, filenames).
  Opt-in only (REQ-020, AC-020).
- **Crash reporting:** Sentry, consent-gated, DSN stripped from enterprise builds.
- **Aggregate analytics:** events like `generation.completed {duration_ms, format}` with
  no PII/document identity; locally buffered, flushed in aggregate.
- **Enterprise:** telemetry + crash upload compiled out; policy file can force-disable
  (REQ-020).
- **Redaction pipeline:** any event payload passes a content-free allowlist before egress
  (ADR-007); verified via `code_review` (AC-019/AC-020).

---

## 10. Scalability & the Headless Core

- **Single implementation, three shells:** `docforge-core` compiles without GUI deps
  (`constraints.json`). The CLI and local REST server reuse the identical tag/fill/export
  code — REQ-016/AC-016 prove the headless path (ADR-001 is what enables this).
- **Concurrency:** generation runs on a dedicated thread pool behind the services layer;
  the WebView never blocks (REQ-101).
- **Large files:** >10MB payloads use binary IPC + chunked/streaming save (REQ-005,
  ADR-004); quick-xml is streaming (constant memory on `document.xml`).
- **SQLite:** WAL mode, foreign keys, single-writer; metadata-only so the DB stays small;
  indexes on `template_versions.template_id`, `generation_log(generated_at)`.
- **Multi-user (Business/Enterprise):** org scoping via `org_id` on every query; shared
  library served from a local/networked shared store in later phases (connectors are
  post-GA per product plan; storage stays behind the `template_store` port).

---

## 11. Deployment Model

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

---

## 12. Database Design (Data Model v2, from product-plan §5.3)

SQLite via rusqlite (bundled). BLOBs are removed — documents live on the filesystem
under `app-data/templates/`; the DB stores paths + metadata (REQ-004, ADR-003).

```sql
-- Index rows only; documents on disk under <data>/templates/<id>/v<n>/template.docx
CREATE TABLE orgs (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  plan          TEXT NOT NULL DEFAULT 'free',      -- free|pro|business|enterprise
  settings_json TEXT NOT NULL DEFAULT '{}'         -- policy overlay (enterprise)
);

CREATE TABLE users (
  id             TEXT PRIMARY KEY,
  org_id         TEXT NOT NULL REFERENCES orgs(id),
  name           TEXT NOT NULL,
  email          TEXT NOT NULL UNIQUE,
  role           TEXT NOT NULL DEFAULT 'viewer',   -- viewer|filler|creator|approver|admin
  license_seat_id TEXT,
  active         INTEGER NOT NULL DEFAULT 1,
  auth_source    TEXT NOT NULL DEFAULT 'local',    -- local|saml
  external_sub   TEXT                              -- SSO subject (SAML NameID)
);

CREATE TABLE templates (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  org_id        TEXT NOT NULL REFERENCES orgs(id),
  version       INTEGER NOT NULL DEFAULT 1,
  status        TEXT NOT NULL DEFAULT 'draft',     -- draft|review|published|archived
  storage_path  TEXT NOT NULL,                     -- NO BLOB (REQ-004)
  fields_json   TEXT NOT NULL,                     -- typed field schema (REQ-008)
  created_by    TEXT NOT NULL REFERENCES users(id),
  created_at    TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE template_versions (
  id            TEXT PRIMARY KEY,
  template_id   TEXT NOT NULL REFERENCES templates(id) ON DELETE CASCADE,
  version       INTEGER NOT NULL,
  storage_path  TEXT NOT NULL,
  fields_json   TEXT NOT NULL,
  created_by    TEXT NOT NULL REFERENCES users(id),
  created_at    TEXT NOT NULL DEFAULT (datetime('now')),
  note          TEXT,
  UNIQUE (template_id, version)                    -- rollback target (REQ-010)
);

-- Immutable audit trail: append-only, no UPDATE/DELETE grants (REQ-013)
CREATE TABLE generation_log (
  id            TEXT PRIMARY KEY,
  template_id   TEXT NOT NULL REFERENCES templates(id),
  template_version INTEGER NOT NULL,
  fields_hash   TEXT NOT NULL,                     -- sha256 of canonical field values
  output_name   TEXT NOT NULL,
  format        TEXT NOT NULL,                     -- docx|pdf|html|dfpkg
  user_id       TEXT NOT NULL REFERENCES users(id),
  machine_id    TEXT NOT NULL,
  status        TEXT NOT NULL,                     -- success|failed|validation_error
  generated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_genlog_time ON generation_log(generated_at);
CREATE INDEX idx_tmpl_versions ON template_versions(template_id);

CREATE TABLE licenses (
  id          TEXT PRIMARY KEY,
  org_id      TEXT REFERENCES orgs(id),
  user_id     TEXT REFERENCES users(id),
  tier        TEXT NOT NULL,                       -- free|pro|business|enterprise
  seats       INTEGER,
  devices     INTEGER NOT NULL DEFAULT 0,          -- Pro 2-device cap (REQ-015)
  issued_at   TEXT NOT NULL DEFAULT (datetime('now')),
  expires_at  TEXT,
  grace_days  INTEGER NOT NULL DEFAULT 30,         -- 30/60/90 per tier
  status      TEXT NOT NULL DEFAULT 'active'       -- active|revoked|expired
);

CREATE TABLE devices (
  id          TEXT PRIMARY KEY,
  license_id  TEXT NOT NULL REFERENCES licenses(id),
  machine_id  TEXT NOT NULL,
  activated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE telemetry_consent (
  user_id      TEXT PRIMARY KEY REFERENCES users(id),
  consented    INTEGER NOT NULL DEFAULT 0,
  crash_reports INTEGER NOT NULL DEFAULT 0,
  updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE policy_config (                        -- enterprise silent deploy (REQ-021)
  id            INTEGER PRIMARY KEY,
  scope         TEXT NOT NULL,                     -- user|org|device
  policy_json   TEXT NOT NULL,
  applied_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE webhook_subscriptions (                -- enterprise events (REQ-016)
  id          TEXT PRIMARY KEY,
  org_id      TEXT NOT NULL REFERENCES orgs(id),
  url         TEXT NOT NULL,
  events      TEXT NOT NULL,                       -- json array
  secret      TEXT NOT NULL,                       -- HMAC secret (DPAPI at rest)
  active      INTEGER NOT NULL DEFAULT 1
);
```

- WAL mode; `PRAGMA foreign_keys = ON`; migrations versioned in `schema.rs` (replacing
  the legacy BLOB schema) with a `schema_version` row.
- The old `original_docx`/`template_docx` BLOB columns and Base64 bridge are deleted
  together with the legacy schema (ADR-003, ADR-004).

---

## 13. Auth / Authorization

- **Local identity (Free/Pro):** single-device profile; `users` row created locally.
  Consumer value is zero-friction — no forced login.
- **Business:** local user registry + org scoping; admin assigns RBAC roles; the
  `licensing` module tracks seat assignment (`users.license_seat_id`).
- **Enterprise SSO (SAML, REQ-021):** `AuthService.authenticate` accepts a SAML
  assertion from the configured IdP; the verified `external_sub` is mapped (JIT or
  admin-provisioned) to a local `users` row with an RBAC role. SSO is additive: local
  auth remains for air-gapped installs.
- **Enforcement points:** (1) every Tauri command in the services layer,
  (2) every CLI subcommand, (3) every REST bridge route — `governance::authorize`
  is called before any domain operation. The UI reflects but never enforces roles
  (AC-011).
- **Policy files (enterprise):** `policy_config` overlays defaults — allowed roles,
  update channel, telemetry off, IdP endpoint, license pool (REQ-021, AC-021).

---

## 14. External Dependencies

| Dependency | Use | Rationale / Decision |
|---|---|---|
| `quick-xml` 0.37 | Streaming `document.xml` parse/serialize for tag + fill | Already adopted; streaming (constant memory); no regex mutation (constraint; ADR-002) |
| `zip` 2.x | OPC container read/write (docx, dfpkg) | Already adopted; handles arbitrary zip members with validation |
| `rusqlite` 0.32 (bundled) | SQLite metadata index + audit | Bundled — no native SQLite dependency on user machines |
| `mammoth` (JS) | DOCX → HTML preview | Renders faithfully; output MUST be DOMPurify-sanitized (ADR-006) |
| `DOMPurify` (JS) | Sanitize preview HTML | CSP complement; blocks embedded script/event-handler exfiltration |
| PDF engine | Print-to-PDF | ADR-005: WebView2 headless primary + guided LibreOffice fallback |
| `tauri-plugin-dialog` | Native file picker | Picker-confirmed paths only (REQ-018) |
| `serde`/`serde_json` | Typed IPC + REST contracts | Structured errors and DTOs (REQ-003) |
| `uuid` | Identifiers | Immutable audit ids |
| `dirs` | App-data location | Cross-platform data dir |
| Sentry (opt-in) | Crash reporting | Consent-gated; stripped in enterprise builds (REQ-020) |
| Paddle / license service (optional cloud) | Billing + license issuance | Zero-knowledge: activation facts only (REQ-019, ADR-007) |
| WebView2 runtime | Tauri webview + headless print | Ships with Win10/11; Tauri bootstrap installs if absent — keeps clean-VM promise |

**Removed:** `docxtemplater`, `pizzip` (JS doc generation) — killed by ADR-001;
Base64 bridge — replaced by binary IPC (ADR-004); `soffice` ghost dependency — replaced
by ADR-005.

---

## 15. Technology Decisions — Summary

| # | Decision | Rationale |
|---|---|---|
| D1 | Unified Rust `docx_engine` owns tag + fill (kill docxtemplater) | One parser/behavior; AC-001; ADR-001 |
| D2 | Cross-run XML replacement via quick-xml with run merging | REQ-002; no regex on `document.xml`; ADR-002 |
| D3 | FS-backed template storage + SQLite index, no BLOBs | REQ-004; small DB; DPAPI-encryptable files; ADR-003 |
| D4 | Binary IPC replaces Base64 for >1MB payloads | REQ-005; 33% payload reduction; ADR-004 |
| D5 | WebView2 headless print-to-PDF primary; guided LibreOffice fallback | REQ-006/AC-006 clean-VM; ADR-005 |
| D6 | Strict CSP + DOMPurify + sandboxed iframe previews | REQ-017/AC-017; ADR-006 |
| D7 | Zero-knowledge licensing/telemetry | REQ-019/020; GDPR/SOC 2 scope; ADR-007 |
| D8 | Deterministic generation only (no LLM in path) | Positioning + REQ-003 determinism; ADR-008 |
| D9 | Layered core/services/shell with port-based I/O | Headless reuse by CLI/server; REQ-016 |
| D10 | RBAC + audit enforced in Rust services, not UI | AC-011/013; tamper-resistant by construction |
| D11 | SQLite WAL metadata-only | Offline-first; instant startup; ADR-003 |
| D12 | Signed multi-channel distribution (MSIX/MSI/EXE) with SBOM | REQ-022; Intune/WSUS silent deploy |

---

## 16. Requirement Traceability

Every requirement maps to a module (or an ADR where the decision is architectural).

| ID | Requirement (abridged) | Module / Artifact | AC |
|---|---|---|---|
| REQ-001 | Unified docx_engine, no frontend generation | `docx_engine` · ADR-001 | AC-001 |
| REQ-002 | Cross-run XML replacement, first-run formatting | `docx_engine` · ADR-002 | AC-002 |
| REQ-003 | Unclosed-tag structured error, no corruption | `docx_engine` · §8 error model | AC-003 |
| REQ-004 | FS-backed storage, SQLite index only | `template_store` · ADR-003 | AC-004 |
| REQ-005 | Binary IPC, no Base64 hot path | `gui_shell` · ADR-004 | AC-005 |
| REQ-006 | PDF export without LibreOffice | `export` · ADR-005 | AC-006 |
| REQ-007 | Formats: docx, pdf, html preview, dfpkg | `export` · §5.3 | AC-007 |
| REQ-008 | Template Creator (upload, 5 field types, preview) | `gui_shell` + `template_store` · §3.1 | AC-008 |
| REQ-009 | Template Filler (validate, preview, export) | `gui_shell` + `docx_engine` · §8 | AC-009 |
| REQ-010 | Versioning: draft/review/published/archived + rollback | `template_store` · §5.4 | AC-010 |
| REQ-011 | RBAC: viewer/filler/creator/approver/admin | `governance` · §13 | AC-011 |
| REQ-012 | Approval workflow; published-only fillable | `governance` · §5.4 | AC-012 |
| REQ-013 | Immutable exportable audit log | `governance` · §12 `generation_log` | AC-013 |
| REQ-014 | Admin console: users/seats/licenses/audit/reports | `governance` + `gui_shell` · §3.6 | AC-014 |
| REQ-015 | Offline activation, device caps, grace, enterprise files | `licensing` · ADR-007 | AC-015 |
| REQ-016 | CLI + local REST bridge + webhooks | `docforge-core` reuse · §6.2/§6.3 | AC-016 |
| REQ-017 | Strict CSP + sanitized mammoth HTML | `gui_shell` + `export` · ADR-006 | AC-017 |
| REQ-018 | DOCX magic/zip validation + path traversal guard | `docx_engine` + `gui_shell` · §7 | AC-018 |
| REQ-019 | DPAPI storage encryption; zero-knowledge licensing/telemetry | `template_store` + `licensing` · ADR-007 | AC-019 |
| REQ-020 | Opt-in telemetry/crash, aggregate only, disable-able | `gui_shell` + `TelemetryService` · §9 | AC-020 |
| REQ-021 | SSO/SAML, on-prem/air-gapped, policy-file deploy | `governance` + `AuthService` · §13 | AC-021 |
| REQ-022 | Signed auto-update, staged rollout, rollback, SBOM | `gui_shell` + `UpdateService` · §11 | AC-022 |
| REQ-101 | 10MB tag/fill < 2s; UI never blocks | `docx_engine` + thread pool · §10 | — |
| REQ-102 | 100% tag fidelity on 50-fixture corpus | `docx_engine` test gate · ADR-002 | — |
| REQ-103 | Win10/11 primary; clean-VM zero manual deps | §11, ADR-005 | — |
| REQ-104 | Unit/component/e2e coverage; CI gate | SDLC `verify_module.py` + CI | — |
| REQ-105 | GDPR local-first; SOC 2 scoped; DPA + whitepaper | §9, §11, ADR-007 | — |

**ADR index:** ADR-001 unified Rust engine · ADR-002 cross-run replacement · ADR-003
FS storage · ADR-004 binary IPC · ADR-005 PDF engine · ADR-006 CSP/previews · ADR-007
licensing/zero-knowledge · ADR-008 deterministic no-AI. All in `docs/adr/`.

---

## 17. Migration Path (prototype → target)

1. Introduce `docforge-core` crate (docx_engine port of `commands.rs` logic + new
   cross-run fill); keep legacy Tauri commands temporarily shimmed to the core.
2. Swap `schema.rs` to Data Model v2 with a one-time migration: copy BLOB rows to
   `templates/<id>/v1/` on disk, drop BLOB columns (REQ-004).
3. Replace Base64 bridge with binary IPC; update React screens to typed DTOs.
4. Replace `soffice` export with WebView2 print bridge (ADR-005); remove
   `docxtemplater`/`pizzip` from `package.json` (AC-001).
5. Add CSP, DOMPurify, DOCX validation; then RBAC/governance/licensing layering.
6. Add CLI/server shells reusing the same core; run the 50-fixture gate at every phase.
