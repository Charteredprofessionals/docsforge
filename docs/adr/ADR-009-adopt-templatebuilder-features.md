# ADR-009: Adopt High-Impact Features from the `templatebuilder` Sibling Project

## Status
Accepted (2026-08-10)

## Context
A parallel DocForge variant (`D:\PROJECTS\templatebuilder`, v0.1.0) prototyped several
features our v2.0.0 lacked. A structured comparison (see `docs/COMPARISON_TEMPLATEBUILDER.md`)
showed it was feature-richer but architecturally divergent (dual-engine `docxtemplater`,
BLOB storage, `csp: null`, no RBAC/audit) and was an unfinished/experimental snapshot.

We adopted only the **high-impact, architecture-aligned** features, re-implementing them
inside our Rust-core design (ADR-001 single source of truth, ADR-003 FS-backed storage +
SHA-256) rather than copying the dual-engine approach.

## Decision
Adopt the following, implemented in `docforge-core` (not the frontend):

1. **Native-Rust PDF export fallback** (`printpdf` + `docx-rs`).
   - Replaces the previous stub `print_bridge` and removes the hard LibreOffice dependency
     for PDF export. `export_to_pdf` now prefers high-fidelity LibreOffice conversion and
     transparently falls back to `export_pdf_from_docx` when LibreOffice is absent.
   - Rationale: PDF export previously *required* LibreOffice (a documented limitation). This
     makes PDF work out-of-the-box on any machine.
2. **Database backup / restore** (`backup_database`, `restore_database` commands).
   - Copies / replaces the local SQLite DB via the OS file dialog.
3. **Template Bundles** (`bundles` + `bundle_templates` tables, v3 migration;
   `core/bundles.rs`; 6 Tauri commands; Bundles UI).
   - Groups templates for batch processing.

Deferred (not adopted, recorded for roadmap): formula/computed fields, signature fields,
CSV bulk-import, MS Store purchase flow, Word-COM PDF path. These are either frontend-heavy
(dual-engine style) or lower priority than the three above.

## Consequences
- PDF export no longer depends on an external install → broader "complete, working desktop
  app" guarantee.
- Four new DB tables; migration bumped to `user_version` 3 (idempotent + auto-repair).
- Binary size grows (~4.9 MB → ~6.8 MB) due to `printpdf`/`docx-rs`; still well within
  budget and installer remains ~3.6 MB MSI.
- New regression tests added (`core::bundles::tests`, `core::export::pdf::tests`).
- `print_bridge.rs` stub retained only as a thin trait; native conversion is the default.
