# ADR-004: Binary IPC

## ADR-004: Binary IPC (raw bytes) replaces Base64 string bridging

**Context:** The prototype transfers document payloads across the Tauri bridge as
Base64-encoded strings (`TemplateFull.template_docx_b64`, `ExportPdfRequest.docx_base64`,
`upload_docx`). Base64 inflates payloads by ~33%, forces two full copies in JS
(`base64ToArrayBuffer`, `arrayBufferToBase64` in `docxProcessor.ts`), and blocks at ~1MB
with visible UI jank. REQ-005/AC-005 require binary transfer for large templates
(15MB fixture) with no Base64 in the hot path.

**Decision:** Tauri 2 IPC arguments/results carry `Vec<u8>`/`Uint8Array` raw bytes for
document payloads; the shell sends binary directly in `invoke` calls. Structs with byte
fields (`TemplateDetail.bytes`, `TagTemplateRequest.bytes`, `ExportArtifact.bytes`)
are serialized with binary-aware encoding (raw array payloads; no Base64 in the hot
path). Base64 survives only in cold, non-performance places (e.g. `.dfpkg` bundle
metadata, legacy-format import). For >10MB files the save path additionally streams in
chunks to disk (ADR-003) so the JS heap never holds a double copy of the document.

**Alternatives:**
1. Keep Base64 with compression before encode — rejected: CPU cost + still two copies;
   does not satisfy AC-005's "without Base64 in the hot path".
2. Write files to a temp path and pass paths across IPC — rejected: path-passing
   enlarges the file-picker/traversal attack surface (REQ-018) and forces temp-file
   lifecycle management; binary IPC keeps bytes in-process and validates in Rust.
3. Sidecar localhost HTTP transfer — rejected: heavier, needs port/security handling;
   Tauri's native IPC is the designed mechanism.

**Consequences:**
- Positive: ~33% payload reduction, lower memory, smooth 15MB transfers (AC-005);
   renderer never handles document bytes in transit (bytes stay in Rust until preview).
- Negative: DTOs must be updated across all command boundaries (TypeScript + Rust) in
   one coordinated change; UI code can no longer rely on stringly-typed payloads —
   mitigated by a typed `types.ts` contract update.
