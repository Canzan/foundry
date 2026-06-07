//! `foundry_services::tokens` — the machine-token admin use-cases
//! (mint / list / revoke) for the `machine-token-admin-ux` feature.
//!
//! RED scaffold (DISTILL, Mandate 7 / ADR-025):
//! These are the contract signatures DESIGN fixed in
//! `docs/feature/machine-token-admin-ux/design/token-admin-services.md`. Each
//! body `panic!`s with a `RED scaffold` marker so a test reaching it fails for
//! MISSING_FUNCTIONALITY, not a compile/import error. DELIVER replaces the
//! bodies with the ordered behaviour (authz → TTL validation → scope mapping →
//! claims → sign → persist METADATA ONLY → return the one-time value).
//!
//! Why a neutral service, not a handler: this is the seam Feature A established
//! (`board`/`issues`/`comments`) — authz (`is_workspace_admin`), claims
//! construction, scope mapping, TTL validation, and the one-time-secret return
//! must be identical for the web UI now and a JSON token API later, and the
//! adapter must not name `foundry_store::Store` (boundary guard). The signer is
//! PASSED to `mint_token` (DD4), never stored in `Services`.
//!
//! SCAFFOLD: true

use crate::{Principal, ServiceError};
use foundry_store::Store;

/// Server cap on token lifetime (DD8 / OD4-RATIFIED — "e.g. 1y").
pub const MAX_TTL_DAYS: i64 = 365;
/// Sane default rotation cadence when the admin does not pick (DD8 / OD4).
pub const DEFAULT_TTL_DAYS: i64 = 90;

/// The admin's scope choice (DD9). `Workspace` maps to `scope_team_id = None`;
/// `Team(id)` is validated to belong to the acting workspace before minting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeChoice {
    Workspace,
    Team(uuid::Uuid),
}

/// Neutral mint input (never HTML, never JSON — like `BoardIssue`).
#[derive(Debug, Clone)]
pub struct MintInput {
    pub label: String,
    pub scope: ScopeChoice,
    /// REQUIRED (admin-chosen, validated against `MAX_TTL_DAYS`). OD4: no
    /// never-expires option in v1.
    pub ttl_days: i64,
}

/// The result of a successful mint. `value` is the ONE-TIME token value — it is
/// rendered exactly once and dropped (DD7 / NFR-MT-SEC-01). It is NEVER stored,
/// logged, or returned by any read path.
pub struct MintedToken {
    pub value: secrecy::SecretString,
    pub jti: uuid::Uuid,
    pub label: String,
    pub scope_team_id: Option<uuid::Uuid>,
    pub expires_at: time::OffsetDateTime,
}

/// A registry row projected for the list view. There is deliberately NO `value`
/// field (NFR-MT-SEC-02) — no surface ever re-displays a token value.
#[derive(Debug, Clone)]
pub struct TokenView {
    pub jti: uuid::Uuid,
    pub label: String,
    pub scope_team_id: Option<uuid::Uuid>,
    pub expires_at: time::OffsetDateTime,
    pub revoked: bool,
    pub last_used_at: Option<time::OffsetDateTime>,
    /// Resolved `created_by` display name; `None` renders as "—" (deleted admin
    /// or a pre-feature row, US-MT06 edge path).
    pub minted_by: Option<String>,
}

/// Mint a machine token (US-MT01, US-MT04).
///
/// Ordered behaviour DELIVER implements (token-admin-services.md):
/// 1. authz `is_workspace_admin` → false ⇒ `Forbidden` (US-MT05).
/// 2. TTL validation: `ttl_days <= 0` ⇒ `Validation{code:"ttl_required"}`;
///    `ttl_days > MAX_TTL_DAYS` ⇒ `Validation{code:"ttl_over_cap"}` (cap stated);
///    `ttl_days == MAX_TTL_DAYS` accepted.
/// 3. scope mapping (DD9): `Workspace`⇒None; `Team(t)`⇒validate `t` belongs to
///    the acting workspace, else `Validation{code:"scope_team_not_in_workspace"}`.
/// 4. claims (`sub`/`scope`/`iat`/`exp`/`jti`).
/// 5. sign via `signer.mint(&claims)` → `SecretString` (AuthError ⇒ Internal).
/// 6. persist METADATA ONLY via `insert_machine_token(..., created_by)` — the
///    token VALUE is never passed to the store.
/// 7. return `MintedToken` (the value travels to the handler, renders once, drops).
pub async fn mint_token(
    store: &Store,
    signer: &foundry_auth::MachineTokenSigner,
    principal: &Principal,
    input: MintInput,
) -> Result<MintedToken, ServiceError> {
    // 1. Authz — the load-bearing admin-only gate (US-MT05). A store error
    //    fails closed as Forbidden (no token is minted when authz can't be
    //    confirmed).
    let is_admin = store
        .is_workspace_admin(principal.workspace_id(), principal.user_id())
        .await
        .map_err(|_| ServiceError::Internal)?;
    if !is_admin {
        return Err(ServiceError::Forbidden);
    }

    // 2. TTL validation (DD8 / OD4) — required and bounded by the server cap.
    if input.ttl_days <= 0 {
        return Err(ServiceError::Validation {
            code: "ttl_required".to_string(),
            message: "An expiry is required".to_string(),
        });
    }
    if input.ttl_days > MAX_TTL_DAYS {
        return Err(ServiceError::Validation {
            code: "ttl_over_cap".to_string(),
            message: format!("Maximum expiry is {MAX_TTL_DAYS} days"),
        });
    }

    // 3. Scope mapping (DD9) — Workspace ⇒ None; Team(t) is valid only when t
    //    belongs to the acting workspace, else a non-leaking Validation refusal.
    let scope_team_id = match input.scope {
        ScopeChoice::Workspace => None,
        ScopeChoice::Team(team_id) => {
            let belongs = store
                .team_belongs_to_workspace(team_id, principal.workspace_id())
                .await
                .map_err(|_| ServiceError::Internal)?;
            if !belongs {
                return Err(ServiceError::Validation {
                    code: "scope_team_not_in_workspace".to_string(),
                    message: "The chosen team is not in this workspace".to_string(),
                });
            }
            Some(team_id)
        }
    };

    // 4. Claims — fresh jti, bound to the acting principal, expiring after the
    //    validated TTL. iss/aud are re-stamped to the pinned constants by
    //    `mint`, but default them here for completeness.
    let now = time::OffsetDateTime::now_utc();
    let expires_at = now + time::Duration::days(input.ttl_days);
    let jti = uuid::Uuid::now_v7();
    let claims = foundry_auth::MachineTokenClaims {
        sub: principal.user_id(),
        scope: scope_team_id,
        iat: now.unix_timestamp(),
        exp: expires_at.unix_timestamp(),
        jti,
        iss: foundry_auth::MACHINE_TOKEN_ISS.to_string(),
        aud: foundry_auth::MACHINE_TOKEN_AUD.to_string(),
    };

    // 5. Sign BEFORE persist — a sign failure leaves no orphan registry row. A
    //    mint failure is mapped to Internal; the key/value are never logged.
    let value = signer.mint(&claims).map_err(|_| ServiceError::Internal)?;

    // 6. Persist METADATA ONLY — the token VALUE is never passed to the store
    //    (the registry has no value/secret column, NFR-MT-SEC-02). `created_by`
    //    records the acting admin (NFR-MT-SEC-06). A persist failure after a
    //    successful sign returns Internal and does NOT hand back the value (an
    //    unpersisted token is unusable — the denylist never knew it).
    store
        .insert_machine_token(
            jti,
            principal.user_id(),
            principal.workspace_id(),
            scope_team_id,
            expires_at,
            &input.label,
            principal.user_id(),
        )
        .await
        .map_err(|_| ServiceError::Internal)?;

    // 7. Return the one-time value — it travels to the handler, renders once,
    //    and drops (DD7).
    Ok(MintedToken {
        value,
        jti,
        label: input.label,
        scope_team_id,
        expires_at,
    })
}

/// List the workspace's issued tokens (US-MT02, US-MT06).
///
/// 1. authz `is_workspace_admin` → false ⇒ `Forbidden`.
/// 2. read `list_machine_tokens(workspace_id)` (workspace-scoped, newest-first),
///    resolve `created_by` → `minted_by`, map each row to `TokenView` (NO value).
pub async fn list_tokens(
    store: &Store,
    principal: &Principal,
) -> Result<Vec<TokenView>, ServiceError> {
    // 1. Authz — the admin-only gate (US-MT05). A store error fails closed.
    let is_admin = store
        .is_workspace_admin(principal.workspace_id(), principal.user_id())
        .await
        .map_err(|_| ServiceError::Internal)?;
    if !is_admin {
        return Err(ServiceError::Forbidden);
    }

    // 2. Read — workspace-scoped, newest-first (the store ORDERs by created_at
    //    DESC). Map each metadata row to a value-free `TokenView`, resolving the
    //    minting admin's display name (`created_by` is recorded as the bound
    //    user in slice 1, so the per-row `user_id` resolves the issuer); a
    //    deleted/legacy author resolves to `None`, rendered as "—" (US-MT06).
    let rows = store
        .list_machine_tokens(principal.workspace_id())
        .await
        .map_err(|_| ServiceError::Internal)?;
    let mut views = Vec::with_capacity(rows.len());
    for row in rows {
        let minted_by = store
            .find_user_email_by_id(row.user_id)
            .await
            .map_err(|_| ServiceError::Internal)?;
        views.push(TokenView {
            jti: row.jti,
            label: row.label,
            scope_team_id: row.scope_team_id,
            expires_at: row.expires_at,
            revoked: row.revoked_at.is_some(),
            last_used_at: row.last_used_at,
            minted_by,
        });
    }
    Ok(views)
}

/// Revoke a machine token (US-MT03).
///
/// 1. authz `is_workspace_admin` → false ⇒ `Forbidden`.
/// 2. workspace isolation: `find_machine_token_by_jti(jti)` None OR
///    `row.workspace_id != principal.workspace_id()` ⇒ `NotFound` (non-enumerable).
/// 3. `revoke_machine_token(jti)` (idempotent re-stamp); `Ok(())`.
/// 4. effectiveness is the SHIPPED per-request denylist (no new refusal code).
pub async fn revoke_token(
    store: &Store,
    principal: &Principal,
    jti: uuid::Uuid,
) -> Result<(), ServiceError> {
    // 1. Authz — the admin-only gate (US-MT05). A store error fails closed.
    let is_admin = store
        .is_workspace_admin(principal.workspace_id(), principal.user_id())
        .await
        .map_err(|_| ServiceError::Internal)?;
    if !is_admin {
        return Err(ServiceError::Forbidden);
    }

    // 2. Workspace isolation, NON-ENUMERABLE (NFR-MT-REL-03 / NFR-MT-SEC-03): a
    //    jti that does not exist AND a jti that belongs to ANOTHER workspace
    //    yield the SAME `NotFound` — the response never confirms whether the
    //    credential exists. No oracle leaks across the workspace boundary.
    let row = store
        .find_machine_token_by_jti(jti)
        .await
        .map_err(|_| ServiceError::Internal)?;
    let belongs = matches!(row, Some(ref r) if r.workspace_id == principal.workspace_id());
    if !belongs {
        return Err(ServiceError::NotFound);
    }

    // 3. Flip `revoked_at` (idempotent re-stamp — re-revoking an already-revoked
    //    token only refreshes the timestamp). Effectiveness is the SHIPPED
    //    per-request denylist: the `/api/v1` `token_auth` extractor already
    //    refuses any token whose `revoked_at` is set, so the kill-switch takes
    //    effect on the credential's very next call with no new refusal code.
    store
        .revoke_machine_token(jti)
        .await
        .map_err(|_| ServiceError::Internal)?;
    Ok(())
}
