# ADR-006: Strict CSP + Sanitized HTML Previews

## ADR-006: Strict CSP and sanitized HTML previews

**Context:** `tauri.conf.json` currently ships `"csp": null` (no Content-Security-
Policy), and both TemplateCreator and TemplateFiller render mammoth HTML through
`dangerouslySetInnerHTML` (REQ-017 flags this). A crafted DOCX could embed HTML/script
that executes in the privileged WebView, and Tauri's IPC would then be reachable by
attacker content. AC-017 requires a strict CSP and that a crafted docx with embedded
script cannot execute in the preview renderer.

**Decision:**
1. **Strict CSP** in `tauri.conf.json` replacing `"csp": null`: `default-src 'none'`;
   `script-src 'self'`; `style-src 'self' 'unsafe-inline'` (Tailwind/Vite emit inline
   styles — reviewed and kept minimal); `img-src 'self' data: blob:`;
   `connect-src 'self'` (IPC + local REST bridge host only);
   `frame-src 'self'` (sandboxed preview iframe); `object-src 'none'`;
   `base-uri 'self'`; `form-action 'none'`. No `unsafe-eval` anywhere. Tauri-specific
   allowances (e.g. `asset:` protocol) added only via capability-scoped ACLs, not CSP
   relaxation.
2. **Sanitized previews:** `export::render_html_preview` runs mammoth output through
   DOMPurify (allowlist: headings, paragraphs, tables, b/i/em, lists, links stripped of
   `javascript:`/event handlers) in the core before the HTML ever reaches React. The
   renderer displays it inside a `<iframe sandbox="allow-same-origin">` (no scripts,
   no forms, no top navigation) rather than `dangerouslySetInnerHTML`.
3. The PDF print path reuses the same sanitized HTML, so one sanitizer covers both
   preview and PDF (ADR-005).

**Alternatives:**
1. CSP `null` + raw `dangerouslySetInnerHTML` (status quo) — rejected: AC-017 fails
   outright.
2. Render previews server-side in Rust and send only pixels — rejected: loses
   selection/interaction UX needed by the template creator (REQ-008) and adds IPC
   video-like cost.
3. Iframe-only sandbox without DOMPurify — rejected: defense-in-depth; mammoth emits
   document-controlled HTML that must be normalized regardless of iframe isolation.

**Consequences:**
- Positive: AC-017/security_test passes; defense-in-depth (CSP + DOMPurify + sandboxed
  iframe); PDF inherits the same safe render path.
- Negative: CSP tightening can break dev hot-reload unless `devUrl` allowances are
  scoped to development only (build-time flag); DOMPurify must be updated with the
  frontend bundle; preview fidelity is limited by the allowlist — acceptable, preview
  is not the export artifact.
