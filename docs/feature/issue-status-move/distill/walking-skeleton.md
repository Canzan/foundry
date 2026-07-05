# Walking Skeleton — issue-status-move

The persist path ships (`change_issue_state`). Slice 01's NET-NEW piece is the server card-RELOCATION mechanic
(the two-op OOB move) + the dialog control; slice 02's is the native-DnD JS. 01 ships the relocation 02 reuses.

## First failing test (DELIVER entry) — slice 01
**S2 — "Saving a new status relocates the card to that column"**.
RED→GREEN: add the status `<select>` to `IssueEditModal`; `submit_edit` reuses `edit_issue_details` +
`change_issue_state` (when changed) and returns the two-op OOB (delete old card by `id="issue-{key}"` + append
to `[data-column='{new}']`); add `id="issue-{key}"` to the card render. S1 (control pre-set), S3 (no-JS) green.

## Slice 02
S4 (draggable/drop-target/script wiring) + S5/S6 (drop-persist contract). GREEN: add `static/js/board-dnd.js`
(native DnD, optimistic move + revert, CSRF via `x-csrf-token` cookie), load it from `base.html`, add card
`data-*` (state URL/slug). Then DOGFOOD the drag gesture.

## Lane safety
All @pending (excluded by filter_run); @all green until DELIVER. Full @all at finalize.
