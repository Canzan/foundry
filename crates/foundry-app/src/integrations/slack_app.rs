//! The Foundry Slack app an instance admin saves (ADR-IIS-003): the slots
//! its two secrets are sealed in, the save plan the one write use-case
//! applies, and the app in effect: the environment's when all three
//! `SLACK_*` are set, else the saved app when both its secrets open, else
//! none ([`app_in_effect`], pure and total).

use super::config::{SlackSettings, DEFAULT_SLACK_API_BASE_URL, DEFAULT_SLACK_AUTHORIZE_URL};
use super::keys::{CredentialBox, KeyMode};
use foundry_auth::secret_box::{SealedSecret, SecretBox, SecretSlot};
use foundry_store::integrations::{SavedSlackApp, SlackAppSave};
use secrecy::SecretString;
use time::OffsetDateTime;
use uuid::Uuid;

/// The row both secrets are bound to.
const TABLE: &str = "instance_slack_app";
const ROW: &str = "1";

/// The slot the client secret is sealed in.
pub fn client_secret_slot() -> SecretSlot {
    SecretSlot::new(TABLE, ROW, "sealed_client_secret")
}

/// The slot the signing secret is sealed in.
pub fn signing_secret_slot() -> SecretSlot {
    SecretSlot::new(TABLE, ROW, "sealed_signing_secret")
}

/// The saved app, as far as the held keys go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavedApp {
    Absent,
    /// Both secrets open.
    Opens,
    /// A secret opens under no held key (I-13): not in effect.
    DoesNotOpen,
}

/// Which Slack app is in effect on this server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppInEffect {
    Environment,
    Saved,
    None,
}

/// The app in effect when the environment sets (or not) all three
/// `SLACK_*`, over the saved app.
pub fn app_in_effect(environment: bool, saved: SavedApp) -> AppInEffect {
    match (environment, saved) {
        (true, _) => AppInEffect::Environment,
        (false, SavedApp::Opens) => AppInEffect::Saved,
        (false, SavedApp::Absent | SavedApp::DoesNotOpen) => AppInEffect::None,
    }
}

/// What the instance page says of the saved app in effect (I-6). Never a
/// secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedAppFacts {
    pub client_id: String,
    pub saved_by: Option<Uuid>,
    pub saved_at: OffsetDateTime,
}

impl From<SavedSlackApp> for SavedAppFacts {
    fn from(app: SavedSlackApp) -> Self {
        Self {
            client_id: app.client_id,
            saved_by: app.saved_by,
            saved_at: app.saved_at,
        }
    }
}

/// The saved app opened under `credentials`: its settings when both secrets
/// open (Slack's default addresses), else `None`.
pub fn open_saved(credentials: &CredentialBox<'_>, app: &SavedSlackApp) -> Option<SlackSettings> {
    let open = |sealed: &[u8], slot: SecretSlot| {
        credentials.open(&SealedSecret::from_bytes(sealed.to_vec()), &slot)
    };
    Some(SlackSettings {
        client_id: app.client_id.clone(),
        client_secret: open(&app.sealed_client_secret, client_secret_slot())?,
        signing_secret: open(&app.sealed_signing_secret, signing_secret_slot())?,
        api_base_url: DEFAULT_SLACK_API_BASE_URL.to_string(),
        authorize_url: DEFAULT_SLACK_AUTHORIZE_URL.to_string(),
    })
}

/// The app as the form submitted it (each value trimmed).
pub struct SlackAppForm {
    pub client_id: String,
    pub client_secret: SecretString,
    pub signing_secret: SecretString,
}

impl SlackAppForm {
    /// The form's three fields, each trimmed; a missing one reads empty.
    pub fn from_fields(fields: &[(String, String)]) -> Self {
        let field = |name: &str| {
            fields
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.trim().to_string())
                .unwrap_or_default()
        };
        Self {
            client_id: field("client_id"),
            client_secret: SecretString::from(field("client_secret")),
            signing_secret: SecretString::from(field("signing_secret")),
        }
    }
}

/// Both secrets sealed with `sealing`, fenced on its key id when `mode` is
/// `Sealed` (the stored secrets key's) and on none when `Direct`. `None`
/// when sealing fails.
pub fn plan_save(
    sealing: &SecretBox,
    mode: KeyMode,
    form: &SlackAppForm,
    saved_by: Uuid,
    saved_at: OffsetDateTime,
) -> Option<SlackAppSave> {
    let seal = |secret: &SecretString, slot: SecretSlot| {
        sealing
            .seal(secret, &slot)
            .ok()
            .map(|sealed| sealed.as_bytes().to_vec())
    };
    Some(SlackAppSave {
        client_id: form.client_id.clone(),
        sealed_client_secret: seal(&form.client_secret, client_secret_slot())?,
        sealed_signing_secret: seal(&form.signing_secret, signing_secret_slot())?,
        sealed_under: (mode == KeyMode::Sealed).then(|| sealing.current_key_id().to_string()),
        saved_by,
        saved_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every (environment trio set, saved row) pair: the environment wins,
    /// then a saved app whose secrets both open, else none.
    #[test]
    fn the_app_in_effect_is_the_environments_then_a_saved_app_that_opens() {
        use AppInEffect as E;
        use SavedApp as S;
        #[rustfmt::skip]
        let table = [
            (true,  S::Absent,      E::Environment),
            (true,  S::Opens,       E::Environment),
            (true,  S::DoesNotOpen, E::Environment),
            (false, S::Absent,      E::None),
            (false, S::Opens,       E::Saved),
            (false, S::DoesNotOpen, E::None),
        ];
        for (environment, saved, expected) in table {
            assert_eq!(
                app_in_effect(environment, saved),
                expected,
                "environment {environment}, saved {saved:?}"
            );
        }
    }
}
