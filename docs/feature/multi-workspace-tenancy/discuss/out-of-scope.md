# Out of Scope — Multi-Workspace Tenancy

Explicit non-goals for this feature. Each is either deferred, owned by DESIGN, or a separate
future feature. Listed so DESIGN/DISTILL do not over-build and so reviewers do not flag absence
as a gap.

## Deferred (future features / follow-ups)

- **Per-workspace backup / restore.** v1 keeps WHOLE-INSTANCE backup/restore (the shipped US-03
  restore machinery, unchanged). Per-tenant export/restore — and the isolation-sensitive
  guarantee that restoring one workspace cannot clobber a sibling — is a separate, harder
  feature. (OD-5 / DM7.)
- **Self-serve workspace signup.** v1 provisions workspaces via the instance operator / a new
  instance-level super-admin only (OD-3 / DM6). Public self-serve signup (anyone can create a
  workspace) is deferred.
- **Cross-workspace user experience features.** A "switch workspace" UX may exist (it follows
  from multi-membership, OD-2), but cross-workspace dashboards, cross-tenant search, or
  aggregating data across workspaces a user belongs to are NOT in scope — they would cut against
  the isolation boundary and need their own design.
- **Per-workspace billing / quotas / resource limits.** No metering, quotas, or per-tenant
  rate/storage caps beyond the existing guardrails (the rate-bucket eviction in US-MWT08 is
  about BOUNDING an in-memory map, not introducing per-tenant quotas).
- **Workspace deletion / archival / data export-on-offboarding.** Removing a tenant and its data
  (GDPR-style export/erase) is not in this feature.
- **Read/write or finer-grained token scopes.** Token scope stays workspace-or-team
  (`scope_team_id`) as shipped; no read/write split is introduced here (carried from the token
  features' Q3 ratification).

## Owned by DESIGN (DISCUSS fixes the requirement, not the mechanism)

- **The tenant model ratification** (shared-schema vs schema-per-tenant vs db-per-tenant) —
  DISCUSS recommends shared-schema-with-`workspace_id` (DM3/OD-1); DESIGN ratifies.
- **The request→workspace resolution mechanism** — session claim vs URL segment vs
  host/subdomain vs token claim (DM1). DISCUSS fixes that resolution yields EXACTLY one
  workspace, fail-closed.
- **The workspace-selection / switcher UX** for multi-membership users (OD-2/DM4).
- **The instance-level super-admin role shape** and the provisioning surface (OD-3/DM6).
- **The exact per-surface refusal status/shape** (e.g. 404 vs 403) — DISCUSS fixes that it is
  uniform and non-enumerable (NFR-MWT-SEC-02).

## Carried invariants (NOT changed by this feature)

- The machine-token verify path, the per-request `jti` denylist, and `iss`/`aud`/EdDSA pinning
  are unchanged — this feature scopes WHICH workspace a principal acts on, not how a token is
  verified.
- The browser auth/CSRF/session contract is preserved (workspace resolution is added on top, it
  does not weaken auth).
- ONE binary, ONE Postgres, no Redis, no Node runtime service, no CDN.
