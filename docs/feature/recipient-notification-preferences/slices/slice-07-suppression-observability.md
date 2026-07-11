# Slice 07 — PII-free suppression observability

**Goal**: make opt-out volume visible to operators/compliance without exposing who → Olivia can read suppression
counts by event on `/metrics` and confirm suppression is enforced, with no recipient PII anywhere.
**Story**: US-07.

**IN scope**
- Count each suppressed suppressible-delivery on the shipped observability seam — a `suppressed` outcome added
  to `foundry_notification_deliveries_total{provider,event,outcome}` (`crates/foundry-app/src/notify.rs:39,
  291-297`; `DeliveryOutcome` `:161-176`), OR a sibling `foundry_notification_suppressions_total{event}`
  (DESIGN ODD-5).
- Bounded-label: `event` (∈ the closed catalog) + at most `workspace`; **never** the recipient email or token
  (NFR-4). If widening `DeliveryOutcome`, also update the register-at-0 zero-series and the cardinality guard
  (per the shipped bounded-label discipline that fails closed).
- Exposed on the existing `/metrics` sidecar (`metrics_server.rs:66`) — no new dashboard infra.
- Acceptance / `@property`: N suppressions → suppressed count == N by event; a full `/metrics` scrape + delivery
  logs contain **no** recipient email/token; the suppressed series for every **mandatory** event is always 0
  (US-02, observable); a label/cardinality guard fails closed on a PII/unbounded label.

**OUT of scope**: guardrail **alert thresholds** on a suppression spike (a DEVOPS follow-up); the suppression
**decision** itself (slice 01); anything signed-in (US-05/06).

**Learning hypothesis**: disproves "the shipped delivery-metric seam extends to a PII-free suppression count
(reusing the bounded-label + register-at-0 + fail-closed cardinality discipline) that gives operators opt-out
volume without ever labelling a recipient" if a `suppressed` outcome can't be added without a PII label, if the
cardinality guard can't cover it, or if the mandatory suppressed series isn't provably 0.

**Seams**: delivery metric `foundry_notification_deliveries_total` + `NOTIFICATION_DELIVERIES_METRIC`
(`crates/foundry-app/src/notify.rs:39`, increment `:291-297`); `DeliveryOutcome{Delivered,Failed}` (`:161-176`)
→ possibly `+ Suppressed`; the `/metrics` sidecar (`metrics_server.rs:66`); the register-at-0 + bounded-label
guard convention; the suppression decision from slice 01.
**Dependencies**: slice 01 (US-01) — the suppression it counts. DESIGN ODD-5 (suppressed outcome vs sibling
counter; label set). No new persistence, no new migration.
**Effort**: ~0.5 day (a counter increment on the shipped seam + a no-PII litmus).
</content>
