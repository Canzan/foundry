# Wave Decisions — handler-instrumentation (Slice 6)

DISTILL-wave decisions that gate DELIVER. Finalized 2026-05-25 from the
staged DISTILL pass after user landed picks on D1–D5 from
`proposals.md`. Slice 6 inherits slice-1/2/3/4/5 patterns verbatim per
the project's Architecture of Reference at
`docs/architecture/atdd-infrastructure-policy.md` and adds only the
deltas listed below. Structural template follows slice 5's
`docs/feature/comment-edit-delete/distill/wave-decisions.md`.

## Strategy: C (all real adapters) — inherited

Slice 6 inherits Strategy C from slices 1–5 per
`docs/architecture/atdd-infrastructure-policy.md` (mode = `inherit`).
**No new policy rows needed** — DESIGN wave-decisions.md § Reuse
Analysis records ZERO new ports. Every surface the slice-6 scenarios
exercise was already recorded by slice 1/3 or is internal to existing
crates:

- HTTP API driving port — `reqwest::Client` against the real `foundry`
  subprocess (policy "Driving" row 1; same surface as slice 1+2+5)
- Real Postgres per-scenario schema rotation — `PgPool` against the
  shared `testcontainers-rs` Postgres-16 container (policy "Driven
  internal" row 1)
- The `/metrics` scrape uses the EXISTING sidecar listener
  (`crates/foundry-app/src/metrics_server.rs`, slice-1 DEVOPS commit
  c7cb715) — already a real adapter, no new policy row
- Subprocess invocation via `assert_cmd::Command::cargo_bin("foundry")`
  reuses the slice-3 US-03 precedent (policy "Driving" row 2; no new
  dep)

What IS new for slice 6 is the test-side **invocation pattern**: the
acceptance suite spawns a real `foundry` SUBPROCESS per scenario
(slice-3 US-03 precedent) because the in-process `InProcHarness`
deliberately SKIPS `install_recorder()` to avoid the "global recorder
already installed" panic on the second scenario. The subprocess pattern
is a slice-6 deviation from the slice-2/5 in-process default; it is
the only honest way to assert against the `/metrics` substrate. New
test-infrastructure helper `support/metrics_scrape.rs` (~133 LOC) was
added in the prior DISTILL pass — analogous to slice-2's
`sse_client.rs`, registered in `support/mod.rs`. No new policy rows.

The middleware is internal to the router. The pool poller is internal
to `main.rs`. The `SubscriberGauge` is internal to `foundry-realtime`.
None of these surface a new port.

## Tier composition: Tier A only — Mandate 10 condition not met

10 automated scenarios, **none chained** (each spawns a fresh
`foundry` subprocess + own ephemeral `METRICS_PORT` + own
per-scenario PG schema; preconditions re-established per scenario
rather than chained from the previous scenario's state). No
domain-rich input space (the inputs are "make an HTTP request" and
"scrape /metrics" — config-shaped, not domain-shaped). Mandate 10's
≥3-chained + domain-rich threshold is not crossed. **Tier B is NOT
emitted.**

## PBT input mode: example-only — Mandate 9 layer constraint

All 10 scenarios run at layer 3+ (real subprocess, real HTTP scrape,
real Postgres). Per Mandate 9, layer 3+ tests are example-only. No
proptest, no generated inputs. Sad paths (scenario 8 — abrupt SSE
drop; scenario 7 — clean SSE close decrement) are named examples per
Mandate 11.

## ADR-style decision table (D1–D5 finalized)

### D1 — NFR tag set for slice-6 scenarios

| Option | Status | Rationale |
|---|---|---|
| **A. Reuse existing `@nfr-obs-03` for the 5 metric-emission scenarios + ADD NEW `@nfr-perf-05` for the middleware overhead budget** | **CHOSEN** | NFR-OBS-03 already enumerates the 5 metrics slice 6 ships; the slice-6 "metrics are emitted correctly" scenarios ARE the testable surface of that NFR cell — not a new one. The middleware overhead budget (D7 = ≤10µs P95) is genuinely new (NFR-PERF-01..04 don't cover instrumentation overhead) and earns its own tag: `@nfr-perf-05`. Slice 5 D1 = A precedent. |
| B. Add new `@nfr-obs-05` for "metric-emission correctness" | DEFERRED | Inflates the NFR matrix without buying signal; the contract is NFR-OBS-03. |
| C. Use only inherited tags (no `@nfr-perf-05`) | DEFERRED | Loses the operational link from the overhead-budget scenario to a named NFR row. |

**Slice-6 outcome of D1 = A**: 5 scenarios carry `@nfr-obs-03` (the 5
metric-correctness scenarios — the cardinality safety scenario gets
its own `@cardinality` sub-area tag and is NOT counted in the
`@nfr-obs-03` set per recommendation pick); 1 scenario carries
`@nfr-perf-05` (the `@manual` middleware-overhead contract). The
slice-1 NFR catalogue at
`docs/feature/foundry-backend-mvp/discuss/nfrs.md` gains a one-line
`NFR-PERF-05` entry in this DISTILL pass (back-propagation —
provenance-clean: established 2026-05-25 (slice 6)). This is the first
slice-introduced NFR tag in the project; documented here as the
authoritative slice-of-origin pending optional future expansion of
the NFR section if the contract evolves.

### D2 — Cardinality enforcement test harness

| Option | Status | Rationale |
|---|---|---|
| **A. Unit test in `metrics_server.rs` (DESIGN recommendation)** | **CHOSEN** | Smallest delta (~20 LOC); lives next to the code under test; fails closed on regression. The slice-6 acceptance scenario #3 covers the RUNTIME label-value contract (MatchedPath template emitted as `path`); the unit test covers the STATIC label-key contract (only `path`/`method`/`status` keys). Two-layer enforcement: behavior probe at acceptance + static probe at unit. |
| B. xtask check (`cargo xtask check-cardinality`) | DEFERRED | More visible but heavier than the unit test; revisit if a future slice introduces conditional-label metric families. |
| C. Both unit test AND xtask | DEFERRED | Over-belt-and-braces at slice-1 scale (~800 series). |

**Slice-6 outcome of D2 = A**: DELIVER writes a unit test
exercising the middleware against a representative request and
asserting the emitted label keys are EXACTLY `{path, method,
status}`. The acceptance scenario #3 covers the runtime side. See
"Open Decisions for DELIVER" below for the inline-test-file vs
sibling-file question (deferred to DELIVER's GREEN phase per
project test-organization convention).

### D3 — Probe verifying the middleware actually fired

| Option | Status | Rationale |
|---|---|---|
| **A. Behavioral only — counter sum == N after N requests** | **CHOSEN** | Principle 12 substrate-lie probe: assert the OBSERVABLE (counter incremented), not the structural wiring (tower stack inspection). Identical pattern to ADR-014's `foundry_app_startup_total` line assertion. |
| B. Structural only — middleware appears in tower stack | REJECTED | False-positive risk: the middleware could be in the stack but emit zero counters. Also requires custom tower-stack introspection. |
| C. Both behavioral AND structural | DEFERRED | Doubles assertions without doubling signal. |

**Slice-6 outcome of D3 = A**: Scenario #4 is the behavioral probe —
make N requests, scrape `/metrics`, assert the counter sum equals N
exactly. No structural assertion on tower stack composition.

### D4 — `db_connections_in_use` gauge initial value

| Option | Status | Rationale |
|---|---|---|
| **A. Register at 0 at process start; first poll tick (within 5s) overwrites** | **CHOSEN** | Grafana sees the line immediately — no 5s "metric absent" gap. The line at value 0 is honest (`min_connections=0` pool means `in_use == 0` is true at startup). DESIGN recommendation. |
| B. Only emit after the first poll tick | REJECTED | Creates the 5s metric-absent window slice 6 exists to prevent. |
| C. Eagerly call `Store::pool_stats()` synchronously at startup before the poll task spawns | DEFERRED | Functionally equivalent to A for the first 5s; adds startup-time complexity for no observable benefit. |

**Slice-6 outcome of D4 = A**: Scenario #6 ("`db_connections_in_use`
is scrapable immediately at value 0") is the contract. DELIVER's
poll-task setup emits an initial 0 BEFORE spawning the
`tokio::time::interval` tick loop. The first tick (within 5s)
overwrites with live pool state.

### D5 — Poll-task lifecycle on graceful shutdown

| Option | Status | Rationale |
|---|---|---|
| **A. Abort on shutdown signal — let tokio drop the task** | **CHOSEN** | Polling is purely observational; aborting mid-tick loses at most one gauge update (~5s staleness). NFR-AVAIL-02 cares about in-flight REQUESTS, not background metric refreshes. Simplest contract: `axum::serve.with_graceful_shutdown` returns → runtime drops all background tasks → poll task drops with everything else. No special wiring. |
| B. Run-to-completion of current tick | REJECTED | Current tick is ~100ns; "completion" is instantaneous either way. Special-cased wiring for zero benefit. |
| C. Wait for in-flight scrape to complete | REJECTED | Poll task writes to recorder; scrape is initiated externally by Prometheus, separate concern. |

**Slice-6 outcome of D5 = A**: No special shutdown wiring for the
poll task. The shutdown path remains the slice-1
`axum::serve.with_graceful_shutdown(shutdown_signal())` pattern; the
poll task drops naturally. The "absence of a panic in the structured
shutdown log" is the proxy signal at the @walking_skeleton +
@startup-probe scenario #9. A more rigorous SIGTERM-driving scenario
is deferred to `@manual` documentation only.

## Structural decisions (no user pick — locked by inheritance + brief)

| ID  | Question | Pick | Captured in |
|-----|----------|------|-------------|
| DD-1 | Strategy (per port-class default) | C — all real adapters per policy file | `docs/architecture/atdd-infrastructure-policy.md` (inherited, no new rows) |
| DD-2 | Test invocation pattern | Subprocess via `assert_cmd::Command::cargo_bin("foundry")` (slice-3 US-03 precedent) — NOT in-process `InProcHarness` | `crates/foundry-acceptance/src/steps/handler_instrumentation.rs` (RED scaffold) + `proposals.md` § "How slice-6 scenarios run" |
| DD-3 | New step file vs extending existing step files | NEW file `handler_instrumentation.rs`; all existing step files (us-01..us-13 + us_10_comment_edit_delete) left intact | `crates/foundry-acceptance/src/steps/handler_instrumentation.rs` + `lib.rs` registration |
| DD-4 | Scaffold-RED mechanism | Step bodies `panic!("Not yet implemented -- RED scaffold (DISTILL); DELIVER finishes this")`; production code NOT touched per task brief | step file body + `red-classification.md` |
| DD-5 | Force-link discipline | `tests/acceptance.rs` adds `use foundry_acceptance::steps::handler_instrumentation as _handler_instr;` | `crates/foundry-acceptance/tests/acceptance.rs` |
| DD-6 | World additions | Eight `Option` / `HashMap`-default fields appended under a new `// ---- Slice 6: handler-instrumentation ----` block; all defaulted so existing scenarios unaffected | `crates/foundry-acceptance/src/world.rs` (bottom) |
| DD-7 | New test-infrastructure file | `crates/foundry-acceptance/src/support/metrics_scrape.rs` — ~133 LOC helper for `reqwest::get` against `/metrics` + Prometheus text-exposition parser. Analogous to slice-2's `sse_client.rs`. Module registered in `support/mod.rs`. | `crates/foundry-acceptance/src/support/metrics_scrape.rs` |
| DD-8 | New dep needed for `assert_cmd` | NONE — `assert_cmd` already in workspace deps (slice-3 inheritance) | confirmed in `crates/foundry-acceptance/Cargo.toml` |
| DD-9 | Scope reconciliation (DISCUSS vs DESIGN) | Zero contradictions — NFR-OBS-03 enumerates the 5 metrics; DESIGN D0 ships exactly those 5 (the 5 deferred are flagged "no consumer today", not a contradiction with the catalogue) | this file § "Reconciliation" below |
| DD-10 | Reviewer dispatch deferred to PR time | Per slice-4 wave-decisions.md line 209 / slice-5 DD-7 precedent — no in-DISTILL reviewer parallel-dispatch | this file § "Final Wave Review Gate" |

## Reconciliation (HARD GATE)

Per nw-distill § "Wave-Decision Reconciliation HARD GATE". Files read:

- `docs/feature/foundry-backend-mvp/discuss/stories.md` — slice 6 is
  cross-cutting infrastructure; no specific user story owns it. The
  applicable NFR cells are NFR-OBS-03 (metric emission) and a NEW
  NFR-PERF-05 cell (middleware overhead budget per D1 = A).
- `docs/feature/foundry-backend-mvp/discuss/nfrs.md` — NFR-OBS-03 line
  60 enumerates the 5 metrics slice 6 ships. NFR-PERF-01..04 do NOT
  cover middleware overhead (PERF-01 is the 200ms page render
  INCLUDING instrumentation; PERF-04 is pool sizing). The new
  NFR-PERF-05 row is added in this DISTILL pass (back-propagation).
- `docs/feature/handler-instrumentation/design/wave-decisions.md` —
  D0–D7 picks + 5 ADRs (010–014) + invented-detail flag list.
- `docs/feature/handler-instrumentation/design/architecture.md` —
  slice-specific design, 5 metrics, L3 sequence diagram, bounded-triple
  label spec, perf budget.
- `docs/feature/handler-instrumentation/design/adrs/ADR-010..014.md` —
  all five locked decisions.
- No `docs/feature/handler-instrumentation/devops/` directory (slice 6
  has no infra changes — recorder + sidecar already wired by DEVOPS
  c7cb715; per nw-distill § Graceful Degradation = WARN, default to
  slice-1/2/3 infrastructure recorded in policy file).

**Reconciliation result: PASSED — 0 contradictions** across DISCUSS /
DESIGN / DEVOPS.

Specifically checked:

- NFR-OBS-03 says "Default metrics include: `http_requests_total{path,
  method,status}`, `http_request_duration_seconds` (histogram),
  `db_connections_in_use`, `sse_subscribers_total`, ..." — matches
  DESIGN D0's shipped set exactly. Note: NFR-OBS-03 specifies
  `http_request_duration_seconds` labels = none (line 156 — historical
  catalogue entry), while DESIGN ADR-011 ships `{path, method, status}`.
  This is NOT a contradiction: the NFR catalogue documents what MUST
  exist; ADR-011 tightens the label set for the histogram to match the
  counter (consistency, dashboard-query-shape match). The catalogue's
  null label set is the FLOOR (what's required); ADR-011's triple is
  the CHOSEN implementation that's stricter than the floor.
- NFR-PERF-04 says "≤10 connections per replica" — DESIGN's pool poller
  reads `pool.size()` / `pool.num_idle()` which are read-only; no
  contradiction with the connection-count cap.
- The 5 DEFERRED metrics in D0 (`outbox_pending_jobs`,
  `bootstrap_tokens_unclaimed`, `migration_apply_duration_seconds`,
  `realtime_listen_disconnects_total`, `probe_failures_total`,
  `db_connection_wait_seconds`) are enumerated in NFR-OBS-03 line 156
  but flagged "no dashboard consumer" in D0. The catalogue is the
  FORWARD-LOOKING contract; D0 is the CURRENT-SLICE scope. Not a
  contradiction — same posture slice 5 used for the deferred admin-undelete
  runbook.

## Scenarios per file table

| File | Scenarios | Of which @walking_skeleton | Of which @error | Of which @manual |
|---|---|---|---|---|
| `features/handler-instrumentation.feature` (slice 6, NEW) | 10 | 2 (#1 request-path + #9 startup-probe — see invented detail #1) | 1 (#8 abrupt SSE drop) | 1 (#10 perf budget contract) |

Slice 6 introduces no edits to slice-1..5 feature files. Total
acceptance surface after slice 6: pre-existing ~55 + 10 = ~65
scenarios across the project.

Scenario count of 10 is kept (one above the 7-9 prompt ceiling); the
user picked to keep the scope intact rather than collapse #5 + #6
(connection-pool gauge variants) or merge #7 + #8 (SSE gauge variants)
to preserve verb-level granularity, mirroring the slice-5 enumerated-
scenarios convention.

Error-path ratio for slice 6: 1 of 10 = 10% — below the 40% nw-distill
target. **Justification**: slice 6 is a CONFIG-SHAPED slice (metric
emission correctness; no user-facing flows; no domain inputs). The
"errors" in scope are:

- Cardinality regression (covered by the unit test per D2 = A — STATIC
  enforcement) + acceptance scenario #3 (RUNTIME enforcement).
- Abrupt SSE drop / RAII Drop correctness (scenario #8).
- Startup-probe failure (DEFERRED to ADR-014 § Verification unit test
  per invented detail #6 — DELIVER PBT phase).
- Middleware fail-to-fire (covered by scenario #4 — would be observable
  as "counter is 0 after N requests").

The error surface for a metrics-emission slice is intrinsically thin
(metrics are unidirectional; there's no user input to validate
adversarially). Adding bogus error scenarios would lower signal
quality. Same justification slice 5 used (coverage-matrix.md row 71)
and slice 2 used.

## Tag conventions added

Inherited from slice 1/2/3/4/5 (unchanged):
`@walking_skeleton`, `@real-io`, `@driving_adapter`, `@error`,
`@nfr-obs-02`, `@nfr-obs-03`, `@nfr-perf-01..04`, `@nfr-sec-05`,
`@nfr-sec-06`, `@us-NN`, `@manual`, `@docker-compose`,
`@slice1`..`@slice5`.

Added in slice 6 (deltas only):

- `@slice6` — every scenario in the new feature file.
- `@handler-instrumentation` — feature-level (mirrors slice-2's
  `@realtime`, slice-5's `@comment-edit-delete`).
- `@metrics` — sub-area: scenarios that scrape `/metrics` (all 10
  of them in slice 6).
- `@cardinality` — scenario #3 (bounded triple label-set + forbidden-
  labels probe).
- `@startup-probe` — scenarios #9 (self-scrape probe success; also
  carries `@walking_skeleton`).
- `@nfr-obs-03` — reused per D1 = A (5 metric-emission scenarios).
- `@nfr-perf-05` — **NEW** — scenario #10 (`@manual` middleware
  overhead contract; first slice-introduced NFR tag in the project).
  Documented here pending optional back-propagation to slice-1
  `nfrs.md` — back-propagation is INCLUDED in this DISTILL pass per
  the user pick (cleaner provenance), so the catalogue is now in
  lock-step with the slice-6 tag.

Per D1 = A: `@nfr-obs-03` reused (5 scenarios — the 5 metric-emission
scenarios); `@nfr-perf-05` is the only NEW `@nfr-*` tag.

## CI invocation

Matching slice-2/3/4/5 style:

```bash
# Full suite (slices 1+2+3+4+5+6)
cargo test -p foundry-acceptance --test acceptance

# Slice-6 only (DELIVER iteration)
FOUNDRY_ACCEPTANCE_TAGS=@slice6 cargo test -p foundry-acceptance --test acceptance

# Slice 6 + slice 1 NFR-OBS-03 regression (scrape-related)
FOUNDRY_ACCEPTANCE_TAGS="@slice6 or @nfr-obs-03" cargo test -p foundry-acceptance --test acceptance

# Narrow band by sub-area
FOUNDRY_ACCEPTANCE_TAGS=@metrics       cargo test -p foundry-acceptance --test acceptance
FOUNDRY_ACCEPTANCE_TAGS=@cardinality   cargo test -p foundry-acceptance --test acceptance
FOUNDRY_ACCEPTANCE_TAGS=@startup-probe cargo test -p foundry-acceptance --test acceptance

# Exclude the @manual perf-budget scenario (default; matches default exclusion of @manual)
# Run the perf-budget criterion microbench separately (DELIVER sub-deliverable F)
cargo bench -p foundry-app --bench middleware_overhead   # NOT YET EXISTING; DELIVER creates
```

Concurrency cap stays at `--max-concurrent-scenarios 6` (inherited
from slice 3). Slice-6 scenarios spawn one foundry subprocess each
(~50–80MB RAM per replica × 6 concurrent = ~300–500MB peak under load —
within typical dev-laptop budgets).

## Suite-time budget

| Scenario | Cost | Notes |
|---|---|---|
| 1 Walking skeleton: scrape after one POST | ~3.5 s | subprocess spawn (~2s) + Postgres connect (~0.5s) + 1 POST (~50ms) + 1 scrape (~50ms) + assertions (~50ms) |
| 2 `http_requests_total` correctness across multiple routes | ~4.0 s | subprocess (~2s) + 5 requests (~250ms) + scrape + breakdown assertions |
| 3 Cardinality safety: route template label | ~3.5 s | subprocess + 1 parameterized request + scrape + label-key assertion |
| 4 Middleware-fired behavioral probe: counter == N | ~4.0 s | subprocess + N requests + scrape + sum assertion |
| 5 `db_connections_in_use` reflects pool state | ~8.5 s | subprocess (~2s) + acquire-and-hold connection for 6s (covers one poll tick) + scrape + assertion |
| 6 `db_connections_in_use` registered at 0 at startup | ~3.0 s | subprocess + immediate scrape (before first poll tick fires) + assertion |
| 7 `sse_subscribers_total` increments + decrements (clean close) | ~5.0 s | subprocess + open SSE + scrape (gauge ↑) + close SSE + scrape (gauge ↓) + assertion |
| 8 `sse_subscribers_total` Drop on abrupt disconnect | ~5.0 s | subprocess + open SSE + scrape (gauge ↑) + drop client mid-stream + scrape (gauge ↓) + assertion |
| 9 Startup probe (`/metrics` reachable + contains `foundry_app_startup_total`) | ~3.0 s | subprocess + scrape + line-present assertion |
| 10 `@manual` middleware overhead budget contract | (manual) | Documented; hands off to DELIVER criterion microbench |
| **Slice-6 subtotal (automated 1-9)** | **~39.5 s** | |
| Slice 1+2+3+4+5 baseline | ~123 s | per slice-5 wave-decisions.md |
| **Slice 1+2+3+4+5+6 projected total** | **~162 s** | slice-6 dominates the per-scenario cost due to subprocess spawn overhead |

### Fast-loop budget drift — ACKNOWLEDGED

The fast-loop iteration pattern (slice-5 precedent — strip
`@docker-compose` + `@manual`) projects to ~30s baseline + ~39s slice-6
= **~70s total fast-loop**. This **exceeds the 60s top-line set in
slice 1** by ~10s. Flagged for monitoring per user pick (do NOT
re-litigate scenario #5's 6s connection-hold this session).

| Mitigation option | Status | Cost / consequence |
|---|---|---|
| (a) Shard CI matrix into two parallel jobs (one per `@slice` band) | AVAILABLE — per DEVOPS plan | CI YAML edit (~20 LOC); cuts wall-clock to ~40s per shard; doubles CI minutes |
| (b) Move scenario #5's 6s connection-hold to `@manual-trigger` | AVAILABLE — slice-3 precedent | One-line tag edit; drops slice-6 subtotal to ~31s; loses the only automated probe that the poll task actually overwrites the initial 0 with live state |
| (c) Accept and re-baseline the top-line at ~70s (or ~90s with headroom) | **RECOMMENDED for v0.1 RC** | Zero churn; document new baseline in slice-1 wave-decisions.md back-propagation when v0.1 ships |

**Recommendation for slice 6**: accept-and-re-baseline for v0.1 RC.
Revisit option (a) sharding if the fast loop hits 90s in a future
slice (slice-7+). Option (b) is the bail-out if slice #5's value is
disputed in PR review — not the default.

For slice-6-only iteration:
`FOUNDRY_ACCEPTANCE_TAGS=@slice6 cargo test …` runs in ~40s.

## Open Decisions for DELIVER

| Decision | DISTILL status | DELIVER inheritance |
|---|---|---|
| Cardinality unit test file placement: inline in `metrics_server.rs` (per D2 = A wording) vs sibling `metrics_server_test.rs` | DISTILL flagged as Open per D2 follow-up; both satisfy the contract ("unit test fails closed on cardinality regression") | DELIVER picks at GREEN. Project-wide convention: inline `#[cfg(test)] mod tests` blocks are the default unless the test file is >~200 LOC. The cardinality test is ~20 LOC → inline is the natural choice. |
| Exact micrometer probe shape for the `@manual` perf scenario (#10) | DISTILL scenario asserts "the @manual contract — overhead ≤10µs P95 — is documented; criterion microbench delivers the measurement"; the literal criterion harness shape is not pinned | DELIVER picks. Likely shape: criterion `bench_function` toggling `request_tracking_layer` on/off against a no-op handler; black_box on request input; `to_warmup_time(Duration::from_secs(2))`. Architecture.md § "Performance budget" measurement plan is the spec |
| Histogram bucket boundaries for `http_request_duration_seconds` | DESIGN flagged as decision-driven invented detail #2 (architecture.md lines 295-299): defaults are reasonable; revisit if slice-1 4ms P95 measurement justifies finer low-end buckets | DELIVER may tune; acceptance scenarios assert "the bucket count >= 1" not "specific bucket boundary present" — tuning doesn't red the suite |
| `MatchedPath` 404 fallback literal | DESIGN locked as `"<unmatched>"` (architecture.md line 275 + decision-driven detail #3). DISTILL scenario #3 asserts the route-template path; the `<unmatched>` fallback is NOT exercised in slice-6 scenarios (deferred to a `@manual` operator test or a future scenario) | DELIVER implements; no scenario edit required |
| Startup probe URL host | DESIGN locked as `127.0.0.1` regardless of `METRICS_HOST` (architecture.md line 305 + decision-driven detail #4). DISTILL scenario #9 inherits | DELIVER inherits |
| `SubscriberGauge::new(project_id: Uuid)` signature | ADR-013 § Decision locks the single-arg constructor. DISTILL scenarios #7 + #8 assume this shape (the `project_id` label is asserted on the scraped gauge series) | DELIVER inherits; if signature evolves (e.g., adds `replica_id`), the scenario assertions widen to match |
| Acceptance subprocess binding for `METRICS_PORT` + `FOUNDRY_PORT` | DISTILL picks: each scenario spawns subprocess with `METRICS_PORT=0` + `FOUNDRY_PORT=0` (request ephemeral) + reads the bound ports from structured log lines emitted at startup | DELIVER must ensure `main.rs` emits structured log lines `foundry listening on {addr}` (already exists per line 147) AND `foundry metrics listening on {addr}` (already exists per line 142). The scrape helper parses these lines from the subprocess stderr/stdout |
| Per-scenario `DATABASE_URL` | DISTILL picks: subprocess receives `DATABASE_URL=postgres://...?options=-csearch_path%3D{schema}` pointing at the slice-1 testcontainers Postgres + per-scenario schema (slice-1 `fresh_schema_pool_with_url` returns the URL we need) | DELIVER inherits; helper exists |

## DELIVER Pre-flight Checklist

**Sub-deliverable A — Middleware factory in `metrics_server.rs`**
- [ ] Production artefact: `pub fn request_tracking_layer() -> tower::Layer<...>`
      returning a tower middleware that extracts `MatchedPath` + method
      + status, emits `metrics::counter!("http_requests_total", ...)` +
      `metrics::histogram!("http_request_duration_seconds", ...)`
- [ ] Label keys hard-coded to EXACTLY `{path, method, status}`;
      404-fallback uses literal `path="<unmatched>"`
- [ ] Wired into `build_router` at the SAME tower-stack position as
      CSRF/session/request-id layers (ADR-010 § Decision)
- [ ] Cardinality enforcement unit test (per D2 = A) lives at
      `crates/foundry-app/src/metrics_server.rs::tests::request_tracking_layer_emits_exactly_path_method_status`
      OR sibling file per "Open Decisions for DELIVER" pick
- [ ] **Acceptance criterion**: scenarios 1, 2, 3, 4 GREEN
- [ ] **ADR**: ADR-010 (tower middleware placement) + ADR-011 (label
      keys + bucket set)

**Sub-deliverable B — `Store::pool_stats()` + poll task in `main.rs`**
- [ ] Production artefact: `Store::pool_stats() -> PoolStats { in_use:
      i32, idle: i32, size: i32 }` (read-only snapshot via
      `Pool::size()` + `Pool::num_idle()`)
- [ ] Background `tokio::time::interval` task in `main.rs` ticks every
      5 seconds (or `METRICS_POOL_POLL_SECONDS` env var); reads
      `Store::pool_stats()` + calls
      `metrics::gauge!("db_connections_in_use").set(stats.in_use as f64)`
- [ ] D4 = A: initial gauge value of 0 emitted at startup BEFORE the
      poll task spawns (so Grafana sees the line immediately, no 5s
      "metric absent" window)
- [ ] D5 = A: no special graceful-shutdown wiring for the poll task —
      let tokio drop it on `axum::serve` return
- [ ] **Acceptance criterion**: scenarios 5 + 6 GREEN
- [ ] **ADR**: ADR-012 (pool poller cadence + read-only snapshot)

**Sub-deliverable C — `SubscriberGauge` RAII in `foundry-realtime`**
- [ ] Production artefact: `pub struct SubscriberGauge { project_id:
      Uuid }` per ADR-013
- [ ] `SubscriberGauge::new(project_id)` increments gauge
- [ ] `impl Drop for SubscriberGauge { fn drop(&mut self) { ... decrements } }`
- [ ] `crates/foundry-realtime/Cargo.toml` gains
      `metrics = { workspace = true }`
- [ ] Panic-unwind unit test asserts gauge returns to pre-construction
      value via Drop (ADR-013 § Verification line 4)
- [ ] **Acceptance criterion**: scenarios 7 + 8 GREEN
- [ ] **ADR**: ADR-013 (RAII gauge — Drop trait correctness)

**Sub-deliverable D — `events.rs` guard wire-up**
- [ ] Production artefact: one-line addition near
      `state.realtime_tx.subscribe()` in
      `crates/foundry-app/src/events.rs::sse_stream`:
      `let _gauge = foundry_realtime::SubscriberGauge::new(project_id);`
- [ ] No other changes to events.rs (handler signature unchanged)
- [ ] **Acceptance criterion**: scenarios 7 + 8 GREEN (end-to-end with
      sub-deliverable C; the guard binding is the actual call-site)
- [ ] **ADR**: ADR-013 (same — call-site of the RAII type)

**Sub-deliverable E — Startup probe in `metrics_server.rs`**
- [ ] Production artefact: `pub async fn probe(handle:
      &PrometheusHandle, addr: SocketAddr) -> Result<()>` per ADR-014 §
      Decision (the 3-part assertion: HTTP 200, non-empty body,
      contains `foundry_app_startup_total` line)
- [ ] Called from `main.rs` after `metrics_server::serve` returns and
      BEFORE the main HTTP listener spawn (so failure shows as
      "container restarts" not "container serves traffic with broken
      metrics")
- [ ] Failure-injection unit test against mocked `PrometheusHandle`
      whose `render()` returns empty string (ADR-014 § Verification
      line 3) — covers the DEFERRED startup-probe-failure acceptance
      scenario per invented detail #6
- [ ] **Acceptance criterion**: scenario 9 GREEN
- [ ] **ADR**: ADR-014 (startup probe contract)

**Sub-deliverable F — Middleware overhead criterion microbench**
- [ ] Production artefact: criterion microbench at
      `crates/foundry-app/benches/middleware_overhead.rs` per
      architecture.md § "Performance budget" measurement plan
- [ ] Toggles `request_tracking_layer` on/off against a no-op handler
- [ ] Asserts P95 added overhead < 10µs across 27 routes
- [ ] CI gate `cargo bench -p foundry-app --bench middleware_overhead`
      passes
- [ ] **Acceptance criterion**: scenario #10 (`@manual @nfr-perf-05`)
      is the contract anchor; this microbench IS the executable measurement
- [ ] **NFR**: NFR-PERF-05 (back-propagated to slice-1 nfrs.md in this
      DISTILL pass)

**Regression**
- [ ] All 9 automated slice-6 scenarios GREEN end-to-end via
      `assert_cmd` subprocess + per-scenario PG schema + ephemeral
      ports
- [ ] No regression in the existing ~55 scenarios across slice 1+2+3+4+5
- [ ] `cargo check -p foundry-acceptance --tests` passes
- [ ] `cargo deny check` passes (zero new deps per
      architecture.md § "Technology Stack")
- [ ] `cargo xtask check-arch` passes (no crate-boundary changes;
      `foundry-realtime` gaining `metrics` dep is workspace-declared
      already; `foundry-core` remains I/O-free)

## Final Wave Review Gate

Per slice-4 wave-decisions.md line 209 / slice-5 DD-7 — the project
pattern defers the 4-reviewer wave-gate to PR time (legacy per-wave
file layout, all slices 1–5 reviewer-approved under this convention).
No in-DISTILL parallel reviewer dispatch. The PR will carry the DESIGN
ADRs (010–014) + this DISTILL artifact set + DELIVER work for
reviewers to inspect simultaneously.

## Decision-driven invented detail (slice 6 DISTILL deltas only)

DESIGN's "Decision-driven invented detail" list (architecture.md +
wave-decisions.md lines 285-326) is INHERITED UNCHANGED. DISTILL adds
these phrasing flags, all 6 ACCEPTED per user pick + a 7th
acknowledgement for the fast-loop suite-time drift:

1. **Two `@walking_skeleton` scenarios in one feature file (#1 + #9)** —
   slice-1..5 convention is one WS per feature. Slice 6 needs two
   because there are two structurally distinct end-to-end loops
   (request-path metric flow + process-startup-probe flow). **ACCEPTED**.
   Alternative would be to demote #9 to `@startup-probe` only; not
   taken because each loop is independently a "wired end-to-end"
   contract worth proving via the WS discipline.

2. **`@nfr-perf-05` is a NEW NFR tag** — slice 6 introduces it for the
   middleware overhead contract per D1 = A. **ACCEPTED**. Back-
   propagation included in this DISTILL pass (cleaner-provenance pick):
   one-line `NFR-PERF-05` row added to
   `docs/feature/foundry-backend-mvp/discuss/nfrs.md` next to
   NFR-PERF-04. This is the first slice-introduced NFR tag in the
   project; the row references the slice of origin (slice 6,
   2026-05-25) for audit-trail clarity.

3. **Subprocess test harness via `assert_cmd::Command::cargo_bin("foundry")`** —
   slice 6 introduces a per-scenario foundry subprocess pattern.
   **ACCEPTED**. Inherits the slice-3 US-03 precedent exactly (same
   `assert_cmd` crate, already in `Cargo.toml`). Per-scenario PG schema
   for isolation (slice-1 pattern). Ephemeral `METRICS_PORT` + ephemeral
   `FOUNDRY_PORT` per scenario for concurrency. The two scenarios
   that need the slow 5s poll-task tick (#5 + #6) drive the dominant
   per-scenario suite cost.

4. **Scenario #10 (middleware overhead) is `@manual`** — cucumber-rs
   cannot reliably measure 10µs P95 at scenario granularity. **ACCEPTED**.
   The scenario documents the contract + points at the criterion
   microbench DELIVER writes per architecture.md § "Performance budget"
   (sub-deliverable F).

5. **`support/metrics_scrape.rs` is NEW test-infrastructure** —
   ~133 LOC helper analogous to slice-2's `sse_client.rs`. **ACCEPTED**.
   Does `reqwest::get` + parses the Prometheus text-exposition format
   into typed structs. NOT taking a dependency on `prometheus-parse` or
   similar — the parsing surface needed is small and avoids a new
   crate dep. Already materialised in the prior DISTILL pass.

6. **Startup-probe failure scenario DEFERRED to ADR-014 § Verification
   unit test** — the brief proposed `@error @manual` in the .feature
   file; DISTILL defers. **ACCEPTED**. The failure-injection probe
   (mocked `PrometheusHandle` whose `.render()` returns empty) lives in
   DELIVER's PBT phase per sub-deliverable E.

7. **Suite-time fast-loop budget drift to ~70s** — ACKNOWLEDGED, not a
   re-litigation. Slice 6 pushes the fast loop ~10s over the slice-1
   60s top-line. Recommendation per "Suite-time budget" table:
   accept-and-re-baseline for v0.1 RC; revisit (a) CI sharding if it
   hits 90s in slice 7+. User explicitly chose NOT to re-litigate
   scenario #5's 6s connection-hold this session — that path stays
   open as bail-out (b) if PR review disputes the value.
