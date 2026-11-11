//! The instance integrations page, `GET /admin/instance/integrations`
//! (US-IIS-01), and its Generate, `POST …/secrets-key` (US-IIS-02): both
//! behind `require_instance_admin`, the uniform 404 for everyone else. What
//! the page shows and offers is [`page_state`], a pure function of the key
//! mode, where the instance key comes from and Slack's addresses.
//!
//! Generate applies only where [`KeyMode::generates`] holds; anywhere else it
//! changes nothing and answers the page as it is. It builds the plan (a new
//! key wrapped under the instance key, and its canary), hands it to the one
//! write use-case, and after the commit rebuilds this server's integrations
//! through the shared boot probe ([`Integrations::reboot`]) and installs them
//! before it answers I-21. Other replicas follow on their reload (ADR-IIS-004).
//!
//! Move, `POST …/secrets-key/move` (US-IIS-06), is the same use-case where
//! [`KeyMode::moves`] holds: every value sealed directly under a held
//! instance key is re-sealed under the new key in the same transaction, and
//! it answers I-22 (or I-21 with nothing saved). Both gates are judged over
//! the stored rows as they are at the post, never the last rebuild's, and
//! both are withheld under I-4.
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
//! page itself naming the notice she lands on (I-10 or I-17), as every other
//! door of this feature's handlers answers a write, so a reload never posts
//! the secrets again; a refusal and a save that stored nothing answer the
//! page in place. The Activity reads a replacement in I-18's words.

use super::{reinstall, render};
use crate::bootstrap::{resource_not_found_page, SessionUser};
use crate::csrf::ensure_csrf_cookie;
use crate::integrations::keys::{plan_generate, KeyFile, KeyMode, NoKeyReason};
use crate::integrations::slack_app::{
    client_id_in_effect, plan_replace, Refusal, SavedApp, SlackChange, SlackField, SlackSubmission,
};
use crate::integrations::store_capabilities::{
    InstanceKeyWrites, KeyProbe, ResealRefused, StoreCapabilities,
};
use crate::integrations::{sealed_slot, slack, Integrations};
use crate::AppState;
use askama::Template;
use axum::body::Bytes;
use axum::extract::{RawQuery, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use foundry_store::integrations::{
    InstanceActivityRow, KeyWrapper, SecretsKeyOutcome, SlackSaveOutcome,
};
use serde::Deserialize;
use time::OffsetDateTime;
use tower_sessions::Session;
use uuid::Uuid;

pub const INSTANCE_PATH: &str = "/admin/instance/integrations";
pub const SECRETS_KEY_PATH: &str = "/admin/instance/integrations/secrets-key";
pub const MOVE_PATH: &str = "/admin/instance/integrations/secrets-key/move";
pub const SLACK_APP_PATH: &str = "/admin/instance/integrations/slack";

/// I-25, operator-set.
const KEY_BY_OPERATOR: &str = "Instance key: set by the operator.";
/// I-30: the operator's key took over Foundry's file.
const KEY_REPLACED_FILE: &str = "Instance key: set by the operator. The secrets key, first sealed with {path}, is now sealed with the operator's key. Keep a backup of {path} until you are sure, then remove it.";
/// I-25, made by Foundry.
const KEY_IN_FILE: &str = "Instance key: made by Foundry in {path}. Back up this file apart from the database. Every Foundry server must use the same file. Without it, saved connections must be reconnected.";
/// I-27.
const KEY_UNWRITABLE: &str = "Foundry has no instance key and cannot make one ({path} cannot be written). An instance operator sets the instance key, or gives Foundry a writable data directory, and restarts Foundry.";
/// I-31.
const KEY_UNREADABLE: &str = "This Foundry cannot read its instance key file ({path}). Slack and Matrix are paused until it can. An instance operator restores the file from backup, or sets FOUNDRY_INSTANCE_KEY, and restarts Foundry.";
/// I-2.
const SECRETS_KEY_SET: &str = "Secrets key: set. Credentials entered here are sealed with it.";
/// I-2e.
const DIRECT_SAVED: &str = "Saved credentials are sealed directly with the instance key. Move them under a secrets key to manage it here.";
/// I-2e0.
const DIRECT_NOTHING_SAVED: &str = "Credentials saved here are sealed directly with the instance key. Move to a secrets key to manage it here.";
/// I-3.
const NO_SECRETS_KEY: &str =
    "This Foundry has no secrets key yet. Foundry needs one to store Slack and Matrix credentials.";
/// I-4: some saved credential opens under no key this server holds.
const VALUES_UNOPENABLE: &str = "This Foundry cannot read some saved credentials: neither its instance key nor its previous instance key opens them. An instance operator sets the instance key they were sealed with as the previous instance key, and restarts Foundry.";
/// I-22, n = 1.
const MOVED_ONE: &str =
    "Moved. 1 saved credential is sealed under a new secrets key. Nobody needs to reconnect.";
/// I-22, n ≥ 2.
const MOVED_MANY: &str =
    "Moved. {n} saved credentials are sealed under a new secrets key. Nobody needs to reconnect.";
/// I-26 (restore-only in Deliverable A).
const SECRETS_KEY_UNOPENABLE: &str = "This Foundry cannot open its secrets key: the instance key it was sealed with is missing or different on this server. Until it is restored, saved connections cannot be used. An instance operator restores that instance key, or sets it as the previous instance key, and restarts Foundry.";
/// I-8.
const MATRIX_READY: &str = "Matrix: ready. Workspace admins connect their own Matrix bot on their workspace's integrations page.";
/// I-5.
const SLACK_NOT_SET_UP: &str = "Slack: not set up. Create the Foundry Slack app in Slack from the manifest below, then enter its credentials.";
/// I-29, above I-5's addresses.
const PUBLIC_URL_FELL_BACK: &str = "FOUNDRY_PUBLIC_URL is not set, so the addresses below use {fallback_url}, which Slack cannot reach. An instance operator sets FOUNDRY_PUBLIC_URL to this Foundry's public address and restarts Foundry.";
/// I-21.
const SECRETS_KEY_GENERATED: &str = "Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it.";
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
const MATRIX_NEEDS_KEY: &str =
    "Matrix: needs the secrets key (above). Nothing else is set up for Matrix on this Foundry.";

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
    /// A Slack app is in effect on this server.
    pub slack_app: bool,
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

/// The key action the page offers: a submit control carrying the copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOffer {
    /// I-3's `Generate secrets key`.
    Generate,
    /// I-2e / I-2e0's `Move to a secrets key`.
    Move,
}

impl KeyOffer {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Generate => "Generate secrets key",
            Self::Move => "Move to a secrets key",
        }
    }

    pub fn door(&self) -> &'static str {
        match self {
            Self::Generate => SECRETS_KEY_PATH,
            Self::Move => MOVE_PATH,
        }
    }

    /// Whether this door applies in `mode`, with I-4 holding or not.
    pub fn applies(self, mode: KeyMode, unopenable_values: bool) -> bool {
        match self {
            Self::Generate => mode.generates(unopenable_values),
            Self::Move => mode.moves(unopenable_values),
        }
    }
}

/// What a save that stored the Slack app answers, and the only thing its
/// redirect carries: a closed set of notices, each with its own wire name, so
/// the admin lands on the instance page itself (post, redirect, get) with the
/// words the save earned and no submitted value in the address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavedNotice {
    /// I-10: the app saved here is the one in effect.
    InEffect,
    /// I-17: the environment's app stays in effect until it is removed.
    EnvironmentWins,
}

impl SavedNotice {
    /// How the page's address names it.
    fn wire(self) -> &'static str {
        match self {
            Self::InEffect => "app",
            Self::EnvironmentWins => "app-environment",
        }
    }

    /// The notice the admin lands on.
    fn words(self) -> &'static str {
        match self {
            Self::InEffect => SLACK_APP_SAVED,
            Self::EnvironmentWins => SLACK_SAVED_ENVIRONMENT_WINS,
        }
    }

    /// The notice a `saved` wire name asks for; none for anything else, so an
    /// address nobody was sent to reads as the page itself.
    fn of_wire(wire: &str) -> Option<Self> {
        [Self::InEffect, Self::EnvironmentWins]
            .into_iter()
            .find(|saved| saved.wire() == wire)
    }

    /// Where the save sends the admin to read it.
    fn door(self) -> String {
        format!("{INSTANCE_PATH}?saved={}", self.wire())
    }
}

/// What a Generate or Move that stored the key answers: I-21 when nothing
/// was re-sealed, else I-22 over the `moved` credentials.
pub fn stored_notice(moved: usize) -> String {
    match moved {
        0 => SECRETS_KEY_GENERATED.to_string(),
        1 => MOVED_ONE.to_string(),
        n => MOVED_MANY.replace("{n}", &n.to_string()),
    }
}

/// What the page shows and offers. Never a key, a key id or a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageState {
    /// I-25, or I-27 / I-31 in its place; none when no key file was made
    /// (`Withheld`): the mode's own line says what is wrong.
    pub instance_key: Option<String>,
    /// I-2, I-2e, I-2e0, I-3, I-4 or I-26; none without an instance key.
    pub secrets_key: Option<&'static str>,
    pub offer: Option<KeyOffer>,
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
}

impl PageState {
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
    let offer = [KeyOffer::Generate, KeyOffer::Move]
        .into_iter()
        .find(|offer| offer.applies(facts.mode, facts.unopenable_values));
    let slack = slack_setup(facts);
    let environment_slack = facts
        .environment_slack
        .as_ref()
        .map(|client_id| SLACK_IN_ENVIRONMENT.replace("{client_id}", client_id));
    PageState {
        instance_key: instance_key_line(facts),
        secrets_key,
        offer,
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
/// `direct_seals` stored values sealed directly and the saved app in effect
/// named as saved by `saver`.
fn facts_of(integrations: &Integrations, direct_seals: usize, saver: Option<String>) -> PageFacts {
    let settings = integrations.settings();
    PageFacts {
        mode: integrations.key_mode(),
        operator_key: settings.key_setting.is_some(),
        key_file: crate::integrations::key_file::key_path(&settings.data_dir)
            .display()
            .to_string(),
        key_file_read: integrations.key_file(),
        rewrapped_from: integrations.rewrapped_from().map(str::to_string),
        saved_direct: direct_seals > 0,
        unopenable_values: integrations.secrets_check().degraded(),
        slack_app: integrations.slack().is_some(),
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

/// An activity line as I-18 reads it.
fn activity_line(row: &InstanceActivityRow) -> String {
    let actor = row.actor_name.as_deref().unwrap_or("Foundry");
    let what = match row.action.as_str() {
        "secrets_key_generated" => "generated the secrets key".to_string(),
        "secrets_key_rewrapped" => {
            "re-sealed the secrets key under the operator's instance key".to_string()
        }
        "credentials_moved" => match row.count.unwrap_or_default() {
            1 => "moved 1 credential under a secrets key".to_string(),
            n => format!("moved {n} credentials under a secrets key"),
        },
        "slack_app_saved" => slack_app_line("saved", row),
        "slack_app_replaced" => slack_app_line("replaced", row),
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

/// What a save that stored the app sends the admin back to read: the only
/// thing the page's own address carries, so nothing she submitted reaches a
/// URL.
#[derive(Debug, Default, Deserialize)]
struct PageQuery {
    saved: Option<String>,
}

/// The notice the page's own address asks for, where a save sent the admin
/// here to read it; nothing for any other address. An address nothing can
/// read is no notice and never a refusal: every reader this door turns away
/// gets the uniform 404 and nothing else.
fn saved_notice(query: Option<&str>) -> Option<SavedNotice> {
    let query: PageQuery =
        serde_urlencoded::from_str(query.unwrap_or_default()).unwrap_or_default();
    SavedNotice::of_wire(query.saved.as_deref()?)
}

pub async fn show_instance(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    let Some(user) = crate::instance_admin::require_instance_admin(&state, &session).await else {
        return resource_not_found_page();
    };
    let notice = saved_notice(query.as_deref()).map(|saved| saved.words().to_string());
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

/// Generate's and Move's door: the uniform 404 past the guard, else `door`
/// applied and the page answered.
async fn key_door(
    state: &AppState,
    session: &Session,
    headers: &HeaderMap,
    door: KeyOffer,
) -> Response {
    let Some(user) = crate::instance_admin::require_instance_admin(state, session).await else {
        return resource_not_found_page();
    };
    let notice = store_secrets_key(state, user.user_id, door).await;
    answer(state, &user, headers, notice, String::new()).await
}

/// Generate or Move (one use-case, ADR-IIS-001 decision 5) where `door`
/// applies over the stored rows as they are now, then this server's
/// integrations rebuilt and installed: I-21 or I-22 when this post stored
/// the key, else nothing to tell.
async fn store_secrets_key(state: &AppState, actor: Uuid, door: KeyOffer) -> Option<String> {
    let store = StoreCapabilities::new(state.store.clone());
    let integrations = match store.instance_snapshot().await {
        Ok(snapshot) => state.integrations.current().rebuilt(Ok(snapshot)),
        Err(err) => {
            tracing::warn!(error = %err, "reading the stored credentials failed");
            return None;
        }
    };
    if !door.applies(
        integrations.key_mode(),
        integrations.secrets_check().degraded(),
    ) {
        return None;
    }
    let settings = integrations.settings();
    let wrapped_by = match settings.key_setting {
        Some(_) => KeyWrapper::Operator,
        None => KeyWrapper::File,
    };
    let instance = settings.secrets.as_ref()?;
    let Some(plan) = plan_generate(instance, wrapped_by, Some(actor)) else {
        tracing::warn!(probe = "secrets", "generating the secrets key failed");
        return None;
    };
    let reseal = |slot, sealed: &[u8]| {
        plan.reseal(instance, &sealed_slot(slot), sealed)
            .map(Some)
            .ok_or(ResealRefused)
    };
    let now = state.clock.now();
    let outcome = store
        .store_secrets_key(&plan.key, &plan.canary, now, &reseal)
        .await;
    let notice = match outcome {
        Ok(SecretsKeyOutcome::Stored { resealed, .. }) => Some(stored_notice(resealed)),
        Ok(SecretsKeyOutcome::AlreadyStored { .. }) => None,
        Ok(SecretsKeyOutcome::Refused(ResealRefused)) => {
            tracing::warn!(
                probe = "secrets",
                "a stored credential could not be re-sealed; nothing changed"
            );
            None
        }
        Err(err) => {
            tracing::warn!(error = %err, "storing the secrets key failed");
            return None;
        }
    };
    reinstall(state, &integrations).await;
    notice
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
    match save_slack(&state, user.user_id, &change).await {
        SaveAnswer::Stored(saved) => Redirect::to(&saved.door()).into_response(),
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

/// The save where a sealing mode holds (else I-3, nothing stored), then this
/// server's integrations rebuilt and installed: I-10 once it is saved. When
/// the seal fence finds a newer sealing key, the integrations are rebuilt
/// and the save tried once more.
async fn save_slack(state: &AppState, actor: Uuid, change: &SlackChange) -> SaveAnswer {
    let store = &StoreCapabilities::new(state.store.clone());
    retried(|| async move {
        let integrations = state.integrations.current();
        let Some(sealing) = integrations.sealing_box() else {
            return Attempted::Answered(SaveAnswer::InPlace(Some(NO_SECRETS_KEY)));
        };
        let Some(plan) = plan_replace(
            sealing,
            integrations.key_mode(),
            change,
            actor,
            state.clock.now(),
        ) else {
            tracing::warn!(probe = "secrets", "sealing the Slack app failed");
            return Attempted::Answered(SaveAnswer::InPlace(None));
        };
        let outcome = store.save_slack_app(&plan).await;
        reinstall(state, &integrations).await;
        match outcome {
            Ok(SlackSaveOutcome::Saved { .. }) if integrations.settings().slack.is_some() => {
                Attempted::Answered(SaveAnswer::Stored(SavedNotice::EnvironmentWins))
            }
            Ok(SlackSaveOutcome::Saved { .. }) => {
                Attempted::Answered(SaveAnswer::Stored(SavedNotice::InEffect))
            }
            Ok(SlackSaveOutcome::StaleKey) => Attempted::StaleKey,
            Err(err) => {
                tracing::warn!(error = %err, "saving the Slack app failed");
                Attempted::Answered(SaveAnswer::InPlace(None))
            }
        }
    })
    .await
}

/// What a save answers: the app is stored, so the admin is sent to the page
/// itself to read the notice it earned (post, redirect, get, so a reload
/// never posts the submission again); or nothing was stored and the page is
/// answered in place, with what to tell her (`None` where only the log knows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SaveAnswer {
    Stored(SavedNotice),
    InPlace(Option<&'static str>),
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
async fn answer(
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
    let page = page_state(&facts_of(&integrations, direct_seals, saver));
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
            slack_app: false,
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
            (Sealed, true, Unwritable, false, op.clone(), Some(SECRETS_KEY_SET), None, MATRIX_READY),
            (Sealed, false, Held, true, file.clone(), Some(SECRETS_KEY_SET), None, MATRIX_READY),
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
        let addresses = facts_of(&integrations, 0, None).addresses;
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

    /// What a Generate or Move that stored the key answers, over n in
    /// {0, 1, 2, many}: I-21 for none, I-22 singular for one, plural after.
    #[test]
    fn the_answer_names_how_many_credentials_moved() {
        let i21 = "Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it.";
        let table = [
            (0, i21.to_string()),
            (1, "Moved. 1 saved credential is sealed under a new secrets key. Nobody needs to reconnect.".to_string()),
            (2, "Moved. 2 saved credentials are sealed under a new secrets key. Nobody needs to reconnect.".to_string()),
            (137, "Moved. 137 saved credentials are sealed under a new secrets key. Nobody needs to reconnect.".to_string()),
        ];
        for (moved, words) in table {
            assert_eq!(stored_notice(moved), words, "n = {moved}");
        }
    }

    /// Every mode, with I-4 and without: Generate is offered only while
    /// awaiting a secrets key, Move only while sealing directly, neither
    /// under I-4, and I-4 names itself wherever the key line shows (I-26
    /// keeps its own words).
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
            (Sealed,             false, None,                    Some(SECRETS_KEY_SET)),
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
            for door in [KeyOffer::Generate, KeyOffer::Move] {
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

    #[test]
    fn each_offer_posts_to_its_own_door() {
        assert_eq!(KeyOffer::Generate.label(), "Generate secrets key");
        assert_eq!(KeyOffer::Generate.door(), SECRETS_KEY_PATH);
        assert_eq!(KeyOffer::Move.label(), "Move to a secrets key");
        assert_eq!(KeyOffer::Move.door(), MOVE_PATH);
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
        let none = facts_of(&integrations, 0, None);
        assert!(!none.saved_direct, "no direct seal");
        assert!(!none.saved_unreadable, "no saved app");
        assert_eq!(none.key_file, FILE);
        assert!(facts_of(&integrations, 1, None).saved_direct, "one");
        assert!(facts_of(&integrations, 2, None).saved_direct, "two");
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
