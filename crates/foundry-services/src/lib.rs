//! foundry-services — the shared application-service seam.
//!
//! SCAFFOLD: true  (RED scaffold created by DISTILL, Mandate 7)
//!
//! Per DESIGN (`docs/feature/web-tier-extraction/design/architecture.md`
//! §"The shared application-service seam", ADR-W04 + ADR-W07) this crate is
//! the single, acyclic home for the use-case orchestration that BOTH the HTML
//! adapter (`foundry-app`) and the JSON adapter (`foundry-api`) call. It owns:
//!   - `Principal` (Human | Machine) — the unified authenticated actor.
//!   - `ServiceError` — the single source of truth for use-case failures,
//!     mapped to HTTP/JSON in foundry-api and to HTML in foundry-app.
//!   - the use-cases: `board::list_board_issues`, `issues::{create, change_state}`,
//!     `comments::{create, edit}`.
//!
//! DELIVER lifts the orchestration out of the foundry-app handlers (a pure
//! move-and-call, keeping the HTML responses byte-identical — NFR-WEB-COMPAT-02)
//! and points both adapters at these functions. The `panic!`s below classify
//! as RED (MISSING_FUNCTIONALITY), never BROKEN.
//!
//! Signatures here are intentionally store-agnostic placeholders: the real
//! functions take the `foundry-store` `Store` + repositories, which this
//! scaffold does NOT yet depend on (keeping the workspace build green). DELIVER
//! adds the `foundry-store` / `foundry-auth` dependencies and the real
//! parameter lists per architecture.md.

#![forbid(unsafe_code)]

/// Marker so DELIVER can `grep -rn "SCAFFOLD: true" crates/` to find every
/// stub that still needs replacing before the boundary guard's
/// "no scaffolds remain" check passes.
pub const __SCAFFOLD__: bool = true;

const NOT_IMPLEMENTED: &str = "Not yet implemented -- RED scaffold (foundry-services, Feature A)";

/// The authenticated actor a use-case acts on behalf of. Per architecture.md
/// the service cannot tell whether the caller is a human (browser session) or
/// a machine (bearer credential): both carry a `user_id` + `workspace_id`, and
/// authorization is computed from those exactly as today.
#[derive(Debug, Clone)]
pub enum Principal {
    Human {
        user_id: uuid::Uuid,
        workspace_id: uuid::Uuid,
    },
    Machine {
        user_id: uuid::Uuid,
        workspace_id: uuid::Uuid,
        jti: uuid::Uuid,
        /// Team-narrowing filter; `None` = workspace-wide (still bounded by the
        /// bound principal's membership). Checked in the token-auth extractor.
        scope_team_id: Option<uuid::Uuid>,
    },
}

/// The single source of truth for use-case failures (DESIGN
/// `error-and-observability.md`). foundry-api maps each variant to one HTTP
/// status + JSON envelope code; foundry-app (Feature B) maps the SAME variant
/// to an HTML fragment — the error-side proof of rule-parity.
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
    #[error("unauthorized")]
    Unauthorized,
    #[error("validation: {code}: {message}")]
    Validation { code: String, message: String },
    #[error("gone")]
    Gone,
    #[error("conflict")]
    Conflict,
    #[error("internal error")]
    Internal,
}

/// A board issue as the neutral core returns it — never HTML, never JSON.
/// foundry-app renders it; foundry-api serializes it.
#[derive(Debug, Clone)]
pub struct BoardIssue {
    pub key: String,
    pub number: i32,
    pub title: String,
    /// Canonical lower_snake state (`backlog`, `todo`, `in_progress`, `done`,
    /// `cancelled`) — the same value the store persists.
    pub state: String,
}

/// The freshly-created issue a write use-case returns.
#[derive(Debug, Clone)]
pub struct CreatedIssue {
    pub key: String,
    pub number: i32,
    pub state: String,
}

pub mod board {
    use super::*;

    /// US-W05a / Feature B board read. The SAME function the JSON board
    /// endpoint and (Feature B) the HTML board call — the literal proof of
    /// core neutrality (NFR-WEB-BND-05).
    ///
    /// DELIVER signature (per architecture.md) takes `&Store`, the `Principal`,
    /// and the team/project slugs; performs the membership authz; calls
    /// `store.list_issues_by_project`; returns the neutral list.
    pub fn list_board_issues(
        _principal: &Principal,
        _team_slug: &str,
        _project_slug: &str,
    ) -> Result<Vec<BoardIssue>, ServiceError> {
        panic!("{}", NOT_IMPLEMENTED)
    }
}

pub mod issues {
    use super::*;

    /// US-W05c create-issue use-case. Reuses `insert_issue_with_outbox` and
    /// the same title validation the browser handler enforces.
    pub fn create_issue(
        _principal: &Principal,
        _team_slug: &str,
        _project_slug: &str,
        _title: &str,
    ) -> Result<CreatedIssue, ServiceError> {
        panic!("{}", NOT_IMPLEMENTED)
    }

    /// US-W05c change-state use-case. Reuses `update_issue_state_with_outbox`
    /// and the SAME `normalize_state` logic the UI uses (DD10).
    pub fn change_issue_state(
        _principal: &Principal,
        _team_slug: &str,
        _project_slug: &str,
        _number: i32,
        _new_state: &str,
    ) -> Result<BoardIssue, ServiceError> {
        panic!("{}", NOT_IMPLEMENTED)
    }
}

pub mod comments {
    use super::*;

    /// A comment as the neutral core returns it — `body_html` is the
    /// core-sanitized markup (`render_comment_markdown`), the SAME bytes the UI
    /// stores. foundry-api serializes it inside a JSON string field (which is
    /// NOT an HTML response body — the boundary guard explicitly allows it).
    #[derive(Debug, Clone)]
    pub struct CommentView {
        pub id: uuid::Uuid,
        pub author_email: String,
        pub body_html: String,
        pub edited: bool,
    }

    /// US-W05c create-comment use-case. Calls `render_comment_markdown` in core
    /// (NFR-WEB-BND-03) then `insert_comment_with_outbox`.
    pub fn create_comment(
        _principal: &Principal,
        _team_slug: &str,
        _project_slug: &str,
        _issue_number: i32,
        _body: &str,
    ) -> Result<CommentView, ServiceError> {
        panic!("{}", NOT_IMPLEMENTED)
    }

    /// US-W05c edit-comment use-case. Authorship authz (author or admin) is
    /// decided here, never in the adapter.
    pub fn edit_comment(
        _principal: &Principal,
        _team_slug: &str,
        _project_slug: &str,
        _issue_number: i32,
        _comment_id: uuid::Uuid,
        _new_body: &str,
    ) -> Result<CommentView, ServiceError> {
        panic!("{}", NOT_IMPLEMENTED)
    }
}
