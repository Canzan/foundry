//! A project's route doors: the project page, the route form's preview, test
//! and save, pause, resume, edit, delete and history (K-9, DDR-25, DD-8,
//! US-EIP-05).

use super::matrix_setup::{create_matrix_route, leave_unrouted_room, send_matrix_test};
use super::route_form::{FormAnswer, RouteDoor, RouteForm};
use super::{refusal, render, sign_in, Caller, DELETE_QUESTION, TEST_SENT, TOO_MANY_TESTS};
use crate::bootstrap::resource_not_found_page;
use crate::csrf::ensure_csrf_cookie;
use crate::integrations::chat_kind;
use crate::AppState;
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use foundry_integrations::{
    compose, encode_slack, issue_url, test_message, ChannelKind, ChannelOutcome, ConnectionState,
    ContentLevel, DeliveryChannel, DeliveryKey, DeliveryRecord, Destination, DestinationCatalog,
    DestinationRef, EventKind, EventView, FinishedDelivery, ReasonCode, RouteFacts, RouteStatus,
};
use foundry_services::integrations::{
    ManagedRoute, ProjectIntegrations, RouteAction, RouteDraft, RouteEditDraft, RouteHistory,
    RoutePreview,
};
use foundry_store::integrations::{DeliveryHistoryRow, ProjectRoute, RouteChangeOutcome};
use serde::Deserialize;
use tower_sessions::Session;

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
#[template(path = "integrations/route_history.html")]
struct RouteHistoryPage {
    nav: crate::nav::NavContext,
    project_name: String,
    destination_label: String,
    routes_path: String,
    deliveries: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct NewRouteQuery {
    kind: Option<String>,
    destination: Option<String>,
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
    // The destination check Save makes, with Save's answer: Send test posts
    // only to a channel the picker offers (never a direct message).
    if postable_destination(bot, &form.destination).await.is_none() {
        return door
            .form(&state, &caller, &headers, &form, FormAnswer::cannot_post())
            .await;
    }
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
    let notice = test_notice(outcome, &form.destination_label());
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
            .form(&state, &caller, &headers, &form, FormAnswer::cannot_post())
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
    // Without the route's settings there is no room to leave: nothing is
    // deleted, so no bot is left in a room no route uses.
    let managed = match managed_route(&caller, &team_slug, &project_slug, &route_id).await {
        Ok(managed) => managed,
        Err(response) => return response,
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
pub(super) async fn managed_route(
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

/// What Send test says of `outcome` for the route to `destination` (K-7).
pub(super) fn test_notice(outcome: ChannelOutcome, destination: &str) -> String {
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
pub(super) async fn project_page(
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

/// `/team/{t}/project/{p}/integrations`.
pub(super) fn base_path(team_slug: &str, project_slug: &str) -> String {
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
