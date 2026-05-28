# DISTILL wave-decisions — gc-transient-state-hardening

**Date**: 2026-05-27
**Wave coverage**: DISTILL only (DISCUSS + DESIGN intentionally skipped — test-side
hardening; fourth in the slice-6 / us-06 / slice-7 / this series)
**Inheritance**:
- `docs/evolution/2026-05-27-slice-7-gc-counter-race.md` — fixed the *monotonic*
  `comments_tombstones_purged_total` counter race. This feature fixes the two GC
  scenarios slice-7 D5 explicitly scoped OUT (the `@slow` cap scenario and the
  pending-gauge scenario), which assert *transient / non-monotonic* values.
- `crates/foundry-app/src/main.rs:307-340` — unchanged production: each tick
  increments `purged_total` by `deleted` then sets `comments_tombstones_pending`
  = `count_pending_tombstones()`. Both register at 0 at startup (`main.rs:230-231`).

## Problem in one paragraph

The full release-mode `cargo xtask ci` gate (`FOUNDRY_ACCEPTANCE_TAGS=all cargo test
-p foundry-acceptance --release`) — which prior sessions never ran clean — surfaced
two flaky GC scenarios that the debug-mode `@all` count had masked, failing
intermittently (3 failures one run, 2 the next):

1. **`@slow` cap scenario** asserts the *between-ticks intermediate* state "1000
   tombstones remain after exactly one capped tick" at a fixed wall-clock wait.
   Release timing let the second tick fire before the scrape → "found 0".
2. **gc-metrics gauge scenario** asserts the *exact transient* value
   `comments_tombstones_pending == 1` at a fixed wait. The gauge is non-monotonic
   (drains 3 → 1 → 0); release timing let an extra tick fire → read 0.

These are the same temporal-sampling class as slice-6/us-06/slice-7, but harder:
the values are ones the system only *passes through* between ticks. slice-7's
monotonic-counter fix (`>=` bounded-poll) does not apply — you cannot robustly
"poll until == 1" a value that visits 1 transiently then leaves, and the gauge's
register-at-0 startup value defeats a naive "poll until <= 1".

## Decisions

### D1 — Assert an ordered value subsequence over the whole trajectory

New helper `poll_until_metric_sequence(addr, name, expected: &[f64], timeout)` polls
`/metrics` every 250ms, records the metric's value trajectory, and matches `expected`
as an **ordered subsequence** (values need not be contiguous; leading non-matching
values such as the register-at-0 startup sample are ignored). Returns early once the
full sequence is matched; panics with the observed trajectory + match progress on
timeout. This is immune to *when* each tick fires — only *that* the values occur in
order — which is exactly the property a fixed-wait scrape lacks.

**Safety:** each value plateaus for a full tick cadence (4s / 6s) while the poll runs
at 250ms, so no plateau is skipped between consecutive polls. The only residual risk
(starting the poll after the first target value has already passed) is bounded by
starting the poll immediately after the seed Given, well before the first-tick offset
(= cadence in test mode).

### D2 — Gherkin: `the "<metric>" (counter|gauge) passes through <v1>, <v2>, … within <N> seconds`

- **Cap scenario**: `the "comments_tombstones_purged_total" counter passes through
  10000, 11000 within 30 seconds`. This *proves the cap*: if the first tick deleted
  all 11000 (cap broken) the counter would jump 0 → 11000 and never equal 10000, so
  the match for 10000 times out and the test fails. The subsequent `database holds 0
  … older than 90 days` is deterministic because production increments the counter
  only after the DELETE commits (slice-7 D4 ordering principle).
- **Gauge scenario**: `the "comments_tombstones_pending" gauge passes through 3, 1, 0
  within 30 seconds`. Captures the non-monotonic cap-2 drain (5 →3 →1 →0) as an
  ordered pass-through. The leading register-at-0 / pre-tick value is ignored.

### D3 — Remove the now-subsumed fixed-wait / one-shot-scrape steps

The `running for N seconds` + `the operator scrapes the metrics endpoint` + exact-
value / intermediate-count asserts are removed from both scenarios (the trajectory
poll owns its own scrape loop). The `scrape body contains the line
"comments_tombstones_pending"` sub-step is dropped from the gauge scenario — the
pass-through assertion cannot match an absent metric, so presence is implied; the
register-at-0 contract is covered by the broader metrics scenarios. The removed step
*definitions* stay (used by the gc-lock / gc-failure / threshold scenarios).

### D4 — Production code stays unchanged

Zero changes to `crates/foundry-app/` / `crates/foundry-store/`. The cap, the gauge
update, and the register-at-0 are all correct; the fix is in the test assertion shape.

### D5 — The trajectory poll alone was NOT enough: runtime starvation under @all

First validation (3 release-mode @all sweeps) FAILED with a revealing signal:
`poll_until_metric_sequence` observed `[0.0, 11000.0]` for the counter (never saw
the 10000 plateau) and `[0.0, 3.0, 0.0]` for the gauge (never saw the 1 plateau).
Root cause: **under 6-way @all parallelism + argon2 `spawn_blocking` load, the test's
own poll loop is starved** — it sampled only every several seconds, not every 250ms,
so it skipped the 4–6s transient plateaus. This is distinct from the earlier
"ticks drift" framing: it is the *sampler* being starved, not just the GC ticks.

The lesson generalises: **terminal/monotonic assertions survive sampler starvation**
(slice-6 "eventually > 0", slice-7 "reaches N" — once true, they stay true, so a
sparse sampler eventually catches them), but **any assertion that must catch a
transient intermediate is unreliable in the parallel lane**, regardless of assertion
shape.

Resolution:
- **cap + gauge** genuinely require observing a transient (the 10000 cap step; the
  non-monotonic 3→1→0 drain), so they are tagged **`@serial`**. cucumber-rs runs
  `@serial` scenarios de-contended (never concurrent with any other scenario), so the
  GC subprocess gets the machine, ticks fire on cadence, and the 250ms poll is not
  starved — the trajectory pass-through then observes every plateau. No harness
  change needed; `@serial` is built into the default runner and respected regardless
  of `max_concurrent_scenarios(6)`.
- **gc-threshold** (a third scenario that surfaced as an intermittent flake — fixed-
  wait "holds 3" after a tick) is fixed *without* `@serial` by the monotonic
  counter-reaches pattern (poll `purged_total` to 3 = the 3 ancient deleted, then the
  row-count assertions are deterministic). Monotonic = starvation-robust, so it stays
  in the parallel lane.

### D7 — Slice-6 pool-gauge register-at-0 companion: bounded-poll "settles to 0"

A fourth scenario surfaced after the cap/gauge/threshold fixes went green: the slice-6
`@startup-register` companion ("Immediately after process start, the connection-pool
gauge is scrapable at value 0") read `db_connections_in_use == 1`. Same class — an
exact transient gauge value via one-shot scrape: a startup/readyz query holds a pool
connection when the 1s poll samples, and the test's "scrape immediately" lands after
that poll.

**First attempt — `@serial` — FAILED.** Tagging it `@serial` did NOT fix it (it still
read 1 under @all) and correlated with new `PoolTimedOut` failures across the @all
sweeps (3 `@serial` scenarios appear to have tipped the connection-pool scheduling).
Reverted. **Crucially, a 5×-default-lane sweep showed this scenario flakes ~40% in the
*default* lane too** — so the slice-6 D5 "passes deterministically in the default lane"
claim does not hold in release mode.

**Resolution — bounded-poll "settles to 0".** The register-at-0 *contract* is "the
metric line is present immediately so Grafana never shows no-data" — asserted by the
unchanged `HTTP 200` + `contains the line "db_connections_in_use"` steps. The exact
value at the scrape instant is over-specified and racy; replaced
`sample has value 0` with `sample settles to 0 within 5 seconds` (new
`then_scrape_body_sample_settles_to` step, `poll_until_sample` with `== 0` predicate).
The idle pool returns all connections, so the gauge reaches 0 within a poll or two and
stays — robust in both lanes because the subprocess under test is itself idle
regardless of contention on *sibling* subprocesses. New step lives in
`handler_instrumentation.rs` (slice-6 owns the scenario).

### D8 — Release gates on the DEFAULT lane; the @all stress lane is a tracked follow-up

The full `@all` lane (`cargo xtask ci`'s acceptance step) is a stress superset (all
tags incl. `@slow` + `@docker-compose`, max contention) that has multiple independent
pre-existing flake sources beyond the four fixed here — notably `PoolTimedOut`
(N×10-conn sqlx pools against a shared 100-conn Postgres container) and residual
runtime starvation. Per user decision, **v0.2.0 gates on the DEFAULT lane** (what
normal dev/CI runs; excludes `@slow`/`@docker-compose`), validated green across 5
consecutive release-mode sweeps (107/107). The `@all` stress-lane stabilization
(PoolTimedOut config/concurrency, any remaining transient assertions) is captured as a
tracked follow-up — see the evolution doc's Open Items and `CONTEXT.md`.

### D6 — Production code stays unchanged

Unchanged from D4 — restated after the D5 discovery to be explicit: the starvation is
a *test-runtime* artefact of the parallel lane, not a production scheduling defect.
The GC task ticks correctly; the acceptance harness just cannot reliably *observe*
sub-tick transients while sharing a saturated runtime with 5 sibling scenarios.

## Why the RED gate doesn't apply

Same as the prior three hardenings: no missing production code, scenarios pass in
isolation, change is assertion-shape. Replacement gate: isolation pass + **multiple
release-mode @all sweeps** (the contention condition that exposed the flakes).

## Files touched

| Path | Change |
|---|---|
| `crates/foundry-acceptance/src/support/metrics_scrape.rs` | Add `pub poll_until_metric_sequence` (ordered-subsequence trajectory poll) |
| `crates/foundry-acceptance/src/steps/us_10_tombstone_gc.rs` | Add the `…(counter\|gauge) passes through …` Then |
| `crates/foundry-acceptance/tests/features/comment-tombstone-gc.feature` | Reword cap + gauge to pass-through assertions and tag them `@serial`; convert gc-threshold to the monotonic counter-reaches pattern |
| `crates/foundry-acceptance/tests/features/handler-instrumentation.feature` | Slice-6 register-at-0 companion: `sample has value 0` → `sample settles to 0 within 5 seconds` (D7) |
| `crates/foundry-acceptance/src/steps/handler_instrumentation.rs` | Add `then_scrape_body_sample_settles_to` (bounded-poll `== target`) for D7 |

Production code: untouched. DESIGN docs: untouched. DEVOPS / CI: untouched.

## Verification protocol

Isolation:
```
cargo test -p foundry-acceptance --test acceptance -- --name "per-run cap|pending-tombstones gauge"
```
Sufficient evidence: **N≥3 consecutive release-mode @all sweeps green**:
```
FOUNDRY_ACCEPTANCE_TAGS=all cargo test -p foundry-acceptance --release
```
Results appended below after the sweeps complete.

### Verification results

The investigation ran 17 release-mode acceptance sweeps total. Timeline:

1. **@all, trajectory only (no @serial)** — FAILED. Revealed the starvation root
   cause: counter trajectory `[0, 11000]` (missed 10000), gauge `[0, 3, 0]` (missed
   1). The poll sampler is starved under @all and skips transient plateaus. → D5.
2. **@all, cap+gauge `@serial` + threshold counter-reaches** — cap/gauge/threshold
   GREEN across all 3 sweeps. Surfaced a 4th scenario: slice-6 register-at-0
   (`db_connections_in_use == 0` read 1) once. → D7.
3. **@all, + register-at-0 `@serial`** — WORSE: register-at-0 still flaked and
   `PoolTimedOut` appeared in all 3 sweeps. `@serial` reverted. → D7, D8.
4. **Default lane ×5, register-at-0 `@serial` reverted** — register-at-0 flaked 2/5
   (so it is NOT default-lane-deterministic). Everything else green ×5. → D7.
5. **Default lane ×5, register-at-0 `settles to 0`** — **107/107 GREEN ×5.** The
   default-lane gate is clean. → D7, D8.

**Outcome:** cap (`@serial` + trajectory), gauge (`@serial` + trajectory), gc-threshold
(counter-reaches), and register-at-0 (settles-to-0) are all hardened and green in their
respective lanes. v0.2.0 gates on the DEFAULT lane (5/5 green). The `@all` stress lane's
remaining flake sources (`PoolTimedOut` pool exhaustion; any residual transient under
heavy starvation) are a tracked follow-up — NOT a v0.2.0 blocker per D8.
