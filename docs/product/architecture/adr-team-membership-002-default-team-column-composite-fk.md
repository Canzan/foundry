# ADR-TEAM-MEMBERSHIP-002: A workspace's default team is a nullable column with a same-workspace composite foreign key that clears itself, and newcomers are placed on it inside a savepoint

- Status: Accepted (2026-10-09)
- Feature: `team-membership` (DISCUSS D17, OQ-3 to OQ-8, INV-7; DESIGN DDD-12 to DDD-15)

## Context

A workspace admin picks at most one default team per workspace (OQ-4). Unset on upgrade means
behaviour is unchanged. Newcomers then land on it as Member: a member-invite accept
(`Store::create_member_and_consume`) and a Keycloak first sign-in that provisions
(`Store::provision_federated_member`, the instance's oldest workspace) both apply it, in the same
transaction as the workspace membership (OQ-5). Linking an existing account adds no team.

INV-7: the default must always be one of the workspace's own teams, or none, including after a
team is deleted by any means. OQ-6: a deleted default clears itself. D17: an accept or a sign-in is
never refused, and never fails, because of the default. Rolling deploy: an older replica ignores
the setting.

There is no team delete today, so self-clearing guards future features and hand-written SQL.
Precedent: `users.active_workspace_id … ON DELETE SET NULL` (0010) is a self-clearing pointer.
`TENANT_TABLES` is fixed at ten, pinned by a gold test and mirrored in `verify_export`, so a new
table is not a free choice.

## Decision

1. **Schema (migration `0022_workspace_default_team.sql`, additive):**
   - `ALTER TABLE teams ADD CONSTRAINT teams_workspace_id_id_key UNIQUE (workspace_id, id)`. This
     is redundant as uniqueness (`id` is the PK), but it is required as a composite FK target.
   - `ALTER TABLE workspaces ADD COLUMN default_team_id UUID NULL`. It is NULL for every existing
     row, so the default is unset on upgrade.
   - `ALTER TABLE workspaces ADD CONSTRAINT workspaces_default_team_same_workspace FOREIGN KEY (id,
     default_team_id) REFERENCES teams (workspace_id, id) ON DELETE SET NULL (default_team_id)`.

   Under `MATCH SIMPLE` a NULL default is never checked. A non-NULL default must name a team whose
   `workspace_id` is this row's `id`, so INV-7 is a schema fact for every caller. Deleting the team
   nulls only `default_team_id`. The column-list form needs Postgres 15 or later. Production and
   the harness pin 16, and an older server fails the migration loudly rather than installing
   something weaker.
2. **One placement statement, one home.** A private store helper,
   `place_on_default_team(tx, workspace_id, user_id)`, runs `INSERT INTO team_memberships (team_id,
   user_id, role) SELECT w.default_team_id, $user, 'member' FROM workspaces w WHERE w.id = $ws AND
   w.default_team_id IS NOT NULL ON CONFLICT (team_id, user_id) DO NOTHING`. The composite FK
   already guarantees the team is in the workspace, so the statement needs no join.
3. **Placement is best-effort inside the newcomer's transaction (D17).** Both callers run the
   helper right after their `workspace_memberships` INSERT, inside a savepoint (a nested sqlx
   transaction). On any error from that statement, the savepoint is rolled back, one `warn` is
   logged with the workspace and user ids (no email), and the outer transaction commits the
   account and membership as today. Placement only applies on `create_member_and_consume`'s
   consumed path and on `provision_federated_member`'s `Created` branch. The `Existing` branch
   (linking) and the first-admin invite (`set_first_admin_password_and_consume`) never place
   anyone.
4. **Setting it.** `POST /teams/default` (workspace admin only) carries `team` as a slug resolved
   in the acting workspace, or empty for "No team". One `UPDATE workspaces SET default_team_id =
   $team WHERE id = $ws` follows, and the last writer wins. An unknown slug is the uniform 404,
   and nothing is written.
5. **Probe.** `Store::probe` refuses readiness without `workspaces.default_team_id` (the
   0017/0018 precedent), because `/teams` and both newcomer paths read it.

## Alternatives considered

- **A separate `workspace_default_teams (workspace_id PK, team_id, FK (workspace_id, team_id) →
  teams ON DELETE CASCADE)` table.** It is equally correct for INV-7 and self-clearing, and it
  avoids the circular FK. Rejected: it is a new tenant table. Either it joins `TENANT_TABLES`
  (moving the fixed ten, the plant-a-row gold test, `verify_export`, and `run_backup_verify`'s
  list), or it is deliberately excluded and a per-workspace export silently drops the setting.
  Neither is worth one nullable value.
- **`teams.is_default boolean` with a partial unique index `(workspace_id) WHERE is_default`.**
  INV-7 would hold trivially and deletion would self-clear, with no circular FK. Rejected: changing
  the default is a two-row write. Two admins saving at once race into a 23505 on the partial index
  unless the workspace row is locked first, a protocol the column gets for free. It also stores a
  workspace setting on a team.
- **A plain `default_team_id REFERENCES teams(id) ON DELETE SET NULL` plus an app check.**
  Rejected: INV-7 would then hold only at the app door. psql or a future caller could point a
  workspace at another workspace's team.
- **Placement without a savepoint.** Rejected: any error in the placement statement (for example,
  a future concurrent team delete) would abort the accept transaction, which D17 forbids. Two extra
  round trips buy the guarantee.

## Consequences

- Positive: INV-7 and OQ-6 are schema facts, the setting is one cell, and placement is one
  statement shared by both doors.
- Positive: rolling-deploy safe. An older replica never reads the column, its `INSERT INTO
  workspaces (id, name)` leaves it NULL, and an accept on it lands the newcomer on no team, which
  is today's behaviour.
- Negative: `workspaces` and `teams` now reference each other. `pg_dump` creates FKs post-data,
  so restore is unaffected. A workspace delete cascades its teams, and their SET NULL touches a row
  that is itself being deleted. Store test: deleting a workspace whose default is set succeeds.
- Negative: the per-workspace export's `workspaces` row gains a `default_team_id` key (it exports
  whole rows). DELIVER confirms the export gold test does not pin the key set.
- Earned Trust: store tests on `postgres:16` prove:
  - a foreign-workspace default is refused (23503);
  - deleting the default team nulls only `default_team_id`;
  - a placement failure forced inside the savepoint leaves the account and membership committed.
