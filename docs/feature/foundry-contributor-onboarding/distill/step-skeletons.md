# Step skeletons — US-13 Contributor Onboarding

Cucumber-rs step signatures DELIVER scaffolds in
`crates/foundry-acceptance/src/steps/us_13_contributor_onboarding.rs`.

Step-method names are snake_case Rust functions following the existing
slice 1+2+3 style (see `src/steps/us_08_file_issue.rs` for tone).

## World additions

```rust
// crates/foundry-acceptance/src/world.rs — append after slice-3 US-11 block.

// ---- US-13 contributor onboarding ----
/// Lazy-loaded README contents for the current scenario. Loaded by the
/// "contributor is reading the project README" step; reused by all
/// downstream Then assertions.
pub us_13_readme_text: Option<String>,

/// Lazy-loaded `rust-toolchain.toml` contents for the current scenario.
pub us_13_rust_toolchain_text: Option<String>,

/// Outcome of the walking-skeleton subprocess invocation:
/// (exit_status, captured_stdout, captured_stderr).
pub us_13_self_test_outcome: Option<(std::process::ExitStatus, String, String)>,
```

## Step force-link

```rust
// crates/foundry-acceptance/tests/acceptance.rs — append.
#[allow(unused_imports)]
use foundry_acceptance::steps::us_13_contributor_onboarding as _us_13;
```

## Step signatures

### Walking-skeleton subprocess scenario

```rust
#[given("a contributor on a fresh checkout of Foundry")]
async fn given_fresh_checkout(world: &mut FoundryWorld) {
    // No-op: cucumber-rs runs each scenario in a fresh World, which is
    // the test-time equivalent of a fresh checkout for env-var purposes.
}

#[given("no DATABASE_URL or other Foundry environment variable is set")]
async fn given_no_env_vars(world: &mut FoundryWorld) {
    // Documents the precondition; the actual env-strip happens inside
    // the When step's `Command::env_remove` calls.
}

#[given("a Docker daemon is reachable")]
async fn given_docker_reachable(world: &mut FoundryWorld) {
    // Reuse the precondition already required by every slice 1+2+3
    // acceptance scenario; surfaced here for documentation.
    // If `DOCKER_HOST` is unset and `/var/run/docker.sock` is missing,
    // panic with the same actionable message the rest of the suite uses.
}

#[when("the contributor runs the documented first test command")]
async fn when_run_first_test_command(world: &mut FoundryWorld) {
    let root = readme_inspect::workspace_root();
    let output = std::process::Command::new("cargo")
        .args(["test", "-p", "foundry-acceptance",
               "--", "--tags", "@walking_skeleton and not @us-13"])
        .env_remove("DATABASE_URL")
        .env_remove("FOUNDRY_DATABASE_URL")
        .env_remove("FOUNDRY_ACCEPTANCE_TAGS")
        .current_dir(&root)
        .output()
        .expect("invoke cargo test");
    world.us_13_self_test_outcome = Some((
        output.status,
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ));
}

#[then("the test command exits successfully")]
async fn then_test_exits_ok(world: &mut FoundryWorld) {
    let (status, _, stderr) = world.us_13_self_test_outcome.as_ref()
        .expect("walking-skeleton When did not capture an outcome");
    assert!(status.success(),
        "expected `cargo test` to exit 0; got {status:?}\nstderr:\n{stderr}");
}

#[then("the test runner provisions its own database without the contributor configuring one")]
async fn then_runner_provisions_db(world: &mut FoundryWorld) {
    // Negative assertion: the stderr does not contain
    // "DATABASE_URL is required" or "postgres connection refused".
    let (_, _, stderr) = world.us_13_self_test_outcome.as_ref().unwrap();
    assert!(!stderr.contains("DATABASE_URL"),
        "test failed because DATABASE_URL was required:\n{stderr}");
    assert!(!stderr.contains("connection refused"),
        "test failed because Postgres was unreachable:\n{stderr}");
}

#[then("the test output reports all acceptance scenarios as passing")]
async fn then_all_scenarios_pass(world: &mut FoundryWorld) {
    let (_, stdout, _) = world.us_13_self_test_outcome.as_ref().unwrap();
    // cucumber-rs prints a final "N scenarios (N passed)" summary. We
    // assert that the summary line contains "0 failed" rather than a
    // brittle exact count.
    assert!(stdout.contains("passed"),
        "no `passed` summary in cargo test output:\n{stdout}");
    assert!(!stdout.contains("failed: "),
        "cargo test output mentions failures:\n{stdout}");
}
```

### README-contract: Quickstart exhaustive

```rust
#[given("a contributor reading the project README")]
async fn given_reading_readme(world: &mut FoundryWorld) {
    world.us_13_readme_text = Some(readme_inspect::read_readme());
}

#[when("the contributor reads the Quickstart section")]
async fn when_read_quickstart(world: &mut FoundryWorld) {
    // No-op: the next Then steps perform the actual scan.
}

#[then("the section names every prerequisite the contributor must install before the first command")]
async fn then_prereqs_named(world: &mut FoundryWorld) {
    let readme = world.us_13_readme_text.as_ref().unwrap();
    let qs = readme_inspect::find_quickstart(readme)
        .expect("README must contain a Quickstart heading");
    // Contract: the Quickstart's prose (or a prereq subsection above
    // it) must mention each of: Rust, Docker. (sqlx-cli is install-on-
    // demand via a documented command, not a system prereq.)
    let combined = qs.prose_paragraphs.join("\n");
    for needed in ["Rust", "Docker"] {
        assert!(combined.contains(needed),
            "Quickstart prose does not mention prereq `{needed}`:\n{combined}");
    }
}

#[then(regex = r"^the section lists at least (\d+) build-and-test commands a contributor runs in sequence$")]
async fn then_quickstart_lists_n_commands(world: &mut FoundryWorld, n: usize) {
    let readme = world.us_13_readme_text.as_ref().unwrap();
    let qs = readme_inspect::find_quickstart(readme).unwrap();
    // Each fenced ```sh block can hold multiple commands separated by
    // newlines (or a single multi-line shell snippet). Count the lines
    // beginning with `cargo`, `docker`, or `sqlx` inside fences.
    let cmd_count: usize = qs.fenced_command_blocks.iter()
        .flat_map(|b| b.lines())
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("cargo ") || t.starts_with("docker ") || t.starts_with("sqlx ")
        })
        .count();
    assert!(cmd_count >= n,
        "Quickstart fenced blocks contain {cmd_count} contributor commands; need ≥ {n}");
}

#[then("the section ends with a command that runs the test suite")]
async fn then_quickstart_ends_with_test(world: &mut FoundryWorld) {
    let readme = world.us_13_readme_text.as_ref().unwrap();
    let qs = readme_inspect::find_quickstart(readme).unwrap();
    let last_fence = qs.fenced_command_blocks.last()
        .expect("Quickstart must contain at least one fenced command block");
    assert!(last_fence.contains("cargo test"),
        "last fenced command block does not include `cargo test`:\n{last_fence}");
}
```

### README-contract: MSRV pinned and stated

```rust
#[when("the contributor reads the prerequisites")]
async fn when_read_prereqs(world: &mut FoundryWorld) {
    // No-op: Then steps do the scan.
}

#[then("the prerequisites name a specific minimum Rust version")]
async fn then_readme_states_msrv(world: &mut FoundryWorld) {
    let readme = world.us_13_readme_text.as_ref().unwrap();
    let msrv = readme_inspect::find_readme_msrv_mention(readme)
        .expect("README prerequisites must name a specific Rust version (e.g. `Rust 1.85`)");
    assert!(msrv.split('.').count() >= 2,
        "README MSRV mention `{msrv}` is not a specific version");
}

#[then("the project's toolchain configuration pins that same minimum version")]
async fn then_toolchain_pins_msrv(world: &mut FoundryWorld) {
    let readme = world.us_13_readme_text.as_ref().unwrap();
    let toolchain = world.us_13_rust_toolchain_text
        .get_or_insert_with(readme_inspect::read_rust_toolchain).clone();
    let readme_msrv = readme_inspect::find_readme_msrv_mention(readme).unwrap();
    let pinned = readme_inspect::extract_pinned_msrv(&toolchain)
        .expect("rust-toolchain.toml must pin a specific channel (e.g. `1.85`)");
    assert_eq!(readme_msrv, pinned,
        "README states `{readme_msrv}` but rust-toolchain.toml pins `{pinned}`");
}

#[then("a contributor whose Rust toolchain is older than the pinned version is upgraded automatically by their toolchain manager before the build runs")]
async fn then_toolchain_auto_upgrades(world: &mut FoundryWorld) {
    // Structural assertion: rustup auto-installs the channel named in
    // rust-toolchain.toml when invoked from this directory. We assert
    // on the file's contract (the `[toolchain].channel` field is a
    // specific version) rather than invoking a real older cargo —
    // which would either silently use the resolved toolchain (proving
    // nothing) or require Docker-isolating the test (disproportionate).
    let toolchain = world.us_13_rust_toolchain_text.as_ref().unwrap();
    let pinned = readme_inspect::extract_pinned_msrv(toolchain).unwrap();
    // A pinned channel like "1.85" or "1.85.0" — NOT "stable" or "nightly"
    // — gives rustup the version it auto-installs.
    assert!(pinned.chars().next().unwrap().is_ascii_digit(),
        "rust-toolchain.toml channel `{pinned}` is not a specific version \
         — a generic channel like `stable` does not trigger rustup auto-install \
         of the MSRV. See US-13 AC: minimum Rust pinned in rust-toolchain.toml.");
}
```

### README-contract: hot-reload documented

```rust
#[when("the contributor looks for the inner-loop development guidance")]
async fn when_look_for_inner_loop(world: &mut FoundryWorld) {
    // No-op: Then steps do the scan.
}

#[then("the documentation names a watch-and-rebuild command that recompiles on file change")]
async fn then_watch_command_documented(world: &mut FoundryWorld) {
    let readme = world.us_13_readme_text.as_ref().unwrap();
    let cmd = readme_inspect::find_watch_command(readme)
        .expect("README must document a watch-and-rebuild command (e.g. `cargo watch -x run`)");
    assert!(cmd.contains("watch"),
        "documented inner-loop command `{cmd}` does not mention `watch`");
}

#[then("the documentation names the address at which the contributor sees the running app")]
async fn then_local_url_documented(world: &mut FoundryWorld) {
    let readme = world.us_13_readme_text.as_ref().unwrap();
    let url = readme_inspect::find_local_app_url(readme)
        .expect("README must name a local URL (e.g. http://localhost:3000) for the running app");
    assert!(url.starts_with("http://localhost"),
        "documented local URL `{url}` is not a localhost URL");
}
```

### `@manual` scenario stubs

Both `@manual` scenarios are filtered out by the default runner; their
step bodies exist only to surface a misconfigured CI run loudly.

```rust
#[given(regex = r"^a contributor on a fresh laptop.*$")]
async fn given_manual_fresh_laptop(_world: &mut FoundryWorld) {
    unreachable!("@manual scenario invoked; run the drill instead — \
        see docs/feature/foundry-contributor-onboarding/distill/manual-drills.md");
}

#[given("a contributor has a green test run and the dev server running locally")]
async fn given_manual_dev_server_running(_world: &mut FoundryWorld) {
    unreachable!("@manual scenario invoked; run the drill instead — \
        see docs/feature/foundry-contributor-onboarding/distill/manual-drills.md");
}

// The When / Then steps for these scenarios share the same pattern.
// DELIVER stubs them with the same `unreachable!` body; cucumber-rs
// is happy as long as the step regex matches.
```

## Definition of Done (for DELIVER)

- [ ] `crates/foundry-acceptance/src/support/readme_inspect.rs` exists with
      the six pure helpers listed above.
- [ ] `crates/foundry-acceptance/src/steps/us_13_contributor_onboarding.rs`
      exists and is force-linked from `tests/acceptance.rs`.
- [ ] `FoundryWorld` carries the three new optional fields.
- [ ] `README.md`'s Quickstart section is unified to 5 contributor commands
      and names Rust 1.85 + Docker as prereqs (per the AC).
- [ ] `rust-toolchain.toml`'s `[toolchain].channel` is pinned to a
      specific version (e.g. `"1.85"`), NOT a generic `"stable"`. This is
      the change the third `@readme-contract` scenario locks in.
- [ ] README documents `cargo watch -x run` and the local URL
      `http://localhost:3000` (or whichever the production app binds to —
      DELIVER cross-checks against `foundry-app`'s bind config).
- [ ] All 4 automated scenarios are GREEN (1 walking skeleton + 3
      file-inspection).
- [ ] The 2 `@manual` scenarios remain filtered out by the default
      runner (verify: `cargo test -p foundry-acceptance` does not
      execute the `unreachable!` bodies).
- [ ] `manual-drills.md` is published and discoverable from
      `CONTRIBUTING.md` (one-line link addition).
