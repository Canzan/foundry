# Acceptance Criteria (Given/When/Then) — dashboard-enhancements

These scenarios seed the DISTILL acceptance suite. Ports: the browser (signed-in session) for behaviour,
the store (integration) for query scoping. All "signed-in" steps assume a claimed workspace with a member
seeded (the harness's existing sign-in support).

---

## US-01 — Personalized greeting

```gherkin
Scenario: The dashboard greets the signed-in user by name and names the workspace
  Given a workspace "Acme" with an admin "Ada" (display name "Ada Lovelace")
  And I am signed in as "Ada"
  When I visit "/"
  Then I see "Ada Lovelace"
  And I see "Acme"
  And I see the heading "Foundry"

Scenario: A display name containing markup is rendered inert
  Given a user whose display name is "<b>x</b>"
  And I am signed in as that user
  When I visit "/"
  Then the response body contains the escaped text, not a live "<b>" element

Scenario: The greeting degrades gracefully if identity cannot be loaded
  Given the identity query fails
  When I visit "/" while signed in
  Then the page still renders with a neutral greeting
  And the response status is 200 (not 500)
```

## US-02 — Sign out

```gherkin
Scenario: A signed-in user signs out from the dashboard
  Given I am signed in
  When I visit "/"
  Then I see a "Sign out" control
  And the sign-out form carries a "_csrf" token matching the "foundry_csrf" cookie
  When I submit the sign-out form
  Then I am redirected to "/sign-in"
  And visiting "/" now redirects me to "/sign-in"

Scenario: Sign-out with a forged CSRF token is refused
  Given I am signed in
  When I POST "/sign-out" with a "_csrf" that does not match the cookie
  Then the request is refused by CSRF middleware
  And my session is still active
```

## US-03 — Instance-admin link (super-admin only)

```gherkin
Scenario: A super-admin sees the instance-admin link
  Given I am signed in as an instance super-admin
  When I visit "/"
  Then I see an "Instance admin" link to "/admin/instance/workspaces"

Scenario: A non-super-admin never sees the instance-admin link
  Given I am signed in as a workspace member (not an instance admin)
  When I visit "/"
  Then the response body does NOT contain a link to "/admin/instance/workspaces"
```

## US-04 — Styles promoted to the stylesheet (behaviour-preserving)

```gherkin
Scenario: Dashboard styles are served from the vendored stylesheet, not inline
  Given the app is built
  When I fetch the dashboard HTML
  Then it contains no inline "<style>" block
  And "base.html" links "/static/css/foundry.<newhash>.css"
  When I fetch "/static/css/foundry.<newhash>.css"
  Then the response is 200 and contains the ".dash" rules
```

## US-05 — Coverage (store integration + acceptance)

```gherkin
Scenario: list_projects_for_workspace is workspace-isolated and ordered
  Given workspace A with projects "Zebra" and "Alpha"
  And workspace B with a project "Foreign"
  When I list projects for workspace A
  Then I get ["Alpha", "Zebra"] in name order
  And "Foreign" is not in the result

Scenario: list_projects_for_workspace is empty for a project-less workspace
  Given a workspace with no projects
  When I list its projects
  Then the result is empty

Scenario: The signed-in dashboard lists projects and links to a board (acceptance)
  Given a signed-in user in a workspace with a project "Sandbox" (key "GEN")
  When I visit "/"
  Then I see a project card "GEN" / "Sandbox"
  And its link targets the project board
```
