//! Slice 6 (ADR-010 / D7 / NFR-PERF-05) — middleware overhead microbench.
//!
//! Measures the per-request overhead added by
//! [`foundry_app::metrics_server::request_tracking_layer`] versus a
//! no-op handler with no layer.  P95 added overhead must be ≤ 10 µs
//! per request (the slice-6 D7 performance budget; back-propagated to
//! slice-1 NFR-PERF-05 in the 2026-05-25 DISTILL pass).
//!
//! ## Why a hand-rolled bench instead of `criterion`
//!
//! Adding `criterion` would introduce a new workspace dependency,
//! which violates the slice-6 task brief ("NO new crate dependencies").
//! The architecture document explicitly listed criterion as ONE of the
//! options ("microbench harness (criterion or `wrk` + `hyperfine` shell
//! script)") — this file implements the equivalent of the latter
//! using std-only primitives:
//!
//!   - `std::time::Instant` for high-resolution timing (the same source
//!     the production middleware uses for its histogram observation).
//!   - `std::hint::black_box` to prevent the optimizer from constant-
//!     folding the request shape.
//!   - In-process sort to compute P50 / P95 / P99 percentiles across
//!     the sample population.
//!
//! ## Methodology
//!
//! For each of the 27 routes the production router exposes (the union
//! of `crates/foundry-app/src/lib.rs::build_router` plus the slice-1
//! `/healthz` + `/readyz` sidecar routes), we build:
//!   - a NO-LAYER baseline: a router with just the route + a no-op
//!     handler returning `200 OK`.
//!   - a WITH-LAYER probe: the same router wrapped in
//!     `request_tracking_layer()`.
//!
//! For each, we issue N=10_000 synthetic in-memory requests via
//! `tower::Service::call`, measuring per-request wall-clock.
//!
//! The "added overhead" per route is `with_layer_p95 - baseline_p95`.
//! We aggregate the per-route added P95 across all 27 routes, then
//! report (and assert) the overall P95.
//!
//! Run via:
//!
//!   ```text
//!   cargo run --release --bin middleware_overhead_bench -p foundry-app --features bench
//!   ```
//!
//! ## Acceptance contract anchor
//!
//! The `@manual @nfr-perf-05` cucumber scenario (#10 in
//! `crates/foundry-acceptance/tests/features/handler-instrumentation.feature`)
//! is the documented contract anchor; this bench is the executable
//! enforcement.

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use foundry_app::metrics_server::request_tracking_layer;
use std::hint::black_box;
use std::time::{Duration, Instant};
use tower::ServiceExt;

/// Sample size per route per variant. 10k iterations is enough to get
/// a stable P95 in <1s per route on modern hardware (~30s total bench
/// wall-clock for the 27-route × 2-variant matrix).
const SAMPLES_PER_ROUTE: usize = 10_000;

/// The slice-6 D7 performance budget: ≤10 µs P95 added per request.
const P95_BUDGET: Duration = Duration::from_micros(10);

#[tokio::main(flavor = "current_thread")]
async fn main() {
    eprintln!("[middleware_overhead] starting bench …");
    eprintln!("[middleware_overhead] samples per route per variant: {SAMPLES_PER_ROUTE}");
    eprintln!(
        "[middleware_overhead] P95 added-overhead budget: {} µs",
        P95_BUDGET.as_micros()
    );

    // The 27-route surface the production router exposes (cf.
    // crates/foundry-app/src/lib.rs::build_router). We synthesize
    // requests against each so the cardinality across routes matches
    // production. The exact handler-side cost is irrelevant — both
    // the baseline and the with-layer variants share the same handler.
    let routes: Vec<(&str, Method)> = vec![
        ("/healthz", Method::GET),
        ("/readyz", Method::GET),
        ("/dashboard", Method::GET),
        ("/bootstrap", Method::GET),
        ("/bootstrap", Method::POST),
        ("/invites", Method::POST),
        ("/workspaces", Method::POST),
        ("/sign-in", Method::GET),
        ("/sign-in", Method::POST),
        ("/sign-out", Method::POST),
        ("/forgot-password", Method::GET),
        ("/forgot-password", Method::POST),
        ("/keyboard-help", Method::GET),
        ("/", Method::GET),
        ("/team/eng/projects/new", Method::GET),
        ("/team/eng/projects", Method::POST),
        ("/team/eng/project/foo", Method::GET),
        ("/team/eng/project/foo/issues", Method::POST),
        ("/team/eng/project/foo/issues/new", Method::GET),
        ("/team/eng/project/foo/search", Method::GET),
        ("/team/eng/project/foo/issues/1/state", Method::POST),
        ("/team/eng/project/foo/events", Method::GET),
        ("/team/eng/project/foo/issues/1", Method::GET),
        ("/team/eng/project/foo/issues/1/comments", Method::POST),
        (
            "/team/eng/project/foo/issues/1/comments/abc/edit",
            Method::GET,
        ),
        ("/team/eng/project/foo/issues/1/comments/abc", Method::GET),
        ("/team/eng/project/foo/issues/1/comments/abc", Method::PATCH),
        (
            "/team/eng/project/foo/issues/1/comments/abc",
            Method::DELETE,
        ),
    ];

    let mut all_overheads_ns: Vec<u128> = Vec::with_capacity(routes.len() * SAMPLES_PER_ROUTE);
    let mut per_route_summary = Vec::with_capacity(routes.len());

    for (path, method) in &routes {
        // Baseline: minimal router with this route mounted; no layer.
        let baseline = synthetic_router(path, method.clone(), /* with_layer */ false);
        // With layer: same router + the request-tracking layer.
        let probed = synthetic_router(path, method.clone(), /* with_layer */ true);

        let baseline_samples = run_samples(baseline, path, method.clone()).await;
        let probed_samples = run_samples(probed, path, method.clone()).await;

        let baseline_p95 = percentile_ns(&baseline_samples, 95.0);
        let probed_p95 = percentile_ns(&probed_samples, 95.0);
        let added = probed_p95.saturating_sub(baseline_p95);

        // Per-sample added overhead is the WITH minus the matched-index
        // BASELINE sample. We use the same N samples per variant; the
        // pair-wise difference at index k approximates added cost per
        // request (timing noise washes out across N).
        let mut paired = Vec::with_capacity(baseline_samples.len());
        for (b, p) in baseline_samples.iter().zip(probed_samples.iter()) {
            paired.push(p.saturating_sub(*b));
        }
        all_overheads_ns.extend(&paired);

        per_route_summary.push((
            path.to_string(),
            method.clone(),
            baseline_p95,
            probed_p95,
            added,
        ));
    }

    eprintln!();
    eprintln!("[middleware_overhead] per-route summary:");
    eprintln!(
        "  {:<60}  {:>10}  {:>10}  {:>10}",
        "route", "base_p95", "probe_p95", "added"
    );
    for (path, method, base, probe, added) in &per_route_summary {
        let label = format!("{} {}", method.as_str(), path);
        eprintln!(
            "  {:<60}  {:>8} ns  {:>8} ns  {:>8} ns",
            label, base, probe, added,
        );
    }

    let overall_p95 = percentile_ns(&all_overheads_ns, 95.0);
    let overall_p99 = percentile_ns(&all_overheads_ns, 99.0);
    let overall_p50 = percentile_ns(&all_overheads_ns, 50.0);

    eprintln!();
    eprintln!(
        "[middleware_overhead] overall added-overhead percentiles across {} routes:",
        routes.len()
    );
    eprintln!("  P50: {overall_p50} ns");
    eprintln!(
        "  P95: {overall_p95} ns  (budget: {} ns)",
        P95_BUDGET.as_nanos()
    );
    eprintln!("  P99: {overall_p99} ns");

    let budget_ns = P95_BUDGET.as_nanos();
    if overall_p95 > budget_ns {
        eprintln!();
        eprintln!(
            "[middleware_overhead] FAIL — overall added P95 = {overall_p95} ns > budget {budget_ns} ns ({} µs)",
            P95_BUDGET.as_micros()
        );
        std::process::exit(1);
    }

    eprintln!();
    eprintln!(
        "[middleware_overhead] PASS — overall added P95 = {overall_p95} ns ≤ budget {budget_ns} ns ({} µs)",
        P95_BUDGET.as_micros()
    );
}

/// Build a minimal router with `path`+`method` mounted to a no-op
/// handler, optionally wrapped with the request-tracking layer.
fn synthetic_router(path: &str, method: Method, with_layer: bool) -> Router {
    async fn noop() -> impl IntoResponse {
        StatusCode::OK
    }
    let router = match method {
        Method::GET => Router::new().route(path, get(noop)),
        Method::POST => Router::new().route(path, post(noop)),
        Method::PATCH => Router::new().route(path, axum::routing::patch(noop)),
        Method::DELETE => Router::new().route(path, axum::routing::delete(noop)),
        _ => Router::new().route(path, get(noop)),
    };
    if with_layer {
        router.layer(request_tracking_layer())
    } else {
        router
    }
}

/// Issue `SAMPLES_PER_ROUTE` synthetic requests through the router
/// and return per-request elapsed-ns measurements.
async fn run_samples(router: Router, path: &str, method: Method) -> Vec<u128> {
    let mut samples = Vec::with_capacity(SAMPLES_PER_ROUTE);
    // Warm-up: first N=200 iterations land outside the histogram to
    // amortise router-state caching + JIT-like first-call effects.
    let warmup = 200usize;
    for i in 0..(SAMPLES_PER_ROUTE + warmup) {
        let req = Request::builder()
            .method(method.clone())
            .uri(path)
            .body(Body::empty())
            .expect("synthetic request");
        let started = Instant::now();
        let response = router.clone().oneshot(req).await.expect("oneshot");
        let elapsed = started.elapsed().as_nanos();
        // Force read of the response to defeat dead-code elimination.
        black_box(response.status());
        if i >= warmup {
            samples.push(elapsed);
        }
    }
    samples
}

/// Compute `percentile`-th percentile (0.0..=100.0) of a sample
/// population in nanoseconds. Uses nearest-rank (not interpolated).
fn percentile_ns(samples: &[u128], percentile: f64) -> u128 {
    if samples.is_empty() {
        return 0;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let rank = ((percentile / 100.0) * sorted.len() as f64).ceil() as usize;
    let idx = rank.saturating_sub(1).min(sorted.len() - 1);
    sorted[idx]
}
