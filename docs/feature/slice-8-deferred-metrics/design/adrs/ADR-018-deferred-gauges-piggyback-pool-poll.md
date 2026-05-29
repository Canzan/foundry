# ADR-018: Deferred DB-State Gauges Piggyback the Pool-Poll Loop

## Status
Accepted — 2026-05-28

## Context

Slice 8 ships the 5 deferred metrics from slice-6 D0's
`observability-infra.md` catalog. Two of them are gauges over Postgres
state:

- `outbox_pending_jobs` — `count(*)` of unprocessed outbox rows
  (`WHERE notified_at IS NULL`, served by the `idx_outbox_pending`
  partial index).
- `bootstrap_tokens_unclaimed` — `count(*)` of active unclaimed admin
  tokens (`WHERE used_at IS NULL AND expires_at > now()`).

Gauges over DB state are refreshed by reading the value and calling
`metrics::gauge!(...).set(...)`. Two background-task precedents already
exist in `crates/foundry-app/src/main.rs`:

1. **The slice-6 pool-stats poll** (ADR-012, `main.rs:196-219`) — a
   `tokio::spawn` with `tokio::time::interval(5s)` that reads
   `Store::pool_stats()` and sets `db_connections_in_use`.
2. **The slice-7 daily GC tick** (ADR-015, `main.rs:282-349`) — a daily
   task that, after each sweep, polls `count_pending_tombstones` and
   sets the `comments_tombstones_pending` gauge.

The design lever: which loop refreshes the two new gauges, or do they
get their own task?

Quality attributes driving this decision: **operational simplicity
(HIGH)** — no new background task unless cohesion requires; **deploy-
time visibility (MEDIUM)** — `bootstrap_tokens_unclaimed` must be
visible within seconds of boot, not next-day; **performance (MEDIUM)** —
the refresh must not cost the request hot path anything.

## Decision

**Fold the two new DB-state gauges into the EXISTING slice-6 5-second
pool-poll loop. No new `tokio::spawn`, no new cadence constant, no new
env var. Both gauges register at value 0 before the task spawns
(slice-6 D4 / ADR-014 register-at-0 precedent).**

Each tick of the pool-poll loop already runs to refresh
`db_connections_in_use`; it gains two additional reads
(`Store::count_pending_outbox()`,
`Store::count_unclaimed_bootstrap_tokens(now)`) and two
`gauge!(...).set(...)` calls. Both queries are index-served `count(*)`
reads — negligible cost at the 5s cadence.

Failure semantics match the slice-7 pending-gauge pattern
(`main.rs:337-347`): if a count query errors, the gauge is simply not
updated that tick; the stale value ages out / goes flat, and operators
alert on flatness rather than on a missing series.

This establishes the project invariant: **a new gauge over DB state
reuses the nearest existing poll loop unless its cadence genuinely
diverges.** When a future gauge needs a different cadence, promote ALL
DB-state gauges into a dedicated `gauge_poll` task in the same slice
that introduces the divergence.

## Alternatives Considered

### A: Piggyback the slice-6 5s pool poll (chosen)
See Decision.

### B: New dedicated `tokio::spawn` poll task at its own cadence (rejected)
- **Pros**: decouples cadence from the pool poll; could pick a slower
  tick for slow-moving counts.
- **Cons**: new spawn + cadence constant + (likely) env var — exactly
  the ceremony slice-6 D5 + slice-7 D3 said to avoid for a single
  cohesive concern. The counts are index-served and trivially cheap, so
  a slower cadence buys nothing.
- **Rejected because**: three DB-state gauges sharing one
  read-DB-state loop IS the cohesive grouping; B is premature.

### C: Piggyback the slice-7 daily GC tick (rejected)
- **Pros**: conceptually groups "DB-state gauges" with the existing
  gauge-polling task.
- **Cons**: daily cadence is far too coarse — `bootstrap_tokens_unclaimed`
  at deploy time needs sub-minute visibility (an operator who just
  deployed wants to see "admin not yet claimed" now, not tomorrow).
- **Rejected because**: cadence mismatch defeats the metric's purpose.

## Consequences

### Positive
- Smallest delta; zero new background tasks; one cadence to reason about.
- `bootstrap_tokens_unclaimed` refreshes every 5s — crisp deploy-time
  signal.
- Register-at-0 means Grafana never shows "no data" for either gauge.
- Test cadence override is free — the existing `METRICS_POOL_POLL_SECONDS`
  env var already shortens the loop for acceptance scenarios.

### Negative
- Couples three unrelated gauges to one 5s cadence. Mitigation: the
  established invariant says promote to a dedicated task only when a
  cadence genuinely diverges.
- If the pool poll is ever removed, the two gauges lose their home.
  Mitigation: documented as part of the same task; removal is a code-
  review-visible change.

### Neutral
- Reversibility: removing the gauges is a code-level change inside the
  poll closure + the register-at-0 block; no schema/migration impact.
- The two new `Store` count methods sit next to the slice-7
  `count_pending_tombstones`, forming a small "count pending/unclaimed"
  family on the adapter.

## Verification

- An acceptance scenario seeds N outbox rows with `notified_at IS NULL`
  and M with `notified_at` set; after a poll tick (cadence overridden
  to ~1s) asserts `outbox_pending_jobs == N` via the `/metrics` scrape.
- An acceptance scenario seeds an unclaimed token, a used token, and an
  expired token; asserts `bootstrap_tokens_unclaimed == 1` after a tick.
- An acceptance scenario asserts both gauges read 0 immediately at
  startup (register-at-0), before the first poll tick.
- Cardinality: both gauges are unlabelled (1 series each) — covered by
  the extended `metrics_server.rs` cardinality test (no labels leak).
