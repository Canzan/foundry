# Definition of Ready — issue-status-move

| # | DoR item | Status | Evidence |
|---|----------|--------|----------|
| 1 | User value clear (job traced) | ✓ | JTBD + 2 elevator pitches (`user-stories.md`) |
| 2 | Stories INVEST-sized | ✓ | 2 slices (`story-map.md`) |
| 3 | ACs testable | ✓ | Given/When/Then (`acceptance-criteria.md`) + dogfood for the gesture |
| 4 | Dependencies identified | ✓ | Reuses shipped /state + issue-edit-dialog (`requirements.md` table) |
| 5 | No unresolved questions | ◑ | DnD/mechanic/dialog-fold/realtime deferred to DESIGN (ODD-1..4) |
| 6 | Feasibility confirmed | ✓ | State backend shipped; normalize_state accepts slugs; columns are drop-target placeholders |
| 7 | Scope boundaries explicit | ✓ | Cross-column status only; no reorder/cancelled/priority-drag |
| 8 | NFR constraints stated | ✓ | One persist path, tenancy/CSRF, progressive enhancement, no migration, CSP-safe JS |
| 9 | Measurable outcome | ✓ | `outcome-kpis.md` |

**Verdict**: READY for DESIGN (required — DnD approach + card-move mechanic).
