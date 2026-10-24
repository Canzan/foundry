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
//! - `POST …/integrations/routes/test`: exactly one K-7 message to the entered
//!   channel, no row written, six a minute per person (DDR-25, R-5).
//! - `POST …/integrations/routes`: checks the channel with Slack, then saves
//!   the route and its audit record; sends nothing (DDR-25).
//! - `GET …/integrations/routes/{route_id}/history`: every delivery of the
//!   route, newest first, in Foundry's words (US-EIP-05, Manage).

use super::slack::{self, InstallState, SlackGrant};
use super::{Integrations, SlackOffer};
use crate::bootstrap::{resource_not_found_page, SessionUser};
use crate::csrf::{ensure_csrf_cookie, response_with_optional_cookie};
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use foundry_auth::secret_box::SecretSlot;
use foundry_integrations::{
    encode_slack, test_message, ChannelKind, ChannelOutcome, ConnectionState, DeliveryChannel,
    DeliveryKey, DeliveryRecord, Destination, DestinationCatalog, DestinationRef, FinishedDelivery,
    ReasonCode, RouteFacts, RouteStatus,
};
use foundry_services::integrations::{
    IntegrationError, ProjectIntegrations, RouteDraft, RouteHistory, RouteTarget, SlackInstall,
    WorkspaceIntegrations,
};
use foundry_services::{Principal, Services};
use foundry_store::integrations::{ConnectionWriteOutcome, DeliveryHistoryRow, ProjectRoute};
use serde::Deserialize;
use tower_sessions::Session;

pub const WORKSPACE_PATH: &str = "/workspace/integrations";
pub const SETUP_PATH: &str = "/workspace/integrations/slack/setup";
pub const INSTALL_PATH: &str = "/workspace/integrations/slack/install";
pub const RETURN_PATH: &str = slack::RETURN_PATH;

/// K-3.
pub const NOT_CONNECTED: &str = "Slack was not connected. Nothing changed.";
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

#[derive(Template)]
#[template(path = "integrations/workspace.html")]
struct WorkspacePage {
    nav: crate::nav::NavContext,
    notice: Option<String>,
    slack_line: Option<String>,
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
    project_name: String,
    routes_path: String,
    test_path: String,
    notice: Option<String>,
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

/// What the route form submitted.
#[derive(Debug, Clone, Default)]
struct RouteForm {
    destination: String,
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
                _ => {}
            }
        }
        form
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
        Ok(_) => Redirect::to(WORKSPACE_PATH).into_response(),
        Err(why) => {
            tracing::warn!(reason = why, "slack install: not connected");
            workspace_page(&state, &caller, &headers, Some(NOT_CONNECTED)).await
        }
    }
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
        nav: caller.nav(&state, csrf).await,
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
        None,
        StatusCode::OK,
    )
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
                Some(TOO_MANY_TESTS),
                StatusCode::TOO_MANY_REQUESTS,
            )
            .await;
    }
    let Some(bot) = door.bot.as_ref() else {
        return door
            .form(&state, &caller, &headers, &form, None, StatusCode::OK)
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
    door.form(&state, &caller, &headers, &form, Some(&notice), status)
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
                Some(CANNOT_POST),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
            .await;
    };
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
        let bot = target.connection.as_ref().and_then(|connection| {
            let sealed = connection.sealed_credential.clone()?;
            state
                .integrations
                .current()
                .slack_bot(connection.id, sealed)
        });
        Ok(Self {
            target,
            bot,
            routes_path: format!("{}/routes", base_path(team_slug, project_slug)),
        })
    }

    /// The route form for `form`, with `notice`; K-4 without a usable
    /// connection.
    async fn form(
        &self,
        state: &AppState,
        caller: &Caller,
        headers: &HeaderMap,
        form: &RouteForm,
        notice: Option<&str>,
        status: StatusCode,
    ) -> Response {
        let channels = match &self.bot {
            Some(bot) => bot.list_destinations().await.unwrap_or_default(),
            None => Vec::new(),
        };
        let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
        let page = RouteFormPage {
            nav: caller.nav(state, csrf.clone()).await,
            csrf,
            project_name: self.target.project_name.clone(),
            routes_path: self.routes_path.clone(),
            test_path: format!("{}/test", self.routes_path),
            notice: notice.map(str::to_string),
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
                response_with_optional_cookie(status, Html(html).into_response(), set_cookie)
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
    RouteLine {
        id: route.id,
        destination_label: route.destination_label.clone(),
        status: RouteStatus::derive(&facts).words(&route.destination_label),
        history_path: format!("{base}/routes/{}/history", route.id),
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
async fn connect(
    state: &AppState,
    caller: &Caller,
    principal: &Principal,
    code: &str,
) -> Result<ConnectionWriteOutcome, &'static str> {
    let integrations: std::sync::Arc<Integrations> = state.integrations.current();
    let (Some(app), Some(secrets)) = (
        integrations.slack(),
        integrations.settings().secrets.as_ref(),
    ) else {
        return Err("slack_not_offered");
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
        Ok(ConnectionWriteOutcome::StaleRow) => Err("stale_row"),
        Ok(outcome) => Ok(outcome),
        Err(_) => Err("store"),
    }
}

async fn workspace_page(
    state: &AppState,
    caller: &Caller,
    headers: &HeaderMap,
    notice: Option<&str>,
) -> Response {
    let WorkspaceIntegrations { slack, activity } = match caller
        .services
        .workspace_integrations(&caller.principal())
        .await
    {
        Ok(view) => view,
        Err(err) => return refusal(err),
    };
    let offer = state.integrations.current().slack_offer();
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let page = WorkspacePage {
        nav: caller.nav(state, csrf).await,
        notice: notice.map(str::to_string),
        not_set_up: offer == SlackOffer::NotSetUp,
        no_secrets_key: offer == SlackOffer::NoSecretsKey,
        unavailable: offer == SlackOffer::Unavailable && slack.is_some(),
        slack_line: slack,
        activity,
    };
    render(page.render(), set_cookie)
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
