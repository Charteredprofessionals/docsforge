# DocForge, Inc. — Company & Product Plan

> **Scope:** Consumer / Company (SMB) / Enterprise readiness
> **Owner:** Executive Team (CEO + CTO) | **Engine:** Authr OS SDLC Studio (9-agent system)
> **Status:** PLANNING — GATE 0 passed (Conditional GO), pending Phase 1 architecture approval

---

## 1. Company Identity

```
Company:   DocForge, Inc.
Product:   DocForge — deterministic, privacy-first document automation
Mission:   Eliminate repetitive document work without surrendering data to the cloud.
Positioning: "Deterministic document automation that never sees your data."
           Offline-first. Governed templates. Headless core. Enterprise-grade trust.
Values:   Privacy by architecture · Determinism over AI magic · Templates are assets
           · Security is a feature, not a checkbox.
```

### Departments & Leadership (roles mapped to SDLC agents)

| Department | Lead | SDLC agent mapping | Mandate |
|---|---|---|---|
| Product | Product Manager | requirements_analyst | Roadmap, personas, acceptance criteria |
| Engineering | CTO | system_architect, tech_lead, feature_developer, integration_engineer, test_engineer | Architecture, build, verify |
| Quality & Review | QA Lead | code_reviewer, security_auditor | Paradigm compliance, audits, release gates |
| DevOps & Release | DevOps Engineer | devops_engineer, release_manager | Signing, packaging, distribution, updates |
| Data & Docs | Data Engineer / Doc Writer | data_engineer, doc_writer | Schema, exports, manuals, compliance pack |
| Business | Revenue Team | — | Pricing (see monetization-strategy), GTM, partnerships |
| Legal & Compliance | Counsel | security_auditor (audit trail) | DPA, SOC 2, GDPR, licensing |

---

## 2. Target Segments & Value Delivery

| Segment | Users | Core value | Entry product |
|---|---|---|---|
| **Consumer / Prosumer** | Freelancers, agents, notaries, admins | Generate polished Word/PDF docs from their own templates, offline, free/cheap | Free + Pro |
| **Company (SMB)** | Teams of 5–100 (legal, HR, ops, property mgmt) | Governed shared template library, consistent branding, audit trail | Business |
| **Enterprise** | Regulated/confidential (law firms, insurance, healthcare-adjacent, government) | On-prem/air-gapped, SOC 2, SSO, immutable audit, SLA, integrations | Enterprise |

**Enterprise readiness = capability pillars (Section 4) + compliance program (Section 5).**

---

## 3. Product Strategy

### 3.1 Product Principles
1. **Single source of truth:** One document core (Rust) — GUI, CLI, and API use it.
2. **Deterministic by design:** identical input → identical output, always. No AI in
   the generation path (differentiator against hallucination risk).
3. **Privacy by architecture:** document data never leaves the device unless the user
   opts into sync. Zero-knowledge licensing/telemetry.
4. **Templates are assets:** versioned, governable, auditable — the switching-cost moat.
5. **Enterprise features are additive:** consumer UX never regresses for enterprise.

### 3.2 Feature Roadmap Themes

**Foundation (must fix before any launch):**
- Unify document engine (kill dual engine: quick-xml backend + docxtemplater frontend)
- Cross-run XML replacement (tags spanning multiple `<w:t>` runs)
- Filesystem-backed template storage (SQLite index only; kill BLOB flooding)
- Binary IPC for large files (kill Base64 bridge)
- PDF export without LibreOffice ghost dependency
- CSP + DOCX HTML sanitization
- .gitignore + repo hygiene + CI test baseline

**Consumer (GA):** field types (text/date/dropdown/checkbox/signature), template
versioning, watermark/tiering, offline license activation, opt-in telemetry, signed
MSIX/MSI/EXE + winget, auto-update.

**Company (Business):** team library, RBAC (creator/approver/filler/viewer),
draft→approve→publish governance, admin console, exportable audit log, shared field
dictionaries, template health reports.

**Enterprise:** SSO/SAML, on-prem/air-gapped, immutable audit, SOC 2 report, DPA,
SLA, Intune/WSUS deployment, API/CLI headless, connectors (SharePoint, OneDrive,
Google Drive, Dropbox), floating licenses, data retention policies.

---

## 4. Enterprise-Readiness Pillars

### P1 — Trust & Security
- EV code signing (Windows) — tamper-evident binaries
- Signed auto-update with rollback
- CSP enabled in Tauri + sanitized mammoth HTML rendering
- Encrypted local storage (DPAPI on Windows; keychain on macOS)
- SBOM published with every release (SDLC Studio exporter)
- Zero-knowledge licensing: no document contents in any telemetry
- Vulnerability response policy + security.txt

### P2 — Compliance
- **GDPR:** local-first data processing → minimal controller surface; DPA offered;
  right-to-delete = delete local app data; telemetry is aggregated & consent-gated.
- **SOC 2 Type II:** scoped to licensing/telemetry/billing services (audit-ready by
  Enterprise milestone); report delivered to enterprise prospects.
- **Data residency:** on-prem/air-gapped option removes cloud entirely — the
  strongest residency story in the category.
- **Audit trail:** immutable, exportable generation log (who/what/when/template
  version) — the `generation_log` table becomes a first-class governed artifact.

### P3 — Licensing & Entitlement
- Offline activation with device limits; 30–90 day grace windows
- Pro: 2 devices · Business: per-seat pool (floating optional) · Enterprise:
  offline-issued license files (no phone-home after activation)
- Admin-managed seat assignment & revocation
- License transparency: users always see tier, seats, expiry

### P4 — Distribution & Updates
- **Consumer:** MSIX (Microsoft Store + sideload), winget, EXE from web
- **Company:** MSI (Intune/WSUS/GPO deployment) with silent install & config file
- **Enterprise:** MSI + offline update channel (no internet required)
- Signed auto-update, staged rollout, one-click rollback, update channels
  (stable/beta)

### P5 — Administration
- Admin console: users, seats, licenses, template library, audit log viewer,
  settings policy (JSON policy file for silent enterprise config)
- RBAC: viewer / filler / creator / approver / admin
- Template governance workflow with approval gates
- Usage reports (docs generated, by team, by template) — aggregated, no content

### P6 — Integrations & Headless
- **CLI** (`docforge generate --template X --data data.json --out out.docx`)
- **REST API** (local HTTP bridge; enterprise mode only)
- **Webhooks** on generation events (enterprise)
- **Connectors:** file pick/save via SharePoint, OneDrive, Google Drive, Dropbox
- Template import/export (`.dfpkg` bundle: docx + fields + metadata + version)

### P7 — Observability & Support
- Opt-in crash reporting (Sentry, consent-gated) — never captures doc content
- Aggregate usage analytics (counts/timing only)
- Support tiers: Community → Email (Pro) → Priority (Business) → Dedicated + SLA
  (Enterprise, 99.5% licensing uptime target)
- Status page for licensing/billing services

### P8 — Onboarding & Documentation
- User manual, admin guide, compliance pack (SOC 2 report, DPA, security whitepaper)
- Template gallery (20 seeded vertical templates: legal/HR/real-estate/insurance)
- Interactive first-run: "Import your own contract" guided activation
- CLI/API reference docs + cookbook examples

---

## 5. Technical Architecture Evolution

### 5.1 Target Architecture (Headless Core)

```
                    ┌────────────────────────────────────────────┐
                    │            docforge-core (Rust)             │
                    │  Single source of truth · no GUI deps       │
                    │  ────────────────────────────────────────   │
                    │  docx_engine  — parse / replace / fill      │
                    │                 (cross-run aware, quick-xml)│
                    │  template_store — FS-backed + SQLite index  │
                    │  governance   — versions, workflows, audit  │
                    │  export       — docx · pdf · html · dfpkg   │
                    │  licensing    — entitlement, grace, revoke  │
                    └───────────┬───────────┬───────────┬─────────┘
                                │           │           │
                    ┌───────────▼──┐  ┌─────▼─────┐  ┌──▼───────────┐
                    │ docforge-gui │  │ docforge- │  │ docforge-    │
                    │ (Tauri 2)    │  │ cli       │  │ server(opt.) │
                    │ React + Rust │  │ headless  │  │ REST bridge  │
                    └──────────────┘  └───────────┘  └──────────────┘
```

### 5.2 Decommissioning the Dual Engine (Critical)
- **Today:** backend quick-xml replaces text→`{{tag}}`; frontend docxtemplater fills
  `{{tag}}`→value. Two parsers, two behaviors → drift risk (documented in audits).
- **Target:** `docx_engine` owns **both** operations:
  - `tag_document(docx, fields)` — user-selected text → placeholder, **merged across
    XML runs**, preserving formatting from the first run of the selection
  - `fill_document(docx, values)` — placeholder → value, cross-run aware, with
    leftover-tag detection (`{{` unclosed → structured error, not silent corruption)
- Frontend keeps **preview only** (mammoth) — no generation logic in JS.

### 5.3 Data Model v2
```sql
templates      (id, name, org_id, version, status[draft|review|published|archived],
                storage_path, fields_json, created_by, created_at, updated_at)
template_versions (id, template_id, version, storage_path, fields_json,
                created_by, created_at, note)          -- governance + rollback
generation_log (id, template_id, version, fields_hash, output_name, format,
                user_id, machine_id, status, generated_at)  -- immutable audit
users          (id, name, email, role, license_seat_id, active)
orgs           (id, name, plan, settings_json)         -- Business/Enterprise
licenses       (id, org_id/user_id, tier, seats, devices, issued_at, expires_at)
```
- BLOBs **removed** from DB → documents stored under app-data/templates, paths in DB.

### 5.4 Performance & Reliability
- Binary IPC (Tauri `invoke` with raw bytes) replaces Base64 string plumbing
- Chunked/streaming save for >10MB templates
- Generation runs off the UI thread (Rust async or thread pool); WebView never blocks
- Fuzz corpus: 50 real-world DOCX fixtures (tables, headers/footers, multi-run,
  RTL, tracked changes) gating every release — tag fidelity must be 100%

### 5.5 Security Hardening (with current-state references)
- `tauri.conf.json` `"csp": null` → **replace with strict CSP**
- `dangerouslySetInnerHTML` (TemplateCreator/TemplateFiller previews) → sanitize
  mammoth output (DOMPurify) or render via iframe sandbox
- Validate DOCX magic bytes + zip structure before processing (`commands.rs:59`)
- Path traversal guard on file picker → only allow user-selected paths
- Keep `quick-xml` (already adopted — audit remediated) but upgrade `replace_in_text`
  (`commands.rs:257`) to run-aware merging

---

## 6. Compliance & Security Program Timeline

| Milestone | Deliverable | Target phase |
|---|---|---|
| M0 | Repo hygiene, .gitignore, CI test gate, security audit baseline | Sprint 0 |
| M1 | Strict CSP, sanitization, signed binaries, SBOM per release | Consumer GA |
| M2 | Opt-in telemetry + privacy policy + consent UX | Consumer GA |
| M3 | Exportable audit log, RBAC, admin console | Business |
| M4 | DPA template, security whitepaper, vulnerability policy | Business |
| M5 | SOC 2 Type II audit (scoped services) + report | Enterprise |
| M6 | On-prem/air-gapped build + offline licensing | Enterprise |

---

## 7. Phased Roadmap (Company Execution)

### Phase A — Sprint 0 · Takeover & Foundation (Weeks 1–2)
- Onboard into SDLC Studio: `config.json`, Phase 1 architecture, task DAG, approval gate
- `.gitignore` + first commit hygiene + CI baseline (`verify_module.py`)
- Test scaffolding: Rust unit tests (docx_engine), TS component tests, e2e smoke
- **Validate 3 GATE-0 items:** enterprise interviews, PDF engine proof, 50-fixture corpus

### Phase B — Reliability & Security Hardening (Weeks 3–6) → internal v0.2
- Unified `docx_engine` (tag + fill, cross-run), kill docxtemplater dependency
- Filesystem-backed storage + SQLite index v2; binary IPC
- PDF export via bundled engine (remove LibreOffice requirement)
- CSP + sanitization + DOCX validation
- Signed MSIX/MSI/EXE, winget, auto-update with rollback

### Phase C — Consumer GA (Weeks 7–10) → public v0.3
- Field types (text/date/dropdown/checkbox/signature)
- Template versioning + duplicate/rename
- Licensing & activation (Free/Pro), paywall, Paddle integration
- Opt-in telemetry + crash reporting
- Template gallery (20 vertical seeds), onboarding flow
- ProductHunt + MS Store launch, privacy-first messaging

### Phase D — Company/Business (Weeks 11–16) → v1.0
- Team library + RBAC + governance workflow (draft→approve→publish)
- Admin console, seat/license management, audit log viewer
- Shared field dictionaries, template health reports
- CLI + local REST bridge + `.dfpkg` import/export
- Exportable audit log, usage reports (aggregate only)

### Phase E — Enterprise (Weeks 17–28) → v1.5
- SSO/SAML, on-prem/air-gapped build, offline license files
- SOC 2 Type II scoped audit + compliance pack
- SLA + dedicated support, status page
- Intune/WSUS MSI + policy-file configuration
- Connectors: SharePoint, OneDrive, Google Drive, Dropbox
- 3 enterprise pilots → first 8 enterprise accounts

---

## 8. KPIs & Success Metrics

| Area | Metric | Target |
|---|---|---|
| North star | Successful document generations / week | 5× quarterly growth |
| Activation | First template created within 7 days | ≥ 40% |
| Retention | D30 active (paid) | ≥ 60% |
| Monetization | Free→paid conversion | 2–5% |
| Growth | Pro monthly churn | < 2% |
| Quality | Tag-fidelity on fixture corpus | 100% |
| Trust | Telemetry opt-in rate | > 60% |
| Enterprise | Enterprise ACV / signed pilots | 3 pilots by M6, 8 by M12 |
| Revenue | MRR | $34k by M12 ($408k ARR) |

---

## 9. Risk Register (Post-Takeover)

| Risk | L | I | Mitigation |
|---|---|---|---|
| AI commoditization | H | H | Deterministic/no-AI positioning, governed templates, compliance moat |
| Dual-engine corruption | H | H | Unified Rust core + 100% fixture gate (Phase B, first) |
| PDF engine replacement slips | M | M | Proof in Sprint 0; fallback: guided LibreOffice installer with checksum |
| Licensing infra adds cloud surface | M | M | Zero-knowledge scope; enterprise offline licenses; status page |
| Scope creep into enterprise before GA | H | M | Phase gates; enterprise features additive only |
| Single-owner knowledge loss | M | H | SDLC agent system, architecture docs, CI verification, code review |
| Regulatory (e.g., e-sign requirements) | M | M | Keep e-signature as integration point, not core promise at GA |

---

## 10. Immediate Actions (Next 48h)

1. **Approve GATE 0** (done — Conditional GO) and **approve this company plan**.
2. **Approve SDLC onboarding config** for `projects/docsforge`:
   - Preset: **Web App (Desktop)** · Language: **TypeScript + Rust** ·
     Framework: **React + Tauri 2** · Database: **SQLite** · Paradigm: **/clean-code**
     · Pattern: **Layered (core / services / shell)** · Modules: **6**
     (docx_engine, template_store, governance, licensing, export, gui_shell)
     · Distribution: **MSIX, MSI, EXE, ZIP**
3. Execute **Phase A (Sprint 0)**: SDLC Phase 1 architecture → approval gate → test
   scaffolding → 3 validations.
4. Create `.gitignore` at repo root before any commit.

---

**Approved:** Executive Team | **Date:** 2026-08-09
**Next gate:** Phase 1 Architecture Approval (SDLC Studio mandatory gate)
