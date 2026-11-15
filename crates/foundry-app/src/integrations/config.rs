//! The integration settings an operator gives Foundry (DESIGN Wave Decisions
//! handoff; DDR-9, DDR-10, DD-2; ADR-EIP-004, ADR-EIP-005 decision 1).
//!
//! Read once through a lookup (the environment in production, the scenario's
//! settings in the acceptance harness), so both read the same names the same
//! way. A malformed instance key, the two key names set to different values
//! (ADR-IIS-001 decision 1), a partial Slack trio or a malformed
//! `FOUNDRY_OUTBOUND_ALLOW` entry refuses start naming the settings; a missing
//! key is not an error (integrations are simply not offered: DD-2, R-3).
//! The delivery worker's settings (poll, lease,
//! give-up, on/off) are read here too, so the acceptance harness hands them to
//! the in-process worker the same way (ADR-EIP-002 decisions 4 and 11).

use foundry_auth::secret_box::{
    SecretBox, SecretBoxError, SecretsKey, INSTANCE_KEY_PREVIOUS_SETTING, INSTANCE_KEY_SETTING,
    SECRETS_KEY_PREVIOUS_SETTING, SECRETS_KEY_SETTING,
};
use foundry_integrations::{MalformedAllowEntry, OutboundAllow, OUTBOUND_ALLOW_SETTING};
use foundry_store::integrations::KeyWrapper;
use secrecy::SecretString;
use std::path::PathBuf;
use std::time::Duration;

pub const SLACK_CLIENT_ID: &str = "SLACK_CLIENT_ID";
pub const SLACK_CLIENT_SECRET: &str = "SLACK_CLIENT_SECRET";
pub const SLACK_SIGNING_SECRET: &str = "SLACK_SIGNING_SECRET";
pub const SLACK_API_BASE_URL: &str = "SLACK_API_BASE_URL";
pub const SLACK_AUTHORIZE_URL: &str = "SLACK_AUTHORIZE_URL";
pub const PUBLIC_URL: &str = "FOUNDRY_PUBLIC_URL";

pub const POLL_MS: &str = "FOUNDRY_INTEGRATIONS_POLL_MS";
pub const LEASE_SECONDS: &str = "FOUNDRY_INTEGRATIONS_LEASE_SECONDS";
pub const GIVE_UP_SECONDS: &str = "FOUNDRY_INTEGRATIONS_GIVE_UP_SECONDS";
/// `off` stops this process's delivery worker (another process sends).
pub const WORKER: &str = "FOUNDRY_INTEGRATIONS_WORKER";

/// How often a replica checks for instance integration changes (ADR-IIS-004).
pub const RELOAD_MS: &str = "FOUNDRY_INTEGRATIONS_RELOAD_MS";

/// The data directory's setting and default live with the one adapter that
/// reads it (check-arch `integrations-key-file-seam`).
pub use super::key_file::{DATA_DIR, DEFAULT_DATA_DIR};
pub const DEFAULT_RELOAD_MS: u64 = 5_000;
pub const DEFAULT_POLL_MS: u64 = 1_000;
pub const DEFAULT_LEASE_SECONDS: u64 = 45;
pub const DEFAULT_GIVE_UP_SECONDS: u64 = 86_400;
/// The slowest reload a deployment may ask for, 20 s (ADR-IIS-SKR-004, L14):
/// the envelope the live-replica gate window is sized from, and the one owner
/// of that number - [`super::secrets_key_rotation::LiveOpeners`] reads it from
/// here. A server polling more slowly leaves longer between two of its own
/// replica-state beats than the gate counts one as current for, so its running
/// servers look gone to one another; above this the read refuses rather than
/// serve with a window its own beats outgrow (D11).
pub const MAX_RELOAD_MS: u64 = 20_000;
/// The worst-case claim-to-record path (resolve, pre-send read, send) plus a
/// spare: a shorter lease could expire under a live send.
pub const MIN_LEASE_SECONDS: u64 =
    RESOLVE_SHARE_SECONDS + PRE_SEND_READ_SHARE_SECONDS + SEND_SHARE_SECONDS + SPARE_SECONDS;
/// The send's share of the lease: no send may take longer, whatever else
/// bounds it (a project email through the notifier's transport included).
pub const SEND_SHARE_SECONDS: u64 = 10;
const RESOLVE_SHARE_SECONDS: u64 = 5;
const PRE_SEND_READ_SHARE_SECONDS: u64 = 10;
const SPARE_SECONDS: u64 = 10;

pub const DEFAULT_SLACK_API_BASE_URL: &str = "https://slack.com/api";
pub const DEFAULT_SLACK_AUTHORIZE_URL: &str = "https://slack.com/oauth/v2/authorize";

/// Why the settings refuse start. Names settings only, never a value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("Slack is configured incompletely: {missing} is not set (set all three of SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET, or none)")]
    PartialSlack { missing: &'static str },
    #[error(transparent)]
    SecretsKey(#[from] SecretBoxError),
    #[error("{INSTANCE_KEY_SETTING} and {SECRETS_KEY_SETTING} are set to different values: set one of them, or both to the same key")]
    ConflictingKeys,
    #[error("{setting} is not a whole number")]
    NotANumber { setting: &'static str },
    #[error("FOUNDRY_INTEGRATIONS_LEASE_SECONDS is under {MIN_LEASE_SECONDS}: a delivery's lease could expire during its send")]
    LeaseTooShort,
    #[error("{RELOAD_MS} is {value}, over the {MAX_RELOAD_MS} ms (20 s) a Foundry may reload at: a server polling that slowly leaves longer between two of its own replica-state beats than another server counts one as current for, so running servers look gone to one another and an instance admin is offered a way out that discards credentials they are still serving with. Unset {RELOAD_MS} for the {DEFAULT_RELOAD_MS} ms default, or set it to {MAX_RELOAD_MS} or less")]
    ReloadTooSlow { value: u64 },
    #[error(transparent)]
    OutboundAllow(#[from] MalformedAllowEntry),
}

/// Which setting supplied the operator's instance key (ADR-IIS-001 decision 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySetting {
    /// `FOUNDRY_INSTANCE_KEY`.
    InstanceKey,
    /// The legacy `FOUNDRY_SECRETS_KEY`, alone or with the same value under
    /// both names.
    LegacySecretsKey,
}

impl KeySetting {
    /// Which name supplies the key, and its text, given each name's
    /// non-blank value: the one that is set, the legacy name when both hold
    /// the same value, and a refusal when they hold different values. Does not
    /// parse the key.
    fn resolve(
        instance_key: Option<String>,
        legacy_key: Option<String>,
    ) -> Result<Option<(Self, SecretString)>, ConfigError> {
        let chosen = match (instance_key, legacy_key) {
            (None, None) => return Ok(None),
            (Some(new), None) => (Self::InstanceKey, new),
            (None, Some(legacy)) => (Self::LegacySecretsKey, legacy),
            (Some(new), Some(legacy)) if new == legacy => (Self::LegacySecretsKey, legacy),
            (Some(_), Some(_)) => return Err(ConfigError::ConflictingKeys),
        };
        Ok(Some((chosen.0, SecretString::from(chosen.1))))
    }

    /// The setting's name.
    pub fn name(self) -> &'static str {
        match self {
            Self::InstanceKey => INSTANCE_KEY_SETTING,
            Self::LegacySecretsKey => SECRETS_KEY_SETTING,
        }
    }
}

/// The delivery worker's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerSettings {
    /// Whether this process runs the dispatcher and a sender.
    pub enabled: bool,
    pub poll: Duration,
    pub lease: Duration,
    pub give_up_after: Duration,
}

impl WorkerSettings {
    /// No worker (what a harness starts with).
    pub fn off() -> Self {
        Self {
            enabled: false,
            poll: Duration::from_millis(DEFAULT_POLL_MS),
            lease: Duration::from_secs(DEFAULT_LEASE_SECONDS),
            give_up_after: Duration::from_secs(DEFAULT_GIVE_UP_SECONDS),
        }
    }

    fn read(get: &impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let number = |setting: &'static str, default: u64| whole_number(get, setting, default);
        let lease = number(LEASE_SECONDS, DEFAULT_LEASE_SECONDS)?;
        if lease < MIN_LEASE_SECONDS {
            return Err(ConfigError::LeaseTooShort);
        }
        Ok(Self {
            enabled: get(WORKER).is_none_or(|v| !v.trim().eq_ignore_ascii_case("off")),
            poll: Duration::from_millis(number(POLL_MS, DEFAULT_POLL_MS)?.max(1)),
            lease: Duration::from_secs(lease),
            give_up_after: Duration::from_secs(number(GIVE_UP_SECONDS, DEFAULT_GIVE_UP_SECONDS)?),
        })
    }
}

/// A whole-number setting, `default` when unset.
fn whole_number(
    get: &impl Fn(&str) -> Option<String>,
    setting: &'static str,
    default: u64,
) -> Result<u64, ConfigError> {
    match get(setting) {
        None => Ok(default),
        Some(raw) => raw
            .trim()
            .parse::<u64>()
            .map_err(|_| ConfigError::NotANumber { setting }),
    }
}

/// The Foundry Slack app this instance runs (OD-3): all three settings.
pub struct SlackSettings {
    pub client_id: String,
    pub client_secret: SecretString,
    pub signing_secret: SecretString,
    pub api_base_url: String,
    pub authorize_url: String,
}

/// Where this Foundry reaches Slack's API and sends admins for approval:
/// Slack's own addresses, or the `SLACK_API_BASE_URL` / `SLACK_AUTHORIZE_URL`
/// test seams. They apply to whichever Slack app is in effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackUrls {
    pub api_base_url: String,
    pub authorize_url: String,
}

impl SlackUrls {
    fn read(get: &impl Fn(&str) -> Option<String>) -> Self {
        Self {
            api_base_url: get(SLACK_API_BASE_URL)
                .unwrap_or_else(|| DEFAULT_SLACK_API_BASE_URL.to_string())
                .trim_end_matches('/')
                .to_string(),
            authorize_url: get(SLACK_AUTHORIZE_URL)
                .unwrap_or_else(|| DEFAULT_SLACK_AUTHORIZE_URL.to_string()),
        }
    }
}

impl Default for SlackUrls {
    fn default() -> Self {
        Self::read(&|_| None)
    }
}

/// Every integration setting, read once.
pub struct IntegrationSettings {
    /// `FOUNDRY_PUBLIC_URL`, without a trailing slash: Slack's return and
    /// events addresses are built from it.
    pub public_url: String,
    pub slack: Option<SlackSettings>,
    /// Slack's addresses, for the environment's or the saved app.
    pub slack_urls: SlackUrls,
    /// Whether `FOUNDRY_PUBLIC_URL` was unset and `public_url` is the
    /// process's own fallback address (I-29).
    pub public_url_fell_back: bool,
    /// `None` when neither key name is set (DD-2: integrations off).
    pub secrets: Option<SecretBox>,
    /// Which name supplied the operator's key; `None` when neither is set.
    pub key_setting: Option<KeySetting>,
    /// `FOUNDRY_DATA_DIR`.
    pub data_dir: PathBuf,
    /// `FOUNDRY_INTEGRATIONS_RELOAD_MS`.
    pub reload: Duration,
    pub worker: WorkerSettings,
    /// `FOUNDRY_OUTBOUND_ALLOW`: the private ranges and hosts outbound
    /// requests may reach (ADR-EIP-006 decision 6); empty when unset.
    pub outbound: OutboundAllow,
}

/// A setting's value through `lookup`: a blank value counts as unset (the
/// one rule every integrations setting, the public URL included, follows).
fn setting(lookup: &impl Fn(&str) -> Option<String>, name: &str) -> Option<String> {
    lookup(name).filter(|value| !value.trim().is_empty())
}

/// `FOUNDRY_PUBLIC_URL` read through `lookup`, or `fallback` (I-29's
/// address) when it is unset or blank: what the composition root hands
/// every reader.
pub fn public_url_or(lookup: impl Fn(&str) -> Option<String>, fallback: &str) -> String {
    setting(&lookup, PUBLIC_URL).unwrap_or_else(|| fallback.to_string())
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
        let get = |name: &str| setting(&lookup, name);
        let configured_public_url = get(PUBLIC_URL);
        let public_url_fell_back = configured_public_url.is_none();
        let public_url = configured_public_url
            .unwrap_or_else(|| default_public_url.to_string())
            .trim_end_matches('/')
            .to_string();
        let (key_setting, secrets) = secrets(&get)?;
        let slack_urls = SlackUrls::read(&get);
        // The reload bound, read here and refused here: no settings value is
        // produced above it, so the server never serves with a reload the
        // live-replica gate window cannot cover (L14, D11). A value under a
        // millisecond still reads as one, and a value that is no whole number
        // still reads as the number error.
        let reload_ms = whole_number(&get, RELOAD_MS, DEFAULT_RELOAD_MS)?;
        if reload_ms > MAX_RELOAD_MS {
            return Err(ConfigError::ReloadTooSlow { value: reload_ms });
        }
        Ok(Self {
            public_url,
            public_url_fell_back,
            slack: slack_settings(&get, &slack_urls)?,
            slack_urls,
            secrets,
            key_setting,
            data_dir: get(DATA_DIR).map_or_else(|| PathBuf::from(DEFAULT_DATA_DIR), PathBuf::from),
            reload: Duration::from_millis(reload_ms.max(1)),
            worker: WorkerSettings::read(&get)?,
            outbound: OutboundAllow::parse(&get(OUTBOUND_ALLOW_SETTING).unwrap_or_default())?,
        })
    }

    /// No Slack app and no secrets key: what a harness starts with.
    pub fn off(public_url: &str) -> Self {
        Self {
            public_url: public_url.trim_end_matches('/').to_string(),
            public_url_fell_back: false,
            slack: None,
            slack_urls: SlackUrls::default(),
            secrets: None,
            key_setting: None,
            data_dir: PathBuf::from(DEFAULT_DATA_DIR),
            reload: Duration::from_millis(DEFAULT_RELOAD_MS),
            worker: WorkerSettings::off(),
            outbound: OutboundAllow::default(),
        }
    }

    /// Which instance key wraps a secrets key this server makes: the
    /// operator's wherever a key name supplied one, else the instance key
    /// file (ADR-IIS-002). The one owner of that rule, so Generate, Move,
    /// Replace and Start over cannot record different answers for the same
    /// settings.
    pub fn key_wrapper(&self) -> KeyWrapper {
        match self.key_setting {
            Some(_) => KeyWrapper::Operator,
            None => KeyWrapper::File,
        }
    }
}

fn slack_settings(
    get: &impl Fn(&str) -> Option<String>,
    urls: &SlackUrls,
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
        api_base_url: urls.api_base_url.clone(),
        authorize_url: urls.authorize_url.clone(),
    }))
}

/// The operator's key under whichever name supplies it, and every retired
/// key listed under either previous-key name (both are for opening only).
pub(crate) fn secrets(
    get: &impl Fn(&str) -> Option<String>,
) -> Result<(Option<KeySetting>, Option<SecretBox>), ConfigError> {
    let resolved = KeySetting::resolve(get(INSTANCE_KEY_SETTING), get(SECRETS_KEY_SETTING))?;
    let Some((setting, text)) = resolved else {
        return Ok((None, None));
    };
    let current = SecretsKey::parse(&text, setting.name())?;
    let mut previous = Vec::new();
    for name in [INSTANCE_KEY_PREVIOUS_SETTING, SECRETS_KEY_PREVIOUS_SETTING] {
        if let Some(list) = get(name) {
            previous.extend(SecretsKey::parse_list(&SecretString::from(list), name)?);
        }
    }
    Ok((Some(setting), Some(SecretBox::new(current, previous))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use foundry_auth::secret_box::KeyId;
    use std::collections::HashMap;

    const FALLBACK_URL: &str = "http://localhost:8080";
    /// 20 characters: base64url, but 15 bytes.
    const MALFORMED: &str = "c2hvcnQta2V5LXRleHQh";

    fn key_text(byte: u8) -> String {
        URL_SAFE_NO_PAD.encode([byte; 32])
    }

    fn key_id(text: &str) -> KeyId {
        SecretsKey::parse(&SecretString::from(text.to_string()), INSTANCE_KEY_SETTING)
            .expect("a well-formed key")
            .key_id()
    }

    fn read(settings: &[(&str, &str)]) -> Result<IntegrationSettings, ConfigError> {
        let settings: HashMap<String, String> = settings
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        IntegrationSettings::from_lookup(|name| settings.get(name).cloned(), FALLBACK_URL)
    }

    /// One value a key name can hold.
    #[derive(Debug, Clone, Copy)]
    enum Value {
        Unset,
        Empty,
        KeyA,
        KeyB,
        Malformed,
    }

    impl Value {
        fn text(self) -> Option<String> {
            match self {
                Self::Unset => None,
                Self::Empty => Some(String::new()),
                Self::KeyA => Some(key_text(1)),
                Self::KeyB => Some(key_text(2)),
                Self::Malformed => Some(MALFORMED.to_string()),
            }
        }
    }

    /// What the two key names resolve to.
    #[derive(Debug, PartialEq, Eq)]
    enum Outcome {
        NoKey,
        Key(KeySetting, KeyId),
        Malformed(&'static str),
        Conflict,
    }

    fn outcome(new: Value, legacy: Value) -> Outcome {
        let mut settings = Vec::new();
        let (new_text, legacy_text) = (new.text(), legacy.text());
        if let Some(text) = &new_text {
            settings.push((INSTANCE_KEY_SETTING, text.as_str()));
        }
        if let Some(text) = &legacy_text {
            settings.push((SECRETS_KEY_SETTING, text.as_str()));
        }
        match read(&settings) {
            Ok(read) => match (read.key_setting, read.secrets) {
                (None, None) => Outcome::NoKey,
                (Some(setting), Some(secrets)) => Outcome::Key(setting, secrets.current_key_id()),
                _ => panic!("a key setting without a key, or the reverse"),
            },
            Err(err) => {
                let text = format!("{err} {err:?}");
                for value in [&new_text, &legacy_text].into_iter().flatten() {
                    assert!(
                        value.is_empty() || !text.contains(value.as_str()),
                        "the refusal never prints a value: {text}"
                    );
                }
                match err {
                    ConfigError::SecretsKey(SecretBoxError::MalformedKey { setting }) => {
                        Outcome::Malformed(setting)
                    }
                    ConfigError::ConflictingKeys => Outcome::Conflict,
                    other => panic!("unexpected refusal {other:?}"),
                }
            }
        }
    }

    #[test]
    fn the_two_key_names_resolve_over_every_pair_of_values() {
        use KeySetting::{InstanceKey as New, LegacySecretsKey as Legacy};
        use Value::{Empty, KeyA, KeyB, Malformed, Unset};
        let a = || key_id(&key_text(1));
        let b = || key_id(&key_text(2));
        let table = [
            (Unset, Unset, Outcome::NoKey),
            (Unset, Empty, Outcome::NoKey),
            (Unset, KeyA, Outcome::Key(Legacy, a())),
            (Unset, KeyB, Outcome::Key(Legacy, b())),
            (Unset, Malformed, Outcome::Malformed(SECRETS_KEY_SETTING)),
            (Empty, Unset, Outcome::NoKey),
            (Empty, Empty, Outcome::NoKey),
            (Empty, KeyA, Outcome::Key(Legacy, a())),
            (Empty, KeyB, Outcome::Key(Legacy, b())),
            (Empty, Malformed, Outcome::Malformed(SECRETS_KEY_SETTING)),
            (KeyA, Unset, Outcome::Key(New, a())),
            (KeyA, Empty, Outcome::Key(New, a())),
            (KeyA, KeyA, Outcome::Key(Legacy, a())),
            (KeyA, KeyB, Outcome::Conflict),
            (KeyA, Malformed, Outcome::Conflict),
            (KeyB, Unset, Outcome::Key(New, b())),
            (KeyB, Empty, Outcome::Key(New, b())),
            (KeyB, KeyA, Outcome::Conflict),
            (KeyB, KeyB, Outcome::Key(Legacy, b())),
            (KeyB, Malformed, Outcome::Conflict),
            (Malformed, Unset, Outcome::Malformed(INSTANCE_KEY_SETTING)),
            (Malformed, Empty, Outcome::Malformed(INSTANCE_KEY_SETTING)),
            (Malformed, KeyA, Outcome::Conflict),
            (Malformed, KeyB, Outcome::Conflict),
            (
                Malformed,
                Malformed,
                Outcome::Malformed(SECRETS_KEY_SETTING),
            ),
        ];
        for (new, legacy, expected) in table {
            assert_eq!(outcome(new, legacy), expected, "{new:?} / {legacy:?}");
        }
    }

    #[test]
    fn a_conflict_names_both_settings() {
        let text = ConfigError::ConflictingKeys.to_string();
        assert!(text.contains(INSTANCE_KEY_SETTING) && text.contains(SECRETS_KEY_SETTING));
    }

    #[test]
    fn retired_keys_are_held_under_either_previous_name() {
        let (current, new_old, legacy_old) = (key_text(1), key_text(3), key_text(4));
        let read = read(&[
            (INSTANCE_KEY_SETTING, &current),
            (INSTANCE_KEY_PREVIOUS_SETTING, &new_old),
            (SECRETS_KEY_PREVIOUS_SETTING, &format!(" {legacy_old} ,")),
        ])
        .expect("well-formed keys");
        let held = read.secrets.expect("a key").key_ids();
        assert_eq!(
            held,
            [key_id(&current), key_id(&new_old), key_id(&legacy_old)]
        );

        for name in [INSTANCE_KEY_PREVIOUS_SETTING, SECRETS_KEY_PREVIOUS_SETTING] {
            let refusal = read_err(&[(INSTANCE_KEY_SETTING, &current), (name, MALFORMED)]);
            assert_eq!(
                refusal,
                ConfigError::SecretsKey(SecretBoxError::MalformedKey { setting: name })
            );
        }
    }

    fn read_err(settings: &[(&str, &str)]) -> ConfigError {
        match read(settings) {
            Ok(_) => panic!("the settings are refused"),
            Err(err) => err,
        }
    }

    /// (settings, data dir, reload ms, public URL, fell back)
    type Reported = (
        &'static [(&'static str, &'static str)],
        &'static str,
        u64,
        &'static str,
        bool,
    );

    #[test]
    fn data_dir_reload_period_and_public_url_fallback_are_reported() {
        let table: [Reported; 4] = [
            (&[], DEFAULT_DATA_DIR, DEFAULT_RELOAD_MS, FALLBACK_URL, true),
            (
                &[(DATA_DIR, ""), (RELOAD_MS, " "), (PUBLIC_URL, "")],
                DEFAULT_DATA_DIR,
                DEFAULT_RELOAD_MS,
                FALLBACK_URL,
                true,
            ),
            (
                &[
                    (DATA_DIR, "/srv/foundry"),
                    (RELOAD_MS, " 200 "),
                    (PUBLIC_URL, "https://foundry.example/"),
                ],
                "/srv/foundry",
                200,
                "https://foundry.example",
                false,
            ),
            (&[(RELOAD_MS, "0")], DEFAULT_DATA_DIR, 1, FALLBACK_URL, true),
        ];
        for (settings, data_dir, reload_ms, public_url, fell_back) in table {
            let read = read(settings).expect("valid settings");
            assert_eq!(read.data_dir, PathBuf::from(data_dir), "{settings:?}");
            assert_eq!(
                read.reload,
                Duration::from_millis(reload_ms),
                "{settings:?}"
            );
            assert_eq!(read.public_url, public_url, "{settings:?}");
            assert_eq!(read.public_url_fell_back, fell_back, "{settings:?}");
        }
        assert_eq!(
            read_err(&[(RELOAD_MS, "soon")]),
            ConfigError::NotANumber { setting: RELOAD_MS }
        );
    }

    /// The composition root's public URL reads blank as unset, as the
    /// settings do: never "use , which Slack cannot reach" (item 48).
    #[test]
    fn a_blank_public_url_falls_back_like_an_unset_one() {
        for (value, expected) in [
            (None, FALLBACK_URL),
            (Some(""), FALLBACK_URL),
            (Some(" \t"), FALLBACK_URL),
            (Some("https://foundry.example"), "https://foundry.example"),
        ] {
            let lookup = |name: &str| {
                (name == PUBLIC_URL)
                    .then_some(value)
                    .flatten()
                    .map(str::to_string)
            };
            assert_eq!(public_url_or(lookup, FALLBACK_URL), expected, "{value:?}");
            let read = IntegrationSettings::from_lookup(lookup, FALLBACK_URL).unwrap();
            assert_eq!(read.public_url, expected, "{value:?}: one rule");
        }
    }
}
