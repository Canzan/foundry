# Story: US-13 — Contributor clones, runs, and ships a change
# Slice: 4 (Contributor onboarding — last MVP story)
# JTBD: outcome-3 (Minimize contributor time-to-meaningful-change)
#
# Scope note: this feature is doc + contract shaped. Most of US-13's value
# (the README quickstart, the rust-toolchain.toml pin, the testcontainers
# end-to-end suite) is already half-built by slices 1-3. US-13's job is to
# (a) finish the contract (pin the MSRV explicitly, unify the README
# quickstart) and (b) pin those contracts in acceptance tests so they do
# not regress.
#
# Two layers of testing:
#   - Automated (this file): file-inspection + one subprocess scenario that
#     proves `cargo test -p foundry-acceptance` runs end-to-end with no
#     pre-existing DATABASE_URL or external services. These pin the
#     structural contracts the manual drill depends on.
#   - @manual (manual-drills.md): the two contributor-experience drills
#     (≤10-min time-to-green, one-line-edit-and-see). Process metrics
#     that require a human, a stopwatch, and a fresh-ish machine to
#     answer honestly — automating them would require a Docker-in-Docker
#     VM image + a synthetic stopwatch that does not reflect a real
#     contributor's pause-to-read tempo.
#
# Driving adapters exercised:
#   - The README file (read via std::fs::read_to_string from the workspace
#     root) — driving port: contributor's `cat README.md`.
#   - The `rust-toolchain.toml` file — driving port: rustup's automatic
#     resolution when a contributor runs `cargo build`.
#   - The `cargo test -p foundry-acceptance` subprocess — the contributor's
#     literal first test invocation.
#
# Mandate 9: layer 3+ (subprocess + real FS) → example-only, no PBT.
# Mandate 11: sad path (toolchain-error) is one named example.

@slice4 @us-13 @contributor-onboarding
Feature: A new contributor clones Foundry, builds it, and runs the test suite
  A Rust developer arriving at the Foundry repository for the first time
  can follow the README's quickstart from `git clone` to a green test run
  without leaving the README, without installing services outside the
  documented prerequisites, and with a clear error if their Rust toolchain
  is older than Foundry requires.

  @walking_skeleton @real-io @driving_port @us-13
  Scenario: The contributor's test command succeeds end-to-end with no pre-existing services
    # Walking skeleton: this is the literal first command a new contributor
    # runs after `git clone` + the documented `docker compose up postgres`.
    # The test suite is responsible for booting its own ephemeral Postgres
    # via testcontainers; the contributor brings only the Rust toolchain
    # and a running Docker daemon. No DATABASE_URL, no env file, no
    # external services beyond the local Docker engine.
    Given a contributor on a fresh checkout of Foundry
    And no DATABASE_URL or other Foundry environment variable is set
    And a Docker daemon is reachable
    When the contributor runs the documented first test command
    Then the test command exits successfully
    And the test runner provisions its own database without the contributor configuring one
    And the test output reports all acceptance scenarios as passing

  @real-io @driving_port @us-13 @readme-contract
  Scenario: The README's Quickstart section walks from clone to green tests in five named commands
    # Pins the AC: "README Quickstart walks from clone to green tests in 5
    # commands." The structural contract is: a Quickstart heading exists,
    # it names exactly the prerequisites the contributor needs, and it
    # lists the five commands without omissions.
    Given a contributor reading the project README
    When the contributor reads the Quickstart section
    Then the section names every prerequisite the contributor must install before the first command
    And the section lists at least five build-and-test commands a contributor runs in sequence
    And the section ends with a command that runs the test suite

  @real-io @driving_port @us-13 @readme-contract
  Scenario: The README states the minimum supported Rust version and the project pins it
    # Pins the AC: "Minimum Rust version pinned in rust-toolchain.toml and
    # called out in README." The two surfaces must agree — if the README
    # says 1.85 but rust-toolchain.toml says 1.83, the contributor's
    # toolchain resolution and the documented contract diverge.
    Given a contributor reading the project README
    When the contributor reads the prerequisites
    Then the prerequisites name a specific minimum Rust version
    And the project's toolchain configuration pins that same minimum version
    And a contributor whose Rust toolchain is older than the pinned version is upgraded automatically by their toolchain manager before the build runs

  @real-io @driving_port @us-13 @readme-contract @hot-reload
  Scenario: The README documents how a contributor sees a code change without rebuilding manually
    # Pins the AC: "Hot-reload path documented (cargo watch -x run)."
    # The contributor wants the inner loop "edit → save → reload" — the
    # README must show them the command that gives them that loop.
    Given a contributor reading the project README
    When the contributor looks for the inner-loop development guidance
    Then the documentation names a watch-and-rebuild command that recompiles on file change
    And the documentation names the address at which the contributor sees the running app

  @manual @us-13 @demo @nfr-onboarding-10min
  Scenario: A new contributor reaches green tests within ten minutes on a fresh laptop
    # Manual scenario — verifies the JTBD outcome-3 promise that a single
    # Rust developer can be productive in a day.
    #
    # Reason for manual classification: this is a process metric, not a
    # software contract. It needs a fresh human, a fresh laptop, a
    # stopwatch, and an honest tempo (a human pausing to read the README
    # is part of the measured experience). Automating it would require a
    # Docker-in-Docker VM with a known cache state and a synthetic
    # stopwatch that does not reflect a real contributor's pause-to-read
    # tempo — the automation would measure compile time + CI cache, not
    # the user experience the AC names.
    #
    # Drill script: docs/feature/foundry-contributor-onboarding/distill/manual-drills.md § "Drill A".
    Given a contributor on a fresh laptop with a Rust toolchain and a Docker daemon installed
    When the contributor follows the README's Quickstart from `git clone` to the first green test run
    Then the contributor reaches a green test run within ten minutes
    And the contributor does not need to consult any source other than the README

  @manual @us-13 @demo
  Scenario: A contributor sees their one-line change reflected in the running app
    # Manual scenario — verifies the JTBD outcome-3 "meaningful change on
    # day one" promise.
    #
    # Reason for manual classification: this asserts on the inner-loop
    # experience (edit, save, reload, see). Automating it requires
    # spawning the app, hitting it, editing a template file, restarting,
    # re-hitting it, and inspecting the rendered HTML — heavy, and the
    # asserted property (the contributor visually confirms the change at
    # localhost:3000) is exactly what a human does in the drill. The
    # automation cost is disproportionate to the contract.
    #
    # Drill script: docs/feature/foundry-contributor-onboarding/distill/manual-drills.md § "Drill B".
    Given a contributor has a green test run and the dev server running locally
    When the contributor changes a heading in one of the project's templates
    And the contributor reloads the app
    Then the contributor sees the new heading at the documented local URL within thirty seconds
