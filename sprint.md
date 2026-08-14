# DocForge — Sprint Plan v2.0.0 (Tech Lead DAG)

> Owner: Tech Lead | Source: `task_dag.json` (20 tasks, 5 waves: B1/F1/M1/G1/Q1)
> Paradigm: `/clean-code` | Release: **2.0.0** (evolution from 1.0.0, full Phase 1–4 re-approval)
> Rule: a task starts only when every `depends_on` task in an earlier wave is green (verify loop).
> The v1 DAG (TASK-001..032, all verified) remains as shipped project history; this run
> continues the task-id scheme from **TASK-101**.

## Objectives

1. **Stabilize the Bundle model** — the reusable generation definition (identity,
   documents, canonical schema, mappings, rules, output config, versions) as a first-class
   domain entity.
2. **Canonical fields + mappings + groups + validation** — the explicit deterministic
   mapping layer (document `{{placeholder}}` → canonical field) with shared vs
   document-specific groups and three-level validation (field / matter / bundle).
3. **Matter** — create against an exact published Bundle Version, grouped form entry of
   data once, validate, preview, generate.
4. **Generation** — generate all / selected over the Rust `docx_engine`, deterministic
   output naming, append-only run records with input snapshot + engine version, and
   immutable generated documents.
5. **Zero regression** — v1 standalone template flows, the 50-fixture corpus, and the
   clean-VM PDF path stay green throughout.

## Scope (4 phases, from `architecture.md` §20.2)

| Phase | Scope | Modules |
|---|---|---|
| **Phase 1 — Bundle stabilize** | Bundle CRUD + manifest + versioning; `.dfpkg` v2 container; output config | `bundle`, `governance` (schema v5), `export` (dfpkg) |
| **Phase 2 — Fields & mappings** | Canonical schema + 13 field types + groups; explicit mapping layer; placeholder extraction; bundle validation / Health Check | `field_mapping`, `docx_engine` (scan_placeholders) |
| **Phase 3 — Matter** | Matter CRUD, matter data entry, grouped form assembly, three-level validation | `matter` |
| **Phase 4 — Generation** | Rules DSL + conditional docs; generation runs; preview; generated documents | `rules`, `generation_run` |
| **Q1 — Quality/Integration** | Migration regression, v2 UI, contract suite, cargo gate | `template_store`, `gui_shell`, `generation_run` |

## 🛑 MANDATORY APPROVAL GATE

> **Between Phase 1 (Architecture) and Phase 2 (Coding).**
> The author must inspect `architecture.md` (v2, DRAFT — pending gate) and this DAG before
> any code is written. `config.json` sits at `currentPhase: "Architecting"` with
> `approvalGates.architecture: false` until the gate is approved. Coding begins only after
> the gate flips to approved.

## Execution Waves

| Wave | Theme | Tasks (parallel) |
|---|---|---|
| **B1** | Bundle stabilize | ► `TASK-101` schema migration v4→v5 │ ► `TASK-102` bundle manifest + persistence │ ► `TASK-103` versioning (draft→published, immutable) │ ► `TASK-104` .dfpkg v2 import/export │ ► `TASK-105` output config |
| **F1** | Fields & mappings | ► `TASK-106` canonical field schema (13 types) + registry │ ► `TASK-107` field groups (shared vs document-specific) │ ► `TASK-108` mapping layer (set/list/resolve) │ ► `TASK-109` placeholder extraction + unmapped list |
| **M1** | Matter | ► `TASK-110` matter create (exact bundle_version binding) │ ► `TASK-111` matter data CRUD + input_hash │ ► `TASK-112` grouped form assembly │ ► `TASK-113` three-level validation |
| **G1** | Generation | ► `TASK-114` rules DSL (safe deterministic) │ ► `TASK-115` conditional documents + preview │ ► `TASK-116` generation run record (append-only) │ ► `TASK-117` generate all/selected + naming + history |
| **Q1** | Quality/Integration | ► `TASK-118` v1 template flow regression │ ► `TASK-119` v2 UI (Bundles/Matter/Generation) │ ► `TASK-120` integration gate + contract suite + cargo gate |

## Dependency Graph (edges)

```
B1  TASK-101  TASK-102  TASK-105
        │         │
        │         ▼
        ├──► TASK-103 ────┐
        │         │       │
        │         ▼       ▼
        │   TASK-104 ◄────┘
F1      ▼
   TASK-106 ──► TASK-107
        │           │
        ├──► TASK-108 ──► TASK-109
M1      ▼               │
   TASK-110 ──► TASK-111◄┘
        │           │
        │           ├──► TASK-112
        │           │
        ├──► TASK-113 (deps 106, 108, 111)
G1      ▼           │
   TASK-114 ◄───────┘
        │
   TASK-115 ◄── TASK-111   TASK-116 ◄── TASK-103, TASK-111
        │                       │
        └───────────┬───────────┘
                    ▼
              TASK-117 ◄── TASK-104, TASK-105
Q1                  │
              TASK-118 ◄── TASK-101
              TASK-119 ◄── TASK-105, TASK-112, TASK-117
              TASK-120 ◄── TASK-117, TASK-118, TASK-119
```

## Critical Path

```
TASK-101 → TASK-102 → TASK-103 → TASK-110 → TASK-111 → TASK-115 → TASK-117 → TASK-119 → TASK-120
```

## Definition of Done per Wave

- **B1:** v5 schema applied with `foreign_key_check` clean; bundle manifest round-trips
  persistence and `.dfpkg`; a published version rejects edits; output config persists.
  (AC-023, AC-024, AC-025, AC-007, AC-035-partial)
- **F1:** all 13 field types validate; groups persist scopes; four placeholders resolve to
  one canonical field; unmapped placeholders are listed per exact document+field.
  (AC-026, AC-027, AC-028, AC-038-partial)
- **M1:** a matter binds its exact `bundle_version_id`; one data entry feeds all
  documents; the grouped form schema separates shared vs document-specific; validation
  identifies the exact failing field/document. (AC-029, AC-030, AC-031-partial, AC-032)
- **G1:** run records are append-only with input snapshot + engine version; conditional
  documents skip with reasons in preview; generate-all/selected produce correctly named
  DOCX/PDF; rerun never mutates historical outputs. (AC-033, AC-034, AC-035, AC-036,
  AC-037)
- **Q1:** v1 template flows + 50-fixture gate green on the migrated DB; v2 UI navigation
  and screens present; full pytest contract suite + `cargo test --workspace` green;
  AC-040 code review confirms zero docx manipulation under `src/`. (AC-004, AC-010,
  AC-031, AC-035, AC-036, AC-040)

## Test Strategy

- **Rust unit tests** per module (`bundle`, `field_mapping`, `matter`, `rules`,
  `generation_run`) via `cargo test` — the primary verification of domain invariants
  (immutability, mapping determinism, run snapshots, DSL safety).
- **Fixture tests** on the 50-fixture DOCX corpus for placeholder scanning and the v1
  tag-fidelity regression (`src-tauri/tests/fidelity_gate.rs`, `regression_v2.rs`).
- **Integration tests** for `.dfpkg` round-trips, generate-all/selected output naming,
  and the schema v5 migration.
- **Contract tests** in `tests/contract_v2.py` verifying the typed command surface
  (create/publish bundle, matter, generate) against the Rust services.
- **e2e tests** for the v2 UI journeys (Bundles → Matter form → preview → generate) via
  vitest component tests and the SDLC harness.
- **Code review** gates: REQ-039 (professional neutrality) and REQ-040 (no docx
  manipulation in `src/`) enforced like AC-001.
- Every wave re-runs the v1 regression gates; the SDLC `verify_task` loop re-dispatches
  the Feature Developer until green before a task is marked `verified`.
