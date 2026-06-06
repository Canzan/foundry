//! US-12 — keyboard-nav server contracts.
//!
//! Three routes that back the alpine.js keyboard-shortcut handlers:
//!
//! - `GET /team/{team}/project/{slug}/issues/new`
//!   The `c` shortcut opens a "new issue" modal. The handler emits a
//!   modal-shaped htmx fragment when `HX-Request: true` is present
//!   (no `<html>` wrapper, just the modal markup with the new-issue
//!   form). Without the header it falls back to a full page so a
//!   no-JS client gets a usable form.
//!
//! - `GET /team/{team}/project/{slug}/search?q=...`
//!   The `/` shortcut focuses a search box; submitting it GETs this
//!   endpoint. The handler returns a list of `<li class="search-result"
//!   data-issue-key="...">` items matching the query. Matching is
//!   case-insensitive substring match against title OR exact match
//!   against the issue key (PREFIX-N).
//!
//! - `GET /keyboard-help`
//!   The `?` shortcut opens a help overlay listing every shortcut.
//!   Intentionally public — the alpine.js bootstrap GETs it once on
//!   page load and caches it, so requiring a session would break
//!   the help on the sign-in page itself. The response is a
//!   `<dl>` with one `<dt data-shortcut="X">` / `<dd>` pair per
//!   shortcut, which is what the acceptance step walks.
//!
//! Authorization on the team-scoped routes mirrors issues.rs / projects.rs:
//! signed-in user must belong to the project's team. Non-members get
//! 403; unknown teams/projects get 404.

use crate::bootstrap::{invalid_page, SessionUser};
use crate::csrf::{build_csrf_cookie, generate_token};
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::header::{HeaderMap, HeaderValue, COOKIE, LOCATION, SET_COOKIE};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use foundry_core::ProjectKey;
use serde::Deserialize;
use tower_sessions::Session;

const HX_REQUEST_HEADER: &str = "hx-request";

/// Every keyboard shortcut shipped in MVP. Update this list when
/// adding new shortcuts; the help overlay enumerates exactly this set
/// (the acceptance test asserts the list is complete).
const SHORTCUTS: &[(&str, &str)] = &[
    ("c", "Create issue"),
    ("/", "Search"),
    ("j", "Next"),
    ("k", "Previous"),
    ("Enter", "Open selected"),
    ("?", "Show this help"),
    ("Esc", "Close modal"),
];

// =========================================================================
// GET /team/:team/project/:slug/issues/new — new-issue modal
// =========================================================================

pub async fn show_new_issue_modal(
    State(state): State<AppState>,
    Path((team_slug, project_slug)): Path<(String, String)>,
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
    let project = match state
        .store
        .find_project_by_slug(team.id, &project_slug)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => return project_not_found_page(&team_slug, &project_slug),
        Err(err) => return internal_error("find_project_by_slug", err),
    };
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let action = format!("/team/{team_slug}/project/{project_slug}/issues");
    let body = if is_htmx(&headers) {
        // Modal fragment: just the modal element + form. No <html>
        // wrapper — htmx swaps this into the page DOM.
        render_modal_fragment(&action, &csrf, &project.name)
    } else {
        // Full page fallback. The form action is identical so a no-JS
        // submit posts to the same endpoint.
        render_modal_full_page(&action, &csrf, &project.name, &team_slug)
    };
    response_with_optional_cookie(StatusCode::OK, Html(body).into_response(), set_cookie)
}

/// Render the BARE htmx modal fragment (US-12 / US-R02). Renders the ONE shared
/// `partials/new_issue_modal.html` partial via the `NewIssueModal` view-model —
/// the SAME partial the full-page fallback includes (the one-partial rule,
/// NFR-WEBB-MAINT-02). Selector-and-substring-identical to the previous `format!`:
/// `data-modal="new-issue"`, `role="dialog"`, `aria-modal`, the identical
/// `action`, the hidden `_csrf` field, `input[name=title][autofocus]`. Stays BARE
/// (no `base.html`) so the htmx swap is not double-wrapped.
fn render_modal_fragment(action: &str, csrf_token: &str, project_name: &str) -> String {
    crate::views::NewIssueModal {
        project_name: project_name.to_string(),
        action: action.to_string(),
        csrf: csrf_token.to_string(),
    }
    .render()
    .expect("new_issue_modal partial renders from a fully-resolved, infallible view-model")
}

/// Render the no-JS new-issue FULL-PAGE fallback (US-R02). Extends `base.html`
/// (which links the vendored `/static` stylesheet + scripts, replacing the prior
/// bare `<head>`) and `{% include %}`s the SAME shared partial as the htmx
/// fragment — the form `action` is identical so a no-script submit posts to the
/// same endpoint.
fn render_modal_full_page(
    action: &str,
    csrf_token: &str,
    project_name: &str,
    team_slug: &str,
) -> String {
    crate::views::NewIssueModalPage {
        project_name: project_name.to_string(),
        action: action.to_string(),
        csrf: csrf_token.to_string(),
        team_slug: team_slug.to_string(),
    }
    .render()
    .expect("new_issue_modal_page.html renders from a fully-resolved, infallible view-model")
}

// =========================================================================
// GET /team/:team/project/:slug/search?q=... — search fragment
// =========================================================================

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    pub q: String,
}

pub async fn search_issues(
    State(state): State<AppState>,
    Path((team_slug, project_slug)): Path<(String, String)>,
    session: Session,
    Query(query): Query<SearchQuery>,
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
    let key_prefix = match ProjectKey::try_new(&project.key_prefix) {
        Ok(k) => k,
        Err(err) => return internal_error("project_key_prefix invalid", err),
    };
    let issues = match state.store.list_issues_by_project(project.id).await {
        Ok(rows) => rows,
        Err(err) => return internal_error("list_issues_by_project", err),
    };
    let matches = filter_matches(&issues, &query.q, key_prefix.as_str());
    Html(render_search_fragment(&matches, key_prefix.as_str())).into_response()
}

/// Filter `issues` by either an exact-key match (`PREFIX-N`) or a
/// case-insensitive substring match against the title. An empty query
/// returns every issue (the alpine.js handler uses the empty result to
/// distinguish "no query" from "query matched nothing").
fn filter_matches<'a>(
    issues: &'a [foundry_store::IssueRow],
    query: &str,
    key_prefix: &str,
) -> Vec<&'a foundry_store::IssueRow> {
    let q = query.trim();
    if q.is_empty() {
        return issues.iter().collect();
    }
    // Exact-key path: `AUTH-2` matches issue number 2 exactly.
    if let Some((prefix, number_str)) = q.rsplit_once('-') {
        if prefix.eq_ignore_ascii_case(key_prefix) {
            if let Ok(n) = number_str.parse::<i32>() {
                return issues.iter().filter(|i| i.number == n).collect();
            }
        }
    }
    // Substring path: title contains the query (case-insensitive).
    let needle = q.to_ascii_lowercase();
    issues
        .iter()
        .filter(|i| i.title.to_ascii_lowercase().contains(&needle))
        .collect()
}

/// Render the BARE htmx search-results fragment (US-K01 / US-12) from the
/// `partials/search_results.html` Askama partial via the `SearchResults`
/// view-model. Selector-and-substring-identical to the previous `format!`:
/// the `ul.search-results` wrapper with one `li.search-result[data-issue-key]`
/// (each carrying its `.key` + `.title` spans) per match, AND the empty
/// `ul.search-results[data-empty="true"]` no-match state. Stays BARE (no
/// `base.html`) so the alpine.js swap is not double-wrapped. Askama
/// auto-escapes the key/title (replacing the manual `html_escape`).
fn render_search_fragment(matches: &[&foundry_store::IssueRow], key_prefix: &str) -> String {
    crate::views::SearchResults {
        items: matches
            .iter()
            .map(|i| crate::views::SearchResultRow {
                key: format!("{key_prefix}-{n}", n = i.number),
                title: i.title.clone(),
            })
            .collect(),
    }
    .render()
    .expect("search_results partial renders from a fully-resolved, infallible view-model")
}

// =========================================================================
// GET /keyboard-help — shortcut help overlay (public)
// =========================================================================

pub async fn show_keyboard_help() -> Response {
    // Render the BARE htmx help overlay (US-K02 / US-12) from the
    // `partials/keyboard_help.html` Askama partial via the `KeyboardHelp`
    // view-model. Selector-and-substring-identical to the previous `format!`:
    // `section.keyboard-help[role="dialog"][aria-label="Keyboard shortcuts"]`
    // with the `header>h2` heading and one `dt[data-shortcut]`+`dd` pair per
    // shortcut. Stays BARE (no `base.html`). Askama auto-escapes key/label
    // (replacing the manual `html_escape`).
    let body = crate::views::KeyboardHelp {
        entries: SHORTCUTS
            .iter()
            .map(|(key, label)| crate::views::ShortcutEntry {
                key: (*key).to_string(),
                label: (*label).to_string(),
            })
            .collect(),
    }
    .render()
    .expect("keyboard_help partial renders from a fully-resolved, infallible view-model");
    Html(body).into_response()
}

// =========================================================================
// internals (mirror issues.rs / projects.rs)
// =========================================================================

async fn signed_in_user(session: &Session) -> Option<SessionUser> {
    session
        .get::<SessionUser>(SESSION_KEY_USER_ID)
        .await
        .ok()
        .flatten()
}

fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get(HX_REQUEST_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
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
            "You are not a member of the {team_slug:?} team and cannot view its keyboard endpoints."
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
