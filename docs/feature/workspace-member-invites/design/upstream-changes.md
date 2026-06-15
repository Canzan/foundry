# Upstream Changes — workspace-member-invites

DESIGN-wave findings where the shipped code diverges from, or refines, what the DISCUSS artifacts
recorded. Per trunk-based policy, parent DISCUSS docs are NOT modified; the corrections are recorded here
for DISTILL/DELIVER. None changes behavior; all REDUCE the feature's footprint (confirming ZERO
migration).

## Finding 1 — `invites` already carries `used_at`/`used_by` (grounding correction)

`requirements.md` (grounding table) and `shared-artifacts-registry.md` describe the `invites` columns as
`id, workspace_id, invitee_email, created_by, expires_at, used_at, used_by` in places but the original
brief premise (and the OD-1 framing) implied the single-use marker might be new. **It is not.**
`0001_init.sql:93-102` ships `used_at TIMESTAMPTZ` + `used_by UUID REFERENCES users(id)` in the very
first migration. This was already established by the shipped `invite-accept-flow` (its `wave-decisions.md`
headline finding). Consequence: the member consume reuses these columns verbatim — **no migration**.

## Finding 2 — `users.email_lower` is already `UNIQUE` (the OD-1 collision guard exists)

`0001_init.sql:19`: `email_lower TEXT NOT NULL UNIQUE`. The DISCUSS framing treated the email-already-a-
user collision (OD-1 / A-E9) as a behavior to design, with a HIGH-risk note that it "must not leak via a
500/constraint error." The grounding refines this: the constraint that DETECTS the collision already
exists; the design only needs to CATCH it (SQLSTATE 23505) inside the tx and map it to the uniform
refusal (adr-002/004, D5). No new constraint, no migration.

## Finding 3 — `workspace_memberships.role` already CHECK-allows `'member'`

`0001_init.sql:29`: `role TEXT NOT NULL CHECK (role IN ('admin', 'member'))`. The member-role membership
the new tx inserts is already a valid value — `create_initial_workspace` inserts `'admin'`; the member tx
inserts `'member'`. **No role migration, no new CHECK.**

## Finding 4 — `invites` has NO `kind`/`role` column (kind is data-derived)

`0001_init.sql:93-102` has no discriminator column. The task asked whether one is needed; the answer is
NO — first-admin vs member is derived from `created_by` + `invitee_email` (adr-003, D3). Recorded so
DISTILL does not look for a column the schema does not have. (If a FUTURE admin-role member-invite feature
needed one, it would be the additive `0012_invites_role.sql` — reserved on paper, not created.)

## Finding 5 — `insert_invite` already fits the member case (no new store fn)

`store/lib.rs:541` already takes `created_by: Uuid` (required) + `invitee_email: Option<&str>`. The
DISCUSS open-decision asked whether a new `insert_member_invite` is needed; it is NOT (adr-001, D2). The
member case binds `created_by = the inviting admin`.

## Net effect

All five findings CONFIRM the reuse-heavy verdict and REDUCE scope to: ONE new store tx
(`create_member_and_consume`), ONE new web file (`member_invites.rs`), a small `submit_accept` dispatch,
two thin templates, ZERO new crates, **ZERO migration**. No DISCUSS doc is edited; no shipped code is
modified by this DESIGN pass (it is a design document only).
