# ADR-010: HTTP Request Metrics via Tower Middleware

## Status
Accepted — 2026-05-25

## Context

Slice 6 (`handler-instrumentation`) emits the metric series the shipped
Grafana "Foundry Overview" dashboard references. Two of the five
dashboard-referenced metric families — `http_requests_total` (counter) and
`http_request_duration_seconds` (histogram) — must be emitted once per HTTP
request with bounded labels.

The implementation needs an explicit decision on WHERE the counter
increment and histogram observation happen in the request lifecycle. The
choice determines:

- How much per-handler boilerplate is required (every new contributor adds
  a route; do they have to remember to instrument it?).
- Whether `MatchedPath` route templates can be used uniformly (vs concrete
  URIs leaking as labels — the cardinality killer).
- How the implementation composes with the existing tower stack (CSRF
  middleware, session middleware, request-id middleware — all already
  layered in `build_router`).
- Whether existing comments/issues handlers need editing (slice scope
  inflation) or can ride the new layer transparently.

Quality attributes driving this decision: **maintainability (HIGH)** — the
slice-1 taste filter requires that the workspace stay groakkable in a day;
new instrumentation must not bloat handler bodies; **cardinality safety
(HIGH)** — Prometheus's killer is unbounded labels; the chosen pattern
must make `MatchedPath` (route template) the natural label, not a manual
opt-in; **observability completeness (HIGH)** — the dashboard panels are
the AC; every routed request must emit, no exceptions.

## Decision

**A tower middleware layer wired into the router via `.layer()` in
`build_router`.** A single `tower::Layer` (the `request_tracking_layer`)
extracts `MatchedPath` (axum 0.8 native via `axum::extract::MatchedPath`),
method, and the observed response status, then emits exactly one
`metrics::counter!("http_requests_total", "path" => matched, "method" => m,
"status" => s).increment(1)` and one
`metrics::histogram!("http_request_duration_seconds", "path" => matched,
"method" => m, "status" => s).record(d)` per request.

The middleware factory `request_tracking_layer()` lives in
`crates/foundry-app/src/metrics_server.rs` — the same file that owns the
recorder install and the sidecar listener. Wiring is a single line in
`build_router`:

```rust
.layer(metrics_server::request_tracking_layer())
```

Composition: the layer sits in the tower stack next to the existing
CSRF, session, and request-id layers. No handler signatures change. No
per-handler instrumentation calls.

## Alternatives Considered

### A: Tower middleware layer (chosen)
See Decision.

### B: Per-handler explicit `metrics::counter!` calls
Every handler in `comments.rs`, `issues.rs`, etc. calls
`metrics::counter!("http_requests_total", ...)` at the top and
`metrics::histogram!("http_request_duration_seconds", ...)` after rendering.

- **Pros**: Maximum precision — the handler decides exactly what to emit
  and when. Sub-handler labels (e.g., `is_htmx=true`) are trivial. The
  emission site is grep-discoverable.
- **Cons**: Every new handler must remember the call — a forgetfulness
  foot-gun for the next contributor. Inflates handler bodies (the slice-1
  taste filter explicitly resists this). Hard to enforce via static
  analysis (would need a custom clippy lint or ArchUnit-style import-graph
  check). Touches every comments/issues handler we already shipped,
  doubling slice scope. The "remember to instrument" property degrades
  monotonically as more contributors join.
- **Rejected because**: principle 5 (existing-code reuse over
  reimplementation) plus the maintainability quality attribute — the
  layer-once approach instruments every current AND every future handler
  without a foot-gun.

### C: Proc-macro attribute (`#[instrument]`)
A custom or third-party (`tracing`-style) attribute macro wraps each
handler.

- **Pros**: Clean call-site. Composes with `tracing::instrument` if we
  ever want to unify span + metrics emission.
- **Cons**: Adds a proc-macro dependency or in-house build burden. Macro
  expansion is opaque to grep (the slice-1 taste filter resists clever
  macros explicitly). Diagnostics get worse on the handler signature
  (errors point inside the expansion). Crucially: doesn't actually solve
  the "remember to apply it" problem — just relocates the foot-gun from
  the call site to the attribute placement.
- **Rejected because**: solves a problem we don't have (call-site
  cleanliness) while inheriting the problem we DO have (must remember to
  apply per handler). Plus the build complexity is a maintainability
  regression.

## Consequences

### Positive
- Zero handler-signature changes. Slice-1 ADR-001's "handlers stay thin"
  property preserved.
- Every current AND future route auto-instrumented. New contributors
  cannot forget to instrument — there is no opt-in.
- `MatchedPath` collapses concrete URIs like
  `/team/acme/project/foo/issues/42/comments` to the single template
  label `/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments`.
  Cardinality is bounded by route count (~27), not by concrete-URI count
  (unbounded).
- Single point of authority. The `metrics_server.rs` file owns the
  recorder install AND the request-tracking middleware AND the startup
  probe (ADR-014). One file to audit for "how does request metric
  emission work in this codebase?"
- Composes naturally with the existing tower stack (CSRF, session,
  request-id layers). No interaction surprises.

### Negative
- Loses sub-handler granularity. The histogram observes "whole-handler
  duration", not "DB query duration alone". For sub-handler timing, code
  must add `metrics::histogram!("db_query_duration_seconds", ...)` at the
  relevant Store method (acceptable per slice-1 thin-handler ADR-001 —
  handlers don't do enough non-DB work to warrant sub-handler timing).
- Status code from a panicking handler defaults to 500 only if the panic
  handler converts the panic to a response. The acceptance suite should
  include a panic scenario asserting the 500 is observed by the metrics
  layer (covered by the principle-12 probe noted below).
- Removing the layer (e.g., for a benchmark with instrumentation off)
  requires a feature flag or env-var conditional in `build_router` —
  acceptable trade-off for the always-on default.

### Neutral
- Reversibility: switching to per-handler explicit calls (option B) is a
  per-handler decision, not an architectural one. The middleware can
  coexist with per-handler calls — if some specific handler needs
  sub-handler labels, it can call `metrics::counter!` in addition to the
  layer's emissions. This is a path forward without a re-architecture.
- The layer's emitted-label-key set should be enforced by a unit test
  (called out in ADR-011) — the layer-once approach makes this single
  point easy to audit, while the per-handler approach would require N
  audits.

## Verification

- A scenario hitting N distinct endpoints (mix of GET/POST, mix of
  status codes) followed by `GET /metrics` asserts
  `http_requests_total` sums to N across all emitted series. This is
  the principle-12 probe ("substrate lie that the middleware actually
  fired").
- A unit test inspecting the emitted-label-key set asserts EXACTLY
  `{path, method, status}` and no others. Cardinality regression
  prevention; the probe for ADR-011's invariant.
- A scenario that hits a handler which panics asserts the 500 status is
  observed in the counter (i.e., the panic handler's response status
  flows through the layer's status observation).
- Microbench (per ADR D7 documentation in `architecture.md`) asserts
  ≤10µs P95 added per request — the layer's performance budget.
