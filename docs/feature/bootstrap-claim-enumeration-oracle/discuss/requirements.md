# Requirements — bootstrap-claim-enumeration-oracle

**Feature ID**: `bootstrap-claim-enumeration-oracle`
**Type**: Existing-behavior hardening (security)
**Classification**: Backend / cross-cutting (security)
**Project**: brownfield
**Seeded from**: `/nw:continue` backlog (CONTEXT.md — "close the bootstrap claim-flow enumeration oracle").

## Problem

The bootstrap claim flow (`POST` handler in `crates/foundry-app/src/bootstrap.rs`) closes the
enumeration oracle on the **token** (unknown / used / expired all render one byte-identical
refusal). But a **second, downstream** oracle remains: after the atomic single-use token claim
succeeds (`claim_bootstrap_token`, bootstrap.rs:112-123), the handler calls
`create_initial_workspace` (bootstrap.rs:143-164). On an email-uniqueness collision
(`users.email_lower` UNIQUE, SQLSTATE 23505) that call currently surfaces as a generic
`500 INTERNAL_SERVER_ERROR` — **distinguishable** from the `303 → /dashboard` success path.

Two defects follow:

1. **Enumeration oracle (primary)**: a bootstrap-token holder can submit different emails and
   learn which already map to an account (500 = exists, 303 = created) — an email-existence
   probe on an already-shipped surface.
2. **Token burned on a legitimate collision (secondary)**: the token is consumed *before*
   `create_initial_workspace` runs, so a genuine collision both 500s and destroys the token.

## Precedent (reuse, do not reinvent)

`workspace-member-invites` solved the identical shape: `Store::create_member_and_consume`
(foundry-store/src/lib.rs:411) folds the guarded-UPDATE consume and the account create into
ONE transaction, catches 23505 **specifically** on the create-user INSERT, and rolls the whole
tx back → the invite stays UNCONSUMED and the handler renders a uniform non-enumerable refusal,
never a 500. This feature applies that exact idiom to the bootstrap claim.

## User Stories

### US-01 — Colliding email yields a uniform, non-enumerable refusal
**As** an operator claiming the bootstrap link,
**when** I submit an email that already maps to an existing user,
**then** I get the SAME byte-identical refusal (status + body) that an unknown/used/expired
token produces — never a 500, so the response reveals nothing about whether the email exists.

### US-02 — A colliding submit leaves the bootstrap token reusable
**As** an operator who typo'd / re-used an already-registered email,
**when** the claim is refused for collision,
**then** the bootstrap token remains UNCONSUMED (the claim+create tx rolled back), so I can
retry with a correct email.

### US-03 — The happy path and genuine-error paths are unchanged
**As** the first admin,
**when** I claim with a fresh email,
**then** the workspace/user/team/project/instance-admin seed commits and I land signed-in at
`/dashboard` exactly as today; and any NON-23505 DB error still surfaces as a 500 (the narrow
catch must not mask FK/connection failures as a refusal).

## Non-Functional / Constraints

- **NFR-1 (non-enumerability)**: collision refusal MUST be byte-identical to the existing
  token-refusal (`bootstrap_refusal_page()`), same status code and body.
- **NFR-2 (atomicity)**: token consume + workspace create MUST be one transaction; a 23505 on
  the users INSERT rolls back BOTH (token unconsumed, no partial workspace).
- **NFR-3 (narrow catch)**: only SQLSTATE 23505 on the users insert maps to refusal; every other
  error keeps the 500 path (mirror `create_member_and_consume`'s narrowed catch).
- **C-1**: NO migration (the `users.email_lower` UNIQUE and `bootstrap_tokens` guard already exist).
- **C-2**: NO new crate. Store-layer method + handler rewire only.
- **C-3**: Regression-guard the shipped token-refusal (unknown/used/expired) behavior.

## Out of Scope

- Rate-limiting the bootstrap endpoint.
- Any change to the token-refusal (unknown/used/expired) path itself — already non-enumerable.
- The invite-accept collision path — already handled by `create_member_and_consume`.
