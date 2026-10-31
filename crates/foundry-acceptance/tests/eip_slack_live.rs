//! external-integration-plugins — `eip-slack-live` (DEVOPS H-3, manual, never
//! in CI): records real Slack answers into sanitised fixtures. Run it through
//! `scripts/slack-record.sh`, which gives it a public address and the test app's
//! secrets; the secrets stay in memory here and the secrets file is deleted on
//! read.
//!
//! The run serves Foundry's return and events addresses, prints Foundry's own
//! manifest and approval link, and waits for someone to click Allow. It then
//! calls each Slack method the way Foundry does, uninstalls the app, waits for
//! the `app_uninstalled` and `tokens_revoked` events, and writes
//! `<method>/<case>.json`. It posts only to the installing person's own app
//! messages, reads the workspace's `#general` and, if there is one, an archived
//! channel, and creates nothing. It really uninstalls the app, so never point
//! it at an app anything else depends on.
//!
//! Fixtures keep Slack's keys, `ok`, `error` codes and types. Every other value is
//! replaced, so a fixture cannot carry a token, a name or an id. A final scan
//! fails the run, naming only the file, if any value it replaced is still there.

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::Router;
use foundry_acceptance::support::slack_double::sign_v0;
use foundry_app::integrations::config::SlackSettings;
use foundry_app::integrations::slack::{manifest, SlackApp, EVENTS_PATH, RETURN_PATH};
use secrecy::SecretString;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

const SLACK_API: &str = "https://slack.com/api";
const SLACK_AUTHORIZE: &str = "https://slack.com/oauth/v2/authorize";
const LISTEN_ADDRESS: &str = "0.0.0.0:8765";
const APPROVAL_WAIT: Duration = Duration::from_secs(30 * 60);
const EVENTS_WAIT: Duration = Duration::from_secs(120);
const STATE: &str = "eip-slack-live";
/// A real workspace can list thousands; a fixture keeps this many channels.
const FIXTURE_CHANNELS: usize = 3;
const NO_SUCH_CHANNEL: &str = "C0000000000";
const REDACTED: &str = "redacted";
const REDACTED_URL: &str = "https://redacted.slack.com/";
/// Keys whose values are Slack's vocabulary, not the workspace's data.
const KEPT_KEYS: [&str; 7] = [
    "ok",
    "error",
    "type",
    "token_type",
    "scope",
    "needed",
    "provided",
];
/// Replaced strings shorter than this are too common to scan for.
const SHORTEST_SCANNED: usize = 6;

/// One recorded answer, as Slack sent it.
struct Recording {
    method: &'static str,
    case: &'static str,
    answer: Value,
}

struct Listener {
    /// Every event body and whether Slack's signature on it checked out.
    events: Mutex<Vec<(Value, bool)>>,
    code: Mutex<Option<oneshot::Sender<String>>>,
    signing_secret: String,
}

#[test]
#[ignore = "manual: scripts/slack-record.sh, against a real Slack test workspace"]
fn eip_slack_live() {
    tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(record());
}

async fn record() {
    let secrets = Secrets::take();
    let public_url = required_env("EIP_SLACK_RECORD_PUBLIC_URL");
    let out = PathBuf::from(required_env("EIP_SLACK_RECORD_OUT"));

    let (code_sent, code_received) = oneshot::channel();
    let listener = Arc::new(Listener {
        events: Mutex::default(),
        code: Mutex::new(Some(code_sent)),
        signing_secret: secrets.signing_secret.clone(),
    });
    serve(listener.clone()).await;

    let redirect_uri = format!("{public_url}{RETURN_PATH}");
    let approval = secrets.app().approval_url(STATE, &redirect_uri);
    println!("SLACK-RECORD-MANIFEST-BEGIN");
    println!("{}", manifest(&public_url));
    println!("SLACK-RECORD-MANIFEST-END");
    println!("SLACK-RECORD-APPROVAL {approval}");

    let code = tokio::time::timeout(APPROVAL_WAIT, code_received)
        .await
        .expect("someone clicks Allow within 30 minutes")
        .expect("the return address hands over the code");
    println!("SLACK-RECORD-APPROVED");

    let slack = Slack::new(secrets);
    let mut recordings = slack.record_before_install().await;
    let install = slack
        .record_install(&code, &redirect_uri, &mut recordings)
        .await;
    let token = install.token;
    slack
        .record_workspace(&token, &install.installer, &mut recordings)
        .await;
    slack.record_uninstall(&token, &mut recordings).await;

    let events = await_events(&listener).await;
    write_fixtures(&out, &recordings, &events, &slack.known_secrets(&token));
    println!(
        "SLACK-RECORD-DONE answers={} events={}",
        recordings.len(),
        events.len()
    );
}

fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is set by scripts/slack-record.sh"))
}

struct Install {
    token: String,
    /// Who clicked Allow; the bot's posts go to their app messages.
    installer: String,
}

/// The test app's secrets, read from a file that is gone once read.
struct Secrets {
    client_id: String,
    client_secret: String,
    signing_secret: String,
}

impl Secrets {
    fn take() -> Self {
        let path = required_env("EIP_SLACK_RECORD_SECRETS");
        let text = std::fs::read_to_string(&path).expect("the secrets file");
        std::fs::remove_file(&path).expect("the secrets file is removed");
        let values: HashMap<&str, &str> = text.lines().filter_map(|l| l.split_once('=')).collect();
        let value = |name: &str| {
            values
                .get(name)
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| panic!("{name} is missing from the secrets file"))
                .to_string()
        };
        Self {
            client_id: value("EIP_SLACK_TEST_CLIENT_ID"),
            client_secret: value("EIP_SLACK_TEST_CLIENT_SECRET"),
            signing_secret: value("EIP_SLACK_TEST_SIGNING_SECRET"),
        }
    }

    fn app(&self) -> SlackApp {
        SlackApp::build(&SlackSettings {
            client_id: self.client_id.clone(),
            client_secret: SecretString::from(self.client_secret.clone()),
            signing_secret: SecretString::from(self.signing_secret.clone()),
            api_base_url: SLACK_API.to_string(),
            authorize_url: SLACK_AUTHORIZE.to_string(),
        })
        .expect("the Slack app's client builds")
    }
}

async fn serve(listener: Arc<Listener>) {
    let router = Router::new()
        .route(RETURN_PATH, get(on_return))
        .route(EVENTS_PATH, post(on_event))
        .with_state(listener);
    let socket = tokio::net::TcpListener::bind(LISTEN_ADDRESS)
        .await
        .expect("the recording address is free");
    tokio::spawn(async move { axum::serve(socket, router).await });
}

async fn on_return(
    State(listener): State<Arc<Listener>>,
    Query(query): Query<HashMap<String, String>>,
) -> &'static str {
    let code = query
        .get("code")
        .filter(|_| query.get("state").map(String::as_str) == Some(STATE));
    let Some(code) = code else {
        return "Slack did not approve this install. Nothing was recorded.";
    };
    let waiting = listener
        .code
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take();
    if let Some(waiting) = waiting {
        let _ = waiting.send(code.clone());
    }
    "Foundry's Slack recording has started. You can close this tab."
}

/// Slack's post to the events address: the challenge is answered, as Foundry
/// answers it, and everything is kept with whether its signature checked out.
async fn on_event(
    State(listener): State<Arc<Listener>>,
    headers: HeaderMap,
    body: Bytes,
) -> String {
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
    };
    let text = String::from_utf8_lossy(&body);
    let signed = sign_v0(
        &listener.signing_secret,
        header("x-slack-request-timestamp"),
        &text,
    ) == header("x-slack-signature");
    let envelope: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let answer = envelope["challenge"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    listener
        .events
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .push((envelope, signed));
    answer
}

struct Slack {
    http: reqwest::Client,
    secrets: Secrets,
}

impl Slack {
    fn new(secrets: Secrets) -> Self {
        Self {
            http: reqwest::Client::new(),
            secrets,
        }
    }

    fn url(method: &str) -> String {
        format!("{SLACK_API}/{method}")
    }

    async fn answer(request: reqwest::RequestBuilder) -> Value {
        let response = request.send().await.expect("Slack answers");
        response.json().await.expect("Slack answers JSON")
    }

    async fn read(&self, method: &str, token: Option<&str>, query: &[(&str, &str)]) -> Value {
        let mut request = self.http.get(Self::url(method)).query(query);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        Self::answer(request).await
    }

    async fn post_message(&self, token: Option<&str>, body: &Value) -> Value {
        let mut request = self.http.post(Self::url("chat.postMessage")).json(body);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        Self::answer(request).await
    }

    fn exchange(&self, code: &str, redirect_uri: &str) -> reqwest::RequestBuilder {
        self.http.post(Self::url("oauth.v2.access")).form(&[
            ("client_id", self.secrets.client_id.as_str()),
            ("client_secret", self.secrets.client_secret.as_str()),
            ("code", code),
            ("redirect_uri", redirect_uri),
        ])
    }

    /// The refusals that need no install.
    async fn record_before_install(&self) -> Vec<Recording> {
        let bad_token = Some("xoxb-not-a-real-token");
        let message = json!({ "channel": NO_SUCH_CHANNEL, "text": "never sent" });
        vec![
            Recording {
                method: "oauth.v2.access",
                case: "invalid_code",
                answer: Self::answer(self.exchange("not-a-code", "https://example.test/")).await,
            },
            Recording {
                method: "auth.test",
                case: "not_authed",
                answer: self.read("auth.test", None, &[]).await,
            },
            Recording {
                method: "auth.test",
                case: "invalid_auth",
                answer: self.read("auth.test", bad_token, &[]).await,
            },
            Recording {
                method: "chat.postMessage",
                case: "not_authed",
                answer: self.post_message(None, &message).await,
            },
            Recording {
                method: "chat.postMessage",
                case: "invalid_auth",
                answer: self.post_message(bad_token, &message).await,
            },
        ]
    }

    async fn record_install(
        &self,
        code: &str,
        redirect_uri: &str,
        recordings: &mut Vec<Recording>,
    ) -> Install {
        let answer = Self::answer(self.exchange(code, redirect_uri)).await;
        let token = answer["access_token"]
            .as_str()
            .unwrap_or_else(|| {
                panic!(
                    "oauth.v2.access was refused: {}",
                    answer["error"].as_str().unwrap_or("no error code")
                )
            })
            .to_string();
        let installer = answer["authed_user"]["id"]
            .as_str()
            .expect("oauth.v2.access names who installed the app")
            .to_string();
        recordings.push(Recording {
            method: "oauth.v2.access",
            case: "ok",
            answer,
        });
        Install { token, installer }
    }

    async fn record_workspace(
        &self,
        token: &str,
        installer: &str,
        recordings: &mut Vec<Recording>,
    ) {
        let token = Some(token);
        let auth = self.read("auth.test", token, &[]).await;
        recordings.push(Recording {
            method: "auth.test",
            case: "ok",
            answer: auth,
        });
        let listed = self
            .read(
                "conversations.list",
                token,
                &[
                    ("types", "public_channel,private_channel"),
                    ("exclude_archived", "false"),
                    ("limit", "200"),
                ],
            )
            .await;
        let general = channel_where(&listed, "is_general")
            .expect("every Slack workspace has a general channel");
        let archived = channel_where(&listed, "is_archived");
        recordings.push(Recording {
            method: "conversations.list",
            case: "ok",
            answer: first_channels(listed),
        });
        let mut channels = vec![
            ("ok", general.as_str()),
            ("channel_not_found", NO_SUCH_CHANNEL),
        ];
        if let Some(archived) = &archived {
            channels.push(("archived", archived));
        }
        for (case, channel) in channels {
            recordings.push(Recording {
                method: "conversations.info",
                case,
                answer: self
                    .read("conversations.info", token, &[("channel", channel)])
                    .await,
            });
        }
        let text = "Foundry is recording Slack's answers; ignore this message.";
        let mut posts = vec![
            ("ok", json!({ "channel": installer, "text": text })),
            (
                "channel_not_found",
                json!({ "channel": NO_SUCH_CHANNEL, "text": text }),
            ),
            (
                "invalid_blocks",
                json!({ "channel": installer, "text": text, "blocks": [{ "type": "nonsense" }] }),
            ),
        ];
        if let Some(archived) = &archived {
            posts.push(("is_archived", json!({ "channel": archived, "text": text })));
        }
        for (case, body) in posts {
            recordings.push(Recording {
                method: "chat.postMessage",
                case,
                answer: self.post_message(token, &body).await,
            });
        }
    }

    async fn record_uninstall(&self, token: &str, recordings: &mut Vec<Recording>) {
        let answer = Self::answer(
            self.http
                .post(Self::url("apps.uninstall"))
                .bearer_auth(token)
                .form(&[
                    ("client_id", self.secrets.client_id.as_str()),
                    ("client_secret", self.secrets.client_secret.as_str()),
                ]),
        )
        .await;
        assert_eq!(
            answer["ok"],
            json!(true),
            "apps.uninstall: {}",
            answer["error"]
        );
        recordings.push(Recording {
            method: "apps.uninstall",
            case: "ok",
            answer,
        });
        let token = Some(token);
        let message = json!({ "channel": NO_SUCH_CHANNEL, "text": "never sent" });
        recordings.push(Recording {
            method: "auth.test",
            case: "after_uninstall",
            answer: self.read("auth.test", token, &[]).await,
        });
        recordings.push(Recording {
            method: "chat.postMessage",
            case: "after_uninstall",
            answer: self.post_message(token, &message).await,
        });
    }

    fn known_secrets(&self, token: &str) -> Vec<String> {
        vec![
            token.to_string(),
            self.secrets.client_id.clone(),
            self.secrets.client_secret.clone(),
            self.secrets.signing_secret.clone(),
        ]
    }
}

/// The id of the first listed channel with `field` set.
fn channel_where(listed: &Value, field: &str) -> Option<String> {
    listed["channels"]
        .as_array()?
        .iter()
        .find(|channel| channel[field] == json!(true))?["id"]
        .as_str()
        .map(str::to_string)
}

fn first_channels(mut listed: Value) -> Value {
    if let Some(channels) = listed["channels"].as_array_mut() {
        channels.truncate(FIXTURE_CHANNELS);
    }
    listed
}

/// Waits for both events the uninstall sends, then for anything just behind
/// them; a missing event fails after `EVENTS_WAIT`.
async fn await_events(listener: &Listener) -> Vec<(Value, bool)> {
    let deadline = Instant::now() + EVENTS_WAIT;
    loop {
        let events = listener
            .events
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let seen = |kind: &str| events.iter().any(|(e, _)| e["event"]["type"] == kind);
        if seen("app_uninstalled") && seen("tokens_revoked") {
            assert!(
                events.iter().all(|(_, signed)| *signed),
                "every event carries Slack's signature, by this app's signing secret"
            );
            return events;
        }
        assert!(
            Instant::now() < deadline,
            "both uninstall events arrive; saw {:?}",
            events
                .iter()
                .map(|(e, _)| e["type"].clone())
                .collect::<Vec<_>>()
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

fn event_case(envelope: &Value) -> Option<String> {
    let kind = envelope["event"]["type"]
        .as_str()
        .or(envelope["type"].as_str())?;
    Some(kind.to_string())
}

fn write_fixtures(
    out: &Path,
    recordings: &[Recording],
    events: &[(Value, bool)],
    known_secrets: &[String],
) {
    let mut files: BTreeMap<PathBuf, Value> = BTreeMap::new();
    let mut replaced = known_secrets.iter().cloned().collect::<BTreeSet<_>>();
    for recording in recordings {
        files.insert(
            out.join(recording.method)
                .join(format!("{}.json", recording.case)),
            sanitise(&recording.answer, None, &mut replaced),
        );
    }
    for (envelope, _) in events {
        let case = event_case(envelope).expect("an event says what it is");
        files.insert(
            out.join("events").join(format!("{case}.json")),
            sanitise(envelope, None, &mut replaced),
        );
    }
    for (path, fixture) in files {
        assert_no_leak(&path, &fixture, &replaced);
        let text = serde_json::to_string_pretty(&fixture).expect("a fixture serialises") + "\n";
        std::fs::create_dir_all(path.parent().expect("a fixture has a directory"))
            .expect("the fixture directory");
        std::fs::write(&path, text).expect("the fixture is written");
    }
}

/// Keeps keys and Slack's vocabulary; every other value becomes a stand-in of
/// its type, and each replaced string joins `replaced` for the leak scan.
fn sanitise(value: &Value, key: Option<&str>, replaced: &mut BTreeSet<String>) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(name, field)| (name.clone(), sanitise(field, Some(name), replaced)))
                .collect::<Map<_, _>>(),
        ),
        Value::Array(items) => {
            Value::Array(items.iter().map(|i| sanitise(i, key, replaced)).collect())
        }
        Value::Null | Value::Bool(_) => value.clone(),
        Value::Number(_) => json!(0),
        Value::String(_) if key.is_some_and(|k| KEPT_KEYS.contains(&k)) => value.clone(),
        Value::String(text) if text.is_empty() => value.clone(),
        Value::String(text) => {
            replaced.insert(text.clone());
            json!(if key == Some("url") {
                REDACTED_URL
            } else {
                REDACTED
            })
        }
    }
}

/// Everything in a fixture `sanitise` did not write itself: its keys and the
/// values under `KEPT_KEYS`.
fn slack_strings<'a>(value: &'a Value, key: Option<&str>, out: &mut Vec<&'a str>) {
    match value {
        Value::Object(fields) => {
            for (name, field) in fields {
                out.push(name);
                slack_strings(field, Some(name), out);
            }
        }
        Value::Array(items) => items.iter().for_each(|i| slack_strings(i, key, out)),
        Value::String(text) if key.is_some_and(|k| KEPT_KEYS.contains(&k)) => out.push(text),
        _ => {}
    }
}

/// Names the file, never the value: this panic can land in a log. A key is
/// compared whole, because a value like `general` is part of `is_general`.
fn assert_no_leak(path: &Path, fixture: &Value, replaced: &BTreeSet<String>) {
    let mut strings = Vec::new();
    slack_strings(fixture, None, &mut strings);
    let leaks = strings.iter().any(|text| {
        replaced.contains(*text) && text.len() >= SHORTEST_SCANNED && !KEPT_KEYS.contains(text)
    });
    let text = fixture.to_string();
    assert!(
        !leaks && !text.contains("xox"),
        "{} still holds a replaced value",
        path.display()
    );
}

#[test]
fn a_fixture_keeps_slacks_keys_and_vocabulary_and_nothing_else() {
    let answer = json!({
        "ok": true,
        "access_token": "xoxb-1-real",
        "token_type": "bot",
        "url": "https://canzanlabs.slack.com/",
        "team": { "id": "T0REAL", "name": "Canzan Labs" },
        "channels": [{ "id": "C0REAL", "is_archived": false, "created": 1_761_055_320 }],
        "enterprise": null,
        "next_cursor": ""
    });
    let mut replaced = BTreeSet::new();
    let fixture = sanitise(&answer, None, &mut replaced);
    assert_eq!(
        fixture,
        json!({
            "ok": true,
            "access_token": "redacted",
            "token_type": "bot",
            "url": "https://redacted.slack.com/",
            "team": { "id": "redacted", "name": "redacted" },
            "channels": [{ "id": "redacted", "is_archived": false, "created": 0 }],
            "enterprise": null,
            "next_cursor": ""
        })
    );
    assert!(replaced.contains("xoxb-1-real") && replaced.contains("C0REAL"));
}

#[test]
#[should_panic(expected = "still holds a replaced value")]
fn a_fixture_holding_a_replaced_value_is_refused() {
    let replaced = BTreeSet::from(["T0123456".to_string()]);
    assert_no_leak(Path::new("f.json"), &json!({ "T0123456": 1 }), &replaced);
}

#[test]
fn a_key_that_merely_contains_a_replaced_value_is_not_a_leak() {
    let replaced = BTreeSet::from(["general".to_string()]);
    let fixture = json!({ "is_general": true, "name": "redacted" });
    assert_no_leak(Path::new("f.json"), &fixture, &replaced);
}

#[test]
#[should_panic(expected = "still holds a replaced value")]
fn a_fixture_holding_a_token_is_refused() {
    assert_no_leak(
        Path::new("f.json"),
        &json!({ "a": "xoxb-1" }),
        &BTreeSet::new(),
    );
}

#[test]
fn an_event_is_named_for_its_inner_type_and_a_handshake_for_its_own() {
    let uninstalled = json!({ "type": "event_callback", "event": { "type": "app_uninstalled" } });
    assert_eq!(event_case(&uninstalled).as_deref(), Some("app_uninstalled"));
    let handshake = json!({ "type": "url_verification", "challenge": "c" });
    assert_eq!(event_case(&handshake).as_deref(), Some("url_verification"));
}

#[test]
fn channels_are_found_by_flag_and_a_fixture_keeps_only_a_few() {
    let listed = json!({ "channels": [
        { "id": "C1", "is_general": false, "is_archived": true },
        { "id": "C2", "is_general": true, "is_archived": false },
        { "id": "C3" },
        { "id": "C4" }
    ]});
    assert_eq!(channel_where(&listed, "is_general").as_deref(), Some("C2"));
    assert_eq!(channel_where(&listed, "is_archived").as_deref(), Some("C1"));
    assert_eq!(channel_where(&listed, "is_shared"), None);
    let kept = first_channels(listed);
    assert_eq!(kept["channels"].as_array().map(Vec::len), Some(3));
}
