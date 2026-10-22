//! The integration settings an operator gives Foundry (DESIGN Wave Decisions
//! handoff; DDR-9, DDR-10, DD-2; ADR-EIP-004, ADR-EIP-005 decision 1).
//!
//! Read once through a lookup (the environment in production, the scenario's
//! settings in the acceptance harness), so both read the same names the same
//! way. A malformed secrets key or a partial Slack trio refuses start naming
//! the setting; a missing secrets key is not an error (integrations are simply
//! not offered: DD-2, R-3). The delivery worker's settings (give-up, lease,
//! poll) are read where the worker is built.

use foundry_auth::secret_box::{
    SecretBox, SecretBoxError, SECRETS_KEY_PREVIOUS_SETTING, SECRETS_KEY_SETTING,
};
use secrecy::SecretString;

pub const SLACK_CLIENT_ID: &str = "SLACK_CLIENT_ID";
pub const SLACK_CLIENT_SECRET: &str = "SLACK_CLIENT_SECRET";
pub const SLACK_SIGNING_SECRET: &str = "SLACK_SIGNING_SECRET";
pub const SLACK_API_BASE_URL: &str = "SLACK_API_BASE_URL";
pub const SLACK_AUTHORIZE_URL: &str = "SLACK_AUTHORIZE_URL";
pub const PUBLIC_URL: &str = "FOUNDRY_PUBLIC_URL";

pub const DEFAULT_SLACK_API_BASE_URL: &str = "https://slack.com/api";
pub const DEFAULT_SLACK_AUTHORIZE_URL: &str = "https://slack.com/oauth/v2/authorize";

/// Why the settings refuse start. Names settings only, never a value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("Slack is configured incompletely: {missing} is not set (set all three of SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET, or none)")]
    PartialSlack { missing: &'static str },
    #[error(transparent)]
    SecretsKey(#[from] SecretBoxError),
}

/// The Foundry Slack app this instance runs (OD-3): all three settings.
pub struct SlackSettings {
    pub client_id: String,
    pub client_secret: SecretString,
    pub signing_secret: SecretString,
    pub api_base_url: String,
    pub authorize_url: String,
}

/// Every integration setting, read once.
pub struct IntegrationSettings {
    /// `FOUNDRY_PUBLIC_URL`, without a trailing slash: Slack's return and
    /// events addresses are built from it.
    pub public_url: String,
    pub slack: Option<SlackSettings>,
    /// `None` when `FOUNDRY_SECRETS_KEY` is not set (DD-2: integrations off).
    pub secrets: Option<SecretBox>,
}

impl IntegrationSettings {
    /// Read from the process environment; `default_public_url` stands in for
    /// an unset `FOUNDRY_PUBLIC_URL`.
    pub fn from_env(default_public_url: &str) -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok(), default_public_url)
    }

    /// Read through `lookup` (a blank value counts as unset).
    pub fn from_lookup(
        lookup: impl Fn(&str) -> Option<String>,
        default_public_url: &str,
    ) -> Result<Self, ConfigError> {
        let get = |name: &str| lookup(name).filter(|v| !v.trim().is_empty());
        let public_url = get(PUBLIC_URL)
            .unwrap_or_else(|| default_public_url.to_string())
            .trim_end_matches('/')
            .to_string();
        Ok(Self {
            public_url,
            slack: slack_settings(&get)?,
            secrets: secrets(&get)?,
        })
    }

    /// No Slack app and no secrets key: what a harness starts with.
    pub fn off(public_url: &str) -> Self {
        Self {
            public_url: public_url.trim_end_matches('/').to_string(),
            slack: None,
            secrets: None,
        }
    }
}

fn slack_settings(
    get: &impl Fn(&str) -> Option<String>,
) -> Result<Option<SlackSettings>, ConfigError> {
    let (client_id, client_secret, signing_secret) = match (
        get(SLACK_CLIENT_ID),
        get(SLACK_CLIENT_SECRET),
        get(SLACK_SIGNING_SECRET),
    ) {
        (None, None, None) => return Ok(None),
        (Some(id), Some(secret), Some(signing)) => (id, secret, signing),
        (None, _, _) => {
            return Err(ConfigError::PartialSlack {
                missing: SLACK_CLIENT_ID,
            })
        }
        (_, None, _) => {
            return Err(ConfigError::PartialSlack {
                missing: SLACK_CLIENT_SECRET,
            })
        }
        (_, _, None) => {
            return Err(ConfigError::PartialSlack {
                missing: SLACK_SIGNING_SECRET,
            })
        }
    };
    Ok(Some(SlackSettings {
        client_id,
        client_secret: SecretString::from(client_secret),
        signing_secret: SecretString::from(signing_secret),
        api_base_url: get(SLACK_API_BASE_URL)
            .unwrap_or_else(|| DEFAULT_SLACK_API_BASE_URL.to_string())
            .trim_end_matches('/')
            .to_string(),
        authorize_url: get(SLACK_AUTHORIZE_URL)
            .unwrap_or_else(|| DEFAULT_SLACK_AUTHORIZE_URL.to_string()),
    }))
}

fn secrets(get: &impl Fn(&str) -> Option<String>) -> Result<Option<SecretBox>, ConfigError> {
    let Some(current) = get(SECRETS_KEY_SETTING) else {
        return Ok(None);
    };
    let previous = get(SECRETS_KEY_PREVIOUS_SETTING).map(SecretString::from);
    Ok(Some(SecretBox::from_settings(
        &SecretString::from(current),
        previous.as_ref(),
    )?))
}
