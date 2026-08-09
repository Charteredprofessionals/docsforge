# ADR-007: Licensing / Zero-Knowledge Architecture

## ADR-007: Zero-knowledge licensing and telemetry architecture

**Context:** DocForge's differentiator is "document automation that never sees your
data" (viability §4). Licensing must support offline activation with device caps
(REQ-015, AC-015), grace windows, enterprise offline-issued files, and admin
revocation (REQ-014). Telemetry must be opt-in, aggregate-only, and fully disableable
in enterprise builds (REQ-020, AC-020). REQ-019/AC-019 require zero-knowledge:
document contents and field values must never transit the licensing or telemetry
cloud surface. SOC 2 Type II scope (REQ-105) covers licensing/billing/telemetry
services — a small attack surface is a scoping advantage.

**Decision:**
1. **Zero-knowledge by contract:** the licensing payload is an activation fact —
   `{ product_id, tier, seat/device ids, issued_at, expiry, grace_days, signature }`.
   No document bytes, field values, filenames, or template hashes are ever sent. The
   license server cannot distinguish one document operation from another; it sees only
   activation/seat lifecycle events (webhook events from monetization-strategy §3).
2. **Offline-first entitlement:** `licensing` evaluates entitlement locally against a
   signed license (embedded public key). Online checks are opportunistic (registration,
   seat sync, revocation poll) and never gate an already-granted entitlement during the
   grace window (30/60/90 days per tier). Enterprise license files activate fully
   offline with no phone-home required after activation (REQ-015, AC-015).
3. **Telemetry:** consent-gated (REQ-020); events are counts/timing only
   (`generation.completed {duration_ms, format}`); a redaction pipeline drops any
   payload containing document-derived data before egress; enterprise builds compile
   telemetry and crash upload out and honor policy-file disable. Crash reports
   (Sentry) carry no document content and are also consent-gated.
4. **Key management:** signing keys held offline in the SDLC exporter; device/seat
   bindings stored in the local `devices`/`licenses` tables; at-rest template files
   remain DPAPI-encrypted and unrelated to licensing data.

**Alternatives:**
1. Full online licensing (validate every operation server-side) — rejected: violates
   offline-first constraint and the privacy promise; also enlarges SOC 2 scope.
2. License server sees document metadata (names/counts) — rejected: leaks usage
   patterns; contradicts zero-knowledge marketing and GDPR local-first (REQ-105).
3. No cloud at all — rejected: revocation, seat management, and billing (Paddle
   webhooks) need a small control plane; zero-knowledge makes it acceptable.

**Consequences:**
- Positive: REQ-019/020/AC-015/AC-019/AC-020 verifiable; GDPR surface minimized
  (minimal controller scope, REQ-105); SOC 2 scope stays small; strong enterprise
  story ("air-gapped after activation").
- Negative: offline revocation is eventual (revoked users keep entitlement until grace
  expiry/next online check) — mitigated by license-file expiry and admin notifications;
  the zero-knowledge contract must be enforced in code review (AC-019) and enforced by
  an egress allowlist in the telemetry service.
