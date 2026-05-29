# Driver notes — slice-8-deferred-metrics (DISTILL)

How the slice-8 acceptance scenarios drive the system: driving ports,
the observable surface, the test-invocation pattern, and the
fixture/seam inventory. Companion to `wave-decisions.md` +
`coverage-matrix.md` + `step-skeletons.md`.

## 1. Driving ports + the observable surface

Per the Architecture of Reference (project infra policy) the entry
points this slice exercises are:

| Driving port | Kind | How the scenario drives it | Observable surface |
|---|---|---|---|
| `GET /metrics` (sidecar listener) | HTTP pull | `reqwest::Client` scrape via `support/metrics_scrape.rs` | The parsed `ScrapeSnapshot` — the operator's actual Prometheus surface. EVERY metric assertion reads here. |
| 5s pool-poll task (main.rs) | Internal background driver | Indirectly — a write enqueues an outbox row / a token is seeded, the tick reads it, the scrape observes the gauge | Gauge values on the scrape |
| `run_pg_listener` reconnect arm (foundry-realtime) | Internal event driver | Force a real LISTEN drop (restart the scenario's dedicated Postgres); the reconnect arm fires once per drop | `realtime_listen_disconnects_total` on the scrape + subprocess liveness |
| Startup probe sequence (main.rs) | Internal boot driver | Pre-bind METRICS_PORT to force the metrics probe to fail (slice-6 ADR-014 precedent) | Process exit code + `health.startup.refused` log line (refuse-to-start); `probe_failures_total` register-at-0 on the scrape (healthy boot) |
| `run_migrations` at boot (foundry-store) | Internal boot driver | Stage migrations via the slice-4 seam; boot applies them | `migration_apply_duration_seconds` `_count` on the scrape |

**Hexagonal boundary (Mandate 1)**: no scenario instantiates an internal
component (`Store::count_pending_outbox`, the migrator iterator, the
reconnect arm) directly. Every assertion is read at the `/metrics`
driving port (or, for refuse-to-start, at the process boundary —
exit code + log line). Internal components are exercised indirectly,
exactly as the slice-6/7 metric scenarios do.

## 2. Test-invocation pattern (mirrors slice-6/7 subprocess pattern)

Same as slice-6 (handler-instrumentation) and slice-7
(comment-tombstone-gc), NOT the slice-2/5 in-process pattern:

- Each scenario spawns a real `foundry` subprocess (the in-process
  `InProcHarness` deliberately SKIPS `install_recorder()` to avoid the
  "global recorder already installed" panic on the second scenario;
  the `/metrics` substrate requires a real recorder install + real
  sidecar listener, which only the subprocess path provides honestly).
- Per-scenario Postgres schema (slice-1 `fresh_schema_pool_*` rotation).
- Ephemeral `METRICS_PORT` + `FOUNDRY_PORT` (slice-6 pattern), EXCEPT
  the probe-failure WS #8 which deliberately pre-binds `METRICS_PORT`
  to a port already held by the test, forcing the metrics probe to
  fail at boot.
- `METRICS_POOL_POLL_SECONDS=1` cadence override (slice-6 D4) for the
  gauge scenarios so the bounded-poll deadline covers several ticks.

## 2a. The bounded-poll discipline (the load-bearing decision)

The single most important driver decision: **every metric-value
assertion polls; none scrapes once.** Sources:
`docs/evolution/2026-05-28-gc-transient-state-hardening.md` +
`docs/evolution/2026-05-27-slice-6-scenario-hardening.md`.

Why it matters HERE specifically: all 5 slice-8 metrics are
asynchronously updated —
- the two gauges by the 5s poll tick (a scrape can land before the
  first tick),
- the histogram only after `run_migrations` finishes (absent before),
- the disconnect counter only after a reconnect-backoff cycle,
- the probe counter register-at-0 before the first scrape can race the
  bind.

A one-shot scrape after a fixed sleep flakes under `@all` contention
because the test's OWN poll loop is starved of CPU and samples sparsely
(gc-transient-state-hardening Lesson 1). The robust shapes:

| Metric kind | Assertion shape | Helper |
|---|---|---|
| Gauge, terminal value known | `settles to N within S seconds` | `poll_until_sample` (predicate `value == N`, held) |
| Gauge, lower bound only | `eventually at least N within S seconds` | `poll_until_sample` (predicate `value >= N`) |
| Counter (monotonic) | `eventually at least N within S seconds` | `poll_until_sample` (predicate `value >= N`) |
| Counter/gauge transient trajectory | `passes through [v1, v2, ...]` | `poll_until_metric_sequence` (ordered subsequence) — NOT needed by any slice-8 scenario (no non-monotonic transient to catch) |
| register-at-0 | line present immediately (HTTP 200 + contains-line) + `settles to 0 within S seconds` | `scrape_metrics` + `poll_until_sample` |
| Histogram observation count | `_count` line eventually >= N | `poll_until_sample` over the `_count` series / `histogram_observation_count` |

`@serial` (de-contention) is reached for ONLY when a scenario must
observe a transient or force a real event under load — slice-8: the
real-disconnect scenario (#7b) + the probe-failure scenario (#8).
Reach for a robust ASSERTION first, `@serial` second (the evolution
docs warn `@serial` perturbs pool scheduling and once caused
`PoolTimedOut`).

## 2b. Why no `prometheus-parse` dep / no parser change

`support/metrics_scrape.rs` already parses arbitrary
`{name}{labels?} {value}` lines including histogram `_count`/`_sum`/
quantile lines and label blocks with braces in values. The slice-8
metrics introduce no new exposition shape. Per the slice-2/slice-6
"roll our own small parser" justification, no new crate dep and no
parser change. Confirmed by re-reading the module:
`label_keys_for`, `histogram_observation_count`, `sum_for`,
`contains_metric_line`, `samples_for` cover every slice-8 assertion.

## 3. Fixture + seam inventory

| Fixture / seam | Origin | Used by | Production-touching? |
|---|---|---|---|
| Per-scenario PG schema rotation | slice-1 | all | No (test infra) |
| `METRICS_POOL_POLL_SECONDS=1` cadence override | slice-6 D4 | gauge scenarios (#1-#4, #11) | No (existing prod env var; tests set it) |
| Comment-write → outbox-row enqueue | slice-5 + `0003_outbox_notify.sql` | #1 (outbox gauge) | No (real production write path) |
| Bootstrap-token seeding (unclaimed / used / expired) | slice-1 `bootstrap_tokens` table + slice-5 direct-SQL fixture shape | #3, #4 | No (test fixture insert; mirrors slice-7 tombstone_factory direct-SQL approach) |
| `support/test_migration.rs` staging seam | slice-4 | #5, #6, #11 | No (stages into tempdir; prod migrations untouched) |
| Dedicated per-scenario Postgres the scenario can restart | slice-3 multi-replica-harness shape | #7b (real disconnect) | No — and crucially NO new production seam (DD-5; slice-7 deviation #2 honoured) |
| METRICS_PORT pre-bind to force probe failure | slice-6 ADR-014 | #8 (probe-failure WS) | No (test pre-binds a port; prod probe behaviour is unchanged) |
| Startup-log capture (assert `health.startup.refused`) | slice-6 (the log line already emitted by `metrics_server.rs:296`) | #8 | No (reads existing prod log line) |

**Seam policy (DD-5)**: the only place a new production test-only seam
was tempting is the listen-disconnect scenario. Rejected — a dedicated
restartable Postgres forces a REAL drop without a seam, honouring
slice-7 deviation #2 ("prefer no new production test-only seam"). The
slice-3 infra-policy note already records that killing the SHARED
container poisons siblings, which is why the scenario uses a dedicated
DB. If DELIVER finds the harness cannot restart a dedicated PG cleanly,
that is a tracked DELIVER open question (wave-decisions.md), not a
license to add a production seam.

## 4. Driving-adapter verification (RCA-fix P1)

The only externally-invocable user entry this slice touches is the
`GET /metrics` pull surface — exercised by every scenario via a real
`reqwest` scrape (not a direct call to the recorder render function).
The four internal drivers (poll task, reconnect arm, probe sequence,
migration loop) are not user-invocable; they are observed through the
`/metrics` port + the process boundary, which is the honest operator
surface for them. No CLI subcommand or HTTP endpoint is added this
slice, so there is no new driving-adapter subprocess scenario beyond
the scrape itself (contrast slice-7's `foundry doctor restore-comment`,
which DID add a CLI driving adapter).

## 5. Layer + PBT-mode declaration

- All 11 scenarios are layer 3+ (real subprocess + real HTTP scrape +
  real Postgres + real migrations + real LISTEN connection).
- Example-only; NO proptest at this layer (Mandate 9). Sad paths
  (probe failure, no-op migration, already-claimed token) are
  enumerated as named example scenarios (Mandate 11).
- PBT lives at layers 1-2 in DELIVER: the `WHERE notified_at IS NULL`
  count predicate, the `used_at IS NULL AND expires_at > now()`
  predicate, the migrator iterate-and-time loop, the bounded
  `probe_name` register-at-0 set, and the cardinality-key assertion in
  the extended `metrics_server.rs` unit test.
