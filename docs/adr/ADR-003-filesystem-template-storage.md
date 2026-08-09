# ADR-003: Filesystem-Backed Template Storage

## ADR-003: Filesystem-backed template storage + SQLite index (no BLOBs)

**Context:** The legacy schema stores templates as BLOBs (`original_docx`,
`template_docx`) in SQLite (`schema.rs`), and the current API round-trips them as
Base64 (`commands.rs`). Consequences observed: DB bloat, slow list queries, Base64
memory overhead on large files, and no natural path for versioned snapshots or
DPAPI-encrypted files. REQ-004 mandates paths + metadata in the DB and documents on
the filesystem; REQ-010 requires version history with rollback; REQ-019 requires
at-rest encryption.

**Decision:** Templates are stored as files under the app-data directory tree
`<data>/docforge/templates/<template_id>/v<version>/template.docx` (plus per-version
`fields.json`). SQLite (`template_store` index tables) stores only `storage_path`,
metadata, field schema JSON, status, and audit facts. The `template_versions` table
keeps one immutable snapshot row per version — rollback resolves a prior `storage_path`
and creates a new version (REQ-010, AC-010). On Windows the template files are wrapped
in DPAPI encryption at rest (REQ-019, AC-019); macOS uses Keychain-based keys. A
one-time migration copies existing BLOB rows to disk and drops the BLOB columns.

**Alternatives:**
1. Keep BLOBs, add `original_docx`/`template_docx` to versioned tables — rejected:
   DB growth, slow backups, no filesystem-level encryption, and no streaming.
2. Store everything in one encrypted container file (SQLCipher/single archive) —
   rejected: couples storage to one vendor feature; DPAPI file encryption is simpler
   and matches platform expectations; also complicates the shared-library story.
3. Object store / network DB — rejected: offline-first constraint; no network DB.

**Consequences:**
- Positive: metadata-only DB stays small and fast; files are independently
  encrytable/backable; `.dfpkg` export is a direct file copy; AC-004 round-trip test
  becomes a filesystem test; streaming save for >10MB files (REQ-005 path).
- Negative: filesystem integrity becomes a concern — mitigated by `storage_missing`
  structured errors (§8) and the audit log; concurrent access requires care on shared
  libraries (kept behind the `template_store` port for later network store).
