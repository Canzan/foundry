# Application Architecture — comment-tombstone-gc (slice 7)

Owner: solution-architect (Morgan). Slice-specific design summary.
Inherits the entire slice-1..6 architecture by reference; does NOT
restate the 5-crate workspace, dependency direction, advisory-lock
pattern, metrics-recorder lifecycle, or soft-delete semantics.

**Status**: FINAL — user picks on Q1–Q7 confirmed (D1–D7 below; full
rationale in `wave-decisions.md`). Picks were A / B / A / A / C / A / A
(all recommendations accepted verbatim; no overrides). ADRs ADR-015
(scheduling pattern), ADR-016 (observability + admin-undelete),
ADR-017 (VIEW deferred) capture the binding decisions.

## Inheritance

- **Workspace shape** — unchanged from `docs/feature/foundry-backend-mvp/design/adrs/ADR-001.md`.
  No new crates. All slice-7 code lands in existing files within
  `foundry-app` and `foundry-store`.
- **Advisory-lock pattern** — inherited from `crates/foundry-store/src/lib.rs`
  (`MIGRATION_LOCK_ID` + `scoped_migration_lock_id`). Slice 7 adds a
  second canonical lock id `TOMBSTONE_GC_LOCK_ID` following the same
  `i64` literal convention.
- **Soft-delete schema** — inherited from `0006_comments_edit_delete.sql`
  (ADR-007). The `deleted_at TIMESTAMPTZ NULL` + `deleted_by UUID NULL`
  columns are a strict subset of what GC needs; ADR-007 § Decision
  committed to this. NO new migration this slice.
- **Background task hosting** — inherited from slice-6 D5 (hybrid;
  inline in `main.rs`) and the pool-poll task (slice-6 main.rs lines
  160–183). Slice 7 follows the same shape: `tokio::spawn` with
  `tokio::time::interval`, infallible task body that logs on error and
  continues.
- **Metrics emission patterns** — inherited from slice-6 (poll-based
  gauge per ADR-012; counter-on-event per ADR-010). Bounded cardinality
  invariant (slice-6 D2) holds: new metrics carry no labels.
- **Operator CLI surface** — `foundry doctor <action>` family established
  by slice-3 `backup-verify` (`crates/foundry-app/src/admin_cli.rs` +
  `dispatch_subcommand` in `main.rs`). Slice 7 adds a sibling action
  `restore-comment`.

## What this slice changes

| Surface | Change |
|---|---|
| Background tasks | +1 new task: tombstone GC ticker in `main.rs` (Q3 = A, inline) |
| Store methods | +1 method: `Store::gc_tombstoned_comments(older_than, batch, cap)` |
| Store methods | +1 method: `Store::count_pending_tombstones(older_than)` (feeds the pending gauge) |
| Store methods | +1 method: `Store::undelete_comment(comment_id)` (CLI dispatch + psql parity) |
| Operator CLI | +1 subcommand: `foundry doctor restore-comment <comment_id>` (Q5 = C, CLI primary path) |
| Metrics | +2 metrics: `comments_tombstones_purged_total` (counter) + `comments_tombstones_pending` (gauge) (Q4 = A) |
| Advisory-lock constants | +1 constant: `TOMBSTONE_GC_LOCK_ID` next to `MIGRATION_LOCK_ID` |
| Env vars | +3 vars: `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS` (default 86400), `FOUNDRY_TOMBSTONE_GC_MAX_PER_RUN` (default 10000), `FOUNDRY_TOMBSTONE_GC_OLDER_THAN_DAYS` (default 90) |
| Database schema | NO change — slice-5 schema covers GC needs (per ADR-007) |
| HTTP routes | NO change — GC is background-only; restore is CLI-only |
| `EventPayload` | NO change — hard-delete fires no SSE event (after 90 days, no viewers care) |
| Documentation | `RELEASING.md` gains "Recovering an accidentally-deleted comment" runbook subsection (CLI primary + psql fallback per Q5 = C) |
| `comments_visible` SQL VIEW | NOT shipped this slice (Q6 = A, deferred to v0.3; ADR-017) |

Zero new crates. Zero new dependencies. Zero new external integrations.
Zero new database migrations.

## Component diagram (C4 Level 3) — GC task lifecycle + admin-undelete flow

```mermaid
sequenceDiagram
    autonumber
    participant M as main.rs (process start)
    participant T as GC tokio task
    participant ST as Store
    participant PG as Postgres
    participant MET as metrics recorder
    participant OP as Operator
    participant CLI as foundry doctor restore-comment

    Note over M,MET: process boot — every replica
    M->>ST: Store::connect + migrate + probe (existing slice 1+5)
    M->>T: tokio::spawn (interval = FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS, default 86400)

    Note over T,PG: each tick (first ~5s after boot per MissedTickBehavior::Skip; then every 24h)
    T->>ST: gc_tombstoned_comments(older_than=90d, batch=1000, cap=10000)
    ST->>PG: SELECT pg_try_advisory_lock(TOMBSTONE_GC_LOCK_ID)
    alt lock acquired (this replica wins)
        loop until rows_affected < batch OR cumulative >= cap
            ST->>PG: DELETE FROM comments WHERE id IN (SELECT id WHERE deleted_at < now() - interval '90 days' LIMIT 1000)
            PG-->>ST: rows_affected
        end
        ST->>PG: SELECT pg_advisory_unlock(TOMBSTONE_GC_LOCK_ID)
        ST-->>T: Ok(total_deleted)
        T->>MET: counter comments_tombstones_purged_total += total_deleted
    else lock contended (another replica running)
        ST-->>T: Ok(0) — graceful no-op
    end
    T->>ST: count_pending_tombstones(older_than=90d)
    ST->>PG: SELECT count(*) FROM comments WHERE deleted_at < now() - interval '90 days'
    PG-->>ST: pending_count
    ST-->>T: Ok(pending_count)
    T->>MET: gauge comments_tombstones_pending = pending_count

    Note over T: on error path (ADR-016 / Q7 = A)
    alt ST returns Err (pool drop, lock blocked, transient PG error)
        T->>T: tracing::warn!(error, "tombstone GC tick failed; will retry next interval")
        Note right of T: task survives; next tick fires at normal cadence
    end

    Note over OP,PG: admin-undelete (Q5 = C primary path)
    OP->>CLI: foundry doctor restore-comment <uuid>
    CLI->>ST: Store::undelete_comment(uuid) on DATABASE_URL pool
    ST->>PG: UPDATE comments SET deleted_at=NULL, deleted_by=NULL WHERE id=$1 AND deleted_at IS NOT NULL
    PG-->>ST: rows_affected (1 = restored; 0 = not tombstoned / not found)
    ST-->>CLI: Ok(rows_affected)
    CLI-->>OP: stdout "status: restored" + exit 0 (or non-zero error code)
```

Properties the diagram makes obvious:

1. **Single-replica execution**: advisory lock ensures only one replica
   actually deletes; sibling replicas exit gracefully with `Ok(0)`.
2. **No HTTP path involvement for GC**: GC runs entirely on the background
   tokio runtime. No request flow, no handler involvement, no SSE.
3. **Metrics emission outside the lock**: counter increment + gauge set
   happen after lock release, minimising lock-hold time.
4. **Failure isolation**: errors are logged, never crash the process or
   abort the task (per ADR-016 / Q7 = A).
5. **CLI path bypasses the background task**: admin-undelete is a
   synchronous operator action against the live DB pool, independent of
   the GC tick schedule.

## Store method additions

Three new methods on the existing `Store` adapter
(`crates/foundry-store/src/lib.rs`). Signatures shape only — internals
are software-crafter territory:

| Method | Signature shape | Notes |
|---|---|---|
| `Store::gc_tombstoned_comments` | `(older_than: Duration, batch: usize, cap: usize) -> Result<u64, StoreError>` | Acquires `TOMBSTONE_GC_LOCK_ID` via `pg_try_advisory_lock` (non-blocking; returns `Ok(0)` if contended). Loops DELETE-with-LIMIT-subquery until rows_affected < batch OR cumulative >= cap. Releases lock in all paths (including error). Returns total rows deleted. |
| `Store::count_pending_tombstones` | `(older_than: Duration) -> Result<u64, StoreError>` | Pure read — `SELECT count(*) FROM comments WHERE deleted_at < now() - $1`. Feeds the `comments_tombstones_pending` gauge. |
| `Store::undelete_comment` | `(comment_id: Uuid) -> Result<u64, StoreError>` | Single UPDATE — `UPDATE comments SET deleted_at = NULL, deleted_by = NULL WHERE id = $1 AND deleted_at IS NOT NULL`. Returns rows affected (0 = comment not tombstoned or doesn't exist; 1 = restored). Idempotent — re-invoking on a restored row is a no-op zero-return. |

Constant added next to `MIGRATION_LOCK_ID`:

```rust
/// Advisory-lock id for the tombstone GC task (slice 7). Different
/// literal so `pg_locks` output distinguishes the GC lock from the
/// migration lock during operational triage.
const TOMBSTONE_GC_LOCK_ID: i64 = 0x_60_C0_DE_60_C0_DE_60_u64 as i64;
```

## Background task additions (in `crates/foundry-app/src/main.rs`)

Per Q3 = A (inline in main.rs). Spawned next to the slice-6 pool-poll
task. Env-var-driven configuration with defaults wired as `const` next
to `DEFAULT_METRICS_POOL_POLL_SECONDS` (slice-6 precedent):

| Default constant | Value | Override env var |
|---|---|---|
| `DEFAULT_TOMBSTONE_GC_INTERVAL_SECONDS` | 86400 (1 day) | `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS` |
| `DEFAULT_TOMBSTONE_GC_OLDER_THAN_DAYS` | 90 | `FOUNDRY_TOMBSTONE_GC_OLDER_THAN_DAYS` |
| `DEFAULT_TOMBSTONE_GC_MAX_PER_RUN` | 10000 | `FOUNDRY_TOMBSTONE_GC_MAX_PER_RUN` |

Spawn shape (illustrative; software-crafter owns the exact construction):

```rust
// Slice 7 (ADR-015) — background tombstone GC task. Runs once per
// FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS (default 86400 = daily).
// Advisory lock ensures only one replica actually deletes; sibling
// replicas exit Ok(0). Per ADR-015, errors are logged + task survives.
let gc_interval_seconds = std::env::var("FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS")
    .ok()
    .and_then(|v| v.parse::<u64>().ok())
    .filter(|n| *n > 0)
    .unwrap_or(DEFAULT_TOMBSTONE_GC_INTERVAL_SECONDS);
// (analogous parsing for OLDER_THAN_DAYS + MAX_PER_RUN)
let store_for_gc = state.store.clone();
tokio::spawn(async move {
    let mut ticker = tokio::time::interval(Duration::from_secs(gc_interval_seconds));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        // Run GC, log + emit metrics on success; log + continue on failure.
    }
});
```

Env-var precedent: `FOUNDRY_` prefix follows the slice-1
`FOUNDRY_PORT` / `FOUNDRY_HOST` family, distinguishing operator
configuration from internal `METRICS_*` / `SESSION_*` vars.

First-tick semantics: `tokio::time::interval`'s default
`MissedTickBehavior::Skip` fires the first tick almost immediately
after the interval is created (operator-visible signal "did the GC
actually start?" within ~5 seconds of boot, matching slice-6 pool-poll
behaviour).

## Failure handling pattern (ADR-015 § failure-handling)

Per Q7 = A. Each tick:

1. Call `store.gc_tombstoned_comments(...)`.
2. On `Ok(n)` — log `tracing::info!(deleted_count = n, "tombstone GC tick completed")`; emit counter increment + poll the pending gauge.
3. On `Err(e)` — log `tracing::warn!(error = ?e, "tombstone GC tick failed; will retry next interval")`. Task survives; next tick fires at normal cadence.

No retry-with-backoff; daily cadence already IS the backoff. No
process exit on failure; the GC is non-essential for serving traffic.
No metric for "task health" beyond the pending gauge going flat
(which is the operator-facing alerting signal).

This pattern is the precedent for ALL future cleanup tasks (expired
sessions, expired bootstrap tokens, expired reset tokens, expired
invites). ADR-015 captures it for posterity alongside the scheduling
choice.

## Observability additions (ADR-016 / Q4 = A)

Two new metrics emitted from the GC task, registered at startup at
value 0 per slice-6 D4 precedent (avoid "no data" Grafana panel):

| Metric | Type | Labels | Emission point |
|---|---|---|---|
| `comments_tombstones_purged_total` | counter | (none) | After each GC tick completes successfully; incremented by `rows_deleted`. |
| `comments_tombstones_pending` | gauge | (none) | At the end of each GC tick (after lock release); set to `count(*) WHERE deleted_at < now() - interval '90 days'`. |

Cardinality: both unlabelled — bounded at exactly 1 series each.
Honors slice-6 D2 invariant ("cardinality invariant: forbidden labels
list is binding").

Added to `docs/feature/foundry-backend-mvp/design/system/observability-infra.md`
metric-naming table (2 new rows). The slice-6 D0 deferred-metrics list
gains a note: "slice 7 ships 2 of these in the deferred metric
family ahead of the broader v0.3 instrumentation slice; the catalog
goes from 5 deferred → 3 deferred + 2 shipped. No dashboard panel
added this slice — the metrics are emitted but no Grafana panel
consumer yet (the same 'instrument me' recursive pattern slice 6
established with `foundry_app_startup_total`)."

Operator alerting story: `comments_tombstones_pending` flat over 48h
indicates a stalled GC task. Standard Prometheus rate / increase /
flatness alert; no custom rule shipped this slice (operator-specific).

## Admin-undelete operator surface (ADR-016 / Q5 = C — both)

### RELEASING.md runbook addition

New subsection "Recovering an accidentally-deleted comment", placed
after the existing `foundry doctor backup-verify` section:

> When a workspace admin deletes a comment that they later realize
> should not have been deleted (a moderation reversal), the comment
> can be restored as long as it has been less than 90 days since
> deletion. After 90 days the background tombstone GC task
> (ADR-015) hard-deletes the row and recovery is no longer possible
> without restoring from backup.
>
> Two recovery paths are supported. Prefer the CLI path.
>
> #### Path 1 — CLI (recommended)
>
> Run on any host with the `foundry` binary and `DATABASE_URL` set:
>
> ```sh
> foundry doctor restore-comment <comment_uuid>
> ```
>
> Exit 0 + `status: restored` on success. Exit non-zero if the
> comment is not in the database or is not currently tombstoned.
>
> #### Path 2 — psql (fallback)
>
> If the foundry binary is unavailable, the same UPDATE can be run
> manually:
>
> ```sql
> UPDATE comments
>    SET deleted_at = NULL, deleted_by = NULL
>  WHERE id = '<comment_uuid>'
>    AND deleted_at IS NOT NULL
> RETURNING id, body_markdown;
> ```
>
> Before running:
> - Confirm the UUID matches what was deleted (e.g., by joining to
>   `issues` for context).
> - Verify `deleted_at` is within the 90-day window. If the row no
>   longer exists, the tombstone GC already ran; the only recovery
>   path is restoring from a backup (per the slice-3 backup
>   runbook).
> - Inform the affected users that the comment is back.
>
> Both paths emit the existing `tracing::info!` log line on the
> Foundry app (path 1) or appear in Postgres audit logs (path 2),
> giving operators an audit trail of restorations.

### CLI subcommand

`foundry doctor restore-comment <comment_id>` follows the slice-3
`backup-verify` shape:

- Dispatch in `main.rs` `dispatch_subcommand` next to `"backup-verify"`
  arm.
- Implementation in `crates/foundry-app/src/admin_cli.rs` (new
  `pub fn run_restore_comment(comment_id: &str) -> i32`).
- Reads `DATABASE_URL` (unlike backup-verify's `FOUNDRY_DOCTOR_PROBE_URL`,
  the restore operates on the live production DB).
- Connects to the live pool, calls `Store::undelete_comment(uuid)`,
  prints `status: restored` on success.
- Exit codes (proposed; DISTILL confirms): 0 = restored; 2 = invalid UUID
  syntax; 3 = DB connect failure; 4 = comment not found or not tombstoned.

Acceptance scenario (DISTILL handoff documents the full shape): scenario
seeds a tombstoned comment, invokes the CLI via
`assert_cmd::Command::cargo_bin("foundry")`, asserts exit 0 and the
row's `deleted_at` returns to NULL.

## Quality attributes addressed

| Attribute | Mechanism |
|---|---|
| Operational simplicity (HIGH) | First cleanup task picks the simplest viable cadence (daily, ADR-015); failure handling pattern (log + continue, ADR-015) establishes precedent for future cleanup tasks without ceremony. |
| Storage boundedness (HIGH) | Daily GC at 90-day threshold ensures storage growth is bounded by recent-90-days deletion volume. Per-run cap (10k default) protects against runaway deletion from misconfigured `deleted_at`. |
| Privacy / GDPR (HIGH) | Tombstoned content is unrecoverable after 90 days. Operators can document a defensible retention policy. |
| Recoverability (MEDIUM) | Within the 90-day window, both CLI (`foundry doctor restore-comment`) and SQL paths preserve the slice-5 schema-affords-undelete property. |
| Observability (MEDIUM) | ADR-016 emits 2 metrics — `comments_tombstones_purged_total` (counter) + `comments_tombstones_pending` (gauge) — enabling alerting on GC stall. |
| Fault tolerance (MEDIUM) | Per ADR-015, transient errors (pool drop, lock contention with migration, Postgres restart) log and continue; the task survives. Multi-replica deployments survive any single-replica failure naturally. |

## External integration check (principle 10)

NONE new. The GC task is fully internal — same Postgres, same advisory-
lock pattern, same metrics sidecar. No contract test annotation for
the platform-architect handoff. Existing SMTP annotation from slice 1
remains.

## Architecture enforcement (principle 11)

Existing tooling suffices; no additions needed:

- `cargo xtask check-arch` — no crate-boundary changes.
- `cargo deny check` — zero new dependencies.
- `cargo sqlx prepare --check` — three new queries (gc DELETE,
  pending SELECT count, undelete UPDATE) added to the offline cache
  in this slice's PR.

Cardinality enforcement of the two new metrics is covered by
the slice-6 D2 cardinality invariant unit test in
`metrics_server.rs` — both new metrics are unlabelled so no extension
needed.

## Earned Trust (principle 12)

No new adapters. The existing `Store::probe()` validates Postgres
reachability + migration version + LISTEN/NOTIFY round-trip + slice-5
migration-0006 columns. The `gc_tombstoned_comments` + `undelete_comment`
+ `count_pending_tombstones` methods ride the already-probed adapter
— no separate probe needed.

The substrate lies relevant to this slice's correctness are exercised
by the acceptance scenarios (DISTILL handoff documents these in detail):

1. **Date arithmetic lie** — rows on both sides of the 90-day boundary
   confirm only the older ones go.
2. **Batch cap lie** — 11,000 ancient tombstones confirm the cap
   stops at 10,000 per tick.
3. **Advisory-lock lie** — two concurrent task spawns confirm exactly
   one does the work.
4. **Transient failure lie** — DB cycled mid-task confirms the task
   survives and the next tick succeeds.
5. **Undelete idempotency lie** — restoring an already-restored
   comment is a zero-return no-op.

These are NOT new probes in production code; they are acceptance
scenarios that probe the contract empirically through the production
driving + driven adapters.

## ADRs created

- `adrs/ADR-015-tombstone-gc-scheduling.md` — captures Q1 + Q2 + Q3 + Q7
  (the four scheduling-related decisions): daily cadence, batched 1000
  with 10k cap, inline in main.rs, log + continue on failure.
  Establishes the "scheduled cleanup task" pattern for future cleanup
  work.
- `adrs/ADR-016-gc-observability-and-admin-undelete.md` — captures Q4
  (emit 2 metrics now) + Q5 (CLI + psql, CLI primary).
- `adrs/ADR-017-comments-visible-view-deferred.md` — captures Q6: the
  `comments_visible` SQL VIEW is intentionally deferred to v0.3 as its
  own slice.
