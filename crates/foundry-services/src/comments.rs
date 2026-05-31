//! Comment write use-cases — the shared seam both the HTML adapter
//! (`foundry-app`) and the JSON adapter (`foundry-api`, 03-02) call.
//!
//! `create_comment` renders the body through `foundry_core::render_comment_markdown`
//! (NFR-WEB-BND-03) then persists via `Store::insert_comment_with_outbox`.
//! `edit_comment` decides author-or-admin authz HERE (never in the adapter)
//! then persists via `Store::update_comment_with_outbox`. Both reuse the exact
//! core path the browser handlers use so an API write and a browser write
//! accept/reject identically and store identical bytes (NFR-WEB-API-CON-02).

use crate::{Principal, ServiceError};
use foundry_core::render_comment_markdown;
use foundry_store::{CommentInsertError, Store};

/// Comment body length cap (chars, after trimming) — the SAME bound the
/// browser handler enforces.
const BODY_MAX_LEN: usize = 65_536;

/// A comment as the neutral core returns it — `body_html` is the
/// core-sanitized markup (`render_comment_markdown`), the SAME bytes the UI
/// stores. foundry-api serializes it inside a JSON string field.
#[derive(Debug, Clone)]
pub struct CommentView {
    pub id: uuid::Uuid,
    pub author_email: String,
    pub body_html: String,
    pub edited: bool,
}

/// US-W05c create-comment use-case. Renders markdown in core then writes via
/// `Store::insert_comment_with_outbox`, reusing the browser handler's validation
/// (trimmed, non-empty, ≤65_536) and authz (team membership).
pub async fn create_comment(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    issue_number: i32,
    body: &str,
) -> Result<CommentView, ServiceError> {
    let team = resolve_member_team(store, principal, team_slug).await?;

    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err(ServiceError::Validation {
            code: "comment_empty".to_string(),
            message: "Comment cannot be empty".to_string(),
        });
    }
    if trimmed.chars().count() > BODY_MAX_LEN {
        return Err(ServiceError::Validation {
            code: "comment_too_long".to_string(),
            message: "Comment is too long".to_string(),
        });
    }

    let issue = store
        .find_issue_by_team_project_number(team.id, project_slug, issue_number)
        .await
        .map_err(|_| ServiceError::Internal)?
        .ok_or(ServiceError::NotFound)?;

    let html = render_comment_markdown(body);

    let author_email = store
        .find_user_email_by_id(principal.user_id())
        .await
        .map_err(|_| ServiceError::Internal)?
        .ok_or(ServiceError::Internal)?;

    let comment_id = uuid::Uuid::now_v7();
    store
        .insert_comment_with_outbox(
            comment_id,
            issue.workspace_id,
            issue.project_id,
            &issue.project_key_prefix,
            issue.issue_id,
            issue_number,
            principal.user_id(),
            &author_email,
            body,
            html.as_str(),
        )
        .await
        .map_err(|err| match err {
            CommentInsertError::IssueNotFound => ServiceError::NotFound,
            CommentInsertError::Store(_) => ServiceError::Internal,
        })?;

    Ok(CommentView {
        id: comment_id,
        author_email,
        body_html: html.into_inner(),
        edited: false,
    })
}

/// US-W05c edit-comment use-case. Author-only authz is decided HERE (ADR-006):
/// a non-author (even a team member) is refused Forbidden. Tombstoned rows are
/// Gone, missing rows are NotFound. Persists via `Store::update_comment_with_outbox`.
#[allow(clippy::too_many_arguments)]
pub async fn edit_comment(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    _project_slug: &str,
    _issue_number: i32,
    comment_id: uuid::Uuid,
    new_body: &str,
) -> Result<CommentView, ServiceError> {
    let _team = resolve_member_team(store, principal, team_slug).await?;

    let trimmed = new_body.trim();
    if trimmed.is_empty() {
        return Err(ServiceError::Validation {
            code: "comment_empty".to_string(),
            message: "Comment cannot be empty".to_string(),
        });
    }
    if trimmed.chars().count() > BODY_MAX_LEN {
        return Err(ServiceError::Validation {
            code: "comment_too_long".to_string(),
            message: "Comment is too long".to_string(),
        });
    }

    let comment = store
        .find_comment_by_id(principal.workspace_id(), comment_id)
        .await
        .map_err(|_| ServiceError::Internal)?
        .ok_or(ServiceError::NotFound)?;
    if comment.deleted {
        return Err(ServiceError::Gone);
    }
    // ADR-006: edit is author-only.
    if comment.author_id != principal.user_id() {
        return Err(ServiceError::Forbidden);
    }

    let author_email = store
        .find_user_email_by_id(principal.user_id())
        .await
        .map_err(|_| ServiceError::Internal)?
        .ok_or(ServiceError::Internal)?;

    let html = render_comment_markdown(new_body);
    match store
        .update_comment_with_outbox(
            principal.workspace_id(),
            comment_id,
            new_body,
            html.as_str(),
            principal.user_id(),
            &author_email,
        )
        .await
    {
        Ok(true) => Ok(CommentView {
            id: comment_id,
            author_email,
            body_html: html.into_inner(),
            edited: true,
        }),
        // A false return means a race: tombstone landed between the pre-check
        // and the UPDATE. The row IS gone now.
        Ok(false) => Err(ServiceError::Gone),
        Err(_) => Err(ServiceError::Internal),
    }
}

/// Resolve the team after the SAME membership authz the browser comment
/// handlers run: team-by-slug → is_team_member. A scoped machine credential
/// targeting another team is refused (Forbidden).
async fn resolve_member_team(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
) -> Result<foundry_store::TeamRow, ServiceError> {
    let team = store
        .find_team_by_slug(principal.workspace_id(), team_slug)
        .await
        .map_err(|_| ServiceError::Internal)?
        .ok_or(ServiceError::NotFound)?;

    if let Principal::Machine {
        scope_team_id: Some(scoped_team),
        ..
    } = principal
    {
        if *scoped_team != team.id {
            return Err(ServiceError::Forbidden);
        }
    }

    let is_member = store
        .is_team_member(team.id, principal.user_id())
        .await
        .map_err(|_| ServiceError::Internal)?;
    if !is_member {
        return Err(ServiceError::Forbidden);
    }
    Ok(team)
}
