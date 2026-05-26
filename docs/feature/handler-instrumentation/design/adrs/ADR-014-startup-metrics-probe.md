# ADR-014: Startup Metrics Probe — Self-Scrape `/metrics`, Refuse to Start on Fail

## Status
Accepted — 2026-05-25

## Context

The DEVOPS slice (commit c7cb715) wired the
`metrics_exporter_prometheus` recorder install and the sidecar axum
listener exposing `/metrics` on `METRICS_PORT` (default 9090). Slice 6
adds the actual metric-emitting code (per ADRs 010-013). Together, the
two slices form an adapter — the sidecar metrics listener — that
Prometheus consumes via PULL scrape.

Per principle 12 (Earned Trust), every driven adapter MUST have a probe
that empirically verifies the adapter can honor its contract in the real
environment where it will run. The metrics-listener adapter's contract:
"I expose a `/metrics` endpoint that returns at least one metric line in
Prometheus text-exposition format on HTTP 200."

The failure modes the probe must catch:

- **Wrong `METRICS_PORT` config**. Default is 9090; an operator
  setting an invalid value (parseable but conflict-prone, e.g., `80`)
  surfaces only when Prometheus fails to scrape — silent for hours.
- **Port-in-use**. Another process is bound to `METRICS_PORT`. The
  axum bind fails; current code logs and continues (the main listener
  still starts on `PORT`). Result: app serves traffic with broken
  metrics; operator only notices when looking at Grafana.
- **Recorder install silently swallowed**. The recorder is process-global
  and re-init panics; if some test-only init code path leaks into
  production (e.g., a refactor introduces a second `install_recorder`
  call), the second install panics in production but the first install's
  recorder may have been replaced silently in some failure modes. The
  `/metrics` endpoint returns empty body.
- **Firewall / network policy between app and own port**. In some
  container-network configurations (overlay networks with mTLS
  enforcement), the loopback path can be filtered. Result: external
  Prometheus scrape works; internal probe fails — catches a class of
  misconfig the external scrape can't even detect.

This is a textbook application of slice-5's "probe the substrate lie"
pattern — the slice-5 evolution doc's lesson #6 captures the principle.
The cost of NOT probing was demonstrated by the DEVOPS slice itself: the
dashboard panels stayed empty from initial DEVOPS commit until slice-6
start because no automated check would have caught it.

Quality attributes driving this decision: **observability completeness
(HIGH)** — silent metric failure is the failure mode the slice exists to
prevent; **operational robustness (HIGH)** — container restart loop is
preferable to "serve traffic while broken"; **inheritance discipline
(MEDIUM)** — slice-5's probe pattern should apply uniformly to all
adapters, not selectively.

## Decision

**Extend the startup probe sequence to self-scrape `/metrics`. Refuse to
start (non-zero process exit) on failure. Container orchestrator
restarts; restart loop surfaces the misconfig.**

Implementation lives in `crates/foundry-app/src/metrics_server.rs` as a
new function:

```rust
pub async fn probe(handle: &PrometheusHandle, addr: SocketAddr) -> Result<()> {
    let url = format!("http://127.0.0.1:{}/metrics", addr.port());
    let resp = reqwest::get(&url).await
        .with_context(|| format!("metrics probe: connect failed at {url}"))?;
    if resp.status() != 200 {
        anyhow::bail!("metrics probe: expected 200, got {}", resp.status());
    }
    let body = resp.text().await
        .context("metrics probe: read body failed")?;
    if body.is_empty() {
        anyhow::bail!("metrics probe: body empty");
    }
    if !body.contains("foundry_app_startup_total") {
        anyhow::bail!("metrics probe: foundry_app_startup_total line missing — recorder install swallowed?");
    }
    Ok(())
}
```

Called from `main.rs` after the sidecar `serve()` task is spawned and the
listener has had a moment to bind:

```rust
let metrics_addr = metrics_server::start(...).await?;
metrics_server::probe(&recorder_handle, metrics_addr).await?;
// only NOW spawn the main HTTP listener
```

On `Err(_)`: `main` returns the error via `anyhow::bail!` propagation,
process exits non-zero, container orchestrator restarts the pod.
Structured log line (`health.startup.refused`) captures the specific
probe failure for operator observability.

Probe URL is the loopback `127.0.0.1` regardless of `METRICS_HOST`
binding. This ensures the probe works in containers that bind the
sidecar listener to `0.0.0.0` but where only loopback is reachable from
the same process (a common k8s networking shape).

The probe asserts THREE things, each catching a distinct failure mode:

1. **HTTP 200 reachable**. Catches: wrong port, port-in-use,
   firewall/network-policy blocking loopback.
2. **Body non-empty**. Catches: bind succeeded but handler isn't wired
   to the recorder.
3. **`foundry_app_startup_total` line present**. Catches: handler wired
   but the recorder install was silently swallowed (the counter the
   DEVOPS slice emits at startup was lost).

The three assertions are semantically orthogonal — a single-point bypass
on any one is caught by the other two.

## Alternatives Considered

### A: Self-scrape probe; refuse to start on fail (chosen)
See Decision.

### B: Defer probe; rely on `/healthz` and operator observation
The metrics sidecar already exposes `/healthz` returning `"ok"`. Operators
monitoring the Grafana dashboard would notice empty series and
investigate.

- **Pros**: Smallest delta — zero new code.
- **Cons**: "Operators would notice" is the EXACT failure mode this slice
  exists to address. The DEVOPS slice demonstrated the cost: from initial
  commit to slice-6 start, the dashboard panels were empty and no
  automated check caught it. Without an app-side probe, port-conflict
  misconfig is silent until someone opens Grafana (could be days,
  weeks, or never in a small team). Violates principle 12 explicitly.
- **Rejected because**: this is the textbook anti-pattern principle 12
  exists to prevent.

### C: Probe as a periodic background task (not blocking startup)
A `tokio::time::interval` task that scrapes `/metrics` every N seconds and
logs a warning on failure (but does NOT crash the process).

- **Pros**: Non-fatal; catches transient issues; provides observability
  into long-term health.
- **Cons**: A "warning that's logged but doesn't crash" has the same
  signal-to-noise problem as the empty-series state — operators learn
  to ignore it. Doesn't prevent the "serve traffic with broken metrics"
  state; just observes it. The startup-time blocking probe is the
  forcing function that turns silent failure into loud failure.
- **Rejected because**: defeats the purpose. Combine with A is possible
  but adds complexity for marginal value.

## Consequences

### Positive
- Catches the entire class of deploy-time metrics misconfig (wrong
  port, port-in-use, recorder swallow, network-policy filter) at process
  start, before traffic flows.
- Failure mode is "container restarts in a loop", which is loud and
  observable in any container orchestrator (k8s, Docker Swarm, plain
  systemd with restart=on-failure). Restart loop is the operator's
  pager signal.
- Mirrors slice-5 precedent (`Store::probe()`) — known pattern, low
  novelty cost; reviewers already understand the model.
- Self-scrape latency is negligible (<10ms on localhost); doesn't
  meaningfully slow startup.
- The three-part assertion (200 + non-empty + startup-counter present)
  catches semantically distinct failure modes; orthogonal probes per
  principle 12's three-layer enforcement model.

### Negative
- Adds ~25-35 LOC + a `reqwest` dependency. Actually, `reqwest` is
  already in the workspace (used by the existing acceptance harness);
  the production binary gains it as a tiny new dependency for the probe
  call. Alternative: use `hyper` directly (already in axum's tree) to
  avoid the additional crate. Recommended: use `reqwest` for readability;
  the slice-5 probe established the precedent of using high-level
  HTTP clients in startup code.
- Adds ~10ms to startup time. Acceptable for a process that lives for
  hours/days.
- A genuinely-broken downstream (e.g., Prometheus is down but our
  metrics endpoint is fine) does NOT trip this probe — the probe is
  self-contained, not end-to-end through Prometheus. This is intentional:
  the probe verifies OUR adapter's contract, not the downstream
  consumer's availability.
- Probe failures during deployment rollout (e.g., port-conflict because
  the previous pod hasn't released `METRICS_PORT` yet) cause the new pod
  to restart-loop briefly. Mitigation: container orchestrator's grace
  period + the existing previous-pod-shutdown ordering should prevent
  the conflict; if not, add a short startup delay before the probe (~1s)
  to give the OS time to release the port.

### Neutral
- Reversibility: the probe is a single function call in `main.rs`;
  commenting it out (or guarding with `if env::var("SKIP_METRICS_PROBE").is_ok()`)
  is a one-line change for emergency rollback. NOT recommended as
  permanent posture — the entire ADR's point is "no silent failure mode".
- The probe complements (does NOT replace) the existing `Store::probe()`.
  Both probes are required; both must pass; either failure refuses
  startup. They are layered, not alternative.

## Inheritance from slice-5 lesson #6

The slice-5 evolution doc identifies lesson #6: "Substrate lies — probe
or be probed." The lesson articulates that any adapter operating against
an external surface (file system, kernel syscall, network) must
empirically verify the surface honors its contract; assumptions don't
survive contact with reality.

The metrics sidecar is exactly such an adapter: it operates against a
TCP port, an HTTP exposition format, and a process-global recorder
singleton. Each of those substrates has historical "lies" — the recorder
silently dropping installs is a known `metrics-exporter-prometheus` foot
gun (the comment in `metrics_server.rs` acknowledges it); TCP port-in-use
manifests as a panic-or-continue depending on bind code; Docker overlay
networks have been observed to filter loopback in certain mTLS
configurations.

This ADR applies the lesson uniformly: the metrics adapter probes the
metrics substrate, just as the store adapter probes the Postgres
substrate.

## Verification

- An acceptance scenario starts the process with an invalid
  `METRICS_PORT` (e.g., `1` which would require root) and asserts the
  process exits non-zero with a `health.startup.refused` log line. The
  test framework reads the log and asserts the structured event shape.
- An acceptance scenario starts the process normally and asserts (a)
  the probe runs to completion, (b) the main HTTP listener accepts
  traffic AFTER the probe passes.
- A failure-injection unit test: construct a `PrometheusHandle` whose
  `render()` returns an empty string (simulating recorder swallow);
  call `probe()`; assert it returns `Err` with a message mentioning
  the missing `foundry_app_startup_total` line.
- A failure-injection acceptance test: bind a dummy socket to
  `METRICS_PORT` before process start; assert the process exits
  non-zero (bind conflict caught by the sidecar listener; probe never
  runs because the listener never binds — equivalent operational
  signal).
- An assertion that `probe()` is called from `main.rs` after the
  sidecar start and before the main listener spawn (the orthogonality
  guarantee — probe before traffic, not after).
