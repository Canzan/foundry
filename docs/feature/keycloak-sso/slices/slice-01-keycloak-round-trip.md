# Slice 01 — Keycloak round trip (walking skeleton)

## Goal
The operator clicks "Sign in with Keycloak" on `/sign-in` and lands on their foundry dashboard.

## Learning hypothesis
**Disproves if it fails**: that foundry's session layer can be established from an
external identity without touching the password path — i.e. that
`SessionUser { user_id, workspace_id }` is genuinely credential-agnostic.
**Confirms if it succeeds**: the OIDC path is a second door onto the same session
seam, so every downstream surface (dashboard, `/api/v1`, SSE, authorship) works with
no further change. Everything after this slice is refusal semantics and wiring.

## Pre-slice SPIKE-0 — CLOSED before it ran (DESIGN, 2026-08-21)

Both questions were answered by reading the tree during DESIGN, so the 2-hour timebox
is not spent.

1. **Mechanism** — thin flow over the already-present `reqwest` + `jsonwebtoken`
   9.3.1, in a new `crates/foundry-oidc`. Not `openidconnect`. Decided by
   `xtask/src/check_arch.rs::check_jwt_alg_pin`: foundry pins JWT algorithms with a
   build-time scanner over first-party source, and a crate-internal validation is
   somewhere that scanner cannot reach. `DecodingKey::from_jwk` covers JWKS→key, so
   the dependency delta is zero. See `adr-oidc-001-crate-placement.md`.
2. **Harness** — a FAKE issuer with a fixed RSA test keypair, not a Keycloak
   testcontainer. Decided by the ATDD infrastructure policy: Keycloak is
   driven-external / non-deterministic, which that policy fakes. The real Keycloak is
   exercised by slice 03. See DDD-12.

Only **OQ-2** remains open, and it belongs to DISTILL: whether the fake issuer is a
`wiremock`-style server (a new dev-dependency that must clear `cargo deny`) or a
hand-rolled axum app in the acceptance crate.

## IN scope
- `GET /auth/oidc/start` — mint `state`, `nonce`, PKCE `code_verifier`; store in a
  short-lived `HttpOnly` `SameSite=Lax` cookie; 302 to the authorization endpoint.
- `GET /auth/oidc/callback` — verify `state`, exchange the code server-to-server,
  validate the ID token (JWKS signature, `iss`, `aud`, `exp`, `nonce`), read `email`
  and `email_verified`.
- Link the verified email to `users.email_lower`; establish the session through the
  P0 helper; 303 to `/`.
- OIDC configuration read at startup (issuer, client id, client secret, redirect);
  absent config = feature off, rendering no control and refusing both routes (D6).
- The "Sign in with Keycloak" control on `/sign-in`, rendered only when configured.
- Both routes mounted alongside `/sign-in` under `csrf_middleware` + `session_layer`.

## OUT of scope
- Every refusal path beyond what the happy path structurally requires — slice 02.
- Keycloak realm client, tofu variables, cluster deployment — slice 03.
- Auto-provisioning, realm-role mapping, RP-initiated logout (D3, D4; out of feature).

## Precursors (land BEFORE this slice, neither is a slice)

**P0 — extract the session seam.** Pull the tail of `submit_signin` — from
`resolve_active_workspace` through `session.insert(SESSION_KEY_USER_ID, ...)` and the
303 — into `signin::establish_session`. Pure refactor, no behaviour change, covered by
the existing `us_06_signin` scenarios. ~1h.

**P1 — land the crate and its guard, empty.** Create `crates/foundry-oidc` with its
config and error types, add `check_oidc_alg_pin` to `xtask/src/check_arch.rs`, and add
the `foundry-oidc` ban with `wrappers = ["foundry-app", "foundry-acceptance"]` to
`deny.toml`. The guard only fires on files constructing a `jsonwebtoken::Validation`,
so an empty crate passes it trivially — and the guard is therefore in place BEFORE the
first validation is written, which is the only ordering that makes it protective
rather than decorative. ~2h.

Both are `@infrastructure` and so cannot ship as slices of their own (the slice
composition gate). They ride in as preceding commits. Splitting them out is what keeps
this slice inside its one-day budget after DESIGN added the crate and the guard.

## Acceptance criteria
AC-1.1 … AC-1.6 (`feature-delta.md`, US-01). Plus, from US-05, the unconfigured
half of the env switch: AC-5.1, AC-5.2, AC-5.3.

## Dependencies
The P0 and P1 precursors. A real Postgres (testcontainers) and the fake issuer.

## Effort
~1 day, after ~3h of precursors. Reference class: `invite-accept-flow` (a new
signed-token public route pair mounted beside `/sign-in`, establishing a session at the
end) — the closest shipped shape in this repo.

## Taste-test note
This slice ships four things (two handlers, the config wiring, the control + mounts),
which still brushes the "4+ new components is not thin" test. It is NOT split further
because a partial OIDC flow has no end-to-end value: a `start` without a `callback`
redirects the operator to Keycloak and then 404s. The flow is the smallest unit that
demos. DESIGN's additions (the crate, the alg-pin guard, the deny.toml ban) were moved
OUT into precursor P1 rather than swelling this slice — which is the "ship the
abstraction first" taste test applied literally. Documented per the skill's "failures
documented with a reason" allowance.

## Dogfood moment
Same day: the operator signs into their own foundry with Keycloak against whichever
issuer SPIKE-0 chose, then files the slice-02 issue from that session.
