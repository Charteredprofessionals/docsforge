# ADR-008: Deterministic No-AI Positioning Constraint

## ADR-008: Deterministic no-AI positioning constraint

**Context:** Product positioning (company plan §1, viability §4) is "Deterministic
document automation that never sees your data." Requirements list AI-assisted
generation as **out of scope for v1.0** and the constraints require deterministic
generation only — no LLM in the document generation path. The risk register flags AI
commoditization as the top threat; the mitigations are determinism, offline privacy,
and governed templates. This ADR records the architectural consequence: the decision
must be enforced in the build, not just the roadmap.

**Decision:** The document generation path is closed to non-deterministic
components **by construction**. Concretely:
1. `docforge-core`'s `docx_engine` and `export` depend only on deterministic
   primitives: quick-xml (streaming XML), the zip crate (OPC I/O), string/field-schema
   substitution, and the print bridge (HTML→PDF). No inference/LLM SDK, no remote model
   calls, no "smart fill" heuristics that vary output between runs.
2. Identical input (template bytes + canonical field values) ⇒ byte-identical output
   for a given engine version — this is a regression-testable invariant and the
   product's core promise. `fields_hash` in `generation_log` supports reproducibility
   verification (REQ-013).
3. Any future AI-assisted feature (explicitly out of v1.0 scope) must be a separate,
   additive, clearly-labeled module with its own opt-in consent and its own licensing
   tier — it may never touch the deterministic fill path or weaken its guarantees.
4. The SDLC CI gate asserts this: dependency allowlist (no `llm`, `openai`, `genai`
   crates/packages in the generation path) plus byte-for-byte repeatability tests on
   the 50-fixture corpus.

**Alternatives:**
1. AI-assisted drafting as a first-class v1 feature — rejected: contradicts
   positioning, trust narrative, and offline-first privacy; hallucination risk is the
   explicit competitive differentiator against Copilot/Gemini (viability §4).
2. "AI by default with deterministic fallback" — rejected: breaks the determinism
   promise (users cannot trust output stability) and muddies telemetry scope.
3. No formal constraint, rely on roadmap discipline — rejected: without a build-level
   gate, a dependency slip silently ships AI into the core path.

**Consequences:**
- Positive: hard, verifiable differentiator; byte-identical reproducibility; CI gate
  makes the constraint self-enforcing; smaller dependency surface and attack surface.
- Negative: AI-assisted features are deferred and must be architected as clean,
  additive modules later; competitors with AI drafting need a different answer —
  addressed via governed templates, compliance, and determinism (the moat, per
  viability §8).
