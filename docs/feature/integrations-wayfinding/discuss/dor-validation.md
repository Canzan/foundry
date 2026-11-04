# Definition of Ready — integrations-wayfinding

| # | DoR item | US-IW-01 | US-IW-02 | US-IW-03 | Evidence |
|---|----------|----------|----------|----------|----------|
| 1 | Problem stated in the user's terms | ✓ | ✓ | ✓ | User reports, 2026-11-02 and 2026-11-03: "something is wrong with my account…"; "supposed to add a link… what happened?"; "use a configured integration with multiple projects" |
| 2 | User and persona identified | ✓ | ✓ | ✓ | Priya (workspace admin, persona-instance-operator); Lena (lead, not admin) |
| 3 | At least 3 domain examples with real data | ✓ | ✓ | ✓ | Dev workspace "Default", project ABC (`team-2/abc`), no Slack app; scenario cast Priya, Lena, Sam, Marco from eip-06 |
| 4 | UAT scenarios in Given/When/Then | ✓ | ✓ | ✓ | user-stories.md, journey yaml |
| 5 | Acceptance criteria derived from the scenarios | ✓ | ✓ | ✓ | 4, 7 and 4 numbered ACs |
| 6 | Right-sized (1–3 days, ≤ 7 scenarios each) | ✓ | ✓ | ✓ | US-IW-01 0.5 d, US-IW-02 1 d, US-IW-03 0.5 d; no schema change |
| 7 | Technical notes and constraints | ✓ | ✓ | ✓ | System Constraints: access rules, locked copy, the 5 cause cells (Slack × 3, Matrix × 2) and their precedence |
| 8 | Dependencies resolved or tracked | ✓ | ✓ | ✓ | Tracked: `NavContext` (`nav.rs:29`) carries only `is_instance_admin`, so US-IW-01 must add the workspace-admin fact (DESIGN). Otherwise uses existing `is_workspace_admin`, `team_access_facts`, `IntegrationSettings` and `MatrixOffer` |
| 9 | Outcome KPIs defined | ✓ | ✓ | ✓ | outcome-kpis.md: KPI-IW-1 to 3, G1, G2, M |

Open for DESIGN, not blocking:
- How to add the workspace-admin fact to `NavContext` without one more query
  per page render (US-IW-01).
- Do the preview and save posts share the route form's state code, so US-IW-02
  AC 7 holds by construction?
