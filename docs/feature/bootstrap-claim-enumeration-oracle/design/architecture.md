# Architecture — bootstrap-claim-enumeration-oracle

**Feature ID**: `bootstrap-claim-enumeration-oracle`
**Design mode**: propose (decision pinned during `/nw:new` intake)
**Inputs**: `../discuss/requirements.md`

## Design Decision (D1) — Atomic one-tx claim+create, token unconsumed on collision

Fold the two current store calls (`claim_bootstrap_token` → `create_initial_workspace`) into
ONE new atomic store method. On an email-uniqueness collision the whole transaction rolls back,
so the bootstrap token stays UNCONSUMED and the handler renders the uniform refusal — never a 500.

**Chosen over** the smaller "catch-23505-after-consume, single-shot" alternative because it
(a) also fixes the secondary defect (token burned on legitimate collision), and (b) is
structurally identical to the already-shipped, mutation-hardened `create_member_and_consume`,
so it inherits a proven pattern rather than inventing a weaker one.

## Component Changes

### Store (`crates/foundry-store/src/lib.rs`)

New method, modeled line-for-line on `create_member_and_consume` (lib.rs:411) and reusing the
guarded-UPDATE SQL already in `claim_bootstrap_token` (lib.rs:322):

```
pub async fn claim_bootstrap_and_create_workspace(
    &self,
    token_hash: &[u8],
    now: OffsetDateTime,
    /* workspace_id, workspace_name, user_id, email_lower, email_display,
       display_name, password_hash, team_id, ..., project_key_prefix */
) -> Result<BootstrapClaimOutcome, StoreError>
```

Transaction body (single `tx`):
1. **Guarded UPDATE** `bootstrap_tokens SET used_at=$now WHERE token_hash=$1 AND used_at IS NULL
   AND expires_at>$now RETURNING id` — 0 rows ⇒ `rollback` ⇒ `Refused` (the existing
   unknown/used/expired refusal, now inside the tx).
2. The six INSERTs currently in `create_initial_workspace` (workspaces, users, memberships,
   teams, team_memberships, projects, instance_admins seed).
3. The **users INSERT** is the collision point: catch SQLSTATE **23505 specifically** →
   `rollback` → `EmailCollision`. Any other error → propagate as `StoreError` (500 path).
4. Success ⇒ `commit` ⇒ `Consumed { workspace_id, user_id }`.

Outcome enum (mirrors `MemberConsumeOutcome`):
```
enum BootstrapClaimOutcome { Consumed { workspace_id, user_id }, Refused, EmailCollision }
```
**D1a**: `Refused` (bad token) and `EmailCollision` (email exists) both map to the SAME uniform
refusal in the handler — the enum distinguishes them only for tracing, never for the response.

The old `create_initial_workspace` becomes dead once the handler is rewired; per the repo's
"remove dead code pre-stable" policy it is deleted (not left inert) unless another caller exists
(verify via callers before deleting).

### Handler (`crates/foundry-app/src/bootstrap.rs`)

Replace the two-call sequence (claim → hash → create) with: hash password → call
`claim_bootstrap_and_create_workspace` → match:
- `Consumed { .. }` → insert session, `303 → /dashboard` (unchanged).
- `Refused | EmailCollision` → `bootstrap_refusal_page()` (the existing byte-identical refusal);
  log the distinguishing reason via `tracing` only.
- `Err(StoreError)` → existing `500` path (unchanged, for non-23505 errors).

Password hashing stays BEFORE the tx (no crypto inside the DB transaction), matching the current
ordering and `create_member_and_consume`'s handler.

## Non-Enumerability Argument (NFR-1)

Post-change, the four negative outcomes — unknown token, used token, expired token, colliding
email — all return `bootstrap_refusal_page()` with identical status + body. The only observable
difference (success vs refusal) tracks whether a *valid unused token* met a *fresh email*, which
is the intended semantics, not an email-existence oracle.

## Risk / Verification Notes for DISTILL

- **HIGH-risk arm**: the 23505 catch must be narrowed to the users insert; a broadened catch
  could mis-map an FK/connection error to a refusal (masking a real 500). Acceptance + mutation
  must pin this.
- Store-scope mutation testing should target the new method (the member-invites feature required
  100% store-scope kill; hold the same bar).
- Regression scenarios: existing unknown/used/expired refusal, and the happy-path seed
  (workspace + instance_admins) must stay green.

## DELIVER Entry Conditions (from DISTILL review — Sentinel APPROVED, Atlas CONDITIONALLY APPROVED)

DISTILL is approved; acceptance tests are genuine E2E RED through the real `/bootstrap` handler
(this satisfied Atlas's "must be E2E not store-unit" condition). Two conditions carry into DELIVER:

1. **Narrow-catch mutation bar is a HARD gate** (not a "should"): a mutant that broadens the
   23505 catch (e.g. catches any `DatabaseError`) MUST die under store-scope mutation testing.
   Hold the member-invites bar (100% store-scope kill on the new method).
2. **`create_initial_workspace` is NOT trivially deletable — it has ~9 callers (mostly tests)**
   (reviewer-counted: provision_workspace_store.rs, bootstrap_claim_seeds_superadmin.rs,
   revoke_and_list_use_cases.rs, machine_tokens_repo.rs, write_use_cases.rs, mint_token_use_case.rs,
   provision_workspace_use_case.rs, board_use_case.rs, feature_mwt_slice_06_provision_and_prove.rs).
   Before removing it per the remove-dead-code policy, DELIVER must first re-verify the live caller
   set, then either migrate callers to `claim_bootstrap_and_create_workspace` / a shared seed helper,
   or KEEP `create_initial_workspace` if callers still legitimately need the non-claim seed path.
   Do not delete blind.

DELIVER implementation sequence: (a) implement the `claim_bootstrap_and_create_workspace` tx body
(guarded UPDATE → six seed INSERTs → 23505-narrow-catch on the users INSERT → commit); (b) rewire
the `POST /bootstrap` handler to call it and match `Consumed → 303` / `Refused | EmailCollision →
bootstrap_refusal_page()` / `Err → 500`; (c) resolve the `create_initial_workspace` caller question
above; (d) store-scope mutation pass as the hard gate.
