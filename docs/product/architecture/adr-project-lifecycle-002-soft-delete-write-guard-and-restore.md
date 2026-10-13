# ADR-PROJECT-LIFECYCLE-002: A project delete locks the project row FOR UPDATE and sets its marker; every project-scoped write first takes FOR KEY SHARE on the live row, so no write commits after a delete; restore clears the marker under the team's KEY SHARE

- Status: Accepted (2026-10-12)
- Feature: `project-lifecycle` (DISCUSS D-A, D6, D10, D11, D12, D17, INV-PL-1, INV-PL-3, INV-PL-4, INV-PL-7; DESIGN DDD-6 to DDD-12)
- Builds on: ADR-PROJECT-LIFECYCLE-001 (the marker and `live_projects`), ADR-TEAM-LIFECYCLE-002 (the `FOR UPDATE` versus FK `KEY SHARE` argument)

## Context

A delete hides a project and changes no other row (D6, INV-PL-1). A write racing the delete gets
its normal outcome or the uniform 404, never a 500, and nothing that committed before the delete is
lost (INV-PL-3). D17 adds that after the delete no write can reach the project, so no new
notification about it arises. Notifications already queued are delivered. A restore puts back
exactly what was there (INV-PL-4). The record commits with its change (INV-PL-7).

Project-scoped writers in the store today (code read, 2026-10-12):
- `insert_issue_attempt` (the counter `UPDATE` locks the project row);
- `reposition_issue_with_outbox`, `update_issue_details` and the single-card delete (through
  `issue_delete::delete_issues_with_outbox`);
- `insert_comment_with_outbox`, `update_comment_with_outbox`, `soft_delete_comment_with_outbox`;
- the attachment insert (`attachments.rs`);
- the lane writers `rename_lane`, `insert_lane_at`, `move_lane_before` and `delete_lane_with_fate`.

Only the issue insert touches the project row. All the others resolve the project once, through the
services resolver, and then write child rows by id. Without a guard, a write that resolved the
project a millisecond before the delete commits would commit after it.

Postgres row locks at READ COMMITTED:
- `FOR UPDATE` conflicts with `FOR KEY SHARE`. `FOR NO KEY UPDATE` does not.
- A locking `SELECT` or an `UPDATE` that waited re-evaluates its `WHERE` against the row version
  that committed.

## Decision

1. **Delete: `Store::soft_delete_project(workspace_id, team_id, project_id, actor_id) ->
   ProjectDeleteOutcome { Deleted { name, key_prefix } | NotFound }`**, in `project_lifecycle.rs`,
   in one transaction:
   1. `SELECT name, slug, key_prefix FROM projects WHERE id = $p AND team_id = $t AND workspace_id
      = $ws AND deleted_at IS NULL FOR UPDATE`. No row: roll back, `NotFound`. This covers an
      unknown project, a foreign one, and an already-deleted one (a double submit).
   2. `UPDATE projects SET deleted_at = clock_timestamp(), deleted_by = $actor WHERE id = $p`.
      Nothing else is written. Not a single issue, comment, attachment, change event, lane, outbox
      row or `next_issue_number` changes (D6).
   3. `INSERT INTO project_lifecycle_events` (`kind = 'deleted'`, ADR-PROJECT-LIFECYCLE-003), with
      `created_at` equal to the marker's `deleted_at`.
   4. Commit.

   It must be `FOR UPDATE`, not `FOR NO KEY UPDATE`, because only `FOR UPDATE` conflicts with the
   writers' `KEY SHARE` (decision 2). This is ADR-TEAM-LIFECYCLE-002's trap (a), on the project row.
2. **The live-project write guard.** `project_lifecycle::lock_live_project(tx, project_id) ->
   Result<bool, sqlx::Error>` is `SELECT 1 FROM projects WHERE id = $1 AND deleted_at IS NULL FOR
   KEY SHARE`. Every project-scoped writer calls it as the **first statement of its transaction**.
   On `false`, the writer returns the "not found" outcome it already has:
   - `IssueInsertError::ProjectNotFound`;
   - `Ok(None)` for `update_issue_details`;
   - `RepositionOutcome::IssueNotFound`;
   - `Ok(false)` for the comment writes;
   - `Ok(0)` for the card delete;
   - the lane writers' not-found outcomes.

   The services already map each of these to `ServiceError::NotFound`, which is the uniform 404 in
   HTML and the API's not-found JSON.
   - **Two writers run without a transaction today**: `rename_lane` (`lanes.rs:543`, "no
     transaction and no lock") and `insert_attachment` (`attachments.rs:55-68`, a pool insert).
     Both now open a transaction whose first statement is the guard. This is decided here, not
     deferred.
   - The comment writers, which are keyed by comment id, gain a `project_id` parameter. Their
     callers already hold it from `find_issue_by_team_project_number`.
   - **`undelete_comment`** (`lib.rs:3259`, behind `foundry doctor restore-comment`) is guarded
     too. Restoring a comment inside a deleted project would change a row under it (D6, PL-KPI-2),
     so it answers "not restorable", exit 4, until the project is restored. That is the shipped
     exit, and the stderr reason names the project as deleted.
   - `renumber_column` and the other helpers that take a caller's transaction are covered through
     their callers (decision 7).
   - **Issue create is the one writer whose guard is a different statement.**
     `insert_issue_attempt`'s first statement is already `UPDATE live_projects SET
     next_issue_number = … RETURNING …`. That statement takes `FOR NO KEY UPDATE`, which conflicts
     with the delete's `FOR UPDATE`, and after a wait it re-evaluates the view's predicate. It *is*
     the guard. Adding a separate `KEY SHARE` first would make the transaction upgrade its own row
     lock while a delete may be queued on that row, a lock-upgrade shape whose deadlock-freedom we
     would have to prove. Not taking the extra lock removes the question. check-arch rule 2
     recognises this form (decision 7).

   Why this closes D17 and INV-PL-3. Let W be a write and D a delete of the same project.
   - **W's guard ran first.** D's `FOR UPDATE` waits until W ends. W commits before D, so its rows
     and its outbox row are "before the delete": they are kept, hidden, and restored (INV-PL-3).
     Its notification goes out like any notification queued before the delete (D17).
   - **D's lock ran first.** W's guard waits. When D commits, the guard re-evaluates
     `deleted_at IS NULL` against the committed version, finds no row, and W writes nothing and
     answers the uniform 404. If D rolls back, W proceeds.

   No order gives a 500, and no write commits after the delete commits.

   `KEY SHARE` is compatible with `KEY SHARE` and with `FOR NO KEY UPDATE`, so the guard serialises
   writers against a delete only, never against each other, a rename or the issue counter.
3. **Lock order, one rule for every path:** team row → project row → the project's children
   (lanes, issues, comments, attachments).
   - Writers take project `KEY SHARE`, then children.
   - A delete takes project `FOR UPDATE` only.
   - A rename takes project `FOR NO KEY UPDATE` only.
   - A restore takes team `KEY SHARE`, then project `FOR UPDATE`.
   - A team delete takes team `FOR UPDATE`, then project rows (the detach,
     ADR-PROJECT-LIFECYCLE-004).

   No lifecycle path takes a team lock after a project lock, so the lifecycle paths cannot
   deadlock one another. There is one exception outside the lifecycle: `insert_project` inserts its
   row and then takes the team's `KEY SHARE` through the FK. The new row is invisible to every
   other transaction until commit, so no path can wait on it, and no cycle forms.
4. **Restore: `Store::restore_deleted_project(project_id) -> ProjectRestoreOutcome { Restored |
   NotDeleted | NotFound | TeamGone { team_slug } }`**, in `project_lifecycle.rs`, in one
   transaction:
   1. `SELECT team_id, detached_from_team_id, deleted_at FROM projects WHERE id = $1`, with no lock.
      - No row: `NotFound`.
      - `deleted_at IS NULL`: `NotDeleted`.
      - `team_id IS NULL`: `TeamGone`. The slug is read from the `team_lifecycle_events` `deleted`
        record of `detached_from_team_id`, which was written in the same transaction as the
        detach (ADR-PROJECT-LIFECYCLE-004).
   2. `SELECT 1 FROM teams WHERE id = $team FOR KEY SHARE`. No row means the team was deleted while
      we read. Re-read the project and return `TeamGone`. This lock is what orders a restore
      against a team delete (ADR-PROJECT-LIFECYCLE-004 decision 3).
   3. `SELECT 1 FROM projects WHERE id = $1 AND team_id = $team AND deleted_at IS NOT NULL FOR
      UPDATE`. No row means a concurrent restore won: `NotDeleted`.
   4. `UPDATE projects SET deleted_at = NULL, deleted_by = NULL WHERE id = $1`. Nothing else is
      written. Name, slug, key, `next_issue_number`, lanes, issues, live comments and attachments
      are exactly as the delete left them, because no product write could reach them while the
      project was deleted (decision 2). That is INV-PL-4.

      There is one exception, and it is a locked decision: D12 keeps the comment-tombstone GC
      running. A comment that a user had already deleted (a tombstone) and that passes its 90-day
      window while the project is deleted is purged as usual, and does not come back. A restore
      brings back every row that existed at the delete, minus such tombstones.
   5. `INSERT INTO project_lifecycle_events` (`kind = 'restored'`, `actor_id` NULL: the operator).
   6. Commit.

   A restore can never collide on its **slug or key**. Both stayed in their unique indexes
   (`UNIQUE (team_id, slug)`, `UNIQUE (workspace_id, key_prefix)`) while the project was deleted.
   Its **name** is protected because every create and rename checks it against deleted siblings
   (OQ-4, INV-PL-5). That check is check-then-write, so the only way to an equal name is the
   accepted INV-PL-9 race, the same one that applies to any two live projects.
5. **The operator doors (D10).**
   - `foundry doctor list-deleted-projects` calls the services use-case
     `projects::list_deleted_projects`, which wraps `Store::list_deleted_projects()`. That method
     reads every deleted project, instance-wide, by `deleted_at` then `id`, as `(project_id,
     workspace_name, team_slug, name, key_prefix, issue_count, deleted_at, deleted_by_email)`. The
     team slug is the live team's, or, for a detached project, the slug from its team's `deleted`
     record. The email is `users.email_lower`, the `list-users` precedent.
   - `foundry doctor restore-project <uuid>` calls `projects::restore_project`.
   - Both mirror `run_restore_comment`: `DATABASE_URL`, a thread-isolated runtime, exit 0 restored,
     2 bad uuid, 3 database, 4 not restorable. The reason goes on stderr in the D14 copy: "is not
     deleted", "not in database", "its team {team_slug} was deleted".
   - The operator's shell access is the authority (NFR-PL-6). Neither use-case checks a user.
   - No SQL is written in `admin_cli.rs`. `run_restore_comment`'s best-effort `SELECT EXISTS` is not
     repeated: the typed outcome already says which case it is.
6. **D12, D17.** The comment tombstone GC (`gc_tombstoned_comments`) is unchanged and unguarded:
   it purges tombstones inside a deleted project on its usual schedule. The outbox, the notifier
   and queued mail are unchanged. A restore replays nothing.
7. **Enforcement.**
   - check-arch `project-lifecycle-write-guard`. In `crates/foundry-store/src`, every function
     whose body writes `issues`, `comments`, `issue_attachments`, `lanes` or `issue_change_events`
     must call `lock_live_project(`, **whether or not it opens a transaction**.
     - "Writes" means SQL `INSERT INTO`, `UPDATE` or `DELETE FROM` on one of those tables. The
       table names follow rule 1's lexical rules: any case, quoted or `public.`-qualified, line
       breaks allowed, inside string literals. It also means a call to any **transaction-taking
       helper that itself writes**. The rule computes that set in a first pass over every function
       in the crate with a transaction parameter whose body writes (directly or transitively). The
       set therefore includes `renumber_column`, `delete_issues_with_outbox`,
       `move_cards_to_destination` and `record_issue_change` without a hand-kept list.
     - SQL assembled with `format!` in `crates/foundry-store/src` that interpolates a table name is
       a violation in itself: the rule cannot see it.
     - Exempt:
       - helpers that take a caller's transaction as a parameter. They ride a guarded caller, and
         their callers are in scope by the first pass;
       - `insert_issue_attempt`, **only while its first SQL statement is `UPDATE live_projects`**
         (decision 2). The rule checks that this is the first SQL literal executed in the body. A
         gold test where an `INSERT` precedes it is flagged;
       - the allow-list, with reasons: `gc_tombstoned_comments` (D12) and `insert_project` (a
         brand-new project seeds its lanes).
     - Injected-violation gold tests:
       - a guarded writer passes;
       - an unguarded autocommit `INSERT INTO issue_attachments` is flagged;
       - an unguarded `INSERT INTO comments` inside a `.begin()` body is flagged;
       - a transaction-taking helper is not flagged;
       - the allow-listed GC is not flagged.
     - The behavioural table-driven gold test (Earned Trust below) enumerates its writers from the
       same list the rule scans, so the two cannot drift.
   - check-arch `project-lifecycle-one-seam` (ADR-PROJECT-LIFECYCLE-003 decision 6) keeps
     `soft_delete_project(` and `restore_deleted_project(` out of `foundry-app` and `foundry-api`.
     The web door and the CLI go through the `foundry-services` use-cases `projects::delete_project`
     and `projects::restore_project`.

## Alternatives considered

- **No write guard; accept the race.** A write that resolved before the delete committed may commit
  just after it. Its row is kept, hidden and restored, so INV-PL-3 holds. Rejected:
  - a write could commit after Lena was told "Deleted Auth v2 Spike.", and its notification would
    arise after the delete, against D17;
  - "nothing changed while deleted" would hold only probabilistically, which weakens INV-PL-4's
    "exactly as the delete left it".

  The guard costs one statement per writer.
- **Delete with `FOR NO KEY UPDATE`.** It is sufficient for the delete's own `UPDATE`, because
  `deleted_at` is not a key column. It is wrong for the guard: it does not conflict with
  `KEY SHARE`, so a guarded write proceeds and commits after the delete. This is pinned as a
  negative control.
- **A database trigger guard on `issues`, `comments`, `issue_attachments` and `lanes`** that
  refuses writes into a deleted project. It covers psql. Rejected:
  - it must exempt the GC (D12), workspace-delete cascades and fixtures;
  - every writer still needs to map a new SQLSTATE to its not-found outcome;
  - the rule is hidden from the Rust code.

  ADR-TEAM-LIFECYCLE-002 already took the same posture: a trigger is the designated backstop if a
  second writer family appears outside the store.
- **SERIALIZABLE with retry, or a per-project advisory lock.** These protect only writers that opt
  in, as in ADR-BOARD-LANE-002 and ADR-TEAM-LIFECYCLE-002. The row lock needs the same opt-in (the
  guard call), but check-arch enforces it structurally, and it composes with the FK locks the
  writers already take.
- **Restore without the team's `KEY SHARE`.** A restore could clear the marker while a team delete
  holds the team row. The detach would then meet a live project (ADR-PROJECT-LIFECYCLE-004), and
  the team delete would end in a 23514, a 500, instead of its 409. Worse, a detach filtered to
  deleted rows would let the cascade destroy the restored project. Pinned as a negative control.
- **A restore that re-derives or re-numbers anything** (a fresh counter, a lane re-seed). Nothing
  under the project can change while it is deleted, so there is nothing to re-derive. Clearing the
  marker is the whole restore (slice 03's hypothesis).

## Consequences

- Positive: "deleted" is one transaction writing one marker and one record. "Restored" is one
  transaction clearing it and writing one record. INV-PL-1 holds by construction. INV-PL-4 holds by
  construction for every row except tombstoned comments that pass the 90-day GC window while the
  project is deleted (D12, decision 4), because no product write can reach a deleted project.
- Positive: every racing write gets a normal outcome. No new error type or mapping is needed: each
  writer reuses its shipped not-found outcome.
- Negative: about eleven writers gain one guard statement, and the comment writers gain a
  `project_id` parameter. This is mechanical. check-arch and a table-driven gold test keep it
  complete.
- Negative: while a delete is in flight (milliseconds), every write on that project waits. This is
  negligible at homelab scale.
- Negative: psql can still write into a deleted project. That is a deliberate operator act, as for
  team deletes.
- Earned Trust (store gold tests on `postgres:16`):
  - The guard, blocked by a held delete, returns `false` after the delete commits and `true` after
    it rolls back.
  - Negative control: a delete taking `FOR NO KEY UPDATE` lets a guarded comment insert commit after
    it.
  - A table-driven test runs every guarded writer against a held, then committed, delete. Each
    returns its not-found outcome, and the row counts are unchanged.
  - Deterministic: an issue create whose counter `UPDATE` waits behind a held delete returns
    `ProjectNotFound` after the commit, with no 40P01.
  - `rename_lane` and `insert_attachment`, now transactional, refuse under a held then committed
    delete.
  - The barrier race, delete versus issue create, at least 100 runs: 0 unmapped errors, 0 lost
    issues. Every issue that committed is present after a restore.
  - A delete changes exactly `deleted_at`, `deleted_by` and one record. A whole-database row-count
    and checksum snapshot of `issues`, `comments`, `issue_attachments`, `issue_change_events`,
    `lanes` and `outbox` is identical before and after.
  - A restore then a re-read equals the pre-delete snapshot, scoped to live rows plus tombstones
    still inside their 90-day GC window (decision 4), and the next issue number is unchanged.
  - `undelete_comment` on a comment in a deleted project changes nothing and reports not
    restorable.
  - A restore of a live project, an unknown id, and a concurrent double restore each yield their
    outcome with nothing written.
