# Slice 03 — Delete an empty lane

Story: US-BLM-03 | Estimate: 1 day | job_id: `job-board-lane-shaping`

## Goal

Each rendered lane carries a delete affordance. Deleting an **empty** lane is
a confirm-only dialog; confirmed deletion removes the lane from every surface.
A board can never drop below one lane (D6). Cards-in-lane fate handling is
slice 04.

## IN

- Per-lane delete control on the column header; htmx GET fetches the confirm
  dialog into `#modal-root`.
- Confirm dialog (empty-lane arm of D7): "Delete lane 'Todo'? It holds no
  issues. This cannot be undone." Confirm + × close. Close is the declarative
  `data-action="close-modal"` mechanism ONLY — no new Esc listener (BR-4,
  adr-modal-close-001).
- Delete-lane write, two arms this slice: refuse-if-last (422 + inline reason
  "A board needs at least one lane" into the dialog's `[data-error-slot]`);
  delete-empty (lane row removed; board fragment swapped, no full reload).
- New-issue landing follows the surviving leftmost lane (D6) — observable
  when Backlog itself is deleted.
- Authz: team membership, uniform non-enumerable 404 otherwise; `_csrf` on
  the trigger and the confirm form (D10).

## OUT

- Deleting a lane that holds ≥1 card (slice 04 — this slice's write refuses
  a non-empty lane defensively; the refusal shape is replaced in 04 by the
  fate dialog).
- Add/rename/reorder (feature OUT, D9).

## Learning Hypothesis

Lane deletion is a pure lane-data mutation: no issue rows change. If any
issue write is needed to delete an empty lane, the lane/issue seam from
slice 01 is mis-drawn.

## Acceptance Criteria

- [ ] Confirmed delete of empty Todo on "Homelab Ops": column gone without a
      reload, gone on reload, absent from dialog options and API validation.
- [ ] ×/Esc dismissal leaves the board untouched.
- [ ] Deleting the sole lane of "Scratch" is refused 422 with the inline
      reason; lane survives.
- [ ] After deleting Backlog on "Reading List", filing READ-4 lands it in
      In-Progress (new leftmost).
- [ ] Marco (non-member) delete POST → uniform 404, lane untouched; missing
      `_csrf` refused by middleware pre-handler.

## Dependencies

Slice 01 (lane data). Slice 02 independent but sequenced earlier.
