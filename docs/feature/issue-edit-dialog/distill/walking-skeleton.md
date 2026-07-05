# Walking Skeleton — issue-edit-dialog

The modal infra ships (board-new-issue `#modal-root` + `.modal`). The NET-NEW load-bearing piece is the
title/description update path (store `update_issue_details` + service `edit_issue_details`) — that IS the
skeleton of slice 01.

## First failing test (DELIVER entry)
**S3 — "Saving edits the issue and replaces the board card in place"** (drives the whole vertical).

RED → GREEN:
1. RED store test: `update_issue_details` persists title+description (fails — method absent).
2. RED S1/S2: card `hx-get`; pre-filled dialog (fail — card static, no endpoint/view).
3. GREEN: store `update_issue_details` (mirror `update_issue_state_with_outbox` minus outbox) → service
   `edit_issue_details` (mirror `change_issue_state`) → handlers `show_edit_form` + `submit_edit` → view
   `IssueEditModal` → routes → card gains `hx-get` + the issue `number` in the card-view + OOB-replace on save.
4. S4 (empty-title error), S5 (foreign non-enumerable), S6 (no-JS 303) green.
5. fmt + release clippy; commit. Then DOGFOOD the live click→edit→save→card-updates.

## Slice sequence
One slice (`slice-01-edit-title-description`); DELIVER may order it store→service→web→acceptance internally.

## Lane safety
All @pending (excluded by `filter_run`); @all stays green until DELIVER. Full @all at finalize.
