# Story: US-07 — User creates and views a project under a team
# Slice: 1 (Walking Skeleton)
# JTBD: outcome-4 (Linear-feel hierarchy: teams own projects, projects own issues)
#
# Driving port: HTTP form POST to /team/{team_slug}/projects — see
# architecture.md and data-access.md § projects table.
# Driven adapters exercised: real Postgres (projects, teams,
# team_memberships, workspace_memberships).
#
# A project key is 2 to 10 capital letters A to Z. Every refused key is
# answered with one sentence that states that whole rule, and nothing is
# created. A project that already carries a shorter key keeps it, keeps its
# issue keys and keeps taking new issues under it. Laws L1, L4, L5 and L6.

@slice1 @us-07 @driving_port
Feature: A team member creates a project and reaches its empty board
  A workspace member who belongs to a team can create a project under that
  team, immediately navigate to its board, and see the three default state
  columns (board-lane-management D4: Backlog, In-Progress, Done). Project names are unique within a team; project keys are unique
  within a workspace and match the agreed shape.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And Mei is signed in

  @walking_skeleton @real-io
  Scenario: Member creates a project under their team and lands on its empty board
    When Mei creates a project under "Backend" with name "Auth v2" and key prefix "AUTH"
    Then the response redirects to "/team/backend/project/auth-v2"
    And the response body lists the columns "Backlog", "In-Progress", "Done"
    And the response body contains "New issue"
    And the project "Auth v2" is recorded in the "Backend" team with key prefix "AUTH"

  @error @real-io
  Scenario: Duplicate project key within the same workspace is rejected
    Given a project named "Auth v1" with key prefix "AUTH" already exists in "Backend"
    When Mei attempts to create a project under "Backend" with name "Other" and key prefix "AUTH"
    Then the response status is 409 Conflict
    And the response body explains the project key is already in use
    And no second project is created

  @error @real-io
  Scenario: Duplicate project name within the same team is rejected
    Given a project named "Auth v2" with key prefix "AUTH" already exists in "Backend"
    When Mei attempts to create a project under "Backend" with name "Auth v2" and key prefix "AV2"
    Then the response shows an inline error explaining the name must be unique within the team
    And no second project is created

  @error @real-io @nfr-sec-06
  Scenario: A workspace member who is not on the team cannot create a project there
    Given Hiroshi is a workspace member but not a member of the "Backend" team
    And Hiroshi is signed in
    When Hiroshi attempts to create a project under "Backend" with name "Sneaky" and key prefix "SNK"
    Then the response status is 404 Not Found
    And no project named "Sneaky" exists in any team

  # L1: a key of 2 to 10 capital letters A to Z is accepted, and nothing else is.
  @property @real-io
  Scenario Outline: A key of two to ten capital letters is accepted on the create form
    When Mei attempts to create a project under "Backend" with name "Probe" and key prefix "<key>"
    Then the project-create outcome is "accepted"

    Examples:
      | key        |
      | AU         |
      | AUTH       |
      | AUTHWS     |
      | PLATFORMOP |

  # L1, L5: every refused key gets the one sentence that states the whole rule,
  # and the create leaves nothing behind — not the typed key, and not a tidied
  # up version of it.
  @error @real-io
  Scenario Outline: A key outside two to ten capital letters is refused with the whole rule and creates nothing
    When Mei attempts to create a project under "Backend" with name "Probe" and key prefix "<key>"
    Then the refusal states that a project key must be 2 to 10 capital letters A to Z
    And no project was created from the key "<key>", in that form or tidied up

    Examples: longer than ten letters
      | key          |
      | PLATFORMOPS  |
      | PLATFORMOPSX |

    Examples: shorter than two letters
      | key |
      | A   |
      |     |

    Examples: a character that is not a capital letter A to Z
      | key         |
      | auth        |
      | platformop  |
      | AU1         |
      | AUTHWORD2   |
      | AUTH-X      |
      | PLATFORM OP |
      | PLATFORM_OP |
      | PLÄTFORMOP  |
      | 日本語ボ      |

  # L1, L4: the whole ten-letter key is what the operator sees afterwards — on
  # the board she lands on and in the project list she comes back to.
  @driving_port @real-io
  Scenario: A ten-letter key creates the project and is shown in full
    When Mei creates a project under "Backend" with name "Platform Operations" and key prefix "PLATFORMOP"
    Then the response redirects to "/team/backend/project/platform-operations"
    And the project "Platform Operations" is recorded in the "Backend" team with key prefix "PLATFORMOP"
    And opening the board of "Platform Operations" shows the project key "PLATFORMOP"
    And the project list on her landing page shows the project key "PLATFORMOP"

  # L1: the longer limit does not take away the short keys she could already use.
  @driving_port @real-io
  Scenario: A two-letter key is still accepted and shown in full
    When Mei creates a project under "Backend" with name "Payments" and key prefix "PL"
    Then the project "Payments" is recorded in the "Backend" team with key prefix "PL"
    And opening the board of "Payments" shows the project key "PL"
    And the project list on her landing page shows the project key "PL"

  # L4: the first issue carries the whole key.
  @real-io
  Scenario: The first issue of a ten-letter-key project carries the whole key
    Given Mei has created a project under "Backend" with name "Platform Operations" and key prefix "PLATFORMOP"
    When Mei files an issue against "Platform Operations" with title "Rotate the signing key"
    Then the new issue is assigned the key "PLATFORMOP-1"
    And opening the board of "Platform Operations" lists exactly the issues "PLATFORMOP-1"

  # L4: a later issue keeps the whole key and takes the next number, and the key
  # is still whole when she comes back to the board.
  @real-io
  Scenario: A later issue of a ten-letter-key project keeps the whole key
    Given Mei has created a project under "Backend" with name "Platform Operations" and key prefix "PLATFORMOP"
    And Mei has filed 9 issues against "Platform Operations"
    When Mei files a new issue against "Platform Operations" with title "Archive the old runbook"
    Then the new issue is assigned the key "PLATFORMOP-10"
    And opening the board of "Platform Operations" lists exactly the issues "PLATFORMOP-1 through PLATFORMOP-10"
    And opening issue 7 of "Platform Operations" shows the key "PLATFORMOP-7"

  # L6: a project that carries a key from the shorter rule keeps its key and
  # every issue key it already had.
  @edge @guard @real-io
  Scenario Outline: A project with a shorter key keeps its key and its issue keys
    Given a project "<project>" with key prefix "<key>" exists in the "Backend" team
    And the "<project>" project already has issues <key>-1 through <key>-3
    When Mei opens the board of "<project>"
    Then the board she opened shows the project key "<key>"
    And the board she opened lists exactly the issues "<key>-1 through <key>-3"
    And opening issue 2 of "<project>" shows the key "<key>-2"

    Examples:
      | project      | key    |
      | Auth v1      | AU     |
      | Auth Website | AUTHWS |

  # L6: the shorter-key project still takes new issues, under its own unchanged key.
  @edge @guard @real-io
  Scenario: A project with a shorter key still takes new issues under that key
    Given a project "Auth v1" with key prefix "AU" exists in the "Backend" team
    And the "Auth v1" project already has issues AU-1 through AU-3
    When Mei files a new issue against "Auth v1" with title "Retire the old gateway"
    Then the new issue is assigned the key "AU-4"
    And opening the board of "Auth v1" lists exactly the issues "AU-1 through AU-4"

  # L4, L6: a short-key project and a long-key project live side by side and
  # their issue keys never mix, including past the first nine numbers.
  @edge @guard @real-io
  Scenario: A short-key and a ten-letter-key project keep their own issue keys side by side
    Given a project "Auth v1" with key prefix "AU" exists in the "Backend" team
    And the "Auth v1" project already has issues AU-1 through AU-9
    And Mei has created a project under "Backend" with name "Platform Operations" and key prefix "PLATFORMOP"
    And Mei has filed 9 issues against "Platform Operations"
    When Mei files a new issue against "Platform Operations" with title "Rotate the signing key"
    Then opening the board of "Platform Operations" lists exactly the issues "PLATFORMOP-1 through PLATFORMOP-10"
    And opening the board of "Auth v1" lists exactly the issues "AU-1 through AU-9"
