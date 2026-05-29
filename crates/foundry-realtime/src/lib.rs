//! foundry-realtime — SSE + Postgres LISTEN/NOTIFY fan-out.
//!
//! Slice 2 responsibilities:
//!
//! - Define [`EventPayload`] — the JSON envelope every replica
//!   serializes/deserializes through `pg_notify('issue_events', ...)`.
//!   The shape mirrors what migration `0003_outbox_notify.sql` emits
//!   from the trigger.
//! - [`run_pg_listener`] — long-lived background task that holds a
//!   dedicated `PgListener` connection (NOT borrowed from the request
//!   pool), `LISTEN issue_events`, decodes each NOTIFY, and forwards
//!   onto a `tokio::sync::broadcast::Sender<EventPayload>` so the
//!   in-process SSE handler can fan out to all locally connected
//!   subscribers.
//! - Reconnect-with-backoff: on connection loss we log and re-LISTEN
//!   with exponential backoff up to 30s. Slice 2 acceptance does not
//!   exercise reconnect (deferred per realtime-infrastructure.md), but
//!   the task must never panic and must survive a transient Postgres
//!   blip.
//!
//! The SSE handler itself lives in `foundry-app::events` — it depends
//! on `AppState` (sessions, store, broadcast channel) and is therefore
//! the wrong layer for this crate.

#![forbid(unsafe_code)]
#![deny(clippy::all)]

use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgListener, PgPoolOptions};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::broadcast;

/// Channel name shared by the `pg_notify` trigger and the per-replica
/// listener. Single channel by design (one project = one filter, not a
/// separate Postgres channel — see realtime-infrastructure.md fan-out
/// section).
pub const ISSUE_EVENTS_CHANNEL: &str = "issue_events";

/// Maximum reconnect backoff. Empirically derived from the
/// realtime-infrastructure.md design — long enough to absorb a
/// Postgres restart, short enough that operators see recovery within
/// SLA windows.
const RECONNECT_BACKOFF_MAX: Duration = Duration::from_secs(30);
const RECONNECT_BACKOFF_INITIAL: Duration = Duration::from_millis(100);

/// Slice 8 (ADR-019 / D2) — counter incremented once per LISTEN
/// connection drop at the [`run_pg_listener`] reconnect chokepoint.
/// Unlabelled (bounded at 1 series). Exported so the cardinality unit
/// test + the acceptance scrape assert against the same identifier the
/// production code emits. Register-at-0 happens in `foundry-app::main`.
pub const REALTIME_LISTEN_DISCONNECTS_TOTAL: &str = "realtime_listen_disconnects_total";

#[derive(Debug, Error)]
pub enum RealtimeError {
    #[error("notify failed: {0}")]
    NotifyFailed(String),
}

/// One realtime event flowing through the in-process broadcast.
///
/// The payload mirrors the JSON envelope emitted by the
/// `notify_outbox_event` trigger in migration 0003. `schema_version`
/// guards forward compatibility per `realtime-roadmap.md` invariant 4
/// (never rename a field; bump schema_version + add).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    pub event_type: String,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub timestamp: Option<String>,
    pub project_id: uuid::Uuid,
    #[serde(default)]
    pub workspace_id: Option<uuid::Uuid>,
    #[serde(default)]
    pub issue_id: Option<uuid::Uuid>,
    #[serde(default)]
    pub number: Option<i32>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub author_id: Option<uuid::Uuid>,
    #[serde(default)]
    pub state: Option<String>,
    // ---- US-10 CommentAdded additions -------------------------------
    /// Comment row id. Set on CommentAdded events; None otherwise.
    /// schema_version stays 1 — this is a forward-compatible field
    /// addition per realtime-roadmap.md invariant 4.
    #[serde(default)]
    pub comment_id: Option<uuid::Uuid>,
    /// Author email at fan-out time. Carried in the payload so
    /// subscribers can render the comment author without a JOIN against
    /// the users table (wave-decisions.md "Comment-event payload shape").
    #[serde(default)]
    pub author_email: Option<String>,
    // ---- US-10 slice-5 CommentDeleted addition (ADR-008) ------------
    /// Set to `Some(true)` on `CommentDeleted` events; `None` for every
    /// other event_type. Forward-compatible per realtime-roadmap.md
    /// invariant 4; `schema_version` stays at 1. Receivers that match
    /// on payload structure (not just `event_type`) can use this to
    /// detect tombstones without parsing the discriminator.
    #[serde(default)]
    pub deleted: Option<bool>,
}

fn default_schema_version() -> u32 {
    1
}

/// Spawn a background task that holds a dedicated LISTEN connection.
///
/// The caller is responsible for cloning `broadcast_tx` and storing
/// the corresponding `Receiver` factory (or `broadcast::Sender` —
/// receivers are minted on-demand via `subscribe()`) on AppState so
/// the SSE handler can subscribe.
///
/// Returns a `JoinHandle` so the binary can `.abort()` on shutdown.
pub fn spawn_pg_listener(
    database_url: String,
    broadcast_tx: broadcast::Sender<EventPayload>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        run_pg_listener(database_url, broadcast_tx).await;
    })
}

/// Long-lived LISTEN loop with exponential reconnect.
///
/// We use `sqlx::postgres::PgListener` which internally manages a
/// dedicated connection. The connection is NOT taken from the request
/// pool — `PgListener::connect` opens its own. The realtime
/// infrastructure design specifically warns against pgbouncer in
/// transaction-pooling mode here; that is a deployment concern, not
/// a code concern.
pub async fn run_pg_listener(database_url: String, broadcast_tx: broadcast::Sender<EventPayload>) {
    let mut backoff = RECONNECT_BACKOFF_INITIAL;
    loop {
        match listen_loop(&database_url, &broadcast_tx).await {
            Ok(()) => {
                // listen_loop only returns Ok on graceful shutdown
                // (caller dropped broadcast); exit.
                tracing::info!("pg_listener: broadcast channel closed, exiting");
                return;
            }
            Err(err) => {
                // Slice 8 (ADR-019 / D2) — the single decision chokepoint
                // where "we observed a LISTEN drop and will reconnect"
                // happens. Increment before the backoff sleep. Unlabelled
                // (bounded at exactly 1 series); register-at-0 lives in
                // `foundry-app::main` so Grafana shows a flat-zero baseline.
                metrics::counter!(REALTIME_LISTEN_DISCONNECTS_TOTAL).increment(1);
                tracing::warn!(
                    error = %err,
                    backoff_ms = backoff.as_millis() as u64,
                    "pg_listener: connection error, retrying after backoff"
                );
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(RECONNECT_BACKOFF_MAX);
            }
        }
    }
}

async fn listen_loop(
    database_url: &str,
    broadcast_tx: &broadcast::Sender<EventPayload>,
) -> Result<(), sqlx::Error> {
    // PgListener owns a dedicated connection; we keep it for the
    // lifetime of this function call. It also auto-reconnects
    // internally on some failure classes, but we keep our outer
    // backoff loop for the cases it does not handle.
    let mut listener = PgListener::connect(database_url).await?;
    listener.listen(ISSUE_EVENTS_CHANNEL).await?;
    tracing::info!("pg_listener: LISTEN {ISSUE_EVENTS_CHANNEL} established");

    loop {
        // No receivers means nobody cares — but we keep listening so
        // a future subscriber will see events. Only exit if the
        // sender side closes (all senders dropped — shutdown).
        if broadcast_tx.receiver_count() == 0 {
            // Yield, but stay in the loop. Cheap idle.
        }

        let notification = match listener.try_recv().await? {
            Some(n) => n,
            None => {
                // PgListener returns None on connection drop — let the
                // outer reconnect loop handle it.
                return Err(sqlx::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::ConnectionAborted,
                    "pg_listener: try_recv returned None",
                )));
            }
        };

        let payload_str = notification.payload();
        match serde_json::from_str::<EventPayload>(payload_str) {
            Ok(event) => {
                // SendError fires only if no receivers; expected during
                // quiet periods when no SSE client is subscribed.
                let _ = broadcast_tx.send(event);
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    payload = %payload_str,
                    "pg_listener: failed to decode NOTIFY payload, skipping"
                );
            }
        }
    }
}

/// Build a `(broadcast_tx, _rx)` pair suitable for AppState. Buffer
/// size sized to absorb a burst of events without backpressuring the
/// LISTEN loop; receivers that fall behind get a `RecvError::Lagged`
/// which the SSE handler treats as "client too slow, close stream".
pub fn build_broadcast() -> broadcast::Sender<EventPayload> {
    let (tx, _rx) = broadcast::channel::<EventPayload>(1024);
    tx
}

/// Helper for tests + main: build a dedicated 1-connection pool for
/// the LISTEN task. Reusing the existing request pool is forbidden by
/// the design (a LISTEN borrow would never be returned).
///
/// Returns the database URL since `PgListener::connect` wants a URL,
/// not a pool — this helper exists for symmetry with the broadcast
/// builder and as a future expansion point (e.g. if we ever switch
/// to a dedicated `PgConnection`).
#[allow(dead_code)]
pub async fn validate_listen_connectivity(database_url: &str) -> Result<(), sqlx::Error> {
    let _ = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await?;
    Ok(())
}

// ---------------------------------------------------------------------
// Slice 6 (ADR-013) — RAII gauge for live SSE subscribers
// ---------------------------------------------------------------------

/// Name of the Prometheus gauge counting currently-connected SSE
/// subscribers per project. Exported so unit tests can assert against
/// the same identifier the production code emits.
pub const SSE_SUBSCRIBERS_GAUGE: &str = "sse_subscribers_total";

/// RAII guard that increments the `sse_subscribers_total` gauge on
/// `new` and decrements it on `Drop`.
///
/// Construct one in the SSE handler immediately after subscribing to
/// the broadcast — the binding holds the guard for the lifetime of
/// the streaming future:
///
/// ```ignore
/// let _gauge = foundry_realtime::SubscriberGauge::new(project_id);
/// ```
///
/// Drop fires uniformly on:
///   - Clean client disconnect (browser tab close / EventSource.close())
///   - Server graceful shutdown (the streaming future is cancelled)
///   - Panic unwind (Rust's default panic strategy is `unwind`; if a
///     deployment ever switches to `panic = "abort"` this guarantee
///     no longer holds — see ADR-013 § Negative consequences)
///
/// `project_id` is kept as a label per ADR-013 / ADR-011 — bounded
/// by the count of projects with active SSE subscriptions (low
/// hundreds at the long tail; cardinality-safe).
#[derive(Debug)]
pub struct SubscriberGauge {
    project_id: uuid::Uuid,
}

impl SubscriberGauge {
    /// Increment the gauge by 1 and return the RAII handle. The
    /// matching decrement runs automatically via [`Drop`].
    pub fn new(project_id: uuid::Uuid) -> Self {
        metrics::gauge!(
            SSE_SUBSCRIBERS_GAUGE,
            "project_id" => project_id.to_string(),
        )
        .increment(1.0);
        Self { project_id }
    }
}

impl Drop for SubscriberGauge {
    fn drop(&mut self) {
        metrics::gauge!(
            SSE_SUBSCRIBERS_GAUGE,
            "project_id" => self.project_id.to_string(),
        )
        .decrement(1.0);
    }
}

#[cfg(test)]
mod gauge_tests {
    //! Slice 6 (ADR-013 § Verification) — panic-unwind correctness
    //! and inc/dec balance for [`SubscriberGauge`].

    use super::*;
    use metrics_exporter_prometheus::PrometheusBuilder;

    /// The gauge MUST balance to its pre-construction value after the
    /// guard is dropped on a normal scope exit.
    #[test]
    fn gauge_balances_after_normal_drop() {
        let recorder = PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();
        let project_id = uuid::Uuid::now_v7();
        metrics::with_local_recorder(&recorder, || {
            {
                let _g = SubscriberGauge::new(project_id);
                let body = handle.render();
                assert!(
                    body.contains(SSE_SUBSCRIBERS_GAUGE),
                    "expected gauge line present after `new`; body:\n{body}"
                );
                assert!(
                    body.lines().any(|l| {
                        l.starts_with(SSE_SUBSCRIBERS_GAUGE) && l.trim_end().ends_with(" 1")
                    }),
                    "expected gauge value 1 after `new`; body:\n{body}"
                );
            }
            // Guard dropped; gauge should be back to 0.
            let after = handle.render();
            assert!(
                after.lines().any(|l| {
                    l.starts_with(SSE_SUBSCRIBERS_GAUGE) && l.trim_end().ends_with(" 0")
                }),
                "expected gauge value 0 after Drop; body:\n{after}"
            );
        });
    }

    /// The gauge MUST balance even on panic unwind. This is the
    /// principle-12 substrate-lie probe for Rust's default
    /// `panic = "unwind"` strategy. If a future deployment ever
    /// switches to `panic = "abort"` this test would not run to its
    /// post-catch assertion (the process would abort), which is the
    /// invariant ADR-013 documents.
    #[test]
    fn gauge_balances_after_panic_unwind() {
        let recorder = PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();
        let project_id = uuid::Uuid::now_v7();
        let result = metrics::with_local_recorder(&recorder, || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _g = SubscriberGauge::new(project_id);
                panic!("simulated panic mid-stream");
            }))
        });
        assert!(
            result.is_err(),
            "scoped panic should propagate to catch_unwind"
        );
        let body = handle.render();
        assert!(
            body.lines()
                .any(|l| { l.starts_with(SSE_SUBSCRIBERS_GAUGE) && l.trim_end().ends_with(" 0") }),
            "expected gauge value 0 after unwind-triggered Drop; body:\n{body}"
        );
    }
}
