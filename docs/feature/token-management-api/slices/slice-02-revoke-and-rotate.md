# Slice 2 — Revoke + Rotate: hands-free credential lifecycle

> Adds the first (and only, in v1) mutation surface. Revoke is NOT self-amplifying — the worst case
> is a loud, reversible, workspace-confined DoS, never a credential leak — so it ships safely once
> Slice 1 has proven the authz gate. Revoke-self is the gentlest mutation and the core of rotation.

## Learning hypothesis (one line)
If a management bearer can `DELETE /api/v1/.../tokens/{jti}` (including its OWN jti), then a rotation
job or incident runbook can kill and rotate credentials hands-free and prove — on the credential's
next call — that it is dead, with no human in the loop.

## Stories
- **US-TMA02** (mt-api-job-2) — revoke a token via the API; refused on its next `/api/v1` call;
  idempotent; cross-workspace/unknown → non-enumerable 404.
- **US-TMA03** (mt-api-job-2) — revoke-SELF: a token disables its own future use (the rotation flow;
  the in-flight request still succeeds, the next call 401s).

## Backbone coverage
Re-exercises Authenticate; adds Revoke/Rotate; relies on Trust-the-contract (non-enumerable 404).

## Reused primitives (verified in code)
- `foundry_services::tokens::revoke_token(store, principal, jti)` — SHIPPED, mutation-hardened: authz
  → non-enumerable cross-workspace `NotFound` → idempotent `revoked_at` re-stamp.
- The SHIPPED per-request `jti` denylist in `token_auth::authenticate` (`resolve_active_token`) — the
  revoke-effectiveness mechanism; no new code.
- `status_for` (404 not_found, 403 forbidden, 401 unauthorized).

## Done when
- A management bearer revokes a workspace token; the target's next `/api/v1` call is refused (401),
  one-request latency (NFR-TMA-SEC-04).
- Re-revoke is idempotent success (NFR-TMA-REL-01).
- Cross-workspace/unknown jti returns the identical non-enumerable 404 (NFR-TMA-SEC-03).
- Revoke-self: the in-flight request succeeds, the NEXT call with that credential 401s; idempotent
  across rotation cycles.
- A non-management bearer is refused (403); no token revoked.

## Risk addressed
Hands-free rotation + incident response. The revoke-storm DoS vector is bounded (workspace-confined +
Q-RATE-LIMIT guardrail in Slice 3); revoke-self is the safest possible mutation.

## Depends on
Slice 1 (US-TMA00/01); ratified Q-AUTHZ + Q-REVOKE-VERB (default `DELETE`) + Q-REVOKE-SELF
(default YES).
