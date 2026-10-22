//! external-integration-plugins — the Matrix homeserver for `@needs-matrix`
//! (DEVOPS-3, DESIGN SC-10, ADR-EIP-006).
//!
//! Extension Justification (quality-framework Mandate against Parallel
//! Implementations):
//!   WHY-NEW-FILE: crates/foundry-acceptance/src/support/matrix_homeserver.rs
//!   CLOSEST-EXISTING: support/browser_harness.rs (a pinned container shared
//!     per run, port read back from Docker) and support/harness.rs (the shared
//!     Postgres container).
//!   EXTENSION-COST: neither speaks the Matrix client-server API; folding a
//!     homeserver into the browser harness would couple two unrelated lanes.
//!   PARALLEL-RATIONALE: the pinned homeserver IS the Matrix contract (DESIGN
//!     External Integrations); its lifecycle (one per run, SQLite, raised rate
//!     limits) is its own.
//!
//! One container per run (the `PG_CONTAINER` OnceCell pattern), image pinned by
//! tag (`matrixdotorg/synapse:v1.163.0`; DISTILL pins `tag@digest` after the
//! first pull, DEVOPS U-D10), server name `canzan.dev`, SQLite storage, open
//! dummy registration and every `rc_*` limit raised so concurrent scenarios do
//! not 429 each other. Readiness is a bounded poll of
//! `GET /_matrix/client/versions`, never a fixed sleep. The container carries
//! the label `foundry-acceptance=eip` so an interrupted run's leftover can be
//! removed with `docker rm -f $(docker ps -aq --filter label=foundry-acceptance=eip)`.
//!
//! Every scenario registers its own users and rooms with a unique suffix
//! ([`Names`]); the scenario's names (@foundry:canzan.dev, #auth:canzan.dev)
//! are what the driver maps them back to when it reads Foundry's pages.
//!
//! Foundry reaches the homeserver as `https://matrix.canzan.dev` through the
//! in-process fault proxy ([`FaultProxy`]) and the test-support outbound seam
//! DESIGN does not have yet (DEVOPS U-D1, C-7, C-13). SCAFFOLD: true — the
//! proxy forwards TCP and injects faults today; its TLS termination (the
//! committed test CA and the leaf for matrix.canzan.dev) lands with the U-D1
//! seam in DELIVER step 04-01, because nothing can use it before the seam.

use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use testcontainers::core::IntoContainerPort;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tokio::net::{TcpListener, TcpStream};

/// DEVOPS-3 pin. Digest read from the Hub API on 2026-10-21 (not pulled):
/// sha256:e98ab72138d90fa8b8876c6421fe78a9fdb181c0d7cc690ad740e5173e0c65e6.
pub const SYNAPSE_IMAGE: &str = "matrixdotorg/synapse";
/// Pinned by tag AND digest (DEVOPS U-D10; the digest was confirmed by the
/// first pull, 2026-10-21, and matches the Hub API value DEVOPS recorded).
pub const SYNAPSE_TAG: &str =
    "v1.163.0@sha256:e98ab72138d90fa8b8876c6421fe78a9fdb181c0d7cc690ad740e5173e0c65e6";
pub const SERVER_NAME: &str = "canzan.dev";
/// The address Foundry is given for the homeserver (US-EIP-08).
pub const FOUNDRY_ADDRESS: &str = "https://matrix.canzan.dev";

/// The homeserver's generated config tail: open dummy registration and raised
/// limits (appended to the generated `homeserver.yaml`).
const CONFIG_TAIL: &str = r#"
enable_registration: true
enable_registration_without_verification: true
rc_message: {per_second: 1000, burst_count: 1000}
rc_registration: {per_second: 1000, burst_count: 1000}
rc_login: {address: {per_second: 1000, burst_count: 1000}, account: {per_second: 1000, burst_count: 1000}, failed_attempts: {per_second: 1000, burst_count: 1000}}
rc_joins: {local: {per_second: 1000, burst_count: 1000}, remote: {per_second: 1000, burst_count: 1000}}
rc_invites: {per_room: {per_second: 1000, burst_count: 1000}, per_user: {per_second: 1000, burst_count: 1000}}
rc_admin_redaction: {per_second: 1000, burst_count: 1000}
suppress_key_server_warning: true
trusted_key_servers: []
"#;

/// Keep-alive slot (the `harness::PG_CONTAINER` pattern): `Mutex<Option<_>>` so
/// [`shutdown_synapse`] can take it and remove the container while the runtime
/// is alive (testcontainers-rs 0.23 cannot do it from `Drop`).
static SYNAPSE_CONTAINER: std::sync::Mutex<Option<ContainerAsync<GenericImage>>> =
    std::sync::Mutex::new(None);
static SYNAPSE_INIT: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();

/// The run's homeserver base URL (`http://127.0.0.1:<port>`), started once.
pub async fn ensure_synapse() -> String {
    SYNAPSE_INIT
        .get_or_init(|| async {
            // The image's own entry point: `generate` writes a homeserver.yaml
            // that listens on every interface (a bare --generate-config binds
            // localhost only, unreachable through Docker's port mapping), then
            // the raised limits are appended and the server starts.
            let script = "mkdir -p /data && /start.py generate && printf '%s' \"$EIP_CONFIG_TAIL\" >> /data/homeserver.yaml && exec /start.py".to_string();
            let container = GenericImage::new(SYNAPSE_IMAGE, SYNAPSE_TAG)
                .with_exposed_port(8008.tcp())
                .with_entrypoint("sh")
                .with_cmd(vec!["-c".to_string(), script])
                .with_env_var("EIP_CONFIG_TAIL", CONFIG_TAIL)
                .with_env_var("SYNAPSE_SERVER_NAME", SERVER_NAME)
                .with_env_var("SYNAPSE_REPORT_STATS", "no")
                .with_label("foundry-acceptance", "eip")
                .start()
                .await
                .expect("start the pinned Synapse homeserver (@needs-matrix needs Docker)");
            let port = container
                .get_host_port_ipv4(8008.tcp())
                .await
                .expect("the homeserver's published port");
            let url = format!("http://127.0.0.1:{port}");
            *SYNAPSE_CONTAINER.lock().expect("synapse slot") = Some(container);
            wait_ready(&url).await;
            url
        })
        .await
        .clone()
}

/// Remove the run's homeserver (called from `tests/acceptance.rs` at the end
/// of `main`, beside `harness::shutdown_postgres`). A no-op when no
/// `@needs-matrix` scenario ran.
pub async fn shutdown_synapse() {
    let container = SYNAPSE_CONTAINER.lock().expect("synapse slot").take();
    if let Some(container) = container {
        if let Err(e) = container.rm().await {
            eprintln!("warning: failed to remove the Synapse container: {e}");
        }
    }
}

async fn wait_ready(url: &str) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .expect("a client");
    for _ in 0..300 {
        if let Ok(resp) = client
            .get(format!("{url}/_matrix/client/versions"))
            .send()
            .await
        {
            if resp.status().is_success() {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!("the homeserver at {url} never answered /_matrix/client/versions within 60 s");
}

/// The scenario's names and the homeserver's real ones (unique per scenario).
#[derive(Debug, Clone, Default)]
pub struct Names {
    pub suffix: String,
    /// `(scenario name, real name)`, longest first when mapped back.
    pub map: Vec<(String, String)>,
}

impl Names {
    pub fn new() -> Self {
        Self {
            suffix: uuid::Uuid::new_v4().simple().to_string()[..8].to_string(),
            map: Vec::new(),
        }
    }

    /// `@foundry:canzan.dev` → `@foundry-<sfx>:canzan.dev` (and remember it).
    pub fn real(&mut self, scenario: &str) -> String {
        if let Some((_, r)) = self.map.iter().find(|(s, _)| s == scenario) {
            return r.clone();
        }
        let (sigil, rest) = scenario.split_at(1);
        let (local, server) = rest
            .split_once(':')
            .expect("a Matrix name has a server part");
        let real = format!("{sigil}{local}-{}:{server}", self.suffix);
        self.map.push((scenario.to_string(), real.clone()));
        real
    }

    /// Foundry's page text with every real name replaced by the scenario's.
    pub fn as_scenario(&self, text: &str) -> String {
        let mut out = text.to_string();
        for (s, r) in &self.map {
            out = out.replace(r, s);
        }
        out
    }
}

/// A signed-in Matrix account.
#[derive(Debug, Clone)]
pub struct Account {
    pub user_id: String,
    pub access_token: String,
}

/// The client-server calls the scenarios' preconditions and observations need.
#[derive(Debug, Clone)]
pub struct Homeserver {
    pub base: String,
    http: reqwest::Client,
}

impl Homeserver {
    pub async fn connect() -> Self {
        Self {
            base: ensure_synapse().await,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .expect("a client"),
        }
    }

    async fn call(
        &self,
        method: reqwest::Method,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> Value {
        let mut req = self.http.request(method, format!("{}{path}", self.base));
        if let Some(token) = token {
            req = req.bearer_auth(token);
        }
        if let Some(body) = body {
            req = req.json(&body);
        }
        let resp = req
            .send()
            .await
            .unwrap_or_else(|e| panic!("homeserver {path}: {e}"));
        let status = resp.status();
        let value: Value = resp.json().await.unwrap_or(Value::Null);
        assert!(
            status.is_success(),
            "precondition on the homeserver: {path} answered {status}: {value}"
        );
        value
    }

    /// Register a user with dummy auth (open registration on this test server).
    pub async fn register(&self, localpart: &str) -> Account {
        let v = self
            .call(
                reqwest::Method::POST,
                "/_matrix/client/v3/register",
                None,
                Some(json!({ "username": localpart, "password": "eip-matrix-password", "auth": { "type": "m.login.dummy" } })),
            )
            .await;
        Account {
            user_id: v["user_id"].as_str().expect("user_id").to_string(),
            access_token: v["access_token"]
                .as_str()
                .expect("access_token")
                .to_string(),
        }
    }

    /// A fresh access token for an existing user (a new device login).
    pub async fn login(&self, user_id: &str) -> String {
        let v = self
            .call(
                reqwest::Method::POST,
                "/_matrix/client/v3/login",
                None,
                Some(json!({ "type": "m.login.password", "identifier": { "type": "m.id.user", "user": user_id }, "password": "eip-matrix-password" })),
            )
            .await;
        v["access_token"]
            .as_str()
            .expect("access_token")
            .to_string()
    }

    pub async fn logout(&self, token: &str) {
        self.call(
            reqwest::Method::POST,
            "/_matrix/client/v3/logout",
            Some(token),
            Some(json!({})),
        )
        .await;
    }

    /// Create a room with an alias local part, a name and, optionally, encryption.
    pub async fn create_room(
        &self,
        owner: &Account,
        alias_local: &str,
        name: &str,
        encrypted: bool,
    ) -> String {
        let mut initial_state = Vec::new();
        if encrypted {
            initial_state.push(json!({ "type": "m.room.encryption", "state_key": "", "content": { "algorithm": "m.megolm.v1.aes-sha2" } }));
        }
        let v = self
            .call(
                reqwest::Method::POST,
                "/_matrix/client/v3/createRoom",
                Some(&owner.access_token),
                Some(json!({ "room_alias_name": alias_local, "name": name, "preset": "private_chat", "initial_state": initial_state })),
            )
            .await;
        v["room_id"].as_str().expect("room_id").to_string()
    }

    pub async fn invite(&self, by: &Account, room_id: &str, user_id: &str) {
        self.call(
            reqwest::Method::POST,
            &format!(
                "/_matrix/client/v3/rooms/{}/invite",
                urlencoding::encode(room_id)
            ),
            Some(&by.access_token),
            Some(json!({ "user_id": user_id })),
        )
        .await;
    }

    pub async fn join(&self, who: &Account, room_id: &str) {
        self.call(
            reqwest::Method::POST,
            &format!("/_matrix/client/v3/join/{}", urlencoding::encode(room_id)),
            Some(&who.access_token),
            Some(json!({})),
        )
        .await;
    }

    pub async fn kick(&self, by: &Account, room_id: &str, user_id: &str) {
        self.call(
            reqwest::Method::POST,
            &format!(
                "/_matrix/client/v3/rooms/{}/kick",
                urlencoding::encode(room_id)
            ),
            Some(&by.access_token),
            Some(json!({ "user_id": user_id, "reason": "removed by a room moderator" })),
        )
        .await;
    }

    pub async fn enable_encryption(&self, by: &Account, room_id: &str) {
        self.call(
            reqwest::Method::PUT,
            &format!(
                "/_matrix/client/v3/rooms/{}/state/m.room.encryption/",
                urlencoding::encode(room_id)
            ),
            Some(&by.access_token),
            Some(json!({ "algorithm": "m.megolm.v1.aes-sha2" })),
        )
        .await;
    }

    pub async fn joined_members(&self, by: &Account, room_id: &str) -> Vec<String> {
        let v = self
            .call(
                reqwest::Method::GET,
                &format!(
                    "/_matrix/client/v3/rooms/{}/joined_members",
                    urlencoding::encode(room_id)
                ),
                Some(&by.access_token),
                None,
            )
            .await;
        v["joined"]
            .as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// The room's message events, oldest first, as `reader` sees them.
    pub async fn messages(&self, reader: &Account, room_id: &str) -> Vec<Value> {
        let v = self
            .call(
                reqwest::Method::GET,
                &format!(
                    "/_matrix/client/v3/rooms/{}/messages?dir=b&limit=100",
                    urlencoding::encode(room_id)
                ),
                Some(&reader.access_token),
                None,
            )
            .await;
        let mut events: Vec<Value> = v["chunk"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e["type"] == "m.room.message")
            .collect();
        events.reverse();
        events
    }
}

/// Would this event notify the room or a person (D-3, DDR-13)? True when it
/// lacks an empty `m.mentions`, when its plain body carries a literal `@room`,
/// or when its formatted body carries a user pill (`matrix.to/#/@`).
pub fn notifies(event: &Value) -> bool {
    let content = &event["content"];
    let mentions_empty = content
        .get("m.mentions")
        .and_then(Value::as_object)
        .map(|m| {
            m.get("room") != Some(&Value::Bool(true))
                && m.get("user_ids")
                    .is_none_or(|u| u.as_array().is_none_or(Vec::is_empty))
        })
        .unwrap_or(false);
    let body = content["body"].as_str().unwrap_or_default();
    let formatted = content["formatted_body"].as_str().unwrap_or_default();
    !mentions_empty || body.contains("@room") || formatted.contains("matrix.to/#/@")
}

/// The fault proxy between Foundry and the homeserver (DEVOPS-3). An HTTP
/// reverse proxy: every request is forwarded on its own to the homeserver, so
/// an armed fault answers the next matching request on ANY connection (pooled
/// or new) and records that it was given. SCAFFOLD: true — TLS termination
/// with the committed test CA wraps it when the U-D1 seam lands (DELIVER 04-01).
#[derive(Debug)]
pub struct FaultProxy {
    pub addr: SocketAddr,
    state: Arc<ProxyState>,
}

#[derive(Debug, Default)]
struct ProxyState {
    upstream: String,
    silent: Arc<std::sync::atomic::AtomicBool>,
    armed: std::sync::Mutex<Option<(&'static str, Fault)>>,
    consumed: std::sync::atomic::AtomicBool,
    connections: std::sync::atomic::AtomicUsize,
    requests: std::sync::Mutex<Vec<(String, String)>>,
}

/// A canned answer the proxy gives instead of the homeserver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// 429 `M_LIMIT_EXCEEDED` with `retry_after_ms: 30000` (Foundry's own
    /// backoff starts at 10 s, so the two are told apart).
    SlowDown,
    /// 502 with an HTML body (a proxy in front of the homeserver).
    ServerError,
}

/// Forward, or refuse every connection (the homeserver "does not answer").
pub const PROXY_FORWARD: u8 = 0;
pub const PROXY_SILENT: u8 = 1;

/// The client-server path of a message send.
pub const SEND_PATH: &str = "/send/m.room.message/";

struct CountingListener {
    inner: TcpListener,
    state: Arc<ProxyState>,
}

impl axum::serve::Listener for CountingListener {
    type Io = TcpStream;
    type Addr = SocketAddr;
    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            if let Ok((stream, addr)) = self.inner.accept().await {
                self.state.connections.fetch_add(1, Ordering::SeqCst);
                if self.state.silent.load(Ordering::SeqCst) {
                    drop(stream);
                    continue;
                }
                return (stream, addr);
            }
        }
    }
    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.inner.local_addr()
    }
}

async fn forward(
    axum::extract::State(state): axum::extract::State<Arc<ProxyState>>,
    request: axum::extract::Request,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let method = request.method().clone();
    let path = request
        .uri()
        .path_and_query()
        .map(|p| p.to_string())
        .unwrap_or_default();
    state
        .requests
        .lock()
        .expect("requests")
        .push((method.to_string(), path.clone()));
    if state.silent.load(Ordering::SeqCst) {
        return std::future::pending::<axum::response::Response>().await;
    }
    let armed = {
        let mut slot = state.armed.lock().expect("armed slot");
        match *slot {
            Some((needle, fault)) if path.contains(needle) => {
                *slot = None;
                Some(fault)
            }
            _ => None,
        }
    };
    if let Some(fault) = armed {
        state.consumed.store(true, Ordering::SeqCst);
        return match fault {
            Fault::SlowDown => (
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                axum::Json(json!({ "errcode": "M_LIMIT_EXCEEDED", "error": "Too many requests", "retry_after_ms": 30000 })),
            )
                .into_response(),
            Fault::ServerError => (
                axum::http::StatusCode::BAD_GATEWAY,
                [("content-type", "text/html")],
                "<html>502 Bad Gateway</html>",
            )
                .into_response(),
        };
    }
    let headers = request.headers().clone();
    let body = axum::body::to_bytes(request.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap_or_default();
    let mut upstream =
        reqwest::Client::new().request(method, format!("http://{}{path}", state.upstream));
    for (k, v) in headers.iter() {
        if k != "host" {
            upstream = upstream.header(k, v);
        }
    }
    match upstream.body(body).send().await {
        Ok(resp) => {
            let status = axum::http::StatusCode::from_u16(resp.status().as_u16())
                .unwrap_or(axum::http::StatusCode::BAD_GATEWAY);
            let ct = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/json")
                .to_string();
            let bytes = resp.bytes().await.unwrap_or_default();
            (status, [("content-type", ct)], bytes).into_response()
        }
        Err(_) => axum::http::StatusCode::BAD_GATEWAY.into_response(),
    }
}

impl FaultProxy {
    pub async fn spawn(upstream: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind the Matrix fault proxy");
        let addr = listener.local_addr().expect("the proxy's address");
        let state = Arc::new(ProxyState {
            upstream: upstream.trim_start_matches("http://").to_string(),
            ..ProxyState::default()
        });
        let app = axum::Router::new()
            .fallback(forward)
            .with_state(state.clone());
        let l = CountingListener {
            inner: listener,
            state: state.clone(),
        };
        tokio::spawn(async move {
            let _ = axum::serve(l, app).await;
        });
        Self { addr, state }
    }

    pub fn set_mode(&self, mode: u8) {
        self.state
            .silent
            .store(mode == PROXY_SILENT, Ordering::SeqCst);
    }

    /// The next request whose path contains `needle` gets `fault`.
    pub fn arm(&self, needle: &'static str, fault: Fault) {
        self.state.consumed.store(false, Ordering::SeqCst);
        *self.state.armed.lock().expect("armed slot") = Some((needle, fault));
    }

    /// Whether the armed fault was given (fail-closed check for the scenario).
    pub fn fault_given(&self) -> bool {
        self.state.consumed.load(Ordering::SeqCst)
    }

    pub fn connections(&self) -> usize {
        self.state.connections.load(Ordering::SeqCst)
    }

    /// Requests whose path contains `needle`.
    pub fn requests_to(&self, needle: &str) -> usize {
        self.state
            .requests
            .lock()
            .expect("requests")
            .iter()
            .filter(|(_, p)| p.contains(needle))
            .count()
    }
}

/// A listener on this machine (an address the judge refuses), bound on an
/// ephemeral port for one scenario; it counts every connection and stops when
/// dropped with the World.
#[derive(Debug)]
pub struct Decoy {
    pub port: u16,
    connections: Arc<std::sync::atomic::AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Decoy {
    pub async fn bind() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind the decoy listener on this machine");
        let port = listener.local_addr().expect("the decoy's port").port();
        let connections = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let c = connections.clone();
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                c.fetch_add(1, Ordering::SeqCst);
                drop(stream);
            }
        });
        Self {
            port,
            connections,
            task,
        }
    }

    pub fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }
}

impl Drop for Decoy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// The pinned mail relay for `@needs-smtp` (DEVOPS-4), by tag and digest
/// (confirmed by the first pull, 2026-10-21).
pub const MAILPIT_IMAGE: &str = "axllent/mailpit";
pub const MAILPIT_TAG: &str =
    "v1.31.6@sha256:23fe39e81bde596a918000b3135c0f85dc8bb4c9ea6be274b051143f3e300387";

static MAILPIT_CONTAINERS: std::sync::Mutex<Vec<ContainerAsync<GenericImage>>> =
    std::sync::Mutex::new(Vec::new());

/// Keep a scenario's relay until the end of the run (the Synapse pattern):
/// removed by [`shutdown_mailpit`] while the runtime is alive.
pub fn keep_mailpit(container: ContainerAsync<GenericImage>) {
    MAILPIT_CONTAINERS
        .lock()
        .expect("mailpit slot")
        .push(container);
}

/// Remove every relay this run started (beside `shutdown_synapse`).
pub async fn shutdown_mailpit() {
    let containers: Vec<_> = std::mem::take(&mut *MAILPIT_CONTAINERS.lock().expect("mailpit slot"));
    for c in containers {
        if let Err(e) = c.rm().await {
            eprintln!("warning: failed to remove a mailpit container: {e}");
        }
    }
}
