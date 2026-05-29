# Wave Decisions — slice-8-deferred-metrics (slice 8)

DESIGN-wave decisions for the slice that closes slice-6 D0's deferred
5-metric gap. **STATUS: ACCEPTED** (2026-05-28) — user confirmed all
six recommended picks (D1–D6) as proposed; no adjustments. The two
honest findings (probe_failures_total scoped to the existing main.rs
startup probes; outbox_pending_jobs forward-compatible `notified_at IS
NULL` semantics ≈ total rows today) carry forward to DISTILL as open
questions. Three ADRs accepted (ADR-018, ADR-019, ADR-020) consolidating
the six decisions into coherent clusters.

This document is the slice-8 handoff artifact for the DISTILL wave
alongside `architecture.md`.

## Decisions (D1 – D6) — recommended picks

| ID | Question | Recommended pick | ADR |
|----|----------|------------------|-----|
| D1 | Where do the two DB-state gauges get refreshed? | **A — Piggyback the slice-6 5s pool-poll loop** (no new task) | ADR-018 |
| D2 | `realtime_listen_disconnects_total` emission site | **A — The `run_pg_listener` `Err(err)` reconnect arm** (single chokepoint) | ADR-019 |
| D3 | Gauge cadence + register-at-0 | **A — Inherit 5s cadence + register-at-0** (no new env var) | ADR-018 |
| D4 | `migration_apply_duration_seconds` timing strategy | **B — Iterate the `Migrator` and time each apply** (per-`migration_id`) | ADR-020 |
| D5 | `probe_failures_total` emission site + scope | **A — Wrap the existing `main.rs` startup probes; `probe_name` ∈ {store, metrics}** | ADR-019 |
| D6 | Label-cardinality bounds for the two labelled metrics | **Bounded + documented** (`migration_id` ≈ file count; `probe_name` ≈ probe count); extends ADR-011, enforced by the cardinality test | (ADR-011 ext.) |

ADR consolidation rationale: D1+D3 form the "DB-state gauge hosting"
cluster (cadence is part of the hosting decision) → ADR-018.
D2+D5 form the "counter-on-event + self-monitoring" cluster (both are
`.increment(1)` at an existing call-site; D5 adds the Principle-9
recursion) → ADR-019. D4 stands alone as the migration-timing
mechanism decision → ADR-020. D6 extends the existing slice-6 ADR-011
cardinality invariant rather than minting a new ADR.

### D1 — Piggyback the 5s pool poll (RECOMMENDED: A)

**Rationale**: two indexed `count(*)` queries every 5s is operationally
free; honours slice-6 D5 / slice-7 D3 "no new task unless cohesion
requires" (three DB-state gauges sharing one read-DB-state loop IS
cohesive); 5s gives crisp deploy-time visibility for
`bootstrap_tokens_unclaimed`. **Rejected**: B (dedicated task) — ceremony
for free queries; C (daily GC tick) — cadence far too coarse for
deploy-time bootstrap visibility. **Captured in**: ADR-018.

### D2 — Reconnect-arm increment (RECOMMENDED: A)

**Rationale**: the `Err(err)` arm of `run_pg_listener` is the single
chokepoint where a drop-and-reconnect is decided; one line; matches
slice-6 ADR-010 counter-at-call-site. **Rejected**: B (observe from
main.rs) — the task reconnects internally, main can't see individual
drops; C (count in `listen_loop`) — risks double-counting with the
outer arm. **Captured in**: ADR-019.

### D3 — Inherit cadence + register-at-0 (RECOMMENDED: A)

**Rationale**: consistent with `db_connections_in_use` (slice-6 D4); no
new env var; both gauges register-at-0 before the poll task runs so
Grafana never shows "no data". **Rejected**: B (per-gauge cadence env
vars) — over-engineering for free shared-tick queries. **Captured in**:
ADR-018.

### D4 — Migrator iteration timing (RECOMMENDED: B)

**Rationale**: the only option satisfying the catalog's `migration_id`
label contract + NFR-MIG-03 per-release latency prediction;
reimplementation risk is bounded because the slice-4
`run_migrations_from_dir` (`lib.rs:1463`) already demonstrates a
hand-rolled migration loop under the `MIGRATION_LOCK_ID` advisory lock —
B extends that proven pattern with a timing observation; only emits for
migrations that actually run (honest semantic). **Rejected**: A (no /
coarse timing) — loses per-migration fidelity the spec + NFR demand;
C (fork sqlx) — new dependency, maintenance hazard. **Captured in**:
ADR-020.

### D5 — Wrap existing startup probes (RECOMMENDED: A)

**Rationale**: closes the gap with the probes that EXIST today
(`Store::probe`, `metrics_server::probe`); bounded code-defined
`probe_name` set; preserves the slice-6 ADR-014 refuse-to-start
posture; the recursive Principle-9 self-monitoring metric is wired to
real probes, not speculative ones. **Rejected**: B (defer) — recreates
the no-emitter gap slice 8 exists to close; C (periodic re-probe task) —
scope creep; clean v0.x evolution that reuses the same counter without
rename. **Captured in**: ADR-019.

### D6 — Bounded labels, documented + enforced (RECOMMENDED)

**Rationale**: `migration_id` is bounded by the compile-time migration
directory (~7 files); `probe_name` by the code-defined probe set (~2).
Neither is request-derived. Both acceptable per ADR-011. Enforced by
extending the `metrics_server.rs` cardinality test. **Captured in**:
ADR-011 extension (no new ADR).

## Reuse Analysis — HARD GATE artifact

Every CREATE NEW is challenged; every EXTEND justified by reuse over
reimplementation (principle 5). Default EXTEND.

| Action | Target | Why | LOC delta |
|---|---|---|---|
| EXTEND | `crates/foundry-store/src/lib.rs` (next to `count_pending_tombstones`, line 1242) | `Store::count_pending_outbox()` (`SELECT count(*) FROM outbox WHERE notified_at IS NULL`, uses `idx_outbox_pending`) + `Store::count_unclaimed_bootstrap_tokens(now)` (`... WHERE used_at IS NULL AND expires_at > $1`). Pure reads, no lock, mirror the slice-7 method. | +~30 |
| EXTEND | `crates/foundry-app/src/main.rs` — slice-6 pool-poll task (lines 196-219) | Fold the two new gauges into the EXISTING 5s tick (D1=A). No new spawn/cadence. Register-at-0 next to line 194. | +~25 |
| EXTEND | `crates/foundry-realtime/src/lib.rs` — `run_pg_listener` Err arm (line 140) | `metrics::counter!("realtime_listen_disconnects_total").increment(1)` before the backoff sleep (D2=A). | +~3 |
| EXTEND | `crates/foundry-app/src/main.rs` — startup probe sequence (lines 356-371) | Wrap `Store::probe()` + `metrics_server::probe()` so `Err` increments `probe_failures_total{probe_name}` before propagating (D5=A). Register-at-0 for {store, metrics}. | +~20 |
| EXTEND | `crates/foundry-store/src/lib.rs` — `run_migrations` (line 1349) | Iterate the Migrator + time each apply → `histogram!("migration_apply_duration_seconds", "migration_id" => ...)` (D4=B). Preserve the `MIGRATION_LOCK_ID` guard. | +~25 |
| EXTEND | `crates/foundry-app/src/main.rs` — register-at-0 block (lines 194, 230-231) | Register the 4 register-able new metrics at 0 (3 unlabelled + the bounded `probe_name` set). Histogram has none. | +~10 |
| EXTEND | `crates/foundry-app/src/metrics_server.rs` — cardinality unit test | Assert `migration_apply_duration_seconds`→`{migration_id}` + `probe_failures_total`→`{probe_name}`; assert no labels leak onto the 3 unlabelled metrics. Enforces D6. | +~30 |
| EXTEND | `observability/grafana-dashboards/foundry-overview.json` | 5 new panels (one per metric), matching existing panel shapes. | +~90 JSON |
| EXTEND | `docs/feature/foundry-backend-mvp/design/system/observability-infra.md` | Mark the 5 catalog rows "shipped slice 8"; close slice-6 D0 deferred list (3 → 0). | +~5 |
| CREATE NEW | none | All work attaches to existing tasks / call-sites / files. No new poll task (gauges piggyback slice-6 poll); no new crate; no new migration. | — |

**Total estimated delta**: ~140 LOC Rust + ~30 LOC test + ~90 lines
dashboard JSON + ~5 lines catalog doc. Comparable to slices 6–7.

## Architecture Summary

- **Pattern**: Layered with strict inward dependency + dependency-
  inversion at the crate boundary (inherited from slice-1 ADR-001).
- **Paradigm**: OOP-flavoured Rust with plain async fns. New emitters
  follow established patterns verbatim (poll-task gauge, counter-at-
  call-site, register-at-0).
- **Components touched**: `foundry-store` (+2 count methods, migration
  timing), `foundry-app::main` (gauge folds + probe wrap + register-at-0),
  `foundry-realtime` (+1 counter line), `metrics_server.rs` (cardinality
  test). `foundry-core` unchanged (I/O-free invariant preserved);
  `foundry-auth` unchanged.
- **Communication**: all emission flows into the existing process-global
  `metrics_exporter_prometheus` recorder; scraped via the existing
  sidecar `/metrics`. No new wire protocols, ports, or integrations.

## Technology Stack

**Zero new dependencies.** `metrics` (MIT/Apache-2.0) + sqlx + tokio +
axum all already present. `metrics` is already a dep of both
`foundry-app` (slice 6) and `foundry-realtime` (slice 6). `cargo deny
check` expected to pass unchanged; AGPLv3-clean graph preserved.

## Constraints Established

These become invariants downstream waves + future slices honour:

1. **New DB-state gauges reuse the nearest existing poll loop**
   (ADR-018). A new gauge over DB state does NOT spawn its own task
   unless its cadence genuinely diverges from the pool poll. When a
   gauge needs a different cadence, promote ALL DB-state gauges into a
   dedicated `gauge_poll` task in the same slice that introduces the
   divergence.

2. **Counter-on-event metrics increment at the single decision
   chokepoint** (ADR-019), never at multiple sites that could
   double-count. `realtime_listen_disconnects_total` lives at the
   reconnect arm; future event counters follow.

3. **`probe_failures_total` is wired to every code-defined probe**
   (ADR-019). When a developer adds a probe (a new `*::probe()` call in
   the startup sequence or a future periodic re-probe task), they MUST
   add its `probe_name` to the register-at-0 set AND increment the
   counter on its failure. The `probe_name` set is bounded + code-
   defined; never request-derived. This is the Principle-9 recursive
   self-monitoring invariant.

4. **`migration_id` + `probe_name` are the only NEW bounded labels**
   (D6 / ADR-011 extension). `migration_id` is bounded by the migration
   directory; `probe_name` by the probe set. Adding any label whose
   value derives from request/row data requires a new ADR (slice-6
   ADR-011 forbidden-labels list remains binding).

5. **`outbox_pending_jobs` uses `WHERE notified_at IS NULL`** —
   forward-compatible. The day a consumer marks `notified_at`, the
   gauge becomes a true backlog measure with no metric rename. Until
   then it equals total outbox rows (a meaningful unbounded-growth
   signal in its own right).

6. **Register-at-0 holds for all new gauges + counters** (slice-6
   ADR-014 / slice-7 precedent). Histograms are exempt (no current
   value).

## Open Questions for DISTILL/DELIVER

(Full text in `proposals.md` § 8.)

1. Histogram bucket boundaries for `migration_apply_duration_seconds`
   (explicit ms→30s buckets vs default summary shape). Recommendation:
   explicit buckets.
2. `outbox_pending_jobs` semantics (`notified_at IS NULL` vs total-rows
   documentation) + `@nfr-obs-03` tagging. Recommendation: keep NULL
   filter + tag `@nfr-obs-03`.
3. Confirm D1 (gauges piggyback the 5s pool poll). If suite-time
   pressure, override poll cadence in tests like slice-6's
   `METRICS_POOL_POLL_SECONDS`.
4. How the probe-failure scenario asserts a refuse-to-start (final
   pre-exit `/metrics` scrape vs the existing `health.startup.refused`
   log line). Recommendation: log line for refuse-to-start.
5. Migration-timing test approach (reuse slice-4 per-scenario schema
   rotation vs unit test on extended `run_migrations`).
6. Suite-time impact of the 6 new scenarios; `@slow` tag for the
   listen-disconnect scenario (slice-7 precedent).

## Constraint contradictions found

**None blocking.** Two notes surfaced for transparency:

1. **The brief points `probe_failures_total` emission at
   `admin_cli.rs` `foundry doctor` probes; those don't exist today.**
   The only probes are the two `main.rs` startup probes. Slice 8 wires
   the counter onto THOSE (D5=A) — honest with the catalog's Principle-9
   intent ("every probe emits the counter on failure"), adjusted to
   reflect where probes actually live. A future `doctor`-probe family
   would reuse the same counter.

2. **`outbox_pending_jobs` measures total rows today** because
   `notified_at` is never written (the outbox is fire-and-forget via the
   COMMIT-time NOTIFY trigger). Not blocking — the gauge is correct
   against its stated purpose and forward-compatible. Documented in
   Constraint 5 + Open Question 2.

## Handoff to DISTILL

Acceptance-designer (DISTILL wave) inherits:

1. `architecture.md` — slice-specific design summary + L2 emission-
   surface Container diagram + per-metric emission mechanism + Earned-
   Trust scenario list.
2. `wave-decisions.md` (this file) — D1–D6 with rationale + Reuse
   Analysis + 6 constraints + 6 open questions.
3. `adrs/ADR-018-deferred-gauges-piggyback-pool-poll.md`.
4. `adrs/ADR-019-counter-call-site-emission-and-probe-self-monitoring.md`.
5. `adrs/ADR-020-migration-timing-via-migrator-iteration.md`.

DISTILL's first task is to author `.feature` files for the 5 substrate-
lie scenarios in `architecture.md` § Earned Trust (one per metric),
resolving the 6 open questions with the acceptance-designer. The two
labelled metrics get a cardinality-bound assertion (extend the slice-6
`metrics_server.rs` test).
