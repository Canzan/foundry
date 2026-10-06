# Mutation report: instance-workspace-name-rule (DELIVER Phase 5)

**Date**: 2026-10-05
**Tool**: cargo-mutants 25.3.1 (default copy mode, never `--in-place`)
**Scope**: the production lines this feature changes. They are selected with `--in-diff` over
`git diff d8319d0..HEAD -- crates/foundry-core/src crates/foundry-services/src crates/foundry-app/src xtask/src`
(base `d8319d0` = DELIVER roadmap; HEAD = `4dea2a8`):
- `crates/foundry-core/src/workspace_name.rs` (new): `WorkspaceName::try_new`, `as_str`, `Display`,
  and the D4 refused set `is_refused`. `foundry-core/src/lib.rs` only adds the `mod`/`pub use`
  lines, so it has no mutants.
- `crates/foundry-services/src/workspaces.rs`: `classify_workspace_rename` now delegates to
  `WorkspaceName::try_new`. `crates/foundry-services/src/lib.rs`: `ProvisionRequest.workspace_name`
  is now a `WorkspaceName`.
- `crates/foundry-app/src/instance_admin.rs`: `ProvisionFormEcho` (new), `render_dashboard` (new,
  shared by the GET and the 422 re-render), and the touched bodies of `show_dashboard`,
  `submit_provision` and `submit_workspace_rename`.
- `crates/foundry-app/src/bootstrap.rs`: the name gate in `submit`, `refuse_unfit_name` (new),
  `render_claim` (new) and `render_claim_form`.
- `crates/foundry-app/src/admin_cli.rs`: `run_provision_workspace` (usage guard, name rule before
  `DATABASE_URL`). `crates/foundry-app/src/main.rs`: the `provision-workspace` dispatcher arm.
- `crates/foundry-app/src/views.rs`: template-struct fields only, so cargo-mutants generates **no
  mutants** here.
- `xtask/src/check_arch.rs`: the `workspace-name-one-source` guard and its wiring into `run`
  and `source_violations`.

`--in-diff` also generates whole-function mutants for `dispatch_subcommand`, `run`,
`source_violations`, `submit`, `show_dashboard`, `submit_provision`, `submit_workspace_rename`
and `provisioning::provision_workspace`, whose bodies this feature only touches. They stay in
the denominator, which is the stricter choice.

Not mutated: the askama templates, and the string literals (the `WorkspaceNameError` copy,
the usage lines). The acceptance lane pins the copy byte for byte (`Then every door refuses it
saying "…"`, `the command exits 2 saying "…"`).

**Gate**: kill rate ≥ 80% PASS, 70–80% WARN, < 70% FAIL (`CLAUDE.md`, per-feature).

## Result

| Metric | Value |
|---|---|
| Mutants generated | 55 |
| Unviable (do not compile, excluded) | 5 |
| Viable | 50 |
| Killed by the package's own tests | 32 |
| Killed on re-check against the acceptance lane | 14 |
| Killed on re-check by the survivor-closing scenarios (2026-10-06) | 4 |
| Survived | 0 (was 4; see "Survivors re-check") |
| Timeouts | 0 |
| **Kill rate** | **100.0%** (50/50 viable). Was 92.0% (46/50) before the survivor-closing scenarios. Package tests alone: 64.0% (32/50) |
| Equivalent mutants excluded | none |
| **Gate verdict** | **PASS** |

Every kill below was checked in its log. Each one failed on a test assertion in a test that
exercises the mutated code. None was "caught" by an infrastructure error:
- The package mutant logs contain no `PortNotExposed`, `SSLRequest`, sqlx `Protocol(…)` or
  `Connection reset`.
- Every acceptance kill log has zero `PortNotExposed`/`SSLRequest`/`Protocol(` lines. They do
  contain 27–30 `curl: (56) Recv failure: Connection reset by peer` lines. These come from the
  browser harness polling the Selenium container until it is ready. The green iwnr baseline
  shows 29 of them, so they are not a failure cause.
- One re-check run hit the known sqlx `Protocol("unknown message type: '\0'")` flake in a
  Background step (`main.rs:846:39` guard → `true`, 1 failed scenario). It was **not** counted
  as a kill. The rerun was 55/55 green, so that mutant is recorded as missed.

## Per-file table

| File | Total | Killed | Survived | Timeout | Unviable | Kill rate |
|---|---|---|---|---|---|---|
| `crates/foundry-core/src/workspace_name.rs` | 10 | 9 | 0 | 0 | 1 | 100% (9/9) |
| `crates/foundry-services/src/workspaces.rs` | 1 | 0 | 0 | 0 | 1 | no viable mutants |
| `crates/foundry-services/src/lib.rs` | 1 | 0 | 0 | 0 | 1 | no viable mutants |
| `xtask/src/check_arch.rs` | 19 | 19 | 0 | 0 | 0 | 100% (19/19) |
| `crates/foundry-app/src/instance_admin.rs` | 7 | 5 (0 package + 5 acceptance) | 0 | 0 | 2 | 100% (5/5) |
| `crates/foundry-app/src/bootstrap.rs` | 6 | 6 (0 package + 6 acceptance) | 0 | 0 | 0 | 100% (6/6) |
| `crates/foundry-app/src/admin_cli.rs` | 4 | 4 (0 package + 4 acceptance) | 0 | 0 | 0 | 100% (4/4) |
| `crates/foundry-app/src/main.rs` | 7 | 7 (4 package + 3 acceptance) | 0 | 0 | 0 | 100% (7/7) |
| `crates/foundry-app/src/views.rs` | 0 | – | – | – | – | no mutants generated |
| **Total** | **55** | **50** | **0** | **0** | **5** | **100.0%** (was 92.0%, 46/50, before the re-check) |

## Procedure and exact commands

`S` is `/private/tmp/claude-501/-Users-jeffbailey-Projects-canzan-foundry/9298978f-…/scratchpad/mut-iwnr`.

**Safety.** Nothing wrote to git in the working repository. No `checkout`, `restore`, `stash`,
`reset` or `clean` was run, and cargo-mutants never ran `--in-place`. cargo-mutants mutates its
own copy of the tree under `$TMPDIR`. The acceptance re-check applied mutants to a **separate
`git clone`** (`$S/tree`, own `target/`, with no `.env` or secrets copied). It used a `cp`
backup and a `cmp` restore against the working tree. The clone was deleted at the end, and the
working tree was never mutated. `FOUNDRY_STAMP_*` was never set (the seed script `unset`s them).

1. **Snapshot.**
   ```sh
   shasum -a 256 crates/foundry-core/src/workspace_name.rs crates/foundry-core/src/lib.rs \
     crates/foundry-services/src/workspaces.rs crates/foundry-services/src/lib.rs \
     crates/foundry-app/src/{instance_admin,bootstrap,admin_cli,main,views}.rs \
     xtask/src/check_arch.rs > $S/sha-before.txt
   docker ps -q | sort > $S/docker-before.txt
   ```
2. **Scope diff** (read-only git):
   ```sh
   git diff d8319d0..HEAD -- crates/foundry-core/src crates/foundry-services/src \
     crates/foundry-app/src xtask/src > $S/iwnr.diff
   cargo mutants --in-diff $S/iwnr.diff --list   # 55 mutants
   ```
3. **Per-package runs**, one after another (`$S/chain.sh`). Each was run as
   `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false timeout 3600 cargo mutants --in-diff $S/iwnr.diff … --output $S/<pkg> -j 2 --timeout 600`,
   against all of the package's tests:
   - **foundry-core**: `--file crates/foundry-core/src/workspace_name.rs --package foundry-core`
     (lib + `tests/workspace_name.rs`). Baseline green (6.5 s build + 0.4 s test). 10 mutants in
     16 s: 9 caught, 1 unviable.
   - **foundry-services**: `--file crates/foundry-services/src/workspaces.rs --file crates/foundry-services/src/lib.rs --package foundry-services`
     (lib property tests plus every `tests/*.rs`, including `rename_workspace_use_case` and
     `provision_workspace_use_case`). Baseline green (20.8 s build + 18.6 s test). 2 mutants in
     55 s: 2 unviable.
   - **xtask**: `--file xtask/src/check_arch.rs --package xtask` (scoped to the diff by
     `--in-diff`). Baseline green (2.0 s build + 0.6 s test). 19 mutants in 17 s: 19 caught.
   - **foundry-app**: `--file` for `instance_admin.rs`, `bootstrap.rs`, `admin_cli.rs`, `main.rs`
     and `views.rs`, `--package foundry-app` (lib unit tests, `backup_verify_fail_closed`,
     `csrf_middleware`, `password_less_account_doors`). Baseline green (38.0 s build + 18.1 s
     test). 24 mutants in 5m 56s: 4 caught, 18 missed, 2 unviable.
4. **Acceptance re-check of the 18 foundry-app survivors** (foundry-services had none), as in
   instance-admin-workspace-rename. The script `$S/seed.sh` applied each survivor's **exact
   cargo-mutants diff** (`$S/app/mutants.out/diff/*.diff`) with `patch -p0` in `$S/tree`. For
   each run it:
   - backs up the target file with `cp` and applies the patch;
   - rebuilds the `foundry` binary (`CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false cargo build -p foundry-app --bin foundry`)
     and the acceptance test binary (`--no-run`);
   - warms the binary (three `foundry --version` execs);
   - runs `FOUNDRY_ACCEPTANCE_TAGS=<tag> timeout 1800 cargo test -p foundry-acceptance --test acceptance`;
   - restores with `cp` and checks with `cmp` against the working tree.

   All 18 were first run against `iwnr`. The 3 that survived `iwnr` were then run against their
   neighbour surfaces: `bootstrap-enum-oracle` and `us-05` for `render_claim_form` (the GET of
   a live claim link), and `mwt-slice-06` (the multi-workspace provisioning CLI) for the
   `admin_cli.rs` usage guard. The runs went one after another, with no other lane running. A
   positive tag selection includes the `@needs-browser` scenarios, which run in real Chrome
   through the Docker Selenium harness.
   - **Baseline iwnr (unmutated clone): 55/55 scenarios, 308/308 steps.**
   - **Baseline bootstrap-enum-oracle: 4/4 scenarios, 29/29 steps.**
   - **Baseline us-05: 23/23 scenarios, 147/147 steps.**
   - **Baseline mwt-slice-06: 9/9 scenarios, 59/59 steps.**

   One flake (the sqlx `'\0'` one above) was rerun. No other reruns were needed.
5. **Restore proof.** All 25 seeds ended with `RESTORED_CMP_OK`. After the whole run:
   - `diff $S/sha-before.txt $S/sha-after.txt` is identical (hashes below);
   - `docker ps -q` before and after is identical;
   - `git status --short` is empty except for this report.

## Every mutant and its disposition

### foundry-core (`crates/foundry-core/src/workspace_name.rs`)

All killed by `crates/foundry-core/tests/workspace_name.rs`.

| Mutant | Outcome | Killing test (assertion) |
|---|---|---|
| `:44:36` `>` → `==` in `try_new` | killed | `the_length_gate_sits_at_24_trimmed_scalars`, `the_cap_is_24_scalars`, `empty_comes_before_control_and_control_before_length`: "\"Canzan Labs Platform Ops\" must be accepted, got TooLong" |
| `:44:36` `>` → `>=` | killed | `the_cap_is_24_scalars`, `the_length_gate_sits_at_24_trimmed_scalars`, `an_accepted_name_displays_exactly_as_stored`: same assertion (24 scalars refused) |
| `:44:36` `>` → `<` | killed | `the_cap_is_24_scalars`, `whitespace_controls_at_the_edges_are_trimmed_not_refused`, `the_bidi_embedding_override_and_isolate_boundaries`: `left: Ok("Canzan Labs Platform Team")` for a 25-scalar name |
| `:51:9` `as_str -> ""` | killed | `the_c0_and_c1_control_boundaries`, `the_line_and_paragraph_separators`, …: "\"Ops\\u{202f}Team\" must be stored as …", `left: ""` |
| `:51:9` `as_str -> "xyzzy"` | killed | `whitespace_controls_at_the_edges_are_trimmed_not_refused`, …: "\"\\tKitchen\\n\" must be stored as \"Kitchen\"", `left: "xyzzy"` |
| `:57:9` `Display::fmt -> Ok(Default::default())` | killed | `an_accepted_name_displays_exactly_as_stored` (`:174`): `left: ""` |
| `:66:5` `is_refused -> true` | killed | `other_format_characters_stay_allowed`, `the_cap_is_24_scalars`, …: "\"Ops\\u{202f}Team\" must be accepted, got ControlCharacter" |
| `:66:5` `is_refused -> false` | killed | `non_whitespace_controls_at_the_edges_are_refused`, `the_c0_and_c1_control_boundaries`, `empty_comes_before_control_and_control_before_length`: `left: Err(TooLong)` for "Canzan Labs Platform\tEngineering" |
| `:67:9` `\|\|` → `&&` in `is_refused` | killed | `the_line_and_paragraph_separators`, `the_bidi_embedding_override_and_isolate_boundaries`, `empty_comes_before_control_and_control_before_length` |
| `:37:9` `try_new -> Ok(Default::default())` | unviable | `WorkspaceName` has no `Default` |

### foundry-services (`workspaces.rs`, `lib.rs`)

| Mutant | Outcome | Reason |
|---|---|---|
| `workspaces.rs:72:5` `classify_workspace_rename -> Ok(Default::default())` | unviable | `WorkspaceNameDecision` has no `Default` |
| `lib.rs:263:9` `provisioning::provision_workspace -> Ok(Default::default())` | unviable | `Provisioned` has no `Default` |

The feature *removed* logic from this crate: the empty and length arms of
`classify_workspace_rename` moved into `WorkspaceName::try_new`, and `ProvisionRequest` now
holds a typed `WorkspaceName`. So no viable mutant is left here. The delegated rule is mutated
in foundry-core, where all 9 viable mutants die. The `classify_workspace_rename` property suite
(`interior_refused_char_is_control_at_any_length`, `byte_equal_current_is_noop_before_every_gate`,
`legacy_tab_name_untouched_is_noop_edited_is_refused`) still pins the precedence at this seam.

### xtask (`xtask/src/check_arch.rs`)

All 19 killed by `check_arch` unit tests.

| Mutant | Outcome | Killing test (assertion) |
|---|---|---|
| `:84:5` `source_violations -> vec![]` / `vec![String::new()]` / `vec!["xyzzy".into()]` (3) | killed | `tests::layer_1_aggregates_every_rule` (`:3856`): "these LAYER 1 rules are no longer wired into the guard: [… ]"; plus `the_verdict_folds_both_layers_and_the_arguments_into_an_exit_code` |
| `:136:5` `run -> Default::default()` | killed | `tests::run_returns_a_different_exit_code_for_each_failure_it_reports` (`:3745`): "`--root` with no directory argument … must exit 2" |
| `:4242:5` `check_workspace_name_one_source -> vec![]` | killed | `a_missing_crates_directory_fails_the_rule`: "a missing crates directory must FAIL the guard, never pass it vacuously"; `a_second_copy_in_any_adapter_or_service_crate_is_flagged`; `a_production_call_to_the_test_seeding_seam_is_flagged` |
| `:4242:5` `-> vec![String::new()]` / `vec!["xyzzy".into()]` (2) | killed | `the_copy_in_foundry_core_is_its_one_home`, `expected_strings_in_the_acceptance_suite_and_crate_tests_are_not_flagged`: `[""]` / `["xyzzy"]` |
| `:4243:8` delete `!` | killed | `a_missing_crates_directory_fails_the_rule`, `the_copy_in_foundry_core_is_its_one_home` |
| `:4253:12` delete `!` | killed | `the_copy_in_foundry_core_is_its_one_home`, …: "cannot list crates/foundry-app/src …" |
| `:4284:5` `workspace_seam_call_violations_in -> vec![]` | killed | `a_production_call_to_the_test_seeding_seam_is_flagged` (`:4414`): `left: 0` |
| `:4284:5` `-> vec![String::new()]` / `vec!["xyzzy".into()]` (2) | killed | the same test, `a_second_copy_in_any_adapter_or_service_crate_is_flagged`, `tests::layer_1_aggregates_every_rule` |
| `:4290:17`, `:4291:17` `&&` → `\|\|` (2) | killed | `a_production_call_to_the_test_seeding_seam_is_flagged`: `left: 6` (every file flagged) |
| `:4290:20`, `:4291:20` delete `!` (2) | killed | `a_production_call_to_the_test_seeding_seam_is_flagged` (`:4432`): `left: 0` |
| `:4307:5` `workspace_name_copy_violations_in -> vec![]` | killed | `a_second_copy_in_any_adapter_or_service_crate_is_flagged` (`:4366`): `left: 0` |
| `:4307:5` `-> vec![String::new()]` / `vec!["xyzzy".into()]` (2) | killed | the same test, `the_seam_itself_and_its_test_callers_are_not_flagged`: `left: 4` |

### foundry-app

| Mutant | Outcome | Killing test (assertion) |
|---|---|---|
| `main.rs:794:5` `dispatch_subcommand -> None` / `Some(0)` / `Some(1)` / `Some(-1)` (4) | killed (package) | `backup_verify_fail_closed.rs:148/173/217` (`unreachable_probe_exits_8_…`, `failing_count_on_a_present_table_exits_9_…`, `healthy_schema_counts_present_tables_…`): exit code / stdout mismatch |
| `main.rs:846:39` guard `!admin_email.is_empty()` → `false` | missed by package → **killed by acceptance** | `iwnr`: 55 scenarios (34 passed, 21 failed). Every present `--name` gets the usage line: "stderr must carry the line \"foundry doctor provision-workspace: Workspace name must be at most 24 characters\"; got \"… missing required flags. Usage: …\"", "every door must refuse with exactly \"Workspace name must not contain control characters\"" |
| `main.rs:846:39` delete `!` | missed by package → **killed by acceptance** | `iwnr`: 55 scenarios (34 passed, 21 failed), the same assertions |
| `main.rs:846:39` guard → `true` | missed (package; `iwnr` 55/55 on rerun) → **killed by the re-check** | `iwnr` 61 scenarios (59 passed, 2 failed): "Leaving out the first admin says required flags are missing, whatever the name" (2 examples): "stderr must carry exactly the line \"foundry doctor provision-workspace: missing required flags. Usage: …\"; got \"… --name, --admin-email and --as are required. Usage: …\"" |
| `admin_cli.rs:609:5` `run_provision_workspace -> 0` / `1` / `-1` (3) | missed by package → **killed by acceptance** | `iwnr`: 55 scenarios (34 passed, 21 failed) each: "an unfit name is an argument error: exit 2 (D8, DDD-10); stdout=\"\" stderr=\"\"" (7), "the command exits 2 saying \"Workspace name must be at most 24 characters\" …" (3), the KPI-2 parity outlines (4) |
| `admin_cli.rs:609:31` `\|\|` → `&&` | missed (package, `iwnr` 55/55, `mwt-slice-06` 9/9) → **killed by the re-check** | `iwnr` 61 scenarios (58 passed, 3 failed): "Not saying who she is gets the usage line before the name or the database is looked at" (3 examples): exit 4 "not authorized — status: refused" with a database; exit 3 "DATABASE_URL is required …" without one; the name-rule line "Workspace name must be at most 24 characters" instead of the usage line for an unfit name |
| `bootstrap.rs:97:5` `submit -> Default::default()` | missed by package → **killed by acceptance** | `iwnr`: 55 scenarios (34 passed, 21 failed): "a live link with an unfit name gets the claim page again with status 422 (D9, DDD-9)" (4), "every door must refuse with exactly …" (7), the redirect and trimmed-name scenarios |
| `bootstrap.rs:214:5` `refuse_unfit_name -> Default::default()` | missed by package → **killed by acceptance** | `iwnr`: 55 scenarios (40 passed, 15 failed): "a live link with an unfit name gets the claim page again with status 422 (D9, DDD-9)" (4), the KPI-2 parity outlines (7), the dead-link byte-identity scenario |
| `bootstrap.rs:350:5` `render_claim -> String::new()` | missed by package → **killed by acceptance** | `iwnr`: 55 scenarios (42 passed, 13 failed): "the 422 answer must be the claim page, with its form to correct; body = \"\"" (4), parity outlines (7) |
| `bootstrap.rs:350:5` `render_claim -> "xyzzy".into()` | missed by package → **killed by acceptance** | `iwnr`: 55 scenarios (42 passed, 13 failed): same, `body = "xyzzy"` |
| `bootstrap.rs:340:5` `render_claim_form -> String::new()` | missed (package, `iwnr` 55/55, `bootstrap-enum-oracle` 4/4, `us-05` 23/23) → **killed by the re-check** | `iwnr` 61 scenarios (60 passed, 1 failed): "Opening a live link shows an empty claim form for that link": "the claim page must carry one form posting back to the link it was opened with; body = \"\"" |
| `bootstrap.rs:340:5` `render_claim_form -> "xyzzy".into()` | missed (same four suites) → **killed by the re-check** | `iwnr` 61 scenarios (60 passed, 1 failed): the same scenario, `body = "xyzzy"` |
| `instance_admin.rs:83:5` `show_dashboard -> Default::default()` | missed by package → **killed by acceptance** | `iwnr`: 55 scenarios (52 passed, 3 failed): "the dashboard must list workspace \"Canzan Platform Eng\"", and two browser scenarios: "the dashboard must carry the Provision form", "the \"Household\" row must carry a rename input" |
| `instance_admin.rs:117:9` `ProvisionFormEcho::status -> Default::default()` (always 200) | missed by package → **killed by acceptance** | `iwnr`: 55 scenarios (40 passed, 15 failed): "an unfit name gets 422 (D9, DDD-8)" (5), parity outlines (7) |
| `instance_admin.rs:134:5` `render_dashboard -> Default::default()` | missed by package → **killed by acceptance** | `iwnr`: 55 scenarios (38 passed, 17 failed): "an unfit name gets 422 (D9, DDD-8); body = " (5), parity outlines (7), dashboard listing |
| `instance_admin.rs:261:5` `submit_provision -> Default::default()` | missed by package → **killed by acceptance** | `iwnr`: 55 scenarios (31 passed, 24 failed): "an unfit name gets 422", the provisioned-confirmation and trimmed-name scenarios, parity outlines |
| `instance_admin.rs:556:5` `submit_workspace_rename -> Default::default()` | missed by package → **killed by acceptance** | `iwnr`: 55 scenarios (26 passed, 29 failed): "a validation refusal must be 422 (D3); body = Some(\"\")" (8), "the answer must be the workspace head ([data-workspace-head]); got \"\"" (6), parity outlines |
| `instance_admin.rs:100:9` `ProvisionFormEcho::blank -> Default::default()` | unviable | `ProvisionFormEcho` has no `Default` |
| `instance_admin.rs:108:9` `ProvisionFormEcho::refused -> Default::default()` | unviable | same |

**Why the foundry-app mutants miss at package level.** This is the same pattern as
instance-admin-workspace-rename and keycloak-sso OD-10. The handlers (`show_dashboard`,
`render_dashboard`, `submit_provision`, `submit_workspace_rename`, bootstrap `submit` and
`refuse_unfit_name`) are async axum handlers over `AppState` (a live store, session and CSRF).
`run_provision_workspace` and the dispatcher arm run only in the real binary. The package has
no unit test for any of them. The acceptance lane drives the real binary over Postgres and real
Chrome. It kills 14 of the 18 on product assertions, not on infrastructure.

## Survivors analysis

Four survivors in the original run. None is claimed as equivalent. No production code was
changed. Each entry names the test that would kill it, for routing to acceptance-designer.
**All four are now killed.** The scenarios that kill them are described under "Survivors
re-check" below.

### 1. `crates/foundry-app/src/admin_cli.rs:609:31`: `||` → `&&` in `run_provision_workspace`

`if admin_email.is_empty() || acting_email.is_empty()` becomes `… && …`.

- **What it breaks.** The dispatcher already refuses an empty `--admin-email`, so the live
  difference is an absent `--as`. Today that exits **2** with the usage line, before
  `DATABASE_URL` is read. Under the mutant the command carries on:
  - with no database it exits 3;
  - with a database it resolves the acting user `""`, finds none, and exits **4** "not
    authorized — status: refused".
  The ordering is also lost: an absent `--as` plus an unfit name would get the name-rule
  refusal instead of the usage line.
- **Why every suite misses it.** No scenario ever omits `--as`. Both `iwnr`
  (`feature_instance_workspace_name_rule.rs:414`) and `mwt-slice-06`
  (`feature_mwt_slice_06_provision_and_prove.rs:258`, `:1440`) always pass it.
- **Killing test.** An `@us-wnr-04` scenario next to "Leaving the name out entirely still gets
  the usage line": *"When Priya runs the provisioning command naming the workspace "Globex"
  for first admin "dana@canzan.net" without saying who she is / Then the command exits 2 with
  its usage line and no name-rule message / And nothing was created or changed."* Run it with
  and without a configured database. It can reuse the existing `command_usage` Then step.

### 2. `crates/foundry-app/src/main.rs:846:39`: match guard `!admin_email.is_empty()` → `true`

`Some(name) if !admin_email.is_empty() => name` becomes `Some(name) if true => name`.

- **What it changes.** With `--name` present and `--admin-email` absent, the dispatcher no
  longer answers. It passes `admin_email = ""` to `run_provision_workspace`, whose own guard
  (`admin_cli.rs:609`) refuses it before the name rule and before `DATABASE_URL`. The exit code
  (2), the empty stdout, the full `Usage: foundry doctor provision-workspace --name <name>
  --admin-email <addr> --as <super-admin-email>` line, and "nothing reached" are all
  **identical**. Only the lead clause of stderr differs: "missing required flags." becomes
  "--name, --admin-email and --as are required.".
- **Equivalence.** This mutant is near-equivalent. It is equivalent on every pinned contract
  (exit 2, usage line, no name-rule message, no DB). It is not byte-equivalent, so it is
  **counted as missed**, which is the stricter choice. If it were excluded as equivalent, the
  rate would be 46/49 = 93.9%.
- **Why every suite misses it.** No scenario omits `--admin-email` (the iwnr flake run aside;
  the rerun was 55/55).
- **Killing test.** A CLI scenario that omits `--admin-email` and asserts the dispatcher's own
  stderr line `foundry doctor provision-workspace: missing required flags.` A design choice
  would serve as well: route all three missing-flag cases through one usage message, which
  would make the mutant truly equivalent. That is a DESIGN decision, not a crafter one.

### 3 and 4. `crates/foundry-app/src/bootstrap.rs:340:5`: `render_claim_form -> String::new()` / `-> "xyzzy".into()`

- **What it breaks.** `GET /bootstrap?token=<live>` (`bootstrap.rs:73`) answers 200 with an
  empty or garbage body instead of the claim form. In practice, a fresh install's operator
  cannot claim the instance from a browser at all.
- **Why every suite misses it.** Every acceptance claim **POSTs** straight to
  `/bootstrap?token=…`:
  - `us-05` "Admin claims the workspace via the bootstrap URL" submits the form without
    reading the GET;
  - `slice-8` GETs only to mint the CSRF cookie;
  - `bootstrap-enum-oracle` GETs only dead links, which take the uniform-refusal branch,
    never `render_claim_form`.
  The iwnr step already asserts `form[action^="/bootstrap?token="]` (`:926`), but only on the
  422 refusal, which goes through `render_claim`.
- **Note.** This feature only touched this function: it now builds a `BootstrapClaim` with
  empty `error/email/display_name/workspace_name` and delegates to `render_claim`. The gap
  predates the feature (the GET of a live link was never pinned). The diff brought it into
  scope.
- **Killing test.** Either of these:
  - an acceptance step, *"When a visitor opens the bootstrap URL for live link "claim-001" /
    Then the claim page is shown with an empty form posting to that link and no error"*,
    which reuses the `form[action^="/bootstrap?token="]` selector and asserts empty
    email/display-name/workspace-name inputs and no `.error`;
  - a foundry-app unit test on `render_claim_form("tok")` asserting
    `action="/bootstrap?token=tok"` and the absence of `class="error"`.
  The us-05 Gherkin already has `When a visitor opens the bootstrap URL "…"`
  (`us_05_bootstrap.rs:300`). Only a Then step for the live-link body is missing.

### Survivors re-check (2026-10-06): all four killed

Six scenarios were added to `crates/foundry-acceptance/tests/features/instance-workspace-name-rule.feature`
(steps in `crates/foundry-acceptance/src/steps/feature_instance_workspace_name_rule.rs`). Only test
code changed. All six pass on the unmutated code. None is `@pending`.

- US-WNR-02, "Opening a live link shows an empty claim form for that link" (`@driving_port
  @real-io @contract-shape:unbounded-preservation`). Priya GETs live link `claim-001`. The answer
  must be 200 with exactly one post form whose `action` is `/bootstrap?token=<that link>`. The
  email, password, display-name and workspace-name inputs must all be empty, and there must be no
  `.error`. Nothing is created, and the link stays unconsumed.
- US-WNR-04, "Not saying who she is gets the usage line before the name or the database is looked
  at" (outline, 3 examples). `--as` is left out: `Globex` with the instance's database, `Globex`
  with no database, and the 32-character name with the database. Each must exit 2 with the usage
  line, no name-rule message and nothing on stdout. Nothing is created. It reuses the
  `command_usage` Then step.
- US-WNR-04, "Leaving out the first admin says required flags are missing, whatever the name"
  (outline, 2 examples: `Globex` and the 32-character name). `--admin-email` is left out. It must
  get `command_usage`, plus the dispatcher's exact line `foundry doctor provision-workspace:
  missing required flags. Usage: foundry doctor provision-workspace --name <name> --admin-email
  <addr> --as <super-admin-email>`. Nothing is created.

Procedure, per mutant:
1. Apply the exact cargo-mutants diff from `mutants.out/diff` to the working tree with `patch`.
2. Rebuild and warm `foundry` (`CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false cargo build -p
   foundry-app --bin foundry`, then one run of the binary).
3. Run `FOUNDRY_ACCEPTANCE_TAGS=iwnr timeout 1800 cargo test -p foundry-acceptance --test acceptance`.
4. Restore the file with `git checkout` and confirm `git diff --quiet -- crates/foundry-app/src`.

| Mutant | `iwnr` result | Failed scenarios (all new) |
|---|---|---|
| `admin_cli.rs:609:31` `\|\|` → `&&` | 61: 58 passed, 3 failed | the 3 "Not saying who she is" examples (exit 4 / exit 3 / name-rule line) |
| `main.rs:846:39` guard → `true` | 61: 59 passed, 2 failed | the 2 "Leaving out the first admin" examples (stderr lead clause) |
| `bootstrap.rs:340:5` `render_claim_form -> String::new()` | 61: 60 passed, 1 failed | "Opening a live link …" (`body = ""`) |
| `bootstrap.rs:340:5` `render_claim_form -> "xyzzy".into()` | 61: 60 passed, 1 failed | "Opening a live link …" (`body = "xyzzy"`) |

Each kill failed on a product assertion in a new scenario. No pre-existing scenario failed. The
logs contain no `PortNotExposed` and no sqlx `Protocol(` line. The restored-tree run was green
both before and after the four mutants: `iwnr` 61 scenarios (61 passed), 344 steps (344 passed).
The `foundry-app/src/*.rs` hashes were identical before and after. `cargo fmt --check` and
`cargo clippy -p foundry-acceptance --all-targets -- -D warnings` are clean.

`main.rs:846:39` → `true` is no longer near-equivalent in practice: the new scenario pins the
dispatcher's own missing-flags wording. If DESIGN later merges the two usage messages into one,
this mutant becomes truly equivalent and the scenario's last Then should be updated with it.

## Structural checks (guards cargo-mutants does not mutate)

cargo-mutants generates whole-function and operator mutants. It does not delete match arms,
swap arms, or reorder statements. So each ordering and branch choice this feature relies on was
checked by hand against the suite that pins it:

- **Typed `ProvisionRequest`, so there is no service re-check.** `ProvisionRequest.workspace_name`
  is a `WorkspaceName`, and its field is private (`pub struct WorkspaceName(String)`). The only
  constructor is `try_new`. An unvalidated name cannot reach `provision_workspace` without a
  compile error, so the absent re-check is enforced by the type, not by a test. Nothing is
  left to mutate.
- **Bootstrap "dead link → uniform refusal page" (`refuse_unfit_name`).** Both arms of the
  liveness match are pinned:
  - `Valid` → 422 claim page: "An over-long workspace name is refused and the link still
    works" and the blank/invisible-character outline;
  - every non-valid link → today's byte-identical refusal: "A dead link answers exactly as
    before, whatever the name", over used (`claim-001`), expired (`stale-002`) and never-issued
    (`never-issued-003`) links, compared byte for byte with the answer for an acceptable name.
  Swapping the arms would fail both. The `Err(_) → 500` arm is **not** pinned. It is
  infrastructure-only and low risk, and noted here for completeness.
- **Bootstrap ordering (name rule before the password hash and the claim transaction).** This
  is pinned observably: "the bootstrap token "claim-001" remains unconsumed" after a refusal,
  and "The corrected claim with the same link succeeds". Moving the rule after the claim would
  consume the link. The skipped argon2 work is not observable and is not pinned.
- **Provision authz before the name (`submit_provision`).** Pinned by "A non-admin is refused
  before the name is looked at": a 40-character name gets the byte-identical never-existed
  answer.
- **CLI ordering.**
  - Absent `--name` → usage line, never the name rule: pinned by "Leaving the name out entirely
    still gets the usage line" (`command_usage` asserts `!stderr.contains("Workspace name must")`).
  - Name rule before `DATABASE_URL`: pinned by "A name mistake is caught without a database",
    with no database and with an unreachable one.
  - The `--admin-email`/`--as` usage guard before the name rule: now pinned by the two
    survivor-closing outlines, each with an unfit-name example (see "Survivors re-check").
- **Rename service delegation.** `classify_workspace_rename` → `WorkspaceName::try_new` cannot
  be deleted without a type error. Its precedence (no-op before the rule; control before
  length) is pinned by the services property suite and the core ordering test
  `empty_comes_before_control_and_control_before_length`.

## Final state

**Production file hashes, before and after** (`diff $S/sha-before.txt $S/sha-after.txt`:
identical):

```
51cc58c4afa9ae311c9fb8d3fa1299c3b45f2854bd7a11afa860c7f7a804110a  crates/foundry-core/src/workspace_name.rs
046d081f3a1270bff8320d4ed7c8ffc19c5861f48b043c692dc8799a149eaf2e  crates/foundry-core/src/lib.rs
240ba8e32c558590727cb7f7d986b71069a0d8deea6b94a76de81d676db58b7c  crates/foundry-services/src/workspaces.rs
0c0490c01f414e759dcaa487e432318de13f1963ca3080dace394a8ef2a64081  crates/foundry-services/src/lib.rs
2df18b1a0f916ae011105c19ebf2d7ed6f711debc406cc12649c3d691512a9d1  crates/foundry-app/src/instance_admin.rs
be071217ed6d5ab74d55912e98c4a45417c81049e0826d9f89fb9fd33ac3a622  crates/foundry-app/src/bootstrap.rs
cdf9df6dbf25bd912d5c7e5fbf87164299755d0b7e3fdc447162f9fb60fe905c  crates/foundry-app/src/admin_cli.rs
495e797a0a7a30eee135c7e9a9197770bca521f9c20fed01dd2ca7bdda1c5fa1  crates/foundry-app/src/main.rs
6251a6282fa8b84fa239cd58fec1eb61b4d5426763ea94a0f4732a08bb90e6bf  crates/foundry-app/src/views.rs
1dbfae46db1dd0b3110c7acc7415479dd94a04adecc97996e39f25fa35dfdd83  xtask/src/check_arch.rs
```

- `git status --short`: only this report (untracked, not committed).
- Containers: the set of running containers is the same before and after (`docker ps -q`
  diff empty). The testcontainers Postgres and Selenium instances were removed by the harness.
  Pre-existing containers were not touched.
- The scratch clone `$S/tree` was deleted.
- The original run changed no test, feature file, step module or production file.
  `FOUNDRY_STAMP_*` was never set. The 2026-10-06 re-check changed only the iwnr feature file and
  step module. Production files stayed byte-identical.

**Gate verdict: PASS.** 50/50 = 100.0% after the 2026-10-06 re-check, or 64.0% (32/50) on
package tests alone. The original run scored 46/50 = 92.0% with 4 survivors, all in CLI-usage and
GET paths rather than in the name rule itself:
- `admin_cli.rs:609:31`: an absent `--as` was unpinned;
- `bootstrap.rs:340:5` ×2: the live-link GET claim form was unpinned;
- `main.rs:846:39` → `true`: near-equivalent.
Six new acceptance scenarios now kill all four (see "Survivors re-check"). There are 0 survivors
and no equivalent mutants excluded.
