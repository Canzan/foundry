//! foundry-services — the shared application-service seam.
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
//! Both adapters hold a [`Services`] handle (which owns the single
//! `Arc<Store>`) and call the use-cases as methods — they never name
//! `foundry_store::Store` themselves, which is what makes the
//! `foundry-api ⊀ foundry-store` `cargo-deny` ban (boundary-guard.md LAYER 2)
//! satisfiable on the real tree.

#![forbid(unsafe_code)]

use std::sync::Arc;

use foundry_store::Store;

/// The shared application-service handle both adapter crates hold.
///
/// This is the boundary seam (ADR-W04 / boundary-guard.md LAYER 2): the
/// adapter crates (`foundry-api`, and Feature B's `foundry-web`) hold a
/// `Services` — they NEVER name `foundry_store::Store` themselves, so the
/// `cargo-deny` `foundry-api ⊀ foundry-store` ban is satisfiable on the real
/// tree. `Services` owns the single `Arc<Store>` and exposes the use-cases as
/// methods that delegate to the free functions below; the only `Store`-typed
/// surface lives INSIDE this crate.
#[derive(Clone)]
pub struct Services {
    store: Arc<Store>,
}

impl Services {
    /// Wrap the composition root's shared `Arc<Store>`. Built once at boot in
    /// `foundry-app`'s composition and handed to both adapters via `FromRef`.
    pub fn new(store: Arc<Store>) -> Self {
        Services { store }
    }

    /// US-W05a board read — delegates to [`board::list_board_issues`].
    pub async fn list_board_issues(
        &self,
        principal: &Principal,
        team_slug: &str,
        project_slug: &str,
    ) -> Result<Vec<BoardIssue>, ServiceError> {
        board::list_board_issues(&self.store, principal, team_slug, project_slug).await
    }

    /// US-W05c create-issue — delegates to [`issues::create_issue`].
    pub async fn create_issue(
        &self,
        principal: &Principal,
        team_slug: &str,
        project_slug: &str,
        title: &str,
        description: &str,
    ) -> Result<CreatedIssue, ServiceError> {
        issues::create_issue(
            &self.store,
            principal,
            team_slug,
            project_slug,
            title,
            description,
        )
        .await
    }

    /// US-W05c change-state — delegates to [`issues::change_issue_state`]. The
    /// optional `after` neighbour key carries the board drop's target rank
    /// (card-ranking-within-status, ADR-002); the JSON API path passes `None`.
    pub async fn change_issue_state(
        &self,
        principal: &Principal,
        team_slug: &str,
        project_slug: &str,
        number: i32,
        new_state: &str,
        after: Option<&str>,
    ) -> Result<BoardIssue, ServiceError> {
        issues::change_issue_state(
            &self.store,
            principal,
            team_slug,
            project_slug,
            number,
            new_state,
            after,
        )
        .await
    }

    /// US-W05c create-comment — delegates to [`comments::create_comment`].
    pub async fn create_comment(
        &self,
        principal: &Principal,
        team_slug: &str,
        project_slug: &str,
        issue_number: i32,
        body: &str,
    ) -> Result<comments::CommentView, ServiceError> {
        comments::create_comment(
            &self.store,
            principal,
            team_slug,
            project_slug,
            issue_number,
            body,
        )
        .await
    }

    /// US-W05c edit-comment — delegates to [`comments::edit_comment`].
    #[allow(clippy::too_many_arguments)]
    pub async fn edit_comment(
        &self,
        principal: &Principal,
        team_slug: &str,
        project_slug: &str,
        issue_number: i32,
        comment_id: uuid::Uuid,
        new_body: &str,
    ) -> Result<comments::CommentView, ServiceError> {
        comments::edit_comment(
            &self.store,
            principal,
            team_slug,
            project_slug,
            issue_number,
            comment_id,
            new_body,
        )
        .await
    }

    /// issue-change-history program feed (US-03) — delegates to
    /// [`issues::list_issue_change_history`]. Membership-authz gated then reads
    /// the SAME `list_issue_changes` rows the human timeline renders, oldest→
    /// newest (ADR-002 §2). A foreign/absent issue is refused non-enumerably.
    pub async fn list_issue_change_history(
        &self,
        principal: &Principal,
        team_slug: &str,
        project_slug: &str,
        number: i32,
    ) -> Result<Vec<issues::IssueChangeHistoryEntry>, ServiceError> {
        issues::list_issue_change_history(&self.store, principal, team_slug, project_slug, number)
            .await
    }

    /// US-W05b per-request `jti` denylist read — delegates to
    /// [`auth::resolve_active_token`]. The ONLY store touch the token-auth
    /// extractor needs, routed through `Services` so `foundry-api` never names
    /// `foundry_store::Store` for it.
    pub async fn resolve_active_token(
        &self,
        jti: uuid::Uuid,
        now: time::OffsetDateTime,
    ) -> Result<auth::ActiveMachineToken, ServiceError> {
        auth::resolve_active_token(&self.store, jti, now).await
    }

    /// US-MT01/US-MT04 mint a machine token — delegates to
    /// [`tokens::mint_token`]. The signer is PASSED IN (DD4), never stored in
    /// `Services`; it is confined to the mint call path.
    pub async fn mint_token(
        &self,
        signer: &foundry_auth::MachineTokenSigner,
        principal: &Principal,
        input: tokens::MintInput,
    ) -> Result<tokens::MintedToken, ServiceError> {
        tokens::mint_token(&self.store, signer, principal, input).await
    }

    /// US-MT02/US-MT06 list the workspace's issued tokens — delegates to
    /// [`tokens::list_tokens`]. No value field on the returned views.
    pub async fn list_tokens(
        &self,
        principal: &Principal,
    ) -> Result<Vec<tokens::TokenView>, ServiceError> {
        tokens::list_tokens(&self.store, principal).await
    }

    /// US-MT03 revoke a machine token — delegates to [`tokens::revoke_token`].
    /// Workspace-isolated, idempotent; effectiveness is the SHIPPED denylist.
    pub async fn revoke_token(
        &self,
        principal: &Principal,
        jti: uuid::Uuid,
    ) -> Result<(), ServiceError> {
        tokens::revoke_token(&self.store, principal, jti).await
    }

    /// multi-workspace-provisioning (US-MWT07, ADR-002/003) — provision a NEW
    /// isolated workspace + its first admin, gated by the INSTANCE super-admin
    /// authority. Delegates to [`provisioning::provision_workspace`].
    pub async fn provision_workspace(
        &self,
        request: provisioning::ProvisionRequest<'_>,
    ) -> Result<provisioning::Provisioned, ServiceError> {
        provisioning::provision_workspace(&self.store, request).await
    }
}

/// multi-workspace-provisioning use-case (US-MWT07, ADR-002/003).
///
/// The operator-CLI driving port (`foundry doctor provision-workspace`) calls
/// this; it is the single trusted place the `is_instance_admin` authz gate and
/// the atomic provision transaction are composed. Kept inline in this crate's
/// orchestration seam (alongside the `board` use-case) rather than a sibling
/// module — it is one fail-closed gate + one delegating call, with the same
/// `Store`-typed surface the seam already owns.
pub mod provisioning {
    use super::{ServiceError, Store};
    use secrecy::SecretString;

    /// What the operator asks for: a NEW workspace `name` with a first admin at
    /// `admin_email`, provisioned by the super-admin identified by
    /// `acting_user_id` (already resolved from `--as`/the bootstrap claim).
    pub struct ProvisionRequest<'a> {
        pub acting_user_id: uuid::Uuid,
        pub workspace_name: &'a str,
        pub admin_email: &'a str,
        /// Initial credential the operator sets for the first admin. The first
        /// admin can reset it by accepting the emitted invite link.
        pub admin_password: SecretString,
        /// When the emitted first-admin invite expires.
        pub invite_expires_at: time::OffsetDateTime,
    }

    /// The observable outcome the CLI reports: the new workspace identity + the
    /// invite row to sign into the first-admin invite link.
    pub struct Provisioned {
        pub workspace_id: uuid::Uuid,
        pub admin_user_id: uuid::Uuid,
        pub invite_id: uuid::Uuid,
        pub invite_expires_at: time::OffsetDateTime,
    }

    /// Provision a new isolated workspace + first admin, FAIL-CLOSED on the
    /// instance super-admin gate.
    ///
    /// - `acting_user_id` is NOT a super-admin ⇒ [`ServiceError::Forbidden`]
    ///   (the CLI maps this to a "not authorized" exit; the refusal is
    ///   observationally independent of whether the target already exists).
    /// - otherwise the workspace + admin + invite commit atomically (the store
    ///   transaction mirrors `create_initial_workspace`).
    pub async fn provision_workspace(
        store: &Store,
        request: ProvisionRequest<'_>,
    ) -> Result<Provisioned, ServiceError> {
        let is_admin = store
            .is_instance_admin(request.acting_user_id)
            .await
            .map_err(|_| ServiceError::Internal)?;
        if !is_admin {
            return Err(ServiceError::Forbidden);
        }

        let admin_email_lower = request.admin_email.to_ascii_lowercase();
        let password_hash = foundry_auth::hash_password(&request.admin_password)
            .await
            .map_err(|_| ServiceError::Internal)?;

        let workspace_id = uuid::Uuid::now_v7();
        let admin_user_id = uuid::Uuid::now_v7();
        let invite_id = uuid::Uuid::now_v7();

        store
            .provision_workspace(
                workspace_id,
                request.workspace_name,
                admin_user_id,
                &admin_email_lower,
                request.admin_email,
                "Workspace Admin",
                &password_hash,
                invite_id,
                request.invite_expires_at,
            )
            .await
            .map_err(|_| ServiceError::Internal)?;

        Ok(Provisioned {
            workspace_id,
            admin_user_id,
            invite_id,
            invite_expires_at: request.invite_expires_at,
        })
    }
}

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
    /// The TRIMMED title actually persisted (NFR-WEB-API-CON-02). The create
    /// response must echo this — not the raw request title — so the returned
    /// representation equals what a subsequent read returns.
    pub title: String,
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
pub mod projects;
pub mod tokens;
