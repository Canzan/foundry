# Slice 01 — New issue button

**Goal**: clicking "New issue" on the board opens the modal, filing a title creates the issue, the card
appears in Backlog, and the modal closes.

**Story**: US-01.

**IN scope** (template/markup only — D5)
- `board.html`: give the "New issue" button `hx-get="…/issues/new"` + `hx-target="#modal-root"`
  `hx-swap="innerHTML"`; add a `<div id="modal-root"></div>` container.
- `partials/new_issue_modal.html`: give the form `hx-post="{{ action }}"` + `hx-target="#modal-root"`
  `hx-swap="innerHTML"`, keeping `method="post" action="{{ action }}"` and the hidden `_csrf` (D4).
- Acceptance glue: browser-driven scenarios (button→modal→create→card / empty-title error-in-modal / no-JS
  fallback), reusing the board/sign-in step helpers (`us_07`, `us_08`).

**OUT of scope**
- `c`/`Esc`/focus keyboard layer; drag; non-Backlog create; any `src/` change.

**Learning hypothesis**: disproves "button-only is pure htmx attribute wiring over the shipped OOB create" if
the close/error-in-modal interplay needs JS beyond htmx, or the plain-form fallback regresses.

**Acceptance**: `acceptance-criteria.md` US-01 scenarios + wiring assertions.

**Seams**: `board.html:5`; `partials/new_issue_modal.html`; `GET …/issues/new` (modal);
`POST …/issues` → OOB card (`issues.rs:293`); `NewIssueModal.csrf`.

**Dependencies**: none. **Effort**: ~0.5 day. **Reference class**: htmx fragment wiring, like the shipped
comment edit/delete swaps (`partials/comment_card.html`).
