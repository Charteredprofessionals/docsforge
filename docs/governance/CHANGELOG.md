# DocForge — Change Request Register (Post-Release)

> Governed by `docs/governance/POST_RELEASE_CHANGE_POLICY.md`.
> Every post-release change MUST have a CR here before/with the work.

## Register

| CR-ID | Date | Tier | Title | Source | Status | Artifacts |
|---|---|---|---|---|---|---|
| CR-2026-001 | 2026-08-10 | normal | Bug Book module (Admin Console crash/error log) | user directive | completed | TASK-031, architecture.md §Post-Release, 10 passing tests |
| CR-2026-002 | 2026-08-10 | standard | `save_template` camelCase contract fix | runtime error report | completed | TASK-032, regression test |

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
