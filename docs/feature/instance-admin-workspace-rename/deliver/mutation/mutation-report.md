# Mutation report: instance-admin-workspace-rename (DELIVER Phase 5)

**Date**: 2026-10-05
**Tool**: cargo-mutants 25.3.1 (default copy mode, never `--in-place`)
**Scope**: the production lines this feature changes. They are selected with `--in-diff` over
`git diff 4194293..HEAD -- crates/foundry-store/src crates/foundry-services/src crates/foundry-app/src`
(base `4194293` = DELIVER roadmap; HEAD = `e4a04c0`):
- `crates/foundry-store/src/lib.rs`: `WorkspaceRenameWrite`, `Store::rename_workspace_with_audit`,
  and `Store::probe`'s migration-0018 column check.
- `crates/foundry-services/src/workspaces.rs` (new): `classify_workspace_rename`,
  `rename_workspace`, and the `Services::rename_workspace` delegate.
- `crates/foundry-app/src/instance_admin.rs`: `submit_workspace_rename` (new),
  `workspace_head_view` (new), `rename_error_fragment` (now takes a marker), and the
  touched bodies of `show_dashboard` and `submit_project_rename`.
- `crates/foundry-app/src/views.rs`: `InstanceWorkspaceHeadView` and the row's `head` field.
  These are struct and template changes only, so cargo-mutants generates **no mutants** here.
- `crates/foundry-app/src/lib.rs`: the route mount. Its `build_router` whole-function mutant is
  in the diff, so it is included. That is the stricter choice.

`--in-diff` also generates whole-function mutants for `show_dashboard`, `submit_project_rename`
and `Store::probe`, whose bodies this feature only touches. They stay in the denominator, which
is the stricter choice.

Not mutated: migration 0018's SQL (cargo-mutants does not mutate SQL), the askama templates,
and the string literals (refusal copy, `*_RENAME_ERROR_MARKER`). The acceptance lane pins the
copy and markers byte for byte (`Then the workspace rename is refused saying "…"`).

**Gate**: kill rate ≥ 80% PASS, 70–80% WARN, < 70% FAIL (`CLAUDE.md`, per-feature).

## Result

| Metric | Value |
|---|---|
| Mutants generated | 20 |
| Unviable (do not compile, excluded) | 6 |
| Viable | 14 |
| Killed by the package's own tests | 10 |
| Killed on re-check against the acceptance lane | 4 |
| Survived | 0 |
| Timeouts | 0 |
| **Kill rate** | **100%** (14/14 viable). Package tests alone: 71.4% (10/14) |
| Equivalent mutants excluded | none |
| **Gate verdict** | **PASS** |

Every kill below was checked in its log. Each one failed on a test assertion in a test that
exercises the mutated code. None was "caught" by an infrastructure error:
- The package mutant logs contain no `PortNotExposed`, `SSLRequest`, sqlx `Protocol(…)` or
  `Connection reset`.
- The acceptance logs do contain `curl: (56) Recv failure: Connection reset by peer`. These lines
  come from the browser harness polling the Selenium container until it is ready. Both green
  baselines show the same lines (iawr: 29, iapr: 27), so they are not a failure cause. Every
  failed step in the mutant runs is a product assertion; they are quoted below.

## Per-file table

| File | Total | Killed | Survived | Timeout | Unviable | Kill rate |
|---|---|---|---|---|---|---|
| `crates/foundry-store/src/lib.rs` | 6 | 4 | 0 | 0 | 2 | 100% (4/4) |
| `crates/foundry-services/src/workspaces.rs` | 8 | 5 (4 package + 1 acceptance) | 0 | 0 | 3 | 100% (5/5) |
| `crates/foundry-app/src/instance_admin.rs` | 5 | 4 (1 package + 3 acceptance) | 0 | 0 | 1 | 100% (4/4) |
| `crates/foundry-app/src/lib.rs` | 1 | 1 | 0 | 0 | 0 | 100% (1/1) |
| `crates/foundry-app/src/views.rs` | 0 | – | – | – | – | no mutants generated |
| **Total** | **20** | **14** | **0** | **0** | **6** | **100%** |

## Procedure and exact commands

`S` is `/private/tmp/claude-501/-Users-jeffbailey-Projects-canzan-foundry/9298978f-…/scratchpad/mut-iawr`.

**Safety.** Nothing wrote to git in the working repository. No `checkout`, `restore`, `stash`,
`reset` or `clean` was run, and cargo-mutants never ran `--in-place`. cargo-mutants mutates its
own copy of the tree under `$TMPDIR`. The acceptance re-check applied mutants to a **separate
`git clone`** (`$S/tree`, own `target/`, with no `.env` or secrets copied). It used a `cp`
backup and a `cmp` restore. The clone was deleted at the end, and the working tree was never
mutated.

1. **Snapshot.**
   ```sh
   shasum -a 256 crates/foundry-store/src/lib.rs crates/foundry-services/src/workspaces.rs \
     crates/foundry-app/src/instance_admin.rs crates/foundry-app/src/views.rs \
     crates/foundry-app/src/lib.rs > $S/sha-before.txt
   ```
2. **Scope diff** (read-only git):
   ```sh
   git diff 4194293..HEAD -- crates/foundry-store/src crates/foundry-services/src crates/foundry-app/src > $S/iawr.diff
   cargo mutants --in-diff $S/iawr.diff --list   # 20 mutants
   ```
3. **foundry-store**: run against every integration test that calls `rename_workspace_with_audit`
   or `Store::probe`, over real Postgres (testcontainers):
   ```sh
   CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false timeout 3600 \
   cargo mutants --in-diff $S/iawr.diff --file crates/foundry-store/src/lib.rs \
     --package foundry-store --output $S/store -j 2 --timeout 600 \
     -- --test workspace_rename_with_audit --test probe_schema_scoping --test machine_tokens_repo
   ```
   Baseline green (24.3 s build + 6.1 s test). 6 mutants tested in 1m 19s: 4 caught,
   2 unviable.
4. **foundry-services**: run against all of its tests (lib unit/property tests plus every
   `tests/*.rs`):
   ```sh
   CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false timeout 3600 \
   cargo mutants --in-diff $S/iawr.diff --file crates/foundry-services/src/workspaces.rs \
     --package foundry-services --output $S/services -j 2 --timeout 600
   ```
   Baseline green (14.6 s build + 13.4 s test). 8 mutants tested in 58s: 4 caught, 1 missed,
   3 unviable.
5. **foundry-app**: run against all of its tests (lib unit tests, `backup_verify_fail_closed`,
   `csrf_middleware`, `password_less_account_doors`):
   ```sh
   CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false timeout 3600 \
   cargo mutants --in-diff $S/iawr.diff --file crates/foundry-app/src/instance_admin.rs \
     --file crates/foundry-app/src/views.rs --file crates/foundry-app/src/lib.rs \
     --package foundry-app --output $S/app -j 2 --timeout 600
   ```
   Baseline green (30.4 s build + 15.9 s test). 6 mutants tested in 2m 3s: 2 caught, 3 missed,
   1 unviable.
6. **Acceptance re-check of the 4 survivors**, as in keycloak-sso OD-10. The script `$S/seed.sh`
   applied each survivor's **exact cargo-mutants diff** (`$S/<pkg>/mutants.out/diff/*.diff`)
   with `patch -p0` in `$S/tree`. For each run it:
   - backs up the target file with `cp` and applies the patch;
   - rebuilds the `foundry` binary (`CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false cargo build -p foundry-app --bin foundry`)
     and the acceptance test binary (`--no-run`);
   - warms the binary (three `foundry --version` execs);
   - runs `FOUNDRY_ACCEPTANCE_TAGS=<tag> timeout 1800 cargo test -p foundry-acceptance --test acceptance`;
   - restores with `cp` and checks with `cmp` against the working tree.

   The tag is `iawr` for this feature's code. `submit_project_rename` belongs to the
   project-rename surface, so its relevant tag is `iapr`. The runs went one after another, with
   no other lane running. A positive tag selection includes the `@needs-browser` scenarios,
   which run in real Chrome through the Docker Selenium harness.
   - **Baseline iawr (unmutated clone): 27/27 scenarios, 215/215 steps.**
   - **Baseline iapr (unmutated clone): 21/21 scenarios, 132/132 steps.**

   No flakes occurred, so no reruns were needed.
7. **Restore proof.** All four seeds ended with `RESTORED_CMP_OK`. After the whole run:
   - `diff $S/sha-before.txt $S/sha-after.txt` is identical (hashes below);
   - `git status --short` is empty except for this report.

## Every mutant and its disposition

### foundry-store (`crates/foundry-store/src/lib.rs`)

| Mutant | Outcome | Killing test (assertion) |
|---|---|---|
| `:294:26` `<` → `>` in `Store::probe` | killed | `the_probe_refuses_a_schema_without_the_rename_record` (`workspace_rename_with_audit.rs:494`: "the probe must refuse a schema without workspace_rename_events") |
| `:294:26` `<` → `<=` in `Store::probe` | killed | `probe_column_check_is_scoped_to_current_schema` (`probe_schema_scoping.rs:132`) and `probe_refuses_a_schema_without_the_provisioned_marker` (`:182`): the current schema is refused with `Failed("workspace_rename_events missing migration-0018 columns (found 6 of 6)")` |
| `:294:26` `<` → `==` in `Store::probe` | killed | the same two tests, with the same message: the current schema is refused |
| `:1427:21` `==` → `!=` in `rename_workspace_with_audit` | killed | 4 tests: `an_effective_rename_changes_one_name_and_appends_one_record` (`:309`, `left: Ok(Unchanged)`, `right: Ok(Renamed { old_name: "Bailey Family" })`), `the_same_name_writes_nothing_and_a_case_change_is_a_rename` (`:342`, `workspace_rename_events_check` violation instead of `Ok(Unchanged)`), `a_failed_record_write_leaves_the_name_unchanged_and_nothing_on_record` (`:379`), `concurrent_renames_serialize_into_a_chain_of_records` (`:457`, "two effective renames, two records: []") |
| `:207:9` `Store::probe -> Ok(Default::default())` | unviable | `ProbeReport` has no `Default` |
| `:1418:9` `rename_workspace_with_audit -> Ok(Default::default())` | unviable | `WorkspaceRenameWrite` has no `Default` |

### foundry-services (`crates/foundry-services/src/workspaces.rs`)

| Mutant | Outcome | Killing test (assertion) |
|---|---|---|
| `:68:16` `==` → `!=` in `classify_workspace_rename` | killed (package) | property `byte_equal_current_is_noop_before_every_gate` (`:185`): `left: Some(Write { name: "Aa" })`, `right: Some(NoOp { name: "Aa" })` |
| `:76:32` `>` → `==` | killed (package) | `exactly_24_accepted_25_refused` (`:212`): "24 scalars sit AT the cap and must be written verbatim", `left: None` |
| `:76:32` `>` → `>=` | killed (package) | `exactly_24_accepted_25_refused` (`:212`), same assertion |
| `:76:32` `>` → `<` | killed (package) | property `case_only_change_is_written` (`:200`): `left: None`, `right: Some(Write { name: "AA" })` |
| `:95:8` delete `!` in `rename_workspace` (`if !is_admin` → `if is_admin`) | missed by package → **killed by acceptance** | `iawr`: 27 scenarios (7 passed, 20 failed). The admin's own renames are refused as `Forbidden` → uniform 404: `left: Some(404)`, `right: Some(200)` (17 steps) and `right: Some(422)` (4), e.g. "Priya's rename of "Bailey Family" to "Household" must succeed", "the rename must answer with the re-rendered workspace row", "a validation refusal must be 422 (D3)" |
| `:67:5` `classify_workspace_rename -> Ok(Default::default())` | unviable | `WorkspaceNameDecision` has no `Default` |
| `:91:5` `rename_workspace -> Ok(Default::default())` | unviable | `WorkspaceRenameOutcome` has no `Default` |
| `:126:9` `Services::rename_workspace -> Ok(Default::default())` | unviable | same |

### foundry-app (`crates/foundry-app/src/instance_admin.rs`, `lib.rs`)

| Mutant | Outcome | Killing test (assertion) |
|---|---|---|
| `instance_admin.rs:561:5` `rename_error_fragment -> Default::default()` | killed (package) | `response_helper_tests::rename_error_fragment_is_a_422_with_marker_and_copy` (`:590`): "a validation refusal must answer 422", `left: 200`, `right: 422` |
| `lib.rs:383:5` `build_router -> Default::default()` | killed (package) | `password_less_account_doors.rs:361` / `:258`: `left: 404`, `right: 200` |
| `instance_admin.rs:166:5` `workspace_head_view -> Default::default()` | unviable | `InstanceWorkspaceHeadView` has no `Default` |
| `instance_admin.rs:488:5` `submit_workspace_rename -> Default::default()` | missed by package → **killed by acceptance** | `iawr`: 27 scenarios (3 passed, 24 failed). The empty 200 fails "the answer must be the workspace head ([data-workspace-head]); got """ (8), "a validation refusal must be 422 (D3)" (4), "the refusal status must match the never-existed answer" (4), "workspace … must now be named "Household"" and "the workspace row must re-render in place showing "Household"" (browser) |
| `instance_admin.rs:82:5` `show_dashboard -> Default::default()` | missed by package → **killed by acceptance** | `iawr`: 27 scenarios (23 passed, 4 failed): "the reloaded dashboard must show the workspace as "Household"" (`left: []`, `right: ["Household"]`), "the dashboard must list workspace …", and two browser scenarios: "the "Bailey Family" row must hold content beside its head …; plant returned Number(-1)" |
| `instance_admin.rs:424:5` `submit_project_rename -> Default::default()` | missed by package → **killed by acceptance** | `iapr`: 21 scenarios (4 passed, 17 failed): "the answer is byte-identical to a never-existed address" (4), "the rename is refused saying "Project name must be unique within the team"" (3), "… must not be empty" (2), "… at most 256 characters", "the row she gets back shows "Sandbox" with key prefix "SBX"…", "that row shows "Identity Platform" without the page reloading" |

**Why the three whole-function handler mutants miss at package level.** This is the same
pattern as keycloak-sso D3a and OD-10. `show_dashboard`, `submit_workspace_rename` and
`submit_project_rename` are async axum handlers over `AppState` (a live store, session and
CSRF). The package covers their pure helpers (`rename_error_fragment`,
`html_with_optional_cookie`). The handlers themselves are reached only through the acceptance
lane, which drives the real binary over Postgres and real Chrome. All three are killed there on
product assertions, not on infrastructure.

## Survivors analysis

None. All 14 viable mutants are killed. The pure classification (`classify_workspace_rename`) is
killed in every operator direction by its property suite and its exact 24/25 boundary example.
The store's no-op guard and the probe's 0018 check are killed in every direction by the
real-Postgres integration tests.

No equivalent mutants were found or excluded.

**Observation (not a gate item, routed to acceptance-designer for decision).** The service's
admin guard (`workspaces.rs:95`, `if !is_admin { return Err(Forbidden) }`) is killed only
indirectly. Its inverted form breaks every admin rename, so acceptance catches it. Nothing pins
the guard's refusal of a non-admin on its own terms:
- the handler's `require_instance_admin` turns non-admins away before the service is called;
- foundry-services has no `rename_workspace` use-case test. Other surfaces have one, for example
  `tests/rename_project_use_case.rs`.

So deleting the guard outright would survive every suite. cargo-mutants does not generate that
mutant, so this report does not count it. The defence-in-depth claim is still untested. The test
that would pin it: a `crates/foundry-services/tests/rename_workspace_use_case.rs` case where a
non-admin `acting_user_id` gets `Err(RenameWorkspaceError::Forbidden)`, and the workspace name
and `workspace_rename_events` are unchanged. A paired case would have an admin get `Renamed`.

## Final state

**Production file hashes, before and after** (`diff $S/sha-before.txt $S/sha-after.txt`:
identical):

```
e96d67a3c1567c6b9fbd94fc5fe8560b454b0ff3584f079480fec5dcedd7b5a1  crates/foundry-store/src/lib.rs
368b5147537d8a525345a43b3c04d77c3685bb67b99ab6d7d07cbeef3a36441d  crates/foundry-services/src/workspaces.rs
ccee1f61050c14e57395bee1edbb64af0aa717b39abd99220fa1a13808409a8c  crates/foundry-app/src/instance_admin.rs
0de58283abee2d5c8a76d996d8937051fbe0e706966cc5baaf9c1613de06541f  crates/foundry-app/src/views.rs
c47197a93591fb2fb2d5e2a75896bb67fa114ce3d524d10fb465ada58c52325e  crates/foundry-app/src/lib.rs
```

- `git status --short`: only this report (untracked, not committed).
- Containers: the set of running containers is the same before and after (`docker ps -q`
  diff empty). The testcontainers Postgres and Selenium instances were removed by the harness.
  Pre-existing containers were not touched.
- The scratch clone `$S/tree` was deleted.
- No test, feature file, step module or production file was changed. `FOUNDRY_STAMP_*` was
  never set.

**Gate verdict: PASS.** 14/14 = 100%, or 71.4% (10/14) on package tests alone. There are no
survivors and no equivalent mutants.
