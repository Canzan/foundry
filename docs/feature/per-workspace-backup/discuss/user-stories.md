<!-- markdownlint-disable MD024 -->

# User Stories: per-workspace-backup

> Persona: **Devansh**, the self-hosting operator (the persona behind `foundry doctor backup-verify`).
> Surface: `foundry doctor` CLI. Scope: EXPORT ONLY (per-workspace restore deferred — DD-MWT-09).
>
> **job_id note**: no `docs/product/jobs.yaml` exists in this repo. OD-5 is a clearly-stated,
> DIVERGE-equivalent validated job ("export one workspace's data so I can archive / migrate /
> recover just that tenant"), ratified as DD-MWT-09. Each story below carries
> `job_id: od-5-per-workspace-export` referencing that decision. DESIGN/orchestrator may register it
> in a jobs registry if one is later created.

## System Constraints

- Operator CLI only (`foundry doctor` subcommands), OFF the bearer API; mirrors the shipped
  `admin_cli.rs` scaffold (structured `key: value` stdout, terminating `status:` line, structured
  exit codes 0/2/3/4/5, thread-isolated tokio runtime, live DB via `DATABASE_URL` + `Store`/`Services`).
- Isolation-scoped logical export over the 10 TENANT_TABLES; "belongs to workspace" = the SHIPPED
  scope definition (direct `workspace_id` or transitive team/project/issue FK).
- The export selection predicate and the verify isolation predicate are the SAME definition.
- Read-only: an export never mutates the source instance.

---

## US-PWB-01: Export one workspace's data to a portable archive

- **job_id**: od-5-per-workspace-export
- **Release**: Walking Skeleton (R1) | **MoSCoW**: Must | **Est**: 2-3 days | **Scenarios**: 5

### Elevator Pitch

- **Before**: Devansh can only `pg_dump` the whole instance; to give Globex its own data or archive
  one churned tenant he would have to hand-edit a multi-tenant dump — there is no safe way.
- **After**: Devansh runs `foundry doctor export-workspace globex /backups/globex-2026-06-16.dump`
  and sees a per-table row-count report ending `status: OK`, with an archive written at that path.
- **Decision enabled**: he decides the archive is the right tenant and ready to hand off / store —
  because the printed counts and slug match the workspace he intended.

### Problem

Devansh is a self-hosting operator who runs a multi-tenant Foundry instance. He finds it impossible
to lift a single tenant's data out: the whole-instance `pg_dump` mixes Acme, Globex, and every other
workspace together, so archiving one churned customer or handing a departing workspace its own data
means dangerous manual surgery on a combined dump.

### Who

- Self-hosting operator | runs `foundry doctor` on the host (shell access ⇒ host trust) | motivated to
  archive / migrate / hand off / pre-deletion-snapshot exactly one tenant without touching the others.

### Solution

Two new `foundry doctor` subcommands: `list-workspaces` (shows id + slug + name so the operator can
name the target) and `export-workspace <id|slug> <out-path>` (writes an isolation-scoped logical
archive of that one workspace across the 10 tenant tables, printing per-table row counts and ending
`status: OK`). Mirrors the shipped privileged-CLI scaffold.

### Domain Examples

#### 1: Happy Path — Globex export

Devansh runs `foundry doctor list-workspaces`, sees `Globex LLC` with slug `globex`, then runs
`foundry doctor export-workspace globex /backups/globex-2026-06-16.dump`. Output reports `users: 7`,
`issues: 412`, `comments: 1893`, all 10 tables, ending `status: OK`. The archive exists at the path.

#### 2: Select by id instead of slug — Acme export

Devansh prefers ids in scripts; he runs
`foundry doctor export-workspace 0190a1b2-...-acme /backups/acme.dump`. The id resolves to Acme Corp;
the archive holds Acme's 12 members, 8 projects. Same `status: OK` report.

#### 3: Single-tenant install — sole workspace export

Devansh runs Foundry for one team only. He runs `foundry doctor export-workspace acme /backups/acme.dump`
as a pre-migration snapshot. The export succeeds, notes "this is the only workspace on the instance",
and Acme's data remains fully intact on the instance (export read nothing destructively).

### UAT Scenarios (BDD)

#### Scenario: Operator sees every workspace's identity before exporting

```gherkin
Given an instance with workspaces "Acme Corp" (slug "acme") and "Globex LLC" (slug "globex")
When Devansh runs "foundry doctor list-workspaces"
Then the output lists both workspaces with their id, slug, and name
And the output ends with "status: OK"
```

#### Scenario: Operator exports exactly one tenant's data by slug

```gherkin
Given "Globex LLC" has 7 members, 3 teams, 8 projects, 412 issues, and 1893 comments
When Devansh runs "foundry doctor export-workspace globex /backups/globex-2026-06-16.dump"
Then an archive is written to "/backups/globex-2026-06-16.dump"
And the output reports a per-table row count for all 10 tenant tables
And the output ends with "status: OK"
```

#### Scenario: Operator exports a workspace by id

```gherkin
Given Acme Corp's workspace id is "0190a1b2-...-acme"
When Devansh runs "foundry doctor export-workspace 0190a1b2-...-acme /backups/acme.dump"
Then the id resolves to Acme Corp
And an archive of Acme Corp is written to "/backups/acme.dump"
And the output ends with "status: OK"
```

#### Scenario: Exporting the only workspace is valid and removes nothing

```gherkin
Given a single-tenant instance whose only workspace is "Acme Corp"
When Devansh runs "foundry doctor export-workspace acme /backups/acme.dump"
Then an archive is written
And the output notes that this is the only workspace on the instance
And "Acme Corp" and all its data still exist on the instance unchanged
```

#### Scenario: The export reports a clear error when the database is unreachable

```gherkin
Given DATABASE_URL points at an unreachable database
When Devansh runs "foundry doctor export-workspace globex /backups/globex.dump"
Then the command exits with code 3
And the message says it could not connect to the database
```

### Acceptance Criteria

- [ ] `list-workspaces` lists every workspace with id + slug + name, ending `status: OK`.
- [ ] `export-workspace` accepts the target by id OR by slug.
- [ ] A successful export writes an archive at the given path and reports a per-table row count for
      all 10 tenant tables.
- [ ] Output ends with a terminating `status: OK` line (greppable in cron, like backup-verify).
- [ ] Exporting the sole workspace succeeds and mutates nothing on the instance.
- [ ] An unreachable database yields exit code 3 with an actionable message.

### Outcome KPIs

- **Who**: self-hosting operators of multi-tenant Foundry instances.
- **Does what**: extract a single tenant's full data set end-to-end via one command.
- **By how much**: from impossible-without-manual-surgery (baseline 0% achievable safely) to a
  one-command export demonstrable in a single session.
- **Measured by**: acceptance scenarios green + operator can demo list -> export in one session.
- **Baseline**: no per-workspace export exists today.

### Technical Notes

- Reuse the shipped `foundry doctor` scaffold (`admin_cli.rs`): thread-isolated tokio runtime, live DB
  via `DATABASE_URL`, `Store`/`Services` seam, structured exit codes, `key: value` + `status:` stdout.
- id-or-slug resolution must be ONE function feeding the archive header (shared-artifacts-registry:
  `workspace_identity`, HIGH risk).
- Depends on the shipped scoping seam + TENANT_TABLES (US-MWT slices 1-5, all shipped).

---

## US-PWB-02: Prove the export contains only this tenant's data (isolation + verification)

- **job_id**: od-5-per-workspace-export
- **Release**: Walking Skeleton (R1) | **MoSCoW**: Must | **Est**: 2-3 days | **Scenarios**: 5
- **Depends on**: US-PWB-01

### Elevator Pitch

- **Before**: even if Devansh exports Globex, he has no way to PROVE the archive holds only Globex's
  rows and no Acme data — so he cannot safely hand it to a departing customer.
- **After**: Devansh runs `foundry doctor verify-export /backups/globex-2026-06-16.dump` and sees
  `every row belongs to the declared workspace: YES` and `no rows reference a sibling workspace: YES`,
  exit code 0.
- **Decision enabled**: he decides it is safe to release the archive externally — the isolation
  guarantee is machine-confirmed, not assumed.

### Problem

Devansh's deepest fear with a per-tenant export is a cross-tenant leak: handing Globex an archive that
secretly contains Acme's issues or members would be a data-breach incident. Today he would have no way
to confirm an export is isolation-clean — he would have to trust it blindly.

### Who

- Self-hosting operator about to hand a workspace's archive to a third party (departing customer, new
  instance) | needs an auditable, machine-checkable guarantee of completeness AND isolation before
  release.

### Solution

The export contains ALL of the target workspace's tenant rows and NONE of any sibling's (the scope
predicate selects directly via `workspace_id` and transitively via team/project/issue FK chains). A
`verify-export <path>` subcommand reads the archive's declared-workspace header and confirms (a) all
10 tenant tables are present (completeness) and (b) every row resolves to the declared workspace with
zero sibling rows (isolation). The selection predicate and the isolation predicate are the SAME
definition.

### Domain Examples

#### 1: Happy Path — clean Globex archive verifies

After exporting Globex, Devansh runs `verify-export` on it. Report: all 10 tables present; every row
belongs to Globex; no sibling rows; exit 0. Devansh hands the archive to the departing Globex admin.

#### 2: Edge / transitive scope — a Globex comment on a Globex issue

Globex's `comments` reach `workspace_id` only transitively (comment -> issue -> workspace). verify
resolves each comment's owning workspace through the FK chain and confirms it is Globex — proving the
transitive scope, not just the direct-`workspace_id` tables, is isolation-checked.

#### 3: Error / the crux bites — a planted Acme row

A buggy export wrongly includes one Acme issue in the Globex archive. Devansh runs `verify-export`;
the isolation check fails, the command exits non-zero, and the message says a row resolves to a
workspace other than the declared one. The leak is caught before release.

### UAT Scenarios (BDD)

#### Scenario: The archive contains only the target workspace's data

```gherkin
Given "Acme Corp" and "Globex LLC" each have their own teams, projects, issues, and comments
When Devansh exports "globex"
Then every row in the archive belongs to the Globex workspace
And no row in the archive belongs to the Acme workspace
And the archive's member set is exactly Globex's members, not any Acme member
```

#### Scenario: Operator confirms an export is complete and isolation-clean

```gherkin
Given a freshly exported archive of the Globex workspace at "/backups/globex-2026-06-16.dump"
When Devansh runs "foundry doctor verify-export /backups/globex-2026-06-16.dump"
Then the report confirms all 10 tenant tables are present
And the report confirms every row belongs to the declared Globex workspace
And the report confirms no row references a sibling workspace
And the command exits with code 0
```

#### Scenario: Transitively-scoped rows are isolation-checked too

```gherkin
Given Globex comments reach their workspace only through issue and project foreign keys
When Devansh verifies a Globex export
Then each comment is resolved to its owning workspace through the foreign-key chain
And every comment is confirmed to belong to Globex
```

#### Scenario: Verification fails loudly if a sibling row leaked into an archive

```gherkin
Given an archive that wrongly contains one row belonging to the Acme workspace
When Devansh runs "foundry doctor verify-export" on that archive
Then the isolation check fails
And the command exits with a non-zero code
And the message identifies that a row resolves to a workspace other than the declared one
```

#### Scenario: An export of any single workspace contains no sibling data

```gherkin
@property
Given an instance with two or more workspaces each holding tenant data
When any one workspace is exported and then verified
Then the verification confirms zero rows resolve to any other workspace
```

### Acceptance Criteria

- [ ] An export of workspace W contains every tenant row belonging to W across all 10 tables.
- [ ] An export of workspace W contains NO row belonging to any sibling workspace (the crux).
- [ ] Isolation is checked for transitively-scoped tables (comments, team_memberships) via the FK
      chain, not only for direct-`workspace_id` tables.
- [ ] `verify-export <path>` confirms completeness (all 10 tables present) using only the archive path.
- [ ] `verify-export` confirms isolation (every row resolves to the declared workspace, zero siblings)
      and exits 0 on a clean archive.
- [ ] A planted sibling row makes `verify-export` exit non-zero with a message identifying the foreign
      row (falsifiability — the proof bites).

### Outcome KPIs

- **Who**: operators releasing a workspace archive externally.
- **Does what**: machine-confirm an export is complete and isolation-clean before release.
- **By how much**: 100% of exports verifiable without manual inspection; 0 cross-tenant leaks pass
  verification (a planted leak always reds).
- **Measured by**: verify-export exit code + the falsifiability test in the acceptance suite.
- **Baseline**: no verification of per-tenant isolation exists today.

### Technical Notes

- The selection predicate (US-PWB-01 export) and the isolation predicate (verify) MUST be one
  definition (shared-artifacts-registry: `workspace_scope_predicate`, HIGH).
- Falsifiability mirrors the shipped slice-05 discipline (plant a row, assert the proof reds).
- **OPEN (OD-PWB-1)**: the `users` rule under multi-membership — a user may belong to Acme AND Globex.
  RECOMMENDED: export the `workspace_memberships` edges for W + the `users` rows that are members of W
  (so the archive is self-contained for W) and let DESIGN ratify whether a multi-membership user's row
  is considered "sibling data" for the isolation check (recommend: NOT a violation — the user is a
  legitimate member of W; the isolation check applies to workspace-owned resources, not to shared
  user identities).

---

## US-PWB-03: Survive every failure path without surprising or burning the operator

- **job_id**: od-5-per-workspace-export
- **Release**: R2 (failure-path hardening) | **MoSCoW**: Should | **Est**: 2 days | **Scenarios**: 6
- **Depends on**: US-PWB-01, US-PWB-02

### Elevator Pitch

- **Before**: a typo'd workspace name, a full disk, or a single-tenant install could leave Devansh
  with a cryptic failure or — worse — a half-written archive he might mistake for complete.
- **After**: Devansh runs `foundry doctor export-workspace nope /backups/x.dump` and sees
  `no workspace matches "nope" ... run foundry doctor list-workspaces`, exit 2 — and a disk-full
  export leaves no file that verify would ever accept.
- **Decision enabled**: he decides exactly what to fix next (correct the name, free disk, re-run) and
  trusts that anything left on disk is either complete-and-verified or absent.

### Problem

Real operator runs go wrong: Devansh mistypes a slug, the backup disk fills mid-export, or he is on a
single-tenant install. If the tool fails cryptically, leaves a truncated archive that looks complete,
or refuses a valid sole-workspace export, the feature's trust collapses — a partial archive that
passes for complete is worse than an obvious failure.

### Who

- Self-hosting operator under real conditions: fat-fingered arguments, constrained disks, cron jobs
  that branch on exit codes | needs documented codes, actionable messages, and atomic writes.

### Solution

Every failure path exits with a documented code and an actionable message, mirroring the shipped
scaffold's exit-code discipline: unknown workspace -> 2 (+ redirect to list-workspaces); output-path
unwritable / parent missing -> 5, failing BEFORE any DB read; disk-full / killed mid-write -> atomic
`.partial` -> rename so no complete-looking file survives; truncated archive -> verify exits 4;
sole/last workspace -> valid `status: OK` with a note (export is read-only, never a delete). On
success the CLI prints a one-line at-rest sensitivity note (the archive holds password hashes +
token rows).

### Domain Examples

#### 1: Happy Path (of failure handling) — typo'd slug

Devansh types `globx` instead of `globex`. Output: `no workspace matches "globx" (looked up by id and
by slug). Run foundry doctor list-workspaces`. Exit 2. No archive created. He re-runs with the right
slug.

#### 2: Edge — disk fills mid-export

Globex is large; the backup disk fills at 70%. The export was writing to
`/backups/globex.dump.partial`; on failure it is discarded and no `/backups/globex.dump` exists. A
later `verify-export /backups/globex.dump` finds nothing to accept. Devansh frees space and re-runs.

#### 3: Error / boundary — single-tenant install + sensitivity note

Devansh exports the sole workspace `acme`. It succeeds with a note "this is the only workspace on the
instance", and prints "this export contains password hashes and machine-token rows; treat as sensitive
at rest". Acme's data is untouched on the instance.

### UAT Scenarios (BDD)

#### Scenario: Exporting an unknown workspace is refused with guidance

```gherkin
Given no workspace has the id or slug "nope"
When Devansh runs "foundry doctor export-workspace nope /backups/x.dump"
Then the command exits with code 2
And the message tells Devansh to run "foundry doctor list-workspaces"
And no archive file is created
```

#### Scenario: A failed export never leaves a half-written archive

```gherkin
Given the output path "/nope/x.dump" has a parent directory that does not exist
When Devansh runs "foundry doctor export-workspace globex /nope/x.dump"
Then the command exits with code 5
And no file exists at "/nope/x.dump"
And the failure happened before any tenant data was read
```

#### Scenario: A disk-full export leaves no complete-looking archive

```gherkin
Given the backup disk fills while exporting "globex"
When the export fails mid-write
Then no file exists at the final output path
And only a discardable partial file may remain
And a later verify-export on the final path finds no archive to accept
```

#### Scenario: Exporting the only workspace is valid and removes nothing

```gherkin
Given a single-tenant instance whose only workspace is "Acme Corp"
When Devansh runs "foundry doctor export-workspace acme /backups/acme.dump"
Then an archive is written
And the output notes that this is the only workspace on the instance
And "Acme Corp" and all its data still exist on the instance unchanged
```

#### Scenario: Verification detects an incomplete archive

```gherkin
Given an archive that was truncated when the disk filled mid-export
When Devansh runs "foundry doctor verify-export" on the truncated archive
Then the command exits with code 4
And the message says the archive is truncated or incomplete and to re-run the export
```

#### Scenario: The operator is warned about sensitive at-rest contents

```gherkin
Given Devansh successfully exports "globex"
When the export completes
Then the output prints a note that the archive contains password hashes and machine-token rows
And the note advises treating the archive as sensitive at rest
```

### Acceptance Criteria

- [ ] Unknown workspace (id or slug matches nothing) exits 2 and redirects to `list-workspaces`; no
      archive created.
- [ ] Output-path errors (parent missing / unwritable) exit 5 and fail before any DB read.
- [ ] Export writes atomically (`.partial` -> rename); a failed/killed export leaves no file at the
      final path that could be mistaken for complete.
- [ ] Exporting the sole workspace succeeds with an "only workspace" note and mutates nothing.
- [ ] `verify-export` exits 4 on a truncated/incomplete archive with an actionable message.
- [ ] A successful export prints an at-rest sensitivity note (password hashes + token rows).

### Outcome KPIs

- **Who**: operators hitting real-world failure conditions.
- **Does what**: recover from a failed export using the printed code + message without guesswork, and
  never act on a partial archive.
- **By how much**: 100% of documented failure paths exit with the specified code + actionable message;
  0 partial archives pass `verify-export`.
- **Measured by**: each failure-path acceptance scenario asserts its exact exit code; the atomicity
  test asserts no file survives a killed export.
- **Baseline**: n/a (no export exists today).

### Technical Notes

- Exit-code contract mirrors `admin_cli.rs` (0/2/3/4/5). Operators grep `status:` + branch on code in
  cron exactly as for backup-verify (shared-artifacts-registry: `exit_code_contract`).
- Atomic write = `<out>.partial` + fsync + rename (NFR-PWB-ATOM-01).
- The sensitivity note is the operator-trust disclosure (NFR-PWB-SEC-01) — same at-rest posture as the
  whole-instance dump.
