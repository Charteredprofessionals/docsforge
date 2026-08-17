# DocForge — Change Request Register (Post-Release)

> Governed by `docs/governance/POST_RELEASE_CHANGE_POLICY.md`.
> Every post-release change MUST have a CR here before/with the work.
> **Machine-readable register:** `config.json` → `changeGovernance.crRegister` points to
> `docs/governance/cr_register.json` (JSON). Keep this markdown table in sync with that file.

## Register

| CR-ID | Date | Tier | Title | Source | Status | Artifacts |
|---|---|---|---|---|---|---|
| CR-2026-001 | 2026-08-10 | normal | Bug Book module (Admin Console crash/error log) | user directive | completed | TASK-031, architecture.md §Post-Release, 10 passing tests |
| CR-2026-002 | 2026-08-10 | standard | `save_template` camelCase contract fix | runtime error report | completed | TASK-032, regression test |
| CR-2026-003 | 2026-08-17 | normal | Mail-merge workflow realignment (CSV per template/bundle + sample template) | user directive | completed | commands.rs, lib.rs, ipc.ts, AdminConsole.tsx, TemplateFiller.tsx, Bundles.tsx, TemplateList.tsx |

---

## CR-2026-001 — Bug Book module

- **Requester:** Product owner
- **Date:** 2026-08-10
- **Source:** User directive — "Add a dedicated 'Bug Book' module to the admin console that automatically captures, categorizes, and permanently records all application crashes and runtime errors…"
- **Tier:** normal (new feature, multi-module, new command surface + schema migration v4)
- **Rationale:** Enterprises require auditable, permanent capture of crashes/errors with severity, context, stack traces, attachments, and reporting/notification.
- **Affected modules:** `core/bug_book` (new), `services/webhook`, `commands` (8 new commands), `core/mod`, `migrations` (v4), frontend `BugBook.tsx`, `AdminConsole.tsx`, `errorCapture.ts`, `ipc.ts`, `types.ts`.
- **Risk:** medium | **Effort:** large
- **Acceptance criteria:**
  - Auto-capture of JS `window.onerror` / `unhandledrejection` + Rust panic hook.
  - Each entry: timestamp, error type, severity (critical/high/medium/low), status, context, stack trace.
  - Manual entry + attachment (log/screenshot) support.
  - Filter/sort/search by date range, severity, status, keyword.
  - Export filtered list to CSV and PDF.
  - Critical-bug webhook dispatch (`bug.critical`) to `webhook_subscriptions`.
- **Test evidence:** `core::bug_book::tests` (create/get, severity reject, filter, sort, status resolve, attachment, csv, pdf) + `services::webhook::tests` — all passing.
- **Approver:** Tech Lead | **Target version:** 1.0.1
- **Linked ADR:** ADR-BugBook (new module)

## CR-2026-002 — `save_template` camelCase contract fix

- **Requester:** QA
- **Date:** 2026-08-10
- **Source:** Runtime error — `Failed to save template: invalid args 'request' for command 'save_template': missing field 'original_text'`
- **Tier:** standard (single-field serialization contract; no architecture change)
- **Rationale:** Frontend sends `TemplateField` with camelCase keys (`originalText`, `tagName`); `TemplateFieldSpec` lacked `#[serde(rename_all = "camelCase")]`, so the request was rejected. Fix also makes stored `fields_json` consistent with the frontend type.
- **Affected modules:** `core/docx_engine.rs` (`TemplateFieldSpec`), frontend `types.ts`.
- **Risk:** low | **Effort:** small
- **Acceptance criteria:** `save_template` accepts camelCase fields; round-trip (list/get) returns camelCase; regression test proves deserialization.
- **Test evidence:** `test_field_spec_deserializes_camel_case` (in `docx_engine.rs`) — passing.
- **Approver:** Tech Lead | **Target version:** 1.0.1

## CR-2026-003 — Mail-merge workflow realignment

- **Requester:** Product owner
- **Date:** 2026-08-17
- **Source:** User directive — "the CSV export/batch features were placed in the Admin Console which has no template context; move them to the template, add bundle-level CSV batch, and seed a sample template for first-time users."
- **Tier:** normal (multi-module UI change + one new command; no schema change)
- **Rationale:** Per-template mail-merge operations (`export_template_fields_csv`, `batch_fill_from_csv`) had been wired into `AdminConsole`, which is org-wide config and carries no template selection — the buttons were hardcoded to a `"placeholder"` template id and left `disabled`. The correct mental model is: a template (or a bundle of templates) IS the context. The user flow must be: create/save template → export its fields as CSV → fill CSV → upload → batch-generate; for bundles, create the bundle → per-template CSV upload (or manual fill) → generate all docs.
- **Affected modules:**
  - `commands.rs` — new `seed_sample_template` command (idempotent; builds a minimal valid DOCX, tags its marker words into `{{...}}` placeholders, persists via `template_store::save_template`).
  - `lib.rs` — registers `seed_sample_template`.
  - `src/lib/ipc.ts` — adds `seedSampleTemplate`, keeps `exportTemplateFieldsCsv` / `batchFillFromCsv`.
  - `src/components/AdminConsole.tsx` — **removed** the misplaced CSV export/batch UI (Admin Console now contains only users, licenses, audit, policy, data backup/restore, bug book).
  - `src/components/TemplateFiller.tsx` — **added** "Export Fields CSV" + "Batch Generate from CSV" mail-merge panel bound to the real `templateId` (each CSV row → one generated `.docx`).
  - `src/components/Bundles.tsx` — **added** bundle detail view: per-template "Export Fields CSV" + "Upload CSV" + "Fill manually", shared output folder, formats, and "Generate All Docs from Bundle".
  - `src/components/TemplateList.tsx` — **added** "Load Sample Template" CTA (empty state + header) calling `seedSampleTemplate`.
  - `src/App.tsx` — passes `onUseTemplate` into `Bundles` so "Fill manually" navigates to the filler.
- **Risk:** medium (UI restructure) | **Effort:** medium
- **Acceptance criteria:**
  - No template/batch feature remains in `AdminConsole`.
  - `TemplateFiller` exports the selected template's fields as CSV and batch-generates from an uploaded CSV into a chosen folder.
  - `Bundles` detail lets the user export each member template's CSV, upload filled CSVs, and generate every document from the bundle.
  - First run with no templates offers a seeded sample template.
  - `cargo build` and `npm run build` pass clean.
- **Test evidence:** `cargo build` (lib + bin) — 0 errors, 0 warnings; `npm run build` — clean. (Note: the v2 `cargo test` suite has pre-existing breakage in `fidelity_gate`/`regression_v2` test targets — arity + helper `write_all`/`GroupScope` issues — tracked under the still-pending TASK-120 integration gate; not introduced by this CR.)
- **Approver:** Tech Lead | **Target version:** 2.0.1
