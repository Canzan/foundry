//! The Slack adapter for the install flow (ADR-EIP-005 decisions 1-4): the
//! approval address, the session-held install state, `oauth.v2.access`,
//! `auth.test`, and the app manifest the setup page renders (DD-9).
//!
//! The client follows no redirect and keeps the bot token inside a
//! [`SecretString`]; no error carries Slack's own text or a credential.

use super::config::SlackSettings;
use base64::Engine;
use rand::RngCore;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
