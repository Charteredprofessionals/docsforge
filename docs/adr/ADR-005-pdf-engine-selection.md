# ADR-005: PDF Engine Selection

## ADR-005: PDF engine selection — bundled headless Chromium primary, guided LibreOffice fallback

**Context:** PDF export currently shells out to `soffice` (`export_to_pdf`,
`commands.rs:352`) and hard-fails with "please install LibreOffice" when absent —
a ghost dependency that violates REQ-006 (no LibreOffice required) and AC-006 (PDF
succeeds on a clean Windows VM with no LibreOffice). The release definition requires
`clean_vm_pdf: true`. Candidates were evaluated against: pixel fidelity for Word
layouts, zero manual installs, offline operation, license posture, and Windows-10/11
as the primary platform.

**Decision:** Two-tier strategy.
1. **Primary — bundled headless Chromium print-to-PDF (WebView2/Edge).** Tauri on
   Windows already depends on the WebView2 runtime (preinstalled on Win10/11; the
   Tauri installer bootstraps it otherwise), so this adds no new runtime dependency.
   The `export` module renders the document through the same sanitized HTML path as
   the preview (`render_html_preview` → DOMPurify) and prints to PDF via a hidden
   WebView2 webview or `msedge --headless --print-to-pdf`, producing a faithful,
   deterministic, offline PDF.
2. **Fallback — guided, checksum-verified LibreOffice portable.** Offered only on
   explicit user opt-in (never silent, never bundled as an installer requirement) for
   layouts where Word-pixel fidelity exceeds HTML print fidelity. The download is
   checksum-verified; the fallback remains disabled by default and is disabled in
   Free tier (licensing gates it). Enterprise/air-gapped builds can pre-provision the
   portable LO payload via policy file.

**Alternatives:**
1. Native Rust DOCX→PDF renderer (e.g. `printpdf`) — rejected: a full Word layout
   engine is out of scope; fidelity would not meet REQ-102/REQ-006 standards.
2. Headless Chromium only, no fallback — rejected: complex Word layouts (headers,
   exact pagination, section breaks) degrade in HTML print; the opt-in fallback is the
   honesty valve for fidelity.
3. Keep `soffice` as the only path — rejected: violates REQ-006/AC-006 directly.
4. Ship a bundled LibreOffice portable always — rejected: ~300MB payload, update
   complexity, and needless weight for the common consumer case.

**Consequences:**
- Positive: clean-VM PDF with zero installs (AC-006); offline-first preserved; the
  HTML preview path is exercised by PDF too (one renderer to audit for sanitization);
  enterprise can pre-provision portable LO for pixel-fidelity needs.
- Negative: WebView2 print fidelity is layout-good but not pixel-identical to Word for
  exotic documents — documented in the help UI as "High fidelity" vs "Best fidelity"
  modes; headless Edge invocation adds a process dependency on Windows (mitigated by
  using the in-process WebView2 where available).
- Verification: e2e test renders a canonical fixture on a clean Windows VM and
  compares against a golden PDF (Sprint 0 proof per viability §10).
