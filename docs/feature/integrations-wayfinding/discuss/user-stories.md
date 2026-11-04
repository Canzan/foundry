# User stories — integrations-wayfinding

Job: `job-route-project-events-to-chat` (docs/product/jobs.yaml). This feature
changes no capability. It makes the existing one findable, and makes every
refusal on the way name its real cause and the next step: the journey's own
transition rule ("Every refusal on the way names the next step",
journey-route-project-events-to-chat), which K-4 breaks today for an admin.

Found 2026-11-02 on dev (v0.18.1). The only user, a workspace admin, opened
`/team/team-2/project/abc/integrations/routes/new?kind=slack` and read K-4,
"This workspace has no chat connection yet. Ask a workspace admin to connect
Slack or Matrix." They were the admin. No link led anywhere, and the real
cause was that dev has no Slack app configured (instance level), which K-4
does not mention.

## System Constraints

- **The connection model does not change.** One Slack connection and one Matrix
  connection per Foundry workspace (`integration_connections.workspace_id`).
  Each project chooses its own channel or room through its routes. The user
  confirmed this model on 2026-11-03.
- **Visibility follows access** (as eip-06, v0.18.1). A link is offered only to
  someone the target page answers. Otherwise they would follow it to the
  uniform not-found page.
  - `/workspace/integrations` and `/workspace/integrations/slack/setup`:
    workspace admins.
  - `/team/{t}/project/{p}/integrations`: team leads and workspace admins
    (`TeamAccess::can_manage`).
- **Locked copy stays.** Existing locked copy (D-13: K-4, K-19, the workspace
  page's "not set up" lines) keeps its wording for the audience it was written
  for. New wording is added as W-n below and locked the same way.
- **No new telemetry.** The instance has no product analytics. KPIs are
  measured by acceptance scenarios and a manual walk on dev.
- **The causes of "no usable connection" must stay distinguishable, worded
  as the workspace page already words them** (`workspace.rs`, `IntegrationSettings`):
  - Slack, in this order of precedence:
    1. No Slack app on the instance (`not_set_up`).
    2. A saved connection this Foundry cannot read (K-19).
    3. No secrets key (`no_secrets_key`).
    4. The workspace has not connected Slack.
  - Matrix has no instance app. Its only instance-level cause is a missing
    secrets key (`MatrixOffer::NoSecretsKey`), which the workspace page words
    as "not set up" (`matrix_not_set_up`). So:
    1. A saved connection this Foundry cannot read (K-19).
    2. No secrets key.
    3. The workspace has not connected Matrix.
  - That gives 5 new cells: Slack × 3 and Matrix × 2. K-19 already exists and
    keeps its wording; its place in the order is AC 6 (refined in DESIGN,
    accepted 2026-11-03).

## Impacted Journeys

- `journey-route-project-events-to-chat` (SSOT
  docs/product/journeys/journey-external-integration-plugins.yaml):
  - Step 1 "Connect Slack" gains an entry point (the sidebar).
  - Step 2 "Choose a destination" changes the "no connection" failure mode
    into W-2 to W-5.
  - The project page gains a connection line (W-6 to W-8).

## Copy (locked once approved)

| ID | Shown to | Where | Words |
|----|----------|-------|-------|
| W-1 | workspace admin | sidebar | `Integrations` (link to `/workspace/integrations`) |
| W-2 | lead and admin | route form: Slack with no Slack app, or Matrix with no secrets key | The workspace page's words. Slack: `Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once.` For an admin, followed by the link `How to set up Slack` (`/workspace/integrations/slack/setup`). Matrix: `Matrix is not set up on this Foundry yet. An instance operator sets it up once.` |
| W-3 | lead and admin | route form: Slack app present, no secrets key | `Slack cannot be connected on this Foundry yet: an instance operator needs to set the secrets key.` (the workspace page's words; no link, since the fix is outside Foundry) |
| W-4 | workspace admin | route form, workspace not connected | `This workspace has no {Kind} connection yet.` followed by the link `Connect {Kind} in workspace integrations` (`/workspace/integrations`) |
| W-5 | lead, not admin | route form, workspace not connected | K-4, unchanged |
| W-6 | lead and admin | project integrations page, connected | `Routes here use the workspace's {Kind} connection to {connection label}.` One line per connected kind. Admin: followed by `Manage workspace integrations` (`/workspace/integrations`). |
| W-7 | workspace admin | project integrations page, nothing connected | `This workspace has no chat connection yet.` followed by `Connect Slack or Matrix in workspace integrations` (`/workspace/integrations`) |
| W-8 | lead, not admin | project integrations page, nothing connected | K-4, unchanged |

`{Kind}` is `Slack` or `Matrix`. `{connection label}` is the connection's
existing label, the same one the workspace page shows (Slack: the Slack
workspace name; Matrix: the homeserver).

---

## US-IW-01 — Integrations in the sidebar for workspace admins

job_id: job-route-project-events-to-chat

**As** a workspace admin, **I want** Integrations in the sidebar on every page,
**so that** I can get to the workspace's Slack and Matrix connections from
wherever I notice I need them, not only from the dashboard.

### Elevator Pitch
- **Before:** the only ways to workspace integrations are the dashboard's Quick
  actions and typing the address.
- **After:** a workspace admin chooses Integrations in the sidebar on any page
  and lands on `/workspace/integrations`.
- **Decision enabled:** whether to connect Slack or Matrix now, decided from
  the page they were already on.

Estimate: 0.5 day. The sidebar partial, plus the workspace-admin fact added to
the navigation context (`NavContext` carries only `is_instance_admin` today,
`nav.rs:29`).

Acceptance criteria:
1. A workspace admin sees W-1 in the sidebar's user section, after "Keyboard
   shortcuts" and before "Instance admin". It links to `/workspace/integrations`.
2. A team lead who is not a workspace admin, a member, and someone on no team
   do not see W-1.
3. W-1 is not marked as the current page. Home stays current on the
   workspace integrations pages, as on every page reached from the user
   section. (Changed in DESIGN, accepted 2026-11-03.)
4. Following W-1 lands on the workspace integrations page (200), from the
   board, a project integrations page and the route form.

```gherkin
Scenario: A workspace admin reaches the workspace integrations from the sidebar
  Given Priya is a workspace admin
  When Priya follows "Integrations" in the sidebar on Identity Platform's new-route form
  Then Priya is on the workspace integrations page

Scenario Outline: Only workspace admins are offered the sidebar entry
  When <person> opens Identity Platform's board
  Then <person> is <offered> "Integrations" in the sidebar
  Examples:
    | person | offered     |
    | Priya  | offered     |
    | Lena   | not offered |
    | Sam    | not offered |
```

## US-IW-02 — The route form says what is actually missing

job_id: job-route-project-events-to-chat

**As** a team lead or workspace admin opening a new Slack or Matrix route
without a usable connection, **I want** to read why (this Foundry, its
secrets key, or this workspace) and, if I can fix it, a link to where,
**so that** I am not told to ask an admin when I am the admin, or to connect
something that cannot be connected yet.

### Elevator Pitch
- **Before:** every case reads K-4, "Ask a workspace admin to connect Slack or
  Matrix", even for the admin, and even when the server itself lacks the Slack
  app.
- **After:** the form names the real cause (this Foundry, its secrets key, or
  this workspace), and gives an admin a link to the page that fixes it.
- **Decision enabled:** fix it now (connect, or follow the setup steps), or
  hand it to the instance operator, without guessing.

Estimate: 1 day. The route form's no-connection branch, the preview and save
refusals, and an outline over 5 cells × 2 roles.

Acceptance criteria:
1. When the instance has no Slack app, the Slack route form shows W-2 for
   Slack instead of K-4. An admin also gets "How to set up Slack". A lead does
   not get the link.
2. When the secrets key is missing, the Matrix route form shows W-2 for Matrix,
   as the workspace page does, with no link (no Matrix setup page exists).
3. When the instance has a Slack app but no secrets key, the Slack route form
   shows W-3, with no link.
4. When the kind is set up and usable but the workspace has not connected it,
   an admin sees W-4 and its link, and a lead who is not an admin sees K-4
   (W-5).
5. W-2 to W-4 name only the form's kind. K-4, shown to a lead who is not an
   admin, keeps its locked wording. (Changed in DESIGN, accepted 2026-11-03.)
6. When a connection exists, K-19 takes precedence over W-3 and W-4, and over
   Matrix's W-2. Slack's W-2 (no Slack app) takes precedence over K-19, as on
   the workspace page. (Changed in DESIGN, accepted 2026-11-03.)
7. The preview and save posts (`.../routes/preview`, `.../routes`) refuse with
   the same words as the form for the same state.

```gherkin
Scenario: The admin is told the Slack app is missing, and where to set it up
  Given this Foundry has no Slack app
  When Priya opens a new Slack route for Identity Platform
  Then Priya reads "Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once."
  And Priya is offered a "How to set up Slack" link

Scenario: A workspace admin is sent to connect, not to ask an admin
  Given this Foundry has a Slack app and Canzan Labs has no Slack connection
  When Priya opens a new Slack route for Identity Platform
  Then Priya reads "This workspace has no Slack connection yet."
  And following "Connect Slack in workspace integrations" lands on the workspace integrations page

Scenario: The secrets key is missing for Slack
  Given this Foundry has a Slack app but no secrets key
  When Priya opens a new Slack route for Identity Platform
  Then Priya reads "Slack cannot be connected on this Foundry yet: an instance operator needs to set the secrets key."

Scenario: A team lead still asks a workspace admin
  Given this Foundry has a Slack app and Canzan Labs has no Slack connection
  When Lena opens a new Slack route for Identity Platform
  Then Lena reads "This workspace has no chat connection yet. Ask a workspace admin to connect Slack or Matrix."
  And Lena is not offered a link to the workspace integrations page
```

## US-IW-03 — The project page shows which connection its routes use

job_id: job-route-project-events-to-chat

**As** a team lead or workspace admin on a project's integrations page,
**I want** to see that routes use the workspace's existing Slack or Matrix
connection, **so that** I know I only pick a channel per project and never
connect again. If nothing is connected, I want to see that before choosing
New route.

### Elevator Pitch
- **Before:** the page lists routes and offers New route. It never says that
  the workspace's connection is reused, so it looks as if each project must be
  configured separately.
- **After:** the page says "Routes here use the workspace's Slack connection to
  Canzan Labs." (or that nothing is connected), with a link for admins.
- **Decision enabled:** pick a channel for this project now, or connect the
  workspace first.

Estimate: 0.5 day. The project page view and template, reading the existing
workspace connections.

Acceptance criteria:
1. With Slack connected, the page shows W-6 for Slack with the Slack workspace
   name, and likewise W-6 for Matrix with the homeserver when Matrix is
   connected.
2. A workspace admin also gets "Manage workspace integrations". A lead who is
   not an admin does not.
3. With nothing connected, an admin sees W-7 with its link, and a lead sees K-4
   (W-8). "New route" stays, and the form then explains per kind (US-IW-02).
4. A connection that is lost, or unreadable (K-19), shows its state the way the
   workspace page words it, not W-6.

```gherkin
Scenario: The project page names the shared Slack connection
  Given Canzan Labs is connected to the Slack workspace "Canzan Labs"
  When Lena opens Identity Platform's integrations page
  Then Lena reads "Routes here use the workspace's Slack connection to Canzan Labs."
  And Lena is not offered a "Manage workspace integrations" link

Scenario: An admin sees nothing is connected and where to connect
  Given Canzan Labs has no chat connection
  When Priya opens Identity Platform's integrations page
  Then Priya reads "This workspace has no chat connection yet."
  And following "Connect Slack or Matrix in workspace integrations" lands on the workspace integrations page
```
