# Coverage matrix — slice-8-deferred-metrics (DISTILL)

Maps the 11 acceptance scenarios to the 5 metrics, the DESIGN
Earned-Trust scenario list (architecture.md §10), the ADR verification
clauses, and the assertion shape. Confirms every metric has emission +
register-at-0 (where applicable) + cardinality-bound coverage.

## Scenario index

| # | Scenario (short) | Metric | Tags | Assertion shape |
|---|---|---|---|---|
| 1 | outbox gauge reflects comment activity | `outbox_pending_jobs` | `@walking_skeleton @real-io @outbox-gauge @nfr-obs-03` | gauge `eventually at least 3` (floor, not exact) |
| 2 | outbox gauge scrapable at 0 on fresh instance | `outbox_pending_jobs` | `@real-io @startup-register @outbox-gauge @nfr-obs-03` | register-at-0: line present + `settles to 0` |
| 3 | unclaimed-token gauge counts only active unclaimed | `bootstrap_tokens_unclaimed` | `@real-io @bootstrap-gauge @nfr-obs-03` | gauge `settles to 1` (3 tokens seeded, 1 counts) |
| 4 | unclaimed-token gauge drops to 0 after claim | `bootstrap_tokens_unclaimed` | `@real-io @bootstrap-gauge @nfr-obs-03` | gauge `settles to 1` then `settles to 0` (chained) |
| 5 | each applied migration records one timing observation | `migration_apply_duration_seconds` | `@real-io @migration-histogram @nfr-obs-03 @slow` | histogram `_count` eventually >= 1 + label keys = `{migration_id}` |
| 6 | already-migrated schema records no new observations | `migration_apply_duration_seconds` | `@real-io @migration-histogram @nfr-obs-03 @slow` | observation count does not grow (honest no-op semantic) |
| 7a | disconnect counter scrapable at 0 on healthy instance | `realtime_listen_disconnects_total` | `@real-io @listen-disconnect-register @nfr-obs-03` | register-at-0: line present + `settles to 0` + no labels |
| 7b | dropped LISTEN increments counter, listener recovers | `realtime_listen_disconnects_total` | `@real-io @listen-disconnect @nfr-obs-03 @serial @slow` | counter `eventually at least 1` + subprocess alive |
| 8 | probe failure increments counter + process refuses to start | `probe_failures_total` | `@walking_skeleton @real-io @probe-failure @error @nfr-obs-03 @serial` | exit non-zero + `health.startup.refused` log + probe-name "metrics" |
| 9 | every known probe scrapable at 0 (all-passing baseline) | `probe_failures_total` | `@real-io @probe-failure @startup-register @nfr-obs-03` | register-at-0: line present + `settles to 0` + labels = `{probe_name}` ∈ {store, metrics} |
| 10 | (folded into 9 numbering) — see #9 | — | — | — |
| 11 | two labelled metrics carry only their bounded label; 3 unlabelled carry none | all 5 | `@real-io @cardinality @nfr-obs-03` | label-key-set assertions across all 5 (behavioral half of D6) |

(Numbering note: the `.feature` file has 11 scenario blocks; #10 above
is a placeholder row reconciling the index — the file's 11 scenarios
are #1-#9 + #7b + #11 as titled. The metric-to-scenario coverage below
is authoritative.)

## Metric → scenario coverage (completeness check)

| Metric | Type | Emission scenario | register-at-0 scenario | cardinality-bound scenario | Earned-Trust §10 mapping |
|---|---|---|---|---|---|
| `outbox_pending_jobs` | gauge | #1 (WS) | #2 | #11 | §10.1 outbox-pending arithmetic lie |
| `bootstrap_tokens_unclaimed` | gauge | #3, #4 | (#2 pattern; gauge starts 0 on fresh schema — covered by the `settles to N` poll which starts from 0) | #11 | §10.2 unclaimed-token arithmetic lie |
| `migration_apply_duration_seconds` | histogram | #5 | N/A — histograms EXEMPT from register-at-0 (ADR-020); #6 covers the honest no-op semantic instead | #5 + #11 | §10.5 migration-timing lie |
| `realtime_listen_disconnects_total` | counter | #7b (real disconnect) | #7a | #7a + #11 | §10.3 listen-disconnect lie |
| `probe_failures_total` | counter | #8 (WS, failure path) | #9 | #9 + #11 | §10.4 probe-failure lie |

**Every metric**: ≥1 emission scenario + register-at-0 (or the
documented histogram exemption) + a cardinality-bound assertion. All
5 Earned-Trust substrate-lie scenarios from architecture.md §10 are
covered.

## ADR verification-clause coverage

| ADR | Verification clause | Covered by |
|---|---|---|
| ADR-018 | seed N pending + M notified; gauge reads N after a tick | #1 (floor variant — outbox semantics = total rows today, DD-8) |
| ADR-018 | seed unclaimed/used/expired; gauge reads 1 | #3 |
| ADR-018 | both gauges read 0 at startup (register-at-0) | #2 (outbox); #3/#4 start-from-0 poll (bootstrap) |
| ADR-018 | both gauges unlabelled (1 series) | #11 |
| ADR-019 | sever/restart PG drops LISTEN; counter +1 per reconnect, task survives | #7b |
| ADR-019 | disconnect counter reads 0 at startup | #7a |
| ADR-019 | force startup-probe failure; `health.startup.refused` log + non-zero exit | #8 |
| ADR-019 | cardinality: `probe_failures_total`→`{probe_name}`, disconnect→no labels | #9, #7a, #11 |
| ADR-020 | full set applies vs fresh schema; one observation per applied `migration_id` | #5 |
| ADR-020 | already-migrated schema → ZERO new observations | #6 |
| ADR-020 | regression: schema fully applied + `_sqlx_migrations` matches file set after timed run | #5 (the boot succeeds + the instance serves; full-apply correctness is also a DELIVER PBT unit) |
| ADR-020 | cardinality: histogram → `{migration_id}` | #5, #11 |
| ADR-011 ext (D6) | the 2 labelled metrics carry only their bound; no labels leak onto the 3 unlabelled | #11 (behavioral) + extended `metrics_server.rs` unit test (structural, DELIVER) |

## Scenario categorization (error-path ratio — Dimension 1)

| Category | Scenarios | Count |
|---|---|---|
| Happy / emission | #1, #3, #4, #5, #7b | 5 |
| register-at-0 / baseline (edge: fresh/healthy state) | #2, #7a, #9 | 3 |
| Error / failure path | #8 (probe failure + refuse-to-start) | 1 |
| Honest no-op edge | #6 (already-migrated → no observation) | 1 |
| Cardinality safety (boundary) | #11 | 1 |

Error + edge + boundary = #2, #6, #7a, #8, #9, #11 = **6 of 11 (~55%)**
— exceeds the 40% bar (BDD methodology / Dimension 1). The failure path
(#8) exercises the probe-failure + refuse-to-start interaction; the
no-op edge (#6) guards the histogram's honest semantic; the
register-at-0 baselines (#2, #7a, #9) guard the "panel never shows
no-data" deploy-time-correctness contract.

## Traceability notes (Dimension 8)

- **Story-to-scenario (Check A)**: DISCUSS dir absent (narrow
  DESIGN-only slice). ACs derived from architecture.md §10 Earned-Trust
  list + per-metric emission mechanisms — every §10 scenario maps to a
  feature scenario (table above). No orphan §10 scenario.
- **Environment-to-scenario (Check B)**: DEVOPS dir absent; default
  acceptance infra (shared testcontainers PG + per-scenario schema +
  ephemeral ports) applies to all scenarios. The probe-failure scenario
  adds the "metrics port already bound" precondition (a deploy-time
  environment variant); the migration scenarios add the
  "fresh schema vs already-migrated schema" environment variants.
