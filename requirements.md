# DocForge — Requirements (Phase 0)

> Source: `docs/company/product-plan.md`, `docs/business/*` | Version 1 | Traceable via REQ-NNN

## Functional Requirements

| ID | Category | Description |
|---|---|---|
| REQ-001 | core | Unified document core (`docx_engine`) owns BOTH template tagging and filling; no generation logic in the frontend (kills dual engine). |
| REQ-002 | core | Cross-run XML replacement: user-selected text spanning multiple `<w:t>` runs must be tagged correctly, preserving formatting from the selection's first run. |
| REQ-003 | core | Fill operation must detect leftover/unclosed tags (`{{`) and return a structured error instead of silent corruption. |
| REQ-004 | data | Template documents stored on the filesystem (app-data dir); SQLite stores paths + metadata only (no BLOBs). |
| REQ-005 | data | Binary IPC (raw bytes) replaces Base64 string bridging for large document transfer. |
| REQ-006 | export | PDF export must not require LibreOffice; bundled engine or headless print-to-PDF with pixel-faithful output on a clean Windows VM. |
| REQ-007 | export | Export formats: DOCX, PDF, HTML preview, `.dfpkg` template bundle (docx + fields + metadata + version). |
| REQ-008 | gui | Template Creator: upload DOCX, define fields (text/date/dropdown/checkbox/signature), preview, save. |
| REQ-009 | gui | Template Filler: fill values, preview, export; structured validation errors per field. |
| REQ-010 | data | Template versioning: draft/review/published/archived states + version history with rollback. |
| REQ-011 | governance | RBAC roles: viewer, filler, creator, approver, admin; enforced at command and UI level. |
| REQ-012 | governance | Approval workflow: draft → approve → publish; only published templates fillable by non-creators. |
| REQ-013 | governance | Immutable, exportable audit log: who/what/when/template version/format/status for every generation. |
| REQ-014 | admin | Admin console: users, seats, licenses, template library, audit log viewer, usage reports (aggregate only). |
| REQ-015 | licensing | Offline activation: Pro (2 devices), Business (per-seat pool), Enterprise (offline-issued files, no phone-home after activation); 30–90 day grace windows. |
| REQ-016 | headless | CLI: `docforge generate --template X --data data.json --out out.docx`; local REST bridge (enterprise); webhooks on generation events. |
| REQ-017 | security | Strict CSP in Tauri; mammoth HTML output sanitized before rendering. |
| REQ-018 | security | DOCX validation: magic bytes + zip structure checked before processing; path traversal guard on file picker. |
| REQ-019 | security | Local storage encryption (DPAPI on Windows); zero-knowledge licensing/telemetry (never captures document contents). |
| REQ-020 | telemetry | Opt-in telemetry + crash reporting (consent screen); aggregate counts/timing only; fully disableable in enterprise builds. |
| REQ-021 | ent | SSO/SAML authentication (enterprise); on-prem/air-gapped build; policy-file configuration for silent deploy (Intune/WSUS MSI). |
| REQ-022 | ent | Signed auto-update with staged rollout + rollback; update channels (stable/beta); SBOM published per release. |

## Non-Functional Requirements

| ID | Category | Target |
|---|---|---|
| REQ-101 | performance | 10MB template: tag/fill < 2s, UI never blocks (generation off main thread). |
| REQ-102 | reliability | 100% tag fidelity on the 50-fixture DOCX corpus (tables, headers/footers, multi-run, RTL, tracked changes). |
| REQ-103 | compatibility | Windows 10/11 (x64) primary; macOS/Linux secondary; clean-VM install with zero manual dependencies. |
| REQ-104 | quality | Unit + component + e2e coverage; CI test gate required before any release candidate. |
| REQ-105 | compliance | GDPR local-first; SOC 2 Type II for licensing/billing/telemetry services; DPA + security whitepaper for enterprise. |

## Out of Scope (v1.0)
- AI-assisted document generation (deterministic-only by design — differentiator)
- E-signature (integration point only, not core promise at GA)
- Cloud document sync (opt-in, post-v1.5)
