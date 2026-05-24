# ADR-104: Observability minimal-by-default

## Status

Accepted (2026-05-23).

## Context

Observability stacks are a tar pit for "boring monolith" deployments. The naive choice is to ship a full Prometheus + Loki + Grafana + Tempo bundle so operators get a dashboard out of the box. The cost: doubles the container count, adds 1-2 GB of memory pressure, introduces 4 new services to maintain, and turns "under an hour install" into "under three hours install."

The MVP's tension: operators want monitoring (NFR-OBS-01..04 are required), but they don't want a bundled observability stack getting between them and Foundry running.

## Decision

The default Foundry deploy ships **observability primitives but no observability stack**. Specifically:

- The binary exposes JSON logs to stdout (NFR-OBS-01).
- The binary exposes Prometheus metrics on a separate `METRICS_PORT` (default 9090; NFR-OBS-03).
- The binary exposes `/healthz` and `/readyz` on the main port (NFR-OBS-02).
- The binary emits OpenTelemetry traces via OTLP if and only if `OTEL_EXPORTER_OTLP_ENDPOINT` is set (off by default).
- An **optional** `docker-compose.observability.yml` overlay starts Prometheus + Loki + Promtail + Grafana with a pre-provisioned "Foundry Overview" dashboard. The overlay is layered explicitly: `docker compose -f docker-compose.yml -f docker-compose.observability.yml up -d`.

Operators with existing monitoring infrastructure (Datadog, CloudWatch, Splunk, internal Grafana) scrape Foundry directly. Operators with nothing get the overlay. Operators evaluating Foundry get neither and use `docker compose logs` like any other Docker workload.

## Alternatives considered

### A — Bundle full observability stack by default

- **Pros**: first-time operator gets a dashboard immediately; no second-step required.
- **Cons** (decisive):
  - Adds 4 containers (Prometheus, Loki, Promtail, Grafana) plus volumes — 1.5-2 GB RAM minimum.
  - Doubles the initial-install time; violates US-01.
  - For operators with existing Datadog/CloudWatch, the bundled stack is wasted resources they'll immediately disable.
  - Maintenance burden: every release we test 5 container images, not 2.

### B — Ship nothing; document how to set up observability yourself

- **Pros**: simplest from a maintenance perspective; treats observability as the operator's concern.
- **Cons**:
  - Operators evaluating Foundry have nothing to point at when asking "how do I know it's healthy?" — failure of the trust-and-evaluate moment.
  - First-time operator who wants a dashboard now has 30 minutes of YAML to write.

### C — Bundle by default but allow opt-out

- **Pros**: best of both worlds.
- **Cons**: still adds the install-time overhead unless the operator knew in advance to opt out — defeats the purpose.

### D — Observability cloud service (free tier of Grafana Cloud, etc.)

- Rejected: violates the data-sovereignty ethos of the project (JTBD outcome #2). Foundry is self-host; sending metrics to a vendor cloud contradicts that.

## Consequences

### Positive

- Install time is unaffected: 2-container default vs. 6-container with-overlay. Operator chooses.
- The contract is clean: Foundry produces metrics / logs / traces; the operator chooses what consumes them.
- The overlay is a working reference — operators can copy and customize without starting from scratch.
- Sub-port for metrics (NFR-OBS-03) keeps the scrape endpoint off the public LB.

### Negative (explicit trade-offs)

- Operator who wants a default dashboard must run a second command. We mitigate with strong README copy: "If you want a Grafana dashboard, add `-f docker-compose.observability.yml` to your `docker compose` command."
- The overlay ships an opinionated Grafana dashboard; operators who want a different shape will edit it. We don't try to ship "the perfect dashboard" — just a credible starter.
- Loki + Promtail + Prometheus all have ongoing maintenance tax; we pin versions in the overlay and test it in CI on each release.

## Probe contract recursion (Principle 9)

The observability stack itself can lie:

- A scrape job pointing at the wrong port silently returns no metrics → looks like Foundry is generating no traffic.
- A Loki misconfig drops logs silently.
- A Prometheus retention misconfig means alerts fire on empty data.

The MVP doesn't try to monitor the monitor. The starter alert set in `failure-modes.md` includes a `probe_failures_total` watcher that signals "Foundry's own self-tests are failing" — that's the closest we get to meta-monitoring. Operators wanting tighter observability hygiene set up their own watchdog (e.g., a Prometheus alert on `up{job="foundry"} == 0` that pages them).

## Review trigger

Revisit if:

1. Operator feedback consistently asks for a bundled default — suggests the overlay-friction is too high.
2. A major Foundry feature (e.g., audit log) requires structured log shipping such that the overlay becomes effectively mandatory.
3. The Prometheus / Loki / Grafana stack has a major breaking change that significantly raises the overlay-maintenance cost — we might switch to VictoriaMetrics or similar.
