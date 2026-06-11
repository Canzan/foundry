# ADR-004 — Existing-install migration-safety guarantee (slice 5)

## Status
Proposed (Propose mode). FIRMS the migration-guarantee half of the parent
`multi-workspace-tenancy` ADR-006 (the `0009`/`0010` schema change already shipped; the
upgrade-safety PROOF was deferred here). OD-4 ratified: forward-only, no data touch.

## Context
Every existing Foundry install is single-workspace. The `0009` migration already DROPped
`uniq_one_workspace` and `0010` added `users.active_workspace_id` — both shipped in the parent
milestone. Slice 5's job is the **user-visible upgrade-safety GUARANTEE**: prove against a REAL
pre-feature DB snapshot that the upgrade is forward-only, loses ZERO data, leaves the existing
workspace as workspace 1 with its id unchanged, and keeps existing sessions/tokens/sign-in
resolving. The genuinely-open design question is: **does any backfill migration exist?**

Grounding (read the code):
- `resolve_active_workspace` (`lib.rs:419-436`):
  `SELECT w.id … JOIN workspace_memberships m … WHERE m.user_id=$1 ORDER BY (w.id =
  u.active_workspace_id) DESC, w.id LIMIT 1`. For an upgraded user, `active_workspace_id` is NULL
  (the `0010` column defaults NULL) and they have exactly ONE membership ⇒ the JOIN yields that
  one workspace; the `ORDER BY` tiebreak is moot. The user resolves deterministically to
  workspace 1 **with no value written**.
- `set_active_workspace` (`lib.rs:453`) writes `active_workspace_id` only on an explicit switch and
  only when membership holds. It is never required for resolution.
- `0010` uses `ADD COLUMN IF NOT EXISTS … REFERENCES workspaces(id) ON DELETE SET NULL` — additive,
  nullable, no rewrite.
- `0011` (this feature, ADR-003) adds `instance_admins` — additive, empty, no rewrite.

## Options considered
### Backfill?
- **(a) NO backfill migration — NULL active workspace resolves to the sole membership.** The
  shipped `resolve_active_workspace` already maps (NULL active ws + one membership) ⇒ that
  workspace, deterministically. Nothing to backfill.
- **(b) A backfill migration setting `active_workspace_id = <the sole workspace>` for every
  existing user.** Would write a value into every upgraded user row — a data *rewrite* the
  forward-only no-touch discipline (OD-4) specifically avoids, for ZERO functional gain over (a).
  REJECTED: it contradicts OD-4 and changes rows the guarantee promises to leave untouched.

### How to PROVE upgrade safety
- **(c) A real-snapshot before/after-equality acceptance test.** Seed a pre-0009 schema+data
  state (or restore a real pre-feature dump), capture row-level checksums/counts for every tenant
  table, apply `0009`/`0010`/`0011`, re-capture, assert byte/row equality + the workspace id
  unchanged + a carried-over session AND machine token still resolve to workspace 1 + the existing
  auth/workspace acceptance suites stay green.
- **(d) A migration code-review + assertion only (no real-data test).** Cheaper but trusts the
  migration's contract instead of probing it — exactly the act-of-faith Earned Trust forbids for
  the single highest-stakes data-safety step. REJECTED.

## Decision
**No backfill migration (option a) + a real-snapshot upgrade-safety PROOF (option c).** Slice 5
ships NO new data migration beyond `0011`'s additive `instance_admins` table; the proof is a
`foundry-acceptance` scenario that:
1. seeds a REAL pre-feature single-workspace DB (pre-0009 schema + representative data: workspace,
   users, memberships, team, project, issues, invites, a live session row, a machine token);
2. records row counts + checksums for `workspaces, users, workspace_memberships, teams,
   team_memberships, projects, issues, invites` and the workspace id;
3. applies the forward-only migrations (`0009`, `0010`, `0011`);
4. asserts row-level before/after EQUALITY across all tenant tables, the workspace id UNCHANGED,
   the carried session AND machine token STILL resolve to workspace 1
   (`resolve_active_workspace` → workspace 1 with `active_workspace_id` still NULL), and the
   existing auth + workspace acceptance suites stay green post-migration.

The detailed test construction is a DISTILL/DELIVER concern; DESIGN's ruling is the **no-backfill
finding** and the **probe-don't-assume PROOF** shape.

## Consequences
- **Positive**: the smallest possible upgrade (no row rewrite at all — `0009` drops an index,
  `0010`/`0011` add a nullable column + an empty table); the guarantee is *probed* against a real
  snapshot (Earned Trust); existing sessions/tokens/sign-in keep working because resolution maps
  NULL-active + sole-membership ⇒ workspace 1.
- **Negative**: the proof needs a real pre-feature DB fixture (a seeded pre-0009 state or a stored
  dump) — modest test-fixture effort, intentionally so (the rigor is the point).
- **Security/Reliability**: forward-only, no row touched; re-apply is a no-op (`IF EXISTS` /
  `IF NOT EXISTS`); a stale `active_workspace_id` can never scope a user to a foreign tenant
  (resolution honours the column only through a live membership JOIN).

## Relationship to parent ADR-006
FIRMS parent ADR-006's deferred "Verification (Earned Trust)" clause (US-MWT06 against a real
pre-feature DB snapshot) and resolves the open backfill question explicitly: **no backfill —
NULL-resolves-fine**. The `0011 instance_admins` table referenced here is the same table the
parent ADR-006 folded into its migration; in this feature it is its own additive `0011`.
</content>
