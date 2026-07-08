# Acceptance Criteria: navigation-bar-linear-ui

All criteria are Given/When/Then and testable. These are the ATDD foundation for the DISTILL wave and align 1:1 with `journey-navigation.feature`. IDs map to user stories in `user-stories.md`.

---

## AC-01 — Sidebar present on authenticated pages (US-01, US-04)

### AC-01.1 Sidebar on the dashboard
Given Devon Park is signed in to the "Acme" workspace
When Devon opens the dashboard at "/"
Then a persistent left sidebar is shown with the workspace name "Acme"
And the sidebar shows primary navigation items "Home" and "Projects"
And the sidebar footer shows the signed-in name "Devon Park"

### AC-01.2 Sidebar on every authenticated app page
Given Devon Park is signed in to the "Acme" workspace
When Devon opens any of "/", "/team/acme/project/web", "/team/acme/project/web/report", "/admin/tokens", "/workspace/invites"
Then a persistent left sidebar is shown
And the sidebar shows primary navigation items "Home" and "Projects"

---

## AC-02 — Sidebar absent on pre-auth / utility pages (US-01)

### AC-02.1 Pre-auth pages are chrome-free
Given a visitor is not signed in
When the visitor opens "/signin" or "/forgot"
Then no navigation sidebar is shown
And only the page's own content is visible

### AC-02.2 Excluded pages are unchanged
Given the navigation feature is deployed
When any of signin, forgot, forgot_sent, bootstrap_dashboard, bootstrap_claim, bootstrap_invite, invite_accept, invalid_page, payload_too_large, events_signin_required is rendered
Then the rendered output contains no sidebar markup

---

## AC-03 — Active-state highlighting (US-01, US-06)

### AC-03.1 Home active on the dashboard
Given Devon Park is signed in to the "Acme" workspace
When Devon opens the dashboard at "/"
Then the "Home" navigation item is marked as the current page (aria-current="page")
And the "Projects" navigation item is not marked as current

### AC-03.2 Projects active on a board
Given Devon Park is signed in to the "Acme" workspace
When Devon views a project board under "/team/acme/project/web"
Then the "Projects" navigation item is marked as the current page
And the "Home" navigation item is not marked as current

### AC-03.3 Exactly one active item
Given Devon Park is signed in to the "Acme" workspace
When Devon opens any authenticated app page
Then exactly one primary navigation item is marked as the current page

---

## AC-04 — Primary navigation behavior (US-01)

### AC-04.1 Home navigates to the dashboard
Given Devon is viewing a project board under "/team/acme/project/web"
When Devon clicks "Home" in the sidebar
Then Devon is taken to the dashboard at "/"

### AC-04.2 Projects navigates to the projects surface
Given Devon is viewing the dashboard at "/"
When Devon clicks "Projects" in the sidebar
Then Devon is taken to the projects surface

---

## AC-05 — User menu / account actions (US-02)

### AC-05.1 Anchor shows workspace and identity
Given Devon Park is signed in to the "Acme" workspace
When Devon opens the user menu in the sidebar footer
Then the menu shows the workspace name "Acme"
And the menu shows the signed-in name "Devon Park"

### AC-05.2 Keyboard shortcuts link
Given Devon Park is signed in to the "Acme" workspace
When Devon opens the user menu in the sidebar footer
Then the menu contains "Keyboard shortcuts" linking to "/keyboard-help"

### AC-05.3 CSRF-safe sign out
Given Devon Park is signed in to the "Acme" workspace
When Devon opens the user menu and activates "Sign out"
Then the control submits a POST to "/sign-out" with a CSRF token
And Devon's session ends

---

## AC-06 — Instance admin gating (US-03)

### AC-06.1 Admins see the item
Given Ariane Cole is signed in and is an instance administrator
When Ariane opens the user menu in the sidebar footer
Then the menu contains "Instance admin" linking to "/admin/instance/workspaces"

### AC-06.2 Non-admins do not see the item
Given Devon Park is signed in and is not an instance administrator
When Devon opens the user menu in the sidebar footer
Then the menu does not contain an "Instance admin" item
And the item is absent from the rendered HTML

---

## AC-07 — Scoping guard: dashboard Quick actions preserved (US-05, Decision #5)

### AC-07.1 Invites and tokens remain reachable from the dashboard
Given Devon Park is signed in to the "Acme" workspace
When Devon opens the dashboard at "/"
Then the "Quick actions" list still links to "/workspace/invites"
And the "Quick actions" list still links to "/admin/tokens"

### AC-07.2 These items are not promoted into the nav
Given Devon Park is signed in to the "Acme" workspace
When Devon opens any authenticated app page
Then the sidebar does not contain an "Invite a member" item
And the sidebar does not contain a "Machine tokens" item

---

## AC-08 — Accessibility (US-06)

### AC-08.1 Navigation landmark and aria-current
Given Devon Park is on any authenticated app page
Then the sidebar is exposed as a navigation landmark (semantic <nav>)
And the current item carries an aria-current="page" marker

### AC-08.2 Keyboard operability
Given Devon Park is on any authenticated app page
When Devon navigates the sidebar using only the keyboard
Then every navigation item and the user menu are focusable with visible focus states
And each item can be activated by keyboard

---

## AC-09 — Visual quality ("looks like Linear") (US-06)

### AC-09.1 Rail surface and active state
Given Devon Park is viewing a project board
Then the sidebar rail is ~220–260px wide with a quiet neutral surface and a subtle right border
And the active "Projects" item has an accent-tinted background and a higher-contrast label
And idle items have a quiet, transparent background

### AC-09.2 Content offset
Given Devon Park is on any authenticated app page
Then the page content region is offset to the right of the rail and is not overlapped by it

---

## Traceability

| AC | Story | Feature scenario | KPI |
|----|-------|------------------|-----|
| AC-01 | US-01, US-04 | "sidebar present…" | KPI-1 |
| AC-02 | US-01 | "Pre-auth pages do not show the sidebar" | KPI-1 |
| AC-03 | US-01, US-06 | "…is highlighted…" / "Exactly one primary item…" | KPI-4 |
| AC-04 | US-01 | "Home navigates…" / "Projects navigates…" | KPI-1 |
| AC-05 | US-02 | "user menu…" / "Sign out posts with a CSRF token" | KPI-2 |
| AC-06 | US-03 | "Instance admins see…" / "Non-admins do not see…" | KPI-3 |
| AC-07 | US-05 | "Invites and machine tokens remain reachable…" | KPI-3 |
| AC-08 | US-06 | "@property accessible navigation landmark" | KPI-5 |
| AC-09 | US-06 | (visual, verified via component/visual test) | KPI-4 |
