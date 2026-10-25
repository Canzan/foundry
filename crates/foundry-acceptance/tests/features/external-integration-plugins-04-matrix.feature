# Feature: external-integration-plugins, slice 04 (US-EIP-08, US-EIP-09) — a
# workspace admin connects Foundry's Matrix bot, and a team lead sends a
# project's events to a Matrix room: the token is checked before it is saved
# and never shown again; rooms the bot was not invited to and encrypted rooms
# are refused with the reason (OD-4, OD-5, DDR-13, DDR-14, ADR-EIP-006).
#
# Authored @pending in DISTILL 2026-10-21 (ADR-025 scaffolded RED).
#
# Lane `@needs-matrix` (DEVOPS-3, eip-matrix-lane): excluded from the default
# lane, included in `all`. The homeserver is a pinned Synapse container
# (support::matrix_homeserver, `matrixdotorg/synapse:v1.163.0`, server name
# canzan.dev, SQLite, rate limits raised), shared per run; users and rooms are
# created per scenario with unique local parts, and the scenario's names
# (@foundry:canzan.dev, #auth:canzan.dev) are aliases the driver maps to them.
# Foundry reaches it as https://matrix.canzan.dev through the in-process TLS
# fault proxy and the test-support outbound seam (DEVOPS U-D1, C-7, C-13):
# production builds cannot contain that seam. "The room shows a notice" is read
# from the room's timeline as Lena's Element account sees it.
#
# The private-address refusal copy ("Foundry will not send to this address. An
# instance operator can allow it.") is DESIGN's UI-6, ratified by the user on
# 2026-10-22 (R-1).

# Copy and behaviour beyond D-13 that these scenarios assert were ratified by
# the user on 2026-10-22 (feature-delta DISTILL "[REF] User ratification
# (2026-10-22)", R-1..R-11, with the exact wording).

@external-integration-plugins @eip-04 @us-eip-08 @us-eip-09 @needs-matrix @real-io
Feature: A workspace admin connects Foundry's Matrix bot, and a team lead routes a project to a Matrix room

  Background:
    Given the Canzan Labs workspace with these people
      | person       | workspace role | teams                         |
      | Priya Raman  | admin          | Backend as lead, Ops as lead  |
      | Lena Park    | member         | Backend as lead               |
      | Sam Okafor   | member         | Backend as member             |
      | Marco        | member         |                               |
    And Identity Platform (AUTH) belongs to Backend and Homelab Ops (OPS) belongs to Ops
    And Identity Platform holds these issues
      | key    | title                              | lane        | filed by   |
      | AUTH-7 | Move Grafana sign-in to Keycloak   | In progress | Sam Okafor |
    And the next issue filed in Identity Platform becomes AUTH-12
    And it is 14:02 on 21 Oct 2026
    And the homeserver matrix.canzan.dev has the bot account @foundry:canzan.dev and Lena's account @lena:canzan.dev
    And Lena has the unencrypted room #auth:canzan.dev named "Identity Platform" and 5 members
    And Lena has the end-to-end encrypted room #backend-private:canzan.dev

  @us-eip-08 @driving_port @kpi @contract-shape:bounded-change
  Scenario: A workspace admin connects the Matrix bot and sees who it is
    When Priya connects Matrix with https://matrix.canzan.dev and the bot's access token
    Then Priya reads "Connected as @foundry:canzan.dev on matrix.canzan.dev · by Priya Raman · 21 Oct 2026"
    And the access token field reads "saved 21 Oct 2026"
    And the bot's access token appears nowhere on the workspace integrations page
    And the workspace's integration activity gains the line "21 Oct 2026 · Priya Raman connected Matrix (matrix.canzan.dev)"

  @us-eip-08 @error @contract-shape:unbounded-preservation
  Scenario Outline: A Matrix connection that cannot be checked is not saved
    Given the homeserver <state>
    When Priya connects Matrix with https://matrix.canzan.dev and <token>
    Then Priya reads "<message>"
    And Canzan Labs has no Matrix connection

    Examples:
      | state           | token                                  | message                                                               |
      | answers         | an access token the homeserver refuses | The homeserver did not accept this access token. Nothing was saved.   |
      | does not answer | the bot's access token                 | Foundry could not reach https://matrix.canzan.dev. Nothing was saved. |

  @us-eip-08 @error @kpi @contract-shape:unbounded-preservation
  Scenario Outline: A homeserver on this machine is refused before it is contacted
    When Priya connects Matrix with a homeserver listening on this machine, written <form>
    Then Priya reads "Foundry will not send to this address. An instance operator can allow it."
    And Canzan Labs has no Matrix connection
    And nothing reached the listener on this machine or the homeserver

    # Link-local, metadata and private addresses cannot be observed from a test
    # (nothing listens there); they are pinned by the judge's property test
    # (foundry-integrations core_properties.rs, judge_refuses_every_reserved_range_and_mapped_form).
    Examples:
      | form                              |
      | as 127.0.0.1                      |
      | as an IPv4-mapped IPv6 address    |

  @us-eip-08 @error @contract-shape:unbounded-preservation
  Scenario Outline: Only a workspace admin connects Matrix
    When <person> connects Matrix with https://matrix.canzan.dev and the bot's access token
    Then <person> gets the same not-found page any unknown address gets
    And Canzan Labs has no Matrix connection

    Examples:
      | person |
      | Lena   |
      | Marco  |

  @us-eip-09 @walking_skeleton @driving_port @kpi @contract-shape:bounded-change
  Scenario: A new issue appears in the Matrix room as a formatted notice
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed New issue on Identity Platform to #auth:canzan.dev
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then #auth:canzan.dev shows within 10 seconds a notice from @foundry:canzan.dev with "AUTH-12 Rotate LLDAP bind password" in bold, "New issue in Identity Platform by Sam Okafor · Backlog" and an Open in Foundry link
    And the notice's plain text carries the same two lines

  @us-eip-09 @contract-shape:bounded-change
  Scenario: Saving a Matrix route brings the bot into the room and says so
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena is creating a Matrix route from Identity Platform to #auth:canzan.dev
    When Lena saves the route
    Then Lena reads "Foundry is in #auth:canzan.dev (room "Identity Platform", 6 members)"
    And @foundry:canzan.dev is a member of #auth:canzan.dev

  @us-eip-09 @error @contract-shape:unbounded-preservation
  Scenario Outline: A room Foundry's bot cannot use is explained and no route is saved
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And <precondition>
    And Lena is creating a Matrix route from Identity Platform to <room>
    When Lena saves the route
    Then Lena reads "<message>"
    And Identity Platform still has no route
    And @foundry:canzan.dev is not a member of <room>

    Examples:
      | precondition                                                     | room                         | message                                                                                                       |
      | @foundry:canzan.dev has not been invited to #auth:canzan.dev      | #auth:canzan.dev             | Invite @foundry:canzan.dev to #auth:canzan.dev in your Matrix client first, then try again.                  |
      | Lena has invited @foundry:canzan.dev to #backend-private:canzan.dev | #backend-private:canzan.dev | #backend-private:canzan.dev is end-to-end encrypted. Foundry's bot cannot post to encrypted rooms yet. Choose an unencrypted room. |

  @us-eip-09 @contract-shape:bounded-change
  Scenario: A saved Matrix route sends its test message
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed New issue on Identity Platform to #auth:canzan.dev
    When Lena chooses Send test on the route to #auth:canzan.dev
    Then #auth:canzan.dev shows a notice "Test message from Foundry for Identity Platform (AUTH). This route will post here."

  @us-eip-09 @error @contract-shape:bounded-change
  Scenario: A bot removed from the room flags the route
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed New issue on Identity Platform to #auth:canzan.dev
    And a room moderator has removed @foundry:canzan.dev from #auth:canzan.dev
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then Lena's route to #auth:canzan.dev reads "Needs attention: @foundry:canzan.dev was removed from #auth:canzan.dev. Invite it back, then Resume."

  @us-eip-09 @error @kpi @contract-shape:unbounded-preservation
  Scenario: A room that became encrypted after the route was saved receives nothing in the clear
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed New issue on Identity Platform to #auth:canzan.dev
    And Lena has turned on end-to-end encryption in #auth:canzan.dev
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then Lena's route to #auth:canzan.dev reads "Needs attention: #auth:canzan.dev is now end-to-end encrypted. Foundry's bot cannot post to encrypted rooms yet."
    And #auth:canzan.dev shows no notice from @foundry:canzan.dev about AUTH-12

  @us-eip-08 @error @kpi @contract-shape:bounded-change
  Scenario: A logged-out bot pauses the Matrix routes
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed New issue on Identity Platform to #auth:canzan.dev
    And the operator has logged the bot out of matrix.canzan.dev
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then Lena's route to #auth:canzan.dev reads "Paused: connection lost"
    And Priya receives one email "Foundry lost its Matrix connection for Canzan Labs" within 5 minutes

  @us-eip-08 @kpi @contract-shape:bounded-change
  Scenario: A new token resumes the Matrix routes and delivers what waited
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed New issue on Identity Platform to #auth:canzan.dev
    And the operator has logged the bot out of matrix.canzan.dev
    And Sam has filed "Rotate LLDAP bind password" in Identity Platform
    And Lena's route to #auth:canzan.dev reads "Paused: connection lost"
    When Priya connects Matrix with https://matrix.canzan.dev and a new access token for the bot
    Then Lena's route to #auth:canzan.dev reads "Delivering" with the time of that delivery
    And #auth:canzan.dev shows a notice from @foundry:canzan.dev about AUTH-12

  @us-eip-09 @error @kpi @contract-shape:bounded-change
  Scenario Outline: Text from people can never ping a Matrix room
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed every chat event on Identity Platform to #auth:canzan.dev with Summary with comment text
    When Sam comments "<text>" on AUTH-7
    Then #auth:canzan.dev shows that text without notifying the room or anyone in it

    Examples:
      | text                                               |
      | @room deploy is broken                             |
      | @lena:canzan.dev look at this <b>now</b>           |
      | <a href="https://matrix.to/#/@lena:canzan.dev">Lena</a> please |

  @us-eip-09 @contract-shape:bounded-change
  Scenario: Every Matrix delivery is in the route's history
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed New issue on Identity Platform to #auth:canzan.dev
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then the history of the route to #auth:canzan.dev reads "New issue · AUTH-12 · Delivered"

  @us-eip-09 @contract-shape:bounded-change
  Scenario: A Matrix route carries an event added by editing it
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed New issue on Identity Platform to #auth:canzan.dev
    And Lena has added New comment to the route to #auth:canzan.dev
    When Sam comments "Grafana login verified." on AUTH-7
    Then #auth:canzan.dev shows a notice from @foundry:canzan.dev about AUTH-7

  @us-eip-09 @contract-shape:unbounded-preservation
  Scenario: A paused Matrix route skips events and does not send them later
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed New issue on Identity Platform to #auth:canzan.dev
    And Lena has paused the route to #auth:canzan.dev
    And Sam has filed "Freeze LDAP writes during the migration" in Identity Platform
    When Lena resumes the route to #auth:canzan.dev
    Then #auth:canzan.dev shows no notice from @foundry:canzan.dev about AUTH-12

  @us-eip-09 @contract-shape:bounded-change
  Scenario: Deleting a Matrix route takes the bot out of the room
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed New issue on Identity Platform to #auth:canzan.dev
    And Lena has chosen Delete on the route to #auth:canzan.dev
    When Lena confirms the route's delete
    Then Identity Platform's integrations page no longer lists the route to #auth:canzan.dev
    And @foundry:canzan.dev is not a member of #auth:canzan.dev

  # Matrix disconnect (US-EIP-07 for Matrix; DESIGN door
  # /workspace/integrations/matrix/disconnect). USER DECISION 2026-10-25: built
  # like Slack's (03 route-lifecycle, scenarios 76-78 and the admin-only rows):
  # a counted GET dialog and POST confirm, the routes read K-9 "Paused: Matrix
  # was disconnected", the bot's access token is forgotten, no admin email, the
  # R-4 activity line, and what is filed while disconnected is never sent (R-9).
  # The bot stays in its rooms (chosen over leaving them). The dialog question
  # and the activity line were ratified by the user on 2026-10-25
  # (feature-delta DISTILL DIST-25).

  @us-eip-07 @error @contract-shape:unbounded-preservation
  Scenario: Disconnecting Matrix asks first and counts what stops
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed New issue on Identity Platform to #auth:canzan.dev
    When Priya chooses Disconnect Matrix
    Then Priya reads "Disconnect Matrix? 1 route in 1 project stops posting. Foundry forgets the bot's access token."
    And Canzan Labs' Matrix stays connected and every route reads "Delivering"

  @us-eip-07 @contract-shape:bounded-change
  Scenario: Confirming the Matrix disconnect pauses the routes and forgets the bot's token
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed New issue on Identity Platform to #auth:canzan.dev
    And Priya has chosen Disconnect Matrix
    When Priya confirms the disconnect of Matrix
    Then Lena's route to #auth:canzan.dev reads "Paused: Matrix was disconnected"
    And Priya is offered Connect Matrix on the workspace integrations page
    And Foundry no longer holds the bot's access token
    And @foundry:canzan.dev is a member of #auth:canzan.dev
    And the workspace's integration activity gains the line "21 Oct 2026 · Priya Raman disconnected Matrix (matrix.canzan.dev)"
    And Priya receives no email about the Matrix connection

  @us-eip-07 @edge @contract-shape:unbounded-preservation
  Scenario: What happens while Matrix is disconnected is not sent after reconnecting
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed New issue on Identity Platform to #auth:canzan.dev
    And Priya has disconnected Matrix
    And Sam has filed "Rotate LLDAP bind password" in Identity Platform
    When Priya connects Matrix with https://matrix.canzan.dev and the bot's access token
    Then Lena's route to #auth:canzan.dev reads "Delivering"
    And #auth:canzan.dev shows no notice from @foundry:canzan.dev about AUTH-12

  @us-eip-07 @error @contract-shape:unbounded-preservation
  Scenario Outline: Only a workspace admin disconnects Matrix
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed New issue on Identity Platform to #auth:canzan.dev
    When Lena <asks>
    Then Lena gets the same not-found page any unknown address gets
    And Canzan Labs' Matrix stays connected and every route reads "Delivering"
    And Priya still finds Disconnect Matrix on the workspace integrations page

    Examples:
      | asks                            |
      | chooses Disconnect Matrix       |
      | confirms a disconnect of Matrix |

  @us-eip-09 @error @contract-shape:bounded-change
  Scenario: A notice waiting for an unreachable homeserver arrives when it answers again
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed New issue on Identity Platform to #auth:canzan.dev
    And the homeserver does not answer
    And Sam has filed "Rotate LLDAP bind password" in Identity Platform
    When the homeserver answers again
    Then #auth:canzan.dev shows a notice from @foundry:canzan.dev about AUTH-12

  @us-eip-09 @error @kpi @contract-shape:unbounded-preservation
  Scenario: No notice carries the bot's access token or the secrets key
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed every chat event on Identity Platform to #auth:canzan.dev with Summary with comment text
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then #auth:canzan.dev shows a notice from @foundry:canzan.dev about AUTH-12
    And no notice in #auth:canzan.dev carries the bot's access token or the secrets key

  @us-eip-09 @contract-shape:unbounded-preservation
  Scenario: A Matrix route form names the room's audience
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    When Lena chooses #auth:canzan.dev as the destination of a new Matrix route on Identity Platform
    Then Lena reads "Everyone who can read #auth:canzan.dev will see what this route sends. Foundry cannot check who that is."

  @us-eip-09 @kpi @contract-shape:bounded-change
  Scenario Outline: A Matrix notice reveals comment text only when chosen
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed every chat event on Identity Platform to #auth:canzan.dev with <level>
    When Sam comments "The bind DN changed; staging is green." on AUTH-7
    Then #auth:canzan.dev shows AUTH-7's comment notice <shows> "The bind DN changed; staging is green." in both its formatted and plain text

    Examples:
      | level                     | shows   |
      | Link only                 | without |
      | Summary                   | without |
      | Summary with comment text | with    |

  @us-eip-09 @error @contract-shape:bounded-change
  Scenario Outline: A homeserver that asks Foundry to wait, or fails, is retried
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed New issue on Identity Platform to #auth:canzan.dev
    And the homeserver will answer Foundry's next send with <answer>
    And Sam has filed "Rotate LLDAP bind password" in Identity Platform
    When 1 minute passes
    Then the homeserver gave the answer it was set to give
    And #auth:canzan.dev shows a notice from @foundry:canzan.dev about AUTH-12
    And the history of the route to #auth:canzan.dev reads "New issue · AUTH-12 · Delivered after 2 attempts"

    Examples:
      | answer                                 |
      | a request to slow down for 30 seconds  |
      | a server error                         |

  @us-eip-09 @error @contract-shape:unbounded-preservation
  Scenario: A homeserver's request to wait longer than Foundry's own backoff is honoured
    Given Priya has connected Canzan Labs to Matrix with the bot's access token
    And Lena has invited @foundry:canzan.dev to #auth:canzan.dev
    And Lena has routed New issue on Identity Platform to #auth:canzan.dev
    And the homeserver will answer Foundry's next send with a request to slow down for 30 seconds
    And Sam has filed "Rotate LLDAP bind password" in Identity Platform
    When 20 seconds pass
    Then the homeserver gave the answer it was set to give
    And the homeserver was asked to send AUTH-12's notice only once
