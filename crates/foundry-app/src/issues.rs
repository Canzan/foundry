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
//! Non-members get the uniform non-enumerable 404 — mirrors US-07's
//! pattern. Step 04-01 converged every authz refusal in this adapter on
//! `resource_not_found_page()`: the prior 403 named the team in its body,
//! and that status/body pair separated "this team exists but is not yours"
//! from "no such team". ADR-003 forbids a refusal from confirming that the
//! resource it refuses exists, so all three reaches are byte-identical.
//!
//! Empty / whitespace-only title returns 400 Bad Request with an htmx
//! error fragment rendered from the SHARED `error_fragment.html` template
//! (US-R03 — reuses the `views::ErrorFragment` view-model introduced in
//! US-R01, parameterized with the `issue-create-error` marker). The
//! state-change chip renders from `partials/state_chip.html`. Both are
//! BARE fragments (no `base.html` wrapper).

use crate::bootstrap::{html_escape, resource_not_found_page, SessionUser};
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
use askama::Template;
use axum::extract::{Form, Path, State};
use axum::http::header::{HeaderMap, HeaderValue, LOCATION};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use foundry_core::ProjectKey;
use foundry_services::{issues as issue_service, Principal, ServiceError};
use serde::Deserialize;
use tower_sessions::Session;

/// The board this issue lives on — the destination every non-htmx success in
/// this module redirects to: issue create (US-R03), issue edit, and the
/// issue-card-delete confirm's `303` (D8, "back to the board, never a re-render
/// of a page for a resource that no longer exists"). Named here rather than
/// spelt inline three times so the four `…/issues/{n}/…` builders below and the
/// redirect target are demonstrably the same board.
fn board_url(team_slug: &str, project_slug: &str) -> String {
    format!("/team/{team_slug}/project/{project_slug}")
}

/// Build the canonical edit-dialog URL for an issue (issue-edit-dialog). The
/// SAME string the board card's `hx-get`, the dialog form `action`/`hx-post`,
/// and the save handler all use — one source of truth for the endpoint.
fn edit_url(team_slug: &str, project_slug: &str, number: i32) -> String {
    format!("/team/{team_slug}/project/{project_slug}/issues/{number}/edit")
}

/// Build the canonical delete URL for an issue (issue-card-delete,
/// ADR-ISSUE-DELETE-002). ONE string for the GET that opens the confirm dialog
/// and the POST that carries it out — the dialog form's `action`/`hx-post` and
/// the Delete controls that open it all read it from here.
fn delete_url(team_slug: &str, project_slug: &str, number: i32) -> String {
    format!("/team/{team_slug}/project/{project_slug}/issues/{number}/delete")
}

/// Build the `POST …/issues/{n}/state` endpoint the DnD drop handler
/// (`board-dnd.js`, issue-status-move slice 02) targets. Rendered onto every
/// card as `data-state-url` so an htmx-appended card (dialog relocation / new
/// issue) is drag-persistable exactly like a board-rendered one.
fn state_url(team_slug: &str, project_slug: &str, number: i32) -> String {
    format!("/team/{team_slug}/project/{project_slug}/issues/{number}/state")
}

/// Build the full issue-detail page URL (`…/issues/{n}`) — the "open full page"
/// link rendered inside the quick-edit dialog (issue-change-history ADR-002 §1).
/// The board card itself no longer navigates, so the dialog is the route to the
/// full view.
fn detail_url(team_slug: &str, project_slug: &str, number: i32) -> String {
    format!("/team/{team_slug}/project/{project_slug}/issues/{number}")
}

#[derive(Debug, Deserialize)]
pub struct CreateIssueForm {
    pub title: String,
    /// Optional markdown description (new-issue-dialog-description). Absent on a
    /// title-only submit, so it defaults to the empty string — mirrors
    /// `EditIssueForm::description`.
    #[serde(default)]
    pub description: String,
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
        &form.description,
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
            redirect_to(&board_url(&team_slug, &project_slug))
        }
        // The over-long description carries its own copy so the create dialog
        // shows the specific reason; every OTHER validation (empty/oversized
        // title) keeps the shipped byte-identical "Title is required" fragment.
        Err(ServiceError::Validation { code, message }) if code == "description_too_long" => {
            bad_request_fragment(&message)
        }
        Err(ServiceError::Validation { .. }) => {
            // Inline error fragment — htmx swap target. Empty/oversized title
            // renders the shipped one-contract fragment.
            bad_request_fragment("Title is required")
        }
        Err(ServiceError::Forbidden) => resource_not_found_page(),
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
    /// The `data-issue-key` of the card immediately ABOVE the dropped card in
    /// the target column (card-ranking-within-status, ADR-002). Absent/empty ⇒
    /// drop at the TOP of the column. The DnD drop handler always sends it; the
    /// no-JS status path omits it.
    #[serde(default)]
    pub after: Option<String>,
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
        form.after.as_deref(),
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
        Err(ServiceError::Forbidden) => resource_not_found_page(),
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
        Err(ServiceError::Forbidden) => return resource_not_found_page(),
        // A foreign/missing issue is byte-identical to a never-existed one
        // (ADR-003): the requested key is NOT echoed, so there is no
        // enumeration oracle.
        Err(ServiceError::NotFound) => return resource_not_found_page(),
        Err(_) => return internal_error("edit_issue_form", "service error"),
    };

    let (csrf, set_cookie) = crate::csrf::ensure_csrf_cookie(&state, &headers);
    let action = edit_url(&team_slug, &project_slug, issue_number);
    let detail = detail_url(&team_slug, &project_slug, issue_number);
    let delete = delete_url(&team_slug, &project_slug, issue_number);
    let body = crate::views::IssueEditModal {
        action,
        csrf,
        key: view.key,
        title: view.title,
        description: view.description_md,
        selected_state: view.state,
        lanes: view.lanes,
        detail_url: detail,
        delete_url: delete,
    }
    .render()
    .expect("issue_edit_modal partial renders from a fully-resolved, infallible view-model");
    crate::csrf::response_with_optional_cookie(
        StatusCode::OK,
        Html(body).into_response(),
        set_cookie,
    )
}

// -------------------- GET+POST /team/:t/project/:p/issues/:n/delete --------
//
// The shape both handlers honour (feature-delta DDD-5/DDD-6,
// `adr-issue-delete-002-get-post-confirm-not-delete-verb.md`):
//
//  - GET branches on `is_htmx`: htmx → the bare `delete_issue_modal.html`
//    fragment for `#modal-root`; direct navigation → `delete_issue_modal_page`,
//    which extends `base.html` and `{% include %}`s that SAME partial, so the
//    fragment and the page can never disagree.
//  - POST branches on `is_htmx` for RENDERING ONLY: htmx → cleared `#modal-root`
//    plus the out-of-band `#board-columns` refresh; otherwise `303` to the
//    board. Both call the SAME `issue_service::delete_issue`.
//  - Refusals on BOTH verbs collapse into the SINGLE uniform
//    `resource_not_found_page`: `Forbidden` and `NotFound` alike (DDD-9). No
//    key and no team slug is echoed in any branch.

/// issue-card-delete — the confirm dialog (GET). Resolves the issue and its two
/// advisory child counts through the `resolve_member_project`-gated service
/// read, then renders ONE of the two carriers of the SAME partial (DDD-6):
/// htmx gets the bare `delete_issue_modal.html` fragment for `#modal-root`, a
/// direct navigation gets `delete_issue_modal_page.html`, which extends
/// `base.html` and includes that same partial — so the popup and the page can
/// never disagree. Mints/reuses the CSRF cookie exactly as `show_edit_form`
/// does, so the confirm POST (under `csrf_middleware`) has a matching
/// double-submit token.
pub async fn show_delete_form(
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
    let view = match issue_service::delete_issue_dialog(
        &state.store,
        &principal,
        &team_slug,
        &project_slug,
        issue_number,
    )
    .await
    {
        Ok(v) => v,
        // DDD-9 collapses BOTH refusals into ONE uniform non-enumerable answer.
        // A 403 that names the team IS the enumeration oracle ADR-003 closes — it
        // confirms the team exists and separates "not yours" from "never existed".
        // A non-member reach, a foreign reach and a never-existed reach must be
        // byte-identical, so all three render the SAME page and echo neither key
        // nor slug.
        Err(ServiceError::Forbidden | ServiceError::NotFound) => return resource_not_found_page(),
        Err(_) => return internal_error("delete_issue_dialog", "service error"),
    };

    let (csrf, set_cookie) = crate::csrf::ensure_csrf_cookie(&state, &headers);
    let action = delete_url(&team_slug, &project_slug, issue_number);
    let rendered = "delete confirm renders from a fully-resolved, infallible view-model";
    let body = if is_htmx(&headers) {
        crate::views::IssueDeleteModal {
            action,
            csrf,
            key: view.key,
            comment_count: view.comment_count,
            attachment_count: view.attachment_count,
        }
        .render()
        .expect(rendered)
    } else {
        crate::views::IssueDeleteModalPage {
            action,
            csrf,
            key: view.key,
            comment_count: view.comment_count,
            attachment_count: view.attachment_count,
        }
        .render()
        .expect(rendered)
    };
    crate::csrf::response_with_optional_cookie(
        StatusCode::OK,
        Html(body).into_response(),
        set_cookie,
    )
}

/// issue-card-delete — the confirm (POST). ONE delete seam shared by both
/// surfaces (DDD-5, `adr-issue-delete-002`): the full page and the board popup
/// both call the SAME `issue_service::delete_issue`, so there is nothing
/// between them that could drift. The branch is on RENDERING only.
///
/// Success on a direct navigation is a `303` back to the board (D8) — never a
/// re-render of the issue page, which would be a page for a resource that no
/// longer exists. Success on an htmx confirm is the house OOB idiom (D7,
/// identical in shape to lane delete): the refreshed `#board-columns` inside an
/// `hx-swap-oob="true"` envelope, and NOTHING outside it — htmx lifts the
/// envelope out and applies the EMPTY remainder to the primary `#modal-root`
/// target, which is what closes the dialog. There is no "empty modal" template.
///
/// Both arms call the SAME `issue_service::delete_issue` above. One use case,
/// two renderings (AC-2.9) — there is deliberately no second write path.
///
/// Membership, not authorship, is the gate (D2): a card filed by another member
/// deletes cleanly, so there is deliberately no author check. The dialog's
/// advisory counts do not gate either (D13) — a comment filed between the GET
/// and this POST simply goes with the issue via the schema's `ON DELETE
/// CASCADE`, which is correct, not a race.
///
/// CSRF is the layer-wide `csrf_middleware`'s job by route registration alone;
/// no per-route token work happens here. Refusals mirror `show_delete_form`
/// exactly (DDD-9, ADR-003): non-member, foreign, absent and already-deleted all
/// render the SINGLE uniform `resource_not_found_page`, echoing neither the
/// requested key nor the team slug.
pub async fn submit_delete(
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
    match issue_service::delete_issue(
        &state.store,
        &principal,
        &team_slug,
        &project_slug,
        issue_number,
    )
    .await
    {
        Ok(()) if is_htmx(&headers) => {
            crate::views::board_columns_oob_response(&state, &principal, &team_slug, &project_slug)
                .await
        }
        Ok(()) => redirect_to(&board_url(&team_slug, &project_slug)),
        // The same uniform refusal `show_delete_form` gives, on the same two
        // errors, so the two verbs cannot drift apart (DDD-9). `Ok(0)` from the
        // double-submit race arrives here as `NotFound` and lands on this arm.
        Err(ServiceError::Forbidden | ServiceError::NotFound) => resource_not_found_page(),
        Err(_) => internal_error("delete_issue", "service error"),
    }
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

    // A submitted status only counts as a real move when it DIFFERS from the
    // stored slug. The issue-edit-dialog path posts no `state`, so this stays
    // `None` and the in-place card replace is preserved. The dialog's options
    // are the project's OWN lane slugs (board-lane-management D8), so the
    // value is compared verbatim; VALIDATION of the lane (alias fold + per-
    // project membership) happens in the ONE `change_issue_state` seam below —
    // never a duplicate per-surface check here. We read the current state
    // through the SAME authz-gated read the dialog pre-fill uses, so a
    // foreign/missing issue is still refused non-enumerably (ADR-003).
    let submitted_state = form.state.trim().to_string();
    let relocate_to = if submitted_state.is_empty() {
        None
    } else {
        match issue_service::edit_issue_form(
            &state.store,
            &principal,
            &team_slug,
            &project_slug,
            issue_number,
        )
        .await
        {
            Ok(view) if view.state != submitted_state => Some(submitted_state),
            Ok(_) => None,
            Err(ServiceError::Forbidden) => return resource_not_found_page(),
            Err(ServiceError::NotFound) => return resource_not_found_page(),
            Err(_) => return internal_error("edit_issue_form", "service error"),
        }
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
            let board = board_url(&team_slug, &project_slug);

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
            // outbox → SSE, ODD-4). Reuses `change_issue_state` — the ONE
            // seam that validates the lane per-project (D8); no new write.
            let landed_state = match issue_service::change_issue_state(
                &state.store,
                &principal,
                &team_slug,
                &project_slug,
                issue_number,
                &new_state,
                // The edit-dialog status change carries no board slot; the card
                // lands at the top of the target column.
                None,
            )
            .await
            {
                // The CANONICAL slug the seam accepted — the target column's
                // `data-column` selector for the relocation render.
                Ok(moved) => moved.state,
                Err(ServiceError::Validation { .. }) => {
                    return bad_request_fragment("Invalid issue state")
                }
                Err(ServiceError::Forbidden) => return resource_not_found_page(),
                Err(ServiceError::NotFound) => return resource_not_found_page(),
                Err(_) => return internal_error("change_issue_state", "service error"),
            };

            if is_htmx(&headers) {
                (
                    StatusCode::OK,
                    Html(render_card_relocation(
                        &issue_key,
                        &updated.title,
                        &edit,
                        &state_post,
                        &landed_state,
                    )),
                )
                    .into_response()
            } else {
                redirect_to(&board)
            }
        }
        // The over-long description carries its own copy so the edit dialog shows
        // the specific reason (mirrors `submit_create`); every OTHER validation
        // (empty/oversized title) keeps the shipped "Title is required" fragment.
        Err(ServiceError::Validation { code, message }) if code == "description_too_long" => {
            bad_request_fragment(&message)
        }
        Err(ServiceError::Validation { .. }) => bad_request_fragment("Title is required"),
        Err(ServiceError::Forbidden) => resource_not_found_page(),
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
/// converged on the SAME uniform 404 in step 04-01, so every authz refusal in
/// this adapter — non-member, foreign, never-existed — is now byte-identical
/// and none of them confirms that the resource they refuse exists.
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
    // The WHOLE card opens the quick-edit dialog (the `hx-get`); the key is inert
    // text. The link to the full issue-detail page (issue-change-history ADR-002
    // §1) now lives INSIDE the dialog (`issue_edit_modal.html`), not on the card,
    // so a click anywhere on the card is a dialog-open and never a full-page nav.
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
