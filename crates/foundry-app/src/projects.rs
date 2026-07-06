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
    // Render-failure → clean 500 seam (US-B01 @error,
    // error-and-observability.md §"Render-error handling"). The board view
    // renders to a complete String BEFORE any bytes hit the response, so a
    // render `Err` can never emit a half-page. We map it centrally here: a
    // clean 500 full page, or — for an htmx request — a 500 error fragment so
    // the swap target shows a clean message instead of a torn DOM. The
    // test-only `force_board_render_failure` flag forces the `Err` arm so the
    // mapping is observable without a genuinely-broken template.
    match render_board(&state, &team.name, &project, &issues, &key_prefix) {
        Ok(html) => Html(html).into_response(),
        Err(err) => render_500(&headers, "board", err),
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

/// Build the typed project-create view-model and render it via Askama.
///
/// The render contract is selector-and-substring-identical to the previous
/// `format!` markup (design/render-contract.md §US-R01): `method="post"`,
/// `action="/team/{slug}/projects"`, the hidden `_csrf` field, the `name` +
/// `key_prefix` required inputs with their repopulated values, and the optional
/// `.error` paragraph. The page now extends `base.html` (`project_create.html`),
/// which links the vendored content-hashed `/static` stylesheet + htmx/Alpine
/// the bare `<head>` `format!` lacked. Askama auto-escapes `{{ … }}` exactly as
/// the previous `html_escape` calls did.
fn render_create_form(
    team_slug: &str,
    team_name: &str,
    csrf_token: &str,
    error: Option<&str>,
    raw_name: &str,
    raw_key: &str,
) -> String {
    crate::views::ProjectCreatePage {
        team_name: team_name.to_string(),
        action: format!("/team/{team_slug}/projects"),
        csrf: csrf_token.to_string(),
        error: error.map(str::to_string),
        raw_name: raw_name.to_string(),
        raw_key: raw_key.to_string(),
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

/// Build the typed board view-model and render it via Askama.
///
/// The render contract is selector-and-substring-identical to the previous
/// `format!` markup (design/render-contract.md): the template (`board.html`
/// extending `base.html`) reproduces the same columns, `data-column` slugs,
/// `issue-card` partials, and the hidden `#kb-items` ASC carrier — and now
/// links the vendored `/static` stylesheet + htmx/Alpine scripts via the base
/// layout. Data ordering (column state-filtering + the ASC keyboard carrier)
/// stays HERE in the handler-side builder; the template only loops.
fn render_board(
    state: &AppState,
    team_name: &str,
    project: &foundry_store::ProjectRow,
    issues: &[foundry_services::BoardIssue],
    key_prefix: &ProjectKey,
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
    build_board_page(team_name, project, issues, key_prefix).render()
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
    team_name: &str,
    project: &foundry_store::ProjectRow,
    issues: &[foundry_services::BoardIssue],
    key_prefix: &ProjectKey,
) -> crate::views::BoardPage {
    // Group issues by state. Slice 1: all newly filed issues land in
    // 'backlog'; the other columns stay empty placeholders until drag-
    // and-drop ships in slice 2.
    // Slugs for the per-card edit-dialog `hx-get` (issue-edit-dialog, R1). The
    // BoardPage carries these same slugs; compute them once here so the card
    // renderer can build each `…/issues/{n}/edit` URL.
    let team_slug = slugify(team_name);
    let project_slug = slugify(&project.name);
    let columns = DEFAULT_COLUMNS
        .iter()
        .map(|col| {
            let state_key = column_label_to_state(col);
            let cards = issues
                .iter()
                .filter(|i| i.state == state_key)
                .map(|row| issue_card(key_prefix, row, &team_slug, &project_slug))
                .collect();
            crate::views::BoardColumn {
                slug: col.to_ascii_lowercase().replace('-', "_"),
                label: col.to_string(),
                cards,
            }
        })
        .collect();

    // Hidden keyboard-navigation carrier (US-12). The visible board
    // renders most-recent-first (DESC); the alpine.js j/k handler walks
    // this hidden list which is sorted ASCENDING by issue number so
    // pressing `j` moves "to the next-older issue" consistently no
    // matter which column the user is in.
    let mut sorted_issues: Vec<&foundry_services::BoardIssue> = issues.iter().collect();
    sorted_issues.sort_by_key(|i| i.number);
    let kb_items = sorted_issues
        .iter()
        .map(|row| issue_key_string(key_prefix, row))
        .collect();

    crate::views::BoardPage {
        team_name: team_name.to_string(),
        project_name: project.name.clone(),
        team_slug,
        project_slug,
        key_prefix: project.key_prefix.clone(),
        columns,
        kb_items,
    }
}

fn issue_card(
    key_prefix: &ProjectKey,
    row: &foundry_services::BoardIssue,
    team_slug: &str,
    project_slug: &str,
) -> crate::views::IssueCard {
    crate::views::IssueCard {
        key: issue_key_string(key_prefix, row),
        title: row.title.clone(),
        edit_url: format!(
            "/team/{team_slug}/project/{project_slug}/issues/{number}/edit",
            number = row.number
        ),
        state_url: format!(
            "/team/{team_slug}/project/{project_slug}/issues/{number}/state",
            number = row.number
        ),
    }
}

fn issue_key_string(key_prefix: &ProjectKey, row: &foundry_services::BoardIssue) -> String {
    foundry_core::IssueKey::try_new(key_prefix, row.number as u32)
        .expect("number >= 1 - allocator guarantees")
        .to_string()
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

#[cfg(test)]
mod board_render_tests {
    use super::build_board_page;
    use askama::Template;
    use foundry_core::ProjectKey;

    /// Render the board page through the same builder + `Template::render`
    /// path the handler uses on the success arm (the test-only flag-injection
    /// `Err` arm is covered by the US-B01 @error acceptance scenario, not
    /// here — it needs a live `AppState`).
    fn render_board(
        team_name: &str,
        project: &foundry_store::ProjectRow,
        issues: &[foundry_services::BoardIssue],
        key_prefix: &ProjectKey,
    ) -> String {
        build_board_page(team_name, project, issues, key_prefix)
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
    /// asset references AND every card under its column's `data-column` slug,
    /// with the `#kb-items` carrier sorted ASCENDING by issue number — the
    /// selector-and-substring contract the acceptance suite reads.
    #[test]
    fn populated_board_renders_assets_cards_and_ascending_keyboard_carrier() {
        let issues = vec![
            issue(3, "Revoke on password change", "backlog"),
            issue(2, "Refresh token rotation", "in_progress"),
        ];
        let key_prefix = ProjectKey::try_new("AUTH").unwrap();

        let html = render_board("Backend", &project(), &issues, &key_prefix);

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
        assert!(html.contains(r#"src="/static/vendor/alpine.min.js"#));
        assert!(!html.contains("http://") && !html.contains("https://"));

        // Each card sits under its column's data-column section.
        let backlog = html.split(r#"data-column="backlog""#).nth(1).unwrap();
        assert!(backlog.contains(r#"data-issue-key="AUTH-3""#));
        let in_progress = html.split(r#"data-column="in_progress""#).nth(1).unwrap();
        assert!(in_progress.contains(r#"data-issue-key="AUTH-2""#));

        // Hidden carrier: AUTH-2 before AUTH-3 (ASC by number).
        let carrier = html.split(r#"id="kb-items""#).nth(1).unwrap();
        let pos2 = carrier.find("AUTH-2").unwrap();
        let pos3 = carrier.find("AUTH-3").unwrap();
        assert!(pos2 < pos3, "kb-items must list AUTH-2 before AUTH-3");
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
        let key_prefix = ProjectKey::try_new("AUTH").unwrap();

        let html = render_board("Backend", &project(), &issues, &key_prefix);

        // (column slug, the key that BELONGS in it)
        let placement = [
            ("backlog", "AUTH-1"),
            ("todo", "AUTH-2"),
            ("in_progress", "AUTH-3"),
            ("done", "AUTH-4"),
        ];
        // Slice the HTML into per-column regions at each `data-column` marker so
        // a key found in one region is genuinely under that column, not merely
        // somewhere on the page. The visible columns precede the hidden
        // `#kb-items` carrier (which lists EVERY key, ASC), so we truncate each
        // region at the carrier before bounding it at the next column.
        let visible = html.split(r#"id="kb-items""#).next().unwrap();
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

    /// An empty board renders the grown, inviting empty-state guidance in each
    /// column (US-B01 scenario 2) while still showing all four column labels.
    #[test]
    fn empty_board_renders_inviting_empty_state_guidance() {
        let key_prefix = ProjectKey::try_new("AUTH").unwrap();

        let html = render_board("Backend", &project(), &[], &key_prefix);

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
