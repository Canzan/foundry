# Definition of Ready — pwa-mobile-rendering

| # | DoR item | Status | Evidence |
|---|----------|--------|----------|
| 1 | User value clear (job traced) | ✓ | Primary mobile job + elevator pitches (`user-stories.md`); KPI-tied (no SSOT, navigation-bar precedent) |
| 2 | Story INVEST-sized | ✓ | 3 stories → 3 slices, each ≤~1.5 day, independently shippable (`story-map.md`) |
| 3 | Acceptance criteria testable | ✓ | Given/When/Then in the fantoccini mobile lane (`acceptance-criteria.md`); layout facts, not screenshots |
| 4 | Dependencies identified | ✓ | Seam table verified in-tree (`requirements.md`): base.html head, hashed CSS + lib.rs:297, /static, browser_harness |
| 5 | No unresolved open questions | ◑ | ODD-1..5 deferred to DESIGN (columns strategy, nav affordance, SW-for-install, icons, modal sheet). None blocks slicing. |
| 6 | Technical feasibility confirmed | ✓ | Verified: no viewport meta, zero @media breakpoints, no manifest today; all changes are head tags + CSS + static assets; fantoccini can size the window |
| 7 | Scope boundaries explicit | ✓ | v1 = viewport+responsive+installable; offline/SW-caching, push, mobile SPA, Playwright all OUT (`requirements.md`) |
| 8 | NFR constraints stated | ✓ | fantoccini-not-Playwright (D-TOOL), CSS hash rotation ×2, HTTPS for install, no new route/migration, no-JS + oracles preserved |
| 9 | Measurable outcome defined | ✓ | `outcome-kpis.md` + falsification counter-metric (no-overflow red against the current no-viewport tree) |

## Notes on item 5

The five ODDs are genuine DESIGN calls, not deferred DISCUSS work:
- **ODD-1** (columns: scroll vs stack) and **ODD-5** (modal sheet) are layout decisions best made against a
  real phone dogfood; AC-02 pins the *invariant* (no page overflow, dialog ≤ viewport) regardless of the choice.
- **ODD-3** (does the install prompt need a service worker) is a factual check against current browser criteria
  — it decides whether v1 ships a minimal SW at all, and the scope boundary (no offline caching) holds either way.
- **ODD-2** (nav affordance) and **ODD-4** (icon assets) are straightforward once DESIGN picks the pattern.

## Verdict

**READY for DESIGN** (required — the responsive strategy, the nav/modal patterns, and the SW-for-install
question are real architectural calls; ODD-1..5 to resolve). The fantoccini-not-Playwright decision (D-TOOL)
and the CSS hash-rotation constraint (D5) are locked and must be honored by DESIGN/DELIVER.
