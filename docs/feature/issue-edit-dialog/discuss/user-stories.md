# User Stories — issue-edit-dialog

## US-01 — Edit an issue's title + description from a dialog

**As a** workspace member on the board (P1)
**I want** to click an issue card and edit its title and description in a dialog
**so that** I can keep the board accurate without leaving it.

### Elevator Pitch
Before: clicking an issue card does nothing; title/description can't be edited anywhere.
After: click a card → an edit dialog opens pre-filled with the current title + description → change them →
**Save** → the board card updates in place and the dialog closes.
Decision enabled: I decide to correct the issue and see it reflected immediately.

### Acceptance Criteria
- AC-01.1: Clicking an issue card opens an edit dialog pre-filled with the issue's current `title` and
  `description_md`.
- AC-01.2: Editing the fields and Saving persists both `title` and `description_md` for that issue (scoped by
  the acting workspace).
- AC-01.3: On save, the board card for that issue updates in place to show the new title, and the dialog
  closes — no full-page reload.
- AC-01.4: Saving an empty title is rejected in the dialog ("Title is required"); nothing is persisted.
- AC-01.5: A title over 256 chars / description over the schema limit is rejected (mirrors the create bounds).
- AC-01.6: A member cannot open/save the edit dialog for an issue outside their acting workspace — it returns
  the uniform non-enumerable 404 (ADR-003), same as the shipped detail path.
- AC-01.7: CSRF-protected save; a forged token is refused, nothing persisted.
- AC-01.8: No-JS fallback — with htmx unavailable, the dialog form is a plain POST that saves and returns to
  the board showing the updated card.

## US-02 — (deferred, documented for scope) Edit state / priority / assignee

Out of scope for v1; recorded so DESIGN sizes the dialog for later fields. The schema already carries `state`,
`priority`, `assignee_id`; a later increment adds their controls to the same dialog + their update paths.
