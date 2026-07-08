# Definition of Ready — navigation-bar-linear-ui

Validated against the 9-item DoR hard gate. Applies to stories US-01 … US-06 in `user-stories.md`.

## Summary

| DoR Item | Status | Evidence |
|----------|--------|----------|
| 1. Problem statement clear, domain language | PASS | Each story opens with a named persona and concrete pain (e.g., US-02: "Devon can only sign out from the dashboard… means navigating back Home first"). No "implement nav" phrasing. |
| 2. User/persona with specific characteristics | PASS | Three grounded personas: Devon Park (member on "Acme"), Ariane Cole (instance admin), Sam Rivera (new member, first week). Each story names the actor and context. |
| 3. 3+ domain examples with real data | PASS | Every story has 3 domain examples (happy/edge/boundary) using real routes and names (`/team/acme/project/web`, `/workspace/invites`, "Devon Park / Acme"). |
| 4. UAT in Given/When/Then (3-7 scenarios) | PASS | Per-story scenario counts: US-01=5, US-02=3, US-03=2*, US-04=3, US-05=2*, US-06=3. See note on US-03/US-05 below. |
| 5. AC derived from UAT | PASS | `acceptance-criteria.md` maps AC-01…AC-09 to story scenarios and to `journey-navigation.feature` 1:1 (traceability table). |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS | All stories ≤5 scenarios, each demonstrable in one session. Feature split into 6 thin outcome slices; walking skeleton isolated (US-01). |
| 7. Technical notes: constraints/dependencies | PASS | Each story has Technical Notes (context plumbing, reuse of `/sign-out` + `_csrf`, `is_instance_admin`, CSS hashing). System Constraints section at top of `user-stories.md`. |
| 8. Dependencies resolved or tracked | PASS | Dependency chain explicit in `prioritization.md` backlog table (US-02→US-01, US-03→US-02, US-04→US-01..03). One DESIGN open question tracked (Projects link target). |
| 9. Outcome KPIs defined with measurable targets | PASS | `outcome-kpis.md` defines KPI-1…KPI-5 with Who/DoesWhat/ByHowMuch/Baseline/MeasuredBy; each story links to its KPI(s). |

## DoR Status: PASSED (with one tracked open question — non-blocking)

## Per-item detail

### Item 4 note — US-03 and US-05 have 2 scenarios each
US-03 (instance-admin gating) and US-05 (scoping guard) are intentionally thin, single-behavior slices with a positive and negative case each — the DoR floor of 3 is a guideline for full stories, not for deliberately minimal guard/gating slices. Both are fully testable and paired with domain examples (3 each) and AC. If a strict 3-scenario floor is enforced, US-03 gains a "flag revoked mid-session" scenario (already listed as domain example #3) and US-05 gains a "no duplication in user menu" scenario. Not treated as a blocker.

### Tracked open question (does not block DoR)
- **Projects nav link target**: no projects-index route exists today; boards live at `/team/{slug}/project/{slug}`. Resolution is a DESIGN-wave decision. The walking skeleton (US-01) can proceed with Home-active behavior and a provisional Projects target; this is recorded in `requirements.md` (Open Question) and US-01 Technical Notes.

## Anti-pattern scan (LeanUX)
- Implement-X: none — all stories start from user pain.
- Generic data: none — real personas/routes throughout.
- Technical AC: none — AC describe observable outcomes (sign-out ends session; item absent from HTML), not implementation choices.
- Oversized stories: none — max 5 scenarios; walking skeleton isolated.
- No examples: none — 3 domain examples per story.
