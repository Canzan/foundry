Feature: Shared Linear-style navigation sidebar
  As a signed-in Foundry workspace member
  I want one consistent left sidebar across every authenticated page
  So that I always know where I am and can reach any primary surface in one click

  Background:
    Given Devon Park is signed in to the "Acme" workspace

  # --- Presence on authenticated pages ---

  Scenario: The shared sidebar is present on the dashboard
    When Devon opens the dashboard at "/"
    Then a persistent left sidebar is shown
    And the sidebar shows the workspace name "Acme"
    And the sidebar shows primary navigation items "Home" and "Projects"
    And the sidebar footer shows the signed-in name "Devon Park"

  Scenario Outline: The shared sidebar is present on every authenticated app page
    When Devon opens "<page>"
    Then a persistent left sidebar is shown
    And the sidebar shows primary navigation items "Home" and "Projects"

    Examples:
      | page                                    |
      | /                                       |
      | /team/acme/project/web                  |
      | /team/acme/project/web/report           |
      | /admin/tokens                           |
      | /workspace/invites                      |

  # --- Absence on pre-auth / utility pages ---

  Scenario Outline: Pre-auth and utility pages do not show the sidebar
    Given a visitor is not signed in
    When the visitor opens "<page>"
    Then no navigation sidebar is shown
    And only the page's own content is visible

    Examples:
      | page                 |
      | /signin              |
      | /forgot              |

  # --- Active-state highlighting ---

  Scenario: Home is highlighted on the dashboard
    When Devon opens the dashboard at "/"
    Then the "Home" navigation item is marked as the current page
    And the "Projects" navigation item is not marked as current

  Scenario: Projects is highlighted while viewing a board
    When Devon views a project board under "/team/acme/project/web"
    Then the "Projects" navigation item is marked as the current page
    And the "Home" navigation item is not marked as current

  Scenario: Exactly one primary item is marked current at a time
    When Devon opens any authenticated app page
    Then exactly one primary navigation item is marked as the current page

  # --- Primary navigation ---

  Scenario: Home navigates to the dashboard
    Given Devon is viewing a project board under "/team/acme/project/web"
    When Devon clicks "Home" in the sidebar
    Then Devon is taken to the dashboard at "/"

  Scenario: Projects navigates to the projects surface
    Given Devon is viewing the dashboard at "/"
    When Devon clicks "Projects" in the sidebar
    Then Devon is taken to the projects surface

  # --- User menu / account actions ---

  Scenario: The user menu anchor shows workspace and signed-in identity
    When Devon opens the user menu in the sidebar footer
    Then the menu shows the workspace name "Acme"
    And the menu shows the signed-in name "Devon Park"

  Scenario: The user menu links to keyboard shortcuts
    When Devon opens the user menu in the sidebar footer
    Then the menu contains "Keyboard shortcuts" linking to "/keyboard-help"

  Scenario: Sign out posts with a CSRF token
    When Devon opens the user menu in the sidebar footer
    Then the menu contains a "Sign out" control
    And the "Sign out" control submits a POST to "/sign-out" with a CSRF token

  # --- Instance admin gating ---

  Scenario: Instance admins see the Instance admin item
    Given Devon Park is an instance administrator
    When Devon opens the user menu in the sidebar footer
    Then the menu contains "Instance admin" linking to "/admin/instance/workspaces"

  Scenario: Non-admins do not see the Instance admin item
    Given Devon Park is not an instance administrator
    When Devon opens the user menu in the sidebar footer
    Then the menu does not contain an "Instance admin" item

  # --- Scoping guard (Decision #5): dashboard quick actions preserved ---

  Scenario: Invites and machine tokens remain reachable from the dashboard
    When Devon opens the dashboard at "/"
    Then the dashboard "Quick actions" list still links to "/workspace/invites"
    And the dashboard "Quick actions" list still links to "/admin/tokens"

  # --- Accessibility properties ---

  @property
  Scenario: The sidebar is an accessible navigation landmark
    Given Devon is on any authenticated app page
    Then the sidebar is exposed as a navigation landmark
    And the current item carries an aria-current="page" marker
    And every navigation item is reachable and focusable by keyboard
