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

impl Principal {
    /// The acting user's id — the same value whether the caller is a browser
    /// session (Human) or a bearer credential (Machine). Authorization is
    /// computed from this exactly as the HTML handler does today.
    pub fn user_id(&self) -> uuid::Uuid {
        match self {
            Principal::Human { user_id, .. } | Principal::Machine { user_id, .. } => *user_id,
        }
    }

    /// The workspace the actor is bound to.
    pub fn workspace_id(&self) -> uuid::Uuid {
        match self {
            Principal::Human { workspace_id, .. } | Principal::Machine { workspace_id, .. } => {
                *workspace_id
            }
        }
    }
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

/// US-W05b machine-token authentication helper. Lives in `foundry-services`
/// (NOT `foundry-api`) so the `/api/v1` adapter never depends on
/// `foundry-store` directly — staying ahead of the Phase-04 boundary guard
/// (`foundry-api ⊀ foundry-store`, architecture.md / step-skeletons option B).
/// The bearer extractor authenticates the JWT crypto itself (verifier in
/// foundry-api), then routes the `jti` denylist read through here.
pub mod auth {
    use super::*;
    use foundry_store::Store;

    /// The principal binding recovered from an ACTIVE machine-token registry
    /// row. The extractor turns this into `Principal::Machine`.
    #[derive(Debug, Clone)]
    pub struct ActiveMachineToken {
        pub user_id: uuid::Uuid,
        pub workspace_id: uuid::Uuid,
        pub scope_team_id: Option<uuid::Uuid>,
    }

    /// The single allowed `foundry-store` touch for the JSON adapter: the
    /// per-request `jti` denylist read. A credential is ACTIVE iff a registry
    /// row exists, is not revoked (`revoked_at IS NULL`), and has not expired
    /// (`expires_at > now`). Every refusal collapses to
    /// `ServiceError::Unauthorized` — the caller is NEVER told which condition
    /// failed (non-enumerable refusal, error-and-observability.md). A store
    /// error also fails closed as `Unauthorized` (no credential is honored when
    /// the denylist cannot be consulted).
    pub async fn resolve_active_token(
        store: &Store,
        jti: uuid::Uuid,
        now: time::OffsetDateTime,
    ) -> Result<ActiveMachineToken, ServiceError> {
        let row = store
            .find_machine_token_by_jti(jti)
            .await
            .map_err(|_| ServiceError::Unauthorized)?
            .ok_or(ServiceError::Unauthorized)?;
        if row.revoked_at.is_some() {
            return Err(ServiceError::Unauthorized);
        }
        if row.expires_at <= now {
            return Err(ServiceError::Unauthorized);
        }
        // Best-effort operational visibility ("when was this credential last
        // seen"). Fire-and-forget — a touch failure NEVER fails the request.
        let _ = store.touch_machine_token_last_used(jti).await;
        Ok(ActiveMachineToken {
            user_id: row.user_id,
            workspace_id: row.workspace_id,
            scope_team_id: row.scope_team_id,
        })
    }
}

pub mod board {
    use super::*;
    use foundry_store::Store;

    /// US-W05a / Feature B board read. The SAME function the JSON board
    /// endpoint and (Feature B) the HTML board call — the literal proof of
    /// core neutrality (NFR-WEB-BND-05).
    ///
    /// Takes `&Store`, the `Principal`, and the team/project slugs; performs
    /// the membership authz (`Store::is_team_member`) THEN
    /// `Store::list_issues_by_project`; returns the neutral list (no HTML,
    /// no JSON).
    pub async fn list_board_issues(
        store: &Store,
        principal: &Principal,
        team_slug: &str,
        project_slug: &str,
    ) -> Result<Vec<BoardIssue>, ServiceError> {
        let team = store
            .find_team_by_slug(principal.workspace_id(), team_slug)
            .await
            .map_err(|_| ServiceError::Internal)?
            .ok_or(ServiceError::NotFound)?;

        // Scope narrowing (NFR-WEB-API-SEC-02): a machine credential scoped to
        // a specific team can NEVER reach beyond it, even where the bound
        // principal is a member of other teams. The scope lives in the SIGNED
        // claim, so it cannot be tampered with in transit. A scoped token
        // requesting a different team is refused as not-allowed (403).
        if let Principal::Machine {
            scope_team_id: Some(scoped_team),
            ..
        } = principal
        {
            if *scoped_team != team.id {
                return Err(ServiceError::Forbidden);
            }
        }

        // Membership authz BEFORE any fetch — a non-member never sees the
        // board's issues (NFR: refuse without leaking data).
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

        let rows = store
            .list_issues_by_project(project.id)
            .await
            .map_err(|_| ServiceError::Internal)?;

        Ok(rows
            .into_iter()
            .map(|row| BoardIssue {
                key: issue_key(&key_prefix, row.number),
                number: row.number,
                title: row.title,
                state: row.state,
            })
            .collect())
    }

    /// Render the canonical `{PREFIX}-{N}` issue key, matching the format the
    /// HTML board produces via `foundry_core::IssueKey`. Falls back to a manual
    /// format only if the (allocator-guaranteed `>= 1`) number is ever invalid,
    /// so a bad row can never panic the read path.
    fn issue_key(key_prefix: &foundry_core::ProjectKey, number: i32) -> String {
        match u32::try_from(number)
            .ok()
            .and_then(|n| foundry_core::IssueKey::try_new(key_prefix, n).ok())
        {
            Some(key) => key.to_string(),
            None => format!("{}-{}", key_prefix.as_str(), number),
        }
    }
}

pub mod comments;
pub mod issues;
