# Out of Scope — Token-Management API

What this feature deliberately does NOT do. Each item names why and where it lives instead.

## Programmatic MINT (deferred — the escalation-sensitive op)
- **NOT in v1.** Exposing `POST /api/v1/.../tokens` to a bearer caller creates a self-replication
  surface (the mint loop — a leaked management-capable token mints unlimited fresh credentials; see
  `wave-decisions.md` escalation analysis). Provisioning a NEW credential remains a human-session
  action (`/admin/tokens`, shipped in `machine-token-admin-ux`).
- **If ratified later**, it ships as its OWN slice (a future US-TMA06 / mt-api-job-4) WITH: option
  (b)'s explicit `tokens:manage` capability claim (never reachable by a plain admin-bound token); a
  "management tokens cannot mint management tokens" anti-self-replication rule; the SEC-07 mint-rate
  guardrail; the SHIPPED one-time-value guarantees; and the Q-SIGNER-WIRING work to reach
  `AppState.machine_token_signer` from the foundry-api adapter (today only the web handler reads it).

## OAuth / OIDC / token exchange
- **NOT this feature.** No authorization-code/client-credentials grants, no `/token` exchange
  endpoint, no refresh tokens, no scopes-as-OAuth. Foundry's machine tokens are self-contained
  Ed25519 JWTs verified offline; this feature manages those, it does not introduce a new auth
  protocol. A standards-based grant flow would be its own large feature.

## A general-purpose / public API platform
- **NOT this feature.** No API versioning strategy beyond the existing `/api/v1`, no developer
  portal, no published OpenAPI/SDK generation, no per-app API keys, no usage-based metering. This is
  a narrow token-management surface for the existing machine-token credential, not a public API
  product.

## Key rotation / signing-key management
- **NOT this feature.** Rotating the Ed25519 SIGNING key, managing the verifier's key SET, or
  re-keying issued tokens is signing-key lifecycle (owned elsewhere; `MachineTokenVerifier` already
  supports overlapping-key rotation, and key-at-rest is a DESIGN/ops concern carried from
  `machine-token-admin-ux` Q1). Token rotation HERE means rotating a *credential* (revoke-self +
  human-mint a new one), not rotating the *signing key*.

## A new web UI / changes to the admin UI
- **NOT this feature.** The human admin surface (`/admin/tokens`, Askama) shipped in
  `machine-token-admin-ux` and is unchanged. This feature adds only the JSON `/api/v1` peer.

## New persistence / schema changes
- **NOT expected.** This is a pure adapter feature over SHIPPED use-cases. No migration is
  anticipated; `NFR-TMA-DATA-01` guards against adding any secret column if one is ever proposed.
  (`created_by` already exists from `machine-token-admin-ux`.)

## Changing the use-case logic
- **NOT this feature.** `foundry_services::tokens::{mint_token,list_tokens,revoke_token}` are
  mutation-hardened (100%) and reused AS-IS. This feature adds a JSON adapter + an authz gate ON TOP;
  it does not re-spec authz order, TTL validation, scope mapping, or the one-time-value handling.

## Cross-workspace / multi-tenant administration
- **NOT this feature.** A bearer acts only within its bound workspace; there is no super-admin
  cross-workspace token administration. Cross-workspace isolation is reused from the use-cases
  (non-enumerable `NotFound`).
