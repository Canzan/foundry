# Slice 02 — Drag-and-drop between columns

**Goal**: drag a card into another column → it lands there and its state persists.
**Story**: US-02.

**IN scope**
- Cards `draggable`; columns are drop targets (per `data-column` slug).
- The app's FIRST client JS (DESIGN ODD-1: native HTML5 DnD + a small app JS file vs vendored SortableJS vs
  Alpine) — self-contained, CSP-safe, vendored/app `/static` file loaded from `base.html`.
- On drop: optimistic client move + POST the target slug to the shipped `/state` endpoint; on failure, revert
  the card to its origin (ODD-2).
- Progressive enhancement: no-JS → no drag (board unchanged).
- Acceptance: the drop-persist contract (endpoint) + draggable/drop-target wiring (scraper); the gesture is
  browser-dogfooded.

**OUT of scope**: within-column reorder; touch polish; multi-select; realtime broadcast.

**Learning hypothesis**: disproves "DnD is a small self-contained client JS over the shipped /state persist" if
the DnD approach or optimistic-move+revert is heavier/flakier than expected.

**Seams**: `board.html`/`issue_card.html`; the shipped `/state` endpoint + `normalize_state`; the slice-01
card-relocation mechanic; a new `/static` JS file.
**Dependencies**: slice 01 (card-move mechanic) + DESIGN ODD-1/ODD-2. **Effort**: ~1–1.5 days (novel JS).
