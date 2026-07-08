# Prioritization: navigation-bar-linear-ui

Prioritized by outcome impact and dependency order (Value × Urgency / Effort, walking-skeleton and riskiest-assumption first). Scales are 1–5.

## Release Priority

| Priority | Release / Story | Target Outcome | KPI | Rationale |
|----------|-----------------|----------------|-----|-----------|
| 1 | Walking Skeleton (US-01) | Persistent rail on dashboard+board with Home/Projects + active state; pre-auth pages stay chrome-free | KPI-1 (consistent nav presence), KPI-4 (active-state correctness) | Validates the core assumption: shared layout can host a rail on authed pages only, and `active_section` can be set per route. Everything depends on this. |
| 2 | Release 1 (US-02) | Account actions in one predictable place | KPI-2 (account actions ≤1 click) | Highest everyday value after orientation; sign-out currently lives only on the dashboard. Depends on US-01. |
| 3 | Release 2 (US-03, US-05) | Only the right people see admin entry; nothing orphaned | KPI-3 (admin-visibility correctness) | Must land before broad rollout so non-admins never hit a 403 trap and Invites/Tokens links are provably preserved (Decision #5). Depends on US-02. |
| 4 | Release 3 (US-04) | Same nav on every authed page | KPI-1 (100% authed coverage) | Highest template fan-out and shared-context risk; sequenced after the pattern is proven. Depends on US-01–US-03. |
| 5 | Release 4 (US-06) | Reads as Linear-quality; accessible | KPI-4, KPI-5 (accessibility) | Refinement over working behavior; last so we polish a settled structure. |

## Priority Scoring

| Story | Value | Urgency | Effort | Score (V×U/E) | Notes |
|-------|-------|---------|--------|---------------|-------|
| US-01 Walking skeleton | 5 | 5 | 3 | 8.3 | Riskiest assumption + end-to-end flow. |
| US-02 User menu + sign-out | 5 | 4 | 2 | 10.0 | Quick win, high everyday reach. |
| US-03 Instance-admin gating | 3 | 4 | 1 | 12.0 | Cheap, prevents 403 trap; gated before rollout. |
| US-05 Scoping guard (invites/tokens) | 3 | 4 | 1 | 12.0 | Regression protection for Decision #5. |
| US-04 Extend to remaining pages | 4 | 3 | 4 | 3.0 | High fan-out, context-plumbing risk. |
| US-06 Linear visual + a11y polish | 4 | 2 | 3 | 2.7 | Refinement; last. |

> Scores rank within their dependency tier; US-01 is executed first regardless of raw score because it is the walking skeleton (tie-break rule: Walking Skeleton > Riskiest Assumption > Highest Value).

## MoSCoW

| Story | MoSCoW | Justification |
|-------|--------|---------------|
| US-01 | Must | No rail, no feature. |
| US-02 | Must | Account actions in the always-present surface are the point of consolidation. |
| US-03 | Must | Admin-gating correctness is a security/UX safety requirement. |
| US-05 | Must | Prevents deletion of still-needed dashboard links (Decision #5). |
| US-04 | Should | Full coverage is the goal, but dashboard+board deliver value first. |
| US-06 | Should | "Linear-quality" polish; behavior works without final polish. |

## Backlog Suggestions

| Story | Release | Priority | Outcome Link | Dependencies |
|-------|---------|----------|--------------|--------------|
| US-01 | Walking Skeleton | P1 | KPI-1, KPI-4 | None |
| US-02 | Release 1 | P2 | KPI-2 | US-01 |
| US-03 | Release 2 | P3 | KPI-3 | US-02 |
| US-05 | Release 2 | P3 | KPI-3 | US-01 |
| US-04 | Release 3 | P4 | KPI-1 | US-01, US-02, US-03 |
| US-06 | Release 4 | P5 | KPI-4, KPI-5 | US-01 |
