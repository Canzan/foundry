//! external-integration-plugins — foundry-app's integrations module (DESIGN
//! [REF] Component Decomposition slices 01 and 04): settings ([`config`]), the
//! Slack adapter ([`slack`]), the Matrix adapter ([`matrix`]), the client for
//! user-supplied addresses ([`outbound`]), the doors ([`handlers`]), why a
//! route door has no connection ([`no_connection`]) and members' project
//! emails ([`project_emails`]); the key mode and the credential box
//! ([`keys`]), which [`Integrations::boot`], the one boot function, resolves.
//!
//! [`Integrations`] is what the settings built; [`IntegrationsHandle`] is how
//! [`crate::AppState`] holds it. Production installs it once at boot; the
//! acceptance harness installs a scenario's settings after the app started
//! (`rewire`, DISTILL DIST-4), so handlers read the current value per request.

pub mod config;
pub mod handlers;
pub mod key_file;
pub mod keys;
pub mod matrix;
pub mod no_connection;
pub mod outbound;
pub mod project_emails;
pub mod reload;
#[cfg(feature = "test-support")]
pub mod secrets_key_seam;
pub mod slack;
pub mod slack_app;
pub mod store_capabilities;
pub mod worker;

use crate::rate_limit::RevokeRateLimiter;
use config::IntegrationSettings;
use foundry_auth::secret_box::{SealedSecret, SecretBox, SecretSlot};
use foundry_integrations::ChannelKind;
use foundry_services::integrations::{DestinationCatalogs, UnreadableConnections};
use foundry_store::integrations::{
    InstanceSnapshot, KeyCheck, SavedSlackApp, SealedConnection, SealedSlot,
};
use keys::{CredentialBox, InstanceKeySource, KeyFile, KeyFileRead, KeyMode, Keyring};
use outbound::OutboundHttp;
use project_emails::ProjectMail;
use secrecy::{ExposeSecret, SecretString};
use slack::{SlackApp, SlackBot, SlackClientError};
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
    /// K-19: this Foundry cannot read the workspace's saved Slack connection
    /// (DD-2, judged per connection).
    Unavailable,
    Ready,
}

/// What `/workspace/integrations` may offer for Matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixOffer {
    /// K-18: no secrets key, so no bot token can be sealed (DD-6).
    NoSecretsKey,
    /// K-19: this Foundry cannot read the workspace's saved Matrix
    /// connection (DD-2, judged per connection), with a key or without.
    Unavailable,
    Ready,
}

/// A chat route's kind as stored or submitted: Matrix, else Slack.
pub(crate) fn chat_kind(wire: &str) -> ChannelKind {
    match ChannelKind::from_wire_name(wire) {
        Some(ChannelKind::Matrix) => ChannelKind::Matrix,
        _ => ChannelKind::Slack,
    }
}

/// Hand the integrations the composition root's notifier (DDR-23: a lost
/// connection's admins are told through it; ADR-EIP-007 decision 4: members'
/// project emails go through its first `smtp` or `email_api` provider, none
/// and they are off, K-13) and `SESSION_SECRET` (decision 5: every project
/// email's signed stop link), then spawn the delivery worker (ADR-EIP-002)
/// and, beside it in the same task, this replica's reload (ADR-IIS-004 r4):
/// stopping the task stops both. `main.rs` and the acceptance harness both
/// start delivery and reload here.
pub fn start(state: &crate::AppState) -> tokio::task::JoinHandle<()> {
    let integrations = &state.integrations;
    integrations
        .admin_mail()
        .use_notifier(state.notifier.clone());
    integrations
        .project_mail()
        .use_transport(state.notifier.email_transport());
    integrations
        .project_mail()
        .sign_stop_links_with(state.session_secret.clone());
    let capabilities = store_capabilities::StoreCapabilities::new(state.store.clone());
    let worker = worker::IntegrationsWorker::new(
        capabilities.clone(),
        integrations.clone(),
        state.clock.clone(),
    );
    let reload = reload::Reload::new(capabilities, integrations.clone(), state.clock.clone());
    tokio::spawn(async move {
        tokio::join!(worker.run(), reload.run());
    })
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

/// The row a stored sealed value is bound to, as a Generate or Move is
/// handed it.
pub fn sealed_slot(slot: SealedSlot) -> SecretSlot {
    match slot {
        SealedSlot::Connection(connection_id) => credential_slot(connection_id),
        SealedSlot::SlackClientSecret => slack_app::client_secret_slot(),
        SealedSlot::SlackSigningSecret => slack_app::signing_secret_slot(),
    }
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
    /// Foundry does not hold, or holds under an id whose stored check value
    /// does not open to the canary (never a key). A held key with no check
    /// value stored counts as proven, by design.
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
    /// Shared with the integrations a reload rebuilds from them.
    settings: Arc<IntegrationSettings>,
    /// The Slack app in effect (ADR-IIS-003 decision 2).
    slack: Option<Arc<SlackApp>>,
    /// The saved app's facts, when it is the app in effect.
    saved_slack: Option<slack_app::SavedAppFacts>,
    /// The saved app's secrets as the held keys opened them, whichever app
    /// is in effect: what a replace's blank field keeps (US-IIS-05 AC 1).
    saved_secrets: Option<slack_app::SavedSecrets>,
    /// The saved app as the held keys open it (I-13 when it does not).
    saved_app: slack_app::SavedApp,
    /// The key file as boot found it (consulted only without an operator key).
    key_file: KeyFile,
    /// The key file the stored secrets key was re-wrapped from (I-30).
    rewrapped_from: Option<String>,
    /// Built on first use, so a test-support build reaches a test host
    /// admitted after these integrations were installed.
    outbound: std::sync::OnceLock<OutboundHttp>,
    secrets: SecretsCheck,
    keyring: Keyring,
    /// The config version of the stored rows these were judged over; `None`
    /// before any were read, or when the read failed.
    applied_version: Option<i64>,
    /// Woken when a sealed value names a key id this server does not hold:
    /// this replica's reload then runs at once (ADR-IIS-004, rate-limited
    /// there). Shared with every rebuild.
    reload_nudge: Arc<tokio::sync::Notify>,
}

impl Integrations {
    /// The integrations of `settings`; refused when the Slack app's client
    /// cannot be built, and startup stops.
    pub fn try_new(settings: IntegrationSettings) -> Result<Self, SlackClientError> {
        let slack = settings.slack.as_ref().map(SlackApp::build).transpose()?;
        Ok(Self::assembled(settings, slack.map(Arc::new)))
    }

    /// [`Self::try_new`] for tests, whose clients always build.
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(settings: IntegrationSettings) -> Self {
        Self::try_new(settings).expect("the Slack app's client builds")
    }

    /// Integrations judged over no stored rows (the boot function judges
    /// them over the stored ones).
    fn assembled(settings: IntegrationSettings, slack: Option<Arc<SlackApp>>) -> Self {
        Self::assembled_from(Arc::new(settings), slack)
    }

    /// The one boot function (ADR-IIS-001 decision 9), shared by `main.rs`
    /// and the acceptance harness's boot, rewire and restart: the held
    /// instance keys (the operator's, else the key file's), the current
    /// instance key's check value recorded, the stored rows read, the key
    /// mode resolved, then the probe: the check values of the held keys
    /// opened, and the key ids of every stored sealed value (connections and
    /// the saved Slack app) compared with them. A store failure never stops
    /// the board: the server starts degraded and says so. Refused only when
    /// the Slack app's client cannot be built.
    ///
    /// With an operator key, a readable key file's key is held as a previous
    /// instance key (never written or removed), and a secrets key the file
    /// wrapped is re-wrapped under the operator's key at `now`
    /// (ADR-IIS-002 r5, I-30).
    pub async fn boot(
        mut settings: IntegrationSettings,
        store: &dyn store_capabilities::KeyProbe,
        key_source: &dyn InstanceKeySource,
        now: time::OffsetDateTime,
    ) -> Result<Self, SlackClientError> {
        let snapshot = store.instance_snapshot().await;
        let mut rewrap_tried = false;
        // With an operator key, the key file's state does not enter the mode.
        // Without one, it is made only over a database holding nothing an
        // earlier key sealed (ADR-IIS-002 r3).
        let key_file = if let Some(operator) = settings.secrets.take() {
            let held_file = match key_source.key_file(false) {
                KeyFileRead::Held(key) => Some(*key),
                _ => None,
            };
            let superseded = held_file.is_some();
            settings.secrets = Some(match held_file {
                Some(key) => operator.with_previous(key),
                None => operator,
            });
            if superseded {
                rewrap_tried = rewrap_file_key(&settings, store, snapshot.as_ref().ok(), now).await;
            }
            KeyFile::Unwritable
        } else {
            // Decided over the snapshot read above, not under the instance
            // lock (a TOCTOU window): a value another server seals under
            // another key and commits between that read and the file's link
            // is not seen, and the file is made over it anyway. That value
            // then names a key this server does not hold, which the probe
            // reports as it does any key not held. The window is one boot
            // wide, on a database that held nothing sealed.
            let may_make = snapshot.as_ref().is_ok_and(|snapshot| {
                snapshot.secrets_key.is_none() && sealed_values(snapshot).is_empty()
            });
            match key_source.key_file(may_make) {
                KeyFileRead::Unwritable => KeyFile::Unwritable,
                KeyFileRead::Unreadable => KeyFile::Unreadable,
                KeyFileRead::Withheld => KeyFile::Withheld,
                KeyFileRead::Held(key) => {
                    settings.secrets = Some(SecretBox::new(*key, Vec::new()));
                    KeyFile::Held
                }
            }
        };
        let integrations = Self {
            key_file,
            ..Self::try_new(settings)?
        };
        let recorded = match integrations.key_check() {
            Some(check) => match store.record_key_check(&check).await {
                Ok(()) => Some(check),
                Err(err) => {
                    tracing::warn!(error = %err, "secrets probe: recording the key check failed");
                    None
                }
            },
            None => None,
        };
        let judged = match snapshot {
            Ok(snapshot) if !rewrap_tried => Ok(with_key_check(snapshot, recorded)),
            // A re-wrap moved the stored row on (or lost to a sibling's), and
            // a failed read gets its second chance: read the rows again.
            _ => store.instance_snapshot().await,
        };
        Ok(integrations.rebuilt(judged))
    }

    /// The rebuild over the stored rows as they are now: what the serving
    /// replica installs after an instance write commits, or after the seal
    /// fence found a newer key. Boot rebuilds over the rows it already read.
    pub async fn reboot(&self, store: &dyn store_capabilities::KeyProbe) -> Self {
        self.rebuilt(store.instance_snapshot().await)
    }

    /// The one rebuild every path shares (ADR-IIS-001 decision 9, ADR-IIS-004
    /// r4): boot, the serving replica's apply and every replica's reload.
    /// From these integrations' settings, held keys, Slack app and key file
    /// (never read again), over `snapshot`: the key mode resolved, keeping
    /// the held secrets key while its stored id is unchanged (r6), the stored
    /// key ids judged, the verdict logged.
    pub fn rebuilt(&self, snapshot: Result<InstanceSnapshot, foundry_store::StoreError>) -> Self {
        let mut integrations = Self::assembled_from(self.settings.clone(), self.slack.clone());
        integrations.key_file = self.key_file;
        integrations.rewrapped_from = self.rewrapped_from.clone();
        integrations.saved_slack = self.saved_slack.clone();
        integrations.saved_secrets = self.saved_secrets.clone();
        integrations.saved_app = self.saved_app;
        integrations.reload_nudge = self.reload_nudge.clone();
        integrations.probed(snapshot, self.keyring.secrets_key())
    }

    fn assembled_from(settings: Arc<IntegrationSettings>, slack: Option<Arc<SlackApp>>) -> Self {
        let keyring = Keyring::resolve(
            settings.secrets.as_ref(),
            settings.key_setting,
            KeyFile::Unwritable,
            None,
            &BTreeSet::new(),
        );
        Self {
            settings,
            slack,
            saved_slack: None,
            saved_secrets: None,
            saved_app: slack_app::SavedApp::Absent,
            key_file: KeyFile::Unwritable,
            rewrapped_from: None,
            outbound: std::sync::OnceLock::new(),
            secrets: SecretsCheck::default(),
            keyring,
            applied_version: None,
            reload_nudge: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// The boot's probe (ADR-IIS-001 decision 9) over the stored rows read:
    /// the key mode resolved and the stored key ids judged, the verdict logged.
    fn probed(
        self,
        snapshot: Result<InstanceSnapshot, foundry_store::StoreError>,
        held: Option<&Arc<SecretBox>>,
    ) -> Self {
        let key_file = self.key_file;
        let mut applied_version = None;
        let mut rewrapped_from = self.rewrapped_from.clone();
        let (keyring, secrets, saved) = match snapshot {
            Ok(snapshot) => {
                applied_version = Some(snapshot.version);
                rewrapped_from = snapshot
                    .secrets_key
                    .as_ref()
                    .and_then(|key| key.rewrapped_from_path.clone());
                let (keyring, secrets) = self.probe(&snapshot, key_file, held);
                let saved = snapshot.slack_app.map(|app| {
                    let (state, opened) = slack_app::open_saved(
                        &keyring.credentials(self.settings.secrets.as_ref()),
                        &app,
                        &self.settings.slack_urls,
                    );
                    (app, state, opened)
                });
                (keyring, secrets, Some(saved))
            }
            Err(err) => {
                tracing::warn!(error = %err, "secrets probe: reading stored credentials failed");
                let unknown = SecretsCheck {
                    missing_key_ids: vec!["unknown".to_string()],
                };
                (
                    self.keyring_over(None, &BTreeSet::new(), key_file),
                    unknown,
                    None,
                )
            }
        };
        let integrations = Self {
            keyring,
            applied_version,
            rewrapped_from,
            ..self
        }
        .with_secrets_check(secrets);
        let integrations = match saved {
            Some(saved) => integrations.with_saved_slack(saved),
            None => integrations,
        };
        integrations.log_secrets_verdict();
        integrations
    }

    /// The Slack app in effect over the stored `saved` app (as the held keys
    /// open it, with its settings when both secrets open): the
    /// environment's, else the saved one, else none. A saved app whose
    /// client cannot be built is not in effect.
    fn with_saved_slack(
        self,
        saved: Option<(
            SavedSlackApp,
            slack_app::SavedApp,
            Option<config::SlackSettings>,
        )>,
    ) -> Self {
        let environment = self.settings.slack.is_some();
        let saved_app = saved
            .as_ref()
            .map_or(slack_app::SavedApp::Absent, |(_, state, _)| *state);
        let saved_secrets = saved
            .as_ref()
            .and_then(|(_, _, opened)| opened.as_ref())
            .map(slack_app::SavedSecrets::opened);
        let in_effect = slack_app::app_in_effect(environment, saved_app);
        let (slack, saved_slack) = match (in_effect, saved) {
            (slack_app::AppInEffect::Environment, _) => (self.slack.clone(), None),
            (slack_app::AppInEffect::Saved, Some((app, _, Some(settings)))) => {
                match SlackApp::build(&settings) {
                    Ok(built) => (Some(Arc::new(built)), Some(app.into())),
                    Err(err) => {
                        tracing::warn!(error = %err, "the saved Slack app's client could not be built");
                        (None, None)
                    }
                }
            }
            _ => (None, None),
        };
        Self {
            slack,
            saved_slack,
            saved_secrets,
            saved_app,
            ..self
        }
    }

    /// The key file the stored secrets key was re-wrapped from (I-30).
    pub fn rewrapped_from(&self) -> Option<&str> {
        self.rewrapped_from.as_deref()
    }

    /// The saved Slack app as the held keys open it: absent, opening, or
    /// saved but unreadable here (I-13).
    pub fn saved_app(&self) -> slack_app::SavedApp {
        self.saved_app
    }

    /// The saved Slack app's facts, when it is the app in effect (I-6).
    pub fn saved_slack(&self) -> Option<&slack_app::SavedAppFacts> {
        self.saved_slack.as_ref()
    }

    /// The saved Slack app's secrets as the held keys opened them: the
    /// secrets a replace's blank field keeps, already opened here, so no
    /// other path opens a stored secret.
    pub fn saved_secrets(&self) -> Option<&slack_app::SavedSecrets> {
        self.saved_secrets.as_ref()
    }

    /// The client ID of the Slack app in effect on this server: the
    /// environment's where its trio is set, else the saved app's.
    pub fn client_id_in_effect(&self) -> Option<&str> {
        slack_app::client_id_in_effect(
            self.settings
                .slack
                .as_ref()
                .map(|app| app.client_id.as_str()),
            self.saved_slack.as_ref().map(|app| app.client_id.as_str()),
        )
    }

    /// The key mode and the probe's verdict over `snapshot`.
    fn probe(
        &self,
        snapshot: &InstanceSnapshot,
        key_file: KeyFile,
        held: Option<&Arc<SecretBox>>,
    ) -> (Keyring, SecretsCheck) {
        let sealed = sealed_values(snapshot);
        let keyring = Keyring::resolve_holding(
            held,
            self.settings.secrets.as_ref(),
            self.settings.key_setting,
            key_file,
            snapshot.secrets_key.as_ref(),
            &key_ids(&sealed),
        );
        let credentials = keyring.credentials(self.settings.secrets.as_ref());
        let verdict = judge(&credentials, &sealed, &snapshot.key_checks);
        (keyring, verdict)
    }

    fn keyring_over(
        &self,
        stored: Option<&foundry_store::integrations::StoredSecretsKey>,
        stored_key_ids: &BTreeSet<String>,
        key_file: KeyFile,
    ) -> Keyring {
        Keyring::resolve(
            self.settings.secrets.as_ref(),
            self.settings.key_setting,
            key_file,
            stored,
            stored_key_ids,
        )
    }

    /// The line for a server with no key and nothing stored. A degraded
    /// verdict is logged with the alert inputs ([`reload::raise_alert_inputs`]).
    fn log_secrets_verdict(&self) {
        if !self.secrets.degraded() && !self.credentials().holds_a_key() {
            tracing::info!(
                probe = "secrets",
                setting = foundry_auth::secret_box::INSTANCE_KEY_SETTING,
                "no instance key and no stored credential: integrations are not offered"
            );
        }
    }

    /// The boot probe's verdict.
    pub fn secrets_check(&self) -> &SecretsCheck {
        &self.secrets
    }

    /// The credential box: what this server seals with and opens under.
    pub fn credentials(&self) -> CredentialBox<'_> {
        self.keyring.credentials(self.settings.secrets.as_ref())
    }

    /// The box new credentials are sealed with: `None` unless the key mode
    /// is `Direct` or `Sealed` (the Matrix and Slack offers key off it).
    pub fn sealing_box(&self) -> Option<&SecretBox> {
        self.credentials().sealing()
    }

    /// The key id a connection write's seal fence compares (ADR-IIS-004
    /// rule 2): the sealing secrets key's in `Sealed` mode, none otherwise.
    pub fn sealed_under(&self) -> Option<String> {
        self.sealing_box()
            .filter(|_| self.key_mode() == KeyMode::Sealed)
            .map(|sealing| sealing.current_key_id().to_string())
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
        judge(&self.credentials(), credentials, checks)
    }

    pub fn settings(&self) -> &IntegrationSettings {
        &self.settings
    }

    /// The config version these integrations were judged over (ADR-IIS-004
    /// r8: a replica's `applied_version`).
    pub fn applied_version(&self) -> Option<i64> {
        self.applied_version
    }

    /// What the instance key file gave at boot (never read again).
    pub fn key_file(&self) -> KeyFile {
        self.key_file
    }

    /// Whether these integrations were judged over an older config version
    /// than `current`.
    fn judged_before(&self, current: &Self) -> bool {
        matches!(
            (self.applied_version, current.applied_version),
            (Some(next), Some(now)) if next < now
        )
    }

    /// The key mode the instance integrations page follows.
    pub fn key_mode(&self) -> KeyMode {
        self.keyring.mode()
    }

    /// How many of `snapshot`'s stored key ids are sealed directly under a
    /// held instance key (I-2e when some, I-2e0 when none), counted by key
    /// id with no stored flag. Only `Direct` has direct seals to move.
    pub fn direct_seals(&self, snapshot: &InstanceSnapshot) -> usize {
        if self.keyring.mode() != KeyMode::Direct {
            return 0;
        }
        let held: BTreeSet<String> = self
            .settings
            .secrets
            .as_ref()
            .map(|keys| keys.key_ids().iter().map(ToString::to_string).collect())
            .unwrap_or_default();
        keys::direct_seal_count(&key_ids(&sealed_values(snapshot)), &held)
    }

    /// What a workspace whose `unreadable` connections these are is offered
    /// for Matrix.
    pub fn matrix_offer(&self, unreadable: UnreadableConnections) -> MatrixOffer {
        match self.sealing_box() {
            _ if unreadable.matrix => MatrixOffer::Unavailable,
            None => MatrixOffer::NoSecretsKey,
            Some(_) => MatrixOffer::Ready,
        }
    }

    /// What a workspace whose `unreadable` connections these are is offered
    /// for Slack.
    pub fn slack_offer(&self, unreadable: UnreadableConnections) -> SlackOffer {
        match (&self.slack, self.sealing_box()) {
            (None, _) => SlackOffer::NotSetUp,
            (Some(_), _) if unreadable.slack => SlackOffer::Unavailable,
            (Some(_), None) => SlackOffer::NoSecretsKey,
            (Some(_), Some(_)) => SlackOffer::Ready,
        }
    }

    /// Whether the boot probe proved the key of every stored credential: then
    /// every connection opens, and none needs judging one by one (DD-2).
    pub fn every_credential_opens(&self) -> bool {
        self.credentials().holds_a_key() && !self.secrets.degraded()
    }

    /// Whether this Foundry's keys open `connection`'s credential.
    pub(crate) fn opens(&self, connection: &SealedConnection) -> bool {
        connection
            .sealed_credential
            .clone()
            .and_then(|sealed| self.open_credential(connection.id, sealed))
            .is_some()
    }

    pub(crate) fn slack(&self) -> Option<&SlackApp> {
        self.slack.as_deref()
    }

    /// Slack's fixed return address for this instance.
    pub fn slack_return_url(&self) -> String {
        format!("{}{}", self.settings.public_url, slack::RETURN_PATH)
    }

    /// Slack's events address for this instance.
    pub fn slack_events_url(&self) -> String {
        format!("{}{}", self.settings.public_url, slack::EVENTS_PATH)
    }

    /// Whether this process delivers: its worker is on and it can seal, or a
    /// held key opens a stored credential, so openable deliveries never stop
    /// (ADR-IIS-001 decision 4).
    pub fn runs_worker(&self) -> bool {
        self.settings.worker.enabled && (self.keyring.mode().seals() || self.keyring.opens_stored())
    }

    /// The connection's bot token, when this Foundry's keys open it for that
    /// row (ADR-EIP-004: the row-bound slot).
    /// A credential sealed under a key id this server does not hold nudges
    /// the reload: another replica may have just moved or replaced the key.
    fn open_credential(&self, connection_id: uuid::Uuid, sealed: Vec<u8>) -> Option<SecretString> {
        let sealed = SealedSecret::from_bytes(sealed);
        let credentials = self.credentials();
        let opened = credentials.open(&sealed, &credential_slot(connection_id));
        if opened.is_none() && !credentials.holds(&sealed.key_id().to_string()) {
            self.reload_nudge.notify_one();
        }
        opened
    }

    /// What this replica's reload waits on besides its period.
    pub fn reload_nudge(&self) -> Arc<tokio::sync::Notify> {
        self.reload_nudge.clone()
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
    pub(crate) fn matrix_bot(&self, connection: &SealedConnection) -> Option<matrix::MatrixBot> {
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

/// Every stored sealed value: the connections' credentials and the saved
/// Slack app's two secrets.
/// Boot's re-wrap (ADR-IIS-002 r5): a stored secrets key the key file
/// wrapped, opened under the file's key `settings` holds as a previous key,
/// wrapped again under the operator's key, compare-and-set. One replica wins
/// and records Foundry's line; the rest change nothing and adopt the row.
async fn rewrap_file_key(
    settings: &IntegrationSettings,
    store: &dyn store_capabilities::KeyProbe,
    snapshot: Option<&InstanceSnapshot>,
    now: time::OffsetDateTime,
) -> bool {
    use foundry_store::integrations::{KeyWrapper, RewrapOutcome};
    let Some((instance, stored)) = settings.secrets.as_ref().zip(
        snapshot
            .and_then(|snapshot| snapshot.secrets_key.as_ref())
            .filter(|stored| stored.wrapped_by == KeyWrapper::File),
    ) else {
        return false;
    };
    let path = key_file::key_path(&settings.data_dir).display().to_string();
    let Some((_, Some(rewrap))) = keys::plan_rewrap(instance, stored, Some(path)) else {
        return false;
    };
    match store.rewrap_secrets_key(&rewrap, now).await {
        Ok(RewrapOutcome::Rewrapped { version }) => tracing::info!(
            version,
            "secrets key re-wrapped under the operator's instance key"
        ),
        Ok(RewrapOutcome::AlreadyRewrapped) => {}
        Err(err) => tracing::warn!(error = %err, "re-wrapping the secrets key failed"),
    }
    true
}

/// `snapshot` as the store holds it once boot's `recorded` key check is in:
/// the insert keeps a check already stored for that key id (`ON CONFLICT DO
/// NOTHING`) and bumps no version, so boot needs no second read for it.
fn with_key_check(mut snapshot: InstanceSnapshot, recorded: Option<KeyCheck>) -> InstanceSnapshot {
    if let Some(check) = recorded {
        if !snapshot
            .key_checks
            .iter()
            .any(|stored| stored.key_id == check.key_id)
        {
            snapshot.key_checks.push(check);
        }
    }
    snapshot
}

fn sealed_values(snapshot: &InstanceSnapshot) -> Vec<Vec<u8>> {
    let mut sealed = snapshot.sealed_credentials.clone();
    if let Some(app) = &snapshot.slack_app {
        sealed.push(app.sealed_client_secret.clone());
        sealed.push(app.sealed_signing_secret.clone());
    }
    sealed
}

/// The key ids the sealed values claim.
fn key_ids(sealed: &[Vec<u8>]) -> BTreeSet<String> {
    sealed
        .iter()
        .map(|value| SealedSecret::from_bytes(value.clone()).key_id().to_string())
        .collect()
}

/// The probe over `credentials`: each key that a stored sealed value needs
/// but this server lacks, or holds under an id whose stored check value does
/// not open to the canary, is missing.
fn judge(credentials: &CredentialBox<'_>, sealed: &[Vec<u8>], checks: &[KeyCheck]) -> SecretsCheck {
    let missing = key_ids(sealed)
        .into_iter()
        .filter(|key_id| !proves_key(credentials, key_id, checks))
        .collect();
    SecretsCheck {
        missing_key_ids: missing,
    }
}

/// Whether `credentials` hold the key `key_id` and its stored check value,
/// if any, opens to the canary.
fn proves_key(credentials: &CredentialBox<'_>, key_id: &str, checks: &[KeyCheck]) -> bool {
    credentials.holds(key_id)
        && checks
            .iter()
            .filter(|check| check.key_id == key_id)
            .all(|check| {
                credentials
                    .open(
                        &SealedSecret::from_bytes(check.sealed_canary.clone()),
                        &key_check_slot(key_id),
                    )
                    .is_some_and(|canary| canary.expose_secret() == KEY_CHECK_CANARY)
            })
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

/// A route's destination is re-checked through the bot of the connection's
/// kind (Resume, D-8): a Slack channel through the Slack bot, a Matrix room
/// through the Matrix bot.
impl DestinationCatalogs for Integrations {
    fn catalogue(
        &self,
        kind: &str,
        connection: &SealedConnection,
    ) -> Option<Box<dyn foundry_integrations::DestinationCatalog>> {
        match ChannelKind::from_wire_name(kind)? {
            ChannelKind::Matrix => Some(Box::new(self.matrix_bot(connection)?)),
            ChannelKind::Slack => {
                let sealed = connection.sealed_credential.clone()?;
                Some(Box::new(self.slack_bot(connection.id, sealed)?))
            }
            _ => None,
        }
    }

    fn readable(&self, connection: &SealedConnection) -> bool {
        self.opens(connection)
    }
}

/// The shared, replaceable [`Integrations`] (cloning shares it), and the
/// Send test limit that outlives a replacement.
#[derive(Clone)]
pub struct IntegrationsHandle {
    current: Arc<RwLock<Arc<Integrations>>>,
    test_messages: Arc<RevokeRateLimiter>,
    admin_mail: AdminMail,
    project_mail: ProjectMail,
}

impl IntegrationsHandle {
    pub fn new(integrations: Integrations) -> Self {
        Self {
            current: Arc::new(RwLock::new(Arc::new(integrations))),
            test_messages: Arc::new(RevokeRateLimiter::test_messages()),
            admin_mail: AdminMail::default(),
            project_mail: ProjectMail::default(),
        }
    }

    /// How workspace admins are told a connection was lost (DDR-23); the
    /// composition root hands it the shipped `Notifier`.
    pub fn admin_mail(&self) -> &AdminMail {
        &self.admin_mail
    }

    /// The transport members' project emails go through (ADR-EIP-007); the
    /// composition root hands it the notifier's.
    pub fn project_mail(&self) -> &ProjectMail {
        &self.project_mail
    }

    /// Send test's per-person limit (DDR-25, R-5).
    pub fn test_messages(&self) -> &RevokeRateLimiter {
        &self.test_messages
    }

    /// No Slack app, no secrets key: integrations are not offered.
    pub fn off(public_url: &str) -> Self {
        Self::new(Integrations::assembled(
            IntegrationSettings::off(public_url),
            None,
        ))
    }

    /// The integrations as they are now.
    pub fn current(&self) -> Arc<Integrations> {
        match self.current.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// Replace the integrations with `next` only if they are still `seen`
    /// (what `next` was rebuilt from): a reload never undoes an install that
    /// landed while it read. Whether `next` was installed.
    pub fn install_over(&self, seen: &Arc<Integrations>, next: Integrations) -> bool {
        self.swap(next, |current| Arc::ptr_eq(current, seen))
    }

    /// Replace the integrations every later request reads: the serving
    /// server's rebuild after its own write. Whether `next` was installed.
    pub fn install(&self, next: Integrations) -> bool {
        self.swap(next, |_| true)
    }

    /// The one install (ADR-IIS-004 r4): `next` replaces the current
    /// integrations when `may_replace` them and it was judged over no older
    /// config version than they were (a rebuild over no snapshot has no
    /// version and is never refused for one); the installed integrations
    /// then set the secrets alert inputs.
    fn swap(
        &self,
        next: Integrations,
        may_replace: impl FnOnce(&Arc<Integrations>) -> bool,
    ) -> bool {
        let mut guard = match self.current.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if !may_replace(&guard) || next.judged_before(&guard) {
            return false;
        }
        // Under the lock, so two racing installs set the inputs in the
        // order they installed.
        reload::raise_alert_inputs(&next);
        *guard = Arc::new(next);
        true
    }
}

impl Default for IntegrationsHandle {
    fn default() -> Self {
        Self::off("http://localhost")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::SlackSettings;

    /// The integrations of an instance with or without the Slack app and the
    /// secrets key.
    fn instance(slack_app: bool, secrets_key: bool) -> Integrations {
        let mut settings = IntegrationSettings::off("https://foundry.example.org");
        settings.slack = slack_app.then(|| SlackSettings {
            client_id: "client".to_string(),
            client_secret: SecretString::from("client-secret"),
            signing_secret: SecretString::from("signing-secret"),
            api_base_url: "https://slack.com/api".to_string(),
            authorize_url: "https://slack.com/oauth/v2/authorize".to_string(),
        });
        settings.key_setting = secrets_key.then_some(config::KeySetting::LegacySecretsKey);
        settings.secrets = secrets_key.then(|| {
            SecretBox::from_settings(
                &SecretString::from("ZWlwLXBoYXNlNS1yb3RhdGVkLW5ldy1rZXktMzJieXQ"),
                None,
            )
            .expect("the secrets key parses")
        });
        Integrations::new(settings)
    }

    /// `instance(false, true)` rebuilt over config `version`, holding
    /// `sealed` values.
    fn judged_over(version: i64, sealed: Vec<Vec<u8>>) -> Integrations {
        instance(false, true).rebuilt(Ok(InstanceSnapshot {
            version,
            secrets_key: None,
            slack_app: None,
            key_checks: Vec::new(),
            sealed_credentials: sealed,
        }))
    }

    /// A value sealed under a key this instance does not hold (I-4).
    fn sealed_elsewhere() -> Vec<u8> {
        let other = foundry_auth::secret_box::SecretsKey::generate().unwrap();
        SecretBox::new(other.key, Vec::new())
            .seal(
                &SecretString::from("xoxb-a-bot-token"),
                &foundry_auth::secret_box::SecretSlot::new(
                    "integration_connections",
                    "1",
                    "credential",
                ),
            )
            .expect("seal")
            .as_bytes()
            .to_vec()
    }

    /// H4: the serving server's post-write rebuild read before a reload
    /// installed a newer version never undoes it; the same version installs
    /// (a connection write bumps none, item 27).
    #[test]
    fn a_stale_post_write_rebuild_never_replaces_a_newer_install() {
        let handle = IntegrationsHandle::new(judged_over(4, Vec::new()));
        let seen = handle.current();
        assert!(
            handle.install_over(&seen, judged_over(6, Vec::new())),
            "the reload"
        );
        handle.install(judged_over(5, Vec::new()));
        assert_eq!(handle.current().applied_version(), Some(6), "older refused");
        let reload_saw = handle.current();
        handle.install(judged_over(6, vec![sealed_elsewhere()]));
        assert!(
            handle.current().secrets_check().degraded(),
            "the same version installs"
        );
        assert!(
            !handle.install_over(&handle.current(), judged_over(5, Vec::new())),
            "no install goes back a version"
        );
        assert!(!handle.install_over(&reload_saw, judged_over(7, Vec::new())));
        assert_eq!(handle.current().applied_version(), Some(6));
    }

    /// H4: every install sets the secrets alert inputs, the post-write one
    /// included: a write that degrades the instance raises the gauge, one
    /// that heals it lowers it.
    #[test]
    fn a_post_write_install_moves_the_degraded_gauge() {
        let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
        let rendered = recorder.handle();
        let gauge = || {
            rendered
                .render()
                .lines()
                .find(|line| line.starts_with(reload::SECRETS_DEGRADED))
                .map(str::to_string)
        };
        let degraded = format!("{} 1", reload::SECRETS_DEGRADED);
        let healed = format!("{} 0", reload::SECRETS_DEGRADED);
        metrics::with_local_recorder(&recorder, || {
            let handle = IntegrationsHandle::new(judged_over(1, Vec::new()));
            handle.install(judged_over(2, vec![sealed_elsewhere()]));
            assert_eq!(gauge(), Some(degraded.clone()), "degraded by the write");
            handle.install(judged_over(3, Vec::new()));
            assert_eq!(gauge(), Some(healed.clone()), "healed by the write");
            handle.install(judged_over(2, vec![sealed_elsewhere()]));
            assert_eq!(gauge(), Some(healed), "a refused install sets nothing");
        });
    }

    /// Every (Slack app, secrets key, unreadable Slack, unreadable Matrix):
    /// the offer for Slack and for Matrix. An unreadable connection outranks
    /// a missing key; Slack's missing app outranks everything.
    #[rustfmt::skip]
    #[test]
    fn each_kinds_offer_follows_the_instance_then_the_unreadable_connection() {
        use MatrixOffer as M;
        use SlackOffer as S;
        const APP: bool = true;
        const KEY: bool = true;
        const UNREADABLE: bool = true;
        let table = [
            // app,  key,   slack unreadable, matrix unreadable, Slack offer, Matrix offer
            (!APP, !KEY, !UNREADABLE, !UNREADABLE, S::NotSetUp, M::NoSecretsKey),
            (!APP, !KEY, UNREADABLE, UNREADABLE, S::NotSetUp, M::Unavailable),
            (!APP, KEY, !UNREADABLE, !UNREADABLE, S::NotSetUp, M::Ready),
            (!APP, KEY, UNREADABLE, UNREADABLE, S::NotSetUp, M::Unavailable),
            (APP, !KEY, !UNREADABLE, !UNREADABLE, S::NoSecretsKey, M::NoSecretsKey),
            (APP, !KEY, UNREADABLE, !UNREADABLE, S::Unavailable, M::NoSecretsKey),
            (APP, !KEY, !UNREADABLE, UNREADABLE, S::NoSecretsKey, M::Unavailable),
            (APP, KEY, !UNREADABLE, !UNREADABLE, S::Ready, M::Ready),
            (APP, KEY, UNREADABLE, !UNREADABLE, S::Unavailable, M::Ready),
            (APP, KEY, !UNREADABLE, UNREADABLE, S::Ready, M::Unavailable),
        ];
        for (app, key, slack, matrix, slack_offer, matrix_offer) in table {
            let integrations = instance(app, key);
            let unreadable = UnreadableConnections { slack, matrix };
            let case = format!("app {app}, key {key}, unreadable slack {slack} matrix {matrix}");
            assert_eq!(integrations.slack_offer(unreadable), slack_offer, "Slack: {case}");
            assert_eq!(integrations.matrix_offer(unreadable), matrix_offer, "Matrix: {case}");
        }
    }

    /// The integrations of an instance holding the test key under
    /// `setting` (none when `None`), its worker on or off.
    fn keyed(setting: Option<config::KeySetting>, worker: bool) -> Integrations {
        let mut settings = IntegrationSettings::off("https://foundry.example.org");
        settings.key_setting = setting;
        settings.secrets = setting.map(|_| {
            SecretBox::from_settings(
                &SecretString::from("ZWlwLXBoYXNlNS1yb3RhdGVkLW5ldy1rZXktMzJieXQ"),
                None,
            )
            .expect("the secrets key parses")
        });
        settings.worker.enabled = worker;
        Integrations::new(settings)
    }

    /// A value sealed under `keys` for connection `connection_id`.
    fn sealed_for(keys: &SecretBox, connection_id: uuid::Uuid, token: &str) -> Vec<u8> {
        keys.seal(
            &SecretString::from(token.to_string()),
            &credential_slot(connection_id),
        )
        .expect("seal")
        .as_bytes()
        .to_vec()
    }

    fn held(integrations: &Integrations) -> &SecretBox {
        integrations
            .settings()
            .secrets
            .as_ref()
            .expect("a held key")
    }

    /// The probe proves a held key by its check value: a check that opens to
    /// the canary proves it, one that opens to anything else does not, and a
    /// key no check names is proved by holding it. A value under a key not
    /// held is missing.
    #[test]
    fn the_probe_proves_a_held_key_only_by_a_check_that_opens_to_the_canary() {
        let integrations = instance(false, true);
        let keys = held(&integrations);
        let id = keys.current_key_id().to_string();
        let mine = sealed_for(keys, uuid::Uuid::now_v7(), "xoxb-mine");
        let good = integrations.key_check().expect("a check value");
        let wrong = KeyCheck {
            key_id: id.clone(),
            sealed_canary: keys
                .seal(&SecretString::from("not the canary"), &key_check_slot(&id))
                .expect("seal")
                .as_bytes()
                .to_vec(),
        };
        let other = KeyCheck {
            key_id: "another-key".to_string(),
            sealed_canary: b"opaque".to_vec(),
        };
        let missing = |sealed: &[Vec<u8>], checks: &[KeyCheck]| {
            integrations
                .check_secrets(sealed, checks)
                .missing_key_ids
                .into_iter()
                .collect::<Vec<_>>()
        };
        let none: Vec<String> = Vec::new();
        assert_eq!(
            missing(std::slice::from_ref(&mine), &[]),
            none,
            "held, no check"
        );
        assert_eq!(
            missing(std::slice::from_ref(&mine), &[good, other]),
            none,
            "its check opens"
        );
        assert_eq!(
            missing(std::slice::from_ref(&mine), &[wrong]),
            vec![id],
            "its check opens to something else"
        );
        let elsewhere = sealed_elsewhere();
        let elsewhere_id = SealedSecret::from_bytes(elsewhere.clone())
            .key_id()
            .to_string();
        assert_eq!(missing(&[elsewhere], &[]), vec![elsewhere_id], "not held");
    }

    /// DD-2: every credential opens only with a held key and a probe that
    /// found nothing missing.
    #[test]
    fn every_credential_opens_only_with_a_key_and_nothing_missing() {
        assert!(
            judged_over(1, Vec::new()).every_credential_opens(),
            "a key, nothing missing"
        );
        assert!(
            !judged_over(1, vec![sealed_elsewhere()]).every_credential_opens(),
            "a key, something missing"
        );
        assert!(!instance(false, false).every_credential_opens(), "no key");
    }

    /// ADR-IIS-001 decision 4: the worker runs when it is on and this server
    /// seals or opens a stored credential.
    #[test]
    fn the_worker_runs_only_when_on_and_able_to_seal_or_open() {
        let with = |enabled: bool, key: bool| {
            keyed(key.then_some(config::KeySetting::LegacySecretsKey), enabled).runs_worker()
        };
        assert!(with(true, true), "on, sealing directly, nothing stored");
        assert!(!with(false, true), "off");
        assert!(!with(true, false), "on, no key");
    }

    /// A credential opens for its own row only; one that does not open under
    /// a key id this server lacks nudges the reload, one under a held key
    /// (another row's) does not.
    #[test]
    fn a_credential_under_a_key_not_held_nudges_the_reload() {
        use futures::FutureExt as _;
        let integrations = instance(false, true);
        let keys = held(&integrations);
        let row = uuid::Uuid::now_v7();
        let nudged = || {
            integrations
                .reload_nudge()
                .notified()
                .now_or_never()
                .is_some()
        };

        let opened = integrations.open_credential(row, sealed_for(keys, row, "xoxb-row"));
        assert_eq!(
            opened.map(|t| t.expose_secret().to_string()),
            Some("xoxb-row".to_string())
        );
        assert!(!nudged(), "opened");

        let another_row = sealed_for(keys, uuid::Uuid::now_v7(), "xoxb-other");
        assert!(integrations.open_credential(row, another_row).is_none());
        assert!(!nudged(), "a held key that does not open this row");

        assert!(integrations
            .open_credential(row, sealed_elsewhere())
            .is_none());
        assert!(nudged(), "a key id not held");
    }

    /// I-2e: direct seals are counted by the held key ids that seal stored
    /// values, and only in `Direct` mode.
    #[test]
    fn direct_seals_count_the_held_key_ids_that_seal_stored_values() {
        let integrations = instance(false, true);
        assert_eq!(integrations.key_mode(), KeyMode::Direct);
        let keys = held(&integrations);
        let snapshot = |sealed: Vec<Vec<u8>>| InstanceSnapshot {
            version: 1,
            secrets_key: None,
            slack_app: None,
            key_checks: Vec::new(),
            sealed_credentials: sealed,
        };
        assert_eq!(integrations.direct_seals(&snapshot(Vec::new())), 0);
        let two = vec![
            sealed_for(keys, uuid::Uuid::now_v7(), "a"),
            sealed_for(keys, uuid::Uuid::now_v7(), "b"),
            sealed_elsewhere(),
        ];
        assert_eq!(
            integrations.direct_seals(&snapshot(two.clone())),
            1,
            "one held key id"
        );
        let awaiting = keyed(Some(config::KeySetting::InstanceKey), false);
        assert_eq!(awaiting.key_mode(), KeyMode::AwaitingSecretsKey);
        assert_eq!(awaiting.direct_seals(&snapshot(two)), 0, "not Direct");
    }

    /// A fresh key's text and parsed key.
    fn fresh() -> (SecretString, foundry_auth::secret_box::SecretsKey) {
        let generated = foundry_auth::secret_box::SecretsKey::generate().unwrap();
        (generated.text, generated.key)
    }

    fn parsed(text: &SecretString) -> foundry_auth::secret_box::SecretsKey {
        foundry_auth::secret_box::SecretsKey::parse(
            text,
            foundry_auth::secret_box::INSTANCE_KEY_SETTING,
        )
        .expect("parse")
    }

    /// The stored secrets key `secrets` wrapped under `wrapper`.
    fn stored_under(
        secrets: &SecretString,
        wrapper: &SecretBox,
        by: foundry_store::integrations::KeyWrapper,
        from_path: Option<&str>,
    ) -> foundry_store::integrations::StoredSecretsKey {
        foundry_store::integrations::StoredSecretsKey {
            key_id: parsed(secrets).key_id().to_string(),
            wrapped_key: wrapper
                .seal(secrets, &keys::secrets_key_slot())
                .expect("wrap")
                .as_bytes()
                .to_vec(),
            wrapped_by_key_id: wrapper.current_key_id().to_string(),
            wrapped_by: by,
            rewrapped_from_path: from_path.map(str::to_string),
            created_by: None,
            created_at: time::OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn with_secrets_key(stored: foundry_store::integrations::StoredSecretsKey) -> InstanceSnapshot {
        InstanceSnapshot {
            version: 2,
            secrets_key: Some(stored),
            slack_app: None,
            key_checks: Vec::new(),
            sealed_credentials: Vec::new(),
        }
    }

    /// ADR-IIS-004 r6 and rule 2: a replica keeps the secrets key it holds
    /// while the stored id is unchanged, even re-wrapped under an instance
    /// key it lacks; the seal fence names that key. A changed id is judged
    /// afresh.
    #[test]
    fn a_held_secrets_key_survives_a_rewrap_under_a_key_this_replica_lacks() {
        let (operator_text, _) = fresh();
        let (secrets, _) = fresh();
        let mut settings = IntegrationSettings::off("https://foundry.example.org");
        settings.key_setting = Some(config::KeySetting::InstanceKey);
        settings.secrets = Some(SecretBox::new(parsed(&operator_text), Vec::new()));
        let operator = SecretBox::new(parsed(&operator_text), Vec::new());
        let elsewhere = SecretBox::new(fresh().1, Vec::new());
        let by = foundry_store::integrations::KeyWrapper::Operator;

        let sealed = Integrations::new(settings).rebuilt(Ok(with_secrets_key(stored_under(
            &secrets, &operator, by, None,
        ))));
        let id = parsed(&secrets).key_id().to_string();
        assert_eq!(sealed.key_mode(), KeyMode::Sealed);
        assert_eq!(sealed.sealed_under(), Some(id.clone()));

        let rewrapped = sealed.rebuilt(Ok(with_secrets_key(stored_under(
            &secrets, &elsewhere, by, None,
        ))));
        assert_eq!(rewrapped.key_mode(), KeyMode::Sealed, "kept under its id");
        assert_eq!(rewrapped.sealed_under(), Some(id));

        let (replaced, _) = fresh();
        let replaced = rewrapped.rebuilt(Ok(with_secrets_key(stored_under(
            &replaced, &elsewhere, by, None,
        ))));
        assert_eq!(
            replaced.key_mode(),
            KeyMode::Unopenable,
            "a new id, not unwrapped"
        );
        assert_eq!(replaced.sealed_under(), None);
    }

    /// Boot's store: `first` until a re-wrap lands, `after` from then on;
    /// every read and re-wrap counted.
    struct RewrapStore {
        first: InstanceSnapshot,
        after: InstanceSnapshot,
        reads: std::sync::atomic::AtomicUsize,
        rewraps: std::sync::Mutex<Vec<foundry_store::integrations::SecretsKeyRewrap>>,
    }

    #[async_trait::async_trait]
    impl store_capabilities::KeyProbe for RewrapStore {
        async fn record_key_check(
            &self,
            _check: &KeyCheck,
        ) -> Result<(), foundry_store::StoreError> {
            Ok(())
        }
        async fn instance_snapshot(&self) -> Result<InstanceSnapshot, foundry_store::StoreError> {
            self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let rewrapped = !self.rewraps.lock().unwrap().is_empty();
            Ok(if rewrapped {
                self.after.clone()
            } else {
                self.first.clone()
            })
        }
        async fn rewrap_secrets_key(
            &self,
            rewrap: &foundry_store::integrations::SecretsKeyRewrap,
            _at: time::OffsetDateTime,
        ) -> Result<foundry_store::integrations::RewrapOutcome, foundry_store::StoreError> {
            self.rewraps.lock().unwrap().push(rewrap.clone());
            Ok(foundry_store::integrations::RewrapOutcome::Rewrapped { version: 3 })
        }
    }

    /// The key file's key, as boot reads it.
    struct HeldFile(SecretString);

    impl InstanceKeySource for HeldFile {
        fn key_file(&self, _may_make: bool) -> KeyFileRead {
            KeyFileRead::Held(Box::new(parsed(&self.0)))
        }
    }

    /// ADR-IIS-002 r5 and I-30 through the boot: with an operator key and
    /// the key file that wrapped the stored secrets key, the key is re-wrapped
    /// under the operator's key once, the rows read again, and the file it
    /// left named. A secrets key already under the operator's key is not
    /// re-wrapped and the rows are read once.
    #[tokio::test]
    async fn boot_rewraps_a_file_wrapped_secrets_key_under_the_operator_key() {
        use foundry_store::integrations::KeyWrapper;
        let (file_text, _) = fresh();
        let (operator_text, _) = fresh();
        let (secrets, _) = fresh();
        let file = SecretBox::new(parsed(&file_text), Vec::new());
        let operator = SecretBox::new(parsed(&operator_text), Vec::new());
        let path = key_file::key_path(std::path::Path::new("/srv/foundry-data"))
            .display()
            .to_string();
        let settings = || {
            let mut settings = IntegrationSettings::off("https://foundry.example.org");
            settings.data_dir = "/srv/foundry-data".into();
            settings.key_setting = Some(config::KeySetting::InstanceKey);
            settings.secrets = Some(SecretBox::new(parsed(&operator_text), Vec::new()));
            settings
        };
        let at = time::OffsetDateTime::UNIX_EPOCH;

        let store = RewrapStore {
            first: with_secrets_key(stored_under(&secrets, &file, KeyWrapper::File, None)),
            after: with_secrets_key(stored_under(
                &secrets,
                &operator,
                KeyWrapper::Operator,
                Some(&path),
            )),
            reads: Default::default(),
            rewraps: Default::default(),
        };
        let booted = Integrations::boot(settings(), &store, &HeldFile(file_text.clone()), at)
            .await
            .expect("boot");
        let rewraps = store.rewraps.lock().unwrap().clone();
        assert_eq!(rewraps.len(), 1, "one re-wrap");
        assert_eq!(rewraps[0].from_key_id, file.current_key_id().to_string());
        assert_eq!(
            rewraps[0].wrapped_by_key_id,
            operator.current_key_id().to_string()
        );
        assert_eq!(
            rewraps[0].rewrapped_from_path.as_deref(),
            Some(path.as_str())
        );
        assert_eq!(
            store.reads.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "read again"
        );
        assert_eq!(booted.key_mode(), KeyMode::Sealed);
        assert_eq!(booted.rewrapped_from(), Some(path.as_str()), "I-30");

        let settled = RewrapStore {
            first: store.after.clone(),
            after: store.after.clone(),
            reads: Default::default(),
            rewraps: Default::default(),
        };
        let booted = Integrations::boot(settings(), &settled, &HeldFile(file_text), at)
            .await
            .expect("boot");
        assert!(
            settled.rewraps.lock().unwrap().is_empty(),
            "nothing to re-wrap"
        );
        assert_eq!(
            settled.reads.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "read once"
        );
        assert_eq!(booted.key_mode(), KeyMode::Sealed);
    }

    /// Boot's recorded check joins the snapshot unless one is stored for its
    /// key id, which is kept (`ON CONFLICT DO NOTHING`).
    #[test]
    fn a_recorded_key_check_joins_the_snapshot_only_for_a_new_key_id() {
        let check = |id: &str, canary: &[u8]| KeyCheck {
            key_id: id.to_string(),
            sealed_canary: canary.to_vec(),
        };
        let with = |checks: Vec<KeyCheck>, recorded: Option<KeyCheck>| {
            let snapshot = InstanceSnapshot {
                version: 1,
                secrets_key: None,
                slack_app: None,
                key_checks: checks,
                sealed_credentials: Vec::new(),
            };
            with_key_check(snapshot, recorded)
                .key_checks
                .into_iter()
                .map(|c| (c.key_id, c.sealed_canary))
                .collect::<Vec<_>>()
        };
        let pair = |id: &str, canary: &[u8]| (id.to_string(), canary.to_vec());
        assert_eq!(
            with(vec![check("a", b"stored")], Some(check("b", b"new"))),
            vec![pair("a", b"stored"), pair("b", b"new")],
            "a new key id joins"
        );
        assert_eq!(
            with(vec![check("a", b"stored")], Some(check("a", b"new"))),
            vec![pair("a", b"stored")],
            "the stored one is kept"
        );
        assert_eq!(
            with(vec![check("a", b"stored")], None),
            vec![pair("a", b"stored")]
        );
    }
}
