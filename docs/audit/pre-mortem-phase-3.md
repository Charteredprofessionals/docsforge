# Pre-Mortem Audit Report: DocForge (Implementation Phase 3)

> [!CAUTION]
> **Scenario: 6 months from now.**
> A user tries to fill a 15MB template with 50 fields. The app freezes for 8 seconds, then "OutOfMemory" crashes the WebView. Half the fields are missing because the Rust backend's "Simple Search & Replace" logic and the Frontend's "Docxtemplater" logic have drifted out of sync.

---

## 🚩 Phase 3: Technical Implementation Risks

### 1. Architectural Schizophrenia (Duplicate Logic)
The project currently implements document generation logic in **two disconnected places**:
- **Backend (Rust):** `replace_across_xml_runs` in `commands.rs`.
- **Frontend (TS):** `Docxtemplater` in `docxProcessor.ts`.
This leads to "Single Source of Truth" failure. If a template is saved on the backend but filled on the frontend, which XML structures are preserved? This is a recipe for silent document corruption.

### 2. Main-Thread Blocking (The Base64 Penalty)
Frontend functions like `base64ToArrayBuffer` and `arrayBufferToBase64` iterate over full multi-MB documents in a single synchronous loop on the JS main thread.
- **Evidence:** `docxProcessor.ts:44` (for loop with `+=` string concatenation). This is $O(N^2)$ or at least extremely slow for large strings.

### 3. No Validation for `Docxtemplater`
The frontend blindly calls `doc.render(fieldValues)`. If the template has malformed tags (e.g. `{{name}`), the entire generation fails, and the user is left with a generic "Something went wrong" or a silent crash.
- **Evidence:** `docxProcessor.ts:70` lacks try/catch and error reporting.

### 4. Shadow Dependency: LibreOffice
The backend's PDF export is a "Ghost Dependency." It requires a system-wide LibreOffice installation without any check or installer-guided prompt.
- **Evidence:** `commands.rs:330`.

---

## 🛡️ Hard Recommendations

1. **Unify Logic:** Choose **either** the Rust backend or the Frontend for document processing. Recommendation: Use the Rust backend (it's faster for large files) but use a proper XML library instead of manual replacement.
2. **Streaming Binary:** Use Tauri's `Binary` IPC to move `ArrayBuffer` data directly without Base64 overhead.
3. **WebWorkers:** If processing must happen on the frontend, move `mammoth` and `docxtemplater` to a WebWorker to prevent UI freezing.
4. **Graceful PDF Fallback:** Implement a PDF export that doesn't need LibreOffice, OR add a UI-driven check for `soffice` existence with a helpful "Download" link.

---

**Report Generated:** 2026-04-06
**Implementation Auditor Status:** Very Concerned 🧐
