# ADR-001 — DnD via native HTML5 + card relocation (ODD-1, ODD-2)

**Status**: ACCEPTED (user-ratified 2026-07-05)

## Decision
- **DnD = native HTML5 Drag-and-Drop API** in a NEW app-owned `static/js/board-dnd.js` (loaded from base.html,
  `defer`). No library. Cards `draggable`; `[data-column]` sections are drop targets; drop → optimistic client
  `appendChild` into the column + POST the slug to the shipped `/state` (CSRF via the `x-csrf-token` header read
  from the `foundry_csrf` cookie); revert on non-2xx.
- **Card relocation** = server OOB (dialog): success returns `hx-swap-oob="delete"` on the old card (matched by
  a stable `id="issue-{key}"`) + `hx-swap-oob="beforeend:[data-column='{new}']"` appending a fresh card; empty
  primary closes the dialog. Client move (DnD): the JS relocates the DOM directly.

## Alternatives rejected
- **SortableJS** (vendored ~40KB): a new dependency; built for within-list reordering, more than column-to-
  column status moves need.
- **Alpine DnD**: fiddly hand-rolled handlers, mixes concerns; a dedicated tiny JS file is clearer.
- **Server-driven DnD (no client move)**: htmx has no native drag; a JS shim is required regardless, so the
  client optimistic move is the simplest snappy path.

## Consequences
First app-owned JS (self-contained, CSP-safe, same-origin). Cards gain a stable `id` + `data-*` for the state
URL/slug. Progressive enhancement: no-JS → no drag; the dialog is the no-JS status path.
