# DISTILL Driver Design — Slice 6 Acceptance Harness (handler-instrumentation)

Owner: acceptance-designer (DISTILL). Companion: `step-skeletons.md`,
`coverage-matrix.md`, `wave-decisions.md`, `proposals.md`. This
document is an **additive delta** to:

- `docs/feature/foundry-backend-mvp/distill/driver.md` (slice 1)
- `docs/feature/foundry-realtime-collab/distill/driver.md` (slice 2 —
  SSE consumer + HTML assertions + heartbeat env override)
- `docs/feature/foundry-operator-grade/distill/driver.md` (slice 3 —
  multi-replica + backup-restore + attachments + `assert_cmd` subprocess
  pattern)
- `docs/feature/foundry-contributor-onboarding/distill/driver.md`
  (slice 4 — subprocess walking skeleton)
- `docs/feature/comment-edit-delete/distill/driver.md` (slice 5 — zero
  new infra; additive step file only)

Everything not mentioned here is inherited unchanged.

## 1. What slice 6 reuses (existing infrastructure only)

| Adapter / helper | Reused from | Slice-6 use |
|---|---|---|
| `support::harness::ensure_postgres()` + `fresh_schema_pool_with_url()` | slice 1 `harness.rs` | All 9 automated slice-6 scenarios use a per-scenario PG schema. The schema URL is passed to the foundry subprocess via `DATABASE_URL` env. |
| Testcontainers Postgres-16 container | slice 1 | Same shared container; no new resource pressure |
| `assert_cmd::Command::cargo_bin("foundry")` | slice 3 (us_03_backup_restore.rs) | Slice-6 spawns a foundry subprocess per scenario — the only honest way to test the `/metrics` substrate (the in-process `InProcHarness` deliberately skips `install_recorder` per the comment in metrics_server.rs line 26-27). |
| `reqwest::Client` | slice 1+ | Used by the new `metrics_scrape` helper to GET `/metrics`. |
| `tokio::time::sleep` (no fake clock; real wall-clock waits) | std | Scenario #5 acquires + holds a DB connection for ~6s to cover one full poll-task tick. Wall-clock necessity (the production poll task uses real `tokio::time::interval`). |

## 2. What slice 6 adds to the harness

**ONE new support module + ONE new step file + minimal world/lib/test
edits.** Production code is untouched per task brief.

### 2a. NEW: `support/metrics_scrape.rs` — Prometheus text-exposition consumer

Analogous to slice-2's `support/sse_client.rs`. ~80 LOC. Does
`reqwest::get` against the subprocess's bound metrics URL and parses
the Prometheus text-exposition format into typed structs the step
bodies assert against. NO new crate dependency — the parsing surface
needed is small enough (~30 LOC parser) to avoid pulling in
`prometheus-parse` or similar.

Public surface:

```rust
/// One metric sample as exposed in the Prometheus text format.
/// Example line: `http_requests_total{path="/healthz",method="GET",status="200"} 5`
#[derive(Clone, Debug, PartialEq)]
pub struct MetricSample {
    pub name: String,
    pub labels: std::collections::BTreeMap<String, String>,
    pub value: f64,
}

#[derive(Debug)]
pub struct ScrapeSnapshot {
    pub raw_body: String,
    pub samples: Vec<MetricSample>,
}

impl ScrapeSnapshot {
    /// Return all samples whose name matches `name` exactly.
    pub fn samples_for(&self, name: &str) -> Vec<&MetricSample>;

    /// Return all samples whose name starts with `prefix` (e.g.
    /// `"http_request_duration_seconds_bucket"` to collect histogram
    /// bucket lines).
    pub fn samples_with_prefix(&self, prefix: &str) -> Vec<&MetricSample>;

    /// Sum of `value` across all samples named `name`.
    pub fn sum_for(&self, name: &str) -> f64;

    /// Return true if the body contains the exact line prefix
    /// `{name}{ ` or `{name} ` (Prometheus exposition format markers).
    /// Cheap pre-check before parsing.
    pub fn contains_metric_line(&self, name: &str) -> bool;

    /// Collect the set of label KEYS used across all samples whose
    /// metric NAME matches `name`. Used by the cardinality safety
    /// scenario to assert no forbidden keys appear.
    pub fn label_keys_for(&self, name: &str) -> std::collections::BTreeSet<String>;
}

/// Scrape `http://{addr}/metrics` and parse the response. Panics on
/// HTTP error, non-200, or parse error — these are test failures, not
/// recoverable conditions.
pub async fn scrape_metrics(addr: std::net::SocketAddr) -> ScrapeSnapshot;

/// As above but returns the raw `reqwest::Response` so the caller can
/// assert on the status / headers. Used by the startup-probe
/// success scenario (#9).
pub async fn scrape_metrics_raw(addr: std::net::SocketAddr) -> (reqwest::StatusCode, String);
```

Parsing rules (Prometheus text-exposition subset we handle):

- Lines beginning with `#` are HELP / TYPE comments — skipped.
- Blank lines — skipped.
- Other lines parse as `{name}{labels?} {value}` where `{labels?}` is
  an optional `{k1="v1",k2="v2"}` block.
- Values parse as `f64`; `NaN`, `+Inf`, `-Inf` supported.
- Label values containing quotes/backslashes are decoded per the
  exposition spec (the subset slice-6 emits is plain — no escapes —
  but the parser handles the basic escape sequences for robustness).
- Histogram bucket lines (e.g.
  `http_request_duration_seconds_bucket{le="0.005"}` ) are parsed
  generically; the caller uses `samples_for` / `samples_with_prefix`
  to slice the histogram.

The parser is intentionally minimal — slice 6 emits only counters,
gauges, and histograms, all with single-value samples; no summary
families; no `# TYPE` introspection required.

### 2b. NEW: subprocess spawn helper (lives inside `steps/handler_instrumentation.rs`)

Each slice-6 scenario spawns a fresh `foundry` subprocess. The helper
pattern (one helper, ~50 LOC, lives in the step file rather than a
new support module per slice-5's "no new support files" precedent
where reasonable):

```rust
struct FoundrySubprocess {
    process: std::process::Child,
    main_addr: std::net::SocketAddr,
    metrics_addr: std::net::SocketAddr,
    db_schema: String,
}

impl FoundrySubprocess {
    /// Spawn the foundry binary as a subprocess with:
    ///   - DATABASE_URL pointing at the slice-1 testcontainers Postgres
    ///     + per-scenario PG schema (slice-1 `fresh_schema_pool_with_url`)
    ///   - METRICS_PORT=0 (request ephemeral)
    ///   - FOUNDRY_PORT=0 (request ephemeral)
    ///   - SESSION_SECRET=<32-byte test fixture>
    ///   - SESSION_COOKIE_SECURE=false
    ///   - METRICS_HOST=127.0.0.1
    ///   - FOUNDRY_PUBLIC_URL=http://127.0.0.1:0 (placeholder; subprocess
    ///     overrides at runtime once it knows the bound port)
    /// Waits up to 10 seconds for both ports to bind (by parsing
    /// the structured `foundry listening on {addr}` and
    /// `foundry metrics listening on {addr}` log lines from stderr).
    /// Returns the addrs + the per-scenario schema name (for teardown).
    async fn spawn(database_url_with_schema: &str, db_schema: String)
        -> anyhow::Result<Self>;

    /// SIGTERM the subprocess and wait up to 5s for clean exit.
    /// Called from Drop too as belt-and-braces.
    fn shutdown(self);
}

impl Drop for FoundrySubprocess {
    fn drop(&mut self) {
        // Best-effort kill; the test runner's process group cleanup is
        // the ultimate safety net.
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}
```

The structured log lines this helper parses are ALREADY emitted by
`main.rs` lines 142 + 147 (slice 1 unchanged):
- `foundry metrics listening on {addr}` (line 142)
- `foundry listening on {addr}` (line 147)

These are the subprocess's `tracing::info!` output to stderr in JSON
(NFR-OBS-01) or pretty format (depending on `RUST_LOG_FORMAT`); the
helper greps for the addr substring with a small regex.

**No production-code change required for the addr discovery** — the
log lines already exist.

### 2c. NEW step file: `steps/handler_instrumentation.rs`

The slice-6 work lands in exactly ONE new step file + 5 small
test-side edits:

1. NEW: `crates/foundry-acceptance/src/steps/handler_instrumentation.rs`
   (the step body file — scaffolded RED in DISTILL, filled in by
   DELIVER).
2. NEW: `crates/foundry-acceptance/src/support/metrics_scrape.rs`
   (the scrape helper — described above; minimal happy-path
   implementation in DISTILL so the step bodies type-check).
3. EDIT: `crates/foundry-acceptance/src/lib.rs` — append one line in
   the `pub mod steps { ... }` block to register the new module.
4. EDIT: `crates/foundry-acceptance/src/support/mod.rs` — append one
   line to register the new `metrics_scrape` support module.
5. EDIT: `crates/foundry-acceptance/tests/acceptance.rs` — append one
   force-link `use foundry_acceptance::steps::handler_instrumentation
   as _slice6;`.
6. EDIT: `crates/foundry-acceptance/src/world.rs` — append eight
   `Option`/`HashMap`-typed fields under a new `// ---- Slice 6:
   handler-instrumentation ----` block at the bottom of the
   `FoundryWorld` struct (matching the slice-4 + slice-5 convention).

All six edits are test-infrastructure changes; production code is
untouched per the task brief.

## 3. World struct additions (`FoundryWorld`)

Slice 6 adds eight fields. All default to empty `HashMap` or `None`;
existing slice-1-through-slice-5 scenarios are unaffected.

```rust
// ---- Slice 6: handler-instrumentation ----
/// The foundry subprocess for the current scenario. Spawned by the
/// "the operator's foundry instance is running" Given (or equivalent
/// in each scenario's wording). Dropped at scenario teardown (its
/// Drop impl kills + reaps the child process).
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
pub slice6_request_count_by_route: std::collections::HashMap<(String, String), u64>,
/// The SSE subscription opened in scenarios #7 + #8. Distinct from
/// `us_09_subscriptions` because this one rides through a foundry
/// SUBPROCESS, not the in-process harness, so its connection
/// management differs. Drop = client-side abrupt close.
pub slice6_sse_subscription: Option<reqwest::Response>,
/// The connection acquired-and-held in scenario #5 (forces
/// `db_connections_in_use` to be > 0 for at least one poll tick).
/// Held as a long-lived sqlx connection from the per-scenario
/// schema pool. Dropped to release.
pub slice6_held_connection: Option<sqlx::pool::PoolConnection<sqlx::Postgres>>,
/// Per-scenario PG schema name (slice-1 pattern). Captured so
/// teardown can drop it. The subprocess connected via DATABASE_URL
/// with this schema pinned via search_path.
pub slice6_schema: Option<String>,
```

## 4. Step phrase contracts (slice-6 inventory)

Per `step-skeletons.md`. Slice 6 registers **NEW** phrases only — no
existing slice-1..5 phrase is touched. Step-method names follow
slice-1..5 style: `fn given_*`, `fn when_*`, `fn then_*`.

### Givens (3 new)
- `^the operator's foundry instance is running$`
- `^the operator's foundry instance has been running for at least (\d+) seconds$`
- `^Mei has subscribed to events on "([^"]+)"$` (slice-6 SSE
  subscription opens against the SUBPROCESS, distinct from slice-2
  in-process pattern — different World slot)

### Whens (7 new)
- `^the operator scrapes the metrics endpoint$`
- `^the operator scrapes the metrics endpoint immediately$`  (scenario #6 — before first poll tick)
- `^Mei posts a comment on "(\w+)-(\d+)" with body "([\s\S]*)"$` (re-routed
  through the subprocess; uses the existing slice-2 POST contract)
- `^the operator issues (\d+) HTTP requests across the routes "([^"]+)" and "([^"]+)"$`
- `^the operator issues (\d+) HTTP requests to "([^"]+)"$`
- `^Mei holds an open database connection for (\d+) seconds$`
- `^Mei abruptly disconnects from the SSE stream$`

### Thens (12 new)
- `^the scrape returns HTTP 200$`
- `^the scrape body contains the line "([^"]+)"$`
- `^the scrape body contains a sample for "([^"]+)" with labels "([^"]+)"$`
- `^the scrape body's "([^"]+)" sample sums to (\d+)$`
- `^the scrape body's "([^"]+)" sample has value (\d+)$`
- `^the scrape body's "([^"]+)" samples carry only the label keys "([^"]+)"$`
- `^the scrape body's "([^"]+)" samples do NOT carry any of the label keys "([^"]+)"$`
- `^the scrape body's "([^"]+)" sample's "([^"]+)" label is "([^"]+)"$` (route-template assertion)
- `^the scrape body's "([^"]+)" histogram has at least one bucket with count >= (\d+)$`
- `^the scrape body's "([^"]+)" sample is greater than (\d+)$` (gauge non-zero assertion)
- `^the scrape body's "([^"]+)" sample returns to (\d+)$` (gauge round-trip)
- `^the foundry subprocess is alive$` (smoke check for scenario #9 + scenario shutdown ordering)

### Inherited (reused unchanged from slice 2/5)

- Background phrases for workspace/member/team/project/issue/sign-in
  seeding (slice-1 + slice-2 modules). Slice 6 reuses them but routes
  the resulting state mutations through the SUBPROCESS, not the
  in-process harness. The Background steps populate the per-scenario
  schema via direct SQL (slice-1 `support::harness::ensure_postgres()`
  + `fresh_schema_pool_with_url`) BEFORE the subprocess spawns; the
  subprocess starts against the already-seeded schema.

cucumber-rs treats step phrases as globally unique; the new phrases
above were verified non-colliding by compile (`cargo check -p
foundry-acceptance --tests`).

## 5. Per-scenario isolation — subprocess pattern

The slice-1 invariant holds: per-scenario PG schema, shared
container. Slice 6 adds per-scenario foundry SUBPROCESS:

- Each scenario calls `fresh_schema_pool_with_url()` (slice-1) to
  provision a fresh schema + get back the `postgres://...?options=-csearch_path%3D{schema}`
  URL.
- The scenario seeds Background state (workspace, members, project,
  issue) via direct SQL against the schema pool — fast, deterministic.
- The scenario spawns the foundry subprocess with that DATABASE_URL +
  ephemeral METRICS_PORT + ephemeral FOUNDRY_PORT.
- The subprocess runs migrations into the schema (it doesn't know
  the schema is pre-seeded — `Store::migrate` is idempotent).
- All subsequent HTTP interactions in the scenario go through the
  subprocess (slice-1 `reqwest::Client` against the subprocess's bound
  `FOUNDRY_PORT`).
- The metrics scrape goes through the subprocess's bound
  `METRICS_PORT`.
- At scenario teardown:
  1. `FoundrySubprocess::Drop` kills + reaps the subprocess.
  2. Slice-1 schema drop reclaims the PG schema.
  3. World fields drop; the held connection (if any) returns to the
     dead schema pool (a brief warn line; safe).

Concurrency: the slice-3 `--max-concurrent-scenarios 6` cap holds.
Each subprocess is ~50–80MB resident; 6 concurrent slice-6 scenarios
= ~300–500MB peak. Within typical dev-laptop budgets.

## 6. Real-I/O budget — slice 6 adds ~40s on top of slice 5

Per `proposals.md` + `wave-decisions.md` § "Suite-time budget":

| Scenario | Cost estimate | Notes |
|---|---|---|
| 1 walking skeleton: scrape after one POST | ~3.5 s | subprocess (~2s) + 1 POST + scrape + assertions |
| 2 http_requests_total breakdown | ~4.0 s | subprocess + 5 requests + scrape + breakdown |
| 3 cardinality safety | ~3.5 s | subprocess + 1 param-route req + scrape + label-key assertion |
| 4 behavioral probe (counter == N) | ~4.0 s | subprocess + N requests + scrape + sum assertion |
| 5 db_connections_in_use reflects pool | ~8.5 s | subprocess + 6s held connection + scrape + assertion |
| 6 db_connections_in_use registered at 0 | ~3.0 s | subprocess + immediate scrape + assertion |
| 7 sse_subscribers_total round-trip (clean) | ~5.0 s | subprocess + open SSE + scrape + close + scrape + assert |
| 8 sse_subscribers_total Drop on abrupt disconnect | ~5.0 s | subprocess + open SSE + scrape + abrupt drop + wait + scrape + assert |
| 9 startup probe success | ~3.0 s | subprocess + scrape + line-present assertion |
| 10 @manual perf budget contract | (manual) | Hands off to DELIVER criterion microbench |
| **Subtotal (automated 1-9)** | **~39.5s** | within 60s top-line budget |

After slice 6, total suite wall-clock projects to ~162s. The fast-loop
iteration pattern (strip `@docker-compose` + `@manual`) excludes the
~80s slice-3 docker-compose Caddy scenario; fast-loop projects to
~70s including slice-6. For slice-6-only iteration:
`FOUNDRY_ACCEPTANCE_TAGS=@slice6 cargo test ...` runs in ~40s.

## 7. Tag conventions (additions only)

Inherited (unchanged): see `wave-decisions.md` § "Tag conventions
added".

Added in slice 6:
- `@slice6`, `@handler-instrumentation`, `@metrics`, `@cardinality`,
  `@startup-register`, `@startup-probe`, `@sse`, `@nfr-perf-05`.

`@nfr-obs-03`, `@real-io`, `@driving_adapter`, `@error`, `@manual`,
`@walking_skeleton` are reused unchanged.

## 8. CI invocation (delta only)

The slice-1/2/3/4/5 invocations stay as-is. The slice-6 scenarios
pick up automatically because they live under the same feature-files
root. The `--max-concurrent-scenarios 6` cap holds (slice 6 adds no
PG-contention-sensitive scenarios beyond the slice-3 baseline; each
scenario is one subprocess + a handful of HTTP requests).

Local fast loop for slice-6-only iteration:

```bash
cargo test -p foundry-acceptance --test acceptance -- -t "@slice6"
```

Note: the `@nfr-perf-05` @manual scenario is excluded by default
(matches the standing `@manual` exclusion in `tests/acceptance.rs`
lines 76 + 96). To include it explicitly (documentation-only; no
automated assertion):

```bash
FOUNDRY_ACCEPTANCE_TAGS=all cargo test -p foundry-acceptance --test acceptance
```

The perf-budget contract is honored at CI level via the criterion
microbench (sub-deliverable F in `wave-decisions.md`):

```bash
cargo bench -p foundry-app --bench middleware_overhead
```

## 9. Standing rules carried into DELIVER (additions)

- The `metrics_scrape` helper MUST handle the case where a counter
  family has ZERO samples (Prometheus exposition omits the metric
  line entirely if no series has been emitted). The
  `contains_metric_line` pre-check + `samples_for` returning empty
  vec covers this; tests assert presence/absence explicitly.
- Every scenario MUST tear down its subprocess via the World's Drop
  chain. The `FoundrySubprocess::Drop` impl kills + reaps; the slice-1
  schema drop releases the PG resources. Scenarios that hold a sqlx
  connection (#5) MUST drop the held connection BEFORE the schema
  drop fires (the test framework's Drop order handles this when the
  World drops its fields in declaration order).
- Subprocess port discovery via structured log line parsing — the
  helper greps stderr for `foundry listening on {addr}` and
  `foundry metrics listening on {addr}`. These lines already exist
  in `main.rs` (lines 142 + 147); DELIVER must not remove or rename
  them. If a future slice adds a `RUST_LOG_FORMAT=pretty` default,
  the helper's regex must continue to match both pretty and JSON
  formats.
- The metrics scrape is a `reqwest::get` against `http://127.0.0.1:{port}/metrics`.
  The subprocess binds `METRICS_HOST=127.0.0.1` per the slice-6
  acceptance env to avoid loopback firewall surprises in CI. ADR-014's
  production probe URL (also `127.0.0.1`) is consistent.
- Scenario #5 (db_connections_in_use reflects pool) is the slowest
  scenario at ~8.5s due to the 6-second connection-hold. The hold
  duration must be `> METRICS_POOL_POLL_SECONDS` to cover at least
  one full tick; we use 6s with a 1s safety margin over the 5s
  default. DELIVER can shorten by setting
  `METRICS_POOL_POLL_SECONDS=1` in the subprocess env for this
  specific scenario to speed it up — flagged as a future optimization,
  not required for slice-6 GREEN.
- Scenarios #7 + #8 hold the SSE subscription across multiple scrape
  calls. The subscription is a `reqwest::Response` whose body stream
  is implicitly kept alive by the client connection. Scenario #8
  ("abrupt disconnect") explicitly drops the Response (Drop closes
  the underlying TCP connection mid-stream); the SubscriberGauge's
  Drop on the server side fires when the SSE handler's send loop
  observes the broken pipe.
