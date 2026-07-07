# Definition of Ready — issue-change-history

| # | DoR item | Status | Evidence |
|---|----------|--------|----------|
| 1 | User value clear (job traced) | ✓ | JTBD + 3 personas → 3 surfaces; 4 elevator pitches (`user-stories.md`) |
| 2 | Stories INVEST-sized | ✓ | 4 slices, model-first; each end-to-end (`story-map.md`) |
| 3 | ACs testable | ✓ | Given/When/Then (`acceptance-criteria.md`); HTTP/store contract + report/CSV + dogfood for render polish |
| 4 | Dependencies identified | ✓ | Reuses the durable outbox precedent, the 4 issue write paths, the `/api/v1` router, the comments pattern (`requirements.md` seams) |
| 5 | No unresolved questions | ◑ | History store, capture mechanism, timeline home, report shape, genesis, program envelope deferred to DESIGN (ODD-1..6) |
| 6 | Feasibility confirmed | ✓ | A durable append-only outbox already records issue events; write paths hold old+new in-tx; API + render + tenancy patterns all shipped |
| 7 | Scope boundaries explicit | ✓ | Records changes only (does NOT add priority/assignee editing); append-only; no live push; no retention policy; comments/attachments out of the timeline v1 (`requirements.md`) |
| 8 | NFR constraints stated | ✓ | In-tx capture (no phantom/drop); append-only immutable; one model→three surfaces; field-agnostic; tenancy/non-enumerability; no realtime regression |
| 9 | Measurable outcome | ✓ | `outcome-kpis.md` |

**Verdict**: READY for DESIGN (required — the durable change-event **model** that must serve all three surfaces,
the reuse-outbox-vs-dedicated-table call, the in-tx capture across four write paths, and the timeline's home are
genuine architecture decisions, ODD-1..6).
