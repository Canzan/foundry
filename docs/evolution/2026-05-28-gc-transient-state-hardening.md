# Evolution — gc-transient-state-hardening

**Finalized**: 2026-05-28
**Ship commit**: [4e722ed](../../) — "Harden transient-state GC/pool metric scenarios for the @all lane"
**Wave coverage**: DISTILL only (test-side hardening; fourth in the slice-6 / us-06 / slice-7 / this series)

## Feature summary

Hardens four acceptance scenarios that asserted **transient or non-monotonic**
metric values via one-shot scrapes at fixed waits and flaked under release-mode
contention. Establishes the principle that distinguishes a robust metric
assertion from a fragile one under parallel execution, and lands the
`poll_until_metric_sequence` and `settles to` helpers that express it.

This pass began as "fix the two slice-7 scenarios slice-7 D5 scoped out" (cap +
pending-gauge) and expanded as the full release-mode `@all` gate — never run
clean by prior sessions — surfaced two more (gc-threshold, slice-6
register-at-0) and one infrastructure flake (`PoolTimedOut`).

## Business context

The v0.2.0 release process (`RELEASING.md`) gates on `cargo xtask ci`, whose
acceptance step runs `FOUNDRY_ACCEPTANCE_TAGS=all` — a stress superset. That
gate had been red since before slice 7 (the `cargo fmt` gate was also broken),
masked because prior sessions ran targeted tests rather than the full pipeline.
Greening it fully turned out to be an open-ended stabilization problem with
multiple independent root causes, so v0.2.0 was re-gated on the **default lane**
(see Decisions / Open Items).

## Key decisions (from DISTILL `docs/feature/gc-transient-state-hardening/distill/`)

- **D1/D2 — `poll_until_metric_sequence`.** Asserts a metric *passes through* an
  ordered value subsequence over its whole observed trajectory (cap:
  `purged_total` 10000→11000 proves the per-run cap; gauge:
  `comments_tombstones_pending` 3→1→0 captures the non-monotonic drain). Immune
  to *when* each tick fires.
- **D5 — the real root cause is sampler starvation, not assertion shape.** The
  first @all validation FAILED with `[0, 11000]` / `[0, 3, 0]` trajectories: under
  6-way parallelism the test's own poll loop is starved and samples every few
  seconds, skipping transient plateaus. **Terminal/monotonic assertions survive
  starvation** (slice-6 "eventually > 0", slice-7 "reaches N"); transient-catching
  ones do not.
- **Resolution per scenario:**
  - **cap + gauge** genuinely need to observe a transient → tagged `@serial`
    (cucumber runs them de-contended; the 250ms poll then catches every plateau).
    Green across 3 @all sweeps.
  - **gc-threshold** → monotonic counter-reaches (poll `purged_total` to 3, then
    deterministic row counts). Starvation-robust; stays parallel.
  - **D7 — slice-6 register-at-0** → bounded-poll "settles to 0". `@serial` was
    tried first and **reverted**: it didn't fix the flake and correlated with
    `PoolTimedOut`. The register-at-0 *contract* (line present immediately) stays
    asserted by HTTP-200 + contains-the-line; the racy exact `== 0` became
    `settles to 0 within 5 seconds` (idle pool reaches 0 and stays — robust in
    both lanes because the subprocess under test is itself idle).
- **D8 — gate on the default lane.** Per user decision, v0.2.0 gates on the
  DEFAULT lane (normal dev/CI; excludes `@slow`/`@docker-compose`), green across 5
  consecutive release-mode sweeps. The `@all` stress lane is a tracked follow-up.

## Files touched (commit `4e722ed`)

| Path | Change |
|---|---|
| `crates/foundry-acceptance/src/support/metrics_scrape.rs` | `pub poll_until_metric_sequence` (ordered-subsequence trajectory poll) |
| `crates/foundry-acceptance/src/steps/us_10_tombstone_gc.rs` | `the "<m>" (counter\|gauge) passes through … within N seconds` step |
| `crates/foundry-acceptance/src/steps/handler_instrumentation.rs` | `the "<m>" sample settles to N within N seconds` step |
| `crates/foundry-acceptance/tests/features/comment-tombstone-gc.feature` | cap + gauge → pass-through + `@serial`; gc-threshold → counter-reaches |
| `crates/foundry-acceptance/tests/features/handler-instrumentation.feature` | register-at-0 → `settles to 0` |

Production code: untouched. DESIGN docs: untouched.

## Verification

17 release-mode acceptance sweeps across the investigation (see wave-decisions
§ Verification results for the full timeline). Final state:

- **Default lane (the v0.2.0 gate): 107/107 GREEN across 5 consecutive
  release-mode sweeps.**
- cap / gauge / gc-threshold: green across 3 release-mode `@all` sweeps.
- register-at-0: green in isolation + the 5× default-lane sweep.
- `cargo clippy --release` + `cargo fmt --all --check`: clean.

## Lessons learned

1. **Under parallel execution, the sampler can be as starved as the system under
   test.** The flake was not (only) that GC ticks drift — the test's poll loop
   itself was denied the CPU and sampled too sparsely to catch transient
   plateaus. Any "watch for a transient value" assertion is fragile in a
   contended lane no matter how the assertion is written.
2. **Prefer terminal/monotonic observables.** "Eventually reaches N" / "settles to
   N" / "passes through [monotonic sequence]" survive starvation because the
   target, once reached, persists. Asserting a value the system only *visits*
   (a non-monotonic gauge step, a between-ticks intermediate) requires either a
   monotonic reframing or de-contention (`@serial`).
3. **`@serial` is a scheduling change with side effects.** It fixed cap/gauge
   (they genuinely need de-contention) but did not fix register-at-0 and
   correlated with `PoolTimedOut` — adding serial scenarios perturbs the
   connection-pool scheduling. Reach for a robust *assertion* before reaching for
   `@serial`; use `@serial` only when observing a transient is unavoidable.
4. **Run the real gate early.** `cargo xtask ci` (full release `@all`) had been
   red for multiple slices because nobody ran it; targeted tests passed and hid
   it. A broken gate that no one runs is indistinguishable from no gate.
5. **A stress superset is not the same as the CI gate.** `@all` (all tags, max
   contention) is a useful stress run but a poor release gate — its flakes are
   dominated by shared-resource contention (`PoolTimedOut`), not by product
   correctness. The default lane is the honest gate; `@all` stabilization is its
   own infrastructure track.

## Issues encountered

- **`@serial` for register-at-0 was a dead end** (reverted) — see D7.
- **`PoolTimedOut` under @all** — N×10-conn sqlx pools against a shared 100-conn
  Postgres container; surfaced/aggravated by the @serial scheduling change.
  Deferred to the @all follow-up.

## Open items / tracked follow-up

1. **@all stress-lane stabilization** (NOT a v0.2.0 blocker):
   - `PoolTimedOut`: right-size the testcontainer `max_connections` vs. N×pool
     demand, and/or lower @all concurrency, and/or stagger heavy-insert scenarios.
   - Re-audit any remaining transient-value assertions for heavy-starvation
     robustness once PoolTimedOut is resolved.
2. **5 deferred metrics** (`outbox_pending_jobs`, `bootstrap_tokens_unclaimed`,
   `migration_apply_duration_seconds`, `realtime_listen_disconnects_total`,
   `probe_failures_total`) — still need dashboard consumers + emission.
3. **v0.2.0 tag** — unblocked on the default-lane gate; release prep proceeds.

## Workflow note

Per project convention, the 4-reviewer parallel gate is deferred to PR time.
