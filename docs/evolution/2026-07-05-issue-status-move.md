# Evolution — issue-status-move (move issues between status columns: dialog + drag-and-drop)

**Finalized**: 2026-07-05
**Commits**: DISCUSS `5f9a962` → DESIGN `f5f3964` → DISTILL `3276c85` → DELIVER `fe483b1` (slice 01) + `6feca50`
(slice 02) → finalize (this). Trunk-based; repo legacy multi-file convention; DES exempt. Feature dir PRESERVED.
**Wave coverage**: full pipeline WITH a real DESIGN — DISCUSS (2 stories → 2 slices) → DESIGN (ADR-001 native
DnD + card relocation, ADR-002 dialog-fold + realtime; ODD-1..4 user-ratified) → DISTILL (6-scenario SSOT) →
DELIVER (2 slices).
**Scope**: a board card couldn't move between the four status columns from the UI (they were "empty placeholders
until drag-and-drop", projects.rs:580). This adds TWO ways to move an issue between statuses — a status control
in the edit dialog, and drag-and-drop — both through the shipped `change_issue_state` path. No migration.

## Milestone — the board is a working kanban

Combined with `board-new-issue` (file) and `issue-edit-dialog` (edit), the board now supports the full loop:
**create → edit → move**. A member changes an issue's status by picking it in the edit dialog or by dragging
the card, and the card relocates to the right column and persists. This is also the app's **first client-side
JavaScript** (a self-contained native-DnD file).

## What shipped

### Slice 01 — status control in the edit dialog (`fe483b1`)
- `IssueEditModal` + `partials/issue_edit_modal.html` gain a status `<select>` (Backlog/Todo/In-Progress/Done,
  current pre-selected via `selected_state`); the store/service pre-fill read now returns `state`.
- `submit_edit` reuses the shipped `edit_issue_details` (title/desc) AND — only when the state actually changed
  (`view.state != new_state`) — `issue_service::change_issue_state` (fires the outbox → SSE, ODD-4). Response:
  **state changed** → a two-op OOB card relocation (`hx-swap-oob="delete"` on `#issue-{key}` + `beforeend` a
  fresh card into `[data-column='{new}']`), empty primary closes the dialog; **state unchanged** → in-place
  `outerHTML` replace (position preserved); **no-JS** → 303 to the board. NO new store/service method.
- The card gained a stable `id="issue-{key}"` for the OOB delete.

### Slice 02 — drag-and-drop (`6feca50`)
- **`static/js/board-dnd.js`** (NEW — the app's first client JS): native HTML5 Drag-and-Drop. Cards
  `draggable`; each `[data-column]` is a drop target. On drop: optimistic `appendChild` into the column, then
  `fetch` POST to the card's `data-state-url` (`…/issues/{n}/state`) with the target slug; **revert on
  non-2xx/network error**. Self-contained, external, CSP-safe (all `addEventListener`, no inline).
- **CSRF**: the drop sends `x-csrf-token` carrying the `foundry_csrf` cookie value — `csrf_middleware` already
  accepts `CSRF_HEADER`, so it authenticates with **no server change** and no CSRF weakening.
- `base.html` loads the script (`defer`); `issue_card.html` + `render_issue_card` carry `draggable` +
  `data-state-url` (threaded through all three OOB card wrappers, so dialog-relocated + newly-created cards are
  also drag-persistable).
- **Progressive enhancement**: no JS → no drag; the dialog (slice 01) remains the no-JS status path.

Both mechanisms reuse the SHIPPED `POST /issues/{n}/state` → `change_issue_state` → `update_issue_state_with_
outbox`. `normalize_state` accepts the column slugs. ONE persist path. No migration.

## Decisions realized (ODD-1..4, ratified)
| # | Decision | Status |
|---|---|---|
| ODD-1 | DnD = native HTML5 in a new app JS (no library) | IMPLEMENTED |
| ODD-2 | Card relocation: dialog = server two-op OOB; DnD = client optimistic move + revert | IMPLEMENTED |
| ODD-3 | `submit_edit` reuses `edit_issue_details` + `change_issue_state` (state only when changed) | IMPLEMENTED |
| ODD-4 | Realtime free — the shipped outbox emit broadcasts moves via SSE | IMPLEMENTED |

## Verification
- **DELIVER**: issue-status-move 6/6 (3 dialog + 3 DnD); regressions green — board-new-issue 5/5,
  issue-edit-dialog 6/6, us-08 10/10, us-b01 4/4; fmt + release clippy clean.
- **Finalize**: `cargo xtask ci` all gates green incl. the full `@all` acceptance lane (recorded at finalize).
- **Browser dogfood**: dialog → change status → card relocates to the new column + persists (GEN-4 → Todo);
  drag-and-drop → a real HTML5 drop moves the card + persists (GEN-1 → In-Progress, `state=in_progress` in DB).
  NOTE: the OS-level drag *gesture* isn't automatable (synthetic CDP mouse events don't trigger native HTML5
  DnD), so the handler was exercised via a genuine `dragstart`→`drop` DragEvent — the same events a mouse drag
  fires; a human drag uses the identical path.

## Deferred (out of scope)
Within-column reorder; the `cancelled` state (no column); priority/assignee via drag; multi-select; touch-drag
polish; a same-state save keeps the card in place (in-place replace) — cross-column moves only.
