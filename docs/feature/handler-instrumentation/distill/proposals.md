# DISTILL Proposals — handler-instrumentation (slice 6)

Owner: acceptance-designer (DISTILL). Propose-mode pass for the 5 small
DISTILL-bounded open questions enumerated in
`docs/feature/handler-instrumentation/design/wave-decisions.md` § "Open
Questions for DISTILL" (lines 244–281).

This file is the historical reasoning behind each pick recorded in
`wave-decisions.md`. Slice 5 established the precedent of carrying a
proposals.md alongside the final decisions; slice 6 inherits.

## Q1 — Exact `@nfr-*` tag set for slice-6 scenarios

The slice-1 NFR catalogue (`docs/feature/foundry-backend-mvp/discuss/nfrs.md`)
defines:

- `@nfr-obs-01` — structured JSON logs
- `@nfr-obs-02` — `/healthz` + `/readyz`
- `@nfr-obs-03` — Prometheus metrics on a sidecar port (enumerates the
  5 metrics slice 6 ships)
- `@nfr-obs-04` — request IDs
- `@nfr-perf-01..04` — perf budgets (200ms page render; 10MB upload;
  realtime fan-out; multi-replica pool sizing)

The slice-6 "5 metrics are emitted correctly" family is the testable
surface of NFR-OBS-03 (the catalogue text names every one of those 5
metrics). The slice-6 "middleware adds ≤10µs P95 per request" is a NEW
performance contract NOT covered by NFR-PERF-01..04 (PERF-01 is the
200ms ceiling INCLUDING instrumentation; PERF-04 is pool sizing).

| Option | Status | Rationale |
|---|---|---|
| **A. Reuse `@nfr-obs-03` for the 5 metric-emission scenarios + add NEW `@nfr-perf-05` for the middleware overhead budget** | **RECOMMENDED** | NFR-OBS-03 already enumerates the 5 metrics (`http_requests_total`, `http_request_duration_seconds`, `db_connections_in_use`, `sse_subscribers_total`, plus the inherited `foundry_app_startup_total` slice-1 startup counter). The slice-6 scenarios that assert "this metric appears in `/metrics` with this label set after this action" ARE the testable surface of NFR-OBS-03 — same NFR cell, not a new one. The middleware overhead budget (D7 = ≤10µs P95) is genuinely new (PERF-01..04 don't cover instrumentation overhead) and deserves its own row: `@nfr-perf-05`. |
| B. Add new `@nfr-obs-05` for "metric-emission correctness" | DEFERRED | Inventing a sub-tag for what NFR-OBS-03 already specifies inflates the NFR matrix without buying signal. Slice 5 precedent (D1 = A) was the same — reuse the existing matrix cell unless the new contract is genuinely separate. |
| C. Use only inherited tags (no `@nfr-perf-05`) | DEFERRED | Loses the operational link from the slice-6 perf-budget scenario back to a NFR row; future contributors couldn't grep "what NFR governs the middleware overhead?". |

**Pick (recommendation)**: A — `@nfr-obs-03` on the 5 metric-emission
scenarios (one per metric family, plus the cardinality-safety scenario);
NEW `@nfr-perf-05` on the middleware overhead probe scenario (a single
`@manual` scenario that hands off to the criterion microbench called
out in `architecture.md` § "Performance budget" — the cucumber suite
cannot reliably measure 10µs at scenario granularity, so it documents
the contract and points at the microbench). The microbench itself
lands in DELIVER; DISTILL declares the tag and the scenario shape.

The NFR catalogue will need a one-line addition under "NFR-PERF" for
`NFR-PERF-05` (middleware overhead). This is a CROSS-FEATURE edit
(touches `foundry-backend-mvp/discuss/nfrs.md`); flagged for user
override — alternative is to inline the budget contract in the slice-6
wave-decisions.md only and skip the NFR-catalogue back-propagation
until v0.2.

## Q2 — Cardinality enforcement test harness

DESIGN recommendation (architecture.md § "Architecture enforcement"):
unit test in `metrics_server.rs` asserting the middleware emits EXACTLY
`{path, method, status}` and no other label keys.

| Option | Status | Rationale |
|---|---|---|
| **A. Unit test in `metrics_server.rs` (DESIGN recommendation)** | **RECOMMENDED** | Smallest delta (~20 LOC); lives next to the code under test; runs on every `cargo test -p foundry-app`; fails closed on regression (any new label key added to the middleware fails the test before merge). Acceptance-suite coverage is COMPLEMENTARY: the cardinality-safety acceptance scenario (slice-6 scenario 3) hits the parameterized route `/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments` and asserts the emitted `path` label is the template, not the concrete URI — proves the runtime contract; the unit test proves the STATIC contract (no key drift). |
| B. xtask check (`cargo xtask check-cardinality`) | DEFERRED | More visible CI gate, but requires grep-and-parse of `metrics::counter!` invocation sites OR loading the binary and emitting a probe request — both are heavier than the unit test for marginal visibility gain. The slice-1 `cargo xtask check-arch` already covers crate-boundary discipline; an extra xtask for label-key discipline doubles the xtask surface. |
| C. Both unit test AND xtask | DEFERRED | Over-belt-and-braces for slice-1 scale (~800 series); revisit if a future slice introduces a metric family with conditional labels. |

**Pick (recommendation)**: A — unit test in `metrics_server.rs`. The
acceptance scenario (#3) covers the runtime label-value side
(MatchedPath template) while the unit test covers the static label-key
side (only `path`/`method`/`status`). DELIVER writes both.

## Q3 — Probe verifying the middleware actually fired

DESIGN recommendation (`wave-decisions.md` § "Open Questions" #3):
behavioral only — the counter assertion is the substrate-lie probe
(per principle 12); a structural check verifies wiring but not effect.

| Option | Status | Rationale |
|---|---|---|
| **A. Behavioral only — counter sum == N after N requests** | **RECOMMENDED** | Principle 12 ("Earned Trust") application: the probe asserts the OBSERVABLE behavior (the counter incremented), not the structural wiring (tower stack inspection). A structural probe that says "the middleware is in the stack" is a fragile false-positive: the middleware could be in the stack but emit zero counters if a future refactor breaks the emission. The behavioral probe is the substrate-lie probe — it asserts the contract empirically. This is identical to the slice-5 ADR-014 self-scrape probe pattern (asserts `foundry_app_startup_total` LINE present, not just that recorder install was called). |
| B. Structural only — middleware appears in tower stack | REJECTED | False positive risk per above. Also requires introspecting the tower stack which axum doesn't expose ergonomically (would require a custom Layer marker and a layer-walker — adds infrastructure for marginal value). |
| C. Both behavioral AND structural | DEFERRED | Doubles the assertion surface without doubling the signal. If the behavioral probe passes, the middleware fired; if it fails, the message ("counter is 0 but I made 3 requests") is enough for the developer to look at the tower stack. |

**Pick (recommendation)**: A — behavioral only. Slice-6 scenario 4
("Operator scrapes `/metrics` after N HTTP requests …") IS this probe.

## Q4 — `db_connections_in_use` gauge initial value

DESIGN recommendation (`wave-decisions.md` § "Open Questions" #4):
register at 0 at startup; first poll-tick (within 5s) overwrites.
Avoids a 5s window of "metric absent" in Grafana.

| Option | Status | Rationale |
|---|---|---|
| **A. Register at 0 at process start; first poll tick overwrites within 5s** | **RECOMMENDED** | Grafana sees the line immediately after process start — no "metric absent" gap during the 5s before the first tick. The line being 0 is honest (no requests have occurred yet; the pool was minted with `min_connections=0` so `in_use == 0` is true). The poll task runs every 5s; the first tick overwrites the 0 with the real value (or with the same 0 if still no traffic). |
| B. Only emit after the first poll tick | REJECTED | Creates a 5s window where Grafana queries return "metric absent" — that's the EXACT failure mode DESIGN D0 calls out (empty-series state). Defeats the purpose of the slice. |
| C. Eagerly call `Store::pool_stats()` at startup and emit synchronously before the poll task spawns | DEFERRED | Functionally equivalent to A for the first 5s; adds startup-time complexity for no observable benefit (Grafana sees the same "line present, value 0" either way). |

**Pick (recommendation)**: A — register at 0. Slice-6 scenario 6
("Process starts: `db_connections_in_use` is scrapable immediately at
value 0") is this contract.

## Q5 — Poll-task lifecycle on graceful shutdown

DESIGN recommendation (`wave-decisions.md` § "Open Questions" #5):
abort (it's a `tokio::time::interval` spawn; the runtime drops it on
shutdown).

| Option | Status | Rationale |
|---|---|---|
| **A. Abort on shutdown signal — let tokio drop the task** | **RECOMMENDED** | The polling task is purely observational — aborting mid-tick loses at most one gauge update (~5s of staleness). NFR-AVAIL-02 (graceful shutdown) cares about IN-FLIGHT REQUESTS, not about background metric refreshes. The simplest contract: when `axum::serve.with_graceful_shutdown` returns, the runtime drops all background tasks; the polling task drops with everything else. No special-case shutdown wiring needed. |
| B. Run-to-completion of current tick | REJECTED | The current tick takes ~100ns (read `pool.size()` + `pool.num_idle()`); "completion" is essentially instantaneous either way. Special-cased shutdown wiring adds complexity for zero operational benefit. |
| C. Wait for the in-flight scrape to complete before shutdown | REJECTED | The poll task does NOT scrape the sidecar listener — it WRITES to the recorder. The scrape is initiated by Prometheus, which is a separate concern (handled by the sidecar listener's `with_graceful_shutdown` if at all; for slice-1 sidecar this is implicit at process exit). |

**Pick (recommendation)**: A — abort. Slice-6 scenario 9 ("Process
receives SIGTERM: poll task is cleanly dropped, no panic in logs") is
this contract — but this is naturally `@manual` because acceptance
cucumber-rs cannot easily SIGTERM a long-lived subprocess mid-test
without flakiness. The contract is documented; the assertion is the
absence of a panic message in the shutdown log.

## Scenario plan — confirmation of brief's table

The task brief proposed an 8–10 scenario plan. DISTILL refines to
**10 scenarios** with the following composition:

| # | Scenario | Tier / Tags | Notes |
|---|---|---|---|
| 1 | Walking skeleton: Operator scrapes `/metrics` after a single comment POST and sees `http_requests_total` + histogram bucket increment | `@walking_skeleton @real-io @driving_adapter @nfr-obs-03` | The WS — proves the middleware is wired AND the recorder accepts the emissions AND the scrape returns them. |
| 2 | `http_requests_total` correctness: after a mix of GET + POST requests across multiple routes, scraping shows the counter sums correctly broken down by route template + method + status | `@real-io @nfr-obs-03` | Verifies the per-route + per-method + per-status breakdown matches D2's bounded triple. |
| 3 | Cardinality safety: requests to a parameterized route emit the route template `path` label, not the concrete URI; the forbidden-label list is empty in the scrape | `@real-io @nfr-obs-03 @cardinality` | Behavioral probe of ADR-011's invariant (the static probe lives in the metrics_server.rs unit test per Q2 pick A). |
| 4 | Middleware-fired probe: after N requests, the counter sum across all label combinations equals N exactly | `@real-io @nfr-obs-03` | Q3 pick A — the principle-12 substrate-lie probe. |
| 5 | `db_connections_in_use` gauge: after the poll task ticks once (waits 5s + safety margin), the scraped gauge value matches `pool.size() - pool.num_idle()` | `@real-io @nfr-obs-03` | Verifies the polling task spawned + ran + emitted. ~5–7s scenario cost (the dominant per-scenario cost in slice 6). |
| 6 | `db_connections_in_use` startup-register: scrape immediately after process start (before first poll tick) shows the gauge line at value 0 | `@real-io @nfr-obs-03 @startup-register` | Q4 pick A — Grafana sees the line immediately, no metric-absent window. |
| 7 | `sse_subscribers_total` increments and decrements: when a subscriber opens an SSE stream, the gauge increments; when they close cleanly, the gauge returns to baseline | `@real-io @nfr-obs-03 @sse` | ADR-013 RAII Drop probe — the principle-12 substrate-lie probe for the gauge round-trip. |
| 8 | `sse_subscribers_total` Drop correctness: when the SSE stream is abruptly dropped (client disconnect, not graceful close), the gauge still decrements via Drop | `@real-io @nfr-obs-03 @sse @error` | ADR-013 abrupt-drop assertion — the Drop fires uniformly across termination paths. |
| 9 | Startup probe success: process starts, sidecar binds, `/metrics` self-scrape returns 200 + contains `foundry_app_startup_total` line | `@walking_skeleton @real-io @startup-probe @nfr-obs-03` | ADR-014 self-scrape probe — observable as "the process started without crashing AND `/metrics` is reachable". |
| 10 | Middleware overhead budget contract: per-request overhead ≤10µs P95 across 27 routes | `@manual @nfr-perf-05` | The contract is documented; the cucumber suite hands off to the criterion microbench called out in architecture.md. cucumber-rs cannot reliably measure 10µs in a single scenario. |

Two walking skeletons (#1 + #9) because slice 6 has TWO distinct
end-to-end loops: (a) request → middleware → counter → scrape, and
(b) process-start → self-scrape probe → "ready to serve traffic".
The convention says "exactly 1 `@walking_skeleton` per feature file";
two skeletons in one feature file is unusual but the right call here
because each proves a structurally different "is the slice wired
end-to-end" contract. Flagged for user override; alternative is to
demote #9 to `@startup-probe` only (drop `@walking_skeleton`).

The original brief proposed a "Startup probe failure" scenario tagged
`@error @manual` (process starts with `METRICS_PORT=1` → exits non-zero).
DISTILL **defers** this to `@manual` documentation only (not in the
.feature file as an automated scenario) because: (a) cucumber-rs lacks
a clean way to spawn a subprocess that's expected to crash + read its
exit code AND its structured log line, without inheriting all the
slice-3 `assert_cmd`-based subprocess plumbing; (b) the probe shape is
already covered by ADR-014's unit-test plan (failure-injection unit
test against a mocked `PrometheusHandle`). Net: 10 scenarios in the
slice-6 .feature file; the failure-injection probe lives in DELIVER's
PBT phase per ADR-025 D2.

## How slice-6 scenarios run — subprocess vs in-process

Critical observation: the slice-6 acceptance scenarios CANNOT use the
existing in-process `InProcHarness::spawn_app` flow because:

1. `install_recorder()` is process-global (panic on second call); the
   existing harness deliberately SKIPS it (per the comment in
   `crates/foundry-app/src/metrics_server.rs` line 26-27: "The
   acceptance harness does NOT call this (the test app doesn't expose
   `/metrics` and we'd hit 'global recorder already installed' on the
   second scenario)").
2. The slice-6 contract IS the `/metrics` scrape — without the
   sidecar listener bound + the recorder installed, there's nothing
   to scrape.

Two options for slice-6 acceptance, each with trade-offs:

| Option | Pros | Cons | Pick |
|---|---|---|---|
| **A. Subprocess via `assert_cmd::Command::cargo_bin("foundry")` (slice-3 US-03 precedent)** | Real process boundary — exactly what production runs; metrics sidecar binds for real; recorder install is real; matches ADR-014 self-scrape probe contract verbatim | One foundry subprocess per scenario (~500ms startup + Postgres connect); needs `DATABASE_URL` env + ephemeral metrics port allocation; ~5s overhead per scenario × 10 scenarios = ~50s slice cost | **RECOMMENDED** |
| B. Docker-compose harness (slice-1 US-01 precedent) | Closest to production (real container networking) | ~30–60s per scenario; massive slice cost (~5–10 min); recorder still only installs once per process so concurrent scenarios CAN'T share a stack | REJECTED |
| C. New in-process harness variant that calls `install_recorder()` exactly once per `cargo test` process + spawns the sidecar at a process-wide ephemeral port | Fastest; reuses InProcHarness | Requires `OnceCell<PrometheusHandle>` + careful tear-down to avoid scenario bleed; the `metrics::counter!` calls accumulate ACROSS scenarios into the same recorder, so per-scenario reset is impossible; tests would have to assert deltas, not absolutes — adds harness complexity | DEFERRED |

**Pick (recommendation)**: A — subprocess via `assert_cmd::Command::cargo_bin("foundry")`,
one per scenario, with per-scenario PG schema (reusing the slice-1
schema-rotation pattern) + ephemeral `METRICS_PORT` + ephemeral
`FOUNDRY_PORT` + small `support/metrics_scrape.rs` helper that does
`reqwest::get` against the metrics URL and parses the text-exposition
format.

The slice-6 suite-time budget is ~50s — significant but within the
project's overall 60s top-line budget when sharded. The `@walking_skeleton`
+ `@startup-register` + cardinality scenarios can be tagged
`@docker-compose`-style for opt-in slow-lane running if needed; default
is to run them.

## Slice-6 invented detail flags (DISTILL deltas only)

DESIGN's "Decision-driven invented detail" list (architecture.md +
wave-decisions.md lines 285–326) is inherited unchanged. DISTILL adds
these phrasing flags:

1. **Two `@walking_skeleton` scenarios in one feature file (#1 + #9)** —
   slice-1..5 convention is one WS per feature. Justification: slice 6
   has two distinct end-to-end loops (request-path metric flow +
   process-startup-probe flow); each is a structurally different
   "slice wired end-to-end" contract. Flagged for user override —
   alternative: demote #9 to `@startup-probe` only.

2. **`@nfr-perf-05` is a NEW NFR tag** — slice-6 introduces it for the
   middleware overhead contract. Optional back-propagation: add a
   one-line `NFR-PERF-05` row to `docs/feature/foundry-backend-mvp/discuss/nfrs.md`.
   If user prefers no back-propagation: skip the NFR-catalogue edit;
   the slice-6 wave-decisions.md is the authoritative home for the
   tag's contract.

3. **Subprocess test harness via `assert_cmd::Command::cargo_bin("foundry")`** —
   slice-6 introduces a per-scenario foundry subprocess pattern.
   Inherits the slice-3 US-03 precedent exactly (same `assert_cmd`
   crate, already in `Cargo.toml`). Per-scenario PG schema for
   isolation (slice-1 pattern). Ephemeral `METRICS_PORT` + ephemeral
   `FOUNDRY_PORT` per scenario for concurrency. The two scenarios
   that need the slow 5s poll-task tick (#5 + #6) drive the dominant
   suite cost.

4. **Scenario #10 (middleware overhead) is `@manual`** — cucumber-rs
   cannot reliably measure 10µs P95 at scenario granularity. The
   scenario documents the contract + points at the criterion microbench
   that DELIVER writes per architecture.md § "Performance budget".

5. **`support/metrics_scrape.rs` is NEW test-infrastructure** —
   ~80 LOC helper analogous to slice-2's `sse_client.rs`. Does
   `reqwest::get` + parses the Prometheus text-exposition format into
   `MetricFamily { name, type_, samples: Vec<MetricSample { labels,
   value }> }`. NOT taking a dependency on `prometheus-parse` or
   similar — the parsing surface needed is ~30 LOC and avoids a new
   crate dep (same justification slice-2 used for `sse_client.rs`).

6. **Startup-probe failure scenario is DEFERRED to a unit test** —
   the brief proposed it as `@error @manual` in the .feature file;
   DISTILL defers per "How slice-6 scenarios run" above. The
   failure-injection probe (probe against a mocked `PrometheusHandle`
   whose `.render()` returns empty) lives in DELIVER's PBT phase per
   ADR-025 D2 + ADR-014 § Verification line 4.

## Scope confirmation

- Brief proposed 8–10 scenarios → DISTILL ships **10** (top of range).
- All 10 are layer-3+ (real HTTP scrape, real Postgres, real
  subprocess); example-only per Mandate 9; sad paths enumerated
  explicitly per Mandate 11.
- Tier B (`RuleBasedStateMachine`) NOT emitted — slice 6 is
  config-shaped (single-shot metrics emission per request; no chained
  journey of 3+ scenarios). Per Mandate 10 condition not met.
- Suite-time delta ~50s (dominant cost = scenario #5's 5s poll-tick
  wait); within 60s top-line budget when sharded.
- Reuse Analysis HARD GATE: ZERO new ports (the `/metrics` scrape IS
  the existing sidecar listener); ZERO new policy rows in
  `docs/architecture/atdd-infrastructure-policy.md`; ONE new
  test-infrastructure file (`support/metrics_scrape.rs`); ONE new
  step file (`steps/handler_instrumentation.rs`); 4 test-side edits
  (force-link in `tests/acceptance.rs`, module reg in `src/lib.rs`,
  world fields in `src/world.rs`, `support/mod.rs` module reg).
- Production code: NOT TOUCHED per task brief.

## Final Wave Review Gate

Per slice-4/5 wave-decisions.md precedent — the project pattern defers
the 4-reviewer wave-gate to PR time (legacy per-wave file layout).
No in-DISTILL parallel reviewer dispatch. The PR will carry the DESIGN
ADRs + this DISTILL artifact set + DELIVER work for reviewers to
inspect simultaneously.
