# Step Skeletons — machine-token-admin-ux (DISTILL → DELIVER wiring)

What DELIVER must implement to flip each RED scaffold GREEN. Scaffolds created
THIS wave are listed first (with their RED markers), then the precise wiring per
DESIGN doc. Nothing here is speculative — every item traces to a DESIGN contract.

## Scaffolds created this wave (RED-ready, workspace compiles, existing suite green)

| File | Kind | RED marker | DELIVER replaces with |
|---|---|---|---|
| `crates/foundry-store/migrations/0008_machine_tokens_created_by.sql` | REAL migration (applied) | n/a (additive, nullable — harmless to existing tests) | nothing — it is the real forward-only migration (DD6/ADR-MT04). |
| `crates/foundry-services/src/tokens.rs` | RED scaffold module | `panic!("… RED scaffold")` + `// SCAFFOLD: true` | the three use-case bodies (mint/list/revoke). |
| `crates/foundry-services/src/lib.rs` | EXTEND | n/a | already wired — `pub mod tokens;` + 3 delegating `Services` methods. |
| `crates/foundry-app/src/admin_tokens.rs` | RED scaffold handlers | `501 Not Implemented` body + `// SCAFFOLD: true` | the 3 real handlers (GET index, POST mint, POST revoke). |
| `crates/foundry-app/src/lib.rs` | EXTEND | n/a | `AppState.machine_token_signer` field + `/admin/tokens` routes mounted under session+CSRF. |
| `crates/foundry-app/src/main.rs` | scaffold field `= None` | comment marker | EXTEND the self-test block to RETAIN the signer (see §Signer). |

Scaffold detection: `grep -rn "SCAFFOLD: true" crates/foundry-services/src crates/foundry-app/src`.

## 1. Signer in AppState (US-MT00, ADR-MT01/DD1, signer.md)

- **DONE (scaffold)**: `AppState.machine_token_signer: Option<Arc<MachineTokenSigner>>`
  added (non-cfg-gated; production field). All 6 construction sites set it
  (`None` everywhere except the issuer test harness, which sets the test signer).
- **DELIVER**: in `main.rs`, EXTEND the existing `if let Ok(signing_key) =
  std::env::var("MACHINE_TOKEN_SIGNING_KEY")` self-test block (main.rs ~199-216)
  so that on a SUCCESSFUL probe it RETAINS the parsed signer instead of dropping
  it, and threads it into the `AppState { … }` literal (currently
  `machine_token_signer: None`). Reuse `from_pkcs8_pem` + the SHIPPED
  `\n`-normalization (main.rs:171). The single added line is
  `Some(Arc::new(signer))` AFTER the probe passes (signer.md §"How it is
  loaded"). Absent key ⇒ `None` ⇒ verifier-only (graceful).
- **NOT exposed via `FromRef`** — the handler reads it from `State<AppState>`
  and passes it to `services.mint_token(signer, …)` (DD4).
- **Probe/readyz substrate check (optional, data-and-migration.md §Probe)**:
  extend `Store::probe`'s `machine_tokens` column-existence list to include
  `created_by` once 0008 ships, so a binary on a pre-0008 DB refuses at
  `/readyz` rather than failing the first mint.

## 2. `created_by` migration + repo (US-MT00, DD6/ADR-MT04, data-and-migration.md)

- **DONE (real migration)**: `0008_machine_tokens_created_by.sql` —
  `ALTER TABLE machine_tokens ADD COLUMN created_by UUID NULL REFERENCES
  users(id) ON DELETE SET NULL;` (nullable, forward-only, no backfill, SET NULL).
- **DELIVER — `insert_machine_token` gains `created_by`**: DISTILL deliberately
  KEPT the current 6-arg signature so the 5 existing call sites
  (`feature_a_programmatic.rs`, `machine_tokens_repo.rs` ×4) stay green. DELIVER
  adds a `created_by: uuid::Uuid` parameter (data-and-migration.md §Repo) and
  updates those call sites (the Feature-A seeding can pass the bound user's id;
  the repo tests pass any user id). The column is nullable but the NEW mint call
  site always supplies the acting admin (NFR-MT-SEC-06).
- **DELIVER — list read attribution (US-MT06)**: surface `minted_by` by either
  (a) `LEFT JOIN users u ON u.id = m.created_by` projecting `email_display`
  (recommended — `COALESCE(..., '—')` for NULL, matching
  `list_comments_for_issue`), or (b) per-row `find_user_email_by_id`.
  `MachineTokenRow` gains `created_by: Option<Uuid>`.

## 3. `foundry_services::tokens` use-cases (US-MT01/02/03/04/05, token-admin-services.md)

Replace the three `panic!` bodies with the ordered behaviour (DESIGN fixes the
contract; constants `MAX_TTL_DAYS=365`, `DEFAULT_TTL_DAYS=90` already declared):

- **`mint_token`**: authz (`is_workspace_admin` → `Forbidden`); TTL validation
  (`<=0` → `Validation{ttl_required}`; `> MAX` → `Validation{ttl_over_cap}` with
  "Maximum expiry is 365 days"; `== MAX` accepted); scope mapping (`Workspace`→
  None; `Team(t)`→ validate `t ∈ workspace` else
  `Validation{scope_team_not_in_workspace}`); claims; `signer.mint(&claims)?`;
  persist METADATA ONLY via `insert_machine_token(…, created_by =
  principal.user_id())`; return `MintedToken{ value, … }` (sign BEFORE persist;
  persist BEFORE returning the value — token-admin-services.md ordering note).
- **`list_tokens`**: authz; `list_machine_tokens(workspace_id)` (newest-first,
  workspace-scoped); resolve `minted_by`; map to `TokenView` (NO value).
- **`revoke_token`**: authz; `find_machine_token_by_jti(jti)` → None OR
  `row.workspace_id != principal.workspace_id()` ⇒ `NotFound` (non-enumerable);
  else `revoke_machine_token(jti)` (idempotent); `Ok(())`. Effectiveness is the
  SHIPPED per-request denylist (no new refusal code).

## 4. Admin handlers + view-models + templates (US-MT01..06, admin-routes.md)

- **DONE (scaffold)**: `admin_tokens::{show_index, submit_mint, submit_revoke}`
  return `501`; routes mounted in `build_router` under session + CSRF.
- **DELIVER — handlers** (follow `projects.rs` verbatim):
  - `signed_in_user(&session)` → redirect `/sign-in` if absent; resolve
    `workspace_id` from the session; `is_workspace_admin` gate → on false render
    a GENERIC 404 (non-enumerable, NFR-MT-SEC-03 — NOT a 403 that confirms the
    surface).
  - `show_index`: if `state.machine_token_signer.is_none()` render the "issuing
    not enabled" notice WITHOUT a mint form; else render `TokenListPage` (list +
    mint form). Empty workspace → inviting empty state.
  - `submit_mint`: signer-present pre-check; read the signer from `state`; parse
    the form (label, scope radio + team, ttl_days); call
    `services.mint_token(signer, principal, MintInput{…})`; on `Ok` render
    `TokenMintedPage` exposing `value.expose_secret()` EXACTLY ONCE (no redirect)
    then drop; on `Validation`/`Forbidden`/signer-absent re-render the form / a
    403-style page — NEVER a partial token (all-or-nothing, NFR-MT-REL-01, reuse
    the `force_board_render_failure` seam shape).
  - `submit_revoke`: admin pre-check; `services.revoke_token(principal, jti)`;
    303 → `GET /admin/tokens` (or htmx row-fragment swap to Revoked).
- **DELIVER — view-models (EXTEND `views.rs`)**: `TokenListPage`, `MintFormVm`,
  `TokenMintedPage` (the ONLY view-model carrying a value), `TokenRow` (NO value
  field), `TokenRevokeConfirm` — shapes in admin-routes.md §View-models.
- **DELIVER — templates** extend `base.html` like `project_create.html`.

### Template data-attribute contract (the acceptance assertions key off these)

The step assertions in `feature_machine_token_admin.rs` look for these stable
hooks (mirroring the Feature-B `data-*` marker idiom). DELIVER's templates MUST
emit them:

| Marker | Where | Asserted by |
|---|---|---|
| `[data-token-surface]` | root of the list page | "the token surface is shown" |
| `form[data-mint-form]` | the mint form | "no mint form is offered" (verifier-only) |
| `[data-token-value]` | the one-time value (mint page ONLY) | shown-once / never-again |
| `[data-copy-token]` | the copy affordance | shown-once |
| `[data-token-jti] / -label / -scope / -expiry` | one-time page + each row | metadata shown |
| `[data-token-row]` (+ `data-token-label`, `data-token-status="active\|revoked"`) | each list row | list count, row status, isolation |
| `[data-token-last-used]` | each row | US-MT06 last-used |
| copy text containing "only time" | mint page | only-time warning |
| copy text "not enabled" | verifier-only notice | issuing-not-enabled |
| copy "—" or "unknown" | a NULL-`created_by` row | US-MT06 unknown issuer |
| copy "never" | a never-used row | US-MT06 |
| empty-state copy "no tokens" / "issue one" | empty list | US-MT02 empty state |
| revoke confirm copy "immediate" / "cannot be undone" | confirm UI | US-MT03 warning |
| revoke-and-reissue guidance ("revoke" + "issue") | anxiety path | display-once |

(These are a SUGGESTED stable contract; if DELIVER prefers different hooks, update
the assertions in lockstep — the marker NAMES are the shared contract, like the
Feature-B `b_asset_*` / `data-hx-fragment` precedent.)

## 5. CSRF + status-code mapping

- `/admin/tokens` is mounted UNDER `csrf::csrf_middleware` → a mint/revoke POST
  without a valid `_csrf` is refused 403 BEFORE the handler (US-MT03 no-CSRF
  scenario already exercises this — it will go GREEN as soon as the route is
  real, since the middleware already runs).
- Validation refusals map to `422` (the mint label/TTL/scope scenarios assert
  422); non-admin + cross-workspace map to a generic `404` (non-enumerable).

## 6. Deferred / explicitly NOT done this wave

- `insert_machine_token` signature change (kept 6-arg to preserve green suite).
- The `Store::probe` `created_by` column check (one-line, optional hardening).
- A REAL second workspace for cross-workspace fixtures — blocked by the
  single-workspace schema (`upstream-issues.md` UI-1); modelled via synthetic
  foreign jti/team uuids.
- The JSON token-management API (`/api/v1/.../tokens`) — DISCUSS/DESIGN deferred
  it as a fast-follow; the `tokens` use-cases are shaped to make it a thin second
  adapter.
