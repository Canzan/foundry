# DISTILL wave-decisions — acceptance-all-lane-stabilization

**Date**: 2026-05-28
**Wave coverage**: DISTILL only (test-infrastructure hardening; follow-on to
gc-transient-state-hardening D8, which deferred the @all stress-lane instability)
**Goal**: Make the release-mode `@all` acceptance lane reliably green so
`cargo xtask ci` passes, closing the gap left by the v0.2.0 default-lane gate.

## Problem

`cargo xtask ci`'s acceptance step runs `FOUNDRY_ACCEPTANCE_TAGS=all cargo test
-p foundry-acceptance --release` — a stress superset (all tags incl. `@slow` +
`@docker-compose`, max contention). After the four transient-assertion fixes
(gc-transient-state-hardening), the residual @all failure observed was
`PoolTimedOut` on connection-pool acquisition during scenario setup
(e.g. "insert admin user").

## Connection-demand analysis (the suspected root cause)

Shared resource: **one** `Postgres::default()` testcontainer per `cargo test`
invocation (`support/harness.rs:46`). PostgreSQL default `max_connections = 100`
(~3 reserved for superuser → ~97 usable).

Demand: each scenario opens a per-scenario sqlx pool with
`max_connections(10)` + `acquire_timeout(5s)` (`harness.rs:107-117` and the
`_no_migrations` variant `163-167`). Several scenario shapes open *more than one*
pool against the shared container:
- slice-6/slice-7 subprocess scenarios: the in-process harness pool **plus** the
  spawned foundry subprocess's own pool (production default also 10).
- US-02 multi-replica: two replica pools.
- gc-lock: an extra advisory-lock-holder pool from a separate connection.

Under `@all` with `max_concurrent_scenarios(6)`, peak demand can be
`6 × (10 + 10 + …)` which exceeds the ~97-connection ceiling. When the ceiling is
hit, a pool's attempt to open a new backend blocks; `pool.acquire()` waits up to
its 5s `acquire_timeout` and then returns `PoolTimedOut`. The seed step that
happens to acquire at peak is the visible victim ("insert admin user").

Note: `@docker-compose` scenarios run their *own* compose Postgres (not the shared
container), but they add CPU/IO load and overlap with the shared-container
scenarios, raising the chance of a peak.

## Hypothesis to confirm against the baseline sweep

The `PoolTimedOut` appeared in the 3-`@serial`-scenario batch (when register-at-0
was also `@serial`) but NOT in the 2-`@serial` batch. register-at-0 `@serial` is
now reverted (it uses `settles to 0`). So the **current committed code has only
cap + gauge `@serial`** — i.e. the configuration that did NOT show `PoolTimedOut`.
Baseline question: **does `PoolTimedOut` still occur with the current code?**

- If NO across the baseline sweeps → the @all lane may already be green; the
  `@serial`-scheduling perturbation was the trigger, now removed. Minimal/no fix.
- If YES → raise the testcontainer ceiling (see candidate fix), since the demand
  math exceeds 97 regardless of `@serial`.

## Candidate fix (if PoolTimedOut persists)

**Raise the shared testcontainer's `max_connections`** by starting Postgres with a
command override, e.g. `Postgres::default().with_cmd(["postgres", "-c",
"max_connections=300"])` (via the testcontainers `ImageExt` trait). The container
is ephemeral (one per test run), so the headroom is free; 300 covers 6 concurrent
scenarios × ~30 connections with margin. This is the **root-cause** fix — it
removes the ceiling as the bottleneck without touching the production-mirrored
per-scenario pool size (10, pinned by US-02's NFR-PERF-04 ceiling assertion) or the
`@all` concurrency cap (6, chosen to mirror the default lane for the slice-6
db_connections scenario).

Rejected alternatives:
- Lower per-scenario pool size — re-introduces the post-argon2 workspace-seed
  `PoolTimedOut` (commit `906ceab` bumped it TO 10 to fix exactly that) and breaks
  US-02's ≤10 assertion.
- Lower `@all` concurrency (6→4) — slows the suite and weakens the default-lane
  parity the cap was chosen for.
- Bigger `acquire_timeout` — band-aid; hides the ceiling instead of removing it.

## Findings (baseline + iterations)

### Baseline (3 @all sweeps, current committed code) — `PoolTimedOut` is GONE

No `PoolTimedOut` in any baseline sweep. Confirmed: it was a v3 artifact of the
(now-reverted) register-at-0 `@serial` — 3 concurrent serial scenarios perturbed
the connection scheduling at the 100 ceiling. With register-at-0 reverted (only
cap + gauge `@serial`), seeds acquire fine. Sweeps 2 & 3 were 111/111.

**Residual flake (1/3): slice-6 `db_connections_in_use > 0`** — the bounded-poll
timed out (38 scrapes, gauge never rose above 0).

### Root cause of the residual — load-generator starvation, not the ceiling

The scenario's `Mei holds an open database connection` step
(`handler_instrumentation.rs:624`) spawns **32 `tokio::spawn` tasks hammering
`/readyz`** to keep the subprocess pool's `in_use > 0`. Under @all those tasks
share the test runtime with 5 sibling scenarios and are **starved** — they don't
sustain enough requests to saturate the subprocess pool, so the gauge stays 0 and
the bounded-poll correctly times out. Same starvation family as
gc-transient-state-hardening D5, but here the *load generator* is starved (not the
sampler). Raising `max_connections` 100→300 only moved the flake 1/3 → 1/5 (the
gauge stayed 0 even with ample connections — the requests weren't being sent),
confirming it is a scheduling problem, not a ceiling problem.

### Fixes applied

1. **`max_connections=300`** (`harness.rs`) — still justified: removes the real
   connection-ceiling risk (the v3 `PoolTimedOut`; the demand math exceeds ~97)
   and, critically, makes it **safe to add a 3rd `@serial` scenario** (3 serial
   scenarios hit `PoolTimedOut` at the old 100 ceiling).
2. **`@serial` on the slice-6 `db_connections_in_use` scenario** — de-contends it
   so the 32 load-generator tasks get the CPU to actually saturate the subprocess
   pool. Same class of fix as cap/gauge (a scenario that must *produce* a specific
   runtime condition, not just observe one, needs de-contention under @all).

## Verification protocol

Baseline: 3 @all sweeps (done — see Findings). Post-fix: 5 consecutive release-mode
`@all` sweeps with `max_connections=300` + db_connections `@serial`. Results
appended below.

### Result — 5/5 release-mode @all sweeps GREEN (111/111 each)

`max_connections=300` + `@serial` on the slice-6 db_connections scenario produced
**five consecutive clean `@all` sweeps (111 scenarios / 983 steps each, exit 0)**.
The stress lane is stable. Combined with the four gc-transient-state-hardening
fixes and the v0.2.0 default-lane gate (107/107 ×5), both acceptance lanes are now
reliably green in release mode.

Clippy (`--release`) + fmt clean.

Across the full investigation (this feature + gc-transient-state-hardening): the
@all flakes had three distinct root causes, all surfaced by running the real
release-mode gate that prior sessions never ran — (1) transient/non-monotonic
assertions sampled at one instant, (2) test-runtime starvation of pollers AND
load-generators under 6-way parallelism, (3) shared-container connection-ceiling
contention. Fixes, respectively: bounded-poll/ordered-subsequence assertions;
`@serial` for scenarios that must produce/observe a runtime condition; and a raised
ephemeral-container `max_connections`.

## Follow-up note (not blocking)

`cargo xtask ci`'s acceptance step is now expected to pass (its `@all` run is
green). If the project wants belt-and-suspenders, a future change could add a short
retry/quarantine policy, but no flake remains across 5 sweeps. The DEFAULT lane
remains the v0.2.0 gate; the @all lane being green now means the documented
`cargo xtask ci` gate and the release gate agree.
