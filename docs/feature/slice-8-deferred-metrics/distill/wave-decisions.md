# Wave Decisions — slice-8-deferred-metrics (slice 8, DISTILL)

DISTILL-wave decisions for the slice that ships the 5 deferred
observability metrics from slice-6 D0. Inherits the DESIGN wave
(`../design/architecture.md` + `wave-decisions.md` + ADR-018/019/020,
all ACCEPTED 2026-05-28). This document records the acceptance-test
decisions (DD-1..DD-9), resolves the 6 DESIGN open questions, and
records the pre-scenario reconciliation result.

## Phase-0 facts

- **Language**: Rust (`[lang-mode] rust`) — workspace `Cargo.toml`; the
  cucumber-rs acceptance suite is `crates/foundry-acceptance`.
- **Infrastructure Policy**: `docs/architecture/atdd-infrastructure-policy.md`
  PRESENT → `--policy=inherit`. All ports in scope (HTTP `/metrics`
  scrape, subprocess `foundry` binary, real Postgres per-scenario
  schema, migration-staging seam, multi-replica harness) are already
  recorded from slices 1-7. No new port rows appended this slice.
- **State-delta port**: N/A. The Python `assert_state_delta` /
  `tests/common/state_delta.<ext>` contract is for layer 1-2 in-memory
  tests. This slice is entirely layer 3+ (real subprocess + real scrape
  + real Postgres). Per Mandate 8, layers 4+ MAY use traditional
  assertions; per Mandate 11, layer 3+ sad paths are example-based. The
  project idiom is bounded-poll scrape assertions (`poll_until_sample`,
  `poll_until_metric_sequence`), which ARE the universe-bound
  observable-at-the-port assertions for this suite — the `/metrics`
  endpoint is the operator's observable port.

## Wave-Decision Reconciliation HARD GATE

- DISCUSS dir: **MISSING** (WARN — derive ACs from DESIGN; skip
  story-to-scenario traceability). Matches the slice-6/7 narrow-slice
  pattern: the contract is settled by DESIGN's ADR-018/019/020, not a
  DISCUSS story map. ACs derived from the architecture.md §10
  Earned-Trust scenario list + the per-metric emission mechanisms.
- DEVOPS dir: **MISSING** (WARN — use slice-6/7 acceptance infra
  defaults: shared testcontainers Postgres + per-scenario schema,
  ephemeral METRICS_PORT/FOUNDRY_PORT, default tag lane excludes
  `@slow`/`@docker-compose`/`@manual`).
- DESIGN dir: **PRESENT + ACCEPTED**. Driving ports identified
  (`/metrics` scrape + the 4 internal drivers). Hexagonal boundary
  verifiable. Not blocked.
- DESIGN wave-decisions §"Constraint contradictions found" = "None
  blocking"; D1–D6 internally consistent; the two honest findings
  (probe scope = main.rs probes; outbox semantics = total-rows-today)
  are carried as open questions, not contradictions.

**Reconciliation passed — 0 contradictions.** Proceeded to scenario
writing.

## DISTILL Decisions (DD-1 .. DD-9)

| ID | Decision | Rationale |
|----|----------|-----------|
| DD-1 | **Two `@walking_skeleton` scenarios** (#1 outbox-gauge flow; #8 probe-failure refuse-to-start flow). | Slice-6 (DD-11) + slice-7 precedent: two WS when the end-to-end loops are structurally independent. The gauge-poll loop (write → tick → scrape) and the probe-self-monitoring loop (force probe fail → refuse start → observe) are independent operator-facing demos. |
| DD-2 | **11 scenarios total**: 5 metrics × (≥1 happy + register-at-0/cardinality) + 1 cross-cutting cardinality scenario. Error/edge ratio: 5 of 11 carry `@error`/`@startup-register`/cardinality-bound framing (~45%, meets the 40% bar). | Each metric gets a positive emission scenario; the four register-able metrics each get a register-at-0 scenario; the histogram gets a no-op-honest-semantic scenario; one cross-cutting cardinality scenario covers all 5 at once. |
| DD-3 | **All metric-value assertions are bounded-poll**, never one-shot. Gauges → `settles to N` / `eventually at least N`; counters → `eventually at least N` (monotonic) or `passes through [seq]` (transient); histogram → bounded-poll the `_count` observation count; register-at-0 → line-present-immediately + `settles to 0`. | The #1 hard-won project lesson (gc-transient-state-hardening + slice-6-scenario-hardening evolution docs). All 5 metrics are async-updated; one-shot scrapes flake under `@all` contention. Terminal/monotonic observables survive sampler starvation. |
| DD-4 | **`@serial @slow` on the real-disconnect + the migration scenarios; `@serial` on the probe-failure scenario.** | Per the evolution docs: scenarios that generate sustained activity or force a real reconnect get starved under `@all` and must be de-contended; `@serial` is a scheduling change reached for only when observing a transient/event is unavoidable. `@slow` for Postgres-restart + migration-set-apply (seconds, not ms). |
| DD-5 | **No new production test-only seam for the listen-disconnect.** Force a REAL drop by restarting the scenario's OWN dedicated Postgres, not the shared container. | Honours slice-7 deviation #2 (prefer no new production test-only seam). The slice-3 infra-policy note already establishes that killing the shared container poisons siblings; a dedicated per-scenario Postgres (multi-replica-harness shape) is the clean seam. Resolves Open Question 6. |
| DD-6 | **Split disconnect coverage into two scenarios**: a deterministic default-lane register-at-0 + valid-counter scenario, and the `@serial @slow` real-disconnect scenario. | The real-disconnect is inherently slow/contended; the register-at-0 baseline (line present + settles to 0 + no labels) is cheap and deterministic and covers the "metric is wired and bounded" contract in the default lane. |
| DD-7 | **Reuse the slice-4 migration-staging seam** (`support/test_migration.rs` + `AppState::test_migrations_dir` + `run_migrations_from_dir`) for the two migration-timing scenarios. No touch to `crates/foundry-store/migrations/`. | The seam already stages production migrations + per-scenario extras into a tempdir under the production advisory lock. Resolves Open Question 5 (reuse slice-4 rotation, not a unit test on `run_migrations`). |
| DD-8 | **Outbox gauge asserts ">= N after N writes", never exact pending count.** | DESIGN Constraint 5 / Open Question 2: `notified_at` is never written today, so `outbox_pending_jobs` = total outbox rows. Other slice-1 background activity may enqueue rows; the only stable contract is a floor. Tagged `@nfr-obs-03` (reuse, not a new NFR tag). Resolves Open Question 2. |
| DD-9 | **Reuse the slice-6 `@nfr-obs-03` NFR tag** for all metric-correctness scenarios; reuse `poll_until_sample` + `poll_until_metric_sequence` + the `metrics_scrape.rs` parser verbatim. No new NFR tag, no new support helper, no parser change. | The parser already handles arbitrary gauge/counter/histogram-summary families; the bounded-poll helpers already cover gauge-settles + counter-reaches + transient-sequence. The only NEW step phrases are domain wrappers around existing helpers (see step-skeletons.md). |

## Resolution of the 6 DESIGN open questions

1. **Histogram bucket boundaries** → **Recommend explicit ms→30s
   buckets** (DESIGN's own recommendation; confirmed). The acceptance
   scenarios assert on the `_count` observation count, NOT on specific
   bucket boundaries, so the bucket choice is a DELIVER tuning detail
   that does NOT change any scenario. Recorded as a DELIVER open item
   (note: the `metrics-exporter-prometheus` default renders histograms
   as summaries with `_count`/`_sum`/quantile lines per the slice-6
   `histogram_observation_count` helper comment — DELIVER decides
   whether to configure explicit buckets vs. accept the summary shape;
   either way the `_count` assertion holds).

2. **outbox-pending semantics + `@nfr-obs-03` tagging** → **Keep the
   `WHERE notified_at IS NULL` filter (forward-compatible) and tag
   `@nfr-obs-03`** (DESIGN recommendation; confirmed). The acceptance
   contract is ">= N after N writes" (DD-8), never an exact pending
   count — honest about the total-rows-today semantic.

3. **Confirm D1 (gauges piggyback the 5s pool poll)** → **Confirmed.**
   The two gauge scenarios override the cadence to 1s via the existing
   `METRICS_POOL_POLL_SECONDS` env var (slice-6 precedent) so the
   bounded-poll deadline covers several ticks. No dedicated task; no
   new env var.

4. **Probe-failure refuse-to-start assertion shape** → **Log line +
   exit code** (DESIGN recommendation; confirmed). A process that
   refuses to start cannot reliably serve a final `/metrics` scrape, so
   the WS #8 scenario asserts the `health.startup.refused` log line +
   probe-name mention + non-zero exit. The counter's register-at-0
   (all probes at 0 on a healthy boot) is covered by the separate
   register-at-0 scenario where a live scrape IS reliable.

5. **Migration-timing test approach** → **Reuse the slice-4
   per-scenario migration-staging seam** (DD-7), not a unit test on
   `run_migrations`. The unit-level test of the iterate-and-time loop is
   DELIVER's layer-1-2 responsibility (PBT on the migrator iteration);
   the acceptance scenarios prove the histogram emits at the port.

6. **`@slow`/`@serial` for the listen-disconnect scenario** →
   **`@serial @slow`, real disconnect via a dedicated Postgres restart,
   no new production seam** (DD-4 + DD-5 + DD-6). Split into a cheap
   deterministic register-at-0 baseline (default lane) + the heavy real
   disconnect (`@serial @slow`).

## Reuse Analysis — test-side (HARD GATE)

| Action | Target | Why |
|---|---|---|
| EXTEND | `crates/foundry-acceptance/src/steps/` (new `slice_8_deferred_metrics.rs` step module) | New domain step phrases wrapping the EXISTING bounded-poll helpers (see step-skeletons.md). RED-scaffolded (panic bodies) per Mandate 7 / slice-7 precedent. |
| REUSE | `support/metrics_scrape.rs` (`scrape_metrics`, `poll_until_sample`, `poll_until_metric_sequence`, `ScrapeSnapshot::label_keys_for` / `histogram_observation_count` / `contains_metric_line`) | Parser + bounded-poll helpers handle every assertion shape this slice needs. Zero changes. |
| REUSE | `support/test_migration.rs` (`stage` + `TestMigrationsDir`) | Migration-staging seam for the two histogram scenarios. |
| REUSE | `support/multi_replica_harness.rs` + dedicated per-scenario Postgres | Dedicated DB the listen-disconnect scenario can restart without poisoning siblings. |
| REUSE | slice-1 Background steps (workspace/member/project/issue) + slice-6 subprocess + `METRICS_POOL_POLL_SECONDS` cadence override + slice-6 `METRICS_PORT`-self-bind probe-failure precedent | All inherited. |
| CREATE NEW | none (infrastructure) | Zero new support helpers, zero new NFR tags, zero parser changes. Only new test-only code is the step module + the two `Store` count-method RED scaffolds + the production emitters (DELIVER). |

## DELIVER pre-flight checklist (sub-deliverables)

DELIVER unskips the 11 scaffolds (ADR-025 D2) and implements, most-
leveraged first:

- **A — `Store::count_pending_outbox()` + `Store::count_unclaimed_bootstrap_tokens(now)`** (foundry-store, next to `count_pending_tombstones`). Unblocks the two gauge metrics (4 scenarios). PBT unit: the count predicates.
- **B — Fold the two gauges into the 5s pool-poll loop + register-at-0** (foundry-app main.rs). Unblocks scenarios #1-#4.
- **C — `realtime_listen_disconnects_total.increment(1)` at the reconnect arm + register-at-0** (foundry-realtime + main.rs). Unblocks #7-#8 (disconnect).
- **D — Wrap the startup probes with `probe_failures_total{probe_name}` + register-at-0 for {store, metrics}** (foundry-app main.rs). Unblocks WS #8 + the probe register-at-0 scenario.
- **E — Iterate the Migrator + time each apply** (foundry-store `run_migrations`, ADR-020). Unblocks the two histogram scenarios. PBT unit: the iterate-and-time loop + apply correctness.
- **F — Extend the `metrics_server.rs` cardinality unit test** (D6): assert `migration_apply_duration_seconds`→`{migration_id}`, `probe_failures_total`→`{probe_name}`, no labels leak onto the 3 unlabelled metrics.
- **G — 5 Grafana panels** (`observability/grafana-dashboards/foundry-overview.json`) + annotate the catalog doc (slice-6 D0 deferred list 3→0). Not acceptance-gated; demo-verified.

## New open questions for DELIVER

1. **Histogram render shape**: configure explicit ms→30s buckets in the
   recorder, OR accept the `metrics-exporter-prometheus` default summary
   shape (`_count`/`_sum`/quantile)? The acceptance scenarios assert on
   `_count` (works for both), but the Grafana panel JSON depends on the
   choice. DELIVER decides; if explicit buckets, verify the panel uses
   `_bucket{le=...}` and the scrape parser already handles bucket lines
   (it does — `samples_with_prefix`).
2. **Dedicated-Postgres restart mechanism** for the listen-disconnect
   `@serial @slow` scenario: confirm `multi_replica_harness.rs` (or a
   thin new harness helper) can boot the foundry subprocess against a
   per-scenario Postgres it can `docker restart` / pause without a new
   production seam. If the only honest way needs a seam after all,
   escalate (would revisit DD-5).
3. **Probe-failure counter observability**: WS #8 asserts the refuse-
   to-start via the log line + exit code (Q4). If DELIVER finds a
   non-fatal probe path where the counter increment IS observable on a
   later scrape (ADR-019 §Verification's "non-fatal observation path"),
   add a focused scenario; otherwise the log-line assertion stands.
