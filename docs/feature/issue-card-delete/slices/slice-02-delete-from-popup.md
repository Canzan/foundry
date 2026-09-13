# Slice 02 — Delete from the edit popup, board refreshes in place

**Story**: US-ICD-02 | **Estimate**: 1 day | **Depends on**: slice 01's write port

## Goal

The popup that every board card already opens can delete that card, and the
board behind it loses the card in the same paint that closes the dialog —
without putting a destructive control next to Save.

## IN scope

- A Delete control in `issue_edit_modal.html`, rendered **outside** the `<form>` beside the `×`, so it can never submit the edit form (D5). This is a markup contract, asserted as such — not a styling choice.
- Its `hx-get` of the confirm dialog into `#modal-root`, replacing the edit dialog in place.
- The popup-path success response: cleared `#modal-root` **plus** the out-of-band `#board-columns` refresh, through the shipped `partials/oob/board_columns_oob.html` (D7) — the same shape lane-delete success already returns.
- Handler branching on `is_htmx` (or the explicit split DESIGN picks — Pre-requisite 2) so one use case serves both surfaces and only the rendering differs (AC-2.9).
- `@needs-browser` scenarios: the full popup→confirm→board chain, `Escape` cancelling the confirm, and **real-browser CSRF** on the delete POST (AC-2.7 — the `fix-comment-delete-csrf` lesson: HTTP-lane token injection can mask a live 403).
- Regression coverage that the shipped edit-and-save path and its in-place card replace are byte-identical to today (AC-2.8).

## OUT of scope

- Any outbox emit or cross-viewer fan-out (slice 03).
- Any affordance on the board card itself (D12).
- Any change to the lane `⋯` menu, its items, or its order — only proof that the OOB refresh does not disturb them (AC-2.5).
- Changing the edit dialog's layout, copy or save behaviour beyond adding one control outside its form.

## Learning hypothesis

**Disproves, if it fails:** that the lane-delete OOB response shape generalises
from a *lane* operation to a *single-card* operation. Lane delete re-renders
`#board-columns` because the column set itself changed; a card delete re-renders
the same fragment for a change *inside* one column. If that turns out to
desync the `⋯` menu's `hidden` state, disturb card drag bindings, or force a
narrower fragment than `#board-columns`, this slice grows a targeted OOB swap
and the response shape stops being shared with lane delete.

**Confirms, if it succeeds:** `#board-columns` is the board's single refresh
unit for any mutation, and any future card-level operation (bulk delete, move,
a `/api/v1` write echoed to the browser) can reuse it without inventing a
fragment.

## Acceptance criteria

AC-2.1 … AC-2.9 (see `feature-delta.md` US-ICD-02).

## Production data

Real seeded board (Backend/auth, AUTH-41/42/43 across real lane rows), driven
through a real browser in the `@needs-browser` lane — the card is clicked, the
popup is the one the app actually serves, and the CSRF token is the one the
browser actually holds.

## Dogfood moment

Same day: delete a duplicate straight from the board without leaving it, and
watch the card vanish. This is the interaction the feature exists for; if it
does not feel immediate, that is a finding.

## Dependencies

Slice 01's delete write port and confirm dialog. The OOB refresh
(`board_columns_oob.html`), `#modal-root`, `closeTopLayer()` and the
`@needs-browser` lane are all shipped.
