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
use crate::bootstrap::{html_escape, invalid_page, SessionUser};
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
use axum::extract::{Form, Path, State};
use axum::http::header::{HeaderMap, HeaderValue, LOCATION};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use foundry_core::render_comment_markdown;
use foundry_store::{AttachmentSummary, CommentInsertError, CommentRow};
use serde::Deserialize;
use tower_sessions::Session;

const BODY_MAX_LEN: usize = 65_536;

#[derive(Debug, Deserialize)]
pub struct CreateCommentForm {
    pub body: String,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

// ------------------------------------- GET /team/:team/project/:project/issues/:n

pub async fn show_issue(
    State(state): State<AppState>,
    Path((team_slug, project_slug, issue_number)): Path<(String, String, i32)>,
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
    let issue = match state
        .store
        .find_issue_by_team_project_number(team.id, &project_slug, issue_number)
        .await
    {
        Ok(Some(i)) => i,
        Ok(None) => return issue_not_found_page(&team_slug, &project_slug, issue_number),
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

    let key = format!("{}-{}", issue.project_key_prefix, issue_number);
    Html(render_issue_page(
        &team_slug,
        &project_slug,
        &key,
        &comments,
        &attachments,
    ))
    .into_response()
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

    // Validate the body BEFORE touching the issue row — empty body is a
    // 400, not a 404, regardless of whether the issue exists.
    let trimmed = form.body.trim();
    if trimmed.is_empty() {
        return bad_request_fragment("Comment cannot be empty");
    }
    if trimmed.chars().count() > BODY_MAX_LEN {
        return bad_request_fragment("Comment is too long");
    }

    let issue = match state
        .store
        .find_issue_by_team_project_number(team.id, &project_slug, issue_number)
        .await
    {
        Ok(Some(i)) => i,
        Ok(None) => return issue_not_found_page(&team_slug, &project_slug, issue_number),
        Err(err) => return internal_error("find_issue_by_team_project_number", err),
    };

    // Sanitized render. The raw markdown is also persisted so a future
    // sanitizer revision can re-render the table without lossy data.
    let html = render_comment_markdown(&form.body);

    // Look up the actor's display email for the outbox payload (per
    // wave-decisions.md, fan-out carries `author_email` so subscribers
    // don't JOIN at delivery time).
    let author_email = match state.store.find_user_email_by_id(user.user_id).await {
        Ok(Some(email)) => email,
        Ok(None) => {
            // Shouldn't happen — the session is anchored to a user row.
            return internal_error(
                "find_user_email_by_id",
                "session user not found in users table",
            );
        }
        Err(err) => return internal_error("find_user_email_by_id", err),
    };

    let comment_id = uuid::Uuid::now_v7();
    if let Err(err) = state
        .store
        .insert_comment_with_outbox(
            comment_id,
            issue.workspace_id,
            issue.project_id,
            &issue.project_key_prefix,
            issue.issue_id,
            issue_number,
            user.user_id,
            &author_email,
            &form.body,
            html.as_str(),
        )
        .await
    {
        return match err {
            CommentInsertError::IssueNotFound => {
                issue_not_found_page(&team_slug, &project_slug, issue_number)
            }
            CommentInsertError::Store(e) => internal_error("insert_comment_with_outbox", e),
        };
    }

    if is_htmx(&headers) {
        // htmx fragment: just the new comment card. The list page can
        // hx-swap-oob "beforeend" into `[data-comment-list]`.
        let row = CommentRow {
            id: comment_id,
            author_email,
            body_html: html.into_inner(),
            created_at: time::OffsetDateTime::now_utc(),
        };
        return (StatusCode::OK, Html(render_comment_card_oob(&row))).into_response();
    }

    // Plain redirect back to the issue page.
    redirect_to(&format!(
        "/team/{team_slug}/project/{project_slug}/issues/{issue_number}"
    ))
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
            "You are not a member of the {team_slug:?} team and cannot comment on its issues."
        ),
    )
}

fn issue_not_found_page(team_slug: &str, project_slug: &str, n: i32) -> Response {
    invalid_page(
        StatusCode::NOT_FOUND,
        "Issue not found",
        &format!("No issue #{n} in project {project_slug:?} (team {team_slug:?})."),
    )
}

fn internal_error<E: std::fmt::Display>(label: &str, err: E) -> Response {
    tracing::error!(error = %err, "{label} failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}

fn bad_request_fragment(message: &str) -> Response {
    // Inline htmx-aware error fragment. Mirrors issues.rs: small element,
    // marked as a fragment so the front-end can hx-swap into the same
    // error slot. Empty + whitespace-only + over-length all flow through
    // here with distinct messages.
    let body = format!(
        r#"<div class="error" data-hx-fragment="comment-create-error">{}</div>"#,
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

fn render_issue_page(
    team_slug: &str,
    project_slug: &str,
    issue_key: &str,
    comments: &[CommentRow],
    attachments: &[AttachmentSummary],
) -> String {
    let comment_list = if comments.is_empty() {
        "<p class=\"empty\">No comments yet.</p>".to_string()
    } else {
        comments.iter().map(render_comment_card).collect::<String>()
    };
    let number = extract_number(issue_key);
    let post_url = format!("/team/{team_slug}/project/{project_slug}/issues/{number}/comments");
    let attachments_section =
        render_attachments_section(team_slug, project_slug, &number, attachments);
    let upload_url =
        format!("/team/{team_slug}/project/{project_slug}/issues/{number}/attachments");
    format!(
        r#"<!doctype html>
<html><head><title>{key} - {project_slug}</title></head>
<body>
<header><h1>{key}</h1></header>
{attachments_section}
<form method="post" action="{upload_url}" enctype="multipart/form-data">
  <label>Attach a file <input type="file" name="file" required></label>
  <button type="submit">Upload</button>
</form>
<section class="comments" data-comment-list>{comment_list}</section>
<form method="post" action="{post_url}">
  <label>Add a comment <textarea name="body" required></textarea></label>
  <button type="submit">Post</button>
</form>
</body></html>"#,
        key = html_escape(issue_key),
        project_slug = html_escape(project_slug),
        post_url = html_escape(&post_url),
        upload_url = html_escape(&upload_url),
    )
}

fn render_attachments_section(
    team_slug: &str,
    project_slug: &str,
    issue_number: &str,
    attachments: &[AttachmentSummary],
) -> String {
    if attachments.is_empty() {
        return String::from(
            r#"<section class="attachments" data-attachment-list>
  <p class="attachments-empty">No attachments yet.</p>
</section>"#,
        );
    }
    let items = attachments
        .iter()
        .map(|a| {
            let href = format!(
                "/team/{team_slug}/project/{project_slug}/issues/{issue_number}/attachments/{id}",
                id = a.id
            );
            format!(
                r#"<li class="attachment" data-filename="{filename}">
  <a class="attachment-link" href="{href}">{filename}</a>
  <span class="size">{size}</span>
</li>"#,
                filename = html_escape(&a.filename),
                href = html_escape(&href),
                size = html_escape(&humanize_size(a.size_bytes)),
            )
        })
        .collect::<String>();
    format!(
        r#"<section class="attachments">
<ul data-attachment-list>{items}</ul>
</section>"#
    )
}

fn render_comment_card(row: &CommentRow) -> String {
    // Important: `row.body_html` is ALREADY sanitized HTML emitted by
    // `foundry_core::render_comment_markdown`. We embed it verbatim;
    // double-escaping would render the tags as text. The author email
    // IS user input and must be escaped.
    format!(
        r#"<article class="comment" data-author="{author}" data-comment-id="{id}">
  <header class="comment-author">{author}</header>
  <div class="comment-body">{body}</div>
</article>"#,
        author = html_escape(&row.author_email),
        id = row.id,
        body = row.body_html,
    )
}

/// htmx OOB-swap variant: same card wrapped so the front-end can append
/// it to the comment list without a full page reload.
fn render_comment_card_oob(row: &CommentRow) -> String {
    format!(
        r#"<div hx-swap-oob="beforeend:[data-comment-list]">{card}</div>"#,
        card = render_comment_card(row),
    )
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
