# ADR-PROJECT-LIFECYCLE-003: Project renames, deletes and restores are recorded in one append-only `project_lifecycle_events` table; both rename doors share one locked, audited rename core and differ only in their authority gate

- Status: Accepted (2026-10-12)
- Feature: `project-lifecycle` (DISCUSS D-B, D2, D3, D16, OQ-2, OQ-3, OQ-4, INV-PL-5, INV-PL-7, INV-PL-9, NFR-PL-5; DESIGN DDD-13 to DDD-18)
- Follows: ADR-TEAM-LIFECYCLE-003 (the team record and locked rename), ADR-WORKSPACE-RENAME-002 (audit rows are not tenant export)
- Amends: ADR-PROJECT-RENAME-002 decision 2 (the write gains a lock and a record; uniqueness stays check-then-write)

## Context

OQ-3 (user) asks for a record of every effective rename (actor, project, old name to new name,
when), every delete (actor, project, name, key, when) and every restore (the operator, project,
when). Each record is written in the same transaction as its change. **Both** rename doors write it:
the new team-page door and the shipped instance-admin door (D16). There is no UI for the records;
they are read with psql. OQ-2 (user) says a rename changes the name only, never the slug or the key.
OQ-4 (user) says a deleted project's name, slug and key stay taken.

Today the instance-admin rename (`foundry_services::projects::rename_project`) works like this:
- it checks `is_instance_admin`;
- it reads the context (`team_id`, current name) and the siblings;
- it runs the pure `classify_rename` (trim, then byte-equal no-op, then `ProjectName::try_new`, then
  `ensure_unique_among`);
- it calls `Store::update_project_name`, a bare `UPDATE` with no lock and no record.

The team door needs exactly the same rule (D3, job-project-naming) under a different authority
(D-B: `TeamAccess::can_manage`).

## Decision

1. **One table, migration `0025_project_lifecycle_events.sql` (slice 01, additive):**
   - `id UUID PRIMARY KEY` (app-minted v7)
   - `workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE`
   - `project_id UUID NOT NULL` (**no FK**: the record outlives a psql cascade, and the id
     correlates records about one project)
   - `team_id UUID NOT NULL` (no FK; the project's team when the event happened)
   - `project_slug TEXT NOT NULL`, `key_prefix TEXT NOT NULL`
   - `kind TEXT NOT NULL CHECK (kind IN ('renamed', 'deleted', 'restored'))`
   - `actor_id UUID NULL REFERENCES users(id)` (no ON DELETE action, as 0018 and 0023; NULL means
     the operator through `foundry doctor`)
   - `name TEXT NOT NULL` (the name before the event, or at it)
   - `new_name TEXT NULL`
   - `created_at TIMESTAMPTZ NOT NULL DEFAULT now()`. The store writes `clock_timestamp()`, the
     team-lifecycle DELIVER lesson: a write that waited on a lock is recorded in commit order.
   - `CONSTRAINT project_lifecycle_events_kind_shape CHECK (`
     - `(kind = 'renamed' AND actor_id IS NOT NULL AND new_name IS NOT NULL AND new_name <> name)`
     - `OR (kind = 'deleted' AND actor_id IS NOT NULL AND new_name IS NULL)`
     - `OR (kind = 'restored' AND actor_id IS NULL AND new_name IS NULL))`
   - indexes `(workspace_id, created_at)` and `(project_id, created_at)`
   - an apply-time self-check (columns, shape CHECK, no FK to `projects` or `teams`), the 0023 idiom.

   All three kinds are in the CHECK from the start, so slices 02 and 03 need no change to this
   table. No code path updates or deletes a row.
2. **One locked, audited rename write: `Store::rename_project_with_audit(project_id, actor_id,
   new_name) -> ProjectRenameWrite { Renamed { old_name } | Unchanged | NotFound }`**, in
   `project_lifecycle.rs`, in one transaction:
   1. `SELECT workspace_id, team_id, name, slug, key_prefix FROM projects WHERE id = $1 AND
      deleted_at IS NULL FOR NO KEY UPDATE`. No row (unknown, deleted, or deleted while we waited):
      `NotFound`.
   2. A byte-equal name: `Unchanged`, nothing written (a case-only change is a rename).
   3. `UPDATE projects SET name = $new WHERE id = $1`. Name only; the slug and key are never
      written (OQ-2).
   4. The `renamed` record (`name` = the old name read under the lock).
   5. Commit.

   `FOR NO KEY UPDATE` serialises renames of the same project, so each record's old name is exact
   and consecutive records chain. It conflicts with the delete's `FOR UPDATE`, so a rename and a
   delete are totally ordered: a rename that loses gets `NotFound`, which is the uniform 404. It
   does not conflict with the writers' `KEY SHARE` guard or with the issue counter. The shipped
   `update_project_name` is removed.
3. **One rename core, two authority gates (`foundry_services::projects`):**
   - private `apply_rename(store, project_id, actor_id, raw_name)`. It reads the live context, then
     the siblings, then runs the unchanged `classify_rename`, then calls `rename_project_with_audit`
     (`Unchanged` is `NoOp`, `NotFound` is `NotFound`). The **siblings include deleted projects**
     (`list_team_sibling_projects` moves into `project_lifecycle.rs` unchanged), so OQ-4 holds on
     both doors and on create, with the shipped copy.
   - `rename_project` (the instance-admin door). Its signature is unchanged: `is_instance_admin`,
     then `apply_rename`. Its context read now uses `live_projects`, so a deleted project's id is
     `NotFound`, the uniform 404 (D16).
   - `rename_team_project` (new, the team door). It resolves team facts by (session workspace, team
     slug), requires `TeamAccess::can_manage`, resolves the live project by (team, project slug),
     then calls `apply_rename`. It returns the re-read members view.

   One rule, one copy (`ProjectNameError`'s `Display`), one write and one record, whichever door is
   used. "Who renamed this?" has one answer.
4. **Uniqueness stays check-then-write (INV-PL-9, accepted residual).** Two concurrent renames, or a
   rename and a create, of *different* projects to case-insensitively equal names can both succeed.
   This is ADR-PROJECT-RENAME-002's residual. Identity (slug, key) cannot be corrupted, and each
   record's old name stays exact (decision 2). Two renames of the *same* project are serialised.
5. **Delete and restore records** are written by `soft_delete_project` and
   `restore_deleted_project` in their transactions (ADR-PROJECT-LIFECYCLE-002):
   - `deleted`: actor, `name`, `project_slug`, `key_prefix`;
   - `restored`: `actor_id` NULL, the restored name.
6. **Enforcement and export.**
   - The table is not in `TENANT_TABLES` (it stays at ten) or `KNOWN_FOUNDRY_TABLES`. It is in
     every whole-instance `pg_dump` (ADR-WORKSPACE-RENAME-002: the actor may not be in the archive's
     membership-bounded `users` closure).
   - `Store::probe` requires its columns from slice 01.
   - check-arch `project-lifecycle-one-seam` (the `OneSeamRule` of team-lifecycle):
     - (i) SQL `INSERT INTO` / `UPDATE` / `DELETE FROM` on `projects`, `live_projects` or
       `project_lifecycle_events` appears only under `crates/foundry-store/src` (and the
       acceptance harness);
     - (ii) the store writers `rename_project_with_audit(`, `soft_delete_project(` and
       `restore_deleted_project(` have no call site under `crates/{foundry-app,foundry-api}/src`.
       Their names differ from the services use-cases (`rename_team_project`, `delete_project`,
       `restore_project`), so the shipped `call_sites` scan needs no receiver analysis.

     Each clause gets one injected-violation gold test.
   - Migration 0025 adds a `workspaces` and a `users` reference, so it gets an
     `UPGRADE_WARNING_0025`, the 0023 mechanism (`upgrade_warnings.rs`, user decision 2026-10-11).

## Alternatives considered

- **Reuse `team_lifecycle_events`**, with `kind` widened and a nullable `project_id`. Rejected: it
  is a different aggregate. Its `team_slug` would mean "the project's team" on some rows and "the
  deleted team" on others, and its shape CHECK would grow a project arm per kind. ADR-TEAM-LIFECYCLE-003
  rejected the same move against `workspace_rename_events`.
- **Three tables, one per kind** (the 0018 shape). They are equally typed, but the migration, probe
  and test surface triples for three kinds that share every column but `new_name` and `actor_id`.
  The shape CHECK keeps one table typed.
- **Two rename use-cases with their own validation.** That is two copies of "trim, no-op, rule,
  unique", which is the drift job-project-naming exists to prevent. Rejected.
- **One rename use-case with an "authority" parameter** (`InstanceAdmin | TeamManage`). Viable.
  Rejected because each door's gate needs different inputs (a user id, versus session workspace and
  slugs), so the parameter would carry both shapes. Two thin gates over one private core keep each
  gate testable alone.
- **Keep the rename unlocked (ADR-PROJECT-RENAME-002's posture) and record anyway.** Two concurrent
  renames would both record the same old name. ADR-WORKSPACE-RENAME-001 rejected this for the same
  reason.
- **Record from a trigger on `projects`.** The trigger cannot know the actor without a GUC, and it
  hides the write from the Rust code (ADR-WORKSPACE-RENAME-001).
- **Free a deleted project's name** with partial unique indexes and a restore that renames on
  collision. Rejected by the user (OQ-4).

## Consequences

- Positive: one record per effective rename, delete and restore, enforced by code and by the shape
  CHECK. Records outlive projects and teams, and the export contract is untouched.
- Positive: the instance-admin door gains the record and the lock with no behaviour change visible
  to its user, except that a deleted project is now not found.
- Negative: an `actor_id` NULL convention for the operator. The shape CHECK makes it exact: only
  `restored` may have it, and `restored` must.
- Negative: a future user hard-delete meets the `actor_id` FK, alongside 0018, 0021 and 0023
  (team-membership OQ-D9).
- Negative: `pg_restore --clean` of a dump taken before 0025 over the upgraded database fails, as
  for 0023. The boot warns once and the CHANGELOG says so.
- Earned Trust (store tests on `postgres:16`):
  - A rename whose record INSERT fails (an unknown actor) rolls the name back.
  - Two concurrent renames of one project record two chained old names.
  - A byte-equal rename writes no record.
  - A rename and a delete in either order leave either a renamed-then-deleted project or a deleted
    project and a `NotFound` rename.
  - The shape CHECK refuses each wrong shape: `renamed` with an equal name, `deleted` without an
    actor, `restored` with an actor.
  - `slug` and `key_prefix` are byte-identical after every rename.
