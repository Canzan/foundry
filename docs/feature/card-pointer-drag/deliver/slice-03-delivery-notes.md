# Slice 03 delivery notes — carry a card off-screen, and never strand it

Step 03-01 GREEN, 2026-10-02. Production file changed: `static/js/board-dnd.js`
(the edge scroller). The only `.feature` diffs are the four `@pending`
removals in `card-pointer-drag.feature` (#21, #22, #23, #27); every other
shipped `.feature` is byte-identical. No CSS change, so no D18 re-hash.

## Gates (03-01)

| Gate | Result |
|---|---|
| Step scenarios `FOUNDRY_ACCEPTANCE_TAGS=us-cpd-03` | 4/4 scenarios, 36/36 steps (the other four US-CPD-03 scenarios are still `@pending`) |
| Guards | us-cpd-01 12/12 (102 steps), us-cpd-02 18/18 (151), cdf 54/54 (400), blr 26/26 (141; lane auto-scroll unchanged), kb 38/38 (261) |
| POST body | `moveBody` and `dropInto` untouched; the marker = landing = POST `after` oracles in #21 and #27 and the reload oracles in #21 and #23 GREEN |
| `cargo xtask check-arch` | passed (incl. no `keydown` in `board-*.js`) |
| `cargo xtask smoke` | all gates green (fmt, clippy `-D warnings`, check-arch, workspace tests) with `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false`. The first run, without it, failed at `cargo clippy` on the known host fault `libsqlx_macros … mis-aligned LINKEDIT` (DISTILL U6); nothing in Rust changed |
| Browser | Chrome 151.0.7922.108 (`selenium/standalone-chrome:latest`, `cd778b6f38d9`) |

## What 03-01 added (DDD-11, DDD-18, DDD-26, AC-3.1-3.4, AC-3.7)

- **The edge scroller.** `EDGE_ZONE` 48 and `EDGE_STEP` 14, copied from
  `board-lane-dnd.js::autoScroll` (DDD-18, not shared). From the lift, the
  session steps once per animation frame from the last point the card was
  carried to (`track` stores it), so a finger held still at an edge keeps it
  going. Sideways it scrolls `#board-columns` only, clamped to
  `[0, scrollWidth - clientWidth]`; the page never scrolls sideways (AC-3.2).
  Up and down it scrolls the page (`document.scrollingElement`), clamped the
  same way (DDD-11). The edges are measured inside the viewport
  (`max(rect.left, 0)`, `min(rect.right, innerWidth)`), where
  `elementFromPoint` can still read the point (spike Q4).
- **Marker = landing = `after`.** After every step that moved anything, the
  session runs `track` again from the stored point, so the lit lane and the
  marker are recomputed; the release still lands at the live marker. The
  scroller does nothing until the card has been carried, so a card lifted by a
  hold and released in place still lands nowhere (#17).
- **Stops on every exit.** `end()` sets `ended` and cancels the frame, and
  every exit (drop, off-lane release, Escape, `pointercancel`, detached card)
  goes through it (DDD-8).
- **Scroll events after the lift abort nothing.** The 02-03 capture `scroll`
  listener acts only while a body hold is pending (`holding()`), so the
  scroller's own scroll events are ignored (DDD-26). No change was needed.

## Scenario results (03-01, un-pended one at a time in this order)

| # | Scenario | First run |
|---|---|---|
| 21 | Holding a carried card at the board's edge scrolls to an off-screen lane | **RED** for the business reason: "did not scroll the board until Done was in view … scrollLeft went from 0 to 0 of 1514". GREEN after the change |
| 22 | Auto-scroll stops at the board's end | GREEN on #21's code |
| 23 | Holding a carried card near the bottom reaches the end of a long lane | GREEN on #21's code |
| 27 | A mouse drag in a narrow window reaches an off-screen lane too | GREEN on #21's code |

## Named faults (one seeded at a time; us-cpd-03 run; restored and cmp-verified)

| Fault | Result | Killed on |
|---|---|---|
| Vertical page scroll missing | killed | #23 "did not scroll the page until AUTH-60 was in view … page scrollY is 0, AUTH-60's bottom is at 1923 of a 844 px viewport" |
| Marker not recomputed after a scroll step | **survived** | see Survivors |
| Scroller not clamped at the board's end | **survived, equivalent** | see Survivors |
| Extra: the board's end spills into a sideways page scroll | **survived** | see Survivors |
| Extra: scroller not stopped in `end()` | **survived** | see Survivors |

**Kill rate over 03-01's faults:** 1/5 (the brief's three: 1/3). Below the
80% gate; the gaps are test-strength gaps, routed to `nw-acceptance-designer`
below. The crafter does not author tests to lift the score.

## Survivors (routed to nw-acceptance-designer)

1. **Marker not recomputed after a scroll step.** The harness holds "still"
   with `PointerStep::Jitter`, a trickle of 1 px moves. Every one of them is a
   `pointermove` that runs `track`, which recomputes lane and marker, and each
   When step glides to the release point before letting go. So no scenario
   ever reads the marker after a scroll step with no `pointermove` after it.
   A still-pointer oracle (hold with `PointerStep::Hold`, or read the marker
   between a scroll step and the next move) would kill it.
2. **Scroller not clamped (literal `Math.min` removed).** Equivalent mutant:
   the browser clamps `scrollLeft` and `scrollTop` to their extent itself, so
   the behaviour is identical. AC-3.2 holds either way.
3. **The board's end spills into a sideways page scroll.** At 390 px the page
   has no horizontal extent, so `window.scrollBy(14, 0)` moves nothing and
   #22's "the page has not scrolled sideways" cannot see it.
4. **Scroller not stopped in `end()`.** Every scenario releases away from the
   edges, so a scroller still running after the release finds nothing to
   scroll. An oracle that releases (or cancels) while held at an edge and then
   reads `scrollLeft` and the lit lanes a little later would kill it.
