# Technology Stack — board-lane-overflow-menu

## Nothing new is adopted

Every dependency this feature needs is already vendored, pinned and shipped.
No crate is added, no JS library is introduced, no build step changes.

| Layer | Technology | Status | Why not something else |
|---|---|---|---|
| Menu behaviour | **Vanilla JS in `keyboard.js`** | Shipped | The file is already the single owner of `Escape` (BR-4) and of one delegated `click` listener. A menu library would need its own key handling — the one thing this architecture forbids. |
| Dialogs | **htmx 2.0.4** (vendored) | Shipped | `hx-get` → `#modal-root`, `hx-post` → OOB `#board-columns`. Identical to the shipped delete dialog. |
| Templating | **Askama** | Shipped | `board_columns.html` is the shared partial; menu markup is authored once (D14). |
| Styling | **`foundry.<hash>.css`** on canzan tokens | Shipped | ADR-CANZAN-THEME-004: colour enters at one token seam. Menu must read correctly in both palettes. |
| HTTP | **axum** | Shipped | Routes mount under the existing `csrf_middleware` + `session_layer` stack, beside the delete route. |
| DB access | **sqlx** against **PostgreSQL 16** | Shipped | The insert transaction is plain SQL; no ORM, no new abstraction. |
| Slug minting | **`foundry-core`** pure fn | Extended | New `lane_slug` sibling to `slugify` — see `data-models.md` §5. |
| Tests | **cucumber** HTTP lane + **fantoccini** `@needs-browser` lane | Shipped | Menu interaction, focus return and error-slot routing need a real browser; contracts need the HTTP lane. |

## Version pinning note

The D8 spike ran against `postgres:16-alpine` — the exact tag `harness.rs:76`
pins, with the comment explaining why: testcontainers' default is `11-alpine`,
which would test a different major version than ships. The spike honoured that
pin, so its results describe production behaviour, not a lucky older planner.

## Explicitly rejected

| Option | Why not |
|---|---|
| A popup/menu JS library (Popper, Floating UI, etc.) | Would register its own key and outside-click handlers, racing `closeTopLayer()` — BR-4's named failure. The whole point of ADR-BOARD-LANE-005 is that the menu is an *arm*, not a component. |
| `<details>`/`<summary>` as the menu primitive | Native toggling is attractive, but its open state lives in an attribute the OOB swap would replace, and `Escape` handling is browser-inconsistent. DOM-derived state via the existing arm is both simpler and correct. |
| A server-rendered menu fetched on open | An extra round-trip for four static items, and it would put layer state on the server — D11 keeps menu opening client-side. |
| `SET CONSTRAINTS ALL DEFERRED` in the insert tx | Measured as unnecessary: `DEFERRABLE INITIALLY IMMEDIATE` already checks at end-of-statement. Adding it would imply a constraint that does not exist. |
| Migration 0016 relaxing the slug CHECK to `^[a-z0-9]` | Would let `2024_review` through without a prefix, but costs a migration to save a prefix the operator never sees. |
