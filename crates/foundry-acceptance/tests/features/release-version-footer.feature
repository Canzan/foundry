# Feature: release-version-footer — every full page says which release, and which build of it, the
# server is running.
#
# Spec: docs/feature/release-version-footer/feature-delta.md — US-RVF-01 (D1-D4, shipped v0.5.0) and
# US-RVF-02 (D5-D14, DESIGN DDD-1..17; DISTILL 2026-10-04). US-RVF-01's scenarios were authored inside
# DELIVER step 01-01 (lean path); the build-stamp amendment below is the DISTILL wave's (OQ-D2).
#
# HARNESS NOTE — HTTP lane. The contract is server-rendered markup (the text, its single
# occurrence, its place after the main content, the commit id on the footer element, its absence
# from htmx fragments), so a reqwest GET against the in-process harness observes all of it; no
# browser is needed. The in-process app is compiled in this checkout, so its build stamp comes from
# git here (DDD-14), never from a prebuilt binary.
#
# THE ORACLE (DDD-15) never reads foundry-app's own constants, which would make the check circular:
#   * the release is foundry-app's crate version, read from its Cargo.toml at test compile time —
#     never a literal, so a release bump keeps this green without an edit;
#   * the commit date and short id come from git at test time, with the same two commands the build
#     uses (`git log -1 --format=%cd --date=short`, `git rev-parse --short=7 HEAD`); `unknown` when
#     git cannot answer;
#   * the steps refuse to run if FOUNDRY_STAMP_SHA / FOUNDRY_STAMP_DATE is set in the test process:
#     an explicit stamp outranks git (D9), so the oracle would compare against the wrong source.
#     Explicit-input precedence (AC-5) and the degraded `unknown` rendering (AC-6) are unit
#     examples in foundry-app, not scenarios here.
#
# @pending (US-RVF-02): the two page scenarios fail until DELIVER ships the build stamp — today's
# footer has neither the date nor the commit id. DELIVER removes @pending from both together.
#
# REUSE: the Background and the board / new-issue-modal fetches are the shipped board-new-issue
# steps (feature_board_new_issue.rs). The signed-out fetch and the Then oracles live in
# feature_release_version_footer.rs.

@rvf @release-version-footer @driving_port @real-io
Feature: Every page shows the release and the build the server is running
  An operator who has just pushed a commit or a release tag opens any Foundry page — signed in or
  not — and reads "Foundry v<version> · <commit date>" at the bottom, with the commit's short id on
  the same footer, so she knows the build she pushed is the one serving, not merely the same release.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" with key prefix "GEN" exists under "Backend"
    And Mei is signed in

  @us-rvf-02 @pending @contract-shape:pure-function
  Scenario: The sign-in page names the running build without an account
    When a visitor with no session opens the sign-in page
    Then the page names the running release and the date of the commit it was built from, once, below the main content
    And that footer carries the short id of the commit the server was built from

  @us-rvf-02 @pending @contract-shape:pure-function
  Scenario: A signed-in board page names the running build
    When Mei fetches the "Sandbox" board
    Then the page names the running release and the date of the commit it was built from, once, below the main content
    And that footer carries the short id of the commit the server was built from

  @contract-shape:pure-function
  Scenario: An htmx fragment carries no release footer
    When Mei fetches the new-issue modal for "Sandbox"
    Then the response carries no release version footer
