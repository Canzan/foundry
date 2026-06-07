# Token-Admin Services — `foundry_services::tokens`

DESIGN (Propose) for the use-case seam: the three functions that orchestrate mint/list/revoke. They
hold authz, claims construction, scope mapping, TTL validation, and the one-time-secret return.
Decisions: ADR-MT02, DD3/DD4/DD8/DD9/DD10 (`wave-decisions.md`). This document fixes the CONTRACTS;
the crafter owns the bodies.

## Why this is a service, not a handler

Feature A established `foundry-services` as the single acyclic home for use-case orchestration that
both adapters share (ADR-W04/W07; `foundry-services/src/lib.rs:1-17`). Mint/list/revoke carry authz
+ validation that MUST be identical for the web UI now and a JSON token API later, and the adapter
must not name `foundry_store::Store` (boundary guard). So they live in a NEW
`foundry_services::tokens` module beside `board`/`issues`/`comments`, with thin `Services` methods
delegating to free functions — exactly the shape of `board::list_board_issues` (lib.rs:283-355).

## Module shape

```
// foundry-services/src/lib.rs — register the module (beside comments, issues)
pub mod tokens;

// Services methods (delegating, like list_board_issues / create_issue):
impl Services {
    pub async fn mint_token(
        &self,
        signer: &foundry_auth::MachineTokenSigner,   // DD4: passed in, not stored in Services
        principal: &Principal,
        input: tokens::MintInput,
    ) -> Result<tokens::MintedToken, ServiceError> { tokens::mint_token(&self.store, signer, principal, input).await }

    pub async fn list_tokens(
        &self, principal: &Principal,
    ) -> Result<Vec<tokens::TokenView>, ServiceError> { tokens::list_tokens(&self.store, principal).await }

    pub async fn revoke_token(
        &self, principal: &Principal, jti: uuid::Uuid,
    ) -> Result<(), ServiceError> { tokens::revoke_token(&self.store, principal, jti).await }
}
```

`Services` itself is UNCHANGED in its construction (`FromRef` from `Arc<Store>`, lib.rs:153) — the
signer is threaded through `mint_token`'s parameter only (DD4).

## Constants (DD8)

```
// foundry-services::tokens
pub const MAX_TTL_DAYS: i64 = 365;     // server cap (ratified "e.g. 1y")
pub const DEFAULT_TTL_DAYS: i64 = 90;  // sane default rotation cadence
```

(OD4 numbers — the one place the user may most want to override.)

## DTOs (neutral — never HTML, never JSON, like `BoardIssue`)

```
pub struct MintInput {
    pub label: String,
    pub scope: ScopeChoice,        // Workspace | Team(uuid::Uuid)
    pub ttl_days: i64,             // REQUIRED (admin-chosen, validated against the cap)
}
pub enum ScopeChoice { Workspace, Team(uuid::Uuid) }

pub struct MintedToken {
    pub value: secrecy::SecretString,  // the ONE-TIME token value — drop after render (DD7)
    pub jti: uuid::Uuid,
    pub label: String,
    pub scope_team_id: Option<uuid::Uuid>,
    pub expires_at: time::OffsetDateTime,
}

pub struct TokenView {                 // NO value field (NFR-MT-SEC-02)
    pub jti: uuid::Uuid,
    pub label: String,
    pub scope_team_id: Option<uuid::Uuid>,
    pub expires_at: time::OffsetDateTime,
    pub revoked: bool,
    pub last_used_at: Option<time::OffsetDateTime>,
    pub minted_by: Option<String>,     // resolved created_by display name; None -> "—"
}
```

## `mint_token` contract (US-MT01, US-MT04)

```
pub async fn mint_token(
    store: &Store,
    signer: &MachineTokenSigner,
    principal: &Principal,
    input: MintInput,
) -> Result<MintedToken, ServiceError>
```

Ordered behaviour (the crafter writes the body; this is the contract):
1. **Authz**: `store.is_workspace_admin(principal.workspace_id(), principal.user_id())` → false ⇒
   `ServiceError::Forbidden` (US-MT05). (The handler ALSO pre-checks; this is the load-bearing one.)
2. **TTL validation**: `ttl_days <= 0` ⇒ `Validation { code:"ttl_required", … }`; `ttl_days >
   MAX_TTL_DAYS` ⇒ `Validation { code:"ttl_over_cap", message:"Maximum expiry is 365 days" }`
   (US-MT04 scenario 2 — cap stated). Boundary `ttl_days == MAX_TTL_DAYS` is accepted (scenario 2
   edge "at the cap").
3. **Scope mapping** (DD9): `ScopeChoice::Workspace` ⇒ `scope_team_id = None`;
   `ScopeChoice::Team(t)` ⇒ validate `t` belongs to `principal.workspace_id()` (reuse a team lookup,
   e.g. resolve the team and confirm its `workspace_id`) — a foreign team ⇒
   `Validation { code:"scope_team_not_in_workspace", … }` (US-MT04 scenario 3 evil-user path);
   `scope_team_id = Some(t)`.
4. **Claims construction**: `MachineTokenClaims { sub: principal.user_id(), scope: scope_team_id,
   iat: now, exp: now + ttl, jti: Uuid::now_v7(), iss/aud: defaulted (mint stamps the pinned
   constants anyway, foundry-auth:124) }`.
5. **Sign**: `signer.mint(&claims)? -> SecretString` (map `AuthError` ⇒ `ServiceError::Internal`,
   mint failure is never a token-value leak).
6. **Persist METADATA ONLY**: `store.insert_machine_token(jti, sub, workspace_id, scope_team_id,
   exp, &label, created_by = principal.user_id())` (data-and-migration.md). The token VALUE is NOT
   passed to the store.
7. **Return** `MintedToken { value, jti, label, scope_team_id, expires_at }`. The `SecretString`
   travels to the handler, is rendered ONCE, and drops (DD7).

Ordering note: sign BEFORE persist so a sign failure leaves no orphan row; persist BEFORE returning
the value so a persist failure never hands out a token the registry doesn't know about (so the
denylist could never revoke it). If persist fails after a successful sign, return
`ServiceError::Internal` and DO NOT return the value — the unpersisted token is unusable anyway (no
registry row ⇒ `resolve_active_token` returns `Unauthorized`, fail-closed, foundry-services:256).

## `list_tokens` contract (US-MT02, US-MT06)

```
pub async fn list_tokens(store: &Store, principal: &Principal) -> Result<Vec<TokenView>, ServiceError>
```
1. **Authz**: `is_workspace_admin` → false ⇒ `Forbidden` (US-MT05).
2. **Read**: `store.list_machine_tokens(principal.workspace_id())` — workspace-scoped, newest-first
   already (foundry-store:1441; NFR-MT-REL-03). Resolve `created_by` → `minted_by` name (LEFT JOIN
   or per-row lookup, data-and-migration.md). Map each row to `TokenView` (NO value).

## `revoke_token` contract (US-MT03)

```
pub async fn revoke_token(store: &Store, principal: &Principal, jti: uuid::Uuid) -> Result<(), ServiceError>
```
1. **Authz**: `is_workspace_admin` → false ⇒ `Forbidden`.
2. **Workspace isolation**: `store.find_machine_token_by_jti(jti)` → `None` OR
   `row.workspace_id != principal.workspace_id()` ⇒ `ServiceError::NotFound` (non-enumerable —
   US-MT03 scenario 3; the caller is never told whether the token exists elsewhere, NFR-MT-REL-03 +
   NFR-MT-SEC-03).
3. **Revoke**: `store.revoke_machine_token(jti)` (flips `revoked_at = now()`, idempotent —
   re-revoking just re-stamps, NFR-MT-REL-02). Return `Ok(())`.
4. **Effectiveness**: NONE of this is new refusal code. The SHIPPED per-request denylist
   (`resolve_active_token`, foundry-services:256) refuses the `jti` on its NEXT `/api/v1` call
   (NFR-MT-SEC-05). Reuse, don't rebuild (DD10).

## Testability

All three are `async fn(&Store, …)` — exactly the `board::list_board_issues` shape, unit/integration
testable against a real store without a running server (the seam's whole point). The
admin-only invariant (US-MT05), the TTL cap (US-MT04), and the cross-workspace non-enumerability
(US-MT03) are assertable here, not just through the web surface — which is what makes the SECURITY
NFRs structurally verifiable.

## Architecture enforcement (principle 11)

No new crate edge: `foundry-services` already depends on `foundry-store` and `foundry-auth`; this
module adds nothing the existing `xtask check-arch` + `cargo-deny` boundary guard doesn't already
cover. The one feature-specific static assertion worth keeping in the migration-review gate: "no
`machine_tokens` migration adds a token/secret/hash column" (NFR-MT-DATA-02).
