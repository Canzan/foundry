# ADR-PROJECT-LIFECYCLE-001: A deleted project is hidden by one database view, `live_projects`, that every reader uses; only one store module may name the `projects` table, and check-arch enforces it

- Status: Accepted (2026-10-12)
- Feature: `project-lifecycle` (DISCUSS D-A, D5, D9, D13, OQ-4, INV-PL-2, NFR-PL-2; DESIGN DDD-1 to DDD-5, DDD-22, DDD-32; DDD-5 and DDD-32 shipped in bugfix v0.14.1)

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
- There are 20 SQL sites in `crates/foundry-store/src` that name `projects` (re-checked
  2026-10-13, after bugfix v0.14.1, which removed two issue joins). They are the four readers
  above, the issue joins at `lib.rs:1980` (`reposition_issue_with_outbox`) and `:2259`
  (`list_project_changes`), the comment-write sub-selects at `:2789, :2853`, the issue counter
  `UPDATE` at `:1824`, the lane internals at `lanes.rs:302, 361, 664`, the instance list at
  `:4301`, the rename context, siblings and write at `:4327, :4349, :4365`, `TEAM_PROJECTS_SQL`
  (`teams.rs:589`), the export at `:1054`, an acceptance-only count at `:2422`, and the two inserts
  (`:1649`, `:3661`).
- Exact `"projects"` string literals, which name the table without SQL: `lib.rs:123`
  (`TENANT_TABLES`), `lib.rs:1053` (the export's label), `verify_export.rs:124`
  (`DIRECT_WORKSPACE_ID_TABLES`), `name_rule_legacy_seam.rs:30` (`NameRuleTable::table`) and
  `foundry-app/src/admin_cli.rs:207` (`KNOWN_FOUNDRY_TABLES`).
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
   - the export's `projects` entry, label and SQL together, a `const` that `export_workspace` uses
     (D13: the whole row, so `deleted_at` and `deleted_by` mark it as deleted);
   - acceptance-only counting helpers, which count every row.

   `INSERT INTO projects` stays where it is (`insert_project` and the bootstrap seed), because a
   new row is live by construction.
5. **The doors do not change.** Handlers in `foundry-app` and `foundry-api` keep calling the same
   store readers and services resolvers. A deleted project resolves to `None`, the same value as a
   project that never existed. Each route therefore answers what it answers today for an unknown
   slug, which is the D5 oracle, provided the route treats `None` as not found.

   **Attachment download was the one exception, and v0.14.1 fixed it.** Before v0.14.1,
   `download_attachment` called `find_issue_by_team_project_number` as a "defensive lookup", checked
   only `Err` and ignored `Ok(None)`. It then served the attachment by (attachment id, workspace)
   alone, so a valid attachment id under any slug of a team the caller was on returned the bytes.
   The differential oracle could not catch this, because the unknown-slug answer was the same leak.

   Bugfix v0.14.1 (`ff62b40..7d679a0`, fix-issue-attachment-scoping) closed it outside this
   feature: the download (`foundry-app/src/attachments.rs:172`) resolves the issue through
   `resolve_member_issue`, then calls `find_attachment_for_issue(attachment_id, issue.issue_id)` and
   maps `Ok(None)` to `resource_not_found_page()` (:207-211). This feature changes nothing in that
   route. Rule: every call site of the four resolvers handles `Ok(None)` as not found. DESIGN
   re-checked every call site after v0.14.1; there is no exception left.
6. **The key-prefix-keyed issue lookups: shipped in v0.14.1, not part of this feature.** Before
   v0.14.1, `reposition_issue_with_outbox`, `update_issue_details` and `issue_edit_view` selected
   `WHERE p.key_prefix = $1 AND i.number = $2` with no workspace predicate. `key_prefix` is unique
   per workspace only (`UNIQUE (workspace_id, key_prefix)`), so two workspaces sharing a key made
   that lookup ambiguous. v0.14.1 re-keyed all three by `i.project_id = $1`, and its check-arch
   rule `store-natural-key-scoped` keeps every natural-key filter scoped. In this feature, only
   `reposition_issue_with_outbox`'s `projects` join (`lib.rs:1980`) moves to `live_projects`.
   `update_issue_details` gains the guard (ADR-PROJECT-LIFECYCLE-002). `issue_edit_view` has no
   `projects` SQL and does not change: its `project_id` comes from `resolve_member_project`, which
   reads the view.
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
     SQL. **Reviewed exemptions**, named by file and enclosing item, not by line. Each is a
     table-name label with no SQL:
     - `foundry-store/src/lib.rs`, const `TENANT_TABLES`: the export and manifest table list;
     - `foundry-store/src/verify_export.rs`, const `DIRECT_WORKSPACE_ID_TABLES`: the offline
       archive verifier, which reads JSON, not the database;
     - `foundry-store/src/name_rule_legacy_seam.rs`, fn `NameRuleTable::table`: the test-support
       seam that switches the name-rule triggers by table name, fenced from the doors by check-arch
       `name-rule-legacy-seam`;
     - `foundry-app/src/admin_cli.rs`, const `KNOWN_FOUNDRY_TABLES`: `backup-verify` counts every
       row of a restored sandbox, deleted projects included; it is a backup check, not a project
       door.

     The export's `("projects", "SELECT … FROM projects t …")` entry (`lib.rs:1053-1054`) is not
     exempted: it moves into the module (decision 4). The list is settled before the rule turns on
     (slice 02, after the reader sweep). Any other exact literal fails the build.
   - Violations are named by `file:line`.
   - Comments are not SQL and are not scanned.

   Injected-violation gold tests:
   - one hit each in `lib.rs`, `lanes.rs` and `teams.rs` is flagged;
   - a comma join, a line break between `FROM` and the name, and `public.projects` are flagged;
   - `FROM live_projects`, `INSERT INTO projects` and a route string are not flagged;
   - the module itself is not flagged, and neither is an acceptance fixture;
   - each of the four exempted items passes; the same `"projects"` literal in another const or fn
     of the same file is flagged; the export label left in `lib.rs` is flagged;
   - a missing `crates/` directory fails the rule.

   The rule runs beside the shipped `store-natural-key-scoped` (v0.14.1,
   `xtask/src/check_arch.rs:6042-6114`). Any `live_projects` query that filters on `slug` or
   `key_prefix` must also compare `team_id` or `workspace_id` with a bind parameter in the same
   `OR`-branch and parenthesis level.

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
  This means about 13 edits and no default. A check-arch rule of the form "every SQL literal naming
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
- Positive: no door changes. The D5 oracle ("exactly as an address that never existed") falls out
  per route, because the resolvers already return `None` for an unknown slug, and since v0.14.1
  every call site treats `None` as not found.
- Negative: 13 SQL strings move to the view and 5 into the module, in one slice. This is
  mechanical, and the rule finds any that are missed.
- Negative: a later migration that adds a `projects` column must recreate the view. The 0026
  self-check, `Store::probe` and a gold test turn a forgotten one into a refused boot or a red test,
  not a silent gap.
- Negative: **rolling deploy.** Until the last v0.14.x replica is replaced, an old replica reads
  `projects` and serves a project deleted through a new replica. Its writes to that project land,
  hidden, and come back with a restore. Its unlocked instance-admin rename can rename a deleted
  project, with no record, so the restore would bring back a different name (an INV-PL-4 and
  INV-PL-7 exception, limited to the window). Nothing is lost. The window is the rollout. The
  CHANGELOG says: do not delete or rename projects while this release rolls out.
- Negative: psql `SELECT … FROM projects` still shows deleted rows. That is intended (D5: the
  database is one of the two ways to see them).
- Earned Trust (store gold tests on `postgres:16`):
  - `live_projects` has exactly `projects`' columns, in 0026 and again after 0027;
  - `UPDATE live_projects … RETURNING` that waited on a delete's `FOR UPDATE` returns no row once the
    delete commits;
  - `lock_live_project` (ADR-PROJECT-LIFECYCLE-002), the function every writer calls, blocks a
    concurrent `FOR UPDATE` and returns no row once a delete commits;
  - a deleted project's attachment id, requested through the deleted project's path, answers the
    uniform 404 body. This is asserted directly, not only by differential equality, so the
    v0.14.1 download fix stays in force;
  - an `INSERT INTO projects` row appears in `live_projects`, and a row with `deleted_at` set does
    not;
  - the workspace delete cascade (the acceptance "workspace is deleted" step) still removes a
    workspace whose projects are live, deleted or detached.
