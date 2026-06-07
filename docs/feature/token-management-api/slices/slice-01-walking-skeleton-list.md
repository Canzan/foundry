# Slice 1 (Walking Skeleton) — Prove the authz model on GET list

> The thinnest end-to-end slice across all backbone activities. It answers the riskiest question —
> "is a management-capable bearer authorized and a non-management bearer refused?" — on the SAFEST
> real op (read-only LIST), before any mutation ships.

## Learning hypothesis (one line)
If we expose `GET /api/v1/.../tokens` and gate it with the ratified authz model, then a
management-capable bearer can inventory the workspace's tokens as value-free JSON while a
non-management bearer is refused non-enumerably — proving the authz/escalation model end-to-end with
zero blast radius.

## Stories
- **US-TMA00** (`@infrastructure`, folded) — stand up the `/api/v1/.../tokens` route group + the
  single authz-gate seam where the ratified Q-AUTHZ decision lives.
- **US-TMA01** (mt-api-job-1) — `GET .../tokens` lists the workspace's tokens as value-free JSON for
  a management bearer; refuses a non-management bearer (403, non-enumerable).

## Backbone coverage
Authenticate (reuse `MachinePrincipal`) → Inventory (GET list) → Trust-the-contract (200 array shape
+ 403 refusal shape, both via `status_for`). Revoke/Rotate not yet.

## Reused primitives (verified in code)
- `foundry_services::tokens::list_tokens(store, principal)` — SHIPPED, mutation-hardened; authz
  (`is_workspace_admin`) → workspace-scoped, newest-first → value-free `TokenView`.
- `foundry_api` `MachinePrincipal`/`token_auth::authenticate` — SHIPPED bearer extractor (fail-closed
  identical 401).
- `foundry_api::status_for` — SHIPPED JSON error envelope (403 forbidden).
- `routes<S>()` extension point (`crates/foundry-api/src/lib.rs`), dispatch via `Services`.

## Done when
- A management bearer gets a JSON array (label/scope/expiry/status/last-used/minted-by; no value).
- An empty registry returns `[]` + 200 (not 404).
- A non-management bearer is refused (403, non-enumerable — no registry data).
- No read path returns a token/secret/hash value (NFR-TMA-SEC-02).
- The authz decision is enforced in exactly one seam (NFR-TMA-SEC-08 / Q-AUTHZ).
- foundry-api still does not name `foundry_store::Store`.

## Risk addressed
The Q-AUTHZ crux — proven on a read-only op so a wrong model is caught before any mutation or any
mint surface could ship.

## Depends on
**Ratified Q-AUTHZ model** (the gate). Reflects option (c): bearer may LIST.
