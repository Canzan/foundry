# Acceptance Review — issue-status-move (DISTILL self-review)

| Criterion | Verdict | Note |
|-----------|---------|------|
| Every AC covered | ✅ | S1–S6 cover AC-01.1/.2/.3/.6 + AC-02.1/.2/.3 |
| Port-driven | ✅ | board + edit + /state endpoints |
| Honest harness boundary | ✅ | drag gesture + optimistic-move/revert = dogfood, per precedent |
| Negative path | ✅ | S6 invalid-state rejected, state unchanged |
| Reuse (one persist path) | ✅ | S5 hits the shipped /state; no new write tested |
| Lane safety | ✅ | all @pending |

## Watch-items for DELIVER
- **R1 card `id`**: slice 01 adds `id="issue-{key}"` for the OOB `delete` — ensure it doesn't collide and the
  DnD (slice 02) can also target it.
- **R2 OOB two-op**: the dialog move must DELETE the old card AND append a fresh one to the new column; assert
  BOTH in S2 (not just the append).
- **R3 CSRF for DnD**: the drop POST sends `x-csrf-token` from the `foundry_csrf` cookie — S5/S6 post the token
  the same way; confirm `csrf_middleware` accepts the header form.
- **R4 script wiring only**: S4 asserts the board LINKS `board-dnd.js` + the `draggable`/`data-*` markers; the
  actual DnD behaviour is dogfood — keep S4 to markers, not behaviour.

## Verdict
READY for DELIVER. Slice 01 (RED S2) then slice 02 (RED S4).
