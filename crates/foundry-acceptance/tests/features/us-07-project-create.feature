# Story: US-07 — User creates and views a project under a team
# Slice: 1 (Walking Skeleton)
# JTBD: outcome-4 (Linear-feel hierarchy: teams own projects, projects own issues)
#
# Driving port: HTTP form POST to /team/{team_slug}/projects — see
# architecture.md and data-access.md § projects table.
# Driven adapters exercised: real Postgres (projects, teams,
# team_memberships, workspace_memberships).
# Invariant I-P3: project key prefix matches regex ^[A-Z]{2,6}$ (enforced
# by Postgres CHECK constraint + domain construction).

@slice1 @us-07 @driving_port
Feature: A team member creates a project and reaches its empty board
  A workspace member who belongs to a team can create a project under that
  team, immediately navigate to its board, and see the four default state
  columns. Project names are unique within a team; project keys are unique
  within a workspace and match the agreed shape.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And Mei is signed in

  @walking_skeleton @real-io
  Scenario: Member creates a project under their team and lands on its empty board
    When Mei creates a project under "Backend" with name "Auth v2" and key prefix "AUTH"
    Then the response redirects to "/team/backend/project/auth-v2"
    And the response body lists the columns "Backlog", "Todo", "In-Progress", "Done"
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
    Then the response status is 403 Forbidden
    And no project named "Sneaky" exists in any team

  @property @real-io
  Scenario Outline: Project key prefix must match the invariant I-P3 (^[A-Z]{2,6}$)
    When Mei attempts to create a project under "Backend" with name "Probe" and key prefix "<key>"
    Then the project-create outcome is "<outcome>"

    Examples: accepted
      | key    | outcome  |
      | AU     | accepted |
      | AUTH   | accepted |
      | AUTHWS | accepted |

    Examples: rejected
      | key       | outcome  |
      | A         | rejected |
      | AUTHWORD2 | rejected |
      | auth      | rejected |
      | AUTH-X    | rejected |
      |           | rejected |
