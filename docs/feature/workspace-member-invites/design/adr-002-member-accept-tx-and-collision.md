# ADR-002: The account-creating member-accept transaction + email-collision handling

## Status
Accepted (DESIGN, Propose mode).

## Context
The shipped `set_first_admin_password_and_consume` (`store/lib.rs:290`) writes a password onto the
**pre-existing** `created_by` row — it assumes the consumer already exists (`used_by = created_by`,
`RETURNING created_by`). A member invitee has **no account**. FR-5/NFR-2/BR-3/BR-4 require accept to,
in ONE atomic tx, CREATE the user (email = `invites.invitee_email`), ADD a `member`-role membership,
consume the invite (single-use guard), and write the argon2id password — then auto-sign-in.

OD-1 (USER-RATIFIED): if `invitee_email` already maps to an existing user, refuse NON-ENUMERABLY — the
tx aborts, the invite is NOT consumed, byte-identical uniform refusal (NOT a DB-constraint 500).
Multi-workspace-membership-via-invite is DEFERRED.

Schema grounded (`0001_init.sql`):
- `users.email_lower TEXT NOT NULL UNIQUE` (line 19) — the collision surfaces as a UNIQUE violation.
- `workspace_memberships.role TEXT CHECK (role IN ('admin','member'))` (line 29) — `'member'` is valid; no migration.
- `invites.used_by UUID REFERENCES users(id)` (line 100) — the new user must exist before `used_by` can
  point at it.
- The proven race-safe idiom: `claim_bootstrap_token` / `set_first_admin_password_and_consume` —
  `UPDATE … SET used_at WHERE used_at IS NULL AND expires_at > now RETURNING …`.

## Decision
Add ONE new store tx, `create_member_and_consume(invite_id, password_hash, now) -> MemberConsumeOutcome`
(`{ Consumed { workspace_id, user_id }, Refused, EmailCollision }` — the crafter owns the exact enum
shape). BEGIN:

1. **Guarded-UPDATE consume** (the authoritative single-use + expiry point, mirrors the shipped guard):
   ```sql
   UPDATE invites SET used_at = $2
    WHERE id = $1 AND used_at IS NULL AND expires_at > $2
    RETURNING workspace_id, invitee_email
   ```
   0 rows ⇒ ROLLBACK ⇒ `Refused` (unknown / used / expired / lost race — A-E7, AC-03.5/03.6).
2. **Create the user**: `INSERT INTO users (id = now_v7(), email_lower, email_display, display_name,
   password_hash) VALUES (…)` with `email_lower = lower(invitee_email)`. On the **`users.email_lower`
   UNIQUE violation** (`SQLSTATE 23505`) ⇒ ROLLBACK ⇒ `EmailCollision` (OD-1 / A-E9). The whole tx
   rolls back, so the invite stays UNCONSUMED.
3. **Add the membership**: `INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES
   ($workspace_id, $new_user_id, 'member')`.
4. **Set `used_by`**: `UPDATE invites SET used_by = $new_user_id WHERE id = $1` (the FK is satisfiable
   now the user exists).
5. COMMIT ⇒ `Consumed { workspace_id, user_id = new_user_id }`.

The handler (`invites_accept::submit_accept`) maps BOTH `Refused` AND `EmailCollision` to the SAME
`invite_refusal_page()` (byte-identical body+status — D5, NFR-3). The collision is NEVER a 500: it is a
named tx outcome, caught by matching `SQLSTATE 23505` on the user INSERT, NOT an unhandled `StoreError`
bubbling to the 500 path (AC-03.8 — the HIGH-risk row the DISCUSS flagged).

`display_name` for the new user: derived from the email local-part (a sensible default; the invitee can
have no separate display name at creation). The crafter owns the exact derivation; the contract is that a
non-empty `display_name` satisfying the `users.display_name CHECK (length 1–64)` is written. **Edge: an
email local-part can exceed 64 chars (RFC 5321 allows 64; the stored `invitee_email` is a free TEXT
column).** The contract is therefore: derive `display_name = first 64 chars of the email local-part`
(truncate, never error) — truncation keeps the happy path total (a long-local-part invitee still joins);
a non-empty local-part always yields a length-1..=64 value. The crafter owns whether truncation is by
byte or grapheme; DESIGN requires only that no valid invite ever fails the CHECK.

**Collision-vs-error precision (the named outcome MUST distinguish them):** the email collision is
detected by matching the UNIQUE-violation specifically — `SQLSTATE 23505` on the `users.email_lower`
insert — and returning the named `MemberConsumeOutcome::EmailCollision`. A broad `StoreError` catch is
FORBIDDEN: an FK violation, a connection drop, or any other DB error MUST surface as the generic
`StoreError` (→ the handler's 500 path), NOT be mis-mapped to the uniform refusal. The handler maps ONLY
`Refused` and `EmailCollision` to `invite_refusal_page()`; a `StoreError` stays a 500. DISTILL asserts
the collision arm distinctly from a generic store error (so a regression that broadens the catch REDs).

## Alternatives Considered
- **Pre-check `SELECT … FROM users WHERE email_lower = $1` then INSERT** — REJECTED. Introduces a TOCTOU
  race (two concurrent accepts could both pass the SELECT) AND an extra round-trip. The UNIQUE-constraint
  catch inside the tx is race-safe by construction and is the same posture the codebase already trusts
  for single-use (a guard, not a read-then-act). See ADR-004.
- **Generalize `set_first_admin_password_and_consume` to optionally create the user** — REJECTED. It
  would entangle the first-admin path (write onto `created_by`) with the member path (create a NEW user)
  in one fn with divergent `used_by`/`RETURNING` semantics. Two focused fns + a handler dispatch
  (ADR-003) keep each tx single-responsibility and the shipped fn untouched (no regression risk to the
  shipped flow).
- **Consume in a separate tx from the create** — REJECTED. Violates NFR-2 atomicity (a crash between
  could consume the invite without creating the account, stranding the invitee with a dead link and no
  account). One tx is mandatory.
- **Auto-join an existing user instead of refusing (OD-1)** — DEFERRED (USER-RATIFIED). Silent
  cross-workspace joins raise multi-membership questions out of v1 scope; the non-enumerable refusal is
  the simplest coherent v1.

## Consequences
- Positive: one atomic tx; race-safe single-use AND single-create (the guarded-UPDATE wins exactly one
  consumer; the UNIQUE email wins exactly one user); collision is a structured outcome, not a 500;
  shipped first-admin fn untouched; zero migration.
- Negative: the new user's `display_name` is a derived default (no real name at creation) — acceptable
  for v1 (profile editing is a separate concern); the `used_by` is set in a second UPDATE within the tx
  (the FK forces ordering) rather than inline in the guard — a minor cost for FK correctness.
- Probe (Earned Trust): two concurrent POSTs for one live invite ⇒ exactly one `Consumed` (one user, one
  membership, one consumed invite), the other `Refused` → uniform refusal (NFR-2 @property, AC-03.6); an
  `invitee_email` that already maps to a user ⇒ `EmailCollision` → uniform refusal, NO second account,
  invite NOT consumed (AC-03.8, A-E9); a crash mid-tx leaves the invite live with no orphan user/
  membership.
