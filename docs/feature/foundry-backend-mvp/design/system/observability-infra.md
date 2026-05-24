# Observability Infrastructure

## Audience

Operators wiring Foundry into their monitoring stack. Companion `solution-architect` owns *what* gets logged and *what* metric names exist (tracing instrumentation in code); this document owns *where the bytes go* and the docker-compose extensions that make a Prometheus / Loki / Tempo stack one command away.

## Default posture — minimal by default

Per ADR-104, the default Foundry deploy ships with the *minimum* observability surface: structured JSON logs to stdout, Prometheus metrics on a separate port. No bundled Prometheus, no bundled Loki. Reasons:

- The under-an-hour install (US-01) cannot be slowed by a 5-container observability stack the operator didn't ask for.
- Operators already running a monitoring stack want to scrape Foundry, not deploy a parallel stack.
- Operators with no monitoring stack get `docker compose logs foundry` (stdout) and can graduate when they're ready.

For operators who want a Prometheus + Loki + Grafana stack alongside Foundry, we ship a *separate* compose overlay file (`docker-compose.observability.yml`) that they layer on top via `docker compose -f docker-compose.yml -f docker-compose.observability.yml up -d`. The overlay is documented but unobtrusive.

## What the Foundry binary exposes

| Surface | Where | Port | Format | Owned by |
|---------|-------|------|--------|----------|
| Structured logs | stdout | n/a | JSON Lines (NFR-OBS-01) | solution-architect (tracing setup) |
| Prometheus metrics | separate HTTP listener | `METRICS_PORT`, default 9090 | OpenMetrics text (NFR-OBS-03) | solution-architect (metrics emission) |
| OpenTelemetry traces | OTLP gRPC out | env-configurable `OTEL_EXPORTER_OTLP_ENDPOINT`; not set by default | OTLP | solution-architect (tracing) |
| Health endpoints | main HTTP port | `FOUNDRY_PORT` | text/plain (NFR-OBS-02) | both (semantics infra, impl app) |
| Request IDs | response header `X-Request-Id` | main HTTP port | UUIDv7 (NFR-OBS-04) | solution-architect |

The infrastructure contract is: **logs go to stdout, metrics to the sidecar port, traces to OTLP if and only if `OTEL_EXPORTER_OTLP_ENDPOINT` is set**. The app does not write to files, does not call back to any control plane, does not phone home.

## Why a separate `METRICS_PORT` (NFR-OBS-03)

The Prometheus `/metrics` endpoint is unauthenticated by design — Prometheus scrapers don't typically carry auth. Exposing it on the same port as the user-facing app would:

- Mean the LB needs an explicit rule to block `/metrics` from external traffic (easy to forget).
- Leak the metrics endpoint to any user who guesses the path.
- Pollute the app's HTTP histogram with scrape requests.

Putting `/metrics` on port 9090, bound to the container network only (not exposed via the LB) makes it firewall-by-default. Operators wanting metrics in a Prometheus running outside the container network add an explicit `ports: ["9090:9090"]` mapping.

## docker-compose.observability.yml overlay

The shipped overlay starts a minimal Prometheus + Loki + Grafana stack that scrapes Foundry and tails its logs. It's a single file the operator can opt in to.

```yaml
# docker-compose.observability.yml
# Layered on top of docker-compose.yml via:
#   docker compose -f docker-compose.yml -f docker-compose.observability.yml up -d

services:
  prometheus:
    image: prom/prometheus:v2.55.0
    volumes:
      - ./observability/prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - prometheus-data:/prometheus
    command:
      - "--config.file=/etc/prometheus/prometheus.yml"
      - "--storage.tsdb.retention.time=30d"
    networks: [foundry_net]
    # Not exposed publicly; reachable from grafana

  loki:
    image: grafana/loki:3.0.0
    volumes:
      - ./observability/loki-config.yml:/etc/loki/local-config.yaml:ro
      - loki-data:/loki
    networks: [foundry_net]

  promtail:
    image: grafana/promtail:3.0.0
    volumes:
      - /var/lib/docker/containers:/var/lib/docker/containers:ro
      - /var/run/docker.sock:/var/run/docker.sock
      - ./observability/promtail-config.yml:/etc/promtail/config.yml:ro
    networks: [foundry_net]
    depends_on: [loki]

  grafana:
    image: grafana/grafana:11.3.0
    ports: ["3001:3000"]  # 3001 to avoid clash with default Foundry on 3000
    environment:
      - GF_AUTH_ANONYMOUS_ENABLED=true
      - GF_AUTH_ANONYMOUS_ORG_ROLE=Viewer
    volumes:
      - grafana-data:/var/lib/grafana
      - ./observability/grafana-datasources.yml:/etc/grafana/provisioning/datasources/datasources.yml:ro
      - ./observability/grafana-dashboards:/etc/grafana/provisioning/dashboards
    networks: [foundry_net]

volumes:
  prometheus-data:
  loki-data:
  grafana-data:
```

Bundled `prometheus.yml` scrapes the foundry replicas:

```yaml
global:
  scrape_interval: 15s
scrape_configs:
  - job_name: foundry
    static_configs:
      - targets:
          - foundry-app-1:9090
          - foundry-app-2:9090
          - foundry-app-3:9090
```

(Static targets are fine for compose; the K8s translation uses `kubernetes_sd_configs` instead.)

Promtail tails Docker container logs by matching label `com.docker.compose.service=foundry-app` and ships them to Loki with the container's labels as Loki labels.

Grafana provisions one default dashboard ("Foundry Overview") that displays:

- p50 / p95 / p99 HTTP request latency by route.
- Request rate by status class (2xx / 4xx / 5xx).
- Postgres connection pool: in-use / idle / waiters.
- SSE subscribers, total.
- Outbox pending count.
- Recent ERROR-level log lines (last 100, from Loki).

That dashboard is the operator's "is Foundry happy?" view. We do NOT ship 20 dashboards; the minimum-viable one is the contract.

## Tempo / Jaeger (traces) — optional

If the operator already runs Tempo or Jaeger, they set `OTEL_EXPORTER_OTLP_ENDPOINT=http://tempo:4317` in `.env` and Foundry starts emitting OTLP traces. The overlay does NOT bundle Tempo because (a) trace storage adds non-trivial disk, (b) the operator is more likely to send traces to an existing tool. Documentation provides the env var and a sample config; bundling is post-MVP if there's demand.

## Log shipping for operators without the overlay

Operators running with a different log aggregator (Datadog, CloudWatch Logs, Vector, Fluent Bit, etc.) don't need anything Foundry-specific. The contract is:

1. Foundry writes JSON to stdout (NFR-OBS-01).
2. Docker captures stdout via its configured logging driver.
3. The operator's log driver (`json-file`, `awslogs`, `splunk`, `loki`, etc.) ships from there.

Recommended Docker logging-driver options for the foundry-app service:

```yaml
foundry-app:
  logging:
    driver: json-file
    options:
      max-size: "50m"
      max-file: "5"
      tag: "foundry"
```

The `max-size` + `max-file` rotation is critical — without it, the host disk fills with logs over weeks (a real failure mode operators have reported with other OSS tools).

## Metric naming convention (cross-reference)

The metric names live in solution-architect's territory but the *contract* — what counters and histograms exist — affects every operator dashboard, so they're enumerated in NFR-OBS-03. Summary:

| Metric | Type | Labels | Purpose |
|--------|------|--------|---------|
| `http_requests_total` | counter | `path`, `method`, `status` | Request rate by route + outcome |
| `http_request_duration_seconds` | histogram | `path`, `method` | Latency distribution |
| `db_connections_in_use` | gauge | (none) | Postgres pool saturation |
| `db_connection_wait_seconds` | histogram | (none) | Time waiting for a pool connection (catches pool exhaustion) |
| `sse_subscribers_total` | gauge | `project_id` | Active SSE clients per project |
| `outbox_pending_jobs` | gauge | (none) | Outbox depth (catches background-processing backlog) |
| `bootstrap_tokens_unclaimed` | gauge | (none) | Catches "operator hasn't claimed admin yet" |
| `migration_apply_duration_seconds` | histogram | `migration_id` | How long migrations take (feeds NFR-MIG-03 release-notes prediction) |
| `realtime_listen_disconnects_total` | counter | (none) | LISTEN connection drops; should be near-zero |
| `probe_failures_total` | counter | `probe_name` | Probe verifications that have failed (Principle 9 self-monitoring) |

These names are stable across Foundry minor versions; renaming requires an ADR.

## Probe contract (Principle 9)

The observability stack itself can lie:

1. **`probe.metrics.endpoint_reachable`** — on startup, the app binds `METRICS_PORT` and self-scrapes `GET /metrics`. Refuses to start if the bind failed or the response is empty. Detects port-conflict misconfiguration.

2. **`probe.logs.stdout_writable`** — implicit (the app writes a startup banner to stdout and exits if write fails). Detects `/dev/stdout` redirected to a full disk.

3. **`probe_self_check`** — every probe emits a `probe_failures_total{probe_name=...}` counter on failure. This counter exists so a dashboard can show "any probes failing right now?" — which is the recursive Principle 9 self-application: monitoring whether the substrate-honesty checks themselves are still being run after every Foundry upgrade. A future probe that silently stopped running would show as `probe_failures_total` being suspiciously flat over time, alertable.

## What's explicitly out of scope for MVP

- Real User Monitoring (RUM): post-MVP; the htmx + alpine.js frontend has no built-in RUM.
- Alerting rules shipped with Foundry: operators define their own; we provide a recommended starter set in docs but don't bundle into the overlay (too opinionated).
- Long-term metric storage: Prometheus default retention is 15-30 days; tighter SLAs need Thanos/Mimir/Cortex (post-MVP).
- Distributed tracing across instances: each Foundry instance is its own trace island.

## Cross-references

- NFR-OBS-01 through NFR-OBS-04 for the formal requirements.
- `failure-modes.md` for the alerting catalog these metrics feed into.
- `topology.md` for how `METRICS_PORT` differs across deploy variants.
- ADR-104 (observability minimal-by-default).
