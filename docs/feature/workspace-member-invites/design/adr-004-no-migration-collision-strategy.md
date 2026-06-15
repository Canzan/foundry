# ADR-004: No migration; UNIQUE-constraint-catch for the email collision

## Status
Accepted (DESIGN, Propose mode).

## Context
The task asked explicitly: is a migration needed, and what is its number if so? And: how is the OD-1
email collision detected without a TOCTOU race or a leaking 500?

Schema grounded (`0001_init.sql`; latest applied migration is `0011`):
- `invites.used_at`/`used_by` — the single-use markers — ALREADY SHIPPED (`0001:99-100`).
- `users.email_lower TEXT NOT NULL UNIQUE` — the collision guard — ALREADY SHIPPED (`0001:19`).
- `workspace_memberships.role CHECK (role IN ('admin','member'))` — `'member'` already valid (`0001:29`).
- `users.password_hash TEXT NOT NULL`, `display_name CHECK length 1–64` — the new user's required cols.

Every column the feature writes already exists with the right constraint.

## Decision
**NO migration.** The feature reuses `invites` + `users` + `workspace_memberships` as-is. Specifically:
- single-use: reuse `invites.used_at`/`used_by` (the guarded-UPDATE idiom, ADR-002);
- the member role: reuse the `workspace_memberships.role` CHECK — bind `'member'`;
- the OD-1 collision: rely on the EXISTING `users.email_lower UNIQUE` constraint.

**Collision detection = catch the UNIQUE violation inside the tx (SQLSTATE 23505), not a pre-check
SELECT.** The `INSERT INTO users … email_lower` either succeeds (no existing user → create) or raises
`23505` (an existing user → ROLLBACK → `EmailCollision` → uniform refusal, ADR-002). This is:
- **race-safe**: a pre-check SELECT-then-INSERT has a TOCTOU window (two concurrent accepts could both
  pass the SELECT then one fails the INSERT anyway); the constraint catch is the single authoritative
  point — the same "guard, don't read-then-act" posture the codebase already trusts for single-use
  (`claim_bootstrap_token`, `set_active_workspace`'s `WHERE EXISTS`);
- **non-enumerable**: no SELECT in the handler means no SELECT-driven existence oracle; the only signal is
  the structured `EmailCollision` outcome, mapped to the byte-identical refusal (NFR-3, AC-03.8);
- **non-500**: the `23505` is matched explicitly on the user INSERT and converted to `EmailCollision`,
  never bubbled as a generic `StoreError` to the 500 path.

## Alternatives Considered
- **Add an `invites.kind`/`role` column** — REJECTED (no migration needed; the kind is data-derived,
  ADR-003). **For the record**, were such a column ever required (a future admin-role member-invite
  feature, explicitly deferred), it would be the next additive migration: **`0012_invites_role.sql`**
  (forward-only, `ALTER TABLE invites ADD COLUMN role TEXT NOT NULL DEFAULT 'member' CHECK (role IN
  ('admin','member'))`). Recorded so the number is reserved/known; NOT created in v1.
- **Pre-check SELECT for the email collision** — REJECTED (TOCTOU + extra round-trip + enumeration oracle;
  see Decision).
- **A partial unique index or new constraint for invites** — REJECTED. Each invite is independently
  single-use; a second live invite to the same email is explicitly ALLOWED (US-01 domain example 3). No
  new invite-level uniqueness is wanted.

## Consequences
- Positive: zero schema change; the collision guard is a constraint the DB already enforces; race-safe
  and non-enumerable by construction; consistent with the codebase's guard-not-read posture.
- Negative: the crafter must match `SQLSTATE 23505` specifically (a broad `StoreError` catch would
  mis-map other DB errors to the refusal) — called out so DISTILL asserts the collision arm distinctly
  from a generic store error (which stays a 500). The `0012` number is reserved-on-paper only.
- Probe (Earned Trust): a full issue→accept→re-accept→collision cycle runs with ZERO migrations applied
  beyond `0011`; the collision is asserted to render the uniform refusal (not a 500) AND to leave the
  invite unconsumed (AC-03.8); the `@all` suite stays green (no schema drift).
