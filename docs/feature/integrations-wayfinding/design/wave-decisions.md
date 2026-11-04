# DESIGN Decisions — integrations-wayfinding

Read before designing:
- ✓ docs/product/architecture/brief.md, specifically "Project events leave the
  board" and "For Software Crafter (external-integration-plugins)"
- ✓ ADR-EIP-001 to 007 (list)
- ✓ docs/product/journeys/journey-external-integration-plugins.yaml
- ✓ discuss/ (all files)
- ⊘ discover/ (none ran)

Scope: application. Mode: propose (user, 2026-11-03).

## Key Decisions
- [D1] The route form's refusal comes from one pure cause function over the
  instance offers the workspace page already uses (`slack_offer`,
  `matrix_offer`), plus "has a connection". A second pure function maps the
  cause and the viewer's role to words and a link (see:
  adr-iw-001-refusal-cause-from-instance-offers.md).
- [D2] Form, preview and save all render through `RouteDoor::form`, but two
  paths get there with the wrong state today, and US-IW-02 AC 7 needs both
  fixed:
  - **Save** (`routes.rs:303-321`): with no Slack bot, save falls through to
    `cannot_post` ("Foundry cannot post to that channel. Choose another.").
    The Matrix save (`matrix_setup.rs:162`) answers K-4 directly. Both are
    replaced by one guard right after `of_kind`: when `door.cause()` is `Some`,
    re-render the form with no notice, so the form shows the cause words. The
    `None => None` bot arm and `matrix_setup.rs:162` then become unreachable
    and are removed.
  - **Preview** (`routes.rs:206`): it builds the door with `RouteDoor::at` and
    never calls `of_kind`, so a Matrix preview renders the Slack form and the
    Slack cause. That is an existing bug, found in DESIGN review. Preview
    applies `.of_kind(chat_kind(&form.kind))` as save does.
  - **Send test** (`routes.rs:221`) and **edit** (`routes.rs:577`): the same
    gap, found at the final review gate. Send test applies
    `.of_kind(chat_kind(&form.kind))`; the edit door applies
    `.of_kind(route.kind)`. Every door that renders the route form then names
    the cause of the route's own kind. Send test is an acceptance row; a
    Matrix edit needs a saved Matrix route (Synapse), so DELIVER covers it
    with a handler unit test.
  - **Lost connection.** A lost connection is still a connection
    (`usable_connection` keeps every state but disconnected): the cause
    function returns `None` for it and the form behaves as it does today
    (routes save paused, eip-03). It is not a "no connection" refusal.
- [D3] The viewer's admin fact for integration pages comes free from the
  access check that is already made. `managed_project` reads
  `team_access(...)` facts, including `is_workspace_admin`, and returns them.
  `RouteTarget` and `ProjectIntegrations` gain `viewer_is_workspace_admin`.
  This adds no query.
- [D4] The project page's connection line reuses the workspace page's
  `IntegrationRecords::connections`, which gives kind, state and label.
  `project_view` returns those rows for the workspace. The app formats each
  kind as follows:
  - Connected: W-6.
  - Lost: the workspace page's `lost_words`.
  - Unreadable: K-19. Readability is judged the way the route form already
    judges it (`RouteDoor::at`, `route_form.rs:233`): `Integrations::opens`
    over the kind's sealed connection. It is **not** judged through
    `Caller::unreadable_connections`, which runs `require_admin` and would
    404 a lead. So `project_view` also returns the Slack and Matrix sealed
    connections through the same `usable_connection` read `route_target`
    uses.
  - When no kind is connected: W-7 for an admin (with its link) and K-4 for a
    lead (W-8).
  - "New route" always stays (US-IW-03 AC 3).

  Cost: three more reads per project integrations page render
  (`workspace_connections` and two `sealed_connection` reads). There is no
  admin read: `managed_project`'s access facts already carry
  `is_workspace_admin`. The board and the route form gain no query.
- [D5] The sidebar's workspace-admin fact rides one widened identity read,
  `Store::nav_identity`. `home_for` and `board_for` use it instead of
  `dashboard_greeting`, so there is no extra round trip. It fails closed (see:
  adr-iw-002-workspace-admin-fact-rides-the-nav-identity-read.md).
- [D6] W-1 is not marked as the current page. This changes US-IW-01 AC 3 (see:
  upstream-changes.md).

## Architecture Summary
- Pattern: the existing modular monolith with ports and adapters. No new
  component, crate or port.
- Paradigm: unchanged. Rust, with `nw-software-crafter`, as for every feature
  so far.
- Components touched:
  - `foundry-store`: `nav_identity`.
  - `foundry-services/integrations`: `managed_project` returns the facts;
    `RouteTarget` and `ProjectIntegrations` gain fields.
  - `foundry-app/integrations`: a new `no_connection.rs` (cause and words);
    `RouteDoor`, the project page view.
  - `foundry-app/nav.rs`, `partials/sidebar.html`.
  - Templates: `route_form.html`, `integrations/project.html`.

## Technology Stack
- No change: axum, askama, sqlx/Postgres.

## C4

System context and containers are unchanged. The browser talks to `foundry`
(axum), which talks to Postgres, Slack and Matrix. See brief.md, "Crate
graph". The component view of this change:

```mermaid
flowchart LR
  subgraph app[foundry-app]
    SB[partials/sidebar.html] --> NAV[nav.rs NavContext\n+is_workspace_admin]
    RF[handlers/route_form.rs RouteDoor] --> NC[no_connection.rs\ncause + words (pure)]
    NC --> OFF[Integrations::slack_offer / matrix_offer]
    WP[handlers/workspace.rs] --> OFF
    PP[handlers/routes.rs show_project] --> PV
  end
  subgraph services[foundry-services/integrations]
    RT[route_target] --> MP[managed_project\nreturns access facts]
    PV[project_view\n+connections, +viewer_is_workspace_admin] --> MP
  end
  subgraph store[foundry-store]
    NI[nav_identity\n1 query]
    WC[workspace_connections]
    TA[team_access_facts]
  end
  NAV --> NI
  RF --> RT
  PV --> WC
  MP --> TA
```

## Constraints Established
- A link's visibility comes from the same access facts the target page checks:
  workspace admin for workspace pages, `can_manage` for project pages. It never
  comes from a separate cached copy.
- Every no-connection message on the route form comes from `no_connection.rs`.
  No template computes a cause.
- There are no new queries on board or route-form renders, and the nav adds
  no round trip. The project integrations page adds three reads (D4).
- The cause precedence is each offer's own order, so the two pages agree:
  - Slack: no app (W-2) comes before an unreadable connection (K-19), then no
    secrets key (W-3), then not connected (W-4 or K-4). See `slack_offer`,
    `mod.rs:251`.
  - Matrix: an unreadable connection (K-19) comes before no secrets key (W-2),
    then not connected. See `matrix_offer`, `mod.rs:241`.

## For Acceptance Designer
Driving ports are HTTP pages through `InProcHarness`, as eip-06 does:
- `GET /team/{t}/project/{p}/integrations/routes/new?kind={slack|matrix}`, and
  `POST .../routes/preview` and `POST .../routes` for AC 7.
- `GET /team/{t}/project/{p}/integrations`.
- Any app-shell page for the sidebar: board, project integrations, route form.

Instance states to arrange:
- No Slack app.
- Slack app present but no secrets key.
- Both present and the workspace not connected.
- Connected.
- Lost.
- Unreadable (K-19).

Each in turn for Priya (admin), Lena (lead) and Sam or Marco (no access, 404).
Reuse eip-06's `link_hrefs` and follow-the-link steps. KPI-IW-G1 is checked by
following every offered link and expecting 200.

## For Software Crafter
- `NoConnectionCause` and its words are pure and table-tested: 5 cells × 2
  roles, plus `Unreadable`.
- `RouteDoor.no_connection: bool` becomes `cause: Option<NoConnectionCause>`.
- `RouteFormPage.no_chat_connection: String` becomes the words plus an
  optional `(label, href)`.
- `managed_project` returns `(ProjectRow, TeamAccessFacts)`, or just the admin
  bool. Keep the `NotFound` refusal unchanged.
- `NavContext::for_page` gains `is_workspace_admin`. Update the two
  `signin.rs` callers and the test helpers in `nav.rs` and `projects.rs:987`.
- Save guard and preview `of_kind` per D2. Remove the unreachable no-bot
  `cannot_post` arm and `matrix_setup.rs:162`.

## Changed at the final review gate (2026-11-03)
- D2 extended to Send test and edit, and the lost-connection rule stated.
- ADR-IW-001 defines "has a connection".

## Upstream Changes
- US-IW-01 AC 3, US-IW-02 AC 5 and US-IW-02 AC 6 are refined. See
  upstream-changes.md. The user accepted them on 2026-11-03.
