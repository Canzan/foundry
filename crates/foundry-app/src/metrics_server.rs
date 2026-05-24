//! Sidecar metrics listener (NFR-OBS-03).
//!
//! Binds a second HTTP listener on `METRICS_PORT` (default 9090) that
//! exposes a Prometheus text-format `/metrics` endpoint. Intentionally
//! bound on its own port so the load balancer never accidentally
//! relays scrape traffic to public clients (observability-infra.md
//! "Why a separate METRICS_PORT").
//!
//! This module owns the listener and the
//! [`metrics_exporter_prometheus::PrometheusHandle`]. The application
//! emits counters / histograms via the `metrics` crate facade; the
//! handle renders them on scrape.

use anyhow::Context;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::net::SocketAddr;

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
