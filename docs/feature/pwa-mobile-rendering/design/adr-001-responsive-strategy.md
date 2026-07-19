# ADR-001 — Responsive strategy: viewport meta + `@media`-bounded CSS on the existing surfaces

**Status**: Accepted (2026-07-19) · **Resolves**: ODD-1, ODD-2, ODD-5 · **Stories**: US-01, US-02

## Context

The app has no viewport meta and zero `@media` breakpoints — it's desktop-fixed. The frontend is
server-rendered askama (`base.html → app_shell.html → board.html`) with a single hand-hashed stylesheet. We
want mobile rendering without a mobile SPA, a new route, or a build step.

## Decision

**One viewport meta + additive `@media` rules on the existing markup.**

1. `base.html` `<head>` gains `<meta name="viewport" content="width=device-width, initial-scale=1">` — the
   precondition for any correct mobile render (US-01).
2. A mobile breakpoint (≤ ~640px) in the hashed stylesheet adds:
   - **Columns (ODD-1)**: `.board { overflow-x: auto; }` with `.column { min-width: … }` — the columns strip
     scrolls WITHIN its own container; the *page* never overflows. Keeps the kanban mental model.
   - **Nav (ODD-2)**: `.app-shell { flex-direction: column; }` at mobile so the sidebar reflows to a compact
     horizontal **top bar** — **CSS-only, no new JS** in v1. (A hamburger drawer is a follow-up; it would add
     JS for no v1-critical gain.)
   - **Modal (ODD-5)**: `.modal-dialog` becomes a **full-width scrollable sheet** (`width:100%`,
     `max-height:100vh`, body scrolls) so dialogs fit small screens.
   - **Tap targets**: primary controls (New issue, card open, nav items, dialog Save/Cancel) sized ≥ ~44px in
     the smaller dimension at mobile width (WCAG 2.5.5).
3. Desktop (> breakpoint) is untouched — every rule is `@media`-bounded.

**CSS hash-rotation (D5)**: each CSS change rotates `foundry.<hash>.css` in `base.html` AND `lib.rs:297`.

## Alternatives considered

- **A separate mobile route / SPA** — rejected: a whole new frontend for a server-rendered app; enormous cost,
  duplicated logic, and a Node toolchain the repo deliberately avoids.
- **A hamburger-drawer nav (JS)** — deferred: adds JS + state for v1; the CSS-only top-bar reflow gets the nav
  usable on a phone with zero new script. Drawer is a follow-up if the top bar proves cramped.
- **Stack the board columns vertically** — rejected for the default: loses the kanban side-by-side model and
  makes long boards very tall; horizontal-scroll-within-container preserves the mental model and the "page
  never overflows" invariant. (Revisit per phone dogfood.)
- **Inline styles / per-page CSS** — rejected: the app has one hashed stylesheet by design; keep it.

## Consequences

- Pure CSS + one meta tag; no markup rewrite (the responsive rules hook existing classes `.app-shell`,
  `.board`, `.column`, `.modal-dialog`). If a surface resists CSS-only responsiveness (e.g. the DnD board),
  that's the slice-02 learning hypothesis — flagged, not assumed.
- **Touch drag-and-drop is NOT in scope**: `board-dnd.js` must keep working (not break) at mobile width, but
  touch-DnD usability is a noted limitation, not a v1 goal (read/triage is the job).
- The `@media` bound must be correct or desktop regresses — asserted by keeping the shipped desktop
  `@needs-browser` scenarios green (AC-02.5) and a falsification against an unbounded rule.
- Invariant pinned regardless of the layout choices: **no page horizontal overflow at 390px; dialog ≤
  viewport** — so ODD-1/2/5 can be tuned at DELIVER/dogfood without changing acceptance.
