//! Minimal SSE consumer for the US-09 / US-10 acceptance scenarios.
//!
//! Per `distill/wave-decisions.md` and `distill/driver.md` §2a, we
//! intentionally do NOT depend on `eventsource-client`. This module
//! is a ~150-line bespoke parser layered on `reqwest::Response::bytes_stream()`.
//!
//! Public surface:
//! - [`open_sse_subscription`] / [`open_sse_subscription_unauthenticated`]
//!   — issue the HTTP GET and (on 200) spawn a background tokio task
//!   that drains the response body into the in-memory event vec.
//! - [`SseSubscription::wait_for`] — wait up to `timeout` for an event
//!   matching `predicate`, returning the event + per-event arrival
//!   latency.
//! - [`SseSubscription::drain`] — snapshot current events with no wait.
//! - [`SseSubscription::heartbeat_count`] — count of `:keepalive`
//!   comment lines observed.
//!
//! SSE wire-format subset handled (per the SSE spec — the slice-2
//! production handler emits exactly this subset):
//!  - Lines beginning `:` are comments — `:keepalive` heartbeats.
//!  - Lines beginning `event:` set the next dispatch's event name.
//!  - Lines beginning `data:` accumulate into the dispatch buffer.
//!  - An empty line dispatches a buffered event into `received`.

use futures::StreamExt;
use reqwest::header::COOKIE;
use reqwest::StatusCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

#[derive(Clone, Debug)]
pub struct SseEvent {
    pub event_type: String,
    pub payload_json: Option<serde_json::Value>,
    #[allow(dead_code)]
    pub raw_data: String,
}

#[derive(Debug)]
pub struct SseOpenAttempt {
    pub status: StatusCode,
    pub body: String,
}

pub struct SseSubscription {
    #[allow(dead_code)]
    pub project_slug: String,
    pub open_status: StatusCode,
    received: Arc<Mutex<Vec<SseEvent>>>,
    arrival_times: Arc<Mutex<Vec<Instant>>>,
    heartbeats: Arc<Mutex<u32>>,
    ready: Arc<Mutex<bool>>,
    _shutdown: oneshot::Sender<()>,
}

impl SseSubscription {
    /// Wait up to `timeout` for the server's initial `:ready\n\n`
    /// marker, which signals the SSE handler has finished its
    /// auth/lookup work AND subscribed to the broadcast channel. Test
    /// drivers MUST call this before triggering event-producing
    /// actions, otherwise the NOTIFY can race the subscription and
    /// vanish.
    pub async fn wait_until_ready(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if *self.ready.lock().expect("ready") {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

impl std::fmt::Debug for SseSubscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SseSubscription")
            .field("project_slug", &self.project_slug)
            .field("open_status", &self.open_status)
            .field("events", &self.received.lock().expect("recv").len())
            .field("heartbeats", &self.heartbeat_count())
            .field("ready", &*self.ready.lock().expect("ready"))
            .finish()
    }
}

impl SseSubscription {
    /// Drain a snapshot of received events (no wait). Heartbeats are
    /// NOT included in this list — they live in `heartbeat_count`.
    pub fn drain(&self) -> Vec<SseEvent> {
        self.received.lock().expect("received mutex").clone()
    }

    pub fn heartbeat_count(&self) -> u32 {
        *self.heartbeats.lock().expect("heartbeats mutex")
    }

    /// Wait up to `timeout` for an event matching `predicate`. Polls
    /// every 25ms — the realtime budget is 1s median, so 25ms is
    /// negligible overhead.
    ///
    /// Returns `(matching_event, elapsed_since_started_at)` on match,
    /// `None` on timeout. `started_at` is the instant the When step
    /// began so latency is per-event, not per-scenario.
    pub async fn wait_for(
        &self,
        timeout: Duration,
        started_at: Instant,
        predicate: impl Fn(&SseEvent) -> bool,
    ) -> Option<(SseEvent, Duration)> {
        let deadline = Instant::now() + timeout;
        loop {
            let snapshot = {
                let events = self.received.lock().expect("received mutex");
                let times = self.arrival_times.lock().expect("arrival_times mutex");
                events
                    .iter()
                    .cloned()
                    .zip(times.iter().cloned())
                    .collect::<Vec<_>>()
            };
            for (event, arrived_at) in snapshot {
                if predicate(&event) {
                    let latency = arrived_at.saturating_duration_since(started_at);
                    return Some((event, latency));
                }
            }
            if Instant::now() >= deadline {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    /// All per-event arrival timestamps relative to `started_at`. Used
    /// by the NFR-PERF-03 median assertion.
    pub fn latencies_relative_to(&self, started_at: Instant) -> Vec<Duration> {
        self.arrival_times
            .lock()
            .expect("arrival_times mutex")
            .iter()
            .map(|t| t.saturating_duration_since(started_at))
            .collect()
    }
}

/// Open an SSE stream as the user identified by `session_cookie_header`.
/// `session_cookie_header` is the full `Cookie:` value — usually the
/// `foundry_session=...` pair produced by signing in.
///
/// Returns once the response headers are in (so `open_status` is set);
/// a background tokio task continues draining the body into the
/// in-memory vec until either the server closes the stream or the
/// returned `SseSubscription` is dropped.
pub async fn open_sse_subscription(
    base_url: &str,
    project_slug: &str,
    team_slug: &str,
    session_cookie_header: &str,
) -> SseSubscription {
    let url = format!("{base_url}/team/{team_slug}/project/{project_slug}/events");
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(false)
        .build()
        .expect("build sse client");
    let resp = client
        .get(&url)
        .header(COOKIE, session_cookie_header)
        .send()
        .await
        .expect("open sse");
    let open_status = resp.status();
    let received: Arc<Mutex<Vec<SseEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let arrival_times: Arc<Mutex<Vec<Instant>>> = Arc::new(Mutex::new(Vec::new()));
    let heartbeats: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    let ready: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    if open_status.is_success() {
        spawn_reader(
            resp,
            received.clone(),
            arrival_times.clone(),
            heartbeats.clone(),
            ready.clone(),
            shutdown_rx,
        );
    } else {
        // Drop the body silently for the failure path; the caller will
        // inspect `open_status` and call `open_sse_subscription_*` paths
        // that capture the body if needed.
    }

    SseSubscription {
        project_slug: project_slug.to_string(),
        open_status,
        received,
        arrival_times,
        heartbeats,
        ready,
        _shutdown: shutdown_tx,
    }
}

/// Variant for the @error scenarios that do NOT present a session
/// cookie. Captures the body so the assertion can sniff for a
/// sign-in prompt.
pub async fn open_sse_subscription_unauthenticated(
    base_url: &str,
    project_slug: &str,
    team_slug: &str,
) -> SseOpenAttempt {
    let url = format!("{base_url}/team/{team_slug}/project/{project_slug}/events");
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(false)
        .build()
        .expect("build unauth sse client");
    let resp = client.get(&url).send().await.expect("open sse anon");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    SseOpenAttempt { status, body }
}

fn spawn_reader(
    resp: reqwest::Response,
    received: Arc<Mutex<Vec<SseEvent>>>,
    arrival_times: Arc<Mutex<Vec<Instant>>>,
    heartbeats: Arc<Mutex<u32>>,
    ready: Arc<Mutex<bool>>,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    tokio::spawn(async move {
        let mut stream = resp.bytes_stream();
        let mut buffer: Vec<u8> = Vec::with_capacity(4096);
        let mut current_event_name: Option<String> = None;
        let mut current_data: Vec<String> = Vec::new();

        loop {
            tokio::select! {
                biased;
                _ = &mut shutdown_rx => break,
                next = stream.next() => {
                    match next {
                        Some(Ok(bytes)) => {
                            buffer.extend_from_slice(&bytes);
                            // Process any complete lines (\n terminated).
                            while let Some(nl) = buffer.iter().position(|b| *b == b'\n') {
                                let line_bytes: Vec<u8> = buffer.drain(..=nl).collect();
                                // Strip the trailing \n and an optional \r.
                                let mut line = String::from_utf8_lossy(&line_bytes).into_owned();
                                if line.ends_with('\n') {
                                    line.pop();
                                }
                                if line.ends_with('\r') {
                                    line.pop();
                                }
                                if line.is_empty() {
                                    // Dispatch the buffered event, if any.
                                    let event_name = current_event_name.take();
                                    let data = current_data.join("\n");
                                    current_data.clear();
                                    if let Some(name) = event_name {
                                        let payload_json = serde_json::from_str::<serde_json::Value>(&data).ok();
                                        let evt = SseEvent {
                                            event_type: name,
                                            payload_json,
                                            raw_data: data,
                                        };
                                        let now = Instant::now();
                                        received.lock().expect("received").push(evt);
                                        arrival_times.lock().expect("arrival_times").push(now);
                                    }
                                } else if let Some(rest) = line.strip_prefix(':') {
                                    // Comment line — used for keepalives + the
                                    // initial ":ready" handshake.
                                    let trimmed = rest.trim();
                                    if trimmed == "keepalive" {
                                        *heartbeats.lock().expect("heartbeats") += 1;
                                    } else if trimmed == "ready" {
                                        *ready.lock().expect("ready") = true;
                                    }
                                } else if let Some(rest) = line.strip_prefix("event:") {
                                    current_event_name = Some(rest.trim().to_string());
                                } else if let Some(rest) = line.strip_prefix("data:") {
                                    current_data.push(rest.trim_start().to_string());
                                }
                                // Other field names (id:, retry:) are
                                // not used by the slice-2 handler;
                                // ignore silently.
                            }
                        }
                        Some(Err(_)) => break,
                        None => break,
                    }
                }
            }
        }
    });
}
