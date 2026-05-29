# ADR-020: Migration Timing via Migrator Iteration

## Status
Accepted — 2026-05-28

## Context

Slice 8 ships `migration_apply_duration_seconds` (histogram, label
`migration_id`) from the slice-6 D0 deferred catalog. Its purpose
(`observability-infra.md:162`): "how long migrations take (feeds
NFR-MIG-03 release-notes prediction)" — i.e. per-migration latency that
lets release notes warn operators "migration X took ~Ns on a
reference dataset".

The obstacle: production migration is a single opaque call.
`crates/foundry-store/src/lib.rs:1349`:

```rust
pub async fn run_migrations(pool: &PgPool) -> Result<(), StoreError> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
```

`sqlx::migrate!().run()` acquires the advisory lock, reads
`_sqlx_migrations`, applies each pending migration, and records each —
all internally, with no per-migration hook or callback. To emit a
per-`migration_id` observation we need to see the boundary between
individual migration applies.

Crucially, a proven hand-rolled migration loop already exists in the
same file: `run_migrations_from_dir` (slice 4, `lib.rs:1463`) iterates a
`Migrator`, applies each migration under the `MIGRATION_LOCK_ID`
advisory lock, and returns a `MigrationReport` distinguishing
`applied` from `already_applied`. That is the test-path variant; it
demonstrates the iterate-and-apply pattern is feasible and reviewed.

Quality attributes driving this decision: **observability fidelity
(HIGH)** — the catalog contract demands per-`migration_id` data;
**maintenance hygiene / zero new deps (HIGH)** — no forked sqlx;
**correctness (HIGH)** — the migration loop must remain correct
(advisory lock, version table, ordering) regardless of the timing
addition.

## Decision

**Time each individual migration apply by iterating the `Migrator`'s
ordered migration set and recording one
`migration_apply_duration_seconds{migration_id=...}` histogram
observation per migration that ACTUALLY runs.** Already-applied
migrations are skipped and emit no observation (the honest semantic:
the histogram measures real apply latency, not no-ops).

`run_migrations` is extended (not replaced) to wrap each apply with a
`std::time::Instant` and record the elapsed seconds. The existing
`MIGRATION_LOCK_ID` advisory-lock guard is preserved — the whole set is
applied under the lock exactly as today. The implementation extends the
already-proven slice-4 `run_migrations_from_dir` iterate-and-apply
pattern with the timing observation; software-crafter owns the exact
construction (the design specifies WHAT is timed and labelled, not the
internal loop shape).

`migration_id` is bounded by the number of migration files (currently 7
in `crates/foundry-store/migrations/`), grows by ~1 per schema-changing
slice, and is never request-derived — acceptable per ADR-011 (D6).

NO register-at-0: histograms have no "current value", so there is
nothing to register. The Grafana panel shows "no data" until the first
migration applies. This is acceptable — migrations are a boot-time
one-shot, and the histogram is consulted post-hoc for the NFR-MIG-03
latency prediction, not for live alerting. (This mirrors the slice-6
deferral of `db_connection_wait_seconds`, whose panel also stays empty
until data exists — the "instrument me" precedent.)

The exact bucket boundaries are an open question for DISTILL (default
`metrics-exporter-prometheus` summary/quantile shape vs explicit ms→30s
buckets); the recommendation is explicit buckets spanning sub-ms DDL to
multi-second backfills.

## Alternatives Considered

### A: No timing / coarse whole-set timing only (rejected)
Time the entire `run()` as a single observation (no `migration_id`, or
`migration_id="<all>"`).
- **Pros**: trivial — one `Instant` around the existing call.
- **Cons**: loses the per-migration fidelity the catalog contract
  (`labels: migration_id`) and NFR-MIG-03's per-release prediction
  require. A release adding two migrations couldn't tell the operator
  which one is slow.
- **Rejected because**: diverges from the stable catalog contract; the
  metric would be misleading relative to its documented label.

### B: Iterate the Migrator and time each apply (chosen)
See Decision.

### C: Fork / patch sqlx for a per-migration callback (rejected)
- **Pros**: cleanest in theory — a hook in sqlx's own loop.
- **Cons**: a forked dependency violates "zero new deps" and the
  project's OSS-maintenance hygiene (`cargo deny`, AGPLv3-clean graph,
  no patched upstream to track).
- **Rejected because**: maintenance hazard far outweighs the marginal
  cleanliness; B achieves the same result with project-owned code.

## Consequences

### Positive
- Honours the catalog's `migration_id` contract + NFR-MIG-03.
- Reuses the proven slice-4 iterate-and-apply pattern; bounded
  reimplementation risk.
- Only real applies are timed — the data is honest.
- Zero new dependencies.

### Negative
- Reimplements the apply-loop that `sqlx::migrate!().run()` does
  internally (~25 LOC): acquire lock, check `_sqlx_migrations`, apply,
  record. Risk of subtle divergence from sqlx's loop. Mitigation: an
  acceptance scenario asserts the full migration set still applies
  correctly against a fresh schema (no regression of the slice-1/4
  migration behaviour), and the slice-4 variant already validates the
  pattern.
- Empty panel until the first migration applies. Mitigation: documented
  as expected; histograms can't register-at-0; migrations are boot-time.

### Neutral
- Reversibility: reverting to the opaque `sqlx::migrate!().run()` call
  is a localized change to `run_migrations`; no schema impact.
- Bucket boundaries are deferred to DISTILL — a tuning detail, not a
  structural one.

## Verification

- An acceptance scenario applies the full migration set against a fresh
  per-scenario schema (slice-4 rotation precedent); asserts one
  `migration_apply_duration_seconds` observation per applied
  `migration_id`.
- An acceptance scenario runs migrations against an already-migrated
  schema; asserts ZERO new observations (only real applies are timed).
- A regression assertion: after the timed `run_migrations`, the schema
  is fully applied and `_sqlx_migrations` matches the file set (the
  timing addition did not break the apply loop).
- The extended `metrics_server.rs` cardinality test asserts the
  histogram carries exactly `{migration_id}`.
