//! US-08 — file-issue handler.
//!
//! Route (mounted in `lib::build_router`):
//!
//! - `POST /team/{team_slug}/project/{project_slug}/issues`
//!   → validate title (1-256, trimmed), allocate per-project number,
//!   insert issue + outbox `IssueCreated` event in one transaction,
//!   303 → back to the board (full reload) OR an htmx fragment swap
//!   (when the `HX-Request: true` header is present).
//!
//! Authorization: signed-in user must belong to the project's team.
//! Non-members get 403 Forbidden — mirrors US-07's pattern. The team
//! slug appears in the response body for the 403 case; team slugs are
//! visible to all workspace members in any project URL they construct,
//! so this is not an information leak — it's a confirmation that the
//! team exists, which a workspace member could discover otherwise.
//!
//! Empty / whitespace-only title returns 400 Bad Request with an htmx
//! error fragment. The fragment is rendered inline (no shared helper)
//! — we only have one current error message; abstracting before a
//! second use would be premature.

use crate::bootstrap::{html_escape, invalid_page, SessionUser};
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
use axum::extract::{Form, Path, State};
use axum::http::header::{HeaderMap, HeaderValue, LOCATION};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use foundry_core::ProjectKey;
use foundry_services::{issues as issue_service, Principal, ServiceError};
use serde::Deserialize;
use tower_sessions::Session;

#[derive(Debug, Deserialize)]
pub struct CreateIssueForm {
    pub title: String,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

// ----------------------------------- POST /team/:team/project/:project/issues

pub async fn submit_create(
    State(state): State<AppState>,
    Path((team_slug, project_slug)): Path<(String, String)>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<CreateIssueForm>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return redirect_to("/sign-in");
    };

    // The write orchestration (team -> member -> project -> validate ->
    // insert+outbox) lives in the shared `foundry-services` seam so an API
    // write and this browser write accept/reject identically and store
    // identical bytes (NFR-WEB-API-CON-02). The 404 pages still distinguish
    // team-vs-project, so we resolve those lookups HERE purely to pick the
    // right error PAGE; the service re-runs the SAME authz before the write.
    let principal = Principal::Human {
        user_id: user.user_id,
        workspace_id: user.workspace_id,
    };

    let raw_title = form.title.trim().to_string();
    match issue_service::create_issue(
        &state.store,
        &principal,
        &team_slug,
        &project_slug,
        &form.title,
    )
    .await
    {
        Ok(created) => {
            // Re-wrap the prefix off the returned key for the IssueKey
            // Display path. The service guarantees `created.key` is the
            // canonical `{PREFIX}-{N}`; render the card from the same parts
            // the previous inline path produced (byte-identical).
            let issue_key = parse_issue_key(&created.key, created.number);
            if is_htmx(&headers) {
                return (
                    StatusCode::OK,
                    Html(render_issue_card_with_column_marker(&issue_key, &raw_title)),
                )
                    .into_response();
            }
            redirect_to(&format!("/team/{team_slug}/project/{project_slug}"))
        }
        Err(ServiceError::Validation { .. }) => {
            // Inline error fragment — htmx swap target. Same 400 for both
            // "empty" and "too long" so the front-end has one error contract.
            bad_request_fragment("Title is required")
        }
        Err(ServiceError::Forbidden) => non_member_page(&team_slug),
        Err(ServiceError::NotFound) => {
            // Distinguish team-not-found from project-not-found for the
            // error PAGE wording (the service collapses both to NotFound).
            resolve_not_found_page(&state, &principal, &team_slug, &project_slug).await
        }
        Err(_) => internal_error("create_issue", "service error"),
    }
}

// ------------------------ POST /team/:team/project/:project/issues/:n/state

#[derive(Debug, Deserialize)]
pub struct ChangeStateForm {
    pub state: String,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

pub async fn submit_state_change(
    State(state): State<AppState>,
    Path((team_slug, project_slug, issue_number)): Path<(String, String, i32)>,
    session: Session,
    Form(form): Form<ChangeStateForm>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return redirect_to("/sign-in");
    };

    // Delegate to the shared seam (same authz -> normalize_state -> update+
    // outbox path the API uses). The 404 page still distinguishes
    // team-vs-project, resolved here only for the error PAGE wording.
    let principal = Principal::Human {
        user_id: user.user_id,
        workspace_id: user.workspace_id,
    };
    match issue_service::change_issue_state(
        &state.store,
        &principal,
        &team_slug,
        &project_slug,
        issue_number,
        &form.state,
    )
    .await
    {
        Ok(updated) => {
            let normalized = updated.state;
            (
                StatusCode::OK,
                Html(format!(
                    r#"<span class="state" data-state="{normalized}">{normalized}</span>"#,
                )),
            )
                .into_response()
        }
        Err(ServiceError::Validation { .. }) => bad_request_fragment("Invalid issue state"),
        Err(ServiceError::Forbidden) => non_member_page(&team_slug),
        Err(ServiceError::NotFound) => {
            resolve_not_found_page(&state, &principal, &team_slug, &project_slug).await
        }
        Err(_) => internal_error("change_issue_state", "service error"),
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

/// Reconstruct an `IssueKey` from the service's canonical `{PREFIX}-{N}` key
/// string so the card renderer keeps producing byte-identical markup. Falls
/// back to a manual split if the key is ever malformed (allocator guarantees
/// `number >= 1`, so this never fires in practice).
fn parse_issue_key(key: &str, number: i32) -> foundry_core::IssueKey {
    let prefix = key.rsplit_once('-').map(|(p, _)| p).unwrap_or(key);
    ProjectKey::try_new(prefix)
        .ok()
        .and_then(|p| u32::try_from(number).ok().map(|n| (p, n)))
        .and_then(|(p, n)| foundry_core::IssueKey::try_new(&p, n).ok())
        .expect("service returns a canonical {PREFIX}-{N} key with number >= 1")
}

/// The shared service collapses team-not-found and project-not-found into one
/// `ServiceError::NotFound`. The browser renders DISTINCT 404 pages, so when
/// the service refuses NotFound we re-run the cheap slug lookups purely to pick
/// the correct page wording (byte-identical to the pre-extraction handler).
async fn resolve_not_found_page(
    state: &AppState,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
) -> Response {
    let team = match state
        .store
        .find_team_by_slug(principal.workspace_id(), team_slug)
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return team_not_found_page(team_slug),
        Err(err) => return internal_error("find_team_by_slug", err),
    };
    match state
        .store
        .find_project_by_slug(team.id, project_slug)
        .await
    {
        Ok(Some(_)) => project_not_found_page(team_slug, project_slug),
        Ok(None) => project_not_found_page(team_slug, project_slug),
        Err(err) => internal_error("find_project_by_slug", err),
    }
}

fn redirect_to(location: &str) -> Response {
    let mut hdrs = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(location) {
        hdrs.insert(LOCATION, v);
    }
    (StatusCode::SEE_OTHER, hdrs, "").into_response()
}

fn team_not_found_page(team_slug: &str) -> Response {
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
            "You are not a member of the {team_slug:?} team and cannot file issues in its projects."
        ),
    )
}

fn project_not_found_page(team_slug: &str, project_slug: &str) -> Response {
    invalid_page(
        StatusCode::NOT_FOUND,
        "Project not found",
        &format!("No project with slug {project_slug:?} exists in team {team_slug:?}."),
    )
}

fn internal_error<E: std::fmt::Display>(label: &str, err: E) -> Response {
    tracing::error!(error = %err, "{label} failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}

fn bad_request_fragment(message: &str) -> Response {
    let body = format!(
        r#"<div class="error" data-hx-fragment="issue-create-error">{}</div>"#,
        html_escape(message)
    );
    (StatusCode::BAD_REQUEST, Html(body)).into_response()
}

fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("hx-request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Render a single issue card. The card markup is intentionally simple —
/// the acceptance suite asserts the key + Backlog column wording is
/// present; a designer rounds out the visual treatment in slice 2.
pub(crate) fn render_issue_card(issue_key: &foundry_core::IssueKey, title: &str) -> String {
    format!(
        r#"<article class="issue-card" data-issue-key="{key}"><span class="key">{key}</span> <span class="title">{title}</span></article>"#,
        key = html_escape(&issue_key.to_string()),
        title = html_escape(title),
    )
}

/// htmx response variant: same card wrapped with an out-of-band marker
/// that names the Backlog column. The acceptance test checks the body
/// contains both the issue key and the "Backlog" label.
fn render_issue_card_with_column_marker(issue_key: &foundry_core::IssueKey, title: &str) -> String {
    format!(
        r#"<div hx-swap-oob="beforeend:[data-column='backlog']" data-target-column="Backlog">{card}</div>"#,
        card = render_issue_card(issue_key, title),
    )
}
