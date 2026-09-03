# Slice 02 — Edit list: rename a lane, touch nothing else

**Story**: US-BLO-02 | **Estimate**: 1 day | **Depends on**: slice 01 (menu is the only entry point)

## Goal

`⋯ → Edit list` opens a dialog pre-filled with the lane's current label; saving
re-renders the column under the new name with every card, key, URL and slug
provably unchanged.

## IN scope

- Lane rename write port: set `lanes.label` for one lane of one project (store → services → app).
- One lane-name validation seam (label 1–64, non-blank) enforced **below the adapter**, not only by the DB CHECK — shared with slice 03 per DD10 (Driving Port 3).
- Edit dialog template in `#modal-root`, declarative close, `_csrf` confirm POST, 422 → `[data-error-slot]`.
- OOB refresh of `#board-columns` on success, matching the delete confirm's shape.
- Team-membership gate; uniform non-enumerable 404 on both verbs for non-members.

## OUT of scope

- Renaming a lane **slug** — forbidden by inherited invariant (D4), not a deferral.
- Insert (slice 03), reorder, archive.
- Any change to `lanes.position`.

## Learning hypothesis

**Disproves, if it fails:** that label and slug are genuinely separable in the
running system — that the board, dnd targets, edit-dialog Status options,
`/api/v1` validation, report labels and 0013 event values really do read slug
for identity and label only for display. A failure means some surface is
label-keyed, which is a latent identity bug this slice would surface.

**Confirms, if it succeeds:** the `brief.md` §lanes claim is load-bearing in
fact, not just in prose — and slice 03 can mint a slug once and never revisit it.

## Acceptance criteria

AC-2.1 … AC-2.6 (see `feature-delta.md` US-BLO-02).

## Production data

Rename a seeded lane on a project holding real issues (OPS-3, OPS-7) and assert
`issues.state`, issue keys and card URLs unchanged by SQL, not by DOM alone.

## Dogfood moment

Same day: rename a real lane on the live board and drag a real card into it.

## Pre-slice SPIKE

None.
