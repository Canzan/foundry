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
    _store: &Store,
    _signer: &foundry_auth::MachineTokenSigner,
    _principal: &Principal,
    _input: MintInput,
) -> Result<MintedToken, ServiceError> {
    panic!("foundry_services::tokens::mint_token not yet implemented — RED scaffold (US-MT01/04)")
}

/// List the workspace's issued tokens (US-MT02, US-MT06).
///
/// 1. authz `is_workspace_admin` → false ⇒ `Forbidden`.
/// 2. read `list_machine_tokens(workspace_id)` (workspace-scoped, newest-first),
///    resolve `created_by` → `minted_by`, map each row to `TokenView` (NO value).
pub async fn list_tokens(
    _store: &Store,
    _principal: &Principal,
) -> Result<Vec<TokenView>, ServiceError> {
    panic!("foundry_services::tokens::list_tokens not yet implemented — RED scaffold (US-MT02/06)")
}

/// Revoke a machine token (US-MT03).
///
/// 1. authz `is_workspace_admin` → false ⇒ `Forbidden`.
/// 2. workspace isolation: `find_machine_token_by_jti(jti)` None OR
///    `row.workspace_id != principal.workspace_id()` ⇒ `NotFound` (non-enumerable).
/// 3. `revoke_machine_token(jti)` (idempotent re-stamp); `Ok(())`.
/// 4. effectiveness is the SHIPPED per-request denylist (no new refusal code).
pub async fn revoke_token(
    _store: &Store,
    _principal: &Principal,
    _jti: uuid::Uuid,
) -> Result<(), ServiceError> {
    panic!("foundry_services::tokens::revoke_token not yet implemented — RED scaffold (US-MT03)")
}
