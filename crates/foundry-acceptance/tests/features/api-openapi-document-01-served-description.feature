# Feature: api-openapi-document, slice 01 (US-AOD-01) — every instance serves its own API
# description at /api/v1/openapi.json, with no token, byte-identical to the committed file.
#
# Spec: docs/feature/api-openapi-document/feature-delta.md — DISCUSS D1, D4, D6-D8, D13-D15,
# OQ-5/7/8/10; DESIGN DDD-3/4/5, the Statuses table and Changed Assumption 1 (any method other
# than GET or HEAD on the description's address answers the uniform 404, byte-identical to the
# catch-all, decided by the user 2026-10-18).
#
# HARNESS — HTTP lane, layer 3. reqwest against the SAME in-process router every API feature uses
# (`InProcHarness::spawn`, real Postgres, per-scenario schema, the fixed EdDSA test key). The
# Background seeds Canzan Labs through SQL and files AUTH-1..AUTH-7 through the SHIPPED create
# operation with a fixture token, so every precondition goes through a real door or a plain row.
#
# ORACLES never read foundry-api's constants:
#   * "the running release" is foundry-app's crate version read from its Cargo.toml at compile
#     time, and the page footer is fetched and compared (the release-version-footer oracle);
#   * "the committed API description" is `crates/foundry-api/openapi.json` read from disk at test
#     time;
#   * the API's not-found answer is compared byte for byte with an address that was never part of
#     the API (`/api/v1/no-such-address`).
# Refusal-shaped rows carry a control ("the description is still served"), because before the
# feature every one of them already answers the catch-all 404 and would pass for the wrong reason.
#
# RED (ADR-025 scaffolded RED): every scenario was authored pending, the walking skeleton included (house
# precedent: notification-preferences-ui, team-membership). DELIVER's first step un-pends the
# skeleton and makes it GREEN. Classification: docs/feature/api-openapi-document/distill/red-classification.md.
#
# Layer 3: example-based only, sad paths enumerated (Mandates 9 and 11). Steps:
# crates/foundry-acceptance/src/steps/feature_api_openapi_document.rs; driver:
# crates/foundry-acceptance/src/support/api_openapi_document.rs.

@api-openapi-document @real-io
Feature: An integrator downloads the API's description from the instance they script against
  Sven scripts against a Foundry instance. He fetches the instance's own description of its API,
  with no token, and imports it into his tools. Every caller on one release gets the same bytes,
  the bytes committed in the repository, and nothing in them is about the instance itself.

  Background:
    Given Foundry is running for the workspace "Canzan Labs", administered by Priya
    And team Backend's project "auth" holds the issues AUTH-1 to AUTH-7
    And Sven holds the machine token "ci-issue-filer" that Priya minted
    And team Ops exists, and Sven also holds a machine token that reaches only Ops

  @aod-01 @us-aod-01 @walking_skeleton @driving_port @kpi @contract-shape:pure-function
  Scenario: An integrator downloads the API description from the instance without a token
    When Sven fetches /api/v1/openapi.json with no credential
    Then Sven gets the API description as an OpenAPI 3.1 JSON document titled "Foundry API v1"
    And it describes listing a project's issues, the machine token and the error answer
    And its version is the running release, the same one the page footer shows
    And it names "/" as its only server, so every address resolves on the instance it came from

  @aod-01 @us-aod-01 @kpi @contract-shape:pure-function
  Scenario: The description an instance serves is the one committed in the repository
    When Sven fetches /api/v1/openapi.json with no credential
    Then the bytes he receives equal the committed API description exactly

  @aod-01 @us-aod-01 @error @kpi @contract-shape:unbounded-preservation
  Scenario Outline: A bad or stale credential does not change the answer
    Given Sven has fetched /api/v1/openapi.json with no credential
    When Sven fetches /api/v1/openapi.json with <credential>
    Then he gets the same 200 and the same bytes as with no credential

    Examples:
      | credential                        |
      | an expired machine token          |
      | a forged machine token            |
      | a revoked machine token           |
      | a malformed bearer value          |
      | Priya's signed-in browser session |

  @aod-01 @us-aod-01 @edge @kpi @contract-shape:unbounded-preservation
  Scenario: The description names nothing about the instance serving it
    Given the instance also holds the team "Platform Ops" and its project "Identity Platform"
    When Sven fetches /api/v1/openapi.json with no credential
    Then the description names no workspace, team, project, person, address or token of the instance

  @aod-01 @us-aod-01 @edge @kpi @contract-shape:unbounded-preservation
  Scenario: Two instances running the same release serve the same description
    Given a second Foundry of the same release runs for the workspace "Globex" with its own teams
    When Sven fetches /api/v1/openapi.json from both instances
    Then the two descriptions are the same bytes

  @aod-01 @us-aod-01 @error @contract-shape:unbounded-preservation
  Scenario Outline: An address next to the description still answers the API's not-found
    When Sven requests <address> with no credential
    Then he gets the API's not-found answer, the same as for an address that was never part of the API
    And the description is still served at /api/v1/openapi.json

    Examples:
      | address                  |
      | /api/v1/openapi.yaml     |
      | /api/v1/openapi          |
      | /api/v1/openapi.json/    |
      | /api/v1/OPENAPI.JSON     |

  @aod-01 @us-aod-01 @error @contract-shape:unbounded-preservation
  Scenario Outline: Anything but reading the description's address answers the API's not-found
    When Sven sends <write> to /api/v1/openapi.json
    Then he gets the API's not-found answer, the same as for an address that was never part of the API, and no list of allowed methods
    And the description is still served at /api/v1/openapi.json

    Examples:
      | write    |
      | a POST   |
      | a PUT    |
      | a PATCH  |
      | a DELETE |

  @aod-01 @us-aod-01 @edge @contract-shape:pure-function
  Scenario: Reading the description needs no credential, and every API operation asks for the machine token
    When Sven fetches /api/v1/openapi.json with no credential
    Then the description says reading it needs no credential
    And every other operation it lists asks for the machine token

  @aod-01 @us-aod-01 @contract-shape:unbounded-preservation
  Scenario: Listing a project's issues answers the way the description says
    When Sven tries to list a project's issues with his token
    Then he gets 200, exactly as the description documents it for that operation
    And nothing in Canzan Labs changed

  @aod-01 @us-aod-01 @error @contract-shape:unbounded-preservation
  Scenario Outline: A refused list of a project's issues answers the way the description says
    When Sven tries to list a project's issues <situation>
    Then he gets <status>, exactly as the description documents it for that operation
    And nothing in Canzan Labs changed

    Examples:
      | situation                                 | status |
      | with no token                             | 401    |
      | with the token that reaches only Ops      | 404    |
      | of a project that does not exist          | 404    |
      | in a way the address does not offer       | 405    |
