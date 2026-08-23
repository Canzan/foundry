//! US-07 — project create + view handlers.
//!
//! Routes (mounted in `lib::build_router`):
//!
//! - `GET  /team/{team_slug}/projects/new`  → HTML form (name + key prefix + CSRF)
//! - `POST /team/{team_slug}/projects`      → validate, insert, 303 → board
//! - `GET  /team/{team_slug}/project/{slug}` → minimal empty board view
//!
//! Authorization: requires a signed-in user; the user must be a member
//! of the named team. Non-members get 403 Forbidden with an explanatory
//! page (the team_slug is leaked here intentionally — the team's
//! *existence* is not secret, only its *contents*).
//!
//! Uniqueness:
//! - project name unique WITHIN a team (rendered as an inline 200-OK
//!   form re-render with the error message, htmx-aware)
//! - project key unique WITHIN a workspace (rendered as 409 Conflict
//!   with an htmx-aware error fragment)
//!
//! Invariant I-P3 (project key shape) is enforced by
//! [`foundry_core::ProjectKey::try_new`] — the single domain entry
//! point — and re-checked by the Postgres CHECK constraint. Invalid
//! keys are rejected with 422 Unprocessable Entity (the request is
//! syntactically well-formed but fails business validation).

use crate::bootstrap::{invalid_page, resource_not_found_page, SessionUser};
use crate::csrf::{build_csrf_cookie, generate_token};
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
use askama::Template;
use axum::extract::{Form, Path, Query, State};
use axum::http::header::{
    HeaderMap, HeaderValue, CONTENT_DISPOSITION, CONTENT_TYPE, COOKIE, LOCATION, SET_COOKIE,
};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use foundry_core::{slugify, ProjectKey, ProjectKeyError};
use foundry_store::{ProjectChangeRow, ProjectInsertError, ProjectRow};
use serde::Deserialize;
use std::collections::BTreeMap;
use time::format_description::well_known::Rfc3339;
use tower_sessions::Session;

const HX_REQUEST_HEADER: &str = "hx-request";

// `DEFAULT_COLUMNS` + `column_label_to_state` are DELETED (board-lane-
// management 01-02, D8): columns derive from the project's lane ROWS via
// `board_view`, and `cargo xtask check-arch` fails any build that
// reintroduces a static lane list under crates/foundry-app/src or
// crates/foundry-api/src outside #[cfg(test)].

#[derive(Debug, Deserialize)]
pub struct CreateProjectForm {
    pub name: String,
    pub key_prefix: String,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

// --------------------------------------------------------- GET /team/:team/projects/new

pub async fn show_create_form(
    State(state): State<AppState>,
    Path(team_slug): Path<String>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return redirect_to("/sign-in");
    };
    let team = match state
        .store
        .find_team_by_slug(user.workspace_id, &team_slug)
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return team_not_found_page(&team_slug),
        Err(err) => return internal_error("find_team_by_slug", err),
    };
    match state.store.is_team_member(team.id, user.user_id).await {
        Ok(true) => {}
        Ok(false) => return non_member_page(&team_slug),
        Err(err) => return internal_error("is_team_member", err),
    }
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    // The rail footer reuses the SAME `csrf` token minted for the create form (so the
    // sign-out form is cookie-matched) and the user's REAL instance-admin authority
    // (04-03).
    let is_instance_admin = crate::nav::resolve_is_instance_admin(&state, user.user_id).await;
    let nav = crate::nav::NavContext::home_for(
        &state,
        user.user_id,
        user.workspace_id,
        is_instance_admin,
        csrf.clone(),
    )
    .await;
    let body = render_create_form(&team_slug, &team.name, &csrf, None, "", "", nav);
    response_with_optional_cookie(StatusCode::OK, Html(body).into_response(), set_cookie)
}

// ----------------------------------------------------------- POST /team/:team/projects

pub async fn submit_create(
    State(state): State<AppState>,
    Path(team_slug): Path<String>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<CreateProjectForm>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return redirect_to("/sign-in");
    };
    let team = match state
        .store
        .find_team_by_slug(user.workspace_id, &team_slug)
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return team_not_found_page(&team_slug),
        Err(err) => return internal_error("find_team_by_slug", err),
    };
    match state.store.is_team_member(team.id, user.user_id).await {
        Ok(true) => {}
        Ok(false) => return non_member_page(&team_slug),
        Err(err) => return internal_error("is_team_member", err),
    }

    // Shared sidebar carrier for any inline form re-render below (only the
    // full-page, non-htmx error branches render it; the htmx error fragments and
    // the success redirect ignore it). Each error branch is a returning path, so
    // `nav` moves into exactly one of them. The rail footer reuses the request's
    // existing double-submit CSRF token (this POST already cleared `csrf_middleware`,
    // so a valid `foundry_csrf` cookie is present — the error helpers below reuse the
    // SAME token for the create form) and the user's REAL instance-admin authority
    // (04-03).
    let (csrf, _set_cookie) = ensure_csrf_cookie(&state, &headers);
    let is_instance_admin = crate::nav::resolve_is_instance_admin(&state, user.user_id).await;
    let nav = crate::nav::NavContext::home_for(
        &state,
        user.user_id,
        user.workspace_id,
        is_instance_admin,
        csrf,
    )
    .await;

    let is_htmx = headers
        .get(HX_REQUEST_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let raw_name = form.name.trim();
    let raw_key = form.key_prefix.trim();

    if raw_name.is_empty() {
        return name_error_response(
            &team_slug,
            &team.name,
            &state,
            &headers,
            raw_name,
            raw_key,
            "Project name must not be empty",
            is_htmx,
            nav,
        );
    }

    let slug = slugify(raw_name);

    // Check name uniqueness within team BEFORE key validation. The
    // feature contract treats name duplication as the more actionable
    // error — a user with a colliding key only sees the key error if
    // their name is otherwise valid. Race-free: the unique index on
    // (team_id, slug) still rejects on insert if another writer slips
    // in between this check and the INSERT below.
    match state.store.find_project_by_slug(team.id, &slug).await {
        Ok(Some(_)) => {
            return name_error_response(
                &team_slug,
                &team.name,
                &state,
                &headers,
                raw_name,
                raw_key,
                "Project name must be unique within the team",
                is_htmx,
                nav,
            );
        }
        Ok(None) => {}
        Err(err) => return internal_error("find_project_by_slug", err),
    }

    // I-P3: domain validation. The empty / wrong-length / non-uppercase
    // cases all map to 422 with an inline explanation so the property
    // outline assertions pass.
    let key = match ProjectKey::try_new(raw_key) {
        Ok(k) => k,
        Err(err) => {
            let message = key_error_message(err);
            return key_error_response(
                &team_slug, &team.name, &state, &headers, raw_name, raw_key, message, is_htmx, nav,
            );
        }
    };

    let project_id = uuid::Uuid::now_v7();

    match state
        .store
        .insert_project(
            project_id,
            user.workspace_id,
            team.id,
            raw_name,
            &slug,
            key.as_str(),
        )
        .await
    {
        Ok(()) => {
            let location = format!("/team/{team_slug}/project/{slug}");
            redirect_to(&location)
        }
        Err(ProjectInsertError::DuplicateKey) => duplicate_key_response(
            &team_slug, &team.name, &state, &headers, raw_name, raw_key, is_htmx, nav,
        ),
        Err(ProjectInsertError::DuplicateName) => name_error_response(
            &team_slug,
            &team.name,
            &state,
            &headers,
            raw_name,
            raw_key,
            "Project name must be unique within the team",
            is_htmx,
            nav,
        ),
        Err(ProjectInsertError::Other(err)) => internal_error("insert_project", err),
    }
}

// --------------------------------------------------- GET /team/:team/project/:slug

pub async fn show_board(
    State(state): State<AppState>,
    Path((team_slug, project_slug)): Path<(String, String)>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return redirect_to("/sign-in");
    };
    // Scope EVERY tenant lookup on this path by the RESOLVED acting workspace
    // (ADR-002) — never by a path-parsed id. A team/project that belongs to a
    // FOREIGN workspace resolves to `None` exactly as a never-existed slug does,
    // and BOTH render the SINGLE uniform `resource_not_found_page` (ADR-003): no
    // requested slug is echoed, so a foreign-id reach and a missing-id reach are
    // byte-identical (no enumeration oracle, NFR-MWT-SEC-02). The
    // shared-core membership check below keeps its intra-workspace 403 shape (a
    // member off their OWN workspace's team is a separate, non-cross-tenant
    // concern per ADR-003's boundary clause).
    let acting = user.acting_workspace();
    let team = match state
        .store
        .find_team_by_slug(acting.workspace_id(), &team_slug)
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return resource_not_found_page(),
        Err(err) => return internal_error("find_team_by_slug", err),
    };
    // The project lookup supplies the page chrome (name + key prefix). Membership
    // authz + the issue rows come from the shared core path
    // (`foundry_services::board::list_board_issues`) so the browser board and the
    // JSON API read the SAME data the SAME way (NFR-WEB-BND-05); the use-case
    // re-validates membership before fetching. A missing project under a real
    // (own-workspace) team collapses to the SAME uniform 404 as a foreign one.
    let project = match state
        .store
        .find_project_by_slug(team.id, &project_slug)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return resource_not_found_page(),
        Err(err) => return internal_error("find_project_by_slug", err),
    };

    let principal = foundry_services::Principal::Human {
        user_id: user.user_id,
        workspace_id: user.workspace_id,
    };
    // ONE authz gate for lanes + issues (board-lane-management D8): the board's
    // columns are the project's own lane ROWS (`Store::list_project_lanes` via
    // the shared `board_view` use-case), never a static list.
    let board = match foundry_services::board::board_view(
        &state.store,
        &principal,
        &team_slug,
        &project_slug,
    )
    .await
    {
        Ok(view) => view,
        Err(foundry_services::ServiceError::Forbidden) => return non_member_page(&team_slug),
        Err(err) => return internal_error("board_view", err),
    };
    // Render-failure → clean 500 seam (US-B01 @error,
    // error-and-observability.md §"Render-error handling"). The board view
    // renders to a complete String BEFORE any bytes hit the response, so a
    // render `Err` can never emit a half-page. We map it centrally here: a
    // clean 500 full page, or — for an htmx request — a 500 error fragment so
    // the swap target shows a clean message instead of a torn DOM. The
    // test-only `force_board_render_failure` flag forces the `Err` arm so the
    // mapping is observable without a genuinely-broken template.
    // Board family (`/team/{slug}/project/{slug}`) — mark the Board primary item
    // current (02-02 deterministic active rule). Every non-board authed surface
    // stays `home_for`, so exactly one primary item is ever current.
    // Mint (or reuse) the double-submit CSRF cookie so the rail footer sign-out form
    // carries a cookie-matched token on the board page too (04-03 D1 — the board
    // previously set no CSRF cookie, leaving the sign-out `_csrf` empty). Resolve the
    // user's REAL instance-admin authority for the Instance-admin item (04-03 D2).
    let is_instance_admin = crate::nav::resolve_is_instance_admin(&state, user.user_id).await;
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let nav = crate::nav::NavContext::board_for(
        &state,
        user.user_id,
        user.workspace_id,
        is_instance_admin,
        csrf,
    )
    .await;
    let location = BoardLocation {
        team_name: &team.name,
        team_slug: &team_slug,
        project_slug: &project_slug,
    };
    match render_board(&state, &location, &project, &board, nav) {
        Ok(html) => {
            response_with_optional_cookie(StatusCode::OK, Html(html).into_response(), set_cookie)
        }
        Err(err) => render_500(&headers, "board", err),
    }
}

// --------------------------------------- GET /team/:team/project/:slug/report

/// Query for the report endpoint. `?format=csv` selects the CSV export; any
/// other value (or none) renders the HTML report page. Both branches read the
/// SAME `list_project_changes` events (one source of truth, ADR-002 §3).
#[derive(Debug, Deserialize)]
pub struct ReportQuery {
    #[serde(default)]
    pub format: Option<String>,
}

/// The project change-report (issue-change-history ADR-002 §3, US-04): a table
/// of change events across the project's issues (newest-first) plus status-flow
/// and per-actor summaries, with a `?format=csv` export. Resolution mirrors the
/// board/attachment read paths: team scoped by the acting workspace, membership
/// gate, then the project. Workspace isolation rides on `project_id` — a foreign
/// project/issue never appears (watch-item R9); a foreign/absent team collapses
/// to the SAME uniform `resource_not_found_page` (no enumeration oracle).
pub async fn show_report(
    State(state): State<AppState>,
    Path((team_slug, project_slug)): Path<(String, String)>,
    Query(query): Query<ReportQuery>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return redirect_to("/sign-in");
    };
    let acting = user.acting_workspace();
    let team = match state
        .store
        .find_team_by_slug(acting.workspace_id(), &team_slug)
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return resource_not_found_page(),
        Err(err) => return internal_error("find_team_by_slug", err),
    };
    match state.store.is_team_member(team.id, user.user_id).await {
        Ok(true) => {}
        Ok(false) => return non_member_page(&team_slug),
        Err(err) => return internal_error("is_team_member", err),
    }
    let project = match state
        .store
        .find_project_by_slug(team.id, &project_slug)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return resource_not_found_page(),
        Err(err) => return internal_error("find_project_by_slug", err),
    };
    // One read for BOTH surfaces (HTML + CSV) — newest-first, workspace-scoped.
    let changes = match state.store.list_project_changes(project.id).await {
        Ok(rows) => rows,
        Err(err) => return internal_error("list_project_changes", err),
    };

    if query.format.as_deref() == Some("csv") {
        return csv_response(&project_slug, &changes);
    }

    // The project's LIVE lanes (board-lane-management D8): status labels
    // resolve to `lanes.label`, with `humanize_state` as the fallback for
    // dead/historical slugs (architecture-design.md §6.5). CSV above stays
    // raw slugs (column contract unchanged).
    let lanes = match state.store.list_project_lanes(project.id).await {
        Ok(rows) => rows,
        Err(err) => return internal_error("list_project_lanes", err),
    };

    // Board family (project change report) — Board is the current primary item
    // (02-02 deterministic active rule), same as the board it belongs to. Mint (or
    // reuse) the CSRF cookie for the rail footer sign-out form (04-03 D1 — the report
    // previously set no CSRF cookie) and resolve the user's REAL instance-admin
    // authority for the Instance-admin item (04-03 D2).
    let is_instance_admin = crate::nav::resolve_is_instance_admin(&state, user.user_id).await;
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let nav = crate::nav::NavContext::board_for(
        &state,
        user.user_id,
        user.workspace_id,
        is_instance_admin,
        csrf,
    )
    .await;
    match build_report_page(
        &team.name,
        &project,
        &team_slug,
        &project_slug,
        &changes,
        &lanes,
        nav,
    )
    .render()
    {
        Ok(html) => {
            response_with_optional_cookie(StatusCode::OK, Html(html).into_response(), set_cookie)
        }
        Err(err) => render_500(&headers, "report", err),
    }
}

/// Materialize the [`crate::views::ReportPage`] view-model from the change rows.
/// Data grouping + ordering lives HERE (the template only loops): the event
/// table stays newest-first (store order); the status-flow + per-actor summaries
/// are tallied into `BTreeMap`s for deterministic (label-/name-sorted) output.
fn build_report_page(
    team_name: &str,
    project: &ProjectRow,
    team_slug: &str,
    project_slug: &str,
    changes: &[ProjectChangeRow],
    lanes: &[foundry_store::LaneRow],
    nav: crate::nav::NavContext,
) -> crate::views::ReportPage {
    let events = changes
        .iter()
        .map(|row| crate::views::ReportEvent {
            issue_key: row.issue_key.clone(),
            field: row.field.clone(),
            old_display: display_value(&row.field, row.old_value.as_deref().unwrap_or(""), lanes),
            new_display: display_value(&row.field, &row.new_value, lanes),
            actor: row.actor_name.clone(),
            when: row.created_at.format(&Rfc3339).unwrap_or_default(),
        })
        .collect();

    // Status-flow transition counts: only `status` events carry a state old→new.
    let mut transition_tally: BTreeMap<String, u32> = BTreeMap::new();
    for row in changes.iter().filter(|row| row.field == "status") {
        let old = status_label(lanes, row.old_value.as_deref().unwrap_or(""));
        let new = status_label(lanes, &row.new_value);
        *transition_tally
            .entry(format!("{old} → {new}"))
            .or_insert(0) += 1;
    }
    let transitions = transition_tally
        .into_iter()
        .map(|(label, count)| crate::views::TransitionCount { label, count })
        .collect();

    // Per-actor change counts across every field.
    let mut actor_tally: BTreeMap<String, u32> = BTreeMap::new();
    for row in changes {
        *actor_tally.entry(row.actor_name.clone()).or_insert(0) += 1;
    }
    let actor_counts = actor_tally
        .into_iter()
        .map(|(actor, count)| crate::views::ActorCount { actor, count })
        .collect();

    crate::views::ReportPage {
        team_name: team_name.to_string(),
        project_name: project.name.clone(),
        key_prefix: project.key_prefix.clone(),
        board_url: format!("/team/{team_slug}/project/{project_slug}"),
        csv_url: format!("/team/{team_slug}/project/{project_slug}/report?format=csv"),
        events,
        transitions,
        actor_counts,
        nav,
    }
}

/// Humanize a value for the report table: status slugs resolve to the
/// project's LIVE lane label ([`status_label`]); other fields render verbatim.
fn display_value(field: &str, value: &str, lanes: &[foundry_store::LaneRow]) -> String {
    if field == "status" {
        status_label(lanes, value)
    } else {
        value.to_string()
    }
}

/// Resolve a status slug to its report display label (board-lane-management
/// D8 / architecture-design.md §6.5): the project's LIVE `lanes.label` when
/// the slug is a current lane; `humanize_state` fallback for a dead/
/// historical slug (a lane deleted after the event was recorded), so old
/// report rows never blank.
fn status_label(lanes: &[foundry_store::LaneRow], slug: &str) -> String {
    lanes
        .iter()
        .find(|lane| lane.slug == slug)
        .map(|lane| lane.label.clone())
        .unwrap_or_else(|| crate::comments::humanize_state(slug))
}

/// Serialize the change events as a CSV attachment (issue-change-history ADR-002
/// §3 / watch-item R8), mirroring `attachments.rs`'s `Content-Disposition`
/// idiom. Stable header row `issue,actor,field,old,new,at`; one row per event in
/// the SAME newest-first order the page renders. Every field is CSV-escaped
/// (quote-wrapped with doubled quotes) so commas/quotes/newlines never break the
/// column contract.
fn csv_response(project_slug: &str, changes: &[ProjectChangeRow]) -> Response {
    let mut body = String::from("issue,actor,field,old,new,at\n");
    for row in changes {
        let at = row.created_at.format(&Rfc3339).unwrap_or_default();
        let cells = [
            csv_escape(&row.issue_key),
            csv_escape(&row.actor_name),
            csv_escape(&row.field),
            csv_escape(row.old_value.as_deref().unwrap_or("")),
            csv_escape(&row.new_value),
            csv_escape(&at),
        ];
        body.push_str(&cells.join(","));
        body.push('\n');
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    let disposition = format!("attachment; filename=\"{project_slug}-change-report.csv\"");
    if let Ok(value) = HeaderValue::from_str(&disposition) {
        headers.insert(CONTENT_DISPOSITION, value);
    }
    (StatusCode::OK, headers, body).into_response()
}

/// Escape one CSV field per RFC 4180: wrap in quotes and double any embedded
/// quote when the value contains a comma, quote, CR, or LF; pass through
/// otherwise.
fn csv_escape(field: &str) -> String {
    if field.contains(['"', ',', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

// ----------------------------------------------------------------- internals

async fn signed_in_user(session: &Session) -> Option<SessionUser> {
    session
        .get::<SessionUser>(SESSION_KEY_USER_ID)
        .await
        .ok()
        .flatten()
}

fn ensure_csrf_cookie(state: &AppState, headers: &HeaderMap) -> (String, Option<String>) {
    let existing = headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(crate::csrf::extract_csrf_cookie);
    if let Some(token) = existing {
        return (token, None);
    }
    let token = generate_token();
    let cookie = build_csrf_cookie(&token, state.session_cookie_secure);
    (token, Some(cookie))
}

fn response_with_optional_cookie(
    status: StatusCode,
    body: Response,
    set_cookie: Option<String>,
) -> Response {
    let (mut parts, body) = body.into_parts();
    parts.status = status;
    if let Some(cookie) = set_cookie {
        if let Ok(v) = HeaderValue::from_str(&cookie) {
            parts.headers.insert(SET_COOKIE, v);
        }
    }
    Response::from_parts(parts, body)
}

fn redirect_to(location: &str) -> Response {
    let mut hdrs = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(location) {
        hdrs.insert(LOCATION, v);
    }
    (StatusCode::SEE_OTHER, hdrs, "").into_response()
}

fn team_not_found_page(team_slug: &str) -> Response {
    // Treat unknown team as 404 to avoid leaking membership info.
    invalid_page(
        StatusCode::NOT_FOUND,
        "Team not found",
        &format!("No team with slug {team_slug:?} exists in this workspace."),
    )
}

fn non_member_page(team_slug: &str) -> Response {
    invalid_page(
        StatusCode::FORBIDDEN,
        "Not a team member",
        &format!(
            "You are not a member of the {team_slug:?} team and cannot create projects in it."
        ),
    )
}

fn internal_error<E: std::fmt::Display>(label: &str, err: E) -> Response {
    tracing::error!(error = %err, "{label} failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}

fn key_error_message(err: ProjectKeyError) -> &'static str {
    match err {
        ProjectKeyError::Empty => "Project key must not be empty",
        ProjectKeyError::WrongLength => "Project key must be 2-6 characters",
        ProjectKeyError::InvalidCharacters => {
            "Project key must contain only uppercase A-Z characters"
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn key_error_response(
    team_slug: &str,
    team_name: &str,
    state: &AppState,
    headers: &HeaderMap,
    raw_name: &str,
    raw_key: &str,
    message: &str,
    is_htmx: bool,
    nav: crate::nav::NavContext,
) -> Response {
    if is_htmx {
        let body = render_error_fragment(message);
        return response_with_optional_cookie(
            StatusCode::UNPROCESSABLE_ENTITY,
            Html(body).into_response(),
            None,
        );
    }
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let body = render_create_form(
        team_slug,
        team_name,
        &csrf,
        Some(message),
        raw_name,
        raw_key,
        nav,
    );
    response_with_optional_cookie(
        StatusCode::UNPROCESSABLE_ENTITY,
        Html(body).into_response(),
        set_cookie,
    )
}

#[allow(clippy::too_many_arguments)]
fn name_error_response(
    team_slug: &str,
    team_name: &str,
    state: &AppState,
    headers: &HeaderMap,
    raw_name: &str,
    raw_key: &str,
    message: &str,
    is_htmx: bool,
    nav: crate::nav::NavContext,
) -> Response {
    if is_htmx {
        let body = render_error_fragment(message);
        return response_with_optional_cookie(
            StatusCode::UNPROCESSABLE_ENTITY,
            Html(body).into_response(),
            None,
        );
    }
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let body = render_create_form(
        team_slug,
        team_name,
        &csrf,
        Some(message),
        raw_name,
        raw_key,
        nav,
    );
    response_with_optional_cookie(
        StatusCode::UNPROCESSABLE_ENTITY,
        Html(body).into_response(),
        set_cookie,
    )
}

#[allow(clippy::too_many_arguments)]
fn duplicate_key_response(
    team_slug: &str,
    team_name: &str,
    state: &AppState,
    headers: &HeaderMap,
    raw_name: &str,
    raw_key: &str,
    is_htmx: bool,
    nav: crate::nav::NavContext,
) -> Response {
    let message = "Project key is already in use in this workspace";
    if is_htmx {
        let body = render_error_fragment(message);
        return response_with_optional_cookie(
            StatusCode::CONFLICT,
            Html(body).into_response(),
            None,
        );
    }
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let body = render_create_form(
        team_slug,
        team_name,
        &csrf,
        Some(message),
        raw_name,
        raw_key,
        nav,
    );
    response_with_optional_cookie(StatusCode::CONFLICT, Html(body).into_response(), set_cookie)
}

/// Build the typed project-create view-model and render it via Askama.
///
/// The render contract is selector-and-substring-identical to the previous
/// `format!` markup (design/render-contract.md §US-R01): `method="post"`,
/// `action="/team/{slug}/projects"`, the hidden `_csrf` field, the `name` +
/// `key_prefix` required inputs with their repopulated values, and the optional
/// `.error` paragraph. The page now extends `base.html` (`project_create.html`),
/// which links the vendored content-hashed `/static` stylesheet + htmx script
/// the bare `<head>` `format!` lacked. Askama auto-escapes `{{ … }}` exactly as
/// the previous `html_escape` calls did.
fn render_create_form(
    team_slug: &str,
    team_name: &str,
    csrf_token: &str,
    error: Option<&str>,
    raw_name: &str,
    raw_key: &str,
    nav: crate::nav::NavContext,
) -> String {
    crate::views::ProjectCreatePage {
        team_name: team_name.to_string(),
        action: format!("/team/{team_slug}/projects"),
        csrf: csrf_token.to_string(),
        error: error.map(str::to_string),
        raw_name: raw_name.to_string(),
        raw_key: raw_key.to_string(),
        nav,
    }
    .render()
    .expect("project_create.html renders from a fully-resolved, infallible view-model")
}

/// Build the SHARED bare error fragment for the project-create surface.
///
/// Reproduces the byte-stable `<div class="error"
/// data-hx-fragment="project-create-error">{message}</div>` marker
/// (design/render-contract.md §US-R01) from the shared `error_fragment.html`
/// template, parameterized by the `fragment_marker`. Bare fragment — does NOT
/// extend `base.html` (extending it double-wraps the htmx swap).
fn render_error_fragment(message: &str) -> String {
    crate::views::ErrorFragment {
        fragment_marker: "project-create-error".to_string(),
        message: message.to_string(),
    }
    .render()
    .expect("error_fragment.html renders from a fully-resolved, infallible view-model")
}

/// Where the board lives, as the VALIDATED request path resolved it — the D2
/// seam (ADR-PROJECT-RENAME-001). The two slugs arrive from the request path
/// the handler resolved the project BY (`WHERE slug = $2`), so they are
/// byte-equal to the stored columns; they are NEVER re-derived from the
/// display names travelling beside them (after a name-only rename
/// `slugify(name)` diverges from the URL identity and every card action
/// would 404 — the D2 defect).
struct BoardLocation<'a> {
    team_name: &'a str,
    team_slug: &'a str,
    project_slug: &'a str,
}

/// Build the typed board view-model and render it via Askama.
///
/// The render contract is selector-and-substring-identical to the previous
/// `format!` markup (design/render-contract.md): the template (`board.html`
/// extending `base.html`) reproduces the same columns, `data-column` slugs,
/// `issue-card` partials — and now links the vendored `/static` stylesheet +
/// htmx script via the base layout. Data ordering (column state-filtering)
/// stays HERE in the handler-side builder; the template only loops.
fn render_board(
    state: &AppState,
    location: &BoardLocation<'_>,
    project: &foundry_store::ProjectRow,
    board: &foundry_services::BoardView,
    nav: crate::nav::NavContext,
) -> Result<String, askama::Error> {
    // Test-only render-injection: when the harness has flipped the
    // `force_board_render_failure` flag, short-circuit to the same `Err`
    // shape `Template::render()` returns, so the central
    // render-`Err` → clean-500 mapping is exercised without a genuinely-
    // broken (uncompilable) template. Compiled only under
    // cfg(any(test, feature = "test-support")); release builds skip it.
    #[cfg(any(test, feature = "test-support"))]
    {
        use std::sync::atomic::Ordering;
        if state.force_board_render_failure.load(Ordering::SeqCst) {
            return Err(askama::Error::Custom(
                "forced board render failure (test injection)".into(),
            ));
        }
    }
    let _ = state;
    build_board_page(location, project, board, nav).render()
}

/// Map a template render failure to a CLEAN server error (US-B01 @error,
/// error-and-observability.md §"Render-error handling"). Because the engine
/// returns a complete `String` before the handler builds the response, the
/// client never sees a half-emitted page — only a complete 500 (full-page
/// request) or a small 500 error fragment (htmx request). Logs at `error`
/// with the template name + the formatting error (no user data), mirroring
/// the [`internal_error`] helper.
fn render_500(headers: &HeaderMap, template_name: &str, err: askama::Error) -> Response {
    tracing::error!(error = %err, template = template_name, "template render failed");
    let is_htmx = headers
        .get(HX_REQUEST_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if is_htmx {
        let fragment = r#"<div class="error" data-hx-fragment="render-error">Something went wrong rendering this view. Please retry.</div>"#;
        return (StatusCode::INTERNAL_SERVER_ERROR, Html(fragment)).into_response();
    }
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}

/// Materialize the [`crate::views::BoardPage`] view-model from the neutral
/// service rows. Kept separate from rendering so it is unit-testable without a
/// running server.
fn build_board_page(
    location: &BoardLocation<'_>,
    project: &foundry_store::ProjectRow,
    board: &foundry_services::BoardView,
    nav: crate::nav::NavContext,
) -> crate::views::BoardPage {
    // The columns are the project's own lane rows in board order
    // (board-lane-management D8), materialized by the SHARED
    // `views::board_columns` builder — the same one the lane-delete OOB
    // refresh renders from, so fragment and page stay byte-identical.
    let columns = crate::views::board_columns(location.team_slug, location.project_slug, board);

    crate::views::BoardPage {
        team_name: location.team_name.to_string(),
        project_name: project.name.clone(),
        team_slug: location.team_slug.to_string(),
        project_slug: location.project_slug.to_string(),
        key_prefix: project.key_prefix.clone(),
        columns,
        nav,
    }
}

// `slugify` lives in `foundry_core` (the SINGLE production definition,
// ADR-PROJECT-RENAME-001); its unit + property tests moved with it. A local
// redefinition under crates/foundry-app/src fails `cargo xtask check-arch`.

#[cfg(test)]
mod report_label_tests {
    use super::status_label;

    fn lane(slug: &str, label: &str, position: i32) -> foundry_store::LaneRow {
        foundry_store::LaneRow {
            id: uuid::Uuid::now_v7(),
            project_id: uuid::Uuid::now_v7(),
            slug: slug.to_string(),
            label: label.to_string(),
            position,
        }
    }

    /// board-lane-management D8 / §6.5: a status slug resolves to the
    /// project's LIVE lane label when the lane exists; a dead/historical slug
    /// falls back to `humanize_state` (known slug → its humanized form,
    /// unknown slug → verbatim) so old report rows never blank.
    #[test]
    fn status_labels_resolve_live_lane_label_with_humanize_fallback() {
        // Live lane label DIFFERS from the humanized form on purpose — the
        // live label must win, proving the row (not the fallback) resolved it.
        let lanes = vec![lane("in_progress", "Doing", 0), lane("done", "Done", 1)];
        for (slug, expected) in [
            ("in_progress", "Doing"),     // live lane row label wins
            ("done", "Done"),             // live lane row label
            ("cancelled", "Cancelled"),   // dead-but-known slug → humanize_state
            ("triage_old", "triage_old"), // unknown historical slug → verbatim
        ] {
            assert_eq!(
                status_label(&lanes, slug),
                expected,
                "status label for {slug:?}"
            );
        }
    }
}

#[cfg(test)]
mod board_render_tests {
    use super::{build_board_page, BoardLocation};
    use askama::Template;

    /// The four grandfathered lanes as migration 0015 seeds them — what
    /// `board_view` returns for a pre-existing board. A test-local fixture
    /// (the production static lane expressions are DELETED in 01-02; the
    /// check-arch no-static-lane-list rule exempts `#[cfg(test)]`).
    fn grandfather_lanes() -> Vec<foundry_services::BoardLane> {
        [
            ("backlog", "Backlog"),
            ("todo", "Todo"),
            ("in_progress", "In-Progress"),
            ("done", "Done"),
        ]
        .iter()
        .map(|(slug, label)| foundry_services::BoardLane {
            slug: slug.to_string(),
            label: label.to_string(),
        })
        .collect()
    }

    /// Render the board page through the same builder + `Template::render`
    /// path the handler uses on the success arm (the test-only flag-injection
    /// `Err` arm is covered by the US-B01 @error acceptance scenario, not
    /// here — it needs a live `AppState`).
    fn render_board(
        team_name: &str,
        team_slug: &str,
        project_slug: &str,
        project: &foundry_store::ProjectRow,
        issues: &[foundry_services::BoardIssue],
    ) -> String {
        // The board now renders inside the shared `app_shell.html`, so the
        // builder needs a nav carrier. Board-family active-state lands in 02-02;
        // for this render test the `Home` section + provisional `/` target
        // suffice (the assertions below pin the board content, not the rail).
        let nav = crate::nav::NavContext::for_page(
            team_name.to_string(),
            "Tester".to_string(),
            false,
            String::new(),
            crate::nav::NavSection::Home,
            "/".to_string(),
        );
        let location = BoardLocation {
            team_name,
            team_slug,
            project_slug,
        };
        let board = foundry_services::BoardView {
            lanes: grandfather_lanes(),
            issues: issues.to_vec(),
        };
        build_board_page(&location, project, &board, nav)
            .render()
            .expect("board template renders")
    }

    fn project() -> foundry_store::ProjectRow {
        foundry_store::ProjectRow {
            id: uuid::Uuid::now_v7(),
            name: "Auth v2".to_string(),
            key_prefix: "AUTH".to_string(),
        }
    }

    fn issue(number: i32, title: &str, state: &str) -> foundry_services::BoardIssue {
        foundry_services::BoardIssue {
            key: format!("AUTH-{number}"),
            number,
            title: title.to_string(),
            state: state.to_string(),
        }
    }

    /// A populated board renders, via the base layout, the vendored `/static`
    /// asset references AND every card under its column's `data-column` slug —
    /// the selector-and-substring contract the acceptance suite reads.
    #[test]
    fn populated_board_renders_assets_and_cards_under_their_columns() {
        let issues = vec![
            issue(3, "Revoke on password change", "backlog"),
            issue(2, "Refresh token rotation", "in_progress"),
        ];

        let html = render_board("Backend", "backend", "auth-v2", &project(), &issues);

        // Base-layout vendored asset references, all /static-local. The CSS is
        // cache-busted by a content hash in its committed filename
        // (`/static/css/foundry.<hash>.css`, ADR-B03 / FIX 1) so the blanket
        // `immutable` cache on /static is safe on the hand-authored stylesheet.
        let css_link = r#"link rel="stylesheet" href="/static/css/foundry."#;
        assert!(
            html.contains(css_link) && html.contains(r#".css">"#),
            "board must link the content-hashed /static CSS; html was:\n{html}"
        );
        assert!(html.contains(r#"src="/static/vendor/htmx.min.js"#));
        assert!(!html.contains("http://") && !html.contains("https://"));

        // Each card sits under its column's data-column section.
        let backlog = html.split(r#"data-column="backlog""#).nth(1).unwrap();
        assert!(backlog.contains(r#"data-issue-key="AUTH-3""#));
        let in_progress = html.split(r#"data-column="in_progress""#).nth(1).unwrap();
        assert!(in_progress.contains(r#"data-issue-key="AUTH-2""#));
    }

    /// Each issue lands in EXACTLY its own state column and nowhere else.
    ///
    /// This pins the state→column mapping (`column_label_to_state`) and the
    /// `i.state == state_key` filter in `build_board_page`. With one issue per
    /// state we can assert both directions: every column contains its own key
    /// (kills arm-deletions / constant-replacements that would route an issue
    /// to the wrong — or no — column) AND contains none of the other three
    /// keys (kills the `==` → `!=` filter flip that would fan every issue into
    /// every other column).
    #[test]
    fn each_issue_lands_in_exactly_its_state_column() {
        let issues = vec![
            issue(1, "Triage", "backlog"),
            issue(2, "Planned", "todo"),
            issue(3, "Doing", "in_progress"),
            issue(4, "Shipped", "done"),
        ];

        let html = render_board("Backend", "backend", "auth-v2", &project(), &issues);

        // (column slug, the key that BELONGS in it)
        let placement = [
            ("backlog", "AUTH-1"),
            ("todo", "AUTH-2"),
            ("in_progress", "AUTH-3"),
            ("done", "AUTH-4"),
        ];
        // Slice the HTML into per-column regions at each `data-column` marker so
        // a key found in one region is genuinely under that column, not merely
        // somewhere on the page. Each region is bounded at the NEXT column
        // marker below, which is what makes "not in this column" checkable.
        //
        // This used to split the page on the hidden ASC navigation carrier's id
        // and keep the head, to cut off a list that named every key on the page.
        // ADR-008 retired that carrier, and the slice had to be repointed HERE,
        // in the same change: with nothing left to split on, `.next()` silently
        // returns the WHOLE page, the test goes on passing, and it stops being
        // able to see a card that leaked outside its column. Green while blind
        // is the defect this entire feature exists to end.
        let visible = html.as_str();
        for (slug, expected_key) in placement {
            let marker = format!(r#"data-column="{slug}""#);
            let region = visible
                .split(&marker)
                .nth(1)
                .unwrap_or_else(|| panic!("missing column section {slug}"));
            // The next column section (or end of the visible board) bounds this.
            let region = region.split(r#"data-column=""#).next().unwrap();
            for (_, key) in placement {
                let card = format!(r#"data-issue-key="{key}""#);
                if key == expected_key {
                    assert!(
                        region.contains(&card),
                        "{expected_key} must appear under column {slug}"
                    );
                } else {
                    assert!(
                        !region.contains(&card),
                        "{key} must NOT appear under column {slug}"
                    );
                }
            }
        }
    }

    /// D2 regression pin (ADR-PROJECT-RENAME-001): after a display-name-only
    /// rename, `slugify(name)` diverges from the stored `projects.slug` — the
    /// URL identity the request path resolved by. Every card `edit_url` /
    /// `state_url` and the new-issue dialog URL MUST carry the stored slug,
    /// never a render-time re-derivation from the (renamed) display name.
    /// # bypass: exact-URL pin for the D2 regression — single-example by design
    #[test]
    fn renamed_project_urls_carry_stored_slug_not_name_derivation() {
        let renamed = foundry_store::ProjectRow {
            id: uuid::Uuid::now_v7(),
            name: "Identity Platform".to_string(), // display name after rename
            key_prefix: "AUTH".to_string(),
        };
        let issues = vec![issue(1, "Refresh token rotation", "backlog")];

        // "auth-v2" is the STORED slug the request path resolved the project by.
        let html = render_board("Backend", "backend", "auth-v2", &renamed, &issues);

        assert!(
            html.contains(r#"hx-get="/team/backend/project/auth-v2/issues/1/edit""#),
            "card edit_url must use the stored slug auth-v2; html was:\n{html}"
        );
        assert!(
            html.contains("/team/backend/project/auth-v2/issues/1/state"),
            "card state_url must use the stored slug auth-v2"
        );
        assert!(
            html.contains(r#"hx-get="/team/backend/project/auth-v2/issues/new""#),
            "new-issue dialog URL must use the stored slug auth-v2"
        );
        assert!(
            !html.contains("identity-platform"),
            "no URL may re-derive a slug from the renamed display name"
        );
    }

    /// An empty board renders the grown, inviting empty-state guidance in each
    /// column (US-B01 scenario 2) while still showing all four column labels.
    #[test]
    fn empty_board_renders_inviting_empty_state_guidance() {
        let html = render_board("Backend", "backend", "auth-v2", &project(), &[]);

        for label in ["Backlog", "Todo", "In-Progress", "Done"] {
            assert!(html.contains(label), "missing column label {label}");
        }
        let lower = html.to_ascii_lowercase();
        assert!(
            lower.contains("press") && lower.contains("file the first"),
            "empty board lacks file-the-first-issue guidance"
        );
    }
}
