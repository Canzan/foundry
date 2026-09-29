# Mutation report: keycloak-sso increment D3a (DELIVER Phase 5)

**Date**: 2026-09-27
**Tool**: cargo-mutants 25.3.1 (default copy mode, never `--in-place`)
**Scope**: the production lines D3a changes, selected with `--in-diff` over the uncommitted
`git diff` of the four production files:
- `crates/foundry-app/src/oidc.rs`: `provision`, `judge_newcomer`, `greeting_name`,
  `log_provisioned`, and the callback's find-or-provision branch.
- `crates/foundry-app/src/signin.rs`: `verify_against`, plus the `None`-hash handling in
  `submit_signin` and `submit_change_password`.
- `crates/foundry-oidc/src/lib.rs`: `IdentityClaims::from_id_token`, `has_realm_role`,
  `OidcProvider::provision_role`, `OidcConfig::with_provision_role`.
- `crates/foundry-store/src/lib.rs`: `provision_federated_member`, `find_user_by_id`, and
  `find_user_by_email` (the hash decode).

`--in-diff` also generates whole-function mutants for three functions whose bodies D3a only
touches: `OidcConfig::from_env`, `OidcConfig::from_parts` and `OidcProvider::exchange_code`.
They are kept in the denominator, which is the stricter choice.

**Gate**: kill rate ≥ 80% PASS, 70–80% WARN, < 70% FAIL (`CLAUDE.md`, per-feature).

## Result

| Metric | Value |
|---|---|
| Mutants generated | 33 |
| Unviable (do not compile, excluded) | 8 |
| Viable | 25 |
| Killed by the package's own tests | 21 |
| Killed on re-check against the acceptance lane | 3 |
| Survived | 1 (`OidcConfig::from_env -> Ok(None)`, see below) |
| Timeouts | 0 |
| **Kill rate** | **96.0%** (24/25 viable). Package tests alone: 84.0% (21/25) |
| Equivalent mutants excluded | none |
| **Gate verdict** | **PASS** |

Every kill below was checked in its log. Each one failed on a test assertion, or on
`expect()` inside a test, or on the production invariant noted in the table. None was "caught"
by an infrastructure error: no testcontainers port fetch failure, no sqlx `Protocol('\0')`, no
`Connection reset by peer`.

## Per-file table

| File | Total | Killed | Survived | Timeout | Unviable | Kill rate |
|---|---|---|---|---|---|---|
| `crates/foundry-oidc/src/lib.rs` | 14 | 8 | 1 | 0 | 5 | 88.9% (8/9) |
| `crates/foundry-store/src/lib.rs` | 5 | 2 | 0 | 0 | 3 | 100% (2/2) |
| `crates/foundry-app/src/oidc.rs` | 10 | 10 (8 package + 2 acceptance) | 0 | 0 | 0 | 100% (10/10) |
| `crates/foundry-app/src/signin.rs` | 4 | 4 (3 package + 1 acceptance) | 0 | 0 | 0 | 100% (4/4) |
| **Total** | **33** | **24** | **1** | **0** | **8** | **96.0%** |

## Procedure and exact commands

`S` is the session scratchpad
(`/private/tmp/claude-501/-Users-jeffbailey-Projects-canzan-foundry/f9ce8d1d-…/scratchpad`).

**Safety.** The whole feature is uncommitted, so nothing wrote to git. No `checkout`,
`restore`, `stash`, `reset` or `clean` was run, and cargo-mutants never ran `--in-place`.
cargo-mutants mutates its own copy of the tree under `$TMPDIR`. Where a mutant had to be
applied by hand (the acceptance re-check below), it was applied to a **separate `rsync` copy of
the tree** (`$S/tree`, own `target/`), with a `cp` backup and a `cmp` restore. The working tree
was never mutated.

1. **Snapshot.**
   ```sh
   shasum -a 256 crates/foundry-app/src/oidc.rs crates/foundry-app/src/signin.rs \
     crates/foundry-oidc/src/lib.rs crates/foundry-store/src/lib.rs > $S/sha-before.txt
   ```
2. **Scope diff** (read-only git):
   ```sh
   git diff -- <the four files> > $S/d3a.diff
   cargo mutants --in-diff $S/d3a.diff --list   # 33 mutants
   ```
3. **foundry-oidc**, run against its unit tests:
   ```sh
   cargo mutants --in-diff $S/d3a.diff --file crates/foundry-oidc/src/lib.rs \
     --package foundry-oidc --output $S/oidc -j 2 --timeout 300
   ```
   14 mutants tested in 17s: 8 caught, 1 missed, 5 unviable.
4. **foundry-store**, run against the D3a integration tests over real Postgres
   (testcontainers):
   ```sh
   cargo mutants --in-diff $S/d3a.diff --file crates/foundry-store/src/lib.rs \
     --package foundry-store --output $S/store -j 2 --timeout 600 \
     -- --test nullable_password_hash --test provision_federated_member
   ```
   5 mutants tested in 51s: 2 caught, 3 unviable.
5. **foundry-app**, run against all of its tests: the lib unit tests (74), `csrf_middleware`,
   and `password_less_account_doors`:
   ```sh
   cargo mutants --in-diff $S/d3a.diff --file crates/foundry-app/src/oidc.rs \
     --file crates/foundry-app/src/signin.rs --package foundry-app \
     --output $S/app -j 2 --timeout 600
   ```
   14 mutants tested in 2m 42s: 11 caught, 3 missed.
6. **Acceptance re-check of the 3 foundry-app survivors.**
   - `cargo mutants --test-package foundry-acceptance` did not work in 25.3.1: the debug log
     shows `test_packages=Explicit([foundry-app])`, and `--test-workspace=true` behaved the
     same way. The baseline therefore failed (`no test target named acceptance in foundry-app`),
     and no mutants were tested.
   - Instead, each survivor's **exact cargo-mutants diff** (`$S/app/mutants.out/diff/*.diff`) was
     applied with `patch` in `$S/tree` by `$S/seed.sh`. The script does `cp` backup, `patch`,
     runs the lane, restores with `cp`, then checks with `cmp`.
   - Warm-up and baselines in the copy:
     - `cargo test -p foundry-acceptance --test acceptance --no-run`
     - `FOUNDRY_ACCEPTANCE_TAGS=keycloak-sso`: **15/15 scenarios, 100/100 steps**
     - `FOUNDRY_ACCEPTANCE_TAGS=notification-delivery`: **30/30 scenarios, 168/168 steps**
   - The `keycloak-sso` tag runs every scenario of `keycloak-sso-provisioning.feature`.
7. **Restore proof.** Every seed ended with `cmp` clean. The restored oidc.rs and signin.rs hashed
   to `61fe7c36…` and `2a0421e8…`, the same as the working tree.

## Every mutant and its disposition

### foundry-oidc (`crates/foundry-oidc/src/lib.rs`)

| Mutant | Outcome | Killing test |
|---|---|---|
| `:91:77` delete `!` in `with_provision_role` | killed | `a_blank_or_unset_provision_role_means_provisioning_off` / `a_provision_role_is_carried_through_the_provider` |
| `:218:9` `has_realm_role -> true` | killed | `only_an_exact_realm_role_counts_never_a_client_role` |
| `:218:9` `has_realm_role -> false` | killed | same |
| `:218:49` `==` → `!=` in `has_realm_role` | killed | same |
| `:301:9` `provision_role -> None` | killed | `a_provision_role_is_carried_through_the_provider` |
| `:301:9` `provision_role -> Some("")` | killed | `a_blank_or_unset_provision_role_means_provisioning_off`, `a_provision_role_is_carried_through_the_provider` |
| `:301:9` `provision_role -> Some("xyzzy")` | killed | same two tests |
| `:108:9` `from_parts -> Ok(None)` | killed | 5 tests incl. `partial_config_is_refused_and_names_what_is_missing`, `a_provision_role_is_carried_through_the_provider` |
| `:77:9` `from_env -> Ok(None)` | **SURVIVED** | see below |
| `:77:9` `from_env -> Ok(Some(Default))` | unviable | `OidcConfig` has no `Default` |
| `:108:9` `from_parts -> Ok(Some(Default))` | unviable | same |
| `:91:9` `with_provision_role -> Default` | unviable | same |
| `:206:9` `from_id_token -> Default` | unviable | `IdentityClaims` has no `Default` |
| `:400:9` `exchange_code -> Ok(Default)` | unviable | same |

**Survivor: `OidcConfig::from_env -> Ok(None)`.**
- **Classification.** Accepted survivor, not equivalent. Replacing the body switches OIDC off
  entirely. It is kept in the denominator.
- **Why it survives.** `from_env` is the deliberately untested humble adapter over
  `from_parts`. `lib.rs:96-100` records the design decision: `std::env::set_var` is
  process-global while tests run in parallel threads, so every rule lives in the pure
  `from_parts`, and no test touches the environment. The acceptance harness also builds its
  config through `from_parts` (`feature_keycloak_sso_provisioning.rs:173`). No lane reaches
  `from_env`. The mutant is not reachable without an env-based test, which the crate's design
  rules out.
- **Residual risk D3a adds.** D3a adds one line to `from_env`,
  `.map(|cfg| cfg.map(|c| c.with_provision_role(read("FOUNDRY_OIDC_PROVISION_ROLE"))))`. No test
  pins the variable name. A typo would ship with provisioning silently off, which fails safe:
  link-only, exactly D3. `with_provision_role`'s blank-means-off rule is fully covered.
- **Candidate fix, for the owner.** Give `from_env` a pure lookup seam, e.g.
  `from_lookup(impl Fn(&str) -> Option<String>)`, so the variable names can be unit-tested
  without `set_var`. That is a production change, out of scope for Phase 5, and was not made.

### foundry-store (`crates/foundry-store/src/lib.rs`)

| Mutant | Outcome | Killing test (assertion) |
|---|---|---|
| `:1101:9` `find_user_by_email -> Ok(None)` | killed | `nullable_password_hash.rs:85`: "the password-less account is found by email" |
| `:2721:9` `find_user_by_id -> Ok(None)` | killed | `nullable_password_hash.rs:94`: "the password-less account is found by id" |
| `:1101:9` `find_user_by_email -> Ok(Some(Default))` | unviable | `UserRow` has no `Default` |
| `:2721:9` `find_user_by_id -> Ok(Some(Default))` | unviable | same |
| `:510:9` `provision_federated_member -> Ok(Default)` | unviable | `FederatedProvisionOutcome` has no `Default` |

cargo-mutants 25.3.1 generates no mutants inside `provision_federated_member`: SQL strings,
`?` and `let … else` are not mutated. The body was therefore probed with a supplementary
hand-seeded fault (H4, below).

### foundry-app (`crates/foundry-app/src/oidc.rs`, `signin.rs`)

| Mutant | Outcome | Killing test (assertion) |
|---|---|---|
| `oidc.rs:277:5` `judge_newcomer -> Ok(())` | killed (package) | `a_newcomer_is_refused_unless_provisioning_is_on_and_the_role_is_held`: `provision role None … left: Ok(())` |
| `oidc.rs:289:5` `greeting_name -> None` | killed (package) | `the_greeting_is_the_first_usable_candidate`: `left: None` |
| `oidc.rs:289:5` `greeting_name -> Some("")` | killed (package) | same, `left: Some("")` |
| `oidc.rs:289:5` `greeting_name -> Some("xyzzy")` | killed (package) | same, `left: Some("xyzzy")` |
| `oidc.rs:298:15` delete `!` in `greeting_name` | killed (package) | same, `left: None` |
| `oidc.rs:298:29` `&&` → `\|\|` in `greeting_name` | killed (package) | same, row `Some("   ")`, `left: Some("")` |
| `oidc.rs:298:50` `<=` → `>` in `greeting_name` | killed (package) | same, `left: None` |
| `oidc.rs:304:5` `log_provisioned -> ()` | killed (package) | `a_provision_logs_the_user_and_workspace`: "exactly one line", `left: 0` |
| `oidc.rs:245:5` `provision -> Ok(Default::default())` | missed by package → **killed by acceptance** | `keycloak-sso`: 15 scenarios (6 passed, 9 failed). E.g. "Then the newcomer arrives signed in to the board": `expected a redirect onto the board; got 401`, `left: 401`, `right: 303` |
| `oidc.rs:170:5` `callback -> Default::default()` | missed by package → **killed by acceptance** | `keycloak-sso`: 15 scenarios (15 failed): `expected a redirect onto the board; got 200 OK`, `left: 200`, `right: 303` |
| `signin.rs:594:5` `verify_against -> false` | killed (package) | `forgot_password_gives_a_password_less_account_a_password_the_form_then_accepts`: `left: 401` |
| `signin.rs:594:5` `verify_against -> true` | killed (package) | `the_password_form_refuses_a_password_less_account_exactly_like_an_unknown_address`: the unknown-address attempt now "verifies", and production's own invariant `expect("verified implies user row found")` (`signin.rs:147`) panics. This is a genuine fault signal, not infrastructure. |
| `signin.rs:84:5` `submit_signin -> Default::default()` | killed (package) | `password_less_account_doors.rs:218`: `left: 200` (expected 401) |
| `signin.rs:295:5` `submit_change_password -> Default::default()` | missed by package → **killed by acceptance** | `notification-delivery`: 30 scenarios (27 passed, 3 failed). "the password change is refused as unauthorized": `left: 200`, `right: 401`. "…refused as a bad request": `left: 200`, `right: 400`. "delivered through the log provider": `got 0` |

## Supplementary hand-seeded faults (outside the cargo-mutants denominator)

cargo-mutants generated no mutant for three things:
- the `None`-hash arm in `submit_change_password`;
- the callback's find-or-provision outcome mapping;
- the SQL in `provision_federated_member`.

These are named in the brief, so each was probed with one hand-seeded fault. All were applied
in `$S/tree` only, with `cp` backup and `cmp` restore.

| Fault | Seed | Before test addition | After |
|---|---|---|---|
| **H1** a password-less account passes change-password reauth | `signin.rs:328`: `let reauthenticated = stored_hash.is_none() \|\| verify_against(…)` | **SURVIVED.** foundry-app 74+3+2 green, `keycloak-sso` 15/15 green. Real gap: a signed-in SSO-only session alone could set a password. | **KILLED** by the new test: `a password-less account cannot reauthenticate with "any-password-at-all"`, `left: 200`, `right: 401` |
| **H2** the race loser (`Existing`) is refused instead of signed in | `oidc.rs:262`: `Existing { .. } => Err(ProvisionFailure::Refused(NO_ACCOUNT))` | **SURVIVED.** foundry-app green, `keycloak-sso` 15/15 green. | Open, see below |
| **H4** provision into the newest workspace | `lib.rs:513`: `ORDER BY created_at DESC, id` | **KILLED**: `provisions_a_password_less_member_into_the_original_workspace`, "provisioned into the original workspace" | n/a |

**H2 disposition: open gap, documented, no test added.**
- `Existing` is reached only when a concurrent first sign-in inserts the address between the
  callback's `find_user_by_email` (returns `None`) and `provision_federated_member`'s
  `ON CONFLICT DO NOTHING`.
- The store half is covered: `concurrent_first_sign_ins_share_one_account`.
- The app half, "both callers sign in", needs that interleaving to be deterministic. `Store` is a
  concrete type with no port to double, and racing two real callbacks would be a flaky test,
  which is forbidden.
- Candidate fix, for the owner: a store seam, or an acceptance step that pre-inserts the row
  between the two queries.

## Test added

`crates/foundry-app/tests/password_less_account_doors.rs` is the existing D3a test file for the
password doors. It is not a new file.

**The new test: `a_signed_in_password_less_account_cannot_set_a_password_by_changing_it`** (DDD-21).
- **Setup.**
  - Seed the password-less member.
  - Give it a temporary hash only long enough to obtain a real signed-in session through
    `/sign-in`.
  - NULL the hash. That is the state a federated sign-in leaves.
- **Action and assertions.** `POST /account/password` with the current password set to a guess,
  to `""`, and to the old password. Each attempt must get **401**. The 401 also proves the
  session is valid: a signed-out caller gets 404.
- **Final state.** The hash stays NULL, and no notification is sent.
- **Refactoring.** `post_form` was refactored behind a new `post_form_as`, which sends session
  cookies and returns the cookies set. `password_hash_of` was added. The module doc gained one
  bullet.
- **Falsifiability.** Green on the unmutated tree, red under H1 (above).
- **File hashes.** sha256 before `19b8aed3…1a62`, after `cdc13182…66f3b9`
  (+108/−2 lines).

No production file, feature file or step module was changed.

## Final state

**Production file hashes, before and after** (`diff $S/sha-before.txt $S/sha-after.txt`:
identical):

```
61fe7c36827517e333df3ec91cae649f4514a76f1de1cc7203e5d1c37428fb31  crates/foundry-app/src/oidc.rs
2a0421e8e327ed8a358965a53fe08139ad295633cf0e00a38665637f242d9013  crates/foundry-app/src/signin.rs
1961b8cf36c06d6432de510ee14b5f70f905bc9af8ce8d962d36f087ab59555a  crates/foundry-oidc/src/lib.rs
e510ec0bc9990de6846306bcc92d1eeebcf5b5caa17a8f3d671f8d6a6e71fc78  crates/foundry-store/src/lib.rs
```

**Checks, all green:**
- `cargo test --workspace --exclude foundry-acceptance`: all test binaries pass, 0 failed.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.

**Gate verdict: PASS.** 24/25 = 96.0%, or 84.0% on package tests alone.
- One accepted survivor: `from_env -> Ok(None)`, the untested env adapter, by design.
- Two supplementary faults, H1 and H4, are killed. H1 was killed by the test added here.
- H2 is an open, documented gap outside the gate's denominator.
