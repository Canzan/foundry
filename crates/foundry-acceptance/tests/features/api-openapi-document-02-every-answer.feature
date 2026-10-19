# Feature: api-openapi-document, slice 02 (US-AOD-03, US-AOD-04) — the description covers every
# /api/v1 operation and every answer it can give, and every answer the server really gives is the
# one described.
#
# Spec: docs/feature/api-openapi-document/feature-delta.md — DISCUSS C1-C11, D4, D9, D14, D15,
# US-AOD-03/04; DESIGN C12/C13, DDD-5..8, the Statuses table (pinned below, row by row), Changed
# Assumptions 1 and 2, [REF] For Acceptance Designer (the real-request matrix).
#
# TWO KINDS OF SCENARIO
#   * Reading the description (US-AOD-03/04 scenarios 1, 3-6; US-AOD-04 2-3): the description is
#     fetched from the instance and read as a document — which operations it lists, which answers
#     each lists, the body kind and error codes of each, the D14 copy byte for byte.
#   * The real-request matrix (US-AOD-03 scenario 2, US-AOD-04 scenario 5; DESIGN "one row per
#     (operation, deterministic status), 500 excluded"): a real request through the real router,
#     then the answer is judged ONLY against the description: the status must be listed for that
#     operation; a JSON body must satisfy the documented schema, validated by `jsonschema` 0.59
#     (draft 2020-12, default features off: no network resolver, DESIGN DDD-12); an
#     error answer must equal the documented example for its code; a plain-text answer must say
#     text/plain; an empty answer must be empty; a 201 carries the documented `Location`, a 405
#     the documented `Allow`. The extractor refusals (400/413/415/422 plain text) are rows here
#     too, so the integrator's real answers are all exercised in the acceptance lane (the
#     conformance test repeats them below the HTTP lane).
#
# UNIVERSE (Mandate 8, layer 3): every matrix row snapshots Canzan Labs — every issue, comment and
# machine token as stored (the token's last-used stamp excluded: every authenticated call touches
# it) — after its own setup and before its request. A refusal changes nothing; a success changes
# exactly the one thing its answer reports (a new AUTH-8, AUTH-7's state, one new or edited
# comment, one revoked token). Read-only base-table SQL for the universe is the house exception
# (project-lifecycle U2, OQ-D6).
#
# RED (ADR-025): every scenario @pending. Before the feature the answers already exist, so each row
# is RED only because the description is not served; that is the missing functionality.

@api-openapi-document @real-io
Feature: The description tells an integrator every operation's real answers
  Sven writes one branch per answer his script can get. The description lists every operation of
  the API, every answer each can give and the shape of each, says where a machine token comes from
  and that the API cannot make one, and every answer the server really gives is the one described.

  Background:
    Given Foundry is running for the workspace "Canzan Labs", administered by Priya
    And team Backend's project "auth" holds the issues AUTH-1 to AUTH-7
    And Sven holds the machine token "ci-issue-filer" that Priya minted
    And Mei, a member of team Backend, holds her own machine token
    And team Ops exists, and Sven also holds a machine token that reaches only Ops

  # ---------------------------------------------------------------------------------------------
  # Reading the description
  # ---------------------------------------------------------------------------------------------

  @pending @aod-02 @us-aod-03 @us-aod-04 @driving_port @kpi @contract-shape:pure-function
  Scenario Outline: Every operation is described with every answer it can give
    When Sven reads how the description says he can <operation>
    Then it lists exactly the answers <answers>

    Examples:
      | operation                    | answers                                               |
      | read the API description     | 200, 404                                              |
      | list a project's issues      | 200, 401, 404, 405, 500                               |
      | create an issue              | 201, 400, 401, 404, 405, 413, 415, 422, 500           |
      | change an issue's state      | 200, 400, 401, 404, 405, 413, 415, 422, 500           |
      | read an issue's history      | 200, 400, 401, 404, 405, 500                          |
      | comment on an issue          | 201, 400, 401, 404, 405, 413, 415, 422, 500           |
      | edit a comment               | 200, 400, 401, 403, 404, 405, 410, 413, 415, 422, 500 |
      | list the workspace's tokens  | 200, 401, 403, 404, 405, 500                          |
      | revoke a token               | 204, 400, 401, 403, 404, 405, 429, 500                |

  @pending @aod-02 @us-aod-03 @us-aod-04 @edge @contract-shape:pure-function
  Scenario Outline: Each refusal lists the error codes it can carry
    When Sven reads how the description says he can <operation>
    Then its <status> answer carries only the error codes <codes>

    Examples:
      | operation                    | status | codes                                 |
      | list a project's issues      | 401    | unauthorized                          |
      | list a project's issues      | 404    | not_found                             |
      | list a project's issues      | 500    | internal                              |
      | create an issue              | 422    | title_required, description_too_long  |
      | change an issue's state      | 422    | invalid_state                         |
      | comment on an issue          | 422    | comment_empty, comment_too_long       |
      | edit a comment               | 422    | comment_empty, comment_too_long       |
      | edit a comment               | 403    | forbidden                             |
      | edit a comment               | 410    | gone                                  |
      | list the workspace's tokens  | 403    | forbidden                             |
      | revoke a token               | 403    | forbidden                             |
      | revoke a token               | 429    | rate_limited                          |
      | read the API description     | 404    | not_found                             |

  @pending @aod-02 @us-aod-03 @edge @contract-shape:pure-function
  Scenario: No answer the code cannot give is described
    When Sven reads the API description
    Then no operation lists a 409 answer, and no answer carries the code "invalid_json"

  @pending @aod-02 @us-aod-03 @us-aod-04 @contract-shape:pure-function
  Scenario Outline: The shapes the description gives are the ones the server sends
    When Sven reads the shape of <thing> in the API description
    Then it has exactly the fields <fields>
    And the fields it requires are <required>
    And the fields that may be null are <nullable>

    Examples:
      | thing                     | fields                                                                                      | required                                                                                    | nullable                                                     |
      | an issue                  | key, number, title, state                                                                   | key, number, title, state                                                                   | none                                                         |
      | a comment                 | id, author_email, body_html, edited                                                         | id, author_email, body_html, edited                                                         | none                                                         |
      | an issue history entry    | actor, field, old, new, at                                                                  | actor, field, old, new, at                                                                  | old                                                          |
      | a token                   | jti, label, scope_team_id, scope_team_name, expires_at, revoked, last_used_at, minted_by    | jti, label, scope_team_id, scope_team_name, expires_at, revoked, last_used_at, minted_by    | scope_team_id, scope_team_name, last_used_at, minted_by      |
      | the error answer          | error                                                                                       | error                                                                                       | none                                                         |
      | a new issue request       | title, description                                                                          | title                                                                                       | none                                                         |
      | a state change request    | state                                                                                       | state                                                                                       | none                                                         |
      | a comment request         | body                                                                                        | body                                                                                        | none                                                         |

  @pending @aod-02 @us-aod-04 @error @kpi @contract-shape:pure-function
  Scenario: The description never shows a token's value
    When Sven reads the shape of a token in the API description
    Then nothing in it could hold a token's value

  @pending @aod-02 @us-aod-03 @edge @contract-shape:pure-function
  Scenario: An issue's state is described as one of its project's lanes, not a fixed list
    When Sven reads how the description describes an issue's state
    Then it is a text with no fixed list of values
    And its description reads "The lane the issue is in, by its slug. Each project has its own lanes; the defaults are backlog, todo, in_progress, done and cancelled. `in-progress` and `canceled` are accepted as aliases."

  @pending @aod-02 @us-aod-03 @edge @contract-shape:pure-function
  Scenario: Malformed requests are described as they really answer
    When Sven reads how the description says he can create an issue
    Then its 400, 413 and 415 answers and one of its 422 answers have plain-text bodies
    And its other 422 answer is the error answer with the codes title_required and description_too_long

  @pending @aod-02 @us-aod-03 @edge @contract-shape:pure-function
  Scenario: Refusals that must not reveal anything are described as uniform
    When Sven reads the API description
    Then every operation's 404 answer is the error answer "not_found" with the message "not found"
    And that 404 answer is described as "Not found. The same answer, byte for byte, for a team, project, issue or comment that does not exist, one your token cannot reach, and a path that is not part of the API."
    And the 401 answer is described once, for the machine token, and says "Also answered when the token cannot be checked."

  @pending @aod-02 @us-aod-03 @edge @contract-shape:pure-function
  Scenario: A wrong way of asking at a known address is described
    When Sven reads how the description describes the address of one issue
    Then it offers only changing the issue's state there
    And it says any other way of asking answers 405, naming the ways that are offered

  @pending @aod-02 @us-aod-04 @error @contract-shape:pure-function
  Scenario: The description offers no way to create a token, and says where one comes from
    When Sven looks for a way to get a machine token in the API description
    Then no operation creates a machine token, and asking to create one at the tokens address is described as answering 405
    And the machine token is described as "A machine token: an EdDSA-signed JWT sent as `Authorization: Bearer <token>`. A workspace admin mints one, signed in to Foundry, at /admin/tokens; the value is shown once. This API lists and revokes tokens but cannot create one."

  @pending @aod-02 @us-aod-04 @driving_port @contract-shape:unbounded-preservation
  Scenario: The place the description sends people to mint a token exists
    Given Priya is signed in to Canzan Labs as its workspace admin
    When Priya opens the address where the description says machine tokens are minted
    Then she reaches the page where she can mint a machine token

  # ---------------------------------------------------------------------------------------------
  # The real-request matrix: every deterministic answer, produced for real, judged by the
  # description alone (500 is documented and held by the drift check only). Every row's setup is
  # its own Given (<precondition>); the When is the one request, and it snapshots the universe
  # after all setup and immediately before that request.
  # ---------------------------------------------------------------------------------------------

  @pending @aod-02 @us-aod-03 @kpi @contract-shape:bounded-change
  Scenario Outline: An issue or comment call that succeeds answers the way the description says
    Given <precondition>
    When Sven tries to <operation> <situation>
    Then he gets <status>, exactly as the description documents it for that operation
    And Canzan Labs changed only as that answer reports

    Examples:
      | precondition                                     | operation                | situation                          | status |
      | Canzan Labs is as the Background left it         | create an issue          | with his token                     | 201    |
      | Canzan Labs is as the Background left it         | change an issue's state  | with his token                     | 200    |
      | Canzan Labs is as the Background left it         | change an issue's state  | to the alias "in-progress"         | 200    |
      | Backend's project auth has its own lane "review" | change an issue's state  | to the project's own lane "review" | 200    |
      | AUTH-7 has moved to todo                         | read an issue's history  | with his token                     | 200    |
      | Canzan Labs is as the Background left it         | read an issue's history  | of an issue that never changed     | 200    |
      | Canzan Labs is as the Background left it         | comment on an issue      | with his token                     | 201    |
      | Sven has commented on AUTH-7                     | edit a comment           | of his own                         | 200    |
      | Canzan Labs is as the Background left it         | read the API description | with his token                     | 200    |

  @pending @aod-02 @us-aod-03 @error @kpi @contract-shape:unbounded-preservation
  Scenario Outline: A refused issue or comment call answers the way the description says
    Given <precondition>
    When Sven tries to <operation> <situation>
    Then he gets <status>, exactly as the description documents it for that operation
    And nothing in Canzan Labs changed

    Examples:
      | precondition                             | operation                                     | situation                                | status |
      | Canzan Labs is as the Background left it | create an issue                               | with no token                            | 401    |
      | Canzan Labs is as the Background left it | create an issue                               | in a project that does not exist         | 404    |
      | Canzan Labs is as the Background left it | create an issue                               | with an empty title                      | 422    |
      | Canzan Labs is as the Background left it | create an issue                               | with a title of 257 characters           | 422    |
      | Canzan Labs is as the Background left it | create an issue                               | with a description of 262,145 characters | 422    |
      | Canzan Labs is as the Background left it | create an issue                               | with a body that is not JSON             | 400    |
      | Canzan Labs is as the Background left it | create an issue                               | without saying the body is JSON          | 415    |
      | Canzan Labs is as the Background left it | create an issue                               | with a misspelt title field              | 422    |
      | Canzan Labs is as the Background left it | create an issue                               | with a body over 2 MB                    | 413    |
      | Canzan Labs is as the Background left it | create an issue                               | in a way the address does not offer      | 405    |
      | Canzan Labs is as the Background left it | change an issue's state                       | with no token                            | 401    |
      | Canzan Labs is as the Background left it | change an issue's state                       | of an issue that does not exist          | 404    |
      | Canzan Labs is as the Background left it | change an issue's state                       | to a lane the project does not have      | 422    |
      | Canzan Labs is as the Background left it | change an issue's state                       | of an issue numbered "seven"             | 400    |
      | Canzan Labs is as the Background left it | change an issue's state                       | with a body that is not JSON             | 400    |
      | Canzan Labs is as the Background left it | change an issue's state                       | without saying the body is JSON          | 415    |
      | Canzan Labs is as the Background left it | change an issue's state                       | without a state                          | 422    |
      | Canzan Labs is as the Background left it | change an issue's state                       | with a body over 2 MB                    | 413    |
      | Canzan Labs is as the Background left it | change an issue's state                       | in a way the address does not offer      | 405    |
      | Canzan Labs is as the Background left it | read an issue's history                       | with no token                            | 401    |
      | Canzan Labs is as the Background left it | read an issue's history                       | of an issue that does not exist          | 404    |
      | Canzan Labs is as the Background left it | read an issue's history                       | of an issue numbered "seven"             | 400    |
      | Canzan Labs is as the Background left it | read an issue's history                       | in a way the address does not offer      | 405    |
      | Canzan Labs is as the Background left it | comment on an issue                           | with no token                            | 401    |
      | Canzan Labs is as the Background left it | comment on an issue                           | of an issue that does not exist          | 404    |
      | Canzan Labs is as the Background left it | comment on an issue                           | with an empty comment                    | 422    |
      | Canzan Labs is as the Background left it | comment on an issue                           | with a comment of 65,537 characters      | 422    |
      | Canzan Labs is as the Background left it | comment on an issue                           | with a body that is not JSON             | 400    |
      | Canzan Labs is as the Background left it | comment on an issue                           | without saying the body is JSON          | 415    |
      | Canzan Labs is as the Background left it | comment on an issue                           | without a comment body                   | 422    |
      | Canzan Labs is as the Background left it | comment on an issue                           | with a body over 2 MB                    | 413    |
      | Canzan Labs is as the Background left it | comment on an issue                           | in a way the address does not offer      | 405    |
      | Sven has commented on AUTH-7             | edit a comment                                | with no token                            | 401    |
      | Mei has commented on AUTH-7              | edit a comment                                | written by someone else                  | 403    |
      | Sven has commented on AUTH-6             | edit a comment                                | of another issue                         | 404    |
      | Sven's comment on AUTH-7 was deleted     | edit a comment                                | that was deleted                         | 410    |
      | Sven has commented on AUTH-7             | edit a comment                                | with an empty comment                    | 422    |
      | Sven has commented on AUTH-7             | edit a comment                                | with a comment of 65,537 characters      | 422    |
      | Sven has commented on AUTH-7             | edit a comment                                | with a comment id that is not an id      | 400    |
      | Sven has commented on AUTH-7             | edit a comment                                | with a body that is not JSON             | 400    |
      | Sven has commented on AUTH-7             | edit a comment                                | without saying the body is JSON          | 415    |
      | Sven has commented on AUTH-7             | edit a comment                                | without a comment body                   | 422    |
      | Sven has commented on AUTH-7             | edit a comment                                | with a body over 2 MB                    | 413    |
      | Sven has commented on AUTH-7             | edit a comment                                | in a way the address does not offer      | 405    |
      | Canzan Labs is as the Background left it | read the API description                      | in a way the address does not offer      | 404    |
      | Canzan Labs is as the Background left it | reach an address outside the API's operations | with his token                           | 404    |

  @pending @aod-02 @us-aod-04 @kpi @contract-shape:bounded-change
  Scenario Outline: A token call that succeeds answers the way the description says
    Given Canzan Labs also has the machine tokens "scratch-rotation" and the revoked "old-ci-issue-filer"
    And <precondition>
    When Sven tries to <operation> <situation>
    Then he gets <status>, exactly as the description documents it for that operation
    And Canzan Labs changed only as that answer reports

    Examples:
      | precondition                             | operation                   | situation               | status |
      | Canzan Labs is as the Background left it | list the workspace's tokens | with his token          | 200    |
      | Canzan Labs is as the Background left it | revoke a token              | with his token          | 204    |
      | Canzan Labs is as the Background left it | revoke a token              | that is already revoked | 204    |

  @pending @aod-02 @us-aod-04 @error @kpi @contract-shape:unbounded-preservation
  Scenario Outline: A refused token call answers the way the description says
    Given Canzan Labs also has the machine tokens "scratch-rotation" and the revoked "old-ci-issue-filer"
    And <precondition>
    When Sven tries to <operation> <situation>
    Then he gets <status>, exactly as the description documents it for that operation
    And nothing in Canzan Labs changed

    Examples:
      | precondition                             | operation                   | situation                           | status |
      | Canzan Labs is as the Background left it | list the workspace's tokens | with no token                       | 401    |
      | Canzan Labs is as the Background left it | list the workspace's tokens | with Mei's token                    | 403    |
      | Canzan Labs is as the Background left it | list the workspace's tokens | of a project that does not exist    | 404    |
      | Canzan Labs is as the Background left it | list the workspace's tokens | by creating one                     | 405    |
      | Canzan Labs is as the Background left it | revoke a token              | with no token                       | 401    |
      | Canzan Labs is as the Background left it | revoke a token              | with Mei's token                    | 403    |
      | Canzan Labs is as the Background left it | revoke a token              | of a project that does not exist    | 404    |
      | Canzan Labs is as the Background left it | revoke a token              | with a token id that is not an id   | 400    |
      | Sven has used his revoke budget          | revoke a token              | past the revoke limit               | 429    |
      | Canzan Labs is as the Background left it | revoke a token              | in a way the address does not offer | 405    |
