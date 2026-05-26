# ADR-011: Bounded Label Cardinality `{path, method, status}` for HTTP Metrics

## Status
Accepted — 2026-05-25

## Context

Slice 6 emits `http_requests_total` (counter) and
`http_request_duration_seconds` (histogram) via a tower middleware layer
(ADR-010). These metrics MUST have an explicit label-set policy because
Prometheus's primary failure mode is unbounded label cardinality: every
distinct combination of label values produces a new time series, and a
sufficiently-large series count can DoS the TSDB (write amplification,
memory pressure, query slowness).

The slice's risk profile:

- A label like `user_id` would mint a new time series per registered user
  (~10K series at scale).
- A label like `request_id` (UUID per request) would be effectively
  infinite — series-per-request defeats the whole point of a counter.
- A label containing a concrete URI segment (e.g., the literal
  `/issues/42/comments` rather than the template
  `/issues/{issue_number}/comments`) would explode with the number of
  issues ever observed.

The dashboard query shapes confirm the minimum label requirements:
`sum by (status) (rate(http_requests_total[1m]))` requires `status` as a
label; `sum by (le, path) (rate(http_request_duration_seconds_bucket[5m]))`
requires `path`. Method (GET vs POST) is needed to distinguish read-vs-write
performance on routes that have both verbs.

The "what set of labels" question is structurally a security/operational
boundary as much as a metrics-modeling question — adding the wrong label
later is a cardinality regression that may not surface until the TSDB
falls over.

Quality attributes driving this decision: **cardinality safety
(HIGH)** — runaway labels are an operational outage; **diagnostic
granularity (HIGH)** — operators need to distinguish 200 vs 404 vs 410 on
a given route; **maintainability (HIGH)** — the policy must be encoded
such that a future contributor cannot accidentally add a high-cardinality
label without architectural visibility.

## Decision

**Three labels exactly: `path`, `method`, `status`. No others.**

- `path` = the `MatchedPath` route template extracted by axum 0.8
  (e.g., `/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments`).
  NEVER a concrete URI. Requests that don't match any route emit
  `path="<unmatched>"` to bound the 404-against-unknown-paths cardinality.
- `method` = the HTTP verb string (`GET`, `POST`, `PATCH`, `DELETE`, etc.).
  Bounded by the HTTP spec.
- `status` = the full 3-digit HTTP status code (`200`, `303`, `404`, `410`,
  `500`, etc.). Full 3-digit preserves the 400-vs-404-vs-410 distinction
  that slice-5 ADR-008 went out of its way to establish.

**Forbidden labels** (binding; the middleware MUST NOT emit these and
adding any of them requires a new ADR):

- `user_id` (unbounded — one series per registered user)
- `workspace_id` (unbounded long-term)
- `team_id` (unbounded)
- `project_id` (unbounded — allowed only on `sse_subscribers_total`
  where the active-subscription set is naturally small)
- `issue_id` (unbounded)
- `comment_id` (unbounded)
- `session_id` (unbounded)
- `request_id` (effectively infinite — UUID per request)
- IP address (~4B IPv4 possible series, larger for IPv6)
- User-Agent string (effectively infinite)

Series-count estimate at slice-1 scale: 27 routes × ~3 methods/route × ~10
status codes = ~800 counter series; ×~10 histogram buckets per duration
entry = ~8000 histogram series. Comfortably manageable for any Prometheus
that's already scraping Foundry.

**Enforcement**: a unit test in `metrics_server.rs` exercises the
middleware against a representative request and asserts the emitted label
key set is EXACTLY `{path, method, status}` and no other keys appear.

## Alternatives Considered

### A: `{path, method, status}` 3-digit (chosen)
See Decision.

### B: `{path, method, status_class}` (2xx/3xx/4xx/5xx)
Same triple but `status` collapses to status class.

- **Pros**: Smaller TSDB footprint (~325 counter series, ~3200 histogram
  series — about 40% reduction). Still answers the dashboard's
  "request rate by class" panel.
- **Cons**: Loses the 400-vs-404-vs-410 distinction at the metrics layer
  (still visible in structured logs, but not graphable). The dashboard's
  "Error rate (5xx / total)" panel works either way. The
  "Request rate by status" panel would show only "2xx/3xx/4xx/5xx"
  labels, less diagnostic. Crucially, slice-5 ADR-008 deliberately
  established 410 Gone as a distinct semantic from 404 — the metrics
  layer should preserve that distinction so operators can answer
  "are users hitting deleted comments often?" via a Grafana query.
- **Rejected because**: the 40% series reduction does not justify the
  loss of operational diagnostic granularity at slice-1 scale; the
  series count under option A is already well within any healthy
  Prometheus's capacity.

### C: `{path, status}` (drop `method`)
Smallest viable label set.

- **Pros**: Smallest footprint (~270 counter series, ~2700 histogram
  series).
- **Cons**: Loses the GET-vs-POST distinction. Many slice-1 routes have
  both verbs (e.g., the comment form: GET to render the page, POST to
  submit). Cannot answer "is the POST handler slower than the GET
  handler on this URL?" or "are we getting POST 404s on a URL that
  only accepts GET?". Diagnostic value falls measurably below option A
  for trivial series-count gain.
- **Rejected because**: the diagnostic loss is real and the series-count
  saving is immaterial at slice-1 scale.

### D: Add `user_id` (or similar high-cardinality) label
Considered explicitly to document the rejection rather than because anyone
recommended it.

- **Cons**: Cardinality DoS. Series count grows with user count; at 10K
  users + 27 routes + 10 status codes ~= 2.7M counter series and ~27M
  histogram series. Single-instance Prometheus falls over. The
  "who is hitting this endpoint" question is a logging/tracing concern,
  not a metrics concern.
- **Rejected because**: cardinality is a safety property, not a feature.

## Consequences

### Positive
- Dashboard panels work verbatim — no template re-writes needed; queries
  match the emitted labels.
- Series count bounded at ~8000 — operationally trivial for any
  Prometheus.
- Full 3-digit status preserves slice-5 ADR-008's 410 distinction.
- `MatchedPath` plus the `<unmatched>` fallback prevents 404-to-random-URI
  series explosion.
- Forbidden-labels list is enumerated and binding — a future contributor
  who wants to add `user_id` must write a new ADR explaining the
  cardinality cost.

### Negative
- Operators cannot grep metrics for "this specific user's request
  pattern" — must use structured logs or distributed tracing for that
  query.
- The decision is conservative; future product needs (e.g., per-tenant
  SLO reporting) may force re-evaluation. Mitigation: any such addition
  requires an ADR documenting the cardinality cost + a mitigation plan
  (e.g., per-tenant metric exposed only above N requests/sec, or
  exported to a separate Prometheus).

### Neutral
- The `sse_subscribers_total` gauge keeps `project_id` (per ADR-013)
  because the active-subscription set is naturally small (a few dozen
  projects at MVP). This is an exception, documented in ADR-013, NOT a
  precedent for `http_*` metrics.
- The unit-test enforcement makes cardinality regression a build failure,
  not a production incident.

## Verification

- A unit test in `metrics_server.rs` calls the middleware with a
  representative request and asserts the emitted label key set is
  EXACTLY `{path, method, status}` — no other keys, no missing keys.
  The test fails closed on regression.
- An acceptance scenario hits `/totally/random/url/that/no/route/matches`
  and asserts the emitted series carries `path="<unmatched>"` rather
  than the concrete URI.
- An acceptance scenario hits the same route with GET and POST and
  asserts the emitted series are distinct (different `method` labels).
- An acceptance scenario hits a route in a way that returns 410 Gone
  (slice-5 deleted-comment path) and asserts the emitted `status` label
  is `"410"` exactly (not `"4xx"`).
