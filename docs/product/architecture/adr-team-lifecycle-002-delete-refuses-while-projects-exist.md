# ADR-TEAM-LIFECYCLE-002: A team delete locks the team row FOR UPDATE, then decides "no projects" in a fresh statement, so a delete can never cascade over a project

- Status: Accepted (2026-10-11)
- Feature: `team-lifecycle` (DISCUSS D8, D9, D10, OQ-4 (a), INV-TL-1, INV-TL-2, INV-TL-3, INV-TL-8; DESIGN DDD-7 to DDD-10)

## Context

`teams` is the parent of `projects (team_id … ON DELETE CASCADE)`. Projects in turn cascade to
issues, lanes, `issue_change_events`, comments and inline attachments. A team delete also cascades
to `team_memberships` (0021's lead guard exempts a team that is gone) and
`machine_tokens.scope_team_id`, and sets `workspaces.default_team_id` to NULL (0022). The user chose
OQ-4 (a): refuse while the team has **any** project. INV-TL-1 requires that this is decided at
commit time. A project created while the confirm page was open, or in the same millisecond as the
delete, is either seen (and the delete refused) or finds the team gone (and its create answers the
uniform 404). No project row is ever removed by a team delete. INV-TL-2 requires that no racing
write answers 500.

The store runs Postgres 16 at READ COMMITTED. Every child insert takes `FOR KEY SHARE` on its parent
row through its FK check. That covers a project, a team membership, a scoped machine token, and
`workspaces.default_team_id` through the 0022 composite FK. Lock conflicts: `FOR UPDATE` conflicts
with `FOR KEY SHARE`, and `FOR NO KEY UPDATE` does not (the property ADR-TEAM-MEMBERSHIP-001 relies
on to keep adds unblocked).

There is no project delete or move in production code. There is no production workspace delete.
One acceptance step deletes a workspace that has projects (`feature_recipient_notification_preferences.rs:1705`).
`delete_team_by_hand` and two store tests delete teams in psql style.

## Decision

1. **`Store::delete_team_if_empty(workspace_id, team_id, actor_id) -> TeamDeleteOutcome`.** One
   transaction, in exactly this statement order:
   1. `SELECT name, slug FROM teams WHERE id = $team AND workspace_id = $ws FOR UPDATE`. No row:
      roll back, `NotFound`.
   2. `SELECT name, key_prefix FROM projects WHERE team_id = $team ORDER BY name, key_prefix`. Any
      rows: roll back, `HasProjects { projects }`.
   3. `DELETE FROM teams WHERE id = $team`. This cascades memberships and scoped tokens and nulls
      the default.
   4. `INSERT INTO team_lifecycle_events` (`kind = 'deleted'`; see ADR-TEAM-LIFECYCLE-003).
   5. Commit. The result is `Deleted { name, slug }`.
2. **Why this closes the race.** Let P be a concurrent project create on the same team.
   - **P's FK check ran first.** P holds `KEY SHARE` on the team row, so statement 1 waits until P
     ends. If P commits, statement 1 is granted, and statement 2 is a new statement with a new
     snapshot. It sees P's project, and the delete refuses. If P rolls back, statement 2 sees
     nothing, and the delete proceeds.
   - **Statement 1 ran first.** From then until commit, every `KEY SHARE` on the row waits. P's FK
     check waits. If the delete commits, the row is gone, and P's check fails with 23503. P is
     mapped to the uniform 404 (decision 5). If the delete refuses (rollback), P proceeds.

   In both orders, statement 3 cannot cascade over a project. A project inserted between statements 2
   and 3 would need the `KEY SHARE` that statement 1 is holding off. The same argument covers any
   future `UPDATE projects SET team_id` (a move), which also takes `KEY SHARE` on the new parent.
3. **Two traps, pinned by negative-control gold tests.**
   - (a) `FOR NO KEY UPDATE` in statement 1 is **wrong**: it does not conflict with `KEY SHARE`, so
     a project can commit between the check and the delete and then be cascaded.
   - (b) A single statement, `DELETE FROM teams WHERE id = $1 AND NOT EXISTS (SELECT … projects …)`,
     is **wrong** at READ COMMITTED. The `NOT EXISTS` is evaluated against the statement's starting
     snapshot. When the DELETE then waits on P's `KEY SHARE` and P commits, the team tuple itself is
     unmodified, so there is no recheck at all. The delete proceeds, and the RI cascade (which reads
     the latest snapshot) finds and deletes P's project.

   The lock and the check must be separate statements.
4. **The lead guard (INV-TL-8).** Statement 3 deletes the team row first. The cascade to
   `team_memberships` is an AFTER-row RI action, so it runs later in the same command, with the
   deletion visible. 0021's `foundry_team_keeps_a_lead()` then finds no team row and returns `OLD`
   (ADR-TEAM-MEMBERSHIP-001 fact 2, already gold-tested with `DELETE FROM teams`). Its own `FOR NO
   KEY UPDATE` on the team is not needed: the delete already holds the stronger lock. Other effects:
   - The 0022 `SET NULL (default_team_id)` updates only `default_team_id`. `workspaces_name_rule_on_rename`
     is `UPDATE OF name`, so it does not fire.
   - Scoped tokens are deleted, which revokes them (D10).
5. **INV-TL-2: every writer that can lose its team to a delete maps the loss to its normal outcome.**
   The store recognises SQLSTATE 23503 by constraint name, the same pattern as `is_lead_rule_refusal`.

   | Writer | Constraint | Mapped to |
   |---|---|---|
   | `insert_project` | `projects_team_id_fkey` | new `ProjectInsertError::TeamGone` → `CreateProjectError::TeamGone` → uniform 404 |
   | `add_team_member` | `team_memberships_team_id_fkey` | new `AddTeamMemberOutcome::TeamGone` → `TeamError::NotFound` |
   | `set_default_team` | `workspaces_default_team_same_workspace` | new outcome → `TeamError::NotFound` |
   | `insert_machine_token` (team scope) | `machine_tokens_scope_team_id_fkey` | `Validation{scope_team_not_in_workspace}`, the shipped refusal for a team that is not in this workspace (422) |

   Unchanged writers:
   - `set_team_role` / `remove_team_member`: their `FOR NO KEY UPDATE` waits, then finds no row, the
     standing read returns `NotOnTeam`, and the page re-read returns `NotFound`.
   - A rename's locked read finds no row and returns `NotFound`.
   - Newcomer placement runs in its savepoint (D17).
6. **The confirm page is advisory (INV-TL-3).** `GET /team/{slug}/delete` reads
   `Store::team_delete_facts(team_id)` with no lock: member count, whether the team is the default,
   the number of scoped tokens, and the project list. With projects, it renders the refusal and no
   Delete button. The POST decides by decision 1 alone. A stale confirm gets the refusal naming the
   new project, with 409.
7. **Enforcement.**
   - check-arch `team-lifecycle-one-seam`: `DELETE FROM teams` (and `INSERT INTO` / `UPDATE teams`)
     appear only in `foundry-store` src. `delete_team_if_empty(` is never called from `foundry-app`
     or `foundry-api`. The guarded method is the only production team deleter.
   - The authority for delete is `TeamAccess::Administer` (ADR-TEAM-LIFECYCLE-004).

## Alternatives considered

- **Change `projects.team_id` to `ON DELETE RESTRICT` or `NO ACTION`, so the database refuses.** This
  holds for every caller and is race-safe by the same lock conflict. Rejected:
  - `RESTRICT` is checked immediately, so a workspace delete (which cascades to teams, while the
    projects also hang off `workspace_id`) would be refused whenever the workspace has a project.
    That breaks the acceptance "workspace is deleted" scenario (OQ-D6 of team-membership's DESIGN is
    its regression guard for the lead guard) and any future workspace delete.
  - `NO ACTION` defers the check to the end of the cascading SPI query, so whether a workspace delete
    passes depends on the firing order of the `workspaces` RI triggers. That is fragile, and nothing
    states it.
  - Both rewrite a 0001 constraint (`ALTER TABLE … DROP/ADD CONSTRAINT`, a SHARE ROW EXCLUSIVE lock
    on `projects` and `teams` during rollout), and the refusal copy would still need the project
    list from a separate read.
- **A `BEFORE DELETE ON teams` trigger that refuses while projects exist** (exempt when the
  workspace row is gone, 0021's idiom). This is correct and covers psql. Rejected for now:
  - INV-TL-1 governs the product delete, and check-arch makes the guarded store method the only
    production deleter.
  - The trigger would change the shipped psql semantics that `delete_team_by_hand` and two store
    tests rely on.
  - It would have to be dropped or bypassed again when OQ-4 (b) (cascade after a full confirm) lands.

  It is the designated backstop if a second production team deleter appears.
- **SERIALIZABLE with retry, or `pg_advisory_xact_lock(team)`.** Rejected, as in
  ADR-BOARD-LANE-002 and ADR-TEAM-MEMBERSHIP-001: they protect only writers that opt in. The FK's
  own `KEY SHARE` is taken by every project insert, so the row lock needs no cooperation.
- **Count at confirm time and trust it.** This violates INV-TL-3 by construction.

## Consequences

- Positive: INV-TL-1 holds by the FK lock that every project insert already takes, with no new
  isolation level, no retry loop and no schema change to `projects`.
- Positive: the refusal names every project from the same locked view that decided it.
- Negative: for the milliseconds of the delete transaction, every child insert on that team waits:
  members, tokens, projects and the default. Each then gets 23503, which is mapped. This is
  negligible at homelab scale.
- Negative: psql can still cascade-delete a team with projects. That is a deliberate operator act,
  outside D9's product guarantee. It is documented in the brief.
- Negative: an older (v0.13.0) replica during a rolling deploy has no `TeamGone` mapping, so its
  project create racing a delete answers 500. No project is lost. The window is the rollout.
- Earned Trust (store gold tests on `postgres:16`):
  - Statement 1 blocks an FK-checked `INSERT INTO projects` (timeout assertion).
  - After a lock wait, statement 2 sees the committed project.
  - Negative controls (a) and (b) lose the project.
  - A barrier race, delete versus project create, over at least 100 runs: 0 projects destroyed and
    0 errors outside the mapped set.
  - The 23503 constraint names in the table above are the ones Postgres reports.
  - A delete of a team with a sole lead succeeds (the cascade exemption).
  - Deleting the default team nulls only the default.
