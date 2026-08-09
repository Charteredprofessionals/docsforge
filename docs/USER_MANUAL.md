# DocForge — User Manual

**DocForge** is a desktop document-automation application for Windows. It lets you turn a
Word (`.docx`) document into a reusable **template** with fillable fields, then generate
completed documents on demand — as Word (`.docx`) or PDF — without ever sending your data
to the cloud. Everything runs locally on your machine.

> **Privacy note:** DocForge is fully offline. Documents, templates, and field values are
> stored only on your computer. No internet connection or account is required.

---

## 1. Getting Started

### System Requirements
- Windows 10 or later (64-bit)
- About 300 MB of free disk space for the installed app
- A `.docx` (Word) file to use as your first template

### Installation
1. Download the installer from your IT distributor or the release package:
   - `DocForge.msi` (recommended — install for all users or current user)
   - `DocForge.exe` (NSIS installer — single-user, portable-style install)
   - `DocForge.msix` (Microsoft Store / enterprise sideload package)
2. Run the installer and follow the prompts.
3. Launch **DocForge** from the Start Menu.

> If Windows SmartScreen appears, choose **More info → Run anyway** (the binary is
> self-signed in development builds; enterprise deployments should use the MSIX with your
> organization's code-signing certificate).

### The Three Screens
DocForge has three main views, switchable from the top navigation bar:

| View | Purpose |
|------|---------|
| **Templates** | Browse, open, and delete your saved templates |
| **New Template** | Turn a Word document into a reusable template |
| **Fill** | Enter field values and export a finished document |

---

## 2. Creating a Template

A *template* is a Word document with one or more **fillable fields**. Each field marks a
spot of text that gets replaced with a value when you generate a document.

1. Click **New Template** (or **Create First Template** on an empty library).
2. On the **Upload** step, click the dashed box and pick a `.docx` file.
   - DocForge reads the document and shows a live preview.
3. The **Create Template** step opens:
   - The template name is auto-filled from the file name (you can edit it).
   - **Select text in the preview** with your mouse, then a prompt asks for a **field label**
     (e.g. "Employee Name"). DocForge converts it to a safe tag like `employee_name` and
     highlights the text.
   - Repeat for every spot you want to fill later (names, dates, addresses, clauses, etc.).
   - To remove a field, hover its card in the right sidebar and click the **X**.
4. Click **Save Template**. You return to the Templates list.

**Tips**
- Field labels become `{{tag}}` placeholders inside the document. Keep labels unique and
  descriptive.
- A template must have at least one field and a name before it can be saved.
- The original Word formatting (headings, tables, styles) is preserved.

---

## 3. Using a Template (Generating Documents)

1. On the **Templates** screen, click a template card or its **Use Template** button.
2. The **Fill** screen shows one input box per field, pre-labeled with the field name and
   the original text it replaces.
3. Enter the values you want.
4. Choose an action:

| Action | What it does |
|--------|--------------|
| **Preview Document** | Fills the values into the document and shows a formatted preview (read-only). Switch back with **Edit Values**. |
| **Export Word** | Downloads a filled `.docx` file named `<TemplateName>_filled.docx`. |
| **Export PDF** | Converts the filled document to PDF and downloads it. |

> **PDF export requires LibreOffice** installed and available on your system `PATH`.
> If it is missing, DocForge shows a clear message and you can still export Word.
> For fully air-gapped / offline environments, install LibreOffice once; no network call
> is made during conversion.

---

## 4. Managing Templates

On the **Templates** screen:
- **Browse** all saved templates as cards showing the name, field count, and creation date.
- **Open** a template by clicking its card or **Use Template**.
- **Delete** a template by hovering its card and clicking the trash icon, then confirming.
  Deletion is permanent and removes the stored template and its filled-field definitions.

Templates are stored in your local app data directory (SQLite + the original `.docx`
files), so they persist across sessions and survive app restarts.

---

## 5. Troubleshooting

| Problem | Cause / Fix |
|---------|-------------|
| "Failed to load document" on upload | The file is not a valid `.docx`, or it has no `word/document.xml`. Re-save it from Word as a standard `.docx`. |
| Preview is blank or missing styles | The preview uses a simplified HTML conversion (mammoth). The exported `.docx` keeps full Word fidelity — preview is for review only. |
| PDF export fails | LibreOffice is not installed or not on `PATH`. Install LibreOffice, or use **Export Word**. |
| A field didn't get replaced | The selected source text must match the document exactly. Re-create the field by selecting the exact text in **New Template**. |
| Changes not saved | Ensure the template has a name and at least one field before clicking **Save Template**. |

### Error Messages
DocForge surfaces errors as red banners inside the app (e.g. "Failed to save template: …").
These are local operation messages — nothing is transmitted externally.

---

## 6. Privacy & Data Residency

- **Local-only:** All processing happens on your device. No telemetry is sent.
- **Storage:** Templates and documents live under your Windows user profile
  (`%APPDATA%` / local app-data for DocForge).
- **On-Premises build:** An air-gapped `docforge-onprem` engine variant is available for
  enterprise deployment with telemetry hard-disabled at compile time.

---

## 7. Command-Line (Advanced / Enterprise)

For automation and server/on-prem use, DocForge ships headless CLI binaries:

- `docforge-cli` — headless operations (version, generate, fill, list).
- `docforge-onprem` — air-gapped engine build.

Example:
```
docforge-cli version          # prints engine version
docforge-cli list             # lists available templates (JSON)
```

The desktop app talks to the same Rust core (`docforge-core`) that powers these binaries,
so behavior is identical whether you click or script.

---

## 8. Getting Help

- Check the **Troubleshooting** section above first.
- For enterprise deployments, contact your DocForge administrator or IT distributor.
- Report reproducible bugs with: the template used, the steps taken, and the exact error
  text shown in the red banner.

---

*DocForge — Deterministic, offline-first document automation. No accounts. No cloud. No AI.*
