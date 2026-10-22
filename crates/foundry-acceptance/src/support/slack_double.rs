//! external-integration-plugins — the Slack HTTP double (DEVOPS-2: a
//! recorded-fixture Rust double, not Pact; DESIGN SC-10, ADR-EIP-005).
//!
//! Extension Justification (quality-framework Mandate against Parallel
//! Implementations):
//!   WHY-NEW-FILE: crates/foundry-acceptance/src/support/slack_double.rs
//!   CLOSEST-EXISTING: support/webhook_receiver.rs (a local POST recorder) and
//!     support/oidc_issuer.rs (an OAuth double with a scripted authorize step).
//!   EXTENSION-COST: neither speaks Slack's Web API (six methods, `ok: false`
//!     codes, `Retry-After`), its OAuth v2 approval screen, or its signed Events
//!     API; folding them into either would mix three vendors in one module.
//!   PARALLEL-RATIONALE: Slack is its own external system with its own fault
//!     script; the double is reached only through `SLACK_API_BASE_URL` and
//!     `SLACK_AUTHORIZE_URL` (test seams, never set in a cluster).
//!
//! What the double is, per method (DESIGN [REF] External Integrations Requiring
//! Contract Tests): `GET /oauth/v2/authorize` (Slack's approval screen: it
//! redirects to the caller's `redirect_uri` with a code, or with
//! `error=access_denied` when the scenario cancels), and, under `/slack-web/`
//! (never `/api/`: the shipped check-arch rule `api-openapi-operations`
//! reserves every `/api` route for foundry-api), `oauth.v2.access`,
//! `auth.test`, `conversations.list`, `conversations.info`, `chat.postMessage`,
//! `chat.delete`, `apps.uninstall`. It records every call, every
//! post (channel, the JSON body, when) and every code exchanged.
//!
//! Answers are Slack's documented shapes; DEVOPS H-3 / `eip-slack-live` refresh
//! them against a real test workspace and DELIVER records them under
//! `tests/fixtures/slack/<method>/<case>.json` (the double then serves the
//! recorded bodies; the shapes below are the contract until then).
//!
//! Faults are scripted per scenario ([`Mood`]): no answer (the connection is
//! dropped on accept and any pooled request hangs), a held post (answered only
//! when released), an HTML 503, 429 with `Retry-After`, and `ok: false` codes.
//! The connection-level codes (`token_revoked`, `account_inactive`,
//! `invalid_auth`) are what every call gets once the app is "removed".
//!
//! The double also plays Slack's Events API: [`SlackDouble::post_event`] signs
//! an envelope with the test signing secret (`v0` HMAC-SHA256 over
//! `v0:{timestamp}:{body}`), exactly as Slack does.

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;

/// The Foundry Slack app's settings the instance is given (sentinels: a scan for
/// any of them in a page, a header or a log line is a leak, EIP-KPI-5).
pub const CLIENT_ID: &str = "4100000000.eipclientid";
pub const CLIENT_SECRET: &str = "eip-sentinel-client-secret-7f3a9c";
pub const SIGNING_SECRET: &str = "eip-sentinel-signing-secret-2b8e41";
/// The app id Slack's event envelopes carry for the Foundry app.
pub const APP_ID: &str = "A0EIPFOUNDRY";

/// A Slack workspace the scenarios can approve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackTeam {
    pub id: String,
    pub name: String,
    pub domain: String,
}

impl SlackTeam {
    /// The two Slack workspaces DISCUSS names (US-EIP-01, US-EIP-06 example 5).
    pub fn named(name: &str) -> Self {
        match name {
            "Canzan Labs" => Self {
                id: "T0CANZANLABS".into(),
                name: "Canzan Labs".into(),
                domain: "canzanlabs".into(),
            },
            "Canzan Personal" => Self {
                id: "T0CANZANPERS".into(),
                name: "Canzan Personal".into(),
                domain: "canzanpersonal".into(),
            },
            other => panic!("the Slack double knows the workspaces Canzan Labs and Canzan Personal, not {other:?}"),
        }
    }
}

/// What the approval screen does next.
#[derive(Debug, Clone)]
pub enum Approval {
    Approve(SlackTeam),
    Cancel,
}

/// How Slack answers a post (one per call, then [`Mood::Answers`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Mood {
    Answers,
    /// No answer at all: new connections are dropped, pooled requests hang.
    Silent,
    /// The request is held until [`SlackDouble::release`].
    Held,
    /// An HTML error page with status 503 (a proxy in front of Slack).
    ErrorPage,
    /// HTTP 429 with `Retry-After: n`.
    SlowDown(u64),
    /// `{"ok": false, "error": code}` with optional extra vendor text.
    Refuses {
        code: String,
        text: Option<String>,
    },
}

/// One `chat.postMessage` Slack received.
#[derive(Debug, Clone)]
pub struct PostedMessage {
    pub channel: String,
    pub body: Value,
    pub at: Instant,
    /// Whether Slack accepted it (`ok: true`).
    pub accepted: bool,
}

impl PostedMessage {
    /// Every text the message renders, in order (top-level `text`, then each
    /// block's text and each element's text), for the reading oracles.
    pub fn texts(&self) -> Vec<String> {
        let mut out = Vec::new();
        collect_texts(&self.body, &mut out);
        out
    }

    /// The URLs of the message's buttons ("Open in Foundry").
    pub fn button_urls(&self) -> Vec<String> {
        let mut out = Vec::new();
        collect_urls(&self.body, &mut out);
        out
    }

    /// The whole message as one searchable string (the never-sent oracle).
    pub fn all_text(&self) -> String {
        self.body.to_string()
    }
}

fn collect_texts(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            for (k, val) in map {
                if k == "text" {
                    match val {
                        Value::String(s) => out.push(s.clone()),
                        Value::Object(o) => {
                            if let Some(Value::String(s)) = o.get("text") {
                                out.push(s.clone());
                            }
                        }
                        _ => {}
                    }
                } else {
                    collect_texts(val, out);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|i| collect_texts(i, out)),
        _ => {}
    }
}

fn collect_urls(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            for (k, val) in map {
                if k == "url" {
                    if let Value::String(s) = val {
                        out.push(s.clone());
                    }
                } else {
                    collect_urls(val, out);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|i| collect_urls(i, out)),
        _ => {}
    }
}

/// The never-pings oracle (D-3, DDR-12): would Slack notify anyone for this
/// message? True when a `mrkdwn` text (or a top-level `text` without
/// `parse: "none"`) carries a live control sequence, or when `link_names` is on.
pub fn notifies_anyone(message: &PostedMessage) -> bool {
    let body = &message.body;
    if body.get("link_names").and_then(Value::as_bool) == Some(true) {
        return true;
    }
    let live = |s: &str| {
        ["<!channel", "<!here", "<!everyone", "<!subteam", "<@", "<#"]
            .iter()
            .any(|seq| s.contains(seq))
    };
    let parse_none = body.get("parse").and_then(Value::as_str) == Some("none");
    if let Some(top) = body.get("text").and_then(Value::as_str) {
        if !parse_none && live(top) {
            return true;
        }
    }
    let mut mrkdwn = Vec::new();
    collect_mrkdwn(body, &mut mrkdwn);
    mrkdwn.iter().any(|s| live(s))
}

fn collect_mrkdwn(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("mrkdwn") {
                if let Some(Value::String(s)) = map.get("text") {
                    out.push(s.clone());
                }
            }
            map.values().for_each(|val| collect_mrkdwn(val, out));
        }
        Value::Array(items) => items.iter().for_each(|i| collect_mrkdwn(i, out)),
        _ => {}
    }
}

/// A channel the Slack workspace has.
#[derive(Debug, Clone)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub private: bool,
    pub shared: bool,
    pub archived: bool,
    /// Whether the Foundry app is a member (private channels need an invite).
    pub foundry_in: bool,
}

#[derive(Debug, Default)]
struct Script {
    approval: Option<Approval>,
    /// Per channel id: the queued moods of the next posts there.
    post_moods: HashMap<String, VecDeque<Mood>>,
    /// The mood of every post (and every call) until changed.
    standing: Option<Mood>,
    /// The app was removed: every call answers this connection-level code.
    removed: Option<String>,
}

#[derive(Debug, Default)]
struct Ledger {
    calls: Vec<(String, Instant)>,
    posts: Vec<PostedMessage>,
    /// Codes handed out by the approval screen → the team approved.
    issued: HashMap<String, SlackTeam>,
    exchanged: Vec<String>,
    /// Bot token → team.
    tokens: BTreeMap<String, SlackTeam>,
    uninstalls: usize,
}

/// The Slack double: one per scenario, bound on `127.0.0.1:0`.
pub struct SlackDouble {
    addr: SocketAddr,
    channels: Mutex<Vec<Channel>>,
    script: Mutex<Script>,
    ledger: Mutex<Ledger>,
    silent: Arc<AtomicBool>,
    release: Notify,
}

impl std::fmt::Debug for SlackDouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlackDouble")
            .field("addr", &self.addr)
            .finish_non_exhaustive()
    }
}

/// Drops new connections while the double is silent (Slack "does not answer").
struct SilenceableListener {
    inner: TcpListener,
    silent: Arc<AtomicBool>,
}

impl axum::serve::Listener for SilenceableListener {
    type Io = TcpStream;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match self.inner.accept().await {
                Ok((stream, addr)) => {
                    if self.silent.load(Ordering::SeqCst) {
                        drop(stream);
                        continue;
                    }
                    return (stream, addr);
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(5)).await,
            }
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.inner.local_addr()
    }
}

impl SlackDouble {
    /// Bind and serve. The Slack workspace "Canzan Labs" has the channels the
    /// scenarios name: #auth-dev, #auth-releases, #general, #ops-alerts (public),
    /// #ext-auth-vendor (public, shared with another organisation) and
    /// #backend-private (private; the app is not in it until invited).
    pub async fn spawn() -> Arc<Self> {
        announce_answer_source();
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind the Slack double");
        let addr = listener.local_addr().expect("the Slack double's address");
        let silent = Arc::new(AtomicBool::new(false));
        let channel = |id: &str, name: &str, private: bool, shared: bool| Channel {
            id: id.into(),
            name: name.into(),
            private,
            shared,
            archived: false,
            foundry_in: !private,
        };
        let double = Arc::new(Self {
            addr,
            channels: Mutex::new(vec![
                channel("C0AUTHDEV", "auth-dev", false, false),
                channel("C0AUTHREL", "auth-releases", false, false),
                channel("C0GENERAL", "general", false, false),
                channel("C0OPSALRT", "ops-alerts", false, false),
                channel("C0EXTVEND", "ext-auth-vendor", false, true),
                channel("G0BACKPRV", "backend-private", true, false),
            ]),
            script: Mutex::new(Script::default()),
            ledger: Mutex::new(Ledger::default()),
            silent: silent.clone(),
            release: Notify::new(),
        });
        let app = Router::new()
            .route("/oauth/v2/authorize", get(authorize))
            .route("/slack-web/oauth.v2.access", post(oauth_access))
            .route("/slack-web/auth.test", post(auth_test).get(auth_test))
            .route(
                "/slack-web/conversations.list",
                post(conversations_list).get(conversations_list),
            )
            .route(
                "/slack-web/conversations.info",
                post(conversations_info).get(conversations_info),
            )
            .route("/slack-web/chat.postMessage", post(post_message))
            .route(
                "/slack-web/apps.uninstall",
                post(apps_uninstall).get(apps_uninstall),
            )
            .route("/slack-web/chat.delete", post(chat_delete))
            .with_state(double.clone());
        tokio::spawn(async move {
            let _ = axum::serve(
                SilenceableListener {
                    inner: listener,
                    silent,
                },
                app,
            )
            .await;
        });
        double
    }

    /// `SLACK_API_BASE_URL` (plain http, DEVOPS U-D4).
    pub fn api_base_url(&self) -> String {
        format!("http://{}/slack-web", self.addr)
    }

    /// `SLACK_AUTHORIZE_URL`.
    pub fn authorize_url(&self) -> String {
        format!("http://{}/oauth/v2/authorize", self.addr)
    }

    pub fn address(&self) -> SocketAddr {
        self.addr
    }

    // ----------------------------------------------------------- the script

    pub fn next_approval(&self, approval: Approval) {
        self.script.lock().unwrap().approval = Some(approval);
    }

    /// Every post (and call) from now on gets this mood; `Answers` heals.
    pub fn set_standing(&self, mood: Mood) {
        self.silent.store(mood == Mood::Silent, Ordering::SeqCst);
        self.script.lock().unwrap().standing = Some(mood);
        self.release.notify_waiters();
    }

    /// Slack answers again: heal every standing fault and release held posts.
    pub fn heal(&self) {
        self.set_standing(Mood::Answers);
    }

    /// The next post to `channel` gets `mood` (once).
    pub fn queue_post_mood(&self, channel: &str, mood: Mood) {
        let id = self.channel_id(channel);
        self.script
            .lock()
            .unwrap()
            .post_moods
            .entry(id)
            .or_default()
            .push_back(mood);
    }

    /// The Foundry app was removed from the Slack workspace: every call refuses.
    pub fn remove_app(&self, code: &str) {
        self.script.lock().unwrap().removed = Some(code.to_string());
    }

    pub fn release(&self) {
        self.release.notify_waiters();
    }

    pub fn archive(&self, channel: &str) {
        self.with_channel(channel, |c| c.archived = true);
    }

    pub fn remove_foundry_from(&self, channel: &str) {
        self.with_channel(channel, |c| c.foundry_in = false);
    }

    pub fn invite_foundry_to(&self, channel: &str) {
        self.with_channel(channel, |c| c.foundry_in = true);
    }

    fn with_channel(&self, channel: &str, f: impl FnOnce(&mut Channel)) {
        let name = channel.trim_start_matches('#');
        let mut channels = self.channels.lock().unwrap();
        let c = channels
            .iter_mut()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("the Slack double has no channel #{name}"));
        f(c);
    }

    pub fn channel_id(&self, channel: &str) -> String {
        let name = channel.trim_start_matches('#');
        self.channels
            .lock()
            .unwrap()
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.id.clone())
            .unwrap_or_else(|| panic!("the Slack double has no channel #{name}"))
    }

    // ----------------------------------------------------------- the ledger

    /// Every post Slack received for `channel`, accepted or not.
    pub fn posts_to(&self, channel: &str) -> Vec<PostedMessage> {
        let id = self.channel_id(channel);
        self.ledger
            .lock()
            .unwrap()
            .posts
            .iter()
            .filter(|p| {
                p.channel == id
                    || p.channel == channel
                    || p.channel == channel.trim_start_matches('#')
            })
            .cloned()
            .collect()
    }

    /// The messages the channel shows (posts Slack accepted), in order.
    pub fn shown_in(&self, channel: &str) -> Vec<PostedMessage> {
        self.posts_to(channel)
            .into_iter()
            .filter(|p| p.accepted)
            .collect()
    }

    pub fn all_posts(&self) -> Vec<PostedMessage> {
        self.ledger.lock().unwrap().posts.clone()
    }

    pub fn calls(&self) -> Vec<String> {
        self.ledger
            .lock()
            .unwrap()
            .calls
            .iter()
            .map(|(m, _)| m.clone())
            .collect()
    }

    pub fn exchanged_codes(&self) -> Vec<String> {
        self.ledger.lock().unwrap().exchanged.clone()
    }

    pub fn uninstall_count(&self) -> usize {
        self.ledger.lock().unwrap().uninstalls
    }

    /// Every bot token the double has issued (sentinels for the leak scans).
    pub fn issued_tokens(&self) -> Vec<String> {
        self.ledger.lock().unwrap().tokens.keys().cloned().collect()
    }

    fn record_call(&self, method: &str) {
        self.ledger
            .lock()
            .unwrap()
            .calls
            .push((method.to_string(), Instant::now()));
    }

    // ----------------------------------------------------------- Events API

    /// Post a signed Events API envelope to Foundry's events address. `skew`
    /// moves the signature's timestamp into the past; `secret` is the signing
    /// secret used (`None` = unsigned).
    pub async fn post_event(
        &self,
        foundry_base: &str,
        path: &str,
        envelope: &Value,
        secret: Option<&str>,
        skew: Duration,
    ) -> (StatusCode, String) {
        let body = envelope.to_string();
        let ts = (std::time::SystemTime::now() - skew)
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time after the epoch")
            .as_secs()
            .to_string();
        let mut request = reqwest::Client::new()
            .post(format!("{foundry_base}{path}"))
            .header("content-type", "application/json")
            .body(body.clone());
        if let Some(secret) = secret {
            request = request
                .header("x-slack-request-timestamp", &ts)
                .header("x-slack-signature", sign_v0(secret, &ts, &body));
        }
        let resp = request
            .send()
            .await
            .expect("POST Foundry's Slack events address");
        let status = StatusCode::from_u16(resp.status().as_u16()).expect("a status");
        (status, resp.text().await.unwrap_or_default())
    }
}

/// Slack's request signature: `v0=` + hex(HMAC-SHA256(secret, "v0:{ts}:{body}")).
pub fn sign_v0(secret: &str, ts: &str, body: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("any key length");
    mac.update(format!("v0:{ts}:{body}").as_bytes());
    let digest = mac.finalize().into_bytes();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("v0={hex}")
}

/// The envelopes the scenarios send.
pub fn uninstall_envelope(team: &SlackTeam, app_id: &str) -> Value {
    json!({
        "token": "deprecated-verification-token",
        "team_id": team.id,
        "api_app_id": app_id,
        "type": "event_callback",
        "event_id": format!("Ev{}", uuid::Uuid::new_v4().simple()),
        "event_time": 1_761_055_320,
        "event": { "type": "app_uninstalled" }
    })
}

pub fn url_verification_envelope(challenge: &str) -> Value {
    json!({ "token": "deprecated-verification-token", "challenge": challenge, "type": "url_verification" })
}

// ------------------------------------------------------------------ handlers

async fn authorize(
    State(double): State<Arc<SlackDouble>>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    double.record_call("oauth.v2.authorize");
    let redirect = query.get("redirect_uri").cloned().unwrap_or_else(|| {
        panic!("Foundry sent Slack's approval screen no redirect_uri: {query:?}")
    });
    let state = query.get("state").cloned().unwrap_or_default();
    let approval = double
        .script
        .lock()
        .unwrap()
        .approval
        .take()
        .unwrap_or(Approval::Approve(SlackTeam::named("Canzan Labs")));
    let sep = if redirect.contains('?') { '&' } else { '?' };
    match approval {
        Approval::Cancel => Redirect::to(&format!(
            "{redirect}{sep}error=access_denied&state={}",
            urlencoding::encode(&state)
        ))
        .into_response(),
        Approval::Approve(team) => {
            let code = format!("eip-code-{}", uuid::Uuid::new_v4().simple());
            double
                .ledger
                .lock()
                .unwrap()
                .issued
                .insert(code.clone(), team);
            Redirect::to(&format!(
                "{redirect}{sep}code={code}&state={}",
                urlencoding::encode(&state)
            ))
            .into_response()
        }
    }
}

fn form_or_query(body: &str) -> HashMap<String, String> {
    url_pairs(body)
}

fn url_pairs(raw: &str) -> HashMap<String, String> {
    raw.split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| {
            (
                urlencoding::decode(k)
                    .map(|c| c.into_owned())
                    .unwrap_or_default(),
                urlencoding::decode(&v.replace('+', " "))
                    .map(|c| c.into_owned())
                    .unwrap_or_default(),
            )
        })
        .collect()
}

fn ok(v: Value) -> Response {
    (StatusCode::OK, axum::Json(v)).into_response()
}

fn refuse(code: &str) -> Response {
    ok(json!({ "ok": false, "error": code }))
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_string)
}

async fn oauth_access(State(double): State<Arc<SlackDouble>>, body: String) -> Response {
    double.record_call("oauth.v2.access");
    let form = form_or_query(&body);
    let code = form.get("code").cloned().unwrap_or_default();
    double.ledger.lock().unwrap().exchanged.push(code.clone());
    let issued = double.ledger.lock().unwrap().issued.remove(&code);
    let Some(team) = issued else {
        return refuse("invalid_code");
    };
    if form.get("client_secret").map(String::as_str) != Some(CLIENT_SECRET) {
        return refuse("invalid_client");
    }
    let token = format!("xoxb-eip-sentinel-{}", uuid::Uuid::new_v4().simple());
    double
        .ledger
        .lock()
        .unwrap()
        .tokens
        .insert(token.clone(), team.clone());
    ok(json!({
        "ok": true,
        "access_token": token,
        "token_type": "bot",
        "scope": "chat:write,chat:write.public,channels:read,groups:read",
        "bot_user_id": "U0FOUNDRYBOT",
        "app_id": APP_ID,
        "team": { "id": team.id, "name": team.name },
        "enterprise": null,
        "is_enterprise_install": false
    }))
}

fn removed_code(double: &SlackDouble) -> Option<String> {
    double.script.lock().unwrap().removed.clone()
}

async fn auth_test(State(double): State<Arc<SlackDouble>>, headers: HeaderMap) -> Response {
    double.record_call("auth.test");
    if let Some(code) = removed_code(&double) {
        return refuse(&code);
    }
    let token = bearer(&headers).unwrap_or_default();
    let team = double.ledger.lock().unwrap().tokens.get(&token).cloned();
    let Some(team) = team else {
        return refuse("invalid_auth");
    };
    ok(json!({
        "ok": true,
        "url": format!("https://{}.slack.com/", team.domain),
        "team": team.name,
        "user": "foundry",
        "team_id": team.id,
        "user_id": "U0FOUNDRYBOT",
        "bot_id": "B0FOUNDRYBOT",
        "is_enterprise_install": false
    }))
}

/// Slack's answer to a call without the bot token it issued: `not_authed`
/// when none is given, `invalid_auth` when it is not one Slack issued.
fn unauthorised(double: &SlackDouble, headers: &HeaderMap) -> Option<Response> {
    match bearer(headers) {
        None => Some(refuse("not_authed")),
        Some(token) if !double.ledger.lock().unwrap().tokens.contains_key(&token) => {
            Some(refuse("invalid_auth"))
        }
        Some(_) => None,
    }
}

async fn conversations_list(
    State(double): State<Arc<SlackDouble>>,
    headers: HeaderMap,
) -> Response {
    double.record_call("conversations.list");
    if let Some(code) = removed_code(&double) {
        return refuse(&code);
    }
    if let Some(refusal) = unauthorised(&double, &headers) {
        return refusal;
    }
    let channels: Vec<Value> = double
        .channels
        .lock()
        .unwrap()
        .iter()
        .filter(|c| !c.private || c.foundry_in)
        .filter(|c| !c.archived)
        .map(channel_json)
        .collect();
    ok(json!({ "ok": true, "channels": channels, "response_metadata": { "next_cursor": "" } }))
}

fn channel_json(c: &Channel) -> Value {
    json!({
        "id": c.id,
        "name": c.name,
        "is_channel": !c.private,
        "is_group": c.private,
        "is_private": c.private,
        "is_archived": c.archived,
        "is_ext_shared": c.shared,
        "is_shared": c.shared,
        "is_member": c.foundry_in
    })
}

async fn conversations_info(
    State(double): State<Arc<SlackDouble>>,
    Query(query): Query<HashMap<String, String>>,
    body: String,
) -> Response {
    double.record_call("conversations.info");
    if let Some(code) = removed_code(&double) {
        return refuse(&code);
    }
    let mut args = form_or_query(&body);
    args.extend(query);
    let id = args.get("channel").cloned().unwrap_or_default();
    let channel = double
        .channels
        .lock()
        .unwrap()
        .iter()
        .find(|c| c.id == id)
        .cloned();
    match channel {
        Some(c) if !c.private || c.foundry_in => {
            ok(json!({ "ok": true, "channel": channel_json(&c) }))
        }
        _ => refuse("channel_not_found"),
    }
}

async fn post_message(
    State(double): State<Arc<SlackDouble>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    double.record_call("chat.postMessage");
    let json: Value = serde_json::from_str(&body).unwrap_or_else(|_| {
        // Form-encoded posts are recorded too (the reader is the same).
        json!(form_or_query(&body))
    });
    let channel = json
        .get("channel")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let mood = {
        let mut script = double.script.lock().unwrap();
        if let Some(code) = script.removed.clone() {
            Mood::Refuses { code, text: None }
        } else if let Some(m) = script
            .post_moods
            .get_mut(&channel)
            .and_then(VecDeque::pop_front)
        {
            m
        } else {
            script.standing.clone().unwrap_or(Mood::Answers)
        }
    };
    let record = |accepted: bool| {
        double.ledger.lock().unwrap().posts.push(PostedMessage {
            channel: channel.clone(),
            body: json.clone(),
            at: Instant::now(),
            accepted,
        });
    };
    match mood {
        Mood::Silent => {
            record(false);
            std::future::pending::<Response>().await
        }
        Mood::Held => {
            record(false);
            double.release.notified().await;
            ok(json!({ "ok": true, "channel": channel, "ts": "1761055320.000100" }))
        }
        Mood::ErrorPage => {
            record(false);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                [("content-type", "text/html")],
                "<html><body><h1>503 Service Temporarily Unavailable</h1></body></html>",
            )
                .into_response()
        }
        Mood::SlowDown(secs) => {
            record(false);
            (
                StatusCode::TOO_MANY_REQUESTS,
                [("retry-after", secs.to_string())],
                axum::Json(json!({ "ok": false, "error": "ratelimited" })),
            )
                .into_response()
        }
        Mood::Refuses { code, text } => {
            record(false);
            let mut v = json!({ "ok": false, "error": code });
            if let Some(text) = text {
                v["warning"] = json!(text);
                v["response_metadata"] = json!({ "messages": [format!("[ERROR] {text}")] });
            }
            ok(v)
        }
        Mood::Answers => {
            if let Some(refusal) = unauthorised(&double, &headers) {
                record(false);
                return refusal;
            }
            let state = double
                .channels
                .lock()
                .unwrap()
                .iter()
                .find(|c| c.id == channel || c.name == channel.trim_start_matches('#'))
                .cloned();
            match state {
                None => {
                    record(false);
                    refuse("channel_not_found")
                }
                Some(c) if c.archived => {
                    record(false);
                    refuse("is_archived")
                }
                Some(c) if !c.foundry_in => {
                    record(false);
                    refuse("not_in_channel")
                }
                Some(_) => {
                    record(true);
                    ok(json!({ "ok": true, "channel": channel, "ts": "1761055320.000100" }))
                }
            }
        }
    }
}

async fn apps_uninstall(State(double): State<Arc<SlackDouble>>) -> Response {
    double.record_call("apps.uninstall");
    double.ledger.lock().unwrap().uninstalls += 1;
    ok(json!({ "ok": true }))
}

async fn chat_delete(State(double): State<Arc<SlackDouble>>) -> Response {
    double.record_call("chat.delete");
    ok(json!({ "ok": true }))
}

/// Whether recorded Slack answers exist for every method the double serves
/// and for the Events API envelopes (DEVOPS H-3): one non-empty directory each
/// under `tests/fixtures/slack/`.
pub fn answers_are_recorded() -> bool {
    let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/slack"));
    REQUIRED_FIXTURES.iter().all(|dir| {
        std::fs::read_dir(root.join(dir))
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    })
}

/// The fixture directories slice 01 needs before it is un-pended.
pub const REQUIRED_FIXTURES: &[&str] = &[
    "oauth.v2.access",
    "auth.test",
    "conversations.list",
    "conversations.info",
    "chat.postMessage",
    "apps.uninstall",
    "events",
];

/// The fixture gate: without recorded answers the double REFUSES to start, so
/// an un-pended slice-01 run fails instead of passing on hand-written shapes.
/// Two explicit opt-ins serve the hand-written shapes instead, each announced
/// once per run on stderr:
///
/// - `FOUNDRY_EIP_DISTILL_CLASSIFICATION=1`: DISTILL's RED classification;
/// - `FOUNDRY_EIP_HANDWRITTEN_SLACK=1`: DELIVER before the recordings exist
///   (user decision 2026-10-22: the fixture gate moved to release time). The
///   shapes follow Slack's documented `oauth.v2.access` and `auth.test`
///   answers and are diffed against the recordings (H-3) before release.
pub fn announce_answer_source() {
    if answers_are_recorded() {
        return;
    }
    let opted_in = |name: &str| std::env::var(name).as_deref() == Ok("1");
    let handwritten = opted_in("FOUNDRY_EIP_HANDWRITTEN_SLACK");
    assert!(
        handwritten || opted_in("FOUNDRY_EIP_DISTILL_CLASSIFICATION"),
        "slack_double: no recorded Slack fixtures under tests/fixtures/slack ({REQUIRED_FIXTURES:?}); record them (DEVOPS H-3), or set FOUNDRY_EIP_HANDWRITTEN_SLACK=1 to run on the hand-written shapes before release"
    );
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if handwritten {
            eprintln!("slack_double: Slack double serving HAND-WRITTEN shapes; recorded fixtures (H-3) required before release");
        } else {
            eprintln!("slack_double: serving HAND-WRITTEN Slack answer shapes for DISTILL classification (DEVOPS H-3)");
        }
    });
}
