# Mutation Report — instance-admin-project-rename (DELIVER Phase 5)

- **Tool**: cargo-mutants 25.3.1 | **Date**: 2026-08-22 | **Gate**: kill rate >= 80% of viable mutants
- **Scope**: production logic added/changed by `eb244e1..70d74b1`, selected with `--in-diff` over the four target files below.
- **Verdict**: **PASS — 24/24 viable mutants killed (100%)**, 0 missed, 0 timeouts.

## Per-file results

| File | Mutants | Unviable | Viable | Killed (pre-existing tests) | Killed (new tests) | Killed (@iapr acceptance) | Missed |
|---|---|---|---|---|---|---|---|
| `crates/foundry-services/src/projects.rs` | 14 | 3 | 11 | 8 | 3 | 0 | 0 |
| `crates/foundry-core/src/lib.rs` (`slugify`) | 3 | 0 | 3 | 3 | 0 | 0 | 0 |
| `crates/foundry-app/src/instance_admin.rs` | 10 | 4 | 6 | 0 | 2 | 4 | 0 |
| `crates/foundry-app/src/projects.rs` | 5 | 1 | 4 | 1 | 0 | 3 | 0 |
| **Total** | **32** | **8** | **24** | **12** | **5** | **7** | **0** |

Unviable = mutant does not compile (e.g. `Ok(Default::default())` on enums without `Default`,
`HashMap::new()` where `HashMap` is not imported unqualified). Excluded from the kill rate.

## Test layers exercised

1. **Fast unit layer** (`cargo mutants --in-diff … -- --lib`, per mutated package): 32 mutants
   in 2m28s — 12 caught, 12 missed, 8 unviable.
2. **Killing tests added** (commit `3fa6095`), re-verified with targeted cargo-mutants re-runs:
   - `classify_rename` 256/257-scalar boundary example — kills `>` → `>=` (the `1..400`
     proptest range never guarantees sampling exactly 256; mutation testing exposed this).
   - `tests/rename_project_use_case.rs` (@real-io, real Postgres through the `Services`
     driving port, the `provision_workspace_use_case` idiom) — kills the `delete !`
     `is_admin` gate inversion and the `rows_affected ==` → `!=` race-guard inversion,
     previously pinned only by the acceptance lane (the @real-io trap).
   - `instance_admin::response_helper_tests` — kills the `Default::default()` no-op mutants
     on `rename_error_fragment` (422 + marker + copy) and `html_with_optional_cookie`
     (body + SET_COOKIE iff minted).
   - Re-run proof: `foundry-services/src/projects.rs` full-file run — 11/11 viable caught;
     both `foundry-app` helper mutants caught.
3. **@iapr acceptance lane** (browser, 21 scenarios): the 7 handler-level mutants that cannot
   be reached without `AppState`/a live server were each applied by hand, the lane run, and a
   genuine multi-step assertion failure confirmed (0 chromedriver flakes in any kill log):
   `show_dashboard`→default, `submit_project_rename`→default,
   `render_project_row_fragment`→default and its `find(== → !=)` row-pick inversion,
   `show_board`→default, `render_board`→`Ok("")` and `Ok("xyzzy")`. All 7 KILLED.

## Exclusions (documented)

- **`crates/foundry-store` SQL query methods**: thin sqlx driven-port wrappers; mutating SQL
  strings/bind plumbing mostly yields DB errors, not semantic survivors — low signal.
  Excluded from mutation scope by design (per-feature strategy).
- Test modules, templates, docs, and the acceptance step files are not production logic.

## Operational notes

- `cargo mutants --test-package foundry-acceptance` in 25.3.1 resolved back to the mutated
  package (probed: requesting `foundry-store` for a `foundry-core` mutant still ran
  `--package=foundry-core`), so the acceptance-layer verification was done manually per mutant
  instead of via a second cargo-mutants pass.
- Baselines including testcontainers-backed integration tests flake under parallel mutant
  workdirs (`-j 2` → sqlx `SSLRequest` protocol error); container-layer runs were done `-j 1`,
  the lib-layer pass with `-- --lib`.
- One transient chromedriver "session not created / chrome not reachable" flake was seen on
  clean-tree lane runs (environmental; versions match at 151); the final clean run is green
  21/21 scenarios, 132/132 steps.
- Post-mutation safety: all hand-mutations reverted via `git restore`; worktree clean;
  workspace `--lib` tests green; `cargo fmt --check` and `cargo clippy --all-targets` clean.

## Runtime

Pass A 2m28s + services container-layer re-run 1m29s + helper re-run 49s + 7 manual
acceptance verifications (~45–60s each, ~6m) + clean-lane checks (~33s each).
