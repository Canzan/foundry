# Admin Routes — the web admin surface

DESIGN (Propose) for the browser admin surface. Route paths, authz gating, session + CSRF posture,
and the mint → one-time-display → list → revoke flow. Decisions: ADR-MT03, DD5/DD9 (`wave-decisions.md`).

## Route surface (OD3 — RECOMMENDED: `/admin/tokens`, workspace-implicit)

Mounted in `build_router` ALONGSIDE the existing HTML routes (so they sit under
`csrf::csrf_middleware` + `session_layer`, `foundry-app/src/lib.rs:307-311`) — NOT merged via
`foundry_api::routes` (that mount is CSRF-exempt + session-free, lib.rs:321).

| Method + Path | Handler | Story | Renders |
|---|---|---|---|
| `GET /admin/tokens` | `admin_tokens::show_index` | US-MT02, US-MT06 | `TokenListPage` (list + mint form, or "issuing not enabled" notice) |
| `POST /admin/tokens` | `admin_tokens::submit_mint` | US-MT01, US-MT04 | `TokenMintedPage` (one-time value) on success; mint form re-render with error on validation failure |
| `POST /admin/tokens/{jti}/revoke` | `admin_tokens::submit_revoke` | US-MT03 | redirect 303 → `GET /admin/tokens` (htmx: row-fragment swap to Revoked) |

The workspace is **implicit from the session** (`SessionUser.workspace_id`, `bootstrap.rs:26`) — the
slice-1 model binds exactly one workspace per session, so a `{ws}` path segment would be redundant
and invite a cross-workspace mismatch. The deferred JSON API SHOULD use the explicit
`/api/v1/workspaces/{ws}/tokens` form (programmatic, may act cross-workspace) — noted, not built.

`{jti}` on revoke is a `Uuid` path param; the service re-checks it belongs to the acting workspace
before flipping (US-MT03 scenario 3 / NFR-MT-REL-03) — a foreign `jti` is a non-enumerable
not-found, identical to how `find_comment_by_id` scopes by workspace (`foundry-store:996`).

## Authz gating (US-MT05, NFR-MT-SEC-03)

Two layers (defense in depth), both reusing `is_workspace_admin` (`foundry-store:1031`):

1. **Handler pre-check** (mirrors `comments.rs:105-112`): `signed_in_user(&session)` → if absent,
   redirect `/sign-in`; resolve `workspace_id` from the session; call
   `state.store.is_workspace_admin(workspace_id, user_id)` → if false, render a **non-enumerable
   refusal** (a generic 404 "Not found" page, NOT a 403 that confirms the surface exists — matches
   the non-enumerable posture the codebase uses for cross-workspace and the sign-in error). So a
   non-admin cannot even tell `/admin/tokens` exists.
2. **Use-case authz** (DD3, token-admin-services.md): `mint_token`/`list_tokens`/`revoke_token` ALSO
   call `is_workspace_admin` and return `ServiceError::Forbidden`/`NotFound` — so authz holds even
   if a future adapter forgets the handler pre-check. This is the load-bearing guarantee for
   US-MT05's "every entry point" AC.

The refusal SHAPE: a non-admin gets the SAME generic not-found page whether the surface exists, the
token exists, or the workspace is foreign — nothing in status/body distinguishes the cases
(non-enumerable, NFR-MT-SEC-03). (OD-adjacent: 404-vs-403 is DESIGN's call per US-MT05 technical
note — we pick **404 generic** for non-enumerability.)

## Session + CSRF (NFR-MT-SEC-07 — the browser contract, UNCHANGED)

Because the routes sit UNDER the existing layers:
- **Session**: `tower-sessions` Postgres store, 30-day cookie — `signed_in_user` reads
  `SESSION_KEY_USER_ID` exactly as `projects.rs:287`.
- **CSRF**: the double-submit `foundry_csrf` cookie + `_csrf` form field (or `HX-CSRF` header) is
  enforced by `csrf::csrf_middleware` on every POST. The mint form and the revoke form/button BOTH
  carry the hidden `_csrf` field (the view-models carry a `csrf: String`, populated via the SHIPPED
  `ensure_csrf_cookie` helper, `projects.rs:295`). `/admin/tokens` is **NOT** added to
  `csrf::is_exempt_path` (only `/bootstrap` is exempt, csrf.rs:61) — so a forged cross-site POST is
  rejected 403 before the handler runs.
- The existing browser-auth acceptance suite is untouched and stays green (NFR-MT-SEC-07 verify).

## The flow

### Mint (US-MT01, US-MT04)
1. `GET /admin/tokens` renders `TokenListPage`: the existing tokens (US-MT02) + a mint form with
   `label`, scope (radio "Whole workspace / A specific team" + team picker → DD9), and TTL
   (within bounds, default 90d / cap 365d → DD8). If `machine_token_signer.is_none()`, the form is
   replaced by an "issuing not enabled" notice (signer.md).
2. `POST /admin/tokens` (`submit_mint`): pre-check admin + signer-present; read the signer from
   `state`; call `services.mint_token(signer, principal, label, scope_choice, ttl)` →
   `Result<MintedToken, ServiceError>` where `MintedToken { value: SecretString, jti, label, scope,
   expires_at }`.
3. On `Ok`, render `TokenMintedPage` exposing `value.expose_secret()` EXACTLY ONCE into the page,
   with a Copy affordance, the unmistakable "this is the only time you'll see this" warning, and the
   `jti`/label/scope/expiry. The `MintedToken` (and its `SecretString`) is dropped when the handler
   returns. **No redirect** here — a redirect would lose the value; the value is rendered inline in
   the POST response, then never again.
4. Validation failures (`ServiceError::Validation` for TTL-over-cap or foreign team; `Forbidden`;
   signer absent) re-render the mint form (full page) or an `ErrorFragment` (htmx), mapping per
   error-and-observability.md — **never a partial token** (all-or-nothing render, NFR-MT-REL-01,
   reusing the `force_board_render_failure` seam shape).

### One-time display → list (US-MT01, US-MT02, US-MT06)
- After the admin leaves `TokenMintedPage`, returning to `GET /admin/tokens` shows the token in the
  LIST with `jti`, label, scope (workspace vs team name), expiry, status (Active/Revoked), **minted
  by {admin}** (resolved from `created_by` → display name; "—" if NULL), and **last used** (or
  "never"). No `value` field exists on `TokenListPage` (NFR-MT-SEC-02) — there is no reveal path.
- Empty workspace renders an inviting empty state (US-MT02 scenario 3), mirroring the empty-board
  guidance pattern (`projects.rs` board empty state).

### Revoke (US-MT03)
1. Each list row carries a **Revoke** button (a `POST .../{jti}/revoke` form with `_csrf`) and a
   confirm step warning it is immediate and irreversible (a confirm interstitial `TokenRevokeConfirm`
   or an Alpine confirm — DESIGN leaves the confirm UX to the crafter; the AC is "a clear
   immediate-and-irreversible warning before it takes effect").
2. `submit_revoke`: pre-check admin; call `services.revoke_token(principal, jti)` → finds the row
   scoped to the acting workspace, flips `revoked_at` (idempotent), returns `Ok` even if already
   revoked (NFR-MT-REL-02) and `NotFound` for a foreign/unknown `jti` (non-enumerable).
3. Redirect 303 → `GET /admin/tokens` (full page) or swap the row fragment to "Revoked" (htmx). The
   integration's NEXT `/api/v1` call is refused by the SHIPPED denylist — no new refusal code.

## View-models (EXTEND `views.rs`)

New `#[derive(Template)]` structs following `ProjectCreatePage`/`BoardPage` exactly:
- `TokenListPage { workspace_name, tokens: Vec<TokenRow>, mint_form: Option<MintFormVm>, csrf }`
  where `mint_form` is `None` on verifier-only binaries.
- `MintFormVm { csrf, teams: Vec<TeamOption>, default_ttl_days, max_ttl_days, error: Option<String> }`.
- `TokenMintedPage { value_once: String, jti, label, scope_label, expires_at, csrf }` — the ONLY
  view-model that ever carries a token value, populated once and dropped with the response.
- `TokenRow { jti, label, scope_label, expires_at, status, minted_by, last_used }` — NO value field.
- `TokenRevokeConfirm { jti, label, csrf }`.

Templates extend `base.html` (vendored `/static` assets) exactly as `project_create.html`/`board.html`.
