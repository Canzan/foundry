# Evolution: project-lifecycle (rename and delete a project from its team, restore it from the operator CLI, and retire a team whose projects are all deleted)

**Finalized**: 2026-10-15
**Commits**:
- DISCUSS: `51df491`, user answers to OQ-1..OQ-6 `4f20033`.
- DESIGN: `1c91941`, OQ-U1/OQ-U2 `3b8f577`, iteration 3 (aligned with v0.14.1) `cbd3bfc`.
- DEVOPS: `91fb6f0`.
- DISTILL: `5c98860` (59 pending scenarios, 298 rows, gold tests, RED scaffolds).
- DELIVER roadmap: `592429e` (17 steps, approved after two reviews).
- DELIVER steps: `9635d61` (01-01) → `b0eb8e7` (04-03), one commit per step (table below).
- Upstream issues: `8676699`. Refactor (L1-L4): `beb5127`. Review dispositions: `48013c5`.
  Review fixes: `882baf6` (check-arch evasions, F-04..F-07, F-17, F-19, F-21) and `4cc3000`
  (test gaps TG-1..TG-8, F-20).
- Mutation: report `f5b3213`, gaps closed `a6f7971`, write-guard `concat!` fix `263646b`.

`fe237d4` and `62d9b75` sit inside the same range but are not this feature. They are the v0.14.2
dependency-advisory patch (ammonia, rustls), carried onto `main` after `cargo deny` started
failing during 02-05.

Gates:
- DES integrity: all 17 steps have complete DES traces (RED, GREEN, COMMIT each).
- End-of-DISTILL review: four reviewers, three iterations, all approved.
- Phase 4 adversarial review: findings fixed in `882baf6` and `4cc3000`, or dispositioned in
  `upstream-issues.md` UI-3.
- Mutation: every file at or above 80% on the raw figure; overall 94.9% (562 caught, 30 missed,
  all 30 equivalent or unreachable). `check_arch.rs` went from 76.2% to 93.4% after the gap
  tests. The write-guard `concat!` fix scored 8/8. All five SQL re-pins are killed.

Feature dir PRESERVED. **Not released.** `main` was pushed at `4cc3000`; the later commits are
local. No tag (see Release).

Sources:
- `docs/feature/project-lifecycle/feature-delta.md` (DISCUSS, DESIGN, DEVOPS, DISTILL, and the
  DELIVER closing notes)
- `docs/feature/project-lifecycle/deliver/roadmap.json` and `execution-log.json`
- `docs/feature/project-lifecycle/deliver/upstream-issues.md` (UI-1..UI-3)
- `docs/feature/project-lifecycle/deliver/mutation/mutation-report.md`
- `docs/feature/project-lifecycle/distill/red-classification.md`
- `docs/feature/project-lifecycle/slices/slice-0{1..4}-*.md`

**Scope**: a team's lead or a workspace admin can rename a project and delete it from the team's
members page. Rename changes the name only: the slug, the key and every link stay. Delete is a
soft delete. The project disappears for everyone and on every door, and nothing under it is
erased. The instance operator lists and restores deleted projects with `foundry doctor`. A team
whose projects are all deleted can now be deleted; its deleted projects are kept, detached, and
can no longer be restored. Three migrations (0025, 0026, 0027). No new crate and no new
dependency.

## Business context

Before this feature only an instance super-admin could rename a project, from
`/admin/instance/workspaces`, and nothing could delete one. A project lived until psql removed
it. Because team-lifecycle refuses to delete a team that holds a project, every team that ever
held one was undeletable (team-lifecycle OQ-4, TL-KPI-5). Now the team manages its own project
list, a delete never loses work, and a team whose work is all retired can be retired.

This **closes** team-lifecycle's OQ-4 (b) as far as soft-deleted projects go: only live projects
block a team delete. Moving a project between teams and a cascade delete stay out of scope (D-C).

## What shipped (17/17 steps, execution-log.json)

| Step | What landed | Commit |
|---|---|---|
| 01-01 | Migration 0025 (`project_lifecycle_events`), its probe line and `UPGRADE_WARNING_0025` | `9635d61` |
| 01-02 | `rename_project_with_audit`, `list_team_projects`, check-arch `project-lifecycle-one-seam` | `3cedd35` |
| 01-03 | One rename core behind two gates; the instance door now records; the members page's Projects section | `747981f` |
| 01-04 | `POST /team/{t}/project/{p}/rename` from the members page | `97aa450` |
| 02-01 | Migration 0026: the deleted marker, the `live_projects` view, probe lines, `UPGRADE_WARNING_0026` | `33bd59a` |
| 02-02 | `lock_live_project` in every project-scoped writer; check-arch `project-lifecycle-write-guard` | `f10c592` |
| 02-03 | Reader sweep onto `live_projects`; the export `projects` query moved into the module | `e9ebb5a` |
| 02-04 | check-arch `project-lifecycle-live-reads`, turned on last | `17a1e30` |
| 02-05 | `soft_delete_project` and the delete use-cases; the 100-run delete-versus-issue-create race | `cdd813d` |
| 02-06 | The delete confirm page (`GET …/project/{p}/delete`) | `71cb2c1` |
| 02-07 | The delete POST: notice and the refusal matrix | `50ef181` |
| 02-08 | A deleted project answers nowhere and accepts no write (the differential outlines) | `498210f` |
| 03-01 | `foundry doctor list-deleted-projects` and `restore-project <uuid>` | `11586b1` |
| 03-02 | Names held until restore; `restore-comment` names the deleted project | `fdcb3a9` |
| 04-01 | Migration 0027: detach columns, `projects_team_or_detached`, view parity | `1ee97e2` |
| 04-02 | Team delete detaches deleted projects; a detached project's restore is team-gone | `8cd17b6` |
| 04-03 | The team delete page names the deleted projects it keeps; team-gone copy pinned | `b0eb8e7` |

## Design and invariants

User decisions that shaped it: D-A (soft delete), D-B (lead or workspace admin), D-C (no move,
purge or cascade), OQ-1 (b) (a deleted project does not block a team delete and is kept,
detached), OQ-2 (slug and key never change), OQ-3 (every rename, delete and restore is
recorded), OQ-4 (a deleted project's name, slug and key stay taken), OQ-5 (no typed
confirmation), OQ-6 (restore is CLI-only), OQ-U1 (b) (a project row links to its board only for
a viewer on the team), OQ-U2 (use-cases keep the shared `&Store`).

DISTILL user decisions (2026-10-13):
- **U1**: the two machine-token API routes (`…/tokens`, `…/tokens/{jti}`) are exempt from
  "answers the API's not-found". They read no project, so they answer alike under every project
  address. No behaviour change; a follow-up is recorded.
- **U2**: base-table read-only SQL is allowed in acceptance for the "nothing erased" and marker
  checks. Hidden-ness is asserted only through HTTP, API and CLI.
- **U3**: "works with scripting disabled" and "queued notifications are delivered" got real
  assertions (plain-form checks; a store gold test for the outbox).
- **U4**: the `restore-comment` refusal wording (DIST-1) was added to the locked copy.

The mechanism:
- **One read seam (DDD-3, ADR-PROJECT-LIFECYCLE-001).** `live_projects` is
  `SELECT * FROM projects WHERE deleted_at IS NULL`. Every reader uses it. Only
  `crates/foundry-store/src/project_lifecycle.rs` names `projects` (plus `INSERT INTO projects`
  and four reviewed table-name labels). A deleted project resolves to `None`, the value an
  unknown slug already gives, so every door answers it as never-existed without new code.
- **Write guard (DDD-7, ADR-PROJECT-LIFECYCLE-002).** Every writer of `issues`, `comments`,
  `issue_attachments`, `lanes` or `issue_change_events` first takes `FOR KEY SHARE` on the live
  project row, inside the transaction that writes. The delete takes `FOR UPDATE`, which
  conflicts. So no write commits after a delete commits, and a racing write gets its normal
  outcome or its shipped not-found. Issue create's guard is its counter `UPDATE live_projects`.
  `rename_lane`, `insert_attachment` and `undelete_comment` became transactional for it.
- **Lock order (DDD-8):** team → project → children on every lifecycle path.
- **Restore (DDD-9):** team `FOR KEY SHARE` → project `FOR UPDATE`, still deleted → clear the
  marker → `restored` record. Clearing the marker is the whole restore, because nothing could
  write the project while it was deleted.
- **Rename (DDD-15, DDD-16):** one private `apply_rename` behind two gates (the instance door and
  the team door). `FOR NO KEY UPDATE`, a byte-equal name writes nothing, the record uses
  `clock_timestamp()`. The duplicate-label race between two *different* projects is an accepted
  residual (DDD-17, the ADR-PROJECT-RENAME-002 posture).
- **Team delete (DDD-20, ADR-PROJECT-LIFECYCLE-004, amends ADR-TEAM-LIFECYCLE-002):** team
  `FOR UPDATE` → any live project refuses → detach every remaining row → `DELETE FROM teams` →
  record. The FK cascade then reaches no project. A live row that slips through is a loud 23514
  (logged once as the detach backstop), never a silent loss.
- **Authority (DDD-18):** read once per request. Every refusal before the verb is the
  byte-identical uniform 404.

Invariants held: INV-PL-1 (nothing erased, with the D12 tombstone GC carve-out), INV-PL-2 (no
door left open), INV-PL-3 (no 500 from a vanishing project), INV-PL-4 (exact restore, same
carve-out), INV-PL-5 (names held), INV-PL-6 (team delete never removes a project row), INV-PL-7
(record commits with its change), INV-PL-8 (authority at request time), INV-PL-9 (rename never
changes slug or key). The rolling-deploy window is a bounded, accepted exception to INV-PL-2, -3,
-4 and -7 (DESIGN Changed Assumption 4).

Changed assumptions carried into the code:
1. A deleted project's live-updates address answers the events route's shipped 403, because that
   is what a never-existed project gets. An already-open stream is not closed by a delete.
2. The Projects row links to the board only when the viewer is on the team.
3. Tombstoned comments that pass their 90-day window while the project is deleted are purged as
   usual, so "exact restore" excludes them.
4. The rolling-deploy window (one replica, surge 1: well under a minute). The CHANGELOG note is
   the mitigation.
5. Attachment upload and download were added to the deleted-project inventory.
6. The machine-token routes are exempt (U1).

Deviation found in DELIVER (UI-1, accepted): the write-guard allow-list has four names, not two.
`create_initial_workspace` and `claim_bootstrap_and_create_workspace` seed a bootstrap project and
its lanes in one transaction, so no guard can run first, the same reason `insert_project` is
exempt. Review F-21 then keyed the allow-list by file and item path, not bare name.

## Migrations

| Migration | Adds | Upgrade warning | Rollback |
|---|---|---|---|
| `0025_project_lifecycle_events` | Append-only record table, shape CHECK, two indexes; no FK to `projects` or `teams` | `UPGRADE_WARNING_0025` (new FKs to `workspaces` and `users`) | Table stays, unused; older binary renames write no record |
| `0026_project_soft_delete` | `projects.deleted_at`, `deleted_by`, CHECK `projects_deleted_marker_shape`, view `live_projects` with a parity self-check | `UPGRADE_WARNING_0026` (the view depends on `projects`) | Older binary shows deleted projects again; nothing lost |
| `0027_project_detach` | `team_id` nullable, `detached_from_team_id` (no FK), CHECK `projects_team_or_detached`, view recreated | none | Older binary refuses more team deletes (stricter, never lossy) |

All three are additive, forward-only and self-checked. They ship together in one release (R1
widened, R2). `Store::probe` refuses readiness without each one's objects, so a bad rollout
stalls instead of serving errors. 0025 and 0026 each log one warning on an upgrade, never on a
fresh install. A whole-instance dump taken before 0026 restores only into a fresh, empty
database; the CHANGELOG says so. Rule for later migrations that add a `projects` column: recreate
`live_projects` in the same migration (the parity self-check and the probe enforce it).

## Guards (check-arch)

- **`project-lifecycle-live-reads`**: no SQL literal outside the module names `projects`, in any
  form (quoted, `public.`, comma join, line break, sub-select). An exact `"projects"` literal is
  also flagged, except four named table-name labels. Review F-07 made a verbless fragment with
  `FROM`/`JOIN projects` count as SQL, and reads `concat!`/`format!` pieces joined.
- **`project-lifecycle-write-guard`**: every store fn that writes a guarded table, directly or
  through a transaction-taking helper, calls `lock_live_project(` on the same transaction before
  its first write. Review F-04/F-06 require the guard's result to actually gate the write; F-05
  treats a `const`/`static` SQL literal as a write in every fn naming it. `263646b` closed the last
  false negative found by mutation: DML split across `concat!` pieces.
- **`project-lifecycle-one-seam`**: writes on `projects`, `live_projects` and
  `project_lifecycle_events` only in the store; `rename_project_with_audit`, `soft_delete_project`
  and `restore_deleted_project` have no call site in the app or API.

All three run beside v0.14.1's `store-natural-key-scoped`. Each clause has injected-violation
gold tests; `cargo test -p xtask` is at 178.

## Lanes on the final code

- **Default acceptance lane:** 1379/1379 after the review fixes. Failures seen on the way were
  the known connection flakes (sqlx `'\0'`, PortNotExposed); each was rerun alone and was green.
- **`project-lifecycle` tag:** 299 rows (DISTILL's 298 plus TG-8's "no deleted projects, no
  'also has' line" scenario), all green.
- **PL-KPI-3 differential:** 30 web doors and 6 project-reading API doors answer a deleted
  project exactly as a never-existed one, each row with a live control (scenario 29, 87 rows;
  scenario 30, 120 rows over four outsider actors).
- **Learning gate:** the 100-run delete-versus-issue-create race held in 02-05 (no lost issue,
  no 500).
- **`cargo xtask smoke`:** green on rerun. The first run hit PortNotExposed in Docker.
- **`cargo xtask check-arch`:** PASSED, with `project-lifecycle-one-seam`, `-write-guard` and
  `-live-reads` registered.
- **Mutation:** see Gates.

## Lessons

1. **Hand-pinned mutations in each step found weak oracles.** Each was closed later by review
   or mutation:
   - the lane writer fixtures used the slug `"todo"`, which the seeded lanes lack, so a lane
     not-found held even without a guard (TG-1);
   - the double-restore test only met at a barrier and never overlapped (TG-2, now held by
     `pg_stat_activity`);
   - a probe that ignored one record column passed (TG-7);
   - the deleted-projects list had no id tiebreak test (TG-4);
   - the "also has" line rendered for k = 0 with nothing to catch it (TG-8).

   A step's own pins are the cheapest place to find these. Record each survivor as a carry-over,
   not as a pass.
2. **"Waits" claims need proof of waiting.** Sleeps and barriers made race tests pass whether or
   not a lock was taken. Polling `pg_stat_activity` for a Lock wait blocked by the holder's pid
   (TG-5, TG-6) makes the claim checkable.
3. **A lexical rule needs mutation as much as product code.** `check_arch.rs` started at 76.2%.
   The 81 real gaps were mostly in the scanner (nested comments, raw strings, char literals) and
   in how a guard gates a write. Review found four evasions (F-04..F-07) and mutation found one
   more (`concat!`-split DML).
4. **Disk is a resource of the lane.** `target/` grew to 334 GB (148 GB in `debug/incremental`).
   ENOSPC during 02-03 wedged Docker Desktop, which needed a restart. Check `df` before long
   lanes and mutation runs.
5. **Connection errors are not kills.** canzan-lift acceptance held ports in the 55xxx range,
   which caused SSLRequest and PortNotExposed connection errors in store tests and in smoke. About
   1 in 10 early cargo-mutants "kills" were connection errors; they were rerun, not counted.
6. **A step whose RED is a compile failure skips the lane's red.** 02-07's RED was the copy test
   failing to compile because the POST route did not exist. The acceptance scenarios were never
   run red before GREEN (review F-02/F-03, recorded, not re-run).

## Measured KPIs

The acceptance and store halves are measured. Every dev and prod half is owed, because nothing is
deployed.
- **PL-KPI-1** (north star): the `pl-01`/`pl-02` happy paths are green; the operator report and
  the per-kind record counts are owed.
- **PL-KPI-2** (nothing erased): store checksum snapshot and restore equality, both races, and
  acceptance before/after counts; dogfood counts owed.
- **PL-KPI-3** (answers nowhere): scenarios 29 and 30 (above); the slice-02 dogfood is owed.
- **PL-KPI-4** (refusals learn nothing): the authz outlines, with row counts, are green.
- **PL-KPI-5** (links kept): the rename and restore 15-page sweeps are green; bookmark dogfoods
  owed.
- **PL-KPI-6** (restore demand): owed, a count at 30 days.
- **PL-KPI-7** (retired teams deletable): store `team_delete.rs` (detach, mixed team, 100-run
  race) and the `pl-04` scenarios are green; dogfood owed.

## Permanent artifacts

- `docs/product/architecture/adr-project-lifecycle-001-live-projects-view-is-the-read-seam.md`
- `docs/product/architecture/adr-project-lifecycle-002-soft-delete-write-guard-and-restore.md`
- `docs/product/architecture/adr-project-lifecycle-003-project-lifecycle-events-and-one-rename-core.md`
- `docs/product/architecture/adr-project-lifecycle-004-team-delete-detaches-deleted-projects.md`
- `docs/product/architecture/brief.md`: "Projects: one live view, soft delete and restore"
- `docs/product/jobs.yaml`: `job-project-lifecycle`
- `docs/product/outcomes/registry.yaml`: OUT-27..OUT-31; OUT-1, OUT-2 and OUT-25 amended
- `docs/product/kpi-contracts.yaml`: `project-lifecycle`, PL-KPI-1..7
- `CHANGELOG.md` `[Unreleased]`: Added, Changed, Upgrading, migration notes for 0025-0027,
  Backups
- `docs/feature/project-lifecycle/`: the full wave history

The ADRs already live in `docs/product/architecture/` (house layout), and the lean layout has no
`design/`, `discuss/journey-*` or `distill/walking-skeleton.md` files, so nothing else needed
migrating.

## Release (not done)

- **One release for the whole feature** (DEVOPS R1, widened). Tag only on the user's go.
- **Before the tag carrying 0026** (R4): the read-only OQ-D4 project counts on dev and prod, and
  the CHANGELOG's OQ-D4 and Rollback lines. These are the user's.
- **R5:** one green `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`, with the acceptance step
  run, before any tag.
- 0025, 0026 and 0027 ship together (R2). After deploy, verify each host: footer version,
  `/readyz`, one `UPGRADE_WARNING_0025` and one `_0026` in the pod log, then the DoD 8 dogfoods
  (rename and reopen a bookmark; delete, check, restore and recheck a scratch project; delete a
  scratch team whose only project is deleted).

## Follow-ups / open items

- **F-14, post-write authority re-read:** after a committed delete or rename, the members page is
  re-read with the actor's current authority. A demotion in that window answers the uniform 404
  although the write committed. Rare; the operator list shows the truth.
- **`list_project_lanes`** (`lanes.rs:112`) reads lanes by `project_id`, not through
  `live_projects`. Unreachable from the doors today, which resolve the project through the live
  view first (02-08's differential is green).
- **Fixed (follow-up):** `list_project_lanes` now reads through `live_projects`, and every caller
  maps its empty answer to its not-found (c2a7acf); F-14, a committed project rename or delete
  and a team rename render the members page from the authority read once (a91d982).
- **API 403 vs 404** on team-addressed routes still reveals whether a team exists (carried from
  fix-issue-attachment-scoping).
- **The SSE refusal echoes the team slug** (`events.rs` 403 text). A deleted project matches it,
  by design (Changed Assumption 1).
- **Scope machine-token routes by project** (U1).
- **A `Store` capability split** (OQ-U2).
- **A shared app test harness** (`tests/common/mod.rs`).
- **`foundry doctor set-role`** for workspace roles, today SQL only.
- From DISCUSS, still out of scope: restore into a team for a detached project, a web restore
  for workspace admins (if PL-KPI-6 shows demand), a purge.
- Watch: `team_delete::the_detach_backstop_logs_one_error_naming_the_team` flaked once in smoke
  during the refactor phase ("the detach never waited on the held row", inside its 10 s
  `pg_stat_activity` poll window);
  `workspace_rename_with_audit::the_rename_record_refuses_an_entry_that_changes_nothing` failed
  once in a full-crate run and passed four times after.
