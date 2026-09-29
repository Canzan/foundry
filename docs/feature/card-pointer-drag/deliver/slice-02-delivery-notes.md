# Slice 02 delivery notes — a grip that drags at once, and a hold on the text

Steps 02-01 (1bcf72d), 02-02 (45f8e7a) and 02-03, all GREEN, 2026-09-29.
Production files changed across the slice: `partials/issue_card.html` and
`issues.rs::render_issue_card` (the grip, 02-01), the stylesheet (grip 02-01,
arming cue 02-03, each re-hashed), and `static/js/board-dnd.js` (grip drag
02-02, body hold 02-03). The only `.feature` diffs are the `@pending` removals
in `card-pointer-drag.feature`; every other shipped `.feature` is
byte-identical.

## Gates (02-03 close)

| Gate | Result |
|---|---|
| Slice gate `FOUNDRY_ACCEPTANCE_TAGS=us-cpd-02` | 16 declarations / 18 examples, 18/18 scenarios, 151/151 steps; no `@us-cpd-02 … @pending` left |
| Guards | us-cpd-01 12/12 (102 steps), cdf 54/54 (400), blr 26/26 (141), kb 38/38 (261), us-cts-03 5/5 (36), incl. "The board does not move when the typefaces arrive" |
| POST body | `moveBody` and `dropInto` untouched; the byte-identical-request oracles in #30, #20 and #34 GREEN |
| Re-hash (D18) | `foundry.7fa13f60.css` -> `foundry.6b3e4436.css` (02-03); `base.html`, the three `lib.rs` cache-test literals and the `VENDOR.md` row and notes in the same change. 02-01 re-hashed `f9143163` -> `7fa13f60` |
| `cargo xtask check-arch` | passed (incl. no `keydown` in `board-*.js`, hashed name = own sha256 prefix, no colour literal outside the token regions) |
| `cargo xtask smoke` | all gates green (fmt, clippy `-D warnings`, check-arch, workspace tests). Two earlier runs each failed one different `foundry-store` testcontainers test on Postgres connection setup (`PortNotExposed`, then `Connection reset by peer`); `foundry-store` alone then ran 76/76 and the third smoke was green. Nothing in `foundry-store` changed |
| Browser | Chrome 151.0.7922.108 (`selenium/standalone-chrome:latest`, `cd778b6f38d9`) |

## What 02-03 added (DDD-4, DDD-5, DDD-26, DDD-27)

- **Body hold.** A primary touch or pen press on an own card's text (not the
  grip) sets `data-card-arming` and one 500 ms timer (`HOLD_MS`). The timer
  lifts only if the press is still the one followed, still pending and its card
  `isConnected`. Constants: `THRESHOLD` 6, `GRIP_THRESHOLD` 3, `HOLD_MS` 500,
  `HOLD_TOLERANCE` 10.
- **Aborts before the lift.** This pointer's `pointermove` past 10 px, a
  `touchmove` past 10 px, any `scroll` (one passive capture-phase `document`
  listener that acts only on a pending hold) and `pointercancel`. Each ends the
  press through `endPress()`: timer cleared, cue cleared, nothing lit or sent,
  the gesture left to the browser. A release first is a tap and opens the card.
- **After the lift.** A non-passive `document` `touchmove` guard prevents
  scrolling while a session is lifted. A card lifted by a hold and released
  before it was carried anywhere lands nowhere, so nothing moves and nothing is
  sent (#17). Every other rule is slice 01's.
- **The cue.** `--card-hold-ms: 500ms` is written once to `<html>` at init.
  `.issue-card[data-card-arming]` eases to `scale(0.96)` and opacity 0.7,
  `ease-in`, over `var(--card-hold-ms)`. The transition is on that selector
  only, so clearing snaps back. The cue is cleared by query on every exit, with
  `end()` as a backstop, and before the card is measured for the ghost. Under
  `prefers-reduced-motion: reduce` the transform is `none`; the fade stays.
- **Touch posture (DDD-4).** Cards gain `user-select: none` and
  `-webkit-touch-callout: none`; `contextmenu` is prevented while a hold is
  arming or a session exists.

## Scenario results (02-03, un-pended one at a time in this order)

| # | Scenario | First run |
|---|---|---|
| 30 | Holding a card's text arms it visibly, then lifts it, and it drops at an exact slot | **RED** for the business reason: 243 ms in, `arming: false, lifted: false` (trusted touch delivered). GREEN after the change |
| 14 | While lifted by a hold on its text, the lane lights, the marker shows, nothing scrolls | GREEN on #30's code |
| 17 | Lifted by a hold on its text and released where it lifted, opens nothing, stays put | GREEN |
| 20 | A pen lifts a card by holding its text | GREEN |
| 15 | A touch on the text that moves before the hold completes scrolls and lifts nothing | GREEN (the positive control lifts) |
| 16 | A tap on the text still opens it, even right after a touch drag | GREEN |
| 35 | With reduced motion, a card being held only dims | GREEN |

## Named faults (one seeded at a time; us-cpd-02 run; restored and cmp-verified)

02-03's faults. Each kill below is on the named oracle, not a timeout.

| Fault | Result | Killed on |
|---|---|---|
| Hold 350 ms instead of 500 | killed | #30 "between 400 and 480 ms … still arming and not lifted" (lifted at 422 ms); #14, #15's control, #17, #20: "lifted ~365 ms into the hold … must not lift before then" |
| Arming cue never set | killed | #30 "243 ms in … must show it is arming"; #15, #16 "never showed it was arming"; #35 |
| Cue not cleared on a move abort | killed | #15 "the arming cue must clear at once on every early exit" (`matrix(0.96…)`, 0.7) |
| Cue not cleared on a tap | killed | #16, same oracle |
| Cue not cleared at the lift | killed | #30 "at the lift the arming cue is cleared: 718 ms after the press, 1 card still arming" |
| Body `touchstart` prevented | killed | #15 "a swipe that starts on a card must scroll the board: scrollLeft 0 -> 0"; #16 and #28's control (no dialog); #33 |
| No post-lift `touchmove` guard | killed | #14 "In-Progress must be the only lane lit"; #30 "the release must land AUTH-41 between AUTH-3 and AUTH-12; it is at None" (unguarded, the browser took the pan from the carry) |
| Click guard never reset | killed | #16 "AUTH-41's edit dialog did not open … the click guard resets on the next press" |
| `event.target` lane resolution | killed | #14 "In-Progress must be the only lane lit … resolved from the point" (and #18) |
| Reduced-motion rule missing | killed | #35 "with reduced motion the arming cue must not scale the card" |
| Hold timer not cancelled on a move (the move abort clears the cue only) | **survived** | see Survivors |
| Hold timer ignores `isConnected` (DISTILL U8) | **survivor, seeded and verified by reading** | see Survivors |
| The `scroll` abort removed | **survived, as expected** | see Survivors |

02-01's six faults (all killed) and 02-02's seven (all killed) are recorded in
1bcf72d and 45f8e7a and were not re-run.

**Per-feature mutation kill rate over slice 02's named faults:** 02-01 6/6,
02-02 7/7, 02-03 10/13, so **23/26 = 88%** (gate >= 80%). Over the faults the
lane can reach at all (without the three below): 23/23.

## Survivors (accepted, documented)

1. **Hold timer ignores `isConnected` (U8).** Seeded (the `isConnected` check
   in `liftHeld` removed), verified by reading, restored, `cmp`-verified. No
   user gesture can replace the board in place while a finger is held still on
   a card, so no lane scenario can fire it. Read effect: a replace mid-hold
   would lift a detached card, showing a ghost of it; the release still lands
   nowhere (`pointerup` refuses a disconnected card) and `end()` clears
   everything, so nothing is sent or moved.
2. **The move aborts and the `scroll` abort, each on its own.** The lane's
   swipe (#15) glides 25 px per CDP step, past Chrome's touch slop at once, so
   several abort paths fire on the same gesture: the cue-kept fault above was
   killed, which proves `abortHold` runs in #15. Removing the `scroll` abort,
   or making the `pointermove` abort (and, in a second diagnostic run, the
   `touchmove` abort too) clear only the cue while the timer runs on, survives,
   because the remaining paths, down to Chrome's `pointercancel` on its pan
   claim, still end the hold. That redundancy is the design (DDD-26: WebKit
   stops `pointermove` once it pans and may claim a pan with little pointer
   traffic). Each path alone is proven only by the device checklist, steps
   1-3 and 7(a) below.

## Real-device dogfood — OWED — to be run by the user

Not run by the delivery agent. Run the amended checklist (feature-delta
"Device checklist, grip and body hold") in the app on the real board, on
**iOS Safari AND Android Chrome** (Android is U-1's waived check). Record the
OS and browser version, Pass/Fail per step, and the tallies.

| Device | OS / browser version | Tester | Date |
|---|---|---|---|
| iPhone, Safari | | | |
| Android, Chrome | | | |

| # | Step | iOS Safari | Android Chrome |
|---|---|---|---|
| 1 | Prompt swipes up across card text, 20x: page scrolls, 0 lifts | | |
| 2 | Prompt swipes left across card text, 10x: board scrolls, 0 lifts | | |
| 3 | Resting swipes (rest ~1 s, then swipe up), 20x: **lifts / 20** (AC-2.14, KPI 3 lazy-swipe tally); every mistaken lift released off a lane changed nothing | ___ / 20 | ___ / 20 |
| 4 | Grip drags, 10x: lift at once, no scroll, no `pointercancel`, lands in the lane under the finger. Include 3 grips on **one-line cards** (U-3) | ___ / 10 | ___ / 10 |
| 5 | Grip taps, 10x: no edit dialog, no lift | | |
| 6 | Body holds, 5x: card shrinks slightly and dims over ~0.5 s, then lifts; no selection, magnifier, callout, link preview, context menu or native drag; no scroll while carrying; drop lands where aimed | ___ / 5 | ___ / 5 |
| 7 | Cue clears: (a) swipe away before the lift, 3x: snaps back and scrolls; (b) lift the finger before the lift, 3x: snaps back and opens the card | | |
| 8 | Body taps, 5x: edit dialog opens, no lift, no dimmed card left | | |
| 9 | Lifted release in place (grip, then body hold): no dialog, card stays in its slot, no request | | |
| 10 | Second finger during a lifted drag (grip and body hold): first finger keeps the card, or `pointercancel` cleans up; nothing left lifted, arming or lit | | |
| 11 | System gesture during a lifted drag (notification shade / Control Centre): nothing stuck | | |
| 12 | Pen (if a stylus tablet is at hand): repeat 4 and 6 | | |

- [ ] **U-2, arming opacity:** the 0.7 dim reads clearly on the **light AND
      dark** palettes (both devices). Result: ____
- [ ] **U-3, one-line cards:** the grip on a one-line card is easy to hit
      (48 px min-height). Result: ____
- [ ] **Desktop feel, grip visible** (Chrome and Firefox): a mouse click on a
      grip opens nothing; a mouse press-and-move from a grip drags; a click on
      the body opens the card; the grip strip does not get in the way of
      reading. Result: ____
- [ ] **Firefox click guard** (carried from slice 01): after a lifted mouse
      release, no dialog opens. Result: ____
- [ ] **KPI 7 log:** record the dogfood date, devices and outcome in the KPI 7
      log. Result: ____
