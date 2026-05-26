# ADR-012: DB Pool Gauge — Poll-Based with Deferred Wait Histogram

## Status
Accepted — 2026-05-25

## Context

The Grafana "Foundry Overview" dashboard has Panel 5 ("Postgres pool") with
two queries:

- Panel 5a: `db_connections_in_use` (gauge) — current in-use connection
  count.
- Panel 5b: `histogram_quantile(0.95, sum by (le)
  (rate(db_connection_wait_seconds_bucket[5m])))` — p95 acquire wait time.

Both are dashboard-referenced, so by D0 (Q0 = ship 5 dashboard-referenced
metrics) both should be emitted. But the implementation strategies for
each differ significantly:

- sqlx 0.8 exposes `Pool::size()` (total connections in the pool) and
  `Pool::num_idle()` (currently idle) as public read-only methods.
  `in_use = size - num_idle`. No instrumentation needed for the gauge
  beyond reading these accessors.
- sqlx 0.8 does NOT expose a public event hook for "acquire was called"
  or "connection was returned". Those are internal; the only way to
  observe acquire wait time is to wrap every call to `pool.acquire()` in
  a `Timer::elapsed()` measurement. Foundry's `Store` does
  `sqlx::query("...").fetch_one(&self.pool)` in ~30 call sites; each uses
  an implicit acquire that cannot be wrapped without changing the call
  pattern.

Slice 6's load profile (per slice-1 scaling.md: 0.25 req/sec sustained,
~10-connection pool) does not exhaust the pool under any realistic
condition — `db_connection_wait_seconds` would be near-zero or empty
even if we did instrument it.

Quality attributes driving this decision: **observability completeness
(HIGH)** — Panel 5a is the dashboard's primary "is the pool healthy?"
view; **maintainability (HIGH)** — wrapping 30 implicit-acquire sites is
significant slice expansion; **simplicity (HIGH)** — defer what can be
deferred honestly (matching DEVOPS slice's "instrument me" posture for
unconsumed metrics).

## Decision

**Two-part decision aligned with Q3 = C:**

1. **`db_connections_in_use` gauge**: poll-based update via a background
   `tokio::time::interval` task in `crates/foundry-app/src/main.rs`. Every
   5 seconds, the task reads `Store::pool_stats()` (a new read-only
   accessor on the Store wrapping `pool.size()` and `pool.num_idle()`) and
   sets the gauge. Poll interval (5s) is configurable via env var
   `METRICS_POOL_POLL_SECONDS` (default 5).

2. **`db_connection_wait_seconds` histogram**: **DEFERRED to v0.2**.
   Panel 5b stays half-empty until either (a) sqlx upstream exposes
   acquire-time hooks, or (b) operational pain forces the `TimedPool`
   wrapper approach. The empty panel is the honest "instrument me"
   signal, matching the DEVOPS slice's precedent for unconsumed metrics.

The 5s poll interval is chosen to be << the 15s Prometheus scrape
interval, so the gauge is at most one scrape behind reality
(operationally indistinguishable from real-time for a pool-utilization
metric).

`Store::pool_stats()` returns a `PoolStats { in_use: i32, idle: i32, size: i32 }`
struct so the polling task can also export idle/size if needed later
without an API change. Initial slice ships only `db_connections_in_use`
emission.

## Alternatives Considered

### A: Poll-based for both `in_use` AND `wait_seconds`
Polling task samples pool state every 5s AND emits a zero-bucket histogram
observation so the dashboard panel isn't empty.

- **Pros**: Honestly populates both panels with available data; no
  pool-wrapping churn.
- **Cons**: The wait histogram is effectively empty (all observations are
  zero or absent), which is worse than "deliberately blank" — it gives
  the false impression that wait time IS being measured and is always
  zero. Operationally misleading.
- **Rejected because**: better to leave the panel blank with a known
  reason than populate it with misleading data.

### B: Event-based via a `Pool` wrapper (`TimedPool` / `PoolMetrics`)
Wrap the sqlx `Pool` with a `TimedPool` that intercepts `acquire()` calls
to measure wait time. Every `Store` method that does
`sqlx::query(...).fetch_one(&self.pool)` would need to change to
`let mut conn = self.acquire_timed().await?; sqlx::query(...).fetch_one(&mut *conn)`.

- **Pros**: Real-time accurate wait observation. Wait histogram is
  meaningfully populated. Pool exhaustion observable.
- **Cons**: Requires changing ~30 implicit-acquire call sites across
  `Store`. Significant slice expansion for telemetry no operator would
  consume at slice-1 load profile (0.25 req/sec doesn't exhaust a
  10-conn pool). Becomes a maintenance burden — every new Store method
  must use the timed-acquire pattern. Couples telemetry into the
  domain-adjacent code.
- **Rejected because**: cost (30-site churn + ongoing maintenance) far
  exceeds benefit (telemetry no current consumer needs). The decision
  is conditional: this becomes the right answer if operational pain
  forces it (see revisit condition below).

### C: Poll-based `in_use`; defer `wait_seconds` (chosen)
See Decision.

## Consequences

### Positive
- Zero hot-path overhead. The polling task runs asynchronously; the
  Store query path is unchanged.
- Zero new dependencies. `tokio::time::interval` is core tokio.
- Stock sqlx 0.8 — no version pin, no feature flag, no vendored fork.
- Background task pattern is already used (slice-5 deferred GC for
  comments — same shape).
- Honest signal: Panel 5b's empty state mirrors the DEVOPS-slice
  "instrument me" precedent applied recursively. Operators reading the
  dashboard see "this metric isn't implemented yet" rather than
  "this metric is always zero" (which they'd misinterpret as healthy).
- `Store::pool_stats()` is a small, read-only API addition — easy to
  extend (e.g., add per-pool stats when multi-pool sharding lands)
  without breaking changes.

### Negative
- Gauge is up to 5 seconds stale. For a metric scraped every 15s, this is
  operationally indistinguishable from real-time, but technically the
  worst-case "stale gauge" window is 5s.
- Panel 5b is empty until v0.2 or until the revisit condition fires.
  Acceptable per D0 precedent.
- The polling task is a new long-lived spawn point. Must be wired into
  the existing graceful-shutdown handling (see "Open Question 5" in
  `wave-decisions.md`).

### Neutral
- `METRICS_POOL_POLL_SECONDS` env var (default 5) is the operator's
  knob if 5s is too coarse for some future use case. Tunable without
  redeploy.
- If sqlx upstream exposes acquire hooks before operational pain
  forces alternative B, the deferred histogram can land as a drop-in
  v0.2 PR without changing any Store call sites.

## Revisit Condition

Re-evaluate this decision (specifically the `db_connection_wait_seconds`
deferral) when EITHER of the following is true:

1. **sqlx exposes acquire hooks**. If a future sqlx version provides
   a public callback or event stream for `Pool::acquire` start/finish
   timing, implement the wait histogram by hooking that mechanism. No
   call-site changes required.

2. **Operational forcing function**. If a Foundry deployment experiences
   pool-exhaustion symptoms (request latency spikes correlated with
   exhausted pool capacity; sustained `in_use == size` over multiple
   scrape windows; user complaints of intermittent 5xx that map to
   `sqlx::Error::PoolTimedOut`) AND the existing
   `db_connections_in_use` gauge alone is insufficient to root-cause,
   then ship the `TimedPool` wrapper (alternative B) regardless of
   sqlx upstream status.

Until one of these fires, the panel stays half-empty. Document the
revisit in this ADR's amendment section when it happens.

## Verification

- The polling task is started at process startup; an acceptance scenario
  asserts the `db_connections_in_use` line appears in `/metrics` output
  within one poll interval (5s) of startup.
- A scenario that acquires a connection and holds it for >5s + then
  scrapes `/metrics` asserts the gauge value reflects the in-use state
  (was 0 before acquire; >=1 during hold; back to 0 after release within
  one poll interval). This is the principle-12 probe ("substrate lie that
  the polling task spawned but never ran").
- `Store::pool_stats()` is unit-tested with a real testcontainers
  Postgres to confirm `in_use + idle == size` invariant holds.
- An assertion that `db_connection_wait_seconds_bucket` is NOT emitted
  (confirms the deferral is honored; prevents accidental partial
  implementation in a future PR).
