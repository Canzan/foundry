# Slice 02 — Responsive surfaces (board, dialogs, nav)

**Goal**: make the primary surfaces usable with a thumb at phone width — board columns scroll within their
container, dialogs fit as scrollable sheets, the nav collapses, primary controls are tappable.

**Story**: US-02. **Depends on**: slice 01 (the viewport meta + the mobile-window oracle).

**IN scope**
- Responsive `@media` rules in the hashed stylesheet (ROTATE the hash ×2): board columns container
  horizontally scrollable (ODD-1); modals full-width scrollable sheet ≤ viewport (ODD-5); sidebar/nav collapses
  to a mobile affordance (ODD-2); tap targets ≥ ~44px at mobile width.
- fantoccini scenarios (mobile window) asserting: dialog ≤ viewport + body scrolls; columns container scrolls
  while the page doesn't; nav mobile affordance present; New-issue control box ≥ ~44px; **desktop layout
  unchanged**.
- Un-@pend US-02 scenarios.

**OUT of scope**
- The manifest/icons/installability (slice 03); offline/SW; swipe/touch gestures; a separate mobile route.

**Learning hypothesis**: disproves **"responsive CSS alone makes the shipped server-rendered surfaces usable
on a phone"** if the board's column layout or a modal needs markup/JS changes (not just `@media`) to behave —
e.g. the DnD board or the `#modal-root` sizing resists CSS-only responsiveness. Confirms it if `@media` rules
suffice and desktop stays byte-identical.

**Acceptance**: `discuss/acceptance-criteria.md` US-02.

**Seams**: the hashed stylesheet + `lib.rs:297`; `board.html` / the columns markup; `#modal-root` + `.modal`
(`new_issue_modal.html`, `issue_edit_modal.html`); the sidebar/nav (`partials/sidebar.html`, base layout);
`board-dnd.js` (must keep working at mobile width — don't break drag).

**Falsification**: each responsive scenario RED before its rule lands (dialog overflows / columns overflow the
page / nav shows full rail). The **desktop-unchanged** scenario RED against an over-broad `@media` (no min/max
bound) that leaks mobile rules into desktop.

**Watch items**
- The DnD board (`board-dnd.js`) must still function at mobile width — responsive CSS must not break drag
  targets; if drag is unusable on touch, that's a noted limitation (touch-DnD is OUT), not a blocker for
  read/triage.
- `@media` bounds must be correct so desktop (shipped `@needs-browser` scenarios) stays green.
- CSS hash rotation ×2.

**Dependencies**: slice 01. **Effort**: ~1–1.5 day (the bulk of the visual work + phone dogfood).
