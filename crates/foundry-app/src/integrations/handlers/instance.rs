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
//! The Slack app save, `POST …/slack` (US-IIS-03), behind the same guard:
//! a blank or malformed field answers I-11 or I-12 before anything is
//! touched, the typed client ID kept and both secrets cleared; without a sealing mode it answers I-3 and stores nothing; otherwise both
//! secrets are sealed and handed to the one write use-case, fenced on the
//! sealing key, and this server's integrations are rebuilt and installed
//! before it answers I-10 with I-6 (ADR-IIS-003).

use super::render;
use crate::bootstrap::{resource_not_found_page, SessionUser};
use crate::csrf::ensure_csrf_cookie;
use crate::integrations::keys::{plan_generate, KeyMode, NoKeyReason};
use crate::integrations::slack_app::{plan_save, Refusal, SlackAppForm, SlackField};
use crate::integrations::store_capabilities::{InstanceKeyWrites, KeyProbe, StoreCapabilities};
use crate::integrations::{slack, Integrations};
use crate::AppState;
use askama::Template;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use foundry_store::integrations::{
    InstanceActivityRow, KeyWrapper, SecretsKeyOutcome, SlackSaveOutcome,
};
use time::OffsetDateTime;
use tower_sessions::Session;
use uuid::Uuid;

pub const INSTANCE_PATH: &str = "/admin/instance/integrations";
pub const SECRETS_KEY_PATH: &str = "/admin/instance/integrations/secrets-key";
pub const MOVE_PATH: &str = "/admin/instance/integrations/secrets-key/move";
pub const SLACK_APP_PATH: &str = "/admin/instance/integrations/slack";

/// The instance key file's name in `FOUNDRY_DATA_DIR`.
const KEY_FILE_NAME: &str = "instance.key";

/// I-25, operator-set.
const KEY_BY_OPERATOR: &str = "Instance key: set by the operator.";
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
/// I-6, with the saved app's client ID, who saved it and when.
const SLACK_SAVED_HERE: &str = "Slack: set up here · app {client_id} · saved by {name} · {date}";
/// I-6's secret lines: set, never shown.
const CLIENT_SECRET_SET: &str = "Client secret: set";
const SIGNING_SECRET_SET: &str = "Signing secret: set";
/// How many times a save is tried when the seal fence finds a newer key.
const SAVE_ATTEMPTS: usize = 2;
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
}

/// What the page shows and offers. Never a key, a key id or a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageState {
    /// I-25, or I-27 / I-31 in its place.
    pub instance_key: String,
    /// I-2, I-2e, I-2e0, I-3 or I-26; none without an instance key.
    pub secrets_key: Option<&'static str>,
    pub offer: Option<KeyOffer>,
    /// I-8 or I-9.
    pub matrix: &'static str,
    /// I-5: with a secrets key and no Slack app.
    pub slack: Option<SlackSetup>,
    /// I-6: the saved app in effect.
    pub saved_slack: Option<SavedSlackLines>,
    /// I-7: the environment's app in effect.
    pub environment_slack: Option<String>,
    /// The Slack app form: under I-5, or under I-7 where credentials seal.
    pub slack_form: bool,
}

/// The page of `facts` (pure and total).
pub fn page_state(facts: &PageFacts) -> PageState {
    let (secrets_key, offer) = match facts.mode {
        KeyMode::NoInstanceKey(_) => (None, None),
        KeyMode::AwaitingSecretsKey => (
            Some(NO_SECRETS_KEY),
            facts
                .mode
                .generates(facts.unopenable_values)
                .then_some(KeyOffer::Generate),
        ),
        KeyMode::Direct if facts.saved_direct => (Some(DIRECT_SAVED), Some(KeyOffer::Move)),
        KeyMode::Direct => (Some(DIRECT_NOTHING_SAVED), Some(KeyOffer::Move)),
        KeyMode::Sealed => (Some(SECRETS_KEY_SET), None),
        KeyMode::Unopenable => (Some(SECRETS_KEY_UNOPENABLE), None),
    };
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
        slack_form: slack.is_some() || (environment_slack.is_some() && facts.mode.seals()),
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

fn instance_key_line(facts: &PageFacts) -> String {
    let words = match facts.mode {
        KeyMode::NoInstanceKey(NoKeyReason::Unwritable) => KEY_UNWRITABLE,
        KeyMode::NoInstanceKey(NoKeyReason::Unreadable) => KEY_UNREADABLE,
        _ if facts.operator_key => KEY_BY_OPERATOR,
        _ => KEY_IN_FILE,
    };
    words.replace("{path}", &facts.key_file)
}

/// The facts of the integrations in effect on this server, with
/// `direct_seals` stored values sealed directly and the saved app in effect
/// named as saved by `saver`.
fn facts_of(integrations: &Integrations, direct_seals: usize, saver: Option<String>) -> PageFacts {
    let settings = integrations.settings();
    PageFacts {
        mode: integrations.key_mode(),
        operator_key: settings.key_setting.is_some(),
        key_file: settings.data_dir.join(KEY_FILE_NAME).display().to_string(),
        saved_direct: direct_seals > 0,
        unopenable_values: integrations.secrets_check().degraded(),
        slack_app: integrations.slack().is_some(),
        environment_slack: settings.slack.as_ref().map(|app| app.client_id.clone()),
        saved_slack: integrations.saved_slack().map(|app| SavedSlack {
            client_id: app.client_id.clone(),
            saved_by: saver.unwrap_or_else(|| "Foundry".to_string()),
            saved_at: app.saved_at,
        }),
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
        "slack_app_saved" => format!(
            "saved the Slack app ({})",
            row.client_id.as_deref().unwrap_or_default()
        ),
        other => other.replace('_', " "),
    };
    format!("{} · {actor} {what}", display_date(row.created_at))
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
    /// What the post just did (I-21).
    notice: Option<&'static str>,
    /// The client ID a refused save typed, kept in the form (I-11, I-12).
    kept_client_id: String,
    page: PageState,
    activity: Vec<String>,
}

pub async fn show_instance(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(user) = crate::instance_admin::require_instance_admin(&state, &session).await else {
        return resource_not_found_page();
    };
    answer(&state, &user, &headers, None, String::new()).await
}

/// `POST /admin/instance/integrations/secrets-key`: Generate.
pub async fn generate_secrets_key(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(user) = crate::instance_admin::require_instance_admin(&state, &session).await else {
        return resource_not_found_page();
    };
    let notice = generate(&state, user.user_id).await;
    answer(&state, &user, &headers, notice, String::new()).await
}

/// Generate where it applies, then this server's integrations rebuilt and
/// installed: I-21 when this post stored the key, else nothing to tell.
async fn generate(state: &AppState, actor: Uuid) -> Option<&'static str> {
    let integrations = state.integrations.current();
    if !integrations
        .key_mode()
        .generates(integrations.secrets_check().degraded())
    {
        return None;
    }
    let settings = integrations.settings();
    let wrapped_by = match settings.key_setting {
        Some(_) => KeyWrapper::Operator,
        None => KeyWrapper::File,
    };
    let Some(plan) = settings
        .secrets
        .as_ref()
        .and_then(|instance| plan_generate(instance, wrapped_by, Some(actor)))
    else {
        tracing::warn!(probe = "secrets", "generating the secrets key failed");
        return None;
    };
    let store = StoreCapabilities::new(state.store.clone());
    let outcome = store
        .store_secrets_key(&plan.key, &plan.canary, state.clock.now(), &|_, _| Ok(None))
        .await;
    let notice = match outcome {
        Ok(SecretsKeyOutcome::Stored { .. }) => Some(SECRETS_KEY_GENERATED),
        Ok(SecretsKeyOutcome::AlreadyStored { .. } | SecretsKeyOutcome::Refused(_)) => None,
        Err(err) => {
            tracing::warn!(error = %err, "storing the secrets key failed");
            return None;
        }
    };
    state
        .integrations
        .install(integrations.reboot(&*store).await);
    notice
}

/// `POST /admin/instance/integrations/slack`: save the Slack app. The form
/// is read only past the guard, so every refusal is the uniform 404.
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
    let form = SlackAppForm::from_fields(&fields);
    if let Some(refusal) = form.refusal() {
        return answer(
            &state,
            &user,
            &headers,
            Some(refused(refusal)),
            form.client_id,
        )
        .await;
    }
    let notice = save_slack(&state, user.user_id, &form).await;
    answer(&state, &user, &headers, notice, String::new()).await
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
async fn save_slack(state: &AppState, actor: Uuid, form: &SlackAppForm) -> Option<&'static str> {
    let store = StoreCapabilities::new(state.store.clone());
    for _ in 0..SAVE_ATTEMPTS {
        let integrations = state.integrations.current();
        let Some(sealing) = integrations.sealing_box() else {
            return Some(NO_SECRETS_KEY);
        };
        let Some(plan) = plan_save(
            sealing,
            integrations.key_mode(),
            form,
            actor,
            state.clock.now(),
        ) else {
            tracing::warn!(probe = "secrets", "sealing the Slack app failed");
            return None;
        };
        let outcome = store.save_slack_app(&plan).await;
        state
            .integrations
            .install(integrations.reboot(&*store).await);
        match outcome {
            Ok(SlackSaveOutcome::Saved { .. }) => return Some(SLACK_APP_SAVED),
            Ok(SlackSaveOutcome::StaleKey) => continue,
            Err(err) => {
                tracing::warn!(error = %err, "saving the Slack app failed");
                return None;
            }
        }
    }
    None
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
    notice: Option<&'static str>,
    kept_client_id: String,
) -> Response {
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let integrations = state.integrations.current();
    let saver = saver_name(state, &integrations).await;
    let direct_seals = direct_seals(state, &integrations).await;
    let page = page_state(&facts_of(&integrations, direct_seals, saver));
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
            kept_client_id,
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
            saved_direct,
            unopenable_values: false,
            slack_app: false,
            environment_slack: None,
            saved_slack: None,
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

    /// Facts in (mode, operator key, direct seals), then the page expected.
    type Row = (
        KeyMode,
        bool,
        bool,
        String,
        Option<&'static str>,
        Option<KeyOffer>,
        &'static str,
    );

    /// Every mode, with the key from the operator and from the file, and with
    /// or without direct seals: the page's lines and its one offer.
    #[test]
    fn the_page_follows_the_mode() {
        use KeyMode::*;
        let unwritable = KEY_UNWRITABLE.replace("{path}", FILE);
        let unreadable = KEY_UNREADABLE.replace("{path}", FILE);
        let op = KEY_BY_OPERATOR.to_string();
        #[rustfmt::skip]
        let table: Vec<Row> = vec![
            (NoInstanceKey(NoKeyReason::Unwritable), false, false, unwritable, None, None, MATRIX_NEEDS_KEY),
            (NoInstanceKey(NoKeyReason::Unreadable), false, false, unreadable, None, None, MATRIX_NEEDS_KEY),
            (AwaitingSecretsKey, true, false, op.clone(), Some(NO_SECRETS_KEY), Some(KeyOffer::Generate), MATRIX_NEEDS_KEY),
            (AwaitingSecretsKey, false, false, file_line(), Some(NO_SECRETS_KEY), Some(KeyOffer::Generate), MATRIX_NEEDS_KEY),
            (Direct, true, false, op.clone(), Some(DIRECT_NOTHING_SAVED), Some(KeyOffer::Move), MATRIX_READY),
            (Direct, true, true, op.clone(), Some(DIRECT_SAVED), Some(KeyOffer::Move), MATRIX_READY),
            (Direct, false, true, file_line(), Some(DIRECT_SAVED), Some(KeyOffer::Move), MATRIX_READY),
            (Sealed, true, false, op.clone(), Some(SECRETS_KEY_SET), None, MATRIX_READY),
            (Sealed, false, true, file_line(), Some(SECRETS_KEY_SET), None, MATRIX_READY),
            (Unopenable, true, false, op.clone(), Some(SECRETS_KEY_UNOPENABLE), None, MATRIX_NEEDS_KEY),
            (Unopenable, false, true, file_line(), Some(SECRETS_KEY_UNOPENABLE), None, MATRIX_NEEDS_KEY),
        ];
        for (mode, operator_key, saved_direct, instance_key, secrets_key, offer, matrix) in table {
            let facts = facts(mode, operator_key, saved_direct);
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
            (Direct,                                 false, None,     false),
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

    #[test]
    fn each_offer_posts_to_its_own_door() {
        assert_eq!(KeyOffer::Generate.label(), "Generate secrets key");
        assert_eq!(KeyOffer::Generate.door(), SECRETS_KEY_PATH);
        assert_eq!(KeyOffer::Move.label(), "Move to a secrets key");
        assert_eq!(KeyOffer::Move.door(), MOVE_PATH);
    }
}
