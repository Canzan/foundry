# Design proposals — slice-8-deferred-metrics (slice 8)

**Mode**: propose
**Owner (this wave)**: solution-architect (Morgan)
**Status**: AWAITING USER DECISION on D1–D6.
**Predecessor design**: slices 1–7 by inheritance; directly continues
`docs/feature/handler-instrumentation/design/` (slice 6, ADR-010..014)
+ `docs/feature/comment-tombstone-gc/design/` (slice 7, ADR-015..017).
**Layout convention**: legacy per-wave (no `docs/product/`) — matches
slices 4–7.
**ADR numbering**: ADR-018+ (slice 6 = 010..014, slice 7 = 015..017;
the ADR-100s are a separate platform/devops range, unused here).

---

## 0. What this slice is

Closes slice-6 D0's deferred-metrics gap. The slice-1
`observability-infra.md` catalog (lines 159-176) names 5 metrics with
no emitter today (verified: zero code references, zero dashboard
references). Slice 7 already shipped 2 of the originally-deferred
family. This slice ships the remaining 5 (outbox depth, unclaimed
bootstrap tokens, migration latency, LISTEN disconnects, probe
failures) + one Grafana panel each.

---

## 1. Inherited findings

1a. **Two register-at-0 precedents** in `main.rs` (lines 194, 230-231).
All new metrics inherit register-at-0.

1b. **Two poll-loop precedents** in `main.rs`: the slice-6 5s pool-stats
poll (lines 196-219) and the slice-7 daily GC tick that also polls a
gauge (lines 282-349). The DB-state gauges can ride EITHER, or get their
own task. This is the dominant design lever (D1).

1c. **Schema is ready — no migration needed**:
- `outbox` has `notified_at TIMESTAMPTZ` + partial index
  `idx_outbox_pending ... WHERE notified_at IS NULL` (`0001_init.sql:104-111`).
- `bootstrap_tokens` has `used_at` + `expires_at` (`0001_init.sql:85-91`).

1d. **Outbox `notified_at` is never written today** — the COMMIT-time
trigger NOTIFYs but no consumer marks rows. So `WHERE notified_at IS
NULL` currently equals total outbox rows. Flagged for D-? / DISTILL
(see Open Questions); does not block the gauge.

1e. **`run_migrations` is a single opaque `sqlx::migrate!().run(pool)`
call** (`foundry-store/src/lib.rs:1349`) — no per-migration hook. This
shapes D4 (the histogram needs the function to iterate the migrator).

1f. **The realtime reconnect site is exact**:
`run_pg_listener` `Err(err)` arm (`foundry-realtime/src/lib.rs:139-147`).

1g. **No probe-emitting CLI exists** — the only probes are
`Store::probe()` + `metrics_server::probe()` called at startup from
`main.rs`. `probe_failures_total` wraps THOSE, not a new `doctor`
subcommand (the brief mentions `admin_cli.rs` probes; today the
probe call-sites live in the `main.rs` startup sequence — see D5).

---

## 2. Reuse Analysis — HARD GATE

(Verbatim in `wave-decisions.md` § Reuse Analysis; summarised here.)
All five emitters EXTEND existing tasks / call-sites / files. The only
CREATE-NEW candidate is "a dedicated poll task for the two gauges"
(D1 option B) — challenged and rejected in favour of piggybacking the
slice-6 pool poll.

---

## D1 — Where do the two DB-state gauges get refreshed?

**Question**: `outbox_pending_jobs` + `bootstrap_tokens_unclaimed` are
gauges over DB state. What loop polls them? (THE main design lever.)

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Piggyback the slice-6 5s pool-poll loop** | Add the two `SELECT count(*)` reads + `gauge!().set()` calls to the existing pool-stats tick (`main.rs:196-219`). | Smallest delta; zero new `tokio::spawn`; one cadence to reason about; the loop already does "read DB-ish state, set gauge". Operators see all three gauges refresh in lockstep. | Couples three unrelated gauges to one 5s cadence. 5s may be tighter than these slow-moving counts need (cheap, though — two indexed `count(*)` per 5s is negligible). If the pool poll is ever removed, these gauges lose their home. |
| **B. New dedicated `tokio::spawn` poll task** | A second poll loop at its own cadence (e.g. 30s or 60s) for the two gauges. | Decouples cadence from the pool poll; can pick a slower tick for slow-moving counts. | New `tokio::spawn`, new cadence constant, new env var — exactly the ceremony slice-7 D3 + slice-6 D5 said to avoid for a single concern. More to grep. The counts are so cheap that a slower cadence buys nothing. |
| **C. Piggyback the slice-7 daily GC tick** | Fold the two gauge reads into the GC task (which already polls `comments_tombstones_pending`). | Conceptually groups "DB-state gauges". | Daily cadence is far too coarse — `bootstrap_tokens_unclaimed` at deploy time needs sub-minute visibility, not next-day. Rejected on cadence mismatch. |

**Recommendation: A (piggyback the 5s pool poll)**. Rationale: (a) two
indexed `count(*)` queries every 5s is operationally free; (b) it
honours the slice-6 D5 / slice-7 D3 "no new task unless cohesion
requires" discipline — three DB-state gauges sharing one read-DB-state
loop IS cohesive; (c) 5s gives crisp deploy-time visibility for
`bootstrap_tokens_unclaimed`; (d) if a future gauge genuinely needs a
different cadence, B is the clean evolution (promote all DB-state
gauges into a `gauge_poll` task). C is rejected outright on cadence.

**Earned-Trust note**: the gauges read the already-probed `Store`
adapter. If a count query fails, the gauge is simply not updated that
tick (stale value ages out / goes flat — operators alert on flatness),
matching the slice-7 pending-gauge failure semantics (`main.rs:337-347`).

---

## D2 — `realtime_listen_disconnects_total` emission site

**Question**: where does the LISTEN-disconnect counter increment?

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. The `run_pg_listener` `Err(err)` reconnect arm** (`lib.rs:140`) | `.increment(1)` in the branch that logs + backs off on connection error, before the sleep. | Exactly one increment per real connection drop. The branch already exists + already logs; adding a counter is one line. Matches slice-6 ADR-010 "counter at the event call-site". | The realtime crate emits a metric (already does, per slice-6 Cargo.toml — `metrics` is a dep). Couples a metric name into the realtime crate. |
| **B. Wrap `spawn_pg_listener` in main.rs and count restarts there** | main.rs observes the JoinHandle / a channel and counts. | Keeps the metric name out of the realtime crate. | The task never returns on a drop (it reconnects internally); main.rs can't observe individual drops without a new channel — more plumbing for worse fidelity. Rejected. |
| **C. Count inside `listen_loop` on the `None`/error return** | Increment where `try_recv` returns None (`lib.rs:172-181`). | Closest to the literal drop event. | `listen_loop` returns the error UP to `run_pg_listener` which is where the reconnect decision is made; counting in both places risks double-counting. A is the single chokepoint. |

**Recommendation: A (the reconnect arm)**. It is the single chokepoint
where "we observed a drop and will reconnect" is decided. Unlabelled,
1 series, register-at-0 in main.rs.

---

## D3 — Cadence / register-at-0 for the gauges

**Question**: cadence + initial-value handling for the two gauges.

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Inherit the 5s pool cadence + register-at-0** (follows D1=A) | Both gauges register at 0 before the poll task spawns; refreshed every 5s. | Consistent with `db_connections_in_use` (slice-6 D4). No new env var. | 5s cadence inherited, not independently tunable (acceptable — see D1). |
| **B. New env var per gauge for cadence** | `FOUNDRY_OUTBOX_POLL_SECONDS` etc. | Operator tunability. | Over-engineering for free queries on a shared tick; contradicts D1=A. |

**Recommendation: A**. Register-at-0 + ride the existing cadence. No
new env var. (If the user picks D1=B, D3 picks up that task's cadence
constant instead.)

---

## D4 — `migration_apply_duration_seconds` timing strategy

**Question**: `sqlx::migrate!().run(pool)` is one opaque call. How do we
get per-`migration_id` timing?

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Don't emit / coarse whole-set timing only** | Time the entire `run()` call as a single observation with no `migration_id` label (or `migration_id="<all>"`). | Trivial; one `Instant`. | Loses the per-migration fidelity the spec demands (`labels: migration_id`) and that NFR-MIG-03's per-release latency prediction needs. Diverges from the catalog contract. |
| **B. Iterate the `Migrator` and time each apply** | Use `sqlx::migrate::Migrator`'s ordered migration set: for each migration not yet in `_sqlx_migrations`, time its individual apply and record a histogram observation labelled `migration_id`. | Honours the catalog contract exactly. Per-migration latency feeds NFR-MIG-03. Only emits for migrations that ACTUALLY run (already-applied → skipped → no observation), which is the honest semantic. | Reimplements the apply-loop that `sqlx::migrate!().run()` does internally (acquire lock, check version table, apply, record). ~25 LOC; must preserve the existing advisory-lock guard (`MIGRATION_LOCK_ID`). Risk: subtle divergence from sqlx's own loop — mitigated by an acceptance scenario asserting the migration set still applies correctly. |
| **C. Fork/patch sqlx for a per-migration callback** | Carry a patched sqlx with a timing hook. | Cleanest in theory. | New (forked) dependency; violates "zero new deps" + OSS-maintenance hygiene. Rejected hard. |

**Recommendation: B (iterate the Migrator, time each apply)**. It is
the only option that satisfies the catalog's `migration_id` label
contract and NFR-MIG-03. The reimplementation risk is bounded — the
existing `run_migrations_from_dir` (slice-4, `lib.rs:1463`) ALREADY
demonstrates a hand-rolled migration loop under the advisory lock that
produces a `MigrationReport`; B extends the same proven pattern with a
timing observation. No register-at-0 (histograms have no current
value); the panel is empty until the first apply, which is fine for a
boot-time one-shot consulted post-hoc.

**Earned-Trust note**: scenario asserts one observation per applied
`migration_id` and zero for an already-migrated schema — proves the
"only real applies are timed" contract.

---

## D5 — `probe_failures_total` emission site + scope

**Question**: which probes increment this counter, and where?

Finding 1g: there is no `doctor` probe subcommand today. The probes
are `Store::probe()` + `metrics_server::probe()`, called from the
`main.rs` startup sequence (lines 356-371). The brief's pointer to
`admin_cli.rs` reflects the catalog's INTENT ("every probe emits the
counter on failure"); the concrete probe call-sites are in `main.rs`.

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Wrap the existing `main.rs` startup probes** | On each probe `Err`, `.increment(1)` with `probe_name` ∈ `{store, metrics}` before the error propagates (process still refuses to start per slice-6 ADR-014). Register-at-0 for both names at startup. | Wires the recursive self-monitoring metric onto the probes that EXIST today. Bounded `probe_name` set (code-defined). Honours the slice-6 "refuse to start" posture. The final pre-exit `/metrics` (and the next replica's scrape) records the failure. | The counter increments on a process that's about to exit — the value is observed by the NEXT scrape or the next replica, not the dying one (acceptable; the dashboard signal is "a probe failed recently", and replicas restart-loop loudly). |
| **B. Defer until a `doctor`-probe subcommand family exists** | Don't emit yet; wait for a future probe CLI. | No speculative wiring. | Re-creates exactly the "metric with no emitter" gap slice 8 exists to close. The catalog explicitly ties this counter to the EXISTING probe pattern (Principle 9). Rejected. |
| **C. Also emit from a periodic probe-rerun task** | A background task re-runs probes every N seconds and counts failures live. | Live "are probes passing right now?" signal. | New background task + new failure mode (a probe that's fine at boot but flaps later) — scope creep beyond "emit the deferred metric". A future slice can add periodic re-probing; slice 8 wires the counter onto the boot probes. |

**Recommendation: A (wrap the existing startup probes; `probe_name` ∈
{store, metrics})**. It closes the gap with the probes that exist, keeps
the label set bounded + code-defined, and preserves the slice-6 refuse-
to-start posture. C is a clean v0.x evolution (periodic re-probing)
that reuses the same counter without a rename.

---

## D6 — Label-cardinality bounds for the two labelled metrics

**Question**: confirm + document the bound for `migration_id` and
`probe_name` (slice-6 ADR-011 invariant).

| Metric | Label | Bound | Source of bound |
|---|---|---|---|
| `migration_apply_duration_seconds` | `migration_id` | = number of migration files (currently 7); grows by ~1 per schema-changing slice | The migration set is a compile-time-fixed directory (`crates/foundry-store/migrations/`). Cannot be influenced by request data. |
| `probe_failures_total` | `probe_name` | = number of code-defined probes (currently 2: `store`, `metrics`) | Closed set defined in `main.rs`; grows only when a developer adds a probe (each addition is a code review). Never request-derived. |

**Recommendation**: both bounds are acceptable per ADR-011 (bounded,
code-controlled, not request-derived). Document in `wave-decisions.md`
as a constraint and enforce with the extended `metrics_server.rs`
cardinality test (assert exactly `{migration_id}` / `{probe_name}`
respectively; assert no labels on the three unlabelled metrics). No new
ADR — this extends ADR-011. Forbidden: ever labelling these with a
value derived from request/row data.

---

## 3. Proposed ADRs

| ADR | Title | Captures |
|---|---|---|
| ADR-018 | Deferred DB-state gauges piggyback the pool poll | D1 + D3 |
| ADR-019 | Counter call-site emission + probe self-monitoring | D2 + D5 |
| ADR-020 | Migration timing via Migrator iteration | D4 |

D6 extends ADR-011 (no new ADR).

---

## 4. External integration check (principle 10)

No new external integrations. Prometheus/Grafana are existing PULL
consumers. No contract test annotation needed.

---

## 5. Architecture enforcement (principle 11)

`cargo xtask check-arch` + `cargo deny check` + `cargo sqlx prepare
--check` (two new queries) + extended cardinality unit test. No new
tooling.

---

## 6. Earned Trust (principle 12)

No new adapters/ports. Five acceptance scenarios (one per metric)
probe the substrate lies — see `architecture.md` § Earned Trust.
`probe_failures_total` is itself the probe-that-verifies-probes; its
enforcement is the cardinality test (structural) + the probe-failure
scenario (behavioral).

---

## 7. Decision-driven invented detail (FLAGGED for user override)

1. **Gauge cadence = inherited 5s pool poll** (D1=A / D3=A). Alternative:
   dedicated task at 30/60s (D1=B). Override if operators want gauge
   cadence decoupled.
2. **`outbox_pending_jobs` semantics = `WHERE notified_at IS NULL`**
   (currently == total rows, since nothing marks `notified_at`).
   Alternative: document as "total outbox rows" or have slice-8 also
   start writing `notified_at`. Recommended: keep the NULL filter
   (forward-compatible). See Open Questions.
3. **`probe_name` set = `{store, metrics}`** (the two existing boot
   probes). Grows only by code change.
4. **`migration_id` label value** — proposed: the sqlx migration
   version number (or `version_description`). DISTILL/DELIVER picks the
   exact string; bounded either way.
5. **Histogram bucket boundaries** — NOT specified here;
   `metrics-exporter-prometheus` default summary/quantile shape is used
   unless DISTILL specifies explicit buckets (see Open Questions).
6. **No register-at-0 for the histogram** — histograms have no current
   value; panel shows "no data" until first migration applies
   (boot-time one-shot; acceptable).

---

## 8. Open Questions for DISTILL/DELIVER

1. **Histogram bucket boundaries for `migration_apply_duration_seconds`.**
   Migrations range from sub-ms (DDL) to potentially seconds (data
   backfill). Should slice-8 define explicit buckets (e.g.
   `[0.001, 0.01, 0.1, 0.5, 1, 5, 30]`) or accept the
   `metrics-exporter-prometheus` default summary shape (slice-6
   deviation #2 noted histograms render as summaries by default)?
   Recommendation: explicit buckets spanning ms→30s; DISTILL confirms.

2. **`outbox_pending_jobs` semantics + `@nfr-obs` tagging.** Because
   `notified_at` is never written today, the gauge == total outbox
   rows. Ship as `WHERE notified_at IS NULL` (forward-compatible)? And
   should the outbox-depth scenario ride `@nfr-obs-03` (slice-6 catalog
   tag) like the other metric-correctness scenarios? Recommendation:
   yes to both. DISTILL confirms.

3. **Gauge poll cadence (D1).** Confirm the two gauges piggyback the 5s
   pool poll vs a dedicated cadence. This is the main lever. If DISTILL
   surfaces a suite-time concern, the acceptance scenarios can override
   the poll cadence the same way slice-6 does via
   `METRICS_POOL_POLL_SECONDS`.

4. **`probe_failures_total` observability of a refuse-to-start failure.**
   The counter increments just before the process exits. Does DISTILL
   want the scenario to assert via (a) the final pre-exit `/metrics`
   scrape, or (b) the `health.startup.refused` log line that already
   exists (`metrics_server.rs:296`)? Recommendation: assert the log
   line for the refuse-to-start path + the counter for a non-fatal
   probe path if one is added later. DISTILL decides.

5. **Migration-timing test approach.** The histogram only emits on real
   applies. The acceptance scenario needs a fresh schema to observe
   non-zero observations (slice-4 per-scenario schema rotation is the
   precedent). DISTILL confirms whether to reuse that harness or assert
   via a unit test on the extended `run_migrations`.

6. **Suite-time impact.** Five new metric-correctness scenarios + one
   listen-disconnect scenario (which restarts/severs Postgres). The
   disconnect scenario may be slow; consider an `@slow` tag (slice-7
   open-question-3 precedent). DISTILL profiles.

---

## 9. Quality-gate self-check before user decisions

- [x] Requirements traced to components — 5 catalog metrics →
  store methods + pool-poll extension + realtime arm + startup probes +
  run_migrations timing.
- [x] Component boundaries respected — no new crates/files.
- [x] Technology choices justified — zero new deps.
- [x] Quality attributes addressed — observability completeness,
  operational simplicity, bounded cardinality, self-monitoring, perf.
- [x] Dependency-inversion compliance — main → store → PG; realtime →
  PG; no reverse deps.
- [x] C4 diagrams — L1/L2 inherited from slice 1; L2 emission-surface
  Container diagram provided in architecture.md.
- [x] Integration patterns specified — reuse existing poll/counter
  patterns; no new wire surfaces.
- [x] OSS preference validated — no new deps.
- [x] AC behavioural — all options framed around WHAT the system emits.
- [x] External integrations — none new (principle 10).
- [x] Architectural enforcement — existing tooling + extended
  cardinality test (principle 11).
- [ ] Peer review — DEFERRED until user picks D1–D6 and architecture.md
  is finalized.
