# RED classification — project-lifecycle

Gate output of the pre-DELIVER *fail-for-the-right-reason* check (ADR-025
scaffolded RED, rigor profile `adr-025-scaffolded-red`). DELIVER reads this file
at the start of each step to confirm the RED it un-pends is genuine.

## Acceptance (`@project-lifecycle`, five feature files) — iteration 2 (final)

**Run** (2026-10-13, after the iteration-2 review edits): `foundry` rebuilt and
warmed (`CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false cargo build --release -p
foundry-app --bin foundry`, then `target/release/foundry --help` three times);
`@pending` stripped from the five `project-lifecycle-0*.feature` files only (a
`perl` over the working files, byte-exact copies kept in the scratchpad), then
`CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false FOUNDRY_ACCEPTANCE_TAGS=project-lifecycle timeout 4000 cargo test -p foundry-acceptance --release --test acceptance`.
The files were restored byte for byte by an EXIT trap (`cmp` clean; 59 of 59
scenario `@pending` tags back: per file 16, 12, 11, 12, 8).

**Result**: 5 features, **298 scenarios** (59 declarations, outline rows
expanded), 2 517 steps (2 219 passed); **298 RED (MISSING_FUNCTIONALITY), 0
GREEN_ALREADY, 0 BROKEN**; 0 parsing errors, 0 hook errors, 0 undefined or
ambiguous steps (the lane runs `fail_on_skipped`). One row (the restore with
the page sweep) first stopped at its own sweep precondition, because Sam is not
the author of the comment whose edit form the sweep opened (a harness choice);
the sweep now prefers the sweeper's own comment and does not require another
author's comment edit form to open (it is still compared before and after).
Re-run alone (scratch copy): the sweep Given passes and the row stops at "Lena
has deleted the project Auth v2 Spike" (MISSING_FUNCTIONALITY).

**Verdict: PASS.** Zero BROKEN, zero false GREEN.

### Why no row passes for the wrong reason

None of the new doors exists. Every refusal row already gets the
byte-identical 404 it asserts, so each refusal scenario also asserts a control
proving the hidden project or door exists ("Priya still finds the rename form
of …", "… still finds the delete page of …", "Hugo still finds … of Ledger",
"the same project rename / delete with the page's token goes through", "Lena
can still download Identity Platform's attachment", "Ines can still rename …
from the instance dashboard", "Sam can still save the title … on AUTH-1"). The
controls are what is RED today.

The INV-PL-2 rows need the delete, so all of them stop at "Lena has deleted the
project Auth v2 Spike": 87 for Lena, Sam and Priya (the live control must answer
otherwise), 120 for the four who cannot reach Backend (the live control must
answer them the same way), 6 script rows and the 2 machine-token scenarios.

The operator's door: today `restore-project` and `list-deleted-projects` answer
"unknown subcommand" with **exit 2**; the `not-a-uuid` row expects exit 2 too and
stays RED only because it also pins the stderr line
`foundry doctor restore-project: invalid UUID "not-a-uuid"`. The instance
dashboard's rename goes through today and is RED only on its missing record.

### Harness self-checks (scratch, not committed)

Most rows stop at the delete Given, so their later steps never ran in the
lifted run. Scratch copies **without** the delete were run to prove those
mechanics:

- iteration 1: the 27 shipped web doors × Lena, Sam, Priya and the 6 script
  doors are well-formed (the live project answers otherwise, below 500), and
  OQ-D1's never-existed answers are as pinned; the two machine-token doors read
  no project (now exempt, U1);
- iteration 2: **all 120 outsider rows** (a signed-out visitor, Marco, Ines,
  Hugo × 30 doors) pass — for each, the live Identity Platform answers exactly
  as a never-existed project (no oracle) — after the step regex was widened to
  admit "a signed-out visitor" (undefined for that actor until then: caught
  here, fixed); the token **revoke pair** passes (the first revoke revokes the
  spare token and changes nothing else; the second, already revoked, answers
  the same and changes nothing); the **page sweep** passes.

### By failure point

| Where it stops | Rows | Classification |
|---|---|---|
| "… has deleted the project …" (Given): no Projects section, so no **Delete project** link (every INV-PL-2 row among them) | 238 | MISSING_FUNCTIONALITY |
| "… opened the delete page of the project … earlier" (Given) | 5 | MISSING_FUNCTIONALITY |
| A rename or delete "from the page" (When): no Projects section | 15 | MISSING_FUNCTIONALITY |
| The Projects section, its links and controls (Then) | 6 | MISSING_FUNCTIONALITY |
| A refusal's control (Then) | 24 | MISSING_FUNCTIONALITY |
| A tokenless write's control (Then) | 2 | MISSING_FUNCTIONALITY |
| "Labs has no projects yet." is absent | 1 | MISSING_FUNCTIONALITY |
| The instance door renames but writes no record | 1 | MISSING_FUNCTIONALITY |
| The operator's subcommands do not exist | 4 | MISSING_FUNCTIONALITY |
| The delete-versus-file race: run 1's delete answers 404 | 1 | MISSING_FUNCTIONALITY |
| The restore-versus-team-delete race's Given cannot delete its project | 1 | MISSING_FUNCTIONALITY |
| **All** | **298** | **298 MISSING_FUNCTIONALITY** |

### Per scenario

| # | File | Scenario | Rows | RED |
|---|---|---|---|---|
| 1 | 01 | A team's lead renames a project and every link to it keeps working | 1 | MF: no Projects section on the members page |
| 2 | 01 | After a rename every page of the project answers as it did before | 1 | MF: no Projects section on the members page |
| 3 | 01 | A workspace admin renames a project of a team she is not on | 1 | MF: no Projects section on the members page |
| 4 | 01 | A rename made from the instance dashboard is on record too | 1 | MF: the shipped rename goes through, no project_lifecycle_events record |
| 5 | 01 | A change of letter case is a real rename, and a project never collides with itself | 1 | MF: no Projects section on the members page |
| 6 | 01 | A name of exactly 256 characters is accepted | 1 | MF: no Projects section on the members page |
| 7 | 01 | Submitting the same name changes nothing and records nothing | 1 | MF: no Projects section on the members page |
| 8 | 01 | A name already used in the team is refused, and the text is kept | 1 | MF: no Projects section on the members page |
| 9 | 01 | A name that breaks the project-name rule is refused with its reason, and the text is kept | 4 | MF: no Projects section on the members page |
| 10 | 01 | The Projects section names each project by name then key, and each field says which project it is for | 1 | MF: no Projects section |
| 11 | 01 | Everyone on the team sees its projects, and only its lead or a workspace admin gets the controls | 5 | MF: no Projects section |
| 12 | 01 | A team with no projects says so | 1 | MF: no "Labs has no projects yet." |
| 13 | 01 | Only the team's lead or a workspace admin can rename a project | 6 | MF: control (refusal already byte-identical) |
| 14 | 01 | A rename aimed at another workspace's project or at a project that does not exist is refused the same way | 2 | MF: control (refusal already byte-identical) |
| 15 | 01 | A lead who has since been made a member cannot rename from a page she opened as a lead | 1 | MF: no Projects section on the members page |
| 16 | 01 | A rename sent without the page's token is turned away, and nothing changes | 1 | MF: control (the CSRF 403 already holds) |
| 17 | 02 | The delete page says what happens before anything does | 1 | MF: no Projects section on the members page |
| 18 | 02 | A team's lead deletes a project after reading what happens | 1 | MF: opening the delete page earlier (Given): no Delete project link |
| 19 | 02 | Deleting a project erases none of its work | 1 | MF: no Projects section on the members page |
| 20 | 02 | The delete page of a project with no issues says so | 1 | MF: no Projects section on the members page |
| 21 | 02 | A workspace admin deletes a project with no issues on a team she is not on | 1 | MF: opening the delete page earlier (Given): no Delete project link |
| 22 | 02 | Cancel goes back to the team, and nothing changes | 1 | MF: opening the delete page earlier (Given): no Delete project link |
| 23 | 02 | A delete confirmed a second time finds nothing, and deletes once | 1 | MF: opening the delete page earlier (Given): no Delete project link |
| 24 | 02 | Only the team's lead or a workspace admin may open a project's delete page | 6 | MF: control (refusal already byte-identical) |
| 25 | 02 | Only the team's lead or a workspace admin may delete a project | 6 | MF: control (refusal already byte-identical) |
| 26 | 02 | Another workspace's project and a project that does not exist look the same from Canzan Labs | 4 | MF: control (refusal already byte-identical) |
| 27 | 02 | A lead who has since been made a member cannot delete from a page she opened as a lead | 1 | MF: opening the delete page earlier (Given): no Delete project link |
| 28 | 02 | A delete sent without the page's token is turned away, and nothing changes | 1 | MF: control (the CSRF 403 already holds) |
| 29 | 02b | Every web address of a deleted project answers as one that never existed, for everyone | 87 | MF: the delete (Given): no Projects section, no Delete project link |
| 30 | 02b | For someone who cannot reach Backend, a deleted project's address answers as one that never existed | 120 | MF: the delete (Given): no Projects section, no Delete project link |
| 31 | 02b | A deleted project cannot be reached by a script either, and nothing is written | 6 | MF: the delete (Given): no Projects section, no Delete project link |
| 32 | 02b | Listing machine tokens under a deleted project's address answers as under any address, because the list names no project | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 33 | 02b | Revoking a machine token under a deleted project's address revokes it, as under any address, because the token names no project | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 34 | 02b | A deleted project's real attachment cannot be downloaded by anyone | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 35 | 02b | A deleted project disappears from every list, and the others stay | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 36 | 02b | The instance dashboard's rename does not find a deleted project | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 37 | 02b | A change sent from a page opened before the delete is refused, and nothing changes | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 38 | 02b | An open live view of a deleted project hears of no change made after the delete | 2 | MF: the delete (Given): no Projects section, no Delete project link |
| 39 | 02b | Filing an issue during a delete gets a normal answer, never a failure | 1 | MF: run 1's delete answers 404 |
| 40 | 03 | The operator finds a deleted project on the list | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 41 | 03 | The operator restores a deleted project with everything under it | 1 | MF: the delete (Given; re-run alone after the sweep fix) |
| 42 | 03 | A restored project holds exactly what it held when it was deleted | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 43 | 03 | The operator is told when there is nothing to restore | 1 | MF: the operator subcommand does not exist (exit 2; the exit-2 row fails on its pinned stderr line) |
| 44 | 03 | Every deleted project is listed, the oldest deletion first | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 45 | 03 | A restore that cannot be done is refused with its reason, and nothing changes | 3 | MF: the operator subcommand does not exist (exit 2; the exit-2 row fails on its pinned stderr line) |
| 46 | 03 | Restoring the same project twice restores it once | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 47 | 03 | A restore attempted while foundry's records cannot be reached changes nothing | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 48 | 03 | A deleted project's name stays taken for a new project until it is restored | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 49 | 03 | A deleted project's key stays taken in the workspace until it is restored | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 50 | 03 | A deleted project's name stays taken for a rename until it is restored | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 51 | 03 | A deleted comment inside a deleted project cannot be brought back on its own | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 52 | 04 | The team delete page names the deleted project it will keep | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 53 | 04 | A team whose only project is deleted can be deleted, and the project's work is kept | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 54 | 04 | Only live projects keep a team from being deleted, and only they are named | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 55 | 04 | A delete sent for a team that still has a live project is refused, naming only the live one | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 56 | 04 | A deleted project whose team is gone cannot be restored, and is not lost | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 57 | 04 | A project kept from a deleted team is still on the operator's list | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 58 | 04 | A deleted project's key stays taken after its team is deleted | 1 | MF: the delete (Given): no Projects section, no Delete project link |
| 59 | 04 | A restore racing a team delete never loses a project | 1 | MF: the race's Given cannot delete its project (no Projects section) |


## Store, use-case, app and check-arch gold tests

**Runs** (2026-10-13, iteration 2): per file, `RUST_TEST_THREADS=2 timeout … cargo
test -p <crate> --test <file>` (normal, ignored excluded), then `-- --ignored`
with filters for the new tests; `timeout 900 cargo test -p xtask` and
`cargo test -p xtask project_lifecycle -- --include-ignored`.

| File | Tests (new) | GREEN now (not ignored) | RED (ignored) | How each RED fails | BROKEN |
|---|---|---|---|---|---|
| `foundry-store/tests/project_lifecycle.rs` | 37 | 8: six Postgres substrate facts and **the negative control** (a delete with FOR NO KEY UPDATE lets a guarded write commit after it) | 29 (01-01, 02-01, 02-02, 02-03, 03-01, 04-01) | 28 at a schema RED gate, with the lab seeded through the shipped writers first (iteration 2 adds: the production delete waits on a held guard; **a notification queued before the delete stays pending and none arises after it** (D17, U3b); **a restore takes the team lock before the project lock** (DDD-8); **`project_lifecycle_events` is not a tenant table** (DDD-14)); 1 on the `list_team_projects` scaffold panic | 0 |
| `foundry-store/tests/project_lifecycle_migrations.rs` | 13 | 0 | 13 (01-01, 02-01, 04-01) | 10 at the migration-file gate (iteration 2 adds **the previous release boots on the migrated schema and applies nothing**; **0026's parity self-check** now asserts `StoreError::MigrationFailed` naming `live_projects`); 3 on the empty `UPGRADE_WARNING_0025` / `_0026` scaffolds | 0 |
| `foundry-store/tests/probe_schema_scoping.rs` (extended; 2 shipped tests green) | 3 | 0 | 3 (01-01, 02-01, 04-01) | the migration-file gate: a sibling schema holding `project_lifecycle_events` / `deleted_at`+`deleted_by`+`live_projects` / `detached_from_team_id` must not satisfy the probe for the current schema | 0 |
| `foundry-store/tests/team_delete.rs` (extended; shipped tests green) | 9 | 1: **the negative control** of ADR-PROJECT-LIFECYCLE-004 | 8 (04-01) | 7 at the 0026 + 0027 gate; 1 panic stub (**the detach backstop logs one error naming the team**, DDD-30) | 0 |
| `foundry-services/tests/project_lifecycle_use_cases.rs` | 13 | 0 | 13 (01-02, 02-03, 03-01, 04-01) | 4 on scaffold panics; 9 at a 0025/0026 gate (iteration 2 adds **`teams::delete_preview` changes zero rows with deleted projects**) | 0 |
| `foundry-app/tests/project_lifecycle_copy.rs` (new) | 4 | 0 | 4 (01-02, 02-03, 03-01, 04-01) | panic stubs for the byte tests of every new `TeamNotice` string and CLI constant (D14, DIST-1, DIST-2) | 0 |
| `xtask/src/check_arch.rs` (`project_lifecycle_check_arch_tests`) | 24 | 0 | 24 (01-01, 02-01, 02-02, 02-03, 03-01) | the scaffold panic of the three scanners (each gold tree built first) | 0 |
| **All** | **103** | **9** | **94** | | **0** |

Both negative controls lose the write or the project on Postgres 16, exactly as
ADR-PROJECT-LIFECYCLE-002 and -004 argue. They are GREEN substrate facts, not
pending tests.

On D17: nothing in the code stamps `outbox.notified_at` today
(`Store::count_pending_outbox`'s doc: delivery is the COMMIT-time NOTIFY of
migration 0003), so "still delivered" is asserted as "byte-unchanged and still
pending (`notified_at IS NULL`)".

When DELIVER adds the new probe lines, the two shipped tests in
`probe_schema_scoping.rs` need the new objects in their healthy schemas (as for
0018, 0022 and 0023); the comment block above the new tests says so.

## Burn-down

- `grep -cE '^\s*@.*@pending' crates/foundry-acceptance/tests/features/project-lifecycle-0*.feature` = 16 + 12 + 11 + 12 + 8 = 59 now, 0 at the end.
- `grep -c '#\[ignore = "pending: project-lifecycle'` over `crates/foundry-store/tests/{project_lifecycle,project_lifecycle_migrations,probe_schema_scoping,team_delete}.rs`, `crates/foundry-services/tests/project_lifecycle_use_cases.rs`, `crates/foundry-app/tests/project_lifecycle_copy.rs` and `xtask/src/check_arch.rs` = 29 + 13 + 3 + 8 + 13 + 4 + 24 = 94 now, 0 at the end.
- `grep -rn "RED scaffold written by DISTILL (2026-10-13)" crates/ xtask/` lists the scaffold sites, 0 at the end.
