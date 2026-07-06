# ADR-002 — Persist path (`/state` + `after`), wire format, and v1 realtime

**Status**: Accepted (user-ratified 2026-07-06) · **Feature**: card-ranking-within-status · **Slices**: 01 + 02

## Context

Both slices set a card's position; slice 02 also changes its state. We need one write path, a drop→position wire
format, and a realtime story that doesn't corrupt other viewers' boards.

## Decision

### 1. Extend `POST /issues/{n}/state` (ODD-2)
`ChangeStateForm` gains `after: Option<String>` (serde `default`). `board-dnd.js` **always** sends
`state=<target-slug>&after=<neighbour-key>`, whether the drop is within the same column or into another:
- **within-status** (slice 01): `state` = the card's current state, `after` = new neighbour → position-only write.
- **cross-status** (slice 02): `state` = the target column, `after` = new neighbour → state + position, atomic.

One endpoint, **one request shape for both slices** — slice 02 is then nearly free (JS already computes `after`;
the server already handles state+position). No second write path → tenancy/CSRF wiring is inherited, not
duplicated (D5). The success response is unchanged (a state chip); the DnD client only checks `response.ok`, so
the response body is irrelevant to the gesture, and the no-JS edit-dialog status path (`issue-status-move`) is
untouched.

### 2. Wire format = neighbour issue key (ODD-3)
On drop, `board-dnd.js` reads the `data-issue-key` of the card immediately **above** the dropped card in the
target column and sends `after=<key>` (absent/empty ⇒ dropped at top). The server resolves `after` → its
`position` in the target `(project, state)`; target index = `after.position + 1`.
- **Why neighbour key over a client index**: race-robust (the client needn't know absolute indices), and it
  reuses the card's existing `data-issue-key` — **no new card attribute** (`render_issue_card`, `issues.rs:532`).
- **Unknown `after` key** in the target column ⇒ **non-enumerable refusal** (recommended) rather than a silent
  top-drop, so a stale client can't mis-place a card. (DELIVER confirms; DISTILL asserts.)

### 3. Realtime = state-only broadcast in v1 (ODD-4)
The position/rank is **not broadcast**. The `IssueUpdated` outbox row is emitted **iff the state actually
changed** (cross-status). A pure within-status reorder writes `position` with **no outbox emit**.

This mirrors the established precedent in `update_issue_details` (`lib.rs:1323-1332`): *"we do not surprise the
SSE consumer with an event kind it cannot render yet."* The current SSE consumer relocates a card to the target
column but has no notion of position — broadcasting a reorder would move other viewers' cards to the column
**end** (a wrong reorder on their screens). So:
- **cross-status move**: state broadcasts live as today; other viewers see the card in the new column (at the
  end) live, and at the correct slot on their next board load.
- **within-status reorder**: local to the actor; other viewers converge on next board load.

Consequence for the store: the reposition writer must emit **conditionally** — hence the sibling to
`update_issue_state_with_outbox` (Reuse Analysis) rather than reusing its unconditional emit.

Broadcasting rank live (extending the SSE payload + consumer to carry/apply position) is a **named deferred
increment**.

## Consequences

- `board-dnd.js` gains after-key computation but keeps optimistic-move + revert + the `x-csrf-token` header.
- `AC-01.2` / `AC-02.2` ("other viewers show the same order") are satisfied **on the viewer's next load** in v1,
  not via live push — see `upstream-changes.md` for the wording tightening handed to DISTILL.
- Foreign/missing issue OR unknown `after` key → uniform non-enumerable refusal, never a 500 (ADR-003 lineage).
