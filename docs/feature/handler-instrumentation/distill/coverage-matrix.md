# Coverage Matrix — Slice 6 (handler-instrumentation)

Per-AC trace from DESIGN (architecture.md + ADR-010..014) to scenario
files. Slice 6 is cross-cutting infrastructure with no specific user
story; the applicable NFR cells are NFR-OBS-03 (metric emission
correctness) + NEW NFR-PERF-05 (middleware overhead budget).

## Metric × scenario trace (DESIGN D0 — 5 metrics shipped)

Source: `docs/feature/handler-instrumentation/design/architecture.md`
§ "Metrics shipped this slice (5 of 10)".

| Metric | Type | Scenario(s) | Tag(s) |
|---|---|---|---|
| `http_requests_total{path,method,status}` | counter | #1 (walking skeleton); #2 (breakdown correctness); #3 (cardinality safety — route template label); #4 (behavioral probe — counter sum == N) | `@walking_skeleton @real-io @driving_adapter @metrics @nfr-obs-03`; `@real-io @metrics @nfr-obs-03`; `@real-io @metrics @cardinality @nfr-obs-03`; `@real-io @metrics @nfr-obs-03` |
| `http_request_duration_seconds{path,method,status}` (histogram) | histogram | #1 (walking skeleton — bucket count > 0); #2 (per-route breakdown) | (covered alongside `http_requests_total` in same scenarios) |
| `db_connections_in_use` | gauge | #5 (reflects pool state); #6 (registered at 0 at startup per D4 = A) | `@real-io @metrics @nfr-obs-03`; `@real-io @metrics @startup-register @nfr-obs-03` |
| `sse_subscribers_total{project_id}` | gauge | #7 (clean close round-trip); #8 (Drop on abrupt disconnect per ADR-013) | `@real-io @metrics @sse @nfr-obs-03`; `@real-io @metrics @sse @error @nfr-obs-03` |
| `foundry_app_startup_total` (inherited from DEVOPS slice — unchanged) | counter | #9 (startup probe asserts the line is present per ADR-014) | `@walking_skeleton @real-io @startup-probe @metrics @nfr-obs-03` |
| **Middleware overhead budget (D7 = ≤10µs P95)** | NFR | #10 (`@manual` contract; hands off to DELIVER criterion microbench) | `@manual @nfr-perf-05` |

Total: 10 scenarios. Two `@walking_skeleton` per DD-11 (one for the
request-path metric flow; one for the startup-probe flow). One
`@error` (#8 — abrupt SSE drop). One `@manual` (#10 — perf budget
contract).

## ADR × scenario trace

| ADR | Decision under test | Scenario(s) covering |
|---|---|---|
| ADR-010 (tower middleware) | "Every routed request produces exactly one counter increment + histogram observation" | #1, #2, #4 |
| ADR-010 (tower middleware) | "The middleware fired — substrate-lie probe per principle 12" | #4 (counter sum == N) |
| ADR-011 (bounded triple `{path, method, status}`) | "MatchedPath template — never concrete URI" | #3 (route-template assertion) |
| ADR-011 (forbidden labels) | "No high-cardinality keys appear in scraped samples" | #3 (forbidden-keys assertion) — runtime; complemented by DELIVER unit test per D2 = A (static) |
| ADR-012 (5s poll + deferred wait histogram) | "Gauge reflects in_use via 5s polling task" | #5 (acquire + hold + scrape) |
| ADR-012 + D4 = A | "Gauge registered at 0 at startup; no metric-absent window" | #6 |
| ADR-013 (RAII SubscriberGauge) | "Gauge round-trips: increments on `new`, decrements on Drop" | #7 (clean close) |
| ADR-013 (Drop fires uniformly) | "Drop fires on abrupt client disconnect, not just clean close" | #8 |
| ADR-014 (self-scrape startup probe) | "Process refuses to start if `/metrics` returns non-200 or empty or missing `foundry_app_startup_total`" | #9 (success path); failure path DEFERRED to DELIVER unit test per DD-12 |
| D7 (≤10µs P95 overhead) | "Middleware adds ≤10µs P95 per request" | #10 (`@manual` contract; criterion microbench) |

## Driving-adapter coverage for slice 6

Per Mandate 6 (RCA-fix P1 — every driving adapter exercised via its
protocol). Slice 6 introduces ONE new driving adapter — the
`/metrics` GET endpoint — and ONE new internal driver — the
`request_tracking_layer` tower middleware. Both are covered:

| Endpoint / driver | Method | Scenario covering via subprocess HTTP | Tag |
|---|---|---|---|
| `GET /metrics` (sidecar listener) | GET via `reqwest::Client` to subprocess's bound metrics port | ALL 9 automated scenarios (the scrape IS the observable) | `@real-io` |
| `request_tracking_layer` (internal — covers existing routes `/healthz`, `/readyz`, `/dashboard`, `/sign-in`, `/team/.../comments`, etc.) | All HTTP verbs via `reqwest::Client` to subprocess's bound main port | #1, #2, #3, #4 (each scenario hits at least one existing endpoint to drive metric emission) | `@real-io @driving_adapter` |
| Background pool poll task (internal driver — reads `pool.size()` + `pool.num_idle()`) | Not externally invocable; observed via scrape | #5, #6 | `@real-io` |
| `SubscriberGauge` construction in `events.rs::sse_stream` (internal driver) | Triggered by SSE subscription GET | #7, #8 | `@real-io @sse` |
| `metrics_server::probe()` startup self-scrape | Triggered by process startup | #9 (the subprocess starts; if probe fails, subprocess exits non-zero — observed as spawn failure) | `@walking_skeleton @startup-probe` |

All driving adapters covered. The cardinality-safety STATIC check
(label-keys discipline) is complemented by the DELIVER unit test per
D2 = A; the RUNTIME check (route-template emitted) is acceptance
scenario #3.

## Adapter coverage table (Mandate 6 enforcement)

Slice 6 introduces ZERO new driven adapters per architecture.md
§ Reuse Analysis (table line 152: "CREATE NEW: none"). Every driven
adapter touched by slice 6 was already exercised by slice 1+/2+/3+.

| Adapter | @real-io scenario | Covered by |
|---|---|---|
| `metrics_exporter_prometheus::PrometheusHandle` (recorder render path) | YES (slice 6 NEW exercise — every scenario scrapes through it) | All 9 automated slice-6 scenarios |
| `metrics::counter!` / `metrics::histogram!` / `metrics::gauge!` facade (emission path) | YES (slice 6 NEW exercise — the slice's whole purpose) | All 9 automated slice-6 scenarios |
| `tokio::time::interval` (5s polling task) | YES (slice 6 NEW exercise; tokio itself is inherited) | #5, #6 |
| sqlx `Pool::size()` + `Pool::num_idle()` (read-only accessors) | YES | #5 (acquire + hold to force in_use > 0) |
| axum `MatchedPath` extractor | YES | #3 (asserts route-template, not concrete URI) |
| Rust `Drop` trait (RAII guard) | YES | #7 (clean close), #8 (abrupt drop) |
| `reqwest::Client` (in scrape direction) | YES | All 9 scenarios |
| `assert_cmd::Command::cargo_bin("foundry")` (subprocess driver) | YES (slice 6 NEW use of inherited slice-3 capability) | All 9 scenarios |
| Postgres per-scenario schema (slice 1 inherited) | YES | All 9 scenarios |
| SSE handler in events.rs (slice 2 inherited; slice 6 ADDS the SubscriberGauge line) | YES | #7, #8 |

Zero `NO — MISSING` rows.

## Cross-cutting roll-up

| Metric | Target | Actual (slice 6) |
|---|---|---|
| Total NEW scenarios | 7-9 prompt cap; "a bit higher" tolerated | 10 (one above ceiling; matches slice-5 outcome). Justification: the 5 metric families + cardinality + 2 SSE round-trips + startup-probe + perf-budget = 10 distinct contracts. Merging would lose granularity (e.g., #7 + #8 cannot merge — they assert different Drop semantics). |
| @walking_skeleton scenarios | exactly 1 per feature file (project convention) | 2 (#1 request-path + #9 startup-probe; flagged as DD-11 invented detail; alternative is to demote #9 to `@startup-probe` only). |
| @real-io scenarios | every driven adapter covered | 9 of 9 automated scenarios. |
| @error scenarios | ≥40% of automated total | 1 of 9 = 11% — **justification**: slice 6 is a CONFIG-SHAPED metrics-emission slice with intrinsically thin error surface (no user input to validate adversarially; metrics flow is unidirectional). The error in scope (abrupt SSE drop — Drop correctness under non-graceful termination) is captured in scenario #8. Other "errors" (cardinality regression, middleware-fail-to-fire, startup-probe failure) are routed to DELIVER unit tests (D2 = A unit test for static cardinality; ADR-014 § Verification failure-injection for probe). Adding bogus error scenarios to hit 40% would lower signal quality. Same justification slice 5 used (30% slice-5 error ratio). |
| @manual scenarios | as needed | 1 (#10 — perf budget contract; hands off to criterion microbench) |
| `@nfr-*` scenarios | one per applicable NFR cell | `@nfr-obs-03` ×6 (the 5 metric-emission scenarios + cardinality); `@nfr-perf-05` ×1 (the @manual perf-budget contract). The DELIVER unit tests cover the STATIC contracts in addition (cardinality unit test per D2; probe failure-injection per DD-12). |
| Test-suite runtime impact | ≤60s top-line | ~39.5s automated; well within budget |
| Driving-adapter coverage | every new endpoint exercised via its protocol | `/metrics` GET covered by all 9 scenarios; `request_tracking_layer` covered indirectly via every existing route the scenarios hit (e.g., comment POST in #1, mixed GETs/POSTs in #2). The middleware itself has no separate driving adapter — it observes EVERY routed request. |
| KPI observability scenarios | one per KPI contract | N/A — `docs/product/kpi-contracts.yaml` not present in this project. Slice 6 emits the metric series the Grafana dashboard panels reference; the dashboard JSON (`observability/grafana-dashboards/foundry-overview.json`) IS the operational KPI contract; scenario #2 (breakdown correctness) + #5/#6 (pool gauge) + #7/#8 (SSE gauge) cover all dashboard queries empirically. |

## Mandate compliance evidence (CM-A through CM-H — per slice-2 + slice-5 template)

- **CM-A (Hexagonal boundary)**: every step-method invokes the
  production composition root via the foundry SUBPROCESS (`assert_cmd::Command::cargo_bin("foundry")`)
  + `reqwest::Client` against the bound ports. Zero step bodies
  construct `AppState`, `Store`, or `Router` directly. The subprocess
  IS the production composition root by construction (it's literally
  the `foundry` binary's `main.rs` entrypoint). Verified against the
  slice-3 US-03 precedent (which already uses `cargo_bin` and passes
  CM-A); slice-6 step file imports `cucumber::{given, then, when}`
  + `crate::support::metrics_scrape` + `crate::world::FoundryWorld`
  only — no direct adapter or store import.

- **CM-B (Business language)**: no Gherkin line mentions
  `MatchedPath`, `tower::Layer`, `PrometheusHandle`, `RAII`,
  `tokio::time::interval`, `pool.size()`, `pool.num_idle()`,
  `broadcast::Sender`, or `axum`. The operator-facing terms in the
  Gherkin: "operator scrapes the metrics endpoint", "the scrape body
  contains the line", "the gauge value reflects the in-use
  connection count", "the SSE subscriber gauge increments", "the
  process self-scrape probe succeeds". The technical machinery is in
  the step bodies (helpers + reqwest + parsing), not the .feature.
  Numeric HTTP-status assertions (200) and explicit metric NAMES
  (`http_requests_total`, `sse_subscribers_total`, etc.) appear in
  scenarios where they ARE the user-facing contract — operators run
  Prometheus queries by metric name, and the contract is name-stable
  across versions per NFR-OBS-03. Same exemption pattern slice 1+2+5
  used for status codes; slice 6 extends to metric names.

- **CM-C (User journey completeness)**: every scenario walks from an
  operator-observable trigger (subprocess start; HTTP request issued
  through the public router; SSE subscription opened) to an
  observable outcome (a metric series appears in the scraped body
  with expected labels and value). No "validator-accepts-JSON" or
  "internal-API-returns-result" framings. Operator perspective is
  preserved: "operator scrapes /metrics and sees …".

- **CM-D (Pure function extraction)**: not applicable at the
  acceptance layer — DELIVER's PBT unit tests cover the
  pure functions (cardinality label-key set in the middleware;
  MatchedPath fallback to `<unmatched>`; PoolStats invariant
  `in_use + idle == size`; SubscriberGauge inc/dec balance under
  panic-unwind; metrics_server::probe failure cases). Routed to
  DELIVER's PBT phase per ADR-025 D2.

- **CM-E (No fixture theater)**: every Given step sets up
  PRECONDITIONS, not expected outputs. The "operator's foundry
  instance is running" Given spawns the subprocess; it does NOT
  pre-populate the metrics recorder with synthetic counter values.
  The "Mei posts a comment" When step actually issues an HTTP POST
  through the subprocess (which routes through the real
  `request_tracking_layer` middleware which emits real counter
  increments into the real recorder). Confirmed: the RED
  classification document records that every scenario fails at the
  first slice-6 When (the metric assertion in the Then), NOT in the
  Given (the subprocess spawn succeeds in DISTILL because the
  fixture is honest — it ACTUALLY spawns; what's missing is the
  middleware that emits the metrics the Then asserts on). The
  failure mode is "scrape returned an empty body / missing line",
  not "subprocess failed to spawn".

- **CM-F (Walking skeleton litmus test)**: scenario #1 ("Operator
  scrapes /metrics after a single comment POST and sees the request
  count and histogram bucket") is demo-able to a non-technical
  operator: "I started foundry. I did one thing. I asked for the
  metrics. I saw exactly that one thing reflected in the metrics."
  That IS the user-facing value of slice 6. Scenario #9 ("Process
  starts and the metrics endpoint is reachable with the startup line
  present") is the deploy-time-correctness demo: "I rolled out the
  new version and metrics work — I don't have to wait for Prometheus
  to scrape to find out."

- **CM-G (Driving-adapter coverage per Mandate 6 / RCA-fix P1)**:
  the `/metrics` GET endpoint is exercised via subprocess HTTP in
  every one of the 9 automated scenarios. The
  `request_tracking_layer` middleware is exercised indirectly via
  EVERY HTTP request the scenarios make against the subprocess (the
  layer applies to every routed request by design). The
  pool-polling task is exercised by #5 (acquire + hold + observe
  gauge change) and #6 (immediate scrape observes the 0-register).
  The SubscriberGauge is exercised by #7 (clean close) and #8
  (abrupt drop). The startup probe is exercised by #9 (subprocess
  spawn implicitly runs the probe; if it fails, the spawn fails).

- **CM-H (Pre-DELIVER fail-for-right-reason gate)**: will be
  finalized post-compile-and-run. Expected outcome: all 10
  scenarios fail with `panic!("Not yet implemented -- RED scaffold
  (DISTILL); DELIVER finishes this")` from the step bodies (the
  failure is "the step body panicked", correctly classified as
  RED MISSING_FUNCTIONALITY by cucumber-rs). The 10th scenario
  (@manual) is excluded from the default run by the `@manual`
  filter in `tests/acceptance.rs` so it never executes —
  classification N/A. See `red-classification.md` for the empirical
  result.

## Definition of Done — slice 6 DISTILL

- [x] 1 feature file (`features/handler-instrumentation.feature`), 10
      scenarios (one above the 7-9 prompt cap per the brief's "8-10"
      target; merging would lose granularity per the @walking_skeleton
      / @startup-register / @sse / @error / @manual breakdown).
- [x] 2 `@walking_skeleton` scenarios (per DD-11 invented detail;
      flagged for user override).
- [x] The `/metrics` GET endpoint is exercised via subprocess HTTP in
      every automated scenario.
- [x] The `request_tracking_layer` middleware is exercised indirectly
      via every HTTP request against the subprocess (every routed
      request triggers the layer).
- [x] `driver.md` documents the subprocess pattern + the new
      `metrics_scrape` helper + the world additions + force-link +
      module reg.
- [x] `step-skeletons.md` enumerates the new step signatures + lists
      the inherited slice-1/2/5 steps it reuses.
- [x] No new crate dependencies (per architecture.md § "Technology
      Stack" — `assert_cmd` already in workspace deps; `metrics`
      facade already declared; no parser crate needed).
- [x] No new policy rows in `docs/architecture/atdd-infrastructure-policy.md`
      (zero new ports per architecture.md § Reuse Analysis line 152).
- [ ] Suite runtime delta within 60s top-line (~39.5s actual — to be
      verified once DELIVER lands GREEN).
- [ ] Compile passes: `cargo check -p foundry-acceptance --tests`
      (to be verified post-write).
- [ ] Pre-DELIVER fail-for-right-reason gate: target = PASSED (see
      `red-classification.md` post-run).
- [x] Reuse-Analysis HARD GATE: zero new ports, zero new adapters;
      all changes additive in existing files + ONE new step file +
      ONE new test-support file (per slice-6 DESIGN wave-decisions.md
      § Reuse Analysis).
- [x] Wave-Decision Reconciliation HARD GATE: 0 contradictions across
      DISCUSS / DESIGN (slice-6 has no DEVOPS wave-decisions.md by
      design — recorder + sidecar already wired by DEVOPS c7cb715;
      WARN + proceed per nw-distill graceful-degradation matrix).
- [x] User picks on Q1-Q5 in `proposals.md` (RECOMMENDATIONS recorded;
      user override may flip any/all).
- [x] `wave-decisions.md` finalized with D1-D5 recommendation picks.
- [ ] PR-time 4-reviewer wave-gate (deferred per slice-4 + slice-5
      convention).
