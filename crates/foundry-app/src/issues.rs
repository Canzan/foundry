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
use foundry_store::IssueInsertError;
use serde::Deserialize;
use tower_sessions::Session;

const TITLE_MAX_LEN: usize = 256;

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

    // Team lookup + membership check mirror US-07's project-create
    // handler. Order: team -> team_member -> project -> insert.
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
    let project = match state
        .store
        .find_project_by_slug(team.id, &project_slug)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return project_not_found_page(&team_slug, &project_slug),
        Err(err) => return internal_error("find_project_by_slug", err),
    };

    let raw_title = form.title.trim();
    if raw_title.is_empty() || raw_title.chars().count() > TITLE_MAX_LEN {
        // Inline error fragment — htmx swap target. Same 400 for both
        // "empty" and "too long" so the front-end has one error contract.
        return bad_request_fragment("Title is required");
    }

    // Domain construction. The prefix is already validated by ProjectKey
    // on project-create; we re-wrap for the IssueKey Display impl below.
    let key_prefix = match ProjectKey::try_new(&project.key_prefix) {
        Ok(k) => k,
        Err(err) => return internal_error("project_key_prefix invalid", err),
    };

    let issue_id = uuid::Uuid::now_v7();
    let number = match state
        .store
        .insert_issue_with_outbox(
            issue_id,
            user.workspace_id,
            project.id,
            key_prefix.as_str(),
            user.user_id,
            raw_title,
        )
        .await
    {
        Ok(n) => n,
        Err(IssueInsertError::ProjectNotFound) => {
            // The project lookup above succeeded but the row was deleted
            // between then and the allocation. From the client's view
            // the resource is gone, so 404 is correct (vs. 500). The
            // only realistic trigger is a concurrent admin delete.
            return project_not_found_page(&team_slug, &project_slug);
        }
        Err(IssueInsertError::Store(err)) => {
            return internal_error("insert_issue_with_outbox", err)
        }
    };

    let issue_key = foundry_core::IssueKey::try_new(&key_prefix, number as u32)
        .expect("number >= 1 — allocator guarantees");

    if is_htmx(&headers) {
        // htmx fragment swap — render the new issue card alone so the
        // client can `hx-swap-oob` into the Backlog column.
        return (
            StatusCode::OK,
            Html(render_issue_card_with_column_marker(&issue_key, raw_title)),
        )
            .into_response();
    }

    // Full-page redirect. Slice 1 polish: the front-end uses htmx by
    // default; this branch covers no-JS submission.
    redirect_to(&format!("/team/{team_slug}/project/{project_slug}"))
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
    let project = match state
        .store
        .find_project_by_slug(team.id, &project_slug)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return project_not_found_page(&team_slug, &project_slug),
        Err(err) => return internal_error("find_project_by_slug", err),
    };
    let new_state = form.state.trim();
    let normalized = match normalize_state(new_state) {
        Some(s) => s,
        None => return bad_request_fragment("Invalid issue state"),
    };
    let key_prefix = match ProjectKey::try_new(&project.key_prefix) {
        Ok(k) => k,
        Err(err) => return internal_error("project_key_prefix invalid", err),
    };

    match state
        .store
        .update_issue_state_with_outbox(key_prefix.as_str(), issue_number, normalized, user.user_id)
        .await
    {
        Ok(Some(())) => (
            StatusCode::OK,
            Html(format!(
                r#"<span class="state" data-state="{normalized}">{normalized}</span>"#,
            )),
        )
            .into_response(),
        Ok(None) => project_not_found_page(&team_slug, &project_slug),
        Err(IssueInsertError::ProjectNotFound) => project_not_found_page(&team_slug, &project_slug),
        Err(IssueInsertError::Store(err)) => internal_error("update_issue_state_with_outbox", err),
    }
}

/// Map the incoming form value (which may be the human label used in
/// feature files like `"in-progress"`) to the schema-enforced enum
/// stored in `issues.state`.
fn normalize_state(input: &str) -> Option<&'static str> {
    match input.trim().to_ascii_lowercase().as_str() {
        "backlog" => Some("backlog"),
        "todo" => Some("todo"),
        "in-progress" | "in_progress" => Some("in_progress"),
        "done" => Some("done"),
        "cancelled" | "canceled" => Some("cancelled"),
        _ => None,
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
