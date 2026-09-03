# RCA — the lane ⋯ menu was unusable on a phone

**Reported**: 2026-09-03, with a screenshot from a real device over Tailscale.
**Reporter's words**: "the rendering of the … and the menu are incorrect. The menu
should show up over the top of the list and the … should be easier to see and click."

Two independent defects, both reproduced by measurement before anything changed.

## Defect A — the menu was clipped, not layered wrong

**Root cause**: `.board { overflow-x: auto }` inside the `@media (max-width: 480px)`
block, shipped by `pwa-mobile-rendering` so the column strip scrolls without
widening the page. An element whose `overflow` is not `visible` becomes a
**clipping container for its absolutely-positioned descendants**, and the menu's
containing block (`.column`) sits inside `.board`.

Measured on the same page, same geometry, only the overflow value changed:

| `.board` overflow | Last item hit-test | Reachable |
|---|---|---|
| `visible` (desktop) | `"Delete list"` | **yes** |
| `auto` (the mobile rule) | an ancestor, not the menu | **no** |

In the real mobile lane at 390px, **three of four items were unreachable**
(Insert list before, Insert list after, Delete list) — only "Edit list" could be
touched, which is exactly what the screenshot showed.

### Five whys

1. Why did the menu look broken? Its items could not be touched.
2. Why? The menu was clipped to `.board`'s box.
3. Why does `.board` clip it? `overflow-x: auto` below 480px makes it a clipping
   container for absolutely-positioned descendants.
4. Why is the menu absolutely positioned inside the column? `architecture-design.md`
   §7 chose it deliberately, to keep `section.column > h3` a direct child — wrapping
   header and menu in a flex row would have disturbed a markup contract three
   shipped features read.
5. **Why did no test catch it?** Every `@needs-browser` scenario runs at desktop
   width, where `.board` has no overflow and the menu *is* reachable — measured.
   `pwa-mobile-rendering` added that rule months before this feature existed. The
   two never met in a test.

That fifth answer is the same shape as this feature's earlier `normalize_state`
defect: **a change landing on a premise established somewhere else, by someone
else, that is no longer true.**

## Defect B — the trigger was invisible and mis-sized

Measured at rest: **27×32 px**, `color: --cz-muted`, **transparent border,
transparent background**. It gained a border and a surface only on
`:hover` / `:focus-visible`. A phone has no hover, so the one control the
operator needed was indistinguishable from the label until tapped — the white
bordered box in the screenshot is the *focus* state, not the resting one.

It also overlapped the first card, because it was absolutely positioned into a
slot it outgrew:

| | trigger height | overlap with first card |
|---|---|---|
| desktop | 32px | 6px |
| mobile (the stylesheet's own 44px touch-target rule) | 44px | **18px** |

So on a phone it was simultaneously below the 44px floor this stylesheet gives
every other control, and sitting on top of `GEN-1`.

## Fix

1. **Menu → `position: fixed`**, coordinates written from the trigger's rect in
   `toggleLaneMenu()`. This leaves every ancestor's overflow box. Verified no
   `transform` / `filter` / `contain: paint` ancestor exists that would re-trap a
   fixed element in the same way. Closes on scroll and resize rather than
   tracking — a menu that follows a scrolling board is more machinery than this
   control earns.
2. **Trigger becomes a visible chip**: `--cz-surface` + `--cz-line` at rest
   (deepening on hover/focus), in the lane header's own eyebrow voice
   (`--cz-mono`, 0.6875rem, `0.18em` tracking), 44×44 on mobile. The header is
   now a **band** that reserves the chip's space, so it can never reach a card.
3. **One hairline above "Delete list"** — three constructive operations, then the
   destructive one. No danger colour: this palette has no danger token, and adding
   one means editing all three token regions to satisfy `check-arch`'s parity rule,
   for a distinction a rule already makes.

Nothing outside the existing token system was introduced; `check-arch`'s
no-colour-literal and token-parity rules both still pass.

## Regression tests

Two `@needs-browser @mobile` scenarios, both red before the fix:

- **"Every menu item is reachable on a phone-sized screen"** — the oracle is a
  **hit test** on every item, not a visibility check. A clipped menu still reports
  as displayed and still has a bounding rect; what it loses is a point the
  operator can touch. `is_displayed()` would have gone green over this bug.
- **"The menu trigger is visible at rest and big enough to touch"** — asserts a
  non-transparent edge before any hover or focus, a ≥44×44 box, and zero overlap
  with the first card.

Both use the shipped `open_mobile_session()` (real chromedriver mobileEmulation),
not a narrowed desktop window — a narrow window does not reproduce the defect's
own preconditions.

## Three defects the fix's own test runs found

1. **A CSS cascade trap.** The mobile override was written into the existing
   `@media` block ~200 lines *above* the base rules it overrode. Equal specificity,
   earlier loses: the chip came back 32×44. The override now sits after the rule it
   overrides, with a comment saying why.
2. **A probe broken by the fix.** `offsetParent` is `null` for a `position: fixed`
   element by spec, so `menu_is_open` reported every menu closed the moment the
   menu became fixed — six scenarios failed at once, none because the menu was
   broken. Both probes now judge visibility by the rect.
3. **An unexplained headless-only difference, recorded rather than hidden.** In the
   headless lane an empty `style=""` survives on the closed menu:
   `hasAttribute("style")` reads true *immediately after* `removeAttribute("style")`
   returns, which should not be possible. It did not reproduce in a headful Chrome
   loading the same stylesheet, the same script set (htmx included) and the same
   markup, including `hx-get` on the menu items. The byte-identity oracle now
   normalises that one attribute — with a hand-written, unit-tested normaliser that
   strips it **only** from `[data-lane-menu]` tags — plus a separate assertion that
   the residue is always EMPTY and never a live coordinate. A real positioning leak
   still fails the test.

## Files

| File | Change |
|---|---|
| `crates/foundry-app/static/css/foundry.b2612dc9.css` | menu block reworked; mobile overrides moved after the base rules (re-hashed from `78a05f58`) |
| `crates/foundry-app/static/js/keyboard.js` | `positionLaneMenu()`, scroll/resize dismissal, attribute removal on close |
| `crates/foundry-app/templates/base.html`, `src/lib.rs`, `static/VENDOR.md` | stylesheet hash |
| `crates/foundry-acceptance/tests/features/board-lane-overflow-menu.feature` | 2 regression scenarios |
| `crates/foundry-acceptance/src/steps/feature_board_lane_overflow_menu.rs` | regression steps, hardened probes, normaliser + 3 unit tests |
