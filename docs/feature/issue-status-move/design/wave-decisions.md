# DESIGN Decisions — issue-status-move

## Key Decisions (ratified 2026-07-05)
- [D1] DnD = native HTML5 DnD in a new app-owned `static/js/board-dnd.js` (no library). (ADR-001/ODD-1)
- [D2] Card relocation: dialog = server two-op OOB (delete old card by `id="issue-{key}"` + append to target
  column); DnD = client optimistic `appendChild` + revert-on-failure. (ADR-001/ODD-2)
- [D3] `submit_edit` reuses `edit_issue_details` + `change_issue_state` (state only when changed). (ADR-002/ODD-3)
- [D4] Realtime is free — keep the shipped outbox emit; moves broadcast via SSE. (ADR-002/ODD-4)
- [D5] Progressive enhancement: no-JS → no drag; the dialog is the no-JS status path. CSRF via `x-csrf-token`
  from the cookie for the DnD POST; tenancy inherited from `change_issue_state`. No migration.

## Architecture Summary
Slice 01: edit dialog gains a status `<select>`; `submit_edit` reuses the shipped edit + state paths and returns
the two-op OOB card-move (or in-place replace if state unchanged). Slice 02: a self-contained native-DnD JS
file drags cards between columns, persisting via the shipped `/state` (optimistic move + revert). Both write
through `change_issue_state`.

## Constraints for DISTILL/DELIVER
- One persist path; no migration; tenancy/CSRF; progressive enhancement; the JS is external/CSP-safe.
- Card gains `id="issue-{key}"` + `data-*` state URL/slug (reusing issue-edit-dialog's slugs+number).

## Handoff
DISTILL: author the acceptance SSOT (dialog status-move persist + card relocation + no-JS; DnD draggable/drop-
target wiring + drop-persist contract), gesture = dogfood. DELIVER: slice 01 then slice 02.
