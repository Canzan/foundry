# ADR-019: Counter Call-Site Emission + Probe Self-Monitoring

## Status
Accepted — 2026-05-28

## Context

Slice 8 ships two counters from the slice-6 D0 deferred catalog:

- `realtime_listen_disconnects_total` (counter, no labels) — LISTEN
  connection drops; "should be near-zero".
- `probe_failures_total` (counter, `probe_name`) — probe verifications
  that failed; the recursive Principle-9 self-monitoring metric per
  `observability-infra.md` § Probe contract.

Both follow the slice-6 ADR-010 pattern: counters increment at the
event call-site (`.increment(1)`), not on a poll. The two decisions are
bundled because they share that mechanism and because
`probe_failures_total` is the principle-12 self-application — the metric
that observes whether the substrate-honesty probes are still passing.

Relevant existing code:

- **Realtime reconnect**: `crates/foundry-realtime/src/lib.rs`
  `run_pg_listener` (lines 129-150). The `Err(err)` arm (line 139) logs
  `pg_listener: connection error, retrying after backoff` and sleeps
  with exponential backoff. This is exactly one connection-drop event.
- **Startup probes**: `crates/foundry-app/src/main.rs` lines 356-371
  call `metrics_server::probe(metrics_addr)` (slice-6 ADR-014) and the
  process also runs `Store::probe()` earlier in boot. Both refuse to
  start on failure and the metrics probe already emits a structured
  `health.startup.refused` log line (`metrics_server.rs:296`).
- There is **no `foundry doctor` probe-family subcommand** today (the
  `observability-infra.md` text and the slice-8 brief reference one as
  the eventual home; the concrete probes live in the `main.rs` startup
  sequence). Slice 8 wires the counter onto the probes that exist.

Quality attributes driving this decision: **observability completeness
(HIGH)** — close the no-emitter gap; **self-monitoring (MEDIUM)** —
Principle-9 recursion; **bounded cardinality (HIGH)** — `probe_name`
must stay a closed code-defined set; **performance (MEDIUM)** — both
counters sit at cold paths (reconnect, startup), zero request-hot-path
cost.

## Decision

### `realtime_listen_disconnects_total` — increment at the reconnect arm (D2)

Add `metrics::counter!("realtime_listen_disconnects_total").increment(1)`
inside `run_pg_listener`'s `Err(err)` branch
(`foundry-realtime/src/lib.rs:140`), before the backoff sleep. This is
the single decision chokepoint where "we observed a drop and will
reconnect" happens. Unlabelled — bounded at exactly 1 series.
Register-at-0 in `main.rs` so Grafana shows a flat-zero baseline (the
desired near-zero state).

### `probe_failures_total` — wrap the existing startup probes (D5)

On each startup-probe `Err`, increment
`probe_failures_total{probe_name=...}` BEFORE the error propagates (the
process still refuses to start, preserving the slice-6 ADR-014
posture). The `probe_name` set is the closed, code-defined set of
probes — currently `{store, metrics}`. Register-at-0 for each known
`probe_name` at startup so the dashboard shows the full probe set as
flat-zero lines ("all probes passing").

Because the counter increments on a process that is about to exit, its
value is observed by the NEXT `/metrics` scrape (or the next replica's
scrape, or — for a refuse-to-start — via the already-emitted
`health.startup.refused` log line). The dashboard signal is "a probe
failed recently"; replicas restart-loop loudly, which is the operator's
pager signal per ADR-014.

This establishes the invariant: **`probe_failures_total` is wired to
every code-defined probe.** Adding a probe (a new `*::probe()` call in
the startup sequence, or a future periodic re-probe task) requires
adding its `probe_name` to the register-at-0 set AND incrementing the
counter on its failure. The set is bounded + code-defined; never
request-derived. This is the recursive Principle-9 self-application: the
counter monitors whether the substrate-honesty checks themselves are
still passing after every Foundry upgrade (a probe that silently stopped
running shows as a suspiciously flat counter — alertable).

## Alternatives Considered

### `realtime_listen_disconnects_total`

#### A: Increment at the `run_pg_listener` reconnect arm (chosen)
See Decision.

#### B: Observe restarts from `main.rs` (rejected)
- **Pros**: keeps the metric name out of the realtime crate.
- **Cons**: the LISTEN task reconnects internally and never returns on a
  drop; `main.rs` cannot observe individual drops without a new channel —
  more plumbing for worse fidelity.
- **Rejected because**: the reconnect arm is the natural, single
  chokepoint; the realtime crate already depends on `metrics` (slice 6).

#### C: Count inside `listen_loop` on the `None`/error return (rejected)
- **Cons**: `listen_loop` returns the error up to `run_pg_listener`,
  which is where the reconnect decision is made; counting in both places
  risks double-counting.
- **Rejected because**: A is the single chokepoint.

### `probe_failures_total`

#### A: Wrap the existing `main.rs` startup probes (chosen)
See Decision.

#### B: Defer until a `doctor`-probe subcommand family exists (rejected)
- **Cons**: recreates the "metric with no emitter" gap slice 8 exists to
  close; the catalog explicitly ties this counter to the existing probe
  pattern (Principle 9).
- **Rejected because**: defeats the slice's purpose.

#### C: Also emit from a periodic probe-rerun background task (rejected for now)
- **Pros**: a live "are probes passing right now?" signal.
- **Cons**: new background task + new failure mode (boot-healthy but
  later-flapping probe) — scope creep beyond "emit the deferred metric".
- **Rejected because**: clean v0.x evolution that reuses the same
  counter without a rename; slice 8 wires the counter onto the boot
  probes first.

## Consequences

### Positive
- Closes the no-emitter gap for both counters with one line (realtime)
  + a small wrap (probes).
- `probe_failures_total` realises the Principle-9 recursive self-
  monitoring story against real probes.
- Both counters are at cold paths — zero request-hot-path cost; no
  NFR-PERF-05 budget consumed.
- `probe_name` is bounded + code-defined; cardinality invariant holds.

### Negative
- The realtime crate carries a metric name string. Mitigation: it
  already emits no metrics but already depends on `metrics`; one name is
  a small, reviewable surface.
- `probe_failures_total` increments on a dying process; the value is
  observed by the next scrape/replica, not the dying one. Mitigation:
  the `health.startup.refused` log line + restart-loop are the
  immediate operator signals (ADR-014); the counter is the
  dashboard-historical signal.

### Neutral
- Reversibility: both are single-line / small-wrap code changes; no
  schema impact.
- A future periodic re-probe task (alternative C) reuses the same
  counter and the same `probe_name` set without a metric rename.

## Verification

- An acceptance scenario severs / restarts the testcontainers Postgres
  to drop the LISTEN connection; asserts
  `realtime_listen_disconnects_total` increments by exactly 1 per
  reconnect and the task survives (Earned-Trust: "the task always
  recovers").
- An acceptance scenario asserts `realtime_listen_disconnects_total`
  reads 0 at startup (register-at-0).
- An acceptance scenario forces a startup probe failure (bind
  `METRICS_PORT` before boot, slice-6 ADR-014 precedent); asserts the
  `health.startup.refused` log line fires AND the process exits non-zero
  (and, per DISTILL open-question 4, that `probe_failures_total{probe_name="metrics"}`
  reflects the failure on the next scrape if a non-fatal observation
  path is exercised).
- The extended `metrics_server.rs` cardinality test asserts
  `probe_failures_total` carries exactly `{probe_name}` and
  `realtime_listen_disconnects_total` carries no labels.
