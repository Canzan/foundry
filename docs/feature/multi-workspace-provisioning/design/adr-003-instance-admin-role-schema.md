# ADR-003 — `instance_admins` schema + `is_instance_admin` authz seam

## Status
Proposed (Propose mode). FIRMS the role-representation half of the parent
`multi-workspace-tenancy` ADR-004. Awaits user ratification (flagged in `wave-decisions.md`).

## Context
OD-3 ratified a NEW instance-level super-admin above workspace-admin. Today the only roles are
per-workspace (`workspace_memberships.role CHECK (role IN ('admin','member'))`, `0001_init.sql:29`);
there is NO instance-level role. The provisioning use-case (ADR-002) needs a single authz gate,
and that gate must stay OFF the tenant-scoped boundary: a super-admin is NOT a workspace member,
and creating a workspace is a deliberate NON-tenant-scoped action (it produces a literal new
workspace id, not a resolved acting one), so it must not be forced through — nor trip — the
LAYER-1e tenant-scoping guard.

Grounding (read the code):
- `is_workspace_admin(workspace_id, user_id)` (`lib.rs:1128`) is `EXISTS (SELECT 1 FROM
  workspace_memberships WHERE workspace_id=$1 AND user_id=$2 AND role='admin')` — the canonical
  role-authz shape.
- The LAYER-1e guard (`check_arch.rs:332`) flags a `foundry-app` handler that scopes a tenant
  query by a *request-parsed* workspace id. Its allow-list (`is_tenant_scoping_allowlisted`,
  `:387-396`) ALREADY exempts `bootstrap` and `admin_cli` precisely because the
  workspace-creating / resolution paths legitimately handle a literal or parsed id.

## Options considered
### Role representation
- **(a) A new `instance_admins(user_id)` table.** Explicit, queryable, auditable (carries
  `created_at`, future `granted_by`). One small additive table. Mirrors the relational membership
  model already in use.
- **(b) A boolean flag on `users` (`is_instance_admin`).** Fewer objects but mixes an instance
  concern into the global user row, is awkward to audit/grant/revoke, and bloats every `users`
  read with a column most rows don't care about.
- **(c) A "workspace-1 admin == super-admin" convention.** No new schema, but conflates two
  distinct authorities and breaks the instant workspace 1 is not special (e.g. after the operator
  separates the two per ADR-001).

### Keeping the gate off the tenant boundary
- **(i) Provisioning lands in already-allow-listed files (`admin_cli`, `bootstrap`).** No new
  LAYER-1e entry; the guard stays precise.
- **(ii) Provisioning lands in a NEW file** ⇒ MUST add that file's stem to
  `is_tenant_scoping_allowlisted`, else the build fails the moment the provisioning insert names a
  literal workspace id.

## Decision
**(a) A new `instance_admins(user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
created_at TIMESTAMPTZ NOT NULL DEFAULT now())` table** (created by migration `0011`, ADR-004
companion), plus **`is_instance_admin(user_id) -> bool`** in `foundry-store` (the `EXISTS (SELECT 1
FROM instance_admins WHERE user_id=$1)` shape, mirroring `is_workspace_admin`) surfaced through
`foundry-services` (authz lives in services/store, never in adapters — the dependency-direction
guard holds). Minimal: just the role membership in v1 (no per-action grants).

**Keeping it off the tenant boundary — option (i):** the v1 provisioning surface is the
`admin_cli` CLI (ADR-002) and the first-super-admin seed is in `bootstrap`. Both file stems are
ALREADY in `is_tenant_scoping_allowlisted` (`check_arch.rs:394`), so **no new allow-list entry is
required for v1**. `is_instance_admin` takes NO workspace argument and is evaluated against the
instance, so it is structurally not a tenant-scoped call — it cannot trip the LAYER-1e detector
(which only fires on `*_in_workspace(` calls fed a parsed id). **If the deferred web flow (ADR-002,
option d) later lands in a NEW file, that file's stem MUST be added to
`is_tenant_scoping_allowlisted`** — recorded here so DELIVER does not rediscover it.

## Consequences
- **Positive**: a clean, auditable, minimal role (one table, one `EXISTS` function); the authz
  gate is instance-scoped by construction and cannot be confused with a tenant scope; v1 needs NO
  check-arch change; mirrors the shipped `is_workspace_admin` idiom so it inherits the same review
  + test discipline.
- **Negative**: one new table + one new authz function (the smallest footprint that satisfies
  OD-3). A future web surface in a new file owes one allow-list line (documented above).
- **Security**: the super-admin set is the sole authority for provisioning; `is_instance_admin` is
  fail-closed (absent row ⇒ refused); it never crosses into workspace authority (a super-admin has
  no implicit membership/admin rights in any workspace — those remain `workspace_memberships`).

## Relationship to parent ADR-004
FIRMS parent ADR-004's chosen representation (option a — the `instance_admins` table) and its
`is_instance_admin` authz function, and makes the LAYER-1e allow-list reasoning explicit + tied to
the ADR-002 CLI-first surface (so v1 needs no new allow-list entry, unlike the parent's web-flow
assumption which would have).
</content>
