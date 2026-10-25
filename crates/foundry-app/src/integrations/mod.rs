//! external-integration-plugins — foundry-app's integrations module (DESIGN
//! [REF] Component Decomposition slices 01 and 04): settings ([`config`]), the
//! Slack adapter ([`slack`]), the Matrix adapter ([`matrix`]), the client for
//! user-supplied addresses ([`outbound`]) and the doors ([`handlers`]).
//!
//! [`Integrations`] is what the settings built; [`IntegrationsHandle`] is how
//! [`crate::AppState`] holds it. Production installs it once at boot; the
//! acceptance harness installs a scenario's settings after the app started
//! (`rewire`, DISTILL DIST-4), so handlers read the current value per request.

pub mod config;
pub mod handlers;
pub mod matrix;
pub mod outbound;
pub mod slack;
pub mod store_capabilities;
pub mod worker;

use crate::rate_limit::RevokeRateLimiter;
use config::IntegrationSettings;
use foundry_auth::secret_box::{SealedSecret, SecretBox, SecretSlot};
use foundry_store::integrations::KeyCheck;
use outbound::OutboundHttp;
use secrecy::{ExposeSecret, SecretString};
use slack::{SlackApp, SlackBot};
use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};
use worker::AdminMail;

/// What `/workspace/integrations` may offer for Slack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlackOffer {
    /// K-2: no Foundry Slack app on this instance.
    NotSetUp,
    /// K-20: the app is set up, the secrets key is not (R-3, DD-2).
    NoSecretsKey,
    /// K-19: the boot probe found stored credentials no configured key opens
    /// (DD-2 degraded start).
    Unavailable,
    Ready,
}

/// The plaintext every key's check value seals (`integration_key_checks`).
const KEY_CHECK_CANARY: &str = "foundry-integration-key-check";

/// The row a key's check value is bound to.
pub fn key_check_slot(key_id: &str) -> SecretSlot {
    SecretSlot::new("integration_key_checks", key_id, "sealed_canary")
}

/// The row a connection's credential is bound to (ADR-EIP-004: row-bound AAD).
pub fn credential_slot(connection_id: uuid::Uuid) -> SecretSlot {
    SecretSlot::new(
        "integration_connections",
        &connection_id.to_string(),
        "sealed_credential",
    )
}

/// The current key's check value, freshly sealed (`None` if sealing fails).
pub fn key_check(secrets: &SecretBox) -> Option<KeyCheck> {
    let key_id = secrets.current_key_id().to_string();
    let sealed = secrets
        .seal(
            &SecretString::from(KEY_CHECK_CANARY),
            &key_check_slot(&key_id),
        )
        .ok()?;
    Some(KeyCheck {
        key_id,
        sealed_canary: sealed.as_bytes().to_vec(),
    })
}

/// The secrets probe's verdict at boot (DD-2, ADR-EIP-004 decision 4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SecretsCheck {
    /// The ids of the keys stored credentials were sealed with that this
    /// Foundry does not hold, or holds but cannot prove (never a key).
    pub missing_key_ids: Vec<String>,
}

impl SecretsCheck {
    /// Whether some stored credential cannot be opened: start degraded.
    pub fn degraded(&self) -> bool {
        !self.missing_key_ids.is_empty()
    }
}

/// The integration settings and the adapters built from them.
pub struct Integrations {
    settings: IntegrationSettings,
    slack: Option<SlackApp>,
    /// Built on first use, so a test-support build reaches a test host
    /// admitted after these integrations were installed.
    outbound: std::sync::OnceLock<OutboundHttp>,
    secrets: SecretsCheck,
}

impl Integrations {
    pub fn new(settings: IntegrationSettings) -> Self {
        let slack = settings.slack.as_ref().map(SlackApp::new);
        Self {
            settings,
            slack,
            outbound: std::sync::OnceLock::new(),
            secrets: SecretsCheck::default(),
        }
    }

    /// The client for admin- and lead-supplied addresses (ADR-EIP-006).
    pub fn outbound(&self) -> &OutboundHttp {
        self.outbound
            .get_or_init(|| outbound_client(&self.settings))
    }

    /// These integrations as the boot secrets probe found them.
    pub fn with_secrets_check(self, secrets: SecretsCheck) -> Self {
        Self { secrets, ..self }
    }

    /// The current key's check value, freshly sealed (`None` without a key).
    pub fn key_check(&self) -> Option<KeyCheck> {
        key_check(self.settings.secrets.as_ref()?)
    }

    /// The boot probe: compare the key ids of the stored `credentials` with
    /// the configured keys, and open the stored check values of the
    /// configured keys. A key that stored credentials need but this Foundry
    /// lacks, or holds under an id whose check value does not open, is missing.
    pub fn check_secrets(&self, credentials: &[Vec<u8>], checks: &[KeyCheck]) -> SecretsCheck {
        let needed: BTreeSet<String> = credentials
            .iter()
            .map(|sealed| {
                SealedSecret::from_bytes(sealed.clone())
                    .key_id()
                    .to_string()
            })
            .collect();
        let missing = needed
            .into_iter()
            .filter(|key_id| !self.proves_key(key_id, checks))
            .collect();
        SecretsCheck {
            missing_key_ids: missing,
        }
    }

    /// Whether this Foundry holds the key `key_id` and its stored check
    /// value, if any, opens to the canary.
    fn proves_key(&self, key_id: &str, checks: &[KeyCheck]) -> bool {
        let Some(secrets) = self.settings.secrets.as_ref() else {
            return false;
        };
        if !secrets.key_ids().iter().any(|id| id.to_string() == key_id) {
            return false;
        }
        checks
            .iter()
            .filter(|check| check.key_id == key_id)
            .all(|check| {
                secrets
                    .open(
                        &SealedSecret::from_bytes(check.sealed_canary.clone()),
                        &key_check_slot(key_id),
                    )
                    .is_ok_and(|canary| canary.expose_secret() == KEY_CHECK_CANARY)
            })
    }

    pub fn settings(&self) -> &IntegrationSettings {
        &self.settings
    }

    pub fn slack_offer(&self) -> SlackOffer {
        match (&self.slack, &self.settings.secrets) {
            (None, _) => SlackOffer::NotSetUp,
            (Some(_), _) if self.secrets.degraded() => SlackOffer::Unavailable,
            (Some(_), None) => SlackOffer::NoSecretsKey,
            (Some(_), Some(_)) => SlackOffer::Ready,
        }
    }

    pub(crate) fn slack(&self) -> Option<&SlackApp> {
        self.slack.as_ref()
    }

    /// Slack's fixed return address for this instance.
    pub fn slack_return_url(&self) -> String {
        format!("{}{}", self.settings.public_url, slack::RETURN_PATH)
    }

    /// Whether this process delivers: its worker is on and it holds the
    /// secrets key the credentials are sealed with (DD-2).
    pub fn runs_worker(&self) -> bool {
        self.settings.worker.enabled && self.settings.secrets.is_some()
    }

    /// The connection's bot token, when this Foundry's keys open it for that
    /// row (ADR-EIP-004: the row-bound slot).
    fn open_credential(&self, connection_id: uuid::Uuid, sealed: Vec<u8>) -> Option<SecretString> {
        self.settings
            .secrets
            .as_ref()?
            .open(
                &SealedSecret::from_bytes(sealed),
                &credential_slot(connection_id),
            )
            .ok()
    }

    /// Ask Slack to withdraw the access the connection's (already forgotten)
    /// credential grants. `false` when Slack is not set up here, the
    /// credential does not open, or Slack did not agree.
    pub(crate) async fn withdraw_slack_access(
        &self,
        connection_id: uuid::Uuid,
        sealed: Vec<u8>,
    ) -> bool {
        let (Some(app), Some(token)) = (self.slack(), self.open_credential(connection_id, sealed))
        else {
            return false;
        };
        app.uninstall(&token).await.is_ok()
    }

    /// The Matrix bot of the connection `connection_id` on `homeserver`,
    /// when this Foundry can open its token.
    pub(crate) fn matrix_bot_at(
        &self,
        connection_id: uuid::Uuid,
        homeserver: &str,
        sealed: Vec<u8>,
    ) -> Option<matrix::MatrixBot> {
        let token = self.open_credential(connection_id, sealed)?;
        Some(matrix::MatrixBot::new(
            self.outbound().clone(),
            homeserver,
            token,
        ))
    }

    /// The Matrix bot of a workspace's connection (route doors).
    pub(crate) fn matrix_bot(
        &self,
        connection: &foundry_store::integrations::SealedConnection,
    ) -> Option<matrix::MatrixBot> {
        let sealed = connection.sealed_credential.clone()?;
        self.matrix_bot_at(connection.id, &connection.external_id, sealed)
    }

    /// The Slack bot of a connection, when Slack is set up and this Foundry
    /// can open the connection's credential.
    pub(crate) fn slack_bot(&self, connection_id: uuid::Uuid, sealed: Vec<u8>) -> Option<SlackBot> {
        let app = self.slack()?;
        let token = self.open_credential(connection_id, sealed)?;
        Some(SlackBot::new(app, token))
    }
}

#[cfg(not(feature = "test-support"))]
fn outbound_client(settings: &IntegrationSettings) -> OutboundHttp {
    OutboundHttp::new(settings.outbound.clone())
}

/// A test-support build reaches the test host the harness admitted.
#[cfg(feature = "test-support")]
fn outbound_client(settings: &IntegrationSettings) -> OutboundHttp {
    #[cfg(feature = "test-support")]
    let client = outbound::outbound_seam::client_for_this_process(settings.outbound.clone());
    client
}

/// A route's destination is re-checked through the connection's Slack bot
/// (Resume, D-8).
impl foundry_services::integrations::DestinationCatalogs for Integrations {
    fn catalogue(
        &self,
        connection: &foundry_store::integrations::SealedConnection,
    ) -> Option<Box<dyn foundry_integrations::DestinationCatalog>> {
        let sealed = connection.sealed_credential.clone()?;
        let bot = self.slack_bot(connection.id, sealed)?;
        Some(Box::new(bot))
    }
}

/// The shared, replaceable [`Integrations`] (cloning shares it), and the
/// Send test limit that outlives a replacement.
#[derive(Clone)]
pub struct IntegrationsHandle {
    current: Arc<RwLock<Arc<Integrations>>>,
    test_messages: Arc<RevokeRateLimiter>,
    admin_mail: AdminMail,
}

impl IntegrationsHandle {
    pub fn new(integrations: Integrations) -> Self {
        Self {
            current: Arc::new(RwLock::new(Arc::new(integrations))),
            test_messages: Arc::new(RevokeRateLimiter::test_messages()),
            admin_mail: AdminMail::default(),
        }
    }

    /// How workspace admins are told a connection was lost (DDR-23); the
    /// composition root hands it the shipped `Notifier`.
    pub fn admin_mail(&self) -> &AdminMail {
        &self.admin_mail
    }

    /// Send test's per-person limit (DDR-25, R-5).
    pub fn test_messages(&self) -> &RevokeRateLimiter {
        &self.test_messages
    }

    /// No Slack app, no secrets key: integrations are not offered.
    pub fn off(public_url: &str) -> Self {
        Self::new(Integrations::new(IntegrationSettings::off(public_url)))
    }

    /// The integrations as they are now.
    pub fn current(&self) -> Arc<Integrations> {
        match self.current.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// Replace the integrations every later request reads.
    pub fn install(&self, integrations: Integrations) {
        let next = Arc::new(integrations);
        match self.current.write() {
            Ok(mut guard) => *guard = next,
            Err(poisoned) => *poisoned.into_inner() = next,
        }
    }
}

impl Default for IntegrationsHandle {
    fn default() -> Self {
        Self::off("http://localhost")
    }
}
