# instance-integration-setup — mutation report (DELIVER Phase 5)

Tool: cargo-mutants 27.1.0, in the Lima VM `foundry-test`. Scope: the feature's
changed lines only (`--in-diff`), base `9aa38d9^`, head `158b70c` plus the tests
added in this pass. Threshold 80% (>=80 PASS, 70-80 WARN, <70 FAIL).
Kill rate = caught / (caught + missed); unviable mutants (do not compile) are
excluded.

## Verdict

**PASS — 315 caught, 32 missed: 90.8%** (second pass). The first pass ended
at WARN, 264 caught and 83 missed (76.1%); the feature requires 80%, so a
second pass added unit tests to mod.rs, reload.rs, handlers/instance.rs and
slack_app.rs and re-ran cargo-mutants on those files and keys.rs. Every
survivor is listed below with a disposition. Item 41 (key_file.rs) moved from
60.5% to 89.5%. The three B2 proofs are each killed by a named test.

## Per file

| File | Caught | Missed | Unviable | Timeout | Kill rate |
|---|---:|---:|---:|---:|---:|
| foundry-app/src/admin_cli.rs | 8 | 0 | 0 | 0 | 100% |
| foundry-app/src/integrations/config.rs | 17 | 0 | 8 | 0 | 100% |
| foundry-app/src/integrations/handlers/instance.rs | 48 | 12 | 8 | 0 | 80.0% (pass 1: 42 / 18) |
| foundry-app/src/integrations/key_file.rs | 34 | 4 | 15 | 0 | 89.5% |
| foundry-app/src/integrations/keys.rs | 49 | 0 | 16 | 0 | 100% (pass 1: 46 / 3) |
| foundry-app/src/integrations/mod.rs | 71 | 9 | 26 | 0 | 88.8% (pass 1: 38 / 42) |
| foundry-app/src/integrations/reload.rs | 19 | 0 | 6 | 0 | 100% (pass 1: 12 / 7) |
| foundry-app/src/integrations/secrets_key_seam.rs | 0 | 4 | 0 | 0 | 0% |
| foundry-app/src/integrations/slack.rs | 2 | 0 | 0 | 0 | 100% |
| foundry-app/src/integrations/slack_app.rs | 13 | 0 | 10 | 0 | 100% (pass 1: 11 / 2) |
| foundry-auth/src/secret_box.rs | 3 | 0 | 6 | 0 | 100% (with foundry-app lib tests; 1 caught, 2 missed on foundry-auth lib alone) |
| foundry-store/src/integrations/connections.rs | 1 | 0 | 1 | 0 | 100% |
| foundry-store/src/integrations/instance.rs | 50 | 3 | 18 | 0 | 94.3% |
| **Total** | **315** | **32** | **114** | **0** | **90.8%** (pass 1: 264 / 83, 76.1%) |

key_file.rs before this pass (same command, run 1): 23 caught, 15 missed,
15 unviable — 60.5%.

## Commands

```bash
# on the Mac: sync the tree into the VM (scripts/test-in-lima.sh), then in the VM:
git diff 9aa38d9^..158b70c -- <the 13 target files> > ~/mut/feature.diff
# key_file.rs re-run after the new tests: the diff was regenerated on the Mac
# against the working tree (git diff 9aa38d9^ -- crates/foundry-app/src/integrations/key_file.rs)
export CARGO_BUILD_JOBS=4 CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false TMPDIR=$HOME/mut/tmp
# foundry-app groups (key_file / keys / mod / handlers / rest), each one run:
timeout 3600 cargo mutants --in-diff ~/mut/feature.diff -p foundry-app --file <files> \
  --jobs 2 --timeout 300 --output ~/mut/<group> \
  -- --lib --test iis_reload --test iis_admin_cli_rotate --test eip_phase5_doors
# foundry-store:
timeout 3600 cargo mutants --in-diff ~/mut/feature.diff -p foundry-store \
  --file crates/foundry-store/src/integrations/connections.rs \
  --file crates/foundry-store/src/integrations/instance.rs \
  --jobs 2 --timeout 300 --output ~/mut/store -- --lib --test iis_instance_store --test eip_store
# foundry-auth:
timeout 3600 cargo mutants --in-diff ~/mut/feature.diff -p foundry-auth \
  --file crates/foundry-auth/src/secret_box.rs --jobs 2 --timeout 300 --output ~/mut/auth -- --lib
# foundry-auth again, with the consumer crate's tests (parse_list is exercised there):
timeout 1800 cargo mutants --in-diff ~/mut/feature.diff -p foundry-auth \
  --file crates/foundry-auth/src/secret_box.rs --test-package foundry-auth,foundry-app \
  --jobs 2 --timeout 300 --output ~/mut/auth_app -- --lib
```

Second pass (same flags, same test command; the diff regenerated on the Mac
against the working tree, `git diff 9aa38d9^ -- <files>`, since the files'
test modules grew; production lines are unchanged, and cargo-mutants does not
mutate `#[cfg(test)]` code):

```bash
cargo mutants --in-diff <diff> -p foundry-app --file .../mod.rs          # 1946 s
cargo mutants --in-diff <diff> -p foundry-app --file .../keys.rs         # 1117 s
cargo mutants --in-diff <diff> -p foundry-app --file .../reload.rs --file .../slack_app.rs   # 729 s
cargo mutants --in-diff <diff> -p foundry-app --file .../handlers/instance.rs                # 1811 s
# after one more mod.rs test (with_key_check), a targeted re-run:
cargo mutants --in-diff <diff> -p foundry-app --file .../mod.rs --re with_key_check  # 10 mutants: 6 caught, 2 missed, 2 unviable
```

The mod.rs row is the full run (69 caught, 11 missed) with the two
`with_key_check` mutants (758:12, 761:41) moved to caught by the targeted
re-run; its two misses there are the `with_saved_slack` fields already
counted missed.

`rest` = admin_cli.rs, config.rs, reload.rs, secrets_key_seam.rs, slack.rs,
slack_app.rs. TMPDIR is moved off the VM's 12 GB tmpfs `/tmp`: the first run
there filled it (`No space left on device`) and reported 51 of 53 key_file
mutants unviable; that run is discarded. No run hit its timeout (longest:
store, 3147 s). The acceptance lanes are not in any test command.

Wall times: key_file 755 s (run 3), store 3147 s, auth 31 s, keys 1002 s, mod 2448 s,
handlers 2321 s, rest 1473 s.

## B2 proofs

| Proof | Mutant | Killed by |
|---|---|---|
| a. r3 guard in `Integrations::boot` (`may_make`, mod.rs:265) | cargo-mutants `replace && with \|\| in Integrations::boot` (265:48) | `integrations::key_file::tests::a_stored_key_with_no_file_mints_none_and_reads_unopenable` |
| a. (hand-seeded) | `let may_make = true \|\| …` (a file minted over any snapshot) | same test (lib run: 69 passed, 1 failed) |
| b. re-wrap CAS (`update_wrapped_key`, store instance.rs) | `update_wrapped_key -> Ok(true)` (652:5) | `rotation_whose_rewrap_matches_no_row_changes_nothing`, `replicas_racing_the_rewrap_record_exactly_one_system_line`, `rotation_rewraps_and_reseals_all_or_nothing` |
| b. | `rows_affected() == 1` -> `!=` (666:29); `delete !` in `rewrap_secrets_key` (342:12) | the same three; 342 by `replicas_racing_the_rewrap_record_exactly_one_system_line` |
| b. (hand-seeded) | SQL predicate `AND wrapped_by_key_id = $2` replaced with `AND $2::text IS NOT NULL` (key id only) | `replicas_racing_the_rewrap_record_exactly_one_system_line` (both racers re-wrap, line 616) and `rotation_whose_rewrap_matches_no_row_changes_nothing` (line 590) |
| c. link EPERM/ENOTSUP -> I-27 (key_file.rs `attempt`) | guard `err.kind() == AlreadyExists` -> `true` (204:29; EPERM read as a lost race) | `a_filesystem_without_hard_links_is_unwritable_and_keeps_nothing`, `a_link_the_filesystem_refuses_is_tried_once` (new) |
| c. | guard `err.kind() == NotFound` -> `true` (212:39; EPERM retried as a temp file gone) | `a_link_the_filesystem_refuses_is_tried_once` (new; one link call, not two) |

Hand-seeded mutants were applied with `sed` in the VM tree, the named tests
run, and the files restored from a copy (`git diff --stat` empty afterwards).

## Item 41 (key_file.rs)

| Miss in run 1 | Now |
|---|---|
| 46:75 `STALE_TEMP` `10 * 60` -> `10 + 60` | killed by `only_temp_files_ten_minutes_old_are_leftovers` (new) |
| 90:25 non-NotFound stat error -> Unreadable (guard -> `true`) | killed by `a_key_file_that_cannot_be_looked_up_is_unreadable` (new; a 300-byte name, ENAMETOOLONG for any user) |
| 141 wide-mode warning (`!=`->`==`, `&`->`\|`, `&`->`^`) | killed by `only_a_file_readable_beyond_its_owner_is_warned_about` (new; tracing capture) |
| 148 `delete !`, 170 `-> false`, 171 `==`->`!=` (other-owner warning) | killed by `only_a_file_readable_beyond_its_owner_is_warned_about` |
| 212:39 guard -> `true` (EPERM retried) | killed by `a_link_the_filesystem_refuses_is_tried_once` (new) |
| 226 fresh-file check (guard -> `true`, `&&`->`\|\|`) | killed by `a_made_file_that_fails_its_check_is_unwritable` (new; link seam widens the linked file to 0644) |
| EEXIST lost race (204) | already killed (`losing_the_link_race_reads_the_winners_file`) |

Remaining key_file survivors are dispositioned below.

## Surviving mutants

Disposition key: **equivalent** (no observable behaviour differs), **accepted**
(observable, but not reachable by the configured unit/integration test
command; listed as a unit-strength gap for nw-acceptance-designer, not closed
here).

### key_file.rs (4)

| Mutant | Disposition |
|---|---|
| 170:9 `owned_as_its_directory -> true` | accepted: killing it needs a key file owned by another user than its directory, which an unprivileged test cannot create (chown needs root). |
| 288:5 `sync_dir -> ()` | equivalent for any test: the directory fsync only matters across a power loss; nothing in-process can observe it. |
| 301:5 `key_buffer -> Zeroizing::new(String::new())` | equivalent: capacity only. The pre-sized buffer avoids a reallocation that would leave an unzeroed copy of the key; contents and results are identical. |
| 301:5 `key_buffer -> Zeroizing::from(String::new())` | equivalent: same as above. |

### foundry-store instance.rs (3)

| Mutant | Disposition |
|---|---|
| 82:9 `StoredSecretsKey::read_for_test_seam -> Ok(None)` | accepted: the acceptance test seam's reader; only the acceptance lane (KPI-IIS-G1 sweep) calls it. |
| 510:9 `Store::instance_activity -> Ok(vec![])` | accepted: the page's activity read; no store test reads it back (the tests read the audit table directly). |
| 603:5 `lock_exclusive -> Ok(())` | accepted: the exclusive lock serialises instance writes; the racing tests also pass under the compare-and-set alone, so the lock is not separately observable without an injected interleaving. |

### foundry-auth secret_box.rs (0 after the cross-crate run)

`parse_list` 116:9 -> `Ok(vec![])` and 119:29 `delete !` survive the foundry-auth lib tests but are killed by `integrations::config::tests::retired_keys_are_held_under_either_previous_name` (foundry-app) in the `auth_app` run (9 mutants: 3 caught, 6 unviable). The total counts them caught.

### mod.rs (9)

| Mutant | Disposition |
|---|---|
| 419:13 delete arm `(AppInEffect::Environment, _)` in `with_saved_slack` | accepted: needs a saved Slack app row sealed under a held key plus an environment app; no unit test builds one. |
| 420:13 delete arm `(AppInEffect::Saved, Some(...))` | accepted: same. |
| 432:13 delete field `slack` in `with_saved_slack` | accepted: same. |
| 434:13 delete field `saved_app` in `with_saved_slack` | accepted: same. |
| 452:9 `saved_slack -> None` | accepted: same. |
| 494:9 `log_secrets_verdict -> ()`; 494:12, 494:40 `delete !`; 494:37 `&&` -> `\|\|` | accepted: an info line only (no key mode, alert or result depends on it). |

### handlers/instance.rs (12)

All accepted: axum handler bodies and their store reads, reached only
through a running app (the acceptance lane), not by the unit tests:
`show_instance` 409:5, `move_to_secrets_key` 430:5, `save_slack_app` 513:5
-> `Default`; `save_slack` 557:5 -> `None`, `Some("xyzzy")`, 576:51 guard ->
`true`, `false`; `saver_name` 615:5 -> `None`, `Some("")`, `Some("xyzzy")`;
`direct_seals` (the handler's store read) 631:5 -> `0`, `1`.

### secrets_key_seam.rs (4)

All accepted: the acceptance-only test seam (`test-support` builds only).
`generated_secrets_key` 26:5 -> `None`, `Some("")`, `Some("xyzzy")`;
43:31 `==` -> `!=`.

### Killed in the second pass

| Mutants | Killed by |
|---|---|
| mod.rs `proves_key` 802:5 -> `false`, 805:42 and 812:66 `==` -> `!=`; `check_secrets` 548:9 -> `Default` | `integrations::tests::the_probe_proves_a_held_key_only_by_a_check_that_opens_to_the_canary` |
| mod.rs `every_credential_opens` 620 (-> `true`, `false`, `&&` -> `\|\|`, `delete !`); keys.rs `CredentialBox::holds_a_key` 400:9 -> `false` | `every_credential_opens_only_with_a_key_and_nothing_missing` |
| mod.rs `runs_worker` 650 (-> `true`, `false`, `&&` -> `\|\|`, `\|\|` -> `&&`) | `the_worker_runs_only_when_on_and_able_to_seal_or_open` |
| mod.rs `open_credential` 658:9 -> `Some(Default)`, 661:29 `&&` -> `\|\|`, 661:32 `delete !`; `reload_nudge` 669:9 | `a_credential_under_a_key_not_held_nudges_the_reload` |
| mod.rs `direct_seals` 584 (-> `0`, `1`, `!=` -> `==`) | `direct_seals_count_the_held_key_ids_that_seal_stored_values` |
| mod.rs `rewrap_file_key` 730:5 -> `true`, `false`, 734:48 `==` -> `!=`; `boot` 244:17 delete arm `Held`, 292:29 guard -> `true`, `false`, `delete !`; `probed` 389:13; `rewrapped_from` 441:9 (x3) | `boot_rewraps_a_file_wrapped_secrets_key_under_the_operator_key` |
| mod.rs `sealed_under` 522:9 -> `None`; keys.rs `resolve_holding` 303:48 `==` -> `!=`, `Keyring::secrets_key` 340:9 -> `None` | `a_held_secrets_key_survives_a_rewrap_under_a_key_this_replica_lacks` |
| mod.rs `with_key_check` 758:12 `delete !`, 761:41 `==` -> `!=` | `a_recorded_key_check_joins_the_snapshot_only_for_a_new_key_id` |
| reload.rs `Reload::run` 108:9 -> `()` | `integrations::reload::tests::the_reload_task_ticks_when_it_runs` |
| reload.rs `nudge_due` 129:9 -> `true`, `false` | `a_nudge_is_due_once_a_second` |
| reload.rs `tick` 152:59 `>=` -> `<` | `a_tick_with_nothing_to_apply_beats_every_ten_seconds` |
| reload.rs `replica_state` 196:33, `key_mode_name` 203:5 (x2) | `a_replica_state_names_its_key_mode` |
| slack_app.rs `plan_save` 205:5 -> `None`, 215:29 `==` -> `!=` | `integrations::slack_app::tests::a_save_seals_both_secrets_fenced_only_when_sealed` |
| handlers/instance.rs `facts_of` 341:36 (`>` -> `==`, `>=`), 350:52 `==` -> `!=` | `the_page_facts_follow_the_integrations_in_effect` |
| handlers/instance.rs `refused` 542:5 -> `""`, `"xyzzy"` | `a_refused_save_names_its_field` |

## Tests added

Pass 1, crates/foundry-app/src/integrations/key_file.rs `mod tests`:

- `only_a_file_readable_beyond_its_owner_is_warned_about`
- `only_temp_files_ten_minutes_old_are_leftovers`
- `a_key_file_that_cannot_be_looked_up_is_unreadable`
- `a_made_file_that_fails_its_check_is_unwritable`
- `a_link_the_filesystem_refuses_is_tried_once`
- helper `logged`: captures the probe's tracing output (same pattern as `oidc.rs`'s provision-log test) and rebuilds the callsite interest cache inside its scope.

Run 2 of key_file showed, under `cargo test --release` (smoke), the log
assertion in the link test flaky with parallel tests (4 of 20 runs: the
`unwritable` callsite is shared with other tests, and its cached interest
raced). That assertion and the made-file test's log assertion were removed;
the wide-mode test keeps its log assertions (its callsites are reached by two
tests only). Run 3 of key_file, on the final tests, gives the key_file numbers
above (34 caught, 4 missed, 15 unviable, 755 s), with every kill named in this
report; the release `integrations::` lib tests then passed 10 of 10 runs.

Pass 2:

- crates/foundry-app/src/integrations/mod.rs `mod tests`:
  `the_probe_proves_a_held_key_only_by_a_check_that_opens_to_the_canary`,
  `every_credential_opens_only_with_a_key_and_nothing_missing`,
  `the_worker_runs_only_when_on_and_able_to_seal_or_open`,
  `a_credential_under_a_key_not_held_nudges_the_reload`,
  `direct_seals_count_the_held_key_ids_that_seal_stored_values`,
  `a_held_secrets_key_survives_a_rewrap_under_a_key_this_replica_lacks`,
  `boot_rewraps_a_file_wrapped_secrets_key_under_the_operator_key`
  (store double `RewrapStore` counting reads and re-wraps, key source
  `HeldFile`), `a_recorded_key_check_joins_the_snapshot_only_for_a_new_key_id`.
- crates/foundry-app/src/integrations/reload.rs `mod tests`:
  `a_replica_state_names_its_key_mode`,
  `a_tick_with_nothing_to_apply_beats_every_ten_seconds`,
  `a_nudge_is_due_once_a_second`, `the_reload_task_ticks_when_it_runs`
  (store double `Beats`, `MockClock`).
- crates/foundry-app/src/integrations/handlers/instance.rs `mod tests`:
  `the_page_facts_follow_the_integrations_in_effect`,
  `a_refused_save_names_its_field`.
- crates/foundry-app/src/integrations/slack_app.rs `mod tests`:
  `a_save_seals_both_secrets_fenced_only_when_sealed`.

None of the pass 2 tests capture logs. No production code changed in either
pass; no seam added (the existing `link` field seam was enough).

## Gates

- Pass 1: `cargo test -p foundry-app --lib --test iis_reload --test iis_admin_cli_rotate --test eip_phase5_doors`: 186, 15, 1 and 2 passed, 0 failed.
- Pass 2: the foundry-app lib suite with every new test is the unmutated baseline of each pass 2 run (green); `cargo test --release -p foundry-app --lib integrations::` passed 10 of 10 runs (85 passed each).
- foundry-store (`--lib --test iis_instance_store --test eip_store`) and foundry-auth (`--lib`): green as the unmutated baselines of their runs.
- `cargo xtask smoke` (pass 1 and again after pass 2): `xtask smoke :: all gates green`.
- No production code changed, so the instance-integration-setup lane and `cargo xtask ci` were not required and not run.
- Tree after the runs: only the key_file.rs tests and this report are changed by this pass (cargo-mutants worked on copies under `~/mut/tmp`; the hand-seeded VM files were restored from copies).
