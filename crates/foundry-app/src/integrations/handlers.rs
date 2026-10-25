//! The integration doors (DESIGN [REF] Driving Ports; ADR-EIP-005 decisions
//! 2 and 4). Every door answers the uniform 404 to a caller without the right
//! (SC-6), decided by the use-case before anything else.
//!
//! - `GET /workspace/integrations`: the Slack connection (K-1), what is
//!   missing (K-2, K-20) or Add to Slack, and the activity list (R-4).
//! - `GET /workspace/integrations/slack/setup`: the manifest page (DD-9).
//! - `GET /workspace/integrations/slack/install`: keeps a fresh state in the
//!   session and sends the admin to Slack's approval screen.
//! - `GET /workspace/integrations/slack/return`: takes the state back out
//!   (single use), and only for a matching, fresh state exchanges the code,
//!   seals the bot token for its connection row and saves the connection.
//!   Anything else shows K-3 and stores nothing.
//! - `GET /team/{t}/project/{p}/integrations`: the project's routes with their
//!   K-9 status (Manage).
//! - `GET …/integrations/routes/new?kind=slack`: the route form, its channel
//!   picker listing the channels Foundry can post to, or K-4 without a
//!   connection (Manage).
//! - `POST …/integrations/routes/preview`: the form again with the message the
//!   route would send for the project's newest issue, from the one `compose`
//!   the worker sends with; asks Slack nothing, writes nothing (DDR-25).
//! - `POST …/integrations/routes/test`: exactly one K-7 message to the entered
//!   channel, no row written, six a minute per person (DDR-25, R-5).
//! - `POST …/integrations/routes`: checks the channel with Slack, then saves
//!   the route and its audit record; sends nothing (DDR-25). A channel shared
//!   with another organisation is saved only with the K-6 tick.
//! - `GET …/integrations/routes/{route_id}/history`: every delivery of the
//!   route, newest first, in Foundry's words (US-EIP-05, Manage).
//! - `POST …/integrations/routes/{route_id}/pause` and `…/resume`: pause
//!   (its waiting deliveries skipped, DD-8) or resume (receives what happens
//!   from now, DDR-27; a refusal's stop cleared) the route, audited; asking
//!   for what is already so changes nothing (Manage).
//! - `GET`/`POST …/integrations/routes/{route_id}/edit`: the route form for
//!   the route, and its save: events, content level and lanes (Manage).
//! - `GET …/integrations/routes/{route_id}/delete`: the delete question; its
//!   `POST` deletes the route and what it was waiting to send, never a
//!   message already posted (Manage).
//! - `GET /workspace/integrations/slack/disconnect`: the disconnect question,
//!   counting the routes that stop (admins); its `POST` disconnects: the
//!   connection's credential is forgotten and what its routes waited to send
//!   is skipped, in one transaction, and only then is Slack asked to withdraw
//!   Foundry's access (`apps.uninstall`), so a Slack failure never leaves the
//!   credential stored. Works without scripting; CSRF-protected.
//! - `POST /integrations/slack/events`: Slack's Events API (DDR-11,
//!   ADR-EIP-005 decision 6), a machine post with no session, believed only
//!   when Slack signed it (401 otherwise): it answers Slack's address check
//!   with its challenge, and the Foundry app's removal from a connected Slack
//!   workspace (`app_uninstalled`, `tokens_revoked`) turns that connection
//!   lost and tells its admins once. Anything else is acknowledged and
//!   changes nothing.

use super::slack::{self, InstallState, SlackEvent, SlackGrant};
use super::store_capabilities::{ConnectionHealth, StoreCapabilities};
use super::{Integrations, SlackOffer};
use crate::bootstrap::{resource_not_found_page, SessionUser};
use crate::csrf::{ensure_csrf_cookie, response_with_optional_cookie};
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::{Form, Json};
use foundry_auth::secret_box::SecretSlot;
use foundry_integrations::{
    compose, encode_slack, issue_url, test_message, ChannelKind, ChannelOutcome,
    ConnectionLostNotice, ConnectionState, ContentLevel, DeliveryChannel, DeliveryKey,
    DeliveryRecord, Destination, DestinationCatalog, DestinationRef, EventKind, EventView,
    FinishedDelivery, MessageLines, ReasonCode, RouteFacts, RouteStatus,
};
use foundry_services::integrations::{
    IntegrationError, LostSlack, ManagedRoute, ProjectIntegrations, RouteAction, RouteDraft,
    RouteEditDraft, RouteHistory, RoutePreview, RouteTarget, SlackInstall, WorkspaceIntegrations,
};
use foundry_services::{Principal, Services};
use foundry_store::integrations::{
    ConnectionWriteOutcome, DeliveryHistoryRow, DisconnectOutcome, DisconnectPreview, ProjectRoute,
};
use serde::Deserialize;
use tower_sessions::Session;

pub const WORKSPACE_PATH: &str = "/workspace/integrations";
pub const SETUP_PATH: &str = "/workspace/integrations/slack/setup";
pub const INSTALL_PATH: &str = "/workspace/integrations/slack/install";
pub const RETURN_PATH: &str = slack::RETURN_PATH;
pub const DISCONNECT_PATH: &str = "/workspace/integrations/slack/disconnect";

/// K-3.
pub const NOT_CONNECTED: &str = "Slack was not connected. Nothing changed.";
/// US-EIP-06 example 5, for the `{approved}` Slack workspace when the
/// connection is to `{connected}`.
pub const OTHER_SLACK_WORKSPACE: &str =
    "This is a different Slack workspace ({approved}). Routes stay paused. Reconnect to {connected}, or disconnect.";
/// K-4.
pub const NO_CHAT_CONNECTION: &str =
    "This workspace has no chat connection yet. Ask a workspace admin to connect Slack or Matrix.";
/// R-5.
pub const TOO_MANY_TESTS: &str = "Too many test messages. Try again in a minute.";
/// The channel picker's note on private channels (US-EIP-02 example 2).
pub const PRIVATE_CHANNELS: &str =
    "Private channels appear here after someone runs /invite @Foundry in them.";
pub const TEST_SENT: &str = "Test message sent.";
pub const CANNOT_POST: &str = "Foundry cannot post to that channel. Choose another.";
/// K-5, for the chosen `{destination}`.
pub const AUDIENCE: &str =
    "Everyone who can read {destination} will see what this route sends. Foundry cannot check who that is.";
/// US-EIP-07's delete question, for the route to `{destination}`.
pub const DELETE_QUESTION: &str =
    "Delete the route to {destination}? {project} stops posting there. Messages already in {kind} stay.";
/// K-6, for the chosen `{destination}`.
pub const SHARED_CHANNEL: &str =
    "{destination} is shared with another organisation. Tick to confirm that is intended.";

#[derive(Template)]
#[template(path = "integrations/workspace.html")]
struct WorkspacePage {
    nav: crate::nav::NavContext,
    notice: Option<String>,
    slack_line: Option<String>,
    /// K-10: the connection was lost; offer to reconnect it.
    reconnect: bool,
    not_set_up: bool,
    no_secrets_key: bool,
    /// K-19: the saved connection cannot be read by this Foundry (DD-2).
    unavailable: bool,
    activity: Vec<String>,
}

#[derive(Template)]
#[template(path = "integrations/slack_setup.html")]
struct SetupPage {
    nav: crate::nav::NavContext,
    manifest: String,
}

#[derive(Template)]
#[template(path = "integrations/project.html")]
struct ProjectPage {
    nav: crate::nav::NavContext,
    csrf: String,
    project_name: String,
    new_route_path: String,
    routes: Vec<RouteLine>,
}

/// One route as the project page lists it.
struct RouteLine {
    id: uuid::Uuid,
    destination_label: String,
    status: String,
    history_path: String,
    edit_path: String,
    delete_path: String,
    /// Pause, or Resume for a route paused or stopped by a refusal.
    toggle_path: String,
    toggle_label: &'static str,
}

#[derive(Template)]
#[template(path = "integrations/route_delete.html")]
struct RouteDeletePage {
    nav: crate::nav::NavContext,
    csrf: String,
    destination_label: String,
    question: String,
    delete_path: String,
    routes_path: String,
}

#[derive(Template)]
#[template(path = "integrations/slack_disconnect.html")]
struct SlackDisconnectPage {
    nav: crate::nav::NavContext,
    csrf: String,
    question: String,
    disconnect_path: &'static str,
    workspace_path: &'static str,
}

#[derive(Template)]
#[template(path = "integrations/route_history.html")]
struct RouteHistoryPage {
    nav: crate::nav::NavContext,
    project_name: String,
    destination_label: String,
    routes_path: String,
    deliveries: Vec<String>,
}

#[derive(Template)]
#[template(path = "integrations/route_form.html")]
struct RouteFormPage {
    nav: crate::nav::NavContext,
    csrf: String,
    heading: String,
    /// Where Save posts: the new route, or the edited one.
    action_path: String,
    test_path: String,
    preview_path: String,
    notice: Option<String>,
    /// K-5 for the chosen channel.
    audience: Option<String>,
    /// K-6 when the chosen channel is shared with another organisation.
    shared_channel: Option<String>,
    shared_tick: bool,
    shared_confirmed: bool,
    /// The preview, once asked for; `Some(None)` when there is no issue yet.
    preview: Option<Option<MessageLines>>,
    no_issue_yet: String,
    no_connection: bool,
    channels: Vec<Choice>,
    events: Vec<Choice>,
    lanes: Vec<Choice>,
    levels: Vec<Choice>,
    private_channels: &'static str,
    no_chat_connection: &'static str,
}

/// One option of the form: its value, its label, and whether it is chosen.
struct Choice {
    value: String,
    label: String,
    chosen: bool,
}

/// The chat events (D-2), as the form offers them.
const EVENT_CHOICES: [(&str, &str); 4] = [
    ("issue.created", "New issue"),
    ("issue.moved", "Issue moved"),
    ("comment.added", "New comment"),
    ("issue.deleted", "Issue deleted"),
];

/// The content levels (D-3); Summary is the default (OD-6).
const LEVEL_CHOICES: [(&str, &str); 3] = [
    ("link_only", "Link only"),
    ("summary", "Summary"),
    ("summary_with_comment", "Summary with comment text"),
];

/// The field prefix carrying a channel's name, `channel_label.{id}`.
const CHANNEL_LABEL_FIELD: &str = "channel_label.";

/// What the route form submitted.
#[derive(Debug, Clone, Default)]
struct RouteForm {
    destination: String,
    /// Each offered channel's name, by its id, as the picker showed it: so a
    /// preview, which asks Slack nothing, still names the channel (K-5).
    channel_labels: Vec<(String, String)>,
    events: Vec<String>,
    content_level: String,
    lanes: Vec<String>,
    shared_confirmed: bool,
}

impl RouteForm {
    fn read(fields: &[(String, String)]) -> Self {
        let mut form = RouteForm {
            content_level: "summary".to_string(),
            ..RouteForm::default()
        };
        for (name, value) in fields {
            match name.as_str() {
                "destination" => form.destination = value.clone(),
                "events" => form.events.push(value.clone()),
                "content_level" => form.content_level = value.clone(),
                "lanes" => form.lanes.push(value.clone()),
                "shared_confirmed" => form.shared_confirmed = true,
                other => {
                    if let Some(id) = other.strip_prefix(CHANNEL_LABEL_FIELD) {
                        form.channel_labels.push((id.to_string(), value.clone()));
                    }
                }
            }
        }
        form
    }

    /// The chosen channel's name as the picker showed it, else its id.
    fn destination_label(&self) -> String {
        self.channel_labels
            .iter()
            .find(|(id, _)| *id == self.destination)
            .map_or_else(|| self.destination.clone(), |(_, label)| label.clone())
    }

    /// A fresh form: New issue at Summary, for `destination`.
    fn new(destination: Option<String>) -> Self {
        RouteForm {
            destination: destination.unwrap_or_default(),
            events: vec!["issue.created".to_string()],
            content_level: "summary".to_string(),
            ..RouteForm::default()
        }
    }
}

/// What a route form answer shows beside the form.
struct FormAnswer {
    notice: Option<String>,
    status: StatusCode,
    preview: Option<Option<MessageLines>>,
}

impl FormAnswer {
    fn with(notice: Option<&str>, status: StatusCode) -> Self {
        Self {
            notice: notice.map(str::to_string),
            status,
            preview: None,
        }
    }

    fn plain() -> Self {
        Self::with(None, StatusCode::OK)
    }

    /// A preview: it never asks Slack for the channel list (DDR-25).
    fn preview(lines: Option<MessageLines>) -> Self {
        Self {
            preview: Some(lines),
            ..Self::plain()
        }
    }

    fn asks_slack(&self) -> bool {
        self.preview.is_none()
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct NewRouteQuery {
    destination: Option<String>,
}

/// Slack's return: a code and our state, or Slack's `error`.
#[derive(Debug, Default, Deserialize)]
pub struct SlackReturn {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// The signed-in caller as the use-cases see them.
struct Caller {
    user: SessionUser,
    services: Services,
}

impl Caller {
    async fn of(state: &AppState, session: &Session) -> Option<Self> {
        let user = session
            .get::<SessionUser>(SESSION_KEY_USER_ID)
            .await
            .ok()
            .flatten()?;
        Some(Self {
            user,
            services: Services::new(state.store.clone()),
        })
    }

    fn principal(&self) -> Principal {
        Principal::Human {
            user_id: self.user.user_id,
            workspace_id: self.user.workspace_id,
        }
    }

    async fn nav(&self, state: &AppState, csrf: String) -> crate::nav::NavContext {
        let is_instance_admin =
            crate::nav::resolve_is_instance_admin(state, self.user.user_id).await;
        crate::nav::NavContext::home_for(
            state,
            self.user.user_id,
            self.user.workspace_id,
            is_instance_admin,
            csrf,
        )
        .await
    }
}

pub async fn show_workspace(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return sign_in();
    };
    workspace_page(&state, &caller, &headers, None).await
}

pub async fn show_slack_setup(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return sign_in();
    };
    if let Err(err) = caller
        .services
        .require_workspace_admin(&caller.principal())
        .await
    {
        return refusal(err);
    }
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let page = SetupPage {
        nav: caller.nav(&state, csrf).await,
        manifest: slack::manifest(&state.integrations.current().settings().public_url),
    };
    render(page.render(), set_cookie)
}

pub async fn start_slack_install(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return sign_in();
    };
    if let Err(err) = caller
        .services
        .require_workspace_admin(&caller.principal())
        .await
    {
        return refusal(err);
    }
    let integrations = state.integrations.current();
    let (Some(app), SlackOffer::Ready) = (integrations.slack(), integrations.slack_offer()) else {
        return workspace_page(&state, &caller, &headers, None).await;
    };
    let held = InstallState::issue(caller.user.workspace_id, state.clock.now());
    if let Err(err) = session.insert(InstallState::SESSION_KEY, &held).await {
        tracing::warn!(error = %err, "slack install: the session could not hold the install state");
        return workspace_page(&state, &caller, &headers, Some(NOT_CONNECTED)).await;
    }
    Redirect::to(&app.approval_url(held.state(), &integrations.slack_return_url())).into_response()
}

pub async fn finish_slack_install(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Query(returned): Query<SlackReturn>,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return sign_in();
    };
    let principal = caller.principal();
    if let Err(err) = caller.services.require_workspace_admin(&principal).await {
        return refusal(err);
    }
    let held = session
        .remove::<InstallState>(InstallState::SESSION_KEY)
        .await
        .ok()
        .flatten();
    let code = match (held, &returned) {
        (
            Some(held),
            SlackReturn {
                code: Some(code),
                state: Some(returned_state),
                error: None,
            },
        ) if held.accepts(returned_state, caller.user.workspace_id, state.clock.now()) => code,
        _ => return workspace_page(&state, &caller, &headers, Some(NOT_CONNECTED)).await,
    };
    match connect(&state, &caller, &principal, code).await {
        Ok(()) => Redirect::to(WORKSPACE_PATH).into_response(),
        Err(NotConnected::OtherWorkspace {
            approved,
            connected,
        }) => {
            tracing::warn!(reason = "other_workspace", "slack install: not connected");
            let words = OTHER_SLACK_WORKSPACE
                .replace("{approved}", &approved)
                .replace("{connected}", &connected);
            workspace_page(&state, &caller, &headers, Some(&words)).await
        }
        Err(NotConnected::Failed(why)) => {
            tracing::warn!(reason = why, "slack install: not connected");
            workspace_page(&state, &caller, &headers, Some(NOT_CONNECTED)).await
        }
    }
}

pub async fn show_slack_disconnect(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return sign_in();
    };
    let preview = match caller
        .services
        .slack_disconnect_preview(&caller.principal())
        .await
    {
        Ok(Some(preview)) => preview,
        Ok(None) => return Redirect::to(WORKSPACE_PATH).into_response(),
        Err(err) => return refusal(err),
    };
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let page = SlackDisconnectPage {
        nav: caller.nav(&state, csrf.clone()).await,
        csrf,
        question: disconnect_question(&preview),
        disconnect_path: DISCONNECT_PATH,
        workspace_path: WORKSPACE_PATH,
    };
    render(page.render(), set_cookie)
}

/// The disconnect: Foundry forgets the credential first, then asks Slack to
/// withdraw the access it granted, so a Slack failure only goes to the log.
pub async fn disconnect_slack(State(state): State<AppState>, session: Session) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return sign_in();
    };
    match caller
        .services
        .disconnect_slack(&caller.principal(), state.clock.now())
        .await
    {
        Ok(DisconnectOutcome::Disconnected {
            connection_id,
            sealed_credential,
            ..
        }) => {
            let withdrawn = state
                .integrations
                .current()
                .withdraw_slack_access(connection_id, sealed_credential)
                .await;
            if !withdrawn {
                tracing::warn!(
                    reason = "apps_uninstall",
                    "slack disconnect: Slack did not withdraw Foundry's access; the credential is forgotten"
                );
            }
            Redirect::to(WORKSPACE_PATH).into_response()
        }
        Ok(DisconnectOutcome::Unchanged) => Redirect::to(WORKSPACE_PATH).into_response(),
        Err(err) => refusal(err),
    }
}

/// US-EIP-07's disconnect question: "Disconnect Slack? 3 routes in 2
/// projects stop posting. Foundry's access to Canzan Labs' Slack is removed."
fn disconnect_question(preview: &DisconnectPreview) -> String {
    let count = |n: i64, noun: &str| {
        if n == 1 {
            format!("1 {noun}")
        } else {
            format!("{n} {noun}s")
        }
    };
    let verb = if preview.routes == 1 { "stops" } else { "stop" };
    let owner = if preview.label.ends_with('s') {
        format!("{}'", preview.label)
    } else {
        format!("{}'s", preview.label)
    };
    format!(
        "Disconnect Slack? {} in {} {verb} posting. Foundry's access to {owner} Slack is removed.",
        count(preview.routes, "route"),
        count(preview.projects, "project"),
    )
}

pub async fn show_project(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((team_slug, project_slug)): Path<(String, String)>,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return sign_in();
    };
    let view = caller
        .services
        .project_integrations(&caller.principal(), &team_slug, &project_slug)
        .await;
    let ProjectIntegrations {
        project_name,
        routes,
    } = match view {
        Ok(view) => view,
        Err(err) => return refusal(err),
    };
    let base = base_path(&team_slug, &project_slug);
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let page = ProjectPage {
        nav: caller.nav(&state, csrf.clone()).await,
        csrf,
        project_name,
        new_route_path: format!("{base}/routes/new?kind=slack"),
        routes: routes
            .iter()
            .map(|route| route_line(route, &base))
            .collect(),
    };
    render(page.render(), set_cookie)
}

pub async fn show_route_history(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((team_slug, project_slug, route_id)): Path<(String, String, String)>,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return sign_in();
    };
    let Ok(route_id) = uuid::Uuid::parse_str(&route_id) else {
        return resource_not_found_page();
    };
    let history = caller
        .services
        .route_history(&caller.principal(), &team_slug, &project_slug, route_id)
        .await;
    let RouteHistory {
        project_name,
        kind,
        destination_label,
        deliveries,
    } = match history {
        Ok(history) => history,
        Err(err) => return refusal(err),
    };
    let channel = ChannelKind::from_wire_name(&kind).unwrap_or(ChannelKind::Slack);
    let now = state.clock.now();
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let page = RouteHistoryPage {
        nav: caller.nav(&state, csrf).await,
        deliveries: deliveries
            .iter()
            .map(|row| history_line(row, channel, &destination_label, now))
            .collect(),
        project_name,
        destination_label,
        routes_path: base_path(&team_slug, &project_slug),
    };
    render(page.render(), set_cookie)
}

pub async fn show_new_route(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((team_slug, project_slug)): Path<(String, String)>,
    Query(query): Query<NewRouteQuery>,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return sign_in();
    };
    let door = match RouteDoor::open(&state, &caller, &team_slug, &project_slug).await {
        Ok(door) => door,
        Err(response) => return response,
    };
    door.form(
        &state,
        &caller,
        &headers,
        &RouteForm::new(query.destination),
        FormAnswer::plain(),
    )
    .await
}

pub async fn preview_route(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((team_slug, project_slug)): Path<(String, String)>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return sign_in();
    };
    let preview = caller
        .services
        .route_preview(&caller.principal(), &team_slug, &project_slug)
        .await;
    let RoutePreview {
        target,
        latest_issue,
    } = match preview {
        Ok(preview) => preview,
        Err(err) => return refusal(err),
    };
    let form = RouteForm::read(&fields);
    let level = ContentLevel::from_wire_name(&form.content_level).unwrap_or(ContentLevel::Summary);
    let public_url = state.integrations.current().settings().public_url.clone();
    let lines = latest_issue.map(|issue| {
        let view = EventView {
            kind: EventKind::IssueCreated,
            issue_key: format!("{}-{}", target.key_prefix, issue.number),
            title: issue.title,
            project_name: target.project_name.clone(),
            actor_name: issue.author_name.unwrap_or_default(),
            lane_label: issue.lane_label,
            from_lane_label: String::new(),
            issue_url: issue_url(&public_url, &team_slug, &project_slug, issue.number),
            issue_count: 1,
            comment_text: String::new(),
        };
        compose(&view, level)
    });
    let door = RouteDoor::at(&state, target, &team_slug, &project_slug);
    door.form(&state, &caller, &headers, &form, FormAnswer::preview(lines))
        .await
}

pub async fn send_test(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((team_slug, project_slug)): Path<(String, String)>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return sign_in();
    };
    let door = match RouteDoor::open(&state, &caller, &team_slug, &project_slug).await {
        Ok(door) => door,
        Err(response) => return response,
    };
    let form = RouteForm::read(&fields);
    let allowed = state
        .integrations
        .test_messages()
        .consume(caller.user.user_id, state.clock.now())
        .is_allowed();
    if !allowed {
        return door
            .form(
                &state,
                &caller,
                &headers,
                &form,
                FormAnswer::with(Some(TOO_MANY_TESTS), StatusCode::TOO_MANY_REQUESTS),
            )
            .await;
    }
    let Some(bot) = door.bot.as_ref() else {
        return door
            .form(&state, &caller, &headers, &form, FormAnswer::plain())
            .await;
    };
    let message = encode_slack(&test_message(
        &door.target.project_name,
        &door.target.key_prefix,
    ));
    // A test is no delivery: its key is recorded nowhere (DDR-25).
    let outcome = bot
        .deliver(
            &DestinationRef(form.destination.clone()),
            &message,
            DeliveryKey(0),
        )
        .await;
    let (notice, status) = match outcome {
        ChannelOutcome::Delivered => (TEST_SENT.to_string(), StatusCode::OK),
        ChannelOutcome::Transient { reason, .. }
        | ChannelOutcome::RouteRefused { reason }
        | ChannelOutcome::ConnectionRefused { reason } => (
            format!(
                "The test message was not delivered: {}.",
                reason.words("the channel")
            ),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    };
    door.form(
        &state,
        &caller,
        &headers,
        &form,
        FormAnswer::with(Some(&notice), status),
    )
    .await
}

pub async fn create_route(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((team_slug, project_slug)): Path<(String, String)>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return sign_in();
    };
    let door = match RouteDoor::open(&state, &caller, &team_slug, &project_slug).await {
        Ok(door) => door,
        Err(response) => return response,
    };
    let form = RouteForm::read(&fields);
    let checked = match door.bot.as_ref() {
        Some(bot) => postable_destination(bot, &form.destination).await,
        None => None,
    };
    let Some(destination) = checked else {
        return door
            .form(
                &state,
                &caller,
                &headers,
                &form,
                FormAnswer::with(Some(CANNOT_POST), StatusCode::UNPROCESSABLE_ENTITY),
            )
            .await;
    };
    if destination.shared_with_another_organisation && !form.shared_confirmed {
        // K-6 shows on the form for the chosen channel; nothing is saved.
        return door
            .form(
                &state,
                &caller,
                &headers,
                &form,
                FormAnswer::with(None, StatusCode::UNPROCESSABLE_ENTITY),
            )
            .await;
    }
    let draft = RouteDraft {
        destination_ref: destination.reference.0,
        destination_label: destination.label,
        events: form.events,
        content_level: form.content_level,
        lanes: form.lanes,
        shared_confirmed: form.shared_confirmed,
    };
    match caller
        .services
        .create_route(
            &caller.principal(),
            &team_slug,
            &project_slug,
            draft,
            state.clock.now(),
        )
        .await
    {
        Ok(_) => Redirect::to(&base_path(&team_slug, &project_slug)).into_response(),
        Err(err) => refusal(err),
    }
}

pub async fn pause_route(
    State(state): State<AppState>,
    session: Session,
    Path((team_slug, project_slug, route_id)): Path<(String, String, String)>,
) -> Response {
    change_route(
        &state,
        &session,
        &team_slug,
        &project_slug,
        &route_id,
        RouteAction::Pause,
    )
    .await
}

pub async fn resume_route(
    State(state): State<AppState>,
    session: Session,
    Path((team_slug, project_slug, route_id)): Path<(String, String, String)>,
) -> Response {
    let integrations = state.integrations.current();
    change_route(
        &state,
        &session,
        &team_slug,
        &project_slug,
        &route_id,
        RouteAction::Resume(&*integrations),
    )
    .await
}

pub async fn edit_route(
    State(state): State<AppState>,
    session: Session,
    Path((team_slug, project_slug, route_id)): Path<(String, String, String)>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Response {
    let form = RouteForm::read(&fields);
    let draft = RouteEditDraft {
        events: form.events,
        content_level: form.content_level,
        lanes: form.lanes,
    };
    let action = RouteAction::Edit(draft);
    change_route(
        &state,
        &session,
        &team_slug,
        &project_slug,
        &route_id,
        action,
    )
    .await
}

pub async fn delete_route(
    State(state): State<AppState>,
    session: Session,
    Path((team_slug, project_slug, route_id)): Path<(String, String, String)>,
) -> Response {
    change_route(
        &state,
        &session,
        &team_slug,
        &project_slug,
        &route_id,
        RouteAction::Delete,
    )
    .await
}

/// One route change, then back to the project's integrations page (also
/// when the route already was so).
async fn change_route(
    state: &AppState,
    session: &Session,
    team_slug: &str,
    project_slug: &str,
    route_id: &str,
    action: RouteAction<'_>,
) -> Response {
    let Some(caller) = Caller::of(state, session).await else {
        return sign_in();
    };
    let Ok(route_id) = uuid::Uuid::parse_str(route_id) else {
        return resource_not_found_page();
    };
    let changed = caller
        .services
        .change_route(
            &caller.principal(),
            team_slug,
            project_slug,
            route_id,
            action,
            state.clock.now(),
        )
        .await;
    match changed {
        Ok(_) => Redirect::to(&base_path(team_slug, project_slug)).into_response(),
        Err(err) => refusal(err),
    }
}

/// The project's route `route_id` for its edit form or delete dialog.
async fn managed_route(
    caller: &Caller,
    team_slug: &str,
    project_slug: &str,
    route_id: &str,
) -> Result<ManagedRoute, Response> {
    let route_id = uuid::Uuid::parse_str(route_id).map_err(|_| resource_not_found_page())?;
    caller
        .services
        .managed_route(&caller.principal(), team_slug, project_slug, route_id)
        .await
        .map_err(refusal)
}

pub async fn show_edit_route(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((team_slug, project_slug, route_id)): Path<(String, String, String)>,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return sign_in();
    };
    let ManagedRoute { target, route } =
        match managed_route(&caller, &team_slug, &project_slug, &route_id).await {
            Ok(managed) => managed,
            Err(response) => return response,
        };
    let form = RouteForm {
        destination: route.destination_ref.clone(),
        events: route.events,
        content_level: route.content_level,
        lanes: route.lanes,
        ..RouteForm::default()
    };
    let door = RouteDoor::at(&state, target, &team_slug, &project_slug).editing(
        route.id,
        Destination {
            reference: DestinationRef(route.destination_ref),
            label: route.destination_label,
            shared_with_another_organisation: false,
        },
    );
    door.form(&state, &caller, &headers, &form, FormAnswer::plain())
        .await
}

pub async fn show_delete_route(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((team_slug, project_slug, route_id)): Path<(String, String, String)>,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return sign_in();
    };
    let ManagedRoute { target, route } =
        match managed_route(&caller, &team_slug, &project_slug, &route_id).await {
            Ok(managed) => managed,
            Err(response) => return response,
        };
    let kind = ChannelKind::from_wire_name(&route.kind).unwrap_or(ChannelKind::Slack);
    let base = base_path(&team_slug, &project_slug);
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let page = RouteDeletePage {
        nav: caller.nav(&state, csrf.clone()).await,
        csrf,
        question: DELETE_QUESTION
            .replace("{destination}", &route.destination_label)
            .replace("{project}", &target.project_name)
            .replace("{kind}", kind.display_name()),
        destination_label: route.destination_label,
        delete_path: format!("{base}/routes/{}/delete", route.id),
        routes_path: base,
    };
    render(page.render(), set_cookie)
}

/// The channel the form names, when Slack lists it among those Foundry can
/// post to (the destination check before any write).
async fn postable_destination(
    catalog: &dyn DestinationCatalog,
    reference: &str,
) -> Option<Destination> {
    catalog
        .list_destinations()
        .await
        .ok()?
        .into_iter()
        .find(|destination| destination.reference.0 == reference)
}

/// A route door's target (Manage, else the uniform 404) and the workspace's
/// Slack bot, when there is a connection this Foundry can open.
struct RouteDoor {
    target: RouteTarget,
    bot: Option<super::slack::SlackBot>,
    routes_path: String,
    /// Where Save posts: `routes_path`, or the edited route's edit path.
    action_path: String,
    /// The edited route's destination: the only channel its form offers
    /// (an edit keeps its destination and asks Slack nothing).
    editing: Option<Destination>,
}

impl RouteDoor {
    async fn open(
        state: &AppState,
        caller: &Caller,
        team_slug: &str,
        project_slug: &str,
    ) -> Result<Self, Response> {
        let target = caller
            .services
            .route_target(&caller.principal(), team_slug, project_slug)
            .await
            .map_err(refusal)?;
        Ok(Self::at(state, target, team_slug, project_slug))
    }

    /// The door for an already checked `target`. Opening the bot asks Slack
    /// nothing.
    fn at(state: &AppState, target: RouteTarget, team_slug: &str, project_slug: &str) -> Self {
        let bot = target.connection.as_ref().and_then(|connection| {
            let sealed = connection.sealed_credential.clone()?;
            state
                .integrations
                .current()
                .slack_bot(connection.id, sealed)
        });
        let routes_path = format!("{}/routes", base_path(team_slug, project_slug));
        Self {
            target,
            bot,
            action_path: routes_path.clone(),
            routes_path,
            editing: None,
        }
    }

    /// The door for editing the route `route_id` to `destination`.
    fn editing(self, route_id: uuid::Uuid, destination: Destination) -> Self {
        Self {
            action_path: format!("{}/{route_id}/edit", self.routes_path),
            editing: Some(destination),
            ..self
        }
    }

    /// The channels the picker offers: Slack's list, or for a preview only the
    /// channel the form names, since a preview asks Slack nothing.
    async fn channels(&self, form: &RouteForm, answer: &FormAnswer) -> Vec<Destination> {
        if let Some(destination) = &self.editing {
            return vec![destination.clone()];
        }
        match &self.bot {
            Some(bot) if answer.asks_slack() => bot.list_destinations().await.unwrap_or_default(),
            Some(_) if !form.destination.is_empty() => vec![Destination {
                reference: DestinationRef(form.destination.clone()),
                label: form.destination_label(),
                shared_with_another_organisation: false,
            }],
            _ => Vec::new(),
        }
    }

    /// The route form for `form` with `answer`; K-4 without a usable
    /// connection, K-5 for the chosen channel and K-6 when it is shared.
    async fn form(
        &self,
        state: &AppState,
        caller: &Caller,
        headers: &HeaderMap,
        form: &RouteForm,
        answer: FormAnswer,
    ) -> Response {
        let channels = self.channels(form, &answer).await;
        let chosen = channels.iter().find(|c| c.reference.0 == form.destination);
        let audience = chosen.map(|c| AUDIENCE.replace("{destination}", &c.label));
        let shared_channel = chosen
            .filter(|c| c.shared_with_another_organisation)
            .map(|c| SHARED_CHANNEL.replace("{destination}", &c.label));
        let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
        let page = RouteFormPage {
            nav: caller.nav(state, csrf.clone()).await,
            csrf,
            heading: match &self.editing {
                Some(destination) => format!("Edit the route to {}", destination.label),
                None => format!("New Slack route for {}", self.target.project_name),
            },
            action_path: self.action_path.clone(),
            test_path: format!("{}/test", self.routes_path),
            preview_path: format!("{}/preview", self.routes_path),
            notice: answer.notice,
            audience,
            shared_tick: shared_channel.is_some() || form.shared_confirmed,
            shared_channel,
            shared_confirmed: form.shared_confirmed,
            preview: answer.preview,
            no_issue_yet: format!("{} has no issue to preview yet.", self.target.project_name),
            no_connection: self.bot.is_none(),
            channels: channels
                .into_iter()
                .map(|c| Choice {
                    chosen: c.reference.0 == form.destination,
                    value: c.reference.0,
                    label: c.label,
                })
                .collect(),
            events: choices(&EVENT_CHOICES, |value| {
                form.events.iter().any(|e| e == value)
            }),
            lanes: self
                .target
                .lanes
                .iter()
                .map(|lane| Choice {
                    chosen: form.lanes.contains(&lane.slug),
                    value: lane.slug.clone(),
                    label: lane.label.clone(),
                })
                .collect(),
            levels: choices(&LEVEL_CHOICES, |value| form.content_level == value),
            private_channels: PRIVATE_CHANNELS,
            no_chat_connection: NO_CHAT_CONNECTION,
        };
        match page.render() {
            Ok(html) => {
                response_with_optional_cookie(answer.status, Html(html).into_response(), set_cookie)
            }
            Err(err) => {
                tracing::error!(error = %err, "integrations: render failed");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
            }
        }
    }
}

fn choices(table: &[(&str, &str)], chosen: impl Fn(&str) -> bool) -> Vec<Choice> {
    table
        .iter()
        .map(|(value, label)| Choice {
            value: value.to_string(),
            label: label.to_string(),
            chosen: chosen(value),
        })
        .collect()
}

/// `/team/{t}/project/{p}/integrations`.
fn base_path(team_slug: &str, project_slug: &str) -> String {
    format!("/team/{team_slug}/project/{project_slug}/integrations")
}

/// A stored route, its K-9 status (derived, DDR-19) and its history page
/// under the project's integrations path `base`.
fn route_line(route: &ProjectRoute, base: &str) -> RouteLine {
    let facts = RouteFacts {
        kind: ChannelKind::from_wire_name(&route.kind).unwrap_or(ChannelKind::Slack),
        paused: route.paused,
        connection_state: route
            .connection_state
            .as_deref()
            .and_then(ConnectionState::from_wire_name),
        problem: route
            .problem_reason
            .as_deref()
            .and_then(ReasonCode::from_wire_name),
        missing_lane: route.missing_lane.clone(),
        newest_finished: route.newest_finished.as_ref().map(|finished| {
            FinishedDelivery::read(
                &finished.status,
                finished.final_reason.as_deref(),
                finished.issue_key.as_deref(),
            )
        }),
        head_failed: route
            .head_failed
            .as_ref()
            .map(|reason| reason.as_deref().and_then(ReasonCode::from_wire_name)),
        last_delivered_at: route.last_delivered_at,
    };
    let stopped = route.paused || route.problem_reason.is_some();
    let (toggle, toggle_label) = if stopped {
        ("resume", "Resume")
    } else {
        ("pause", "Pause")
    };
    let route_path = format!("{base}/routes/{}", route.id);
    RouteLine {
        id: route.id,
        destination_label: route.destination_label.clone(),
        status: RouteStatus::derive(&facts).words(&route.destination_label),
        history_path: format!("{route_path}/history"),
        edit_path: format!("{route_path}/edit"),
        delete_path: format!("{route_path}/delete"),
        toggle_path: format!("{route_path}/{toggle}"),
        toggle_label,
    }
}

/// A stored delivery's history line as of `now`, worded by the core (UI-7).
fn history_line(
    row: &DeliveryHistoryRow,
    channel: ChannelKind,
    destination: &str,
    now: time::OffsetDateTime,
) -> String {
    DeliveryRecord {
        channel,
        occurred_at: row.occurred_at,
        event_kind: &row.event_kind,
        issue_key: row.issue_key.as_deref(),
        status: &row.status,
        attempts: row.attempts,
        first_attempt_at: row.first_attempt_at,
        finished_at: row.finished_at,
        give_up_at: row.give_up_at,
        lease_until: row.lease_until,
        next_attempt_at: row.next_attempt_at,
        last_vendor_reason: row.last_vendor_reason.as_deref(),
        final_reason: row.final_reason.as_deref(),
        slowed_down: row.slowed_down,
    }
    .line(destination, now)
}

/// Exchange the code, read the domain, seal the token for its row, save.
/// Why a Slack install left the workspace's connection as it was.
enum NotConnected {
    /// The approval was for another Slack workspace than the connected one.
    OtherWorkspace {
        approved: String,
        connected: String,
    },
    Failed(&'static str),
}

impl From<&'static str> for NotConnected {
    fn from(why: &'static str) -> Self {
        Self::Failed(why)
    }
}

async fn connect(
    state: &AppState,
    caller: &Caller,
    principal: &Principal,
    code: &str,
) -> Result<(), NotConnected> {
    let integrations: std::sync::Arc<Integrations> = state.integrations.current();
    let (Some(app), Some(secrets)) = (
        integrations.slack(),
        integrations.settings().secrets.as_ref(),
    ) else {
        return Err("slack_not_offered".into());
    };
    let grant: SlackGrant = app
        .exchange(code, &integrations.slack_return_url())
        .await
        .map_err(|_| "oauth_exchange")?;
    let domain = app
        .domain(&grant.access_token)
        .await
        .map_err(|_| "auth_test")?;
    let connection_id = caller
        .services
        .slack_connection_id(principal)
        .await
        .map_err(|_| "connection_row")?;
    let slot = SecretSlot::new(
        "integration_connections",
        &connection_id.to_string(),
        "sealed_credential",
    );
    let sealed = secrets
        .seal(&grant.access_token, &slot)
        .map_err(|_| "seal")?;
    let approved = grant.team_name.clone();
    let install = SlackInstall {
        team_id: grant.team_id,
        team_name: grant.team_name,
        domain,
        app_id: grant.app_id,
        bot_user_id: grant.bot_user_id,
    };
    match caller
        .services
        .connect_slack(principal, install, connection_id, sealed, state.clock.now())
        .await
    {
        Ok(ConnectionWriteOutcome::StaleRow) => Err("stale_row".into()),
        Ok(ConnectionWriteOutcome::OtherWorkspace { connected_label }) => {
            Err(NotConnected::OtherWorkspace {
                approved,
                connected: connected_label,
            })
        }
        Ok(_) => Ok(()),
        Err(_) => Err("store".into()),
    }
}

async fn workspace_page(
    state: &AppState,
    caller: &Caller,
    headers: &HeaderMap,
    notice: Option<&str>,
) -> Response {
    let WorkspaceIntegrations {
        slack,
        slack_lost,
        activity,
    } = match caller
        .services
        .workspace_integrations(&caller.principal())
        .await
    {
        Ok(view) => view,
        Err(err) => return refusal(err),
    };
    let offer = state.integrations.current().slack_offer();
    let reconnect = slack_lost.is_some();
    let slack = slack.or_else(|| slack_lost.map(|lost| lost_words(&lost)));
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let page = WorkspacePage {
        nav: caller.nav(state, csrf).await,
        notice: notice.map(str::to_string),
        not_set_up: offer == SlackOffer::NotSetUp,
        no_secrets_key: offer == SlackOffer::NoSecretsKey,
        unavailable: offer == SlackOffer::Unavailable && slack.is_some(),
        slack_line: slack,
        reconnect,
        activity,
    };
    render(page.render(), set_cookie)
}

/// K-10, up to its Reconnect action.
fn lost_words(lost: &LostSlack) -> String {
    ConnectionLostNotice {
        kind: ChannelKind::Slack,
        workspace: lost.workspace_name.clone(),
        reason: lost.reason.as_deref().and_then(ReasonCode::from_wire_name),
    }
    .words()
}

/// `POST /integrations/slack/events`: Slack's Events API. The signature's
/// freshness is judged on the wall clock, as Slack stamps it.
pub async fn slack_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let integrations = state.integrations.current();
    let Some(app) = integrations.slack() else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let header = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
    let event = app.read_event(
        header("x-slack-request-timestamp"),
        header("x-slack-signature"),
        &body,
        time::OffsetDateTime::now_utc(),
    );
    match event {
        SlackEvent::Unsigned => StatusCode::UNAUTHORIZED.into_response(),
        SlackEvent::Unreadable => StatusCode::BAD_REQUEST.into_response(),
        SlackEvent::UrlVerification { challenge } => {
            Json(serde_json::json!({ "challenge": challenge })).into_response()
        }
        SlackEvent::AppLost {
            team_id,
            app_id,
            reason,
        } => {
            let health = StoreCapabilities::new(state.store.clone());
            match health
                .slack_app_lost(&team_id, &app_id, reason.wire_name(), state.clock.now())
                .await
            {
                Ok(Some(lost)) => {
                    state
                        .integrations
                        .admin_mail()
                        .connection_lost(&lost, &integrations.settings().public_url)
                        .await;
                    StatusCode::OK.into_response()
                }
                Ok(None) => StatusCode::OK.into_response(),
                Err(err) => {
                    tracing::error!(error = %err, "integrations: Slack event not recorded");
                    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
                }
            }
        }
        SlackEvent::Other => StatusCode::OK.into_response(),
    }
}

/// A signed-out visitor goes to sign in, as from any page that needs a
/// session (the shipped `redirect_to("/sign-in")`: 303, `Location: /sign-in`).
/// The uniform not-found (SC-6) is for signed-in people without the right.
fn sign_in() -> Response {
    Redirect::to("/sign-in").into_response()
}

fn refusal(err: IntegrationError) -> Response {
    if let IntegrationError::Store(err) = err {
        tracing::error!(error = %err, "integrations: store read failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
    }
    resource_not_found_page()
}

fn render(rendered: Result<String, askama::Error>, set_cookie: Option<String>) -> Response {
    match rendered {
        Ok(html) => {
            response_with_optional_cookie(StatusCode::OK, Html(html).into_response(), set_cookie)
        }
        Err(err) => {
            tracing::error!(error = %err, "integrations: render failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
        }
    }
}
