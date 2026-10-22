//! external-integration-plugins — foundry-app's integrations module (DESIGN
//! [REF] Component Decomposition slice 01): settings ([`config`]), the Slack
//! adapter ([`slack`]) and the doors ([`handlers`]).
//!
//! [`Integrations`] is what the settings built; [`IntegrationsHandle`] is how
//! [`crate::AppState`] holds it. Production installs it once at boot; the
//! acceptance harness installs a scenario's settings after the app started
//! (`rewire`, DISTILL DIST-4), so handlers read the current value per request.

pub mod config;
pub mod handlers;
pub mod slack;

use config::IntegrationSettings;
use slack::SlackApp;
use std::sync::{Arc, RwLock};

/// What `/workspace/integrations` may offer for Slack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlackOffer {
    /// K-2: no Foundry Slack app on this instance.
    NotSetUp,
    /// K-20: the app is set up, the secrets key is not (R-3, DD-2).
    NoSecretsKey,
    Ready,
}

/// The integration settings and the adapters built from them.
pub struct Integrations {
    settings: IntegrationSettings,
    slack: Option<SlackApp>,
}

impl Integrations {
    pub fn new(settings: IntegrationSettings) -> Self {
        let slack = settings.slack.as_ref().map(SlackApp::new);
        Self { settings, slack }
    }

    pub fn settings(&self) -> &IntegrationSettings {
        &self.settings
    }

    pub fn slack_offer(&self) -> SlackOffer {
        match (&self.slack, &self.settings.secrets) {
            (None, _) => SlackOffer::NotSetUp,
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
}

/// The shared, replaceable [`Integrations`] (cloning shares it).
#[derive(Clone)]
pub struct IntegrationsHandle(Arc<RwLock<Arc<Integrations>>>);

impl IntegrationsHandle {
    pub fn new(integrations: Integrations) -> Self {
        Self(Arc::new(RwLock::new(Arc::new(integrations))))
    }

    /// No Slack app, no secrets key: integrations are not offered.
    pub fn off(public_url: &str) -> Self {
        Self::new(Integrations::new(IntegrationSettings::off(public_url)))
    }

    /// The integrations as they are now.
    pub fn current(&self) -> Arc<Integrations> {
        match self.0.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// Replace the integrations every later request reads.
    pub fn install(&self, integrations: Integrations) {
        let next = Arc::new(integrations);
        match self.0.write() {
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
