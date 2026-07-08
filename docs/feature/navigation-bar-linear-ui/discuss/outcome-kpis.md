# Outcome KPIs: navigation-bar-linear-ui

## Feature: Shared Linear-style navigation sidebar

### Objective
Within one release, make the Foundry web app feel oriented and consistent — every signed-in member always knows where they are and reaches any primary surface or account action in one click, from any authenticated page, while pre-auth pages stay chrome-free.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| KPI-1 | Signed-in members on authenticated app pages | See the same consolidated nav present | On 100% of authenticated app pages (and 0% of pre-auth/util pages) | 0% — no shared nav exists; navigation is per-page ad-hoc | Template/route audit + rendered-HTML assertion across all 21 templates | Leading |
| KPI-2 | Signed-in members on any authenticated page | Reach primary surfaces (Home, Projects) and account actions (sign out, shortcuts) | In ≤1 click from any authed page | Sign-out/shortcuts reachable only from the dashboard (often ≥2 clicks from a board/issue) | Click-path analysis from each authed route to each target | Leading |
| KPI-3 | Non-admin members / pre-existing dashboard destinations | See only the entries they're entitled to; nothing becomes orphaned | 100% of non-admin renders omit the Instance admin item; 100% of pre-existing dashboard destinations (Invites, Tokens) remain reachable | Admin gating exists on dashboard; risk of 403-trap or deleted links after consolidation | Rendered-HTML assertions (admin vs non-admin) + link-preservation test | Leading |
| KPI-4 | Signed-in members on any authenticated page | Correctly perceive their current location via active-state highlighting | Exactly one primary item marked current, matching the route, on 100% of authed pages (never zero, never two) | No active-state indicator exists today | Automated active-state assertion per route family | Leading |
| KPI-5 | Keyboard and assistive-tech users | Operate the nav without a mouse and perceive the current item | Nav passes automated a11y checks: semantic landmark, aria-current on active item, keyboard focus + activation on 100% of items | No nav component exists to evaluate | axe-core / accessibility test on the nav component | Leading |

### Metric Hierarchy
- **North Star**: KPI-2 — primary surfaces and account actions reachable in ≤1 click from any authenticated page. This is the essence of "consolidated, consistent navigation".
- **Leading Indicators**: KPI-1 (presence coverage) and KPI-4 (active-state correctness) predict whether members can actually orient and move.
- **Guardrail Metrics**:
  - Pre-auth pages remain chrome-free (0% show a rail) — must NOT degrade.
  - No pre-existing destination becomes unreachable (KPI-3 link preservation) — must NOT degrade.
  - Page render output for excluded pages is byte-unchanged (NFR-3).

### Measurement Plan
| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|-------------|-------------------|-----------|-------|
| KPI-1 | Rendered HTML of all templates | Automated test asserting presence/absence of nav markup per route class | CI, per build | foundry-app web tier |
| KPI-2 | Route → target click-path map | Acceptance test verifying ≤1 click to each target from each authed route | CI, per build | foundry-app web tier |
| KPI-3 | Rendered HTML (admin vs non-admin); dashboard links | Assertion tests (gating + link preservation) | CI, per build | foundry-app web tier |
| KPI-4 | Rendered HTML active-state markers | Assertion of exactly-one-active per route family | CI, per build | foundry-app web tier |
| KPI-5 | Nav component DOM | axe-core / a11y assertions | CI, per build | foundry-app web tier |

> These KPIs are verifiable in CI via rendered-HTML and accessibility assertions — no runtime analytics instrumentation is required, which fits the server-rendered web tier. If usage analytics become available later, add an actionable behavioral metric (e.g., proportion of board→dashboard transitions made via the rail vs. the browser back button).

### Hypothesis
We believe that a single, always-present, Linear-style left sidebar for signed-in members will make navigation feel oriented and consistent.
We will know this is true when members can reach any primary surface or account action in ≤1 click from any authenticated page (KPI-2), the nav is present on 100% of authed pages and 0% of pre-auth pages (KPI-1), and active-state highlighting is correct on 100% of authed pages (KPI-4), with no pre-existing destination orphaned (KPI-3) and full keyboard/assistive-tech operability (KPI-5).

## Handoff to DEVOPS (platform-architect)
- **Data collection**: All KPIs are asserted in CI via rendered-HTML and a11y checks; no production analytics instrumentation required for this feature.
- **Guardrails**: pre-auth chrome-free (0% rail), link preservation (Invites/Tokens reachable), excluded-page output unchanged.
- **Baseline**: current state is 0% shared nav; capture "before" render snapshots of excluded pages to guarantee no regression (NFR-3).
