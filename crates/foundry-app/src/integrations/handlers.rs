//! The integration doors (DESIGN [REF] Driving Ports; ADR-EIP-005 decisions
//! 2 and 4). Every door answers the uniform 404 to a caller without the right
//! (SC-6), decided by the use-case before anything else.
//!
//! - `GET /workspace/integrations`: the Slack connection (K-1), what is
//!   missing (K-2, K-20) or Add to Slack, and the activity list (R-4).
//! - `POST /workspace/integrations/matrix`: connects Matrix (admins,
//!   ADR-EIP-006 decision 1): the homeserver's address must be `https` and
//!   pass the outbound address policy before anything is contacted, and
//!   `whoami` must accept the bot's token; only then is the token sealed for
//!   its connection row and saved. Anything else saves nothing and says why.
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
//!   with another organisation is saved only with the K-6 tick. A Matrix
//!   route first brings the bot into the room (`DestinationEnrolment`) and
//!   reads it: a room the bot cannot join, or an encrypted one (which the
//!   bot leaves again), is explained and no route is saved (ADR-EIP-006
//!   decision 2).
//! - `POST …/integrations/routes/test` with a saved Matrix route's
//!   `route_id`: the K-7 message to its room, as a fresh transaction.
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
//!   message already posted (Manage). A deleted Matrix route's bot leaves its
//!   room when no other route of the workspace uses it; a leave the
//!   homeserver refuses goes to the log and never undoes the delete.
//! - `GET /workspace/integrations/slack/disconnect`: the disconnect question,
//!   counting the routes that stop (admins); its `POST` disconnects: the
//!   connection's credential is forgotten and what its routes waited to send
//!   is skipped, in one transaction, and only then is Slack asked to withdraw
//!   Foundry's access (`apps.uninstall`), so a Slack failure never leaves the
//!   credential stored. Works without scripting; CSRF-protected.
//! - `GET`/`POST /workspace/integrations/matrix/disconnect`: the same door
//!   for Matrix (user decision 2026-10-25): the bot's access token is
//!   forgotten and the routes pause; the bot stays in its rooms.
//! - `POST /integrations/slack/events`: Slack's Events API (DDR-11,
//!   ADR-EIP-005 decision 6), a machine post with no session, believed only
//!   when Slack signed it (401 otherwise): it answers Slack's address check
//!   with its challenge, and the Foundry app's removal from a connected Slack
//!   workspace (`app_uninstalled`, `tokens_revoked`) turns that connection
//!   lost and tells its admins once. Anything else is acknowledged and
//!   changes nothing.

use super::matrix::{self, ConnectRefusal, MatrixBot, RoomFacts, RoomRefusal};
use super::slack::{self, InstallState, SlackEvent};
use super::store_capabilities::{ConnectionHealth, StoreCapabilities};
use super::{chat_kind, credential_slot, SlackOffer};
use crate::bootstrap::{resource_not_found_page, SessionUser};
use crate::csrf::{ensure_csrf_cookie, response_with_optional_cookie};
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::{Form, Json};
use foundry_integrations::{
    compose, encode_matrix, encode_slack, issue_url, test_message, ChannelKind, ChannelOutcome,
    ConnectionLostNotice, ConnectionState, ContentLevel, DeliveryChannel, DeliveryKey,
    DeliveryRecord, Destination, DestinationCatalog, DestinationEnrolment, DestinationRef,
    EventKind, EventView, FinishedDelivery, MessageLines, ReasonCode, RouteFacts, RouteStatus,
};
use foundry_services::integrations::{
    IntegrationError, LostSlack, ManagedRoute, MatrixLine, MatrixLogin, ProjectIntegrations,
    RouteAction, RouteDraft, RouteEditDraft, RouteHistory, RoutePreview, RouteTarget, SlackInstall,
    WorkspaceIntegrations,
};
use foundry_services::{Principal, Services};
use foundry_store::integrations::{
    ConnectionWriteOutcome, DeliveryHistoryRow, DisconnectOutcome, DisconnectPreview, ProjectRoute,
    RouteChangeOutcome,
};
use serde::Deserialize;
use tower_sessions::Session;

pub const WORKSPACE_PATH: &str = "/workspace/integrations";
pub const SETUP_PATH: &str = "/workspace/integrations/slack/setup";
pub const INSTALL_PATH: &str = "/workspace/integrations/slack/install";
pub const RETURN_PATH: &str = slack::RETURN_PATH;
pub const DISCONNECT_PATH: &str = "/workspace/integrations/slack/disconnect";
pub const MATRIX_DISCONNECT_PATH: &str = "/workspace/integrations/matrix/disconnect";
pub const MATRIX_CONNECT_PATH: &str = "/workspace/integrations/matrix";

/// US-EIP-08: the homeserver answered and refused the bot's token.
pub const MATRIX_TOKEN_REFUSED: &str =
    "The homeserver did not accept this access token. Nothing was saved.";
/// US-EIP-08, for the entered `{homeserver}`.
pub const MATRIX_UNREACHABLE: &str = "Foundry could not reach {homeserver}. Nothing was saved.";
/// UI-6 (R-1): the outbound address policy refused the address.
pub const ADDRESS_REFUSED: &str =
    "Foundry will not send to this address. An instance operator can allow it.";
/// The Matrix connection could not be saved (another homeserver is connected).
pub const MATRIX_NOT_CONNECTED: &str = "Matrix was not connected. Nothing changed.";
/// K-14, for the bot `{bot}` and the room `{room}`.
pub const INVITE_THE_BOT: &str =
    "Invite {bot} to {room} in your Matrix client first, then try again.";
/// K-15, for the room `{room}`.
pub const ROOM_ENCRYPTED: &str = "{room} is end-to-end encrypted. Foundry's bot cannot post to encrypted rooms yet. Choose an unencrypted room.";
/// US-EIP-09: the saved route's room, its name and its members.
pub const BOT_IN_ROOM: &str = "Foundry is in {room} (room \"{name}\", {members} members)";

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
    /// The Matrix connection line and its token field, when connected.
    matrix: Option<MatrixLine>,
    /// Matrix can be connected: the secrets key is set.
    matrix_offered: bool,
    matrix_connect_path: &'static str,
    matrix_disconnect_path: &'static str,
    csrf: String,
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
    notice: Option<String>,
    project_name: String,
    new_route_path: String,
    /// Send test for a saved Matrix route.
    test_path: String,
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
    /// A Matrix route offers Send test once saved (ADR-EIP-006 decision 2).
    matrix: bool,
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
struct DisconnectPage {
    nav: crate::nav::NavContext,
    csrf: String,
    /// "Slack" or "Matrix".
    kind: &'static str,
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
    /// A Matrix route: its room is typed, not picked.
    matrix: bool,
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
    kind: String,
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
                "kind" => form.kind = value.clone(),
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
    kind: Option<String>,
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

    /// The caller when they are an admin of their workspace; else signing in,
    /// or the uniform not-found page.
    async fn admin(state: &AppState, session: &Session) -> Result<Self, Response> {
        let caller = Self::of(state, session).await.ok_or_else(sign_in)?;
        caller
            .services
            .require_workspace_admin(&caller.principal())
            .await
            .map_err(refusal)?;
        Ok(caller)
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
    let caller = match Caller::admin(&state, &session).await {
        Ok(caller) => caller,
        Err(response) => return response,
    };
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
    let caller = match Caller::admin(&state, &session).await {
        Ok(caller) => caller,
        Err(response) => return response,
    };
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
    let caller = match Caller::admin(&state, &session).await {
        Ok(caller) => caller,
        Err(response) => return response,
    };
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
    match connect(&state, &caller, code).await {
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
    show_disconnect(&state, &session, &headers, ChannelKind::Slack).await
}

pub async fn show_matrix_disconnect(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    show_disconnect(&state, &session, &headers, ChannelKind::Matrix).await
}

/// The disconnect question for the workspace's connection of `kind`
/// (admins; anyone else gets the uniform not-found page).
async fn show_disconnect(
    state: &AppState,
    session: &Session,
    headers: &HeaderMap,
    kind: ChannelKind,
) -> Response {
    let Some(caller) = Caller::of(state, session).await else {
        return sign_in();
    };
    let preview = match caller
        .services
        .disconnect_preview(&caller.principal(), kind.wire_name())
        .await
    {
        Ok(Some(preview)) => preview,
        Ok(None) => return Redirect::to(WORKSPACE_PATH).into_response(),
        Err(err) => return refusal(err),
    };
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let page = DisconnectPage {
        nav: caller.nav(state, csrf.clone()).await,
        csrf,
        kind: kind.display_name(),
        question: disconnect_question(kind, &preview),
        disconnect_path: disconnect_path(kind),
        workspace_path: WORKSPACE_PATH,
    };
    render(page.render(), set_cookie)
}

fn disconnect_path(kind: ChannelKind) -> &'static str {
    match kind {
        ChannelKind::Matrix => MATRIX_DISCONNECT_PATH,
        _ => DISCONNECT_PATH,
    }
}

/// The workspace's connection of `kind` disconnected (admins; anyone else
/// gets the uniform not-found page): its credential forgotten and its routes
/// paused, in one transaction.
async fn disconnect(
    state: &AppState,
    session: &Session,
    kind: ChannelKind,
) -> Result<DisconnectOutcome, Response> {
    let caller = Caller::of(state, session).await.ok_or_else(sign_in)?;
    caller
        .services
        .disconnect(&caller.principal(), kind.wire_name(), state.clock.now())
        .await
        .map_err(refusal)
}

/// The Matrix disconnect: nothing is asked of the homeserver (the bot stays
/// in its rooms).
pub async fn disconnect_matrix(State(state): State<AppState>, session: Session) -> Response {
    match disconnect(&state, &session, ChannelKind::Matrix).await {
        Ok(_) => Redirect::to(WORKSPACE_PATH).into_response(),
        Err(response) => response,
    }
}

/// The Slack disconnect: Foundry forgets the credential first, then asks
/// Slack to withdraw the access it granted, so a Slack failure only goes to
/// the log.
pub async fn disconnect_slack(State(state): State<AppState>, session: Session) -> Response {
    let outcome = match disconnect(&state, &session, ChannelKind::Slack).await {
        Ok(outcome) => outcome,
        Err(response) => return response,
    };
    if let DisconnectOutcome::Disconnected {
        connection_id,
        sealed_credential,
        ..
    } = outcome
    {
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
    }
    Redirect::to(WORKSPACE_PATH).into_response()
}

/// US-EIP-07's disconnect question: "Disconnect Slack? 3 routes in 2
/// projects stop posting. Foundry's access to Canzan Labs' Slack is removed."
/// For Matrix (DIST-25): "… Foundry forgets the bot's access token."
fn disconnect_question(kind: ChannelKind, preview: &DisconnectPreview) -> String {
    let count = |n: i64, noun: &str| {
        if n == 1 {
            format!("1 {noun}")
        } else {
            format!("{n} {noun}s")
        }
    };
    let verb = if preview.routes == 1 { "stops" } else { "stop" };
    let consequence = match kind {
        ChannelKind::Matrix => "Foundry forgets the bot's access token.".to_string(),
        _ => {
            let owner = if preview.label.ends_with('s') {
                format!("{}'", preview.label)
            } else {
                format!("{}'s", preview.label)
            };
            format!("Foundry's access to {owner} Slack is removed.")
        }
    };
    format!(
        "Disconnect {}? {} in {} {verb} posting. {consequence}",
        kind.display_name(),
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
    project_page(&state, &caller, &headers, &team_slug, &project_slug, None).await
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
    let channel = stored_kind(&kind);
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
    let kind = chat_kind(query.kind.as_deref().unwrap_or_default());
    door.of_kind(kind)
        .form(
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
    if let Some((_, route_id)) = fields.iter().find(|(name, _)| name == "route_id") {
        return send_matrix_test(&state, &caller, &headers, &door, route_id).await;
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
    let status = match outcome {
        ChannelOutcome::Delivered => StatusCode::OK,
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    };
    let notice = test_notice(outcome, "the channel");
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
    if chat_kind(&form.kind) == ChannelKind::Matrix {
        let door = door.of_kind(ChannelKind::Matrix);
        return create_matrix_route(&state, &caller, &headers, &door, &form).await;
    }
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
        kind: foundry_services::integrations::SLACK.to_string(),
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
    let Some(caller) = Caller::of(&state, &session).await else {
        return sign_in();
    };
    let Ok(managed) = managed_route(&caller, &team_slug, &project_slug, &route_id).await else {
        return change_route(
            &state,
            &session,
            &team_slug,
            &project_slug,
            &route_id,
            RouteAction::Delete,
        )
        .await;
    };
    let deleted = caller
        .services
        .change_route(
            &caller.principal(),
            &team_slug,
            &project_slug,
            managed.route.id,
            RouteAction::Delete,
            state.clock.now(),
        )
        .await;
    match deleted {
        Ok(outcome) => {
            if matches!(outcome, RouteChangeOutcome::Changed { .. }) {
                leave_unrouted_room(&state, &caller, &managed).await;
            }
            Redirect::to(&base_path(&team_slug, &project_slug)).into_response()
        }
        Err(err) => refusal(err),
    }
}

/// After a Matrix route's delete: the bot leaves its room when no other
/// route of the workspace uses it. A leave that fails goes to the log; the
/// delete stands and the room keeps what was posted.
async fn leave_unrouted_room(state: &AppState, caller: &Caller, managed: &ManagedRoute) {
    let route = &managed.route;
    if ChannelKind::from_wire_name(&route.kind) != Some(ChannelKind::Matrix) {
        return;
    }
    let still_routed = caller
        .services
        .room_still_routed(&caller.principal(), &route.kind, &route.destination_ref)
        .await;
    if !matches!(still_routed, Ok(false)) {
        return;
    }
    let bot = managed
        .target
        .matrix_connection
        .as_ref()
        .and_then(|connection| state.integrations.current().matrix_bot(connection));
    let left = match bot {
        Some(bot) => bot
            .withdraw(&DestinationRef(route.destination_ref.clone()))
            .await
            .map_err(|reason| reason.wire_name()),
        None => Err("no_bot"),
    };
    if let Err(reason) = left {
        tracing::warn!(
            reason,
            "matrix route delete: the bot did not leave the room; the route is deleted"
        );
    }
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
    let kind = stored_kind(&route.kind);
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
    /// The workspace's Matrix bot, when it is connected and opens here.
    matrix: Option<MatrixBot>,
    /// The kind of route the form makes.
    kind: ChannelKind,
    team_slug: String,
    project_slug: String,
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
        let integrations = state.integrations.current();
        let bot = target.connection.as_ref().and_then(|connection| {
            let sealed = connection.sealed_credential.clone()?;
            integrations.slack_bot(connection.id, sealed)
        });
        let matrix = target
            .matrix_connection
            .as_ref()
            .and_then(|connection| integrations.matrix_bot(connection));
        let routes_path = format!("{}/routes", base_path(team_slug, project_slug));
        Self {
            target,
            bot,
            matrix,
            kind: ChannelKind::Slack,
            team_slug: team_slug.to_string(),
            project_slug: project_slug.to_string(),
            action_path: routes_path.clone(),
            routes_path,
            editing: None,
        }
    }

    /// The connection's homeserver address, as the admin entered it.
    fn matrix_homeserver(&self) -> String {
        self.target
            .matrix_connection
            .as_ref()
            .map(|c| c.external_id.clone())
            .unwrap_or_default()
    }

    /// The words for a room the bot cannot take a route in (K-14, K-15).
    fn room_refusal(&self, refusal: RoomRefusal, room: &str) -> String {
        match refusal {
            RoomRefusal::NotInvited => {
                let bot = self
                    .target
                    .matrix_connection
                    .as_ref()
                    .and_then(|c| c.bot_user_id.as_deref())
                    .unwrap_or_default();
                INVITE_THE_BOT.replace("{bot}", bot).replace("{room}", room)
            }
            RoomRefusal::Encrypted => ROOM_ENCRYPTED.replace("{room}", room),
            RoomRefusal::Unreachable => {
                MATRIX_UNREACHABLE.replace("{homeserver}", &self.matrix_homeserver())
            }
        }
    }

    /// The project's integrations page, with `notice` above its routes.
    async fn project_page(
        &self,
        state: &AppState,
        caller: &Caller,
        headers: &HeaderMap,
        notice: String,
    ) -> Response {
        project_page(
            state,
            caller,
            headers,
            &self.team_slug,
            &self.project_slug,
            Some(notice),
        )
        .await
    }

    /// The form's heading: the edited route's, or the new route's.
    fn heading(&self) -> String {
        match &self.editing {
            Some(destination) => format!("Edit the route to {}", destination.label),
            None => format!(
                "New {} route for {}",
                self.kind.display_name(),
                self.target.project_name
            ),
        }
    }

    /// K-4: no connection of the form's kind that this Foundry can open.
    fn no_connection(&self) -> bool {
        match self.kind {
            ChannelKind::Matrix => self.matrix.is_none(),
            _ => self.bot.is_none(),
        }
    }

    /// The door for a route of `kind`.
    fn of_kind(self, kind: ChannelKind) -> Self {
        Self { kind, ..self }
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
        if self.kind == ChannelKind::Matrix {
            // A room is typed; its audience is whoever can read it (K-5).
            return (!form.destination.is_empty())
                .then(|| Destination {
                    reference: DestinationRef(form.destination.clone()),
                    label: form.destination.clone(),
                    shared_with_another_organisation: false,
                })
                .into_iter()
                .collect();
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
            heading: self.heading(),
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
            no_connection: self.no_connection(),
            matrix: self.kind == ChannelKind::Matrix && self.editing.is_none(),
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
        render_with(answer.status, page.render(), set_cookie)
    }
}

/// `POST /workspace/integrations/matrix` (admins): the homeserver's address
/// is judged before it is contacted, the token checked with `whoami`, and
/// only an accepted token is sealed and saved.
pub async fn connect_matrix(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(fields): Form<Vec<(String, String)>>,
) -> Response {
    let caller = match Caller::admin(&state, &session).await {
        Ok(caller) => caller,
        Err(response) => return response,
    };
    let field = |name: &str| {
        fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.trim().to_string())
            .unwrap_or_default()
    };
    let (entered, token) = (field("homeserver"), field("access_token"));
    let notice = match save_matrix(&state, &caller, &entered, token).await {
        Ok(()) => return Redirect::to(WORKSPACE_PATH).into_response(),
        Err(notice) => notice,
    };
    workspace_page(&state, &caller, &headers, Some(&notice)).await
}

/// Check the bot's token at the homeserver, seal it for its row and save
/// the connection; else the words that say why nothing was saved.
async fn save_matrix(
    state: &AppState,
    caller: &Caller,
    entered: &str,
    token: String,
) -> Result<(), String> {
    let integrations = state.integrations.current();
    let unreachable = || MATRIX_UNREACHABLE.replace("{homeserver}", entered);
    let Some(secrets) = integrations.settings().secrets.as_ref() else {
        return Err(MATRIX_NOT_CONNECTED.to_string());
    };
    let Some((homeserver, host)) = matrix::homeserver_address(entered) else {
        return Err(ADDRESS_REFUSED.to_string());
    };
    let token = secrecy::SecretString::from(token);
    let bot = MatrixBot::new(integrations.outbound().clone(), &homeserver, token.clone());
    let bot_user_id = bot.whoami().await.map_err(|refusal| {
        tracing::warn!(reason = ?refusal, "matrix connect: not connected");
        match refusal {
            ConnectRefusal::Address => ADDRESS_REFUSED.to_string(),
            ConnectRefusal::TokenRefused => MATRIX_TOKEN_REFUSED.to_string(),
            ConnectRefusal::Unreachable => unreachable(),
        }
    })?;
    let principal = caller.principal();
    let connection_id = caller
        .services
        .matrix_connection_id(&principal)
        .await
        .map_err(|_| MATRIX_NOT_CONNECTED.to_string())?;
    let sealed = secrets
        .seal(&token, &credential_slot(connection_id))
        .map_err(|_| MATRIX_NOT_CONNECTED.to_string())?;
    let login = MatrixLogin {
        homeserver,
        host,
        bot_user_id,
    };
    match caller
        .services
        .connect_matrix(&principal, login, connection_id, sealed, state.clock.now())
        .await
    {
        Ok(ConnectionWriteOutcome::Connected | ConnectionWriteOutcome::Reconnected) => Ok(()),
        _ => Err(MATRIX_NOT_CONNECTED.to_string()),
    }
}

/// Save a Matrix route: bring the bot into the room, read the room, and save
/// the route only for an unencrypted room the bot joined; then say so on the
/// project's page. A refused room is explained on the form.
async fn create_matrix_route(
    state: &AppState,
    caller: &Caller,
    headers: &HeaderMap,
    door: &RouteDoor,
    form: &RouteForm,
) -> Response {
    let refused = |notice: String| async move {
        door.form(
            state,
            caller,
            headers,
            form,
            FormAnswer::with(Some(&notice), StatusCode::UNPROCESSABLE_ENTITY),
        )
        .await
    };
    let Some(bot) = door.matrix.as_ref() else {
        return refused(NO_CHAT_CONNECTION.to_string()).await;
    };
    let room = form.destination.trim().to_string();
    let (room_id, facts) = match enrol_in_room(bot, &room).await {
        Ok(joined) => joined,
        Err(refusal) => return refused(door.room_refusal(refusal, &room)).await,
    };
    let draft = RouteDraft {
        kind: foundry_services::integrations::MATRIX.to_string(),
        destination_ref: room_id.0.clone(),
        destination_label: room.clone(),
        events: form.events.clone(),
        content_level: form.content_level.clone(),
        lanes: form.lanes.clone(),
        shared_confirmed: false,
    };
    let saved = caller
        .services
        .create_route(
            &caller.principal(),
            &door.team_slug,
            &door.project_slug,
            draft,
            state.clock.now(),
        )
        .await;
    if let Err(err) = saved {
        let _ = bot.withdraw(&room_id).await;
        return refusal(err);
    }
    let notice = BOT_IN_ROOM
        .replace("{room}", &room)
        .replace("{name}", &facts.name)
        .replace("{members}", &facts.members.to_string());
    door.project_page(state, caller, headers, notice).await
}

/// Bring `bot` into `room` and read it: the room's id and what it shows, for
/// an unencrypted room the bot joined. Else why not; the bot leaves a room it
/// joined but cannot use.
async fn enrol_in_room(
    bot: &MatrixBot,
    room: &str,
) -> Result<(DestinationRef, RoomFacts), RoomRefusal> {
    let room_id = DestinationRef(bot.room_id(room).await?);
    let joined = match bot.enrol(&room_id).await {
        Ok(()) => bot.room(&room_id.0).await,
        Err(ReasonCode::BotNotInRoom) => Err(RoomRefusal::NotInvited),
        Err(_) => Err(RoomRefusal::Unreachable),
    };
    match joined {
        Ok(facts) if !facts.encrypted => Ok((room_id, facts)),
        other => {
            let refusal = other.map_or_else(|refusal| refusal, |_| RoomRefusal::Encrypted);
            if refusal != RoomRefusal::NotInvited {
                let _ = bot.withdraw(&room_id).await;
            }
            Err(refusal)
        }
    }
}

/// Send test on a saved Matrix route: K-7 once to its room, as a fresh
/// transaction recorded nowhere; then back to the project's page.
async fn send_matrix_test(
    state: &AppState,
    caller: &Caller,
    headers: &HeaderMap,
    door: &RouteDoor,
    route_id: &str,
) -> Response {
    let managed = managed_route(caller, &door.team_slug, &door.project_slug, route_id).await;
    let ManagedRoute { route, .. } = match managed {
        Ok(managed) => managed,
        Err(response) => return response,
    };
    let (Some(ChannelKind::Matrix), Some(bot)) = (
        ChannelKind::from_wire_name(&route.kind),
        door.matrix.as_ref(),
    ) else {
        return resource_not_found_page();
    };
    let message = encode_matrix(&test_message(
        &door.target.project_name,
        &door.target.key_prefix,
    ));
    let txn_id = uuid::Uuid::new_v4().to_string();
    let outcome = bot.send(&route.destination_ref, &message, &txn_id).await;
    let notice = test_notice(outcome, &route.destination_label);
    door.project_page(state, caller, headers, notice).await
}

/// What Send test says of `outcome` for the route to `destination` (K-7).
fn test_notice(outcome: ChannelOutcome, destination: &str) -> String {
    match outcome {
        ChannelOutcome::Delivered => TEST_SENT.to_string(),
        ChannelOutcome::Transient { reason, .. }
        | ChannelOutcome::RouteRefused { reason }
        | ChannelOutcome::ConnectionRefused { reason } => format!(
            "The test message was not delivered: {}.",
            reason.words(destination)
        ),
    }
}

/// The project's integrations page, with `notice` above its routes.
async fn project_page(
    state: &AppState,
    caller: &Caller,
    headers: &HeaderMap,
    team_slug: &str,
    project_slug: &str,
    notice: Option<String>,
) -> Response {
    let view = caller
        .services
        .project_integrations(&caller.principal(), team_slug, project_slug)
        .await;
    let ProjectIntegrations {
        project_name,
        routes,
    } = match view {
        Ok(view) => view,
        Err(err) => return refusal(err),
    };
    let base = base_path(team_slug, project_slug);
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let page = ProjectPage {
        nav: caller.nav(state, csrf.clone()).await,
        csrf,
        notice,
        project_name,
        new_route_path: format!("{base}/routes/new?kind=slack"),
        test_path: format!("{base}/routes/test"),
        routes: routes
            .iter()
            .map(|route| route_line(route, &base))
            .collect(),
    };
    render(page.render(), set_cookie)
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
        status: RouteStatus::derive(&route_facts(route))
            .words_naming(&route.destination_label, route.bot_user_id.as_deref()),
        history_path: format!("{route_path}/history"),
        edit_path: format!("{route_path}/edit"),
        delete_path: format!("{route_path}/delete"),
        toggle_path: format!("{route_path}/{toggle}"),
        toggle_label,
        matrix: route.kind == foundry_services::integrations::MATRIX,
    }
}

/// What K-9's status is derived from, read from the stored route.
fn route_facts(route: &ProjectRoute) -> RouteFacts {
    RouteFacts {
        kind: stored_kind(&route.kind),
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
    }
}

/// A stored route's kind; one this Foundry does not know reads as Slack.
fn stored_kind(wire: &str) -> ChannelKind {
    ChannelKind::from_wire_name(wire).unwrap_or(ChannelKind::Slack)
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

/// Exchange the code, read the domain, seal the token for its row, save.
async fn connect(state: &AppState, caller: &Caller, code: &str) -> Result<(), NotConnected> {
    let principal = &caller.principal();
    let integrations = state.integrations.current();
    let (Some(app), Some(secrets)) = (
        integrations.slack(),
        integrations.settings().secrets.as_ref(),
    ) else {
        return Err("slack_not_offered".into());
    };
    let grant = app
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
    let sealed = secrets
        .seal(&grant.access_token, &credential_slot(connection_id))
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
        matrix,
        activity,
    } = match caller
        .services
        .workspace_integrations(&caller.principal())
        .await
    {
        Ok(view) => view,
        Err(err) => return refusal(err),
    };
    let integrations = state.integrations.current();
    let offer = integrations.slack_offer();
    let reconnect = slack_lost.is_some();
    let slack = slack.or_else(|| slack_lost.map(|lost| lost_words(&lost)));
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let page = WorkspacePage {
        nav: caller.nav(state, csrf.clone()).await,
        csrf,
        matrix,
        matrix_offered: integrations.settings().secrets.is_some(),
        matrix_connect_path: MATRIX_CONNECT_PATH,
        matrix_disconnect_path: MATRIX_DISCONNECT_PATH,
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
            let public_url = &integrations.settings().public_url;
            slack_app_lost(&state, public_url, &team_id, &app_id, reason).await
        }
        SlackEvent::Other => StatusCode::OK.into_response(),
    }
}

/// The Foundry app left the Slack workspace `team_id`: its connection turns
/// lost, and the record that turned it tells the workspace's admins, with the
/// address at `public_url` that reconnects it.
async fn slack_app_lost(
    state: &AppState,
    public_url: &str,
    team_id: &str,
    app_id: &str,
    reason: ReasonCode,
) -> Response {
    let health = StoreCapabilities::new(state.store.clone());
    let lost = health
        .slack_app_lost(team_id, app_id, reason.wire_name(), state.clock.now())
        .await;
    match lost {
        Ok(Some(lost)) => {
            state
                .integrations
                .admin_mail()
                .connection_lost(&lost, public_url)
                .await;
            StatusCode::OK.into_response()
        }
        Ok(None) => StatusCode::OK.into_response(),
        Err(err) => {
            tracing::error!(error = %err, "integrations: Slack event not recorded");
            internal_error()
        }
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
        return internal_error();
    }
    resource_not_found_page()
}

fn internal_error() -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}

fn render(rendered: Result<String, askama::Error>, set_cookie: Option<String>) -> Response {
    render_with(StatusCode::OK, rendered, set_cookie)
}

fn render_with(
    status: StatusCode,
    rendered: Result<String, askama::Error>,
    set_cookie: Option<String>,
) -> Response {
    match rendered {
        Ok(html) => response_with_optional_cookie(status, Html(html).into_response(), set_cookie),
        Err(err) => {
            tracing::error!(error = %err, "integrations: render failed");
            internal_error()
        }
    }
}
