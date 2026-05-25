//! US-02 round-robin reverse proxy — Option A driving adapter.
//!
//! Per `distill/driver.md` §2a + `wave-decisions.md` §US-02:
//!
//! Purpose-built ~ 200 LoC axum proxy that round-robins reqwest calls
//! to N upstream `SocketAddr`s. Zero new transitive deps in the test
//! crate; full control over which upstream is "currently routed to"
//! (the SSE-landing-replica assertion in US-02 needs that visibility).
//!
//! Key behaviours:
//! - Round-robin via `AtomicUsize`; skips upstreams that have been
//!   `fail_replica`-marked or are missing.
//! - Injects an `X-Foundry-Replica` response header naming the upstream
//!   that served the request. Tests assert distribution by reading this
//!   header. Production replicas do NOT emit this header — it is a
//!   pure test affordance added by the proxy on its return path.
//! - SSE pass-through: detects `Accept: text/event-stream` and streams
//!   the response body through `axum::body::Body::from_stream` over
//!   `reqwest::Response::bytes_stream()`. Does NOT buffer.
//! - Cookies + headers pass through verbatim. The session cookie issued
//!   by replica A on /sign-in is visible to the caller and replays via
//!   the proxy to replica B for the next request — that is the
//!   "session survives replica switch" invariant.
//! - When every upstream is down, the proxy returns 502 Bad Gateway
//!   with an `upstream-unavailable` body so the "/readyz flipped to
//!   503 on every replica" scenario can observe the load-balancer
//!   removing them from rotation.
//!
//! The proxy is intentionally NOT a generic HTTP gateway — it is the
//! minimum surface the US-02 scenarios need. Production traffic goes
//! through Caddy in front of N replicas; the test analogue is this
//! module.

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderName, HeaderValue, Method, StatusCode, Uri};
use axum::response::Response;
use axum::routing::any;
use axum::Router;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// Header the proxy injects on every response naming the upstream that
/// served the request. Production replicas never set this; it is added
/// by the proxy on its return path. Tests read it to assert per-request
/// upstream distribution and to identify the SSE-landing replica.
pub const X_FOUNDRY_REPLICA: &str = "x-foundry-replica";

/// Cap on body buffering for the proxy. The US-11 attachments lane is
/// the only path that pushes payloads; the production cap is 10 MB by
/// default + 50 MB max — keep the proxy generous so a multi-replica
/// upload scenario does not break on the proxy's limit before reaching
/// the per-route limit on the replica.
const PROXY_BODY_LIMIT_BYTES: usize = 60 * 1024 * 1024;

/// Per-upstream rotation slot. `None` means the slot is currently out of
/// rotation — either `fail_replica` was called or the upstream was
/// never inserted. The proxy skips `None` slots when picking the next
/// upstream.
#[derive(Clone, Debug, Default)]
struct UpstreamSlot {
    addr: Option<SocketAddr>,
    request_count: u64,
}

/// Shared proxy state — the round-robin counter + per-slot rotation
/// gates. Wrapped in an `Arc<Mutex<...>>` so the axum service can mutate
/// counts from one task while a test thread flips fail/restore flags
/// from another.
#[derive(Debug)]
struct ProxyInner {
    /// Per-upstream slots in spawn order. Length is fixed at construction;
    /// `fail_replica(i)` flips slot i's `addr` to None without resizing.
    slots: Vec<UpstreamSlot>,
    /// Round-robin cursor. Modulo `slots.len()`. Wrapping is OK; the
    /// next-upstream loop probes forward until it finds a live slot.
    cursor: AtomicUsize,
}

/// Handle returned by [`spawn_round_robin_proxy`].
///
/// Holds the bound proxy address + a shutdown sender. Dropping the
/// handle does NOT abort the proxy task by itself — `_shutdown` is the
/// authoritative signal. The proxy's tokio task is owned by the
/// runtime; cucumber-rs scenarios that finish-and-drop will let it
/// outlive the scenario, which is fine because the tokio runtime is
/// per-test and tears down at process exit.
pub struct ProxyHandle {
    pub addr: SocketAddr,
    inner: Arc<Mutex<ProxyInner>>,
    _shutdown: oneshot::Sender<()>,
}

impl std::fmt::Debug for ProxyHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let snapshot = self.inner.lock().expect("proxy inner mutex");
        f.debug_struct("ProxyHandle")
            .field("addr", &self.addr)
            .field("slots", &snapshot.slots)
            .finish_non_exhaustive()
    }
}

impl ProxyHandle {
    /// Base URL the test reqwest client should hit.
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Per-upstream request counts observed by the proxy since spawn.
    /// Used by the "distributed across all 3 replicas" assertion.
    pub fn request_counts(&self) -> HashMap<SocketAddr, u64> {
        let inner = self.inner.lock().expect("proxy inner mutex");
        inner
            .slots
            .iter()
            .filter_map(|s| s.addr.map(|a| (a, s.request_count)))
            .collect()
    }

    /// All upstream addrs the proxy knows about (live OR failed). The
    /// returned vec mirrors `slots` order so tests can correlate
    /// `addrs()[i]` with `fail_replica(i)`.
    pub fn upstream_addrs(&self) -> Vec<Option<SocketAddr>> {
        let inner = self.inner.lock().expect("proxy inner mutex");
        inner.slots.iter().map(|s| s.addr).collect()
    }

    /// Force the upstream at slot `idx` out of rotation. The proxy will
    /// skip this slot until `restore_replica(idx, addr)` is called.
    /// Idempotent — calling on an already-failed slot is a no-op.
    pub fn fail_replica(&self, idx: usize) {
        let mut inner = self.inner.lock().expect("proxy inner mutex");
        if let Some(slot) = inner.slots.get_mut(idx) {
            slot.addr = None;
        }
    }

    /// Restore an upstream slot to rotation pointing at `addr`. Used by
    /// tests that simulate replica recovery; not exercised by the
    /// initial US-02 scenarios but kept for symmetry.
    #[allow(dead_code)]
    pub fn restore_replica(&self, idx: usize, addr: SocketAddr) {
        let mut inner = self.inner.lock().expect("proxy inner mutex");
        if let Some(slot) = inner.slots.get_mut(idx) {
            slot.addr = Some(addr);
        }
    }

    /// Drop the handle's shutdown sender so the background axum service
    /// stops on the next iteration. Explicit form for tests that want
    /// the proxy down before scenario teardown.
    #[allow(dead_code)]
    pub async fn shutdown(self) {
        let _ = self._shutdown;
    }
}

/// Spawn the proxy in front of N already-booted axum replicas. The
/// proxy binds to 127.0.0.1:0 (ephemeral port); the bound `SocketAddr`
/// is on the returned handle.
///
/// Returns once the listener is bound, so `handle.addr` and
/// `handle.base_url()` are safe to use immediately.
pub async fn spawn_round_robin_proxy(replicas: Vec<SocketAddr>) -> ProxyHandle {
    let slots = replicas
        .iter()
        .map(|a| UpstreamSlot {
            addr: Some(*a),
            request_count: 0,
        })
        .collect::<Vec<_>>();
    let inner = Arc::new(Mutex::new(ProxyInner {
        slots,
        cursor: AtomicUsize::new(0),
    }));

    // Shared reqwest client for ALL upstream forwarding. Cookie store
    // OFF — cookies are caller-attached; the proxy is invisible to the
    // cookie jar. Redirect policy NONE — the test caller sees the
    // upstream's redirect verbatim (the slice-1 dashboard issues 303s).
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(false)
        .build()
        .expect("build proxy reqwest client");

    let app_state = AppCtx {
        inner: inner.clone(),
        client,
    };

    let router = Router::new()
        .fallback(any(proxy_handler))
        .with_state(app_state)
        .layer(axum::extract::DefaultBodyLimit::max(PROXY_BODY_LIMIT_BYTES));

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind proxy listener");
    let addr = listener.local_addr().expect("proxy local_addr");
    let (tx, rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = rx.await;
            })
            .await
            .ok();
    });
    ProxyHandle {
        addr,
        inner,
        _shutdown: tx,
    }
}

#[derive(Clone)]
struct AppCtx {
    inner: Arc<Mutex<ProxyInner>>,
    client: reqwest::Client,
}

/// Pick the next live upstream via round-robin, increment its counter,
/// and return its addr. Returns `None` when every slot is `None`.
fn next_upstream(inner: &Arc<Mutex<ProxyInner>>) -> Option<SocketAddr> {
    let mut guard = inner.lock().expect("proxy inner mutex");
    let n = guard.slots.len();
    if n == 0 {
        return None;
    }
    // Probe up to N slots starting from cursor. Each iteration bumps the
    // cursor so the NEXT call to next_upstream starts after the one we
    // returned (i.e. true round-robin even when some slots are down).
    for _ in 0..n {
        let i = guard.cursor.fetch_add(1, Ordering::SeqCst) % n;
        if let Some(addr) = guard.slots[i].addr {
            guard.slots[i].request_count += 1;
            return Some(addr);
        }
    }
    None
}

/// Catchall handler — forwards the incoming request to the next live
/// upstream, copies status/headers/body back, and injects the
/// `X-Foundry-Replica` response header.
async fn proxy_handler(State(ctx): State<AppCtx>, req: Request) -> Response {
    let Some(upstream) = next_upstream(&ctx.inner) else {
        return upstream_unavailable_response();
    };

    let (parts, body) = req.into_parts();
    let method = parts.method.clone();
    let uri = parts.uri.clone();
    let headers = parts.headers.clone();

    // Drain the request body in one shot. PROXY_BODY_LIMIT_BYTES caps
    // it earlier in the axum layer, so this is safe.
    let body_bytes = match axum::body::to_bytes(body, PROXY_BODY_LIMIT_BYTES).await {
        Ok(b) => b,
        Err(err) => {
            return Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .header(X_FOUNDRY_REPLICA, format_addr(upstream))
                .body(Body::from(format!(
                    "proxy: failed to read request body: {err}"
                )))
                .expect("build proxy body-read failure response");
        }
    };

    let url = build_upstream_url(upstream, &uri);
    let rb = ctx
        .client
        .request(reqwest_method(&method), url)
        .headers(reqwest_headers(&headers))
        .body(body_bytes);
    let upstream_resp = match rb.send().await {
        Ok(r) => r,
        Err(_err) => {
            return Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .header(X_FOUNDRY_REPLICA, format_addr(upstream))
                .body(Body::from("upstream-unavailable"))
                .expect("build upstream-failure response");
        }
    };

    // Detect SSE: if the upstream replied with text/event-stream OR the
    // client asked for it via Accept, stream the body. Otherwise buffer.
    let is_sse = upstream_resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.starts_with("text/event-stream"))
        .unwrap_or(false);

    let status = upstream_resp.status();
    let upstream_headers = upstream_resp.headers().clone();
    let mut builder = Response::builder().status(status.as_u16());
    // Re-emit upstream headers verbatim. Hop-by-hop headers (`Connection`,
    // `Transfer-Encoding`, etc.) are filtered by reqwest itself; the rest
    // — including Set-Cookie + Cache-Control — must round-trip.
    for (name, value) in upstream_headers.iter() {
        builder = builder.header(name.as_str(), value.as_bytes());
    }
    // X-Foundry-Replica goes last so it always wins; one and only one
    // value per response.
    builder = builder.header(X_FOUNDRY_REPLICA, format_addr(upstream));

    let body = if is_sse {
        // Pass through as a stream. axum will flush each chunk as it
        // arrives — the SSE client reads `:ready`, then events, then
        // `:keepalive` heartbeats. Buffering would defeat the whole
        // point.
        use futures::stream::StreamExt;
        let stream = upstream_resp
            .bytes_stream()
            .map(|chunk| chunk.map_err(std::io::Error::other));
        Body::from_stream(stream)
    } else {
        match upstream_resp.bytes().await {
            Ok(b) => Body::from(b),
            Err(_) => Body::from("proxy: failed to read upstream body"),
        }
    };

    builder.body(body).expect("build proxy forwarded response")
}

fn upstream_unavailable_response() -> Response {
    Response::builder()
        .status(StatusCode::BAD_GATEWAY)
        .header(X_FOUNDRY_REPLICA, "none")
        .body(Body::from("upstream-unavailable"))
        .expect("build all-down response")
}

fn build_upstream_url(addr: SocketAddr, uri: &Uri) -> String {
    let path_and_query = uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");
    format!("http://{addr}{path_and_query}")
}

fn format_addr(addr: SocketAddr) -> String {
    addr.to_string()
}

fn reqwest_method(m: &Method) -> reqwest::Method {
    // axum's http::Method and reqwest's reqwest::Method are both
    // re-exports of http::Method in recent versions; the byte-equal
    // round-trip is cheap and avoids a feature-flag bind.
    reqwest::Method::from_bytes(m.as_str().as_bytes())
        .expect("axum method round-trips to reqwest method")
}

fn reqwest_headers(headers: &axum::http::HeaderMap) -> reqwest::header::HeaderMap {
    let mut out = reqwest::header::HeaderMap::with_capacity(headers.len());
    for (name, value) in headers.iter() {
        // Skip Host — reqwest sets it from the upstream URL.
        // Skip the per-hop headers reqwest already strips for us.
        if name == axum::http::header::HOST {
            continue;
        }
        let h_name = match HeaderName::from_bytes(name.as_str().as_bytes()) {
            Ok(n) => n,
            Err(_) => continue,
        };
        let h_value = match HeaderValue::from_bytes(value.as_bytes()) {
            Ok(v) => v,
            Err(_) => continue,
        };
        // reqwest's HeaderName/HeaderValue come from the same http crate
        // so name/value bytes are wire-compatible.
        out.insert(
            reqwest::header::HeaderName::from_bytes(h_name.as_str().as_bytes())
                .expect("header name round-trip"),
            reqwest::header::HeaderValue::from_bytes(h_value.as_bytes())
                .expect("header value round-trip"),
        );
    }
    out
}
