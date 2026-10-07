# Mutation report: project-name-rule (DELIVER Phase 5)

**Date**: 2026-10-07
**Tool**: cargo-mutants 25.3.1 (default copy mode, never `--in-place`)
**Scope**: the production lines this feature changes. They are selected with `--in-diff` over
`git diff 7842e53..HEAD -- crates/foundry-core/src crates/foundry-services/src crates/foundry-app/src crates/foundry-store/src xtask/src`
(base `7842e53` = DELIVER roadmap; HEAD = `a728447`):
- `crates/foundry-core/src/name_chars.rs` (new): the shared refused-character predicate
  `is_refused_name_char`.
- `crates/foundry-core/src/project_name.rs` (new): `ProjectName::try_new`, `as_str`,
  `derived_slug`, `ensure_unique_among`, `Display`, `MintedSlug::as_str` and `mint_project_slug`.
- `crates/foundry-core/src/workspace_name.rs`: now delegates to `is_refused_name_char`.
  `foundry-core/src/lib.rs` only adds `mod`/`pub use` lines, so it has no mutants.
- `crates/foundry-services/src/projects.rs`: `classify_rename` (now `ProjectName`-based),
  `create_project`, `create_project_with_sibling_reads` (test seam), `create_with_retry`,
  `attempt_create`, and the `Services::create_project` delegate.
- `crates/foundry-store/src/lib.rs`: `insert_project` now maps the slug unique index to
  `ProjectInsertError::DuplicateSlug`.
- `crates/foundry-app/src/projects.rs`: `submit_create` (name rule before the use-case).
  `crates/foundry-app/src/instance_admin.rs`: `submit_project_rename` (renders
  `ProjectNameError`'s `Display`).
- `xtask/src/check_arch.rs`: the `project-name-one-source` guard, and the generalised
  `scan_one_source`/`copy_violations_in` it now shares with `workspace-name-one-source`.

`--in-diff` also generates whole-function mutants for `run`, `source_violations`,
`check_workspace_name_one_source`, `WorkspaceName::try_new`, `submit_create` and
`submit_project_rename`, whose bodies this feature only touches. They stay in the denominator,
which is the stricter choice.

Not mutated: the askama templates, the `const`s (`PROJECT_NAME_MAX_CHARS`,
`FALLBACK_SLUG_ATTEMPTS`) and the string literals (the `ProjectNameError` copy). The acceptance
lane pins the copy byte for byte (`Then both doors refuse it saying "…"`).

**Gate**: kill rate ≥ 80% PASS, 70–80% WARN, < 70% FAIL (`CLAUDE.md`, per-feature).

## Result

| Metric | Value |
|---|---|
| Mutants generated | 72 |
| Unviable (do not compile, excluded) | 9 |
| Viable | 63 |
| Killed by the package's own tests | 57 (54 first run + 3 after the survivor-closing tests, see Survivors re-check) |
| Killed on re-check (dependent-crate tests and/or the `pnr` acceptance lane) | 5 |
| Survived | 1 (`xtask/src/check_arch.rs:4577:65` `i * 1`, equivalent, counted as missed) |
| Timeouts | 0 |
| **Kill rate** | **98.4%** (62/63 viable). 100% (62/62) if the equivalent mutant is excluded. Package tests alone: 90.5% (57/63). First run, before the survivor-closing tests: 93.7% (59/63) |
| **Gate verdict** | **PASS** |

The 5 re-check kills split as: 2 foundry-app (acceptance only), 1 foundry-store (foundry-services
tests and acceptance), 2 foundry-core `MintedSlug::as_str` (foundry-services tests and
acceptance). The core pair is outside the re-check scope (app/services/store), so it is counted
separately. Without it the rate is 60/63 = 95.2%, still PASS.

Every kill below was checked in its log. Each one failed on a test assertion in a test that
exercises the mutated code. None was "caught" by an infrastructure error:
- The package mutant logs contain no `PortNotExposed`, `SSLRequest`, sqlx `Protocol(…)` or
  `Connection reset`.
- The acceptance re-check logs contain 26–28 `curl: (56) Recv failure: Connection reset by peer`
  lines each. These come from the browser harness polling the Selenium container. The green
  baseline shows 27 of them, so they are not a failure cause.
- One re-check run (store `insert_project -> Ok(())`) hit the known sqlx
  `Protocol("unknown message type: '\0'")` flake in one Background step (`connect to base
  postgres`). That one failed scenario was not counted. The other 19 failures in that run are
  product assertions ("a create must add exactly one project; added []"), and the same mutant
  also fails 6 foundry-services tests, so it is a kill without a rerun.
- `project_name.rs:135:40` (delete `!` in `mint_project_slug`) was caught after 394 s: the
  fallback search walks `2u32..` until debug-mode overflow panics ("attempt to add with
  overflow") in `a_fallback_address_is_the_lowest_free_candidate` and two siblings. It is a
  real test failure, not a cargo-mutants timeout. In a release build the same mutant would
  wrap and spin forever.

## Per-file table

| File | Total | Killed | Survived | Timeout | Unviable | Kill rate |
|---|---|---|---|---|---|---|
| `crates/foundry-core/src/name_chars.rs` | 3 | 3 | 0 | 0 | 0 | 100% (3/3) |
| `crates/foundry-core/src/project_name.rs` | 20 | 18 (16 package + 2 re-check) | 0 | 0 | 2 | 100% (18/18) |
| `crates/foundry-core/src/workspace_name.rs` | 1 | 0 | 0 | 0 | 1 | no viable mutants |
| `crates/foundry-services/src/projects.rs` | 6 | 0 | 0 | 0 | 6 | no viable mutants |
| `crates/foundry-store/src/lib.rs` | 1 | 1 (0 package + 1 re-check) | 0 | 0 | 0 | 100% (1/1) |
| `crates/foundry-app/src/projects.rs` | 1 | 1 (0 package + 1 acceptance) | 0 | 0 | 0 | 100% (1/1) |
| `crates/foundry-app/src/instance_admin.rs` | 1 | 1 (0 package + 1 acceptance) | 0 | 0 | 0 | 100% (1/1) |
| `xtask/src/check_arch.rs` | 39 | 38 (35 first run + 3 re-check) | 1 (equivalent) | 0 | 0 | 97.4% (38/39) |
| **Total** | **72** | **62** | **1** | **0** | **9** | **98.4%** (62/63) |

## Procedure and exact commands

`S` is `/private/tmp/claude-501/-Users-jeffbailey-Projects-canzan-foundry/9298978f-…/scratchpad/mut-pnr`.

**Safety.** Nothing wrote to git in the working repository. No `checkout`, `restore`, `stash`,
`reset` or `clean` was run, and cargo-mutants never ran `--in-place`. cargo-mutants mutates its
own copy of the tree under `$TMPDIR`. The re-check applied mutants to a **separate `git clone`**
(`$S/tree`, own `target/`, no `.env` or secrets copied; only the tracked `.env.example` exists
in a clone). It used a `cp` backup and a `cmp` restore against the working tree. The clone was
deleted at the end, and the working tree was never mutated. `FOUNDRY_STAMP_*` was never set
(both scripts `unset` them).

1. **Snapshot.**
   ```sh
   shasum -a 256 crates/foundry-core/src/{lib,name_chars,project_name,workspace_name}.rs \
     crates/foundry-services/src/projects.rs crates/foundry-app/src/{projects,instance_admin}.rs \
     crates/foundry-store/src/lib.rs xtask/src/check_arch.rs > $S/sha-before.txt
   docker ps -q | sort > $S/docker-before.txt
   ```
2. **Scope diff** (read-only git):
   ```sh
   git diff 7842e53..HEAD -- crates/foundry-core/src crates/foundry-services/src \
     crates/foundry-app/src crates/foundry-store/src xtask/src > $S/pnr.diff
   cargo mutants --in-diff $S/pnr.diff --list   # 72 mutants
   ```
3. **Per-package runs**, one after another (`$S/chain.sh`). Each was run as
   `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false timeout 5400 cargo mutants --in-diff $S/pnr.diff … --package <pkg> --output $S/<pkg> -j 2 --timeout 600`,
   against all of the package's tests:
   - **foundry-core**: `--file` for `name_chars.rs`, `project_name.rs`, `workspace_name.rs`
     (lib + `tests/project_name.rs` + `tests/workspace_name.rs`). Baseline green (5.1 s build +
     0.3 s test). 24 mutants in 6m 48s: 19 caught, 2 missed, 3 unviable.
   - **xtask**: `--file xtask/src/check_arch.rs`. 39 mutants in 28 s: 35 caught, 4 missed.
   - **foundry-store**: `--file crates/foundry-store/src/lib.rs` (scoped to the diff; every
     `tests/*.rs` over testcontainers Postgres). Baseline green (20.6 s build + 39.2 s test).
     1 mutant in 1m 47s: 1 missed.
   - **foundry-services**: `--file crates/foundry-services/src/projects.rs` (lib property tests
     plus every `tests/*.rs`, including `create_project_use_case` and `rename_project_use_case`
     with `test-support` via the self dev-dependency). Baseline green (15.4 s build + 16.8 s
     test). 6 mutants in 45 s: 6 unviable.
   - **foundry-app**: `--file` for `projects.rs` and `instance_admin.rs` (lib unit tests,
     `backup_verify_fail_closed`, `csrf_middleware`, `password_less_account_doors`). Baseline
     green (30.9 s build + 17.1 s test). 2 mutants in 1m 40s: 2 missed.
4. **Re-check of the package-level survivors** in foundry-app, foundry-store and (outside the
   brief's scope, for completeness) foundry-core. The script `$S/seed.sh` applied each
   survivor's **exact cargo-mutants diff** (`$S/<pkg>/mutants.out/diff/*.diff`) with `patch` in
   `$S/tree`. For each run it:
   - backs up the target file with `cp` and applies the patch;
   - rebuilds the `foundry` binary (`CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false cargo build -p foundry-app --bin foundry`)
     and the acceptance test binary (`--no-run`);
   - warms the binary (three `foundry --version` execs);
   - for store and core mutants, runs `timeout 1800 cargo test -p foundry-services --tests`
     (the crate that consumes them);
   - runs `FOUNDRY_ACCEPTANCE_TAGS=pnr timeout 1800 cargo test -p foundry-acceptance --test acceptance`;
   - restores with `cp` and checks with `cmp` against the working tree.

   The runs went one after another, with no other lane running. A positive tag selection
   includes the `@needs-browser` scenarios, which run in real Chrome through the Docker
   Selenium harness. Every mutant died on `pnr`, so the neighbour lanes (`iapr`, `us-07`,
   `us-r01`) were not needed.
   - **Baseline pnr (unmutated clone): 74/74 scenarios, 400/400 steps.**

   No rerun was needed (see the one flake above).
5. **Restore proof.** All 5 seeds ended with `RESTORED_CMP_OK`. After the whole run:
   - `diff $S/sha-before.txt $S/sha-after.txt` is identical (hashes below);
   - `docker ps -q` before and after is identical;
   - `git status --short` is empty except for this report.

## Every mutant and its disposition

### foundry-core

All package kills are by `crates/foundry-core/tests/project_name.rs` (the `workspace_name.rs`
suite also runs).

| Mutant | Outcome | Killing tests |
|---|---|---|
| `name_chars.rs:11:5` `is_refused_name_char -> true` | killed | `a_name_without_an_address_takes_the_key_prefix_in_lower_case`, `an_empty_derived_address_is_never_an_address_match`, `a_case_insensitive_name_match_alone_is_not_unique` |
| `name_chars.rs:11:5` `-> false` | killed | `empty_comes_before_control_and_control_before_length`, `the_line_and_paragraph_separators`, `the_c0_and_c1_control_boundaries` |
| `name_chars.rs:12:9` `\|\|` → `&&` | killed | `the_c0_and_c1_control_boundaries`, `the_bidi_embedding_override_and_isolate_boundaries` |
| `project_name.rs:50:36` `>` → `==` | killed | `the_cap_is_256_scalars`, `the_length_gate_sits_at_256_trimmed_scalars`, `empty_comes_before_control_and_control_before_length` |
| `project_name.rs:50:36` `>` → `>=` | killed | `the_cap_is_256_scalars`, `the_length_gate_sits_at_256_trimmed_scalars`, `an_accepted_name_displays_exactly_as_stored` |
| `project_name.rs:50:36` `>` → `<` | killed | `a_six_letter_key_and_a_legacy_empty_address_are_handled`, `the_c0_and_c1_control_boundaries` |
| `project_name.rs:57:9` `ProjectName::as_str -> ""` / `"xyzzy"` (2) | killed | `the_bidi_embedding_override_and_isolate_boundaries`, `other_format_characters_stay_allowed`, `the_seeded_sandbox_project_name_passes_the_rule` |
| `project_name.rs:62:9` `derived_slug -> String::new()` | killed | `the_seeded_sandbox_project_mints_its_shipped_address`, `a_name_with_its_own_address_keeps_it`, `a_derived_address_match_alone_is_not_unique` |
| `project_name.rs:62:9` `derived_slug -> "xyzzy".into()` | killed | `a_name_without_an_address_takes_the_key_prefix_in_lower_case`, `a_name_with_its_own_address_keeps_it` |
| `project_name.rs:78:9` `ensure_unique_among -> Ok(())` | killed | `a_case_insensitive_name_match_alone_is_not_unique`, `a_derived_address_match_alone_is_not_unique` |
| `project_name.rs:80:27` delete `!` (empty-address skip) | killed | `an_empty_derived_address_is_never_an_address_match`, `a_derived_address_match_alone_is_not_unique` |
| `project_name.rs:82:33` `==` → `!=` (name arm) | killed | `an_empty_derived_address_is_never_an_address_match`, `distinct_names_and_addresses_are_unique_and_no_siblings_is_unique` |
| `project_name.rs:82:44` `\|\|` → `&&` | killed | `a_case_insensitive_name_match_alone_is_not_unique`, `a_derived_address_match_alone_is_not_unique` |
| `project_name.rs:82:60` `&&` → `\|\|` (address arm guard) | killed | `an_empty_derived_address_is_never_an_address_match`, `distinct_names_and_addresses_are_unique_and_no_siblings_is_unique` |
| `project_name.rs:82:69` `==` → `!=` (address arm) | killed | `a_derived_address_match_alone_is_not_unique`, `distinct_names_and_addresses_are_unique_and_no_siblings_is_unique` |
| `project_name.rs:94:9` `Display::fmt -> Ok(Default::default())` | killed | `an_accepted_name_displays_exactly_as_stored` |
| `project_name.rs:131:8` delete `!` (derived vs fallback) | killed | `the_seeded_sandbox_project_mints_its_shipped_address`, `a_six_letter_key_and_a_legacy_empty_address_are_handled`, `a_name_with_its_own_address_keeps_it` |
| `project_name.rs:135:40` delete `!` (`is_free`) | killed (394 s, overflow panic) | `a_fallback_address_is_the_lowest_free_candidate`, `a_name_without_an_address_takes_the_key_prefix_in_lower_case`, `a_six_letter_key_and_a_legacy_empty_address_are_handled` |
| `project_name.rs:111:9` `MintedSlug::as_str -> ""` | missed by package → **killed on re-check** | foundry-services: all 6 `create_project_use_case` tests (e.g. `a_stale_fallback_address_is_retried_to_the_next_free_one`, `:207`, `left: KeyFallback("jp")`: the stored `""` makes the retry mint `jp` again). `pnr`: 74 scenarios (59 passed, 15 failed): "every new project must have a non-empty address (D15, KPI-5)" (14) |
| `project_name.rs:111:9` `MintedSlug::as_str -> "xyzzy"` | missed by package → **killed on re-check** | foundry-services: the same 6 tests. `pnr`: 74 scenarios (63 passed, 11 failed): "the new project's address", `left: "/team/backend/project/xyzzy"` (fallback-address and Latin-address outlines) |
| `project_name.rs:43:9` `ProjectName::try_new -> Ok(Default::default())` | unviable | `ProjectName` has no `Default` |
| `project_name.rs:130:5` `mint_project_slug -> Default::default()` | unviable | `MintedSlug` has no `Default` |
| `workspace_name.rs:39:9` `WorkspaceName::try_new -> Ok(Default::default())` | unviable | `WorkspaceName` has no `Default` |

**Why `MintedSlug::as_str` misses in foundry-core.** The core suite compares `MintedSlug` values
by `PartialEq` (`MintedSlug::KeyFallback("jp-2".into())`), never through `as_str`. Its only
readers are the use-case (`insert_project(…, minted.as_str(), …)`) and the create door's
redirect, both in other crates. A one-line core test,
`assert_eq!(MintedSlug::KeyFallback("jp".into()).as_str(), "jp")` (and the `Derived` twin),
would kill both at package level.

### foundry-services (`projects.rs`)

| Mutant | Outcome | Reason |
|---|---|---|
| `:72:5` `classify_rename -> Ok(Default::default())` | unviable | `RenameDecision` has no `Default` |
| `:139:9` `Services::create_project -> Ok(Default::default())` | unviable | `CreatedProject` has no `Default` |
| `:184:5` `create_project -> Ok(Default::default())` | unviable | same |
| `:203:5` `create_project_with_sibling_reads -> Ok(Default::default())` | unviable | same |
| `:215:5` `create_with_retry -> Ok(Default::default())` | unviable | same |
| `:237:5` `attempt_create -> Ok(Default::default())` | unviable | `Attempt` has no `Default` |

cargo-mutants has no operator to mutate in this file's new logic: the retry is a `for` over a
range with a `match`, and the outcome mapping is a `match` on `(Result, &MintedSlug)` with no
wildcard arm. So it generates only whole-function replacements, and every return type lacks
`Default`. The logic is checked by hand under "Structural checks" below.

### foundry-store (`lib.rs`)

| Mutant | Outcome | Killing test (assertion) |
|---|---|---|
| `:1570:9` `insert_project -> Ok(())` | missed by package → **killed on re-check** | foundry-services: all 6 `create_project_use_case` tests (e.g. `a_stale_fallback_address_is_retried_to_the_next_free_one`, `left: KeyFallback("jp")`: no row is written and no `DuplicateSlug` is raised, so no retry happens). `pnr`: 74 scenarios (54 passed, 20 failed): "a create must add exactly one project; added []" (15), plus one Background flake not counted |

No foundry-store test calls `insert_project`; the method is exercised only through
`create_project` in foundry-services and the doors. A store-level test (insert twice with the
same slug → `DuplicateSlug`; same key → `DuplicateKey`; a success writes the row and its seed
lanes) would kill this at package level and pin the new `DuplicateSlug` mapping directly.

### foundry-app

| Mutant | Outcome | Killing test (assertion) |
|---|---|---|
| `projects.rs:115:5` `submit_create -> Default::default()` | missed by package → **killed by acceptance** | `pnr`: 74 scenarios (24 passed, 50 failed): "the create must be refused with the form shown again; body = \"\"" (15), "the create must redirect to the new board; got 200 OK with \"\"" (14), "both doors must give the same refusal (KPI-2)" (11), `left: Some(200)`, `right: Some(422)` |
| `instance_admin.rs:492:5` `submit_project_rename -> Default::default()` | missed by package → **killed by acceptance** | `pnr`: 74 scenarios (33 passed, 41 failed): "a validation refusal must be 422 (D6); body = Some(\"\")" (12), "both doors must give the same refusal (KPI-2)" (11), "the rename must answer with the row and no error; got \"\"" (6) |

**Why the foundry-app mutants miss at package level.** This is the same pattern as
instance-workspace-name-rule and instance-admin-workspace-rename. Both handlers are async axum
handlers over `AppState` (a live store, session and CSRF). The package has no unit test for
either. The acceptance lane drives the real binary over Postgres and real Chrome and kills both
on product assertions.

### xtask (`xtask/src/check_arch.rs`)

35 of 39 killed by `check_arch` unit tests.

| Mutant | Outcome | Killing tests |
|---|---|---|
| `:84:5` `source_violations -> vec![]` / `vec![String::new()]` / `vec!["xyzzy".into()]` (3) | killed | `tests::layer_1_aggregates_every_rule`, `the_verdict_folds_both_layers_and_the_arguments_into_an_exit_code` |
| `:137:5` `run -> Default::default()` | killed | `run_returns_a_different_exit_code_for_each_failure_it_reports` |
| `:4243:5` `check_workspace_name_one_source -> …` (3) | killed | `a_missing_crates_directory_fails_the_rule`, `the_copy_in_foundry_core_is_its_one_home`, `a_production_call_to_the_test_seeding_seam_is_flagged` |
| `:4279:5` `scan_one_source -> …` (3) | killed | `a_missing_crates_directory_fails_the_rule`, `a_door_that_inserts_a_project_itself_is_flagged`, `the_copy_in_foundry_core_is_its_one_home` |
| `:4331:5` `copy_violations_in -> …` (3) | killed | `a_second_copy_in_any_adapter_service_or_store_crate_is_flagged`, `the_workspace_copy_and_other_project_copy_are_not_this_rules_business` |
| `:4352:5` `workspace_seam_call_violations_in -> …` (3) | killed | `a_production_call_to_the_test_seeding_seam_is_flagged`, `layer_1_aggregates_every_rule` |
| `:4358:17`, `:4359:17` `&&` → `\|\|`; `:4358:20`, `:4359:20` delete `!` (4) | killed | `a_production_call_to_the_test_seeding_seam_is_flagged` |
| `:4557:5` `check_project_name_one_source -> …` (3) | killed | `a_missing_crates_directory_fails_the_rule`, `a_door_that_inserts_a_project_itself_is_flagged`, `the_copy_in_foundry_core_is_its_one_home` |
| `:4571:5` `project_insert_call_violations_in -> …` (3) | killed | `a_door_that_inserts_a_project_itself_is_flagged`, `the_store_definition_the_use_case_and_test_callers_are_not_flagged` |
| `:4571:55` `==` → `!=` in `is_ident` | killed | `a_door_that_inserts_a_project_itself_is_flagged` |
| `:4576:38` `+` → `-` / `*` (`after` slice) (2) | killed | `a_door_that_inserts_a_project_itself_is_flagged`, `the_store_definition_the_use_case_and_test_callers_are_not_flagged` |
| `:4580:13`, `:4582:20`, `:4583:20` delete `!` (3) | killed | `a_door_that_inserts_a_project_itself_is_flagged`, `the_store_definition_the_use_case_and_test_callers_are_not_flagged` |
| `:4581:17`, `:4582:17`, `:4583:17` `&&` → `\|\|` (3) | killed | `the_store_definition_the_use_case_and_test_callers_are_not_flagged`, `layer_1_aggregates_every_rule` |
| `:4571:50` `\|\|` → `&&` in `is_ident` | survived first run → **killed on re-check** | `a_longer_name_that_ends_in_insert_project_is_not_the_store_call` (see Survivors 1) |
| `:4579:17` `\|\|` → `&&` in `joined_to_ident` | survived first run → **killed on re-check** | `a_longer_name_that_ends_in_insert_project_is_not_the_store_call` (see Survivors 1) |
| `:4577:65` `i + 1` → `i - 1` (`line_start`) | survived first run → **killed on re-check** | `a_comment_mentioning_the_store_call_past_the_first_line_is_not_flagged` (see Survivors 2) |
| `:4577:65` `i + 1` → `i * 1` (`line_start`) | **survived (equivalent)** | see Survivors 3 |

## Survivors analysis

The first run left four survivors, all in the `project-name-one-source` architecture guard
(`project_insert_call_violations_in`), none in the name rule or its doors. Tests were then added
for the three killable ones (see "Survivors re-check" below); only the equivalent one remains.
No production code was changed.

### 1. `xtask/src/check_arch.rs:4571:50` and `:4579:17`: the "longer identifier" exclusion

- `:4571:50`: `let is_ident = |c| c.is_alphanumeric() || c == '_'` becomes `… && c == '_'`,
  which is never true, so `joined_to_ident` is always false.
- `:4579:17`: `before_is_ident || after_is_ident` becomes `… && …`, so a match is "joined" only
  when both neighbours are identifier characters.
- **What it breaks.** The doc comment promises that "a longer identifier containing the name"
  is not a call. Under either mutant, a door line such as `store.bulk_insert_project(x)` or
  `my_insert_project(x)` (identifier character *before* the match, `(` after) is flagged as a
  forbidden `insert_project` call: a false positive that would fail `cargo xtask check-arch`
  on a legitimate helper. (A suffix like `insert_projects(` is still excluded by the
  `starts_with('(')` test, which is why the after-side alone does not expose these.)
- **Why the tests miss it.** No fixture puts an identifier character before `insert_project`.
  `a_door_that_inserts_a_project_itself_is_flagged` uses `.insert_project(`; the not-flagged
  test uses `fn insert_project(`, a services caller, a test caller and a `//` comment.
- **Not equivalent.** The behaviour differs on a realistic input.
- **Killing test.** In `project_name_one_source_tests`: stage
  `crates/foundry-app/src/projects.rs` containing `state.store.bulk_insert_project(a).await;`
  (and `let x = reinsert_project(b);`) and assert `check_project_name_one_source` is empty.
  Kills both mutants.

### 2. `xtask/src/check_arch.rs:4577:65`: `line_start` `i + 1` → `i - 1`

- **What it breaks.** `line_start` should be the byte after the previous `\n`. Under the mutant
  it is the byte *before* that `\n`, so `contents[line_start..].trim_start()` begins with the
  last character of the previous line. A `//` comment that mentions `insert_project(` on any
  line after the first, following a line that ends in a non-whitespace character, is no longer
  recognised as a comment and is flagged. (If the file starts with `\n` and the match is on line
  2, `0 - 1` panics in debug.)
- **Why the tests miss it.** The only comment fixture
  (`the_store_definition_the_use_case_and_test_callers_are_not_flagged`, `foundry-app`) puts
  the comment on line 1, where `rfind('\n')` is `None` and `line_start` is 0 under both versions.
- **Not equivalent.**
- **Killing test.** Stage `crates/foundry-app/src/projects.rs` as
  `"fn a() {}\n    // the use-case calls insert_project( for us\n"` and assert no violation.

### 3. `xtask/src/check_arch.rs:4577:65`: `line_start` `i + 1` → `i * 1` (equivalent)

- `line_start` becomes the index of the `\n` itself instead of the byte after it. The only use
  is `contents[line_start..].trim_start().starts_with("//")`. `trim_start` removes the `\n`
  (it is Unicode whitespace) together with the indentation that follows, so the trimmed slice
  is byte-identical for every input, including `\r\n` endings. The slice index is always a char
  boundary (`\n` is one byte).
- **Equivalent mutant.** No test can kill it. It is still counted as missed, which is the
  stricter choice; excluding it gives 62/62 = 100%.

### Survivors re-check (tests added, production untouched)

Tests added (uncommitted):
- `xtask/src/check_arch.rs`, `project_name_one_source_tests`:
  - `a_longer_name_that_ends_in_insert_project_is_not_the_store_call`: stages
    `crates/foundry-app/src/projects.rs` with `state.store.bulk_insert_project(a).await;` and
    `let x = reinsert_project(b);`, asserts no violation.
  - `a_comment_mentioning_the_store_call_past_the_first_line_is_not_flagged`: stages
    `"fn a() {}\n    // the use-case calls insert_project( for us\n"`, asserts no violation.
- `crates/foundry-services/tests/create_project_use_case.rs`:
  `two_lost_races_still_leave_a_third_attempt_that_lands`: seeds `jp` and `jp-2`, scripts two
  stale reads (`[]`, `[jp]`), and asserts the third attempt (a real store read) returns
  `KeyFallback("jp-3")` and writes that row. This pins the lower side of
  `FALLBACK_SLUG_ATTEMPTS = 3`; `three_lost_races_end_in_contention_and_write_nothing` already
  pinned the upper side.

Kill evidence. Each mutant's exact cargo-mutants diff
(`$S/xtask/mutants.out/diff/*.diff`) was applied with `patch -p0` to the working tree, the suite
was run, and the file was restored from a copy (`shasum -a 256 -c` OK; `git diff --quiet --
crates/*/src` clean):

| Mutant | Command | Result |
|---|---|---|
| `:4571:50` `\|\|` → `&&` | `cargo test -p xtask project_name_one_source` | 8 passed, 1 failed: `a_longer_name_that_ends_in_insert_project_is_not_the_store_call` |
| `:4579:17` `\|\|` → `&&` | same | 8 passed, 1 failed: `a_longer_name_that_ends_in_insert_project_is_not_the_store_call` |
| `:4577:65` `i + 1` → `i - 1` | same | 8 passed, 1 failed: `a_comment_mentioning_the_store_call_past_the_first_line_is_not_flagged` |
| `:4577:65` `i + 1` → `i * 1` | same | 9 passed (equivalent, as predicted) |
| `FALLBACK_SLUG_ATTEMPTS = 2` (hand mutant, consts are not mutated by cargo-mutants) | `cargo test -p foundry-services --test create_project_use_case` | 6 passed, 1 failed: `two_lost_races_still_leave_a_third_attempt_that_lands` panicked "the third attempt must land, not give up: FallbackSlugContention" |

After restore: `cargo test -p xtask` 61 passed; `cargo test -p foundry-services --test
create_project_use_case` 7 passed; `cargo fmt --all -- --check` clean; `cargo clippy -p xtask
-p foundry-services --all-targets -- -D warnings` clean. No `mutants.out` or
`proptest-regressions` left in the repo.

## Structural checks (guards cargo-mutants does not mutate)

cargo-mutants generates whole-function and operator mutants. It does not delete or swap match
arms without a wildcard, change `const`s or range bounds, or reorder statements. So each
ordering and branch choice this feature relies on was checked by hand against the suite that
pins it:

- **Retry only on `KeyFallback`, never on `Derived` (`attempt_create`'s outcome match).**
  - `KeyFallback` + `DuplicateSlug` → retry: pinned by
    `a_stale_fallback_address_is_retried_to_the_next_free_one` (a stale read loses on `jp`, the
    retry lands on `jp-2`). Mapping it to `NotUnique` instead would fail that test.
  - `Derived` + `DuplicateSlug` → `NotUnique` with no retry: the *outcome* is pinned by
    `a_derived_address_collision_is_not_unique_and_writes_nothing`. The *absence of a retry* is
    **not observable**: a retry re-reads the store, sees the sibling holding that address, and
    `ensure_unique_among`'s address arm refuses `NotUnique` anyway. The two differ only if the
    colliding row is deleted between the failed insert and the re-read. Treated as
    near-equivalent; not pinned, low risk.
  - A genuine same-name race on the retry gets the true refusal: pinned by
    `a_concurrent_create_of_the_same_name_is_refused_on_the_retry`.
- **The retry bound `FALLBACK_SLUG_ATTEMPTS = 3`.** **Pinned on both sides after the re-check**
  (the text below records the first-run finding).
  `three_lost_races_end_in_contention_and_write_nothing` pins the upper side: a fourth attempt
  would read the real store, find `jp-4` free and create, failing the test. The lower side is
  **not pinned**: with a bound of 2, that test still gets `FallbackSlugContention` (two stale
  attempts, then give up), and the one-retry test still passes. **Killing test:** seed `jp` and
  `jp-2`, script two stale reads (`[]`, `[jp]`), then let the third attempt read the store and
  assert `KeyFallback("jp-3")` and one new row. A bound of 2 would return contention.
- **The empty-address skip (D6).** Mutated and killed in core (`project_name.rs:80:27`,
  `:82:60`) by `an_empty_derived_address_is_never_an_address_match`. Also pinned end to end by
  "A name with no address of its own is not a duplicate of an old project that has none either"
  and "An old project with no address does not block a new one".
- **The mint (D15).** `Derived` is used verbatim and never checked against `team_slugs`; the
  suffix starts at 2 (`2u32..` is a literal, not mutated). Pinned by the core tests
  `a_name_with_its_own_address_keeps_it`, `a_fallback_address_is_the_lowest_free_candidate`,
  and the acceptance outlines "The fallback address takes the lowest free number, never \"-1\""
  and "Names with Latin letters or digits keep today's addresses".
- **Store `DuplicateSlug` mapping (`insert_project`, `constraint.contains("slug")`).**
  cargo-mutants generated no mutant inside it. It is pinned indirectly: the retry test needs
  `DuplicateSlug` to retry, and the derived-collision test needs it to map to `NotUnique`. No
  store-level test pins it directly (see foundry-store above).
- **Create door gate order (`submit_create`).** Signed-out / unknown team / non-member before
  the name: pinned by "A caller who may not create in the team gets today's answer, whatever
  the name" (byte-identical answers). Name parsed before the use-case and before any write:
  "A name past 256 characters is refused, kept in the form, and nothing is created" and the
  blank/invisible outline ("nothing was created").
- **Use-case order (sibling check before the key).** Pinned by "A name problem is reported
  before a key problem", example `Sandbox` / `ops`: a duplicate name with an invalid key gets
  the uniqueness refusal, not the key one.
- **Rename order.** Non-admin before the name: "A non-admin's rename is answered like a missing
  page before the name is looked at". Unknown project before the name: "A rename aimed at a
  project that does not exist is answered like a missing page, whatever the name". No-op before
  the rule (D8): "An untouched name from before the rule can be left as it is" and the
  `classify_rename_properties` suite.
- **Copy has one home.** The doors render `err.to_string()` and match no rule variant. Pinned by
  "Both doors refuse the same name in the same words" (11 examples, byte for byte) and the
  `project-name-one-source` (a) guard, whose mutants are all killed.

## Final state

**Production file hashes, before and after** (`diff $S/sha-before.txt $S/sha-after.txt`:
identical):

```
8ae926dcaea99c6aeebb4edccb0ba9496106ff10369ff4dd151dab8174c77ebb  crates/foundry-core/src/lib.rs
4fa701b29719de094567d79806676075054afb911aa353e4d6bc73ceead59f24  crates/foundry-core/src/name_chars.rs
d3bfc4433c3df84e77631330b51ecc50988a0892bd5c5444cbda87473e31df9f  crates/foundry-core/src/project_name.rs
df2f35c947cf40c96b5f71cb2e2b0585345a848cfe1315ab6e2cefeeb6e7c278  crates/foundry-core/src/workspace_name.rs
144202681b1b37ac7d756d50cdb51fac4972c36140952c49f2939c980ebf4a84  crates/foundry-services/src/projects.rs
a5681a7b04327b98432a5a19132b7c67ec45ebf9eb281945636030c90d6550ad  crates/foundry-app/src/projects.rs
8508410da2dc8f14d845b03d00ccb94597959faa1015919a387845d17b6f9009  crates/foundry-app/src/instance_admin.rs
5fc250e19147747ad1356e604a40ed64af21d969e4db34a57f8ab72cde72aa56  crates/foundry-store/src/lib.rs
36a3a3e9eaf3e1d3d4e50b169844e09da10c2f721bf9366ff49d2bdcfc87d4c2  xtask/src/check_arch.rs
```

- `git status --short`: only this report (untracked, not committed).
- Containers: the set of running containers is the same before and after (`docker ps -q`
  diff empty). The testcontainers Postgres and Selenium instances were removed by the harness.
- The scratch clone `$S/tree` was deleted.
- First run: no test, feature file, step module or production file was changed.
  `FOUNDRY_STAMP_*` was never set.
- Survivor re-check: three tests added (two in `xtask/src/check_arch.rs`'s
  `project_name_one_source_tests`, one in `crates/foundry-services/tests/create_project_use_case.rs`),
  uncommitted. Production code unchanged (`git diff --quiet -- crates/*/src` clean; the
  `check_arch.rs` diff is one hunk inside the test module).

**Gate verdict: PASS.** 62/63 = 98.4% after the survivor re-check (100% with the equivalent
mutant excluded; 90.5% on package tests alone). First run: 59/63 = 93.7%. Its 4 survivors were all
in the `project-name-one-source` guard's call matcher, not in the name rule or its doors; the
first three are now killed by tests:
- `check_arch.rs:4571:50` and `:4579:17`: the "longer identifier" exclusion is unpinned;
- `check_arch.rs:4577:65` → `i - 1`: a `//` comment past line 1 is unpinned;
- `check_arch.rs:4577:65` → `i * 1`: equivalent.

Gaps routed to acceptance-designer and closed: the two xtask fixtures above and the lower side of
the `FALLBACK_SLUG_ATTEMPTS` bound (a success on the third attempt). Still open, optional: package-level
tests for `MintedSlug::as_str` (foundry-core) and `insert_project`'s `DuplicateSlug`/`DuplicateKey`
mapping (foundry-store), which today die only through foundry-services and the acceptance lane.
