# DocForge — User Manual

**DocForge** is a 100% native desktop document-automation application for Windows. It lets you
turn a Word (`.docx`) document into a reusable **template** with fillable fields, then generate
completed documents on demand — as Word (`.docx`) or PDF — without ever sending your data to the
cloud. Everything runs locally on your computer.

> **Privacy & Security Note:** DocForge is fully offline. Documents, templates, and field values are
> encrypted at rest using Windows DPAPI and stored only on your computer. No internet connection or
> cloud account is required.

---

### DocForge in One Minute

Imagine you send the same kind of letter every week — an offer letter, a contract, an invoice — but
the names, dates, and amounts change each time. Instead of retyping the document:

1. **Make a template once** from your Word file (select changing text and configure fillable fields).
2. **Click the template** and type or select just those changing values.
3. **Export** a finished, formatted Word or PDF document in seconds with toast confirmation.

That is DocForge. You do it all on your own computer; nothing ever leaves your machine.

---

### Everyday Workflow at a Glance

- **Create** a template from any `.docx` file → **Search & Select** it whenever needed →
  **Fill** values in a compact layout → **Export** a filled Word or PDF. Repeat forever; create the
  template only once.

---

## 1. Getting Started

### System Requirements
- Windows 10 or Windows 11 (64-bit)
- Microsoft WebView2 Runtime (included in Windows 10/11)
- About 300 MB of free disk space
- A `.docx` (Word) file to use as your first template

### Installation
1. Download the native installer from your IT distributor or release package:
   - `DocForge_2.0.0_x64_en-US.msi` (recommended — Windows Installer for enterprise/user install)
   - `DocForge_2.0.0_x64-setup.exe` (NSIS installer — single-user desktop setup)
   - `DocForge.msix` (Microsoft Store / enterprise sideload package)
2. Run the installer and follow the prompts.
3. Launch **DocForge** directly from your Start Menu.

> **SmartScreen Note:** If Windows SmartScreen appears during initial setup, select **More info →
> Run anyway** (development builds are self-signed; enterprise deployments use your organization's
> code-signing certificate).

### Navigation & Screens

DocForge features four primary views accessible from the top navigation bar:

| View | Purpose |
|------|---------|
| **Templates** | Search, browse, open, and manage your saved template library |
| **New Template** | Upload a Word document and visually map fillable fields |
| **Fill** | Enter field values using type-specific controls and export documents |
| **Admin** | Manage organization seats, offline license files (`.dflic`), and audit logs |
| **Shield Icon** | Configure privacy consent and anonymous diagnostic preferences |

---

## 2. Creating a Template

A *template* is a Word document with one or more **fillable fields**. Each field marks a spot of text
that gets replaced with a value when you generate a document.

1. Click **New Template** (or **Create First Template** on an empty library).
2. On the **Upload** screen, select a `.docx` file from your computer.
   - DocForge validates the document's structure and renders an interactive preview.
3. The **Create Template** workspace opens:
   - Edit the template name in the header if desired.
   - **Select text in the preview** with your mouse cursor.
   - A custom **Field Modal** appears automatically:
     - **Field Label:** Enter a descriptive title (e.g., *"Candidate Name"*). DocForge previews the resulting placeholder tag (`{{candidate_name}}`).
     - **Field Type:** Choose between **Text**, **Date**, **Dropdown** (with custom options), **Checkbox**, or **Signature**.
     - **Required:** Check whether this field must be filled prior to export.
     - Click **Add Field**.
   - The selected text is highlighted in amber, and a card appears in the right sidebar.
   - To remove a field, hover over its card in the sidebar and click the **X** button.
4. Click **Save Template**. Your template is encrypted with Windows DPAPI and stored in your local library.

---

### Worked Example — An Offer Letter

1. Upload `OfferLetter.docx` containing: *"Dear **Jane Doe**, your start date is **January 5, 2026**."*
2. Highlight `Jane Doe` → Label: `Candidate Name`, Type: `Text`, Required: `Yes`.
3. Highlight `January 5, 2026` → Label: `Start Date`, Type: `Date`, Required: `Yes`.
4. Click **Save Template** as *"Standard Offer Letter"*.
5. Future use: Select *"Standard Offer Letter"*, pick the candidate's name and date from the date picker, and click **Export Word**.

---

## 3. Using a Template (Generating Documents)

1. On the **Templates** screen, click any template card or its **Use Template** button.
2. The **Fill** screen displays:
   - **Compact Multi-Column Layout:** Templates with more than 4 fields automatically arrange into a 2-column grid.
   - **Type-Specific Controls:** Date pickers for Date fields, dropdown menus for Dropdown fields, and toggles for Checkboxes.
   - **Required Field Indicators:** Required fields are marked with a red asterisk (`*`).
3. Enter or select your desired field values.
4. Choose an action from the top right toolbar:

| Action | Description |
|--------|-------------|
| **Preview Document** | Fills the values and opens a read-only document preview. Click **Edit Values** to return. |
| **Export Word** | Generates a byte-identical `.docx` file named `<TemplateName>_filled.docx` and displays a success toast. |
| **Export PDF** | Converts the document to a PDF and downloads `<TemplateName>_filled.pdf`. |

> **PDF Export & Timeout Guard:** PDF export uses an installed LibreOffice engine with a 120-second timeout guard. If LibreOffice is not installed, DocForge displays a clear notification banner while Word export remains fully operational.

---

## 4. Managing Templates & Privacy

### Searching & Deleting
- **Search Bar:** Type into the search input at the top of the **Templates** screen to filter your library by name in real time.
- **Deleting a Template:** Hover over a template card, click the trash icon (`Trash2`), and confirm inside the custom **Delete Confirmation Modal**.

### Admin Console & Offline Licensing
- Click **Admin** in the navigation header to view organization seats, export immutable audit logs (`generation_log`), or activate offline enterprise license files (`.dflic`).

### Privacy Preferences
- Click the **Shield** icon in the header to view or update anonymous diagnostic and crash reporting settings.

---

## 5. Command-Line Interface (CLI Automation)

For server scripting and automated batch processing, DocForge includes a native headless CLI executable (`docforge-cli.exe`):

```cmd
:: Display CLI version and engine metadata
docforge-cli.exe version

:: List all templates stored in the local SQLite database (JSON output)
docforge-cli.exe list

:: Fill a template headlessly and save to an output file
docforge-cli.exe fill --template-id "tpl_12345" --set candidate_name="Jane Doe" --set start_date="2026-09-01" --out "C:\Output\OfferLetter_Jane.docx"
```

The CLI connects directly to the same local Rust engine (`docforge-core`) and SQLite database used by the desktop app.

---

## 6. Troubleshooting & FAQ

| Question / Issue | Resolution |
|------------------|------------|
| **"LibreOffice not found" on PDF export** | Install [LibreOffice](https://www.libreoffice.org/) on your machine. Word export (`.docx`) works without LibreOffice. |
| **Is my document sent to any server?** | **No.** DocForge is 100% offline. All document generation happens on your local CPU. |
| **Where are my templates stored?** | Stored locally in `%APPDATA%\docforge\` in an encrypted SQLite database and filesystem structure. |
| **"PDF conversion timed out"** | Occurs if LibreOffice hangs. DocForge automatically terminates the process after 120 seconds to protect system resources. |

---

*DocForge — Deterministic, offline-first document automation. No accounts. No cloud. No AI.*
