//! Sidecar metrics listener (NFR-OBS-03) + slice-6 instrumentation
//! middleware + startup self-scrape probe.
//!
//! Slice 1 / DEVOPS responsibilities (unchanged):
//!   - Bind a second HTTP listener on `METRICS_PORT` (default 9090) that
//!     exposes a Prometheus text-format `/metrics` endpoint.
//!   - Own the [`metrics_exporter_prometheus::PrometheusHandle`].
//!
//! Slice 6 additions (ADRs 010, 011, 014):
//!   - [`request_tracking_layer`] — tower middleware factory that emits
//!     `http_requests_total{path,method,status}` + the matching
//!     `http_request_duration_seconds` histogram once per routed
//!     request. Wired into the main router via a single `.layer()` in
//!     `build_router`. Label cardinality is hard-capped at the bounded
//!     triple per ADR-011; unmatched routes emit `path="<unmatched>"`.
//!   - [`probe`] — self-scrape `/metrics` startup probe per ADR-014.
//!     Refuses to start if the sidecar listener is unreachable, returns
//!     a non-200, returns an empty body, or omits the
//!     `foundry_app_startup_total` startup-counter line.

use anyhow::Context;
use axum::body::Body;
use axum::extract::{MatchedPath, State};
use axum::http::{Method, Request, StatusCode};
use axum::middleware::{from_fn, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// Sentinel `path` label value for requests that did NOT match any
/// router route (404s to arbitrary URIs). Hard-coded per ADR-011 to
/// bound the cardinality of `http_requests_total` against random-URI
/// 404 storms. Without this fallback, every distinct unmatched URI
/// would mint a new time series.
pub const UNMATCHED_ROUTE_LABEL: &str = "<unmatched>";

/// Install the global metrics recorder and return its render handle.
///
/// Safe to call exactly once per process. The acceptance harness does
/// NOT call this (the test app doesn't expose `/metrics` and we'd hit
/// "global recorder already installed" on the second scenario).
pub fn install_recorder() -> anyhow::Result<PrometheusHandle> {
    PrometheusBuilder::new()
        .install_recorder()
        .context("install Prometheus metrics recorder")
}

/// Bind the metrics listener on `host:port` and serve `/metrics`.
///
/// Returns once the listener is bound; the actual `serve` runs on a
/// background task. The task lives for the lifetime of the process —
/// graceful shutdown is implicit at process exit.
pub async fn serve(host: &str, port: u16, handle: PrometheusHandle) -> anyhow::Result<SocketAddr> {
    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .with_context(|| format!("parse metrics addr {host}:{port}"))?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind metrics listener on {addr}"))?;
    let bound = listener.local_addr()?;

    let router = Router::new()
        .route("/metrics", get(render_metrics))
        .route("/healthz", get(metrics_healthz))
        .with_state(handle);

    tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, router).await {
            tracing::error!(error = %err, "metrics listener stopped");
        }
    });

    Ok(bound)
}

async fn render_metrics(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    (StatusCode::OK, handle.render())
}

async fn metrics_healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

// ---------------------------------------------------------------------
// Slice 6 (ADR-010, ADR-011) — request-tracking middleware
// ---------------------------------------------------------------------

/// Tower middleware factory that emits the slice-6 HTTP request
/// metrics on every routed request. Wired into `build_router` via
/// a single `.layer(metrics_server::request_tracking_layer())` call.
///
/// Per ADR-010: lives at the same tower-stack position as CSRF /
/// session / request-id layers — one layer per request, applies to
/// every route uniformly, zero handler-signature changes.
///
/// Per ADR-011: label keys are hard-coded to the bounded triple
/// `{path, method, status}`. NO other label is emitted. Adding any
/// high-cardinality label (`user_id`, `workspace_id`, `project_id`,
/// `request_id`, IP, UA) requires a new ADR — the unit test below
/// fails closed on regression.
///
/// `path` uses `axum::extract::MatchedPath` (the route template, e.g.
/// `/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments`),
/// NEVER the concrete URI. Requests that don't match any route emit
/// `path="<unmatched>"` per [`UNMATCHED_ROUTE_LABEL`].
///
/// Returned as `impl Layer<...>` to avoid spelling out the unnameable
/// `FromFnLayer<...>` projection that `axum::middleware::from_fn`
/// produces — the layer composes inside `build_router`'s tower stack
/// without requiring a concrete type signature here.
pub fn request_tracking_layer() -> impl tower::Layer<
    axum::routing::Route,
    Service = impl tower::Service<
        axum::extract::Request,
        Response = axum::response::Response,
        Error = std::convert::Infallible,
        Future = impl Send + 'static,
    > + Clone
                  + Send
                  + Sync
                  + 'static,
> + Clone {
    // The `from_fn` factory pattern keeps the layer construction
    // declarative at the call-site in `build_router`. The middleware
    // body lives in `track_request`.
    from_fn(track_request)
}

/// The actual per-request work. Extracted from
/// [`request_tracking_layer`] for unit-testability and so the layer
/// factory stays a one-liner.
///
/// Order of operations:
///   1. Capture `method` + `MatchedPath` (or `<unmatched>` sentinel)
///      from the inbound request — BEFORE forwarding, so a panicking
///      handler doesn't lose the labels.
///   2. Mark the start instant.
///   3. Forward the request through the rest of the tower stack.
///   4. Observe the response status (3-digit code, full granularity
///      per ADR-011 — preserves slice-5 ADR-008's 410-vs-404
///      distinction).
///   5. Emit exactly ONE `http_requests_total` increment and ONE
///      `http_request_duration_seconds` observation.
async fn track_request(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let path = matched_path_label(&request);
    let started = Instant::now();

    let response = next.run(request).await;

    let status = response.status();
    let elapsed = started.elapsed();

    record_request_metrics(&path, &method, status, elapsed);

    response
}

/// Extract the route template from the inbound request, or fall back
/// to [`UNMATCHED_ROUTE_LABEL`] when no route matched (typical for
/// 404s to arbitrary URIs).
///
/// Hot-path inline-able; ~100ns expected per call (axum stores
/// `MatchedPath` as a request extension; this is a hashtable lookup).
fn matched_path_label(request: &Request<Body>) -> String {
    request
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| UNMATCHED_ROUTE_LABEL.to_string())
}

/// Emit the two metrics for a completed request. Extracted as a
/// separate function so the cardinality unit test below can call it
/// directly with synthesized labels and assert on what the metrics
/// recorder observed.
fn record_request_metrics(path: &str, method: &Method, status: StatusCode, elapsed: Duration) {
    let method_label = method.as_str().to_string();
    let status_label = status.as_u16().to_string();
    // Bounded triple — `{path, method, status}` ONLY. Hard-coded; the
    // cardinality unit test fails if any additional label is added.
    metrics::counter!(
        "http_requests_total",
        "path" => path.to_string(),
        "method" => method_label.clone(),
        "status" => status_label.clone(),
    )
    .increment(1);
    metrics::histogram!(
        "http_request_duration_seconds",
        "path" => path.to_string(),
        "method" => method_label,
        "status" => status_label,
    )
    .record(elapsed.as_secs_f64());
}

// ---------------------------------------------------------------------
// Slice 6 (ADR-014) — startup self-scrape probe
// ---------------------------------------------------------------------

/// Self-scrape `/metrics` after the sidecar listener binds. Returns
/// `Ok(())` only if all three substrate-lie checks pass:
///
/// 1. HTTP 200 reachable on `http://127.0.0.1:{addr.port()}/metrics`
///    — catches wrong port, port-in-use, loopback firewall rule, or
///    the sidecar listener failing to bind silently.
/// 2. Response body is non-empty — catches "bind succeeded but
///    handler isn't wired to the recorder".
/// 3. Body contains the `foundry_app_startup_total` line — catches
///    the silent-recorder-swallow failure mode where the global
///    recorder was replaced or its installs were lost.
///
/// Probe host is hard-coded to `127.0.0.1` regardless of
/// `METRICS_HOST` (per ADR-014, architecture.md line 305).
/// Containers that bind the sidecar to `0.0.0.0` are still probed
/// over the loopback path that is reachable from the same process.
///
/// On `Err`, the caller (`main.rs`) bubbles the error up via
/// `anyhow::Context` propagation; the process exits non-zero and the
/// container orchestrator restarts the pod. The restart loop surfaces
/// the misconfig instead of silently serving traffic with broken
/// metrics.
pub async fn probe(addr: SocketAddr) -> anyhow::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let probe_addr: SocketAddr =
        format!("127.0.0.1:{}", addr.port())
            .parse()
            .with_context(|| {
                format!(
                    "metrics probe: parse loopback addr for port {}",
                    addr.port()
                )
            })?;

    // Hand-roll the HTTP/1.1 GET so the probe does not require a
    // production-side `reqwest` (or hyper-util) dependency — slice 6
    // ships zero new crate deps per task brief + architecture.md.
    // The body we expect is short (a few KB at slice-1 metric volume);
    // a 32KB read cap is more than enough for the substrate checks.
    let request_bytes = "GET /metrics HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
    let probe_op = async {
        let mut stream = tokio::net::TcpStream::connect(probe_addr)
            .await
            .with_context(|| format!("metrics probe: connect to {probe_addr}"))?;
        stream
            .write_all(request_bytes.as_bytes())
            .await
            .context("metrics probe: write request")?;
        let mut raw = Vec::with_capacity(8 * 1024);
        let mut buf = [0u8; 4096];
        let mut total = 0usize;
        const READ_CAP: usize = 256 * 1024;
        loop {
            let n = stream
                .read(&mut buf)
                .await
                .context("metrics probe: read response")?;
            if n == 0 {
                break;
            }
            total += n;
            raw.extend_from_slice(&buf[..n]);
            if total >= READ_CAP {
                break;
            }
        }
        Ok::<Vec<u8>, anyhow::Error>(raw)
    };

    // ADR-014 sets the probe latency expectation at "negligible
    // (<10ms on localhost)". A 5-second deadline gives generous
    // headroom for slow CI loopback while still surfacing genuinely
    // broken networking promptly.
    let raw = tokio::time::timeout(Duration::from_secs(5), probe_op)
        .await
        .context("metrics probe: timed out after 5s")??;

    let response = String::from_utf8_lossy(&raw);
    let mut head_body = response.splitn(2, "\r\n\r\n");
    let head = head_body.next().unwrap_or("");
    let body = head_body.next().unwrap_or("");

    let status_line = head.lines().next().unwrap_or("");
    let status_code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if status_code != 200 {
        tracing::error!(
            event = "health.startup.refused",
            probe = "metrics",
            reason = "non_200",
            status = status_code,
            "metrics startup probe failed: expected 200, got {status_code}"
        );
        anyhow::bail!("metrics probe: expected 200, got {status_code}");
    }

    // Chunked transfer encoding is the most common axum response shape
    // for arbitrary-length text bodies. The body block we just split
    // is the raw chunk-encoded payload; for the substrate-lie checks
    // we just need to know "is anything here" + "does it contain the
    // startup-counter line". A naive substring match against the raw
    // chunked stream is sufficient because the metric names we look
    // for are short, well-known, and never appear in chunk-size headers.
    if body.is_empty() {
        tracing::error!(
            event = "health.startup.refused",
            probe = "metrics",
            reason = "empty_body",
            "metrics startup probe failed: body empty"
        );
        anyhow::bail!("metrics probe: body empty");
    }
    if !body.contains("foundry_app_startup_total") {
        tracing::error!(
            event = "health.startup.refused",
            probe = "metrics",
            reason = "startup_counter_missing",
            "metrics startup probe failed: foundry_app_startup_total line missing"
        );
        anyhow::bail!(
            "metrics probe: foundry_app_startup_total line missing — \
             recorder install swallowed?"
        );
    }
    tracing::info!(
        event = "health.startup.passed",
        probe = "metrics",
        body_bytes = body.len(),
        "metrics startup probe passed"
    );
    Ok(())
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    //! Slice 6 unit tests.
    //!
    //! The CARDINALITY unit test (per ADR-011 § Verification line 167)
    //! is the static enforcement of the bounded-triple invariant.
    //! Acceptance scenario #3 covers the runtime side (the
    //! `MatchedPath` template emitted as the `path` label); this test
    //! covers the static side (label KEY set, regardless of values).
    //!
    //! We avoid driving a real recorder install (the harness deliberately
    //! skips that to prevent the "global recorder already installed"
    //! panic across tests). Instead we render-scrape the snapshot the
    //! middleware emitted via a dedicated [`PrometheusBuilder`] handle
    //! built only for the test, exercising the SAME emission path via
    //! a scoped recorder.

    use super::*;
    use metrics_exporter_prometheus::PrometheusBuilder;

    /// Slice 6 — ADR-011 cardinality enforcement.
    ///
    /// Calls the middleware's emission path against a representative
    /// request shape and asserts the label KEY set on the scraped
    /// `http_requests_total` line is EXACTLY `{path, method, status}`.
    /// Fails closed on regression — a future contributor adding
    /// `user_id` or similar to [`record_request_metrics`] turns this
    /// test red before it lands in production.
    #[test]
    fn request_tracking_layer_emits_exactly_path_method_status() {
        // Build a SCOPED recorder for this test. Using
        // `with_local_recorder` (or `set_default`) avoids the global
        // install path that would collide across tests in the same
        // process.
        let recorder = PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();
        metrics::with_local_recorder(&recorder, || {
            record_request_metrics(
                "/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments",
                &Method::POST,
                StatusCode::CREATED,
                Duration::from_micros(450),
            );
        });
        let body = handle.render();

        // Locate the `http_requests_total` line in the scrape body.
        let line = body
            .lines()
            .find(|l| l.starts_with("http_requests_total{"))
            .unwrap_or_else(|| {
                panic!("no `http_requests_total{{...}}` line in scrape body:\n{body}")
            });
        let labels = extract_label_keys(line);
        let expected: std::collections::BTreeSet<String> = ["method", "path", "status"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            labels, expected,
            "cardinality regression: middleware emitted label keys {labels:?}, \
             expected exactly {expected:?} (line: {line})"
        );

        // And the matching histogram-derived summary line. The
        // metrics-exporter-prometheus default histogram representation
        // emits a `_count` aggregate line per series; the user-
        // controlled labels live on it exactly as on the counter.
        let hist_line = body
            .lines()
            .find(|l| l.starts_with("http_request_duration_seconds_count{"))
            .unwrap_or_else(|| {
                panic!(
                    "no `http_request_duration_seconds_count{{...}}` line in scrape body:\n{body}"
                )
            });
        let hist_labels = extract_label_keys(hist_line);
        assert_eq!(
            hist_labels, expected,
            "cardinality regression on histogram-count: emitted keys {hist_labels:?}, \
             expected exactly {expected:?} (line: {hist_line})"
        );
    }

    /// Slice 6 — ADR-014 startup-probe failure injection.
    ///
    /// When the recorder is installed but renders an empty body (the
    /// "recorder install was silently swallowed" failure mode), the
    /// probe MUST return `Err`. We exercise the probe against a
    /// stand-alone listener serving an empty `/metrics`.
    #[tokio::test]
    async fn probe_returns_err_when_body_empty() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind probe-test listener");
        let bound = listener.local_addr().expect("local_addr");
        let router = Router::new().route(
            "/metrics",
            get(|| async { (StatusCode::OK, String::new()) }),
        );
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        // Give the listener a moment to start accepting.
        tokio::time::sleep(Duration::from_millis(50)).await;
        let result = probe(bound).await;
        assert!(
            result.is_err(),
            "probe should fail when body is empty; got Ok"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("body empty"),
            "expected 'body empty' in error, got: {err}"
        );
    }

    /// Slice 6 — ADR-014 startup-probe failure injection (variant 2).
    ///
    /// When the recorder is installed and renders a non-empty body but
    /// the `foundry_app_startup_total` line is missing (the "handler
    /// is wired but the counter the DEVOPS slice emits at startup was
    /// lost" failure mode), the probe MUST return `Err`.
    #[tokio::test]
    async fn probe_returns_err_when_startup_counter_missing() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind probe-test listener");
        let bound = listener.local_addr().expect("local_addr");
        let router = Router::new().route(
            "/metrics",
            get(|| async {
                (
                    StatusCode::OK,
                    "# HELP some_other_metric Just a counter without our startup line\n\
                     # TYPE some_other_metric counter\n\
                     some_other_metric 42\n"
                        .to_string(),
                )
            }),
        );
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        let result = probe(bound).await;
        assert!(
            result.is_err(),
            "probe should fail when foundry_app_startup_total is missing; got Ok"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("foundry_app_startup_total"),
            "expected missing-line error, got: {err}"
        );
    }

    /// Sanity smoke — probe returns Ok against a body that contains
    /// the startup-counter line. Confirms the happy-path side of the
    /// three-part assertion.
    #[tokio::test]
    async fn probe_returns_ok_when_startup_counter_present() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind probe-test listener");
        let bound = listener.local_addr().expect("local_addr");
        let router = Router::new().route(
            "/metrics",
            get(|| async {
                (
                    StatusCode::OK,
                    "# TYPE foundry_app_startup_total counter\n\
                     foundry_app_startup_total 1\n"
                        .to_string(),
                )
            }),
        );
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        probe(bound).await.expect("probe should succeed");
    }

    /// Helper: extract the label KEY set from a Prometheus exposition
    /// line like `http_requests_total{path="...",method="...",status="..."} 1`.
    /// Returns the keys as a BTreeSet so set-equality is order-insensitive.
    ///
    /// Naive `split(',')` is wrong because label VALUES may contain
    /// commas (e.g. the route template
    /// `/team/{team_slug}/...` doesn't, but in general the exposition
    /// format permits it). We walk the bytes and only treat top-level
    /// commas (outside `"..."` quoted values) as separators.
    fn extract_label_keys(line: &str) -> std::collections::BTreeSet<String> {
        // `find('{')` lands on the first `{` of the label block —
        // route-template label VALUES like `/team/{team_slug}` also
        // contain `{` so we MUST not bound the block by the first `}`.
        // The block ends at the LAST `}` before the value field.
        let open = line.find('{').expect("line has `{`");
        // The exposition line ends with `} <value>`; the final `}` is
        // the closing brace of the label block.
        let close = line.rfind('}').expect("line has `}`");
        assert!(
            close > open,
            "malformed metric line — `}}` before `{{`: {line}"
        );
        let label_block = &line[open + 1..close];

        let mut keys = std::collections::BTreeSet::new();
        let mut current_key = String::new();
        let mut in_value = false;
        let mut in_quotes = false;
        let mut escape = false;
        for ch in label_block.chars() {
            if in_value {
                if escape {
                    escape = false;
                    continue;
                }
                match ch {
                    '\\' if in_quotes => escape = true,
                    '"' => in_quotes = !in_quotes,
                    ',' if !in_quotes => {
                        in_value = false;
                    }
                    _ => {}
                }
            } else {
                match ch {
                    '=' => {
                        let key = current_key.trim().to_string();
                        if !key.is_empty() {
                            keys.insert(key);
                        }
                        current_key.clear();
                        in_value = true;
                    }
                    ',' => {
                        // Stray comma between labels (shouldn't happen
                        // but stay defensive).
                        current_key.clear();
                    }
                    c => current_key.push(c),
                }
            }
        }
        keys
    }
}
