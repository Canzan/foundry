# Wave Decisions — handler-instrumentation (slice 6)

DESIGN-wave decisions. All eight user picks (Q0 + Q1-Q7) accepted verbatim
from the proposals dialogue (no overrides). This document is the slice-6
handoff artifact for the DISTILL wave alongside `architecture.md`.

## DDD Decisions (D0 - D7)

| ID  | Question | Pick | Captured in |
|-----|----------|------|-------------|
| D0  | Metric scope | **5 dashboard-referenced** (defer 5 unconsumed) | `architecture.md` (inline) + table below |
| D1  | Recording strategy | **A** — Tower middleware on router; `MatchedPath` for cardinality | ADR-010 |
| D2  | Label cardinality | **A** — `{path, method, status}` 3-digit | ADR-011 |
| D3  | DB pool gauge update | **C** — 5s poll for `in_use`; defer `wait_seconds` histogram | ADR-012 |
| D4  | SSE subscriber gauge | **A** — RAII `SubscriberGauge` in `foundry-realtime` | ADR-013 |
| D5  | Code hosting | **C** — Hybrid: no new crate; recorder install + middleware in `metrics_server.rs`; inline modules elsewhere | `architecture.md` (inline) |
| D6  | Startup probe | **A** — Self-scrape `/metrics`; refuse to start on fail | ADR-014 |
| D7  | Performance budget | **≤10µs P95** added per request | `architecture.md` (inline) |

D1, D2, D3, D4, D6 are promoted to standalone ADRs because they constrain
v0.2 evolution (instrumentation strategy, cardinality posture, schema of
pool stats, RAII pattern, startup invariant). D0, D5, D7 are documented
inline in `architecture.md` because they're scope/hosting/NFR choices
that don't have consequentially independent trade-offs to record.

## D0 — Metric scope (5 shipped, 5 deferred)

**Decision**: ship the 5 dashboard-referenced metric families this slice;
defer the 5 documented-but-unconsumed metrics to v0.2 follow-ups.

**Shipped this slice**:

| Metric | Type | Labels | Dashboard panel |
|---|---|---|---|
| `http_requests_total` | counter | `path`, `method`, `status` | Panel 1, Panel 3 |
| `http_request_duration_seconds` | histogram | `path`, `method`, `status` | Panel 2 |
| `db_connections_in_use` | gauge | (none) | Panel 5a |
| `sse_subscribers_total` | gauge | `project_id` | Panel 4 |
| `foundry_app_startup_total` | counter | (none) | (already shipped by DEVOPS slice; unchanged) |

**Deferred to v0.2**:

| Deferred metric | Reason |
|---|---|
| `outbox_pending_jobs` | No dashboard consumer; slice-1 load profile (0.25 req/sec) doesn't backlog the outbox. |
| `bootstrap_tokens_unclaimed` | No dashboard consumer; tokens are short-lived (15 min TTL); operator psql query suffices. |
| `migration_apply_duration_seconds` | No dashboard consumer; one-shot startup concern already logged structurally by sqlx. |
| `realtime_listen_disconnects_total` | No dashboard consumer; PgListener reconnect is automatic per slice-2; alerting not yet defined. |
| `probe_failures_total` | No dashboard consumer; probe failures already crash the container (slice-5 precedent) — a counter for "things that already crashed" is redundant. |
| `db_connection_wait_seconds` | Dashboard panel EXISTS (Panel 5b) but no clean sqlx 0.8 hook; deferred per D3 with explicit revisit condition. Panel stays half-empty matching DEVOPS "instrument me" posture. |

Rationale: matches the DEVOPS slice's explicit "instrument me" signal for
the 5 dashboard panels; the 5 unreferenced metrics have no consumer that
would notice their absence; aligns with slice-1 "smallest thing that
satisfies the AC" discipline. Documented inline in `architecture.md`.

## D1 — Recording strategy: tower middleware on router (CHOSEN: A)

| | |
|---|---|
| **Question** | Where do `http_requests_total` and `http_request_duration_seconds` get emitted? |
| **Chosen** | A — Single `tower::Layer` wired via `.layer()` in `build_router`. Extracts `MatchedPath` + method + status; emits counter + histogram. |
| **Rationale** | Zero handler-signature changes; every current AND future route auto-instrumented; `MatchedPath` is the natural cardinality bound (route template, not concrete URI); single point of authority for request-metric emission in `metrics_server.rs`. |
| **Alternative B rejected** | Per-handler explicit `metrics::counter!` calls — every new handler must remember; doubles slice scope by touching every existing comments/issues handler; "forgetfulness foot-gun" for next contributor. |
| **Alternative C rejected** | Proc-macro attribute (`#[instrument]`) — adds proc-macro build burden; macro expansion opaque to grep; doesn't solve the "remember to apply" problem (just relocates it). |
| **Captured in** | ADR-010 |

## D2 — Label cardinality: bounded triple (CHOSEN: A)

| | |
|---|---|
| **Question** | What label set keeps the time-series count bounded? |
| **Chosen** | A — `{path, method, status}` where `path` = `MatchedPath` route template, `method` = HTTP verb, `status` = full 3-digit HTTP status. |
| **Rationale** | Matches dashboard queries verbatim (`sum by (status)`, `sum by (le, path)`); ~800 counter series + ~8000 histogram series at 27 routes is manageable; full 3-digit status preserves the 400-vs-404-vs-410 distinction slice-5 went out of its way to establish (ADR-008's 410 Gone). |
| **Alternative B rejected** | `status_class` (`2xx`/`3xx`/`4xx`/`5xx`) — smaller footprint but loses 404-vs-410 distinction; dashboard's "request rate by status" panel becomes coarser. |
| **Alternative C rejected** | Drop `method` — significant loss; many slice-1 routes have both GET and POST variants; can't answer "is POST slower than GET on this URL?". |
| **Forbidden labels (enforced)** | `user_id`, `workspace_id`, `team_id`, `project_id` (allowed only on `sse_subscribers_total`), `issue_id`, `comment_id`, `session_id`, `request_id`, IP address, User-Agent. Each unbounded; each a foot-gun for the next contributor. |
| **Captured in** | ADR-011 |

## D3 — DB pool gauge: 5s poll; defer wait histogram (CHOSEN: C)

| | |
|---|---|
| **Question** | How does `db_connections_in_use` stay current? What about `db_connection_wait_seconds`? |
| **Chosen** | C — Background tokio task in `foundry-app::main` reads `pool.size()` + `pool.num_idle()` every 5 seconds, updates `db_connections_in_use` gauge. Defer `db_connection_wait_seconds` histogram to v0.2; Panel 5b stays half-empty. |
| **Rationale** | Zero hot-path overhead; works with stock sqlx 0.8 (no version pin, no feature flag); 5s poll interval << 15s Prometheus scrape so gauge is at most one scrape behind reality; deferring the wait histogram avoids churning ~30 Store query sites for telemetry that has no consumer at slice-1 load profile (0.25 req/sec doesn't exhaust a 10-conn pool). |
| **Alternative A rejected** | Poll-based for both — wait histogram becomes empty or zero-filled (no observable without wrapper); honest signal but worse than acknowledged deferral. |
| **Alternative B rejected** | Event-based via `TimedPool` wrapper — requires wrapping the pool surface; ~30 implicit-acquire call sites in `Store` would need explicit `acquire_timed`; significant slice expansion for telemetry no current operator would consume. |
| **Revisit condition** | If `db_connection_wait_seconds` panel becomes operationally needed before sqlx exposes acquire hooks, evaluate `TimedPool` wrapper (alternative B). Until then, panel stays half-empty matching DEVOPS "instrument me" precedent. |
| **Captured in** | ADR-012 |

## D4 — SSE subscriber gauge: RAII guard (CHOSEN: A)

| | |
|---|---|
| **Question** | Where does the `sse_subscribers_total` increment/decrement happen? |
| **Chosen** | A — New `pub struct SubscriberGauge { project_id: Uuid }` in `foundry-realtime` with `new(project_id)` (inc) + `Drop::drop` (dec). SSE handler in `foundry-app::events` constructs one per subscription: `let _gauge = SubscriberGauge::new(project_id);`. |
| **Rationale** | Drop is the canonical Rust idiom for lifetime-bound counters; subscriber lifetime IS the binding lifetime (BroadcastStream wraps the Receiver, which follows the same model); single-line handler change; cleanest separation (gauge lives in the realtime crate where the subscriber concept lives); Drop fires on stream termination, client disconnect, and panic unwind uniformly. |
| **Alternative B rejected** | Explicit inc/dec in the SSE handler — easy to miss the decrement when stream is cancelled by client disconnect mid-poll; hand-rolled SSE streaming code makes the drop point non-obvious; foot-gun. |
| **Alternative C rejected** | Tower middleware on `/events` route — SSE handler holds a long-lived `Stream`; middleware sees the request as "complete" when handler returns but the stream is still being polled; decrement fires at the wrong time. Rejected on correctness. |
| **Label decision** | Keep `project_id` per inherited spec; bounded cardinality (number of projects ever subscribed-to); enables "which project has the most viewers right now?" diagnostic query. |
| **Captured in** | ADR-013 |

## D5 — Code hosting: hybrid; no new crate (CHOSEN: C)

| | |
|---|---|
| **Question** | Where does new instrumentation code live? Honor ADR-001 "no new crates" or introduce `foundry-metrics`? |
| **Chosen** | C — Hybrid. Recorder install + request-tracking middleware factory + startup probe all live in existing `crates/foundry-app/src/metrics_server.rs`. `Store::pool_stats()` is a new method on the existing `Store` (no new file). `SubscriberGauge` is a new ~30-line module in `crates/foundry-realtime/src/lib.rs`. |
| **Rationale** | Honors ADR-001's "no new crates without explicit need" — the case for `foundry-metrics` is "tidy" not "necessary"; the recorder install already lives in `metrics_server.rs` so extending that file to also export the middleware factory is natural cohesion; `SubscriberGauge` in `foundry-realtime` is the cohesive home for "subscriber-lifetime aware" type since the subscriber concept lives there; zero workspace.toml change; `cargo deny check` + `xtask check-arch` unaffected. |
| **Alternative A rejected** | Inline `metrics.rs` files per crate — three new files; some duplication; no real benefit over hybrid. |
| **Alternative B rejected** | New `foundry-metrics` workspace crate — breaks ADR-001 precedent; adds workspace member + Cargo.toml + dep edges; increases compile time; premature abstraction (no second consumer of the facade types). Becomes the right answer when a SECOND binary needs to share the recorder. |
| **Captured in** | `architecture.md` (inline) — precedent inheritance from ADR-001; no standalone ADR needed. |

## D6 — Startup probe: self-scrape `/metrics` (CHOSEN: A)

| | |
|---|---|
| **Question** | Should startup verify the recorder is installed + `/metrics` endpoint is reachable + recorder actually accepts emissions? |
| **Chosen** | A — Extend the existing startup probe sequence to self-scrape `http://127.0.0.1:{METRICS_PORT}/metrics` after sidecar listener binds. Assert HTTP 200, non-empty body, contains `foundry_app_startup_total` line. On failure: structured `health.startup.refused` log + non-zero exit; container restarts. |
| **Rationale** | Catches the entire class of deploy-time misconfig — wrong `METRICS_PORT`, port-in-use, recorder install silently swallowed, firewall between app and own port. Mirrors slice-5 precedent (`Store::probe()`); known pattern; low novelty cost. Self-scrape latency negligible (<10ms on localhost). Fires before main HTTP listener binds, so failure mode is "container restarts" not "container serves traffic while metrics are broken". |
| **Alternative B rejected** | Defer probe; rely on `/healthz` + operator observation — "operators would notice empty series" is the failure mode this slice exists to avoid (the DEVOPS slice's empty-series state lasted from initial commit to slice-6 start). Without app-side probe, port-conflict misconfig is silent until someone opens Grafana. |
| **Inheritance** | Slice-5 evolution-doc lesson #6 — "probe the substrate lie" pattern. Principle 12 application: the metrics sidecar listener is a driven adapter; the probe verifies the contract "I can render at least one metric" empirically. |
| **Captured in** | ADR-014 |

## D7 — Performance budget: ≤10µs P95 per request (CHOSEN: 10µs)

| | |
|---|---|
| **Question** | What per-request overhead does this slice promise? |
| **Chosen** | ≤10µs P95 added per request. |
| **Rationale** | 5× safety margin on the expected 2µs per request (`MatchedPath` ~100ns + 2x `Instant::now()` ~100ns + counter ~200-500ns + histogram ~200-500ns). Covers cache misses, atomic contention spikes, bucket-find pathological cases. 0.005% of NFR-PERF-01 200ms budget; 0.25% of slice-1 measured 4ms P95. |
| **Measurement** | Microbench (criterion or `wrk` + `hyperfine` shell script) toggles layer on/off against no-op handler; CI gate asserts P95 added overhead < 10µs across 27 routes. Called out to platform-architect for CI inclusion. |
| **Background tasks** | Pool poll (~100ns every 5s = effectively free); SSE gauge inc/dec (~50ns per long-lived stream, not per request); both negligible. |
| **Captured in** | `architecture.md` (inline) — NFR consequence, not a consequentially independent trade-off; no standalone ADR. |

## Reuse Analysis — HARD GATE artifact

This table is the slice's hard gate. Every CREATE NEW is challenged;
every EXTEND is justified by reuse over reimplementation per principle 5.
Verbatim from `proposals.md` §1, finalized for the chosen options.

| Action | Target | Why | LOC delta |
|---|---|---|---|
| EXTEND | `crates/foundry-app/src/metrics_server.rs` | Add `request_tracking_layer()` (tower middleware factory) so the metrics module owns BOTH the recorder install AND the request-tracking middleware. Also add `probe(handle, addr)` for startup self-scrape (D6). | +~85 |
| EXTEND | `crates/foundry-app/src/lib.rs` (`build_router`) | Add `.layer(metrics_server::request_tracking_layer())` near the existing CSRF/session layers. One-line change. | +1 |
| EXTEND | `crates/foundry-app/src/main.rs` | Add background task that periodically samples `Store::pool_stats()` and updates `db_connections_in_use` gauge. Call `metrics_server::probe()` after sidecar `serve()` returns. | +~50 |
| EXTEND | `crates/foundry-store/src/lib.rs` | Add `Store::pool_stats() -> PoolStats { in_use, idle, size }` — read-only snapshot of `Pool::size()` + `Pool::num_idle()`. | +~20 |
| EXTEND | `crates/foundry-realtime/src/lib.rs` | Add `SubscriberGauge` guard type that increments on construction and decrements on Drop. | +~30 |
| EXTEND | `crates/foundry-app/src/events.rs` | Insert one line at SSE-subscription time: `let _gauge = foundry_realtime::SubscriberGauge::new(project_id);` — RAII handle decrements on drop when the SSE stream terminates. | +~3 |
| EXTEND | `crates/foundry-store/Cargo.toml`, `crates/foundry-realtime/Cargo.toml` | Add `metrics = { workspace = true }` (only `foundry-app` consumes it today). No workspace-level change. | +2 |
| CREATE NEW | none | Per D5 hybrid hosting. Honors ADR-001 "no new crates" precedent. | — |

**Total estimated delta**: ~190 LOC of Rust + 2 manifest lines. Smaller than
slice 5 (~340 LOC) because no SQL migration, no new HTTP verbs, no new SSE
event_types.

## Architecture Summary

- **Pattern**: Layered with strict inward dependency, dependency-inversion
  at the crate boundary (inherited from slice-1 ADR-001).
- **Paradigm**: OOP-flavored Rust with plain async fns. RAII for
  lifetime-bound resources (the `SubscriberGauge` Drop pattern is the
  canonical Rust idiom). No traits introduced (no second implementer
  appears).
- **Key components touched**:
  - `foundry-app::metrics_server` — extended with request-tracking
    middleware factory + startup probe.
  - `foundry-app::main` — gains background pool-polling task and probe
    invocation.
  - `foundry-app::events` — one-line addition constructing
    `SubscriberGauge`.
  - `foundry-app::lib (build_router)` — one-line `.layer(...)` addition.
  - `foundry-store` — gains `Store::pool_stats()` method.
  - `foundry-realtime` — gains `SubscriberGauge` RAII type.
  - `foundry-core` — unchanged (I/O-free invariant preserved; `metrics`
    facade not added to the domain crate).
  - `foundry-auth` — unchanged.
- **Communication**: All `metrics::*` macro emissions converge into the
  single `PrometheusHandle` installed at startup. Prometheus scrapes the
  sidecar listener every 15s. No new wire protocols. No new external
  integrations.

## Technology Stack

**Zero new dependencies.** Slice 6 is a pure extension of existing
adapters:

- Rust 2021 / axum 0.8 / tower / sqlx — unchanged. `MatchedPath` is an
  axum 0.8 first-class extractor (already in the dependency closure).
- `metrics` (MIT/Apache-2.0) — already declared at workspace level per
  commit c7cb715. Added to per-crate manifests for `foundry-store` and
  `foundry-realtime` (workspace declaration already exists).
- `metrics-exporter-prometheus` (MIT/Apache-2.0) — already declared and
  consumed by `foundry-app::metrics_server`.
- `tokio::time::interval` — core tokio capability; no new dep for the 5s
  polling task.

`cargo deny check` expected to pass without changes. AGPLv3-clean
dependency graph preserved.

## Constraints Established

These constraints are established by slice-6 decisions and become
invariants downstream waves and future slices must honor:

1. **Cardinality invariant**: `http_requests_total` and
   `http_request_duration_seconds` MUST emit exactly the label set
   `{path, method, status}` and no others. The forbidden-labels list
   (D2) is binding. Adding any high-cardinality label requires a new
   ADR. Enforcement: unit test in `metrics_server.rs` asserts the
   middleware's emitted label keys are exactly the permitted triple.

2. **MatchedPath-only invariant**: `path` label MUST be the
   `MatchedPath` route template, never a concrete URI. Unmatched
   requests (404 to unknown paths) emit with the literal label
   `path="<unmatched>"` to prevent series-per-URI cardinality explosion.

3. **Recorder lifecycle invariant**: the `metrics_exporter_prometheus`
   recorder is process-global; re-init panics. Slice 6 introduces no
   second install path. Acceptance harness continues to skip
   `install_recorder` (per the existing comment in `metrics_server.rs`).
   Any future code that wants to install a recorder is a bug.

4. **`foundry-core` I/O-free invariant**: the domain crate MUST NOT gain
   a `metrics` dependency. Domain logic emits no metrics; only the
   adapter layer does. Enforced by `cargo xtask check-arch` (existing).

5. **Startup probe invariant**: process refuses to serve traffic if the
   `/metrics` self-scrape returns non-200, empty body, or omits the
   `foundry_app_startup_total` line. No degraded "serve traffic with
   broken metrics" mode. Container restart loop surfaces the misconfig.

6. **New metric families pattern**: new metric families added in future
   slices follow the patterns established here — middleware for
   per-request, RAII guard for lifetime-bound, background poll for
   periodic samples. No new emission strategy without ADR.

7. **Per-request overhead invariant**: any new layer wired into
   `build_router` MUST stay within the cumulative ≤10µs P95 budget for
   instrumentation overhead. New layers that exceed this require an NFR
   waiver or budget renegotiation.

## Open Questions for DISTILL

These are intentionally small and bounded; DISTILL resolves them with
the acceptance-designer:

1. **Exact `@nfr-*` tag set for slice-6 scenarios**: the slice-1 NFR
   catalogue defines `@nfr-obs-*` (observability), `@nfr-perf-*`
   (performance). The slice-6 "5 metrics are emitted correctly" family
   likely rides `@nfr-obs-03` (or a new tag); the "middleware adds
   ≤10µs P95" check rides `@nfr-perf-04` (or new). DISTILL decides
   the exact tag string by reviewing the inherited NFR table.

2. **Cardinality enforcement test harness**: is the
   "middleware emits only `{path, method, status}`" check a unit test
   in `metrics_server.rs` (recommendation) or its own xtask check
   (`cargo xtask check-cardinality`)? Unit test is the smaller delta;
   xtask is the more visible CI gate. DISTILL + software-crafter
   decide during RED.

3. **Probe verifying the middleware actually fired**: should the
   acceptance suite assert (a) counter == N after N requests
   (behavioral), (b) the middleware appears in the tower layer stack
   (structural), or both? Recommendation: behavioral only — the
   counter assertion is the substrate-lie probe (per principle 12); a
   structural check verifies wiring but not effect. DISTILL confirms.

4. **`db_connections_in_use` gauge initial value**: should the gauge
   register at process start with value 0 (so Prometheus scrape sees
   the line immediately), or only emit after the first poll-task tick?
   Recommendation: register at 0 at startup; first tick (within 5s)
   overwrites. Avoids a 5s window of "metric absent" in Grafana.
   DISTILL confirms during scenario design.

5. **Poll-task lifecycle on graceful shutdown**: does the polling task
   abort on graceful shutdown signal, or run-to-completion of the
   current tick? Recommendation: abort (it's a `tokio::time::interval`
   spawn; the runtime drops it on shutdown). DISTILL confirms via the
   shutdown acceptance scenario.

## Decision-driven invented detail (FLAGGED for user override)

The following specifics were chosen during proposals to make the design
concrete, and are re-stated here so the orchestrator + acceptance-designer
have a single point of reference. All are under the user's authority to
override during DISTILL or RED.

1. **Pool-polling interval = 5 seconds.** Picked to be << the 15s
   Prometheus scrape interval (gauge is at most 1 scrape behind reality).
   Tune via env var `METRICS_POOL_POLL_SECONDS` (default 5) if desired.

2. **Histogram bucket boundaries for `http_request_duration_seconds`** —
   `metrics-exporter-prometheus` default: `[0.005, 0.01, 0.025, 0.05,
   0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]`. Reasonable for web traffic.
   Override via `PrometheusBuilder::set_buckets_for_metric` if the
   slice-1 4ms P95 measurement suggests a finer low-end (e.g., add
   `[0.0005, 0.001, 0.002]` to better capture sub-5ms variance).

3. **`MatchedPath` 404 fallback label** — if no route matches, the
   middleware emits `path="<unmatched>"`. Bounded cardinality safety
   for 404s to random URIs.

4. **Startup probe URL** — `http://127.0.0.1:{METRICS_PORT}/metrics`.
   Hits loopback regardless of `METRICS_HOST` so the probe works in
   containers that bind metrics to `0.0.0.0` but where only loopback is
   reachable from the same process.

5. **Probe failure exit shape** — `anyhow::bail!` from `main`, becoming
   a non-zero process exit. Matches the existing `Store::connect`
   failure shape. Container orchestrator restarts the pod; the restart
   loop surfaces the misconfig.

6. **`SubscriberGauge::new(project_id: Uuid)` signature** — single
   constructor arg; `Drop` does the decrement. No fire-and-forget
   subscribe/unsubscribe counter (would require
   `sse_subscriptions_total{event="subscribed"}` + `event="dropped"`;
   not in the dashboard, deferred).

7. **Deferred `db_connection_wait_seconds` panel** — Panel 5b stays
   half-empty until v0.2 ships either (a) the `TimedPool` wrapper if
   operational forcing function appears, or (b) the sqlx upstream
   acquire-hook feature when it lands. Matches DEVOPS slice's empty
   panels for unconsumed metrics — same posture applied recursively.
