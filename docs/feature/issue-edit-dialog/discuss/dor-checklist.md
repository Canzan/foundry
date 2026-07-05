# Definition of Ready — issue-edit-dialog

| # | DoR item | Status | Evidence |
|---|----------|--------|----------|
| 1 | User value clear (job traced) | ✓ | JTBD + elevator pitch (`user-stories.md`) |
| 2 | Story INVEST-sized | ✓ | 1 slice v1 (DESIGN may split) (`story-map.md`) |
| 3 | Acceptance criteria testable | ✓ | Given/When/Then + store scenarios (`acceptance-criteria.md`) |
| 4 | Dependencies identified | ✓ | Reuses board-new-issue modal + state/comment patterns (`requirements.md` table) |
| 5 | No unresolved open questions | ◑ | Backend shape questions deliberately deferred to DESIGN (ODD-1..4) |
| 6 | Technical feasibility confirmed | ✓ | Every mirror seam verified in-tree (state update, service, comment edit, card render) |
| 7 | Scope boundaries explicit | ✓ | v1 = title+description; state/priority/assignee/labels OUT |
| 8 | NFR constraints stated | ✓ | Tenancy, CSRF, no-JS fallback, no migration, validation bounds |
| 9 | Measurable outcome defined | ✓ | `outcome-kpis.md` |

**Verdict**: READY for DESIGN (required — net-new backend; ODD-1..4 to resolve).
