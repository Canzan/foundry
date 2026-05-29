# Feature: slice-8-deferred-metrics
# Slice: 8 (closes slice-6 D0's deferred 5-metric gap; ships the
#         remaining catalog metrics with no emitter + one Grafana panel
#         each so the catalog and the dashboard converge for v0.x)
#
# JTBD: outcome-1 (operators stand up Foundry and confirm it's healthy
#       within an hour — empty Grafana panels are a deploy-time
#       correctness failure this slice removes). probe_failures_total is
#       the recursive Principle-9 self-monitoring metric (does the
#       substrate still pass its own honesty checks).
#
# The 5 metrics shipped this slice (per DESIGN architecture.md §0):
#   | Metric                              | Type      | Labels        |
#   | outbox_pending_jobs                 | gauge     | (none)        |
#   | bootstrap_tokens_unclaimed          | gauge     | (none)        |
#   | migration_apply_duration_seconds    | histogram | migration_id  |
#   | realtime_listen_disconnects_total   | counter   | (none)        |
#   | probe_failures_total                | counter   | probe_name    |
#
# Inheritance (slice 6 — handler-instrumentation):
#   - `metrics_exporter_prometheus` recorder + `/metrics` sidecar are
#     already wired. Slice 8 adds 5 EMISSIONS (no new metric families
#     beyond the slice-6 D0 deferred catalog).
#   - Register-at-0 at startup (ADR-014 / slice-6 D4): gauges + counters
#     register at 0 before their emitters run so Grafana never shows
#     "no data". Histograms are EXEMPT (no current value) — the
#     migration histogram panel stays empty until the first apply.
#   - The 5s pool-poll task (ADR-012, main.rs:196-219) is the host for
#     the two new DB-state gauges (D1=A — piggyback, no new task).
#   - `support/metrics_scrape.rs` parser + `poll_until_sample` +
#     `poll_until_metric_sequence` bounded-poll helpers — reused
#     verbatim; no parser change (it already handles arbitrary
#     gauge/counter/histogram-summary families).
#   - `METRICS_POOL_POLL_SECONDS` env-var cadence override (slice-6 D4)
#     shortens the 5s loop to ~1s for the gauge scenarios.
#
# Inheritance (slice 4 — rolling upgrade):
#   - `support/test_migration.rs` stages production migrations into a
#     `tempfile::TempDir` + appends per-scenario extras, handed to
#     `AppState::test_migrations_dir` so the boot path runs
#     `run_migrations_from_dir` under the production advisory lock. The
#     migration-timing scenarios reuse this staging seam — they do NOT
#     touch `crates/foundry-store/migrations/` (which would poison
#     sibling scenarios). Per DISTILL Q5 = A.
#
# Inheritance (slice 5 — comments) + (slice 8 outbox):
#   - The outbox row is enqueued by the COMMIT-time NOTIFY trigger
#     (`0003_outbox_notify.sql`) on a comment/issue write. `notified_at`
#     is never written today (fire-and-forget), so `outbox_pending_jobs`
#     (`WHERE notified_at IS NULL`) equals the total outbox row count.
#     The gauge is correct against its stated purpose and
#     forward-compatible (DESIGN Constraint 5 / Open Question 2). The
#     scenario asserts ">= N after N writes" — never an exact pending
#     count — per DISTILL D2.
#
# Slice-8 driving adapters (per architecture.md §5):
#   - `GET /metrics` on the sidecar listener — Prometheus's pull surface;
#     ALL metric assertions scrape it (the operator's observable surface).
#   - Background 5s pool-poll task in main.rs (the two gauges).
#   - `run_pg_listener` reconnect arm in foundry-realtime (the
#     disconnect counter) — INTERNAL driver, observed via the scrape.
#   - The startup-probe sequence in main.rs (the probe-failure counter +
#     refuse-to-start) — INTERNAL driver, observed via the scrape +
#     the `health.startup.refused` log line + the exit code.
#   - `run_migrations` at boot (the migration histogram) — INTERNAL
#     driver, observed via the scrape.
#
# Driven adapters exercised (ALL reused; ZERO new infrastructure per
# DESIGN § Reuse Analysis — zero new crates / deps / migrations):
#   - Postgres per-scenario schema (slice-1 inherited).
#   - `outbox` + `bootstrap_tokens` tables (slice-1 schema; new pure-read
#     count queries Store::count_pending_outbox /
#     count_unclaimed_bootstrap_tokens land next to count_pending_tombstones).
#   - `metrics_exporter_prometheus` recorder + `/metrics` sidecar
#     (slice-6 wiring). 5 new emissions; no parser change.
#   - `support/test_migration.rs` migration-staging seam (slice-4).
#   - `support/multi_replica_harness.rs` / per-scenario second Postgres
#     for the LISTEN-disconnect scenario (slice-3 inherited).
#   - `reqwest::Client` in scrape direction (slice-1 inherited).
#   - `METRICS_PORT` self-bind precedent for the probe-failure scenario
#     (slice-6 ADR-014: bind the port before boot to force the metrics
#     probe to fail).
#
# Layer / PBT mode declaration (per nw-test-design-mandates Mandate 9):
#   - Layer 3+ (real subprocess + real HTTP scrape + real Postgres +
#     real migrations + real LISTEN connection).
#   - Example-only. No proptest. Sad paths enumerated explicitly per
#     Mandate 11. PBT belongs at layers 1-2 (unit) — DELIVER's
#     responsibility (the `WHERE notified_at IS NULL` count predicate;
#     the `used_at IS NULL AND expires_at > now()` predicate; the
#     migrator iterate-and-time loop; the bounded `probe_name` set
#     register-at-0; the cardinality-key assertion lives in the
#     extended `metrics_server.rs` unit test).
#
# ROBUST METRIC ASSERTIONS (the #1 hard-won lesson — see
# docs/evolution/2026-05-28-gc-transient-state-hardening.md +
# 2026-05-27-slice-6-scenario-hardening.md). All 5 metrics are updated
# ASYNCHRONOUSLY (poll task, boot-time, reconnect event) and the suite
# runs scenarios concurrently. ONE-SHOT exact scrapes of async-updated
# metrics FLAKE. The assertion shapes used below:
#   - Gauges (outbox_pending_jobs, bootstrap_tokens_unclaimed): bounded
#     poll "eventually reaches/settles to N within S seconds" via
#     poll_until_sample. NEVER a one-shot `== N` after a fixed sleep.
#   - Counters (realtime_listen_disconnects_total, probe_failures_total):
#     monotonic — bounded poll "eventually reaches N" (>=). For a
#     transient/non-monotonic trajectory use poll_until_metric_sequence.
#   - register-at-0: assert the metric LINE is present immediately
#     (HTTP 200 + contains-the-line) + the value "settles to 0" (bounded
#     poll), NEVER a racy one-shot `== 0`.
#   - Histogram: assert the `_count` observation count is present per
#     applied migration_id (bounded-poll the count line), and ZERO
#     observations for an already-migrated schema.
#   - Scenarios that GENERATE sustained load/activity to move a metric
#     (force a LISTEN disconnect; fill the outbox under contention) are
#     tagged @serial (cucumber-rs de-contends them) so the test runtime
#     isn't starved — slice-6 db_connections_in_use + slice-7 cap/gauge
#     precedent.
#
# Two `@walking_skeleton` scenarios (mirrors the slice-6 / slice-7
# precedent — flagged as decision-driven invented detail; see
# wave-decisions.md DD-1):
#   - #1 covers the DB-state gauge flow (write enqueues an outbox row →
#     5s poll tick reads it → gauge eventually reflects the count —
#     operator-visible "the outbox-backlog panel lights up").
#   - #8 covers the probe self-monitoring flow (force the metrics probe
#     to fail at boot → process refuses to start → the probe-failure
#     signal is observable — operator-visible "the substrate told me it
#     refused to start and why"). Structurally distinct end-to-end loops.

@slice8 @deferred-metrics @metrics
Feature: The five deferred observability metrics emit so the operator dashboard story is complete — the outbox-backlog and unclaimed-admin-token gauges ride the existing 5s poll, the migration-timing histogram records each applied migration, the realtime-disconnect and probe-failure counters tick at their event call-sites, and every register-able metric starts at zero so Grafana never shows an empty panel
  An operator who runs Foundry and opens the bundled Grafana "Foundry
  Overview" dashboard sees the last five panels resolve to real data:
  the outbox backlog depth, the count of unclaimed admin bootstrap
  tokens, how long each schema migration took, how often the realtime
  LISTEN connection has flapped, and whether any startup self-check has
  failed. The two gauges ride the existing 5-second pool-poll loop; the
  two counters increment at their single event chokepoints; the
  histogram times each migration as it applies. Every register-able
  metric is present at value zero from the first scrape so an empty
  panel always means "no data yet", never "metric never wired".

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team
    And the "Auth v2" project already has issue AUTH-3

  # --- outbox_pending_jobs (gauge) ---------------------------------

  @walking_skeleton @real-io @outbox-gauge @nfr-obs-03
  Scenario: The outbox-backlog gauge reflects the rows enqueued by comment activity
    # Walking-skeleton wiring proof for the DB-state gauge path:
    # a write enqueues an outbox row via the COMMIT-time NOTIFY trigger,
    # the 5s pool-poll tick reads count_pending_outbox, and the gauge
    # eventually reflects it. ">= N after N writes" (not exact) because
    # outbox semantics are total-rows-today (notified_at never written;
    # DESIGN Constraint 5 / Open Question 2 / DISTILL D2): other slice-1
    # background activity may also enqueue rows, so the contract is a
    # floor, never an exact equality.
    Given the operator's foundry instance is running with the gauge poll cadence set to 1 second
    When Mei posts 3 comments on "AUTH-3"
    Then the scrape body contains the line "outbox_pending_jobs"
    And the scrape body's "outbox_pending_jobs" sample is eventually at least 3 within 10 seconds

  @real-io @startup-register @outbox-gauge @nfr-obs-03
  Scenario: The outbox-backlog gauge is scrapable at zero on a fresh instance so the panel never shows no-data
    # register-at-0 contract (ADR-018 / slice-6 D4): the metric line is
    # present immediately so Grafana never shows no-data, asserted by
    # HTTP 200 + contains-the-line. A fresh per-scenario schema has no
    # outbox rows, so the gauge settles to 0 (bounded-poll, not a racy
    # one-shot == 0 — the first poll tick may not have run at the scrape
    # instant).
    Given the operator's foundry instance is running with the gauge poll cadence set to 1 second
    When the operator scrapes the metrics endpoint immediately
    Then the scrape returns HTTP 200
    And the scrape body contains the line "outbox_pending_jobs"
    And the scrape body's "outbox_pending_jobs" sample settles to 0 within 5 seconds

  # --- bootstrap_tokens_unclaimed (gauge) --------------------------

  @real-io @bootstrap-gauge @nfr-obs-03
  Scenario: The unclaimed-admin-token gauge counts only active unclaimed tokens
    # Earned-Trust arithmetic-lie probe (architecture.md §10.2): seed an
    # unclaimed token, a used token, and an expired token; only the
    # active-unclaimed one counts. Bounded poll for "settles to 1"
    # because the gauge is refreshed asynchronously on the 5s tick.
    Given the operator's foundry instance is running with the gauge poll cadence set to 1 second
    And an unclaimed admin bootstrap token that has not yet expired exists
    And a used admin bootstrap token exists
    And an expired admin bootstrap token exists
    Then the scrape body contains the line "bootstrap_tokens_unclaimed"
    And the scrape body's "bootstrap_tokens_unclaimed" sample settles to 1 within 10 seconds

  @real-io @bootstrap-gauge @nfr-obs-03
  Scenario: The unclaimed-admin-token gauge drops to zero once the operator claims admin
    # The chained narrative continuation: from "an unclaimed token
    # exists and the gauge reads 1" (the Given reuses the prior
    # scenario's setup step), claiming admin clears the gauge.
    Given the operator's foundry instance is running with the gauge poll cadence set to 1 second
    And an unclaimed admin bootstrap token that has not yet expired exists
    And the scrape body's "bootstrap_tokens_unclaimed" sample settles to 1 within 10 seconds
    When the operator claims admin with the unclaimed bootstrap token
    Then the scrape body's "bootstrap_tokens_unclaimed" sample settles to 0 within 10 seconds

  # --- migration_apply_duration_seconds (histogram, migration_id) --

  @real-io @migration-histogram @nfr-obs-03 @slow
  Scenario: Each migration that actually applies records one timing observation labelled with its migration id
    # Earned-Trust migration-timing-lie probe (architecture.md §10.5;
    # ADR-020). Applies the full production migration set plus one
    # staged extra against a FRESH per-scenario schema via the slice-4
    # migration-staging seam; asserts the histogram carries an
    # observation count for the applied migrations. @slow: staging +
    # applying the whole migration set against a fresh schema is heavier
    # than a single scrape (slice-7 @slow precedent for migration-heavy
    # scenarios). Histogram has NO register-at-0 (ADR-020) — the line is
    # absent until the first apply, so this scenario bounded-polls the
    # observation count to appear rather than asserting on a fresh boot.
    Given the operator's foundry instance is staged with one extra migration on top of the production set
    When the operator's foundry instance boots and applies its migrations
    Then the scrape body eventually contains a "migration_apply_duration_seconds" observation count of at least 1 within 15 seconds
    And the scrape body's "migration_apply_duration_seconds" samples carry only the label keys "migration_id"
    # Pin the label VALUE, not just the key: the first production migration
    # always applies on a fresh schema, so its `0001_init` stem must appear.
    # A key-only check passes even if `migration_id_label` emits "" or a
    # constant — this line closes that gap (mutation-testing follow-up).
    And the scrape body's "migration_apply_duration_seconds" samples include the migration_id value "0001_init"

  @real-io @migration-histogram @nfr-obs-03 @slow
  Scenario: An already-migrated schema records no new migration-timing observations
    # ADR-020 honest-semantic probe: only migrations that ACTUALLY run
    # are timed. Boot once to apply everything, capture the observation
    # count, boot a second instance against the SAME (already-migrated)
    # schema, and assert the count did not grow.
    Given the operator's foundry instance has already applied its full migration set
    And the migration-timing observation count has been recorded
    When a second foundry instance boots against the already-migrated schema
    Then the scrape body's "migration_apply_duration_seconds" observation count has not grown

  # --- realtime_listen_disconnects_total (counter) -----------------

  @real-io @listen-disconnect-register @nfr-obs-03
  Scenario: The realtime-disconnect counter is scrapable at zero on a healthy instance so the panel never shows no-data
    # register-at-0 contract (ADR-019). On a healthy instance whose
    # LISTEN connection never drops, the counter stays a flat-zero
    # baseline. This is the deterministic, default-lane half of the
    # disconnect coverage (the real-disconnect half is @serial @slow
    # below — DISTILL D6 / Open Question 6). Asserts the line is present
    # immediately + settles to 0; no production test-only seam needed.
    Given the operator's foundry instance is running
    When the operator scrapes the metrics endpoint immediately
    Then the scrape returns HTTP 200
    And the scrape body contains the line "realtime_listen_disconnects_total"
    And the scrape body's "realtime_listen_disconnects_total" sample settles to 0 within 5 seconds
    And the scrape body's "realtime_listen_disconnects_total" samples carry only the label keys ""

  @real-io @listen-disconnect @nfr-obs-03 @serial @slow
  Scenario: A dropped realtime LISTEN connection increments the disconnect counter and the listener recovers
    # Earned-Trust listen-disconnect-lie probe (architecture.md §10.3;
    # ADR-019). Forces a REAL LISTEN drop by restarting the scenario's
    # OWN dedicated Postgres (NOT the shared container — that would
    # poison siblings; slice-3 infra-policy note). No new production
    # test-only seam (slice-7 deviation #2 honoured — DISTILL D6). The
    # counter is monotonic so the assertion is "eventually reaches at
    # least 1" via bounded-poll (immune to reconnect-timing drift), and
    # the listener must survive (subprocess alive + a later scrape still
    # serves). @serial: forcing a reconnect + waiting out the backoff
    # needs the test runtime undivided; @slow: a Postgres restart is
    # seconds, not milliseconds.
    Given the operator's foundry instance is running against a dedicated database it can lose
    When the realtime LISTEN connection is dropped by restarting that database
    Then the scrape body's "realtime_listen_disconnects_total" sample is eventually at least 1 within 30 seconds
    And the foundry subprocess is alive

  # --- probe_failures_total (counter, probe_name) ------------------

  @walking_skeleton @real-io @probe-failure @error @nfr-obs-03 @serial
  Scenario: A startup probe failure increments the probe-failure counter for that probe and the process refuses to start
    # Walking-skeleton + Earned-Trust probe-failure-lie probe
    # (architecture.md §10.4; ADR-019). Binds METRICS_PORT before boot
    # (slice-6 ADR-014 precedent) so the metrics self-scrape probe
    # fails. The process refuses to start (ADR-014 posture preserved):
    # exit non-zero + the health.startup.refused log line. Per DISTILL
    # Q4 = log line, the refuse-to-start is asserted via the log line +
    # exit code (a dying process cannot serve a final scrape reliably).
    # @serial: pre-binding a port + asserting a non-zero exit perturbs
    # sibling port allocation; de-contend it.
    Given the metrics port is already bound by another process before boot
    When the operator's foundry instance attempts to start
    Then the foundry subprocess exits non-zero
    And the foundry startup log mentions "health.startup.refused"
    And the foundry startup log mentions probe failure for probe "metrics"

  @real-io @probe-failure @startup-register @nfr-obs-03
  Scenario: On a healthy instance every known probe is scrapable at zero so the operator sees the full all-passing baseline
    # register-at-0 for the bounded probe_name set (ADR-019). On a
    # healthy instance both known probes register at 0 so the dashboard
    # shows the full probe set as flat-zero lines ("all probes
    # passing"). Bounded-poll settles-to-0; asserts the line is present
    # immediately and the bounded label set is exactly {probe_name} with
    # values drawn from the closed {store, metrics} set.
    Given the operator's foundry instance is running
    When the operator scrapes the metrics endpoint immediately
    Then the scrape returns HTTP 200
    And the scrape body contains the line "probe_failures_total"
    And the scrape body's "probe_failures_total" sample settles to 0 within 5 seconds
    And the scrape body's "probe_failures_total" samples carry only the label keys "probe_name"
    And the scrape body's "probe_failures_total" samples carry only the probe names "store,metrics"

  @real-io @probe-failure @error @nfr-obs-03 @serial
  Scenario: A failing startup store probe refuses to start so a half-migrated deploy never serves
    # Companion to the metrics-probe scenario above — exercises the OTHER
    # wrapped startup probe (ADR-019 / D5) through `record_probe_result`. A
    # schema migrated EXCEPT for the migration-0006 comments columns boots
    # with migrations skipped: the pre-probe bootstrap `workspaces` check
    # still passes, but the `store` probe's column check fails, so
    # `record_probe_result` increments probe_failures_total{probe_name="store"}
    # and the process refuses to start (ADR-014 posture). A dead process
    # cannot serve a final scrape (DISTILL Q4), so the observable contract
    # is the refuse-to-start itself: non-zero exit + the "startup store
    # probe failed" cause in the boot log. Without the wrapper swallowing
    # the failure, the process would instead boot and serve a broken
    # schema. @serial: a bespoke refuse-to-start subprocess + non-zero-exit
    # assertion perturbs sibling timing; de-contend it.
    Given the operator's foundry instance is missing the latest migration's database columns
    When the operator's foundry instance attempts to start without applying migrations
    Then the foundry subprocess exits non-zero
    And the foundry startup log mentions "startup store probe failed"

  # --- cardinality safety (extends slice-6 ADR-011 / D6) -----------

  @real-io @cardinality @nfr-obs-03
  Scenario: The two new labelled metrics carry only their declared bounded label and the three unlabelled metrics carry none
    # D6 cardinality bound (extends slice-6 ADR-011). The structural
    # half lives in the extended metrics_server.rs unit test (DELIVER);
    # this acceptance scenario is the behavioral half — a real scrape
    # confirms migration_apply_duration_seconds carries exactly
    # {migration_id}, probe_failures_total exactly {probe_name}, and the
    # three unlabelled metrics carry no labels. Single-layer bypass on
    # either the unit test or this scenario is caught by the other
    # (architecture.md §10 self-application note).
    Given the operator's foundry instance has already applied its full migration set
    And the operator's foundry instance is running with the gauge poll cadence set to 1 second
    When the operator scrapes the metrics endpoint
    Then the scrape body's "migration_apply_duration_seconds" samples carry only the label keys "migration_id"
    And the scrape body's "probe_failures_total" samples carry only the label keys "probe_name"
    And the scrape body's "outbox_pending_jobs" samples carry only the label keys ""
    And the scrape body's "bootstrap_tokens_unclaimed" samples carry only the label keys ""
    And the scrape body's "realtime_listen_disconnects_total" samples carry only the label keys ""
