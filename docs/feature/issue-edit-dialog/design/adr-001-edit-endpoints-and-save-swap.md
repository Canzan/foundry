# ADR-001 — Edit endpoints + save swap (ODD-1, ODD-2)

**Status**: ACCEPTED (user-ratified 2026-07-05)

## Context
The edit dialog needs a GET (open, pre-filled) and a save. The board card must update in place on save, and a
no-JS fallback must still work.

## Decision
- **GET `…/issues/{n}/edit`** returns the `IssueEditModal` fragment; **POST `…/issues/{n}/edit`** saves.
  POST (not PATCH) because the no-JS fallback is a native `<form method="post">`, which browsers cannot PATCH.
  htmx `hx-post`s the same URL.
- **Save response (htmx)**: an OOB `outerHTML` swap of the card, keyed on `data-issue-key`, carrying the new
  title; the primary response body is empty so the `#modal-root` target (reused from board-new-issue) clears
  → the dialog closes. **Save response (no-JS)**: `303` → the board (the card re-renders on reload).

## Alternatives rejected
- `PATCH …/issues/{n}`: breaks the no-JS fallback (forms can't PATCH).
- Full board refresh on save: heavier than an in-place card replace; loses scroll/focus.

## Consequences
Mirrors the board-new-issue create swap and the comment inline-edit flow. The card render must expose the
issue `number` for the edit URL (a small card-view field addition).
