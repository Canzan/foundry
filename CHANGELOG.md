# Changelog

All notable changes to Foundry are documented here. The format follows
[keep-a-changelog](https://keepachangelog.com/en/1.1.0/) and the project
follows [Semantic Versioning](https://semver.org). Pre-1.0 releases permit
minor-version breaking changes, flagged with a `BREAKING` heading.

## [Unreleased]

The database now enforces the workspace-name and project-name rules on every
new name write (name-db-checks).

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

[Unreleased]: https://github.com/Canzan/foundry/compare/v0.11.0...HEAD
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
