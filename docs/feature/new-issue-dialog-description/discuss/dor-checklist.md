# Definition of Ready — new-issue-dialog-description

| # | DoR item | Status | Evidence |
|---|----------|--------|----------|
| 1 | User value clear (job traced) | ✓ | J1/J2 + four forces + elevator pitches (`user-stories.md`); every story carries a `job_id` |
| 2 | Story INVEST-sized | ✓ | 3 stories → 3 slices, each ≤1 day, each independently shippable (`story-map.md`) |
| 3 | Acceptance criteria testable | ✓ | Given/When/Then + store + cross-feature scenarios (`acceptance-criteria.md`) |
| 4 | Dependencies identified | ✓ | Seam table verified in-tree with file:line (`requirements.md`); slice 02/03 depend on 01 |
| 5 | No unresolved open questions | ◑ | ODD-1..4 deliberately deferred to DESIGN (bound value, serde shape, error copy, echo-back field). None blocks slicing. |
| 6 | Technical feasibility confirmed | ✓ | Every layer read in-tree: the gap is 2 templates + 2 view models + 1 form + 2 signatures. `description_md` exists and edit writes it — no migration. |
| 7 | Scope boundaries explicit | ✓ | IN/OUT per slice; markdown preview, priority/assignee/labels, backfill, "created" event kind all OUT |
| 8 | NFR constraints stated | ✓ | Tenancy, CSRF, no-JS fallback (both templates), rule-parity NFR-WEB-API-CON-02, no migration (`0014`), US-R07 |
| 9 | Measurable outcome defined | ✓ | `outcome-kpis.md`, incl. falsification counter-metrics |

## Notes on item 5

The four ODDs are genuine DESIGN calls, not deferred DISCUSS work:

- **ODD-1** (`DESCRIPTION_MAX_LEN` value) needs a look at real `description_md` sizes — a product-safety
  number, not a requirements question. It gates slice 03 only.
- **ODD-2** (serde shape) and **ODD-3** (error copy/fragment) have obvious mirrors in shipped code; DESIGN
  ratifies rather than invents.
- **ODD-4** (does `NewIssueModal` need a `description` field to echo failed input back) is the one with real
  design content, and AC-01.6 already pins the required *behavior* regardless of the mechanism.

## Verdict

**READY for DESIGN** (required — the change crosses store/service/API boundaries and touches a shipped
validation rule; ODD-1..4 to resolve).

**Not ready to skip to DISTILL**: D2 changes shipped edit-path behavior and D1 touches the public API
contract. Both warrant an ADR.
