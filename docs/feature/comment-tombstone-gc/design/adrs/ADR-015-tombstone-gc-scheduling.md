# ADR-015: Tombstone GC Scheduling Pattern (Cadence + Batching + Hosting + Failure Handling)

## Status
Accepted — 2026-05-26

## Context

Slice 5 (commit bf35a68 lineage) introduced soft-delete for comments,
deferring the hard-delete GC task to v0.2 per ADR-007 ("Hybrid: soft
now, GC at 90 days"). Slice 7 is the implementation of that GC
commitment. Slice 7 is ALSO the first scheduled cleanup task in the
project — finding 1a of `proposals.md` documents that slice-1's
architecture.md committed to background cleanup with advisory locks
(`expired bootstrap tokens, expired sessions, expired reset tokens`)
but no such task ever landed in production code. The pattern this
slice picks becomes the precedent for ALL future cleanup tasks:
expired sessions GC, expired bootstrap tokens GC, expired reset
tokens GC, expired invites GC.

Four decisions are bundled into this ADR because they form a single
coherent "scheduled cleanup task" pattern. Splitting them across four
ADRs would scatter the same conceptual decision:

- **Cadence** (Q1): how often does the task run?
- **Batching + safety cap** (Q2): how much work does it do per
  invocation, and is there a hard ceiling against runaway?
- **Hosting** (Q3): where does the spawn + loop code live?
- **Failure handling** (Q7): what does the task do on error?

The decisions are co-dependent: e.g., "log + continue on failure" is
viable BECAUSE the cadence is daily (the natural cadence IS the
backoff); the inline hosting choice is viable BECAUSE there's only
one cleanup task today; the batch cap is viable BECAUSE the cadence
will drain a backlog over multiple ticks.

Quality attributes driving these decisions: **operational simplicity
(HIGH)** — first cleanup task should pick the simplest viable pattern;
**storage boundedness (HIGH)** — bounded growth is the whole purpose
of the slice; **privacy / GDPR posture (HIGH)** — 90-day retention is
the operator-facing promise; **fault tolerance (MEDIUM)** — transient
errors must not kill the task.

## Decision

Slice 7 establishes the following scheduled-cleanup-task pattern,
which becomes the precedent for all future cleanup tasks:

### Cadence: Daily (Q1 = A)

The GC tokio task uses `tokio::time::interval(Duration::from_secs(86400))`
(24 hours). `MissedTickBehavior::Skip` fires the first tick within ~5
seconds of interval construction; subsequent ticks at 24h. Env-tunable
via `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS` (default 86400).

The 90-day GDPR-friendly retention threshold tolerates slack measured
in hours, not days. Daily cadence is comfortably inside the operator
promise (a row may live ~89.0 → 90.999 days before hard-delete; not a
privacy regression).

### Batching with safety cap (Q2 = B)

Each GC tick runs a loop:

```sql
DELETE FROM comments
WHERE id IN (
    SELECT id FROM comments
    WHERE deleted_at < now() - ($1 || ' days')::interval
    LIMIT $2
)
```

where `$2` is the batch size (1000 default) and the loop terminates
when either `rows_affected < batch_size` (drained) OR
`cumulative_deleted >= per_run_cap` (cap hit; 10,000 default).
Env-tunable: `FOUNDRY_TOMBSTONE_GC_MAX_PER_RUN` (default 10000) and
`FOUNDRY_TOMBSTONE_GC_OLDER_THAN_DAYS` (default 90).

The per-run cap is cheap insurance against operational misconfig of
`deleted_at` (the textbook "GC hit the wrong threshold" disaster). At
expected steady-state load (~50 tombstones per workspace per quarter),
the cap is never reached — pure safety net. During recovery from a
misconfig, the cap means GC removes 10k rows per tick and stops,
giving operators time to notice in the next scrape window before more
is lost.

### Hosting: Inline in main.rs (Q3 = A)

The GC task spawn lives in `crates/foundry-app/src/main.rs`, directly
next to the slice-6 pool-poll task (main.rs lines 160-183). No new
module `gc.rs` this slice.

The slice-6 D5 precedent ("hybrid: no new file unless cohesion
requires") applies. ONE background cleanup task doesn't yet justify a
`gc.rs`. Promotion to a `gc.rs` module is mechanical when the next
cleanup task arrives (extract both tasks into `gc.rs`, ship in the
same v0.3 slice that adds the second one).

### Failure handling: Log + continue (Q7 = A)

Each tick:

1. Call `store.gc_tombstoned_comments(...)`.
2. On `Ok(n)` — log `tracing::info!(deleted_count = n, "tombstone GC tick completed")`; emit metrics (ADR-016).
3. On `Err(e)` — log `tracing::warn!(error = ?e, "tombstone GC tick failed; will retry next interval")`. Task survives; next tick fires at normal cadence.

No retry-with-backoff; daily cadence already IS the backoff. No
process exit on failure; the GC is non-essential for serving traffic.
No metric for "task health" beyond ADR-016's pending gauge going flat
(which is the operator-facing alerting signal).

### Advisory-lock single-replica execution

The GC task uses `pg_try_advisory_lock(TOMBSTONE_GC_LOCK_ID)` (non-
blocking) to ensure only one replica actually deletes; sibling
replicas exit with `Ok(0)`. Constant:

```rust
const TOMBSTONE_GC_LOCK_ID: i64 = 0x_60_C0_DE_60_C0_DE_60_u64 as i64;
```

Distinct literal from `MIGRATION_LOCK_ID` so `pg_locks` output
distinguishes them during operational triage.

## Alternatives Considered

### A: Daily cadence + batched-with-cap + inline + log-and-continue (chosen)
See Decision.

### B: Hourly cadence (rejected)
- **Pros**: Tighter retention boundary (~1h slip vs ~24h).
- **Cons**: Operational over-engineering for a 90-day threshold. 24×
  more ticks for ~0 user-observable benefit.
- **Rejected because**: simplest viable pattern principle (this is
  the first cleanup task; future cleanup tasks at sub-daily cadence
  get their own task — different concerns shouldn't share a tick).

### C: Manual-trigger only via `foundry doctor gc-comments` (rejected)
- **Pros**: Maximum operator control; zero background-task complexity.
- **Cons**: Operators who don't read the docs ship a privacy
  regression (forgotten cron → unbounded storage).
- **Rejected because**: "automatic by default" is the safer default
  for an OSS tool whose operators are unknown.

### D: Delete-all-in-one-transaction (rejected for batching)
- **Pros**: Simplest SQL; atomic.
- **Cons**: No safety against misconfigured `deleted_at`; the whole
  table could evaporate in one transaction. Long-running DELETE on a
  busy `comments` table holds row-level locks for the duration.
- **Rejected because**: no recovery safety net.

### E: Batched, no cap (rejected for batching)
- **Pros**: Single tick clears any backlog.
- **Cons**: Removes the recovery safety net (cap is cheap insurance).
- **Rejected because**: cap defends the textbook GC failure mode.

### F: New module `crates/foundry-app/src/gc.rs` (rejected for hosting, deferred)
- **Pros**: Keeps `main.rs` thin; cohesive home for cleanup tasks.
- **Cons**: This slice ALONE doesn't justify a module; the future
  pattern does.
- **Rejected because**: deferred — promote when the second cleanup
  task lands. Slice-6 D5 precedent.

### G: Push into `foundry-store` with task driver (rejected for hosting)
- **Pros**: Keeps cleanup logic next to the SQL it executes.
- **Cons**: Mixes concerns; `foundry-store` becomes a runtime
  orchestrator, not just an adapter.
- **Rejected because**: separation of concerns; the store crate
  stays a pure adapter.

### H: Log + exponential backoff (rejected for failure handling)
- **Pros**: Reduces log spam for persistent errors.
- **Cons**: Backoff at daily cadence is awkward ("log every 24h,
  48h, 96h, 192h" doesn't help operators).
- **Rejected because**: the gauge-flatness alerting story (ADR-016)
  is the operator-facing signal; backoff adds state for no operational
  benefit.

### I: Abort GC task after 3 consecutive failures (rejected for failure handling)
- **Pros**: Loudest signal — operator can't miss a dead GC task.
- **Cons**: "Restart the pod to recover from a transient blip" is a
  heavy hammer.
- **Rejected because**: K8s liveness probes would catch a dead task,
  but the rest of Foundry is still serving requests fine; killing the
  pod is collateral damage.

### J: Crash the process on first persistent error (rejected for failure handling)
- **Pros**: Maximum operator-facing signal.
- **Cons**: Conflates "the GC is having a bad day" with "the substrate
  is fundamentally broken." Way too aggressive for a daily cleanup
  task.
- **Rejected because**: violates failure-isolation principle.

## Consequences

### Positive
- **First cleanup task picks the simplest viable cadence.** Daily is
  comfortably inside the 90-day SLA. Simplest possible
  `tokio::time::interval`.
- **Storage growth is bounded.** Daily GC at 90-day threshold ensures
  growth is capped by recent-90-days deletion volume.
- **Recovery safety net.** The 10k per-run cap means a misconfigured
  `deleted_at` is recoverable — operators have one daily cycle of
  warning before more is lost.
- **Multi-replica safe.** Advisory lock ensures only one replica
  actually deletes; sibling replicas exit gracefully.
- **Operational simplicity for future cleanup tasks.** The pattern
  established here (daily + batched + inline + log-and-continue +
  advisory-lock) is the precedent for expired sessions GC, expired
  bootstrap tokens GC, etc.
- **Failure isolation.** Transient errors (pool drop, lock contention)
  log and continue; the task survives; the next tick succeeds.

### Negative
- **Backlog drains slowly.** A genuine backlog of, say, 1M tombstones
  takes 100 days to drain at the 10k cap. Mitigation: operators can
  raise `FOUNDRY_TOMBSTONE_GC_MAX_PER_RUN` during recovery without
  redeploying; the env var is the operator's "go bigger" knob.
- **Daily cadence means retention slips up to 24h past threshold.** A
  row deleted at second N may live to second N + 90 days + 24h. Not a
  privacy regression at the 90-day-promise scale.
- **`main.rs` grows by ~50 LOC.** Becomes more "and another thing"
  parking lot. Mitigation: promote to `gc.rs` when 2nd cleanup task
  lands.
- **Persistent errors produce daily-repeating log lines.** Not
  spammy at daily cadence; operators alert on gauge flatness, not log
  presence. If operators want log-based alerting on the warning,
  Loki-style absence-alerts work.

### Neutral
- **Reversibility**: cadence + batch + cap + threshold are all env-
  tunable; failure-handling and hosting are code-level reversible.
  Promoting to a `gc.rs` module is mechanical extraction.
- **Composition with future manual-trigger CLI**: if v0.3 adds
  `foundry doctor gc-comments [--older-than 90d]`, the inner function
  `Store::gc_tombstoned_comments` is reused verbatim from the CLI; the
  daily task and the CLI share the same code path.

## Establishes-pattern note

ADR-007 (slice 5) cited "the existing cleanup-task pattern from slice
1" but no such pattern existed in production code (finding 1a of
`proposals.md`). Slice 1's `architecture.md` line 259 committed to
background cleanup with advisory locks; the code never landed.

Slice 7 / ADR-015 ESTABLISHES the pattern rather than inherits it.
This is honest with ADR-007's intent (advisory-lock cleanup at a
sensible cadence) but adjusts the framing: the pattern is now real,
in production code, with this slice. Future cleanup tasks inherit
from ADR-015, not from slice-1's prose.

## Verification

- An acceptance scenario inserts 3 rows with `deleted_at = now() - 91
  days` and 3 rows with `deleted_at = now() - 89 days`, runs the GC
  tick, asserts the older 3 vanished and the newer 3 remain.
  (Date-arithmetic substrate-lie probe per principle 12.)
- An acceptance scenario inserts 11,000 ancient tombstones, runs one
  GC tick, asserts 10,000 went and 1,000 remained; ticks again,
  asserts the remaining 1,000 went. (Batch-cap substrate-lie probe.)
- An acceptance scenario spawns two concurrent GC task ticks and
  asserts exactly one performs work. (Advisory-lock substrate-lie
  probe.)
- An acceptance scenario forces a transient `StoreError::Sqlx`
  mid-tick (via the slice-3 test-hooks pattern) and asserts the task
  survives; the next tick succeeds. (Failure-handling substrate-lie
  probe per Q7 = A.)
- A walking-skeleton scenario (DISTILL confirms) spawns the real app
  with `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS=1` and asserts the GC
  task fires within ~5s and updates the counter.
- The metric assertions (counter increment, gauge state) live in
  ADR-016's verification list.
