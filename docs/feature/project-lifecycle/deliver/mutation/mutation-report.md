# project-lifecycle: Phase 5 mutation report

Feature range: DISTILL base `5c98860` .. `4cc3000` (HEAD during the run). Gate:
kill rate >= 80% per file (project CLAUDE.md, per-feature strategy). Run on
2026-10-15 from 03:13 to 10:55 PDT (about 7 h 45 min wall clock).

## Method

- **Tool and scope.** cargo-mutants 25.3.1. The feature diff
  (`git diff 5c98860..HEAD -- crates xtask`) was passed to `--in-diff`, so
  mutants were generated only on lines the feature changed. Each target file
  was named with `--file`. Other settings: `-j 1`, `--gitignore true` (so the
  copied tree never carries `.env`), and `timeout 14400` around every run. The
  runs used copy mode, not `--in-place`, so cargo-mutants never mutated the
  working tree. Free disk stayed above 220 GB.
- **Test command per crate** (filters after `--`):
  - **foundry-store** (`-p foundry-store`, `RUST_TEST_THREADS=1`,
    `--timeout 600`): `project_lifecycle team_delete
    project_lifecycle_migrations probe_schema_scoping team_lifecycle_migrations
    name_rule_in_database comment_attachment_issue_scoping
    insert_issue_with_outbox issue_delete delete_lane_with_fate
    reposition_issue update_issue_details`, plus `list_projects_for_workspace
    export_workspace_gold team_rename`, because the diff also touches those
    readers. The baseline was green in 3m03s.
  - **foundry-services** (`-p foundry-services`, `--baseline run`):
    `project_lifecycle_use_cases team_lifecycle_use_cases
    rename_project_use_case`.
  - **foundry-app** (`-p foundry-app`, every app test, unit and integration,
    `--baseline run`): baseline 2m32s.
  - **xtask** (`-p xtask`, all 158 tests, including the `*_shipped_tree_passes`
    tests). The first pass ran with `--timeout 120`. It was interrupted after
    123 mutants because each infinite loop cost 120 s. It then resumed with
    `--iterate --timeout 20` (the baseline test takes 1.7 s). The iterate pass
    re-tested every earlier miss and timeout.
- **Cross-crate re-check.** cargo-mutants tests only the mutated package. So
  the 12 store survivors were run again (the same mutants, selected with `-F`
  exact names) against the full foundry-services and foundry-app test suites
  (`--test-package foundry-services --test-package foundry-app`).
- **Acceptance re-check.** This covered every app survivor and the
  store/services survivors that a scenario plausibly covers. For each one:
  1. Back up the file and apply the cargo-mutants diff to the real tree with
     `patch`.
  2. Build the release binary:
     `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false cargo build --release -p
     foundry-app --bin foundry`.
  3. For mutants in code the in-process HTTP harness links (store readers,
     `teams.rs`), also run
     `cargo test -p foundry-acceptance --release --test acceptance --no-run`.
     CLI-only mutants (`admin_cli.rs`, and store/services paths only the CLI
     reaches) need only the `foundry` binary rebuilt, because the scenarios exec
     it.
  4. Exec the binaries three times to warm them up (syspolicyd).
  5. Run the acceptance binary with `FOUNDRY_ACCEPTANCE_TAGS=project-lifecycle`
     (pl-01..pl-04, 299 scenarios, about 48 s), then a second tag chosen per
     mutant (`admin-cli`, `team-lifecycle` or `iapr`).
  6. Restore the file from the backup and check it with `cmp`.
  
  A mutant counts as killed when a scenario fails on an assertion. All 33
  re-checks logged `RESTORED-CMP-OK`, and `git status` was clean after each.
  Unmutated baselines: `project-lifecycle` 299/299, `admin-cli` 3/3,
  `team-lifecycle` 95/95. At the end the unmutated release binaries were
  rebuilt, so `target/release` holds no mutant.
- **Kills caused by connection errors are not counted.** Every caught-mutant
  log in the DB crates was searched for `PortNotExposed`, `PoolTimedOut`,
  `ConnectionReset`, `SSLRequest`, `InvalidContentType`, `WaitContainer` and
  `start postgres container`. Some mutants had been "caught" only by such a
  connection or container failure: 9 store mutants, 1 cross-crate re-check and
  1 app mutant. Docker Desktop flaked under load; every preflight found no
  canzan-lift listener on 55xxx. None of these is counted as caught on that
  evidence. Each one was re-run (two of the store ones twice, because the first
  re-run hit `InvalidContentType` and `WaitContainer`) or re-checked through
  acceptance, and is counted by its honest result. In the acceptance re-check
  of `admin_cli.rs:680` (`false`), one of the two failing scenarios was the
  known sqlx `'\0'` seed flake. The kill counts only because the second
  failure is a real assertion ("the operator reads exactly \"status: none\"").
- **SQL is not covered by cargo-mutants.** The DELIVER hand-pins are listed
  below; five of them were pinned again today.

## Per-file results

Kill rate = caught / (caught + missed). Unviable mutants (they do not compile,
mostly because the return type does not implement `Default`) and timeouts are
excluded. "Caught" is the final verdict after the re-checks; the next table
says which method killed each one.

| File | Generated | Caught | Missed | Timeout | Unviable | Kill rate | Gate |
|---|---:|---:|---:|---:|---:|---:|---|
| crates/foundry-store/src/project_lifecycle.rs | 29 | 19 | 2 | 0 | 8 | 90.5% | PASS |
| crates/foundry-store/src/lib.rs | 57 | 46 | 1 | 0 | 10 | 97.9% | PASS |
| crates/foundry-store/src/lanes.rs | 12 | 8 | 0 | 0 | 4 | 100% | PASS |
| crates/foundry-store/src/attachments.rs | 2 | 1 | 0 | 0 | 1 | 100% | PASS |
| crates/foundry-store/src/issue_delete.rs | 3 | 3 | 0 | 0 | 0 | 100% | PASS |
| crates/foundry-store/src/upgrade_warnings.rs | 3 | 3 | 0 | 0 | 0 | 100% | PASS |
| crates/foundry-store/src/teams.rs | 2 | 0 | 0 | 0 | 2 | n/a (no viable mutant) | n/a |
| crates/foundry-services/src/projects.rs | 12 | 3 | 0 | 0 | 9 | 100% | PASS |
| crates/foundry-services/src/lib.rs | 6 | 1 | 0 | 0 | 5 | 100% | PASS |
| crates/foundry-services/src/teams.rs | 1 | 0 | 0 | 0 | 1 | n/a (no viable mutant) | n/a |
| crates/foundry-app/src/admin_cli.rs | 32 | 31 | 1 | 0 | 0 | 96.9% | PASS |
| crates/foundry-app/src/teams.rs | 16 | 15 | 0 | 0 | 1 | 100% | PASS |
| crates/foundry-app/src/main.rs | 4 | 4 | 0 | 0 | 0 | 100% | PASS |
| crates/foundry-app/src/lib.rs | 1 | 1 | 0 | 0 | 0 | 100% | PASS |
| crates/foundry-app/src/views.rs | 0 | — | — | — | — | n/a (the diff adds only fields; no mutable body) | n/a |
| xtask/src/check_arch.rs | 503 | 345 | 108 | 39 | 11 | **76.2%** | **WARN** (70-80) |
| **All** | **683** | **480** | **112** | **39** | **52** | **81.1%** | |

check_arch.rs on other bases (the gate above uses the raw figure):

- Counting the 39 timeouts as detected (each is an infinite loop the suite
  hangs on): 384 / 492 = 78.0%.
- Removing the 27 survivors classified below as equivalent or unreachable from
  the denominator: 345 / 426 = 81.0%, or 384 / 465 = 82.6% with timeouts.

### Which method killed each caught mutant

| File | Own crate's tests | Cross-crate unit/integration tests | Acceptance re-check |
|---|---:|---:|---:|
| store project_lifecycle.rs | 14 | 5 (`list_team_sibling_projects` x5, by services `create_project_use_case::a_concurrent_create_of_the_same_name_is_refused_on_the_retry`) | 0 |
| store lib.rs | 42 (2 of them after a re-run of a connection-error "kill") | 2 (`find_project_by_slug -> Ok(None)` by services `board_use_case`; `find_issue_by_team_project_number -> Ok(None)` by app `api_comment_scoping`) | 2 (`undelete_comment` `==`->`!=`, `list_projects_for_instance -> Ok(vec![])`) |
| store lanes, attachments, issue_delete, upgrade_warnings | 15 | 0 | 0 |
| services projects.rs | 3 | 0 | 0 |
| services lib.rs | 0 | 0 | 1 (`Services::list_deleted_projects -> Ok(vec![])`) |
| app admin_cli.rs | 12 (the `restore_project_copy` byte tests) | 0 | 19 |
| app teams.rs | 3 (`project_list` x2, `TeamNotice` `Display`) | 0 | 12 |
| app main.rs, lib.rs | 5 | 0 | 0 |
| xtask check_arch.rs | 345 | 0 | not applicable |

## Acceptance re-check results

The tags run were `project-lifecycle` (P, 299 scenarios) plus the tag in the
second column. For each tag the table gives the failing scenario count, or
"ok" when all passed.

| Mutant | 2nd tag | P | 2nd | Verdict | Failing step (first) |
|---|---|---|---|---|---|
| store lib.rs:3383 `undelete_comment` `==`->`!=` | admin-cli | ok | 1 | killed | comment-tombstone-gc "the doctor subprocess exits with code 0" (got 4) |
| store lib.rs:4406 `list_projects_for_instance -> Ok(vec![])` | iapr | 3 | 12 | killed | pl-01 "renames the project … from the instance dashboard"; pl-02 "instance dashboard lists …" |
| services lib.rs:343 `Services::list_deleted_projects -> Ok(vec![])` | — | 3 | — | killed | pl-03 "the operator reads exactly these blocks" (got `status: none`) |
| app admin_cli.rs:448 `run_restore_comment -> 0 / 1 / -1` (3) | admin-cli | 1 | 3 | killed x3 | pl-03 "the command exits 4 … is in deleted project …" |
| app admin_cli.rs:553 `comment_not_restorable -> 0 / 1 / -1` (3) | admin-cli | 1 | 1 | killed x3 | pl-03 restore-comment in a deleted project |
| app admin_cli.rs:632 `run_with_services -> 0 / 1 / -1` (3) | admin-cli | 12 | ok | killed x3 | pl-03 list/restore scenarios |
| app admin_cli.rs:633 guard `!v.is_empty()` -> `true` | admin-cli | ok | ok | **survived** | see A-1 |
| app admin_cli.rs:633 guard -> `false`; `delete !` (2) | admin-cli | 12 | ok | killed x2 | pl-03 |
| app admin_cli.rs:677 `run_list_deleted_projects -> 0 / 1 / -1` (3) | admin-cli | 4 | ok | killed x3 | pl-03 |
| app admin_cli.rs:680 guard `rows.is_empty()` -> `true` | admin-cli | 3 | ok | killed | pl-03 list blocks |
| app admin_cli.rs:680 guard -> `false` (its unit-run "kill" was an SSLRequest connection error) | team-lifecycle | 2 | ok | killed | pl-03 "the operator reads exactly \"status: none\"" (the other failure was the sqlx `'\0'` seed flake) |
| app admin_cli.rs:703 `run_restore_project -> 0 / 1 / -1` (3) | admin-cli | 9 | ok | killed x3 | pl-03 restore scenarios |
| app teams.rs:579 `submit_rename -> Default` | team-lifecycle | ok | 18 | killed | tl-04 "Renamed Backend to Platform Engineering." |
| app teams.rs:638 `submit_project_rename -> Default` | team-lifecycle | 24 | ok | killed | pl-01 "Renamed Identity Platform to Identity & Access." |
| app teams.rs:693 `show_project_delete -> Default` | team-lifecycle | 263 | ok | killed | pl-02 delete page heading |
| app teams.rs:748 `submit_project_delete -> Default` | team-lifecycle | 254 | ok | killed | pl-02 "Deleted Auth v2 Spike." |
| app teams.rs:780 `show_delete -> Default` | team-lifecycle | 7 | 25 | killed | team delete page heading |
| app teams.rs:821 `&&`->`\|\|` in `show_delete` | team-lifecycle | 2 | ok | killed | pl-04 TG-8 "must not mention \"also has\"" |
| app teams.rs:821 `delete !` in `show_delete` | team-lifecycle | 2 | ok | killed | pl-04 "Labs also has 1 deleted project(s) …" |
| app teams.rs:902 `projects_refusal -> Default` | team-lifecycle | 2 | 5 | killed | "Ops still has 1 project(s) …" |
| app teams.rs:1042 `==`->`!=` in `PageFields::project_line` | team-lifecycle | 6 | ok | killed | pl-01 the refusal stands in the right project's row |
| app teams.rs:1061 `members_page_with_status -> Default` | team-lifecycle | 293 | 44 | killed | members page has no Projects section |
| app teams.rs:1082 `members_page_rendered -> Default` | team-lifecycle | 294 | 44 | killed | members page has no Projects section |
| app teams.rs:1165 `project_refusal -> Default` | team-lifecycle | 20 | ok | killed | pl-02 "a refusal must be the uniform 404 (D5)" |

Not re-checked through acceptance, because no scenario plausibly reaches them:

- store lib.rs:391 (the probe on a half-migrated schema);
- store project_lifecycle.rs:220 and :222 (the backstop's log line only).

## SQL hand-pin table (DELIVER steps) and today's re-verification

For each re-verified pin, the change was applied to the real file from a
backup and the named test was run. The file was then restored and checked with
`cmp`, and `git status` was clean after each.

| Step | Pin | Result in DELIVER | Re-verified 2026-10-15 |
|---|---|---|---|
| 01-01 | probe ignores one `project_lifecycle_events` column | survived then; closed by TG-7 | **killed**: `if project_records.0 < 11` -> `< 10` fails `project_lifecycle_migrations::the_probe_refuses_a_record_table_missing_any_one_of_its_eleven_columns` ("id: ready") |
| 01-02 | drop `FOR NO KEY UPDATE` on the rename | killed (concurrent renames, 100 runs) | not re-pinned |
| 01-02 | drop the `renamed` record INSERT | killed | not re-pinned |
| 01-02 | the rename also writes slug | killed | not re-pinned |
| 02-01 | 0026 without the parity self-check | killed | not re-pinned |
| 02-01 | `live_projects` view without `deleted_at IS NULL` | killed | not re-pinned |
| 02-01 | `lock_live_project` without `FOR KEY SHARE` | survived in 02-01, killed in 02-02 | **killed**: `project_lifecycle::every_guarded_writer_waits_on_a_held_delete_then_answers_its_not_found_and_writes_nothing` ("IssueEdit: waited=true not_found=false … children_unchanged=false") |
| 02-01 | `WARNED_VERSIONS` without 25 | killed | not re-pinned |
| 02-02 | `rename_lane` without the guard | killed | not re-pinned |
| 02-02 | guard on a pooled transaction | killed by `write_guard_the_shipped_tree_passes` only | not re-pinned (no xtask unit test of a pooled guard either: X-13) |
| 02-02 | counter `UPDATE projects` instead of `live_projects` | killed | not re-pinned |
| 02-02 | rename SELECT without `deleted_at IS NULL` | killed in 02-05 (100-run race); deterministic since TG-3 | not re-pinned |
| 02-03 | reader on `projects` | killed | not re-pinned |
| 02-03 | `list_team_projects` on `projects` | killed | not re-pinned |
| 02-03 | `TEAM_PROJECTS_SQL` interim | superseded by 04-02 | not applicable |
| 02-05 | delete `FOR UPDATE` -> `FOR NO KEY UPDATE` | killed | not re-pinned |
| 02-05 | drop the `deleted` record INSERT | killed | not re-pinned |
| 02-05 | a second clock read | killed | not re-pinned |
| 02-05 | skip `can_manage` | killed | not re-pinned |
| 03-01 | restore without the team `KEY SHARE` | killed | not re-pinned |
| 03-01 | reversed lock order | killed | not re-pinned |
| 03-01 | no `restored` record INSERT | killed | not re-pinned |
| 03-01 | the CLI calling the store directly | killed | not re-pinned |
| 03-01 | id tiebreak dropped | survived then; closed by TG-4 | **killed**: `ORDER BY p.deleted_at, p.id` -> `ORDER BY p.deleted_at` fails `project_lifecycle::deletions_at_the_same_instant_are_listed_by_id` ("same instant: by id") |
| 03-01 | still-deleted re-check dropped | survived then; closed by TG-2 | **killed**: `… AND deleted_at IS NOT NULL FOR UPDATE` -> `… FOR UPDATE` fails `project_lifecycle::a_restore_that_waited_on_a_restore_that_won_answers_not_deleted_and_records_nothing` (left: `Restored`) |
| 04-01 | view not recreated | killed | not re-pinned |
| 04-01 | CHECK without `deleted_at` in the detached arm | killed | not re-pinned |
| 04-01 | the whole detached arm | killed in 04-02 | not re-pinned |
| 04-01 | probe count / `current_schema()` | killed | not re-pinned |
| 04-02 | detach with a `deleted_at` filter | killed (backstop test) | not re-pinned |
| 04-02 | restore without the team `KEY SHARE` | killed (100-run race) | not re-pinned |
| 04-02 | live check reading every row | killed | not re-pinned |
| 04-02 | no detach statement | killed | not re-pinned |
| 04-02 | CHECK without the detached arm | killed | not re-pinned |
| 04-03 | "also has" line when k == 0 | survived then; closed by TG-8 | **killed** (acceptance): `(facts.projects.is_empty() && !facts.deleted_projects.is_empty())` -> `(facts.projects.is_empty())`. `project-lifecycle` ran 298/299; the failing pl-04 step was "the page must not mention \"also has\" for a team with no deleted project" |
| 04-03 | one byte of copy | killed | not re-pinned |

All five re-pins are killed today. The four closed after the fact (TG-2, TG-4,
TG-7, TG-8) hold, and the 02-01 `FOR KEY SHARE` pin is killed at store level.

## Survivors and classification

Each survivor is classified as (a) equivalent, meaning no observable behaviour
changes; (b) a real test gap, with the test that should exist named for the
acceptance designer; or (c) unreachable.

### foundry-store (3)

| Mutant | Class | Reason / test that should exist |
|---|---|---|
| lib.rs:391:24 `marker_cols < 2` -> `marker_cols > 2` in `Store::probe` | (b) | Every probe test that drops the 0026 marker also drops the view, so `!live_view` refuses it anyway. **S-1**: in `project_lifecycle_migrations.rs`, the probe refuses a scratch schema migrated to head that keeps `live_projects` but lacks `projects.deleted_by`, naming "deleted_at/deleted_by found 1 of 2". |
| project_lifecycle.rs:220:5 `is_detach_backstop -> true` | (b), low | The only effect is the `tracing::error!` "the detach backstop refused a live project row" in `delete_team_if_empty`. The error itself is returned unchanged. **S-2** below also covers the next row. |
| project_lifecycle.rs:222:13 `&&` -> `\|\|` in `is_detach_backstop` | (b), low | **S-2**: a team delete whose detach fails with any other error, for example a 23514 on another projects CHECK, is not logged as the backstop. A legacy row breaking a `NOT VALID` name-rule CHECK is re-checked on that UPDATE. Assert on captured tracing output. |

### foundry-services (0)

`Services::list_deleted_projects -> Ok(vec![])` survived its own crate's tests
but was killed by pl-03 through the CLI.

### foundry-app (1)

| Mutant | Class | Reason / test that should exist |
|---|---|---|
| admin_cli.rs:633:18 guard `!v.is_empty()` -> `true` in `run_with_services` | (b), low | The mutant accepts `DATABASE_URL=""`. The connect then fails, still exit 3, but with "could not connect to DATABASE_URL" instead of "DATABASE_URL is required …". **A-1**: `foundry doctor restore-project <uuid>` (and `list-deleted-projects`) with `DATABASE_URL` set to the empty string exits 3, saying "DATABASE_URL is required to reach the live database". |

### xtask/src/check_arch.rs (108 = 81 gaps + 22 equivalent + 5 unreachable)

The `*_shipped_tree_passes` tests only show that the real tree is not flagged.
A scanner that misses more still passes them, so the scanner's lexing and
false-negative paths need direct unit tests. Line numbers refer to HEAD
`4cc3000`.

**(b) real gaps, grouped by the test that kills them:**

| Id | Test that should exist (xtask unit test) | Mutants it kills |
|---|---|---|
| X-1 | `rust_string_literals("let a = 1 / 2; /* SELECT x\n /* nested */ FROM projects */\nlet q = \"SELECT 1\";")` returns exactly `[(3, "SELECT 1")]`: a nested, multi-line block comment yields no literal, code after it is still scanned, the line is right, and a division opens no comment. No Rust-scanner test contains a `/* */` today. | 1269:36, 1275:24, 1282:24, 1286:54, 1289:35, 1292:39 x2, 1293:35 x2, 1294:42, 1299:38 x2, 1299:55 (13) |
| X-2 | A source that ends in a `// comment` with no trailing newline scans without panicking. | 1277:29 `<`->`<=` (1) |
| X-3 | A raw string holding a double quote and a backslash is one literal with the exact text: `r#"SELECT "projects".id FROM t WHERE a ~ '\d'"#`, then `(r#"…"#`, then ` br#"…"#`. Also check `RustSource::code` blanks its body. | 1264:59, 1328:24 (false), 1328:31, 1328:34, 1328:52 (`/`), 1328:58, 1328:69 x2, 1328:74, 1329:42 x2, 1329:75, 1330:36 x2, 1330:40 x2 (16) |
| X-4 | A violation in a literal after a multi-line raw string is reported on its own line. Today's test checks a violation inside the raw string. | 1343:30, 1343:54 (2) |
| X-5 | Char literals: `let q = '"'; let e = '\"'; let u = '\u{7b}'; let b = '{'; let s = "SELECT 1";` yields exactly one literal, and a `'{'` inside a writer's body does not break its fn span or guard detection. | 1356:25, 1357:23, 1358:29 (`==`, `>`), 1358:55, 1359:27, 1363:25 (false), 1363:37 x2, 1363:42, 1364:40 x2, 1364:47 x2, 1365:23 (15) |
| X-6 | `RustSource::code` blanks a literal's contents: a writer fn whose SQL literal contains `}` or `fn` keeps its whole body (guard and write both found). | 1255:28 (1) |
| X-7 | live-reads: `DELETE FROM issues USING projects p WHERE …` is flagged. `projects` stands after USING, not FROM/JOIN, so only `holds_sql_statement` makes it SQL. | 7196:9 (1) |
| X-8 | write-guard: a guarded writer whose write runs on `&self.pool` after the guard is flagged as autocommit, both for an inline literal and for a writing `const`. Add a case with two pooled writes, so a toggled flag is caught. | 7473:34 x2, 7481:27, 7484:38 x2, 7484:84 x2, 7882 `runs_on_the_pool -> false` (8) |
| X-9 | write-guard: a pool writer with the same name as a transaction-taking helper elsewhere (another impl) must still hold the guard. | 7509:31 (1) |
| X-10 | write-guard: `let live = lock_live_project(&mut tx, id).await?; tracing::debug!(live); if !live { return … }` still gates the write. The binding is used before its test; the mutant loops forever or misses it. | 7573:27 (1) |
| X-11 | write-guard: `if live { … } else { return … }` followed by a write after the if/else gates that write. | 7642:37 x2, 7642:41 (3) |
| X-12 | write-guard: a guard nested in another call's arguments, `let ok = wrap(lock_live_project(&mut tx, id).await?);`, is not a binding and gates nothing. | 7683:37, 7683:57 (2) |
| X-13 | write-guard: a guard that takes a pool connection, `lock_live_project(&mut self.pool.begin().await?, id)`, is not a guard. This mirrors DELIVER pin 02-02, which only the shipped-tree test kills. | 7889 `holds_the_transaction -> true` (1) |
| X-14 | write-guard: a generic writer `fn write<'a, T: Into<Uuid>>(…)` is found as a fn and must hold the guard. | 7816:24, 7817:23 x2, 7817:38, 7817:47, 7817:61, 7818:26 (7) |
| X-15 | write-guard: the violation names the fn's own 1-based line. | 7842:52 (1) |
| X-16 | write-guard: a write inside a nested fn declared near the end of a long outer fn is attributed to the nested fn. | 7855:40 `-`->`+` (1) |
| X-17 | write-guard: issue create's attempt with a non-SQL literal (a constraint name) before its counter `UPDATE live_projects` stays exempt. | 7867 `is_sql -> true` (1) |
| X-18 | interpolated DML: `format!("UPDATE {table} SET x = 1")` (inline capture, no arguments) is flagged. | 7942:40 (1) |
| X-19 | interpolated DML: `concat!("UPDATE ", "lanes ", "SET x = 1")` (literals only) is not flagged. | 7958:55, 7958:74, 7958:79 (3) |
| X-20 | interpolated DML: DDL `format!("… REFERENCES t ON DELETE CASCADE, {col} …")` and a word `insert` not followed by INTO are not DML targets. | 7980:25, 7981:25 (2) |

Total (b) in check_arch.rs: 81.

**(a) equivalent (22):**

- 1264:54 `||`->`&&` in `is_ident`, and 1328:24 guard -> `true`, 1328:26
  `i == 0` -> `!=`, 1328:52 `chars[i-1]` -> `chars[i+1]`. These change only
  how an `r` preceded by an identifier character is read. In valid Rust that
  is never followed by `"` or `#"`, except `br`/`cr` prefixes, and the
  unmutated guard already admits `br`. The skip arm then does exactly what the
  default arm does.
- 1320:44 `start + 1` -> `start * 1`: this also blanks a string's opening `"`
  in `code`. No Rust-side rule reads `"` from `code` (literals come from
  `literals`).
- 1363:25 guard -> `true`: lifetimes (`'a>`, `'static`) are skipped three
  characters at a time. The third character after a lifetime's `'` is never a
  quote or a comment opener in valid Rust.
- 7195:9 `||`->`&&` (the `INSERT INTO` arm of `holds_sql_statement`): an
  INSERT-only statement can name `projects` only as its own target, which is
  exempt. Any other mention needs a SELECT, which the other arms catch.
- 7225:74, 7447:78, 7591:17, 7591:41, 7591:58, 7854:36, 7854:55 (`<`->`<=`):
  the compared offsets are a `{`, `}`, `;` or keyword position on one side and
  a literal, word or call start on the other, so they are never equal.
- 7234:50 `semi + 1` -> `semi * 1`: the span ends on the `;` instead of after
  it, and no write or literal ever starts at a `;`.
- 7855:40 `-`->`/`: for nested spans (a, d) ⊃ (b, c), `d/a > c/b`, so the
  innermost span is still the minimum.
- 7570:67 and 7626:79 (`close + i` / `condition_end + i` -> `-`): they find
  the same statement end / the same `{`. Searching from a few bytes earlier
  only re-meets the `let` head or the condition text, which hold no
  `if`/`match` test and no `return`.
- 7801:50 `-`->`/`, 7811:20 `+=`->`-=` / `*=`, 7823:66 `+`->`-`: under rustfmt
  there is exactly one space after `fn` and none between a name and its `(`
  or `<`. So the ratio is 1, the whitespace loop never runs, and `i == 0`.
  These would matter only for unformatted code; `cargo fmt --check` is part of
  CI.

**(c) unreachable (5):**

- 1285:29, 1311:29, 1342:29, 1358:29 `<`->`<=`: these differ only on an
  unterminated block comment, string, raw string or char literal at
  end-of-file, which is not compilable Rust.
- 2391:9 `delete match arm Some(b'[')` in `matching_brace`: no caller passes
  `[`. The callers are `css_regions` on `{` and the Rust scanners on `{` / `(`.

### Timeouts (39, check_arch.rs)

Every timeout is an infinite loop under the 20 s cap (the baseline is 1.7 s).
By function: 22 in `RustSource::scan`, 3 `named_items`, 2
`holds_sql_statement`, 2 `projects_reads_in`, 2 `sql_writes_of`, 1 each in
`fn_spans`, `items_within`, `StoreFn::read` and `writing_helpers`, and 4
whole-function replacements: `let_bound_name -> Some(String::new())`, and
`item_writes -> (vec![String::new()], …)` x3, each of which makes an empty
word searched forever. They are not in the kill rate. A suite that hangs is noticed,
so they are treated as detected in the alternative rate above.

## Verdict

- **foundry-store, foundry-services, foundry-app: PASS.** Every file with a
  viable mutant is at or above 80% after the cross-crate and acceptance
  re-checks:
  - store `project_lifecycle.rs` 90.5%;
  - store `lib.rs` 97.9%;
  - `admin_cli.rs` 96.9%;
  - every other file 100%.
  
  Three files have no viable mutant on the changed lines (store `teams.rs`,
  services `teams.rs`, app `views.rs`).
- **xtask/src/check_arch.rs: WARN (76.2% raw, below the 80% gate).** It
  reaches 81.0% once the 27 equivalent or unreachable survivors leave the
  denominator, and 82.6% with timeouts counted as detected. The 81 real gaps
  are the 20 test suggestions X-1..X-20 above, routed to the acceptance
  designer. X-1, X-3, X-5 and X-8 alone account for 52 of them.
- **SQL: all five re-pins killed.** These are 01-01 probe, 02-01
  `FOR KEY SHARE`, 03-01 tiebreak, 03-01 re-check and 04-03 k == 0.
- **Store, services and app real gaps for the acceptance designer:** S-1, S-2
  and A-1, all small. No test or production code was changed by this run.
