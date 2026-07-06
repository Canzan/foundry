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
//! error fragment rendered from the SHARED `error_fragment.html` template
//! (US-R03 — reuses the `views::ErrorFragment` view-model introduced in
//! US-R01, parameterized with the `issue-create-error` marker). The
//! state-change chip renders from `partials/state_chip.html`. Both are
//! BARE fragments (no `base.html` wrapper).

use crate::bootstrap::{html_escape, invalid_page, resource_not_found_page, SessionUser};
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
use askama::Template;
use axum::extract::{Form, Path, State};
use axum::http::header::{HeaderMap, HeaderValue, COOKIE, LOCATION, SET_COOKIE};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use foundry_core::ProjectKey;
use foundry_services::{issues as issue_service, Principal, ServiceError};
use serde::Deserialize;
use tower_sessions::Session;

/// Build the canonical edit-dialog URL for an issue (issue-edit-dialog). The
/// SAME string the board card's `hx-get`, the dialog form `action`/`hx-post`,
/// and the save handler all use — one source of truth for the endpoint.
fn edit_url(team_slug: &str, project_slug: &str, number: i32) -> String {
    format!("/team/{team_slug}/project/{project_slug}/issues/{number}/edit")
}

/// Build the `POST …/issues/{n}/state` endpoint the DnD drop handler
/// (`board-dnd.js`, issue-status-move slice 02) targets. Rendered onto every
/// card as `data-state-url` so an htmx-appended card (dialog relocation / new
/// issue) is drag-persistable exactly like a board-rendered one.
fn state_url(team_slug: &str, project_slug: &str, number: i32) -> String {
    format!("/team/{team_slug}/project/{project_slug}/issues/{number}/state")
}

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
                let edit = edit_url(&team_slug, &project_slug, created.number);
                let state = state_url(&team_slug, &project_slug, created.number);
                return (
                    StatusCode::OK,
                    Html(render_issue_card_with_column_marker(
                        &issue_key, &raw_title, &edit, &state,
                    )),
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
            // Cross-tenant / missing-resource refusal (ADR-003): the service
            // scoped the team/project lookup by the RESOLVED acting workspace
            // (`principal.workspace_id()`), so a write aimed at a FOREIGN project
            // resolves to `NotFound` exactly as a never-existed one does — and
            // BOTH render the SINGLE uniform `resource_not_found_page` (no echoed
            // slug). A foreign-project write and a never-existed-project write
            // are byte-identical (same status, same body), so the refusal leaks
            // nothing about the foreign project's existence and the write never
            // lands in the foreign workspace (NFR-MWT-SEC-02). The intra-workspace
            // `Forbidden` 403 above is unchanged (ADR-003 boundary clause).
            resource_not_found_page()
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
            // BARE state chip from `partials/state_chip.html` — htmx swaps it
            // into the live board DOM (does NOT extend base.html). Render
            // contract is byte-stable to the prior inline `format!`:
            // `<span class="state" data-state="{normalized}">{normalized}</span>`.
            (StatusCode::OK, Html(render_state_chip(&updated.state))).into_response()
        }
        Err(ServiceError::Validation { .. }) => bad_request_fragment("Invalid issue state"),
        Err(ServiceError::Forbidden) => non_member_page(&team_slug),
        Err(ServiceError::NotFound) => {
            resolve_not_found_page(&state, &principal, &team_slug, &project_slug).await
        }
        Err(_) => internal_error("change_issue_state", "service error"),
    }
}

// ------------------------ GET /team/:team/project/:project/issues/:n/edit

/// issue-edit-dialog — render the pre-filled edit dialog (ADR-001/002). Resolves
/// the issue's current title + description through the `resolve_member_project`-
/// gated service read (so a FOREIGN issue is refused with the uniform
/// non-enumerable `resource_not_found_page`, ADR-003), then renders the
/// `IssueEditModal` fragment htmx swaps into `#modal-root`. Mints/reuses the CSRF
/// cookie exactly as the new-issue modal does, so the save POST (under
/// `csrf_middleware`) has a matching double-submit token.
pub async fn show_edit_form(
    State(state): State<AppState>,
    Path((team_slug, project_slug, issue_number)): Path<(String, String, i32)>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return redirect_to("/sign-in");
    };
    let principal = Principal::Human {
        user_id: user.user_id,
        workspace_id: user.workspace_id,
    };
    let view = match issue_service::edit_issue_form(
        &state.store,
        &principal,
        &team_slug,
        &project_slug,
        issue_number,
    )
    .await
    {
        Ok(v) => v,
        Err(ServiceError::Forbidden) => return non_member_page(&team_slug),
        // A foreign/missing issue is byte-identical to a never-existed one
        // (ADR-003): the requested key is NOT echoed, so there is no
        // enumeration oracle.
        Err(ServiceError::NotFound) => return resource_not_found_page(),
        Err(_) => return internal_error("edit_issue_form", "service error"),
    };

    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let action = edit_url(&team_slug, &project_slug, issue_number);
    let body = crate::views::IssueEditModal {
        action,
        csrf,
        key: view.key,
        title: view.title,
        description: view.description_md,
        selected_state: view.state,
    }
    .render()
    .expect("issue_edit_modal partial renders from a fully-resolved, infallible view-model");
    response_with_optional_cookie(StatusCode::OK, Html(body).into_response(), set_cookie)
}

// ------------------------ POST /team/:team/project/:project/issues/:n/edit

#[derive(Debug, Deserialize)]
pub struct EditIssueForm {
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// The submitted status slug (issue-status-move slice 01). Absent on the
    /// issue-edit-dialog path (title/description-only edits) — an empty/absent
    /// value normalizes to `None`, keeping the in-place card replace.
    #[serde(default)]
    pub state: String,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

/// issue-edit-dialog — save the edited title + description (ADR-001/002). Under
/// `csrf_middleware`. On success + htmx: `200` carrying the updated card as an
/// OOB `outerHTML` swap keyed on `data-issue-key` (the board card updates in
/// place; the empty primary body clears `#modal-root` so the dialog closes). On
/// success + no-JS: `303` → the board. Empty/oversized title → the
/// "Title is required" fragment (mirrors `submit_create`). Foreign/missing →
/// the uniform non-enumerable not-found page (ADR-003).
pub async fn submit_edit(
    State(state): State<AppState>,
    Path((team_slug, project_slug, issue_number)): Path<(String, String, i32)>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<EditIssueForm>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return redirect_to("/sign-in");
    };
    let principal = Principal::Human {
        user_id: user.user_id,
        workspace_id: user.workspace_id,
    };

    // A submitted status only counts as a real move when it NORMALIZES to a
    // valid slug that DIFFERS from the stored one. The issue-edit-dialog path
    // posts no `state`, so this stays `None` and the in-place card replace is
    // preserved. We read the current state through the SAME authz-gated read the
    // dialog pre-fill uses, so a foreign/missing issue is still refused
    // non-enumerably (ADR-003) exactly as before.
    let submitted_state = issue_service::normalize_state(&form.state);
    let relocate_to = if let Some(new_state) = submitted_state {
        match issue_service::edit_issue_form(
            &state.store,
            &principal,
            &team_slug,
            &project_slug,
            issue_number,
        )
        .await
        {
            Ok(view) if view.state != new_state => Some(new_state),
            Ok(_) => None,
            Err(ServiceError::Forbidden) => return non_member_page(&team_slug),
            Err(ServiceError::NotFound) => return resource_not_found_page(),
            Err(_) => return internal_error("edit_issue_form", "service error"),
        }
    } else {
        None
    };

    match issue_service::edit_issue_details(
        &state.store,
        &principal,
        &team_slug,
        &project_slug,
        issue_number,
        &form.title,
        &form.description,
    )
    .await
    {
        Ok(updated) => {
            let issue_key = parse_issue_key(&updated.key, updated.number);
            let edit = edit_url(&team_slug, &project_slug, updated.number);
            let state_post = state_url(&team_slug, &project_slug, updated.number);
            let board = format!("/team/{team_slug}/project/{project_slug}");

            let Some(new_state) = relocate_to else {
                // No status change — the shipped in-place card replace / 303.
                return if is_htmx(&headers) {
                    (
                        StatusCode::OK,
                        Html(render_issue_card_oob_replace(
                            &issue_key,
                            &updated.title,
                            &edit,
                            &state_post,
                        )),
                    )
                        .into_response()
                } else {
                    redirect_to(&board)
                };
            };

            // Persist the state change through the SHIPPED path (fires the
            // outbox → SSE, ODD-4). Reuses `change_issue_state`; no new write.
            match issue_service::change_issue_state(
                &state.store,
                &principal,
                &team_slug,
                &project_slug,
                issue_number,
                new_state,
            )
            .await
            {
                Ok(_) => {}
                Err(ServiceError::Validation { .. }) => {
                    return bad_request_fragment("Invalid issue state")
                }
                Err(ServiceError::Forbidden) => return non_member_page(&team_slug),
                Err(ServiceError::NotFound) => return resource_not_found_page(),
                Err(_) => return internal_error("change_issue_state", "service error"),
            }

            if is_htmx(&headers) {
                (
                    StatusCode::OK,
                    Html(render_card_relocation(
                        &issue_key,
                        &updated.title,
                        &edit,
                        &state_post,
                        new_state,
                    )),
                )
                    .into_response()
            } else {
                redirect_to(&board)
            }
        }
        Err(ServiceError::Validation { .. }) => bad_request_fragment("Title is required"),
        Err(ServiceError::Forbidden) => non_member_page(&team_slug),
        Err(ServiceError::NotFound) => resource_not_found_page(),
        Err(_) => internal_error("edit_issue_details", "service error"),
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
/// Cross-tenant / missing-resource refusal page for the state-change write
/// (ADR-003 / NFR-MWT-SEC-02). The shared service already scoped the
/// team/project lookup by the RESOLVED acting workspace, so a write aimed at a
/// FOREIGN team/project resolves to `NotFound` exactly as a never-existed one
/// does. BOTH must render the SINGLE uniform `resource_not_found_page()` — no
/// echoed team/project slug, no team-vs-project body-shape difference — so a
/// foreign reach is byte-identical to a never-existed reach and leaks nothing
/// about the foreign resource's existence. The intra-workspace `Forbidden`
/// (`non_member_page`, 403) keeps its shipped shape and is handled in the caller
/// (ADR-003 boundary clause); a cross-tenant reach 404s at the team layer above
/// and never reaches it.
async fn resolve_not_found_page(
    _state: &AppState,
    _principal: &Principal,
    _team_slug: &str,
    _project_slug: &str,
) -> Response {
    resource_not_found_page()
}

fn redirect_to(location: &str) -> Response {
    let mut hdrs = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(location) {
        hdrs.insert(LOCATION, v);
    }
    (StatusCode::SEE_OTHER, hdrs, "").into_response()
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

fn internal_error<E: std::fmt::Display>(label: &str, err: E) -> Response {
    tracing::error!(error = %err, "{label} failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}

/// Render the issue-create error from the SHARED `error_fragment.html` template
/// (reused across US-R01 / US-R03 / US-R05), parameterized with the byte-stable
/// `issue-create-error` marker. Bare fragment — does NOT extend `base.html`
/// (extending it double-wraps the htmx swap, NFR-WEBB-COMPAT-02). Byte-identical
/// to the prior inline `format!`: `<div class="error"
/// data-hx-fragment="issue-create-error">{escaped message}</div>` (Askama
/// auto-escapes `{{ message }}`, matching the previous `html_escape`).
fn bad_request_fragment(message: &str) -> Response {
    let body = crate::views::ErrorFragment {
        fragment_marker: "issue-create-error".to_string(),
        message: message.to_string(),
    }
    .render()
    .expect("error_fragment.html renders from a fully-resolved, infallible view-model");
    (StatusCode::BAD_REQUEST, Html(body)).into_response()
}

/// Render the state-change chip from `partials/state_chip.html`. Byte-identical
/// to the prior inline `format!`: `<span class="state"
/// data-state="{normalized}">{normalized}</span>` (Askama auto-escapes
/// `{{ normalized }}`; the underscore-normalized value carries no
/// markup-significant characters, so the bytes are unchanged).
fn render_state_chip(normalized: &str) -> String {
    crate::views::StateChip {
        normalized: normalized.to_string(),
    }
    .render()
    .expect("state_chip.html renders from a fully-resolved, infallible view-model")
}

fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("hx-request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Reuse the request's CSRF cookie if present, else mint one — mirrors
/// `keyboard::ensure_csrf_cookie`. The edit-dialog GET renders the token into
/// the form's hidden `_csrf` field; the save POST (under `csrf_middleware`)
/// double-submits it against this same cookie.
fn ensure_csrf_cookie(state: &AppState, headers: &HeaderMap) -> (String, Option<String>) {
    let existing = headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(crate::csrf::extract_csrf_cookie);
    if let Some(token) = existing {
        return (token, None);
    }
    let token = crate::csrf::generate_token();
    let cookie = crate::csrf::build_csrf_cookie(&token, state.session_cookie_secure);
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

/// Render a single issue card. Selector-and-substring-identical to the board
/// `partials/issue_card.html` (the render contract): both carry the
/// `data-issue-key` marker plus the issue-edit-dialog `hx-get`/`hx-target`/
/// `hx-swap` wiring (R1) so the card opens the pre-filled dialog. `edit_url` is
/// the `…/issues/{n}/edit` endpoint.
pub(crate) fn render_issue_card(
    issue_key: &foundry_core::IssueKey,
    title: &str,
    edit_url: &str,
    state_url: &str,
) -> String {
    format!(
        r##"<article class="issue-card" id="issue-{key}" data-issue-key="{key}" draggable="true" data-state-url="{state}" hx-get="{edit}" hx-target="#modal-root" hx-swap="innerHTML" style="cursor:pointer"><span class="key">{key}</span> <span class="title">{title}</span></article>"##,
        key = html_escape(&issue_key.to_string()),
        state = html_escape(state_url),
        edit = html_escape(edit_url),
        title = html_escape(title),
    )
}

/// htmx response variant: same card wrapped with an out-of-band marker
/// that names the Backlog column. The acceptance test checks the body
/// contains both the issue key and the "Backlog" label.
fn render_issue_card_with_column_marker(
    issue_key: &foundry_core::IssueKey,
    title: &str,
    edit_url: &str,
    state_url: &str,
) -> String {
    format!(
        r#"<div hx-swap-oob="beforeend:[data-column='backlog']" data-target-column="Backlog">{card}</div>"#,
        card = render_issue_card(issue_key, title, edit_url, state_url),
    )
}

/// issue-status-move save response (htmx): a state change MOVES the card between
/// columns via TWO out-of-band ops (ODD-2 / ADR-001 server-driven relocation):
/// (a) DELETE the old card — an element whose stable `id="issue-{key}"` matches
/// the board card, carrying `hx-swap-oob="delete"`; (b) APPEND a fresh card to
/// the target column via `hx-swap-oob="beforeend:[data-column='{new_state}']"`
/// (the same append envelope board-new-issue uses). The primary body is
/// otherwise empty, so `#modal-root` clears and the dialog closes. `new_state`
/// is the normalized slug, which is also the target column's `data-column`.
fn render_card_relocation(
    issue_key: &foundry_core::IssueKey,
    title: &str,
    edit_url: &str,
    state_url: &str,
    new_state: &str,
) -> String {
    let key = html_escape(&issue_key.to_string());
    format!(
        r#"<div id="issue-{key}" hx-swap-oob="delete"></div><div hx-swap-oob="beforeend:[data-column='{state}']">{card}</div>"#,
        key = key,
        state = html_escape(new_state),
        card = render_issue_card(issue_key, title, edit_url, state_url),
    )
}

/// issue-edit-dialog save response (htmx): the SAME card, carrying an
/// `hx-swap-oob="outerHTML:[data-issue-key='{key}']"` directive so htmx replaces
/// the live board card in place (ODD-2 / ADR-001). The primary response body is
/// otherwise empty, so `#modal-root` clears and the dialog closes. The replaced
/// card keeps its own `hx-get` (R2 — it stays clickable after a save).
fn render_issue_card_oob_replace(
    issue_key: &foundry_core::IssueKey,
    title: &str,
    edit_url: &str,
    state_url: &str,
) -> String {
    let key = html_escape(&issue_key.to_string());
    // Inject the OOB directive onto the base card so there is ONE source of
    // truth for the card body (the base renderer). Keyed on data-issue-key via
    // the selector form htmx's board-new-issue create swap already uses.
    render_issue_card(issue_key, title, edit_url, state_url).replacen(
        r#"<article class="issue-card""#,
        &format!(r#"<article class="issue-card" hx-swap-oob="outerHTML:[data-issue-key='{key}']""#),
        1,
    )
}
