# Slice 03 — `c` files an issue without the mouse; `Esc` backs out of it

**Goal**: Mei spots a bug on the AUTH board, presses `c`, the shipped new-issue modal opens with the **title
already focused**, she types `"Session cookie not cleared on sign-out"`, submits, and the card appears. `Esc`
backs out at any point. Zero mouse actions.
**Stories**: US-03 (+ the modal half of US-07).

**IN scope**
- `c` (guarded by slice 02) on a surface with a **team+project context** → `GET
  /team/{team_slug}/project/{project_slug}/issues/new` with `HX-Request: true` → the bare fragment
  (`keyboard.rs:96-101`) swapped into `#modal-root` (`board.html:13`).
- **Reuse the page's own URL**, not a reconstructed one: `board.html:6` already carries the exact `hx-get` the
  "New issue" button uses. `c` triggers the **identical path** — one open path, no divergence (`project_context`).
- `Esc` closes the new-issue modal and returns Mei to the board unchanged (FR-5, BR-4); layered `Esc` closes the
  **topmost only** (help over modal → one press closes help, a second closes the modal).
- `c` on a page with **no project context** (the dashboard) does **nothing** — silently. No modal, no error, no
  navigation (BR-3, ODD-6). The route requires team+project (`keyboard.rs:62-95`).
- **No-JS untouched**: the server-side `HX-Request` fork (`keyboard.rs:96-104`) is not modified; the full-page
  fallback and the "New issue" button behave exactly as today (NFR-4, BR-6, R7).

**OUT of scope**: `/` (slice 04); `j`/`k`/`Enter` (slice 05); any change to the modal's markup, the form action,
or the CSRF handling; `Esc` for layers that don't exist yet.

**Learning hypothesis**: disproves *"`c` can drive the shipped htmx new-issue path — reusing the board's own
`hx-get` URL and `#modal-root` — with zero client CSRF work and zero server change, and `Esc` can dismiss the
result without disturbing the page beneath"* — if the client can't cleanly read `project_context` off the
existing markup (and would have to reconstruct the URL, risking disagreement with the button), if triggering the
swap programmatically diverges from what a real click produces, or if `Esc` handling collides with htmx's own
swap lifecycle over `#modal-root` (ODD-5).

> **Note on CSRF — a real seam worth stating.** Unlike `board-dnd.js` (`:126-133`) and `csrf-upload.js`
> (`:67-74`), which must mirror the `foundry_csrf` cookie into an `x-csrf-token` header for `fetch`, **this path
> needs no client CSRF work at all**: the server mints the cookie on the GET (`ensure_csrf_cookie`,
> `keyboard.rs:94`) and the fragment already carries `<input type=hidden name="_csrf">`
> (`new_issue_modal.html:4`). The form just submits. If DESIGN finds itself writing CSRF code here, something has
> gone wrong.

**Seams**: `show_new_issue_modal` (`keyboard.rs:62-110`, fragment/full-page fork `:96-104`, CSRF `:94`; route
`lib.rs:491-494`); `board.html:6` (the identical `hx-get`) + `:13` (`#modal-root`);
`partials/new_issue_modal.html:4` (`input[name=title][autofocus]`, hidden `_csrf`, action `…/issues`);
`partials/new_issue_modal_page.html` (the no-JS fallback); `lib.rs:487-490` (`…/issues` is **POST-only** — there
is no issue-list page, ODD-6).
**Dependencies**: slice 01 (dispatch layer), **slice 02 (hard — `c` is a character key and MUST NOT be bound
before the guard exists)**. DESIGN **ODD-6** (`c`'s surface scope), **ODD-5** (Esc vs htmx swap lifecycle),
**ODD-9** (browser driver).
**Effort**: ~1 day.
**KPI**: KPI-1 **2/7 → 4/7** (`c`, `Esc`). KPI-3 — **0** mouse actions to file an issue.
