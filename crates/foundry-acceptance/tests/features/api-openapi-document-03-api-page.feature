# Feature: api-openapi-document, slice 03 (US-AOD-05, US-AOD-06) — a self-hosted page to read the
# API and try a call, where a pasted token stays in the tab and goes only to this instance.
#
# Spec: docs/feature/api-openapi-document/feature-delta.md — DISCUSS D2, D10-D12, D14, OQ-2/3/4,
# US-AOD-05/06; DESIGN DDD-15..20, ADR-API-OPENAPI-003/004, [REF] Test Seams (browser lane).
#
# OQ-U4 IS OPEN: these scenarios assume swagger-ui 5.33.3 runs WITHOUT 'unsafe-eval' (DDD-19).
# "the browser refused nothing the page asked for" is the runtime probe of that assumption: an
# eval refused by the content policy is reported by the browser and turns the scenario RED, which
# is the slice 03 step-1 signal to stop and ask the user (OQ-U4), never to loosen the policy.
#
# TWO LANES
#   * HTTP lane (no tag): reqwest against the in-process router — the page's headers, the files it
#     uses, and that it is the same for anyone and sets no cookie. Runs in the default lane.
#   * Browser lane (@needs-browser): real headless Chrome (selenium/standalone-chrome in Docker,
#     ADR-007), excluded from the default lane and run by `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo
#     xtask ci` or `FOUNDRY_ACCEPTANCE_TAGS=aod-03`. Oracles: the browser's resource timing
#     (every request's origin), the browser log armed at session creation (`goog:loggingPrefs`,
#     so content-policy refusals are recorded from the first byte), the page's storage and
#     address read after a call and after a reload ("no trace" means the token string is absent,
#     not that storage is empty: `foundry.theme` may exist), and a second local origin (another
#     port on 127.0.0.1, which the browser treats as another site) that records every request it
#     receives and serves a frame page, a description and a configuration of its own.
#   * The "description cannot be loaded" scenario blocks /api/v1/openapi.json in the browser
#     through the driver's DevTools endpoint (`Network.setBlockedURLs`); recorded as unobserved
#     until the browser lane runs it.
#
# swagger-ui's own markup is upstream's: the driver reads it through swagger-ui 5's stable class
# names (`.opblock`, `.try-out__btn`, `.execute`, `.live-responses-table`, `.curl`, `.authorize`)
# and through the page's own copy for everything Foundry authored (title, heading line, noscript,
# the Authorize label and hint, D14).
#
# RED (ADR-025): every scenario @pending. Before the feature /docs/api answers the uniform
# not-found page, so every scenario stops at its first look at the page.

@api-openapi-document @real-io
Feature: An integrator reads the API and tries a call in a page served by their own instance
  Sven opens the instance's API page, reads every operation, pastes the machine token Priya gave
  him and tries a real call. The page loads nothing from anywhere else, shows only this instance's
  description, sends the token only to this instance, and forgets it on reload.

  Background:
    Given Foundry is running for the workspace "Canzan Labs", administered by Priya
    And team Backend's project "auth" holds the issues AUTH-1 to AUTH-7
    And Sven holds the machine token "ci-issue-filer" that Priya minted
    And Mei, a member of team Backend, holds her own machine token
    And Canzan Labs also has the machine tokens "scratch-rotation" and the revoked "old-ci-issue-filer"

  # ---------------------------------------------------------------------------------------------
  # US-AOD-05: reading the API in the page
  # ---------------------------------------------------------------------------------------------

  @aod-03 @us-aod-05 @driving_port @needs-browser @kpi @contract-shape:pure-function
  Scenario: An integrator reads every API operation in a page on their own instance
    When Sven opens the API page
    Then he sees the title "Foundry API" and the line "Foundry API v{running release}. The OpenAPI document is at /api/v1/openapi.json."
    And every operation of the API description is listed on the page

  @aod-03 @us-aod-05 @error @needs-browser @kpi @contract-shape:unbounded-preservation
  Scenario: The API page loads nothing from another site
    When Sven opens the API page and it finishes loading
    Then every request the browser made went to the instance's own address
    And the browser refused nothing the page asked for

  @aod-03 @us-aod-05 @error @needs-browser @contract-shape:unbounded-preservation
  Scenario: Another site cannot show the API page inside its own page
    Given a page on another site embeds the API page in a frame
    When Sven's browser opens that page
    Then the browser refuses to show the API page inside it

  @aod-03 @us-aod-05 @error @contract-shape:pure-function
  Scenario: The API page tells the browser to refuse anything from another site
    When a visitor asks for the API page
    Then the answer carries the content policy "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; object-src 'none'"
    And the answer tells browsers never to frame it, never to guess its type and never to send a referrer
    And the description at /api/v1/openapi.json and the sign-in page carry no content policy

  @aod-03 @us-aod-05 @needs-browser @contract-shape:pure-function
  Scenario: The API page tells a visitor without scripting where the description is
    Given Priya's browser has scripting disabled
    When Priya opens the API page
    Then she reads "This page needs JavaScript. The OpenAPI document is at /api/v1/openapi.json." on the API page
    And "/api/v1/openapi.json" on the API page is a link to the description

  @aod-03 @us-aod-05 @error @needs-browser @contract-shape:unbounded-preservation
  Scenario: The API page says so when the description cannot be loaded
    Given the API description cannot be loaded by Sven's browser
    When Sven opens the API page and it finishes loading
    Then Sven sees that the description could not be loaded, and the line naming /api/v1/openapi.json is still there

  @aod-03 @us-aod-05 @edge @needs-browser @contract-shape:unbounded-preservation
  Scenario: Other pages do not carry the API page's code
    Given Sven has opened the API page, which loaded its own files
    When Priya opens Backend's board in her browser
    Then the board loads none of the API page's files

  @aod-03 @us-aod-05 @adapter-integration @contract-shape:pure-function
  Scenario: The API page's files come from the instance itself, exactly as the repository holds them
    When Sven fetches every file the API page uses
    Then each answers with the bytes the repository holds for it

  @aod-03 @us-aod-05 @edge @contract-shape:unbounded-preservation
  Scenario: The API page is the same for everyone, and remembers no one
    When a visitor and Priya, signed in, each ask for the API page
    Then both get the same page, and neither answer sets a cookie

  # ---------------------------------------------------------------------------------------------
  # US-AOD-06: trying a call, and where the token goes
  # ---------------------------------------------------------------------------------------------

  @pending @aod-03 @us-aod-06 @driving_port @needs-browser @kpi @contract-shape:unbounded-preservation
  Scenario: An integrator lists a project's issues from the API page with a pasted token
    When Sven pastes "ci-issue-filer" into the API page and tries listing Backend's "auth" project issues
    Then he sees a 200 answer listing AUTH-7
    And the call went to the instance's own address
    And the page shows the command line that made the call, carrying the pasted token

  @pending @aod-03 @us-aod-06 @edge @needs-browser @contract-shape:pure-function
  Scenario: The token field says where the token goes
    When Sven opens the API page's token dialog
    Then the token field is labelled "Machine token" and says "Kept in this tab only and sent only to this Foundry. Reloading forgets it."

  @pending @aod-03 @us-aod-06 @needs-browser @contract-shape:bounded-change
  Scenario: Sven revokes a token from the API page
    When Sven pastes "ci-issue-filer" into the API page and revokes "scratch-rotation"
    Then he sees the 204 answer, and "scratch-rotation" is revoked

  @pending @aod-03 @us-aod-06 @error @needs-browser @contract-shape:unbounded-preservation
  Scenario: Trying a call without a token shows the documented refusal
    When Sven tries listing Backend's "auth" project issues from the API page without a token
    Then he sees the 401 answer with the error code "unauthorized"

  @pending @aod-03 @us-aod-06 @error @needs-browser @contract-shape:unbounded-preservation
  Scenario Outline: A refused call from the page shows the documented answer
    Given <precondition>
    When Sven tries <call> from the API page with <token>
    Then he sees the <status> answer with the error code "<code>"

    Examples:
      | precondition                             | call                                    | token                                  | status | code         |
      | Canzan Labs is as the Background left it | listing Backend's "auth" project issues | the revoked token "old-ci-issue-filer" | 401    | unauthorized |
      | Canzan Labs is as the Background left it | listing Backend's "auth" project issues | an expired machine token               | 401    | unauthorized |
      | Canzan Labs is as the Background left it | listing Backend's "auth" project issues | a forged machine token                 | 401    | unauthorized |
      | Canzan Labs is as the Background left it | revoking "scratch-rotation"             | Mei's token                            | 403    | forbidden    |
      | Sven has used his revoke budget          | revoking "scratch-rotation"             | "ci-issue-filer"                       | 429    | rate_limited |

  @pending @aod-03 @us-aod-06 @error @needs-browser @kpi @contract-shape:unbounded-preservation
  Scenario: A pasted token is forgotten on reload and never stored
    Given Sven has already made a call from the page with "ci-issue-filer"
    When Sven reloads the API page
    Then no token is filled in
    And the browser keeps no trace of the token

  @pending @aod-03 @us-aod-06 @error @needs-browser @kpi @contract-shape:unbounded-preservation
  Scenario: A pasted token is never sent to another site
    When Sven pastes "ci-issue-filer" into the API page and tries every operation once
    Then every request the browser made went to the instance's own address

  @pending @aod-03 @us-aod-06 @error @needs-browser @kpi @contract-shape:unbounded-preservation
  Scenario Outline: A link cannot make the API page show someone else's description
    Given another site serves its own API description and configuration
    When Priya opens the API page from a link that names that site's <what>
    Then the page shows the instance's own API description
    And the browser made no request to that site

    Examples:
      | what          |
      | description   |
      | configuration |
