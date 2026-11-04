# DISTILL Decisions — integrations-wayfinding

Read before writing:
- ✓ docs/product/journeys/journey-external-integration-plugins.yaml (step 2 failure modes W-2 to W-5)
- ✓ docs/product/architecture/brief.md (Wayfinding subsection); ADR-IW-001, ADR-IW-002
- ✓ docs/product/kpi-contracts.yaml (integrations-wayfinding)
- ✓ discuss/user-stories.md, story-map.md, wave-decisions.md, journey files
- ⊘ spike/ (none ran)
- ✓ design/wave-decisions.md, design/upstream-changes.md
- ✓ devops/wave-decisions.md
- ✓ docs/architecture/atdd-infrastructure-policy.md
- Deliverable type: application (no `deliverable_type` in `.nwave/des-config.json`)

Reconciliation passed: 0 contradictions. DESIGN's three AC changes were
accepted by the user and applied to the stories before this wave.

## Key Decisions
- [D1] One feature file, `crates/foundry-acceptance/tests/features/integrations-wayfinding.feature`:
  16 scenario declarations, 54 examples. Strategy (Architecture of
  Reference): real driving port (HTTP through the production router), real
  driven-internal Postgres, Slack faked by the shipped double; no in-memory
  tier. Layer 3 (real HTTP through
  `InProcHarness`, real Postgres, Slack as the in-process double):
  example-only, sad paths enumerated (Mandates 9 and 11). No Tier B: the
  feature is wording and links, with no chained journey of state to model.
- [D2] The driver extends the eip driver rather than copying it:
  `support/integrations_wayfinding.rs` and
  `steps/feature_integrations_wayfinding.rs`. Reused unchanged: the
  Background, "the instance has …", the connection Givens, the uninstall
  event, "opens <page>", "reads …", "is on <page>", "still has no route". The
  eip driver gained one instance phrase ("no secrets key"), a Given that
  connects a named Slack workspace (`given_slack_connected_to`), and seven
  `pub(crate)` helpers (`get`, `post`, `follow`, `text`, `texts_of`,
  `link_hrefs`, `in_sidebar`).
- [D3] An offered page link is counted without the sidebar (`link_hrefs`).
  Without this, W-1 would give Priya two "Integrations" links on the
  dashboard and the board, and eip-06's "exactly one" oracle would turn red
  for a reason that is not a regression. A link that must NOT be offered is
  counted everywhere, sidebar included (`all_link_hrefs`), so a leaked sidebar
  entry still fails eip-06's not-offered rows (review blocker, fixed).
  eip-06 re-ran green 9/9, eip-01 44/44.
- [D4] Instance states reach the app through the eip `rewire` seam: no Slack
  app, Slack app without a secrets key, no secrets key (Matrix), and a
  replaced secrets key (K-19, in process, without the operator's process).
  The lost state is Slack's signed uninstall event.
- [D5] Matrix connected (W-6 for Matrix), Matrix unreadable (K-19 over W-2)
  and a Matrix route's edit form are not acceptance scenarios. Each needs a
  saved Matrix connection, and the product saves one only after the
  homeserver answers for the token (`connect_matrix` → Synapse,
  `@needs-matrix`), which DEVOPS D4 keeps out of this lane. The Slack K-19
  rows work in process because a Slack connection is saved through the
  in-process Slack double. DELIVER obligation, named: unit tests in
  `crates/foundry-app/src/integrations/no_connection.rs` (the full table,
  Matrix rows included), the project page's line formatting for Matrix, and
  a handler test that the edit door applies the route's kind.
- [D6] No RED scaffolds (Mandate 7 N/A): the scenarios import no production
  module. Everything is observed over HTTP. `no_connection.rs` is DELIVER's.
- [D7] Walking skeleton: "A workspace admin reaches the workspace
  integrations from the sidebar" (`@walking_skeleton @driving_port`). It is
  `@pending` like the rest, as in every feature since ADR-025 here: a
  behaviour change has no green end-to-end path before DELIVER. DISCUSS's
  "no walking skeleton" (no skeleton slice) stands: the tag marks the demo
  path, not a slice (noted in discuss/wave-decisions.md). Nine `@control`
  examples are green now and stay green: K-4 for a lead (Slack, Matrix), K-19
  ahead of W-3 and W-4, and the uniform not-found page for Sam and Marco.
- [D8] Outcomes registered: OUT-36 (specification: the refusal cause per
  kind and role) and OUT-37 (invariant: an integrations link is offered only
  to a viewer that page answers).

## Scenario list

| Scenario | Tags | State |
|---|---|---|
| A workspace admin reaches the workspace integrations from the sidebar (3 pages) | @us-iw-01 @driving_port @walking_skeleton @kpi-iw-3 @kpi-iw-g1 | pending |
| Only workspace admins are offered the sidebar entry (7 rows) | @us-iw-01 @kpi-iw-3 | pending |
| The sidebar entry sits in the user section and is never the current page | @us-iw-01 | pending |
| The new-route form names what is missing, and offers an admin the fix (8 rows) | @us-iw-02 @driving_port @kpi-iw-1 @kpi-iw-2 @kpi-iw-g1 | pending |
| A workspace admin is never told to ask an admin, and is told only the form's kind (5 rows) | @us-iw-02 @kpi-iw-1 @error | pending |
| A team lead who is not an admin still reads K-4 (Slack, Matrix) | @us-iw-02 @control @kpi-iw-g2 | green |
| An unreadable saved Slack connection is named first (replaced key, no key) | @us-iw-02 @kpi-iw-2 @error @control | green |
| With no Foundry Slack app, the form says so even over an unreadable connection | @us-iw-02 @kpi-iw-2 @error | pending |
| Preview, save and Send test refuse with the form's words (14 rows) | @us-iw-02 @kpi-iw-1 @error | pending |
| The project page names the shared Slack connection (distinct Slack name), admin link | @us-iw-03 @driving_port @kpi-iw-g1 | pending |
| A team lead sees the shared connection, no workspace link | @us-iw-03 | pending |
| An admin sees nothing is connected, and where to connect | @us-iw-03 @kpi-iw-g1 | pending |
| A team lead sees nothing is connected and still asks an admin | @us-iw-03 @error | pending |
| A lost Slack connection reads as lost | @us-iw-03 @error | pending |
| An unreadable Slack connection reads as unavailable | @us-iw-03 @error | pending |
| Someone who cannot manage the team gets not-found, in any state (5 rows) | @us-iw-02 @us-iw-03 @kpi-iw-g1 @error @control | green |

Examples tagged @error: 30 of 54 (56%).

## Driving ports and adapters

| Door (DESIGN) | Scenarios |
|---|---|
| Any app-shell page's sidebar (board, project integrations, route form, dashboard, workspace integrations) | US-IW-01 outlines |
| `GET …/integrations/routes/new?kind=slack\|matrix` | US-IW-02 form outline, controls, precedence |
| `POST …/routes/preview`, `POST …/routes` | Preview and save outline |
| `GET …/integrations` (project page) | US-IW-03 scenarios |
| Links offered (W-1, W-2 setup, W-4, W-6, W-7) | followed, 200 expected (KPI-IW-G1) |

Driven adapters are unchanged and already covered by eip: Postgres
(per-scenario schema), the Slack double. Policy: the existing "Integration
surfaces" row of `docs/architecture/atdd-infrastructure-policy.md` covers
these doors; no row added.

## Mandate 12

Criteria 1–3 met: the driver's typed parameters (`Kind`, `RouteDoor`), every
step body is one driver call with no control flow. Step reuse ratio
(informational): 82 step lines over 14 new decorators plus 12 reused, about
3.2×, the natural ceiling for a wording-and-links feature.

## Final review gate (2026-11-03)
Four reviewers: DISCUSS and DESIGN and DEVOPS conditionally approved, DISTILL
rejected with one blocker (D3). Fixed in this wave: the blocker; coverage
added for the full sidebar matrix, K-19 over W-3, preview/save/test for every
cause and for a lead, no-access not-found in two states, and a Slack
workspace name distinct from the Foundry workspace's; steps reduced to one
driver call each (typed `Offer`, `Fix`, `RouteDoor`). Upstream fixes: DESIGN
D2 now covers Send test and edit and states the lost rule; ADR-IW-001 defines
"has a connection"; the brief records the precedence and the project-page
line; DISCUSS's precedence lists include K-19; DEVOPS states rollback, the
release go, and G2's instrument; KPI-IW-3's target names the pages checked.

## Upstream Changes
- DESIGN D2 extended (Send test, edit, lost rule); see design/wave-decisions.md.
