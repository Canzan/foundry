# Slice 01 — The guarded dispatch layer + `?` help overlay + `Esc` closes it

**Goal**: the product stops lying. Mei presses `?` on the AUTH board and the shortcut list she was promised
appears **as an overlay over her board** — no navigation, no lost place. `Esc` dismisses it. This also stands up
the **document-delegated dispatch layer** every later slice reuses.
**Story**: US-01 (+ the overlay half of US-07).

**IN scope**
- A new external same-origin script under `crates/foundry-app/static/js/` (e.g. `keyboard.js`), loaded `defer`
  from `base.html:6-9` beside the vendored `htmx.min.js` / `alpine.min.js` and the app's `board-dnd.js` /
  `csrf-upload.js`. **No inline handlers, no CDN** (NFR-5). Vanilla IIFE vs Alpine = **ODD-2**.
- **One `document`-delegated `keydown`** — the `board-dnd.js:67` dragstart idiom, which exists precisely so
  htmx-appended content needs no re-wiring (NFR-6, FR-10).
- The **dispatch table keyed on `SHORTCUTS`** (`keyboard.rs:48-56`) — the same constant that renders the help
  `<dl>`, so bound-set and advertised-set cannot drift (BR-1).
- `?` (`Shift+/`, BR-7) → `GET /keyboard-help` (**public by design**, `keyboard.rs:19-24`, route `lib.rs:536`)
  → render the returned bare `section.keyboard-help[role="dialog"]` (`keyboard_help.html:1`) as an **overlay**,
  URL unchanged (FR-4).
- `Esc` closes the help overlay (BR-4); with nothing open it is a **no-op**, never a navigation (FR-5).
- **Resolve ODD-3**: `#modal-root` exists **only** at `board.html:13`; `app_shell.html` (7 lines) has none — a
  **global** `?` has no mount on non-board pages. Hoist the mount into `app_shell.html` or inject on demand.
- Keep the full-page `/keyboard-help` links (`sidebar.html:13`, `dashboard_root.html:32`) as the no-JS path
  (ODD-8, NFR-4).

**OUT of scope**: the text-input/modifier guards as a proven litmus (slice 02 — **no character key is bound
here**, so the guard gap cannot bite yet); `c` (slice 03); `/` (slice 04); `j`/`k`/`Enter` + the `#kb-items`
retirement (slice 05); `Esc` for modals that don't exist yet.

**Learning hypothesis**: disproves *"a single document-delegated keydown layer, keyed on the shipped `SHORTCUTS`
constant, can render the shipped `/keyboard-help` fragment as an in-place overlay on **any** signed-in page and
dismiss it with `Esc`, without a new route and without touching no-JS"* — if the bare fragment can't be overlaid
without a mount point (`#modal-root` is board-only, **ODD-3**), if Alpine and the vanilla house pattern can't
coexist on one `keydown` (**ODD-2**), or if `defer` ordering against htmx/Alpine bootstrap makes the binding
unreliable.

**Seams**: `SHORTCUTS` (`keyboard.rs:48-56`); `show_keyboard_help` (`keyboard.rs:259-279`, route `lib.rs:536`);
`partials/keyboard_help.html:1`; `base.html:6-9`; `app_shell.html`; `#modal-root` (`board.html:13`);
`board-dnd.js:17,67,149-154` (the delegation + IIFE idiom); `sidebar.html:13`, `dashboard_root.html:32`.
**Dependencies**: DESIGN **ODD-2** (vanilla vs Alpine — and correct `keyboard.rs`'s stale "alpine.js" doc,
R9), **ODD-3** (the global mount point — **blocking for this slice**), **ODD-8** (fate of the full-page links),
**ODD-9** (the harness cannot press keys — this slice needs the browser driver first).
**Effort**: ~1 day (the layer + the mount decision carry the slice; the fragment and route are shipped).
**KPI**: KPI-1 advertised-to-working **0/7 → 2/7**.
