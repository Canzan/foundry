# Mutation report: keycloak-sso phase 04, OD-10 / US-07 (DELIVER Phase 5)

**Date**: 2026-10-04
**Tool**: cargo-mutants 25.3.1 (default copy mode, never `--in-place`)
**Scope**: the production lines phase 04 changes, selected with `--in-diff` over
`git diff 0ea878f HEAD -- crates/foundry-app/src/oidc.rs crates/foundry-store/src/lib.rs`
(HEAD = `182b0a8`; commits `fdf3346`, `25c6ab3`, `24a209d`, `44262c8`, `a901f19`, `182b0a8`):
- `crates/foundry-app/src/oidc.rs`: `judge_returning` (new), the callback's step-1 split
  (an existing account now passes `judge_returning` before linking), the
  `PROVISIONED_LACKS_PROVISION_ROLE` reason, and `provision` passing `state.clock.now()`.
- `crates/foundry-store/src/lib.rs`: `USER_ROW_COLUMNS` and the `UserRow.provisioned` field,
  `find_user_by_email` / `find_user_by_id` (now built from that shared projection),
  `provision_federated_member`'s `now` parameter and `provisioned_at` INSERT, and
  `Store::probe`'s migration-0017 column check.

`--in-diff` also generates whole-function mutants for `callback`, `provision`,
`Store::probe` and `Store::provision_federated_member`, whose bodies phase 04 only touches.
They are kept in the denominator, which is the stricter choice.

Not mutated: migration 0017's SQL (cargo-mutants does not mutate SQL). The backfill was
already probed with named faults during the 04-01 step: the OQ-6 test killed "backfill uses
`now()`" and "marks password accounts". cargo-mutants also generates nothing for the
`USER_ROW_COLUMNS` string, the INSERT text or `.bind(now)`; those are pinned by
`users_provisioned_at.rs` and `provision_federated_member.rs` (green in every run below).

**Gate**: kill rate ≥ 80% PASS, 70–80% WARN, < 70% FAIL (`CLAUDE.md`, per-feature).

## Result

| Metric | Value |
|---|---|
| Mutants generated | 16 |
| Unviable (do not compile, excluded) | 4 |
| Viable | 12 |
| Killed by the package's own tests | 10 |
| Killed on re-check against the acceptance lane | 2 |
| Survived | 0 |
| Timeouts | 0 |
| **Kill rate** | **100%** (12/12 viable). Package tests alone: 83.3% (10/12) |
| Equivalent mutants excluded | none |
| **Gate verdict** | **PASS** |

Every kill below was checked in its log. Each one failed on a test assertion or an
`expect()` in a test that exercises the mutated code. None was "caught" by an infrastructure
error: grepping every mutant log and both acceptance logs for `PortNotExposed`, `SSLRequest`,
sqlx `Protocol(…)` and `Connection reset` finds nothing.

## Per-file table

| File | Total | Killed | Survived | Timeout | Unviable | Kill rate |
|---|---|---|---|---|---|---|
| `crates/foundry-store/src/lib.rs` | 9 | 5 | 0 | 0 | 4 | 100% (5/5) |
| `crates/foundry-app/src/oidc.rs` | 7 | 7 (5 package + 2 acceptance) | 0 | 0 | 0 | 100% (7/7) |
| **Total** | **16** | **12** | **0** | **0** | **4** | **100%** |

## Procedure and exact commands

`S` is the session scratchpad
(`/private/tmp/claude-501/-Users-jeffbailey-Projects-canzan-foundry/9298978f-…/scratchpad`).

**Safety.** Nothing wrote to git. No `checkout`, `restore`, `stash`, `reset` or `clean` was
run, and cargo-mutants never ran `--in-place`. cargo-mutants mutates its own copy of the tree
under `$TMPDIR`. The acceptance re-check applied mutants to a **separate `rsync` copy of the
tree** (`$S/tree`, own `target/`), with a `cp` backup and a `cmp` restore. The working tree was
never mutated.

1. **Snapshot.**
   ```sh
   shasum -a 256 crates/foundry-app/src/oidc.rs crates/foundry-store/src/lib.rs > $S/sha-before.txt
   ```
2. **Scope diff** (read-only git):
   ```sh
   git diff 0ea878f HEAD -- crates/foundry-app/src/oidc.rs crates/foundry-store/src/lib.rs > $S/od10.diff
   cargo mutants --in-diff $S/od10.diff --list   # 16 mutants
   ```
3. **foundry-store**, run against the four phase-04/D3a integration tests over real Postgres
   (testcontainers):
   ```sh
   CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false timeout 3600 \
   cargo mutants --in-diff $S/od10.diff --file crates/foundry-store/src/lib.rs \
     --package foundry-store --output $S/store -j 2 --timeout 600 \
     -- --test users_provisioned_at --test probe_schema_scoping \
        --test provision_federated_member --test nullable_password_hash
   ```
   Baseline green (25.3 s build + 6.4 s test). 9 mutants tested in 1m 25s: 5 caught,
   4 unviable.
4. **foundry-app**, run against all of its tests (lib unit tests, `backup_verify_fail_closed`,
   `csrf_middleware`, `password_less_account_doors`):
   ```sh
   CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false timeout 3600 \
   cargo mutants --in-diff $S/od10.diff --file crates/foundry-app/src/oidc.rs \
     --package foundry-app --output $S/app -j 2 --timeout 600
   ```
   Baseline green (33.3 s build + 17.2 s test). 7 mutants tested in 2m: 5 caught, 2 missed.
5. **Acceptance re-check of the 2 foundry-app survivors**, as in D3a: each survivor's
   **exact cargo-mutants diff** (`$S/app/mutants.out/diff/*.diff`) was applied with
   `patch -p0` in `$S/tree` by `$S/seed.sh`. For each run, the script:
   - backs up `oidc.rs` with `cp` and applies the patch;
   - rebuilds the `foundry` binary (`cargo build -p foundry-app --bin foundry`) and the
     acceptance test binary (`--no-run`);
   - warms the binary (three `foundry --version` execs);
   - runs `FOUNDRY_ACCEPTANCE_TAGS=keycloak-sso-provisioning timeout 1500 cargo test -p foundry-acceptance --test acceptance`;
   - restores with `cp` and checks with `cmp` against the working tree.

   The three runs went one after another, wrapped in `timeout 3500`, with no other lane
   running. **Baseline (unmutated copy): 23/23 scenarios, 171/171 steps.**
6. **Restore proof.** All three seeds ended with `RESTORED_CMP_OK`. After the whole run:
   - `git diff --stat -- crates/` is empty;
   - `diff $S/sha-before.txt $S/sha-after.txt` is identical (hashes below).

## Every mutant and its disposition

### foundry-store (`crates/foundry-store/src/lib.rs`)

| Mutant | Outcome | Killing test (assertion) |
|---|---|---|
| `:276:21` `<` → `<=` in `Store::probe` | killed | `probe_column_check_is_scoped_to_current_schema` (`probe_schema_scoping.rs:128`: "probe must PASS when the active schema has the migration-0006 columns", got `Failed("users table missing migration-0017 column provisioned_at")`), and `probe_refuses_a_schema_without_the_provisioned_marker` (`:174`, "probe must PASS when the active schema has users.provisioned_at") |
| `:276:21` `<` → `==` in `Store::probe` | killed | the same two tests: the current schema is refused (`:128`), and the pre-0017 schema is accepted (`:164`, "probe must FAIL against a schema lacking users.provisioned_at", got `ProbeReport { select_one_ok: true, … }`) |
| `:276:21` `<` → `>` in `Store::probe` | killed | `probe_refuses_a_schema_without_the_provisioned_marker` (`:164`): the pre-0017 schema passes the probe |
| `:1150:9` `find_user_by_email -> Ok(None)` | killed | `nullable_password_hash.rs:85`: "the password-less account is found by email" |
| `:2770:9` `find_user_by_id -> Ok(None)` | killed | `nullable_password_hash.rs:94`: "the password-less account is found by id" |
| `:207:9` `Store::probe -> Ok(Default::default())` | unviable | `ProbeReport` has no `Default` |
| `:532:9` `provision_federated_member -> Ok(Default::default())` | unviable | `FederatedProvisionOutcome` has no `Default` |
| `:1150:9` `find_user_by_email -> Ok(Some(Default::default()))` | unviable | `UserRow` has no `Default` |
| `:2770:9` `find_user_by_id -> Ok(Some(Default::default()))` | unviable | same |

### foundry-app (`crates/foundry-app/src/oidc.rs`)

| Mutant | Outcome | Killing test (assertion) |
|---|---|---|
| `:300:5` `judge_returning -> Ok(())` | killed (package) | `a_returning_account_is_refused_only_when_provisioned_and_the_role_is_missing`: row `provisioned true, provision role Some("foundry-user"), held []`, `left: Ok(())`, `right: Err("provisioned account lacks provision role")` |
| `:301:23` match guard → `true` | killed (package) | same test, row `provisioned false, … held []`, `left: Err(…)`, `right: Ok(())` |
| `:301:23` match guard → `false` | killed (package) | same test, row `provisioned true, … held []`, `left: Ok(())`, `right: Err(…)` |
| `:301:35` `&&` → `\|\|` | killed (package) | same test, row `provisioned false, … held []`, `left: Err(…)`, `right: Ok(())` |
| `:301:38` delete `!` | killed (package) | same test, row `provisioned true, … held ["foundry-user"]`, `left: Err(…)`, `right: Ok(())` |
| `:252:5` `provision -> Ok(Default::default())` | missed by package → **killed by acceptance** | `keycloak-sso-provisioning`: 23 scenarios (8 passed, 15 failed). All 15 failed with `expected a redirect onto the board; got 401 Unauthorized`, `left: 401`, `right: 303`. The failing steps were "And the newcomer has been given an account through the identity provider" (11) and "Then the newcomer arrives signed in to the board" (4) |
| `:170:5` `callback -> Default::default()` | missed by package → **killed by acceptance** | `keycloak-sso-provisioning`: 23 scenarios (23 failed). 18 failed with `expected a redirect onto the board; got 200 OK`, `left: 200`, `right: 303`. The other 5 failed at "Then the newcomer is turned away exactly as a wrong password is" |

**Why the two whole-function mutants miss at package level.** This is the same pair, for the
same reason, as in D3a. `callback` and `provision` are async handlers over a live OIDC
provider and the store. The package covers their pure decision functions (`judge_returning`,
`judge_newcomer`, `greeting_name`); the handlers themselves are reached only through the
acceptance lane, which drives a real Keycloak stand-in and Postgres. Both are killed there on
product assertions, not on infrastructure.

## Survivors analysis

None. The phase-04 decision logic (`judge_returning`, all five mutants) is killed by its
10-row unit table, which fixes each of the three inputs while varying the others. The
`Store::probe` 0017 check is killed in all three operator directions: `<` → `<=` and `==`
refuse the current schema, and `>` accepts a pre-0017 one. Both `UserRow` lookups are killed.

No equivalent mutants were found or excluded.

## Final state

**Production file hashes, before and after** (`diff $S/sha-before.txt $S/sha-after.txt`:
identical):

```
cf6f57070b297db072ec441c802cda5c65db6ce1ffc26ced67f4c5a9ed018989  crates/foundry-app/src/oidc.rs
e2eea218b2fff3afe258acba4475ef32081834798b6988ce1e9f4d81ec828e38  crates/foundry-store/src/lib.rs
```

- `git diff --stat -- crates/`: empty.
- Containers: the set of running containers is the same before and after (the testcontainers
  Postgres instances were removed by their own drop). Pre-existing containers were not
  touched.
- No test, feature file, step module or production file was changed. This report is the only
  file written in the repository.

**Gate verdict: PASS.** 12/12 = 100%, or 83.3% (10/12) on package tests alone. There are no
survivors and no equivalent mutants.
