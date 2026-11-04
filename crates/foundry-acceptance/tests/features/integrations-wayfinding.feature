# Feature: integrations-wayfinding — the integrations pages are findable, and
# every "no connection" refusal names its real cause and, for someone who can
# fix it, where.
#
# Found 2026-11-02 on dev (v0.18.1): the only user, a workspace admin, opened a
# new Slack route and read K-4 ("Ask a workspace admin…") with no link, while
# the real cause was that the instance has no Foundry Slack app.
#
# Copy W-1..W-8 is locked in docs/feature/integrations-wayfinding/discuss/
# user-stories.md; K-4 and K-19 keep their wording for the audience they were
# written for. The connection model is unchanged: one Slack and one Matrix
# connection per workspace, a channel per project.
#
# Harness: as eip-06, the production router in process (InProcHarness), real
# Postgres, Slack as the in-process double. Instance states reach the app
# through the eip `rewire` seam. A link is followed by reading its href off the
# page and opening that address; page links exclude the sidebar, which has its
# own steps. No Docker: every scenario runs in the default lane (DEVOPS D4).
#
# DISTILL 2026-11-03 scaffolded every scenario RED as @pending except the
# @control ones, which describe behaviour that is unchanged (K-4 for a lead who
# is not an admin; K-19 ahead of the new wording). DELIVER un-pended all of
# them (2026-11-03): every scenario runs.
#
# Not here, on purpose: a connected or unreadable Matrix connection (W-6 for
# Matrix, Matrix K-19). Both need the Synapse container (@needs-matrix), which
# DEVOPS D4 keeps out of this feature's lane. The pure cause table and the
# connection line's formatting cover them as unit tests in DELIVER.

@integrations-wayfinding @real-io
Feature: Integrations are reachable, and refusals say what is missing

  Background:
    Given the Canzan Labs workspace with these people
      | person       | workspace role | teams                         |
      | Priya Raman  | admin          | Backend as lead, Ops as lead  |
      | Lena Park    | member         | Backend as lead               |
      | Sam Okafor   | member         | Backend as member             |
      | Marco        | member         |                               |
    And Identity Platform (AUTH) belongs to Backend and Homelab Ops (OPS) belongs to Ops
    And it is 14:02 on 21 Oct 2026

  # ------------------------------------------------------------------ US-IW-01

  @us-iw-01 @driving_port @walking_skeleton @kpi-iw-3 @kpi-iw-g1
  Scenario Outline: A workspace admin reaches the workspace integrations from the sidebar
    When Priya follows "Integrations" in the sidebar on <page>
    Then Priya is on the workspace integrations page

    Examples:
      | page                                   |
      | Identity Platform's board              |
      | Identity Platform's integrations page  |
      | a new Slack route on Identity Platform |

  @us-iw-01 @kpi-iw-3
  Scenario Outline: Only workspace admins are offered the sidebar entry
    When <person> opens <page>
    Then <person> is <offered> "Integrations" in the sidebar

    Examples:
      | person | page                      | offered     |
      | Priya  | Identity Platform's board | offered     |
      | Priya  | the dashboard             | offered     |
      | Lena   | Identity Platform's board | not offered |
      | Lena   | the dashboard             | not offered |
      | Sam    | Identity Platform's board | not offered |
      | Sam    | the dashboard             | not offered |
      | Marco  | the dashboard             | not offered |

  # Marco is on no team, so no board opens for him; the dashboard is his page.
  # Priya is not an instance admin, so "Instance admin" (which W-1 precedes)
  # is absent; its placement after W-1 is the template's order.
  @us-iw-01
  Scenario: The sidebar entry sits in the user section and is never the current page
    When Priya opens the workspace integrations page
    Then Priya's sidebar lists Settings, Keyboard shortcuts and Integrations in that order
    And Home is the sidebar's current page, not Integrations

  # ------------------------------------------------------------------ US-IW-02

  @us-iw-02 @driving_port @kpi-iw-1 @kpi-iw-2 @kpi-iw-g1
  Scenario Outline: The new-route form names what is missing, and offers an admin the fix
    Given the instance has <instance>
    And Canzan Labs has no Slack or Matrix connection
    When <person> opens a new <kind> route for Identity Platform
    Then <person> reads "<words>"
    And <person> is offered <fix>

    Examples:
      | instance                                        | person | kind   | words                                                                                              | fix                                                  |
      | no Foundry Slack app set up                     | Priya  | Slack  | Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once.     | the link "How to set up Slack"                       |
      | no Foundry Slack app set up                     | Lena   | Slack  | Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once.     | no way to fix it here                                |
      | the Foundry Slack app set up but no secrets key | Priya  | Slack  | Slack cannot be connected on this Foundry yet: an instance operator needs to set the secrets key.  | no way to fix it here                                |
      | the Foundry Slack app set up but no secrets key | Lena   | Slack  | Slack cannot be connected on this Foundry yet: an instance operator needs to set the secrets key.  | no way to fix it here                                |
      | the Foundry Slack app set up                    | Priya  | Slack  | This workspace has no Slack connection yet.                                                        | the link "Connect Slack in workspace integrations"   |
      | no secrets key                                  | Priya  | Matrix | Matrix is not set up on this Foundry yet. An instance operator sets it up once.                    | no way to fix it here                                |
      | no secrets key                                  | Lena   | Matrix | Matrix is not set up on this Foundry yet. An instance operator sets it up once.                    | no way to fix it here                                |
      | the Foundry Slack app set up                    | Priya  | Matrix | This workspace has no Matrix connection yet.                                                       | the link "Connect Matrix in workspace integrations"  |

  @us-iw-02 @kpi-iw-1 @error
  Scenario Outline: A workspace admin is never told to ask a workspace admin, and is told only about the form's kind
    Given the instance has <instance>
    And Canzan Labs has no Slack or Matrix connection
    When Priya opens a new <kind> route for Identity Platform
    Then Priya does not read "Ask a workspace admin"
    And Priya does not read "Slack or Matrix"

    Examples:
      | instance                                        | kind   |
      | no Foundry Slack app set up                     | Slack  |
      | the Foundry Slack app set up but no secrets key | Slack  |
      | the Foundry Slack app set up                    | Slack  |
      | no secrets key                                  | Matrix |
      | the Foundry Slack app set up                    | Matrix |

  @us-iw-02 @control @kpi-iw-g2
  Scenario Outline: A team lead who is not an admin still reads K-4 when the workspace is not connected
    Given the instance has the Foundry Slack app set up
    And Canzan Labs has no Slack or Matrix connection
    When Lena opens a new <kind> route for Identity Platform
    Then Lena reads "This workspace has no chat connection yet. Ask a workspace admin to connect Slack or Matrix."
    And Lena is offered no way to fix it here

    Examples:
      | kind   |
      | Slack  |
      | Matrix |

  # Green at DISTILL: the form already answers K-19 here. A control that the
  # new cause function keeps K-19 ahead of W-4 (a replaced key) and of W-3 (no
  # key at all) (US-IW-02 AC 6).
  @us-iw-02 @kpi-iw-2 @error @control
  Scenario Outline: A saved Slack connection this Foundry cannot read is named before anything else it lacks
    Given the instance has the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Foundry has since <change>
    When Priya opens a new Slack route for Identity Platform
    Then Priya reads "Slack is unavailable: this Foundry cannot read its saved connection. An instance operator needs to check the secrets key."
    And Priya does not read "<not>"

    Examples:
      | change                                    | not                                                                                               |
      | been given a new secrets key without the old one | This workspace has no Slack connection yet.                                                |
      | lost its secrets key                      | Slack cannot be connected on this Foundry yet: an instance operator needs to set the secrets key. |

  @us-iw-02 @kpi-iw-2 @error
  Scenario: With no Foundry Slack app, the form says so even over an unreadable saved connection
    Given the instance has the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Foundry has since been given a new secrets key without the old one
    And the Foundry Slack app has since been taken off this instance
    When Priya opens a new Slack route for Identity Platform
    Then Priya reads "Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once."
    And Priya does not read "Slack is unavailable"

  @us-iw-02 @kpi-iw-1 @error
  Scenario Outline: Preview, save and Send test refuse with the same words as the form
    Given the instance has <instance>
    And Canzan Labs has no Slack or Matrix connection
    When <person> <door> a new <kind> route for Identity Platform
    Then <person> reads "<words>"
    And Identity Platform still has no route

    Examples:
      | instance                                        | person | door              | kind   | words                                                                                              |
      | no Foundry Slack app set up                     | Priya  | previews          | Slack  | Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once.     |
      | no Foundry Slack app set up                     | Priya  | saves             | Slack  | Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once.     |
      | no Foundry Slack app set up                     | Lena   | saves             | Slack  | Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once.     |
      | the Foundry Slack app set up but no secrets key | Priya  | previews          | Slack  | Slack cannot be connected on this Foundry yet: an instance operator needs to set the secrets key.  |
      | the Foundry Slack app set up but no secrets key | Priya  | saves             | Slack  | Slack cannot be connected on this Foundry yet: an instance operator needs to set the secrets key.  |
      | the Foundry Slack app set up                    | Priya  | previews          | Slack  | This workspace has no Slack connection yet.                                                        |
      | the Foundry Slack app set up                    | Priya  | saves             | Slack  | This workspace has no Slack connection yet.                                                        |
      | the Foundry Slack app set up                    | Priya  | sends a test from | Slack  | This workspace has no Slack connection yet.                                                        |
      | the Foundry Slack app set up                    | Lena   | saves             | Slack  | This workspace has no chat connection yet. Ask a workspace admin to connect Slack or Matrix.       |
      | no secrets key                                  | Priya  | previews          | Matrix | Matrix is not set up on this Foundry yet. An instance operator sets it up once.                    |
      | no secrets key                                  | Priya  | saves             | Matrix | Matrix is not set up on this Foundry yet. An instance operator sets it up once.                    |
      | the Foundry Slack app set up                    | Priya  | previews          | Matrix | This workspace has no Matrix connection yet.                                                       |
      | the Foundry Slack app set up                    | Priya  | saves             | Matrix | This workspace has no Matrix connection yet.                                                       |
      | the Foundry Slack app set up                    | Priya  | sends a test from | Matrix | This workspace has no Matrix connection yet.                                                       |

  # ------------------------------------------------------------------ US-IW-03

  @us-iw-03 @driving_port @kpi-iw-g1
  Scenario: The project page names the shared Slack connection, and an admin can manage it
    Given the instance has the Foundry Slack app set up
    And Priya has connected Canzan Labs to the Slack workspace "Canzan Personal"
    When Priya opens Identity Platform's integrations page
    Then Priya reads "Routes here use the workspace's Slack connection to Canzan Personal."
    And following "Manage workspace integrations" lands Priya on the workspace integrations page

  @us-iw-03
  Scenario: A team lead sees the shared Slack connection but is offered no workspace link
    Given the instance has the Foundry Slack app set up
    And Priya has connected Canzan Labs to the Slack workspace "Canzan Personal"
    When Lena opens Identity Platform's integrations page
    Then Lena reads "Routes here use the workspace's Slack connection to Canzan Personal."
    And Lena is not offered a "Manage workspace integrations" link
    And Lena is offered a "New route" link

  @us-iw-03 @kpi-iw-g1
  Scenario: An admin sees nothing is connected, and where to connect
    Given the instance has the Foundry Slack app set up
    And Canzan Labs has no Slack or Matrix connection
    When Priya opens Identity Platform's integrations page
    Then Priya reads "This workspace has no chat connection yet."
    And Priya does not read "Ask a workspace admin"
    And following "Connect Slack or Matrix in workspace integrations" lands Priya on the workspace integrations page
    And Priya is offered a "New route" link

  @us-iw-03 @error
  Scenario: A team lead sees nothing is connected and still asks an admin
    Given the instance has the Foundry Slack app set up
    And Canzan Labs has no Slack or Matrix connection
    When Lena opens Identity Platform's integrations page
    Then Lena reads "This workspace has no chat connection yet. Ask a workspace admin to connect Slack or Matrix."
    And Lena is offered no way to fix it here
    And Lena is offered a "New route" link

  @us-iw-03 @error
  Scenario: A lost Slack connection reads as lost, not as one routes use
    Given the instance has the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Slack has told Foundry the Foundry app was uninstalled from Canzan Labs
    When Lena opens Identity Platform's integrations page
    Then Lena reads "Foundry lost its Slack connection for Canzan Labs: the Foundry app was removed from Slack. Routes are paused."
    And Lena does not read "Routes here use the workspace's Slack connection"

  @us-iw-03 @error
  Scenario: An unreadable Slack connection reads as unavailable, not as one routes use
    Given the instance has the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Foundry has since been given a new secrets key without the old one
    When Lena opens Identity Platform's integrations page
    Then Lena reads "Slack is unavailable: this Foundry cannot read its saved connection. An instance operator needs to check the secrets key."
    And Lena does not read "Routes here use the workspace's Slack connection"

  # ------------------------------------------------------------------ no access

  # Green at DISTILL and after: those the pages do not answer get the uniform
  # not-found page in every connection state, so no new link can lead them
  # anywhere (KPI-IW-G1, DESIGN "For Acceptance Designer").
  @us-iw-02 @us-iw-03 @kpi-iw-g1 @error @control
  Scenario Outline: Someone who cannot manage the team is not shown the integrations pages, in any state
    Given the instance has <instance>
    When <person> <door> on Identity Platform
    Then <person> gets the same not-found page any unknown address gets

    Examples:
      | instance                     | person | door                       |
      | no Foundry Slack app set up  | Sam    | opens the integrations page |
      | no Foundry Slack app set up  | Sam    | opens a new Slack route     |
      | no Foundry Slack app set up  | Marco  | opens the integrations page |
      | the Foundry Slack app set up | Sam    | opens a new Slack route     |
      | the Foundry Slack app set up | Marco  | opens a new Slack route     |
