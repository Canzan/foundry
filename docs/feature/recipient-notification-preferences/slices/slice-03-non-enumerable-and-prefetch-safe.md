# Slice 03 — A tampered/unknown link is safely refused, non-enumerable + prefetch-safe

**Goal**: harden the public unsubscribe endpoint so a tampered/unknown token leaks no existence and a bare GET
never mutates state → the link is safe to click and safe to sit in an inbox, and can't be turned into an
enumeration oracle or a prefetch trap.
**Story**: US-03.

**IN scope**
- On any token failure (tampered / malformed / unknown / expired-if-applicable), return the **uniform
  non-enumerable refusal** — a fixed status + byte-identical body for every reason, reusing the shipped
  `invite_refusal_page` shape (`crates/foundry-app/src/invites_accept.rs:332-339`, fixed **200 OK**). Records
  nothing.
- Verify the `UnsubscribeToken` with the shipped **constant-time** compare (`foundry-auth/src/lib.rs:260`).
- Make the raw **GET non-destructive**: state changes only on an explicit confirm (POST / RFC 8058 one-click,
  DESIGN ODD-2). A prefetch/scan of a valid link writes no row.
- No token or recipient email in logs/errors for a refused request.
- Acceptance / `@property`: real vs non-existent address both yield the identical refusal; a tampered token is
  refused like an invalid one; a bare GET of a valid link records nothing; reverting either guard reds the
  litmus.

**OUT of scope**: the happy-path confirm loop (US-01, which this hardens); the RFC 8058 email-header emission
decision beyond the GET-safety guarantee (DESIGN ODD-2 ratifies); mandatory exemption (US-02); anything
signed-in (US-05/06).

**Learning hypothesis**: disproves "reusing the shipped uniform refusal (`invites_accept.rs:332-339`) +
constant-time verify + a non-destructive GET makes the public unsubscribe endpoint non-enumerable and
prefetch-safe with no differential responses" if any invalid reason produces a distinguishable response, if a
real vs fake address differs, or if a GET can be made to mutate state.

**Seams**: uniform refusal `invite_refusal_page` (`crates/foundry-app/src/invites_accept.rs:332-339`);
constant-time `verify` (`crates/foundry-auth/src/lib.rs:260`); the `/unsubscribe` route + token from slice 01
(`lib.rs:371-374` cluster); CSRF for the confirm POST (`csrf.rs:137`); uniform 404 fallback (`lib.rs:535`) as a
sibling precedent.
**Dependencies**: slice 01 (US-01) — the token + route it hardens. DESIGN ODD-2 (GET-safety / RFC 8058
one-click stance). No new persistence.
**Effort**: ~1 day (reuses shipped refusal + verify; the work is the litmuses proving non-enumerability +
prefetch-safety against the adversary).
</content>
