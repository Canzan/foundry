# User Stories — card-ranking-within-status

## US-01 — Reorder cards within a status (slice 1)
**As a** member working the board (P1)
**I want** to drag a card up or down within its own status column and drop it at an exact slot
**so that** the column reads top-to-bottom in the order I plan to work it — not in creation order.

### Elevator Pitch
Before: within a column, cards are frozen in `number DESC` order and a drag only ever drops a card at the end.
After: drag GEN-2 above GEN-4 in Todo → it lands between them and stays there — for me and every other viewer.
Decision enabled: the column itself tells me (and the team) what's next, at a glance.

### Acceptance Criteria
- AC-01.1: A card can be dragged within its own column and dropped at a specific position (top, between any two
  cards, or bottom) — not only appended to the end.
- AC-01.2: On drop, the new intra-column order **persists**; a reload (and any other viewer's board) shows the
  same order.
- AC-01.3: The board read path orders each column by the persisted rank with a deterministic tiebreak (no
  flicker, no ambiguous order for equal ranks).
- AC-01.4: A failed/rejected position write **reverts** the card to its pre-drag slot (no false-positive
  reorder), consistent with the shipped move+revert behaviour.
- AC-01.5: Tenancy/CSRF preserved on the position write; a reorder targeting a foreign issue → uniform
  non-enumerable refusal (never a 500).
- AC-01.6: Existing boards keep their current order on first render after the migration (rank backfilled from the
  present `number DESC` order per status); a newly created issue appears at its defined default slot.
- AC-01.7: Progressive enhancement — without JS there is no reorder gesture, but the ranked order still renders
  for everyone (read path honors rank).

## US-02 — Drop a card at a precise position in another status (slice 2)
**As a** member working the board (P1)
**I want** to drag a card into a different status column and drop it at an exact slot
**so that** a single gesture both changes its status and places it exactly where it belongs in that column.

### Elevator Pitch
Before: dragging a card to another column changes its status but always drops it at the bottom of that column.
After: drag GEN-3 from Backlog and drop it between Todo's GEN-4 and GEN-2 → GEN-3 becomes Todo **and** sits
between them, in one motion.
Decision enabled: I triage straight into the right status *and* the right priority slot, without a second step.

### Acceptance Criteria
- AC-02.1: A card dropped into a different column lands at the exact drop position (top / between / bottom), not
  appended to the end.
- AC-02.2: One gesture atomically persists **both** the new state and the new rank; a reload (and other viewers)
  show the card in the new column at that position.
- AC-02.3: The exact scenario holds: GEN-3 in Backlog → dropped between Todo's GEN-4 and GEN-2 → GEN-3 is `todo`
  and ranked between GEN-4 and GEN-2.
- AC-02.4: A failed/rejected write reverts the card to its origin column **and** its origin slot (state and rank
  both revert together).
- AC-02.5: Reuses the shipped `change_issue_state` persist for the state part; tenancy/CSRF preserved; foreign
  issue → uniform non-enumerable refusal.
- AC-02.6: Progressive enhancement unchanged — no-JS keeps the edit-dialog as the status path (from
  `issue-status-move`); cross-status *positioning* is a JS-only enhancement, and the resulting order renders for
  everyone.
