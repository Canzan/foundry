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

use crate::bootstrap::{html_escape, invalid_page, SessionUser};
use crate::csrf::{build_csrf_cookie, generate_token, CSRF_FORM_FIELD};
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
use axum::extract::{Form, Path, State};
use axum::http::header::{HeaderMap, HeaderValue, COOKIE, LOCATION, SET_COOKIE};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use foundry_core::{ProjectKey, ProjectKeyError};
use foundry_store::ProjectInsertError;
use serde::Deserialize;
use tower_sessions::Session;

const HX_REQUEST_HEADER: &str = "hx-request";

/// Default state columns rendered on the empty board. Fixed in slice 1
/// per AC ("Default states ... not editable in MVP"). Order matches the
/// US-07 happy-path scenario assertion verbatim.
const DEFAULT_COLUMNS: &[&str] = &["Backlog", "Todo", "In-Progress", "Done"];

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
    let body = render_create_form(&team_slug, &team.name, &csrf, None, "", "");
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
                &team_slug, &team.name, &state, &headers, raw_name, raw_key, message, is_htmx,
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
            &team_slug, &team.name, &state, &headers, raw_name, raw_key, is_htmx,
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
        ),
        Err(ProjectInsertError::Other(err)) => internal_error("insert_project", err),
    }
}

// --------------------------------------------------- GET /team/:team/project/:slug

pub async fn show_board(
    State(state): State<AppState>,
    Path((team_slug, project_slug)): Path<(String, String)>,
    session: Session,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return redirect_to("/sign-in");
    };
    // Resolve the team first so an unknown team renders the 404 page (which
    // deliberately leaks the team_slug — its existence is not secret) and so
    // the board heading can show `team.name`.
    let team = match state
        .store
        .find_team_by_slug(user.workspace_id, &team_slug)
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return team_not_found_page(&team_slug),
        Err(err) => return internal_error("find_team_by_slug", err),
    };
    // The project lookup supplies the page chrome (name + key prefix) and the
    // distinct project-not-found page. Membership authz + the issue rows come
    // from the shared core path (`foundry_services::board::list_board_issues`)
    // so the browser board and the JSON API read the SAME data the SAME way
    // (NFR-WEB-BND-05); the use-case re-validates membership before fetching.
    let project = match state
        .store
        .find_project_by_slug(team.id, &project_slug)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => {
            return invalid_page(
                StatusCode::NOT_FOUND,
                "Project not found",
                &format!("No project with slug {project_slug:?} exists in team {team_slug:?}."),
            )
        }
        Err(err) => return internal_error("find_project_by_slug", err),
    };

    let principal = foundry_services::Principal::Human {
        user_id: user.user_id,
        workspace_id: user.workspace_id,
    };
    let issues = match foundry_services::board::list_board_issues(
        &state.store,
        &principal,
        &team_slug,
        &project_slug,
    )
    .await
    {
        Ok(rows) => rows,
        Err(foundry_services::ServiceError::Forbidden) => return non_member_page(&team_slug),
        Err(err) => return internal_error("list_board_issues", err),
    };
    let key_prefix = match ProjectKey::try_new(&project.key_prefix) {
        Ok(k) => k,
        Err(err) => return internal_error("project_key_prefix invalid", err),
    };
    Html(render_board(&team.name, &project, &issues, &key_prefix)).into_response()
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
    );
    response_with_optional_cookie(StatusCode::CONFLICT, Html(body).into_response(), set_cookie)
}

fn render_create_form(
    team_slug: &str,
    team_name: &str,
    csrf_token: &str,
    error: Option<&str>,
    raw_name: &str,
    raw_key: &str,
) -> String {
    let action = format!("/team/{team_slug}/projects");
    let err_html = error
        .map(|m| format!("<p class=\"error\">{}</p>", html_escape(m)))
        .unwrap_or_default();
    format!(
        r#"<!doctype html>
<html><head><title>New project - {team}</title></head>
<body>
<h1>New project in {team}</h1>
{err_html}
<form method="post" action="{action}">
  <input type="hidden" name="{CSRF_FORM_FIELD}" value="{csrf}">
  <label>Project name <input type="text" name="name" required value="{name}"></label>
  <label>Key prefix <input type="text" name="key_prefix" required value="{key}"></label>
  <button type="submit">Create project</button>
</form>
</body></html>"#,
        team = html_escape(team_name),
        action = html_escape(&action),
        csrf = html_escape(csrf_token),
        name = html_escape(raw_name),
        key = html_escape(raw_key),
    )
}

fn render_error_fragment(message: &str) -> String {
    format!(
        r#"<div class="error" data-hx-fragment="project-create-error">{}</div>"#,
        html_escape(message)
    )
}

fn render_board(
    team_name: &str,
    project: &foundry_store::ProjectRow,
    issues: &[foundry_services::BoardIssue],
    key_prefix: &ProjectKey,
) -> String {
    // Group issues by state. Slice 1: all newly filed issues land in
    // 'backlog'; the other columns stay empty placeholders until drag-
    // and-drop ships in slice 2.
    let columns_html = DEFAULT_COLUMNS
        .iter()
        .map(|col| {
            let state_key = column_label_to_state(col);
            let column_issues: Vec<&foundry_services::BoardIssue> =
                issues.iter().filter(|i| i.state == state_key).collect();
            let body = if column_issues.is_empty() {
                "<p class=\"empty\">No issues yet</p>".to_string()
            } else {
                column_issues
                    .iter()
                    .map(|row| {
                        let key = foundry_core::IssueKey::try_new(key_prefix, row.number as u32)
                            .expect("number >= 1 - allocator guarantees");
                        crate::issues::render_issue_card(&key, &row.title)
                    })
                    .collect::<String>()
            };
            format!(
                "<section class=\"column\" data-column=\"{slug}\"><h3>{name}</h3>{body}</section>",
                slug = html_escape(&col.to_ascii_lowercase().replace('-', "_")),
                name = html_escape(col),
                body = body,
            )
        })
        .collect::<String>();

    // Hidden keyboard-navigation carrier (US-12). The visible board
    // renders most-recent-first (DESC); the alpine.js j/k handler walks
    // this hidden list which is sorted ASCENDING by issue number so
    // pressing `j` moves "to the next-older issue" consistently no
    // matter which column the user is in. `hidden` + `aria-hidden` keeps
    // it out of the rendered layout AND the AT tree.
    let mut sorted_issues: Vec<&foundry_services::BoardIssue> = issues.iter().collect();
    sorted_issues.sort_by_key(|i| i.number);
    let kb_items: String = sorted_issues
        .iter()
        .map(|row| {
            let key = foundry_core::IssueKey::try_new(key_prefix, row.number as u32)
                .expect("number >= 1 - allocator guarantees");
            format!(
                r#"<li data-issue-key="{key}"></li>"#,
                key = html_escape(&key.to_string()),
            )
        })
        .collect();

    format!(
        r#"<!doctype html>
<html><head><title>{name} - {team}</title></head>
<body>
<header><h1>{name}</h1><p class="key">Key prefix: {key}</p></header>
<button type="button" data-action="new-issue">New issue</button>
<div class="board">{columns_html}</div>
<ul id="kb-items" hidden aria-hidden="true">{kb_items}</ul>
</body></html>"#,
        name = html_escape(&project.name),
        team = html_escape(team_name),
        key = html_escape(&project.key_prefix),
    )
}

/// Map a column label ("Backlog", "Todo", "In-Progress", "Done") to the
/// `issues.state` enum value persisted in Postgres.
fn column_label_to_state(label: &str) -> &'static str {
    match label {
        "Backlog" => "backlog",
        "Todo" => "todo",
        "In-Progress" => "in_progress",
        "Done" => "done",
        _ => "",
    }
}

/// Slugify a project name into a URL-safe identifier.
///
/// Rules (kept deliberately simple for slice 1):
/// - lower-case ASCII letters/digits are kept verbatim
/// - whitespace + every other run of non-alphanumeric input collapses
///   to a single hyphen
/// - leading/trailing hyphens are stripped
///
/// Examples:
/// - `"Auth v2"` → `"auth-v2"`
/// - `"  Hello, World!  "` → `"hello-world"`
fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_hyphen = true; // suppress leading hyphen
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            for c in ch.to_lowercase() {
                out.push(c);
            }
            last_was_hyphen = false;
        } else if !last_was_hyphen {
            out.push('-');
            last_was_hyphen = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod slug_tests {
    use super::slugify;

    #[test]
    fn slugifies_common_project_names() {
        assert_eq!(slugify("Auth v2"), "auth-v2");
        assert_eq!(slugify("  Hello, World!  "), "hello-world");
        assert_eq!(slugify("Sandbox"), "sandbox");
        assert_eq!(slugify(""), "");
    }
}
