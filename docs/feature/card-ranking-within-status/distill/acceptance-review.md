# Acceptance Review — card-ranking-within-status (DISTILL self-review)

| Criterion | Verdict | Note |
|-----------|---------|------|
| Every AC covered | ✅ | S1–S10 cover AC-01.1/.2/.3/.5/.6/.7 + AC-02.1/.2/.3/.4/.5 |
| Port-driven | ✅ | board GET + `/state` POST (with `after`) + the real create path (S6) |
| Honest harness boundary | ✅ | drag gesture + optimistic move/revert = dogfood, per issue-status-move precedent |
| Negative paths | ✅ | S4 unknown-`after` refuse, S5 foreign refuse, S10 invalid-state inert |
| Reuse (one write path) | ✅ | S2/S3/S8/S9 all issue the SAME `/state` + `after` request; no second endpoint tested |
| Realtime scoped to v1 | ✅ | no live two-client SSE-position scenario; S7 verifies order as a persisted re-read (UC-1) |
| Lane safety | ✅ | all `@pending` |

## Watch-items for DELIVER
- **R1 contiguity invariant**: after ANY reorder/drop, BOTH the source `(project, old_state)` and target
  `(project, new_state)` columns must be a contiguous `0..N-1` permutation — reindex both in the SAME tx. S2/S8
  assert the observable order; add a store-level position check if the reindex is non-trivial.
- **R2 conditional emit (ADR-002 D5)**: a within-status reorder (S2/S3) must write `position` but emit NO
  `IssueUpdated` outbox row (state unchanged); a cross-status drop (S8/S9) emits exactly one (state changed).
  Broadcasting a pure reorder would shove other viewers' cards to the column end. Guard this in the store method.
- **R3 unknown-`after` = refuse, not top-drop (S4)**: resolve `after` within the target `(project, state)`; an
  unresolvable key → uniform non-enumerable refusal, NOT a silent top placement (a stale client must not mis-drop).
- **R4 new-issue slot (S6)**: `insert_issue_with_outbox` must place the new card at Backlog `position 0` and shift
  the column `+1` IN THE SAME TX — else the contiguity invariant breaks on create.
- **R5 zero-shuffle backfill (S1)**: the `0012` backfill must reproduce `number DESC` per `(project, state)`;
  verify a pre-existing board's first render is unchanged (dogfood on the sandbox board).
- **R6 request shape (S2 vs S8)**: same `state`+`after` body for within- and cross-status — the ONLY difference
  is the card's origin state. Keep `capture_drop_post` single; don't fork it per slice.
- **R7 seed positions**: the multi-issue Background seed must leave columns in a deterministic default order
  (number DESC) so the pre-reorder assertions (S1) and the post-reorder assertions (S2/S8) are stable.

## Verdict
READY for DELIVER. Slice 01 (RED **S2**) ships the rank machinery; slice 02 (RED **S8**, the GEN-3 anchor)
extends the gesture cross-column. All `@pending` until wired.
