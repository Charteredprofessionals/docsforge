# DocForge

**Deterministic, offline-first document automation for Windows.**

**Version:** 2.0.0 (Windows-only)  
**Platform Support:** Windows 10/11 | Mac/Linux coming in v2.0.1

DocForge turns a Word (`.docx`) document into a reusable **template** with fillable fields,
then generates completed documents on demand — as Word (`.docx`) or PDF — entirely on your
machine. No cloud. No accounts. No AI.

> DocForge is built for organizations that need reproducible, auditable document generation
> without sending sensitive data to third-party services.

---

## ⚠️ Platform Support

**Current Release (v2.0.0):**
- ✅ **Windows 10/11:** Full support with DPAPI encryption
- ❌ **Mac/Linux:** Cross-platform support coming in v2.0.1

**Why Windows-only in v2.0.0?**  
This release uses Windows DPAPI for secure at-rest encryption of licenses and settings.  
v2.0.1 will add cross-platform encryption using the `age` cryptography library.

---

## Why DocForge

- **Offline by design** — every operation runs locally; nothing leaves the device.
- **Deterministic** — the same inputs always produce byte-stable output (no AI drift).
- **Auditable** — RBAC, versioning, and an audit trail are part of the core engine.
- **Self-serve templates** — non-technical users create templates by selecting text in a
  preview and labeling fields.

---

## Features

| Feature | Description |
|---------|-------------|
| Template creator | Upload a `.docx`, select text → label it as a fillable field, save as a template. |
| Field filling | Type values into fields and generate a finished document. |
| Word export | Download a filled, formatted `.docx` (`<TemplateName>_filled.docx`). |
| PDF export | Convert the filled document to PDF (requires LibreOffice on `PATH`). |
| Versioning | Snapshot versions with non-destructive rollback. |
| Integrity | SHA-256 content verification on every stored template. |
| Governance | Role-based access control and generation audit logging. |
| Licensing | Tiered, zero-knowledge offline licensing. |
| CLI / On-Prem | Headless `docforge-cli` and air-gapped `docforge-onprem` engines. |

---

## Architecture

DocForge is a **Tauri 2** desktop application. The UI is **React 18 + TypeScript** (Vite);
all document logic lives in a unified **Rust** core (`docforge-core`).

```
src/                     React + TypeScript frontend (Tauri IPC bridge)
src-tauri/
  src/
    core/                docx_engine, template_store, governance, licensing,
                         versioning, fields, export (docx/html/pdf/dfpkg)
    services/            service facade (runs off the UI thread)
    infra/               crypto (DPAPI at-rest), print_bridge
    commands.rs          Tauri command surface
    lib.rs               app entry / command registration
  Cargo.toml
```

Key design decisions (see `docs/adr`):
- A single Rust core owns **all** document logic — the frontend never re-implements filling.
- Templates are stored on the filesystem; SQLite holds only an index + SHA-256 metadata (no BLOBs).
- PDF rendering uses a headless print path (no LibreOffice dependency for the core; LibreOffice
  is used only for the optional PDF export step).

---

## Build & Install

### Prerequisites
- Windows 10+ (64-bit)
- [Rust stable](https://rustup.rs/)
- Node.js 20+
- (Optional, for PDF export) [LibreOffice](https://www.libreoffice.org/) on `PATH`

### From source

> **⚠️ Build the GUI binary ONLY through the Tauri CLI.** DocForge is a **Tauri 2** app; the
> frontend (`dist/`) is embedded into the binary *only* by `tauri build`. A plain
> `cargo build --release` compiles but does **not** embed the UI, so the launched app falls back
> to the dev URL `http://localhost:5173` and fails with **`ERR_CONNECTION_REFUSED`**
> ("localhost refused to connect"). Always use `npm run build:tauri`.

```bash
npm install
npm run build          # type-checks + builds the frontend
npm run build:tauri    # runs npm run build, then tauri build (embeds the frontend)
```

Artifacts land in `src-tauri/target/release/bundle/`.

### Installers / Release package

`npm run build:tauri` produces Tauri's own bundles in `src-tauri/target/release/bundle/`:
- `msi/DocForge_2.0.0_x64_en-US.msi` (Windows Installer)
- `nsis/DocForge_2.0.0_x64-setup.exe` (NSIS)
- `msix/DocForge_2.0.0_x64.msix` (MSIX — **requires a code-signing certificate**; built by the
  GitHub Actions pipeline, which signs with the `WINDOWS_CERTIFICATE` / `WINDOWS_CERTIFICATE_PASSWORD`
  secrets or a generated self-signed cert for CI testing)

The **canonical shipped Windows installer** is built separately from the embedded binary with
Inno Setup (`installer.iss`):
```bash
& "C:\Users\cscha\AppData\Local\Programs\Inno Setup 6\ISCC.exe" installer.iss
```
Output: `exports/windows/DocForge_2.0.0_x64-setup.exe`.

> **Note:** `exports/windows/` and `*.exe` are git-ignored — the installer is a build artifact
> and is **not** committed to the repo.

---

## Usage

1. **New Template** → upload a `.docx` → select text and label fields → **Save Template**.
2. Open a template → fill the field values → **Preview**, **Export Word**, or **Export PDF**.

A full walkthrough is in [`docs/USER_MANUAL.md`](docs/USER_MANUAL.md).

---

## Privacy & Data Residency

All templates and generated documents are stored on the local machine. At-rest encryption
uses Windows DPAPI (`infra/crypto.rs`). The `docforge-onprem` build hard-disables telemetry
at compile time for air-gapped deployments.

---

## Repository Layout

```
docs/            Architecture decisions (adr/), audits, business docs, USER_MANUAL
src/             Frontend (React + TS)
src-tauri/       Rust core (docforge-core), services, infra
tests/           Integration tests
exports/         Generated release metadata (SBOM, quality gate, manifests)
```

---

## License

See repository license file. DocForge is distributed as a commercial product of DocForge, Inc.

---

*DocForge — Deterministic, offline-first document automation. No accounts. No cloud. No AI.*
