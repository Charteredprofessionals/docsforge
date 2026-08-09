# ADR-002: Cross-run XML Replacement Strategy

## ADR-002: Cross-run XML replacement strategy with quick-xml

**Context:** In DOCX, a single visual word often spans multiple `<w:t>` runs
(`<w:r>` elements) — Word splits text at formatting changes, spell-check boundaries,
or save round-trips. The current backend (`replace_in_text`, `commands.rs:257`) replaces
only within one run's text buffer and, on tag, can corrupt selections that span runs
(REQ-002, AC-002). The constraint forbids regex-based mutation of `document.xml`.
Replacement must also preserve formatting: the tagged placeholder should inherit the
selection's first-run `<w:rPr>`.

**Decision:** Implement a streaming, run-aware transform in `docx_engine` using
`quick-xml` (already adopted). For tagging: walk sibling `<w:r>` elements under a
paragraph, concatenate their `<w:t>` text into a logical text buffer while tracking
source run boundaries; when a user selection maps into that buffer, emit a single
placeholder `{{tag}}` inside the first run of the selection, copy that run's `<w:rPr>`
onto it, and discard the remaining fragments of the covered runs (with empty runs
dropped). For filling: match `{{tag}}` patterns across run boundaries by the same
logical-buffer approach so a placeholder split by Word into multiple runs resolves to
one value; keep adjacent formatting of the surrounding runs for the inserted text.
Matching is deterministic and driven by the field schema, never by regex over raw XML.

**Alternatives:**
1. Merge all runs in a paragraph into one run before replacing — rejected: loses
   per-run formatting and can reflow documents (REQ-102 fidelity corpus would fail).
2. Regex substitution on the serialized XML — rejected: explicit constraint; cannot
   handle namespace variations, entities, or run boundaries safely.
3. Two-pass naive replace (current behavior) — rejected: corrupts multi-run selections;
   the reported defect.

**Consequences:**
- Positive: 100% tag fidelity target becomes testable on the 50-fixture corpus
  (tables, headers/footers, multi-run, RTL, tracked changes — REQ-102); formatting
  preserved per AC-002; streaming keeps constant memory for 10MB files (REQ-101).
- Negative: run-merging logic is intricate and must handle edge cases (RTL, field
  codes, `w:ins`/`w:del` tracked-change runs); requires an extensive fixture suite
  and property tests for unclosed-tag detection (REQ-003).
- Consequence: `fill_document` returns `DocForgeError::UnclosedTag { tag_name, offset }`
  whenever `{{` remains unclosed after the pass — never a silently corrupted docx.
