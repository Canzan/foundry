# DISTILL wave-decisions — slice-6-scenario-hardening

**Date**: 2026-05-27
**Wave coverage**: DISTILL only (DISCUSS + DESIGN intentionally skipped per user direction — this is a single-scenario test-side hardening, not a new feature)
**Inheritance**:
- `docs/evolution/2026-05-26-handler-instrumentation.md` — slice-6 design + invariants
- This session's `CONTEXT.md` — `@nw-troubleshooter` RCA: scenario asserts at one
  instant, but the gauge is updated by a 1s poll task; the polled value is
  temporal — the assertion shape was wrong, not the production code
- `crates/foundry-app/src/main.rs:208-219` — the unchanged 1s pool-poll task that
  emits `db_connections_in_use`

## Problem in one paragraph

Slice-6 scenario "The Postgres connection pool gauge reflects the in-use
connection count within one polling interval" asserts via a one-shot scrape
after a 6-second connection-hold window. Under `FOUNDRY_ACCEPTANCE_TAGS=all`
contention (cap=6 parallel scenarios × harness pool=10), the subprocess's pool
occasionally shows `in_use=0` at the exact scrape instant — the most recent
poll tick observed a transient idle window. The metric IS being updated; the
test samples it at a moment that may or may not capture `in_use > 0`. Single-
instant assertion of a temporal property = flake.

## Decisions

### D1 — Option A: re-word the Gherkin to express the temporal nature

Picked Option A over Option B (silent step-impl fix preserving the existing
English). Gherkin should reflect what's actually being tested. The metric is a
sampled gauge; asserting "eventually > 0 within 5 seconds" is the truthful
statement of the contract the production code provides. A reader of the
`.feature` file should be able to predict the step implementation's shape from
the words.

**Before**:
```gherkin
And the scrape body's "db_connections_in_use" sample is greater than 0
```

**After**:
```gherkin
And the scrape body's "db_connections_in_use" sample is eventually greater than 0 within 5 seconds
```

The `When the operator scrapes the metrics endpoint` step is REMOVED from this
scenario — the new Then step owns its own scrape loop. Keeping the existing
`When` would either (a) make the Then ignore the captured snapshot, which is
deceptive, or (b) require a two-phase "scrape, then keep scraping" idiom that
muddles the contract.

### D2 — Helper signature

```rust
/// Poll the `/metrics` endpoint up to `timeout`, returning the first
/// `MetricSample` for `metric_name` that satisfies `predicate`. Panics
/// with the full sample history on timeout — flake-debuggable.
async fn poll_until_sample<P>(
    addr: SocketAddr,
    metric_name: &str,
    predicate: P,
    timeout: Duration,
) -> MetricSample
where
    P: Fn(&MetricSample) -> bool,
```

Lives next to the existing step bodies in
`crates/foundry-acceptance/src/steps/handler_instrumentation.rs`. Not promoted
to `support/metrics_scrape.rs` yet — single caller, single scenario. Promote
when the second caller appears (YAGNI).

The predicate operates on a `MetricSample`, matching the established shape of
`ScrapeSnapshot::samples_for(name) -> Vec<&MetricSample>`. The first sample
that satisfies the predicate wins; for gauges the family typically has one
sample, so this is equivalent to "any sample for this metric satisfies it".

### D3 — Deadline, polling interval, per-scrape timeout

| Knob | Value | Reason |
|---|---|---|
| `timeout` (outer deadline) | 10 seconds | Initial 5s was insufficient under `@all` contention: the shared `scrape_metrics_raw`'s 10s reqwest timeout meant a single slow scrape could monopolise the whole deadline (run 6 surfaced this — only one scrape recorded across a 5s window). 10s gives 10+ poll ticks at the 1s `METRICS_POOL_POLL_SECONDS` cadence with 13+ scrape iterations even when each scrape costs 750ms. Acceptable wall-clock growth: at most +4s vs the original 8s scenario budget (6s connection-hold + 10s deadline worst-case if the gauge stays 0 the whole time). |
| inner poll interval | 250ms | Up to ~30 scrapes across the 10s deadline (depends on per-scrape latency). The metrics scrape is a cheap HTTP GET against the local sidecar listener (sub-millisecond serialize + IPC under idle load); 250ms is the smallest interval that doesn't busy-loop while staying responsive enough that we converge fast on the first satisfying tick. |
| per-scrape timeout (`POLL_SCRAPE_TIMEOUT`) | 750ms | The helper builds its own reqwest client with a short timeout instead of calling `scrape_metrics_raw`. Reason: the shared scrape's 10s reqwest timeout is appropriate for one-shot startup-probe scenarios but pathological inside a poll loop — one hung scrape stalls the whole window. 750ms fails-fast under contention; the polling history captures the error and we move on to the next iteration. |
| total scenario wall-clock | unchanged on the happy path; +4s worst case | The 6s connection-hold is the dominant cost; the new Then typically returns after the first satisfying scrape (often <1s into the 10s window). Worst case (gauge stays 0 across the whole deadline): scenario wall-clock grows from ~8s to ~16s — acceptable, and that case indicates the production code legitimately isn't holding a pool conn (i.e., a real bug we WANT to surface). |

### D4 — Panic-on-timeout carries the full sample history

The helper's panic message dumps every scrape outcome it observed during the
deadline window:

```
poll_until_sample for `db_connections_in_use` timed out after 5s.
Scrapes observed:
  [t+0.00s] no samples
  [t+0.25s] samples=[{value: 0.0}]
  [t+0.50s] samples=[{value: 0.0}]
  ...
```

The existing one-shot assertion gives "expected > 0, got 0" with no history —
debugging a flake means re-running with logging on. The new shape captures the
history as part of the failure itself: a flake-investigator gets the temporal
shape of what the subprocess actually emitted, in the test output.

### D5 — Companion `register-at-0` scenario stays unchanged

Scenario "Immediately after process start, the connection-pool gauge is
scrapable at value 0 so Grafana sees the metric line without a delay" stays as
written. It asserts a different invariant (the register-at-0 guarantee from
slice-6 D4 — see `docs/evolution/2026-05-26-handler-instrumentation.md` line
108-110) at a structurally non-flaky moment (immediately after spawn, no
in-flight traffic). It already passes deterministically; touching it would
violate the brief's "no change to the companion scenario" constraint.

### D6 — Production code stays unchanged

Per the brief: zero changes to `crates/foundry-app/`, `crates/foundry-store/`,
`crates/foundry-realtime/`. The 1s poll task in `main.rs:208-219` is correct;
the metric definition is correct; the dashboard is correct. The fix is in the
test layer only.

## Out-of-scope confirmations

- No new ports → no `docs/architecture/atdd-infrastructure-policy.md` change.
  Existing cucumber-rs + tokio + testcontainers-rs(postgres) infra stays.
- No new outcomes registry entry — this is a test hardening, not a new typed
  contract.
- No 4-reviewer parallel review gate at end of DISTILL per project convention
  (review happens at PR time).
- No `__SCAFFOLD__` markers — Rust codebase has no Red Gate Snapshot
  mechanism and no production code changes.
- No DELIVER dispatch — direct TDD via `@nw-software-crafter` is for future
  features; this is a single test-side change.

## Verification protocol applied

Per the brief's verification protocol: run the slice-6 scenario in isolation
post-change.

```
cargo test -p foundry-acceptance --test acceptance -- --name "Postgres connection pool"
```

### First-attempt @all run (run 6, 5s deadline) — FAILED

The initial 5s deadline + shared 10s-timeout scrape proved insufficient under
@all contention. Poll history captured a single observation at `t+0.00s` with
`value=0.0` then panicked. Inspection of `scrape_metrics_raw`
(`crates/foundry-acceptance/src/support/metrics_scrape.rs:150`) revealed the
shared 10s reqwest timeout — one slow scrape consumed the full outer deadline.

### Iterated @all run (run 7, 10s deadline + 750ms per-scrape) — PASSED

The helper now builds its own short-timeout reqwest client; the outer deadline
bumped to 10s. The scenario passed cleanly. Other failures in run 7 are
unrelated pre-existing flakes (slice-7 GC counter race, US-06 timing-symmetry
argon2-blocking-pool contention) — neither belongs to this DISTILL.

The full `@all`-mode flake-resistance check (N≥5 consecutive runs) remains the
user's responsibility per the acceptance criterion. Run 7 = 1-of-1 pass; CI
will accumulate the larger sample.

## Files touched

| Path | Change |
|---|---|
| `crates/foundry-acceptance/tests/features/handler-instrumentation.feature` | Re-word one Then; remove the standalone `When` from that scenario (the Then owns its own poll loop) |
| `crates/foundry-acceptance/src/steps/handler_instrumentation.rs` | Add `poll_until_sample` helper; replace one `#[then]` handler |

Production code: untouched.
DESIGN docs: untouched.
DEVOPS / CI: untouched.
