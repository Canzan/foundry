# ADR-PROJECT-LIFECYCLE-004: A team delete is refused only by live projects; before deleting the team row it detaches every remaining (deleted) project, so the cascade reaches no project, and a CHECK makes detaching a live project impossible

- Status: Accepted (2026-10-12)
- Feature: `project-lifecycle` (DISCUSS OQ-1 (b), US-PL-05, INV-PL-6; DESIGN DDD-19 to DDD-21)
- Amends: ADR-TEAM-LIFECYCLE-002 decision 1 (statements 2 and 3), and with it team-lifecycle INV-TL-1 and OUT-25

## Context

OQ-1 (b), user: a soft-deleted project no longer blocks a team delete. Its row is kept, detached
from the deleted team, and it cannot be restored until a future "restore into another team"
(D-C: no move, no purge). INV-PL-6 has four parts:
- a team delete commits only if the team has no live project at commit time;
- its deleted projects survive with every row;
- a restore racing a team delete either restores first (the delete is refused, naming it) or finds
  the team gone (exit 4);
- a project delete racing a team delete, in either order, leaves no live project destroyed.

It amends INV-TL-1 to "no **live** project row, and no row of any project, is ever removed by a team
delete".

Today `projects.team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE` (0001).
`delete_team_if_empty` makes these four moves (ADR-TEAM-LIFECYCLE-002):
1. it locks the team `FOR UPDATE`;
2. it lists **all** its projects in a fresh statement and refuses on any;
3. it deletes the team;
4. it records the delete.

A workspace delete (an acceptance step; there is no product door) cascades to `teams` and to
`projects` through two FKs.

## Decision

1. **Schema, migration `0027_project_detach.sql` (slice 04):**
   - `ALTER TABLE projects ALTER COLUMN team_id DROP NOT NULL`.
   - `ADD COLUMN detached_from_team_id UUID NULL`, with **no FK**. It is the id of the team that
     was deleted, and it correlates with that team's `team_lifecycle_events` `deleted` record.
   - `ADD CONSTRAINT projects_team_or_detached CHECK ((team_id IS NOT NULL AND
     detached_from_team_id IS NULL) OR (team_id IS NULL AND detached_from_team_id IS NOT NULL AND
     deleted_at IS NOT NULL))`. A live project always has a team. Only a deleted project can be
     detached, and a detached project names the team it lost.
   - `projects_team_id_fkey` stays `ON DELETE CASCADE`, so psql and workspace-delete semantics are
     unchanged.
   - `CREATE OR REPLACE VIEW live_projects` picks up the new column
     (ADR-PROJECT-LIFECYCLE-001 decision 2), and the self-check repeats.
   - `UNIQUE (team_id, slug)` treats NULLs as distinct, so a detached project frees its name and
     slug with its team. `UNIQUE (workspace_id, key_prefix)` is untouched, so its key stays taken
     in the workspace (OQ-4, as the D14 team-delete line says).
2. **`delete_team_if_empty`, new statement order** (same lock, same record, one new statement):
   1. `SELECT name, slug FROM teams WHERE workspace_id = $ws AND id = $team FOR UPDATE`
      (unchanged).
   2. The team's **live** projects, `SELECT name, key_prefix FROM live_projects WHERE team_id =
      $team ORDER BY name, key_prefix`, in a fresh statement. Any row: roll back, `HasProjects`,
      naming live projects only.
   3. **New:** `UPDATE projects SET team_id = NULL, detached_from_team_id = $team WHERE team_id =
      $team`. This deliberately touches **every** remaining row of the team, not only the deleted
      ones. After statement 2 every remaining row is deleted. If a live row were ever still there,
      `projects_team_or_detached` would refuse the statement (23514) and the whole delete would roll
      back. It could never be cascaded.
   4. `DELETE FROM teams WHERE id = $team`. The FK cascade now finds no project row. Memberships
      and scoped tokens cascade, and the default clears, as before.
   5. The `deleted` record in `team_lifecycle_events`, unchanged in shape (US-PL-05 AC).
   6. Commit.

   `TEAM_PROJECTS_SQL` becomes the live list, and it lives in `project_lifecycle.rs` with the
   detach. `team_delete_facts` gains `deleted_projects: Vec<(name, key)>` and `workspace_name`, so
   the confirm page can render the D14 "also has {k} deleted project(s)" line when there are deleted
   projects and no live ones.
3. **Why every race is safe.**
   - **Project create versus team delete.** Unchanged (ADR-TEAM-LIFECYCLE-002): the insert's FK
     `KEY SHARE` conflicts with statement 1.
   - **Restore versus team delete.** A restore takes the team row `FOR KEY SHARE` before it locks
     the project (ADR-PROJECT-LIFECYCLE-002 decision 4).
     - Restore first: statement 1 waits. Statement 2, a fresh statement, sees the restored project
       live and refuses, naming it.
     - Team delete first: the restore's `KEY SHARE` waits. When the delete commits, the team row is
       gone, and the restore re-reads the now-detached project: `TeamGone`, exit 4.

     Both orders lock team → project, so they cannot deadlock.
   - **Project delete versus team delete.** A project delete never takes the team lock. If it has
     not committed by statement 2, statement 2 sees the project live and refuses with 409. If it
     committed first, statement 3 detaches it. Neither order touches a live project, and the CHECK
     backs both.
   - **Writes to a deleted project during the team delete.** There are none: the write guard
     refuses them (ADR-PROJECT-LIFECYCLE-002 decision 2).
4. **Restore of a detached project.** `restore_deleted_project` sees `team_id IS NULL` and returns
   `TeamGone { team_slug }`, with the slug read from the team's `deleted` record (same transaction
   as the detach, so it always exists). The CLI exits 4 with "its team {team_slug} was deleted —
   status: not restorable" and changes nothing. `list-deleted-projects` still lists the project,
   with that `team:` slug.
5. **Probe and upgrade.**
   - `Store::probe` requires `projects.detached_from_team_id` from slice 04.
   - 0027 is `ALTER`-only:
     - `DROP NOT NULL` is a metadata change;
     - the new column is nullable with no default;
     - the CHECK scans tens of rows.

## Alternatives considered

- **`ON DELETE SET NULL (team_id)` on `projects_team_id_fkey`, with the same CHECK**, so the
  cascade itself detaches. Rejected:
  - a workspace delete cascades to `teams` and to `projects` through two separate RI triggers;
  - if the `teams` cascade fires first, its `SET NULL` reaches live projects of the workspace being
    deleted, and the CHECK refuses the whole workspace delete;
  - whether it fires first depends on RI trigger firing order, the fragility ADR-TEAM-LIFECYCLE-002
    rejected for `NO ACTION`;
  - rewriting a 0001 constraint also locks `projects` and `teams` during rollout.
- **Keep `team_id NOT NULL` and drop its FK**, letting a detached row keep the dead id. Rejected: it
  loses referential integrity for every live project in order to serve a few deleted ones. A live
  project could then point at nothing.
- **Detach only deleted rows** (`… WHERE team_id = $team AND deleted_at IS NOT NULL`). It is correct
  while statement 2 is correct. Rejected because a live row slipping past statement 2, through a
  future writer or a broken lock, would then be cascaded silently. Touching every row turns that
  bug into a loud 23514. Pinned as a negative control.
- **Soft-delete the team as well, keeping its row as a tombstone.** Team-level soft delete is out of
  scope (DISCUSS), and it would change every team reader.
- **Move the deleted projects to a holding team.** That is a move (D-C).
- **Store the deleted team's slug on the project row** instead of `detached_from_team_id`. It
  duplicates the team record. The id correlates with that record, and the record is written in the
  same transaction, so the slug is always one join away.
- **Keep OQ-1 (a)** (deleted projects still block the team delete). The user chose (b).

## Consequences

- Positive: TL-KPI-5's demand is met. A team whose projects are all deleted can be deleted, and not
  a row of those projects is lost.
- Positive: "the team delete never cascades over a project" no longer depends on one SELECT being
  right. The CHECK and the detach-all statement make it structural.
- Negative: `projects.team_id` becomes nullable. Every reader that reads `team_id` reads
  `live_projects`, where it is never NULL (by the CHECK). The only readers that can meet a NULL are
  in `project_lifecycle.rs` and the export.
- Negative: a detached project cannot be restored until "restore into a team" exists (a follow-up).
  Its key stays taken in the workspace for good. Both are stated on the confirm page before the
  delete.
- Negative: psql `DELETE FROM teams` still cascades every project of the team, deleted or live. That
  is a deliberate operator act, as before.
- Negative: rolling deploy. A v0.14.x replica's team delete counts deleted projects too, so it
  refuses: stricter, never lossy. A v0.14.x replica that decodes a detached row's NULL `team_id`
  can do so only through `project_rename_context` with a hand-typed id, because its listings join
  `teams`. It would answer 500 for that request. Rollback floor: v0.14.0 runs on this schema, but
  every detached project becomes unreachable to it, which it already is.
- Earned Trust (store gold tests on `postgres:16`):
  - Detaching a live project is refused with 23514 `projects_team_or_detached`.
  - A team with only deleted projects is deleted, and every row of those projects survives,
    detached.
  - A team with a live project and a deleted one is refused, naming the live one only.
  - The barrier race, restore versus team delete, at least 100 runs: 0 projects lost, 0 issues
    lost, 0 errors outside {`Restored`, `TeamGone`, `HasProjects`}.
  - Negative control: a restore without the team `KEY SHARE`, against a detach limited to deleted
    rows, loses the project.
  - The shipped 100-run delete-versus-create race stays green.
  - A workspace delete cascades live, deleted and detached projects alike.
