# ADR-001 — How the first instance super-admin comes to exist

## Status
Proposed (Propose mode). FIRMS one half of the parent `multi-workspace-tenancy` ADR-004
(deferred). Awaits user ratification (flagged in `wave-decisions.md`).

## Context
OD-3 is ratified: a NEW instance-level super-admin role (above workspace-admin) provisions
tenants; no self-serve signup in v1. The role table itself is ADR-003 of this feature. The open
question here is **bootstrapping the role**: who is the FIRST `instance_admins` row, and how does
it come to exist on both a fresh install and an upgraded one?

Grounding (read the code):
- Bootstrap today (`bootstrap.rs:103-215`, `submit`) claims the instance: it atomically creates
  workspace 1 + the first user as that workspace's `admin` + a seeded team/project
  (`create_initial_workspace`, `lib.rs:307`), then stamps the session. This is the single "claim
  the instance" entry.
- `instance_admins` does NOT exist yet; there is no instance-level identity today.
- The parent ADR-004 already decided: "Bootstrap is extended so that initial bootstrap creates
  workspace 1 AND the first `instance_admins` row (the operator who claims the instance)."

## Options considered
- **(a) Bootstrap claim seeds the first super-admin** — the operator who claims the instance
  (and becomes workspace 1's admin) is ALSO inserted into `instance_admins` in the same claim
  transaction. One entry point; the "claim the instance" act establishes both the first workspace
  AND the instance authority. A separate `foundry doctor grant-super-admin --email` CLI exists for
  later promotion/rotation.
- **(b) A dedicated bootstrap env var / CLI to seed the super-admin separately** — e.g.
  `FOUNDRY_SUPERADMIN_EMAIL` read at startup, or `foundry doctor grant-super-admin` run once after
  bootstrap. Decouples the two authorities but adds a second mandatory setup step and a failure
  mode (instance claimed but no super-admin ⇒ no one can provision).
- **(c) Super-admin is a separate instance-level identity, NOT the workspace-1 admin** — a
  distinct account with no workspace membership. Cleanest conceptual separation, but on a
  single-operator self-hosted instance it doubles the credentials the operator manages for no
  v1 benefit, and the operator is already the workspace-1 admin.

## Decision
**(a) The bootstrap claim seeds the first `instance_admins` row.** The operator who claims the
instance becomes BOTH workspace 1's admin AND the first super-admin, in the same atomic claim
transaction. For an UPGRADED install (no fresh bootstrap), the operator is granted via a
`foundry doctor grant-super-admin --email <addr>` subcommand (idempotent `INSERT … ON CONFLICT
DO NOTHING`), so an existing install gains a super-admin without re-bootstrapping. Later
promotion/rotation uses the same `grant-super-admin` (and a `revoke-super-admin`) subcommand.

The super-admin identity is a `users` row referenced by `instance_admins.user_id`; it is NOT a
new credential type — the super-admin signs in exactly like any user. Being a super-admin is an
authority (`is_instance_admin`), orthogonal to workspace membership.

## Consequences
- **Positive**: one "claim the instance" entry (no second mandatory setup step on fresh installs);
  no possibility of a fresh instance with a workspace but no provisioning authority; upgraded
  installs get a clean, idempotent grant path; rotation/promotion is a CLI one-liner.
- **Negative**: on a fresh install the first super-admin == workspace-1 admin (they are the same
  human); separating them later means granting another user and (optionally) the operator
  revoking their own super-admin — supported by the grant/revoke subcommands but a manual step.
- **Security**: the super-admin set is seeded only by the instance operator (bootstrap claim or
  shell-level `foundry doctor grant-super-admin`), never by a workspace member, never over the
  bearer surface. `ON CONFLICT DO NOTHING` makes grant idempotent and race-free.

## Relationship to parent ADR-004
FIRMS parent ADR-004's "bootstrap seeds workspace 1 + first super-admin" clause, and ADDS the
upgraded-install grant path (which the parent left implicit). The role table + authz function are
this feature's ADR-003.
</content>
