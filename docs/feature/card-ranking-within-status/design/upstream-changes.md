# Upstream Changes — card-ranking-within-status (DESIGN → DISCUSS)

One clarification for the product owner / DISTILL. No scope change.

## UC-1 — "other viewers" convergence is on-reload in v1, not live push

**Original (DISCUSS `acceptance-criteria.md`, AC-01.2 / AC-02.2)**:
> AC-01.2 … "a reload (**and any other viewer's board**) shows the same order."
> AC-02.2 … "a reload (**and other viewers**) show the card in the new column at that position."

**Change**: ADR-002 resolves ODD-4 to a **state-only** realtime broadcast in v1 — the rank/position is not pushed
over SSE (mirroring the `update_issue_details` precedent, `lib.rs:1323`, of not emitting an event the SSE
consumer can't render). Therefore:

- The **actor** sees the reorder immediately (optimistic client move).
- **Other viewers** converge on the persisted order **on their next board load**. For a cross-status move they
  additionally see the live state change (card appears in the new column, at the column end) via the existing
  broadcast; the exact slot resolves on reload.

**Rationale**: the current SSE consumer has no position concept; broadcasting a reorder would relocate other
viewers' cards to the column end (a visibly wrong order). Live rank broadcast is a named deferred increment.

**Impact on acceptance (for DISTILL)**: verify "other viewers see the same order" as a **persisted re-read**
(reload / fresh board render), not a live cross-client push. The persist-contract + a reload assertion cover it;
do not write a live two-client SSE-position scenario for v1.

**DISCOVER impact**: none. **Story/slice scope**: unchanged (still US-01 + US-02, slices 01 + 02).
