# ADR-BOARD-LANE-005: The column overflow menu is an arm of `closeTopLayer()`, not a component with its own listeners

## Status

Accepted (board-lane-overflow-menu DESIGN wave, 2026-09-02)

## Context

`board-lane-overflow-menu` replaces the per-column `×` with a `⋯` overflow menu
carrying four items. A menu is a dismissible layer: it must close on `Escape`, on
a click outside, and when one of its items is chosen.

This collides directly with a standing architectural rule. `brief.md`
§dialog-layers and `adr-modal-close-001` pin **BR-4**: `Escape` has exactly one
owner, `keyboard.js::closeTopLayer()`. A second `Escape` listener races the first
and peels two layers per press. The existing `@layered` acceptance scenario reds
precisely on that failure, and the codebase comment at `keyboard.js:131` names it.

`closeTopLayer()` is an ordered arm list — help → modal → search → no-op — whose
stack is **derived from the DOM on every press and never stored** (ADR-003 §2).
`modalIsOpen()` asks `#modal-root.childElementCount`, not a flag, specifically so
that htmx replacing the host cannot desync it.

The menu is also the first layer to live *inside* `#board-columns`, which the
lane-delete confirm replaces wholesale via an out-of-band swap.

## Decision

**The menu is a fourth arm of the existing `closeTopLayer()`. It registers no
listeners of its own.**

1. **Arm order:** `help → modal → MENU → search → no-op`. Menu and modal are
   mutually exclusive in practice (choosing an item closes the menu and lets htmx
   swap a dialog in), so the ordering is defensive — but it must be deterministic
   and covered by the `@layered` scenario shape that already guards the others.

2. **`menuIsOpen()` is DOM-derived.** It queries for the open menu element. There
   is **no** `var openMenu`. This is load-bearing, not stylistic: `#board-columns`
   is replaced wholesale by the OOB refresh, so a stored node reference survives
   as a *detached* element — `Escape` would then no-op while a menu sits on
   screen. That is exactly the failure ADR-003 §2 describes.

3. **Both click behaviours are branches of the existing delegated `click`
   listener** at `keyboard.js:870` — toggle on `[data-action="toggle-lane-menu"]`
   (via `closest()`, so a click on the glyph child resolves), and close-on-outside.
   No second `click` listener, exactly as `close-modal` added none.

4. **`closeMenu()` returns focus to the `⋯` trigger** of the menu it closed,
   resolved from the DOM.

5. **Menu open/close is client-side only** — no request, no server state (D11).

## Alternatives Considered

| Alternative | Rejected because |
|---|---|
| A popup library (Popper, Floating UI, …) | Registers its own key and outside-click handlers. That is BR-4's named failure, imported as a dependency. |
| `<details>`/`<summary>` as the primitive | Native toggle is appealing, but the open state lives in an attribute the OOB swap replaces, and `Escape` behaviour is browser-inconsistent. Deriving state via the existing arm is both simpler and correct. |
| A dedicated `keydown` listener scoped to the menu element | Still a second `Escape` handler on the propagation path. Scoping does not exempt it from BR-4 — two handlers, one press. |
| Track the open menu in a module variable | The OOB-swap desync above. The codebase already rejected the equivalent (`openLayers` array) at ADR-003 §2 for the same reason. |
| Reuse `#modal-root` to host the menu | Would make the menu a modal, blocking the board behind it for a four-item popup, and would put menu and dialog in one host — the shared-host design ADR-003 explicitly rejected. |
| Render the menu server-side on open | An extra round-trip for four static items, and it moves layer state to the server. |

## Consequences

- Adding a future board affordance (reorder, sort, WIP limits) is now a
  template + CSS change plus at most one arm — the menu generalises the way
  `close-modal` generalised dialog dismissal.
- The `@layered` scenario gains a fourth case, and a **new** browser scenario is
  required that DISCUSS did not specify: open a menu, trigger an OOB
  `#board-columns` refresh, press `Escape`. That is the stored-handle trap's
  oracle, and without it rule 2 is unenforced.
- `keyboard.js` continues to hold exactly one `keydown` and one `click`
  document listener. If a future reviewer counts more, this ADR has been violated.
- Focus return makes the menu the first board affordance with an explicit focus
  contract; ADR-006's board `listbox` posture is unaffected (the trigger is
  outside the `role="option"` card set).
