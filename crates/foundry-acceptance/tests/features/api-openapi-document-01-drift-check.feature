# Feature: api-openapi-document, slices 01 and 02 (US-AOD-02, and US-AOD-04's completeness rule)
# — the maintainer cannot merge a change that makes the API description lie.
#
# Spec: docs/feature/api-openapi-document/feature-delta.md — DISCUSS D3, D13, US-AOD-02; DESIGN
# DDD-9 (three layers), DDD-10/11/13/14, DDD-21 and the check-arch rules `api-openapi-operations`
# and `api-openapi-statuses`, ADR-API-OPENAPI-001/002.
#
# THE DOOR — "the drift check" is the part of `cargo xtask ci` (and `cargo xtask smoke`) that holds
# the code to the description, run the way the maintainer runs it, as real subprocesses against a
# real tree:
#   1. `xtask check-arch --root <tree>` (the structural layer: routes, parameters, bodies,
#      statuses, error codes), and
#   2. `cargo test -p foundry-api --lib openapi` in that tree (the type-derived layer: the
#      `schemars` shape test, the OAS 3.1 validity test and the `info.version` test).
# The check passes only when both pass. "The description refresh" is `xtask openapi --root <tree>`
# (DESIGN DDD-13; DISTILL pins the `--root` flag, the check-arch convention, so the subprocess can
# never edit the real checkout). "The image build checks the committed description" is DEVOPS's
# read-only `xtask openapi --check --root <tree>` (DEVOPS Changed Assumption 1, release gate R2),
# the line the Dockerfile builder stage runs so no image is built from a stale description.
#
# THE TREE — each scenario gets a fresh copy of the repository (crates, xtask, manifests; mtimes
# kept so cargo rebuilds only what a plant touched) at `target/aod-drift/tree`, built into
# `target/aod-drift/target`. Each scenario takes one process-wide lock in its Background and holds
# it until the scenario ends (the guard lives in the World), so no two drift scenarios ever touch
# the tree at once, Then steps included. A plant edits the copy only.
#
# @slow: excluded from the default (empty-tag) lane, which would otherwise carry eleven repo copies
# plus cargo test runs; included in the `all` lane (`cargo xtask ci`) and in the DEVOPS lane
# selection `FOUNDRY_ACCEPTANCE_TAGS=aod-01` / `aod-02` (single-tag mode does not exclude @slow).
#
# CONTROLS — before the feature, both commands pass on any tree (no rule, no test), so every
# "passes" scenario also asserts the check REPORTS that it compared the description (the two rule
# names in check-arch's PASSED line, and the `wire_shapes_match_document` test in the test run).
#
# COST — the type-derived layer compiles foundry-api's tests in the copy. The first scenario of a
# run builds the workspace crates once into the dedicated target dir; later ones are incremental.
#
# RED (ADR-025): every scenario @pending; see distill/red-classification.md. Layer 4 (subprocess
# against the source tree, NOT the foundry binary): one named example per drift, never generated.

@api-openapi-document @real-io @drift-check @slow
Feature: The maintainer cannot merge a change that makes the API description lie
  Whoever changes the JSON API runs one check before pushing. It fails, naming the operation, the
  answer and the difference, when the code and the committed description disagree on an address,
  a way of asking, a parameter, an answer, a field or the release; and it passes when nothing the
  description promises has changed.

  Background:
    Given the maintainer works in a copy of the current repository

  @pending @aod-01 @us-aod-02 @driving_port @contract-shape:pure-function
  Scenario: The drift check passes on the current tree, and says what it compared
    When the maintainer runs the drift check
    Then the drift check passes
    And it reports that it compared the API description with the code's routes, statuses and answer shapes

  @pending @aod-01 @us-aod-02 @error @kpi @contract-shape:pure-function
  Scenario Outline: A field the description lacks fails the build
    When the maintainer adds a field "<field>" to <answer> and runs the drift check
    Then the drift check fails
    And it names <where> and the field "<field>"

    Examples:
      | field    | answer            | where                                           |
      | priority | the issue answer  | listing a project's issues and its 200 answer   |
      | hint     | the error answer  | the error answer                                |

  @pending @aod-01 @us-aod-02 @error @kpi @contract-shape:pure-function
  Scenario: An answer the description does not list fails the build
    When the maintainer makes listing a project's issues able to answer 409 and runs the drift check
    Then the drift check fails
    And it names listing a project's issues and the undocumented 409

  @pending @aod-01 @us-aod-02 @error @kpi @contract-shape:pure-function
  Scenario: An operation missing from the description fails the build
    When the maintainer adds an operation that lists a team's projects and runs the drift check
    Then the drift check fails
    And it names "GET /api/v1/teams/{team_slug}/projects" as undocumented

  @pending @aod-01 @us-aod-02 @error @contract-shape:pure-function
  Scenario: A described operation the code no longer offers fails the build
    When the maintainer removes listing a project's issues from the code and runs the drift check
    Then the drift check fails
    And it names "GET /api/v1/teams/{team_slug}/projects/{project_slug}/issues" as described but not offered by the code

  @pending @aod-01 @us-aod-02 @error @kpi @contract-shape:pure-function
  Scenario: A committed description left behind by a release bump fails the build
    When the maintainer bumps the release version without refreshing the committed description and runs the drift check
    Then the drift check fails
    And it names the description's version and the running release, and says to run "cargo xtask openapi"

  @pending @aod-01 @us-aod-02 @edge @contract-shape:pure-function
  Scenario: A wording-only change to the description passes
    When the maintainer rewords the description of listing a project's issues and runs the drift check
    Then the drift check passes
    And it reports that it compared the API description with the code's routes, statuses and answer shapes

  @pending @aod-01 @us-aod-02 @driving_port @contract-shape:bounded-change
  Scenario: One command brings the committed description up to a new release, and changes nothing else
    Given the maintainer has bumped the release version without refreshing the committed description
    When the maintainer runs the description refresh
    Then the committed description's version is the new release
    And every other byte of the committed description is unchanged
    And the drift check now passes on that tree

  @pending @aod-01 @us-aod-02 @edge @contract-shape:unbounded-preservation
  Scenario: Refreshing a description that is already current changes nothing
    When the maintainer runs the description refresh
    Then the committed description is byte for byte what it was

  @pending @aod-01 @us-aod-02 @error @kpi @contract-shape:unbounded-preservation
  Scenario: Checking a description left behind by a release bump refuses it, and changes nothing
    Given the maintainer has bumped the release version without refreshing the committed description
    When the image build checks the committed description without changing it
    Then the description check refuses
    And it names the description's version and the running release, and says to run "cargo xtask openapi"
    And the committed description is byte for byte what it was

  @pending @aod-02 @us-aod-04 @error @kpi @contract-shape:pure-function
  Scenario: Once every operation is described, leaving one out fails the build
    When the maintainer removes revoking a token from the description and runs the drift check
    Then the drift check fails
    And it names "DELETE /api/v1/teams/{team_slug}/projects/{project_slug}/tokens/{jti}" as undocumented
