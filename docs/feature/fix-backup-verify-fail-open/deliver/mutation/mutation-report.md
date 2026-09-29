# Mutation report: fix-backup-verify-fail-open (DELIVER Phase 5)

**Date**: 2026-09-28
**Tool**: cargo-mutants 25.3.1 (default copy mode, never `--in-place`)
**Scope**: the lines the fix changes in `crates/foundry-app/src/admin_cli.rs`, selected with
`--in-diff` over the uncommitted `git diff` of that file:
- `run_backup_verify` (step 3 now delegates to `count_rows_in_probe` and fails closed).
- The new `CountFailure` (`exit_code`, `Display::fmt`), `quote_ident`, `count_rows_in_probe`.
- The new `ProbeDatabase`: `connect`, `count_known_tables`, `count`, `drop_schema`.

`--in-diff` also generates whole-function mutants for `run_restore_comment`, whose body the fix
touches only in a comment (it now points at `count_rows_in_probe`). They are kept in the
denominator, which is the stricter choice.

**Gate**: kill rate ≥ 80% PASS, 70–80% WARN, < 70% FAIL (`CLAUDE.md`, per-feature).

## Result

| Metric | Value |
|---|---|
| Mutants generated | 21 |
| Unviable (do not compile, excluded) | 1 |
| Viable | 20 |
| Killed by the package's own tests (`backup_verify_fail_closed`) | 17 |
| Killed on re-check against the acceptance lane | 3 (`run_restore_comment`, `admin-cli` tag) |
| Survived | 0 |
| Timeouts | 0 |
| **Kill rate** | **100%** (20/20 viable). Package tests alone: 85.0% (17/20) |
| Equivalent mutants excluded | none |
| **Gate verdict** | **PASS** |

Every kill below was checked in its log. Each one failed on a test assertion (an exit-code
`assert_eq!`, the report `ends_with`, a stderr `contains`, or a schema-existence check). None was
"caught" by an infrastructure error: across all logs there is no testcontainers port fetch
failure, no sqlx `Protocol('\0')`, no `Connection reset by peer`.

## Per-function table

| Function | Total | Killed | Survived | Timeout | Unviable | Kill rate |
|---|---|---|---|---|---|---|
| `run_backup_verify` | 3 | 3 | 0 | 0 | 0 | 100% |
| `CountFailure::exit_code` | 3 | 3 | 0 | 0 | 0 | 100% |
| `<CountFailure as Display>::fmt` | 1 | 1 | 0 | 0 | 0 | 100% |
| `quote_ident` | 2 | 2 | 0 | 0 | 0 | 100% |
| `count_rows_in_probe` | 1 | 1 | 0 | 0 | 0 | 100% |
| `ProbeDatabase::connect` | 1 | 0 | 0 | 0 | 1 | n/a |
| `ProbeDatabase::count_known_tables` | 1 | 1 | 0 | 0 | 0 | 100% |
| `ProbeDatabase::count` | 4 | 4 | 0 | 0 | 0 | 100% |
| `ProbeDatabase::drop_schema` | 2 | 2 | 0 | 0 | 0 | 100% |
| `run_restore_comment` (comment-only change) | 3 | 3 (acceptance) | 0 | 0 | 0 | 100% |
| **Total** | **21** | **20** | **0** | **0** | **1** | **100%** |

## Procedure and exact commands

`S` is `/private/tmp/claude-501/-Users-jeffbailey-Projects-canzan-foundry/f9ce8d1d-…/scratchpad/mut-fix`.

**Safety.** The fix is uncommitted, so nothing wrote to git. No `checkout`, `restore`, `stash`,
`reset` or `clean` was run, and cargo-mutants never ran `--in-place`. cargo-mutants mutates its
own copy of the tree under `$TMPDIR`. Where a mutant or fault had to be applied by hand, it was
applied to a **separate `rsync` copy of the tree** (`$S/tree`, own `target/`) by `$S/seed.sh`.
That script makes a `cp` backup, applies the diff with `patch`, runs the command, restores with
`cp` and then `cmp`s against the working tree. Every seed ended `RESTORED_OK`. The working tree
was never mutated.

1. **Snapshot.**
   ```sh
   shasum -a 256 crates/foundry-app/src/admin_cli.rs \
     crates/foundry-app/tests/backup_verify_fail_closed.rs > $S/sha-before.txt
   ```
2. **Scope diff** (read-only git):
   ```sh
   git diff -- crates/foundry-app/src/admin_cli.rs > $S/fix.diff
   cargo mutants -p foundry-app --in-diff $S/fix.diff --list   # 21 mutants
   ```
3. **Run**, against the fix's test file. Exit 8 is hermetic, and exits 9, 10 and the healthy
   path use a testcontainers Postgres probe:
   ```sh
   cargo mutants -p foundry-app --in-diff $S/fix.diff --output $S/run1 -j 2 \
     --timeout 600 --build-timeout 1500 -- --test backup_verify_fail_closed
   ```
   The baseline was 4/4 green. 21 mutants were tested in 3m 16s: 17 caught, 3 missed, 1
   unviable.
4. **Acceptance re-check of the 3 misses** (all `run_restore_comment`).
   - `@us-03-cli` covers only `backup-verify`, so it cannot reach `restore-comment`. The re-check
     used that command's own lane instead, `FOUNDRY_ACCEPTANCE_TAGS=admin-cli`: the 3
     `comment-tombstone-gc.feature` scenarios that run `foundry doctor restore-comment` as a
     subprocess.
   - Each survivor's **exact cargo-mutants diff** (`$S/run1/mutants.out/diff/*line_448*.diff`)
     was applied in `$S/tree` by `seed.sh`, with `$S/lane.sh` as the command.
   - **Pitfall, recorded.** The first attempt ran the lane without rebuilding the binary, and
     all 3 mutants "passed". The acceptance steps find `foundry` through
     `assert_cmd::cargo_bin`, i.e. the prebuilt `target/debug/foundry`, and
     `cargo test -p foundry-acceptance` does not rebuild another package's binary. That result
     was discarded.
   - `lane.sh` therefore does, in order:
     - `cargo build -p foundry-app --bin foundry`;
     - print the binary's sha256;
     - run the binary once as a warm-up, which also shows the mutant is live: exit 0, 1 and
       255 against the normal 2;
     - `cargo test -p foundry-acceptance --test acceptance`.
   - Baselines in the copy, on the unmutated binary (sha `22de7669…2e29`), both before and after
     the seeds:
     - `admin-cli`: **3/3 scenarios, 27/27 steps**.
     - `us-03-cli`: **2/2 scenarios, 18/18 steps**.
5. **Supplementary hand-seeded faults.** Faults H1–H3 (below) were applied the same way and
   run against `cargo test -p foundry-app --test backup_verify_fail_closed`.
6. **Cleanup.**
   - Leftover containers labelled `org.testcontainers.managed-by=testcontainers` or named
     `foundry-acceptance-chrome-*` were checked for. There were none; `canzan-lift-*` is always
     excluded.
   - `canzan-lift-test-pg` was running at the start. It was absent when cleanup ran, before
     any removal. This run removed nothing (`docker rm` was never invoked with an ID).

## Every mutant and its disposition

| Mutant | Outcome | Killing test(s) (assertion) |
|---|---|---|
| `:91:5` `run_backup_verify -> 0` | killed | all 4. The exit tests get `left: Some(0)` against 8/9/10; the healthy test fails on `unexpected report` (no stdout at all) |
| `:91:5` `run_backup_verify -> 1` | killed | all 4, exit-code `assert_eq!` |
| `:91:5` `run_backup_verify -> -1` | killed | all 4, exit-code `assert_eq!` |
| `:236:9` `CountFailure::exit_code -> 0` | killed | exit-8, exit-9 and exit-10 tests: `left: Some(0)`. This is exactly the original fail-open bug: exit 0 on a failure |
| `:236:9` `CountFailure::exit_code -> 1` | killed | same three, `left: Some(1)` |
| `:236:9` `CountFailure::exit_code -> -1` | killed | same three, `left: Some(255)` |
| `:246:9` `Display::fmt -> Ok(Default)` | killed | exit 8 "stderr must name the probe", exit 9 "stderr must name the table", exit 10 "stderr must say why": stderr is just `foundry doctor backup-verify: ` |
| `:267:5` `quote_ident -> String::new()` | killed | exit 9 test `left: Some(10)` (the view is never found); healthy test `left: Some(10)`, `right: Some(0)` |
| `:267:5` `quote_ident -> "xyzzy"` | killed | same two tests, same assertions |
| `:275:5` `count_rows_in_probe -> Ok(Default)` | killed | all 4: an empty count list reports `status: OK` with exit 0 on an unreachable probe, a failing count and a non-Foundry schema; the healthy report lacks the counts |
| `:308:9` `ProbeDatabase::connect -> Ok(Default)` | unviable | `ProbeDatabase` has no `Default` (`E0277`) |
| `:346:9` `count_known_tables -> Ok(Default)` | killed | exit 9 and exit 10 tests (`left: Some(0)`), healthy `unexpected report` |
| `:366:9` `ProbeDatabase::count -> Ok(None)` | killed | exit 9 test `left: Some(10)`, healthy `left: Some(10)` |
| `:366:9` `ProbeDatabase::count -> Ok(Some(0))` | killed | exit 9 and exit 10 tests `left: Some(0)` (every table "present"); healthy `unexpected report` (`issues: 0`, all 11 tables) |
| `:366:9` `ProbeDatabase::count -> Ok(Some(1))` | killed | same three |
| `:366:9` `ProbeDatabase::count -> Ok(Some(-1))` | killed | same three |
| `:384:9` `drop_schema -> ()` | killed | healthy: "the restored schema must be dropped after verification" |
| `:384:19` `==` → `!=` in `drop_schema` | killed | exit 10 test: "the public schema must never be dropped"; healthy: "the restored schema must be dropped after verification" |
| `:448:5` `run_restore_comment -> 0` | missed by package → **killed by acceptance** | `admin-cli`: 3 scenarios (3 failed). Missing UUID: `left: 0`, `right: 4`. Malformed UUID: `left: 0`, `right: 2`. Restore: `stdout contains "status: restored"` ✘ |
| `:448:5` `run_restore_comment -> 1` | missed by package → **killed by acceptance** | `admin-cli`: 3 failed. `left: 1` against `right:` 0 / 4 / 2 |
| `:448:5` `run_restore_comment -> -1` | missed by package → **killed by acceptance** | `admin-cli`: 3 failed. `left: 255` against `right:` 0 / 4 / 2 |

**Why the package tests miss `run_restore_comment`.** Only a comment inside it changed. It is a
separate subcommand, and the fix's test file correctly does not exercise it. Its own acceptance
lane kills all three mutants, so there is no gap.

## Supplementary hand-seeded faults (outside the cargo-mutants denominator)

cargo-mutants 25.3.1 generates no mutants for `?`, SQL strings, or `if` conditions it cannot
negate usefully. That leaves three rules the fix exists to enforce without a cargo-mutants
mutant. Each was probed with one fault, applied in `$S/tree` only.

| Fault | Seed | Outcome |
|---|---|---|
| **H1** re-introduce the fail-open: swallow count errors as "table absent" | `count_known_tables`: `self.count(schema, table).await.unwrap_or(None)` instead of `.map_err(count_failed)?` | **KILLED**: `failing_count_on_a_present_table_exits_9_naming_the_table`, `left: Some(0)`, `right: Some(9)` |
| **H2** drop the "not a Foundry backup" guard | `if false && counts.is_empty()` | **KILLED**: `schema_without_any_foundry_table_exits_10_and_keeps_public`, `left: Some(0)`, `right: Some(10)` |
| **H3** unquoted identifiers | `quote_ident` returns `ident.to_string()` | **KILLED**: `healthy_schema_counts_present_tables_skips_absent_and_drops_the_schema` (mixed-case `"Verify_OK"`), `left: Some(10)`, `right: Some(0)` |

Not probed:
- `PROBE_CONNECT_TIMEOUT`'s value. A timing constant has no observable contract short of a
  10-second black-hole test.
- The panic-join fallback in `count_rows_in_probe`. It is unreachable without a panicking
  worker.

## Tests added

None. The gate passed on the existing four tests in
`crates/foundry-app/tests/backup_verify_fail_closed.rs`.

## Final state

**File hashes, before and after** (`diff $S/sha-before.txt $S/sha-after.txt`: identical):

```
b894a0ee2d62a74831487e8a4b3faba9bcf93d09e08a8499efb73ecb44ea5486  crates/foundry-app/src/admin_cli.rs
50ae0c1af94057ffc13b1d727e176303ed4bff0e55e96034b7385dc6ee166d70  crates/foundry-app/tests/backup_verify_fail_closed.rs
```

**Checks, all green:**
- `cargo test -p foundry-app --test backup_verify_fail_closed`: 4 passed, 0 failed.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.

**Gate verdict: PASS.** 20/20 = 100%, or 85.0% on package tests alone.
- No survivors and no equivalent mutants.
- All three supplementary faults (H1 fail-open regression, H2 empty-schema guard, H3 identifier
  quoting) are killed.
