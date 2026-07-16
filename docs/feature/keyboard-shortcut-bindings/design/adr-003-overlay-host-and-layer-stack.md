# ADR-003: The overlay host and the Esc layer stack; the no-JS help links stay (ODD-3, ODD-8)

## Status
Accepted — 2026-07-15 (Morgan, DESIGN wave). Feature-local. Resolves **ODD-3** (blocking, slice 01),
**ODD-8**, the `Esc`-vs-swap half of **ODD-5**, and **Risk R5**.

## Context
`?` and `Esc` are **global** on any signed-in page (BR-3, locked D2). The only modal mount is
`#modal-root` at **`board.html:13`** — inside the board's `app_content` block. `app_shell.html` (7 lines:
`extends base` + sidebar + `{% block app_content %}`) has **none**. So a global `?` has nowhere to render
on the dashboard or any non-board page. That is the question DISCUSS asked: hoist the mount into the shell,
or inject on demand?

**The question is less open than it looks — BR-4 answers it.** `#modal-root` is htmx's swap target with
`hx-swap="innerHTML"` (`board.html:6`, `issue_card.html:1`). Rendering the help fragment into `#modal-root`
would **replace whatever modal is open**. But US-07 requires:

> *Given Mei has the new-issue modal open and has pressed `?` … When Mei presses `Esc` … Then the help
> overlay closes **And the new-issue modal is still open***

Help and modals must therefore occupy **separate containers**. Any design that renders `?` into
`#modal-root` — including hoisting `#modal-root` into `app_shell.html` — fails AC-07.1 on the board, which
is the surface that matters most. ODD-3 is not a free choice between two mounts; the locked layering
forecloses one of them.

## Decision
1. **`keyboard.js` creates and owns its own overlay host.** On first use it appends
   `<div id="kb-overlay-root">` to `document.body`, on any page. `?` renders the shipped bare
   `section.keyboard-help[role="dialog"]` fragment (`keyboard_help.html:1`) into it. **`#modal-root` is
   untouched** — still board-only, still htmx's target, exactly as shipped. **Zero template delta.**
   Because the host is the last child of `<body>`, it stacks above `#modal-root` by DOM order (plus an
   explicit z-index), which *is* the layer precedence BR-4 wants.
2. **The layer stack is DERIVED from the DOM at `Esc` time, never stored.** `Esc` closes the topmost, one
   per press (BR-4):
   - `#kb-overlay-root` has content → clear it. Stop.
   - else `#modal-root` has content → clear its `innerHTML`. Stop.
   - else the search panel is open → close it, restore the board (ADR-005). Stop.
   - else **no-op** — never navigate, never touch `selectedKey`.
3. **ODD-8 — the full-page links STAY.** `sidebar.html:13` and `dashboard_root.html:32` are unchanged.
   Confirmed, not overturned.

## Alternatives Considered
- **Hoist `#modal-root` into `app_shell.html`** (the DISCUSS's first-named option) — REJECTED, and it is
  the option that looks most natural. Three reasons, the first fatal: (a) it does not actually solve the
  problem — one shared `innerHTML` target still cannot hold help *over* a modal (AC-07.1); (b) it is a
  template delta on the shared shell, so every signed-in page inherits a mount for a feature most of them
  do not use; (c) it would leave `board.html:13`'s mount to be removed, touching the shipped drag/modal
  paths for no gain. A JS-created host is strictly smaller *and* strictly more capable.
- **A second server-rendered mount (`#overlay-root` in `app_shell.html`)** — REJECTED. It solves the
  layering, but it is a template delta that adds markup to every page for a client-only concern, and it
  would breach the zero-server-delta property (D10/AC-X.4) for something JS can create in one line. If the
  overlay is JS-only anyway (with JS off there is no `?` at all), a JS-created host is the honest
  representation: the mount exists exactly when the thing that uses it exists.
- **Store the layer stack as an array (`openLayers.push(...)`)** — REJECTED, and this is the ODD-5 trap.
  htmx can replace `#modal-root`'s content — or a `hx-swap` can close a modal — entirely outside our
  knowledge. An array then claims a layer is open that is gone, and `Esc` becomes a silent no-op while the
  user stares at an open dialog. Derived state **cannot** desync from the DOM because the DOM *is* the
  state. This is the same discipline ADR-004 applies to the ring.
- **Render help via htmx (`hx-get` on a JS-created trigger)** — REJECTED. It would route the overlay
  through htmx's swap lifecycle and back into `#modal-root` semantics. A plain `fetch` + `innerHTML` into
  our own host keeps the overlay entirely inside the layer that owns it. (The route is public and returns a
  bare fragment — there is nothing htmx adds here.)
- **ODD-8 alternative: remove the full-page links now that `?` overlays** — REJECTED on three grounds:
  (a) they are the **no-JS path** (NFR-4) — with scripting off, `?` does not exist and the link is the only
  way to read the shortcut list; (b) removing them would make an advertised capability
  keyboard-and-JS-only, violating BR-6 (*no shortcut is the only path to any action*); (c) the dead-code
  policy does **not** reach them — unlike `#kb-items`, they have live consumers (no-JS users, pointer
  users, and the route is public *by design* precisely so help works before sign-in,
  `keyboard.rs:19-24`). Keeping them costs nothing and is what NFR-4 measures.

## Consequences
- **Positive**: AC-07.1 (layered `Esc`) works **on the board**, where both a modal and help can coexist —
  the case a single shared mount cannot express.
- **Positive**: zero template delta and zero risk to the shipped modal/drag paths. `#modal-root`,
  `board.html:6` and `issue_card.html:1` are byte-for-byte unchanged.
- **Positive**: `?` works on **every** page that loads `base.html` — including the sign-in page, which is
  precisely why `show_keyboard_help` is public by design (`keyboard.rs:19-24`). A consequence of the
  design, not a requirement, and consistent with the shipped intent.
- **Positive**: `Esc` cannot desync from reality, and it cannot clear selection (it only clears
  containers; `selectedKey` is a detached string — ADR-004). AC-07.3 needs no code.
- **Negative / accepted**: the layer precedence is fixed (help > modal > search), not a general z-order.
  Correct for the four layers that exist; if a fifth arrives, this rule is where it registers.
- **Negative / accepted**: clearing `#modal-root.innerHTML` is a blunt close. It matches what an htmx swap
  or a close control does, and no shipped modal has teardown state to run — but if one later does, `Esc`
  is a second close path that must be kept honest.
- **Probe (Earned Trust)**: the browser lane asserts the layered case directly — open the modal via `c`,
  press `?`, press `Esc`, assert **help gone AND `#modal-root` still populated**. That single scenario is
  what would red if anyone later "simplifies" the two hosts back into one.
