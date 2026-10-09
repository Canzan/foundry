# Mutation report: name-db-checks

Strategy: per-feature (CLAUDE.md), gate >= 80% kill rate on every modified Rust
production file. The rule itself lives in SQL (migrations 0019 and 0020), which
cargo-mutants cannot mutate, so the SQL is covered by a hand-mutation table.

Status at step 03-02: the per-step Rust results are below. The feature-wide
`cargo mutants --in-diff` pass over all three Rust files runs next, from the
orchestrator, and its result is appended here.

## Rust (cargo-mutants, per step)

| File | Step | Viable mutants | Caught | Kill rate | Notes |
|---|---|---|---|---|---|
| `xtask/src/check_arch.rs` (`name-rule-legacy-seam`) | 01-01 | 27 | 27 | 100% | `--in-diff` over the step's diff |
| `crates/foundry-store/src/name_rule_legacy_seam.rs` | 01-02 / 02-03 | 3 | 3 | 100% | 3 of 3 viable mutants caught (store seam tests, e.g. `the_legacy_seam_stores_exactly_the_callers_row_and_leaves_both_triggers_enabled`) |
| `crates/foundry-store/src/lib.rs` (`run_boot_migrations_from_dir`, test-support boot entry) | 03-01 | 1 | 1 | 100% | caught after commit 7fb29e9 made the previous-release boot assert what it applied and what it found |

No surviving Rust mutant is waived.

## SQL (hand mutations)

Method: each mutation was applied as a temporary edit to the migration file,
the named test was run, and the file was restored. After each run the file's
sha1 matched HEAD (`0019`: `766c8e3c…`, `0020`: `3a4f8110…`). "Store" tests are
in `crates/foundry-store/tests/`; "ndc" is the `@ndc` acceptance lane
(`name-db-checks.feature`).

| # | Mutation | Killed by | Evidence |
|---|---|---|---|
| 1 | Workspace trigger cap `'24'` -> `'23'` | store `the_triggers_fire_only_on_new_name_writes_with_each_tables_cap_and_say_what_they_mirror` (trigger definition carries the cap) | 03-02, run now |
| 2 | Workspace trigger cap `'24'` -> `'25'` | same store test | 03-02, run now |
| 3 | Project trigger cap `'256'` -> `'257'` | ndc: "the database must refuse the write under `projects_name_max_256_chars`", answered Accepted | 02-02 (d32449e) |
| 4 | Project trigger cap `'256'` -> `'255'` | ndc: the refusal named `projects_name_max_255_chars`, not `…_256_chars` | 02-02 (d32449e) |
| 5 | Length arm `>` -> `>=` (in the verdict function) | 0019's apply-time self-check (`24 astral judged max_24_chars, expected NULL`), so every store and ndc test fails at migrate; the parity boundary pairs at 23/24/25 and 255/256/257 are the second line | 03-02, run now (store `workspace_boundary_code_points_and_lengths_match_exactly`) |
| 6 | One White_Space point dropped from the trim set (U+1680) | store `workspace_boundary_code_points_and_lengths_match_exactly`: `"\u{1680}Globex"` expected `trimmed`, database NULL | 03-02, run now |
| 7 | One refused-range bound shifted (U+202A-U+202E -> U+202A-U+202D) | same store test: `"Ab\u{202E}cd"` expected `no_control_chars`, database NULL | 03-02, run now |
| 8 | Precedence swapped (length arm before the refused-character arm) | same store test: a 51-character name with a TAB judged `max_24_chars`, expected `no_control_chars` | 03-02, run now; also killed by ndc in 01-03 (5ac0ff8, "precedence reordered") |
| 9 | `WHEN (OLD.name IS DISTINCT FROM NEW.name)` dropped, workspaces | store `writes_that_do_not_change_a_legacy_name_never_meet_the_rule` (a same-value write on a legacy workspace refused 23514) | 03-02, run now |
| 10 | `WHEN` dropped, projects | ndc: "rewriting a stored name unchanged must never meet the rule (D6, DDD-5)" | 02-03. It survived the 02-02 lane (18/18), which had no same-value scenario yet; 02-03's scenario kills it |
| 11 | `UPDATE OF name` dropped (plain `BEFORE UPDATE`), `WHEN` kept | **equivalent**: ndc 55/55 green | 02-03. While `WHEN` stays, an update that does not change the name never reaches the function, so the column list only saves a call. Dropping both (trigger on every update) is killed: the legacy project's issue counter update is refused 23514 (02-03, and again in 5d8a855) |
| 12 | Insert trigger dropped, workspaces | store `a_bad_name_write_is_refused_per_arm_and_table_and_nothing_changes` (fails its gate: 3 of 4 triggers) | 03-02, run now |
| 13 | Insert trigger dropped, projects | ndc: refusals under `projects_name_trimmed`, `…_max_256_chars`, `…_no_control_chars` answered Accepted | 02-02 (d32449e) |
| 14 | All triggers dropped (each table) | ndc refusal scenarios answered Accepted | 01-03 (5ac0ff8), 02-02 (d32449e) |
| 15 | Stricter refused set (ZWSP/ZWJ added) | ndc fit-name guards (US-NDC-01 guards 4/5: D3, never stricter) | 01-03 (5ac0ff8) |
| 16 | Unique index on `name` (01-03) / `lower(name)` (02-02) | ndc fit-name guards; in 02-02: "a name the app would accept must be accepted by the database (D3: never stricter)" | 01-03 (5ac0ff8), 02-02 (d32449e) |
| 17 | SQLSTATE drift (not 23514) | ndc refusal scenarios (the oracle compares the SQLSTATE) | 01-03 (5ac0ff8) |
| 18 | Byte counting instead of code points | ndc US-NDC-01 scenarios (01-03); for the verdict function also the apply-time self-checks (24 astral in 0019; 256 x U+00E9 / U+65E5 / astral in 0020) | 01-03 (5ac0ff8) |
| 19 | A DETAIL carrying the row's name added to the RAISE | ndc: "the refusal must never carry a DETAIL: row values are not logged (DDD-4)" | 5d8a855 |
| 20 | HINT text no longer names the mirrored type | ndc: "the refusal must carry the one-line HINT naming the mirrored type (DDD-4)" | 5d8a855 |
| 21 | Counter-aware `WHEN` (fires on the issue counter) | ndc refusal of a hand rename to another bad name | 02-02 (d32449e) |
| 22 | UTF8 guard removed (`IF false`) | **survived** `migration_0019_refuses_a_database_that_is_not_utf8` | 03-02, run now. See below |

### Mutation 22: the UTF8 guard

With the guard removed, 0019 still fails on a SQL_ASCII database, so the test
(refused, no function left, version 19 not recorded) still passes. A direct
psql run of the mutated file against a SQL_ASCII database shows why: the
verdict function's `\u` escapes above U+007F do not convert
(`ERROR: conversion between UTF8 and SQL_ASCII is not supported`), inside the
same transaction. Refusing the database is therefore over-determined. What
the guard adds is the readable refusal (SQLSTATE 0A000, "needs a UTF8
database", the HINT), and no test pins that. This is a test-strength finding
for nw-acceptance-designer: assert the refusal's SQLSTATE or message in
`migration_0019_refuses_a_database_that_is_not_utf8`. It is not a gap in the
rule itself.

## How the SQL arms are pinned

- **Arm-exact parity** over 10,000 generated names per table at each cap
  (`workspace_verdicts_match_workspace_name_arm_for_arm_over_10k_generated_names`,
  `project_verdicts_match_project_name_arm_for_arm_over_10k_generated_names`):
  the database's arm equals `WorkspaceName::try_new` / `ProjectName::try_new`.
- **Exact boundary pairs** (`workspace_/project_boundary_code_points_and_lengths_match_exactly`):
  every White_Space point at the edge and inside, every refused-range bound and
  its neighbours, allowed format characters, 23/24/25 and 255/256/257 code
  points (multi-byte and astral), and precedence cases.
- **Refusal contract and trigger shape** (`name_rule_in_database.rs` and the ndc
  refusal oracle): SQLSTATE 23514, constraint `<table>_name_<arm>`, table,
  column, schema, HINT, no DETAIL; four triggers enabled, each table's cap,
  `UPDATE OF name` with the `WHEN` guard.
- **Apply-time self-check** in each migration, and the drift test
  `the_apply_time_self_check_refuses_a_verdict_function_that_passes_everything`.

## Never log names

`grep -nE 'MESSAGE|HINT|DETAIL|RAISE'` over 0019 and 0020: the refusal's
MESSAGE interpolates only `TG_TABLE_NAME` and the constraint name, the HINT
only the type name and `TG_ARGV[0]` (the cap), and there is no DETAIL. The
self-check messages interpolate fixed probe labels. `NEW.name` appears only as
the verdict function's argument and in the `WHEN` guard. The seam returns a
bare `sqlx::Error` and formats only table and trigger names.
