# ADR-005: Suppression observability contract (ODD-5)

## Status
Accepted — 2026-07-11 (Morgan, DESIGN wave). Feature-local.

## Context
FR-8/NFR-4/R4 require a suppressed delivery to be counted, bounded-label, PII-free, on the shipped `/metrics`
sidecar — labels may carry `event` (+ at most `workspace`), **never** the recipient email or token. The
shipped delivery counter is `foundry_notification_deliveries_total{provider,event,outcome}` with a **binary**
`DeliveryOutcome{Delivered,Failed}` (`notify.rs:160-177`), a register-at-0 cross-product `active × events ×
outcomes` (`delivery_zero_series`, `:837-851`), and a fail-closed cardinality guard. US-07 additionally wants
the mandatory events' suppressed count to be observably 0.

## Decision
A **sibling counter** `foundry_notification_suppressions_total{event}`.
- **Label = `event` only** (∈ `NotificationEvent::ALL`, bounded snake_case). **No `workspace`** label, **no
  PII.**
- **Register-at-0** over the full `NotificationEvent::ALL` catalog, so mandatory events show a permanent
  `…{event="password_reset"} 0` — the never-suppressed invariant is observable (US-07). The increment fires
  only on the suppression early-return (ADR-003), so mandatory series stay 0 structurally.
- **`DeliveryOutcome` stays binary `{Delivered,Failed}`** — untouched.
- Mirrors the shipped bounded-label discipline + a fail-closed cardinality unit test asserting the label key
  set is exactly `{event}`.

## Alternatives Considered
- **Widen `DeliveryOutcome` with a `Suppressed` variant** — rejected: a suppression is provider-**independent**
  (it is never handed to any provider), so it has no `provider` dimension; putting it on the
  `{provider,event,outcome}` counter forces a bogus `provider` value or N suppression counts per single
  suppression (one per active provider). It would also perturb the shipped `delivery_zero_series` cross-product
  and cardinality guard, breaking NFR-7 exactness. Semantically a category error.
- **Add a `workspace` label** (NFR-4 permits "at most workspace") — rejected: `workspace_id` is unbounded
  cardinality (one series per workspace) and semi-identifying at low tenant counts; it violates the bounded-
  label discipline (the shipped ADR-011 cardinality rule). Aggregate `event`-only volume meets Olivia's need.
- **Log-line-only (no metric)** — rejected: FR-8 wants a scrapeable count; a log requires aggregation
  tooling and risks PII in the line.

## Consequences
- **Positive**: the shipped delivery counter is untouched (NFR-7 exact); PII-free + bounded by construction
  (only closed-enum `event` values reach the label); the always-0 mandatory series is a live, scrapeable
  proof of NFR-3 (US-07); no new dashboard/exporter infra.
- **Negative / accepted**: operators cannot slice suppression volume by workspace on the metric (deliberate —
  it would be unbounded/semi-identifying); per-workspace analysis, if ever needed, is a DEVOPS follow-up over
  a different (aggregated) surface.
