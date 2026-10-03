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

---

# Step 03-02 — interruptions leave nothing behind; feature close

Step 03-02 GREEN, 2026-10-02. **No production change**: the three scenarios
were GREEN on the existing code (the single revert path from 01-02/01-04, the
hold aborts from 02-03, the scroller stop from 03-01), so `board-dnd.js` is
untouched. The only `.feature` diffs are the three `@pending` removals in
`card-pointer-drag.feature` (#24, #25, #26); `card-pointer-drag.feature` now
holds no `@pending` tag, and every other shipped `.feature` is byte-identical.
No CSS change; `foundry.6b3e4436.css` is its own sha256 prefix and `base.html`,
`lib.rs` and `VENDOR.md` all name it.

## Scenario results (03-02, un-pended one at a time in this order)

| # | Scenario | First run |
|---|---|---|
| 24 | The system taking the pointer mid-drag puts the card back and leaves nothing behind | GREEN on existing code (us-cpd-03 5/5, 44 steps). The 01-02 `pointercancel` handler already ran `endPress()` -> `end()` |
| 25 | A hold the system interrupts before the card lifts leaves nothing to undo (incl. the positive control) | GREEN on existing code (6/6, 53 steps). `pointercancel` before the lift already went through `endPress()`: timer cleared, cue cleared |
| 26 | Releasing a carried card off every lane changes nothing | GREEN on existing code (7/7, 62 steps). `pointerup` over no lane already landed nowhere |

Not RED for a business reason, so the discrimination is shown by the named
faults below: four of the five seeded faults turn #24, #25 or #26 RED on their
named oracles, never a timeout, so the scenarios exercise the shipped code and
are not fixture theatre.

## Gates (03-02, feature close)

| Gate | Result |
|---|---|
| Feature gate `FOUNDRY_ACCEPTANCE_TAGS=cpd` | 35 declarations / 37 examples: 37/37 scenarios, 315/315 steps; no `@pending` tag left in `card-pointer-drag.feature` |
| Step scenarios `us-cpd-03` | 7/7 scenarios, 62/62 steps; none pending |
| Guards | cdf 54/54 (400 steps), blr 26/26 (141), kb 38/38 (261), us-cts-03 5/5 (36) incl. "The board does not move when the typefaces arrive" |
| Default lane | 654/654 scenarios, 4522/4522 steps. A first run was cut by my own 580 s bound (exit 124, no failure seen); the rerun under a 2400 s bound was green |
| `cargo xtask ci`, `FOUNDRY_XTASK_INCLUDE_DOCKER=1` | exit 0: fmt, clippy, check-arch, release build, workspace tests, `cargo deny`, then `foundry-acceptance` with all tags: **880/880 scenarios, 6187/6187 steps**, against 654 in the default lane. The browser lane really ran: #24, #25 and #26 appear in the log as passed, with "the browser lane has started chromedriver" steps, and nothing was skipped |
| `cargo xtask check-arch` | passed (incl. no `keydown` in `board-*.js`, every hashed name its own sha256 prefix, every VENDOR.md sha256 recomputes) |
| `cargo xtask smoke` | all gates green, with `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false` |
| `.feature` files | only the three `@pending` removals; every other shipped `.feature` byte-identical |
| Browser | Chrome 151.0.7922.108 (`selenium/standalone-chrome:latest`, `cd778b6f38d9`) |

## Named faults (03-02; one seeded at a time; us-cpd-03 run, the ghost also us-cpd-01; restored and cmp-verified)

| Fault | Result | Killed on |
|---|---|---|
| Hold timer not cleared on `pointercancel` (cancel clears the cue only; the timer and the press run on) | killed | #25 "has still not lifted once the hold time has passed": "a pointercancel before the lift abandons the hold (D12, AC-3.5): LiftState { session: true, origin_lifted: true, … any_ghost: 1 }" |
| Arming not cleared on `pointercancel` (timer cleared, press dropped, cue left) | killed | #25 "no longer shows it is arming": "the arming cue must clear at once on every early exit … HoldSample { elapsed: 1084.0, arming: true, any_arming: 1, … matrix(0.96…), opacity: 0.7 }" |
| Ghost left behind (`end()` keeps the ghost) | killed | us-cpd-03: #24 and #26 "a carried card is left behind after the drag ended: … any_ghost: 1"; us-cpd-01: #3 "the carried card outlives the release", #4 "a carried card remains", #6 "a carried card is left behind" |
| `pointercancel` not reverting after the lift (01-04's mouse survivor, now through the shared path) | killed | #24 "a carried card is left behind after the drag ended: LiftState { session: true, origin_lifted: true, any_lifted: 1, … }" |
| Scroller not stopped on exit (`end()` neither sets `ended` nor cancels the frame) | **survived** | see Survivors |

## Survivors (routed to nw-acceptance-designer)

1. **Scroller not stopped on exit** (the same mutant as 03-01's survivor 4).
   #24 cancels while the card is over In-Progress, mid-board, and #26 releases
   over the page header with the page at `scrollY` 0. In both the last carried
   point is outside every edge zone, or in the top zone of a page that cannot
   scroll up, so a frame loop left running finds nothing to scroll and never
   re-runs `track`; nothing is lit, marked or moved. An oracle that cancels or
   releases while the card is held inside an edge zone that can still scroll
   (e.g. `touchCancel` at the board's right edge before it reaches its end),
   then reads `scrollLeft` and the lit lanes ~300 ms later, would kill it.

## Per-feature mutation kill rate (gate >= 80%)

Counted per distinct named fault; a fault re-run in a later step counts once,
at its latest result.

| Source | Faults | Killed | Notes |
|---|---|---|---|
| Slice 01 | 7 | 6 | the mouse `pointercancel` survivor is now killed by #24 (03-02); "click guard never armed" stays a Chrome-specific survivor; the ghost fault re-killed in 03-02 |
| Slice 02 (02-01 6, 02-02 7, 02-03 13) | 26 | 23 | as recorded in the slice 02 notes |
| 03-01 | 4 (+1 equivalent) | 1 | the unclamped scroller is an equivalent mutant and is excluded; "scroller not stopped" re-run in 03-02, still survives |
| 03-02, new faults | 2 | 2 | hold timer kept, arming kept |
| **Feature** | **39** | **32** | **82.1% — PASS** |

Counting the equivalent mutant as a survivor gives 32/40 = 80.0% (at the
gate). A raw per-run tally, which counts the three re-run faults twice
(slice 01 5/7, slice 02 23/26, 03-01 1/4, 03-02 4/5), gives 33/42 = 78.6%.
Every survivor is a test-strength gap or a platform limit, routed above or in
the earlier notes; the crafter does not author tests to lift the score.

## Real-device dogfood, slice 03 — OWED — user

Not run by the delivery agent (it cannot drive real devices). Run on the real
board, on **iOS Safari AND Android Chrome**, plus the **narrow desktop window**
check. Record the OS and browser version, Pass/Fail per step, and the tallies.

| Device | OS / browser version | Tester | Date |
|---|---|---|---|
| iPhone, Safari | | | |
| Android, Chrome | | | |
| Desktop, narrow window (Chrome; Firefox if at hand) | | | |

| # | Step | iOS Safari | Android Chrome |
|---|---|---|---|
| 1 | Far-lane drop on an 8-lane board at ~390 px, 5x: lift by the grip, hold at the right edge until Done shows, drop at the marker; a reload agrees (KPI 6, AC-3.1) | ___ / 5 | ___ / 5 |
| 2 | Hold at the board's end: the board stops, the page never scrolls sideways, the marker stays in the lane under the finger (AC-3.2) | | |
| 3 | Long lane: hold near the bottom until the last card shows, drop below it; a reload agrees (AC-3.3) | | |
| 4 | Body hold, then carry to an edge: same as 1, lifted by a 500 ms hold on the text | | |
| 5 | System takes the pointer mid-drag (notification shade / Control Centre / incoming call), 3x: the card is back in its slot; nothing lit, marked, dimmed or carried; no scroll keeps running; no request (AC-3.5) | | |
| 6 | System takes the pointer during a body hold (before the lift), 3x: no lift, no dimmed or shrunken card left; a hold straight afterwards lifts (AC-2.12) | | |
| 7 | Release off every lane (over the page header), 3x: nothing changes, no dialog opens, no request (AC-3.6) | | |
| 8 | Release while held at an edge, 3x: the scroll stops at once with the release (the 03-02 survivor, device-checked) | | |

- [ ] **Narrow desktop window** (AC-3.7): a mouse drag in a ~600 px window
      reaches an off-screen lane by holding at the board's right edge and lands
      at the marker; releasing at the edge stops the scroll. Result: ____
- [ ] **KPI 7 log:** record the dogfood date, devices and outcome in the KPI 7
      log. Result: ____
