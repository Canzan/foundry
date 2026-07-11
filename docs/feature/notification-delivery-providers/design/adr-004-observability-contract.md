# ADR-004: Per-provider delivery observability contract

## Status
Accepted (DESIGN, Propose mode). Resolves **ODD-5** (Risk R6).

## Context
Per-provider delivery success/failure must be observable (NFR-4) so Ops Olivia can see, per channel, what
delivered and what failed. The repo has a shipped `metrics` facade + `metrics-exporter-prometheus` sidecar
exposing `/metrics` (`metrics_server.rs:45-77`), a labelled-counter emission precedent
(`foundry_token_mutations_total{principal,outcome}`, `rate_limit.rs:98,198-203`), a register-at-0 convention
(`main.rs:355-369`), and — critically — an **ADR-011 bounded-label discipline** enforced by a fail-closed
cardinality unit test (`metrics_server.rs:99-108,374-428`). The delivery metric must reuse all of this, not
add new observability infra (D7).

## Decision
**Metric**: `foundry_notification_deliveries_total` — a monotonic counter.
**Labels (the bounded triple, ADR-011)**:
- `provider` ∈ `{log, smtp, webhook, email_api}` (the `ProviderKind::as_str()` domain).
- `event` ∈ the notification catalog `{password_reset, workspace_invite, member_invite, member_removed,
  password_changed}` (the `NotificationEvent::as_str()` domain, closed enum — ADR-005).
- `outcome` ∈ `{delivered, failed}` (binary — ADR-003).

**Emission** (inside `Notifier::notify`, once per provider per notification — mirrors `rate_limit.rs:198-203`):
```rust
metrics::counter!(
    NOTIFICATION_DELIVERIES_METRIC,
    "provider" => provider_kind.as_str(),
    "event"    => notification.event.as_str(),
    "outcome"  => outcome,      // "delivered" | "failed"
).increment(1);
```
`pub const NOTIFICATION_DELIVERIES_METRIC: &str = "foundry_notification_deliveries_total";` exported from
`notify.rs`.

**Register-at-0** (at startup in `main.rs`, mirroring `main.rs:355-369`): `describe_counter!` the family, then
register a **zero series for the bounded cross-product of ACTIVE providers × the full catalog × both
outcomes** via `.absolute(0)`. Only ACTIVE providers can emit, so only they are registered (an inactive
provider mints no series). This makes every deliverable series present on the first `/metrics` scrape (no
"no-data" panel), exactly like the token-mutations sentinel and the slice-8 probe set.

**Structured log line** (also inside `notify`, one per provider per notification, complementary to the metric):
`notify provider=<kind> event=<event> to=<recipient> outcome=<delivered|failed> [class=<transient|permanent>]`
— keyed on `provider`/`event`/`recipient`/`outcome`(/`class`) ONLY; **never** the payload, a token, or a
secret (NFR-2, ADR-006). This is the slice-01 observable (before the metric lands in slice 03).

**Cardinality enforcement** (mirrors `metrics_server.rs:374-428`): a scoped-recorder unit test emits the
delivery counter through the SAME `metrics::counter!` macro + label keys the production code uses, renders the
scrape, and asserts the label KEY set is **exactly `{provider, event, outcome}`** — failing closed if a future
contributor adds any label (e.g. `recipient`, `workspace_id`). Paired with a bounded-**value** @property test
(every emitted label value is in its enum's domain) so an unbounded value cannot slip in either.

## Alternatives Considered
- **Add a `recipient` (or `workspace_id`) label for per-user delivery attribution** — REJECTED. `recipient`
  is unbounded (per email address) — a cardinality blow-up (R6) and a PII-in-a-label leak (NFR-2). Recipient
  lives in the log line (bounded by log retention, not a time-series axis), never in a label.
- **A third `outcome="retryable"` / four-way outcome** — REJECTED (see ADR-003): no retry in v1; widens the
  bounded label for no consumer. The transient/permanent class is a log field.
- **A per-provider latency histogram** — DEFERRED. No latency NFR exists for v1; the counter + `outcome` is the
  required signal. A histogram is a clean additive follow-up if delivery-latency SLOs are later scoped (it
  would reuse the same bounded labels).
- **A new dashboard/exporter** — REJECTED (D7). The shipped `/metrics` sidecar + Prometheus/Grafana already
  scrape the app; the delivery family appears automatically. No new infra.

## Consequences
- Positive: reuses the shipped metric facade, sidecar, emission pattern, register-at-0 idiom, and — most
  importantly — the ADR-011 bounded-label discipline + fail-closed cardinality test. One consistent
  observability posture; zero new infra (D7, R6 mitigated by construction).
- Positive: the register-at-0 cross-product means Olivia sees a flat-zero baseline for every active
  provider×event×outcome on first scrape (no "no-data" panels), and the delivered/failed split is visible per
  channel (US-03).
- Negative: register-at-0 mints |active providers| × |catalog| × 2 zero-series (e.g. 2×5×2 = 20) — bounded and
  small; accepted.
- Probe (Earned Trust): the cardinality unit test fails closed on an added label (single-layer-bypass safe with
  the value @property); after N notifications across M active providers the families sum to N×M with the
  correct outcome split (AC-03.4, @property fan-out completeness + bounded labels, AC-06.5).
