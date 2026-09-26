# Feature: release-version-footer — every full page shows the release the server is running.
#
# Spec: docs/feature/release-version-footer/feature-delta.md (US-RVF-01, D1-D3). Lean path: this
# scenario file is authored inside DELIVER step 01-01 (no separate DISTILL wave).
#
# HARNESS NOTE — HTTP lane. The contract is server-rendered markup (the text, its single
# occurrence, its place after the main content, its absence from htmx fragments), so a reqwest
# GET against the in-process harness observes all of it; no browser is needed. The version the
# steps expect is foundry-app's crate version read from its Cargo.toml at test compile time —
# never a hard-coded literal — so a release bump keeps this green without an edit.
#
# REUSE: the Background and the board / new-issue-modal fetches are the shipped board-new-issue
# steps (feature_board_new_issue.rs). The signed-out fetch and the Then oracles are new.

@rvf @release-version-footer @driving_port @real-io
Feature: Every page shows the release version the server is running
  An operator who has just pushed a release tag opens any Foundry page — signed in or not — and
  reads "Foundry v<version>" at the bottom, so she knows the new release is the one serving.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" with key prefix "GEN" exists under "Backend"
    And Mei is signed in

  Scenario: The sign-in page shows the running release without an account
    When a visitor with no session opens the sign-in page
    Then the page shows the running release version once, below the main content

  Scenario: A signed-in board page shows the running release
    When Mei fetches the "Sandbox" board
    Then the page shows the running release version once, below the main content

  Scenario: An htmx fragment carries no release footer
    When Mei fetches the new-issue modal for "Sandbox"
    Then the response carries no release version footer
