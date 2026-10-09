# Feature: fix-hide-unreachable-boards — never offer a board the member cannot open.
#
# RCA (user-approved): the dashboard "Your projects" list (signin.rs dashboard_root) and the rail
# Board link (nav.rs resolve_board_href) both read Store::list_projects_for_workspace, which has
# NO membership filter (WHERE p.workspace_id = $1). The board gate (resolve_board_project ->
# Store::is_team_member) is TEAM-scoped. Invite-accept and OIDC provisioning add ONLY a workspace
# membership, so such a member is shown Sandbox and a Board link to /team/general/project/sandbox
# that 404s. Real incident on dev: a Keycloak-provisioned second account saw Sandbox and got 404.
#
# The no-team member is seeded through the SHIPPED member-invite issue + accept flow (the real
# production path that omits team_memberships) — never a direct membership insert, which would
# hide the defect. Assertions are over observable HTML (hrefs, listed names, empty-state copy)
# and HTTP status only.
#
# The no-team scenario is @pending in step 01-01 (RED captured in the execution log); step 01-02
# lands the member-scoped query and removes the tag. The lead guard is live and must stay green.

@fix-hub @fix-hide-unreachable-boards @real-io @driving_adapter
Feature: Hide boards a member cannot open
  A workspace member is only offered the boards of the teams they belong to, so every project
  link and the rail Board link open; a member on no team sees a clear empty state instead.

  Background:
    Given Dana Reyes is signed in as an admin of the "Northwind" workspace
    And the "Northwind" workspace has a project "Sandbox" on team "general" led by Dana

  @us-01 @error
  Scenario: A workspace member on no team is not offered a board they cannot open
    When Dana invites "sam.okafor@northwind.example" to "Northwind"
    And Sam opens his invite link and sets a password meeting the strength policy
    Then Sam is a member of "Northwind" on no team
    When Sam opens his dashboard
    Then the dashboard renders 200 with a "Welcome back" greeting
    And the "Your projects" list does not link to "/team/general/project/sandbox"
    And the rail Board link does not point to "/team/general/project/sandbox"
    And the no-team empty state is shown

  @us-01 @guard
  Scenario: The team lead still sees the team's project and the Board link opens
    When Dana opens her dashboard
    Then the dashboard renders 200 with a "Welcome back" greeting
    And the "Your projects" list shows "Sandbox" linking to "/team/general/project/sandbox"
    And the rail Board link points to "/team/general/project/sandbox"
    And opening the rail Board link as Dana returns 200
