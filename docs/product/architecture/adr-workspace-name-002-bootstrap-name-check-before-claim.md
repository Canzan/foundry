# ADR-WORKSPACE-NAME-002: The bootstrap claim checks the workspace name before the claim transaction, and answers with the rule only for a live link

- Status: Accepted (2026-10-05)
- Feature: `instance-workspace-name-rule` (D7, D8, D9; DESIGN DDD-9)

## Context

`POST /bootstrap?token=…` claims a one-time link. In one transaction it consumes the token
and seeds the workspace, user, team, project and first `instance_admins` row
(`Store::claim_bootstrap_and_create_workspace`). Every non-valid link state (used,
expired, unknown) and an email collision all render one byte-identical
`bootstrap_refusal_page()`, so a prober learns nothing about why a link is dead
(bootstrap-claim-enumeration-oracle). The handler calls the store directly. It hashes
the password before the transaction.

The workspace-name rule (ADR-WORKSPACE-NAME-001) must now refuse a bad name here. The
refusal must not consume the link or create anything (D7). It must show a 422 claim page
with the reason and the non-secret fields retained (D9). It must leave the dead-link
answer byte-identical whatever the name (D8). `GET /bootstrap` already tells a live link
from a dead one through the non-consuming `Store::bootstrap_token_status`.

## Decision

In `bootstrap::submit`, after the token-present check and before the password hash:

1. Run `WorkspaceName::try_new` on the submitted workspace name (pure, no I/O).
2. On `Ok`, the shipped flow runs unchanged, and the validated name goes to the claim
   transaction. The happy path gains no query.
3. On `Err`, read `bootstrap_token_status` without consuming the link:
   - `Valid` returns 422 with the claim page (`base.html`), the error copy, and email,
     display name and workspace name retained. The password is never echoed.
   - Any other status returns today's byte-identical `bootstrap_refusal_page()`.
   - A read error returns the existing 500 `internal error`.

   In every branch, nothing is hashed, claimed or written.

The claim transaction stays the authority on liveness. A link that dies between a
refusal and the corrected retry still gets the uniform refusal page.

## Alternatives considered

- **Validate first and answer 422 whatever the link state.** Rejected. A dead link with
  a bad name would answer 422 instead of the uniform refusal. That splits the uniform
  answer and breaks D8.
- **Validate inside the claim transaction** (a new `BootstrapClaimOutcome` variant, then
  rollback). Rejected. The rule would move into the store (ADR-WORKSPACE-NAME-001
  decision 5), and the store would have to tell rule failures apart from link refusals.
  The token UPDATE would also run and roll back on every bad name, for no gain over a
  plain read.
- **Hash the password first, as today.** Rejected for the refusal path. An argon2 hash
  for a request that is going to be refused wastes CPU and buys nothing. On the success
  path the existing order (hash before the transaction) is unchanged.

## Consequences

- Positive: a refused claim provably touches no state, because it never reaches the
  transaction. The same link works on the corrected retry.
- Positive: there is no new oracle. The extra read happens only on the refusal path, and
  it reveals liveness only to the extent that `GET /bootstrap` already does. Dead links
  answer as before for any name.
- Neutral: the refusal path skips argon2, so it answers faster than a valid-name dead-link
  POST. The difference tells the caller only that their own name failed a public rule,
  not the link's state. Both live-link and dead-link refusals take the fast path.
- Negative: the claim template gains retained-field `value` attributes and an error slot,
  so the `GET /bootstrap` markup changes additively. The dead-link refusal page does not
  change.
