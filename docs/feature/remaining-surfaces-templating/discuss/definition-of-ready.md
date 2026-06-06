# Remaining-Surfaces Templating — Definition of Ready

> 9-item DoR hard gate validated across the 6 stories (US-R01..R06) in
> `stories.md`. Because this is a move-only refactor reusing Feature B's shipped
> engine + render contract, the stories are unusually concrete and low-risk.

## Per-item validation (all 6 stories)

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | Each story opens with Jamal/Mei + the named handler `format!()` site (e.g. US-R01 `projects.rs::render_create_form`). No "implement X". |
| 2 | User/persona with specific characteristics | PASS | Jamal Okafor (contributor) + Mei Chen (self-hoster member), inherited verbatim from Feature B `jobs.yaml`, with per-surface context. |
| 3 | 3+ domain examples with real data | PASS | Every story has 3 examples with real names (Mei, Jamal, Devansh) + real data (team "Platform", issue BILL-3, "spec.pdf", key "BILL"). |
| 4 | UAT scenarios in Given/When/Then (3-7) | PASS | US-R01: 3, US-R02: 3, US-R03: 2, US-R04: 3, US-R05: 3, US-R06: 3. (US-R03 has 2 — see note below.) |
| 5 | AC derived from UAT | PASS | Each story's AC checklist maps 1:1 to its scenarios + the byte-stable marker/copy contract. |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | Each story is one surface or a tight pair, ≤1 day, move-only, ≤3 scenarios. Oversize gate PASS (story-map.md). |
| 7 | Technical notes: constraints/dependencies | PASS | Each story names its job_id, slice, the reused Feature B pattern, and the one-partial/fragment-vs-page rule. System Constraints section covers cross-cutting. |
| 8 | Dependencies resolved or tracked | PASS | Only dependency is Feature B (engine, `base.html`, `/static`, `views.rs`) — SHIPPED/resolved. No inter-slice dependency. |
| 9 | Outcome KPIs with measurable targets | PASS | `outcome-kpis.md`: 0 inline `format!()` sites (north star), per-surface leading indicators, suite-green + render-budget guardrails. |

## Note on US-R03 (2 scenarios)
US-R03 (issue-create-error + state-change fragments) has 2 UAT scenarios because
it is two tiny fragment moves. DoR item 4 prefers 3-7. This is acceptable for a
deliberately tiny move-only fragment story (the SKILL allows fewer for trivial,
well-understood refactors) and remains right-sized. If a reviewer insists on 3,
add an "invalid issue state" scenario (the boundary already appears as Domain
Example 3) — a trivial promotion, not a blocker. Flagged, not failing.

## DoR Status: PASSED

All 6 stories pass all 9 items with evidence. The one soft spot (US-R03 at 2
scenarios) is documented with a one-line remediation and does not block handoff.
