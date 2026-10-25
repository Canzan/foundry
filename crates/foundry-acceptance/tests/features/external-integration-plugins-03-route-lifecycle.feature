# Feature: external-integration-plugins, slice 03 (US-EIP-07) — a team lead
# pauses, edits or deletes a route, and an admin disconnects safely: pause
# skips (D-8, DD-8), a resumed route receives from the resume (DDR-27, UI-10),
# delete and disconnect are a GET dialog plus a POST confirm that counts
# (ADR-ISSUE-DELETE-002), disconnect withdraws Foundry's access at Slack and
# queues nothing (ADR-EIP-005 decision 7), a deleted project sends nothing and
# a restore resumes (D-8), every change is on record (D-16).
#
# Authored @pending in DISTILL 2026-10-21 (ADR-025 scaffolded RED).
#
# Harness as in external-integration-plugins-01-connect-slack.feature. The
# operator's restore is the real, prebuilt binary: `foundry doctor
# restore-project <id>` (project-lifecycle's door). "Without the page's
# token" is the same POST with the double-submit token removed (the CSRF layer
# refuses it).

# Copy and behaviour beyond D-13 that these scenarios assert were ratified by
# the user on 2026-10-22 (feature-delta DISTILL "[REF] User ratification
# (2026-10-22)", R-1..R-11, with the exact wording).

@external-integration-plugins @eip-03 @us-eip-07 @real-io
Feature: A team lead pauses, edits or deletes a route, and an admin disconnects safely

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
    And the next issue filed in Identity Platform becomes AUTH-17
    And it is 14:02 on 21 Oct 2026
    And the instance has the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Lena has routed every chat event on Identity Platform to #auth-dev

  @contract-shape:bounded-change
  Scenario: Pausing a route shows it paused
    When Lena pauses the route to #auth-dev
    Then Lena's route to #auth-dev reads "Paused"

  @contract-shape:unbounded-preservation
  Scenario: A paused route skips events and does not send them later
    Given Lena has paused the route to #auth-dev
    And Sam has filed "Freeze LDAP writes during the migration" in Identity Platform
    When Lena resumes the route to #auth-dev
    Then nothing is posted to #auth-dev within 10 seconds
    And Lena's route to #auth-dev reads "Delivering"

  @contract-shape:bounded-change
  Scenario: A resumed route carries what happens after the resume
    Given Lena has paused the route to #auth-dev
    And Lena has resumed the route to #auth-dev
    When Sam files "Unfreeze LDAP writes" in Identity Platform
    Then #auth-dev shows AUTH-17's message

  @edge @contract-shape:unbounded-preservation
  Scenario: Pausing a route that waits for Slack drops what it was waiting to send
    Given Slack does not answer
    And Sam has filed "Freeze LDAP writes during the migration" in Identity Platform
    And Lena has paused the route to #auth-dev
    And Lena has resumed the route to #auth-dev
    When Slack answers again
    Then nothing is posted to #auth-dev within 10 seconds

  @contract-shape:bounded-change
  Scenario: Editing a route's events takes effect on the next event
    Given Lena has routed Issue moved into Done on Identity Platform to #auth-releases
    And Lena has added New comment to the route to #auth-releases
    When Sam comments "Grafana login verified." on AUTH-7
    Then #auth-releases shows AUTH-7's comment message

  @error @contract-shape:unbounded-preservation
  Scenario: Deleting a route asks first
    When Lena chooses Delete on the route to #auth-dev
    Then Lena reads "Delete the route to #auth-dev? Identity Platform stops posting there. Messages already in Slack stay."
    And Identity Platform's integrations page still lists the route to #auth-dev

  @contract-shape:bounded-change
  Scenario: Confirming the delete removes the route and leaves Slack's messages alone
    Given Sam has filed "Freeze LDAP writes during the migration" in Identity Platform and #auth-dev has shown it
    And Lena has chosen Delete on the route to #auth-dev
    When Lena confirms the route's delete
    Then Identity Platform's integrations page no longer lists the route to #auth-dev
    And Slack was asked to remove no message from #auth-dev
    And the workspace's integration activity gains the line "21 Oct 2026 · Lena Park deleted the route to #auth-dev"

  @pending @error @contract-shape:unbounded-preservation
  Scenario: Disconnecting Slack asks first and counts what stops
    Given Priya has routed every chat event on Homelab Ops to #ops-alerts
    And Lena has routed every chat event on Identity Platform to #auth-releases
    When Priya chooses Disconnect Slack
    Then Priya reads "Disconnect Slack? 3 routes in 2 projects stop posting. Foundry's access to Canzan Labs' Slack is removed."
    And Canzan Labs' Slack stays connected and every route reads "Delivering"

  @pending @contract-shape:bounded-change
  Scenario: Confirming the disconnect pauses the routes and removes Foundry's access
    Given Priya has routed every chat event on Homelab Ops to #ops-alerts
    And Lena has routed every chat event on Identity Platform to #auth-releases
    And Priya has chosen Disconnect Slack
    When Priya confirms the disconnect
    Then the routes to #auth-dev, #auth-releases and #ops-alerts each read "Paused: Slack was disconnected"
    And Priya is offered Add to Slack on the workspace integrations page
    And Slack was asked once to remove Foundry's access to Canzan Labs

  @pending @edge @contract-shape:unbounded-preservation
  Scenario: What happens while Slack is disconnected is not sent after reconnecting
    Given Priya has disconnected Slack
    And Sam has filed "Freeze LDAP writes during the migration" in Identity Platform
    When Priya reconnects Slack and approves Foundry for the Slack workspace "Canzan Labs"
    Then nothing is posted to #auth-dev within 10 seconds

  @pending @contract-shape:bounded-change
  Scenario: A restored project's routes post again
    Given Identity Platform has been deleted by Lena
    And the instance operator has restored Identity Platform
    When Sam files "Freeze LDAP writes during the migration" in Identity Platform
    Then #auth-dev shows AUTH-17's message
    And Lena's route to #auth-dev reads "Delivering" with the time of that delivery

  @pending @error @contract-shape:unbounded-preservation
  Scenario: What was waiting when a project was deleted is never sent
    Given Slack does not answer
    And Sam has filed "Freeze LDAP writes during the migration" in Identity Platform
    And Identity Platform has been deleted by Lena
    And Slack answers again
    When the instance operator restores Identity Platform
    Then nothing is posted to #auth-dev within 10 seconds

  @pending @edge @contract-shape:bounded-change
  Scenario Outline: Every route and connection change is on record
    When <person> <change>
    Then the workspace's integration activity gains the line "<line>"

    Examples:
      | person | change                                       | line                                                       |
      | Lena   | pauses the route to #auth-dev                | 21 Oct 2026 · Lena Park paused the route to #auth-dev      |
      | Lena   | adds Issue deleted to the route to #auth-dev | 21 Oct 2026 · Lena Park edited the route to #auth-dev      |
      | Priya  | disconnects Slack                            | 21 Oct 2026 · Priya Raman disconnected Slack (Canzan Labs) |

  @edge @contract-shape:unbounded-preservation
  Scenario Outline: Asking for what is already so changes nothing
    Given Lena has <state> the route to #auth-dev
    When Lena <asks> the route to #auth-dev
    Then Lena's route to #auth-dev reads "<status>"
    And the workspace's integration activity gains no line

    Examples:
      | state   | asks    | status     |
      | paused  | pauses  | Paused     |
      | resumed | resumes | Delivering |

  @pending @error @contract-shape:unbounded-preservation
  Scenario Outline: Only the right people change routes and connections
    When <person> <change>
    Then <person> gets the same not-found page any unknown address gets
    And Lena's route to #auth-dev reads "Delivering"
    And Canzan Labs' Slack stays connected and every route reads "Delivering"

    Examples:
      | person | change                                   |
      | Sam    | pauses the route to #auth-dev            |
      | Sam    | chooses Delete on the route to #auth-dev |
      | Sam    | opens the edit form of the route to #auth-dev |
      | Marco  | pauses the route to #auth-dev            |
      | Lena   | chooses Disconnect Slack                 |
      | Lena   | confirms a disconnect of Slack           |

  @pending @error @contract-shape:unbounded-preservation
  Scenario Outline: A change sent from outside the page is turned away
    When <person> <change> from outside the page
    Then the answer is turned away as a forged form
    And Lena's route to #auth-dev reads "Delivering"

    Examples:
      | person | change                                   |
      | Lena   | pauses the route to #auth-dev            |
      | Lena   | confirms a delete of the route to #auth-dev |
      | Priya  | confirms a disconnect of Slack           |
