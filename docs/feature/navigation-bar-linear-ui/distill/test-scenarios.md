# DISTILL — Test Scenarios: navigation-bar-linear-ui

SSOT for the executable acceptance suite. The `.feature` file
(`crates/foundry-acceptance/tests/features/navigation-bar-linear-ui.feature`, 15
scenarios) is the scenario source of truth; this document is the
scenario → AC → story → step traceability map plus the DISTILL decisions.

## Harness + placement (project convention)

- **Framework:** cucumber-rs, real running `foundry` app in-process + real Postgres
  via testcontainers (matches every existing `crates/foundry-acceptance` feature).
- **Driving port:** HTTP GET on the authed routes + the reused CSRF `POST /sign-out`;
  pre-auth GET on `/sign-in` + `/forgot-password`. No production code is imported by
  the steps — the suite is black-box over the rendered HTML, so **no Rust RED
  scaffold** is required (nav.rs / app_shell.html / sidebar.html are DELIVER's to
  create; the tests fail RED at the HTML assertions once un-pended).
- **Files:**
  - `tests/features/navigation-bar-linear-ui.feature`
  - `src/steps/feature_navigation_bar.rs`
  - registered in `src/lib.rs` (`pub mod feature_navigation_bar;`) and
    `tests/acceptance.rs` (force-link `use … as _feature_navigation_bar;`).
- **@pending policy (copied from dashboard-enhancements):** every scenario starts
  `@pending`, which `acceptance.rs` excludes from every lane, so the `@all` lane stays
  GREEN until DELIVER removes the tag per-scenario (Outside-In).
- **Deliberate deviations from the generic nWave template:** no
  `docs/architecture/atdd-infrastructure-policy.md` and no
  `tests/common/state_delta.rs` were bootstrapped — this project uses the legacy
  per-feature wave layout and an established cucumber-rs harness, not the 3.21
  SSOT/state-delta model (per project memory + the task's "follow the project's real
  convention" directive).

## Wave-decision reconciliation (DISCUSS ↔ DESIGN ↔ DEVOPS)

Reconciliation **PASSED — 0 blocking contradictions**. Three DISCUSS→DESIGN
refinements were folded into the executable form (DESIGN legitimately supersedes; the
first was an explicit DISCUSS open question):

| DISCUSS | DESIGN / real code (authoritative) | Why not a contradiction |
|---|---|---|
| primary item **"Projects"** | `NavSection::Board`, item **"Board"**, deep-links to the first project board | DISCUSS US-01 technical note flags the Projects-target as "a DESIGN open question"; ADR-003 resolves it to a Board deep-link |
| pre-auth page **`/signin`**, **`/forgot`** | real routes **`/sign-in`**, **`/forgot-password`** (`lib.rs:348,387`) | route-name facts; DISCUSS used shorthand |
| persona **Devon Park** | Background persona **Ada / "Ada Lovelace"** | task directive: reuse the dashboard-enhancements Background verbatim so DELIVER shares step glue |

## Traceability: scenario → AC → story → steps

Legend for steps: **R** = reused existing phrasing (not redefined), **N** = new
nav-specific phrasing defined in `feature_navigation_bar.rs`.

| # | Scenario | Tags | AC | Story | Key steps |
|---|----------|------|-----|-------|-----------|
| 1 | The dashboard shows the shared sidebar with Home current | `@walking_skeleton @us-01 @real-io` | AC-01.1, AC-03.1 | US-01 | R `Ada visits "/"`; N sidebar shown / workspace name / primary items / Home current / Board not current |
| 2 | Sidebar present on every authenticated app page (Outline ×5) | `@us-04 @real-io` | AC-01.2 | US-04 | N `opens the authenticated page`; N sidebar shown / primary items |
| 3 | Board is current while viewing a board | `@us-01 @real-io` | AC-03.2 | US-01 | N `opens the authenticated page`; N Board current / Home not current |
| 4 | Exactly one primary item current on every page (Outline ×5) | `@property @us-01 @us-06 @real-io` | AC-03.3 | US-01, US-06 | N `opens the authenticated page`; N exactly-one-current |
| 5 | Active item is an accessible landmark carrying aria-current | `@us-06 @real-io` | AC-08.1 | US-06 | N landmark; N current item carries aria-current |
| 6 | Pre-auth/util pages do not show the sidebar (Outline ×2) | `@us-01 @real-io` | AC-02.1 | US-01 | N `visitor is not signed in`; N `opens the pre-auth page`; N no sidebar / only page content |
| 7 | User menu links to keyboard shortcuts | `@us-02 @real-io` | AC-05.2 | US-02 | R `Ada visits "/"`; N user menu contains link `/keyboard-help` |
| 8 | User menu signs out with a CSRF token | `@us-02 @real-io @security` | AC-05.3 | US-02 | R `Ada visits "/"`; N sign-out control posts `/sign-out` + `_csrf` |
| 9 | Super-admin sees the Instance admin item | `@us-03 @real-io` | AC-06.1 | US-03 | R `Ada is an instance super-admin`, `Ada visits "/"`; N user menu contains link `/admin/instance/workspaces` |
| 10 | Non-super-admin never sees the Instance admin item | `@us-03 @real-io @security` | AC-06.2 | US-03 | R `a member "Mei" who is not an instance admin is signed in`, `Mei visits "/"`; N user menu absent link |
| 11 | Rail renders workspace name + signed-in identity | `@us-04 @real-io` | AC-01.1, AC-05.1 | US-04, US-02 | R `Ada visits "/"`; N sidebar workspace name / footer name |
| 12 | Display name containing markup is rendered inert in the rail | `@us-04 @real-io @security` | AC (XSS safety) | US-04 | R `a member "Mallory" whose display name is "<b>pwn</b>" is signed in`, `Mallory visits "/"`, escaped-name Thens |
| 13 | Board item deep-links to the first project board | `@us-01 @real-io` | AC-04.2 | US-01 | R `Ada visits "/"`; N sidebar links "Board" to `/team/general/project/sandbox` |
| 14 | Board item falls back to `/` when there are no projects | `@us-01 @real-io` | AC-04.2 (ADR-003 fallback) | US-01 | N `the "Acme" workspace has no projects`; R `Ada visits "/"`; N sidebar links "Board" to `/` |
| 15 | Invites/tokens stay in Quick actions, not promoted | `@us-05 @real-io` | AC-07.1, AC-07.2 | US-05 | R `Ada visits "/"`, R `contains a link to` ×2; N sidebar does not contain item ×2 |

## Error / edge / negative ratio (Mandate: ≥40%)

Non-happy-path scenarios: #4 (invariant guard), #6 (absence), #10 (negative gating),
#12 (XSS-inert edge), #14 (empty-state fallback), #15 (regression guard) = **6 of 15
= 40%**.

## Adapter coverage

Single driven adapter class: the real Postgres store behind the authed page handlers.
Every scenario is `@real-io` and exercises it through the production composition root
(real axum app + real per-scenario schema). No new external adapter is introduced
(DESIGN §8: "No external integrations"), so no additional `@adapter-integration`
scenario is warranted.

## New vs reused step phrasings

- **Reused (12, not redefined — avoids cucumber-rs duplicate-step panic):**
  `a workspace … admin "Ada" … "Ada Lovelace"`, `a project … exists in …`,
  `(\w+) is signed in`, `(\w+) visits "/"`, `Ada is an instance super-admin`,
  `a member … who is not an instance admin is signed in`,
  `a member … whose display name is … is signed in`, `the response body contains "…"`,
  `contains a link to "…"`, `does not contain a link to "…"`,
  `contains the escaped display name`, `does not contain a live "<b>" element`.
- **New (nav-specific, 20):** `opens the authenticated page "…"`,
  `a visitor opens the pre-auth page "…"`, `a visitor is not signed in`,
  `the "…" workspace has no projects`, `a persistent left sidebar is shown`,
  `no navigation sidebar is shown`, `only the page's own content is visible`,
  `the sidebar shows the workspace name "…"`,
  `the sidebar footer shows the signed-in name "…"`,
  `the sidebar shows primary navigation items "…" and "…"`,
  `the "…" navigation item is marked as the current page`,
  `the "…" navigation item is not marked as current`,
  `exactly one primary navigation item is marked as the current page`,
  `the sidebar is exposed as a navigation landmark`,
  `the current navigation item carries an aria-current marker`,
  `the sidebar links "…" to "…"`, `the user menu contains a link to "…"`,
  `the user menu does not contain a link to "…"`,
  `the user menu contains a sign-out control posting to "…" with a CSRF token`,
  `the sidebar does not contain a "…" item`.

## ACs not expressed as executable acceptance scenarios (and where covered)

| AC | Why not an HTTP acceptance scenario | Covered instead by |
|---|---|---|
| AC-08.2 keyboard operability (focus states, activation) | Focus traversal + `:focus-visible` are browser/interaction behaviours; the in-process HTTP harness renders HTML but does not run a browser event loop | DELIVER: template presence of focusable `<a>`/`<button>` is structurally implied by the rail; visible-focus CSS is verified by the US-06 CSS/component check |
| AC-09.1 / AC-09.2 visual quality (rail width ~220–260px, accent tint, content offset) | Pixel/visual properties are not observable in server-rendered HTML text | DELIVER: CSS rules on `.sidebar` / `.sidebar__item--active` / `.app-shell__content`, verified by the hashed-CSS asset check (ADR-004) + component/visual test; DISCUSS already routes AC-09 to "component/visual test" |
| AC-05.1 "opens the user menu" (interaction) | Menu open/close is an Alpine interaction; the acceptance layer asserts the menu's rendered contents, not the open gesture | Contents covered by #7/#8/#9/#11 (the anchor + links render in `.sidebar__user` regardless of open state) |
