# Mutation Report — board-lane-management (DELIVER Phase 5)

- **Tool**: cargo-mutants 25.3.1 | **Date**: 2026-08-23 | **Gate**: kill rate >= 80% of viable mutants
- **Scope**: production logic added/changed by `1f100bf..HEAD`, selected with `--in-diff` over the prioritized files below (per-feature strategy).
- **Verdict**: **PASS — 98/110 viable mutants killed (89.1%)**, 12 survivors (each analyzed below), 0 flakes in any kill log.

## Per-file results

| File | Mutants | Unviable | Viable | Killed (pre-existing) | Killed (new tests) | Killed (@blm by hand) | Killed (timeout) | Missed |
|---|---|---|---|---|---|---|---|---|
| `foundry-services/src/lanes.rs` | 21 | 6 | 15 | 5 (classify proptests) | 8 | 0 | 0 | 2 |
| `foundry-services/src/issues.rs` | 9 | 3 | 6 | 4 (resolve_lane proptests) | 2 | 0 | 0 | 0 |
| `foundry-services/src/lib.rs` | 2 | 2 | 0 | — | — | — | — | 0 |
| `foundry-store/src/lanes.rs` | 29 | 3 | 26 | 5 (crossing-race test) | 13 | 0 | 1 | 7 |
| `foundry-store/src/lib.rs` | 12 | 2 | 10 | 5 (insert/laneless suites) | 2 | 1 (`insert_project`) | 0 | 2 |
| `foundry-app/src/lanes.rs` | 12 | 1 | 11 | 0 | 4 | 7 | 0 | 0 |
| `foundry-app/src/views.rs` | 3 | 1 | 2 | 0 | 2 | 0 | 0 | 0 |
| `xtask/src/check_arch.rs` | 40 | 0 | 40 | 22 | 12 | 0 | 5 | 1 |
| **Total** | **128** | **18** | **110** | **41** | **43** | **8** | **6** | **12** |

Unviable = mutant does not compile (`Ok(Default::default())` on enums/structs without
`Default`, etc.). Excluded from the kill rate. Timeouts are kills: each timeout mutant
turns a bounded loop infinite (retry-forever, brace-scan stuck), which the suite detects
by hanging into the cargo-mutants deadline.

## Test layers exercised

1. **Fast unit layer** (`--lib` per package): the `classify_lane_delete` and
   `resolve_lane` proptests killed every pure-heart mutant on first pass; the retry
   classifiers, response helpers and `board_columns` gained unit tests (below).
2. **Killing tests added**, each re-verified with a targeted cargo-mutants re-run:
   - `foundry-services/tests/delete_lane_use_case.rs` (NEW, @real-io, the
     `rename_project_use_case` idiom): dialog view content, D10 authz uniformity
     (non-member + machine-scope both directions), delete/move fate success, absent-lane
     404 precedence. Kills the `!is_member` gate inversion, the machine-scope `!=`
     inversion, the dialog lane-find inversion, `survivors_of`/`list_lanes` no-ops, and
     the lane-exists/destination prechecks — all previously pinned only by the
     acceptance lane (the @real-io trap, exactly as in the rename feature).
   - `foundry-services/tests/write_use_cases.rs` behaviour 4: `change_issue_state`
     per-project seam — kills `validate_project_lane` → `Ok("xyzzy")`/`Ok("")`.
   - `foundry-store/src/lanes.rs::retry_classifier_tests` (fast unit, fake
     `DatabaseError`): pins `is_fate_retryable` (23503/40P01 only) and
     `is_lane_fk_violation` (23503 AND `fk_issues_lane` only) — 11 classifier mutants
     that are otherwise reachable only through a non-deterministic race.
   - `foundry-store/tests/delete_lane_with_fate.rs` +3 (Mandate 4, real Postgres):
     delete-fate batch DELETE + true counts, move-fate append positions `C..C+N-1`
     below an occupied destination + one outbox row per moved card (kills the
     `occupied + index` → `-`/`*` arithmetic), and the persistent-failure honest-error
     test (kills retry-guard→`true` via the infinite-retry hang) + the live
     `count_issues_in_lane` read (3 mutants — the services killer cannot run for
     store mutants, cargo-mutants tests the mutated package only).
   - `foundry-store/tests/claim_bootstrap_and_create_workspace_store.rs`: the D4
     creation-seed contract (exactly Backlog/In-Progress/Done in board order) — kills
     `seed_creation_lanes` → `Ok(())`.
   - `foundry-app/src/lanes.rs::response_helper_tests` + `views.rs` `board_columns`
     test: 422 marker fragment, 500 helper, confirm-POST route, and the ONE
     column-builder (state-filter inversion + empty replacements).
   - `xtask/src/check_arch.rs::cfg_test_masking_ends_exactly_at_the_block_close`:
     four boundary fixtures pinning that the `#[cfg(test)]` region-skip ends EXACTLY
     at the block close — kills 12 brace-counting/lookahead mutants in
     `block_end`/`lane_scan_mask` (plus 5 by timeout: the stuck-scan loops).
3. **@blm acceptance lane by hand** (24 scenarios, in-process harness; `--test-package`
   is still broken in 25.3.1): each handler-level mutant was applied by hand, the lane
   run, a genuine multi-step assertion failure confirmed, and the source `git restore`d:
   `signed_in_user`→`None` (10 steps failed), `show_delete_lane_dialog`→default (8
   scenarios), delete `"move"` arm (3 scenarios), `oob_columns_response`→default (2
   steps), `submit_delete_lane`→default (8 steps), destination-filter `!` deletion (4
   steps), delete `"delete"` arm (5 steps), and store `insert_project`→`Ok(())` (7
   steps). All 8 KILLED, 0 environment flakes.

## Surviving mutants (12) — why each survives

- **`foundry-services/src/lanes.rs:109` `&&`→`||` and `==`→`!=` (2, equivalent by
  design)**: the `destination_is_survivor` precheck feeds `classify_lane_delete`, but
  `Store::delete_lane_with_fate` re-checks the destination authoritatively inside the
  transaction (ADR-BOARD-LANE-002) and both paths map to the SAME
  `DeleteLaneError::UnknownDestination`. For every reachable input the observable
  behaviour is identical; the precheck only saves a transaction. The third precheck
  mutant (`!=`→`==`, which refuses LEGITIMATE moves) IS killed by the move-fate test.
- **`foundry-store` retry-envelope arithmetic (9)**: `attempt < MAX_ATTEMPTS`
  comparisons (`==`/`>`/`<=`), guard→`false`, `&&`→`||`, `attempt += 1` → `-=`/`*=`,
  and the insert envelope's `is_lane_fk_violation` guard→`true`/`false`. All are
  observable ONLY when a retryable transient (a card racing into the dying lane between
  the membership snapshot and the lane DELETE, or a Postgres deadlock) occurs at the
  exact attempt boundary — deterministic reproduction requires in-transaction fault
  injection. Mitigations in place: guard→`true` (retry-forever) IS killed by the
  persistent-failure test's timeout; the pure classifiers underneath are 100% pinned by
  the new unit tests; the crossing-delete race test exercises the retry path
  non-deterministically on every run.
- **`xtask/src/check_arch.rs:46` `run` → `Default::default()` (1)**: `ExitCode` is
  opaque (`Default` IS `SUCCESS`, no `PartialEq`) and `run` binds to the real workspace
  root, so it cannot be pointed at a staged fixture in-process. Every checker it
  dispatches — including the new lane-list rule — is individually fixture-tested (34
  caught mutants prove it), and CI runs the real `check-arch` binary as a hard gate.

## Exclusions (documented)

- **`foundry-app/src/projects.rs` + `issues.rs` diff mutants (23)**: out of the
  prioritized scope — the changed lines are the L2 one-writer refactor of PRE-EXISTING
  handlers (`show_board`/`render_board`/`build_board_page` were already hand-verified
  against the browser lane in the instance-admin-project-rename Phase 5 pass; the
  report-page helpers are untouched behaviour). Board-columns correctness — the part
  this feature reshaped — is pinned directly by the new `board_columns` unit test and
  the 24-scenario @blm lane.
- **`foundry-store` thin pass-throughs**: per the task brief the store was NOT
  blanket-excluded (fate arms, position math, retry, leftmost-insert all in scope).
  `list_project_lanes` had no mutants generated by cargo-mutants in any pass; its
  ordering contract is pinned by the claim creation-seed test and the dialog tests.
- Test modules, templates, docs and acceptance step files are not production logic.

## Operational notes

- `--test-package` remains broken in 25.3.1 (rename-feature finding re-confirmed), so
  acceptance-layer kills were verified manually per mutant.
- Store/services container-backed passes ran `-j 1`; unit-layer passes at default jobs.
- `--in-diff` staleness: cargo-mutants refuses a diff that no longer matches the tree —
  regenerate the diff after adding killing tests.
- Post-mutation safety: all hand-mutations reverted via `git restore`; `grep "MUTANT"`
  clean; workspace tests green; `cargo fmt --check` and `cargo clippy --all-targets`
  clean; final @blm lane 24/24 scenarios green.

## Runtime

Pass A services `--lib` 1m28s; xtask 1m34s + re-run 2m18s; store scoped 7m15s + full
re-run 9m06s + targeted count re-run 53s; app `--lib` 3m19s; services re-run 2m21s;
8 manual acceptance kills ~3-5 min each (incremental rebuild + 24-scenario lane);
baselines and final clean-lane checks on top.
