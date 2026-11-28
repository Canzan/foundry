//! The instance integrations page, `GET /admin/instance/integrations`
//! (US-IIS-01), and its Generate, `POST …/secrets-key` (US-IIS-02): both
//! behind `require_instance_admin`, the uniform 404 for everyone else. What
//! the page shows and offers is [`page_state`], a pure function of the key
//! mode, where the instance key comes from and Slack's addresses.
//!
//! Generate and Move, `POST …/secrets-key/move` (US-IIS-06), are registered
//! here and carried out by [`super::secrets_key`], which owns what the page
//! offers for the key ([`KeyOffer`]), the one write use-case behind both and
//! the words a post that stored the key answers.
//!
//! The Slack app save, `POST …/slack` (US-IIS-03), behind the same guard:
//! a blank or malformed field answers I-11 or I-12 before anything is
//! touched, the typed client ID kept and both secrets cleared; without a sealing mode it answers I-3 and stores nothing; otherwise both
//! secrets are sealed and handed to the one write use-case, fenced on the
//! sealing key, and this server's integrations are rebuilt and installed
//! before it answers I-10 with I-6 (ADR-IIS-003).
//!
//! The same door replaces the app (US-IIS-05): it reads the client ID in
//! effect before judging, so a submission naming that app may leave a secret
//! field blank and keep the secret already saved, while another client ID
//! needs both of its own secrets. The form is offered wherever credentials
//! seal, over a saved app too, holding the client ID in effect so a rotation
//! needs no retyping. A save that stored the app answers a redirect onto the
//! page itself, with what it earned (I-10 or I-17) waiting in her own
//! session for that page to read once ([`SavedNotice`]), as the confirm and
//! the remove doors answer a write too, so a reload never posts the secrets
//! again and no address carries a notice - while Generate and Move, which
//! carry no submitted value, answer the page in place. A refusal and a save
//! that stored nothing answer the page in place. The Activity reads a
//! replacement in I-18's words.
//!
//! Switching to another app while workspaces are still connected through the
//! app in effect waits for a confirm (US-IIS-05 AC 2): the gate is
//! [`SubmissionDoor`] over the submission's kind and the connected-workspace
//! count, so a rotation and a switch nobody is connected through are saved at
//! once. The gate is judged inside the write's own locked transaction, which
//! is where that count is read (D10): the store takes the lock every instance
//! write takes, reads the count and calls this gate, so a workspace that
//! connects while the post is in flight cannot be switched without its
//! confirm. A submission the gate holds is sealed into the
//! pending row and answered with a redirect to
//! `GET …/slack/confirm`, which reads I-14 in the form that count selects and
//! offers the one control that saves it - no secret and no form field, because
//! what she typed stays sealed. That door only reads ([`ReadOnlyDoor`]), so a
//! prefetch decides nothing. Its `POST` applies the held app for the admin who
//! holds it, before it expires, and answers the page with the notice the save
//! earned; a second confirm, another admin's confirm and a late one change
//! nothing and answer the page as it is.
//!
//! Retiring Slack on this Foundry (US-IIS-05 AC 3): the page offers Replace
//! and Remove over the app saved here and neither over the app Foundry's
//! environment sets ([`SlackOffer`], D7). Remove opens
//! `GET …/slack/remove`, which reads I-15 in the form the
//! connected-workspace count selects and offers the one control that carries
//! the removal out; that door only reads ([`ReadOnlyDoor`]), so a prefetch
//! and a reload take nothing away. Its `POST` deletes the saved app with its
//! activity line and the version bump in one transaction, rebuilds this
//! server's integrations and answers I-16. It touches nothing else: every
//! workspace keeps its Slack connection, its queued deliveries and its
//! silence - nobody is told a connection was lost - and the workspace page
//! reads W-2 from the shipped cause afterwards. A remove while the
//! environment's app is in effect changes nothing and answers the page.

use super::secrets_key::{key_door, KeyOffer, NO_SECRETS_KEY, SECRETS_KEY_GENERATED};
use super::{reinstall, render};
use crate::bootstrap::{resource_not_found_page, SessionUser};
use crate::csrf::ensure_csrf_cookie;
use crate::integrations::keys::{KeyFile, KeyMode, NoKeyReason};
use crate::integrations::secrets_key_rotation::{LiveOpeners, SavedCredentials, UnopenableOffer};
use crate::integrations::slack_app::{
    app_in_effect, client_id_in_effect, plan_hold_pending, plan_replace, reseal_held_for_save,
    AppInEffect, Refusal, SavedApp, SlackChange, SlackField, SlackSubmission, SubmissionKind,
};
use crate::integrations::store_capabilities::{
    InstanceKeyWrites, KeyProbe, ResealRefused, StoreCapabilities,
};
use crate::integrations::{slack, Integrations};
use crate::AppState;
use askama::Template;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use foundry_store::integrations::{
    InstanceActivityRow, PendingApplied, RemoveOutcome, SlackSaveOutcome, SlackSubmissionPlan,
    SubmissionStored,
};
use time::OffsetDateTime;
use tower_sessions::Session;
use uuid::Uuid;

pub const INSTANCE_PATH: &str = "/admin/instance/integrations";
pub const SECRETS_KEY_PATH: &str = "/admin/instance/integrations/secrets-key";
pub const MOVE_PATH: &str = "/admin/instance/integrations/secrets-key/move";
/// Where Priya reads how many saved credentials a replacement seals again
/// before she decides (US-IIS-07 AC 1, I-23), and where the replacement
/// itself is posted (I-24).
pub const REPLACE_PATH: &str = "/admin/instance/integrations/secrets-key/replace";
/// Where Priya reads what starting over discards before she decides
/// (US-IIS-07 AC 3, I-28), and where the start over itself is posted.
pub const START_OVER_PATH: &str = "/admin/instance/integrations/secrets-key/start-over";
pub const SLACK_APP_PATH: &str = "/admin/instance/integrations/slack";
/// Where a submission of another app waits while workspaces are connected
/// through the current one (US-IIS-05 AC 2, I-14).
pub const SLACK_CONFIRM_PATH: &str = "/admin/instance/integrations/slack/confirm";
/// Where Priya reads what a removal stops before she decides (US-IIS-05
/// AC 3, I-15), and where the removal itself is posted.
pub const SLACK_REMOVE_PATH: &str = "/admin/instance/integrations/slack/remove";

/// I-25, operator-set.
const KEY_BY_OPERATOR: &str = "Instance key: set by the operator.";
/// I-30: the operator's key took over Foundry's file.
const KEY_REPLACED_FILE: &str = "Instance key: set by the operator. The secrets key, first sealed with {path}, is now sealed with the operator's key. Keep a backup of {path} until you are sure, then remove it.";
/// I-25, made by Foundry.
const KEY_IN_FILE: &str = "Instance key: made by Foundry in {path}. Back up this file apart from the database. Every Foundry server must use the same file. Without it, saved connections must be reconnected.";
/// I-27.
const KEY_UNWRITABLE: &str = "Foundry has no instance key, and it cannot make one because {path} cannot be written. To fix this, do one of these, then restart Foundry: set FOUNDRY_INSTANCE_KEY to a generated key, or set FOUNDRY_DATA_DIR to a folder Foundry can write to. To generate a key, run: openssl rand -base64 32 | tr '+/' '-_' | tr -d '='";
/// I-31.
const KEY_UNREADABLE: &str = "This Foundry cannot read its instance key file ({path}). Slack and Matrix are paused until it can. An instance operator restores the file from backup, or sets FOUNDRY_INSTANCE_KEY, and restarts Foundry.";
/// I-2.
const SECRETS_KEY_SET: &str = "Secrets key: set. Credentials entered here are sealed with it.";
/// I-2e.
const DIRECT_SAVED: &str = "Saved credentials are sealed directly with the instance key. Move them under a secrets key to manage it here.";
/// I-2e0.
const DIRECT_NOTHING_SAVED: &str = "Credentials saved here are sealed directly with the instance key. Move to a secrets key to manage it here.";
/// I-4: some saved credential opens under no key this server holds.
const VALUES_UNOPENABLE: &str = "This Foundry cannot read some saved credentials: neither its instance key nor its previous instance key opens them. An instance operator sets the instance key they were sealed with as the previous instance key, and restarts Foundry.";
/// I-26 (restore-only in Deliverable A).
const SECRETS_KEY_UNOPENABLE: &str = "This Foundry cannot open its secrets key: the instance key it was sealed with is missing or different on this server. Until it is restored, saved connections cannot be used. An instance operator restores that instance key, or sets it as the previous instance key, and restarts Foundry.";
/// I-8.
const MATRIX_READY: &str = "Matrix: ready. Each workspace admin enters their Matrix server address and bot access token on the workspace's Integrations page.";
/// I-5.
const SLACK_NOT_SET_UP: &str = "Slack: not set up. Create the Foundry Slack app in Slack from the manifest below, then enter its credentials.";
/// I-29, above I-5's addresses.
const PUBLIC_URL_FELL_BACK: &str = "FOUNDRY_PUBLIC_URL is not set, so the addresses below use {fallback_url}, which Slack cannot reach. An instance operator sets FOUNDRY_PUBLIC_URL to this Foundry's public address and restarts Foundry.";
/// I-7, with the environment's client ID.
const SLACK_IN_ENVIRONMENT: &str = "Slack: set in Foundry's environment, app {client_id}. To manage it here, enter the same app below; it takes over once SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET are removed from the environment.";
/// I-11, one per field in form order.
const ENTER_CLIENT_ID: &str = "Enter the client ID from the Slack app's Basic Information.";
const ENTER_CLIENT_SECRET: &str = "Enter the client secret from the Slack app's Basic Information.";
const ENTER_SIGNING_SECRET: &str =
    "Enter the signing secret from the Slack app's Basic Information.";
/// I-12, one per field in form order.
const NOT_A_CLIENT_ID: &str =
    "That is not a Slack client ID. Copy it again from the Slack app's Basic Information.";
const NOT_A_CLIENT_SECRET: &str =
    "That is not a Slack client secret. Copy it again from the Slack app's Basic Information.";
const NOT_A_SIGNING_SECRET: &str =
    "That is not a Slack signing secret. Copy it again from the Slack app's Basic Information.";
/// I-10.
const SLACK_APP_SAVED: &str = "Saved. Workspace admins can now choose Add to Slack.";
/// I-14, n = 1: one workspace still connected through the current app.
const CONNECTED_ONE: &str = "1 workspace is connected through the current Slack app ({client_id}). It keeps posting through it until a workspace admin reconnects Slack. Save the new app?";
/// I-14, n >= 2.
const CONNECTED_MANY: &str = "{n} workspaces are connected through the current Slack app ({client_id}). They keep posting through it until a workspace admin reconnects Slack. Save the new app?";
/// I-14's one control.
const SAVE_NEW_APP: &str = "Save the new app";
/// I-15, n = 0: nobody is connected, so nothing stops posting.
const REMOVE_NONE_CONNECTED: &str =
    "Remove the Slack app from this Foundry? No workspace is connected to Slack.";
/// I-15, n = 1.
const REMOVE_ONE: &str = "Remove the Slack app from this Foundry? 1 workspace's Slack routes stop posting until it is set up again, and no workspace can connect Slack.";
/// I-15, n >= 2.
const REMOVE_MANY: &str = "Remove the Slack app from this Foundry? {n} workspaces' Slack routes stop posting until it is set up again, and no workspace can connect Slack.";
/// I-16: what the removal answers.
const SLACK_APP_REMOVED: &str = "Removed. Slack is not set up on this Foundry.";
/// The Slack app form's own control where no app saved here is in effect:
/// the first save of an app.
const SAVE_SLACK_APP: &str = "Save Slack app";
/// I-17: saved while the environment's app is in effect.
const SLACK_SAVED_ENVIRONMENT_WINS: &str = "Saved. The app in Foundry's environment stays in effect until SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET are removed from it and Foundry restarts; then this one is used.";
/// I-13: a saved app no held key opens.
const SLACK_SAVED_UNREADABLE: &str =
    "Slack: saved, but this Foundry cannot read it (see the secrets key above).";
/// I-6, with the saved app's client ID, who saved it and when.
const SLACK_SAVED_HERE: &str = "Slack: set up here · app {client_id} · saved by {name} · {date}";
/// I-6's secret lines: set, never shown.
const CLIENT_SECRET_SET: &str = "Client secret: set";
const SIGNING_SECRET_SET: &str = "Signing secret: set";
/// How many times a save is tried when the seal fence finds a newer key.
const SAVE_ATTEMPTS: usize = 2;
/// What a save the fence turned back on every try answers: no copy line of
/// its own (upstream item 47 M1), so the closest existing one, nothing saved.
const SAVE_TURNED_BACK: &str = super::NOT_CONNECTED;
/// How many activity lines the page lists.
const ACTIVITY_LINES: i64 = 20;
/// I-9.
const MATRIX_NEEDS_KEY: &str = "Matrix: off. To turn it on, set up the secrets key above. Then workspace admins connect their own Matrix bot.";

/// What the page is judged over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageFacts {
    pub mode: KeyMode,
    /// The operator set the instance key (either setting name).
    pub operator_key: bool,
    /// The instance key file's path, as the page names it.
    pub key_file: String,
    /// What the key file gave at boot: no file was made when `Withheld`.
    pub key_file_read: KeyFile,
    /// The key file the stored secrets key was re-wrapped from (I-30).
    pub rewrapped_from: Option<String>,
    /// Some stored credential is sealed directly under a held instance key.
    pub saved_direct: bool,
    /// Some stored value opens under no key this server holds (I-4).
    pub unopenable_values: bool,
    /// Whether another running Foundry server of this database still opens the
    /// stored secrets key, as the gate read it; `None` where the replica
    /// states could not be read (US-IIS-07 AC 2, D5).
    pub live_opener: Option<bool>,
    /// A Slack app is in effect on this server.
    pub slack_app: bool,
    /// Which app that is (D3): the controls the page offers over it, and the
    /// remove door's own gate, follow from it.
    pub app_in_effect: AppInEffect,
    /// The environment's app's client ID, when its trio is set: it wins.
    pub environment_slack: Option<String>,
    /// The saved app, when it is the one in effect.
    pub saved_slack: Option<SavedSlack>,
    /// A saved app is stored that no held key opens (I-13).
    pub saved_unreadable: bool,
    pub addresses: SlackAddresses,
}

/// The saved app in effect, as I-6 names it. Never a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedSlack {
    pub client_id: String,
    pub saved_by: String,
    pub saved_at: OffsetDateTime,
}

/// I-6: the saved app's line, and its two secrets as set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedSlackLines {
    pub line: String,
    pub client_secret: &'static str,
    pub signing_secret: &'static str,
}

/// What Slack is told about this Foundry, from its integrations (one source
/// with the Slack setup page).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackAddresses {
    pub manifest: String,
    pub redirect_url: String,
    pub events_url: String,
    /// The fallback address, when `FOUNDRY_PUBLIC_URL` is unset (I-29).
    pub fell_back_to: Option<String>,
}

/// I-5 with what to paste into Slack, and I-29 above it when the addresses
/// fell back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackSetup {
    pub warning: Option<String>,
    pub line: &'static str,
    pub addresses: SlackAddresses,
}

/// A control the page offers over the Slack app in effect (US-IIS-05): a
/// closed set shaped like [`KeyOffer`], each offer carrying its words, the
/// door it reaches and when it applies, so the page keeps one way of
/// offering a control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlackOffer {
    /// Enter another app, or another secret of the app in effect: the Slack
    /// app form's own control (US-IIS-05 AC 1).
    Replace,
    /// Take Slack off this Foundry, after I-15 says what stops
    /// (US-IIS-05 AC 3).
    Remove,
}

impl SlackOffer {
    pub fn label(self) -> &'static str {
        match self {
            Self::Replace => "Replace",
            Self::Remove => "Remove",
        }
    }

    /// Where this control posts.
    pub fn door(self) -> &'static str {
        match self {
            Self::Replace => SLACK_APP_PATH,
            Self::Remove => SLACK_REMOVE_PATH,
        }
    }

    /// The door this control opens by itself, where choosing it only asks a
    /// question: Remove opens the remove page, which reads I-15 before
    /// anything goes. Replace opens none, because it submits the app form
    /// Priya filled in.
    pub fn opens(self) -> Option<&'static str> {
        match self {
            Self::Replace => None,
            Self::Remove => Some(self.door()),
        }
    }

    /// Whether this control applies with `effect` in effect and the key in
    /// `mode`: only the app saved here is Priya's to replace or to retire
    /// from the browser, and only where credentials seal. Under the app
    /// Foundry's environment sets, a control here would either do nothing
    /// she can see or take the operator's Slack away behind their back, and
    /// the next restart would bring it back anyway (D7, L5). The page and
    /// the remove door read this one rule, so neither can offer what the
    /// other refuses.
    pub fn applies(self, effect: AppInEffect, mode: KeyMode) -> bool {
        match self {
            Self::Replace | Self::Remove => mode.seals() && effect == AppInEffect::Saved,
        }
    }

    /// Every control the page offers with `effect` in effect, in the order it
    /// reads them.
    fn offered(effect: AppInEffect, mode: KeyMode) -> Vec<Self> {
        [Self::Replace, Self::Remove]
            .into_iter()
            .filter(|offer| offer.applies(effect, mode))
            .collect()
    }
}

/// What a door that changed the Slack app answers: a closed set of notices,
/// each with its own wire name, left in the admin's own session for the page
/// she lands on to read once (post, redirect, get). Her address carries
/// neither a submitted value nor a notice, so no address can claim a change
/// that was not made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavedNotice {
    /// I-10: the app saved here is the one in effect.
    InEffect,
    /// I-17: the environment's app stays in effect until it is removed.
    EnvironmentWins,
    /// I-16: the app saved here is gone, so Slack is not set up at all.
    Removed,
    /// I-24: the secrets key was replaced, with the credentials now sealed
    /// under it (US-IIS-07 AC 1, D7).
    Replaced(usize),
    /// I-21: a start over stored a new secrets key.
    StartedOver,
}

impl SavedNotice {
    /// Where the notice waits in the admin's own session until the page she
    /// lands on reads it.
    const SESSION_KEY: &'static str = "integrations.slack_notice";
    /// What a replacement's wire name starts with, before the count I-24
    /// carries.
    const REPLACED_WIRE: &'static str = "replaced:";

    /// How the session names it, the count I-24 carries included.
    fn wire(self) -> String {
        match self {
            Self::InEffect => "app".to_string(),
            Self::EnvironmentWins => "app-environment".to_string(),
            Self::Removed => "app-removed".to_string(),
            Self::Replaced(count) => format!("{}{count}", Self::REPLACED_WIRE),
            Self::StartedOver => "started-over".to_string(),
        }
    }

    /// The notice the admin lands on.
    fn words(self) -> String {
        match self {
            Self::InEffect => SLACK_APP_SAVED.to_string(),
            Self::EnvironmentWins => SLACK_SAVED_ENVIRONMENT_WINS.to_string(),
            Self::Removed => SLACK_APP_REMOVED.to_string(),
            Self::Replaced(count) => SavedCredentials::counted(count).replaced_notice(),
            Self::StartedOver => SECRETS_KEY_GENERATED.to_string(),
        }
    }

    /// The notice a stored wire name asks for; none for anything else, so a
    /// session holding nothing it names reads as the page itself - a wire
    /// name an older release wrote, and a count that is not a count this
    /// Foundry could hold, alike (Y9).
    fn of_wire(wire: &str) -> Option<Self> {
        if let Some(count) = wire.strip_prefix(Self::REPLACED_WIRE) {
            return count.parse().ok().map(Self::Replaced);
        }
        [
            Self::InEffect,
            Self::EnvironmentWins,
            Self::Removed,
            Self::StartedOver,
        ]
        .into_iter()
        .find(|saved| saved.wire() == wire)
    }

    /// The admin sent to the page itself to read this notice (post, redirect,
    /// get, so a reload never posts the change again), with the notice left
    /// waiting in her own session: nothing she submitted and no notice
    /// reaches the address, so an address alone - typed again, bookmarked or
    /// shared - tells nobody a change was made. The notice outlives the
    /// redirect in the stored session, so whichever replica answers her next
    /// read shows it, even before that replica has reloaded the change
    /// itself. A session that could not hold it leaves the committed change
    /// with no notice.
    pub(super) async fn answered(self, session: &Session) -> Response {
        if let Err(err) = session.insert(Self::SESSION_KEY, self.wire()).await {
            tracing::warn!(error = %err, "the session could not hold what the Slack change answers");
        }
        Redirect::to(INSTANCE_PATH).into_response()
    }

    /// The notice waiting in `session`, taken out as it is read: it is the
    /// answer to one change, so the page shows it once and a reload of that
    /// page shows the page as it is.
    async fn taken_from(session: &Session) -> Option<Self> {
        let wire = session
            .remove::<String>(Self::SESSION_KEY)
            .await
            .ok()
            .flatten()?;
        Self::of_wire(&wire)
    }
}

/// Where a judged submission goes: another app, while some workspace is still
/// connected through the app in effect, waits for the same admin's confirm
/// (I-14); anything else - a rotation of the app in effect, or another app
/// with nobody connected - is saved at once (US-IIS-05 AC 2, AC 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubmissionDoor {
    SaveAtOnce,
    WaitForConfirm,
}

impl SubmissionDoor {
    /// Where a submission of `kind` goes with `connected` workspaces
    /// connected and `in_effect` in effect: the gate, a pure function of
    /// three closed facts. Another app waits only while some workspace is
    /// still connected through an app that is actually in effect: that is
    /// the app I-14 names and the one those workspaces keep posting through.
    /// With no app in effect nothing is being switched - the app saved here
    /// is this Foundry's first - so it is saved at once, however many
    /// connections an app that is gone left behind.
    pub(crate) fn of(kind: SubmissionKind, connected: u32, in_effect: AppInEffect) -> Self {
        match (kind, connected, in_effect) {
            (SubmissionKind::NewApp, 1.., AppInEffect::Environment | AppInEffect::Saved) => {
                Self::WaitForConfirm
            }
            (SubmissionKind::NewApp | SubmissionKind::Rotate, _, _) => Self::SaveAtOnce,
        }
    }
}

/// I-14 over the connected-workspace count: the n = 1 form for exactly one
/// workspace, the counted form above that, each naming the app in effect
/// (`current_client_id`). A count of none never reaches it, because the gate
/// saves at once.
fn confirm_lines(current_client_id: &str, connected: u32) -> String {
    match connected {
        1 => CONNECTED_ONE.to_string(),
        n => CONNECTED_MANY.replace("{n}", &n.to_string()),
    }
    .replace("{client_id}", current_client_id)
}

/// I-15 over the connected-workspace count: the n = 0 form where nobody is
/// connected, the n = 1 form for exactly one workspace, the counted form
/// above that. The count is read when the page is rendered, and an unread
/// count never reaches here: it is not none connected (B1).
fn remove_lines(connected: u32) -> String {
    match connected {
        0 => REMOVE_NONE_CONNECTED.to_string(),
        1 => REMOVE_ONE.to_string(),
        n => REMOVE_MANY.replace("{n}", &n.to_string()),
    }
}

/// One store read a page of this feature's doors makes. Every variant is a
/// read: this type holds no write, so a door that only reads can reach
/// nothing else (L6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PageRead {
    /// The stored instance rows, the app held for its confirm included, and
    /// the credentials a replacement would seal again (I-23), or a start over
    /// would discard (I-28).
    Snapshot,
    /// The connected-workspace count I-14 carries.
    ConnectedWorkspaces,
    /// Every replica's state row, which the live-replica gate of the start
    /// over judges (US-IIS-07 AC 2, D5).
    ReplicaStates,
}

/// A door that answers a page and decides nothing: a browser prefetch, a
/// reload and a link preview of it all change no row, write no activity line
/// and bump no version, because the only store calls it makes are its
/// [`PageRead`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReadOnlyDoor {
    /// `GET /admin/instance/integrations/slack/confirm` (I-14).
    SlackConfirm,
    /// `GET /admin/instance/integrations/slack/remove` (I-15): it asks what
    /// a removal stops and takes nothing away (L6, C3).
    SlackRemove,
    /// `GET /admin/instance/integrations/secrets-key/replace` (I-23): it asks
    /// how many credentials a replacement seals again and replaces nothing
    /// (L4, C2).
    SecretsKeyReplace,
    /// `GET /admin/instance/integrations/secrets-key/start-over` (I-28): it
    /// asks what starting over discards - the credentials this Foundry cannot
    /// open, and whether another live server still opens them - and discards
    /// nothing (L4, C2).
    SecretsKeyStartOver,
}

impl ReadOnlyDoor {
    /// What this door reads, in the order it does: the one list the handler
    /// follows.
    pub(super) fn reads(self) -> &'static [PageRead] {
        match self {
            Self::SlackConfirm => &[PageRead::Snapshot, PageRead::ConnectedWorkspaces],
            Self::SlackRemove => &[PageRead::ConnectedWorkspaces],
            Self::SecretsKeyReplace => &[PageRead::Snapshot],
            Self::SecretsKeyStartOver => &[PageRead::Snapshot, PageRead::ReplicaStates],
        }
    }

    /// Its reads, made in that order, with `gate` judging the replica states
    /// it reads. A read that failed is `None`, never a zero: an unread count
    /// is not none connected, not none saved, and an unread replica list is
    /// not nobody opening the key either (B1, B6).
    pub(super) async fn read(self, store: &StoreCapabilities, gate: LiveOpeners) -> PageFactsRead {
        let mut facts = PageFactsRead::default();
        for read in self.reads() {
            match read {
                PageRead::Snapshot => match store.instance_snapshot().await {
                    Ok(snapshot) => {
                        facts.waiting = Some(snapshot.slack_app_pending.is_some());
                        facts.saved = Some(SavedCredentials::of(
                            snapshot.sealed_credentials.len(),
                            snapshot.slack_app.is_some(),
                        ));
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "reading the stored credentials failed");
                    }
                },
                PageRead::ConnectedWorkspaces => {
                    facts.connected = match store.connected_slack_workspaces().await {
                        Ok(connected) => u32::try_from(connected).ok(),
                        Err(err) => {
                            tracing::warn!(error = %err, "counting the Slack connections failed");
                            None
                        }
                    }
                }
                PageRead::ReplicaStates => facts.live_opener = live_opener(store, gate).await,
            }
        }
        facts
    }
}

/// Whether another running Foundry server of this database still opens the
/// stored secrets key: its own state row says so and its last beat is inside
/// the gate's window (D5). The page and the start-over door read this one
/// answer, so neither judges the other's state. `None` where the rows could
/// not be read: that is neither a server opening the key nor the absence of
/// one, and [`UnopenableOffer::of_gate`] withholds the way out over it.
async fn live_opener(store: &StoreCapabilities, gate: LiveOpeners) -> Option<bool> {
    match store.replica_states().await {
        Ok(states) => Some(gate.still_opening(&states)),
        Err(err) => {
            tracing::warn!(error = %err, "reading what this Foundry's servers open failed");
            None
        }
    }
}

/// The gate as this server reads the time: the clock is the injected one,
/// never `now()`, and it is all the gate takes. The window is the gate's own
/// constant, so no door and no setting of this server can move it (D5,
/// ADR-IIS-SKR-003).
pub(super) fn gate_of(state: &AppState) -> LiveOpeners {
    LiveOpeners::at(state.clock.now())
}

/// What a [`ReadOnlyDoor`]'s reads answered.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct PageFactsRead {
    /// Whether an app is held for its confirm.
    waiting: Option<bool>,
    connected: Option<u32>,
    /// The credentials a replacement seals again (I-22, I-23) and a start over
    /// discards (I-28).
    pub(super) saved: Option<SavedCredentials>,
    /// Whether another live Foundry server still opens the stored secrets key,
    /// as the gate judged the rows it read.
    pub(super) live_opener: Option<bool>,
}

/// What the page shows and offers. Never a key, a key id or a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageState {
    /// I-25, or I-27 / I-31 in its place; none when no key file was made
    /// (`Withheld`): the mode's own line says what is wrong.
    pub instance_key: Option<String>,
    /// I-2, I-2e, I-2e0, I-3, I-4 or I-26; none without an instance key.
    pub secrets_key: Option<&'static str>,
    /// The one sentence appended to the I-26 restore words while another live
    /// Foundry server still opens the stored key, which it never rewrites
    /// (US-IIS-07 AC 2).
    pub secrets_key_appended: Option<&'static str>,
    pub offer: Option<KeyOffer>,
    /// The way out of a stored secrets key this Foundry cannot open
    /// (US-IIS-07 AC 2, I-26): where she reads what starting over discards
    /// before anything is discarded. Never beside `offer`'s Replace: the two
    /// belong to different states of the stored key (L7).
    pub start_over: Option<StartOverWay>,
    /// I-8 or I-9.
    pub matrix: &'static str,
    /// I-5: with a secrets key and no Slack app.
    pub slack: Option<SlackSetup>,
    /// I-6: the saved app in effect.
    pub saved_slack: Option<SavedSlackLines>,
    /// I-13: a saved app this Foundry cannot read.
    pub saved_unreadable: Option<&'static str>,
    /// I-7: the environment's app in effect.
    pub environment_slack: Option<String>,
    /// The Slack app form: wherever credentials seal - under I-5, I-7, I-13
    /// and beside I-6, where it is the replace form (US-IIS-05 AC 1).
    pub slack_form: bool,
    /// The client ID of the app in effect (I-6, I-7), which the form offers.
    pub current_client_id: Option<String>,
    /// The controls the page offers over the app in effect (US-IIS-05):
    /// Replace, which the Slack app form's own control carries, and Remove,
    /// which opens the door that reads I-15 first.
    pub slack_offers: Vec<SlackOffer>,
}

/// The way out the page offers under I-26: the door it opens and the words
/// Priya reads on it. It opens a page and decides nothing, as Remove's and
/// Replace's controls do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartOverWay {
    pub door: &'static str,
    pub label: &'static str,
}

impl PageState {
    /// Whether the page offers `offer` over the app in effect.
    pub fn offers(&self, offer: SlackOffer) -> bool {
        self.slack_offers.contains(&offer)
    }

    /// The Slack app form's own control: over the app saved here it is the
    /// Replace control (US-IIS-05 AC 1); anywhere else the form enters an
    /// app for the first time.
    pub fn slack_form_control(&self) -> &'static str {
        match self.offers(SlackOffer::Replace) {
            true => SlackOffer::Replace.label(),
            false => SAVE_SLACK_APP,
        }
    }

    /// The client ID the Slack app form holds: what a refused save typed
    /// (I-11, I-12), else the client ID of the app in effect, so a rotation
    /// of its secrets needs no retyping (US-IIS-05 AC 1).
    pub fn form_client_id<'page>(&'page self, typed: &'page str) -> &'page str {
        if typed.is_empty() {
            self.current_client_id.as_deref().unwrap_or_default()
        } else {
            typed
        }
    }
}

/// The page of `facts` (pure and total).
pub fn page_state(facts: &PageFacts) -> PageState {
    let secrets_key = match facts.mode {
        KeyMode::NoInstanceKey(_) => None,
        KeyMode::Unopenable => Some(SECRETS_KEY_UNOPENABLE),
        _ if facts.unopenable_values => Some(VALUES_UNOPENABLE),
        KeyMode::AwaitingSecretsKey => Some(NO_SECRETS_KEY),
        KeyMode::Direct if facts.saved_direct => Some(DIRECT_SAVED),
        KeyMode::Direct => Some(DIRECT_NOTHING_SAVED),
        KeyMode::Sealed => Some(SECRETS_KEY_SET),
    };
    let offer = [KeyOffer::Generate, KeyOffer::Move, KeyOffer::Replace]
        .into_iter()
        .find(|offer| offer.applies(facts.mode, facts.unopenable_values));
    // The three I-26 states, decided by the one closed type over the mode,
    // the gate and the key file: the way out, the sentence that says why it
    // is withheld, or neither (L6, L7).
    let unopenable = UnopenableOffer::of_gate(facts.mode, facts.live_opener, facts.key_file_read);
    let slack = slack_setup(facts);
    let environment_slack = facts
        .environment_slack
        .as_ref()
        .map(|client_id| SLACK_IN_ENVIRONMENT.replace("{client_id}", client_id));
    PageState {
        instance_key: instance_key_line(facts),
        secrets_key,
        secrets_key_appended: unopenable.appended(),
        offer,
        start_over: unopenable.offers_start_over().then_some(StartOverWay {
            door: START_OVER_PATH,
            label: super::secrets_key::START_OVER_CONTROL,
        }),
        matrix: if facts.mode.seals() {
            MATRIX_READY
        } else {
            MATRIX_NEEDS_KEY
        },
        slack_form: facts.mode.seals(),
        current_client_id: client_id_in_effect(
            facts.environment_slack.as_deref(),
            facts.saved_slack.as_ref().map(|app| app.client_id.as_str()),
        )
        .map(str::to_string),
        slack,
        environment_slack,
        saved_slack: facts.saved_slack.as_ref().map(|app| SavedSlackLines {
            line: SLACK_SAVED_HERE
                .replace("{client_id}", &app.client_id)
                .replace("{name}", &app.saved_by)
                .replace("{date}", &display_date(app.saved_at)),
            client_secret: CLIENT_SECRET_SET,
            signing_secret: SIGNING_SECRET_SET,
        }),
        saved_unreadable: facts.saved_unreadable.then_some(SLACK_SAVED_UNREADABLE),
        slack_offers: SlackOffer::offered(facts.app_in_effect, facts.mode),
    }
}

fn slack_setup(facts: &PageFacts) -> Option<SlackSetup> {
    (facts.mode == KeyMode::Sealed && !facts.slack_app).then(|| SlackSetup {
        warning: facts
            .addresses
            .fell_back_to
            .as_ref()
            .map(|url| PUBLIC_URL_FELL_BACK.replace("{fallback_url}", url)),
        line: SLACK_NOT_SET_UP,
        addresses: facts.addresses.clone(),
    })
}

fn instance_key_line(facts: &PageFacts) -> Option<String> {
    let words = match facts.mode {
        KeyMode::NoInstanceKey(NoKeyReason::Unwritable) => KEY_UNWRITABLE,
        KeyMode::NoInstanceKey(NoKeyReason::Unreadable) => KEY_UNREADABLE,
        _ => match (facts.operator_key, &facts.rewrapped_from) {
            (true, Some(path)) => return Some(KEY_REPLACED_FILE.replace("{path}", path)),
            (true, None) => KEY_BY_OPERATOR,
            (false, _) if facts.key_file_read == KeyFile::Withheld => return None,
            (false, _) => KEY_IN_FILE,
        },
    };
    Some(words.replace("{path}", &facts.key_file))
}

/// The facts of the integrations in effect on this server, with
/// `direct_seals` stored values sealed directly, the saved app in effect
/// named as saved by `saver` and `live_opener` as the gate read the other
/// servers of this database.
fn facts_of(
    integrations: &Integrations,
    direct_seals: usize,
    saver: Option<String>,
    live_opener: Option<bool>,
) -> PageFacts {
    let settings = integrations.settings();
    PageFacts {
        mode: integrations.key_mode(),
        live_opener,
        operator_key: settings.key_setting.is_some(),
        key_file: crate::integrations::key_file::key_path(&settings.data_dir)
            .display()
            .to_string(),
        key_file_read: integrations.key_file(),
        rewrapped_from: integrations.rewrapped_from().map(str::to_string),
        saved_direct: direct_seals > 0,
        unopenable_values: integrations.secrets_check().degraded(),
        slack_app: integrations.slack().is_some(),
        app_in_effect: app_in_effect_of(integrations),
        environment_slack: settings.slack.as_ref().map(|app| app.client_id.clone()),
        saved_slack: integrations.saved_slack().map(|app| SavedSlack {
            client_id: app.client_id.clone(),
            saved_by: saver.unwrap_or_else(|| "Foundry".to_string()),
            saved_at: app.saved_at,
        }),
        saved_unreadable: integrations.saved_app() == SavedApp::DoesNotOpen,
        addresses: SlackAddresses {
            manifest: slack::manifest(integrations),
            redirect_url: integrations.slack_return_url(),
            events_url: integrations.slack_events_url(),
            fell_back_to: settings
                .public_url_fell_back
                .then(|| settings.public_url.clone()),
        },
    }
}

/// Which app is in effect on this server (D3): the environment's where its
/// trio is set, else the saved app whose secrets both open, else none. The
/// page's controls and the remove door's gate read this one answer.
pub(crate) fn app_in_effect_of(integrations: &Integrations) -> AppInEffect {
    app_in_effect(
        integrations.settings().slack.is_some(),
        integrations.saved_app(),
    )
}

/// An activity line as I-18 reads it.
fn activity_line(row: &InstanceActivityRow) -> String {
    let actor = row.actor_name.as_deref().unwrap_or("Foundry");
    let what = match row.action.as_str() {
        "secrets_key_generated" => "generated the secrets key".to_string(),
        "secrets_key_rewrapped" => {
            "re-sealed the secrets key under the operator's instance key".to_string()
        }
        "secrets_key_replaced" => match row.count.unwrap_or_default() {
            1 => "replaced the secrets key (1 credential sealed again)".to_string(),
            n => format!("replaced the secrets key ({n} credentials sealed again)"),
        },
        "started_over" => match row.count.unwrap_or_default() {
            1 => "started over with a new secrets key (1 credential discarded)".to_string(),
            n => format!("started over with a new secrets key ({n} credentials discarded)"),
        },
        "credentials_moved" => match row.count.unwrap_or_default() {
            1 => "moved 1 credential under a secrets key".to_string(),
            n => format!("moved {n} credentials under a secrets key"),
        },
        "slack_app_saved" => slack_app_line("saved", row),
        "slack_app_replaced" => slack_app_line("replaced", row),
        "slack_app_removed" => slack_app_line("removed", row),
        other => other.replace('_', " "),
    };
    format!("{} · {actor} {what}", display_date(row.created_at))
}

/// I-18's Slack app lines: what was done to the app, naming the app it was
/// done to.
fn slack_app_line(what: &str, row: &InstanceActivityRow) -> String {
    format!(
        "{what} the Slack app ({})",
        row.client_id.as_deref().unwrap_or_default()
    )
}

/// "21 Oct 2026".
fn display_date(at: OffsetDateTime) -> String {
    let month = at.month().to_string();
    format!("{} {} {}", at.day(), &month[..3], at.year())
}

#[derive(Template)]
#[template(path = "integrations/instance.html")]
struct InstancePage {
    nav: crate::nav::NavContext,
    csrf: String,
    /// Where the Slack app form posts.
    slack_door: &'static str,
    /// What the post just did (I-21, I-22, I-10 …).
    notice: Option<String>,
    /// The client ID the form holds: the one a refused save typed (I-11,
    /// I-12), else the app in effect's (US-IIS-05 AC 1).
    form_client_id: String,
    page: PageState,
    activity: Vec<String>,
}

/// I-14: what the held app costs the workspaces connected through the app in
/// effect, and the one control that saves it. Never a secret and never a
/// form field: the secrets she typed stay sealed in the pending row.
#[derive(Template)]
#[template(path = "integrations/slack_confirm.html")]
struct SlackConfirmPage {
    nav: crate::nav::NavContext,
    csrf: String,
    /// Where the one control posts.
    confirm_door: &'static str,
    /// I-14, in the form the connected-workspace count selects.
    line: String,
    /// The control's copy.
    control: &'static str,
}

/// I-15: what a removal stops for the workspaces connected through the app
/// in effect, and the one control that carries it out. Never a key and never
/// a secret: the removal needs nothing typed.
#[derive(Template)]
#[template(path = "integrations/slack_remove.html")]
struct SlackRemovePage {
    nav: crate::nav::NavContext,
    csrf: String,
    /// Where the one control posts.
    remove_door: &'static str,
    /// I-15, in the form the connected-workspace count selects.
    line: String,
    /// The control's copy.
    control: &'static str,
}

pub async fn show_instance(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(user) = crate::instance_admin::require_instance_admin(&state, &session).await else {
        return resource_not_found_page();
    };
    let notice = SavedNotice::taken_from(&session)
        .await
        .map(SavedNotice::words);
    answer(&state, &user, &headers, notice, String::new()).await
}

/// `POST /admin/instance/integrations/secrets-key`: Generate.
pub async fn generate_secrets_key(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    key_door(&state, &session, &headers, KeyOffer::Generate).await
}

/// `POST /admin/instance/integrations/secrets-key/move`: Move.
pub async fn move_to_secrets_key(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    key_door(&state, &session, &headers, KeyOffer::Move).await
}

/// `POST /admin/instance/integrations/slack`: save or replace the Slack app.
/// The form is read only past the guard, so every refusal is the uniform
/// 404. The app in effect on this server is read before the submission is
/// judged, so a field left blank keeps the secret that app already holds.
pub async fn save_slack_app(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(user) = crate::instance_admin::require_instance_admin(&state, &session).await else {
        return resource_not_found_page();
    };
    let fields: Vec<(String, String)> = serde_urlencoded::from_bytes(&body).unwrap_or_default();
    let submission = SlackSubmission::from_fields(&fields);
    let in_effect = state.integrations.current();
    let change = match submission.judge(in_effect.client_id_in_effect(), in_effect.saved_secrets())
    {
        Ok(change) => change,
        Err(refusal) => {
            return answer(
                &state,
                &user,
                &headers,
                Some(refused(refusal).to_string()),
                submission.client_id,
            )
            .await
        }
    };
    let answered =
        store_submission(&state, user.user_id, &change, app_in_effect_of(&in_effect)).await;
    match answered {
        SaveAnswer::Stored(saved) => saved.answered(&session).await,
        SaveAnswer::Waiting => Redirect::to(SLACK_CONFIRM_PATH).into_response(),
        SaveAnswer::InPlace(notice) => {
            answer(
                &state,
                &user,
                &headers,
                notice.map(str::to_string),
                String::new(),
            )
            .await
        }
    }
}

/// `GET /admin/instance/integrations/slack/confirm`: I-14 for the app held
/// for this admin's confirm, with the connected-workspace count and the app
/// in effect, and the one control that saves it. It only reads
/// ([`ReadOnlyDoor`]), so opening it decides nothing; with nothing held, or
/// with a count this Foundry could not read, it answers the instance page
/// instead of asking about a change nobody submitted.
pub async fn show_slack_confirm(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(user) = crate::instance_admin::require_instance_admin(&state, &session).await else {
        return resource_not_found_page();
    };
    let store = StoreCapabilities::new(state.store.clone());
    let integrations = state.integrations.current();
    let facts = ReadOnlyDoor::SlackConfirm
        .read(&store, gate_of(&state))
        .await;
    let (Some(true), Some(connected)) = (facts.waiting, facts.connected) else {
        return answer(&state, &user, &headers, None, String::new()).await;
    };
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let nav = crate::nav::NavContext::home_for(
        &state,
        user.user_id,
        user.workspace_id,
        true,
        csrf.clone(),
    )
    .await;
    render(
        SlackConfirmPage {
            nav,
            csrf,
            confirm_door: SLACK_CONFIRM_PATH,
            line: confirm_lines(
                integrations.client_id_in_effect().unwrap_or_default(),
                connected,
            ),
            control: SAVE_NEW_APP,
        }
        .render(),
        set_cookie,
    )
}

/// `POST /admin/instance/integrations/slack/confirm`: the held app applied,
/// for the admin who holds it and before it expires, and the page answered
/// with the notice the save earned (I-10 or I-17). A second confirm, another
/// instance admin's confirm and one at or past the expiry find nothing to
/// apply: they change nothing and answer the page as it is.
pub async fn confirm_slack_app(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(user) = crate::instance_admin::require_instance_admin(&state, &session).await else {
        return resource_not_found_page();
    };
    match apply_held_slack(&state, user.user_id).await {
        SaveAnswer::Stored(saved) => saved.answered(&session).await,
        SaveAnswer::Waiting => Redirect::to(SLACK_CONFIRM_PATH).into_response(),
        SaveAnswer::InPlace(notice) => {
            answer(
                &state,
                &user,
                &headers,
                notice.map(str::to_string),
                String::new(),
            )
            .await
        }
    }
}

/// `GET /admin/instance/integrations/slack/remove`: I-15 over the
/// connected-workspace count, with the one control that removes the app.
/// It only reads ([`ReadOnlyDoor`]), so a prefetch, a link preview and a
/// reload take nothing away. Where the app in effect is not Priya's to
/// remove it answers the instance page instead of asking about a change it
/// would refuse (D7), and so does a count this Foundry could not read: an
/// unread count is not none connected (B1).
pub async fn show_slack_remove(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(user) = crate::instance_admin::require_instance_admin(&state, &session).await else {
        return resource_not_found_page();
    };
    let integrations = state.integrations.current();
    if !removable(&integrations) {
        return answer(&state, &user, &headers, None, String::new()).await;
    }
    let store = StoreCapabilities::new(state.store.clone());
    let read = ReadOnlyDoor::SlackRemove
        .read(&store, gate_of(&state))
        .await;
    let Some(connected) = read.connected else {
        return answer(&state, &user, &headers, None, String::new()).await;
    };
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let nav = crate::nav::NavContext::home_for(
        &state,
        user.user_id,
        user.workspace_id,
        true,
        csrf.clone(),
    )
    .await;
    render(
        SlackRemovePage {
            nav,
            csrf,
            remove_door: SLACK_REMOVE_PATH,
            line: remove_lines(connected),
            control: SlackOffer::Remove.label(),
        }
        .render(),
        set_cookie,
    )
}

/// `POST /admin/instance/integrations/slack/remove`: the saved app removed
/// where that control applies, and the admin sent to the page itself to read
/// I-16 (post, redirect, get, so a reload never posts the removal again). A
/// remove while the environment's app is in effect, and one with nothing
/// saved, change nothing and answer the page as it is.
pub async fn remove_slack_app(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(user) = crate::instance_admin::require_instance_admin(&state, &session).await else {
        return resource_not_found_page();
    };
    match remove_slack(&state, user.user_id).await {
        Some(removed) => removed.answered(&session).await,
        None => answer(&state, &user, &headers, None, String::new()).await,
    }
}

/// Whether the Slack app in effect on this server is Priya's to remove: the
/// same offer the page makes, so the door and the page cannot disagree
/// (D7, L5).
fn removable(integrations: &Integrations) -> bool {
    SlackOffer::Remove.applies(app_in_effect_of(integrations), integrations.key_mode())
}

/// The removal where that control applies, then this server's integrations
/// rebuilt and installed: I-16 once the app is gone. It seals nothing, so
/// there is no fence to turn it back and nothing to retry. Nothing removed
/// is nothing to tell.
async fn remove_slack(state: &AppState, actor: Uuid) -> Option<SavedNotice> {
    let integrations = state.integrations.current();
    if !removable(&integrations) {
        return None;
    }
    let removed = StoreCapabilities::new(state.store.clone())
        .remove_slack_app(actor, state.clock.now())
        .await;
    match removed {
        Ok(RemoveOutcome::Removed { .. }) => {
            reinstall(state, &integrations).await;
            Some(SavedNotice::Removed)
        }
        Ok(RemoveOutcome::NothingSaved) => None,
        Err(err) => {
            tracing::warn!(error = %err, "removing the Slack app failed");
            None
        }
    }
}

/// What a refused save answers: I-11 for a blank field, I-12 for a
/// malformed one, naming it.
fn refused(refusal: Refusal) -> &'static str {
    match refusal {
        Refusal::Blank(SlackField::ClientId) => ENTER_CLIENT_ID,
        Refusal::Blank(SlackField::ClientSecret) => ENTER_CLIENT_SECRET,
        Refusal::Blank(SlackField::SigningSecret) => ENTER_SIGNING_SECRET,
        Refusal::Malformed(SlackField::ClientId) => NOT_A_CLIENT_ID,
        Refusal::Malformed(SlackField::ClientSecret) => NOT_A_CLIENT_SECRET,
        Refusal::Malformed(SlackField::SigningSecret) => NOT_A_SIGNING_SECRET,
    }
}

/// The submission written where a sealing mode holds (else I-3, nothing
/// stored), with the I-14 gate decided inside that write's own locked
/// transaction (D10): both plans are sealed first - a save of the app and a
/// hold of the same submission for its confirm - and the gate the store calls
/// under its lock, [`SubmissionDoor`] over the count it reads there, picks the
/// one that is written. A workspace that connects while this post is in flight
/// is therefore counted by the gate that judges it, so no app is switched
/// under it without the confirm I-14 asks for.
///
/// A save that stored the app rebuilds and installs this server's
/// integrations and answers I-10 (or I-17); a hold changes nothing a reader
/// sees, so it answers the confirm page with this server's integrations as
/// they are. When the seal fence finds a newer sealing key, the integrations
/// are rebuilt and the submission tried once more.
async fn store_submission(
    state: &AppState,
    actor: Uuid,
    change: &SlackChange,
    in_effect: AppInEffect,
) -> SaveAnswer {
    let store = &StoreCapabilities::new(state.store.clone());
    retried(|| async move {
        let integrations = state.integrations.current();
        let Some(sealing) = integrations.sealing_box() else {
            return Attempted::Answered(SaveAnswer::InPlace(Some(NO_SECRETS_KEY)));
        };
        let mode = integrations.key_mode();
        let now = state.clock.now();
        let (Some(saved), Some(held)) = (
            plan_replace(sealing, mode, change, actor, now),
            plan_hold_pending(sealing, mode, change, actor, now),
        ) else {
            tracing::warn!(probe = "secrets", "sealing the Slack app failed");
            return Attempted::Answered(SaveAnswer::InPlace(None));
        };
        let gate = |connected: i64| match SubmissionDoor::of(
            change.kind(),
            connected_count(connected),
            in_effect,
        ) {
            SubmissionDoor::SaveAtOnce => SlackSubmissionPlan::SaveAtOnce(&saved),
            SubmissionDoor::WaitForConfirm => SlackSubmissionPlan::WaitForConfirm(&held),
        };
        match store.store_slack_submission(&gate).await {
            Ok(SubmissionStored::Saved(SlackSaveOutcome::Saved { .. })) => {
                reinstall(state, &integrations).await;
                Attempted::Answered(SaveAnswer::Stored(
                    match integrations.settings().slack.is_some() {
                        true => SavedNotice::EnvironmentWins,
                        false => SavedNotice::InEffect,
                    },
                ))
            }
            Ok(SubmissionStored::Held(SlackSaveOutcome::Saved { .. })) => {
                Attempted::Answered(SaveAnswer::Waiting)
            }
            Ok(SubmissionStored::Saved(SlackSaveOutcome::StaleKey))
            | Ok(SubmissionStored::Held(SlackSaveOutcome::StaleKey)) => {
                reinstall(state, &integrations).await;
                Attempted::StaleKey
            }
            Err(err) => {
                tracing::warn!(error = %err, "storing the submitted Slack app failed");
                Attempted::Answered(SaveAnswer::InPlace(None))
            }
        }
    })
    .await
}

/// The count the gate is judged over, as the store answers it: a count past
/// what the I-14 forms can carry reads as the most they can, which the gate
/// sends to the confirm either way.
fn connected_count(connected: i64) -> u32 {
    u32::try_from(connected).unwrap_or(u32::MAX)
}

/// What a save answers: the app is stored, so the admin is sent to the page
/// itself to read the notice it earned (post, redirect, get, so a reload
/// never posts the submission again); the app waits for her confirm, so she
/// is sent to the confirm page, which holds no secret she typed; or nothing
/// was stored and the page is answered in place, with what to tell her
/// (`None` where only the log knows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SaveAnswer {
    Stored(SavedNotice),
    Waiting,
    InPlace(Option<&'static str>),
}

/// The held app applied for `actor` at this server's clock, then this
/// server's integrations rebuilt and installed: I-10 (or I-17) once it is
/// stored. Nothing applicable to her at this instant answers the page with
/// nothing to tell. The fence is retried as a save's is.
async fn apply_held_slack(state: &AppState, actor: Uuid) -> SaveAnswer {
    let store = &StoreCapabilities::new(state.store.clone());
    retried(|| async move {
        let integrations = state.integrations.current();
        let Some(sealing) = integrations.sealing_box() else {
            return Attempted::Answered(SaveAnswer::InPlace(Some(NO_SECRETS_KEY)));
        };
        let credentials = integrations.credentials();
        let reseal = |client: &[u8], signing: &[u8]| {
            reseal_held_for_save(&credentials, sealing, client, signing).ok_or(ResealRefused)
        };
        let outcome = store
            .apply_pending_slack_app(
                actor,
                state.clock.now(),
                integrations.sealed_under().as_deref(),
                &reseal,
            )
            .await;
        match outcome {
            Ok(PendingApplied::Applied(SlackSaveOutcome::Saved { .. })) => {
                reinstall(state, &integrations).await;
                Attempted::Answered(SaveAnswer::Stored(
                    match integrations.settings().slack.is_some() {
                        true => SavedNotice::EnvironmentWins,
                        false => SavedNotice::InEffect,
                    },
                ))
            }
            Ok(PendingApplied::Applied(SlackSaveOutcome::StaleKey)) => {
                reinstall(state, &integrations).await;
                Attempted::StaleKey
            }
            Ok(PendingApplied::NotApplicable) => Attempted::Answered(SaveAnswer::InPlace(None)),
            Ok(PendingApplied::Refused(ResealRefused)) => {
                tracing::warn!(
                    probe = "secrets",
                    "the held Slack app's secrets could not be sealed for the saved app; \
                     nothing changed"
                );
                Attempted::Answered(SaveAnswer::InPlace(None))
            }
            Err(err) => {
                tracing::warn!(error = %err, "applying the held Slack app failed");
                Attempted::Answered(SaveAnswer::InPlace(None))
            }
        }
    })
    .await
}

/// One try at a fenced save: its answer, or the fence found a newer
/// sealing key.
enum Attempted {
    Answered(SaveAnswer),
    StaleKey,
}

/// `attempt` up to [`SAVE_ATTEMPTS`] times while the fence finds a newer
/// sealing key; the first answer otherwise, and [`SAVE_TURNED_BACK`] when
/// every try was turned back.
async fn retried<F, Fut>(mut attempt: F) -> SaveAnswer
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Attempted>,
{
    for _ in 0..SAVE_ATTEMPTS {
        if let Attempted::Answered(answer) = attempt().await {
            return answer;
        }
    }
    SaveAnswer::InPlace(Some(SAVE_TURNED_BACK))
}

/// Who saved the app in effect, by name.
async fn saver_name(state: &AppState, integrations: &Integrations) -> Option<String> {
    let saved_by = integrations.saved_slack()?.saved_by?;
    match StoreCapabilities::new(state.store.clone())
        .display_name(saved_by)
        .await
    {
        Ok(name) => name,
        Err(err) => {
            tracing::warn!(error = %err, "reading who saved the Slack app failed");
            None
        }
    }
}

/// The stored values sealed directly under a held instance key, as stored
/// now: a connection made since the last rebuild counts too.
async fn direct_seals(state: &AppState, integrations: &Integrations) -> usize {
    match StoreCapabilities::new(state.store.clone())
        .instance_snapshot()
        .await
    {
        Ok(snapshot) => integrations.direct_seals(&snapshot),
        Err(err) => {
            tracing::warn!(error = %err, "reading the stored credentials failed");
            0
        }
    }
}

/// The page as this server's integrations are now, with `notice` above it.
pub(super) async fn answer(
    state: &AppState,
    user: &SessionUser,
    headers: &HeaderMap,
    notice: Option<String>,
    typed_client_id: String,
) -> Response {
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let integrations = state.integrations.current();
    let saver = saver_name(state, &integrations).await;
    let direct_seals = direct_seals(state, &integrations).await;
    // The replica rows decide a word of this page only where the stored
    // secrets key cannot be opened and a new one could be sealed; everywhere
    // else the gate stays unread, and the offer is the one an unread gate
    // answers anyway (F2, L6).
    let opener = match UnopenableOffer::reads_gate(integrations.key_mode(), integrations.key_file())
    {
        true => live_opener(&StoreCapabilities::new(state.store.clone()), gate_of(state)).await,
        false => None,
    };
    let page = page_state(&facts_of(&integrations, direct_seals, saver, opener));
    let form_client_id = page.form_client_id(&typed_client_id).to_string();
    let activity = match StoreCapabilities::new(state.store.clone())
        .instance_activity(ACTIVITY_LINES)
        .await
    {
        Ok(rows) => rows.iter().map(activity_line).collect(),
        Err(err) => {
            tracing::warn!(error = %err, "reading the instance activity failed");
            Vec::new()
        }
    };
    let nav = crate::nav::NavContext::home_for(
        state,
        user.user_id,
        user.workspace_id,
        true,
        csrf.clone(),
    )
    .await;
    render(
        InstancePage {
            nav,
            csrf,
            slack_door: SLACK_APP_PATH,
            notice,
            form_client_id,
            page,
            activity,
        }
        .render(),
        set_cookie,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "/data/instance.key";

    fn addresses(fell_back_to: Option<&str>) -> SlackAddresses {
        SlackAddresses {
            manifest: "{\"manifest\": true}".to_string(),
            redirect_url: "https://foundry.example.org/workspace/integrations/slack/return".into(),
            events_url: "https://foundry.example.org/integrations/slack/events".into(),
            fell_back_to: fell_back_to.map(str::to_string),
        }
    }

    fn facts(mode: KeyMode, operator_key: bool, saved_direct: bool) -> PageFacts {
        PageFacts {
            mode,
            operator_key,
            key_file: FILE.to_string(),
            key_file_read: KeyFile::Held,
            rewrapped_from: None,
            saved_direct,
            unopenable_values: false,
            live_opener: Some(false),
            slack_app: false,
            app_in_effect: AppInEffect::None,
            environment_slack: None,
            saved_slack: None,
            saved_unreadable: false,
            addresses: addresses(None),
        }
    }

    const ALL_MODES: [KeyMode; 6] = [
        KeyMode::NoInstanceKey(NoKeyReason::Unwritable),
        KeyMode::NoInstanceKey(NoKeyReason::Unreadable),
        KeyMode::Direct,
        KeyMode::AwaitingSecretsKey,
        KeyMode::Sealed,
        KeyMode::Unopenable,
    ];

    fn file_line() -> String {
        KEY_IN_FILE.replace("{path}", FILE)
    }

    /// Facts in (mode, operator key, key file, direct seals), then the page
    /// expected.
    type Row = (
        KeyMode,
        bool,
        KeyFile,
        bool,
        Option<String>,
        Option<&'static str>,
        Option<KeyOffer>,
        &'static str,
    );

    /// Every mode, with the key from the operator and from the file, and with
    /// or without direct seals: the page's lines and its one offer. With no
    /// key file made (Withheld, ADR-IIS-002 r3) there is no key-file line:
    /// I-25 would claim a file that does not exist (item 39).
    #[test]
    fn the_page_follows_the_mode() {
        use KeyFile::{Held, Unreadable, Unwritable, Withheld};
        use KeyMode::*;
        let unwritable = Some(KEY_UNWRITABLE.replace("{path}", FILE));
        let unreadable = Some(KEY_UNREADABLE.replace("{path}", FILE));
        let op = Some(KEY_BY_OPERATOR.to_string());
        let file = Some(file_line());
        #[rustfmt::skip]
        let table: Vec<Row> = vec![
            (NoInstanceKey(NoKeyReason::Unwritable), false, Unwritable, false, unwritable, None, None, MATRIX_NEEDS_KEY),
            (NoInstanceKey(NoKeyReason::Unreadable), false, Unreadable, false, unreadable, None, None, MATRIX_NEEDS_KEY),
            (AwaitingSecretsKey, true, Unwritable, false, op.clone(), Some(NO_SECRETS_KEY), Some(KeyOffer::Generate), MATRIX_NEEDS_KEY),
            (AwaitingSecretsKey, false, Held, false, file.clone(), Some(NO_SECRETS_KEY), Some(KeyOffer::Generate), MATRIX_NEEDS_KEY),
            (AwaitingSecretsKey, false, Withheld, false, None, Some(NO_SECRETS_KEY), Some(KeyOffer::Generate), MATRIX_NEEDS_KEY),
            (Direct, true, Unwritable, false, op.clone(), Some(DIRECT_NOTHING_SAVED), Some(KeyOffer::Move), MATRIX_READY),
            (Direct, true, Unwritable, true, op.clone(), Some(DIRECT_SAVED), Some(KeyOffer::Move), MATRIX_READY),
            (Direct, false, Held, true, file.clone(), Some(DIRECT_SAVED), Some(KeyOffer::Move), MATRIX_READY),
            (Sealed, true, Unwritable, false, op.clone(), Some(SECRETS_KEY_SET), Some(KeyOffer::Replace), MATRIX_READY),
            (Sealed, false, Held, true, file.clone(), Some(SECRETS_KEY_SET), Some(KeyOffer::Replace), MATRIX_READY),
            (Unopenable, true, Unwritable, false, op.clone(), Some(SECRETS_KEY_UNOPENABLE), None, MATRIX_NEEDS_KEY),
            (Unopenable, false, Held, true, file.clone(), Some(SECRETS_KEY_UNOPENABLE), None, MATRIX_NEEDS_KEY),
            (Unopenable, false, Withheld, false, None, Some(SECRETS_KEY_UNOPENABLE), None, MATRIX_NEEDS_KEY),
        ];
        for (
            mode,
            operator_key,
            key_file,
            saved_direct,
            instance_key,
            secrets_key,
            offer,
            matrix,
        ) in table
        {
            let facts = PageFacts {
                key_file_read: key_file,
                ..facts(mode, operator_key, saved_direct)
            };
            let page = page_state(&facts);
            assert_eq!(
                (page.instance_key, page.secrets_key, page.offer, page.matrix),
                (instance_key, secrets_key, offer, matrix),
                "{facts:?}"
            );
        }
    }

    /// Every mode, with I-4 or without: Generate is offered exactly where
    /// the gate lets it apply.
    #[test]
    fn generate_is_offered_only_where_it_applies() {
        for mode in ALL_MODES {
            for unopenable_values in [false, true] {
                let facts = PageFacts {
                    unopenable_values,
                    ..facts(mode, true, false)
                };
                let offered = page_state(&facts).offer == Some(KeyOffer::Generate);
                assert_eq!(offered, mode.generates(unopenable_values), "{facts:?}");
            }
        }
    }

    /// I-5 and its addresses show only with a secrets key and no Slack app;
    /// I-29 names the fallback above them only when the address fell back.
    #[test]
    fn slacks_addresses_show_with_a_secrets_key_and_no_app() {
        for mode in ALL_MODES {
            for slack_app in [false, true] {
                for fell_back_to in [None, Some("http://localhost:8080")] {
                    let facts = PageFacts {
                        slack_app,
                        addresses: addresses(fell_back_to),
                        ..facts(mode, true, false)
                    };
                    let expected = (mode == KeyMode::Sealed && !slack_app).then(|| SlackSetup {
                        warning: fell_back_to.map(|_| "FOUNDRY_PUBLIC_URL is not set, so the addresses below use http://localhost:8080, which Slack cannot reach. An instance operator sets FOUNDRY_PUBLIC_URL to this Foundry's public address and restarts Foundry.".to_string()),
                        line: SLACK_NOT_SET_UP,
                        addresses: addresses(fell_back_to),
                    });
                    assert_eq!(page_state(&facts).slack, expected, "{facts:?}");
                }
            }
        }
    }

    /// Every mode, with or without the environment's app: I-7 names its
    /// client ID exactly when it is set, and the form shows under it only
    /// where credentials seal.
    #[test]
    fn the_environments_app_is_named_in_every_mode() {
        const ENV_APP: &str = "6123456789012.7234567890123";
        let i7 = "Slack: set in Foundry's environment, app 6123456789012.7234567890123. To manage it here, enter the same app below; it takes over once SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET are removed from the environment.";
        use KeyMode::*;
        #[rustfmt::skip]
        let table: [(KeyMode, bool, Option<&str>, bool); 12] = [
            (NoInstanceKey(NoKeyReason::Unwritable), true,  Some(i7), false),
            (NoInstanceKey(NoKeyReason::Unwritable), false, None,     false),
            (NoInstanceKey(NoKeyReason::Unreadable), true,  Some(i7), false),
            (NoInstanceKey(NoKeyReason::Unreadable), false, None,     false),
            (AwaitingSecretsKey,                     true,  Some(i7), false),
            (AwaitingSecretsKey,                     false, None,     false),
            (Direct,                                 true,  Some(i7), true),
            (Direct,                                 false, None,     true),
            (Sealed,                                 true,  Some(i7), true),
            (Sealed,                                 false, None,     true),
            (Unopenable,                             true,  Some(i7), false),
            (Unopenable,                             false, None,     false),
        ];
        for (mode, environment, i7_line, form) in table {
            let facts = PageFacts {
                slack_app: environment,
                environment_slack: environment.then(|| ENV_APP.to_string()),
                ..facts(mode, false, mode == Direct)
            };
            let page = page_state(&facts);
            assert_eq!(page.environment_slack.as_deref(), i7_line, "{facts:?}");
            assert_eq!(page.slack_form, form, "{facts:?}");
        }
    }

    /// One Integrations: the page's manifest is the setup page's, and both
    /// addresses it shows are the ones the manifest names.
    #[test]
    fn the_pages_addresses_and_manifest_come_from_one_integrations() {
        use crate::integrations::config::IntegrationSettings;
        let mut settings = IntegrationSettings::off("http://localhost:8080");
        settings.public_url_fell_back = true;
        let integrations = Integrations::new(settings);
        let addresses = facts_of(&integrations, 0, None, Some(false)).addresses;
        assert_eq!(addresses.manifest, slack::manifest(&integrations));
        assert_eq!(
            addresses.redirect_url,
            "http://localhost:8080/workspace/integrations/slack/return"
        );
        assert_eq!(
            addresses.events_url,
            "http://localhost:8080/integrations/slack/events"
        );
        for url in [&addresses.redirect_url, &addresses.events_url] {
            assert!(addresses.manifest.contains(url.as_str()), "{url}");
        }
        assert_eq!(
            addresses.fell_back_to.as_deref(),
            Some("http://localhost:8080")
        );
    }

    #[test]
    fn an_activity_line_names_its_date_and_actor() {
        let row = |actor: Option<&str>| InstanceActivityRow {
            created_at: time::macros::datetime!(2026-10-21 14:02 UTC),
            actor_name: actor.map(str::to_string),
            action: "secrets_key_generated".into(),
            client_id: None,
            count: None,
        };
        assert_eq!(
            activity_line(&row(Some("Priya Raman"))),
            "21 Oct 2026 · Priya Raman generated the secrets key"
        );
        assert_eq!(
            activity_line(&row(None)),
            "21 Oct 2026 · Foundry generated the secrets key"
        );
    }

    /// Every mode, with I-4 and without: Generate is offered only while
    /// awaiting a secrets key, Move only while sealing directly, Replace only
    /// while the stored key opens (US-IIS-07, D4), none of the three under
    /// I-4, and I-4 names itself wherever the key line shows (I-26 keeps its
    /// own words).
    #[test]
    fn generate_and_move_are_withheld_under_i4_in_every_mode() {
        use KeyMode::*;
        let unwritable = NoInstanceKey(NoKeyReason::Unwritable);
        let unreadable = NoInstanceKey(NoKeyReason::Unreadable);
        #[rustfmt::skip]
        let table = [
            (unwritable,         false, None,                    None),
            (unwritable,         true,  None,                    None),
            (unreadable,         false, None,                    None),
            (unreadable,         true,  None,                    None),
            (AwaitingSecretsKey, false, Some(KeyOffer::Generate), Some(NO_SECRETS_KEY)),
            (AwaitingSecretsKey, true,  None,                    Some(VALUES_UNOPENABLE)),
            (Direct,             false, Some(KeyOffer::Move),    Some(DIRECT_SAVED)),
            (Direct,             true,  None,                    Some(VALUES_UNOPENABLE)),
            (Sealed,             false, Some(KeyOffer::Replace), Some(SECRETS_KEY_SET)),
            (Sealed,             true,  None,                    Some(VALUES_UNOPENABLE)),
            (Unopenable,         false, None,                    Some(SECRETS_KEY_UNOPENABLE)),
            (Unopenable,         true,  None,                    Some(SECRETS_KEY_UNOPENABLE)),
        ];
        for (mode, unopenable_values, offer, line) in table {
            let facts = PageFacts {
                unopenable_values,
                ..facts(mode, true, true)
            };
            let page = page_state(&facts);
            assert_eq!((page.offer, page.secrets_key), (offer, line), "{facts:?}");
            for door in [KeyOffer::Generate, KeyOffer::Move, KeyOffer::Replace] {
                assert_eq!(
                    door.applies(mode, unopenable_values),
                    offer == Some(door),
                    "{door:?} {facts:?}"
                );
            }
        }
    }

    /// M1 / item 35: a save the fence turns back as often as it is tried
    /// answers a visible refusal, never nothing; a save the fence turns back
    /// once is tried once more and answers that try.
    #[tokio::test]
    async fn a_save_turned_back_by_a_newer_key_every_try_still_answers() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let tries = &AtomicUsize::new(0);
        let answer = retried(|| async move {
            tries.fetch_add(1, Ordering::SeqCst);
            Attempted::StaleKey
        })
        .await;
        assert_eq!(tries.load(Ordering::SeqCst), SAVE_ATTEMPTS);
        assert_eq!(answer, SaveAnswer::InPlace(Some(SAVE_TURNED_BACK)));

        let tries = &AtomicUsize::new(0);
        let answer = retried(|| async move {
            match tries.fetch_add(1, Ordering::SeqCst) {
                0 => Attempted::StaleKey,
                _ => Attempted::Answered(SaveAnswer::Stored(SavedNotice::InEffect)),
            }
        })
        .await;
        assert_eq!(
            (tries.load(Ordering::SeqCst), answer),
            (2, SaveAnswer::Stored(SavedNotice::InEffect))
        );
    }

    #[test]
    fn a_move_line_names_how_many_credentials_moved() {
        let row = |count: i32| InstanceActivityRow {
            created_at: time::macros::datetime!(2026-10-21 14:02 UTC),
            actor_name: Some("Priya Raman".to_string()),
            action: "credentials_moved".into(),
            client_id: None,
            count: Some(count),
        };
        assert_eq!(
            activity_line(&row(2)),
            "21 Oct 2026 · Priya Raman moved 2 credentials under a secrets key"
        );
        assert_eq!(
            activity_line(&row(1)),
            "21 Oct 2026 · Priya Raman moved 1 credential under a secrets key"
        );
    }

    /// The page's facts over the integrations in effect: "saved direct" only
    /// with some direct seal, "saved unreadable" only for a saved app that
    /// does not open.
    #[test]
    fn the_page_facts_follow_the_integrations_in_effect() {
        let mut settings =
            crate::integrations::config::IntegrationSettings::off("https://foundry.example.org");
        settings.data_dir = "/data".into();
        let integrations = Integrations::new(settings);
        let none = facts_of(&integrations, 0, None, Some(false));
        assert!(!none.saved_direct, "no direct seal");
        assert!(!none.saved_unreadable, "no saved app");
        assert_eq!(none.key_file, FILE);
        assert!(
            facts_of(&integrations, 1, None, Some(false)).saved_direct,
            "one"
        );
        assert!(
            facts_of(&integrations, 2, None, Some(false)).saved_direct,
            "two"
        );
    }

    /// L3 and L11: the form I-14 and I-15 each read at the
    /// connected-workspace count, and where the gate sends a submission over
    /// that same count and the app in effect. The count partitions at 0, at 1 and above 1, so
    /// these cases decide the whole mapping: a page that always read one
    /// form, or read the plural form for one workspace, is caught here.
    #[test]
    fn the_count_forms_i14_and_i15_read_and_the_gate_the_connected_count_decides() {
        const CURRENT: &str = "6123456789012.7234567890123";
        use SubmissionDoor::{SaveAtOnce, WaitForConfirm};
        use SubmissionKind::{NewApp, Rotate};
        // The connected count, then where each kind of submission goes while
        // an app is in effect - the saved app or the environment's alike,
        // because I-14 names whichever is in effect.
        #[rustfmt::skip]
        let gate = [
            (0, SaveAtOnce,     SaveAtOnce),
            (1, WaitForConfirm, SaveAtOnce),
            (2, WaitForConfirm, SaveAtOnce),
            (3, WaitForConfirm, SaveAtOnce),
            (137, WaitForConfirm, SaveAtOnce),
        ];
        for (connected, new_app, rotate) in gate {
            for in_effect in [AppInEffect::Saved, AppInEffect::Environment] {
                assert_eq!(
                    SubmissionDoor::of(NewApp, connected, in_effect),
                    new_app,
                    "{connected} connected, {in_effect:?} in effect"
                );
                assert_eq!(
                    SubmissionDoor::of(Rotate, connected, in_effect),
                    rotate,
                    "{connected} connected, {in_effect:?} in effect"
                );
            }
            // With no app in effect nothing is being switched and I-14 has
            // no app to name, so this Foundry's first app is saved at once
            // whatever a removed app left connected.
            assert_eq!(
                SubmissionDoor::of(NewApp, connected, AppInEffect::None),
                SaveAtOnce,
                "{connected} connected, no app in effect"
            );
        }
        // The count I-14 is rendered over, then the line it reads. A count of
        // none never reaches it: the gate above saves at once.
        let one = "1 workspace is connected through the current Slack app (6123456789012.7234567890123). It keeps posting through it until a workspace admin reconnects Slack. Save the new app?";
        let two = "2 workspaces are connected through the current Slack app (6123456789012.7234567890123). They keep posting through it until a workspace admin reconnects Slack. Save the new app?";
        let three = "3 workspaces are connected through the current Slack app (6123456789012.7234567890123). They keep posting through it until a workspace admin reconnects Slack. Save the new app?";
        for (connected, line) in [(1, one), (2, two), (3, three)] {
            assert_eq!(confirm_lines(CURRENT, connected), line, "n = {connected}");
        }
        // The count I-15 is rendered over, then the line it reads. Nobody
        // connected has its own form: no workspace's routes stop posting,
        // and none can connect Slack afterwards either.
        let removes_none =
            "Remove the Slack app from this Foundry? No workspace is connected to Slack.";
        let removes_one = "Remove the Slack app from this Foundry? 1 workspace's Slack routes stop posting until it is set up again, and no workspace can connect Slack.";
        let removes_two = "Remove the Slack app from this Foundry? 2 workspaces' Slack routes stop posting until it is set up again, and no workspace can connect Slack.";
        let removes_three = "Remove the Slack app from this Foundry? 3 workspaces' Slack routes stop posting until it is set up again, and no workspace can connect Slack.";
        #[rustfmt::skip]
        let forms = [
            (0, removes_none),
            (1, removes_one),
            (2, removes_two),
            (3, removes_three),
        ];
        for (connected, line) in forms {
            assert_eq!(remove_lines(connected), line, "n = {connected}");
        }
    }

    /// L5: the controls the page offers over each app that can be in effect,
    /// read over the environment's trio and the saved row's own state, and
    /// the gate the remove door judges over that same answer. Replace and
    /// Remove are offered only where the app in effect is the one saved
    /// here; under the app Foundry's environment sets - with an app also
    /// saved here and with none - with a saved app no held key opens, and
    /// with no app at all, neither control is offered and the remove door
    /// refuses, so a POST through it changes nothing. The offer set is a
    /// pure function of two closed enums, so every pair of them is read.
    #[test]
    fn the_table_of_the_controls_the_page_offers_over_each_app_in_effect() {
        const ENV_APP: &str = "6123456789012.7234567890123";
        const SAVED_APP: &str = "6123456789012.8888888888888";
        let both = &[SlackOffer::Replace, SlackOffer::Remove][..];
        let neither = &[][..];
        // The environment's trio set, the saved row's state, the controls.
        #[rustfmt::skip]
        let table: [(bool, SavedApp, &[SlackOffer]); 5] = [
            (true,  SavedApp::Opens,       neither),
            (true,  SavedApp::Absent,      neither),
            (true,  SavedApp::DoesNotOpen, neither),
            (false, SavedApp::Opens,       both),
            (false, SavedApp::Absent,      neither),
        ];
        for (environment, saved, offers) in table {
            let effect = app_in_effect(environment, saved);
            let facts = PageFacts {
                app_in_effect: effect,
                slack_app: effect != AppInEffect::None,
                environment_slack: environment.then(|| ENV_APP.to_string()),
                saved_slack: (effect == AppInEffect::Saved).then(|| SavedSlack {
                    client_id: SAVED_APP.to_string(),
                    saved_by: "Priya Raman".to_string(),
                    saved_at: time::macros::datetime!(2026-10-21 14:02 UTC),
                }),
                saved_unreadable: !environment && saved == SavedApp::DoesNotOpen,
                ..facts(KeyMode::Sealed, true, false)
            };
            let page = page_state(&facts);
            assert_eq!(page.slack_offers, offers.to_vec(), "{facts:?}");
            // The form's own control is the Replace control exactly where
            // Replace applies, and a first save's words anywhere else.
            assert_eq!(
                page.slack_form_control(),
                match offers.is_empty() {
                    true => SAVE_SLACK_APP,
                    false => "Replace",
                },
                "{facts:?}"
            );
            // The remove door reads the same rule, so a POST where no
            // control is offered removes nothing.
            assert_eq!(
                SlackOffer::Remove.applies(effect, facts.mode),
                !offers.is_empty(),
                "{facts:?}"
            );
        }
        // A saved app in effect in every mode: the controls follow the mode's
        // own sealing, so neither is offered where credentials cannot seal.
        for mode in ALL_MODES {
            let facts = PageFacts {
                app_in_effect: AppInEffect::Saved,
                slack_app: true,
                ..facts(mode, true, false)
            };
            let offered = !page_state(&facts).slack_offers.is_empty();
            assert_eq!(offered, mode.seals(), "{facts:?}");
            for offer in [SlackOffer::Replace, SlackOffer::Remove] {
                assert_eq!(
                    offer.applies(AppInEffect::Saved, mode),
                    offered,
                    "{offer:?} {facts:?}"
                );
            }
        }
    }

    /// L6, L4 and C2: what the confirm, remove and replace pages reach when
    /// they are only read.
    /// Every call either door makes is a [`PageRead`], a type that holds no
    /// write at all, so opening either page - twice, as a prefetch and a
    /// reload do - changes no row, writes no activity line and bumps no
    /// config version. A handler reaching a write would have to name it
    /// here, and no [`PageRead`] names one.
    #[test]
    fn the_table_of_what_the_confirm_and_remove_pages_reach_when_they_are_only_read() {
        #[rustfmt::skip]
        let table = [
            (
                ReadOnlyDoor::SlackConfirm,
                [PageRead::Snapshot, PageRead::ConnectedWorkspaces].as_slice(),
            ),
            (
                ReadOnlyDoor::SlackRemove,
                [PageRead::ConnectedWorkspaces].as_slice(),
            ),
            (
                ReadOnlyDoor::SecretsKeyReplace,
                [PageRead::Snapshot].as_slice(),
            ),
            (
                ReadOnlyDoor::SecretsKeyStartOver,
                [PageRead::Snapshot, PageRead::ReplicaStates].as_slice(),
            ),
        ];
        for (door, reads) in table {
            assert_eq!(door.reads(), reads, "{door:?}");
        }
    }

    /// Facts in (mode, key file, gate as it was read), then the secrets-key
    /// line, the sentence appended to it, the control offered and whether the
    /// way out is offered.
    type UnopenableRow = (
        KeyMode,
        KeyFile,
        Option<bool>,
        Option<&'static str>,
        Option<&'static str>,
        Option<KeyOffer>,
        bool,
    );

    /// L6 and L7 on the page itself: what Priya reads and is offered in each
    /// state of a stored secrets key this Foundry cannot open, and in the
    /// state where no instance key is read at all (I-31).
    ///
    /// The shipped I-26 restore words stay as they are in all three I-26
    /// states - the way out is appended to them and the live-server sentence
    /// beside them, never in their place. Under I-31 the page reads its own
    /// instance-key words, no I-26 words at all, and offers neither way out
    /// and no sentence about another server. Replace and Start over are never
    /// both offered: one belongs to a key that opens, the other to one that
    /// does not.
    #[test]
    fn the_table_of_what_the_page_offers_while_the_stored_key_cannot_be_opened() {
        let live_sentence =
            "Another Foundry server still opens the secrets key, so Start over is not offered.";
        use KeyMode::{Sealed, Unopenable};
        // The mode, the key file, the gate as it was read, then the
        // secrets-key line, the sentence appended to it, the control offered
        // and the way out offered.
        #[rustfmt::skip]
        let table: [UnopenableRow; 8] = [
            (Unopenable, KeyFile::Unwritable, Some(false), Some(SECRETS_KEY_UNOPENABLE), None,                 None,                    true),
            (Unopenable, KeyFile::Held,       Some(false), Some(SECRETS_KEY_UNOPENABLE), None,                 None,                    true),
            (Unopenable, KeyFile::Unwritable, Some(true),  Some(SECRETS_KEY_UNOPENABLE), Some(live_sentence),  None,                    false),
            // Nothing held to seal a new key with, and an unread gate: the
            // restore words alone, claiming nothing.
            (Unopenable, KeyFile::Withheld,   Some(false), Some(SECRETS_KEY_UNOPENABLE), None,                 None,                    false),
            (Unopenable, KeyFile::Unwritable, None,        Some(SECRETS_KEY_UNOPENABLE), None,                 None,                    false),
            // I-31: its own words in the instance key line's place, and none
            // of I-26's, whatever the other servers of this database say.
            (KeyMode::NoInstanceKey(NoKeyReason::Unreadable), KeyFile::Unreadable, Some(false), None, None,    None,                    false),
            (KeyMode::NoInstanceKey(NoKeyReason::Unreadable), KeyFile::Unreadable, Some(true),  None, None,    None,                    false),
            // A key that opens: Replace's own state, and no way to start over.
            (Sealed,     KeyFile::Held,       Some(false), Some(SECRETS_KEY_SET),        None,                 Some(KeyOffer::Replace), false),
        ];
        for (mode, key_file, live_opener, line, appended, offer, start_over) in table {
            let facts = PageFacts {
                key_file_read: key_file,
                live_opener,
                ..facts(mode, key_file != KeyFile::Held, false)
            };
            let page = page_state(&facts);
            assert_eq!(
                (page.secrets_key, page.secrets_key_appended, page.offer),
                (line, appended, offer),
                "{facts:?}"
            );
            assert_eq!(page.start_over.is_some(), start_over, "{facts:?}");
            if let Some(way) = page.start_over {
                assert_eq!(way.door, START_OVER_PATH);
                assert_eq!(way.label, "Start over");
            }
            // Neither way out is offered beside the other (L7), and the
            // sentence that says why the way out is withheld is never read
            // beside the way out itself.
            assert!(
                !(start_over && page.offer == Some(KeyOffer::Replace)),
                "{facts:?}"
            );
            assert!(
                !(start_over && page.secrets_key_appended.is_some()),
                "{facts:?}"
            );
            // The I-26 words are read in the I-26 states only, and the ones
            // appended to them never take their place.
            assert_eq!(
                page.secrets_key == Some(SECRETS_KEY_UNOPENABLE),
                mode == Unopenable,
                "{facts:?}"
            );
        }
    }

    /// I-11 names the blank field, I-12 the malformed one.
    #[test]
    fn a_refused_save_names_its_field() {
        use Refusal::{Blank, Malformed};
        use SlackField::{ClientId, ClientSecret, SigningSecret};
        #[rustfmt::skip]
        let table = [
            (Blank(ClientId),          ENTER_CLIENT_ID),
            (Blank(ClientSecret),      ENTER_CLIENT_SECRET),
            (Blank(SigningSecret),     ENTER_SIGNING_SECRET),
            (Malformed(ClientId),      NOT_A_CLIENT_ID),
            (Malformed(ClientSecret),  NOT_A_CLIENT_SECRET),
            (Malformed(SigningSecret), NOT_A_SIGNING_SECRET),
        ];
        for (refusal, answer) in table {
            assert!(!answer.is_empty());
            assert_eq!(refused(refusal), answer);
        }
    }
}
