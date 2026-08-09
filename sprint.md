# DocForge — Sprint Plan (Tech Lead DAG)

> Owner: Tech Lead | Source: `task_dag.json` (30 tasks, 11 waves) | Paradigm: `/clean-code`
> Rule: a task starts only when every `depends_on` task in an earlier wave is green (verify loop).

```
Legend:  ► = task in a wave        │ = depends on task(s) directly above
         ⇢ = critical path step    (critical path: TASK-001 ⇢ 002 ⇢ 003 ⇢ 004 ⇢ 014 ⇢ 024 ⇢ 025 ⇢ 026 ⇢ 027 ⇢ 028 ⇢ 030)
```

## Execution Waves

| Wave | Theme | Tasks (parallel) |
|---|---|---|
| **W1** | Foundation | ► `TASK-001` ⇢ docforge-core + DocForgeError contract |
| **W2** | Data + validation | ► `TASK-002` ⇢ validate_docx (bomb guards) │ ► `TASK-006` ⇢ Data Model v2 schema+migrations │ ► `TASK-008` ⇢ DPAPI at-rest crypto |
| **W3** | Core build-out | ► `TASK-003` ⇢ tag_document cross-run │ ► `TASK-007` FS template_store │ ► `TASK-019` governance core │ ► `TASK-020` licensing core |
| **W4** | Critical-path fixes | ► `TASK-004` ⇢ fill_document (unclosed-tag) │ ► `TASK-009` BLOB→FS migration │ ► `TASK-010` binary IPC │ ► `TASK-012` export module (docx/html/dfpkg) │ ► `TASK-016` versioning+rollback |
| **W5** | Engine + security | ► `TASK-005` 50-fixture gate │ ► `TASK-011` PDF engine (no LO) │ ► `TASK-013` CSP+ACL+iframe │ ► `TASK-014` ⇢ services+commands+thread pool │ ► `TASK-015` field types |
| **W6** | GUI + enforcement | ► `TASK-017` Template Creator │ ► `TASK-018` Template Filler │ ► `TASK-021` audit export+admin cmds │ ► `TASK-022` consent+telemetry │ ► `TASK-024` ⇢ CLI shell |
| **W7** | Admin + headless | ► `TASK-023` Admin console+licensing UI │ ► `TASK-025` ⇢ REST bridge+webhooks │ ► `TASK-029` compliance pack |
| **W8** | Enterprise auth | ► `TASK-026` ⇢ SSO/SAML AuthService |
| **W9** | Enterprise deploy | ► `TASK-027` ⇢ policy overlay + on-prem build |
| **W10** | Distribution | ► `TASK-028` ⇢ signed auto-update + SBOM |
| **W11** | Release gate | ► `TASK-030` ⇢ corpus/perf/clean-VM/CI verification |

## Dependency Graph (edges)

```
W1  TASK-001
    │
W2  ├──► 002      ├──► 006      ├──► 008
    │             │             │
W3  │   ┌─────────┴───────┐     │   ┌──────────┐
    │   ▼               ▼     │   ▼          ▼
    ├──► 003     ├──► 007    │   ├──► 019   ├──► 020
    │            │           │   │          │
W4  │   ┌────────┴─┐    ┌────┴───┴───┐   ┌───┴───┐
    ▼   ▼          ▼    ▼           ▼   ▼       ▼
    ├──► 004   ├──► 009  ├──► 010  ├──► 012  ├──► 016
    │           │        │         │
W5  │   ┌───────┴───┐    │   ┌─────┴───┐
    ▼   ▼           ▼    ▼   ▼         ▼
    ├──► 005   ├──► 011  ├──► 013   ├──► 014   ├──► 015
    │           │         │          │         │
W6  │   ┌───────┴─────────┴─────┐    │   ┌─────┴──────┐
    ▼   ▼                       ▼    ▼   ▼            ▼
    ├──► 017  ├──► 018      ├──► 021 ├──► 022    ├──► 024
    │          │             │        │           │
W7  │   ┌──────┴──────┐      │        │   ┌───────┴──────┐
    ▼   ▼             ▼      │        ▼   ▼              ▼
    ├──► 023    ├──► 025     │   ├──► 029  ├──► 026
    │            │           │              │
W8  │   ┌────────┴───────────┴──────────────┤
    ▼   ▼                                   ▼
    ├──► 027                                ├──► (026)
W9  │   │
    ▼   ▼
    ├──► 028
W10 │   │
    ▼   ▼
    ├──► 030
W11 ▼
```

## Critical Path

```
TASK-001 → TASK-002 → TASK-003 → TASK-004 → TASK-014 → TASK-024
 → TASK-025 → TASK-026 → TASK-027 → TASK-028 → TASK-030
```
11 steps, W1→W11. Everything not on this path is slack: it can be parallelized or slipped
without moving the release date. The highest-leverage parallel track is
`TASK-001 → TASK-006 → TASK-007 → TASK-010 → TASK-013` (storage + binary IPC + CSP),
which merges back at `TASK-014`.

## Roadmap Alignment

- **Phase B hardening (W1–W5):** unified engine (001–004), no-BLOB storage (006,007,009),
  binary IPC (010), PDF without LibreOffice (011), CSP+sanitization (013), corpus gate (005).
- **GUI / field types / versioning (W5–W6):** 015, 016, 017, 018.
- **Governance / licensing / CLI (W3, W6–W7):** 019, 020, 021, 022, 023, 024, 025.
- **Enterprise: SSO / on-prem / connectors (W7–W10):** 026, 027, 028, 029 (connector storage
  stays behind the `template_store` port; post-GA per product plan).
- **Release gate (W11):** 030 proves the release definition from `constraints.json`.
