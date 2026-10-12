# Mutation report: team-lifecycle

Strategy: per-feature (CLAUDE.md). The gate is a kill rate of at least 80% on
every Rust production file the feature modified, steps 01-01 to 04-03
(`git diff f14e979~1..15b8736`). cargo-mutants cannot mutate SQL, so
migrations 0023 and 0024, and the SQL strings inside the store methods, are
covered by hand-mutation tables. Timeouts count as caught. Unviable mutants
(those that do not compile) are left out of the denominator.

The final feature diff yields 154 mutants (`cargo mutants --list --in-diff`)
across 15 files.

Status at step 04-04: the per-step results below, plus a closing re-run
(`cargo mutants -j 1 --in-diff <feature diff>`) of the files whose final figure
was unclear. The core re-run finished. The services and store re-runs could not
run: on 2026-10-12 Docker Desktop stopped publishing container ports (a fresh
`docker run -P postgres:16-alpine` reports `"5432/tcp":[]`), so both baselines
failed in untouched tests with `PortNotExposed` before any mutant was tested.
The store and services figures below are therefore the last per-step
measurements, and the closing re-run of those two crates is owed. A retry
after Docker came back failed the same way (`PortNotExposed` in four
`name_rule_in_database.rs` tests in the unmutated tree): port publishing was
intermittent, and a flaky baseline would also make "caught" verdicts
untrustworthy, so the retry was stopped.

## Per-file verdict

"Mutants" is the count in the final feature diff (`cargo mutants --list`). The
rate is caught / viable at the last measurement, timeouts counted as caught.

| File | Mutants | Last measurement | Rate | Verdict |
|---|---|---|---|---|
| `crates/foundry-core/src/team_name.rs` | 10 | 04-04 re-run, with the two files below: 27 mutants, 21 caught, 1 timeout, 0 missed, 5 unviable | 22/22 | pass |
| `crates/foundry-core/src/project_name.rs` | 12 | 04-04 re-run (see above); 03-02 22/22 after the `MintedSlug::as_str` fix | 100% | pass |
| `crates/foundry-core/src/team_membership.rs` | 5 | 04-04 re-run (see above); 02-02 core 4/4 | 100% | pass |
| `crates/foundry-store/src/teams.rs` | 35 | final re-run 2026-10-12 (`cargo mutants -j 1 -p foundry-store -f crates/foundry-store/src/teams.rs --in-diff <feature diff>`, 1h 19m): 28 caught, 1 missed, 6 unviable, 0 timeouts | 28/29 (97%) | pass; the one miss is W1 (`is_name_rule_refusal -> true`, log-only). Confirms 5cc220e (`is_fk_violation` `&&`->`\|\|` caught) and f4d5179 (both guards at :341 and :473 caught `-> true` and `-> false`) |
| `crates/foundry-store/src/lib.rs` | 9 | 01-01 3/3; 02-01 7/8 after hardening | 7/8 (88%) | pass, with W2 (`Store::migrate`, smoke-only) |
| `crates/foundry-store/src/upgrade_warnings.rs` | 5 | 02-01 5/5 | 100% | pass |
| `crates/foundry-services/src/teams.rs` | 17 | 02-02 2/2; 03-02 1/1; 04-02 2/2; the 03-02 race proofs | 5/5 measured | pass at the step figures; closing re-run owed |
| `crates/foundry-services/src/tokens.rs` | 3 | 02-01 1/1 (`scope_team_not_in_workspace`) | 1/1 | pass at the step figure; `mint_token`'s two mutants were not measured by cargo-mutants; the 422 race test drives them; re-run owed |
| `crates/foundry-services/src/lib.rs` | 4 | not measured by cargo-mutants (four `Services` delegators, each `-> Ok(Default::default())`) | n/a | the services use-case tests and the tl-01..tl-04 lanes call every delegator; re-run owed |
| `crates/foundry-services/src/projects.rs` | 1 | not measured (`attempt_create -> Ok(Default::default())`) | n/a | the project-create use-case tests drive it; re-run owed |
| `crates/foundry-app/src/teams.rs` | 15 | 01-01 1/1; 02-02 4/5 (the survivor, `projects_refusal_not_yet_rendered`, was an interim function 02-04 deleted); 02-03 5/5; 02-04 4/4; 04-02 2/2; 04-03 3/3; plus the 02-04 and 04-03 hand tables | 100% of the code that remains | pass |
| `crates/foundry-app/src/projects.rs` | 2 | 02-04 2/2 in the tl-02 lane | 100% | pass |
| `crates/foundry-app/src/signin.rs` | 2 | 01-02 (4/4 viable with the store's `first_team_slug`) | 100% | pass |
| `crates/foundry-app/src/lib.rs` | 1 | `build_router -> Default::default()`, caught by every lane | 100% | pass |
| `crates/foundry-app/src/views.rs` | 0 | no mutant generated | n/a | the pages are pinned by the tl-02 and tl-04 lanes |
| `crates/foundry-core/src/lib.rs` | 0 | no mutant generated (a module declaration) | n/a | n/a |
| `xtask/src/check_arch.rs` | 33 | 01-03 31 caught + 2 timeouts; 04-01 1/1 (each later seam name has an injected-violation gold test) | 100% | pass |

### Closed (f4d5179): two constraint-name survivors in store `teams.rs`

The 02-01 hardening run (11 mutants, 8 caught) left three survivors. 5cc220e
adds `a_project_insert_refused_by_another_foreign_key_is_not_team_gone`, a 23503
on `projects_workspace_id_fkey`, which is written to kill
`is_fk_violation` `&&` -> `||` (and the `insert_project` guard). No re-run
confirms it (Docker, above). The other two are not addressed by any test:

| Mutant | What it would ship |
|---|---|
| `teams.rs:341` match guard `is_fk_violation(&err, "team_memberships_team_id_fkey")` -> `true` in `Store::add_team_member` | any other failure of the insert (for example a closed pool or a lost connection) answers `TeamGone`, the uniform 404, instead of a `500` |
| `teams.rs:473` match guard `is_fk_violation(&err, "workspaces_default_team_same_workspace")` -> `true` in `Store::set_default_team` | the same, for setting the default team |

These are survivors on a constraint name, which the 04-04 criteria say must be
killed, not waived. Killing them needs a store test that makes each write fail
for another reason and asserts the outcome is an error, not `TeamGone`. That is
test authorship, so it is routed to the acceptance designer, not done here.

## Waivers

Two kinds, each documented and accepted:

| # | Mutant | Kind | Why it is not killed |
|---|---|---|---|
| W1 | `is_name_rule_refusal -> true` / `-> false` (store `teams.rs`, 04-01) | log-only | It decides only whether a 0024 refusal is logged as a `warn` with the team id (DDD-25); the returned error is the same either way. No test captures logs. This is the same class as team-membership's H8. Follow-up: a tracing-capture test harness covering both (owed, below). |
| W2 | `Store::migrate -> Ok(())` (store `lib.rs`, 02-01) | smoke-only | The store tests build their schema through the test harness, not through `Store::migrate`, so only `cargo xtask smoke` (a booted binary against an empty database) kills it. Smoke is a strict subset of `cargo xtask ci`; every one of its stages passed in 04-04's first full CI run. |
| W3 | 02-01 H4, the locked read and delete merged into one `DELETE ... WHERE NOT EXISTS` with the `FOR UPDATE` lock kept | equivalent | With the team row already locked `FOR UPDATE`, no project insert can commit between the check and the delete, so the merged statement behaves the same. The design keeps two statements so the refusal can name the projects (DDD-10). The unlocked single statement (H3) is killed. |

No survivor sits on lock strength, statement order or a check-arch clause
boundary: each of those is killed (H1, H2, H3; the 03-01 and 04-01 tables; the
01-03 check-arch pass). The two constraint-name survivors were closed by f4d5179 (above).

## SQL and hand mutations

Method: each mutation was a temporary edit to the file, the named tests or
lane were run, and the file was restored afterwards.

### 0023 and `delete_team_if_empty` (step 02-01): 4 of 4 non-equivalent killed

Killed by `crates/foundry-store/tests/team_delete.rs` (17 tests, including the
100-run delete-versus-project-create race).

| # | Mutation | Result |
|---|---|---|
| H1 | Team lock `FOR UPDATE` -> `FOR NO KEY UPDATE` | killed (the waiting-delete test and the 100-run race lose a project) |
| H2 | The project read moved before the lock | killed (same two tests) |
| H3 | No lock, one `DELETE ... WHERE NOT EXISTS` | killed (three tests, including the 100-run race) |
| H4 | Lock kept, read and delete merged into one `DELETE ... WHERE NOT EXISTS` | survived, equivalent (W3) |
| H5 | The `deleted` record `INSERT` dropped | killed (three tests, including "the delete record outlives the team") |

Both negative controls in `team_delete.rs` (`FOR NO KEY UPDATE`, and the
single `DELETE ... WHERE NOT EXISTS`) still lose a project, which proves the
race test can see the loss. Migration 0023's shape is pinned by the shape
CHECK test, the no-FK-to-`teams` test, the apply-time self-check tests and the
two upgrade-warning tests in `team_lifecycle_migrations.rs`, the probe test in
`probe_schema_scoping.rs`, and the restore test in `name_rule_in_database.rs`.

### 0024 `team_name_rule` (step 03-01): 6/6 killed

Killed by `team_lifecycle_migrations.rs` and `name_rule_in_database.rs` (the
teams parity at 64, the per-arm refusals and the undo/re-arm test).

| # | Mutation | Result |
|---|---|---|
| S1 | The rename trigger's `WHEN` dropped (fires on every update) | killed (2 tests, including "a legacy team name is never scanned") |
| S2 | Cap 64 -> 63 | killed (3 tests) |
| S3 | Cap 64 -> 65 | killed (2 tests) |
| S4 | The `teams` HINT arm dropped from `foundry_enforce_name_rule()` | killed (2 tests) |
| S5 | The rename trigger dropped | killed (3 tests) |
| S6 | The rename trigger dropped with the self-check left intact | killed (the migration refuses to apply) |

### `rename_team_with_audit` (step 04-01): 8/8 killed

Killed by `crates/foundry-store/tests/team_rename.rs` (six tests, two of them
100-run races).

| # | Mutation | Result |
|---|---|---|
| R1 | Team lock `FOR NO KEY UPDATE` dropped | killed (both 100-run races) |
| R2 | `FOR NO KEY UPDATE` -> `FOR SHARE` | killed (the concurrent-renames race) |
| R3 | The `renamed` record `INSERT` made a no-op | killed (3 tests) |
| R4 | The byte-equal no-op removed | killed |
| R5 | The slug rewritten with the name | killed (2 tests) |
| R6 | The record's old name taken from a constant, not the locked read | killed (the chained-old-names race) |
| R7 | `clock_timestamp()` -> `now()` | killed (the chained-old-names race; this is the ordering 04-01 found and fixed) |
| R8 | The workspace scope dropped from the lock query | killed (another workspace's team is not found) |

### Delete door (step 02-04): 5/5 killed

Run in the tl-02 lane against a rebuilt binary, and the app's unit tests.

| # | Mutation | Result |
|---|---|---|
| D1 | The commit-time refusal `409` -> `200` | killed (tl-02) |
| D2 | The refusal page `200` -> `409` | killed (tl-02) |
| D3 | The Delete button always rendered on the refusal page | killed (tl-02) |
| D4 | `CreateProjectError::TeamGone` -> `500` | killed (tl-02) |
| D5 | The project list left unsorted | killed (app unit test, list-order property) |

### Rename refusal (step 04-03): 4/4 killed

Run in the tl-04 lane against a rebuilt binary.

| # | Mutation | Result |
|---|---|---|
| N1 | Name refusal `422` -> `200` | killed (5 of 22 scenarios fail) |
| N2 | The typed text not kept in the field | killed (5 of 22 fail) |
| N3 | The refusal copy dropped | killed |
| N4 | The `422` arm answers the uniform `404` | killed |

## Owed (not done here)

- Done (f4d5179): the acceptance designer's two store tests for the guard survivors (`an_add_refused_for_another_reason_is_an_error_not_team_gone`, `a_default_set_refused_for_another_reason_is_an_error_not_team_gone`; guard→true fails each).
- DONE 2026-10-12: the cargo-mutants re-run on store `teams.rs` (28/29, see the table).
- Still owed: the re-run on store `lib.rs` and the four services files.
- A tracing-capture test harness that would kill W1 and team-membership's H8.
