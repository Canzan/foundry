# ADR-OIDC-002 — The one-time OIDC state rides in a signed cookie, not a session row

- Status: Accepted (2026-08-21)
- Feature: `keycloak-sso` (DDD-5)

## Context

The authorization-code flow needs `state`, `nonce` and a PKCE `code_verifier` minted at
`/auth/oidc/start` and read back at `/auth/oidc/callback`. Both routes are reachable
signed out — that is the point of a sign-in flow.

## Decision

Carry all three in one dedicated cookie: `HttpOnly`, `SameSite=Lax`, `Secure` in
production, short TTL, signed with the shipped `foundry_auth::sign` / `verify` HMAC
over `SESSION_SECRET`. Cleared on success and on every refusal, which is what makes it
single-use.

## Alternatives

**A pre-authentication `tower-sessions` row.** Rejected: `/auth/oidc/start` is public,
so this is an unauthenticated, unbounded INSERT on a public endpoint — a disk-fill
vector that also pollutes the session table with rows for flows nobody completes.

**An unsigned cookie.** Rejected: `state` and `nonce` only defend against forgery if
the client cannot choose them.

**Server-side in-memory map.** Rejected: foundry runs multiple replicas behind a proxy
(the multi-replica lane in the ATDD policy), so a start on one replica and a callback
on another would fail. A signed cookie is replica-independent by construction.

## Consequences

Stateless and self-expiring, and it reuses the exact primitive `InviteToken` and
`UnsubscribeToken` already use — a third instance of a known shape rather than a new
mechanism. The cookie is bounded in size (two random values plus a verifier), so it
does not approach header limits.
