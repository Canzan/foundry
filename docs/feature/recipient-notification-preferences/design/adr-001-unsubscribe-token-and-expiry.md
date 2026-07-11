# ADR-001: UnsubscribeToken shape + expiry stance (ODD-1)

## Status
Accepted — 2026-07-11 (Morgan, DESIGN wave). Feature-local.

## Context
FR-1/NFR-1 require a signed, non-enumerable, tamper-proof unsubscribe link that works **logged-out** for
account-less recipients. The shipped `InviteToken` (`foundry-auth/src/lib.rs:354-390`) is the cited model,
but it binds a database `invite_id` PK and an `expires_at`, using the DB row as the single-use control and
the HMAC as defense-in-depth. An unsubscribe has different semantics: there is no pre-existing row to bind,
the action is idempotent and self-limited to the holder's own `(email, workspace)`, and the link should
remain usable long after issue. The constant-time primitives `foundry_auth::sign` / `verify`
(`lib.rs:251,260`, URL-safe base64, keyed on `SESSION_SECRET`) are reusable directly.

## Decision
Add a sibling `UnsubscribeToken::{new,verify}` in `foundry-auth`, built on `sign`/`verify`, over a
**domain-separated, versioned payload**: `"unsub|v1|{email_lower}|{workspace_id}"`.
- **No expiry** — the token binds only email + workspace; an unsubscribe link must work indefinitely.
- **`SESSION_SECRET`** as the signing key (the same root `InviteToken` uses); no new config key.
- **Constant-time verify** (inherited from `verify`); any failure → the uniform refusal (ADR-002).

## Alternatives Considered
- **Reuse the `InviteToken` struct verbatim** — rejected: it binds a DB `invite_id` PK + `expires_at`;
  forcing an unsubscribe (which has no pre-issued row and no natural expiry) into it would mean minting and
  persisting a throwaway row per link, adding a table and a write on the emit path for no benefit.
- **Add an expiry (e.g. 90 days)** — rejected: a recipient may act on a months-old email; an expired
  unsubscribe link that then keeps delivering mail is user-hostile, and the blast radius of a
  never-expiring link is low (idempotent, self-scoped, harms no one else). The `InviteToken` expiry exists
  to bound a sensitive one-time account grant — not applicable here.
- **A dedicated `UNSUBSCRIBE_SECRET`** (so rotating `SESSION_SECRET` doesn't invalidate links) — rejected:
  extra config surface and a second signing root for marginal benefit; secret rotation is rare and
  deliberate, and a recipient whose link is invalidated falls back to a fresh email link or the signed-in
  page.
- **Encrypt the payload** (hide the email in the URL) — rejected: the email is the recipient's own address
  in their own inbox; a signed (not encrypted) token is the shipped posture. Log-hygiene is addressed by
  base64url-obfuscating the `t` param (ADR-002), explicitly noted as obfuscation, not confidentiality.

## Consequences
- **Positive**: no new crate, no new secret, no new table for the token itself; constant-time + domain
  separation (an `InviteToken` sig can never be replayed as an unsubscribe); links survive indefinitely.
- **Negative / accepted**: rotating `SESSION_SECRET` invalidates all outstanding unsubscribe links
  (verify-fail → uniform refusal). Documented in `upstream-changes.md`; the `v1` tag in the payload leaves a
  future rotation seam.
