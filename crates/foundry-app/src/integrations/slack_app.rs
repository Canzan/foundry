//! The Foundry Slack app an instance admin saves (ADR-IIS-003): the slots
//! its two secrets are sealed in, the save plan the one write use-case
//! applies, and the app in effect: the environment's when all three
//! `SLACK_*` are set, else the saved app when both its secrets open, else
//! none ([`app_in_effect`], pure and total).

use super::config::{SlackSettings, SlackUrls};
use super::keys::{CredentialBox, KeyMode};
use foundry_auth::secret_box::{SealedSecret, SecretBox, SecretSlot};
use foundry_store::integrations::{SavedSlackApp, SlackAppSave};
use secrecy::{ExposeSecret, SecretString};
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

impl SavedApp {
    /// A saved app by whether each of its two secrets opens.
    pub fn opened(client_secret: bool, signing_secret: bool) -> Self {
        match (client_secret, signing_secret) {
            (true, true) => Self::Opens,
            _ => Self::DoesNotOpen,
        }
    }
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

/// The saved app opened under `credentials`: whether both secrets open,
/// and when they do its settings, reaching Slack at `urls` (the same
/// addresses the environment's app would use).
pub fn open_saved(
    credentials: &CredentialBox<'_>,
    app: &SavedSlackApp,
    urls: &SlackUrls,
) -> (SavedApp, Option<SlackSettings>) {
    let open = |sealed: &[u8], slot: SecretSlot| {
        credentials.open(&SealedSecret::from_bytes(sealed.to_vec()), &slot)
    };
    let client_secret = open(&app.sealed_client_secret, client_secret_slot());
    let signing_secret = open(&app.sealed_signing_secret, signing_secret_slot());
    let state = SavedApp::opened(client_secret.is_some(), signing_secret.is_some());
    let settings = client_secret
        .zip(signing_secret)
        .map(|(client_secret, signing_secret)| SlackSettings {
            client_id: app.client_id.clone(),
            client_secret,
            signing_secret,
            api_base_url: urls.api_base_url.clone(),
            authorize_url: urls.authorize_url.clone(),
        });
    (state, settings)
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

/// A field of the Slack app form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlackField {
    ClientId,
    ClientSecret,
    SigningSecret,
}

/// Why a save is refused: the first field in form order that is blank
/// (I-11) or off its shape (I-12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Blank(SlackField),
    Malformed(SlackField),
}

impl SlackField {
    /// Whether `value`, trimmed, is this field's shape (ADR-IIS-003): the
    /// client ID digits, a dot and digits; each secret 32 lowercase hex.
    pub fn check(self, value: &str) -> Result<(), Refusal> {
        let value = value.trim();
        if value.is_empty() {
            return Err(Refusal::Blank(self));
        }
        let fits = match self {
            Self::ClientId => value
                .split_once('.')
                .is_some_and(|(left, right)| digits(left) && digits(right)),
            Self::ClientSecret | Self::SigningSecret => {
                value.len() == SECRET_LEN
                    && value
                        .bytes()
                        .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            }
        };
        fits.then_some(()).ok_or(Refusal::Malformed(self))
    }
}

/// Each Slack secret's length in hex characters.
const SECRET_LEN: usize = 32;

fn digits(part: &str) -> bool {
    !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
}

impl SlackAppForm {
    /// The save refused, naming the first failing field in form order;
    /// `None` when every field has its shape. Pure: no call to Slack.
    pub fn refusal(&self) -> Option<Refusal> {
        SlackField::ClientId
            .check(&self.client_id)
            .and_then(|()| SlackField::ClientSecret.check(self.client_secret.expose_secret()))
            .and_then(|()| SlackField::SigningSecret.check(self.signing_secret.expose_secret()))
            .err()
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
    use proptest::prelude::*;

    const ID: &str = "6123456789012.7234567890123";
    const SECRET: &str = "3f9c0a7e5b2d4c18a6e0f7b9d2c4e1a8";
    const SIGNING: &str = "b7e1d4c9a2f05e3b8c6d1a7f4e9b2c05";

    fn form(client_id: &str, client_secret: &str, signing_secret: &str) -> SlackAppForm {
        let field = |name: &str, value: &str| (name.to_string(), value.to_string());
        SlackAppForm::from_fields(&[
            field("client_id", client_id),
            field("client_secret", client_secret),
            field("signing_secret", signing_secret),
        ])
    }

    /// Each field's blank, malformed and valid cases, in form order; the
    /// first failing field is the one named.
    #[test]
    fn a_save_is_refused_on_the_first_blank_or_malformed_field_in_form_order() {
        use Refusal::{Blank, Malformed};
        use SlackField::{ClientId, ClientSecret, SigningSecret};
        #[rustfmt::skip]
        let table: [(&str, &str, &str, Option<Refusal>); 14] = [
            (ID,                    SECRET,     SIGNING,     None),
            ("",                    SECRET,     SIGNING,     Some(Blank(ClientId))),
            ("   ",                 SECRET,     SIGNING,     Some(Blank(ClientId))),
            ("6123456789012",       SECRET,     SIGNING,     Some(Malformed(ClientId))),
            ("6123456789012.",      SECRET,     SIGNING,     Some(Malformed(ClientId))),
            (".7234567890123",      SECRET,     SIGNING,     Some(Malformed(ClientId))),
            ("1.2.3",               SECRET,     SIGNING,     Some(Malformed(ClientId))),
            ("6123 copy",           "",         "",          Some(Malformed(ClientId))),
            (ID,                    "",         SIGNING,     Some(Blank(ClientSecret))),
            (ID,                    "3f9c0a7e", SIGNING,     Some(Malformed(ClientSecret))),
            (ID,                    &SECRET.to_uppercase(), SIGNING, Some(Malformed(ClientSecret))),
            (ID,                    "x",        "",          Some(Malformed(ClientSecret))),
            (ID,                    SECRET,     "",          Some(Blank(SigningSecret))),
            (ID,                    SECRET,     "b7e1 d4c9", Some(Malformed(SigningSecret))),
        ];
        for (client_id, client_secret, signing_secret, expected) in table {
            assert_eq!(
                form(client_id, client_secret, signing_secret).refusal(),
                expected,
                "client ID {client_id:?}, row expecting {expected:?}"
            );
        }
    }

    fn any_field() -> impl Strategy<Value = SlackField> {
        prop_oneof![
            Just(SlackField::ClientId),
            Just(SlackField::ClientSecret),
            Just(SlackField::SigningSecret),
        ]
    }

    /// The characters a field's value is made of.
    fn in_alphabet(field: SlackField, c: char) -> bool {
        match field {
            SlackField::ClientId => c.is_ascii_digit() || c == '.',
            SlackField::ClientSecret | SlackField::SigningSecret => {
                matches!(c, '0'..='9' | 'a'..='f')
            }
        }
    }

    proptest! {
        /// Surrounding whitespace never changes a value's verdict.
        #[test]
        fn a_value_is_judged_the_same_with_surrounding_whitespace(
            field in any_field(),
            value in "[0-9a-f.]{0,40}|\\PC{0,40}",
            before in "[ \t\r\n]{0,4}",
            after in "[ \t\r\n]{0,4}",
        ) {
            prop_assert_eq!(field.check(&format!("{before}{value}{after}")), field.check(&value));
        }

        /// Any 32 lowercase hex characters are a secret.
        #[test]
        fn any_32_lowercase_hex_is_accepted_as_a_secret(secret in "[0-9a-f]{32}") {
            prop_assert_eq!(SlackField::ClientSecret.check(&secret), Ok(()));
            prop_assert_eq!(SlackField::SigningSecret.check(&secret), Ok(()));
            prop_assert_eq!(form(ID, &secret, &secret).refusal(), None);
        }

        /// A value holding a character outside its field's alphabet is
        /// malformed (I-12), never blank.
        #[test]
        fn a_character_outside_the_alphabet_is_refused_as_malformed(
            field in any_field(),
            head in "[0-9a-f.]{0,20}",
            foreign in any::<char>(),
            tail in "[0-9a-f.]{0,20}",
        ) {
            prop_assume!(!in_alphabet(field, foreign) && !foreign.is_whitespace());
            let value = format!("{head}{foreign}{tail}");
            prop_assert_eq!(field.check(&value), Err(Refusal::Malformed(field)));
        }
    }

    /// Every (environment trio set, saved row) pair, the saved row by
    /// whether its client and signing secrets open: the environment wins,
    /// then a saved app whose secrets both open, else none.
    #[test]
    fn the_app_in_effect_is_the_environments_then_a_saved_app_that_opens() {
        use AppInEffect as E;
        #[rustfmt::skip]
        let table = [
            // environment, saved (client secret opens, signing secret opens)
            (true,  None,                E::Environment),
            (true,  Some((true,  true)),  E::Environment),
            (true,  Some((false, true)),  E::Environment),
            (true,  Some((true,  false)), E::Environment),
            (true,  Some((false, false)), E::Environment),
            (false, None,                E::None),
            (false, Some((true,  true)),  E::Saved),
            (false, Some((false, true)),  E::None),
            (false, Some((true,  false)), E::None),
            (false, Some((false, false)), E::None),
        ];
        for (environment, saved, expected) in table {
            let saved = saved.map_or(SavedApp::Absent, |(client, signing)| {
                SavedApp::opened(client, signing)
            });
            assert_eq!(
                app_in_effect(environment, saved),
                expected,
                "environment {environment}, saved {saved:?}"
            );
        }
    }
}
