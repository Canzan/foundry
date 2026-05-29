# Evolution — slice-8-deferred-metrics (Slice 8)

**Finalized**: 2026-05-28
**Ship commit**: [73aee8f](../../) — "Slice 8: deferred observability metrics (emission + dashboard)"
**Wave coverage**: DESIGN → DISTILL → DELIVER (full nWave, dispatched waves: `/nw:design` propose-mode → `/nw:distill` → direct-TDD DELIVER)

## Feature summary

Closes the 5-metric gap deferred in slice-6 (handler-instrumentation) decision D0:
emits and dashboards the observability metrics whose names slice-1's
`observability-infra.md` reserved but nothing produced. All 11 acceptance
scenarios green; `cargo xtask ci` at **122/122**.

| Metric | Type | Emission |
|---|---|---|
| `outbox_pending_jobs` | gauge | 5s pool-poll loop → `Store::count_pending_outbox()` |
| `bootstrap_tokens_unclaimed` | gauge | same loop → `Store::count_unclaimed_bootstrap_tokens()` |
| `migration_apply_duration_seconds` | histogram `{migration_id}` | `run_migrator_timed` times each applied migration |
| `realtime_listen_disconnects_total` | counter | `run_pg_listener` reconnect arm |
| `probe_failures_total` | counter `{probe_name}` | startup probes (store, metrics) + refuse-to-start |

Fourth slice in the project to traverse the full nWave workflow as distinct
dispatched waves (after slices 5, 6, 7). The first feature started fresh in this
project after the v0.2.0 release was staged.

## Business context

slice-6 shipped the 5 dashboard-referenced metrics and deferred the other 5 (D0)
because no dashboard panel consumed them. Operators need them for backlog
visibility (outbox), deploy-time admin-claim status (bootstrap tokens), per-release
migration-latency prediction (NFR-MIG-03), realtime-connection health, and
Principle-9 probe self-monitoring. This slice adds both the emission and the
Grafana panels so each metric has a consumer.

## Key decisions

### DESIGN (`docs/feature/slice-8-deferred-metrics/design/` — ADR-018/019/020, D1-D6)

- **D1/D3 (ADR-018)** — the two DB-state gauges piggyback the existing slice-6 5s
  pool-poll loop; register-at-0; no new task or env var. Reuse over ceremony.
- **D2/D5 (ADR-019)** — `realtime_listen_disconnects_total` increments at the single
  reconnect chokepoint; `probe_failures_total` wraps the existing startup probes
  (`probe_name ∈ {store, metrics}`), realising the Principle-9 recursion. The brief
  assumed `doctor` probes — those don't exist; the architect corrected scope to the
  real startup probes.
- **D4 (ADR-020)** — `migration_apply_duration_seconds` via Migrator iteration
  (only way to get the `migration_id` label; `Migrator::run` is opaque).
- **D6** — both labels bounded (`migration_id` ≈ file count; `probe_name` ≈ probe
  count); extends slice-6 ADR-011 cardinality test.
- Reuse Analysis: **all EXTEND, zero CREATE NEW** (notably no second poll task).

### DISTILL (`docs/feature/slice-8-deferred-metrics/distill/`)

11 scenarios, **every metric assertion bounded-poll** (`poll_until_sample` /
"settles to" / "eventually reaches" — zero one-shot exact scrapes), `@serial` on the
two load-generating scenarios, `@slow` on the heavy ones, `@nfr-obs-03` reused. This
deliberately baked in the session's hard-won flake lessons (see the slice-6 /
gc-transient-state-hardening / acceptance-all-lane-stabilization evolution docs) so
the scenarios are robust under the `@all` lane from the start.

### DELIVER deviations (back-propagated for next-feature reference)

1. **Migration runner reimplemented as `run_migrator_timed`.** Drives the `Migrate`
   trait directly (ensure-table → dirty-check → checksum-validate → time each apply),
   mirroring sqlx's `run_direct`. **Production `Store::migrate()` keeps the
   `MIGRATION_LOCK_ID` advisory lock** (verified); the unlocked `run_migrations()` is
   the per-scenario isolated-schema test path only. No locking regression.
2. **`Store::probe()` newly wired into the startup sequence.** ADR-019's
   `{store, metrics}` implied both probes run; only `metrics` was wired. Adding the
   store probe (it already existed) makes the `store` series meaningful. Low-risk —
   every boot already reaches Postgres after migrations.
3. **Histogram renders as an exporter summary** (`_count`/`_sum`/`quantile`), matching
   slice-6's `http_request_duration_seconds`, rather than explicit buckets. Scenarios
   assert on `_count`; the dashboard reads `{quantile="0.95"}`. Explicit buckets would
   need a global recorder-builder change and diverge from slice-6.
4. **Migration-timing tested via a fresh-schema boot** (the real 6-migration set
   applies and is timed) rather than injecting an extra migration through the
   `test_migrations_dir` seam — that seam is `test-support`-gated and wiring runtime
   injection into prod would add exactly the test-only seam slice-7 deviation #2
   forbids. Observable contract (≥1 observation, labelled `migration_id`) holds, no seam.
5. **Listen-disconnect via a dedicated restartable testcontainer** (`stop()`/`start()`,
   host-port persists) for a real LISTEN drop — no production seam.

## Verification at HEAD (`73aee8f`)

- **`cargo xtask ci` green end-to-end** (independently re-run): fmt, clippy
  --release -D warnings, build, `--workspace` (excl. acceptance) unit tests,
  cargo-deny, full `@all` acceptance — **122 scenarios / 1085 steps, 0 failures**
  (111 → 122).
- All 11 slice-8 scenarios verified individually + twice in the concurrent `@all`
  suite.
- Migration advisory-locking preserved (production uses locked `store.migrate()`).
- No new crate dependencies (`Cargo.lock` unchanged); crate versions stay 0.2.0.

## Lessons learned

1. **Designing scenarios robust-from-the-start is far cheaper than hardening them
   later.** Slices 6-7 cost ~17 release-mode sweeps of flake triage because their
   metric scenarios used one-shot exact scrapes. Slice 8 wrote every assertion as a
   bounded-poll up front and was green on the first full `@all` run. The lesson
   transferred forward as a DISTILL constraint, not tribal memory.
2. **`Migrator::run` is opaque; the `Migrate` trait is the seam.** Per-migration
   timing required reimplementing the apply loop. The risk (losing the advisory lock /
   checksum validation) was contained by mirroring `run_direct` faithfully and keeping
   the lock at the caller. When reimplementing a library's loop, port its safety checks
   verbatim, not just its happy path.
3. **A correct brief assumption is still an assumption.** The brief named `doctor`
   probes for `probe_failures_total`; they don't exist. The architect caught it in
   pre-flight and rescoped to the real startup probes. Designs should verify named
   touchpoints against the codebase, not the prose.

## Open items

1. **Mutation testing** — not yet run for slice-8. The 11 scenarios + the extended
   cardinality unit test give behavioral coverage, but a mutation pass on the new
   store/emit code (kill-rate ≥ 80%) is a recommended follow-up.
2. **Histogram bucket shape** — if explicit `migration_apply_duration_seconds` buckets
   are wanted later (vs the current summary/quantile rendering), it needs a global
   recorder-builder change applied consistently with the other histogram.
3. **v0.2.0 push** — slice-8 lands AFTER the staged v0.2.0 tag; it is v0.3.0 material
   (a new feature). The v0.2.0 release (held, unpushed) is unaffected.

## Workflow note

Per project convention, the 4-reviewer parallel gate is deferred to PR time.
