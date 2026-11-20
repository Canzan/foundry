# Feature: nav-quick-actions — the operator reaches Teams, Invite a member and
# Machine tokens from the left nav on every app page, instead of only from the
# signed-in landing page.
#
# Driving port: HTTP GET on the authenticated routes of the running application
# (`/`, `/team/{team}/project/{project}`, `/teams`, `/account/settings`) carrying
# the real session cookie. Driven adapter: real Postgres (testcontainers,
# @real-io) read through the shipped fail-closed rights lookups; no lookup, route
# or authorization is changed, so a destination still decides who may enter.
#
# The rights domain is the closed four-member set (a member with no extra rights,
# a workspace admin, an instance super-admin, both), so every law below is
# exhausted by its enumerated rows rather than by a generator. WHICH of the five
# destinations each row is owed is the rights law of ui-nav-polish; this lane
# reads the ORDER they are offered in and the glyph-and-word shape of each one.
#
# The FOLDED rail is not re-oracled here. The shipped nav-collapse-icon-rail lane
# enumerates whatever members the live document holds and names each one from its
# own word, so the enlarged set is covered by RE-RUNNING that lane with the
# browser included. The glyph-and-word shape of every member of the shipped
# partial, present and future, is falsified by the repository architecture gate.

@nav-quick-actions @real-io
Feature: An operator opens Teams, Invite a member and Machine tokens from the left nav
  Wherever she is in Foundry, the operator reaches her teams, the invite page and
  her machine tokens from the same left nav she uses for Home and her board — one
  press, no detour through the landing page. The landing page keeps its greeting
  and her projects. Nobody loses a destination her rights already open; the two
  admin destinations are offered to the operators those pages let in, which is
  the rights law ui-nav-polish owns.

  Background:
    Given a workspace "Acme" exists with admin "Ada" and display name "Ada Lovelace"
    And a project "Sandbox" with key prefix "GEN" exists in "Acme"
    And Ada is signed in

  # ── The five destinations, offered in order, on a page away from the landing ──

  @driving_port
  Scenario: A workspace admin opens all five destinations from her board
    When Ada opens the authenticated page "/team/general/project/sandbox"
    Then the left nav offers these destinations in this order
      | word            | opens                         |
      | Home            | /                             |
      | Board           | /team/general/project/sandbox |
      | Teams           | /teams                        |
      | Invite a member | /workspace/invites            |
      | Machine tokens  | /admin/tokens                 |
    And each of those destinations shows one icon and its own word
    And the user menu contains a link to "/workspace/integrations"
    And the user menu does not contain a link to "/admin/instance/workspaces"

  @driving_port
  Scenario: A member with no rights beyond her workspace opens the three places her rights open
    Given a member "Mei" who is not an instance admin is signed in
    And Mei is on the team that owns the board
    When Mei opens the authenticated page "/team/general/project/sandbox"
    Then the left nav offers these destinations in this order
      | word            | opens                         |
      | Home            | /                             |
      | Board           | /team/general/project/sandbox |
      | Teams           | /teams                        |
    And the user menu does not contain a link to "/workspace/integrations"
    And the user menu does not contain a link to "/admin/instance/workspaces"

  @driving_port
  Scenario: An instance super-admin who administers no workspace opens the same three places
    Given a member "Mei" who is not an instance admin is signed in
    And Mei is on the team that owns the board
    And Mei has since been made an instance super-admin
    When Mei opens the authenticated page "/team/general/project/sandbox"
    Then the left nav offers these destinations in this order
      | word            | opens                         |
      | Home            | /                             |
      | Board           | /team/general/project/sandbox |
      | Teams           | /teams                        |
    And the user menu does not contain a link to "/workspace/integrations"
    And the user menu contains a link to "/admin/instance/workspaces"

  @driving_port
  Scenario: An operator who administers both the workspace and the instance opens all five from her board
    Given Ada is an instance super-admin
    When Ada opens the authenticated page "/team/general/project/sandbox"
    Then the left nav offers these destinations in this order
      | word            | opens                         |
      | Home            | /                             |
      | Board           | /team/general/project/sandbox |
      | Teams           | /teams                        |
      | Invite a member | /workspace/invites            |
      | Machine tokens  | /admin/tokens                 |
    And the user menu contains a link to "/workspace/integrations"
    And the user menu contains a link to "/admin/instance/workspaces"

  # Before she has any project at all, Board falls back to the landing page and
  # the three destinations are still hers.
  @driving_port
  Scenario: The three destinations are offered before the operator has any project
    Given the "Acme" workspace has no projects
    When Ada visits "/"
    Then the left nav offers these destinations in this order
      | word            | opens              |
      | Home            | /                  |
      | Board           | /                  |
      | Teams           | /teams             |
      | Invite a member | /workspace/invites |
      | Machine tokens  | /admin/tokens      |

  # ── The landing page loses its quick actions and keeps everything else ───────

  @driving_port
  Scenario: The landing page shows no quick actions and still greets the operator with her projects
    When Ada visits "/"
    Then the landing page shows no quick actions
    And the landing page still greets "Ada Lovelace" and names the workspace "Acme"
    And the landing page still shows the projects section
    And the response body contains a project card "GEN" for "Sandbox"

  @driving_port
  Scenario: The landing page shows no quick actions while it invites the first project
    Given the "Acme" workspace has no projects
    When Ada visits "/"
    Then the landing page shows no quick actions
    And the landing page still greets "Ada Lovelace" and names the workspace "Acme"
    And the landing page still shows the projects section

  @driving_port
  Scenario: The landing page shows no quick actions for an operator on no team
    Given a member "Mei" who is not an instance admin is signed in
    When Mei visits "/"
    Then the landing page shows no quick actions
    And the landing page still greets "Mei" and names the workspace "Acme"
    And the landing page still shows the projects section

  # ── A page no nav destination owns is still the landing page's own mark ──────
  # The three moved destinations now mark their OWN pages (ui-nav-polish, P1), so
  # /teams is read there and not here. What stays here is the complement: a page
  # no primary destination owns keeps Home as the one marked item.

  @driving_port
  Scenario: A page no nav destination owns keeps Home as the one marked item
    When Ada opens the authenticated page "/account/settings"
    Then the "Home" navigation item is marked as the current page
    And exactly one primary navigation item is marked as the current page
