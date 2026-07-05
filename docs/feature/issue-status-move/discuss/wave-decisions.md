# DISCUSS Decisions — issue-status-move

## Key Decisions
- [D1] Two mechanisms, ONE persist path: both the dialog status control (US-01) and drag-and-drop (US-02) write
  through the shipped `change_issue_state` / `/state` path. No second state-write.
- [D2] Two slices, dialog first: slice 01 (dialog status) ships the card-relocation mechanic and reuses the
  just-shipped issue-edit-dialog; slice 02 (DnD) reuses that mechanic + adds the app's first client JS.
- [D3] DnD is progressive enhancement: no-JS → no drag, and the dialog remains the no-JS status path. US-01's
  form keeps a plain-POST fallback.
- [D4] Reuse the board-new-issue/issue-edit-dialog modal infra unchanged (just one more control).
- [D5] No migration; tenancy/CSRF inherited from `change_issue_state`.
- [D6] Real DESIGN required: the DnD approach + the card-relocation mechanic + dialog-save-state folding are
  genuine architecture decisions (ODD-1..4). First client-side JS in the app.
- [D7] Repo multi-file convention; no SSOT; DES exempt.

## Requirements Summary
- Move an issue between statuses via the edit dialog (slice 1) and drag-and-drop (slice 2), reusing the shipped
  state backend; the card relocates to its new column.
- Walking skeleton: the state persist path ships; the NET-NEW pieces are the dialog control + card-move
  (slice 1) and the DnD JS (slice 2).
- Feature type: user-facing (UI; slice 2 adds client JS), brownfield.

## Constraints Established
- One persist path; no migration; tenancy/CSRF preserved; DnD is progressive enhancement; first app JS must be
  vendored/self-contained + CSP-safe.

## Scope Assessment: PASS
Right-sized as 2 slices. DnD (slice 2) carries the only real novelty (client JS) — DESIGN de-risks the approach.

## Handoff to DESIGN
Resolve ODD-1 (DnD approach: native HTML5 DnD + app JS vs SortableJS vs Alpine), ODD-2 (card-relocation
mechanic: server OOB move vs client DOM move + persist), ODD-3 (dialog: fold state into submit_edit vs post to
/state), ODD-4 (realtime broadcast or local-only). Plus the CSP implications of the first app JS.

## Upstream Changes
None. Brownfield; grounded in the shipped state/board/issue-edit code + the "until drag-and-drop" placeholder.
