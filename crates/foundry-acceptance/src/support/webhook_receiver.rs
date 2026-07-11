//! In-process local HTTP receiver double for the `webhook` notification channel.
//!
//! Extension Justification (quality-framework Mandate against Parallel
//! Implementations):
//!   WHY-NEW-FILE: crates/foundry-acceptance/src/support/webhook_receiver.rs
//!   CLOSEST-EXISTING: crates/foundry-acceptance/src/support/notify_recorder.rs
//!     (the other notification-port test double — the in-memory delivery recorder)
//!   EXTENSION-COST: notify_recorder.rs is a pure in-memory `Mutex<Vec<..>>`
//!     recorder with a `NotificationProvider` impl; folding a live axum HTTP
//!     server (its own bind + spawned serve task + `SocketAddr` lifecycle) into it
//!     would mix an in-process value recorder with an out-of-process I/O listener.
//!   PARALLEL-RATIONALE: this double is a REAL HTTP endpoint the production
//!     `WebhookProvider` POSTs to over reqwest (a network round-trip with a
//!     different lifecycle — a bound port + a background serve task), whereas the
//!     recorder is a synchronous in-memory sink; the webhook receiver must observe
//!     the wire (headers + body) the real adapter emits, which the recorder cannot.
//!
//! Per the DISTILL harness boundary: external transports are IN-PROCESS TEST
//! DOUBLES — no real POST leaves the test process. This receiver is a local axum
//! server bound on `127.0.0.1:0`; the shipped `WebhookProvider` is pointed at its
//! URL, so a delivery is a genuine reqwest POST to this endpoint (`@real-io`). It
//! records every POST's headers + body so a step def can assert the JSON payload
//! and the HMAC signature header — and the POST count stays 0 across a
//! reachability probe (N-ODD-3: `probe()` makes no POST).

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::Router;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

/// One POST observed at the webhook endpoint boundary.
#[derive(Debug, Clone)]
pub struct RecordedPost {
    /// Lowercased header name → value (so a step can read `x-foundry-signature`).
    pub headers: HashMap<String, String>,
    /// The raw request body (the rendered JSON notification payload).
    pub body: String,
}

/// A local HTTP receiver the shipped `WebhookProvider` POSTs to. Records every
/// POST; a bare TCP reachability probe hits no route, so `post_count()` stays 0.
pub struct WebhookReceiver {
    addr: SocketAddr,
    posts: Mutex<Vec<RecordedPost>>,
}

impl WebhookReceiver {
    /// Bind on an ephemeral `127.0.0.1` port and start serving the `/hook` route.
    pub async fn spawn() -> Arc<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind webhook receiver listener");
        let addr = listener.local_addr().expect("webhook receiver local addr");
        let receiver = Arc::new(Self {
            addr,
            posts: Mutex::new(Vec::new()),
        });
        let app = Router::new()
            .route("/hook", post(handle_post))
            .with_state(receiver.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        receiver
    }

    /// The `http://127.0.0.1:<port>/hook` URL the provider is configured with.
    pub fn url(&self) -> String {
        format!("http://{}/hook", self.addr)
    }

    /// Number of POSTs observed so far (0 across a no-POST probe).
    pub fn post_count(&self) -> usize {
        self.posts
            .lock()
            .expect("webhook receiver posts mutex")
            .len()
    }

    /// The most recent POST observed, if any.
    pub fn last_post(&self) -> Option<RecordedPost> {
        self.posts
            .lock()
            .expect("webhook receiver posts mutex")
            .last()
            .cloned()
    }

    fn record(&self, post: RecordedPost) {
        self.posts
            .lock()
            .expect("webhook receiver posts mutex")
            .push(post);
    }
}

async fn handle_post(
    State(receiver): State<Arc<WebhookReceiver>>,
    headers: HeaderMap,
    body: String,
) -> StatusCode {
    let mut recorded = HashMap::new();
    for (name, value) in headers.iter() {
        if let Ok(value) = value.to_str() {
            recorded.insert(name.as_str().to_ascii_lowercase(), value.to_string());
        }
    }
    receiver.record(RecordedPost {
        headers: recorded,
        body,
    });
    StatusCode::OK
}
