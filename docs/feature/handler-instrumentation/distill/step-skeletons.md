# Step Skeletons — Slice 6 (handler-instrumentation)

Cucumber-rs step signatures the DELIVER wave fills in. Live in
`crates/foundry-acceptance/src/steps/handler_instrumentation.rs` —
this slice-6 work is ADDITIVE; no other step file is modified.

Step-method bodies are scaffolded RED with
`panic!("Not yet implemented -- RED scaffold (DISTILL); DELIVER
finishes this")` per nw-distill § "Mandate 7" (Rust adaptation per the
polyglot matrix — `panic!` is the Rust scaffold idiom that the
cucumber-rs runner classifies as `RED (MISSING_FUNCTIONALITY)`, not
`BROKEN`).

Step-method names follow the slice-1+2+3+4+5 style: `fn given_*`,
`fn when_*`, `fn then_*` — see `crates/foundry-acceptance/src/steps/us_10_comment_edit_delete.rs`
(slice 5) for tone.

## Background — inherited unchanged from slice 1 + slice 2

These phrases are defined in slice-1/2 step files; slice 6 features
call them verbatim and do not redefine them.

```rust
// us_05_bootstrap.rs (slice 1)
#[given(regex = r#"^a workspace "([^"]+)" exists with admin "([^"]+)"$"#)]
async fn workspace_exists_with_admin(...);

// us_07_project_create.rs (slice 1)
#[given(regex = r#"^a member "([^"]+)" belongs to the team "([^"]+)"$"#)]
async fn member_belongs_to_team(...);
#[given(regex = r#"^a project "([^"]+)" with key prefix "([^"]+)" exists in the "([^"]+)" team$"#)]
async fn project_exists_in_team(...);

// us_06_signin.rs (slice 1)
#[given(regex = r"^(\w+) is signed in$")]
async fn member_is_signed_in(...);

// us_08_file_issue.rs (slice 1)
#[given(regex = r#"^the "([^"]+)" project already has issue (\w+)-(\d+)$"#)]
async fn project_has_issue(...);
```

**Important constraint on slice-6 Background**: the slice-1..5
Background steps populate the per-scenario PG schema via DIRECT SQL
(through the in-process slice-1 harness). The slice-6 scenarios then
spawn a foundry SUBPROCESS that connects to the SAME schema. This
means:

1. The slice-1..5 Givens must run successfully against the slice-1
   schema rotation BEFORE the subprocess spawns.
2. The slice-6 "the operator's foundry instance is running" Given
   spawns the subprocess (which the schema seeded in step 1 is
   already populated for).
3. The subprocess runs `Store::migrate` against the schema, which
   is idempotent (`_sqlx_migrations` table prevents re-application).

This is a NEW invocation pattern (Background-seeds-schema, then
subprocess-uses-schema) but reuses ONLY existing slice-1 helpers
(`ensure_postgres`, `fresh_schema_pool_with_url`) and the existing
slice-3 `assert_cmd` pattern. Documented in `driver.md` § 5.

## World additions

`crates/foundry-acceptance/src/world.rs` — append AFTER the slice-5
US-10 edit/delete block.

```rust
// ---- Slice 6: handler-instrumentation ----
/// The foundry subprocess for the current scenario. Spawned by the
/// "the operator's foundry instance is running" Given. Dropped at
/// scenario teardown (its Drop impl kills + reaps the child process).
pub slice6_foundry: Option<crate::steps::handler_instrumentation::FoundrySubprocess>,
/// Most-recent `ScrapeSnapshot` captured by a When step. Then steps
/// read it for assertions (label-key set, sample sum, line presence).
pub slice6_last_scrape: Option<crate::support::metrics_scrape::ScrapeSnapshot>,
/// Status code returned by the most-recent raw scrape (used by the
/// startup-probe success scenario #9 which asserts 200 explicitly).
pub slice6_last_scrape_status: Option<reqwest::StatusCode>,
/// Count of HTTP requests the When step has issued against the
/// subprocess's main listener. Used by scenario #4 (counter sum == N).
pub slice6_request_count: u64,
/// Map (route_template, method) -> count of requests issued. Used
/// by scenario #2 (per-route + per-method breakdown).
pub slice6_request_count_by_route:
    std::collections::HashMap<(String, String), u64>,
/// The SSE subscription opened in scenarios #7 + #8. Distinct from
/// `us_09_subscriptions` because this one rides through a foundry
/// SUBPROCESS, not the in-process harness. Drop = client-side abrupt
/// close (used by scenario #8 to trigger SubscriberGauge::Drop on
/// the server side).
pub slice6_sse_subscription: Option<reqwest::Response>,
/// The connection acquired-and-held in scenario #5 (forces
/// `db_connections_in_use` to be > 0 for at least one poll tick).
/// Held as a long-lived sqlx connection from the per-scenario
/// schema pool. Dropped to release.
pub slice6_held_connection:
    Option<sqlx::pool::PoolConnection<sqlx::Postgres>>,
/// Per-scenario PG schema name (slice-1 pattern). Captured so
/// teardown can drop it. The subprocess connected via DATABASE_URL
/// with this schema pinned via search_path.
pub slice6_schema: Option<String>,
```

## Step force-link

`crates/foundry-acceptance/tests/acceptance.rs` — append next to the
existing `_us_13` import:

```rust
#[allow(unused_imports)]
use foundry_acceptance::steps::handler_instrumentation as _slice6;
```

`crates/foundry-acceptance/src/lib.rs` — append next to
`pub mod us_13_contributor_onboarding;` inside the `pub mod steps`
block:

```rust
pub mod handler_instrumentation;
```

`crates/foundry-acceptance/src/support/mod.rs` — append next to
existing module declarations:

```rust
pub mod metrics_scrape;
```

## Step signatures (the slice-6 contract DELIVER fills in)

Full Rust source with attribute macros + DELIVER implementation
outlines is the SSOT file
`crates/foundry-acceptance/src/steps/handler_instrumentation.rs`.
The signatures below mirror that file for review convenience.

### Subprocess helper (lives in the step file)

```rust
pub struct FoundrySubprocess {
    process: std::process::Child,
    pub main_addr: std::net::SocketAddr,
    pub metrics_addr: std::net::SocketAddr,
    pub db_schema: String,
}

impl FoundrySubprocess {
    pub async fn spawn(
        database_url_with_schema: &str,
        db_schema: String,
    ) -> anyhow::Result<Self>;

    pub fn shutdown(self);
}

impl Drop for FoundrySubprocess {
    fn drop(&mut self);
}
```

### Givens (3 new)

```rust
#[given("the operator's foundry instance is running")]
async fn given_foundry_instance_is_running(world: &mut FoundryWorld);

#[given(regex = r"^the operator's foundry instance has been running for at least (\d+) seconds$")]
async fn given_foundry_instance_has_been_running_for(
    world: &mut FoundryWorld,
    seconds: u64,
);

#[given(regex = r#"^Mei has subscribed to events on "([^"]+)"$"#)]
async fn given_mei_has_subscribed_to_events(
    world: &mut FoundryWorld,
    project_name: String,
);
```

### Whens (7 new)

```rust
#[when("the operator scrapes the metrics endpoint")]
async fn when_operator_scrapes_metrics_endpoint(world: &mut FoundryWorld);

#[when("the operator scrapes the metrics endpoint immediately")]
async fn when_operator_scrapes_metrics_endpoint_immediately(
    world: &mut FoundryWorld,
);

#[when(regex = r#"^Mei posts a comment on "(\w+)-(\d+)" with body "([\s\S]*)"$"#)]
async fn when_mei_posts_comment(
    world: &mut FoundryWorld,
    prefix: String,
    n: i32,
    body: String,
);

#[when(regex = r#"^the operator issues (\d+) HTTP requests across the routes "([^"]+)" and "([^"]+)"$"#)]
async fn when_operator_issues_requests_across_routes(
    world: &mut FoundryWorld,
    count: u64,
    route_a: String,
    route_b: String,
);

#[when(regex = r#"^the operator issues (\d+) HTTP requests to "([^"]+)"$"#)]
async fn when_operator_issues_requests_to(
    world: &mut FoundryWorld,
    count: u64,
    route: String,
);

#[when(regex = r"^Mei holds an open database connection for (\d+) seconds$")]
async fn when_mei_holds_open_db_connection_for(
    world: &mut FoundryWorld,
    seconds: u64,
);

#[when("Mei abruptly disconnects from the SSE stream")]
async fn when_mei_abruptly_disconnects_from_sse(world: &mut FoundryWorld);
```

### Thens (12 new)

```rust
#[then("the scrape returns HTTP 200")]
async fn then_scrape_returns_200(world: &mut FoundryWorld);

#[then(regex = r#"^the scrape body contains the line "([^"]+)"$"#)]
async fn then_scrape_body_contains_line(
    world: &mut FoundryWorld,
    metric_name: String,
);

#[then(regex = r#"^the scrape body contains a sample for "([^"]+)" with labels "([^"]+)"$"#)]
async fn then_scrape_body_contains_sample_with_labels(
    world: &mut FoundryWorld,
    metric_name: String,
    labels_csv: String,
);

#[then(regex = r#"^the scrape body's "([^"]+)" sample sums to (\d+)$"#)]
async fn then_scrape_body_sample_sums_to(
    world: &mut FoundryWorld,
    metric_name: String,
    expected_sum: u64,
);

#[then(regex = r#"^the scrape body's "([^"]+)" sample has value (\d+)$"#)]
async fn then_scrape_body_sample_has_value(
    world: &mut FoundryWorld,
    metric_name: String,
    expected_value: u64,
);

#[then(regex = r#"^the scrape body's "([^"]+)" samples carry only the label keys "([^"]+)"$"#)]
async fn then_scrape_body_samples_carry_only_label_keys(
    world: &mut FoundryWorld,
    metric_name: String,
    permitted_keys_csv: String,
);

#[then(regex = r#"^the scrape body's "([^"]+)" samples do NOT carry any of the label keys "([^"]+)"$"#)]
async fn then_scrape_body_samples_do_not_carry_label_keys(
    world: &mut FoundryWorld,
    metric_name: String,
    forbidden_keys_csv: String,
);

#[then(regex = r#"^the scrape body's "([^"]+)" sample's "([^"]+)" label is "([^"]+)"$"#)]
async fn then_scrape_body_sample_label_is(
    world: &mut FoundryWorld,
    metric_name: String,
    label_key: String,
    expected_label_value: String,
);

#[then(regex = r#"^the scrape body's "([^"]+)" histogram has at least one bucket with count >= (\d+)$"#)]
async fn then_scrape_body_histogram_bucket_count(
    world: &mut FoundryWorld,
    metric_name: String,
    min_count: u64,
);

#[then(regex = r#"^the scrape body's "([^"]+)" sample is greater than (\d+)$"#)]
async fn then_scrape_body_sample_is_greater_than(
    world: &mut FoundryWorld,
    metric_name: String,
    threshold: u64,
);

#[then(regex = r#"^the scrape body's "([^"]+)" sample returns to (\d+)$"#)]
async fn then_scrape_body_sample_returns_to(
    world: &mut FoundryWorld,
    metric_name: String,
    baseline: u64,
);

#[then("the foundry subprocess is alive")]
async fn then_foundry_subprocess_is_alive(world: &mut FoundryWorld);
```

## DELIVER Pre-flight Checklist (slice-6 sub-deliverables)

DELIVER must satisfy these before merging. Categorized by the 5 ADRs +
the perf-budget contract:

### Sub-deliverable A — Middleware factory in `metrics_server.rs`

- [ ] `pub fn request_tracking_layer() -> impl tower::Layer<...>`
      factory returns a tower middleware
- [ ] Middleware extracts `axum::extract::MatchedPath` (or
      `<unmatched>` fallback) + method + observed response status
- [ ] Emits `metrics::counter!("http_requests_total", "path" => ...,
      "method" => ..., "status" => ...).increment(1)`
- [ ] Emits `metrics::histogram!("http_request_duration_seconds",
      "path" => ..., "method" => ..., "status" => ...).record(d)`
- [ ] Label keys are EXACTLY `{path, method, status}` and no others
      (D2 = A unit test enforces statically)
- [ ] Wired into `build_router` in slice-1
      `crates/foundry-app/src/lib.rs::build_router` via a single
      `.layer(metrics_server::request_tracking_layer())` near the
      existing CSRF/session layers
- [ ] Unit test
      `metrics_server::tests::request_tracking_layer_emits_exactly_path_method_status`
      passes
- [ ] Acceptance scenarios 1, 2, 3, 4 GREEN

### Sub-deliverable B — `Store::pool_stats()` + poll task in `main.rs`

- [ ] `Store::pool_stats() -> PoolStats { in_use: i32, idle: i32, size: i32 }`
      method added to `crates/foundry-store/src/lib.rs`
- [ ] `PoolStats` struct exported from `foundry-store`
- [ ] `crates/foundry-store/Cargo.toml` gains `metrics = { workspace = true }`
- [ ] Background `tokio::time::interval` task in
      `crates/foundry-app/src/main.rs` ticks every
      `METRICS_POOL_POLL_SECONDS` seconds (default 5)
- [ ] Each tick reads `Store::pool_stats()` + calls
      `metrics::gauge!("db_connections_in_use").set(stats.in_use as f64)`
- [ ] D4 = A: initial gauge value of 0 emitted at startup BEFORE
      the poll task's first tick (so Grafana sees the line
      immediately, no 5s "metric absent" window)
- [ ] D5 = A: no special graceful-shutdown wiring for the poll
      task — let tokio drop it on `axum::serve` return
- [ ] Acceptance scenarios 5 + 6 GREEN

### Sub-deliverable C — `SubscriberGauge` RAII in `foundry-realtime`

- [ ] `pub struct SubscriberGauge { project_id: Uuid }` added per
      ADR-013 to `crates/foundry-realtime/src/lib.rs`
- [ ] `SubscriberGauge::new(project_id)` increments
      `metrics::gauge!("sse_subscribers_total", "project_id" => ...)`
      by 1.0
- [ ] `impl Drop for SubscriberGauge { fn drop(&mut self) { ... } }`
      decrements the same gauge by 1.0
- [ ] `crates/foundry-realtime/Cargo.toml` gains
      `metrics = { workspace = true }`
- [ ] Panic-unwind unit test asserts gauge returns to
      pre-construction value via Drop (ADR-013 § Verification line 4)
- [ ] Acceptance scenarios 7 + 8 GREEN (covered alongside
      sub-deliverable E end-to-end)

### Sub-deliverable D — Startup probe in `metrics_server.rs`

- [ ] `pub async fn probe(handle: &PrometheusHandle, addr: SocketAddr) -> Result<()>`
      added per ADR-014 § Decision
- [ ] Probe does `reqwest::get("http://127.0.0.1:{port}/metrics")`
- [ ] Asserts (a) HTTP 200, (b) non-empty body, (c) body contains
      `foundry_app_startup_total` line
- [ ] Called from `main.rs` after `metrics_server::serve` returns
      and BEFORE the main HTTP listener spawn (so failure shows as
      "container restarts" not "container serves traffic with broken
      metrics")
- [ ] On `Err`: `anyhow::bail!` propagates → non-zero process exit
- [ ] Structured log line `health.startup.refused` captures the
      specific probe failure
- [ ] Failure-injection unit test against mocked `PrometheusHandle`
      whose `render()` returns empty string (ADR-014 § Verification
      line 3) — covers the DEFERRED startup-probe-failure acceptance
      scenario per DD-12
- [ ] Acceptance scenario 9 GREEN

### Sub-deliverable E — `events.rs` guard wire-up

- [ ] One-line addition near `state.realtime_tx.subscribe()` in
      `crates/foundry-app/src/events.rs::sse_stream`:
      `let _gauge = foundry_realtime::SubscriberGauge::new(project_id);`
- [ ] The `_gauge` binding holds the guard for the lifetime of the
      SSE stream future
- [ ] No other changes to events.rs (handler signature unchanged)
- [ ] Acceptance scenarios 7 + 8 GREEN

### Sub-deliverable F — Middleware overhead criterion microbench

- [ ] criterion microbench at
      `crates/foundry-app/benches/middleware_overhead.rs` per
      architecture.md § "Performance budget" measurement plan
- [ ] Toggles `request_tracking_layer` on/off against a no-op
      handler
- [ ] Asserts P95 added overhead < 10µs across 27 routes
- [ ] CI gate `cargo bench -p foundry-app --bench middleware_overhead`
      runs in CI (acceptance scenario #10 is the `@manual` contract
      anchor; the bench is the enforcement mechanism)

### Cross-cutting regression

- [ ] All 9 automated slice-6 scenarios GREEN end-to-end via
      `assert_cmd` subprocess + per-scenario PG schema + ephemeral
      ports
- [ ] No regression in the existing ~55 scenarios across slice 1+2+3+4+5
- [ ] `cargo check -p foundry-acceptance --tests` passes
- [ ] `cargo deny check` passes (zero new deps per
      architecture.md § "Technology Stack")
- [ ] `cargo xtask check-arch` passes — `foundry-realtime` gaining
      `metrics` dep is workspace-declared already; `foundry-store`
      gaining `metrics` dep is also workspace-declared;
      `foundry-core` remains I/O-free
- [ ] Step-phrase contract: the 22 new phrases (3 Givens + 7 Whens +
      12 Thens) MUST NOT be renamed in GREEN. Awkward phrasings
      should be surfaced as DELIVER → DISTILL retro items, not
      unilateral renames.
- [ ] The `metrics_scrape` helper API (the public surface listed in
      `driver.md` § 2a) MUST NOT change in GREEN. DELIVER may add
      methods if needed; existing methods MUST stay stable.

## Production-side scaffolds (Mandate 7) — NOT done by slice-6 DISTILL

Per the task brief:
> DO NOT touch any production code outside `crates/foundry-acceptance/`.

This is a project-specific deviation from the nw-distill § "Mandate 7:
RED-Ready Scaffolding" default. The slice-6 task explicitly defers
production-side scaffolding to DELIVER's RED phase (per ADR-025 D2:
DELIVER unskips, writes PBT, then implements). The RED classification
in slice 6 is achieved entirely by step-body panics in
`crates/foundry-acceptance/src/steps/handler_instrumentation.rs` —
no production-side `panic!`-shaped scaffolds.

DELIVER picks up production-side scaffolds (or full implementations)
from a clean slate. The acceptance step bodies are the RED contract.

## DELIVER read-back instructions

When DELIVER picks up slice 6:

1. The 9 automated slice-6 scenarios are all live (no `@skip` /
   `@ignore` tag). Each panics on its first slice-6-specific step —
   typically the Given "the operator's foundry instance is running"
   when it tries to spawn the subprocess (panic comes from the
   scaffold body, BEFORE the actual spawn logic). DELIVER's job is to
   replace the scaffold panics with real implementation.
2. The 10th scenario (`@manual`) is excluded from default runs by the
   `@manual` filter in `tests/acceptance.rs` line 81. It exists as the
   contract anchor for the criterion microbench (sub-deliverable F).
3. Cucumber-rs treats `panic!` from a step body as a step failure
   with the panic message as the captured output. DELIVER does NOT
   need to change the step bodies' panic-to-implementation pattern —
   replace the body verbatim with the real implementation.
4. The step phrases (regex strings) registered in
   `crates/foundry-acceptance/src/steps/handler_instrumentation.rs`
   ARE the contract between DISTILL and DELIVER. They MUST NOT change
   during GREEN. If a phrase reads awkwardly during implementation,
   surface it as a DELIVER → DISTILL retro item, not a unilateral
   rename.
5. The subprocess pattern (slice-3 US-03 precedent via `assert_cmd`)
   IS the correct invocation. The in-process `InProcHarness` cannot
   be used because `install_recorder()` is process-global and would
   panic on the second scenario. Documented in `driver.md` § 1-2
   and `proposals.md` § "How slice-6 scenarios run".
6. The 6 sub-deliverables (A through F) are roughly INDEPENDENT and
   can be implemented + landed in any order. Suggested order based
   on dependency chain:
   - C (SubscriberGauge) — pure type addition; no other code depends
   - B (pool stats + poll task) — small addition to Store + main
   - A (middleware factory) — adds tower layer + uses MatchedPath
   - E (events.rs guard wire-up) — one-line, depends on C
   - D (startup probe) — small addition; depends on A being wired
     (so the recorder has something to render)
   - F (criterion microbench) — depends on A; runs as `cargo bench`
