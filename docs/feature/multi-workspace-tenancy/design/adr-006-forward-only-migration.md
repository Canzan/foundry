# ADR-006 — Forward-only migration off `uniq_one_workspace`

## Status
PARTIALLY IMPLEMENTED (milestone, 2026-06-11). The forward-only `0009`/`0010` migrations (drop `uniq_one_workspace` + add active-workspace) SHIPPED in slice 1. The formal existing-install upgrade-safety GUARANTEE (before/after row-equality + sessions/tokens keep resolving) is DEFERRED to follow-up (`multi-workspace-provisioning`, slice 5). OD-4 ratified: forward-only, no data touch.

## Context
Multi-workspace requires removing the single-workspace guard (`CREATE UNIQUE INDEX
uniq_one_workspace ON workspaces ((true))`, `0001_init.sql:15`) and adding the instance-admin role
(ADR-004). Every existing install is single-workspace; the upgrade must be forward-only (ADR-003
discipline used by the shipped features), rewrite/move/delete NO existing row, leave the existing
workspace as "workspace 1" with its id unchanged, and keep existing sessions/tokens/sign-in working
(NFR-MWT-DATA-01/02). DISCUSS assumption #6 asserts "no query depends on `uniq_one_workspace` for
correctness" and asks DESIGN to audit for un-scoped `FROM teams|projects|issues|invites` reads
before dropping the index.

## Pre-drop un-scoped-query audit (assumption #6 — DESIGN finding)
Grepped `foundry-store/src` for `FROM teams|projects|issues|invites`:
- **Scoped or parent-scoped (safe)**: `list_issues_by_project(project_id)`,
  `count_issues(project_id)`, the comment subqueries, `find_team_by_slug(workspace_id, slug)`,
  `find_project_by_slug(team_id, slug)`, `team_exists_in_workspace(team_id, workspace_id)` — each
  is keyed by `workspace_id` directly OR by a parent id (project/issue/team) that was itself
  resolved within the acting workspace. None relies on "there is one workspace."
- **`SELECT id, name FROM workspaces LIMIT 1` (`first_workspace`, `lib.rs:389`)** — the ONLY query
  that depends on "there is one workspace." Its sign-in call-site is replaced by membership
  resolution (ADR-005); the function may remain for the migration default (one membership → that
  workspace) but is no longer the resolution authority.
- **`SELECT expires_at FROM invites WHERE id = $1` (`lib.rs:427`)** and **`SELECT name FROM teams
  WHERE id = $1` (`lib.rs:546`)** — un-scoped single-row lookups by primary key. These do NOT
  depend on `uniq_one_workspace`, but they are **un-scoped tenant reads** that the ADR-002
  tenant-scoping guard should flag: they should take an `acting_workspace_id` (add `AND
  workspace_id = $2` for invites; resolve the team within the acting workspace for the name lookup)
  to be safe under multiple tenants. **This is a DESIGN finding — see upstream-changes.md** (it
  refines, not contradicts, assumption #6: nothing depends on the guard, but two reads are
  un-scoped and must be tightened before/with the boundary work).

## Decision
A NEW forward-only migration `0002_multi_workspace.sql` (edits no prior migration):
1. `DROP INDEX uniq_one_workspace;` — removes the single-workspace cap. Dropping an index touches
   no row data.
2. `CREATE TABLE instance_admins (user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
   created_at TIMESTAMPTZ NOT NULL DEFAULT now());` — the instance super-admin role (ADR-004),
   additive, empty until bootstrap/back-fill seeds it.
3. (Optional, application-time) seed the FIRST `instance_admins` row from the bootstrap operator on
   a fresh install; on an UPGRADED install, the operator is granted via the bootstrap/claim flow —
   the migration itself inserts no admin (no assumption about which existing user is the operator).

The existing workspace's `workspace_id` FKs already point at it, so it remains workspace 1 with its
id unchanged and zero data rewrite. Resolution defaults every existing session to that one workspace
(ADR-005 single-membership path). The forward-only migration is no-op on re-apply (already-applied).

## Consequences
- **Positive**: smallest possible schema change (drop one index, add one table); zero data rewrite;
  existing sessions/tokens/sign-in keep working; idempotent in effect; reversible-by-omission (a new
  forward migration could re-add a guard if ever needed, though OD-4 is forward-only).
- **Negative**: the two un-scoped reads (invites/team-name) must be tightened (upstream-changes.md);
  this is a small additional scope item, surfaced honestly rather than discovered at DELIVER.
- **Verification (Earned Trust)**: US-MWT06 runs the migration against a REAL pre-feature DB
  snapshot and asserts before/after row equality + checksums across all tenant tables, the workspace
  id unchanged, and the existing auth/workspace acceptance suites green — the migration's contract is
  *probed*, not assumed.

## Slice alignment
The migration ships in Slice 1 (walking skeleton); its user-visible safety guarantee is proven in
Slice 5 (US-MWT06, real pre-feature DB). The `instance_admins` table it creates is used by Slice 6.
