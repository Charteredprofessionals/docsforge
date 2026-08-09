# Pre-Mortem Audit Report: DocForge (Initial Hardware Hardening)

> [!CAUTION]
> **Scenario: 12 months after release.**
> DocForge has been discontinued. The primary reason was a 4.2-star rating on the App Store that plummeted to 1.8 stars due to "PDF Export failures" and "Corrupted Documents." Enterprise users found the app unable to handle 50MB+ policy documents, leading to catastrophic data loss for template creators.

---

## 🧠 Reasoning Trace
**Task:** Initial Pessimistic Audit (Phase 1-3) | **Approach:** Adversarial System Analysis
1. **Dependency Analysis** → Found `soffice` (LibreOffice) dependency in `commands.rs`. This is a non-standard, multi-GB install requirement which 90% of end-users won't have. Major SPOF.
2. **Data Integrity Audit** → Reviewed `replace_across_xml_runs`. Manual string replacement in XML without a parser is inherently fragile. Complex DOCX files with nested namespaces or split runs will corrupt silently.
3. **Performance/Scaling Audit** → Storing full binary BLOBs in SQLite and passing them as Base64 strings across the Tauri bridge. This is "jank-by-design" for large documents.

---

## 🚩 Phase 1: Architectural Vulnerabilities

### 1. The "Ghost Dependency" (LibreOffice)
**Risk:** High | **Impact:** Fatal (UX)
PDF export relies on a system-wide `soffice` installation. This is an unacceptable requirement for a consumer/enterprise desktop app. Users expect "it just works." If LibreOffice isn't in the PATH, the core value prop (generating finalized docs) is broken.
- **Evidence:** `commands.rs:334`: `std::process::Command::new("soffice")`.

### 2. Manual XML Manipulation (The Corruptor)
**Risk:** High | **Impact:** Data Integrity
Docx files are zipped XML. The current approach uses regex-like string replacement inside `document.xml`. This ignores formatting namespaces, styles, and "dirty" XML splits where `{{` and `}}` might be separated by metadata tokens.
- **Evidence:** `commands.rs:135` (`document_xml.replace`) and `replace_across_xml_runs`.

### 3. SQLite BLOB Flooding
**Risk:** Medium | **Impact:** Performance
Storing full binary documents in the main DB will lead to massive file growth and read/write latency. SQLite is not optimized for frequently accessed multi-megabyte blobs in one table.
- **Evidence:** `schema.rs:16` - `original_docx BLOB NOT NULL`.

---

## 🚩 Phase 3: Setup & Feature Flaws

### 1. Memory Inefficiency (The Base64 Bridge)
Encoding binary data as base64 to cross the Rust/JS bridge consumes ~133% more memory and incurs overhead. Large templates will freeze the UI thread during IPC.
- **Evidence:** `commands.rs:43`, `commands.rs:283`.

### 2. No "Kill Switch" or Validation
The app doesn't validate if a DOCX is actually a DOCX before saving. It blindly tries to unzip and read `word/document.xml`. A malicious or corrupted file can crash the background worker or fill the disk with junk.
- **Evidence:** `commands.rs:58` lacks robust error handling for user feedback.

---

## 🛡️ Hard Recommendations

1. **Replace LibreOffice:** Move to a Rust-native PDF generator (e.g., `genpdf` or a headless Chromium print-to-pdf) or at least provide a "Download LibreOffice" setup wizard.
2. **Use a Real XML Parser:** Leverage a crate like `quick-xml` or `xml-rs` to manipulate the document structure SAFELY, ensuring tags remain valid after replacement.
3. **Move BLOBs to FS:** Store the documents on the filesystem (app data directory) and store only the **file path** in SQLite.
4. **Binary IPC:** Use Tauri's raw binary buffers instead of base64 for large document transfers.

---

**Report Generated:** 2026-04-06
**Auditor Status:** Skeptical 🤨
