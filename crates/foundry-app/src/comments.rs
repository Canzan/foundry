//! US-10 — comment handlers.
//!
//! Routes (mounted in `lib::build_router`):
//!
//! - `GET  /team/{team}/project/{project}/issues/{issue_number}`
//!   → issue-detail page with the rendered comment thread.
//!   Authorisation: signed-in team-member only; non-members get 403.
//!
//! - `POST /team/{team}/project/{project}/issues/{issue_number}/comments`
//!   → render the markdown through [`foundry_core::render_comment_markdown`],
//!   insert the comment + outbox `CommentAdded` event in one transaction
//!   (the Postgres trigger fans out via pg_notify), then either return a
//!   200 OK htmx fragment of the new comment card when the request carries
//!   `HX-Request: true`, or 303 → back to the issue page otherwise.
//!   Empty / whitespace-only bodies return 400 with an htmx fragment.
//!   Non-members get 403 mirroring the GET path.
//!
//! Authorship: the actor's `users.email_display` is captured at insert
//! time and ridden through the outbox payload so SSE subscribers can
//! render the author without a JOIN at fan-out time (wave-decisions.md).

use crate::attachments::humanize_size;
use crate::bootstrap::{html_escape, invalid_page, resource_not_found_page, SessionUser};
use crate::session::SESSION_KEY_USER_ID;
use crate::views;
use crate::AppState;
use askama::Template;
use axum::extract::{Form, Path, State};
use axum::http::header::{HeaderMap, HeaderValue, LOCATION};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use foundry_core::render_comment_markdown;
use foundry_services::{comments as comment_service, Principal, ServiceError};
use foundry_store::{AttachmentSummary, CommentRow, IssueChangeRow};
use serde::Deserialize;
use tower_sessions::Session;

#[derive(Debug, Deserialize)]
pub struct CreateCommentForm {
    pub body: String,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

/// Slice-5 PATCH form: edit a comment by re-submitting `body_markdown`.
/// CSRF rides in the `_csrf` field per ADR-009 (PATCH carries a urlencoded
/// body just like POST). DELETE uses the HX-CSRF header because htmx
/// `hx-delete` ships an empty body.
#[derive(Debug, Deserialize)]
pub struct EditCommentForm {
    pub body_markdown: String,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

// ------------------------------------- GET /team/:team/project/:project/issues/:n

pub async fn show_issue(
    State(state): State<AppState>,
    Path((team_slug, project_slug, issue_number)): Path<(String, String, i32)>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return redirect_to("/sign-in");
    };
    // Scope by the RESOLVED acting workspace (ADR-002), never a path-parsed id.
    // A FOREIGN team/project/issue resolves to `None` exactly as a never-existed
    // one does, and BOTH render the SINGLE uniform `resource_not_found_page`
    // (ADR-003): the requested slug/number is NOT echoed, so a foreign-id reach
    // and a missing-id reach are byte-identical — no enumeration oracle that
    // could confirm the foreign issue exists (NFR-MWT-SEC-02). The membership
    // gate below keeps its intra-workspace 403 (a member off their OWN
    // workspace's team — a separate, non-cross-tenant concern, ADR-003 boundary).
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
    let issue = match state
        .store
        .find_issue_by_team_project_number(team.id, &project_slug, issue_number)
        .await
    {
        Ok(Some(i)) => i,
        Ok(None) => return resource_not_found_page(),
        Err(err) => return internal_error("find_issue_by_team_project_number", err),
    };
    let comments = match state.store.list_comments_for_issue(issue.issue_id).await {
        Ok(rows) => rows,
        Err(err) => return internal_error("list_comments_for_issue", err),
    };
    // US-11 — list attachments on the issue page. Empty for issues
    // with no uploads; the renderer emits a `.attachments-empty` block
    // in that case so scrapers can distinguish "no attachments" from
    // "render failed".
    let attachments = match state.store.list_attachments_for_issue(issue.issue_id).await {
        Ok(rows) => rows,
        Err(err) => return internal_error("list_attachments_for_issue", err),
    };
    // Slice-5: determine the actor's admin status once per page render
    // so `render_comment_card` can conditionally emit the Delete
    // affordance for the author OR an admin (per ADR-006/007 server-
    // side gating). Edit is author-only.
    let actor_is_admin = match state
        .store
        .is_workspace_admin(user.workspace_id, user.user_id)
        .await
    {
        Ok(b) => b,
        Err(err) => return internal_error("is_workspace_admin", err),
    };

    // issue-change-history ADR-002 §1 — the change timeline below the issue
    // header, NEWEST-first. Read through the SAME authz-gated issue resolution
    // above (a foreign/absent issue already 404'd non-enumerably), so the
    // timeline never leaks a change for an issue the caller may not see. Empty
    // for an unchanged issue (genesis = start empty, UC-1).
    let changes = match state.store.list_issue_changes(issue.issue_id).await {
        Ok(rows) => rows,
        Err(err) => return internal_error("list_issue_changes", err),
    };

    // Mint (or reuse) the CSRF double-submit cookie the SAME way every other
    // write-form GET does (issues.rs edit dialog, keyboard.rs new-issue modal).
    // `render_issue_page` renders the token into the add-comment form's hidden
    // `_csrf` field so the urlencoded POST clears `csrf_middleware`
    // (comment-add-csrf 01-01).
    let (csrf, set_cookie) = crate::csrf::ensure_csrf_cookie(&state, &headers);
    let key = format!("{}-{}", issue.project_key_prefix, issue_number);
    let nav = crate::nav::NavContext::home_for(&state, user.user_id, user.workspace_id).await;
    let html = render_issue_page(
        &team_slug,
        &project_slug,
        &key,
        &comments,
        &attachments,
        &changes,
        user.user_id,
        actor_is_admin,
        &csrf,
        nav,
    );
    crate::csrf::response_with_optional_cookie(
        StatusCode::OK,
        Html(html).into_response(),
        set_cookie,
    )
}

// ----------------------- POST /team/:team/project/:project/issues/:n/comments

pub async fn submit_comment(
    State(state): State<AppState>,
    Path((team_slug, project_slug, issue_number)): Path<(String, String, i32)>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<CreateCommentForm>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return redirect_to("/sign-in");
    };

    // Delegate to the shared seam: membership authz -> validate -> render
    // markdown in core -> insert+outbox. The SAME path the API uses, so an
    // API comment and a browser comment accept/reject identically and store
    // identical bytes (NFR-WEB-API-CON-02).
    let principal = Principal::Human {
        user_id: user.user_id,
        workspace_id: user.workspace_id,
    };
    let view = match comment_service::create_comment(
        &state.store,
        &principal,
        &team_slug,
        &project_slug,
        issue_number,
        &form.body,
    )
    .await
    {
        Ok(v) => v,
        Err(ServiceError::Validation { message, .. }) => return bad_request_fragment(&message),
        Err(ServiceError::Forbidden) => return non_member_page(&team_slug),
        Err(ServiceError::NotFound) => {
            return resolve_comment_not_found_page(
                &state,
                &principal,
                &team_slug,
                &project_slug,
                issue_number,
            )
            .await
        }
        Err(_) => return internal_error("create_comment", "service error"),
    };

    if is_htmx(&headers) {
        // htmx fragment: just the new comment card. The list page can
        // hx-swap-oob "beforeend" into `[data-comment-list]`.
        let row = CommentRow {
            id: view.id,
            author_id: user.user_id,
            author_email: view.author_email,
            body_html: view.body_html,
            created_at: time::OffsetDateTime::now_utc(),
            edited: view.edited,
        };
        // Newly-posted card renders as if the actor is the author (Edit
        // visible, Delete visible). We pass actor.user_id explicitly so
        // the render function can compute the same predicate the list
        // path uses. Admin status is irrelevant for the author's own
        // newly-posted comment.
        return (
            StatusCode::OK,
            Html(render_comment_card_oob(
                &row,
                &team_slug,
                &project_slug,
                &issue_number.to_string(),
                Some(user.user_id),
                false,
            )),
        )
            .into_response();
    }

    // Plain redirect back to the issue page.
    redirect_to(&format!(
        "/team/{team_slug}/project/{project_slug}/issues/{issue_number}"
    ))
}

// =====================================================================
// Slice-5 — US-10 deferred ACs (edit + delete + admin moderation).
// =====================================================================

// ---------- GET /…/issues/:n/comments/:id/edit — edit-form fragment ----

pub async fn show_edit_form(
    State(state): State<AppState>,
    Path((team_slug, project_slug, issue_number, comment_id)): Path<(
        String,
        String,
        i32,
        uuid::Uuid,
    )>,
    session: Session,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return redirect_to("/sign-in");
    };
    // Authorise via the team-membership chain (same as the issue-page
    // GET). Non-members never see anyone else's edit form.
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
    // 404-vs-410-vs-403 dispatch per ADR-008.
    let comment = match state
        .store
        .find_comment_by_id(user.workspace_id, comment_id)
        .await
    {
        Ok(Some(c)) => c,
        Ok(None) => return not_found_fragment("Comment not found"),
        Err(err) => return internal_error("find_comment_by_id", err),
    };
    if comment.deleted {
        return gone_fragment();
    }
    // ADR-006: edit is author-only. GET edit-form enforces the same
    // 403 the PATCH endpoint does (probe-the-substrate-lie that authz
    // is uniform across HTTP verbs).
    if comment.author_id != user.user_id {
        return forbidden_fragment("You may only edit your own comments.");
    }
    let url = format!(
        "/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments/{comment_id}"
    );
    let cancel_url = format!(
        "/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments/{comment_id}"
    );
    let body = views::CommentEditForm {
        id: comment.id.to_string(),
        patch_url: url,
        cancel_url,
        body_markdown: comment.body_markdown.clone(),
    }
    .render()
    .expect("comment edit form render (infallible String buffer)");
    (StatusCode::OK, Html(body)).into_response()
}

// ---------- PATCH /…/issues/:n/comments/:id — edit submit -------------

pub async fn submit_edit_comment(
    State(state): State<AppState>,
    Path((team_slug, project_slug, issue_number, comment_id)): Path<(
        String,
        String,
        i32,
        uuid::Uuid,
    )>,
    session: Session,
    Form(form): Form<EditCommentForm>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return redirect_to("/sign-in");
    };

    // Delegate to the shared seam: membership authz -> validate -> author-only
    // authz -> render markdown in core -> update+outbox. The SAME path the API
    // uses (NFR-WEB-API-CON-02).
    let principal = Principal::Human {
        user_id: user.user_id,
        workspace_id: user.workspace_id,
    };
    let view = match comment_service::edit_comment(
        &state.store,
        &principal,
        &team_slug,
        &project_slug,
        issue_number,
        comment_id,
        &form.body_markdown,
    )
    .await
    {
        Ok(v) => v,
        Err(ServiceError::Validation { message, .. }) => return bad_request_fragment(&message),
        Err(ServiceError::NotFound) => return not_found_fragment("Comment not found"),
        Err(ServiceError::Gone) => return gone_fragment(),
        Err(ServiceError::Forbidden) => {
            // The service collapses "not a team member" and "not the author"
            // into Forbidden; the browser renders DISTINCT fragments. Re-check
            // membership to pick the byte-identical one.
            return forbidden_edit_page(&state, &principal, &team_slug).await;
        }
        Err(_) => return internal_error("edit_comment", "service error"),
    };

    // Determine whether the actor is an admin so the re-rendered card
    // carries the same affordances the full-page render would.
    let actor_is_admin = state
        .store
        .is_workspace_admin(user.workspace_id, user.user_id)
        .await
        .unwrap_or(false);
    let row = CommentRow {
        id: view.id,
        author_id: user.user_id,
        author_email: view.author_email,
        body_html: view.body_html,
        created_at: time::OffsetDateTime::now_utc(),
        edited: view.edited,
    };
    let number_str = issue_number.to_string();
    let body = render_comment_card(
        &row,
        &team_slug,
        &project_slug,
        &number_str,
        Some(user.user_id),
        actor_is_admin,
    );
    (StatusCode::OK, Html(body)).into_response()
}

// ---------- DELETE /…/issues/:n/comments/:id — soft-delete -----------

pub async fn submit_delete_comment(
    State(state): State<AppState>,
    Path((team_slug, project_slug, issue_number, comment_id)): Path<(
        String,
        String,
        i32,
        uuid::Uuid,
    )>,
    session: Session,
) -> Response {
    let _ = (project_slug, issue_number);
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
    // ADR-007 admin-delete contract: workspace admins can moderate
    // comments in teams they don't belong to. Resolve the actor's
    // admin status up-front so we can gate the team-membership check
    // accordingly. (Author-delete still needs team membership in
    // theory, but if the actor authored the comment they were a team
    // member when they posted it; the team-membership check is
    // defense-in-depth for the non-admin non-author path.)
    let is_admin = match state
        .store
        .is_workspace_admin(user.workspace_id, user.user_id)
        .await
    {
        Ok(b) => b,
        Err(err) => return internal_error("is_workspace_admin", err),
    };
    if !is_admin {
        match state.store.is_team_member(team.id, user.user_id).await {
            Ok(true) => {}
            Ok(false) => return non_member_page(&team_slug),
            Err(err) => return internal_error("is_team_member", err),
        }
    }
    let comment = match state
        .store
        .find_comment_by_id(user.workspace_id, comment_id)
        .await
    {
        Ok(Some(c)) => c,
        Ok(None) => return not_found_fragment("Comment not found"),
        Err(err) => return internal_error("find_comment_by_id", err),
    };
    if comment.deleted {
        return gone_fragment();
    }
    // ADR-006/007: delete is author OR workspace admin.
    let is_author = comment.author_id == user.user_id;
    if !is_author && !is_admin {
        return forbidden_fragment("You may only delete your own comments.");
    }
    match state
        .store
        .soft_delete_comment_with_outbox(user.workspace_id, comment_id, user.user_id)
        .await
    {
        Ok(true) => {}
        Ok(false) => return gone_fragment(),
        Err(err) => return internal_error("soft_delete_comment_with_outbox", err),
    }
    // 200 OK with a small "deleted" htmx fragment that htmx can swap
    // into the card's outerHTML to remove the card from the DOM.
    (
        StatusCode::OK,
        Html(format!(
            r#"<div class="comment-deleted" data-comment-id="{comment_id}" data-hx-fragment="comment-deleted"></div>"#,
        )),
    )
        .into_response()
}

// ---------- GET /…/issues/:n/comments/:id — single-card (cancel) -----

pub async fn show_single_comment(
    State(state): State<AppState>,
    Path((team_slug, project_slug, issue_number, comment_id)): Path<(
        String,
        String,
        i32,
        uuid::Uuid,
    )>,
    session: Session,
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
    let comment = match state
        .store
        .find_comment_by_id(user.workspace_id, comment_id)
        .await
    {
        Ok(Some(c)) => c,
        Ok(None) => return not_found_fragment("Comment not found"),
        Err(err) => return internal_error("find_comment_by_id", err),
    };
    if comment.deleted {
        return gone_fragment();
    }
    // Re-render the live card. We need author_email + body_html for
    // CommentRow — fetch them via the existing helpers.
    let author_email = match state.store.find_user_email_by_id(comment.author_id).await {
        Ok(Some(e)) => e,
        Ok(None) => "<deleted>".to_string(),
        Err(err) => return internal_error("find_user_email_by_id", err),
    };
    // We do NOT have updated_at directly from the lookup row; if we
    // need the "edited" indicator on the cancel re-render, we'd add a
    // second query. For the cancel scenario (scenario 10) the comment
    // was never edited, so `edited = false` is correct. A future cancel-
    // after-edit case would need an extra column on CommentLookupRow.
    let html = render_comment_markdown(&comment.body_markdown);
    let row = CommentRow {
        id: comment.id,
        author_id: comment.author_id,
        author_email,
        body_html: html.into_inner(),
        created_at: time::OffsetDateTime::now_utc(),
        edited: false,
    };
    let actor_is_admin = state
        .store
        .is_workspace_admin(user.workspace_id, user.user_id)
        .await
        .unwrap_or(false);
    let number_str = issue_number.to_string();
    let card = render_comment_card(
        &row,
        &team_slug,
        &project_slug,
        &number_str,
        Some(user.user_id),
        actor_is_admin,
    );
    (StatusCode::OK, Html(card)).into_response()
}

// ----------------------------------------------------------------- internals

async fn signed_in_user(session: &Session) -> Option<SessionUser> {
    session
        .get::<SessionUser>(SESSION_KEY_USER_ID)
        .await
        .ok()
        .flatten()
}

/// The shared `create_comment` service collapses team-not-found and
/// issue-not-found into one `ServiceError::NotFound`. The browser renders
/// DISTINCT 404 pages, so on NotFound we re-run the cheap lookups purely to
/// pick the correct page wording (byte-identical to the pre-extraction handler).
/// Cross-tenant / missing-resource refusal page for the comment write
/// (ADR-003 / NFR-MWT-SEC-02). The shared `create_comment` service already
/// scoped the team/issue lookup by the RESOLVED acting workspace, so a comment
/// aimed at a FOREIGN issue resolves to `NotFound` exactly as a never-existed one
/// does. BOTH must render the SINGLE uniform `resource_not_found_page()` — no
/// echoed team/project slug, no team-vs-issue body-shape difference — so a
/// foreign reach is byte-identical to a never-existed reach and leaks nothing
/// about the foreign issue's existence. The intra-workspace `Forbidden`
/// (`non_member_page`, 403) keeps its shipped shape and is handled in the caller
/// (ADR-003 boundary clause); a cross-tenant reach 404s at the team layer above
/// and never reaches it.
async fn resolve_comment_not_found_page(
    _state: &AppState,
    _principal: &Principal,
    _team_slug: &str,
    _project_slug: &str,
    _issue_number: i32,
) -> Response {
    resource_not_found_page()
}

/// The shared `edit_comment` service collapses "not a team member" and "not the
/// author" into `ServiceError::Forbidden`. The browser renders a full-page 403
/// for non-members but a small fragment for non-authors. Re-check membership to
/// pick the byte-identical response.
async fn forbidden_edit_page(state: &AppState, principal: &Principal, team_slug: &str) -> Response {
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
        .is_team_member(team.id, principal.user_id())
        .await
    {
        Ok(false) => non_member_page(team_slug),
        Ok(true) => forbidden_fragment("You may only edit your own comments."),
        Err(err) => internal_error("is_team_member", err),
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
            "You are not a member of the {team_slug:?} team and cannot comment on its issues."
        ),
    )
}

fn internal_error<E: std::fmt::Display>(label: &str, err: E) -> Response {
    tracing::error!(error = %err, "{label} failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}

fn bad_request_fragment(message: &str) -> Response {
    // htmx-aware error fragment via `errors/issue_400.html`: small element,
    // marked as a fragment so the front-end can hx-swap into the same error
    // slot. Empty + whitespace-only + over-length all flow through here with
    // distinct messages. Copy preserved byte-identically; `message` is
    // auto-escaped by Askama (matches the previous `html_escape`).
    let body = views::CommentCreateError {
        message: message.to_string(),
    }
    .render()
    .expect("comment-create-error render (infallible String buffer)");
    (StatusCode::BAD_REQUEST, Html(body)).into_response()
}

/// Slice-5 410 Gone fragment for PATCH/DELETE on a soft-deleted row.
/// Per DISTILL D4 = A — terse copy: "This comment has been deleted.
/// Refresh to see the latest state." Substring match in the acceptance
/// suite so a v0.2 copy polish does not red the test.
fn gone_fragment() -> Response {
    let body = r#"<p class="error comment-deleted-notice" data-hx-fragment="comment-deleted-notice">This comment has been deleted. Refresh to see the latest state.</p>"#;
    (StatusCode::GONE, Html(body)).into_response()
}

/// Slice-5 404 fragment for a missing comment id (random UUID, wrong
/// workspace).
fn not_found_fragment(message: &str) -> Response {
    let body = format!(
        r#"<p class="error comment-not-found-notice" data-hx-fragment="comment-not-found">{}</p>"#,
        html_escape(message)
    );
    (StatusCode::NOT_FOUND, Html(body)).into_response()
}

/// Slice-5 403 fragment for authorized-team-member-but-not-author /
/// not-admin attempts on PATCH (or non-author on GET edit-form).
fn forbidden_fragment(message: &str) -> Response {
    let body = format!(
        r#"<p class="error comment-forbidden-notice" data-hx-fragment="comment-forbidden">{}</p>"#,
        html_escape(message)
    );
    (StatusCode::FORBIDDEN, Html(body)).into_response()
}

fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("hx-request")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Materialize the [`views::IssuePage`] view-model and render it through
/// `issue.html` (extends `base.html`). Selector-and-substring-identical to
/// the previous `format!` markup (render-contract.md §"Issue page +
/// comments"): same `<h1>` key, the `data-comment-list` / `data-attachment-
/// list` markers, one `comment_card.html` per comment, and the empty-state
/// copy. Data ordering + affordance flags stay here in the handler; the
/// template only loops and renders.
#[allow(clippy::too_many_arguments)]
fn render_issue_page(
    team_slug: &str,
    project_slug: &str,
    issue_key: &str,
    comments: &[CommentRow],
    attachments: &[AttachmentSummary],
    changes: &[IssueChangeRow],
    actor_user_id: uuid::Uuid,
    actor_is_admin: bool,
    csrf: &str,
    nav: crate::nav::NavContext,
) -> String {
    let number = extract_number(issue_key);
    let timeline = changes.iter().map(build_timeline_entry).collect();
    let cards = comments
        .iter()
        .map(|row| {
            build_comment_card(
                row,
                team_slug,
                project_slug,
                &number,
                Some(actor_user_id),
                actor_is_admin,
            )
        })
        .collect();
    let attachment_items = attachments
        .iter()
        .map(|a| views::AttachmentItem {
            filename: a.filename.clone(),
            href: format!(
                "/team/{team_slug}/project/{project_slug}/issues/{number}/attachments/{id}",
                id = a.id
            ),
            size: humanize_size(a.size_bytes),
        })
        .collect();
    views::IssuePage {
        issue_key: issue_key.to_string(),
        project_slug: project_slug.to_string(),
        post_url: format!("/team/{team_slug}/project/{project_slug}/issues/{number}/comments"),
        csrf: csrf.to_string(),
        upload_url: format!("/team/{team_slug}/project/{project_slug}/issues/{number}/attachments"),
        attachments: attachment_items,
        comments: cards,
        timeline,
        nav,
    }
    .render()
    .expect("issue page render (infallible String buffer)")
}

/// Build one plain-language [`views::TimelineEntry`] from a stored change row
/// (issue-change-history ADR-002 §1). The raw `field` + `new_value` slugs ride
/// through as scraper-stable `data-` markers; the `summary` is the attributed
/// sentence a reader sees. Slice 01 records only `status`; the generic arm keeps
/// later fields (title/description/rank) legible without a second pass. State
/// slugs are humanized (`in_progress` → `In Progress`) for display only — the
/// stored values are untouched.
fn build_timeline_entry(row: &IssueChangeRow) -> views::TimelineEntry {
    let summary = match row.field.as_str() {
        "status" => format!(
            "{actor} moved status {old} → {new}",
            actor = row.actor_name,
            old = row
                .old_value
                .as_deref()
                .map(humanize_state)
                .unwrap_or_default(),
            new = humanize_state(&row.new_value),
        ),
        _ => format!(
            "{actor} changed {field} {old} → {new}",
            actor = row.actor_name,
            field = row.field,
            old = row.old_value.as_deref().unwrap_or(""),
            new = row.new_value,
        ),
    };
    views::TimelineEntry {
        field: row.field.clone(),
        new_value: row.new_value.clone(),
        summary,
    }
}

/// Humanize a stored state slug for the timeline sentence (display only):
/// `backlog` → `Backlog`, `todo` → `Todo`, `in_progress` → `In Progress`,
/// `done` → `Done`, `cancelled` → `Cancelled`. An unknown slug renders verbatim
/// so a future state never blanks the entry.
pub(crate) fn humanize_state(slug: &str) -> String {
    match slug {
        "backlog" => "Backlog".to_string(),
        "todo" => "Todo".to_string(),
        "in_progress" => "In Progress".to_string(),
        "done" => "Done".to_string(),
        "cancelled" => "Cancelled".to_string(),
        other => other.to_string(),
    }
}

/// Build the [`views::CommentCard`] view-model from a store row + the
/// handler-computed affordance flags. The Edit affordance is offered only
/// when `actor_user_id == row.author_id` (ADR-006 — author-only edit). The
/// Delete affordance is offered when the actor is the author OR a workspace
/// admin (ADR-007). Server-side gating; no JS authorship check (ADR-006 §
/// Consequences). The `(edited)` indicator surfaces whenever `row.edited`
/// (Q4 = A). `body_html` is the ALREADY-core-sanitized HTML — the template
/// embeds it via `|safe`; every other field is auto-escaped (NFR-WEBB-BND-03).
fn build_comment_card(
    row: &CommentRow,
    team_slug: &str,
    project_slug: &str,
    issue_number: &str,
    actor_user_id: Option<uuid::Uuid>,
    actor_is_admin: bool,
) -> views::CommentCard {
    let is_author = actor_user_id == Some(row.author_id);
    views::CommentCard {
        id: row.id.to_string(),
        author: row.author_email.clone(),
        body_html: row.body_html.clone(),
        edited: row.edited,
        can_edit: is_author,
        can_delete: is_author || actor_is_admin,
        edit_url: format!(
            "/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments/{id}/edit",
            id = row.id
        ),
        delete_url: format!(
            "/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments/{id}",
            id = row.id
        ),
    }
}

/// Render a single comment card to HTML through the shared
/// `comment_card.html` partial (the one-partial rule, NFR-WEBB-MAINT-02).
/// Used by the PATCH-edit re-render and the GET single-comment / cancel
/// paths; the issue-page loop includes the SAME partial.
fn render_comment_card(
    row: &CommentRow,
    team_slug: &str,
    project_slug: &str,
    issue_number: &str,
    actor_user_id: Option<uuid::Uuid>,
    actor_is_admin: bool,
) -> String {
    let card = build_comment_card(
        row,
        team_slug,
        project_slug,
        issue_number,
        actor_user_id,
        actor_is_admin,
    );
    views::CommentCardFragment { card }
        .render()
        .expect("comment card render (infallible String buffer)")
}

/// htmx OOB-swap variant: the SAME card wrapped so the front-end can append
/// it to the comment list without a full page reload. The newly-posted card
/// is appended to the bottom of the existing thread; the actor IS the author
/// by construction (a fresh post → `can_edit=true`, `can_delete` per the
/// author/admin rule — ADR-006/ADR-007).
///
/// The OOB wrapper (`partials/oob/comment_card_oob.html`) `{% include %}`s the
/// SAME `comment_card.html` partial the full-page loop and single-card paths
/// use (the one-partial rule, NFR-WEBB-MAINT-02 / DD10). Consequently the live
/// card is SELECTOR-IDENTICAL to the reloaded card, affordances and all — the
/// deliberate Edit/Delete omission at the old comments.rs:841 is gone. The
/// handler computes the affordance flags via [`build_comment_card`] (same seam
/// the list path uses), so the live and reloaded cards can no longer drift.
fn render_comment_card_oob(
    row: &CommentRow,
    team_slug: &str,
    project_slug: &str,
    issue_number: &str,
    actor_user_id: Option<uuid::Uuid>,
    actor_is_admin: bool,
) -> String {
    let card = build_comment_card(
        row,
        team_slug,
        project_slug,
        issue_number,
        actor_user_id,
        actor_is_admin,
    );
    views::CommentCardOob { card }
        .render()
        .expect("comment card OOB render (infallible String buffer)")
}

/// Pull the trailing number off an issue key like "AUTH-3" → "3". The
/// number is the canonical URL component; the key is only used for the
/// page title.
fn extract_number(issue_key: &str) -> String {
    issue_key
        .rsplit_once('-')
        .map(|(_, n)| n.to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod affordance_tests {
    //! Unit coverage for the [`build_comment_card`] affordance predicate
    //! (ADR-006 author-only edit; ADR-007 author-or-admin delete). This is
    //! the security-adjacent seam: a mutation flipping `can_delete` to
    //! always-true, or `can_edit` away from author-only, is a real
    //! authorisation bug. `build_comment_card` is a pure view-model builder —
    //! its signature IS the driving port for the affordance decision, so
    //! calling it directly is port-to-port at domain scope.
    use super::build_comment_card;
    use foundry_store::CommentRow;
    use uuid::Uuid;

    fn comment_row(author_id: Uuid) -> CommentRow {
        CommentRow {
            id: Uuid::now_v7(),
            author_id,
            author_email: "author@example.com".to_string(),
            body_html: "<p>hi</p>".to_string(),
            created_at: time::OffsetDateTime::now_utc(),
            edited: false,
        }
    }

    /// The full author × admin affordance matrix.
    ///
    /// Invariant 1 (ADR-006): `can_edit` is TRUE iff the actor authored the
    /// comment — admin status is irrelevant to edit.
    /// Invariant 2 (ADR-007): `can_delete` is TRUE iff the actor authored the
    /// comment OR is a workspace admin.
    ///
    /// The non-author non-admin row (edit=F, delete=F) pins the `==` author
    /// check; the non-author admin row (edit=F, delete=T) pins the `||` in the
    /// delete predicate — flipping it to `&&` would deny an admin moderator.
    #[test]
    fn affordances_follow_author_and_admin_rules() {
        let author = Uuid::now_v7();
        let other = Uuid::now_v7();
        let row = comment_row(author);

        // (actor, is_admin) -> (expected_can_edit, expected_can_delete)
        let cases = [
            (Some(author), false, true, true),  // author, non-admin
            (Some(author), true, true, true),   // author, admin
            (Some(other), false, false, false), // stranger, non-admin
            (Some(other), true, false, true),   // admin moderator (not author)
            (None, false, false, false),        // anonymous
            (None, true, false, true),          // admin, no actor id
        ];

        for (actor, is_admin, want_edit, want_delete) in cases {
            let card = build_comment_card(&row, "backend", "auth", "3", actor, is_admin);
            assert_eq!(
                card.can_edit, want_edit,
                "can_edit for actor={actor:?} admin={is_admin}"
            );
            assert_eq!(
                card.can_delete, want_delete,
                "can_delete for actor={actor:?} admin={is_admin}"
            );
        }
    }
}
