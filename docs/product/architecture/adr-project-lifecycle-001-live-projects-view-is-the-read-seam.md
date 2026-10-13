# ADR-PROJECT-LIFECYCLE-001: A deleted project is hidden by one database view, `live_projects`, that every reader uses; only one store module may name the `projects` table, and check-arch enforces it

- Status: Accepted (2026-10-12)
- Feature: `project-lifecycle` (DISCUSS D-A, D5, D9, D13, OQ-4, INV-PL-2, NFR-PL-2; DESIGN DDD-1 to DDD-5, DDD-22)

## Context

D-A makes a project delete a soft delete. INV-PL-2 says that every read or write reaching a project
through a web or API address treats a deleted project exactly as one that never existed, that every
list omits it, and that **a new project reader added later must not see deleted projects by
default**. The only readers allowed to see a deleted project are the operator CLI, the per-workspace
export (D13), the uniqueness checks (OQ-4) and the team delete (OQ-1).

The shipped code reaches projects as follows (code read, 2026-10-12):

- About 21 web routes under `/team/{t}/project/{p}/…` in the router, plus the 2 attachment routes
  (upload and download, `foundry-app/src/attachments.rs:52-59`, merged into the router), so about
  23 web routes. There are 7 API routes under `/api/v1/teams/{t}/projects/{p}/…`. None of them reads
  `projects` itself. They resolve through four
  store readers: `find_project_by_slug` (7 call sites across `foundry-app` `projects.rs`, `keyboard.rs`
  and `events.rs`, and `foundry-services` `issues.rs`, `lanes.rs` and `lib.rs`),
  `find_issue_by_team_project_number` (5 call sites), `list_projects_for_workspace` (dashboard, nav)
  and `list_projects_for_instance` (instance dashboard).
- There are 22 SQL sites in `crates/foundry-store/src` that name `projects`. They are the four
  readers above, the issue joins at `lib.rs:1973, 2251, 2315, 2381`, the comment-write
  sub-selects at `:2775, :2836`, the issue counter `UPDATE` at `:1824`, the lane internals at
  `lanes.rs:302, 361, 664`, the rename context, siblings and write at `:4301, :4323, :4339`,
  `TEAM_PROJECTS_SQL` (`teams.rs:589`), the export at `:1054`, an acceptance-only count at `:2415`,
  and the two inserts.
- No other crate's `src` has project SQL **except** `foundry doctor backup-verify`
  (`foundry-app/src/admin_cli.rs:201-207, 366-374`). It counts every row of each table in
  `KNOWN_FOUNDRY_TABLES`, `projects` included, with `format!`-built SQL, against a **restored
  sandbox** (`FOUNDRY_DOCTOR_PROBE_URL`), not the live database. The notifier composes mail from
  the outbox payload and never reads `projects`.

The store runs Postgres 16. Homelab scale is tens of projects.

## Decision

1. **The deleted marker** (migration `0026_project_soft_delete.sql`):
   - `projects.deleted_at TIMESTAMPTZ NULL` and `projects.deleted_by UUID NULL REFERENCES users(id)`,
     the `comments` precedent (0006);
   - `CHECK projects_deleted_marker_shape ((deleted_at IS NULL) = (deleted_by IS NULL))`.
2. **One view defines "live":** `CREATE VIEW live_projects AS SELECT * FROM projects WHERE
   deleted_at IS NULL`. It is a simple, automatically updatable view, with no `CHECK OPTION` and no
   `security_barrier`. `SELECT`, `UPDATE … RETURNING` and row-locking clauses go through it to the
   base rows. The migration self-checks that the view's column list equals the table's. Postgres
   expands `*` when the view is created, so any later migration that adds a column to `projects`
   must `CREATE OR REPLACE VIEW live_projects`. The self-check, `Store::probe` and a gold test catch
   a forgotten one.
3. **Every production reader reads `live_projects`.** This covers the four resolvers, the issue
   joins, the comment sub-selects, the lane internals, the dashboard and nav list, the instance
   dashboard, the rename context and the team's live project list. The issue counter becomes
   `UPDATE live_projects SET next_issue_number = … RETURNING …`. After waiting on a delete's lock,
   it re-evaluates the view's predicate and returns no row, which is the shipped
   `IssueInsertError::ProjectNotFound` and so the uniform 404 (INV-PL-3).
4. **One module may see deleted projects: `crates/foundry-store/src/project_lifecycle.rs`.** It
   holds every SQL that reads, updates or deletes the base table:
   - the sibling read behind both rename doors and create (OQ-4: a deleted project's name and
     slug stay taken);
   - the team delete's live and deleted lists and the detach (ADR-PROJECT-LIFECYCLE-004);
   - the delete, the restore and the rename's locked read (ADR-PROJECT-LIFECYCLE-002 and -003);
   - the live-project write guard (ADR-PROJECT-LIFECYCLE-002);
   - `list_deleted_projects` (the CLI);
   - the export's `projects` selection, a `const` that `export_workspace` uses (D13: the whole row,
     so `deleted_at` and `deleted_by` mark it as deleted);
   - acceptance-only counting helpers, which count every row.

   `INSERT INTO projects` stays where it is (`insert_project` and the bootstrap seed), because a
   new row is live by construction.
5. **The doors do not change.** Handlers in `foundry-app` and `foundry-api` keep calling the same
   store readers and services resolvers. A deleted project resolves to `None`, the same value as a
   project that never existed. Each route therefore answers what it answers today for an unknown
   slug, which is the D5 oracle, provided the route treats `None` as not found.

   **One route does not: attachment download.** `download_attachment`
   (`foundry-app/src/attachments.rs:232-238`) calls `find_issue_by_team_project_number` as a
   "defensive lookup" and checks only `Err`, ignoring `Ok(None)`. It then serves the attachment by
   (attachment id, workspace) alone. Today, a valid attachment id under any slug of a team the
   caller is on returns the bytes, and after this feature a deleted project's attachments would too.
   The differential oracle cannot catch this, because the unknown-slug answer is the same leak.

   Decided: the download maps `Ok(None)` to `resource_not_found_page()`, and the attachment is looked
   up by (attachment id, resolved issue id), so it must belong to the issue the path names. This is a
   scoped fix to a shipped route, required by INV-PL-2. Rule: every call site of the four resolvers
   handles `Ok(None)` as not found. DESIGN checked every call site listed above; this is the one
   exception.
6. **The key-prefix-keyed issue lookups are re-keyed by the resolved project id.** These are
   `reposition_issue_with_outbox`, `update_issue_details` and `issue_edit_view`. Each is rewritten
   in this feature anyway, both for the view and for the write guard, and the guard needs the
   project id. Today they select `WHERE p.key_prefix = $1 AND i.number = $2` with no workspace
   predicate. `key_prefix` is unique per workspace only (`UNIQUE (workspace_id, key_prefix)`), so
   two workspaces sharing a key make that lookup ambiguous. Keying by the id the services resolver
   already holds removes the ambiguity. Single-workspace behaviour is unchanged.
7. **Enforcement (check-arch `project-lifecycle-live-reads`).** The rule scans every `.rs` under
   `crates/*/src` except `crates/foundry-acceptance` and
   `crates/foundry-store/src/project_lifecycle.rs`.
   - **What it scans:** every Rust string literal (normal and raw, multi-line included) that
     contains an SQL keyword (`SELECT`, `INSERT`, `UPDATE`, `DELETE`, `FROM` or `JOIN`, as words,
     case-insensitive). Requiring a keyword keeps route strings such as `/team/{t}/projects/new` out
     of scope.
   - **What it flags:** the table name `projects` as a bare identifier, not preceded or followed by
     an identifier character, in any form: unquoted, quoted (`"projects"`) or schema-qualified
     (`public.projects`). That catches a comma join, a line break between `FROM` and the name, and a
     sub-select. `live_projects` and `project_lifecycle_events` never match.
   - **One exception:** `INSERT INTO projects` (a new row is live).
   - **Also flagged:** an exact `"projects"` string literal, a table name handed to `format!`-built
     SQL. One **reviewed exception**: `KNOWN_FOUNDRY_TABLES` in `admin_cli.rs`. `backup-verify`
     must count every row of a restored sandbox, deleted projects included; it is a backup check,
     not a project door. The exception names that constant and nothing else.
   - Violations are named by `file:line`.
   - Comments are not SQL and are not scanned.

   Injected-violation gold tests:
   - one hit each in `lib.rs`, `lanes.rs` and `teams.rs` is flagged;
   - a comma join, a line break between `FROM` and the name, and `public.projects` are flagged;
   - `FROM live_projects`, `INSERT INTO projects` and a route string are not flagged;
   - the module itself is not flagged, and neither is an acceptance fixture;
   - a missing `crates/` directory fails the rule.

## Alternatives considered

- **An application-level resolution seam only.** One `ProjectResolver` in `foundry-services` would
  serve every door, and check-arch would forbid `foundry-app`/`foundry-api` from calling the store's
  project readers. It covers the doors, and the shipped `resolve_member_project` and
  `resolve_board_project` stay as they are. Rejected as the mechanism: it does not reach the
  store-internal joins (dashboard and nav lists, the instance dashboard, the issue joins, the lane
  internals, the comment sub-selects). Each of those would still need its own `deleted_at IS NULL`,
  and a store reader written next month would see deleted rows unless its author remembered. That
  fails INV-PL-2's last sentence.
- **Row-level security** (`CREATE POLICY … USING (deleted_at IS NULL)`, with a GUC to bypass for
  the allowed readers). It is structural and covers psql. Rejected:
  - RLS does not apply to the table owner unless the table is `FORCE`d. It never applies to a
    superuser or a `BYPASSRLS` role. Which role the deployment connects as is an environment fact
    the code cannot see, so a deployment that "lies" would silently show every deleted project.
    Probing the role at boot would only refuse to start, not hide anything.
  - The bypass would be invisible in Rust code, and every test would have to run as a non-owner
    role.
- **A per-reader `AND p.deleted_at IS NULL`, with an acceptance sweep over the route inventory.**
  This means 19 edits and no default. A check-arch rule of the form "every SQL literal naming
  `projects` mentions `deleted_at`" is a weak proxy: a predicate can be present but OR'd away, or
  apply to the wrong alias. Decision 7's rule asks a simpler question, "does this literal name the
  table at all?", and that can be answered exactly. Rejected for the same INV-PL-2 reason.
- **Rename the base table to `project_rows` and name the live view `projects`.** Every shipped
  reader, every future reader, psql and **old replicas during a rolling deploy** would see live
  projects with zero reader edits. That is a real advantage, and it is the strongest default.
  Rejected for its ripples into shipped contracts:
  - the 0019/0020 trigger function names its refusal arm and HINT from `TG_TABLE_NAME`, so
    `projects_name_max_256_chars` (OUT-20, the name-db-checks acceptance) would become
    `project_rows_…` unless the function is rewritten;
  - the test-support legacy seam (check-arch `name-rule-legacy-seam`) disables triggers by table
    name;
  - every later `ALTER TABLE projects` would hit a view;
  - more than 100 acceptance-fixture lines that read or write `projects` would silently start
    reading only live projects, which breaks the "nothing erased" counts unless each is reviewed.

  The surprise cost lands on every future reader of the schema.
- **Move a deleted project to a separate table.** Every child references `projects(id) ON DELETE
  CASCADE`, so moving the row out deletes its issues, comments, attachments, change events and
  lanes, unless every FK is repointed. That breaks D-A outright.

## Consequences

- Positive: "live" has one definition, in the database, and every new reader gets it by writing the
  obvious `FROM live_projects`, or fails the build by writing `FROM projects`. Seeing deleted
  projects is a reviewed act: an edit to one named module.
- Positive: no door changes except the attachment-download fix (decision 5). The D5 oracle ("exactly as an address that never existed") falls out
  per route, because the resolvers already return `None` for an unknown slug.
- Positive: the cross-workspace key-prefix ambiguity in three issue lookups is closed as a side
  effect.
- Negative: about 19 SQL strings change in one slice. This is mechanical, and the rule finds any
  that are missed.
- Negative: a later migration that adds a `projects` column must recreate the view. The 0026
  self-check, `Store::probe` and a gold test turn a forgotten one into a refused boot or a red test,
  not a silent gap.
- Negative: **rolling deploy.** Until the last v0.14.x replica is replaced, an old replica reads
  `projects` and serves a project deleted through a new replica. Its writes to that project land,
  hidden, and come back with a restore. Its unlocked instance-admin rename can rename a deleted
  project, with no record, so the restore would bring back a different name (an INV-PL-4 and
  INV-PL-7 exception, limited to the window). Nothing is lost. The window is the rollout. The
  CHANGELOG says: do not delete or rename projects while this release rolls out.
- Negative: a fix to a shipped route. Attachment download refuses a path that does not resolve
  (decision 5). Before this feature, an attachment could be fetched through any project slug of a
  team the caller is on.
- Negative: psql `SELECT … FROM projects` still shows deleted rows. That is intended (D5: the
  database is one of the two ways to see them).
- Earned Trust (store gold tests on `postgres:16`):
  - `live_projects` has exactly `projects`' columns, in 0026 and again after 0027;
  - `UPDATE live_projects … RETURNING` that waited on a delete's `FOR UPDATE` returns no row once the
    delete commits;
  - `lock_live_project` (ADR-PROJECT-LIFECYCLE-002), the function every writer calls, blocks a
    concurrent `FOR UPDATE` and returns no row once a delete commits;
  - a deleted project's attachment id, requested through the deleted project's path, answers the
    uniform 404 body. This is asserted directly, not only by differential equality;
  - an `INSERT INTO projects` row appears in `live_projects`, and a row with `deleted_at` set does
    not;
  - the workspace delete cascade (the acceptance "workspace is deleted" step) still removes a
    workspace whose projects are live, deleted or detached.
