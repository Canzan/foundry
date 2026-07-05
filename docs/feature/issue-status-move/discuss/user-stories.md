# User Stories — issue-status-move

## US-01 — Change an issue's status from the edit dialog (slice 1)
**As a** member editing an issue (P1)
**I want** a status control in the edit dialog
**so that** I can move the issue to another column while I'm already editing it.

### Elevator Pitch
Before: the edit dialog edits title + description only; status can't be changed from the UI.
After: open a card's dialog → pick a status (current one pre-selected) → Save → the card moves to that column.
Decision enabled: I set where the work stands as I edit it.

### Acceptance Criteria
- AC-01.1: The edit dialog shows a status control with the four states, the issue's current state pre-selected.
- AC-01.2: Saving with a changed status persists the new state (via the shipped `change_issue_state` path).
- AC-01.3: On save, the card relocates to the matching column (and leaves its old column); dialog closes; no reload.
- AC-01.4: Saving with the SAME status leaves the card where it is (title/description still update).
- AC-01.5: Tenancy/CSRF/validation preserved; foreign issue → uniform non-enumerable refusal.
- AC-01.6: No-JS fallback — the dialog form is a plain POST that saves state + returns the board with the card moved.

## US-02 — Drag a card between columns (slice 2)
**As a** member working the board (P1)
**I want** to drag an issue card into another status column
**so that** I can update its status with a gesture, without opening a dialog.

### Elevator Pitch
Before: cards are static; columns are empty placeholders.
After: drag a card from Backlog to Todo → it lands in Todo and its status persists.
Decision enabled: I triage the board by dragging, fast.

### Acceptance Criteria
- AC-02.1: A card can be dragged and dropped into any of the four columns.
- AC-02.2: On drop, the card lands in the target column and its state persists (via the shipped `/state` path).
- AC-02.3: The drop posts the target column's slug (`normalize_state` accepts it); a rejected/failed persist
  reverts the card to its original column (no false-positive move).
- AC-02.4: Tenancy/CSRF preserved on the drop persist.
- AC-02.5: Progressive enhancement — without JS the board is unchanged (no drag); the dialog (US-01) remains
  the no-JS way to change status.
