# ADR-003: How ONE /invites/accept route serves both first-admin and member invites

## Status
Accepted (DESIGN, Propose mode). This is the headline open decision the task asked to resolve.

## Context
The shipped `/invites/accept` POST (`invites_accept::submit_accept`) calls
`set_first_admin_password_and_consume`, which writes the password onto the pre-existing `created_by`
user. The member flow needs `create_member_and_consume`, which CREATES a new user (ADR-002). ONE route
must serve BOTH invite kinds. The task asked: what is the kind discriminator?

**Schema check (decisive):** the `invites` table (`0001_init.sql:93-102`) has **NO `kind` or `role`
column** — `id, workspace_id, invitee_email, created_by, expires_at, used_at, used_by, created_at`. So
the discriminator must be data-derived, OR a column must be added (a migration).

**The natural, already-present discriminator:**
- **First-admin invite**: `created_by` is the prospective consumer themselves — `provision_workspace`
  seeds the first-admin user AND sets that same user as the invite's `created_by`. The consumer's account
  ALREADY EXISTS; accepting writes the password onto `created_by` (`used_by = created_by`).
- **Member invite** (ADR-001): `created_by` is the inviting ADMIN — a DIFFERENT person. The invitee
  (`invitee_email`) has NO account; accepting must CREATE one.

So the kinds are distinguished by a fact already in the data: **does `invitee_email` already map to an
existing user (and is that user `created_by`)?** Equivalently: **does the invitee already have an
account?** First-admin → yes (it's `created_by`); member → no.

## Decision
**Dispatch inside `submit_accept` on whether `invitee_email` resolves to an existing user — NO schema
column, NO migration.** After the pre-consume password validation (ADR-002 step 3), before opening any
tx:

- Resolve the invite's `invitee_email` + `created_by`. **The shipped `invite_accept_view`
  (`store/lib.rs:582`) currently returns only `(expires_at, used_at, workspace_name)` — it does NOT
  carry `invitee_email` or `created_by`.** This dispatch therefore REQUIRES a load-bearing extension to
  the read contract:
  - **Recommended**: EXTEND `invite_accept_view` to also `SELECT i.invitee_email, i.created_by` and add
    them to the returned `InviteAcceptView` struct. The GET path already calls it and simply ignores the
    new fields (the set-password form is kind-agnostic); the POST dispatch reads them. One read, no extra
    round-trip. (The crafter owns the exact struct/SELECT; the DESIGN contract is: the accept read MUST
    surface `invitee_email` and `created_by` so the POST can dispatch.)
  - Alternative (if extending the view is undesirable): a thin supplemental read
    (`invite_kind_inputs(id) -> (invitee_email, created_by)`) on the POST path only.
- **If `invitee_email` maps to an existing user whose id == `created_by`** → it is the FIRST-ADMIN
  invite → run the SHIPPED `set_first_admin_password_and_consume` (unchanged).
- **Otherwise (no existing user for `invitee_email`)** → it is a MEMBER invite → run the NEW
  `create_member_and_consume` (ADR-002). The account-creation tx's own UNIQUE-email guard catches the
  OD-1 collision (an `invitee_email` that maps to an existing user who is NOT `created_by`) and maps it
  to the uniform refusal — so the dispatch does NOT need to pre-check for the collision, and stays
  non-enumerable (no SELECT-driven oracle in the handler).

Crucially, the discriminator read is the SAME mechanism the member tx already needs (it must know there
is no existing user). The recommended seam: `create_member_and_consume` is the DEFAULT arm; the
first-admin arm is selected only when the invitee already exists as `created_by`. This keeps the shipped
flow's behavior byte-identical (a first-admin invite still routes to the shipped tx) while the member tx
owns the collision semantics.

The GET copy ("join as a member" vs "first administrator") is a display nuance only; the GET handler
need not branch on kind to render (the set-password form is identical).

## Alternatives Considered
- **Add an `invites.kind` (or `role`) column** (one additive migration after `0011`) — REJECTED for v1.
  The discriminator is ALREADY derivable from `created_by` + `invitee_email`; a column would be redundant
  state that must be kept consistent with `created_by`, and adds a migration the feature otherwise does
  not need. (If a FUTURE feature needs admin-role member invites — explicitly deferred — a `role` column
  becomes justified; ADR-004 records the migration number it would take.)
- **Branch on `created_by == used_by`-able / a NULL `created_by`** — REJECTED. `insert_invite` binds
  `created_by` as a required non-NULL `Uuid` (`store/lib.rs:546`); a member invite's `created_by` is the
  admin (non-NULL). A "NULL created_by ⇒ member" scheme would require changing `insert_invite`'s contract
  and the column nullability semantics for no benefit — the existing non-NULL `created_by = inviter`
  already distinguishes the kinds (inviter ≠ invitee for members; inviter == invitee for first-admin).
- **Two separate routes (`/invites/accept` vs `/workspace/invites/accept`)** — REJECTED. The emitted link
  is a single URL shape; the invitee cannot know which kind they hold, and two public accept routes
  double the non-enumerable-refusal surface and the CSRF/verify plumbing. One route, internal dispatch.
- **Always run `create_member_and_consume`, never the first-admin tx** — REJECTED. A first-admin invitee
  already has an account (`created_by`); routing them to the create-user tx would hit the UNIQUE-email
  guard and (correctly per ADR-002) refuse — breaking the SHIPPED first-admin flow. The dispatch MUST
  preserve the first-admin arm.

## Consequences
- Positive: zero migration; the shipped first-admin flow is preserved byte-identically; the discriminator
  is derived from data that already exists and is already loaded; the collision semantics live in the one
  tx that owns account creation.
- Negative: the dispatch needs the invite's `invitee_email` + `created_by` (a small extension to
  `invite_accept_view` or a thin read) and an existing-user lookup keyed on email — one extra cheap read
  on the accept POST. Accepted (it is the same fact the member tx needs anyway).
- Probe (Earned Trust): a first-admin invite still routes to `set_first_admin_password_and_consume` and
  the shipped `@all` acceptance scenarios stay GREEN (regression guard); a member invite routes to
  `create_member_and_consume`; an `invitee_email` colliding with an existing non-`created_by` user routes
  to the member tx and is refused via the UNIQUE guard (uniform refusal, AC-03.8).
