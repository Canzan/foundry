//! The Foundry Slack app an instance admin saves (ADR-IIS-003): the slots
//! its two secrets are sealed in, the save plan the one write use-case
//! applies, and the app in effect: the environment's when all three
//! `SLACK_*` are set, else the saved app when both its secrets open, else
//! none ([`app_in_effect`], pure and total).
//!
//! A submission is judged against that app (US-IIS-05): naming its client ID
//! is a rotation of its secrets, where a field left blank keeps the secret
//! the saved app already holds; naming another client ID is another app,
//! which is accepted only with both of its secrets ([`SlackSubmission::judge`],
//! pure and total). Only its verdict can be sealed, and every sealing of a
//! Slack secret - a kept one included - happens here.

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

/// The client ID of the app in effect: the `environment`'s where its trio is
/// set, else the `saved` app's where it is the one in effect, else none. It
/// is the app the page names in I-7 or I-6, so the replace form, the verdict
/// and the page all read one app.
pub fn client_id_in_effect<'app>(
    environment: Option<&'app str>,
    saved: Option<&'app str>,
) -> Option<&'app str> {
    environment.or(saved)
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
pub struct SlackSubmission {
    pub client_id: String,
    pub client_secret: SecretString,
    pub signing_secret: SecretString,
}

impl SlackSubmission {
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

    /// The value the save seals for this field: the text submitted where it
    /// has this field's shape, or `kept` where the field was left blank and
    /// there is a secret to keep (ADR-IIS-003, US-IIS-05 AC 1). A blank
    /// field with nothing to keep is refused as blank (I-11), and a value
    /// off its shape as malformed (I-12).
    fn sealed_value(
        self,
        submitted: &str,
        kept: Option<&SecretString>,
    ) -> Result<SecretString, Refusal> {
        match kept.filter(|_| submitted.trim().is_empty()) {
            Some(secret) => Ok(secret.clone()),
            None => {
                self.check(submitted)?;
                Ok(SecretString::from(submitted.trim().to_string()))
            }
        }
    }
}

/// Which app a submission names, against the app in effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmissionKind {
    /// The client ID of the app in effect: a rotation of that app's own
    /// secrets, where a field left blank keeps the secret already saved.
    Rotate,
    /// Another client ID, or any client ID with no app in effect: another
    /// app, accepted only with both of its own secrets (DISCUSS D7).
    NewApp,
}

impl SubmissionKind {
    /// Which app a submission naming `client_id` names, against the client
    /// ID of the app in effect (`current`): its own ID is a rotation, any
    /// other ID - and any ID at all with no app in effect - is another app.
    fn of(client_id: &str, current: Option<&str>) -> Self {
        match current {
            Some(current) if current.trim() == client_id.trim() => Self::Rotate,
            _ => Self::NewApp,
        }
    }
}

/// The saved app's two secrets as the held keys opened them ([`open_saved`]):
/// which app they belong to, and the secret each field already holds. It is
/// what a field left blank keeps. No `Debug`: it carries two secrets in the
/// clear.
#[derive(Clone)]
pub struct SavedSecrets {
    client_id: String,
    client_secret: SecretString,
    signing_secret: SecretString,
}

impl SavedSecrets {
    /// The secrets of the saved app, as [`open_saved`] opened it.
    pub fn opened(saved: &SlackSettings) -> Self {
        Self {
            client_id: saved.client_id.clone(),
            client_secret: saved.client_secret.clone(),
            signing_secret: saved.signing_secret.clone(),
        }
    }

    /// Whether these are the secrets of the app `client_id` names.
    fn belong_to(&self, client_id: &str) -> bool {
        self.client_id == client_id.trim()
    }

    /// The secret this field already holds; none for the client ID, which is
    /// no secret.
    fn secret(&self, field: SlackField) -> Option<&SecretString> {
        match field {
            SlackField::ClientId => None,
            SlackField::ClientSecret => Some(&self.client_secret),
            SlackField::SigningSecret => Some(&self.signing_secret),
        }
    }
}

/// What a field left blank keeps: the secret the saved app already holds,
/// where the submission rotates the app in effect and the saved secrets are
/// that same app's. Nothing otherwise, so a blank field is refused with I-11
/// because there is nothing to keep: the saved secrets are the saved app's,
/// and keeping them under another app's client ID would leave every event
/// and every code exchange failing.
struct Kept<'saved> {
    secrets: Option<&'saved SavedSecrets>,
}

impl<'saved> Kept<'saved> {
    /// Nothing is kept: every field answers for itself.
    fn nothing() -> Self {
        Self { secrets: None }
    }

    /// What a blank field of a `kind` submission naming `client_id` keeps,
    /// over the saved app's opened `secrets`.
    fn over(kind: SubmissionKind, client_id: &str, secrets: Option<&'saved SavedSecrets>) -> Self {
        Self {
            secrets: secrets
                .filter(|saved| kind == SubmissionKind::Rotate && saved.belong_to(client_id)),
        }
    }

    /// The secret this field keeps, when there is one.
    fn secret(&self, field: SlackField) -> Option<&SecretString> {
        self.secrets.and_then(|saved| saved.secret(field))
    }
}

/// An accepted submission: which app it names, and for each secret the value
/// the save seals - the one submitted, or the one the saved app already holds
/// where the field was left blank. Built only by
/// [`SlackSubmission::judge`], so nothing is sealed without that verdict. No
/// `Debug`: it carries two secrets in the clear.
pub struct SlackChange {
    kind: SubmissionKind,
    client_id: String,
    client_secret: SecretString,
    signing_secret: SecretString,
}

impl SlackChange {
    /// Which app this change names.
    pub fn kind(&self) -> SubmissionKind {
        self.kind
    }

    /// The app's client ID, as the save stores it.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
}

/// Each Slack secret's length in hex characters.
const SECRET_LEN: usize = 32;

fn digits(part: &str) -> bool {
    !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
}

/// A submission's three values, each paired with the field it was typed in,
/// in form order: the one place that knows which value answers for which
/// field, so every question about the submission is asked one named field at
/// a time. Borrows the submission, and is never printed: it holds two
/// secrets in the clear.
pub struct SlackFields<'submission> {
    in_form_order: [(SlackField, &'submission str); 3],
}

impl SlackFields<'_> {
    /// What the save refuses: the first field, in form order, whose value is
    /// blank (I-11) or off its shape (I-12); `None` when every field has its
    /// shape. It is [`Self::secrets_to_seal`] with nothing kept, so one rule
    /// answers for both questions.
    pub fn refusal(&self) -> Option<Refusal> {
        self.secrets_to_seal(&Kept::nothing()).err()
    }

    /// The two secrets the save seals, in form order, where a field left
    /// blank keeps the secret `kept` holds for it: the client ID answers
    /// first, then the client secret, then the signing secret, so a refusal
    /// names the first failing field in form order.
    fn secrets_to_seal(&self, kept: &Kept<'_>) -> Result<(SecretString, SecretString), Refusal> {
        let [(id, client_id), (first, first_value), (second, second_value)] = self.in_form_order;
        id.check(client_id)?;
        Ok((
            first.sealed_value(first_value, kept.secret(first))?,
            second.sealed_value(second_value, kept.secret(second))?,
        ))
    }
}

impl SlackSubmission {
    /// This submission's values under their field names, in form order.
    pub fn fields(&self) -> SlackFields<'_> {
        SlackFields {
            in_form_order: [
                (SlackField::ClientId, &self.client_id),
                (SlackField::ClientSecret, self.client_secret.expose_secret()),
                (
                    SlackField::SigningSecret,
                    self.signing_secret.expose_secret(),
                ),
            ],
        }
    }

    /// The save refused, naming the first failing field in form order;
    /// `None` when every field has its shape. Pure: no call to Slack.
    pub fn refusal(&self) -> Option<Refusal> {
        self.fields().refusal()
    }

    /// The verdict over the client ID of the app in effect (`current`) and
    /// the saved app's opened secrets (`saved`): which app this submission
    /// names, and the two secrets the save seals, a field left blank keeping
    /// the secret the saved app already holds where there is one
    /// (US-IIS-05 AC 1). Refused on the first failing field in form order
    /// (I-11, I-12). Pure and total: no call to Slack and no store read.
    pub fn judge(
        &self,
        current: Option<&str>,
        saved: Option<&SavedSecrets>,
    ) -> Result<SlackChange, Refusal> {
        let kind = SubmissionKind::of(&self.client_id, current);
        let (client_secret, signing_secret) =
            self.fields()
                .secrets_to_seal(&Kept::over(kind, &self.client_id, saved))?;
        Ok(SlackChange {
            kind,
            client_id: self.client_id.clone(),
            client_secret,
            signing_secret,
        })
    }
}

/// Both of the change's secrets sealed with `sealing`, fenced on its key id
/// when `mode` is `Sealed` (the stored secrets key's) and on none when
/// `Direct`. A secret a blank field kept is re-sealed here like a submitted
/// one, so every sealing of a Slack secret stays in this module. `None` when
/// sealing fails.
pub fn plan_replace(
    sealing: &SecretBox,
    mode: KeyMode,
    change: &SlackChange,
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
        client_id: change.client_id.clone(),
        sealed_client_secret: seal(&change.client_secret, client_secret_slot())?,
        sealed_signing_secret: seal(&change.signing_secret, signing_secret_slot())?,
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

    fn form(client_id: &str, client_secret: &str, signing_secret: &str) -> SlackSubmission {
        let field = |name: &str, value: &str| (name.to_string(), value.to_string());
        SlackSubmission::from_fields(&[
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

    /// A save seals both secrets into their own slots, fenced on the sealing
    /// key's id in `Sealed` mode and on none in `Direct`.
    #[test]
    fn a_save_seals_both_secrets_fenced_only_when_sealed() {
        let generated = foundry_auth::secret_box::SecretsKey::generate().unwrap();
        let sealing = SecretBox::new(generated.key, Vec::new());
        let by = Uuid::now_v7();
        let at = OffsetDateTime::UNIX_EPOCH;
        let form = form(ID, SECRET, SIGNING);
        let open = |sealed: &[u8], slot: SecretSlot| {
            sealing
                .open(
                    &foundry_auth::secret_box::SealedSecret::from_bytes(sealed.to_vec()),
                    &slot,
                )
                .map(|text| text.expose_secret().to_string())
                .ok()
        };
        let change = form.judge(None, None).expect("both secrets submitted");
        let sealed = plan_replace(&sealing, KeyMode::Sealed, &change, by, at).expect("planned");
        assert_eq!(sealed.client_id, ID);
        assert_eq!(
            open(&sealed.sealed_client_secret, client_secret_slot()),
            Some(SECRET.to_string())
        );
        assert_eq!(
            open(&sealed.sealed_signing_secret, signing_secret_slot()),
            Some(SIGNING.to_string())
        );
        assert_eq!(
            sealed.sealed_under,
            Some(sealing.current_key_id().to_string())
        );
        assert_eq!((sealed.saved_by, sealed.saved_at), (by, at));
        let direct = plan_replace(&sealing, KeyMode::Direct, &change, by, at).expect("planned");
        assert_eq!(direct.sealed_under, None);
    }
}
