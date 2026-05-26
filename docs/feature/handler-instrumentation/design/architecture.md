# Application Architecture — handler-instrumentation (slice 6)

Owner: solution-architect (Morgan). Slice-specific design summary. Inherits
the entire slice-1 architecture by reference; does NOT restate the 5-crate
workspace, dependency direction, CSRF model, SSE topology, sidecar metrics
listener topology, or recorder install path.

## Slice Summary

Slice 6 is a brownfield instrumentation slice that emits the metric series the
already-shipped Grafana "Foundry Overview" dashboard panels reference. The
DEVOPS slice (commit `c7cb715`) wired the `metrics_exporter_prometheus`
recorder + sidecar axum listener on `METRICS_PORT` (default 9090) and shipped
the dashboard pre-provisioned in `observability/grafana-dashboards/foundry-overview.json`.
The recorder is installed but nothing emits the metrics the panels graph —
the empty-series state in production is the explicit "instrument me" signal
per the DEVOPS evolution doc. Slice 6's whole job: emit those series, without
inflating handler signatures, blowing up label cardinality, or regressing the
slice-1 architectural promises.

## Inheritance

- **Workspace shape** — unchanged from `docs/feature/foundry-backend-mvp/design/adrs/ADR-001.md`.
  No new crates. All slice-6 code lands in existing files within
  `foundry-app`, `foundry-store`, `foundry-realtime` (hybrid hosting per D5).
- **Observability spec** — `docs/feature/foundry-backend-mvp/design/system/observability-infra.md`
  enumerates 10 metric families; slice 6 ships the 5 that have dashboard
  consumers today (see D0). The remaining 5 are deferred follow-ups.
- **Sidecar metrics listener** — unchanged from `crates/foundry-app/src/metrics_server.rs`
  (DEVOPS slice / commit c7cb715). Recorder install + `/metrics` endpoint
  exposition remain the file's responsibility. Slice 6 EXTENDS the file
  with a request-tracking middleware factory (D1).
- **Recorder lifecycle** — single install per process at `main.rs` startup.
  Slice 6 introduces no second install path. Acceptance harness continues
  to skip recorder install per the existing comment in `metrics_server.rs`.
- **Sidecar listener port** — `METRICS_PORT` (default 9090), bound to the
  container network only. Unchanged.
- **Dashboard provisioning** — `observability/grafana-dashboards/foundry-overview.json`
  unchanged. Slice 6 emits the metric series the existing panels reference.

## Metrics shipped this slice (5 of 10)

The 5 dashboard-referenced metric families this slice emits:

| Metric | Type | Labels | Dashboard panel | Emitted by |
|---|---|---|---|---|
| `http_requests_total` | counter | `path`, `method`, `status` | Panel 1 (req rate by status), Panel 3 (error rate) | `request_tracking_layer` in `foundry-app::metrics_server` |
| `http_request_duration_seconds` | histogram | `path`, `method`, `status` | Panel 2 (req latency p95 by path) | `request_tracking_layer` |
| `db_connections_in_use` | gauge | (none) | Panel 5a (Postgres pool in_use) | Background polling task in `foundry-app::main` |
| `sse_subscribers_total` | gauge | `project_id` | Panel 4 (active SSE subscribers) | `foundry-realtime::SubscriberGauge` (RAII) |
| `foundry_app_startup_total` | counter | (none) | (already shipped; unchanged) | `foundry-app::main` (already emitted by DEVOPS slice) |

The `foundry_app_startup_total` line is the existing DEVOPS-slice probe;
the slice-6 startup probe (D6) leverages it to prove the recorder
actually accepted the increment.

## Metrics deferred (5 of 10) — see D0

Per D0 (Q0 scope decision), these 5 metric families documented in
`observability-infra.md` are deferred. Each has no dashboard consumer
today; shipping them would violate slice-1 "smallest thing that satisfies
the AC" discipline.

| Deferred metric | Reason for deferral |
|---|---|
| `outbox_pending_jobs` | No dashboard panel; outbox depth is operationally meaningful only if a backlog actually exists, which slice-1 load profile (0.25 req/sec) does not produce. |
| `bootstrap_tokens_unclaimed` | No dashboard panel; tokens are short-lived (15 min TTL) and an operator query against `bootstrap_tokens` table suffices for the rare manual check. |
| `migration_apply_duration_seconds` | No dashboard panel; migration timing is a one-shot startup concern, already logged structurally by sqlx. |
| `realtime_listen_disconnects_total` | No dashboard panel; LISTEN/NOTIFY reconnect is automatic per slice-2 PgListener task — operator alerting would be the consumer, not yet defined. |
| `probe_failures_total` | No dashboard panel; probe failures already cause the process to refuse to start (slice-5 precedent) — a counter for "things that already crashed the container" is redundant. |
| `db_connection_wait_seconds` | Listed in the dashboard but deferred per D3: no clean sqlx 0.8 acquire hook; requires wrapping ~30 Store query sites. Panel stays half-empty matching the DEVOPS "instrument me" precedent. |

The first 5 are documented-but-unconsumed. The 6th (`db_connection_wait_seconds`)
IS dashboard-referenced but has a structural blocker (sqlx 0.8 internal hooks
are not public); see D3 for the revisit condition.

## Architecture decisions summary

**D1 — Recording strategy: tower middleware on the router (CHOSEN: A)**.
A single `tower::Layer` wired via `.layer()` in `build_router` extracts
`MatchedPath` (axum 0.8 native), method, and observed status, then emits one
`metrics::counter!("http_requests_total", ...)` increment and one
`metrics::histogram!("http_request_duration_seconds", ...)` observation per
request. Zero handler-signature changes; every current AND future route
auto-instrumented; preserves the slice-1 "handlers stay thin" property
(ADR-001). Captured in **ADR-010**.

**D2 — Label cardinality: bounded triple `{path, method, status}` (CHOSEN: A)**.
`path` uses `MatchedPath` route template (e.g.,
`/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments`),
NEVER a concrete URI. `method` is the HTTP verb. `status` is the full
3-digit HTTP status code. Forbidden label keys (cardinality-unbounded)
are enumerated below. ~800 counter series + ~8000 histogram series at
27 routes — manageable. Captured in **ADR-011**.

**D3 — DB pool gauge: 5s polling task; wait-histogram deferred (CHOSEN: C)**.
Background tokio task in `foundry-app::main` reads `pool.size()` +
`pool.num_idle()` every 5 seconds and updates `db_connections_in_use`.
`db_connection_wait_seconds` panel stays empty pending a v0.2 revisit
(either sqlx exposes acquire hooks or operational pain forces the
`TimedPool` wrapper). Poll interval << 15s Prometheus scrape interval
guarantees the gauge is at most one scrape behind reality. Captured in
**ADR-012**.

**D4 — SSE subscriber gauge: RAII guard in `foundry-realtime` (CHOSEN: A)**.
New `pub struct SubscriberGauge { project_id: Uuid }` with `new(project_id)`
(increments gauge) and `Drop::drop` (decrements gauge). The SSE handler in
`foundry-app::events` constructs one per subscription via
`let _gauge = SubscriberGauge::new(project_id);`. Drop semantics handle
stream termination, panic, and shutdown uniformly. One-line handler
change. Captured in **ADR-013**.

**D5 — Code hosting: hybrid; no new crate (CHOSEN: C)**. Recorder install
+ middleware factory live in existing `foundry-app::metrics_server` (a
single file owning ALL request-metric emission setup).
`Store::pool_stats()` is a new method on the existing `Store` adapter — no
new file. `SubscriberGauge` is a new ~30-line module in `foundry-realtime`
(the natural cohesive home, since the subscriber concept already lives
there). Honors slice-1 ADR-001 "no new crates without explicit need";
zero workspace.toml change. Documented inline (not promoted to standalone
ADR because the rationale is "precedent inheritance", which would amount
to re-stating ADR-001).

**D6 — Startup probe: self-scrape `/metrics`; refuse to start on fail
(CHOSEN: A)**. Extend the existing startup probe sequence to
self-scrape `http://127.0.0.1:{METRICS_PORT}/metrics` after the sidecar
listener binds. The probe asserts (a) HTTP 200, (b) non-empty body,
(c) body contains the `foundry_app_startup_total` line (proves the
recorder actually accepted the counter the DEVOPS slice emitted at
startup). On failure: structured `health.startup.refused` log + non-zero
process exit; container orchestrator restarts. Inherits slice-5 evolution
doc lesson #6 ("probe the substrate lie"). Captured in **ADR-014**.

**D7 — Performance budget: ≤10µs P95 added per request (CHOSEN: 10µs)**.
A 5× safety margin on the expected 2µs per request
(`MatchedPath` lookup ~100ns + two `Instant::now()` calls ~50ns each +
lock-free counter + histogram emission ~200-500ns each). Comfortably
within both NFR-PERF-01 (200ms P95 budget) and the slice-1 measured 4ms
P95. Verified via a microbench called out for platform-architect (see
"Performance budget" section below). Documented inline; no standalone
ADR (perf budgets are NFRs, not architectural decisions in the ADR sense).

## Component diagram (C4 Level 3) — request and metric-emission paths

```mermaid
sequenceDiagram
    autonumber
    participant B as Browser
    participant R as axum Router
    participant L as request_tracking_layer (tower)
    participant H as Handlers (comments, issues, ...)
    participant ST as Store (foundry-store)
    participant PG as Postgres
    participant REC as PrometheusHandle (metrics_server.rs)
    participant SG as SubscriberGauge (foundry-realtime)
    participant Poll as Background poll task (main.rs)
    participant Prom as Prometheus (external)

    Note over B,Prom: REQUEST PATH (every HTTP request)
    B->>R: HTTP request
    R->>L: enters tower layer (extracts MatchedPath + method)
    L->>L: Instant::now() (start)
    L->>H: forwards request
    H->>ST: query (implicit pool acquire)
    ST->>PG: SQL
    PG-->>ST: rows
    ST-->>H: data
    H-->>L: HTTP response (status observed)
    L->>L: Instant::now() (end); compute duration
    L->>REC: counter!(http_requests_total, path, method, status).inc
    L->>REC: histogram!(http_request_duration_seconds, path, method, status).record
    L-->>R: forwards response
    R-->>B: response

    Note over Poll,REC: BACKGROUND POOL POLLING (every 5s)
    loop every 5s
        Poll->>ST: Store::pool_stats()
        ST-->>Poll: PoolStats { in_use, idle, size }
        Poll->>REC: gauge!(db_connections_in_use).set(in_use)
    end

    Note over B,REC: SSE SUBSCRIBER LIFECYCLE (events.rs)
    B->>R: GET /events/{project_id}
    R->>H: events::sse_handler
    H->>SG: SubscriberGauge::new(project_id)  [increments gauge]
    SG->>REC: gauge!(sse_subscribers_total, project_id).inc
    H-->>B: SSE stream (long-lived)
    Note right of H: stream lives for minutes
    B--xH: client disconnects (or shutdown)
    H->>SG: Drop::drop  [automatic; decrements gauge]
    SG->>REC: gauge!(sse_subscribers_total, project_id).dec

    Note over Prom,REC: SCRAPE (every 15s, sidecar listener on :9090)
    Prom->>REC: GET /metrics
    REC-->>Prom: text exposition (all metric families)
```

Key property the diagram makes obvious: all `metrics::*` macro emissions
converge into the single `PrometheusHandle` installed at startup by
`metrics_server::install_recorder()`. No second install path. No cross-crate
facade abstraction. The recorder is the single point of authority for the
Prometheus exposition.

## Component changes — extends Reuse Analysis

Verbatim from `proposals.md` §1, finalized for the chosen options:

| Action | Target | Why | LOC delta |
|---|---|---|---|
| EXTEND | `crates/foundry-app/src/metrics_server.rs` | Add `request_tracking_layer()` factory (tower middleware) so the file owns BOTH recorder install AND request-tracking middleware. Add `probe(handle, addr)` for startup self-scrape (D6). | +~85 |
| EXTEND | `crates/foundry-app/src/lib.rs` (`build_router`) | Add `.layer(metrics_server::request_tracking_layer())` near existing CSRF/session layers. One-line change. | +1 |
| EXTEND | `crates/foundry-app/src/main.rs` | Spawn background polling task (every 5s, samples `Store::pool_stats()`, updates `db_connections_in_use` gauge). Call `metrics_server::probe()` after sidecar `serve()` returns. | +~50 |
| EXTEND | `crates/foundry-store/src/lib.rs` | Add `Store::pool_stats() -> PoolStats { in_use, idle, size }` — read-only snapshot using `Pool::size()` + `Pool::num_idle()`. | +~20 |
| EXTEND | `crates/foundry-realtime/src/lib.rs` | Add `pub struct SubscriberGauge { project_id: Uuid }` with `new` + `Drop`. Add `metrics` to the crate's `Cargo.toml`. | +~30 |
| EXTEND | `crates/foundry-app/src/events.rs` | Insert `let _gauge = foundry_realtime::SubscriberGauge::new(project_id);` at SSE-subscription time. | +~3 |
| EXTEND | `crates/foundry-store/Cargo.toml` | Add `metrics = { workspace = true }` (existing workspace declaration; no new dep). | +1 |
| EXTEND | `crates/foundry-realtime/Cargo.toml` | Add `metrics = { workspace = true }`. | +1 |
| CREATE NEW | none | Per D5 hybrid hosting. Honors ADR-001 "no new crates" precedent. | — |

**Total estimated delta**: ~190 LOC of Rust + 2 manifest lines. Smaller than
slice 5 (~340 LOC) because no SQL migration, no new HTTP verbs, no new SSE
event_types.

## Performance budget — ≤10µs P95 per request

Per D7. The middleware's per-request work:

| Operation | Expected cost | Notes |
|---|---|---|
| `MatchedPath` extension lookup | ~100ns | axum stores it as a request extension; hashtable lookup. |
| `Instant::now()` × 2 | ~50ns each on Linux | vDSO-backed `clock_gettime(CLOCK_MONOTONIC)`. |
| `metrics::counter!.increment(1)` | ~200-500ns | Lock-free atomic via `metrics` crate facade. |
| `metrics::histogram!.record(d)` | ~200-500ns | Same lock-free path; bucket-find is constant time. |

Total expected: ≤2µs per request. Budget is ≤10µs P95 (5× safety margin
for cache misses, atomic contention spikes, bucket-find pathological cases).
That's 0.005% of the NFR-PERF-01 200ms P95 budget and 0.25% of the slice-1
measured 4ms P95.

**Measurement plan** (called out to platform-architect):

1. Microbench harness (criterion or `wrk` + `hyperfine` shell script) that
   measures P50/P95/P99 added overhead per request by toggling the layer
   on/off against a no-op handler.
2. CI gate: assert P95 added overhead < 10µs across the 27 routes.
3. Background tasks (pool poll, SSE gauge inc/dec) contribute zero per-request
   overhead — they run asynchronously.

## Cardinality safety guarantees (D2 forbidden-labels list)

The request-tracking middleware MUST hard-code its emitted label set to
exactly `{path, method, status}`. The following label keys are FORBIDDEN
on `http_requests_total` and `http_request_duration_seconds`:

- `user_id` (unbounded — one series per registered user)
- `workspace_id` (unbounded long-term — one series per workspace)
- `team_id` (unbounded — one series per team)
- `project_id` (unbounded — one series per project; allowed only on
  `sse_subscribers_total` where it bounds to active SSE projects)
- `issue_id` (unbounded — one series per issue ever observed)
- `comment_id` (unbounded)
- `session_id` (unbounded)
- `request_id` (effectively infinite — UUID per request)
- IP address (~4B IPv4 possible series, larger for IPv6)
- User-Agent string (effectively infinite)

**Enforcement strategy**: a unit test in `metrics_server.rs` that exercises
the middleware against a representative request and asserts the emitted
label key set is EXACTLY `{path, method, status}`. Cardinality regression
is the long-term failure mode for this slice; a static check catches new
contributors' foot-guns before production. Future ADRs would be required
to add any high-cardinality label to these metric families.

**`MatchedPath` fallback rule**: requests that don't match any router route
(404 to a path the router doesn't know) MUST emit with the literal label
`path="<unmatched>"` so 404s to random URIs don't mint a series per URI.
Cardinality safety.

## Quality attributes addressed

| Attribute | Mechanism |
|---|---|
| Observability completeness (HIGH) | 5 dashboard-referenced metric families emitted via the slice's whole purpose — middleware (D1) + pool poll (D3) + RAII gauge (D4). Empty-series state ended. |
| Performance / per-request overhead (HIGH) | ≤10µs P95 budget (D7); 5× safety margin on expected 2µs; 0.005% of NFR-PERF-01. |
| Cardinality safety (HIGH) | Bounded triple `{path, method, status}` (D2 / ADR-011) + `MatchedPath` route template (never concrete URI) + forbidden-labels list + cardinality unit test. |
| Maintainability (HIGH) | Zero handler-signature changes (D1); zero new crates (D5); zero new dependencies; new metric families added via the same RAII / middleware patterns established here. |
| Recorder lifecycle correctness (MEDIUM) | Single-shot install at process startup preserved; acceptance harness's existing skip-install path preserved; no second install path introduced. |
| Forward-compat for deferred 5 metrics (MEDIUM) | Middleware pattern extends naturally to additional counters/histograms; RAII pattern extends to other lifetime-bound gauges; pool-poll pattern extends to other periodic samples. |
| Startup-time correctness (HIGH) | Self-scrape probe (D6 / ADR-014) catches misconfig classes — wrong `METRICS_PORT`, port-in-use, recorder install swallowed, firewall between app and own port. |

## What is OUT OF SCOPE for this slice

- The 5 documented-but-unconsumed metrics (`outbox_pending_jobs`,
  `bootstrap_tokens_unclaimed`, `migration_apply_duration_seconds`,
  `realtime_listen_disconnects_total`, `probe_failures_total`). No consumer
  today; ship when there is one.
- The `db_connection_wait_seconds` histogram (D3 deferral). Panel stays
  half-empty pending sqlx hook exposure or operational forcing function.
- New Grafana dashboard panels beyond the existing 6. Operators add their
  own.
- Alerting rules. Operators define their own per ADR-104 minimal-by-default.
- Distributed tracing (OTLP). On the roadmap, not this slice.
- Helm/Kustomize templating of `METRICS_PORT`. DEVOPS owns deploy.

## External integration check (principle 10)

NONE new. Prometheus is already in the architecture (DEVOPS slice). The
metric scrape is a Prometheus-PULL relationship — Prometheus connects to
the sidecar listener; foundry never connects out to Prometheus. The
contract-test annotation from slice 1 (SMTP) remains unchanged; no new
annotation needed.

## Architecture enforcement (principle 11)

Existing tooling suffices; one addition recommended:

- `cargo xtask check-arch` — no crate-boundary changes; hybrid layout (D5)
  preserves the existing dependency graph (`foundry-app` continues to be
  the only crate that knows about axum/tower; `foundry-realtime` and
  `foundry-store` gain only the `metrics` facade crate, which is I/O-free).
- `cargo deny check` — zero new dependencies; workspace already declares
  `metrics` and `metrics-exporter-prometheus` per commit c7cb715.
- `cargo sqlx prepare --check` — no new SQL queries (pool_stats uses
  `Pool::size()` + `Pool::num_idle()` accessors, not SQL).
- **New static check** (called out to platform-architect): unit test in
  `metrics_server.rs` asserting the request middleware's emitted label keys
  are EXACTLY `{path, method, status}`. ~20 LOC. Catches cardinality
  regression — the long-term failure mode for this slice.

The `foundry-core` I/O-free invariant remains unchanged: `foundry-core`
does not gain a `metrics` dependency (none of the slice-6 changes touch
the domain crate).

## Earned Trust (principle 12)

This slice introduces three new adapter contracts; each gets a probe:

1. **`request_tracking_layer`** (driving adapter wrapping every handler).
   Contract: every routed request produces exactly one counter increment
   + one histogram observation, with label set `{path, method, status}` and
   no others. Probe: acceptance scenario hits N distinct endpoints, then
   scrapes `/metrics`, asserts (a) counter sum equals N, (b) only the
   permitted label keys appear, (c) the forbidden-labels static check
   continues to pass.

2. **Pool-polling task** (driven adapter reading sqlx pool internals).
   Contract: gauge value matches `pool.size() - pool.num_idle()` within
   one poll interval (5s). Probe: post-startup self-test acquires a
   connection + holds for >5s + releases, scrapes `/metrics`, asserts the
   gauge transitions from baseline -> non-zero -> baseline.

3. **`SubscriberGauge`** (driven adapter wrapping the subscribe lifecycle
   via RAII). Contract: gauge returns to zero after all subscribers drop;
   Drop fires on stream termination, client disconnect, and panic unwind.
   Probe: extension of existing US-09 SSE acceptance scenario asserts
   `sse_subscribers_total` returns to zero post-disconnect.

Plus the metrics-listener startup probe (D6 / ADR-014): self-scrape
`/metrics` during startup; refuse to serve traffic if it returns non-200,
empty body, or omits the `foundry_app_startup_total` line. This is the
principle-12 application to the metrics sidecar itself — the listener
is a driven adapter exposing `/metrics` for Prometheus to consume, and
the probe verifies the contract empirically before traffic flows.

## ADRs created

- `adrs/ADR-010-recording-strategy-tower-middleware.md` — D1 outcome (tower middleware)
- `adrs/ADR-011-label-cardinality-bounded-triple.md` — D2 outcome (`{path, method, status}` only)
- `adrs/ADR-012-pool-gauge-poll-based-with-deferred-wait-histogram.md` — D3 outcome (5s poll; defer wait)
- `adrs/ADR-013-sse-subscriber-raii-guard.md` — D4 outcome (RAII guard)
- `adrs/ADR-014-startup-metrics-probe.md` — D6 outcome (self-scrape probe)

D5 (hybrid hosting) and D7 (perf budget) are settled inline in this
document — D5 is precedent inheritance from ADR-001 (no new architectural
decision); D7 is an NFR consequence (no consequentially independent
trade-off to record).
