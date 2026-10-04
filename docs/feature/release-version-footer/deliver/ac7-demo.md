# AC-7 demo — the build stamp never goes stale (step 02-01)

Run 2026-10-04 against the 02-01 working tree, per the DESIGN revision's DDD-6 (revised) table.

**Where.** A scratch clone of this repo (`git clone` of the local checkout, then the 02-01 files
copied in and committed there as `59739fc`), with its own `CARGO_TARGET_DIR`. Every demo commit,
the detach, `git pack-refs --all` and the linked worktree happened **only in the scratch clone**.
This repo's `main` and refs were not touched.

**How observed.** `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false cargo build -p foundry-app -vv`,
filtered to foundry-app's `Fresh` / `Dirty … (reason)` / `Compiling` lines and the build script's
`cargo:` output. `$SCR` is the scratch directory. Every build exited 0 unless stated.

**End to end.** The build-script output feeds `views::BUILD_SHA` / `BUILD_DATE` with `env!`. The
rendered page was confirmed in this repo, not the scratch clone. The `@rvf` lane serves `/sign-in`
and a signed-in board page over real HTTP from the in-process app. Its oracle (git in the test
process) compares the footer text to `Foundry v0.7.0 · 2026-10-04` and `data-commit` to
`git rev-parse --short=7 HEAD` (`c7568bd`): 3 scenarios, all passed.

## Directives (as emitted on a branch with a loose ref)

```
cargo:rerun-if-changed=build.rs
cargo:rerun-if-env-changed=FOUNDRY_STAMP_SHA
cargo:rerun-if-env-changed=FOUNDRY_STAMP_DATE
cargo:rerun-if-changed=$SCR/fw/.git/HEAD
cargo:rerun-if-changed=$SCR/fw/.git/refs/heads/main
cargo:rerun-if-changed=$SCR/fw/.git/packed-refs
cargo:rustc-env=FOUNDRY_BUILD_SHA=59739fc
cargo:rustc-env=FOUNDRY_BUILD_DATE=2026-10-04
```

## Rows

| # | State exercised | Observed | Result |
|---|---|---|---|
| 1 | Clean build, then the same command again | First: build script ran, `FOUNDRY_BUILD_SHA=59739fc`, `FOUNDRY_BUILD_DATE=2026-10-04`, the directives above (all absolute and existing). Second: `Fresh foundry-app v0.7.0`, no rerun | PASS |
| 2 | `git commit --allow-empty -m demo` (HEAD `35e029b`), rebuild | `Dirty foundry-app: the file .git/refs/heads/main has changed` → `FOUNDRY_BUILD_SHA=35e029b` | PASS |
| 3 | `git checkout --detach HEAD~1`, rebuild; `git checkout -`, rebuild | Detached: `Dirty: .git/HEAD has changed` → `FOUNDRY_BUILD_SHA=59739fc`; the branch-ref directive is absent (HEAD, packed-refs only). Back: `Dirty: .git/HEAD has changed` → `35e029b`, branch-ref directive back | PASS |
| 4 | `git pack-refs --all` (loose heads: none), rebuild; `git commit --allow-empty -m demo2` (HEAD `f036cf4`), rebuild | First: `Dirty: the file .git/refs/heads/main is missing` → stamp unchanged and correct, `35e029b` (no `unknown`); the amended rule now registers the **directory** `$SCR/fw/.git/refs/heads`. Second: the loose ref reappeared, `Dirty: the file .git/refs/heads has changed` → `FOUNDRY_BUILD_SHA=f036cf4` | PASS |
| 5 | `FOUNDRY_STAMP_SHA=abc1234` | `Dirty: the env variable FOUNDRY_STAMP_SHA changed` → `FOUNDRY_BUILD_SHA=abc1234`, `FOUNDRY_BUILD_DATE=2026-10-04` (date still from git) | PASS |
| 6 | `FOUNDRY_STAMP_SHA=` (blank), then `FOUNDRY_STAMP_SHA='  '` | Each: `Dirty: env variable FOUNDRY_STAMP_SHA changed` → `FOUNDRY_BUILD_SHA=f036cf4` (git's SHA; blank counts as absent) | PASS |
| 7 | Set `abc1234` again (stamps `abc1234`), then unset, then rebuild with no change | Unset: `Dirty: env variable FOUNDRY_STAMP_SHA changed` → `FOUNDRY_BUILD_SHA=f036cf4`, never kept `abc1234`. Again: `Fresh` | PASS |
| 8 | `git worktree add $SCR/fw-demo -b fw-demo-branch`, commit inside it (`66b476c`), build there; commit again (`b055da2`), rebuild | Directives resolve through `--git-path`: `$SCR/fw/.git/worktrees/fw-demo/HEAD`, `$SCR/fw/.git/refs/heads/fw-demo-branch`, `$SCR/fw/.git/packed-refs` → `FOUNDRY_BUILD_SHA=66b476c`; after the second commit, `Dirty` → `b055da2` (the main checkout's HEAD stayed `f036cf4`) | PASS |

Rows 2, 3, 4 and 7 are the stale-stamp cases D11 exists to prevent. All of them re-stamped.

## Named faults that only a build can show (seeded in the scratch clone; build.rs restored, `cmp` identical)

| Fault | Observation | Verdict |
|---|---|---|
| Missing refs-dir directive (the loose ref is registered only if present, which is canzan-lift's rule) | After `pack-refs --all`, only `HEAD` and `packed-refs` were registered. After `git commit --allow-empty -m demo3` (HEAD `741a9df`), the rebuild printed `Fresh foundry-app`, so the binary kept `f72c840`. **Stale stamp.** Restored: re-stamped `741a9df` | KILLED by row 4 |
| The date is the build time (`date +%Y-%m-%d`) instead of the commit date | With a commit dated 2026-09-01: seeded, `FOUNDRY_BUILD_DATE=2026-10-04`; restored, `2026-09-01`. The `@rvf` oracle cannot see this today, because HEAD's commit date *is* today. It kills the fault on any day after the commit | KILLED (demo); lane-blind on the commit day |
| A git failure panics (build fails) | `GIT_DIR=/nonexistent-git-dir`: seeded, `failed to run custom build command for foundry-app`, rc 101; restored, rc 0 with `FOUNDRY_BUILD_SHA=unknown`, `FOUNDRY_BUILD_DATE=unknown` | KILLED |

## Forgejo CI checkout (DDD-14 revised)

Not observable from this host. DDD-14 asks DELIVER to record it from the first Forgejo run's
checkout log. That check is still owed.
