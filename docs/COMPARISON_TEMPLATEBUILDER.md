# Comparison: `templatebuilder` (sibling variant) vs DocForge (`sdlc_studio/projects/docsforge`)

**Date:** 2026-08-10
**Purpose:** Evaluate which `templatebuilder` features to adopt into our product (see `ADR-009`).

## Summary
`D:\PROJECTS\templatebuilder` is another DocForge variant (v0.1.0, Mar–Apr 2026) built via a
different agent workflow. It is **feature-richer but architecturally divergent** and appears to be
an **unfinished/experimental snapshot** (no release binary; numerous `build_errors.txt`,
`cargo_errors.txt`, `hs_err_pid*.log` crash dumps present).

## Feature comparison
| Feature | templatebuilder | Ours (after adoption) |
|---|---|---|
| Template CRUD + versions + rollback | ✅ | ✅ |
| **Native-Rust PDF fallback** (no LibreOffice) | ✅ (printpdf/docx-rs) | ✅ **adopted** |
| **DB backup / restore** | ✅ | ✅ **adopted** |
| **Template Bundles** | ✅ | ✅ **adopted** |
| Formula / computed fields (mathjs) | ✅ | ⏸ deferred |
| Signature fields (ed25519-dalek) | ✅ | ⏸ deferred |
| CSV import (papaparse) | ✅ | ⏸ deferred |
| MS Store purchase flow | ✅ (simulated) | ⏸ deferred |
| Word-COM PDF path | ✅ | ⏸ deferred (LibreOffice preferred) |
| Governance / RBAC / audit | ❌ | ✅ (ours) |
| At-rest DPAPI encryption | ❌ | ✅ (ours) |
| Storage model | BLOB in SQLite | FS-backed + SHA-256 (ours, ADR-003) |
| Fill engine | Frontend `docxtemplater` (dual-engine) | Unified Rust (ours, ADR-001) |
| CSP | `null` | strict (ours, ADR-006) |

## Why we did NOT copy templatebuilder wholesale
- Its ADR is "LOCKED" on a **dual-engine** (frontend `docxtemplater`+`pizzip`) approach that our
  ADR-001 explicitly rejected (drift, no single source of truth).
- It stores `.docx` as **BLOBs** — our ADR-003 mandates FS-backed storage + SHA-256 (no BLOBs).
- `csp: null` and no RBAC/audit conflict with our security posture.

## Adopted (re-implemented in `docforge-core`)
1. **Native PDF fallback** — `core/export/pdf.rs` (`printpdf` + `docx-rs`); `export_to_pdf`
   prefers LibreOffice, falls back to native so PDF works with zero external deps.
2. **DB backup/restore** — `commands::backup_database` / `restore_database` + Admin → Data tab.
3. **Template Bundles** — `migrations.rs` v3, `core/bundles.rs`, `components/Bundles.tsx`.

## Deferred to roadmap
Formula fields, signature fields, CSV bulk-import, MS Store flow. These are either frontend-heavy
(dual-engine style) or lower priority than the three adopted items.
