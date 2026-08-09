# ADR-001: Unified Rust docx_engine (kill docxtemplater)

## ADR-001: Unified Rust docx_engine replaces the dual engine

**Context:** The prototype runs two independent document pipelines: a Rust backend
(`commands.rs`) that tags selected text with `{{tag}}` using quick-xml, and a
JavaScript frontend (`docxProcessor.ts`) that fills `{{tag}}` → value using
docxtemplater + PizZip. Two parsers, two behavior models. Divergences are already
documented in audits and violate REQ-001/AC-001. The product plan explicitly
decommissions the dual engine (product-plan §5.2) and the target architecture mandates
one headless core (`docforge-core`) shared by GUI, CLI, and server shells.

**Decision:** One Rust module, `docx_engine`, owns **both** operations:
`tag_document` (selection → placeholder, cross-run aware) and `fill_document`
(placeholder → value, cross-run aware, unclosed-tag detection). The frontend keeps
preview rendering only (mammoth). `docxtemplater` and `pizzip` are removed from
`package.json`; `src/lib/docxProcessor.ts` is reduced to preview/byte-helper utilities
with no zip/XML logic. All document logic is compiled once, tested once (including the
50-fixture corpus), and reused by every shell.

**Alternatives:**
1. Keep docxtemplater in the frontend, only replace the tagging backend — rejected:
   preserves the drift and the JS zip dependency.
2. Port the JS filling behavior to Rust only for CLI, keeping JS for GUI — rejected:
   creates a second dual engine across shells.
3. Use a third-party Rust DOCX template crate — rejected: immature/less maintained
   than quick-xml + explicit control; determinism and fixture fidelity need in-house
   run-level control (ADR-002).

**Consequences:**
- Positive: single source of truth; AC-001 verifiable by code review (no docxtemplater
  import); headless shells get identical behavior for free (REQ-016); one test corpus
  gates all surfaces.
- Negative: the fill algorithm must be reimplemented in Rust (work), and `fill_document`
  must match the legacy docxtemplater behavior on the fixture corpus before cutover.
- Migration: existing stored templates use `{{tag}}` delimiters — the new engine keeps
  `{{...}}` syntax, so stored artifacts remain compatible; the one-time storage
  migration (ADR-003) preserves all templates.
