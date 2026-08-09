# Monetization & Growth Strategy: DocForge

> Company: DocForge, Inc. | Revenue Team | Approved with viability GATE 0

---

## 1. Revenue Model Decision

**Model: Hybrid — Freemium + Subscription (PLG) + Enterprise License + API/CLI License**

Rationale:
- Desktop tool with near-zero marginal cost → freemium is the right acquisition engine.
- Value scales with usage (documents generated, seats) → subscription captures it.
- Regulated/enterprise customers need compliance + on-prem → enterprise license (high
  ACV, low volume) is the ARR anchor.
- Headless core exists → API/CLI licensing creates a developer-led expansion channel.

---

## 2. Pricing Tiers

| | **Free** | **Pro** | **Business** | **Enterprise** |
|---|---|---|---|---|
| **Price** | $0 | $9/mo or $90/yr (−17%) | $15/user/mo (min 5) | Custom ($15k+/yr) |
| **Templates** | 3 | Unlimited | Unlimited | Unlimited |
| **Fields/template** | 5 | Unlimited | Unlimited | Unlimited |
| **Field types** | Text | Text, date, dropdown, checkbox, signature | All + computed | All + computed |
| **Export** | Word (watermarked) | Word + PDF | Word + PDF | Word + PDF |
| **Template versioning** | — | ✓ | ✓ | ✓ |
| **Team library / shared templates** | — | — | ✓ | ✓ |
| **Template governance (draft→approve→publish)** | — | — | ✓ | ✓ |
| **Admin console + RBAC** | — | — | ✓ | ✓ |
| **Audit log (exportable)** | — | — | ✓ | ✓ (immutable) |
| **SSO / SAML** | — | — | Early access | ✓ |
| **On-prem / air-gapped** | — | — | — | ✓ |
| **API / CLI access** | — | — | Add-on | ✓ |
| **Support** | Community | Email | Priority | Dedicated + SLA |
| **Compliance pack (SOC 2, DPA, security review)** | — | — | — | ✓ |

**Pricing logic:** Value-based. A legal team generating 200 docs/month at 15 min/doc
manual effort saves ~50 hrs/month ≈ $2,000+ value → $15/user/mo is <2% of value
captured. Enterprise price = 10–20% of the value of governed, auditable,
data-resident document generation.

---

## 3. Paywall Architecture

```
User Action (create 4th template / fill 6th field / export PDF on Free)
  → Local entitlement check (license cache) + server verify (when online)
    → Entitled: allow
    → Not entitled:
      → Contextual upgrade prompt (specific value unlocked, not generic)
      → One-click upgrade → hosted checkout (Paddle, merchant of record)
      → Webhook: subscription.created → license issued + downloaded
      → Immediate unlock (no restart, no reload)

Offline handling:
  License cache grants up to 30-day grace offline (enterprise: 90-day, floating).
  No network = no feature lockout for existing entitlements (trust-first).
```

**Webhook events:** `subscription.created|updated|cancelled|expired`,
`payment.failed` (dunning), `license.revoked` (admin action), `refund.issued`.

**Conversion levers:**
- Let free users hit the natural wall (4th template) — show value first.
- Social proof at paywall (documents generated count, logos after enterprise pilots).
- Annual pricing prominent (higher LTV).
- Post-purchase celebration + "create your 5th template now" re-engagement.

---

## 4. Payment & Licensing Infrastructure

- **Provider:** Paddle (merchant of record — handles global VAT/Sales tax for desktop
  software; eliminates per-country tax compliance). Stripe as fallback for
  enterprise invoicing via Stripe Invoicing/Billing.
- **Licensing service (small, optional cloud):** license issuance, device
  registration, seat management, offline grace windows, revocation. No document data
  ever passes through it — **zero-knowledge by architecture**.
- **Entitlement model:**
  - Pro: per-user, 2 devices, offline grace 30 days
  - Business: per-seat pool, admin-managed, floating seats optional, grace 60 days
  - Enterprise: offline-issued license files (air-gapped), 90-day grace, no phone-home
    requirement after initial activation
- **Enterprise invoicing:** annual contracts, PO + NET30, DPA + SOC 2 report delivery.

---

## 5. Growth Loops

1. **Product-Led Growth (primary):** Free user → creates template → hits limit →
   upgrades → invites team (Business) → admin adopts governance → company-wide.
   Metric: free→paid conversion 2–5%, time-to-value < 10 min.
2. **Content loop (secondary):** Vertical guides ("Real-estate lease automation
   checklist", "HR offer-letter templates") → SEO → free users → conversions.
3. **API/CLI developer loop (expansion):** Devs embed `docforge` headless in internal
   tools → org license → seats.
4. **Referral:** Referrer gets 1 free Pro month; referee gets 30-day Pro trial.
   Attribution window 30 days; anti-fraud: email verification + minimum activity.

---

## 6. Analytics & North Star

**North Star Metric: Successful document generations per week** (the moment of value;
captures frequency + depth of usage).

**Funnel (AARRR):**

```
Acquisition:  installs, MS Store page views, signup rate, cost/signup
Activation:   % creating first template within 7 days (target ≥ 40%)
              Time-to-first-generation (target < 10 min)
Retention:    D1/D7/D30, weekly active, monthly churn (target < 2% paid)
Revenue:      free→paid conversion (2–5%), MRR, ARPU, LTV
Referral:     K-factor, NPS (target ≥ 40)
```

**Telemetry policy (trust-first):** opt-in, aggregated, local-only by default;
crash reports via Sentry with consent screen. Never captures document contents —
only counts and timing. Enterprise: fully disableable.

---

## 7. A/B Testing Roadmap (First 5)

1. Upgrade prompt placement: post-wall modal vs. persistent sidebar.
2. Free tier limits: 3 templates/5 fields vs. 2 templates/10 fields.
3. Pricing display: monthly vs. annual-first toggle.
4. Activation: guided "import a real contract" onboarding vs. blank start.
5. Enterprise CTA: "Talk to sales" vs. "Request security pack" form.

---

## 8. Revenue Projections (Lean Team, PLG + 3 Enterprise Pilots)

| Horizon | Free users | Pro | Business | Enterprise | MRR |
|---|---|---|---|---|---|
| M1 (beta) | 200 | 10 | 0 | 0 | ~$90 |
| M3 | 1,200 | 90 | 3 (5 seats) | 0 | ~$1,300 |
| M6 | 5,000 | 320 | 15 | 2 pilots | ~$6,900 |
| M12 | 20,000 | 1,200 | 60 | 8 (avg $18k) | ~$34,000 / $408k ARR |

Assumptions: 2–5% free→paid, $9 blended ARPU Pro, Business 5–20 seats, 8 enterprise
deals at $15–25k/yr. Gross margin ~92% → ~$31k MRR gross profit at M12.

---

**Generated:** 2026-08-09 | **Status:** Approved by Revenue Team
