# htmx Web Tier (Feature B) — Definition of Ready Validation

> 9-item DoR hard gate applied to each story. Feature B of the web-tier-extraction split.
> Stories: US-B01 (board template), US-B02 (vendored assets), US-B03 (issue+comments
> partial), US-B04 (sign-in/base layout), US-B05 (htmx normalize/upgrade), US-B06
> (pipeline scaffolding, `@infrastructure`).

## Per-story DoR matrix

| DoR item | US-B01 | US-B02 | US-B03 | US-B04 | US-B05 | US-B06 |
|----------|--------|--------|--------|--------|--------|--------|
| 1. Problem statement clear, domain language | PASS | PASS | PASS | PASS | PASS | PASS |
| 2. Persona with specific characteristics | PASS | PASS | PASS | PASS | PASS | PASS |
| 3. 3+ domain examples with real data | PASS | PASS | PASS | PASS | PASS | PASS |
| 4. UAT in Given/When/Then (3-7) | PASS (5) | PASS (4) | PASS (6) | PASS (4) | PASS (4) | PASS (3) |
| 5. AC derived from UAT | PASS | PASS | PASS | PASS | PASS | PASS |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS (M) | PASS (M) | PASS (M) | PASS (S-M) | PASS (S-M) | PASS (S) |
| 7. Technical notes: constraints/dependencies | PASS | PASS | PASS | PASS | PASS | PASS |
| 8. Dependencies resolved or tracked | PASS | PASS | PASS | PASS | PASS | PASS |
| 9. Outcome KPIs with measurable targets | PASS | PASS | PASS | PASS | PASS | PASS* |

\* US-B06 is `@infrastructure`; its "KPI" is an enabling/capability metric, acceptable for an
infra story that folds into a value slice (it never ships standalone).

## Evidence per item

1. **Problem statement** — each story names the concrete code pain (e.g. US-B01: "wording lives
   in `render_board` `format!()` in projects.rs"; US-B03: "four `format!` comment-render sites,
   OOB omits affordances"; US-B04: "bare `render_signin_form`, no CSS"). Domain language, no
   "implement X".
2. **Persona** — Jamal Okafor (Rust contributor, AGPLv3-attracted), Mei Chen (member),
   Devansh Rao (operator/self-hoster, runs air-gapped). Specific, grounded in backend-mvp + s
   code reading.
3. **Domain examples** — each story has 3 (happy / edge / error) with real names and real data
   (AUTH-2/3/6/7/8, "Refresh token rotation broken on Safari", Sandbox empty project, air-gapped
   VM, javascript: link sanitization).
4. **UAT** — each story has 3-7 Given/When/Then scenarios with business-outcome titles (no
   implementation in titles).
5. **AC** — each AC traces to a scenario; all observable/testable.
6. **Right-sized** — all S/M, ≤3 days, ≤6 scenarios. No story spans slices.
7. **Technical notes** — constraints (solution-neutral engine/htmx-version/CSS), carried NFRs,
   and what-moves-vs-what-stays are stated per story.
8. **Dependencies** — tracked: US-B01/B02/B06 in Slice 1; US-B03 dep US-B01; US-B04 dep
   US-B01/B06; US-B05 dep US-B01/B03/B04. Feature A (the seam) is shipped (resolved).
9. **Outcome KPIs** — defined per story in `outcome-kpis.md` with Who/Does-what/By-how-much/
   Baseline/Measured-by.

## JTBD traceability (hard-blocking check)

| Story | job_id | Valid? |
|-------|--------|--------|
| US-B01 | htmx-web-1 | YES (in jobs.yaml) |
| US-B02 | htmx-web-2 | YES |
| US-B03 | htmx-web-1 | YES |
| US-B04 | htmx-web-2 | YES |
| US-B05 | htmx-web-3 | YES |
| US-B06 | infrastructure-only (+ `infrastructure_rationale`) | YES (folds into Slice 1) |

## Elevator Pitch check (Dimension 0)

Every non-`@infrastructure` story (US-B01..B05) has an `### Elevator Pitch` with Before /
After / Decision-enabled, each referencing a real user-invocable entry point (a route the
user opens, a UI action) and concrete observable output (styled board, identical card,
styled sign-in). US-B06 is `@infrastructure` (Elevator Pitch not required) and folds into
Slice 1, which also contains US-B01 and US-B02 (user-visible) — so no slice is all-infra.

## DoR Status: PASSED (pending peer review and the open-question confirmations below)

### Items NOT blocking DoR but flagged for the user before DESIGN

These are validation/scoping confirmations, not story-readiness gaps:

1. **jtbd-web-2 retirement** — confirm that "web/api peer consumers of one core" is SATISFIED
   by Feature A (the code already delegates to `foundry_services`), so it is correctly RETIRED
   as a Feature-B job and carried only as a constraint.
2. **Build-time asset step open question** — DESIGN must decide build-time Node step vs pure
   vendored blobs (see `out-of-scope.md`). Does not block DISCUSS readiness.
3. **Secondary jobs not DIVERGE-validated** — htmx-web-1/2/3 remain Luna-derived (no diverge/
   dir). Grounded in backend-mvp + 2026-06 code reading; importances are estimates. Confirm
   the ranking (htmx-web-1 primary) before DESIGN.
