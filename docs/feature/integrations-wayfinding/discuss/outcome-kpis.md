# Outcome KPIs — integrations-wayfinding

There is no product analytics on the instance (System Constraints). Each KPI is
measured by acceptance scenarios on every gate run, and once by a manual walk
on dev after release.

| ID | Outcome | Measure | Target | How |
|----|---------|---------|--------|-----|
| KPI-IW-1 | An admin who meets a "no connection" refusal can reach the fix | Clicks from the route form or the project integrations page to the page where the fix happens (workspace integrations or Slack setup) | ≤ 1 click in each cell with a fix page: Slack with no app (setup page), Slack not connected, Matrix not connected (workspace integrations). The two no-secrets-key cells are fixed outside Foundry, so the KPI there is that they name that cause (KPI-IW-2) and offer no link. | Acceptance outline over the states × kinds |
| KPI-IW-2 | Every refusal names its real cause | Refusal states whose text names the right level: Foundry (instance), secrets key, or workspace | 100% of 5 cells (Slack × 3, Matrix × 2); none says "Ask a workspace admin" to a workspace admin | Acceptance outline |
| KPI-IW-3 | A workspace admin reaches workspace integrations from anywhere | App-shell pages that offer W-1 to an admin | 100% of pages rendered with the sidebar | Acceptance check on board, project integrations and route form, plus the shared sidebar partial |
| KPI-IW-G1 (guardrail) | No one is offered a link they cannot open | Links offered (W-1, W-4, W-6, W-7, the setup link) that answer the viewer with a 404 | 0 | Outline over Priya, Lena, Sam and Marco follows each offered link and expects 200 |
| KPI-IW-G2 (guardrail) | Existing integration scenarios keep passing | external-integration-plugins features 01–06 | 100% pass, with only the K-4 scenario's actor changed if needed | `cargo xtask ci` |
| KPI-IW-M (manual, post-release) | The reported dead end is gone | The user, on dev (workspace "Default", project ABC at `/team/team-2/project/abc`, no Slack app), opens a new Slack route | Reads W-2 and follows "How to set up Slack" | Walk on https://foundry.unintelligent-design.us after the release |
