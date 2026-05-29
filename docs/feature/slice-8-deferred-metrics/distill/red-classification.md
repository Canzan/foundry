# RED classification — slice 8 (slice-8-deferred-metrics)

Per nw-distill § "Pre-DELIVER fail-for-the-right-reason gate" (Rust
adaptation, mirrors the slice-7 form). This file records the EXPECTED
RED classification per scenario, authored at DISTILL time. DELIVER
re-runs the gate after the scaffolds + `.feature` land and confirms the
observed classification matches before entering the GREEN phase
(ADR-025 D2: DELIVER RED phase = unskip these scaffolds + write PBT
unit tests).

## Gate command (Rust adaptation, slice-7 precedent)

```bash
cargo check -p foundry-acceptance --tests          # gate 1: compile (RED, not BROKEN)
cargo test  -p foundry-acceptance --test acceptance -- -t "@slice8"  # gate 2: run @slice8 only
```

Gate 1 (compile) MUST pass once the RED scaffolds land:
- `Store::count_pending_outbox` + `Store::count_unclaimed_bootstrap_tokens` production stubs exist (panic/`unimplemented!` bodies — the calls compile),
- the `slice_8_deferred_metrics.rs` step module registers every NEW phrase with a panic body,
- the World-struct gains its new `Option<...>` fixture fields.

Gate 2 (run): every `@slice8` scenario fails with the canonical Rust
RED scaffold panic from its first slice-8-specific step
(`Not yet implemented -- RED scaffold (DISTILL); DELIVER finishes
this`). The `@slow`/`@serial` scenarios are INCLUDED in the
`-t "@slice8"` gate run (the gate is the RED-classification check, not
the default fast loop — the `@slow` exclusion is a DELIVER concern for
`tests/acceptance.rs`, and the scaffold panics fire identically with or
without the filter).

## Per-scenario expected classification

All 11 scenarios are expected **RED (MISSING_FUNCTIONALITY)** — the
slice-8 production code (the two gauge folds + register-at-0, the
reconnect-arm counter, the probe-failure wrap, the migrator
iterate-and-time loop) and the slice-8 test fixtures (bootstrap-token
direct-SQL seeding, dedicated-DB harness, METRICS_PORT pre-bind,
startup-log capture) are not yet implemented; the step body panics with
the scaffold marker at the first slice-8-specific step. The slice-1
Background + slice-6 inherited "the operator's foundry instance is
running" + scrape steps pass GREEN ahead of the panic (slice-6 DELIVER
has landed).

| # | Scenario | Expected class | First panic at |
|---|---|---|---|
| 1 | outbox gauge reflects comment activity (WS) | RED (MISSING_FUNCTIONALITY) | `Given ... gauge poll cadence set to 1 second` (new `given_foundry_running_with_gauge_cadence`) |
| 2 | outbox gauge scrapable at 0 on fresh instance | RED (MISSING_FUNCTIONALITY) | same cadence Given |
| 3 | unclaimed-token gauge counts only active unclaimed | RED (MISSING_FUNCTIONALITY) | `And an unclaimed admin bootstrap token ... exists` (new `given_unclaimed_unexpired_bootstrap_token`) — or the cadence Given if evaluated first |
| 4 | unclaimed-token gauge drops to 0 after claim | RED (MISSING_FUNCTIONALITY) | cadence Given / token-seed Given |
| 5 | each applied migration records one observation (`@slow`) | RED (MISSING_FUNCTIONALITY) | `Given ... staged with one extra migration` (new `given_foundry_staged_with_extra_migration`) |
| 6 | already-migrated schema records no new observations (`@slow`) | RED (MISSING_FUNCTIONALITY) | `Given ... has already applied its full migration set` (new `given_foundry_already_migrated`) |
| 7a | disconnect counter scrapable at 0 (healthy) | RED (MISSING_FUNCTIONALITY) | `And the scrape body's "realtime_listen_disconnects_total" sample settles to 0 ...` — the line is absent until the production register-at-0 + counter wiring lands (the settles-to-0 poll times out → RED). NOTE: if production isn't wired, `contains the line` already fails first; either way RED. |
| 7b | dropped LISTEN increments counter, recovers (`@serial @slow`) | RED (MISSING_FUNCTIONALITY) | `Given ... running against a dedicated database it can lose` (new `given_foundry_running_against_dedicated_db`) |
| 8 | probe failure + refuse-to-start (WS, `@error @serial`) | RED (MISSING_FUNCTIONALITY) | `Given the metrics port is already bound by another process before boot` (new `given_metrics_port_prebound`) |
| 9 | every known probe scrapable at 0 (all-passing baseline) | RED (MISSING_FUNCTIONALITY) | `And the scrape body contains the line "probe_failures_total"` — line absent until the probe-wrap register-at-0 lands → RED |
| 11 | two labelled metrics carry only their bound; 3 unlabelled carry none | RED (MISSING_FUNCTIONALITY) | `Given ... has already applied its full migration set` / cadence Given |

## Failure-mode categories (expected)

- **MISSING_FUNCTIONALITY** (correct RED): **11 of 11** — every
  scenario panics at its first slice-8-specific step (new Given) OR, for
  the register-at-0 scenarios that reuse the slice-6 scrape Givens,
  fails at the `contains the line` / `settles to 0` Then because the
  production metric is not yet emitted. Both are correct RED: the
  behaviour (metric emission + register-at-0) is unimplemented.
- **IMPORT_ERROR / FIXTURE_BROKEN / SETUP_FAILURE** (wrong RED): **0
  expected**. The test infrastructure is all inherited and sound
  (per-scenario PG schema, testcontainers, slice-1 Background, slice-6
  subprocess + scrape, `metrics_scrape.rs` parser + bounded-poll
  helpers, slice-4 migration-staging seam). The two NEW `Store` count
  methods are production scaffolds (compile, panic at call) — NOT
  imports that fail. If gate 1 surfaces a compile error (e.g. a missing
  `Store` method signature or a World field), that is a DISTILL
  scaffold bug to fix BEFORE handoff, not a DELIVER concern.
- **WRONG_ASSERTION / OBSERVABLE_NOT_AT_PORT** (wrong shape): **0
  expected**. Every assertion is at the operator's observable port: the
  `/metrics` scrape (gauge/counter/histogram values + label sets) and
  the process boundary (exit code + startup log line for the
  refuse-to-start). No scenario reads an internal struct field or
  asserts a method-call count. The bounded-poll shape (never one-shot)
  is applied to every async-updated metric per the gc-transient-state /
  slice-6 hardening lessons.

## Anti-pattern guard — No Fixture Theater

Confirmed at DISTILL time: NO Given step sets up the EXPECTED metric
value. The gauges are moved by REAL production paths (a comment write
enqueues a real outbox row; a real claim transitions the token; a real
migration apply records a real observation; a real PG restart drops the
real LISTEN). The fixtures set PRECONDITIONS (tokens exist, port bound,
migration staged), never the output. A scenario that passed without the
production emitter landing would be a design flaw — the bounded-poll on
an unemitted metric times out → RED, which is the correct signal.

## DELIVER read-back instructions

1. The 11 slice-8 scenarios are all live (no `@skip`/`@ignore`). Each
   fails at its first slice-8-specific step (new Given) or at the
   `contains the line`/`settles to 0` Then for the register-at-0
   scenarios — both are the correct GREEN-phase entry point.
2. Cucumber-rs treats `panic!` from a step body as a step failure with
   the panic message captured. Replace the body verbatim with the real
   implementation; do NOT change the registered phrase (it is the
   DISTILL→DELIVER contract).
3. Most-leveraged first (per wave-decisions § DELIVER pre-flight):
   land sub-deliverable A (`Store::count_pending_outbox` +
   `count_unclaimed_bootstrap_tokens`) + B (fold into the 5s poll +
   register-at-0) → unblocks #1, #2, #3, #4, #11 in one cluster.
4. PHRASE COLLISION CHECK (step-skeletons.md): the slice-8 step module
   MUST NOT re-register the slice-6 phrases (`the operator's foundry
   instance is running`, `the operator scrapes the metrics endpoint
   immediately`, `the scrape returns HTTP 200`, `the scrape body
   contains the line "{}"`, `the scrape body's "{}" sample settles to
   {} within {} seconds`) or the slice-1 Background phrases. Reuse them.
   VERIFY they still resolve before filling new bodies.
5. The `@slow` filter for the migration + real-disconnect scenarios and
   the `@serial` handling are DELIVER's `tests/acceptance.rs` concern
   (slice-7 D3 precedent — one-line filter edit). The scaffold panics
   fire identically with or without the filter; the filter edit does
   NOT change this classification.
6. The two histogram scenarios (#5, #6) depend on the slice-4
   migration-staging seam (`support/test_migration.rs`) which is ALREADY
   implemented — DELIVER reuses it, no new fixture. The dedicated-DB
   restart for #7b is the one open harness question (wave-decisions §
   New open questions for DELIVER #2): confirm `multi_replica_harness`
   can boot against a restartable per-scenario PG before implementing
   #7b; if not, escalate (do NOT add a production seam — DD-5).

Pre-DELIVER gate: **PENDING DELIVER RE-RUN** — this is the projected
classification. DELIVER runs the two gate commands above, confirms
11/11 MISSING_FUNCTIONALITY + 0 wrong-RED, then proceeds.
