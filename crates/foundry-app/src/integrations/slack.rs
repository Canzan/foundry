//! The Slack adapter (ADR-EIP-005 decisions 1-4): the install flow (the
//! approval address, the session-held install state, `oauth.v2.access`,
//! `auth.test`, the app manifest the setup page renders, DD-9), and
//! [`SlackBot`], one workspace's bot posting with `chat.postMessage` and
//! listing channels with `conversations.list` / `conversations.info`.
//!
//! It also reads what Slack posts to the events address (ADR-EIP-005
//! decision 6): [`SlackApp::read_event`] believes a post only when Slack
//! signed it with this app's signing secret (`v0` HMAC-SHA256 over
//! `v0:{timestamp}:{body}`, compared in constant time) within five minutes.
//!
//! The client follows no redirect and keeps the bot token inside a
//! [`SecretString`]; no error carries Slack's own text or a credential.

use super::config::SlackSettings;
use async_trait::async_trait;
use base64::Engine;
use foundry_integrations::{
    classify_slack, ChannelKind, ChannelOutcome, DeliveryChannel, DeliveryKey, Destination,
    DestinationCatalog, DestinationCheck, DestinationRef, EncodedMessage, ReasonCode, SlackAnswer,
};
use hmac::{Hmac, Mac};
use rand::RngCore;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::Sha256;
use std::time::Duration;
use subtle::ConstantTimeEq;
use time::OffsetDateTime;

/// The least bot permissions slices 01-03 need (ADR-EIP-005 decision 3).
pub const BOT_SCOPES: [&str; 4] = [
    "chat:write",
    "chat:write.public",
    "channels:read",
    "groups:read",
];
/// The events Foundry subscribes to (ADR-EIP-005 decision 6).
pub const BOT_EVENTS: [&str; 2] = ["app_uninstalled", "tokens_revoked"];
pub const RETURN_PATH: &str = "/workspace/integrations/slack/return";
pub const EVENTS_PATH: &str = "/integrations/slack/events";
/// A return older than this is refused (US-EIP-01, K-3).
pub const INSTALL_STATE_TTL: time::Duration = time::Duration::minutes(10);
const CALL_TIMEOUT: Duration = Duration::from_secs(10);
/// A signed post whose timestamp is further than this from now is refused
/// (Slack's replay guard).
pub const EVENT_MAX_AGE_SECONDS: i64 = 5 * 60;

/// What the install start keeps in the signed-in session, taken out (single
/// use) by the return (ADR-EIP-005 decision 4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallState {
    state: String,
    workspace_id: uuid::Uuid,
    issued_at: i64,
}

impl InstallState {
    pub const SESSION_KEY: &'static str = "eip_slack_install";

    /// 32 random bytes, base64url, for the session's active workspace.
    pub fn issue(workspace_id: uuid::Uuid, now: OffsetDateTime) -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self {
            state: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes),
            workspace_id,
            issued_at: now.unix_timestamp(),
        }
    }

    pub fn state(&self) -> &str {
        &self.state
    }

    /// The returned `state` is this one (constant time), for the same
    /// workspace, and under ten minutes old.
    pub fn accepts(&self, returned: &str, workspace_id: uuid::Uuid, now: OffsetDateTime) -> bool {
        let same: bool = self.state.as_bytes().ct_eq(returned.as_bytes()).into();
        let age = now.unix_timestamp() - self.issued_at;
        same && self.workspace_id == workspace_id
            && (0..INSTALL_STATE_TTL.whole_seconds()).contains(&age)
    }
}

/// What `oauth.v2.access` granted. The token never leaves this value but to
/// be sealed.
pub struct SlackGrant {
    pub access_token: SecretString,
    pub team_id: String,
    pub team_name: String,
    pub app_id: String,
    pub bot_user_id: String,
}

/// Why a Slack call did not give what the flow needs. Carries no vendor text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SlackError {
    #[error("Slack could not be reached")]
    Unreachable,
    #[error("Slack refused the call")]
    Refused,
    #[error("Slack's answer could not be read")]
    Unreadable,
}

/// The Foundry Slack app's install client.
pub struct SlackApp {
    http: reqwest::Client,
    client_id: String,
    client_secret: SecretString,
    signing_secret: SecretString,
    api_base_url: String,
    authorize_url: String,
}

impl SlackApp {
    pub fn new(settings: &SlackSettings) -> Self {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(CALL_TIMEOUT)
            .build()
            .unwrap_or_default();
        Self {
            http,
            client_id: settings.client_id.clone(),
            client_secret: settings.client_secret.clone(),
            signing_secret: settings.signing_secret.clone(),
            api_base_url: settings.api_base_url.clone(),
            authorize_url: settings.authorize_url.clone(),
        }
    }

    /// Slack's approval screen for this app, returning to `redirect_uri`.
    pub fn approval_url(&self, state: &str, redirect_uri: &str) -> String {
        let sep = if self.authorize_url.contains('?') {
            '&'
        } else {
            '?'
        };
        format!(
            "{}{sep}client_id={}&scope={}&redirect_uri={}&state={}",
            self.authorize_url,
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&BOT_SCOPES.join(",")),
            urlencoding::encode(redirect_uri),
            urlencoding::encode(state)
        )
    }

    /// `oauth.v2.access`: the returned code becomes the bot token.
    pub async fn exchange(&self, code: &str, redirect_uri: &str) -> Result<SlackGrant, SlackError> {
        let answer = self
            .call(self.http.post(self.method("oauth.v2.access")).form(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.expose_secret()),
                ("code", code),
                ("redirect_uri", redirect_uri),
            ]))
            .await?;
        let text = |v: &Value| v.as_str().map(str::to_string).ok_or(SlackError::Unreadable);
        Ok(SlackGrant {
            access_token: SecretString::from(text(&answer["access_token"])?),
            team_id: text(&answer["team"]["id"])?,
            team_name: text(&answer["team"]["name"])?,
            app_id: text(&answer["app_id"])?,
            bot_user_id: text(&answer["bot_user_id"])?,
        })
    }

    /// `auth.test`: the Slack workspace's domain (`canzanlabs.slack.com`).
    pub async fn domain(&self, token: &SecretString) -> Result<String, SlackError> {
        let answer = self
            .call(
                self.http
                    .post(self.method("auth.test"))
                    .bearer_auth(token.expose_secret()),
            )
            .await?;
        answer["url"]
            .as_str()
            .and_then(domain_of)
            .ok_or(SlackError::Unreadable)
    }

    /// `apps.uninstall`: Slack withdraws the Foundry app's access to the
    /// Slack workspace `token` belongs to (US-EIP-07, disconnect).
    pub async fn uninstall(&self, token: &SecretString) -> Result<(), SlackError> {
        self.call(
            self.http
                .post(self.method("apps.uninstall"))
                .bearer_auth(token.expose_secret())
                .form(&[
                    ("client_id", self.client_id.as_str()),
                    ("client_secret", self.client_secret.expose_secret()),
                ]),
        )
        .await
        .map(|_| ())
    }

    /// What a post to the events address says, believed only when Slack
    /// signed `body` with this app's signing secret at `timestamp`, no more
    /// than five minutes from `now`.
    pub fn read_event(
        &self,
        timestamp: Option<&str>,
        signature: Option<&str>,
        body: &[u8],
        now: OffsetDateTime,
    ) -> SlackEvent {
        match (timestamp, signature) {
            (Some(timestamp), Some(signature)) if self.signed(timestamp, signature, body, now) => {
                SlackEvent::read(body)
            }
            _ => SlackEvent::Unsigned,
        }
    }

    fn signed(&self, timestamp: &str, signature: &str, body: &[u8], now: OffsetDateTime) -> bool {
        let Ok(at) = timestamp.parse::<i64>() else {
            return false;
        };
        if (now.unix_timestamp() - at).abs() > EVENT_MAX_AGE_SECONDS {
            return false;
        }
        let Ok(mut mac) =
            Hmac::<Sha256>::new_from_slice(self.signing_secret.expose_secret().as_bytes())
        else {
            return false;
        };
        mac.update(format!("v0:{timestamp}:").as_bytes());
        mac.update(body);
        let hex: String = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("v0={hex}")
            .as_bytes()
            .ct_eq(signature.as_bytes())
            .into()
    }

    fn method(&self, name: &str) -> String {
        format!("{}/{name}", self.api_base_url)
    }

    /// One call; Slack's `ok` field decides, never the status alone.
    async fn call(&self, request: reqwest::RequestBuilder) -> Result<Value, SlackError> {
        let response = request.send().await.map_err(|_| SlackError::Unreachable)?;
        let body = response.text().await.map_err(|_| SlackError::Unreachable)?;
        let answer: Value = serde_json::from_str(&body).map_err(|_| SlackError::Unreadable)?;
        match answer["ok"].as_bool() {
            Some(true) => Ok(answer),
            Some(false) => Err(SlackError::Refused),
            None => Err(SlackError::Unreadable),
        }
    }
}

/// What a post to the events address is (Slack's Events API envelope).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlackEvent {
    /// Not signed by Slack with this app's secret within five minutes.
    Unsigned,
    /// Signed, but not an envelope Foundry can read.
    Unreadable,
    /// Slack confirming the events address: answer with its challenge.
    UrlVerification { challenge: String },
    /// The Slack app `app_id` was removed from the Slack workspace
    /// `team_id`, or its bot token there was revoked.
    AppLost {
        team_id: String,
        app_id: String,
        reason: ReasonCode,
    },
    /// Any other event: acknowledged, nothing done.
    Other,
}

impl SlackEvent {
    /// A signed envelope: `url_verification`, or an `event_callback` whose
    /// event is `app_uninstalled` or a `tokens_revoked` naming bot tokens.
    fn read(body: &[u8]) -> SlackEvent {
        let Ok(envelope) = serde_json::from_slice::<Value>(body) else {
            return SlackEvent::Unreadable;
        };
        match envelope["type"].as_str() {
            Some("url_verification") => {
                envelope["challenge"]
                    .as_str()
                    .map_or(SlackEvent::Unreadable, |challenge| {
                        SlackEvent::UrlVerification {
                            challenge: challenge.to_string(),
                        }
                    })
            }
            Some("event_callback") => Self::app_lost(&envelope).unwrap_or(SlackEvent::Other),
            Some(_) => SlackEvent::Other,
            None => SlackEvent::Unreadable,
        }
    }

    fn app_lost(envelope: &Value) -> Option<SlackEvent> {
        let event = &envelope["event"];
        let reason = match event["type"].as_str()? {
            "app_uninstalled" => ReasonCode::AppRemoved,
            "tokens_revoked" if Self::names_bot_tokens(event) => ReasonCode::TokenRevoked,
            _ => return None,
        };
        Some(SlackEvent::AppLost {
            team_id: envelope["team_id"].as_str()?.to_string(),
            app_id: envelope["api_app_id"].as_str()?.to_string(),
            reason,
        })
    }

    /// `tokens_revoked` lists the revoked users' tokens by type; Foundry
    /// holds only a bot token.
    fn names_bot_tokens(event: &Value) -> bool {
        event["tokens"]["bot"]
            .as_array()
            .is_some_and(|bots| !bots.is_empty())
    }
}

/// The Foundry Slack app acting as one workspace's bot: the opened bot token
/// stays inside it (ADR-EIP-004). It posts (`DeliveryChannel`, write-only)
/// and lists and checks channels (`DestinationCatalog`, read-only).
pub struct SlackBot {
    http: reqwest::Client,
    api_base_url: String,
    token: SecretString,
}

impl SlackBot {
    pub(crate) fn new(app: &SlackApp, token: SecretString) -> Self {
        Self {
            http: app.http.clone(),
            api_base_url: app.api_base_url.clone(),
            token,
        }
    }

    fn method(&self, name: &str) -> String {
        format!("{}/{name}", self.api_base_url)
    }

    /// One read call; Slack's `ok` field decides.
    async fn read(&self, name: &str, query: &[(&str, &str)]) -> Result<Value, ReasonCode> {
        let response = self
            .http
            .get(self.method(name))
            .query(query)
            .bearer_auth(self.token.expose_secret())
            .send()
            .await
            .map_err(|_| ReasonCode::SlackNoAnswer)?;
        let (status, retry_after) = answer_head(&response);
        let body = response.json::<Value>().await.ok();
        let answer = SlackAnswer {
            status,
            retry_after,
            body: body.as_ref(),
        };
        match classify_slack(Some(answer)) {
            ChannelOutcome::Delivered => Ok(body.unwrap_or_default()),
            ChannelOutcome::Transient { reason, .. }
            | ChannelOutcome::RouteRefused { reason }
            | ChannelOutcome::ConnectionRefused { reason } => Err(reason),
        }
    }
}

/// An answer's status and its `Retry-After` (whole seconds, as Slack sends it).
fn answer_head(response: &reqwest::Response) -> (u16, Option<Duration>) {
    let retry_after = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs);
    (response.status().as_u16(), retry_after)
}

/// A channel Foundry can post to: public (`chat:write.public`), or private
/// with the app a member; never archived.
fn postable(channel: &Value) -> Option<Destination> {
    let private = channel["is_private"].as_bool().unwrap_or(false);
    let member = channel["is_member"].as_bool().unwrap_or(false);
    let archived = channel["is_archived"].as_bool().unwrap_or(false);
    if archived || (private && !member) {
        return None;
    }
    Some(Destination {
        reference: DestinationRef(channel["id"].as_str()?.to_string()),
        label: format!("#{}", channel["name"].as_str()?),
        shared_with_another_organisation: shared_with_another_organisation(channel),
    })
}

/// Whether Slack reports the channel shared with another organisation.
fn shared_with_another_organisation(channel: &Value) -> bool {
    channel["is_ext_shared"].as_bool().unwrap_or(false)
}

/// The most pages of `conversations.list` one listing reads.
const MAX_LIST_PAGES: usize = 10;

#[async_trait]
impl DeliveryChannel for SlackBot {
    fn kind(&self) -> ChannelKind {
        ChannelKind::Slack
    }

    async fn deliver(
        &self,
        destination: &DestinationRef,
        message: &EncodedMessage,
        _key: DeliveryKey,
    ) -> ChannelOutcome {
        let mut body = message.0.clone();
        body["channel"] = Value::String(destination.0.clone());
        let sent = self
            .http
            .post(self.method("chat.postMessage"))
            .bearer_auth(self.token.expose_secret())
            .json(&body)
            .send()
            .await;
        let Ok(response) = sent else {
            return classify_slack(None);
        };
        let (status, retry_after) = answer_head(&response);
        let Ok(text) = response.text().await else {
            return classify_slack(None);
        };
        let body = serde_json::from_str::<Value>(&text).ok();
        classify_slack(Some(SlackAnswer {
            status,
            retry_after,
            body: body.as_ref(),
        }))
    }

    async fn probe(&self) -> ChannelOutcome {
        match self.read("auth.test", &[]).await {
            Ok(_) => ChannelOutcome::Delivered,
            Err(reason) => ChannelOutcome::Transient {
                reason,
                retry_after: None,
            },
        }
    }
}

#[async_trait]
impl DestinationCatalog for SlackBot {
    async fn verify_connection(&self) -> Result<(), ReasonCode> {
        self.read("auth.test", &[]).await.map(|_| ())
    }

    async fn list_destinations(&self) -> Result<Vec<Destination>, ReasonCode> {
        let mut destinations = Vec::new();
        let mut cursor = String::new();
        for _ in 0..MAX_LIST_PAGES {
            let page = self
                .read(
                    "conversations.list",
                    &[
                        ("types", "public_channel,private_channel"),
                        ("exclude_archived", "true"),
                        ("limit", "200"),
                        ("cursor", cursor.as_str()),
                    ],
                )
                .await?;
            destinations.extend(
                page["channels"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(postable),
            );
            cursor = page["response_metadata"]["next_cursor"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            if cursor.is_empty() {
                break;
            }
        }
        destinations.sort_by(|a, b| a.label.cmp(&b.label));
        Ok(destinations)
    }

    async fn check_destination(
        &self,
        destination: &DestinationRef,
    ) -> Result<DestinationCheck, ReasonCode> {
        let answer = self
            .read("conversations.info", &[("channel", destination.0.as_str())])
            .await?;
        let channel = &answer["channel"];
        Ok(DestinationCheck {
            can_post: postable(channel).is_some(),
            member: channel["is_member"].as_bool().unwrap_or(false),
            shared_with_another_organisation: shared_with_another_organisation(channel),
            encrypted: false,
        })
    }
}

/// `https://canzanlabs.slack.com/` → `canzanlabs.slack.com`.
fn domain_of(url: &str) -> Option<String> {
    let host = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?
        .split('/')
        .next()?;
    (!host.is_empty()).then(|| host.to_string())
}

/// The Slack app manifest for this instance (DD-9): its return and events
/// addresses, the bot scopes and events, and nothing secret.
pub fn manifest(public_url: &str) -> String {
    let manifest = json!({
        "display_information": { "name": "Foundry" },
        "features": { "bot_user": { "display_name": "Foundry", "always_online": false } },
        "oauth_config": {
            "redirect_urls": [format!("{public_url}{RETURN_PATH}")],
            "scopes": { "bot": BOT_SCOPES }
        },
        "settings": {
            "event_subscriptions": {
                "request_url": format!("{public_url}{EVENTS_PATH}"),
                "bot_events": BOT_EVENTS
            },
            "org_deploy_enabled": false,
            "socket_mode_enabled": false,
            "token_rotation_enabled": false
        }
    });
    serde_json::to_string_pretty(&manifest).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    //! bypass: the install state's three refusals and the manifest are
    //! example-sized contracts; the acceptance lane drives them end to end.
    use super::*;
    use time::macros::datetime;

    const AT: OffsetDateTime = datetime!(2026-10-21 14:02 UTC);

    #[test]
    fn a_return_is_accepted_only_with_its_state_its_workspace_and_in_time() {
        let workspace = uuid::Uuid::now_v7();
        let held = InstallState::issue(workspace, AT);
        let state = held.state().to_string();
        assert!(held.accepts(&state, workspace, AT + time::Duration::minutes(9)));
        assert!(!held.accepts("forged", workspace, AT));
        assert!(!held.accepts(&state, uuid::Uuid::now_v7(), AT));
        assert!(!held.accepts(&state, workspace, AT + INSTALL_STATE_TTL));
        assert!(!held.accepts(&state, workspace, AT - time::Duration::seconds(1)));
    }

    #[test]
    fn the_manifest_names_this_instance_and_asks_only_the_bot_scopes() {
        let manifest = manifest("https://foundry.canzan.dev");
        assert!(manifest.contains("https://foundry.canzan.dev/workspace/integrations/slack/return"));
        assert!(manifest.contains("https://foundry.canzan.dev/integrations/slack/events"));
        for scope in BOT_SCOPES {
            assert!(manifest.contains(scope));
        }
        assert!(!manifest.contains("user_scope"));
    }

    const SECRET: &str = "eip-test-signing-secret";

    fn app() -> SlackApp {
        SlackApp::new(&SlackSettings {
            client_id: "client".into(),
            client_secret: SecretString::from("client-secret"),
            signing_secret: SecretString::from(SECRET),
            api_base_url: "http://127.0.0.1:9".into(),
            authorize_url: "http://127.0.0.1:9/authorize".into(),
        })
    }

    /// Slack's `v0` signature, computed independently of `signed`.
    fn sign(secret: &str, ts: i64, body: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(format!("v0:{ts}:{body}").as_bytes());
        let hex: String = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        format!("v0={hex}")
    }

    fn post(body: &str, secret: &str, age_seconds: i64) -> SlackEvent {
        let ts = AT.unix_timestamp() - age_seconds;
        app().read_event(
            Some(&ts.to_string()),
            Some(&sign(secret, ts, body)),
            body.as_bytes(),
            AT,
        )
    }

    const UNINSTALL: &str = r#"{"type":"event_callback","team_id":"T0CANZANLABS","api_app_id":"A0EIPFOUNDRY","event":{"type":"app_uninstalled"}}"#;

    #[test]
    fn an_event_is_believed_only_signed_with_the_secret_within_five_minutes() {
        let lost = SlackEvent::AppLost {
            team_id: "T0CANZANLABS".into(),
            app_id: "A0EIPFOUNDRY".into(),
            reason: ReasonCode::AppRemoved,
        };
        assert_eq!(post(UNINSTALL, SECRET, 0), lost);
        assert_eq!(post(UNINSTALL, SECRET, 4 * 60), lost, "four minutes old");
        assert_eq!(post(UNINSTALL, SECRET, EVENT_MAX_AGE_SECONDS), lost);
        assert_eq!(post(UNINSTALL, SECRET, -EVENT_MAX_AGE_SECONDS), lost);
        assert_eq!(
            post(UNINSTALL, SECRET, EVENT_MAX_AGE_SECONDS + 1),
            SlackEvent::Unsigned
        );
        assert_eq!(
            post(UNINSTALL, SECRET, -EVENT_MAX_AGE_SECONDS - 1),
            SlackEvent::Unsigned
        );
        assert_eq!(post(UNINSTALL, "another-secret", 0), SlackEvent::Unsigned);
        let ts = AT.unix_timestamp().to_string();
        let good = sign(SECRET, AT.unix_timestamp(), UNINSTALL);
        let body = UNINSTALL.as_bytes();
        for (timestamp, signature) in [
            (None, Some(good.as_str())),
            (Some(ts.as_str()), None),
            (Some("soon"), Some(good.as_str())),
            (Some(ts.as_str()), Some(&good[..good.len() - 1])),
        ] {
            assert_eq!(
                app().read_event(timestamp, signature, body, AT),
                SlackEvent::Unsigned
            );
        }
        let tampered = UNINSTALL.replace("T0CANZANLABS", "T0SOMEONEELS");
        assert_eq!(
            app().read_event(Some(&ts), Some(&good), tampered.as_bytes(), AT),
            SlackEvent::Unsigned
        );
    }

    #[test]
    fn a_signed_envelope_reads_as_slack_documents_it() {
        let read = |body: &str| post(body, SECRET, 0);
        assert_eq!(
            read(r#"{"token":"x","challenge":"c-123","type":"url_verification"}"#),
            SlackEvent::UrlVerification {
                challenge: "c-123".into()
            }
        );
        assert_eq!(
            read(r#"{"type":"url_verification"}"#),
            SlackEvent::Unreadable
        );
        assert_eq!(
            read(
                r#"{"type":"event_callback","team_id":"T1","api_app_id":"A1","event":{"type":"tokens_revoked","tokens":{"oauth":[],"bot":["U0BOT"]}}}"#
            ),
            SlackEvent::AppLost {
                team_id: "T1".into(),
                app_id: "A1".into(),
                reason: ReasonCode::TokenRevoked,
            }
        );
        for other in [
            r#"{"type":"event_callback","team_id":"T1","api_app_id":"A1","event":{"type":"tokens_revoked","tokens":{"oauth":["U1"],"bot":[]}}}"#,
            r#"{"type":"event_callback","team_id":"T1","api_app_id":"A1","event":{"type":"tokens_revoked","tokens":{"oauth":["U1"]}}}"#,
            r#"{"type":"event_callback","team_id":"T1","api_app_id":"A1","event":{"type":"message"}}"#,
            r#"{"type":"event_callback","api_app_id":"A1","event":{"type":"app_uninstalled"}}"#,
            r#"{"type":"event_callback","team_id":"T1","event":{"type":"app_uninstalled"}}"#,
            r#"{"type":"app_rate_limited"}"#,
        ] {
            assert_eq!(read(other), SlackEvent::Other, "{other}");
        }
        assert_eq!(read("not json"), SlackEvent::Unreadable);
        assert_eq!(read(r#"{"no":"type"}"#), SlackEvent::Unreadable);
    }

    #[test]
    fn the_domain_is_the_host_of_auth_tests_url() {
        assert_eq!(
            domain_of("https://canzanlabs.slack.com/").as_deref(),
            Some("canzanlabs.slack.com")
        );
        assert_eq!(domain_of("canzanlabs"), None);
        assert_eq!(domain_of("https:///"), None);
    }
}
