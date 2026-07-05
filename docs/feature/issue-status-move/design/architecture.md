# Architecture — issue-status-move

Design for the DISCUSS requirements. Ratified ODD-1..4 (2026-07-05): native HTML5 DnD + a small app JS file;
dialog = server OOB card-move / DnD = client optimistic move; `submit_edit` reuses `edit_issue_details` +
`change_issue_state`; realtime is free via the shipped outbox. No migration; one persist path.

## The shared mechanic — relocate a card to a new column

State changes must MOVE the card between columns (the shipped `/state` returns only a chip). Two drivers:
- **Server-driven (dialog, slice 1)**: `submit_edit` success returns TWO out-of-band ops — (a) DELETE the old
  card (`hx-swap-oob="delete"` on the card, matched by a stable `id="issue-{key}"`), and (b) APPEND a fresh
  card to the target column (`hx-swap-oob="beforeend:[data-column='{new_state}']"`). Primary body empty → the
  `#modal-root` dialog clears/closes. When state is UNCHANGED, keep the in-place `outerHTML` replace
  (issue-edit-dialog behaviour).
- **Client-driven (DnD, slice 2)**: on drop, the JS `appendChild`s the dragged card into the target column
  immediately (optimistic), then POSTs; on a non-2xx it moves the card back to its origin.

Both persist through the shipped `change_issue_state` path (`normalize_state` accepts the `data-column` slug).

## Slice 01 — dialog status (server-driven)

- **View/template**: `IssueEditModal` + `partials/issue_edit_modal.html` gain a `state` `<select>` (Backlog/
  Todo/In-Progress/Done, current pre-selected via a `selected_state` field).
- **Handler `submit_edit`** (`issues.rs`): parse `state`; call `edit_issue_details` (title/desc, unchanged);
  if the submitted state differs from current, also call `issue_service::change_issue_state` (ODD-3 — reuses the
  shipped path, so the outbox fires → SSE broadcast, ODD-4). Build the response:
  - state changed → the two-op OOB card-move (delete old + append to new column) + empty primary (close dialog).
  - state same → the existing in-place `outerHTML` card replace.
  - validation/foreign → unchanged (ADR-003 uniform 404; "Title is required").
- **Card id**: `render_issue_card` / `issue_card.html` add a stable `id="issue-{key}"` so the OOB `delete` can
  target the old card. (Small addition alongside the issue-edit-dialog `hx-get`/`number`.)
- No new store/service method — reuse `edit_issue_details` + `change_issue_state`.

## Slice 02 — drag-and-drop (client JS, progressive enhancement)

- **`crates/foundry-app/static/js/board-dnd.js`** (NEW, app-owned, ~40–60 lines; loaded from `base.html` with
  `defer`): on load, mark every `.issue-card` `draggable="true"` and wire `[data-column]` sections as drop
  targets. `dragstart` → stash the card's key + its `…/issues/{number}/state` URL (from `data-*` on the card).
  `dragover` on a column → `preventDefault()` (allow drop). `drop` → optimistically `appendChild` the card into
  the column, then `fetch(stateUrl, {method:'POST', body: _csrf + state=<column slug>})`; on non-2xx, move the
  card back to its origin column. CSRF: read the `foundry_csrf` cookie and send it as the `x-csrf-token`
  header (csrf.rs supports `CSRF_HEADER`), so no server change.
- **Card data**: `issue_card.html` exposes the state-post URL + slug via `data-*` (reuse the team/project slugs
  + number already surfaced for issue-edit-dialog).
- **CSP-safe**: an external same-origin `/static/js/board-dnd.js` (like the vendored htmx/alpine), all wiring via
  `addEventListener` — no inline handlers. No new dependency.
- **No-JS**: without the script, cards aren't draggable and the board is unchanged; the dialog (slice 1) is the
  no-JS status path.

## Cross-cutting
- **One persist path**: both mechanisms → `change_issue_state` → `update_issue_state_with_outbox`. No new write.
- **Tenancy/CSRF**: inherited from `change_issue_state` + `csrf_middleware`; a drop/select for a foreign issue →
  uniform non-enumerable refusal. The DnD `x-csrf-token` header is the double-submit token from the cookie.
- **Realtime (ODD-4)**: the shipped outbox emit on state change already drives SSE — moves broadcast for free.
- **No migration.**

## Slice order
01 (dialog status; ships the server card-move + the `id="issue-{key}"`) → 02 (DnD; client move + the JS,
reusing the card data + the persist path). 01 de-risks the relocation before the novel JS.
