# Changelog

All notable changes to Foundry are documented here. The format follows
[keep-a-changelog](https://keepachangelog.com/en/1.1.0/) and the project
follows [Semantic Versioning](https://semver.org). Pre-1.0 releases permit
minor-version breaking changes, flagged with a `BREAKING` heading.

## [Unreleased]

## [v0.17.0] - 2026-10-20

### Added

- **`GET /api/v1/openapi.json`**, an OpenAPI 3.1 description of every `/api/v1` operation and
  answer: the issue, comment and history operations and listing and revoking machine tokens,
  each with every status and error code it can give. No token needed; the same bytes for every
  caller. It shows no token value, offers no way to create a token, and says where one is
  minted (`/admin/tokens`, by a signed-in workspace admin). It is committed at
  `crates/foundry-api/openapi.json`, and CI fails when it stops matching the code, including
  when any operation is left undescribed.
- **The API page at `/docs/api`**: every `/api/v1` operation, readable in the browser on your own
  instance, with the release it describes and a link to `/api/v1/openapi.json` that stays when
  scripting is off or the description cannot be loaded. No sign-in needed; it is the same for
  everyone and sets no cookie. It is served from the instance's own vendored swagger-ui 5.33.3
  and carries its own content policy, so the browser loads nothing from another site and no
  other site can frame it. No other page changes.
- **Try a call from the API page with a machine token**: Authorize takes a pasted machine token
  ("Machine token": kept in this tab only and sent only to this Foundry), and "Try it out" sends
  the call to the instance itself and shows its answer, refusals included, with the `curl` line
  that made it. The token is never stored: not in local or session storage, a cookie or the
  address, and a reload forgets it. A link cannot make the page load another description or
  configuration.
- **check-arch rule `api-openapi-operations`**: `cargo xtask check-arch` fails, naming the method,
  path and difference, when an `/api/v1` route, path parameter, request body or success answer
  disagrees with `crates/foundry-api/openapi.json`, or when a route cannot be read.
- **check-arch rule `api-openapi-statuses`**: `cargo xtask check-arch` fails, naming the operation,
  status and difference, when an answer the code can give (status, body kind or error code) is
  missing from `crates/foundry-api/openapi.json` or listed there without the code giving it, or
  when the code uses a form the rule cannot read.
- **check-arch rule `api-docs-page`**: `cargo xtask check-arch` fails, naming the file and line,
  when the API page moves from `/docs/api`, another template loads swagger-ui, the page's
  initializer names a remote address, drops a pinned option (`url`, `validatorUrl: "none"`,
  `persistAuthorization: false`, `queryConfigEnabled: false`, `withCredentials: false`) or uses
  `urls`, `configUrl`, `spec`, browser storage (local or session storage, IndexedDB, the Cache
  API, `window.name`, cookies) or eval, sets a pinned option more than once, the page's markup
  gains inline code, its `script-src` becomes anything but `'self'`, or the vendored stylesheet
  loads anything.

### Fixed

- Revoking an already-revoked machine token now keeps its first revocation time; the store
  previously moved it to the time of the latest revoke.

### Security

- **rustls-webpki 0.103.16** (RUSTSEC-2026-0382) and **webpki-roots 1.0.10**
  (RUSTSEC-2026-0381): outgoing TLS no longer accepts a certificate path through the
  distrusted emSign intermediates, whose private keys are now public.

### Documented

- Existing behaviour now written down: a JSON body over 2 MB answers 413 `text/plain`; a token
  that cannot be checked answers 401; listing tokens answers the whole workspace's tokens
  whatever team and project the address names; revoking a token can answer 429 before 403.

### Releasing

- `RELEASING.md` step 3 now runs `cargo xtask openapi` (sets `info.version` in
  `crates/foundry-api/openapi.json` to the new release, touching nothing else) and
  `cargo xtask smoke`. The image build runs `cargo xtask openapi --check` and refuses a
  description that names another release.

### Unchanged

- Any method other than GET or HEAD on `/api/v1/openapi.json` answers the uniform 404 envelope,
  as before the route existed. The mechanism is a method fallback to the catch-all's handler on
  that one route (`any(api_not_found_handler).get(openapi_document)`). **This is an
  exception:** every other routed `/api/v1` path still answers a wrong method with axum's 405 and
  an `Allow` header. Every existing `/api/v1` answer is byte-identical.

## [v0.16.0] - 2026-10-17

### Added

- **`foundry doctor set-role`.** An operator makes an existing workspace member an admin or a
  member: `foundry doctor set-role --workspace <id|name> --email <addr> --role admin|member`.
  It never adds a membership, and it refuses to demote a workspace's last admin (exit 4,
  `status: last-admin`), even when two admins demote themselves at the same moment. Until now
  this took hand-written SQL. No migration.

### Fixed

- **A workspace can no longer lose its last admin by removal.** `POST /workspace/members/remove`
  refuses to remove a workspace's only admin with a 409: "{name} ({email}) is the only admin of
  {workspace}. Make someone else an admin first." Nothing changes and no removal notice is sent.
  This is checked before the last-lead refusal, and a removal racing `set-role` or another
  removal still leaves one admin. No migration.

### Security

- **ammonia 4.1.8** (RUSTSEC-2026-0373): the comment-markdown sanitizer now matches the CSS
  `url(` function regardless of case.

## [v0.15.1] - 2026-10-17

### Changed

- **One check for team-scoped machine tokens.** The check that keeps a token scoped to one team
  out of every other team now lives in one place instead of five. Behaviour is unchanged: such a
  request still gets the same 404 as a team that does not exist. No migration.

## [v0.15.0] - 2026-10-16

### Added

- **Rename a project from its team's members page.** A team's lead or a workspace admin renames
  a project from the Projects section; the slug, the key and every link stay. Each effective
  rename, from the members page or the instance dashboard, writes one
  `project_lifecycle_events` row.
- **Delete a project from its team's members page.** A team's lead or a workspace admin follows
  **Delete** in the Projects section, sees how many issues the project has and that nothing is
  erased, and confirms. The project is hidden, not erased: its issues, comments, attachments,
  change events, lanes and issue counter are kept. Each delete writes one
  `project_lifecycle_events` row.
- **The operator lists and restores deleted projects.** `foundry doctor list-deleted-projects`
  prints one block per deleted project, oldest deletion first (`project-id`, `workspace`, `team`,
  `name`, `key`, `issues`, `deleted-at`, `deleted-by`), or `status: none`.
  `foundry doctor restore-project <uuid>` brings one back exactly as it was and prints
  `status: restored`. Both run against `DATABASE_URL` with `restore-comment`'s exits: 0 done,
  2 malformed id, 3 database unreachable or failed, 4 not restorable (the project is not deleted
  or is not in the database), with the reason on stderr. A refused restore changes nothing.

### Changed

- **A deleted project answers like one that never existed.** Its board, issues, report, live
  updates and every other web and API address answer the same "not found" as a project that was
  never created, and no write to it is accepted.
- **`foundry doctor restore-comment`** on a deleted comment inside a deleted project changes
  nothing and exits 4: `foundry doctor restore-comment: comment {uuid} is in deleted project
  {project uuid} — status: not restorable`. Restore the project first.
- **A deleted project's name, address and key stay taken until it is restored.** Creating a
  project with its name in the same team, renaming another project to its name, or reusing its key
  in the workspace is refused as if it were live, so a restore never collides.
- **Only live projects keep a team from being deleted.** A team whose projects are all deleted
  can be deleted; its delete page adds "{team} also has {k} deleted project(s): {list}. They are
  kept, but can't be restored after {team} is deleted, and their keys stay taken in
  {workspace}." The deleted projects are kept, detached from the gone team, with every issue,
  comment and attachment. A team with a live project is still refused, on the page and on a
  submitted delete (409), and the refusal names only the live projects.
- **A deleted project whose team was deleted cannot be restored.** It stays on
  `foundry doctor list-deleted-projects` (its `team` line names the deleted team's address);
  `foundry doctor restore-project` on it changes nothing and exits 4: `foundry doctor
  restore-project: project {uuid}: its team {team_slug} was deleted — status: not restorable`.
  Its key stays taken in the workspace.
- **The API no longer tells a script whether a team exists.** A request to a team the
  credential's user is not on, or outside the team a credential is scoped to, now answers the
  same `404 not_found` as a team that does not exist, byte for byte (it was `403 forbidden`).
  Editing someone else's comment is still `403 forbidden`.
- **Live updates refuse with "not found".** Opening a board's live updates for a team you are not
  on, or for a project that does not exist or was deleted, now answers the same "not found" page
  as every other page (it was a `403` naming the team). Signed out is still the sign-in page.
- **The API's token routes check their address.** `GET` and `DELETE`
  `/api/v1/teams/{team}/projects/{project}/tokens…` now answer `404 not_found` when the team or
  project does not exist, was deleted, or is outside a scoped credential's team; a caller who is
  not a workspace admin still gets `403 forbidden` first. The list and the revoke still cover the
  whole workspace.
- **A lead who removes themselves lands on their teams list.** Removing yourself from a team you
  lead, while another lead remains, now shows your teams page with "You left {team}." instead of
  a "not found" page. A workspace admin who removes themselves still sees the members page.

### Upgrading

- **Do not delete or rename projects while this release rolls out**, while the old pod is still
  serving.
- **OQ-D4:** At release, dev had 3 projects in 1 workspace and prod 1 in 1. None is deleted when
  the upgrade runs, so every project stays live.
- **Rollback:** An older release runs on the new schema. Deleted projects become visible again
  until you upgrade back. Nothing is lost.

### Migration notes

- **`0025_project_lifecycle_events`** adds one table, `project_lifecycle_events`: an
  append-only record of each project rename, delete and restore, with `id`, `workspace_id`
  (`ON DELETE CASCADE` with its workspace), `project_id`, `team_id`, `project_slug`,
  `key_prefix`, `kind` (`renamed`, `deleted` or `restored`), `actor_id` (references `users`,
  no `ON DELETE` action; empty only for a restore, which the operator runs), `name`,
  `new_name` and `created_at`. It also adds the CHECK `project_lifecycle_events_kind_shape`
  (a rename has an actor and names a different new name; a delete has an actor and names none;
  a restore has no actor and names none) and two indexes, on `(workspace_id, created_at)` and
  `(project_id, created_at)`. `project_id` and `team_id` have no foreign key, so a record
  outlives its project and its team.
  - **Additive.** The table is new and empty; no existing row is read or written. The
    migration checks its own result and refuses to apply if the columns, the CHECK or the
    absence of a link to `projects` and `teams` differ.
  - **Not in the per-workspace export.** The table is not one of the ten `TENANT_TABLES`, so
    `export-workspace` and `verify-export` are unchanged. The rows are in every whole-instance
    `pg_dump`.
  - **Readiness:** `Store::probe` now requires `project_lifecycle_events`, so a binary from
    this release refuses `/readyz` against a schema without it.
  - **Rollback is safe.** An older release boots against the migrated database and never
    reads the table. To remove it by hand:

    ```sql
    DROP TABLE IF EXISTS project_lifecycle_events;
    ```

- **`0026_project_soft_delete`** adds the deleted marker to `projects`: `deleted_at`
  (timestamptz) and `deleted_by` (references `users`, no `ON DELETE` action), both empty for a
  live project, with the CHECK `projects_deleted_marker_shape` (both set or both empty). It also
  creates the view `live_projects` (`SELECT * FROM projects WHERE deleted_at IS NULL`), which
  has exactly the columns of `projects` in their order.
  - **Additive.** Both columns are new and empty, so every existing project stays live; no
    existing row is read or written. The migration checks its own result and refuses to apply
    if the marker or its CHECK is missing, or if the columns of `live_projects` differ from
    those of `projects`, naming `live_projects`. A later migration that adds a column to
    `projects` recreates the view in the same file.
  - **In the per-workspace export.** A deleted project is still exported, with its
    `deleted_at` and `deleted_by`.
  - **Readiness:** `Store::probe` now requires `projects.deleted_at`, `projects.deleted_by` and
    `live_projects`, so a binary from this release refuses `/readyz` against a schema without
    them.
  - **`foundry doctor restore-comment`** on a comment inside a deleted project changes nothing
    and exits 4 (`status: not restorable`), naming the project.
  - **Rollback is safe.** An older release boots against the migrated database and never reads
    the marker or the view. To remove them by hand, while no project is deleted:

    ```sql
    DROP VIEW IF EXISTS live_projects;
    ALTER TABLE projects DROP CONSTRAINT IF EXISTS projects_deleted_marker_shape,
        DROP COLUMN IF EXISTS deleted_by, DROP COLUMN IF EXISTS deleted_at;
    ```

- **`0027_project_detach`** lets a deleted project outlive its team: `projects.team_id` may now
  be empty, and the new `projects.detached_from_team_id` (uuid, no foreign key) names the team
  a detached project belonged to. The CHECK `projects_team_or_detached` keeps them exclusive:
  a project has its team and names none, or it is deleted, has no team and names the team it
  was detached from, so a live project can never lose its team. `projects_team_id_fkey` is
  still `ON DELETE CASCADE`. It also recreates `live_projects` so the view keeps exactly the
  columns of `projects`. It ships in the same release as 0026.
  - **Additive.** The column is new and empty, and every existing project has its team, so
    the CHECK holds for every row; no existing row is read or written. The migration checks
    its own result and refuses to apply if the column or its CHECK is missing, if the team
    foreign key no longer cascades, or if the columns of `live_projects` differ from those of
    `projects`, naming `live_projects`.
  - **In the per-workspace export.** A detached project is still exported, with its
    `detached_from_team_id`.
  - **Readiness:** `Store::probe` now requires `projects.detached_from_team_id`, so a binary
    from this release refuses `/readyz` against a schema without it.
  - **Rollback is safe.** An older release boots against the migrated database; it counts
    deleted projects when deleting a team, so it refuses more team deletes, never fewer. To
    remove the detach by hand, while no project is detached:

    ```sql
    ALTER TABLE projects DROP CONSTRAINT IF EXISTS projects_team_or_detached;
    ALTER TABLE projects ALTER COLUMN team_id SET NOT NULL;
    DROP VIEW IF EXISTS live_projects;
    ALTER TABLE projects DROP COLUMN IF EXISTS detached_from_team_id;
    CREATE VIEW live_projects AS SELECT * FROM projects WHERE deleted_at IS NULL;
    ```

### Backups

- **A whole-instance dump taken before 0025 no longer restores over the upgraded database.**
  `project_lifecycle_events` references `workspaces` and `users`, so `pg_restore --clean` of an
  older dump cannot drop those tables and fails. Restore such a dump into a fresh, empty
  database and boot foundry against it, which applies the newer migrations (or run
  `DROP TABLE project_lifecycle_events;` first). Dumps taken after the upgrade restore normally.
- **A whole-instance dump taken before 0026 no longer restores over the upgraded database.**
  The view `live_projects` depends on `projects`, and `projects.deleted_by` references `users`,
  so `pg_restore --clean` of an older dump cannot drop those tables and fails. Restore such a
  dump into a fresh, empty database and boot foundry against it, which applies the newer
  migrations (or drop `live_projects` and `projects.deleted_by` first). Dumps taken after the
  upgrade restore normally. A boot that upgrades through 0025 and 0026 logs one warning for
  each, in that order; a fresh install logs none.

## [v0.14.2] - 2026-10-14

Updates two dependencies with security advisories. No migration. `git log v0.14.1..v0.14.2` is
the full list.

### Security

- **ammonia 4.1.4 -> 4.1.7** for RUSTSEC-2021-0157 (quotes in CSS `url("")` not escaped in
  `style` attributes) and RUSTSEC-2021-0158 (XSS through SVG `animateMotion`,
  `animateTransform` and `animateColor`). Foundry was not exposed: ammonia only sanitises
  comment markdown, raw HTML in a comment is escaped before parsing, and the allowlist has no
  `style` attribute and no SVG elements.
- **rustls 0.23.45 -> 0.23.46** for RUSTSEC-2026-0346 (`Debug` output of client session values
  showed TLS secrets), RUSTSEC-2026-0347 (buffered data sent to the outer server after an ECH
  rejection) and RUSTSEC-2026-0348 (Brotli certificate decompression could allocate 1 GiB).
  rustls is in the shipped binary (Keycloak and other HTTPS calls, SMTP, Postgres TLS) as well
  as test tooling, but Foundry was not exposed: it has no custom session store, does not turn on
  ECH, and builds rustls without the `brotli` feature.

## [v0.14.1] - 2026-10-13

Fixes three ways a member could reach an issue, attachment or comment outside the address in the
URL. No migration. `git log v0.14.0..v0.14.1` is the full list.

### Security

- **Issues are found through their own project, not their key prefix.** Moving an issue on the
  board (web and API), saving an issue edit, and opening the edit form looked the issue up by
  project key prefix and number alone. A key prefix is unique only within a workspace, so with
  two workspaces both using `OPS`, a member of one could move, edit or read the other's `OPS-3`.
  These lookups now use the project the URL resolved to.
- **Attachments download only through their own issue.** A download used to serve any attachment
  in the workspace given its id, through any issue URL the member could open. It now answers the
  "not found" page unless the attachment belongs to the issue in the URL.
- **Comments are reached only through their own issue.** Viewing a comment, its edit form,
  saving an edit, deleting it and the API's comment update looked the comment up by id within
  the workspace. A member of any team could read any comment in the workspace. Edit and delete
  were still limited to the author (and admins for delete). They now answer "not found" unless
  the comment belongs to the issue in the URL.
- **Not exploited.** At release, dev and prod each have one workspace and no attachments or
  comments.

### Changed

- **One "not found" page.** An unknown team, project or issue, a team you're not on, and an
  attachment or comment from another issue all answer the same `404` page, which no longer
  repeats the team name back. This covers the keyboard search and the new-issue dialog.
  Editing or deleting a comment through an unknown address used to answer "Comment not found"
  (edit) or a page naming the team (delete).
- **`cargo xtask check-arch` has a new rule, `store-natural-key-scoped`.** A store query that
  filters on `key_prefix`, `slug` or `number` must also filter on `workspace_id`, `team_id` or
  `project_id`.

## [v0.14.0] - 2026-10-12

Create, rename and delete teams from the product (team-lifecycle): a workspace admin creates
a team and leads it, an admin or the team's lead renames it without changing any link, and an
admin deletes it, refused while the team still has projects. Team names follow one rule in
the app and the database. Upgrading through 0023 changes how old backups restore; read the
migration notes. `git log v0.13.0..v0.14.0` is the full list.

### Added

- **Create a team.** A workspace admin can now create a team on `/teams` with
  **Create team**. The creator becomes the team's lead, and the page
  re-renders with "Created {team}. You're its lead." A name the rule refuses
  (see below) answers `422` with the reason above the field, which keeps what
  was typed. Anyone who isn't a workspace admin sees no form, and a `POST`
  from them gets the same "not found" page as a team that doesn't exist.
  Instance admins get no access from being instance admins.
- **New project on the members page.** Anyone on a team sees a **New project**
  link on `/team/{team}/members`, which opens that team's new-project form.
- **The dashboard's "create your first project" link names a team you're
  on.** It used to point at `/team/general/projects/new` for everyone; it now
  goes to the first team you're on (by name).
- **Rename a team.** A workspace admin, or a lead of that team, can rename it
  from its members page. The page re-renders with "Renamed {old} to {new}."
  Only the name changes: the team's address (slug) stays the same, so every
  link and bookmark keeps working. Submitting the same name writes nothing and
  shows no notice; a change of case is a real rename. A name the rule refuses
  answers `422` with the reason above the Rename field, which keeps what was
  typed. Each rename writes one record to `team_lifecycle_events` (read it
  with `psql`; there is no page for it yet).
- **Delete a team.** A workspace admin sees **Delete team** on a team's members
  page. It opens a confirm page at `/team/{team}/delete` that names the
  people who will lose the team, warns when it is the workspace's default team
  for new members ("After it is deleted, new members will join no team."), and
  counts the machine tokens scoped to it, which will stop working. Confirming
  deletes the team and re-renders `/teams` with "Deleted {team}." A team that
  still has any project cannot be deleted: the confirm page shows
  "{team} still has {n} project(s): {list}. A team with projects can't be
  deleted." with no Delete button, and a delete that meets a project created
  at the same moment answers `409 Conflict` and deletes nothing. Each delete
  writes one record to `team_lifecycle_events`. Team leads cannot delete their
  team; only workspace admins can.
- **One rule for team names.** A team name is trimmed and must be non-empty,
  at most 64 characters, free of control and bidirectional-override
  characters, and unique within the workspace: no other team may have the
  same name ignoring case, or a name that makes the same address. The
  refusals read "Team name must not be empty", "Team name must not contain
  control characters", "Team name must be at most 64 characters" and "Team
  name must be unique within the workspace". A name in any script gets an
  address: one with no ASCII letters or digits gets `team`, then `team-2`,
  and so on. The database refuses a bad new team name from any other tool as
  well (see `0024_team_name_rule` below).

### Changed

- **A project create on a team that no longer exists answers the uniform
  `404`** "not found" page, the same page as any team you may not see. It
  used to answer a separate "Team not found" page (or a `500` when the team
  was deleted mid-request).
- **A machine-token mint that races a team delete answers `422`** with the
  existing `scope_team_not_in_workspace` error, the same answer as a team that
  was never there, instead of a `500`. Adding someone to a team, or setting
  the default team, while that team is being deleted answers the uniform
  `404`.

### Migration notes

- **`0023_team_lifecycle_events`** adds one table, `team_lifecycle_events`: an
  append-only record of each team rename and each team delete, with
  `id`, `workspace_id` (`ON DELETE CASCADE` with its workspace), `team_id`,
  `team_slug`, `kind` (`renamed` or `deleted`), `actor_id` (references
  `users`, no `ON DELETE` action), `name`, `new_name` and `created_at`. It
  also adds the CHECK `team_lifecycle_events_kind_shape` (a rename names a
  different new name; a delete names none) and the index
  `idx_team_lifecycle_events_workspace_created` on `(workspace_id, created_at)`.
  `team_id` has no foreign key to `teams`, so a delete record outlives the team.
  - **Additive.** The table is new and empty; no existing row is read or
    written. The migration checks its own result and refuses to apply if the
    columns, the CHECK or the absence of a link to `teams` differ.
  - **Not in the per-workspace export.** The table is not one of the ten
    `TENANT_TABLES`, so `export-workspace` and `verify-export` are unchanged.
    The rows are in every whole-instance `pg_dump`.
  - **Readiness:** `Store::probe` now requires `team_lifecycle_events`, so a
    binary from this release refuses `/readyz` against a schema without it.
  - **Rolling deploys are safe.** An older replica never reads or writes the
    table. One caveat for the overlap: a project create served by an older
    replica that races a team delete served by a newer one may answer `500`
    instead of "not found". No project is lost.
  - **Backups taken before 0023 no longer restore over the upgraded
    database.** `team_lifecycle_events` references `workspaces` and `users`,
    so `pg_restore --clean` of an older dump cannot drop those tables and
    fails. The boot that upgrades a database through 0023 logs this once, as a
    warning; a fresh install does not. Discard pre-0023 backups once the
    upgrade is verified, and take a new backup. If you must restore an older
    dump, restore it into a fresh database, or run
    `DROP TABLE team_lifecycle_events;` first. Dumps taken after 0023 restore
    as before.
  - **Rollback to v0.13.0 is safe.** It boots against the migrated database
    and never reads the table. To remove it by hand:

    ```sql
    DROP TABLE IF EXISTS team_lifecycle_events;
    ```

- **`0024_team_name_rule`** adds two triggers on `teams`:
  `teams_name_rule_on_insert` (`BEFORE INSERT`) and `teams_name_rule_on_rename`
  (`BEFORE UPDATE OF name`, only when the name changes). Both apply the name
  rule of 0019 at the team cap of 64 characters, reusing
  `foundry_name_rule_violation(text, integer)` unchanged. A new or renamed team
  name that breaks the rule is refused with SQLSTATE `23514` and one of the
  constraint names `teams_name_not_empty`, `teams_name_trimmed`,
  `teams_name_no_control_chars` or `teams_name_max_64_chars`. The HINT reads
  "The name must be a valid foundry_core::TeamName: trimmed, not empty, no
  control characters, at most 64 characters."
  - **The shared trigger function is replaced.** `foundry_enforce_name_rule()`
    is re-created with the same body plus one HINT line for `teams`.
    Workspaces and projects keep their rules, caps and HINTs.
  - **No row is scanned or rewritten.** The rule applies to new writes only.
    A team name stored before 0024 that breaks the rule stays as it is, and
    writes that keep it (the slug, or the same name) still succeed. Renaming
    it to another bad name is refused. The migration checks its own result
    and refuses to apply if the verdict at 64 or either trigger differs.
  - **Counted before this release, read-only, on 2026-10-11:** dev and prod
    each hold 1 team, with 0 team names that break the rule and 0
    case-insensitive duplicate team names within a workspace.
  - **Rolling deploys are safe.** The only team write an older replica makes
    is the bootstrap's `General`, which the rule accepts.
  - **Rollback floor: v0.13.0.** Nothing an older binary reads changes. To
    remove the rule by hand (the extra HINT line is inert without a `teams`
    trigger and may stay):

    ```sql
    DROP TRIGGER IF EXISTS teams_name_rule_on_insert ON teams;
    DROP TRIGGER IF EXISTS teams_name_rule_on_rename ON teams;
    ```

    The v0.12.0 undo of the whole name rule (drop the triggers, then both
    functions) now drops these two triggers too, before the functions. To
    re-arm, forget versions 19, 20 and 24 in `_sqlx_migrations` and boot.

## [v0.13.0] - 2026-10-10

Teams you can manage from the product: a Teams page and per-team member lists, adding
people, making them lead or member, and removing them, with every team always keeping a
lead; plus a default team for new members (team-membership). `git log v0.12.0..v0.13.0` is
the full list.

### Added

- **Teams.** The dashboard's quick actions now start with **Teams**, which
  opens `/teams`. A workspace admin sees every team in the workspace; anyone
  else sees the teams they are on, or "You're not on any team yet — ask a
  workspace admin to add you." Each team links to its members page.
- **A members page for every team** at `/team/{team}/members`. It lists the
  team's people, leads first and then by name, each shown as display name and
  email. A workspace admin can **Add** a workspace member to the team (as a
  member), **Make lead**, **Make member** and **Remove**. A team's own leads can
  do the same for their team. Anyone else on the team sees the page read-only.
  Each change re-renders the page with a one-line notice, for example
  "Added Lena Park (lena.park@canzan.dev) to Backend as a member." Someone who
  may not see the team, or a person who is not in the workspace, gets the same
  "not found" page as a team that does not exist. Instance admins get no
  access from being instance admins.
- **A team is never left without a lead.** Making the last lead a member, or
  removing them, is refused with `409 Conflict` and the members page saying
  "{team} needs at least one lead. Make someone else a lead first." Nothing is
  written. Two admins demoting the last two leads at the same moment cannot
  both win. The database refuses such a write from any other tool as well (see
  `0021_team_lead_guard` below). A team that has never had a lead is not
  forced to get one.
- **A default team for new members.** A workspace admin can pick a "Default
  team for new members" on `/teams`, or set it back to none. A newcomer who
  accepts a member invite, or who signs in with Keycloak for the first time and
  so gets a new account, joins the workspace and lands on that team as a
  member, and so can open its boards straight away. Linking an existing
  account to Keycloak and accepting the first-admin invite place no one. Picking
  a default moves nobody already in the workspace. If the placement fails, the
  newcomer still joins, on no team, and one warning with ids only is logged.

### Changed

- **`POST /workspace/members/remove` also takes the person off the
  workspace's teams.** Removing someone from a workspace now deletes their team
  rows in that workspace in the same transaction; before, the rows stayed
  behind, granting nothing (0021 deletes the leftovers, see below). If the
  person is the last lead of any team in the workspace, the removal is refused
  with `409 Conflict`, `text/plain`, saying
  "{name} ({email}) is the last lead of {team}. Make someone else a lead
  first." Nothing is removed and no `member_removed` notification is sent. Make
  someone else a lead of that team first.

### Fixed

- **Board action row spacing.** The New issue and Search buttons on a board no
  longer touch, and the Change report link, the buttons and the lanes are
  spaced apart. The stylesheet is re-hashed to `foundry.8d661015.css`.

### Migration notes

- **`0021_team_lead_guard`** adds one function, `foundry_team_keeps_a_lead()`,
  and two triggers on `team_memberships`: `team_memberships_lead_guard_on_delete`
  (`BEFORE DELETE`, only for a lead row) and
  `team_memberships_lead_guard_on_update` (`BEFORE UPDATE OF role, team_id`,
  only when a lead row stops being a lead of its team). A write that would
  leave a team that had a lead with none is refused with SQLSTATE `23514` and
  constraint `team_memberships_team_keeps_a_lead`. The function locks the team
  row `FOR NO KEY UPDATE`, the same lock the store takes, so adds and project
  creates never wait on it. Deleting a team or a workspace is exempt: a team
  that is gone needs no lead.
  - **It deletes rows once.** Before it installs the triggers, 0021 deletes the
    team rows of people who are no longer members of the team's workspace (left
    behind by earlier workspace removals). Such rows grant nothing. It reports
    the count with a `NOTICE` (expected 0) and refuses to apply if any remain.
  - **Before upgrading, count those rows** (read-only; expected `0`). Counted on
    2026-10-10 before v0.13.0: dev 0, prod 0 (OQ-D4):

    ```sh
    psql "$DATABASE_URL" -At <<'SQL'
    BEGIN READ ONLY;
    SELECT count(*)
      FROM team_memberships tm JOIN teams t ON t.id = tm.team_id
     WHERE NOT EXISTS (SELECT 1 FROM workspace_memberships wm
                        WHERE wm.workspace_id = t.workspace_id AND wm.user_id = tm.user_id);
    COMMIT;
    SQL
    ```

  - **No other row is touched and nothing is backfilled.** Teams that have no
    lead today keep none; the guard only stops a team losing its last one.
  - **No new Postgres requirement and no readiness change.** `Store::probe` is
    unchanged by 0021; the app's own check, not the trigger, is what users
    meet, and the trigger is the backstop for other tools.
  - **Rolling deploys are safe.** No earlier release removes or demotes a team
    lead, so old replicas never meet the triggers during the overlap.
  - **Rollback floor: the previous release.** It boots against the migrated
    database and ignores migration 21. Nothing an older binary reads changes.
    The deleted orphan rows do not come back. To remove the guard by hand:

    ```sql
    DROP TRIGGER IF EXISTS team_memberships_lead_guard_on_delete ON team_memberships;
    DROP TRIGGER IF EXISTS team_memberships_lead_guard_on_update ON team_memberships;
    DROP FUNCTION IF EXISTS foundry_team_keeps_a_lead();
    ```

  - **A consequence for later work (OQ-D9):** a future hard delete of a user who
    is the only lead of a team is refused until leadership moves to someone
    else. No user delete exists today.

- **`0022_workspace_default_team`** adds the default team for new members. It
  adds the nullable column `workspaces.default_team_id`, a unique constraint
  `teams_workspace_id_id_key UNIQUE (workspace_id, id)` (redundant as
  uniqueness, but the target of the next key), and the foreign key
  `workspaces_default_team_same_workspace`: `(id, default_team_id)` references
  `teams (workspace_id, id)` `ON DELETE SET NULL (default_team_id)`. A default
  must be one of the workspace's own teams; naming another workspace's team is
  refused with SQLSTATE `23503`. Deleting the default team clears only the
  default. A newcomer who accepts a member invite, or whose first Keycloak
  sign-in creates their account, lands on the default team as a member. If
  that placement fails, the newcomer still joins the workspace, on
  no team, and one warning with ids only is logged.
  - **Unset on upgrade, no backfill.** Every existing workspace has no default,
    so behaviour is unchanged until an admin picks one, and picking one moves
    nobody already in the workspace.
  - **Postgres 15 or later is required** for the column-list `SET NULL`.
    Production and the test harness run 16. An older server refuses 0022 loudly
    rather than install a weaker key.
  - **Readiness:** `Store::probe` now requires `workspaces.default_team_id`, so
    a binary from this release refuses `/readyz` against a schema without it.
  - **Rolling deploys are safe.** An older replica never reads the column. Its
    `INSERT INTO workspaces` leaves it NULL, and an invite accept or first
    Keycloak sign-in it serves lands the newcomer on no team, which is today's
    behaviour.
  - **Rollback floor: the previous release.** It boots against the migrated
    database, ignores migration 22 and never reads the column. To remove it by
    hand (the foreign key first, then the column, then the unique key):

    ```sql
    ALTER TABLE workspaces DROP CONSTRAINT IF EXISTS workspaces_default_team_same_workspace;
    ALTER TABLE workspaces DROP COLUMN IF EXISTS default_team_id;
    ALTER TABLE teams DROP CONSTRAINT IF EXISTS teams_workspace_id_id_key;
    ```

## [v0.12.0] - 2026-10-09

The database now enforces the workspace-name and project-name rules on every
new name write (name-db-checks), and the dashboard offers only boards you can
open (fix-hide-unreachable-boards). `git log v0.11.0..v0.12.0` is the full list.

### Changed

- **A bad name written straight to the database is refused.** A workspace or
  project name written by hand in `psql`, by a script or by another tool now
  meets the same rule as the app's doors: trimmed, not empty, no control
  characters, and at most 24 characters for a workspace or 256 for a project.
  The database refuses a bad insert or rename with SQLSTATE 23514 (the
  `check_violation` code) and names the rule it broke as the constraint, for
  example:

  ```
  ERROR:  new row for relation "workspaces" violates check constraint "workspaces_name_trimmed"
  HINT:  The name must be a valid foundry_core::WorkspaceName: trimmed, not empty, no control characters, at most 24 characters.
  ```

  The rule names are `<table>_name_not_empty`, `<table>_name_trimmed`,
  `<table>_name_no_control_chars`, `workspaces_name_max_24_chars` and
  `projects_name_max_256_chars`. The refusal never carries a DETAIL, so the
  refused name is not written to the database log.
  - **The app is unchanged.** The database rule is the app rule, so no app
    door reaches the refusal and no copy changes. Uniqueness stays the app's
    job.
  - **Existing names are untouched.** No row is checked or rewritten. A
    workspace or project stored with a name that breaks the rule keeps working,
    takes new issues, can be renamed to a fit name, and survives a write that
    leaves its name as it is. Only a change of name meets the rule.

### Fixed

- **The dashboard and the Board link only offer boards you can open.** "Your
  projects" and the rail's Board link now list only the projects of the teams
  you are on. Before, a member who joined through an invite link or single
  sign-on, and so was on no team yet, was shown every project in the workspace
  and a Board link that opened a "not found" page. A member on no team now sees
  "You're not on any team yet — ask a workspace admin to add you." instead of a
  create-project link they couldn't use, and the Board link goes to the
  dashboard. The instance-admin project list still shows every project. No
  database migration.

### Migration notes

- **`0019_workspace_name_rule`** adds two functions,
  `foundry_name_rule_violation(text, integer)` (the verdict) and
  `foundry_enforce_name_rule()` (the trigger function), and two triggers on
  `workspaces`: `workspaces_name_rule_on_insert` (`BEFORE INSERT`) and
  `workspaces_name_rule_on_rename` (`BEFORE UPDATE OF name`, only when the name
  changes). **`0020_project_name_rule`** adds `projects_name_rule_on_insert` and
  `projects_name_rule_on_rename` on `projects` over the same functions.
  - **No row is scanned or rewritten.** These are triggers, not CHECK
    constraints, so there is nothing to `VALIDATE`. `CREATE TRIGGER` takes a
    brief SHARE ROW EXCLUSIVE lock on each table.
  - **Both apply at boot** under `Store::migrate`'s `MIGRATION_LOCK_ID`
    advisory lock, so concurrent replicas apply them once. Each migration
    checks the database's trim, regex and length behaviour before it installs
    the rule, and refuses to apply if they differ.
  - **Check before upgrading that the database is UTF8.** 0019 refuses a
    non-UTF8 database, inside its own transaction, and the boot fails:

    ```sql
    SELECT pg_encoding_to_char(encoding) FROM pg_database WHERE datname = current_database();
    ```

  - **Rolling deploys are safe.** v0.11.0 replicas already write only valid
    names on every door, so they never meet the refusal during the overlap.
  - **Rollback floor: v0.11.0.** v0.11.0 boots against the migrated database
    and ignores migrations 19 and 20. Rolling back further, to a binary
    without the app rules (before v0.10.0 for workspaces, before v0.11.0 for
    projects), turns a bad name typed at that older door into a 500 instead
    of a stored bad name.
  - **After upgrading, count the names that break the rule** (read-only; it
    lists only the arms with at least one row, so no output means none). Run it
    with `psql` against the foundry database, for example:

    ```sh
    psql "$DATABASE_URL" -At <<'SQL'
    BEGIN READ ONLY;
    SELECT 'workspaces' AS tbl, foundry_name_rule_violation(name, 24) AS arm, count(*)
      FROM workspaces WHERE foundry_name_rule_violation(name, 24) IS NOT NULL GROUP BY 2
    UNION ALL
    SELECT 'projects', foundry_name_rule_violation(name, 256), count(*)
      FROM projects WHERE foundry_name_rule_violation(name, 256) IS NOT NULL GROUP BY 2;
    COMMIT;
    SQL
    ```

    Such names keep working. Rename them on the dashboard if you want them
    gone.
  - **Watch for refusals reaching the app.** One should never happen; if it
    does, it shows as an internal error whose log line names the rule. Search
    the app's logs for it after the release:

    ```sh
    kubectl -n foundry logs -l app.kubernetes.io/name=foundry --all-containers --prefix --since=168h \
      | grep -E '(workspaces|projects)_name_(not_empty|trimmed|no_control_chars|max_24_chars|max_256_chars)'
    ```

    No output is the expected result. Any hit is a bug: please report it.
  - **Backups.** A full `pg_restore` (for example with `--clean --if-exists`)
    loads table data before it creates the triggers, so names that break the
    rule restore unchanged. A data-only restore
    (`pg_restore -a`) into an already-migrated database fires the triggers on
    every row; run it with `--disable-triggers` to load such names.
  - **Undo.** The migrations are forward-only. To remove the rule by hand:

    ```sql
    DROP TRIGGER IF EXISTS workspaces_name_rule_on_insert ON workspaces;
    DROP TRIGGER IF EXISTS workspaces_name_rule_on_rename ON workspaces;
    DROP TRIGGER IF EXISTS projects_name_rule_on_insert ON projects;
    DROP TRIGGER IF EXISTS projects_name_rule_on_rename ON projects;
    DROP FUNCTION IF EXISTS foundry_enforce_name_rule(), foundry_name_rule_violation(text, integer);
    ```

    To put it back, delete the two migration records and restart Foundry,
    which re-applies both (their bodies are safe to re-run):

    ```sql
    DELETE FROM _sqlx_migrations WHERE version IN (19, 20);
    ```

## [v0.11.0] - 2026-10-07

Project names now follow one rule on both doors, and every new project gets a
working address (project-name-rule). `git log v0.10.0..v0.11.0` is the full
list.

### Changed

- **One project-name rule for create and rename.** The team create form and the
  instance-admin rename apply the same rule and refuse a bad name with the same
  words. The name is trimmed, must not be empty, must not contain control
  characters, must be at most 256 characters, and must be unique within the
  team.
  - **Create is stricter.** It used to accept any length and invisible
    characters, and it checked only the address for duplicates. It now refuses a
    name longer than 256 characters, a name containing control characters
    (newline, tab, NUL, bidi override and isolate characters, U+2028/2029, the
    same set as workspace names), and a name that matches another project in
    the team ignoring case, even if that project was renamed and lives at a
    different address. The new message is "Project name must not contain control
    characters".
  - **Rename refuses control characters** too. Resubmitting an existing name
    unchanged is still a quiet success, even if it predates the rule.
  - **A refused create leaves nothing behind**, so the key prefix stays free for
    the corrected retry. The form keeps the name and key you typed.
  - **A NUL in a name** used to cause an internal error. It is now refused like
    any other bad name.

### Fixed

- **Projects named without Latin letters or digits get a working address.** A
  name such as "日本語ボード" or "🚀" used to get an empty address, so its board
  could not be opened and a second such project was wrongly refused as a
  duplicate. Such a project's address is now its key prefix in lower case, for
  example `/team/backend/project/jp`, then `jp-2`, `jp-3` and so on if the team
  already uses it. Names with Latin letters keep the addresses they get today.
  Existing projects keep their addresses.

### Migration notes

- No database migration, and no existing project name or address is rewritten.
  No API or CLI change: neither writes project names.

## [v0.10.0] - 2026-10-06

Every way of naming a workspace now follows one rule, and control characters
are refused (instance-workspace-name-rule). `git log v0.9.0..v0.10.0` is the
full list.

### Changed

- **One workspace-name rule on every path.** Dashboard provisioning, the
  first-run bootstrap claim, `foundry doctor provision-workspace` and rename all
  apply the same rule, and refuse a bad name with the same words. The name is
  trimmed, must not be empty, must not contain control characters, and must be
  at most 24 characters. Before this release, only rename checked names.
  - **Refused characters:** Unicode control characters (newline, tab, NUL and
    so on), the bidi embedding/override/isolate characters (U+202A–202E,
    U+2066–2069) and the line/paragraph separators U+2028/2029. Zero-width
    joiners and other format characters stay allowed, so emoji and non-Latin
    spellings keep working. The new message is "Workspace name must not contain
    control characters".
  - **A refusal leaves nothing behind:** no workspace, user, membership or
    invite is created, and a bootstrap link is not used up, so you can correct
    the name and retry with the same link or email.
  - **The dashboard and the bootstrap claim page keep what you typed** (never
    the password) and show the reason on the page.
  - **A NUL in a name** used to cause an internal error; it is now refused like
    any other bad name.
  - **Existing names are not rewritten.** Resubmitting an existing name
    unchanged on rename is still a quiet success, even if it predates the rule.

### BREAKING (CLI)

- **`foundry doctor provision-workspace` refuses a name that breaks the rule.**
  Such a name now exits **2** with `foundry doctor provision-workspace: <reason>`
  on stderr and nothing on stdout, before any database is contacted. It used to
  exit 0 and create the workspace. Scripts that provisioned long or padded names
  must adjust.
  - `--name ""` now gets "Workspace name must not be empty" instead of the
    usage line. A missing `--name` still gets the usage line.
  - The name is trimmed, and the success output prints the trimmed name:
    `workspace-name: Globex` for `--name "  Globex  "`.

### Migration notes

- No database migration. Existing workspace names are left as they are. A
  database CHECK on `workspaces.name` is planned as a separate change.

## [v0.9.0] - 2026-10-05

An instance admin can now rename a workspace from the dashboard, and every rename
is on record (instance-admin-workspace-rename). `git log v0.8.0..v0.9.0` is the full
list.

### Added

- **Rename a workspace from `/admin/instance/workspaces`.** Each workspace row gains a
  rename form. The name is trimmed and must be 1 to 24 characters, counted as
  Unicode characters, not bytes. Refusals explain themselves inside the row, and a
  successful rename updates the row in place. Members see the new name and monogram
  on their next page. Only instance admins can rename; anyone else gets the same
  "not found" answer as a page that never existed.
  - **Resubmitting the current name is a quiet success** and writes nothing, so a
    workspace whose name predates the 24-character limit can be left as it is. A
    change of letter case is a real rename.
  - **Two workspaces may share a name**, as provisioning already allows.
- **Every effective rename is on record.** A new append-only
  `workspace_rename_events` table keeps who renamed which workspace, from what, to
  what, and when. It is written in the same transaction as the rename, so neither
  can exist without the other. There is no in-app viewer yet; query the table.
  Rename records are instance records and are **not** included in per-workspace
  exports (`doctor export-workspace` still writes the same ten tables).
- **A long workspace name stays on one line in the sidebar,** ending in an ellipsis,
  with the full name on hover. This holds at desktop width and on phones, without
  making the page scroll sideways. It also covers names created before the limit.

### Migration notes

- **Migration 0018** adds the `workspace_rename_events` table and nothing else: no
  backfill, and no change to `workspaces`. It runs automatically at startup and is
  safe during a rolling deploy, because v0.8.0 replicas never read the new table.
- **The readiness check** (`/readyz`) now refuses a database without the 0018 table,
  so a half-migrated database fails readiness instead of the first rename failing.
- **Rolling back to v0.8.0** is safe: the old binary ignores the extra table. Its
  rows remain and are picked up again on the next upgrade.

## [v0.8.0] - 2026-10-04

The page footer now names the build, not just the release (release-version-footer
US-RVF-02). `git log v0.7.0..v0.8.0` is the full list.

### Added

- **The footer names the commit it was built from.** Every full page now ends with
  `Foundry v0.8.0 · 2026-10-04` and carries the commit's short SHA in the footer's
  `data-commit` attribute (`<footer class="site-footer" data-commit="…">`). So an
  operator can tell which build is serving, not just which release, and can spot a
  stale image or a dev build that shares a version.
  - **The date is the commit date, not the build time,** so a rebuild of the same
    commit reads the same.
  - **Where the stamp comes from:** a new `crates/foundry-app/build.rs` takes it from
    the `FOUNDRY_STAMP_SHA` / `FOUNDRY_STAMP_DATE` build-args when they are set, else
    from git, else `unknown`. It never fails the build and adds no crates. Its rerun
    directives keep the stamp fresh, including after `git pack-refs`.
  - **How published images get it:** the Dockerfile builder stage, the Forgejo
    `build-and-publish.yml` and the GitHub `release.yml` now pass the build-args.
    Both workflows refuse to publish if either value is empty, and
    `cargo xtask check-arch` enforces that shape.
  - **RELEASING.md "Verifying a deployment"** now checks the footer date and
    `data-commit` against `git log -1 --format='%h %cd' --date=short <ref>`.

## [v0.7.0] - 2026-10-04

Withdrawing the Keycloak provision role now takes away Keycloak sign-in from
the accounts that role provisioning created (keycloak-sso OD-10 / US-07).
Migration 0017 records which accounts those are. `git log v0.6.2..v0.7.0` is
the full list.

### Changed

- **Withdrawing the provision role closes the Keycloak door to provisioned
  members.** With `FOUNDRY_OIDC_PROVISION_ROLE` set, an account that role
  provisioning created must still hold that realm role at every Keycloak
  sign-in. Without it, the sign-in gets the same generic refusal as a wrong
  password. The account, its membership and its work are kept, and granting
  the role again lets the member back in as the same account. Accounts that
  were invited or linked are unaffected: the role still means nothing for
  them.
  - **Takes effect at the next sign-in.** A session the member already has
    keeps working until it expires or they sign out.
  - **The password door stays open.** A provisioned member who has set a
    password can still sign in with it, and forgot-password still reaches a
    provisioned account. To take a member's access away completely, also
    remove their workspace membership.
  - **Unsetting `FOUNDRY_OIDC_PROVISION_ROLE` reopens the Keycloak door** to
    every member whose role was withdrawn, as well as stopping new accounts.
    With the variable unset, provisioned accounts sign in like any linked
    account.

### Migration notes

- **`0017_users_provisioned_at`**: `ALTER TABLE users ADD COLUMN
  provisioned_at TIMESTAMPTZ NULL`, then one scan of `users` that marks every
  password-less account as provisioned at its `created_at`. The column add is
  catalog-only (no default, no index); the scan touches only password-less
  rows and is safe to re-run. It is safe for rolling deploys and for rollback
  to v0.6.2: v0.6.2 ignores the column, and while it runs the role is not
  re-checked.
  - **Check before upgrading:** did production ever set
    `FOUNDRY_OIDC_PROVISION_ROLE`? If it never did, no account was
    provisioned and the scan marks zero rows.
  - **Accepted gap:** an account provisioned and then given a password
    through a reset before this upgrade has a password, so the scan cannot
    tell it was provisioned. It keeps signing in through Keycloak without the
    role. Remove its workspace membership if its access should end.

## [v0.6.2] - 2026-10-04

A hardening fix to Keycloak sign-in. `git log v0.6.1..v0.6.2` is the full
list.

### Security

- **Keycloak sign-in refuses an identity whose nonce is empty or missing.**
  `foundry-oidc` used to check only that the nonce in the ID token *equalled*
  the one foundry expected, not that both were present. A token with no
  `nonce` claim, answering an empty expected nonce, would therefore be
  accepted. Before any call to the provider, foundry now also refuses a
  sign-in whose challenge has an empty state, nonce or PKCE verifier.
  - **Not reachable before this fix.** Every real challenge carries 43
    random characters, and it travels in a cookie signed with
    `SESSION_SECRET`. A sign-in with no challenge cookie is refused before
    the exchange.
  - **Why it matters anyway.** The fix stops the nonce depending on the
    cookie check alone.
  - **What a refused sign-in looks like.** Refusals are unchanged: byte for
    byte, the same answer as a wrong password.
  - **After deploying.** Sign in once through the real Keycloak.

## [v0.6.1] - 2026-10-03

Acceptance coverage for Keycloak sign-in. There is no change to runtime
behaviour; `git log v0.6.0..v0.6.1` is the full list.

### Changed (internal)

- **The Keycloak sign-in flow now has acceptance coverage.** The 23 base
  `keycloak-sso.feature` scenarios, `@pending` since the flow shipped in
  v0.4.0, now run against the shipped code over real HTTP and real Postgres
  with an in-process identity-provider double. They cover:
  - arrival and the walking skeleton;
  - who is turned away;
  - forged, stale and replayed arrivals;
  - an unreachable provider;
  - refusals byte-identical to a wrong password;
  - the password door and the bootstrap claim alongside SSO;
  - running without a provider, and a half-configured provider refusing to
    start.

  All 23 passed against the existing code, so no production fix was needed.
  25 named faults seeded in the production code were each caught by a
  scenario.

## [v0.6.0] - 2026-10-03

Card drag on phones and tablets (card-pointer-drag slices 02 and 03, which
completes the feature), opt-in Keycloak role provisioning, a per-workspace
`list-users`, and a fail-closed `backup-verify`. `git log v0.5.0..v0.6.0` is
the full list.

### Added

- **Cards drag on touch and pen.** Every card carries a grip. A touch or pen
  press on the grip lifts the card once it has travelled 3 px, with no wait and
  no page scroll; a tap on the grip opens nothing. A press on the card's text
  is a hold: the card visibly arms (it eases to 96% scale and 70% opacity over
  the 500 ms hold, dim-only under `prefers-reduced-motion`) and then lifts. A
  swipe that starts on the text still scrolls the page or the board, and a tap
  still opens the card. Once lifted, touch carries the card exactly as the
  mouse does: the lane lights, the marker shows the landing slot, and the drop
  sends the same move request. Cards gain a 48 px minimum height so the grip is
  easy to hit on one-line cards. Stylesheet content-hash rotated `f9143163` →
  `7fa13f60` (the grip) → `6b3e4436` (the arming cue).
- **Carried cards scroll the board and the page at the edges.** Holding a
  carried card within 48 px of the board's side edge scrolls the board towards
  the off-screen lanes, 14 px per frame, stopping at the board's end; holding
  it near the top or bottom of the window scrolls the page, so a long lane's
  end is reachable on a phone. The marker and the landing slot are recomputed
  after every scroll step. This works for mouse, touch and pen.
- **Opt-in provisioning of Keycloak realm-role holders.** Set
  `FOUNDRY_OIDC_PROVISION_ROLE` to a realm role name. A verified identity with
  no Foundry account whose ID token carries that role (exact, case-sensitive,
  realm roles only) gets a password-less `member` account in the instance's
  original workspace on first sign-in. It is never made an admin. Unset or
  blank keeps the previous link-only behaviour. A missing role, an unconfirmed
  address or provisioning switched off all return the same generic refusal as
  a wrong password. Password-less members can set a password through
  forgot-password.
- **`foundry doctor list-users --workspace <id|name>`** lists one workspace's
  members, each with a `workspace-role: admin|member` line. The selector is
  the same as `export-workspace`'s: an id, or an exact case-insensitive name.
  It exits 2 when no workspace matches and 3 when the database is unreachable.

### Changed

- **Interruptions leave nothing behind on touch.** When the system takes the
  pointer from a carried card (a notification shade, a second gesture), the
  card goes back to its exact slot and nothing is sent. A hold the system
  interrupts before the lift leaves no arming cue, and a release off every lane
  changes nothing.

### Migration notes

- **`0016_nullable_password_hash`**: `ALTER TABLE users ALTER COLUMN
  password_hash DROP NOT NULL`. This is a catalog-only change with no backfill,
  so it runs instantly at any size. It is safe for rolling deploys: v0.5.0
  never writes a NULL hash. It becomes **one-way** once role provisioning
  creates a password-less account, because v0.5.0 cannot read a NULL hash and
  `NOT NULL` cannot be restored while one exists. Rolling back after that is a
  forward migration: delete those accounts or give them passwords first. If
  `FOUNDRY_OIDC_PROVISION_ROLE` stays unset, no NULL row is ever written and a
  rollback to v0.5.0 stays possible.

### Fixed

- **`foundry doctor backup-verify` fails closed.** It used to print
  `status: OK` and exit 0 when it could not count a single row: its row
  counts shelled out to a bare `psql`, and every failure (no `psql` on PATH,
  an unreachable probe, a dump of the wrong database) was swallowed as "table
  not present". Row counts and the probe-schema `DROP` now run in-binary over
  sqlx with quoted identifiers, so `pg_restore` is the only external tool.
  Only a genuinely absent table is skipped. **Runs that used to report a false
  OK now exit non-zero:** 8 when the probe database is unreachable for row
  counts, 9 when counting a table that exists fails, 10 when the restored
  schema holds none of the known Foundry tables ("not a Foundry backup?").
  CI no longer installs `postgresql-client-16`.

## [v0.5.0] - 2026-09-26

The release footer and the first slice of Pointer Events card drag. The
board-lane-reorder finalize and the card-pointer-drag DISCUSS/DESIGN/DISTILL
commits in this range are docs and pending tests only; `git log
v0.4.0..v0.5.0` is the full list.

### Added

- **Release version in the page footer.** Every full page, signed in or out,
  now ends with a small muted `Foundry vX.Y.Z` footer, so an operator can see
  which release is serving without shell access. The version is
  `foundry-app`'s `CARGO_PKG_VERSION`, exposed as `views::RELEASE_VERSION` and
  read from `base.html`; htmx fragments never extend the base template and
  carry no footer. The footer sits in the app shell's bottom padding, so no
  authed page (the board included) gains a scrollbar. Stylesheet content-hash
  rotated `438142d2` → `f9143163`.

### Changed

- **Card drag runs on Pointer Events for the mouse.** `board-dnd.js` moves its
  event layer from HTML5 drag-and-drop to Pointer Events
  (ADR-BOARD-CARD-004): a primary-button press on a card lifts past 6 px, the
  card stays in its slot while a fixed clone ghost follows the pointer, and
  release over a lane lands at the live marker with the same POST as before.
  Release off every lane, Escape (through `keyboard.js::closeTopLayer()`), or
  `pointercancel` after the lift ends the drag cancelled. One gesture means one
  thing: a lifted release never also opens the card dialog, only the primary
  button starts a drag, and a card drag never moves a lane (nor a header drag a
  card). Desktop files dragged onto the board are still swallowed. Touch and
  pen come in a later release (card-pointer-drag slices 02–03). Stylesheet
  content-hash rotated `f7c36a08` → `438142d2`.

### Changed (internal)

- **`check-arch` forbids keydown listeners in `board-*.js`.** Escape has one
  owner, `keyboard.js::closeTopLayer()` (BR-4); the new rule flags any
  `addEventListener("keydown"`, `.onkeydown =` or `on("keydown"` registration
  in a board script (comments stripped, a missing js directory fails closed).

## [v0.4.0] - 2026-09-19

First tagged release since v0.3.1: everything on `main` since then, including
the card drag-and-drop landing indicator, the issue-card delete fixes, native
OIDC sign-in, and the rustls bump for RUSTSEC-2026-0285. The entries below
are the ones recorded at the time; `git log v0.3.1..v0.4.0` is the full list.

### Added

- **Close (×) control on the issue edit dialog.** The edit dialog's header now
  carries a conventional close button (accessible name "Close", ≥24×24 px
  target, visible focus ring) so a pointer user can leave without saving —
  previously the only no-save exit was the unadvertised Esc key. One close
  mechanism, two triggers: the button is a declarative
  `data-action="close-modal"` trigger resolved by a single document-delegated
  click listener calling the existing `closeModal()`; Esc handling is
  unchanged (single-owner, BR-4). Dismissal issues no save request; typed
  edits are discarded, exactly as Esc does. Stylesheet content-hash rotated
  `7c858984` → `8ce38566`. See
  `docs/product/architecture/adr-modal-close-001-declarative-close-trigger.md`.

## [v0.3.1] - 2026-05-30

Test-suite and release-pipeline hardening. **No functional/runtime changes** —
the binary is behaviorally identical to v0.3.0; all six crates bump `0.3.0` ->
`0.3.1` for the release marker.

### Changed (internal)

- **Acceptance `@all` lane flake eliminated.** The shared Postgres testcontainer
  serves no TLS, but the connection URLs set no `sslmode`, so sqlx's default
  `prefer` SSL probe intermittently failed under the concurrent connect-storm
  (`SSLRequest: 0x00`) and starved the harness pool → `PoolTimedOut` on Background
  seed inserts. Disabled the probe (`ssl_mode=Disable`) and raised the pool
  `acquire_timeout` 5s -> 30s. Validated 5/5 green and a controlled A/B (reverted
  3/5 flaked vs fixed 0/5). Test-infrastructure only.
- **Cargo dependency-graph SBOM** (CycloneDX, 513 crates) now checked in at
  `sbom/crates.cdx.json` (deterministic via `sbom/generate.sh`) and attached to
  release images as a second cosign attestation alongside the image SBOM.

### Tests

- Added `foundry-store`'s first integration test: a cross-schema regression guard
  for `Store::probe()`'s `current_schema()` scoping (a sibling schema's columns
  must not mask a half-migrated active schema). Closes the gap that the
  string-literal fix couldn't be mutation-tested.

## [v0.3.0] - 2026-05-29

Slice 8 — the deferred observability metrics — plus a startup-probe
correctness fix and mutation-test hardening of the slice-8 coverage. All six
crates `0.2.0` -> `0.3.0`.

### Added

- **Slice 8 — deferred observability metrics.** Emits and dashboards the five
  catalog metrics that slice 6 reserved but left unproduced, so every
  "Foundry Overview" panel resolves to real data instead of "no data":
  - `outbox_pending_jobs` and `bootstrap_tokens_unclaimed` gauges, folded into
    the existing 5s pool-poll loop (no new task). (ADR-018)
  - `migration_apply_duration_seconds{migration_id}` histogram — one timing
    observation per migration that actually applies. (ADR-020)
  - `realtime_listen_disconnects_total` and `probe_failures_total{probe_name}`
    counters, incremented at their event call-sites (LISTEN reconnect; the
    `store`/`metrics` startup probes). (ADR-019)
  - Five new Grafana "Foundry Overview" panels; both labelled metrics carry
    bounded label sets.

### Fixed

- **`Store::probe()` scopes its migration-0006 column check to
  `current_schema()`.** It previously counted `comments` columns across every
  visible schema, so a half-migrated active schema could pass the startup
  probe whenever a sibling schema still carried the columns. No behaviour
  change in single-schema production deployments.

### Tests

- Feature-scoped mutation testing (cargo-mutants) of the slice-8 store/emit
  code closed three assertion gaps — the `migration_id` label *value*, and the
  `probe_failures_total` increment path — reaching a 100% kill rate on viable
  mutants. See `docs/feature/slice-8-deferred-metrics/deliver/mutation/`.

## [v0.2.0] - 2026-05-28

Initial public release of the Foundry MVP — a self-hostable, single-binary
issue tracker that `docker compose up`'s on a fresh machine. Bundles slices
1–7 plus the platform/DevOps slice and the observability hardening that
followed it.

### Added

- **Slice 1 — Backend MVP.** Operator install, admin bootstrap (signed
  token), user sign-in (argon2id, server-validated sessions, brute-force
  delay), project create, and issue file. Walking skeleton: fresh machine to
  filed issue in under an hour.
- **Slice 2 — Realtime collaboration.** US-09 realtime issue updates over
  SSE, US-10 markdown comments with sanitization, US-12 keyboard-driven
  navigation contracts.
- **DevOps / platform readiness.** GitHub Actions CI (4 parallel gates),
  tag-driven multi-arch (amd64 + arm64) container release with cosign keyless
  signing + SPDX SBOM, plain-YAML Kubernetes manifests, and an opt-in
  Prometheus/Loki/Promtail/Grafana observability overlay with a starter
  "Foundry Overview" dashboard.
- **Slice 3 — Operator-grade hardening.** US-02 multi-replica fan-out with
  shared sessions, US-03 backup/restore (`foundry doctor backup-verify`),
  US-04 rolling upgrade, US-11 attachments.
- **Slice 4 — Contributor onboarding (US-13).** README Quickstart pins a
  five-command `git clone` → green `cargo test` path with no Redis, no S3, no
  Node toolchain, and a clear too-old-Rust error.
- **Slice 5 — Comment edit/delete (US-10).** Authors edit/delete their own
  comments; workspace admins delete any; "edited" indicators and realtime
  disappearance; soft-delete tombstone preserves the moderation audit trail.
- **Slice 6 — Handler instrumentation.** A tower-middleware layer emits the
  5 metric series the Grafana dashboard references: `http_requests_total`,
  `http_request_duration_seconds`, `db_connections_in_use`, and the realtime
  subscriber gauge — register-at-0 so panels light up immediately.
- **Slice 7 — Comment tombstone GC + admin-undelete.** A daily background
  sweep hard-deletes comments tombstoned >90 days (advisory-lock-coordinated,
  per-run capped), emitting `comments_tombstones_purged_total` +
  `comments_tombstones_pending`. Operators recover in-window deletions with
  `foundry doctor restore-comment <UUID>`.

### Performance

- **argon2id password hashing runs off the async runtime.** `hash_password` /
  `verify_password` run their OWASP-grade CPU work on a blocking thread
  (`tokio::task::spawn_blocking`) so hashing never pins an async worker;
  sign-in stays responsive under concurrent load.

### Known issues / deferred

- Five metric series (`outbox_pending_jobs`, `bootstrap_tokens_unclaimed`,
  `migration_apply_duration_seconds`, `realtime_listen_disconnects_total`,
  `probe_failures_total`) are defined but not yet emitted; each needs a
  dashboard consumer first.
- Helm packaging is deferred to v0.4 (ADR-102); plain-YAML manifests ship
  today.
- A `comments_visible` SQL VIEW for defense-in-depth against missed
  soft-delete filters is deferred to v0.3 (ADR-017).

[Unreleased]: https://github.com/Canzan/foundry/compare/v0.17.0...HEAD
[v0.17.0]: https://github.com/Canzan/foundry/compare/v0.16.0...v0.17.0
[v0.16.0]: https://github.com/Canzan/foundry/compare/v0.15.1...v0.16.0
[v0.15.1]: https://github.com/Canzan/foundry/compare/v0.15.0...v0.15.1
[v0.15.0]: https://github.com/Canzan/foundry/compare/v0.14.2...v0.15.0
[v0.14.2]: https://github.com/Canzan/foundry/compare/v0.14.1...v0.14.2
[v0.14.1]: https://github.com/Canzan/foundry/compare/v0.14.0...v0.14.1
[v0.14.0]: https://github.com/Canzan/foundry/compare/v0.13.0...v0.14.0
[v0.13.0]: https://github.com/Canzan/foundry/compare/v0.12.0...v0.13.0
[v0.12.0]: https://github.com/Canzan/foundry/compare/v0.11.0...v0.12.0
[v0.11.0]: https://github.com/Canzan/foundry/compare/v0.10.0...v0.11.0
[v0.10.0]: https://github.com/Canzan/foundry/compare/v0.9.0...v0.10.0
[v0.9.0]: https://github.com/Canzan/foundry/compare/v0.8.0...v0.9.0
[v0.8.0]: https://github.com/Canzan/foundry/compare/v0.7.0...v0.8.0
[v0.7.0]: https://github.com/Canzan/foundry/compare/v0.6.2...v0.7.0
[v0.6.2]: https://github.com/Canzan/foundry/compare/v0.6.1...v0.6.2
[v0.6.1]: https://github.com/Canzan/foundry/compare/v0.6.0...v0.6.1
[v0.6.0]: https://github.com/Canzan/foundry/compare/v0.5.0...v0.6.0
[v0.5.0]: https://github.com/Canzan/foundry/compare/v0.4.0...v0.5.0
[v0.4.0]: https://github.com/Canzan/foundry/compare/v0.3.1...v0.4.0
[v0.2.0]: https://github.com/Canzan/foundry/releases/tag/v0.2.0
