# Feature: external-integration-plugins — the integrations pages can be reached
# from the product, not only by typing their address.
#
# Added 2026-11-02 after v0.18.0: nothing in the UI linked /workspace/integrations
# or a project's integrations page; every earlier scenario opened them by URL.
# The links are offered only to the people each page answers (a workspace admin;
# a team lead or workspace admin), so no one is shown a link to a not-found page.
#
# Harness: as slice 01, the production router in process. A link is followed by
# reading its href off the page the person opened and opening that address.

@external-integration-plugins @eip-06 @real-io
Feature: The integrations pages are reachable from the dashboard and the board

  Background:
    Given the Canzan Labs workspace with these people
      | person       | workspace role | teams                         |
      | Priya Raman  | admin          | Backend as lead, Ops as lead  |
      | Lena Park    | member         | Backend as lead               |
      | Sam Okafor   | member         | Backend as member             |
      | Marco        | member         |                               |
    And Identity Platform (AUTH) belongs to Backend and Homelab Ops (OPS) belongs to Ops
    And it is 14:02 on 21 Oct 2026

  @driving_port
  Scenario: A workspace admin reaches the workspace integrations page from the dashboard
    When Priya follows "Integrations" on the dashboard
    Then Priya is on the workspace integrations page

  @driving_port
  Scenario: A team lead reaches the project's integrations page from its board
    When Lena follows "Integrations" on Identity Platform's board
    Then Lena is on Identity Platform's integrations page

  Scenario Outline: The dashboard offers Integrations only to a workspace admin
    When <person> opens the dashboard
    Then <person> is <offered> an "Integrations" link

    Examples:
      | person | offered     |
      | Priya  | offered     |
      | Lena   | not offered |
      | Sam    | not offered |
      | Marco  | not offered |

  Scenario Outline: A board offers Integrations only to those who manage its team
    When <person> opens Identity Platform's board
    Then <person> is <offered> an "Integrations" link

    Examples:
      | person | offered     |
      | Priya  | offered     |
      | Lena   | offered     |
      | Sam    | not offered |
