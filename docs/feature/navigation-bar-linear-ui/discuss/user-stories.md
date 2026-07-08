<!-- markdownlint-disable MD024 -->
# User Stories: navigation-bar-linear-ui

LeanUX stories. JTBD was skipped for this feature (per wave config), so each story ties to a **navigation outcome KPI** (see `outcome-kpis.md`) rather than a job_id.

## System Constraints

- Web tier is `crates/foundry-app`; server-rendered askama templates extending `base.html`. No SPA.
- The rail lives in the shared layout (`base.html`); styling goes in the single hashed CSS file `static/css/foundry.4c43c2a8.css` (which today has no nav styles).
- Sign-out MUST reuse the existing `POST /sign-out` endpoint with hidden `_csrf = {{ csrf }}`.
- Admin gating MUST reuse the existing `is_instance_admin` flag.
- Excluded (chrome-free) pages: signin, forgot, forgot_sent, bootstrap_*, invite_accept, invalid_page, payload_too_large, events_signin_required.
- Personas: **Devon Park** (workspace member on "Acme"), **Ariane Cole** (instance admin), **Sam Rivera** (new member, first week).

---

## US-01: Persistent sidebar with Home/Projects and active state (Walking Skeleton)

### Problem
Devon Park is a workspace member who bounces between the dashboard and project boards dozens of times a day. Today navigation is scattered per page — the board only offers "Change report" and "New issue", the dashboard has a "Quick actions" list — so Devon has no consistent way to tell which page they are on or jump back Home. They find it disorienting to hunt for the same links in different places on every screen.

### Who
- Workspace member | signed in to a workspace ("Acme") | wants fast, predictable movement between the dashboard and boards.

### Solution
Add a persistent left-sidebar rail to the shared layout, rendered on the dashboard and project board. The rail shows the workspace identity at the top and two primary items — **Home** (`/`) and **Projects** (board) — with the current section highlighted. Page content is offset to the right. The rail is absent on pre-auth pages.

### Domain Examples
#### 1: Happy Path — Devon opens the dashboard
Devon Park opens `/` on the "Acme" workspace. The rail shows "Acme" at the top and items "Home" (highlighted) and "Projects". Devon clicks "Projects" and lands on the "WEB" board with "Projects" now highlighted and "Home" not.

#### 2: Edge Case — Devon is already deep in a board
Devon is viewing `/team/acme/project/web` (the WEB board). The rail is present, "Projects" is highlighted. Devon clicks "Home" and returns to `/` with "Home" highlighted — no back-button hunting.

#### 3: Boundary — Signed-out visitor hits sign-in
A signed-out visitor opens `/signin`. No rail is rendered; only the sign-in form is shown. The pre-auth experience is unchanged.

### UAT Scenarios (BDD)
#### Scenario: The shared sidebar is present on the dashboard
Given Devon Park is signed in to the "Acme" workspace
When Devon opens the dashboard at "/"
Then a persistent left sidebar is shown with the workspace name "Acme"
And the sidebar shows primary navigation items "Home" and "Projects"

#### Scenario: Home is highlighted on the dashboard
Given Devon Park is signed in to the "Acme" workspace
When Devon opens the dashboard at "/"
Then the "Home" navigation item is marked as the current page
And the "Projects" navigation item is not marked as current

#### Scenario: Projects is highlighted while viewing a board
Given Devon Park is signed in to the "Acme" workspace
When Devon views a project board under "/team/acme/project/web"
Then the "Projects" navigation item is marked as the current page
And the "Home" navigation item is not marked as current

#### Scenario: Primary items navigate between surfaces
Given Devon is viewing a project board under "/team/acme/project/web"
When Devon clicks "Home" in the sidebar
Then Devon is taken to the dashboard at "/"

#### Scenario: Pre-auth pages do not show the sidebar
Given a visitor is not signed in
When the visitor opens the sign-in page at "/signin"
Then no navigation sidebar is shown
And only the sign-in content is visible

### Acceptance Criteria
- [ ] A persistent left sidebar renders on `/` and on project board pages.
- [ ] The sidebar shows the workspace name and the items "Home" and "Projects".
- [ ] Exactly one primary item is marked current, matching the route (Home for `/`, Projects for board).
- [ ] Clicking Home navigates to `/`; clicking Projects navigates to the projects surface.
- [ ] The sidebar is absent on `/signin` and `/forgot` (pre-auth pages unchanged).

### Outcome KPIs (link)
- KPI-1 (consistent nav presence), KPI-4 (active-state correctness).

### Technical Notes
- Depends on `active_section` being set per route (new value) and `workspace_name` in context on both pages.
- Board pages do not currently pass `workspace_name` — thread it in for this slice.
- "Projects" link target is a DESIGN open question (no projects-index route today); skeleton may ship with Home-active behavior if unresolved.

---

## US-02: Account actions in a pinned user menu (sign out + keyboard shortcuts)

### Problem
Devon Park can only sign out from the dashboard today — the sign-out form lives in the dashboard's "Quick actions". When Devon is deep in a board or an issue, signing out or opening keyboard help means navigating back Home first. They find it annoying to leave their current context just to reach an account action.

### Who
- Workspace member | on any authenticated page | wants account actions in one always-present place.

### Solution
Add a user/account menu pinned to the sidebar footer. Its anchor shows the workspace name and the signed-in display name. Opening it reveals "Keyboard shortcuts" (`/keyboard-help`) and "Sign out" (the existing CSRF-protected `POST /sign-out`).

### Domain Examples
#### 1: Happy Path — Devon signs out from a board
Devon Park, viewing the WEB board, opens the footer menu (showing "Devon Park / Acme") and clicks "Sign out". The form POSTs to `/sign-out` with the `_csrf` token and the session ends.

#### 2: Edge Case — Devon opens keyboard help mid-task
Devon opens the footer menu on an issue page and clicks "Keyboard shortcuts", landing on `/keyboard-help` without first going Home.

#### 3: Boundary — Missing CSRF token
If the sign-out form were rendered without `_csrf`, the POST would be rejected. The menu must always render the token from `{{ csrf }}`.

### UAT Scenarios (BDD)
#### Scenario: The user menu anchor shows workspace and identity
Given Devon Park is signed in to the "Acme" workspace
When Devon opens the user menu in the sidebar footer
Then the menu shows the workspace name "Acme"
And the menu shows the signed-in name "Devon Park"

#### Scenario: The user menu links to keyboard shortcuts
Given Devon Park is signed in to the "Acme" workspace
When Devon opens the user menu in the sidebar footer
Then the menu contains "Keyboard shortcuts" linking to "/keyboard-help"

#### Scenario: Sign out posts with a CSRF token
Given Devon Park is signed in to the "Acme" workspace
When Devon opens the user menu and activates "Sign out"
Then the control submits a POST to "/sign-out" with a CSRF token
And Devon's session ends

### Acceptance Criteria
- [ ] The footer anchor shows both `workspace_name` and `display_name`.
- [ ] The menu contains "Keyboard shortcuts" linking to `/keyboard-help`.
- [ ] The menu contains a "Sign out" control that POSTs to `/sign-out` with a hidden `_csrf` token.
- [ ] Sign out ends the session (same behavior as the existing dashboard form).

### Outcome KPIs (link)
- KPI-2 (account actions reachable in ≤1 click from any authed page).

### Technical Notes
- Depends on US-01 (footer anchor exists) and on `display_name` + `csrf` in context.
- Reuse the existing `/sign-out` endpoint and `_csrf` field; do not create a new endpoint.

---

## US-03: Instance admin entry visible only to instance admins

### Problem
Ariane Cole is an instance administrator who needs quick access to instance administration; ordinary members like Devon must never see that entry — if they did and clicked it, they would hit a 403. Today the admin link exists only in the dashboard, gated. Moving admin access into the always-present user menu must preserve that gating exactly.

### Who
- Instance administrator (Ariane Cole) needs the entry | ordinary member (Devon Park) must never see it.

### Solution
Add an "Instance admin" item to the user menu, linking to `/admin/instance/workspaces`, rendered only when `is_instance_admin` is true. For non-admins the item is absent from the rendered HTML, not merely hidden.

### Domain Examples
#### 1: Happy Path — Ariane sees the admin entry
Ariane Cole (instance admin) opens the user menu and sees "Instance admin" → `/admin/instance/workspaces`.

#### 2: Edge Case — Devon does not see it
Devon Park (not an admin) opens the user menu; there is no "Instance admin" item in the rendered HTML at all.

#### 3: Boundary — Admin flag flips
If Ariane's instance-admin grant is revoked, the next page render omits the item entirely.

### UAT Scenarios (BDD)
#### Scenario: Instance admins see the Instance admin item
Given Ariane Cole is signed in and is an instance administrator
When Ariane opens the user menu in the sidebar footer
Then the menu contains "Instance admin" linking to "/admin/instance/workspaces"

#### Scenario: Non-admins do not see the Instance admin item
Given Devon Park is signed in and is not an instance administrator
When Devon opens the user menu in the sidebar footer
Then the menu does not contain an "Instance admin" item

### Acceptance Criteria
- [ ] When `is_instance_admin` is true, the menu contains "Instance admin" → `/admin/instance/workspaces`.
- [ ] When `is_instance_admin` is false, the item is absent from the rendered HTML.
- [ ] Gating reuses the existing `is_instance_admin` flag (no new authorization logic).

### Outcome KPIs (link)
- KPI-3 (admin-visibility correctness — 100% of non-admin renders omit the item).

### Technical Notes
- Depends on US-02 (user menu exists) and `is_instance_admin` in context.

---

## US-04: Same sidebar on every authenticated page

### Problem
Sam Rivera is a new member in their first week. On the dashboard they see the new rail, but on issue detail, reports, token pages, and invite pages the rail context may be missing — so the identity block or active state could render blank or wrong. Sam finds inconsistent chrome confusing and loses the orientation the rail is meant to provide.

### Who
- New workspace member (Sam Rivera) | navigating across all authed surfaces | expects the same nav everywhere.

### Solution
Thread the shared context (`display_name`, `workspace_name`, `is_instance_admin`, `csrf`, `active_section`) into every authenticated page handler so the rail renders correctly and consistently on issue detail, report, token pages, and invite pages — matching the dashboard and board.

### Domain Examples
#### 1: Happy Path — Sam opens an issue
Sam Rivera opens an issue detail page. The rail is present with "Acme" identity, "Projects" highlighted, and the footer showing "Sam Rivera".

#### 2: Edge Case — Sam opens the token list
Sam opens `/admin/tokens`. The rail is present and consistent; the footer menu and identity render correctly.

#### 3: Boundary — Invite page
Sam opens `/workspace/invites`. The rail renders identically to the dashboard, with correct identity and active state.

### UAT Scenarios (BDD)
#### Scenario: The sidebar renders on issue detail
Given Sam Rivera is signed in to the "Acme" workspace
When Sam opens an issue detail page
Then a persistent left sidebar is shown with the workspace name "Acme"
And the footer shows the signed-in name "Sam Rivera"

#### Scenario: The sidebar renders on token and invite pages
Given Sam Rivera is signed in to the "Acme" workspace
When Sam opens "/admin/tokens"
Then a persistent left sidebar is shown with items "Home" and "Projects"

#### Scenario: Active state is correct across authed pages
Given Sam Rivera is signed in to the "Acme" workspace
When Sam opens any authenticated app page
Then exactly one primary navigation item is marked as the current page

### Acceptance Criteria
- [ ] The rail renders on issue detail, report, token pages, and invite pages.
- [ ] `display_name`, `workspace_name`, `is_instance_admin`, `csrf`, `active_section` are present in each of those page contexts.
- [ ] Exactly one primary item is marked current on every authed page.

### Outcome KPIs (link)
- KPI-1 (consistent nav present on 100% of authed pages; 0% of pre-auth pages).

### Technical Notes
- Depends on US-01, US-02, US-03. Highest template fan-out; primary integration risk (see shared-artifacts-registry).

---

## US-05: Preserve dashboard access to Invites and Machine tokens (scoping guard)

### Problem
When consolidating navigation, there is a real risk that DELIVER removes the dashboard "Quick actions" links to Invites and Machine tokens, assuming they moved into the rail. But Decision #5 deliberately did NOT promote them. Devon Park still needs to reach `/workspace/invites` and `/admin/tokens` — deleting those links would orphan the features.

### Who
- Workspace member | needs Invites and Machine tokens | reaches them via the dashboard Quick actions.

### Solution
Explicitly preserve the dashboard's existing "Quick actions" links to `/workspace/invites` and `/admin/tokens`. These are intentionally NOT in the global nav or user menu. Encode this as a regression-guarding acceptance scenario.

### Domain Examples
#### 1: Happy Path — Devon invites a member
Devon opens `/`, finds "Invite a member" in Quick actions, and reaches `/workspace/invites` — exactly as before the nav change.

#### 2: Edge Case — Devon manages machine tokens
Devon opens `/` and reaches `/admin/tokens` via the "Machine tokens" Quick action.

#### 3: Boundary — Rail does not duplicate these
Neither "Invite a member" nor "Machine tokens" appears in the sidebar or user menu (no duplication, no promotion).

### UAT Scenarios (BDD)
#### Scenario: Invites and machine tokens remain reachable from the dashboard
Given Devon Park is signed in to the "Acme" workspace
When Devon opens the dashboard at "/"
Then the "Quick actions" list still links to "/workspace/invites"
And the "Quick actions" list still links to "/admin/tokens"

#### Scenario: These items are not promoted into the global nav
Given Devon Park is signed in to the "Acme" workspace
When Devon opens any authenticated app page
Then the sidebar does not contain an "Invite a member" item
And the sidebar does not contain a "Machine tokens" item

### Acceptance Criteria
- [ ] The dashboard Quick actions still link to `/workspace/invites` and `/admin/tokens`.
- [ ] Neither link is added to the sidebar or user menu.
- [ ] No dashboard Quick action link is deleted by the navigation change.

### Outcome KPIs (link)
- KPI-3 (nothing orphaned — 100% of pre-existing dashboard destinations remain reachable).

### Technical Notes
- Depends on US-01. Pure regression protection for Decision #5; effort is low.

---

## US-06: Linear-quality visual and accessibility polish

### Problem
A rail that works but looks like a plain list of links will not deliver the "looks like Linear" intent, and an inaccessible rail excludes keyboard users. Devon Park should feel the rail is a calm, quiet surface with an unmistakable active state; keyboard-only users must be able to operate it.

### Who
- All workspace members, including keyboard-only users | on any authenticated page | expect a polished, accessible nav.

### Solution
Style the rail to the Linear-style spec: ~220–260px quiet neutral surface with a subtle right border; compact icon+label items with rounded hover backgrounds; a clear accent-tinted active state with higher-contrast label; account block pinned to the footer. Make it a semantic `<nav>` landmark with `aria-current="page"` on the active item and visible keyboard focus states.

### Domain Examples
#### 1: Happy Path — Devon perceives the active section instantly
Devon glances at the rail on the WEB board; "Projects" is clearly tinted and higher-contrast, unmistakable at a glance.

#### 2: Edge Case — Keyboard-only navigation
A keyboard-only user tabs into the rail; each item shows a visible focus ring and is activatable with Enter.

#### 3: Boundary — Screen reader on the active item
A screen reader announces the current item as the current page via `aria-current="page"`.

### UAT Scenarios (BDD)
#### Scenario: The sidebar is an accessible navigation landmark
Given Devon Park is on any authenticated app page
Then the sidebar is exposed as a navigation landmark
And the current item carries an aria-current="page" marker

#### Scenario: The sidebar is keyboard operable
Given Devon Park is on any authenticated app page
When Devon navigates the sidebar using only the keyboard
Then every navigation item and the user menu are focusable with visible focus states
And each item can be activated by keyboard

#### Scenario: The active item is visually distinct
Given Devon Park is viewing a project board
Then the "Projects" item has an accent-tinted background and higher-contrast label
And idle items have a quiet, transparent background

### Acceptance Criteria
- [ ] The rail is ~220–260px, a quiet neutral surface with a subtle right border.
- [ ] The active item is accent-tinted with a higher-contrast label and `aria-current="page"`.
- [ ] The rail is a semantic `<nav>` landmark.
- [ ] All items and the user menu are keyboard-focusable with visible focus states and keyboard-activatable.

### Outcome KPIs (link)
- KPI-4 (active-state correctness), KPI-5 (accessibility checks pass).

### Technical Notes
- Depends on US-01. Styling added to `static/css/foundry.4c43c2a8.css` (new nav classes). Regenerate the hashed filename per the build's CSS hashing convention.
