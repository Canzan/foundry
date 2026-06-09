# ADR-004 — Instance super-admin role + provisioning surface

## Status
Proposed (OD-3 ratified: instance super-admin only, no self-serve signup in v1).

## Context
Creating a tenant on a shared instance is a privileged, instance-level act, distinct from
administering one workspace. Today the only roles are per-workspace `admin`/`member`
(`workspace_memberships.role`); there is NO instance-level role. `bootstrap_tokens` +
`tower_sessions` are instance-global, and the shipped bootstrap flow creates the FIRST workspace.
There is also a **hard-coded application guard**: `create_workspace` (`bootstrap.rs:289`) returns
409 for any second workspace, beyond the DB index (see upstream-changes.md). OD-3 ratified a NEW
instance-level super-admin above workspace-admin as the provisioning authority.

## Options considered
### Role representation
- **(a) A new `instance_admins(user_id)` table.** Explicit, queryable, future-proof (could carry
  granted_at/granted_by). One small additive table.
- **(b) A boolean flag on `users` (`is_instance_admin`).** Simpler schema-wise but mixes an
  instance concern into the global user row and is awkward to audit/grant.
- **(c) A magic "workspace 1 admin = super-admin" convention.** No new schema, but conflates two
  distinct authorities and breaks the moment workspace 1 is not special.

### Provisioning surface
- **(d) A web flow under `/admin/instance/...`** (session + CSRF), reusing the shipped admin UI and
  the bootstrap/invite idiom to seed the new workspace's first admin.
- **(e) A CLI subcommand** (an `admin_cli` path already exists). Operator-friendly, no new routes.
- **(f) A `/api/v1` provisioning endpoint.** Rejected: would put a privileged mint-like creation
  path on the bearer surface; against the spirit of the no-mint boundary.

## Decision
**Role: (a) a new `instance_admins(user_id PK → users)` table** + an `is_instance_admin(user_id)`
authz function in `foundry-store`, surfaced through `foundry-services` (authz lives in services,
never adapters — the boundary guard holds). Minimal: just the membership of the role in v1.

**Surface: (d) a web flow under `/admin/instance/workspaces`** (session + CSRF), gated by
`is_instance_admin`, calling a NEW `create_workspace(name, first_admin_email)` use-case in
`foundry-services` that inserts the workspace + seeds its first admin via the shipped invite idiom.
EXTEND the existing `bootstrap.rs create_workspace` handler — remove the hard-coded 409, gate on
`is_instance_admin`, and perform the real creation. **Bootstrap** is extended so that initial
bootstrap creates workspace 1 AND the first `instance_admins` row (the operator who claims the
instance). (e) the CLI path is noted as a follow-up convenience, not v1-required.

## Consequences
- **Positive**: a clean, auditable, minimal role; provisioning stays off the bearer surface; reuses
  the admin-UI + invite seeding idioms; bootstrap remains the single "claim the instance" entry.
- **Negative**: one new table + one new authz function + one new admin route group (the smallest
  footprint that satisfies OD-3). The `check-arch` tenant-scoping rule (ADR-002) must allow-list the
  provisioning use-case, which legitimately writes a *literal* new workspace id (not a resolved one).
- **Security**: the new attack surface (workspace creation) is super-admin-gated and CSRF-protected;
  a non-super-admin attempt is refused (US-MWT07 scenario 3). Creating B never touches A
  (NFR-MWT-REL-01).

## Slice alignment
Lands in Slice 6 (provision a new tenant). The `instance_admins` table is created by the same
`0002` migration that drops the guard (ADR-006), so the role exists from the moment multi-workspace
is possible; the provisioning UI + use-case are the Slice 6 deliverable.
