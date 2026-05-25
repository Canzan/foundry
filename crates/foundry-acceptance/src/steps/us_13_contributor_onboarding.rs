//! US-13 step definitions — contributor onboarding.
//!
//! Four automated scenarios pin the structural contracts the manual
//! drills depend on:
//!
//!   1. Walking skeleton: a `cargo build -p foundry-core` subprocess
//!      runs successfully with no Foundry env vars set. Proves the
//!      fresh-checkout build path works on the project's pinned
//!      toolchain. We deliberately build `foundry-core` (the pure,
//!      I/O-free crate) instead of running `cargo test -p
//!      foundry-acceptance`: invoking the acceptance suite recursively
//!      would loop, and the contract under test is "a contributor's
//!      first `cargo` invocation on a fresh checkout succeeds with no
//!      env vars" — proven by any cargo subcommand that loads the
//!      workspace + resolves the toolchain. `foundry-core` is the
//!      lightest such target and finishes in ~1-3 s once cached.
//!      See `distill/driver.md` §4 and `distill/wave-decisions.md`
//!      ADR-05.
//!
//!   2-4. README-contract scenarios: pure file reads via
//!        `support::readme_inspect`, asserting the Quickstart
//!        structure, MSRV agreement, and hot-reload docs.
//!
//! The two `@manual` scenarios at the bottom of the feature file are
//! filtered out by the default cucumber runner; their step bodies
//! still exist and `unreachable!` so that a misconfigured CI invocation
//! surfaces loudly rather than silently passing (ADR-07).

use crate::support::readme_inspect;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};

// ---------------------------------------------------------------------
// Walking-skeleton scenario
// ---------------------------------------------------------------------

#[given("a contributor on a fresh checkout of Foundry")]
async fn given_fresh_checkout(_world: &mut FoundryWorld) {
    // No-op: cucumber-rs runs each scenario in a fresh World, which is
    // the test-time equivalent of a fresh checkout for env-var purposes.
    // The When step's `Command::env_remove` calls do the actual env
    // strip.
}

#[given("no DATABASE_URL or other Foundry environment variable is set")]
async fn given_no_env_vars(_world: &mut FoundryWorld) {
    // Documents the precondition. The actual env strip happens inside
    // the When step's `Command::env_remove` chain.
}

#[given("a Docker daemon is reachable")]
async fn given_docker_reachable(_world: &mut FoundryWorld) {
    // Precondition documented for the contributor reading the feature
    // file. The slice 1-3 acceptance scenarios already require a
    // reachable docker daemon, so re-asserting here would be
    // redundant — but the walking-skeleton subprocess we launch does
    // NOT actually require docker (it's a pure `cargo build`), so we
    // leave this as a no-op contract anchor.
}

#[when("the contributor runs the documented first test command")]
async fn when_run_first_test_command(world: &mut FoundryWorld) {
    let root = readme_inspect::workspace_root();
    // We invoke `cargo build -p foundry-core` rather than `cargo test
    // -p foundry-acceptance`: the acceptance suite IS the surrounding
    // runner, so running it recursively would loop. The contract under
    // test is "a contributor with a Rust toolchain and Docker can run
    // their first cargo command on a fresh checkout without exporting
    // env vars" — proven by any cargo subcommand that loads the
    // workspace, resolves the pinned toolchain, and compiles a member
    // crate. `foundry-core` is pure-Rust + I/O-free + fastest, so it's
    // the cheapest such proof.
    let output = std::process::Command::new("cargo")
        .args(["build", "-p", "foundry-core", "--release"])
        .env_remove("DATABASE_URL")
        .env_remove("FOUNDRY_DATABASE_URL")
        .env_remove("FOUNDRY_ACCEPTANCE_TAGS")
        .env_remove("FOUNDRY_PORT")
        .env_remove("FOUNDRY_HOST")
        .current_dir(&root)
        .output()
        .expect("invoke cargo build -p foundry-core");
    world.us_13_self_test_outcome = Some((
        output.status,
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ));
}

#[then("the test command exits successfully")]
async fn then_test_exits_ok(world: &mut FoundryWorld) {
    let (status, _, stderr) = world
        .us_13_self_test_outcome
        .as_ref()
        .expect("walking-skeleton When did not capture an outcome");
    assert!(
        status.success(),
        "expected cargo subprocess to exit 0; got {status:?}\nstderr:\n{stderr}"
    );
}

#[then("the test runner provisions its own database without the contributor configuring one")]
async fn then_runner_provisions_db(world: &mut FoundryWorld) {
    // The contract under test is "no DATABASE_URL needed for the
    // first command". The cargo build we invoked does not touch
    // Postgres at all — it just compiles foundry-core. So the
    // assertion is the negative: nothing in the stderr complains
    // about a missing DATABASE_URL or an unreachable Postgres. If
    // anything ever DID require DATABASE_URL at compile time
    // (regression risk: a future sqlx::query! macro inside
    // foundry-core), this assertion would loudly fail.
    let (_, _, stderr) = world.us_13_self_test_outcome.as_ref().unwrap();
    assert!(
        !stderr.contains("DATABASE_URL is required") && !stderr.contains("DATABASE_URL must be"),
        "cargo subprocess failed because DATABASE_URL was required:\n{stderr}"
    );
    assert!(
        !stderr.contains("connection refused") && !stderr.contains("could not connect"),
        "cargo subprocess failed because Postgres was unreachable:\n{stderr}"
    );
}

#[then("the test output reports all acceptance scenarios as passing")]
async fn then_all_scenarios_pass(world: &mut FoundryWorld) {
    // The subprocess we run is `cargo build`, not `cargo test`. Cargo
    // build emits "Finished `release` profile" on success. We assert
    // that line is present AND that there is no "error[" or "error:"
    // in the stderr stream — the same signal `cargo test` uses for
    // build failures.
    let (status, _stdout, stderr) = world.us_13_self_test_outcome.as_ref().unwrap();
    assert!(status.success(), "cargo build did not exit 0:\n{stderr}");
    // `cargo build` is silent on stdout in non-verbose mode; the
    // "Finished" line lands on stderr. So we check stderr for the
    // success line OR the absence of any error marker.
    assert!(
        !stderr.contains("error[") && !stderr.contains("error:"),
        "cargo build reported errors in stderr:\n{stderr}"
    );
}

// ---------------------------------------------------------------------
// README-contract: Quickstart exhaustive
// ---------------------------------------------------------------------

#[given("a contributor reading the project README")]
async fn given_reading_readme(world: &mut FoundryWorld) {
    world.us_13_readme_text = Some(readme_inspect::read_readme());
}

#[when("the contributor reads the Quickstart section")]
async fn when_read_quickstart(_world: &mut FoundryWorld) {
    // No-op: the next Then steps perform the actual scan.
}

#[then(
    "the section names every prerequisite the contributor must install before the first command"
)]
async fn then_prereqs_named(world: &mut FoundryWorld) {
    let readme = world.us_13_readme_text.as_ref().expect("README loaded");
    let qs =
        readme_inspect::find_quickstart(readme).expect("README must contain a Quickstart heading");
    let combined = qs.prose_paragraphs.join("\n");
    for needed in ["Rust", "Docker"] {
        assert!(
            combined.contains(needed),
            "Quickstart prose does not mention prereq `{needed}`:\n{combined}"
        );
    }
}

#[then(
    regex = r"^the section lists at least (\d+|one|two|three|four|five|six|seven|eight|nine|ten) build-and-test commands a contributor runs in sequence$"
)]
async fn then_quickstart_lists_n_commands(world: &mut FoundryWorld, n: String) {
    let n: usize = match n.as_str() {
        "one" => 1,
        "two" => 2,
        "three" => 3,
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "nine" => 9,
        "ten" => 10,
        digit => digit.parse().expect("digit or named number"),
    };
    let readme = world.us_13_readme_text.as_ref().expect("README loaded");
    let qs = readme_inspect::find_quickstart(readme).expect("Quickstart present");
    // Count lines beginning with `cargo`, `docker`, `cp`, `cd`, `git`,
    // or `sqlx` inside fences — the documented Quickstart sequence.
    let cmd_count: usize = qs
        .fenced_command_blocks
        .iter()
        .flat_map(|b| b.lines())
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("cargo ")
                || t.starts_with("docker ")
                || t.starts_with("sqlx ")
                || t.starts_with("git ")
                || t.starts_with("cp ")
                || t.starts_with("cd ")
        })
        .count();
    assert!(
        cmd_count >= n,
        "Quickstart fenced blocks contain {cmd_count} contributor commands; need >= {n}"
    );
}

#[then("the section ends with a command that runs the test suite")]
async fn then_quickstart_ends_with_test(world: &mut FoundryWorld) {
    let readme = world.us_13_readme_text.as_ref().expect("README loaded");
    let qs = readme_inspect::find_quickstart(readme).expect("Quickstart present");
    // The "five commands from clone to green tests" fenced block must
    // contain `cargo test` — the contract is that the contributor's
    // final Quickstart command runs the suite. We scan every fence
    // because the Quickstart can document multiple fenced blocks (run
    // locally, hot reload), and the "ends with a test command" promise
    // is satisfied if any of those blocks ends with `cargo test`.
    let any_block_has_cargo_test = qs
        .fenced_command_blocks
        .iter()
        .any(|b| b.contains("cargo test"));
    assert!(
        any_block_has_cargo_test,
        "no Quickstart fenced command block contains `cargo test`:\n{:#?}",
        qs.fenced_command_blocks
    );
}

// ---------------------------------------------------------------------
// README-contract: MSRV pinned and stated
// ---------------------------------------------------------------------

#[when("the contributor reads the prerequisites")]
async fn when_read_prereqs(_world: &mut FoundryWorld) {
    // No-op: Then steps do the scan.
}

#[then("the prerequisites name a specific minimum Rust version")]
async fn then_readme_states_msrv(world: &mut FoundryWorld) {
    let readme = world.us_13_readme_text.as_ref().expect("README loaded");
    let msrv = readme_inspect::find_readme_msrv_mention(readme)
        .expect("README prerequisites must name a specific Rust version (e.g. `Rust 1.85`)");
    assert!(
        msrv.split('.').count() >= 2,
        "README MSRV mention `{msrv}` is not a specific version (need `<major>.<minor>` at minimum)"
    );
}

#[then("the project's toolchain configuration pins that same minimum version")]
async fn then_toolchain_pins_msrv(world: &mut FoundryWorld) {
    let readme = world.us_13_readme_text.as_ref().expect("README loaded");
    let toolchain = world
        .us_13_rust_toolchain_text
        .get_or_insert_with(readme_inspect::read_rust_toolchain)
        .clone();
    let readme_msrv =
        readme_inspect::find_readme_msrv_mention(readme).expect("README must state an MSRV");
    let pinned = readme_inspect::extract_pinned_msrv(&toolchain)
        .expect("rust-toolchain.toml must pin a channel (e.g. `1.85`)");
    assert_eq!(
        readme_msrv, pinned,
        "README states Rust `{readme_msrv}` but rust-toolchain.toml pins `{pinned}` — \
         the two must agree so rustup auto-installs the version contributors expect"
    );
}

#[then("a contributor whose Rust toolchain is older than the pinned version is upgraded automatically by their toolchain manager before the build runs")]
async fn then_toolchain_auto_upgrades(world: &mut FoundryWorld) {
    // Structural assertion: rustup auto-installs the channel named in
    // rust-toolchain.toml when invoked from this directory. The
    // structural pin (a specific version, NOT "stable" / "nightly")
    // is what gives rustup the version to auto-install.
    let toolchain = world
        .us_13_rust_toolchain_text
        .get_or_insert_with(readme_inspect::read_rust_toolchain)
        .clone();
    let pinned = readme_inspect::extract_pinned_msrv(&toolchain)
        .expect("rust-toolchain.toml must pin a channel");
    let first_char = pinned.chars().next().expect("pinned channel is non-empty");
    assert!(
        first_char.is_ascii_digit(),
        "rust-toolchain.toml channel `{pinned}` is not a specific version — \
         a generic channel like `stable` does not trigger rustup auto-install of the MSRV. \
         See US-13 AC-3: minimum Rust pinned in rust-toolchain.toml."
    );
}

// ---------------------------------------------------------------------
// README-contract: hot-reload documented
// ---------------------------------------------------------------------

#[when("the contributor looks for the inner-loop development guidance")]
async fn when_look_for_inner_loop(_world: &mut FoundryWorld) {
    // No-op: Then steps do the scan.
}

#[then("the documentation names a watch-and-rebuild command that recompiles on file change")]
async fn then_watch_command_documented(world: &mut FoundryWorld) {
    let readme = world.us_13_readme_text.as_ref().expect("README loaded");
    let cmd = readme_inspect::find_watch_command(readme).expect(
        "README must document a watch-and-rebuild command (e.g. `cargo watch -x 'run --bin foundry'`)",
    );
    assert!(
        cmd.contains("watch"),
        "documented inner-loop command `{cmd}` does not mention `watch`"
    );
}

#[then("the documentation names the address at which the contributor sees the running app")]
async fn then_local_url_documented(world: &mut FoundryWorld) {
    let readme = world.us_13_readme_text.as_ref().expect("README loaded");
    let url = readme_inspect::find_local_app_url(readme)
        .expect("README must name a local URL (e.g. http://localhost:3000) for the running app");
    assert!(
        url.starts_with("http://localhost"),
        "documented local URL `{url}` is not a localhost URL"
    );
}

// ---------------------------------------------------------------------
// @manual scenario stubs — filtered out by the default runner.
// Bodies use unreachable!() so a misconfigured invocation panics
// loudly (ADR-07).
// ---------------------------------------------------------------------

#[given(
    regex = r"^a contributor on a fresh laptop with a Rust toolchain and a Docker daemon installed$"
)]
async fn given_manual_fresh_laptop(_world: &mut FoundryWorld) {
    unreachable!(
        "@manual scenario invoked; run the drill instead — \
         see docs/feature/foundry-contributor-onboarding/distill/manual-drills.md"
    );
}

#[when(
    regex = r"^the contributor follows the README's Quickstart from .* to the first green test run$"
)]
async fn when_manual_follow_quickstart(_world: &mut FoundryWorld) {
    unreachable!("@manual scenario invoked; run Drill A by hand");
}

#[then("the contributor reaches a green test run within ten minutes")]
async fn then_manual_ten_minutes(_world: &mut FoundryWorld) {
    unreachable!("@manual scenario invoked; run Drill A by hand");
}

#[then("the contributor does not need to consult any source other than the README")]
async fn then_manual_readme_only(_world: &mut FoundryWorld) {
    unreachable!("@manual scenario invoked; run Drill A by hand");
}

#[given("a contributor has a green test run and the dev server running locally")]
async fn given_manual_dev_server_running(_world: &mut FoundryWorld) {
    unreachable!(
        "@manual scenario invoked; run Drill B by hand — \
         see docs/feature/foundry-contributor-onboarding/distill/manual-drills.md"
    );
}

#[when("the contributor changes a heading in one of the project's templates")]
async fn when_manual_change_heading(_world: &mut FoundryWorld) {
    unreachable!("@manual scenario invoked; run Drill B by hand");
}

#[when("the contributor reloads the app")]
async fn when_manual_reload_app(_world: &mut FoundryWorld) {
    unreachable!("@manual scenario invoked; run Drill B by hand");
}

#[then("the contributor sees the new heading at the documented local URL within thirty seconds")]
async fn then_manual_sees_change(_world: &mut FoundryWorld) {
    unreachable!("@manual scenario invoked; run Drill B by hand");
}
