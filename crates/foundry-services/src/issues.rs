//! Issue write use-cases — the shared seam both the HTML adapter
//! (`foundry-app`) and the JSON adapter (`foundry-api`, 03-02) call.
//!
//! These REUSE the exact core write+outbox path the browser handlers use
//! (`Store::insert_issue_with_outbox`, `Store::update_issue_state_with_outbox`)
//! plus the SAME trimmed/non-empty/≤256 title validation and the lifted
//! `normalize_state` (DD10). An API write and a browser write therefore accept
//! or reject identically and store identical bytes (NFR-WEB-API-CON-02).

use crate::{BoardIssue, CreatedIssue, Principal, ServiceError};
use foundry_store::{IssueInsertError, Store};

/// The current title + description of an issue, resolved for the edit-dialog
/// pre-fill (issue-edit-dialog). Returned by [`edit_issue_form`] AFTER the
/// `resolve_member_project` authz gate, so a foreign issue is refused before
/// any field is exposed.
#[derive(Debug, Clone)]
pub struct IssueEditView {
    pub key: String,
    pub number: i32,
    pub title: String,
    pub description_md: String,
}

/// Title length cap (chars, after trimming) — the SAME bound the browser
/// handler enforces.
const TITLE_MAX_LEN: usize = 256;

/// Map the incoming state value (which may be the human label used in feature
/// files like `"in-progress"`) to the schema-enforced enum stored in
/// `issues.state`. MOVED here from `foundry-app/src/issues.rs` (DD10) so the
/// HTML and JSON adapters share one normalisation.
pub fn normalize_state(input: &str) -> Option<&'static str> {
    match input.trim().to_ascii_lowercase().as_str() {
        "backlog" => Some("backlog"),
        "todo" => Some("todo"),
        "in-progress" | "in_progress" => Some("in_progress"),
        "done" => Some("done"),
        "cancelled" | "canceled" => Some("cancelled"),
        _ => None,
    }
}

/// US-W05c create-issue use-case. Reuses `Store::insert_issue_with_outbox` and
/// the SAME title validation (trimmed, non-empty, ≤256) the browser enforces.
pub async fn create_issue(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    title: &str,
) -> Result<CreatedIssue, ServiceError> {
    let (_team, project, key_prefix) =
        resolve_member_project(store, principal, team_slug, project_slug).await?;

    let raw_title = title.trim();
    if raw_title.is_empty() || raw_title.chars().count() > TITLE_MAX_LEN {
        return Err(ServiceError::Validation {
            code: "title_required".to_string(),
            message: "Title is required".to_string(),
        });
    }

    let issue_id = uuid::Uuid::now_v7();
    let number = match store
        .insert_issue_with_outbox(
            issue_id,
            principal.workspace_id(),
            project.id,
            key_prefix.as_str(),
            principal.user_id(),
            raw_title,
        )
        .await
    {
        Ok(n) => n,
        Err(IssueInsertError::ProjectNotFound) => return Err(ServiceError::NotFound),
        Err(IssueInsertError::Store(_)) => return Err(ServiceError::Internal),
    };

    let key = foundry_core::IssueKey::try_new(&key_prefix, number as u32)
        .map(|k| k.to_string())
        .unwrap_or_else(|_| format!("{}-{}", key_prefix.as_str(), number));

    Ok(CreatedIssue {
        key,
        number,
        // The TRIMMED title is what `insert_issue_with_outbox` persisted, so the
        // returned representation matches a subsequent read (NFR-WEB-API-CON-02).
        title: raw_title.to_string(),
        state: "backlog".to_string(),
    })
}

/// US-W05c change-state use-case. Reuses `Store::update_issue_state_with_outbox`
/// and the SAME `normalize_state` logic the UI uses (DD10).
pub async fn change_issue_state(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    number: i32,
    new_state: &str,
) -> Result<BoardIssue, ServiceError> {
    let (_team, _project, key_prefix) =
        resolve_member_project(store, principal, team_slug, project_slug).await?;

    let normalized = normalize_state(new_state).ok_or_else(|| ServiceError::Validation {
        code: "invalid_state".to_string(),
        message: "Invalid issue state".to_string(),
    })?;

    match store
        .update_issue_state_with_outbox(
            key_prefix.as_str(),
            number,
            normalized,
            principal.user_id(),
        )
        .await
    {
        Ok(Some(())) => {
            let key = foundry_core::IssueKey::try_new(&key_prefix, number as u32)
                .map(|k| k.to_string())
                .unwrap_or_else(|_| format!("{}-{}", key_prefix.as_str(), number));
            Ok(BoardIssue {
                key,
                number,
                title: String::new(),
                state: normalized.to_string(),
            })
        }
        Ok(None) => Err(ServiceError::NotFound),
        Err(IssueInsertError::ProjectNotFound) => Err(ServiceError::NotFound),
        Err(IssueInsertError::Store(_)) => Err(ServiceError::Internal),
    }
}

/// issue-edit-dialog edit-title+description use-case (ADR-002). Mirrors
/// [`change_issue_state`]: `resolve_member_project` authz + non-enumerable
/// NotFound → validate the title (trimmed, non-empty, ≤256 — the SAME bound
/// create enforces) → `Store::update_issue_details`. Returns the updated
/// `BoardIssue` so the handler re-renders the board card. `description_md` is
/// persisted verbatim (the DB CHECK bounds its length); v1 is last-write-wins
/// with no outbox emit (ODD-3/ODD-4).
#[allow(clippy::too_many_arguments)]
pub async fn edit_issue_details(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    number: i32,
    title: &str,
    description_md: &str,
) -> Result<BoardIssue, ServiceError> {
    let (_team, _project, key_prefix) =
        resolve_member_project(store, principal, team_slug, project_slug).await?;

    let raw_title = title.trim();
    if raw_title.is_empty() || raw_title.chars().count() > TITLE_MAX_LEN {
        return Err(ServiceError::Validation {
            code: "title_required".to_string(),
            message: "Title is required".to_string(),
        });
    }

    match store
        .update_issue_details(
            key_prefix.as_str(),
            number,
            raw_title,
            description_md,
            principal.user_id(),
        )
        .await
    {
        Ok(Some(())) => {
            let key = foundry_core::IssueKey::try_new(&key_prefix, number as u32)
                .map(|k| k.to_string())
                .unwrap_or_else(|_| format!("{}-{}", key_prefix.as_str(), number));
            Ok(BoardIssue {
                key,
                number,
                // The TRIMMED title is what `update_issue_details` persisted, so
                // the re-rendered card matches a subsequent read. `state` is not
                // part of an edit (the card render only needs key + title), so it
                // is left empty exactly as `change_issue_state` leaves `title`.
                title: raw_title.to_string(),
                state: String::new(),
            })
        }
        Ok(None) => Err(ServiceError::NotFound),
        Err(IssueInsertError::ProjectNotFound) => Err(ServiceError::NotFound),
        Err(IssueInsertError::Store(_)) => Err(ServiceError::Internal),
    }
}

/// issue-edit-dialog pre-fill read (ADR-002). Resolves the issue's current
/// title + description for the edit dialog AFTER the `resolve_member_project`
/// authz gate, so a foreign or missing issue is refused with the uniform
/// non-enumerable NotFound (ADR-003) and no title is ever echoed for a resource
/// the caller may not see.
pub async fn edit_issue_form(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    number: i32,
) -> Result<IssueEditView, ServiceError> {
    let (_team, _project, key_prefix) =
        resolve_member_project(store, principal, team_slug, project_slug).await?;

    let row = store
        .issue_edit_view(key_prefix.as_str(), number)
        .await
        .map_err(|_| ServiceError::Internal)?
        .ok_or(ServiceError::NotFound)?;

    let key = foundry_core::IssueKey::try_new(&key_prefix, number as u32)
        .map(|k| k.to_string())
        .unwrap_or_else(|_| format!("{}-{}", key_prefix.as_str(), number));
    Ok(IssueEditView {
        key,
        number,
        title: row.title,
        description_md: row.description_md,
    })
}

/// Resolve `(team, project, key_prefix)` after the SAME membership authz the
/// browser handlers run: team-by-slug → is_team_member → project-by-slug.
/// A machine credential scoped to another team is refused (Forbidden), exactly
/// as the board read use-case does (NFR-WEB-API-SEC-02).
async fn resolve_member_project(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
) -> Result<
    (
        foundry_store::TeamRow,
        foundry_store::ProjectRow,
        foundry_core::ProjectKey,
    ),
    ServiceError,
> {
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

    let project = store
        .find_project_by_slug(team.id, project_slug)
        .await
        .map_err(|_| ServiceError::Internal)?
        .ok_or(ServiceError::NotFound)?;

    let key_prefix = foundry_core::ProjectKey::try_new(&project.key_prefix)
        .map_err(|_| ServiceError::Internal)?;

    Ok((team, project, key_prefix))
}
