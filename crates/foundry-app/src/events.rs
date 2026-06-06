//! US-09 — Server-Sent Events handler.
//!
//! Route: `GET /team/{team_slug}/project/{project_slug}/events`
//!
//! Pipeline:
//!   request → auth (session) → authz (team membership) → project lookup
//!   → subscribe to broadcast → filter by project_id → emit SSE lines.
//!
//! Wire format follows the SSE spec subset our minimal harness parses:
//!   - `event: <name>\ndata: <json>\n\n` for real events
//!   - `:keepalive\n\n` for heartbeats
//!
//! The handler is hand-rolled rather than using `axum::response::sse`
//! because the harness needs deterministic line shapes (the heartbeat
//! must emit a comment line, not a JSON keepalive event) and the
//! `Sse` adapter wraps every chunk in its own keep-alive abstraction.
//! At ~80 lines of streaming code the hand-roll is cheaper than the
//! mismatch.

use crate::bootstrap::SessionUser;
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
use askama::Template;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use foundry_realtime::EventPayload;
use futures::stream::Stream;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::broadcast::Receiver;
use tokio::time::{Instant, Sleep};
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use tower_sessions::Session;

const X_ACCEL_BUFFERING: &str = "x-accel-buffering";

pub async fn sse_stream(
    State(state): State<AppState>,
    Path((team_slug, project_slug)): Path<(String, String)>,
    session: Session,
) -> Response {
    // Subscribe to the broadcast IMMEDIATELY, before the slower
    // auth + project-lookup queries. Otherwise the test harness's
    // race window (SSE-open returns on headers, but the handler body
    // doesn't run until session extraction completes ~tens of ms
    // later) means a NOTIFY that fires in the meantime is broadcast
    // to zero receivers and lost. The receiver is dropped if any of
    // the subsequent auth checks fail.
    let rx = state.realtime_tx.subscribe();

    // (1) Authentication.
    let Some(user) = signed_in_user(&session).await else {
        drop(rx);
        return unauthorized_response();
    };

    // (2) Team lookup + membership.
    let team = match state
        .store
        .find_team_by_slug(user.workspace_id, &team_slug)
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => {
            drop(rx);
            return forbidden_response(&team_slug);
        }
        Err(err) => {
            drop(rx);
            tracing::error!(%err, "events: find_team_by_slug failed");
            return internal_error();
        }
    };
    let is_member = match state.store.is_team_member(team.id, user.user_id).await {
        Ok(b) => b,
        Err(err) => {
            drop(rx);
            tracing::error!(%err, "events: is_team_member failed");
            return internal_error();
        }
    };
    if !is_member {
        drop(rx);
        return forbidden_response(&team_slug);
    }

    // (3) Project lookup so we know which project_id to filter on.
    let project = match state
        .store
        .find_project_by_slug(team.id, &project_slug)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => {
            drop(rx);
            return forbidden_response(&team_slug);
        }
        Err(err) => {
            drop(rx);
            tracing::error!(%err, "events: find_project_by_slug failed");
            return internal_error();
        }
    };

    // Slice 6 (ADR-013) — RAII gauge for the live-subscriber metric.
    // Construction increments `sse_subscribers_total{project_id=...}`
    // by 1; Drop on the streaming future decrements it back. Drop
    // fires uniformly on clean disconnect, panic unwind, and server
    // graceful shutdown — no cleanup arm to forget.
    let gauge = foundry_realtime::SubscriberGauge::new(project.id);

    let heartbeat_ms = state.sse_heartbeat_ms.max(50);
    let stream = SseStream::new(rx, project.id, Duration::from_millis(heartbeat_ms), gauge);

    let body = Body::from_stream(stream);
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/event-stream")
        .header(CACHE_CONTROL, "no-cache")
        .header(X_ACCEL_BUFFERING, "no")
        .body(body)
        .expect("build SSE response")
}

async fn signed_in_user(session: &Session) -> Option<SessionUser> {
    session
        .get::<SessionUser>(SESSION_KEY_USER_ID)
        .await
        .ok()
        .flatten()
}

fn unauthorized_response() -> Response {
    // US-R04: the sign-in-required body now renders through the shared base
    // layout (links the vendored /static stylesheet). Selector-and-substring-
    // identical to the prior bare-<head> string — same "Sign-in required…" copy
    // and `<a href="/sign-in">` link. The 401 status + content type are the
    // byte-stable control-flow contract and are UNCHANGED.
    let body = crate::views::EventsSigninRequired
        .render()
        .expect("events_signin_required.html renders");
    (
        StatusCode::UNAUTHORIZED,
        [(CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response()
}

fn forbidden_response(team_slug: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        [(CONTENT_TYPE, "text/plain; charset=utf-8")],
        format!("Not a member of team {team_slug:?}."),
    )
        .into_response()
}

fn internal_error() -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}

/// Streaming implementation of an SSE response body.
///
/// Two timing-driven sources interleave:
/// - Realtime events arriving on the broadcast receiver.
/// - A heartbeat timer ticking every `heartbeat` interval.
///
/// First emitted byte is an immediate `:ready\n\n` comment so clients
/// know the broadcast Receiver is live BEFORE they trigger
/// side-effects that should produce events. Without this handshake the
/// test harness can race the subscription set-up.
///
/// We expose the Receiver as an async stream via `BroadcastStream` —
/// that wrapper owns the wait-list registration across polls (a hand
/// rolled `recv()` future would be unregistered each `poll_next`
/// invocation and miss wake-ups).
///
/// `RecvError::Lagged` (slow client) is treated as "skip and continue";
/// the browser's EventSource sees no gap because the broadcast is the
/// in-process bridge, not the wire.
struct SseStream {
    inner: BroadcastStream<EventPayload>,
    project_filter: uuid::Uuid,
    heartbeat: Duration,
    next_heartbeat: Pin<Box<Sleep>>,
    sent_ready: bool,
    /// Slice 6 (ADR-013) — RAII guard that holds the
    /// `sse_subscribers_total` gauge incremented for the lifetime of
    /// the stream future. When the stream is dropped (client
    /// disconnect, server shutdown, panic unwind) `_gauge` drops with
    /// it and the gauge decrements automatically. The leading `_` is
    /// intentional — this field is held purely for its Drop side
    /// effect.
    _gauge: foundry_realtime::SubscriberGauge,
}

impl SseStream {
    fn new(
        rx: Receiver<EventPayload>,
        project_filter: uuid::Uuid,
        heartbeat: Duration,
        gauge: foundry_realtime::SubscriberGauge,
    ) -> Self {
        Self {
            inner: BroadcastStream::new(rx),
            project_filter,
            heartbeat,
            next_heartbeat: Box::pin(tokio::time::sleep(heartbeat)),
            sent_ready: false,
            _gauge: gauge,
        }
    }

    fn reset_heartbeat(self: &mut Pin<&mut Self>) {
        let next = Instant::now() + self.heartbeat;
        self.next_heartbeat.as_mut().reset(next);
    }
}

impl Stream for SseStream {
    type Item = Result<bytes::Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Initial readiness marker — the client polls this so it knows
        // the SSE handler has wired up its broadcast receiver before
        // the test driver triggers an event-producing action.
        if !self.sent_ready {
            self.sent_ready = true;
            cx.waker().wake_by_ref();
            return Poll::Ready(Some(Ok(bytes::Bytes::from_static(b":ready\n\n"))));
        }

        let project_filter = self.project_filter;

        // Poll the broadcast stream first; if an event for our project
        // is ready, ship it immediately.
        loop {
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(event))) => {
                    if event.project_id != project_filter {
                        // Foreign event — loop to poll for the next.
                        continue;
                    }
                    let json = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());
                    let frame = format!("event: {}\ndata: {}\n\n", event.event_type, json);
                    return Poll::Ready(Some(Ok(bytes::Bytes::from(frame))));
                }
                Poll::Ready(Some(Err(BroadcastStreamRecvError::Lagged(_)))) => {
                    // Slow consumer — drop the gap, continue.
                    continue;
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => break,
            }
        }

        // No event ready — check the heartbeat timer.
        if self.next_heartbeat.as_mut().poll(cx).is_ready() {
            self.reset_heartbeat();
            return Poll::Ready(Some(Ok(bytes::Bytes::from_static(b":keepalive\n\n"))));
        }

        Poll::Pending
    }
}
