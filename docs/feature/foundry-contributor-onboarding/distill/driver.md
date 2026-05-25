# Driver — Slice 4 / US-13 Contributor Onboarding

## 1. Purpose

US-13 is a contract-shaped feature. The acceptance suite pins three
on-disk contracts (README structure, MSRV pinning, hot-reload docs) and
one subprocess invariant (`cargo test -p foundry-acceptance` runs
end-to-end with no pre-existing env vars). All four scenarios live in
`crates/foundry-acceptance/tests/features/us-13-contributor-onboarding.feature`
and are executed by the existing cucumber-rs runner registered in
`crates/foundry-acceptance/tests/acceptance.rs`. There is no new
runtime or harness.

## 2. What is reused (no new infra)

| Need | Reused from | Note |
|---|---|---|
| cucumber-rs runner | `crates/foundry-acceptance/tests/acceptance.rs` | The runner already inspects `@manual` and skips it by default; US-13's two `@manual` scenarios inherit that behaviour. |
| World struct | `crates/foundry-acceptance/src/world.rs` | Three small per-scenario fields added: `us_13_readme_text`, `us_13_rust_toolchain_text`, `us_13_self_test_outcome`. No per-scenario harness boot for the file-inspection scenarios (the in-process Postgres harness is not used). |
| Step module force-link | `tests/acceptance.rs` | A new `use foundry_acceptance::steps::us_13_contributor_onboarding as _us_13;` line keeps `inventory::submit!` items from being stripped. |
| `assert_cmd` for subprocess | already in `Cargo.toml` (workspace dep) | The walking-skeleton scenario uses `assert_cmd::Command::cargo_bin("cargo")` is not appropriate; instead use `std::process::Command::new("cargo")` because the test invokes the test binary's host cargo and the assertion is on exit status + ability to bootstrap its own Postgres. See §4. |

## 3. New helper module (the only new file in `support/`)

`crates/foundry-acceptance/src/support/readme_inspect.rs` — pure-Rust
helpers that load the workspace's `README.md` and `rust-toolchain.toml`
and expose semantic queries that the Then-step assertions consume.

```rust
// crates/foundry-acceptance/src/support/readme_inspect.rs
//! Helpers for inspecting the workspace's README + toolchain pin.
//!
//! US-13's assertions pin structural contracts (README has a
//! Quickstart, MSRV is pinned, watch-mode docs exist). These helpers
//! locate the workspace root via `CARGO_MANIFEST_DIR` and return small
//! semantic structs the step bodies match against.

pub struct QuickstartSection {
    pub heading_text: String,
    pub fenced_command_blocks: Vec<String>,
    pub prose_paragraphs: Vec<String>,
}

pub fn workspace_root() -> std::path::PathBuf { /* ... two parents up from CARGO_MANIFEST_DIR */ }
pub fn read_readme() -> String { /* read $workspace/README.md */ }
pub fn read_rust_toolchain() -> String { /* read $workspace/rust-toolchain.toml */ }

pub fn find_quickstart(readme: &str) -> Option<QuickstartSection>;
pub fn extract_pinned_msrv(toolchain_toml: &str) -> Option<String>;
pub fn find_readme_msrv_mention(readme: &str) -> Option<String>;
pub fn find_watch_command(readme: &str) -> Option<String>;        // e.g. "cargo watch -x run"
pub fn find_local_app_url(readme: &str) -> Option<String>;        // e.g. "http://localhost:3000"
```

These helpers are pure (Mandate 4): they take strings, return data.
The only impure bits (`read_to_string`) live in `read_readme` /
`read_rust_toolchain` and are stable, fast, and trivially reliable.
Adapters around these would be ceremony for no payoff.

## 4. Walking-skeleton subprocess scenario

The walking skeleton invokes a real `cargo test` subprocess to prove
the literal contributor experience: fresh checkout, no env vars, one
command, green.

Implementation outline (in the step body):

```rust
use std::process::Command;

let workspace_root = readme_inspect::workspace_root();
let output = Command::new("cargo")
    .args(["test", "-p", "foundry-acceptance",
           "--", "--tags", "@walking_skeleton and not @us-13"])
    // Strip every Foundry-shaped env var the contributor's shell may
    // have inherited. The contract: testcontainers boots its own PG.
    .env_remove("DATABASE_URL")
    .env_remove("FOUNDRY_DATABASE_URL")
    .env_remove("FOUNDRY_ACCEPTANCE_TAGS")
    .current_dir(&workspace_root)
    .output()
    .expect("invoke cargo test");
```

Two non-obvious bits:

1. **The scenario filter `and not @us-13`** is deliberate — we run the
   *other* walking skeletons under a fresh env, not this one (which
   would recurse infinitely). The contract under test is "Foundry's
   acceptance suite boots its own Postgres given only Docker" — proven
   by running the existing slice 1+2+3 walking skeletons, not by
   re-running ourselves.
2. **`current_dir(workspace_root)`** lets the nested cargo find the
   same `Cargo.toml` regardless of where the outer runner was invoked
   from (CI may run from `crates/foundry-acceptance`, local dev may
   run from the workspace root).

Suite-time impact: ~25-35 seconds. This is the dominant cost of the
slice. The scenario is tagged `@walking_skeleton @us-13` and runs in
the default fast loop (no `@docker-compose` filter). Justification:
the contract IS that the contributor's first command works — moving
this to an opt-in lane would defeat the point.

If the suite-time impact proves disruptive, the fallback is to move
this scenario behind `@manual-trigger` (matching slice 3's
`@docker-compose @us-02` lane) and rely on the manual drill plus the
file-inspection scenarios in the default loop. DELIVER may make this
call after a CI measurement.

## 5. File-inspection scenarios

The three `@readme-contract` scenarios are pure file-read + regex /
string-search. Per scenario:

1. Background-equivalent: `Given the contributor is reading the
   project README` → step body loads `README.md` into
   `world.us_13_readme_text` exactly once (cached if the same scenario
   re-enters via a chained Given).
2. When step: empty (the read is the action; the When is the
   contributor's eye scanning the section).
3. Then steps: call into `readme_inspect` helpers + `assert!`.

These scenarios are fast (~1-2 ms each) and add negligible suite-time
overhead.

## 6. `@manual` scenarios

The two manual scenarios are runtime no-ops — the cucumber-rs runner
filters them out by default (the `acceptance.rs` runner already
excludes `@manual`, established by US-01's manual scenario in slice 1).
Their step bodies are still implemented and assert
`unreachable!("@manual scenario invoked; run the drill instead — see
manual-drills.md")` so that a misconfigured CI invocation surfaces
loudly rather than silently passing.

Drill scripts live in `manual-drills.md` alongside this file.

## 7. Suite-time budget impact

| Bucket | Estimate | Notes |
|---|---:|---|
| 3 × file-inspection scenarios | ~5 ms total | pure file-read |
| 1 × subprocess walking skeleton | ~25-35 s | nested `cargo test --tags "@walking_skeleton and not @us-13"` |
| 2 × `@manual` (skipped by default) | 0 s | filtered out |
| **Slice 4 default-loop delta** | **~25-35 s** | dominated by the nested cargo invocation |

This is the largest single-scenario cost in the slice. Acceptable
because the contract under test (the contributor's `cargo test` works
out of the box) is exactly what US-13 promises. If the budget breaks
the 60s top-line, DELIVER moves the WS to `@manual-trigger` per §4.

## 8. References

- Slice 1 driver (US-05+): `docs/feature/foundry-backend-mvp/distill/driver.md`
- Slice 3 driver: `docs/feature/foundry-operator-grade/distill/driver.md` § 6 (the docker-compose harness pattern)
- Slice 3 `pg_backup` helper (precedent for "support module that wraps system tooling"): `crates/foundry-acceptance/src/support/pg_backup.rs`
- `assert_cmd` precedent: `crates/foundry-acceptance/src/support/pg_backup.rs` (system `pg_dump` subprocess) and US-03 step bodies
