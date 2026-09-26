# Spike findings — card-pointer-drag (DESIGN, OQ-2 / OQ-3 / OQ-4 / OQ-5 / OQ-9)

Date 2026-09-25. Timebox ~45 min. Nothing under `crates/` or `docs/product/` was touched.

## Method

- **Browser:** Google Chrome **153.0.8010.53** (macOS, `--headless=new`), driven over raw CDP
  WebSocket from Node 26 (scripts kept in the session scratchpad, not the repo).
- **Input is trusted**: `Input.dispatchMouseEvent`, `Input.dispatchTouchEvent`,
  `Input.dispatchDragEvent`. No JS-dispatched events. Touch runs used
  `Emulation.setDeviceMetricsOverride` (390x844, DPR 3, `mobile: true`) +
  `Emulation.setTouchEmulationEnabled` (maxTouchPoints 5).
- **Page:** [`probe.html`](probe.html): a `.board` (`overflow-x:auto`) of 4 lanes x 12
  `<article class="issue-card">`; page scrolls vertically, lanes never scroll (as foundry.css).
  It implements the planned lift rule (mouse: 6 px; touch/pen: hold 350 ms within 10 px),
  a non-passive `touchmove` listener on `document` that `preventDefault`s **only after a lift**,
  and logs pointer*/touch*/drag*/contextmenu/click/selectstart/scroll with timestamps.
  Verdicts use the **final** `scrollY` / `board.scrollLeft`, not event counts.
- One run per row. The touch hold fired 350 ms after `pointerdown` in every run.

## Q1 — OQ-4a: mouse on `draggable="true"` (press, move 10/30/100 px, release)

| Variant | dragstart | pointercancel | pointermove after | pointerup | lift (6 px) | click |
|---|---|---|---|---|---|---|
| A `draggable` | **yes**, 3rd move (~4-6 px) | **yes** | stops (0; one stray at 100 px) | **none** | never | none |
| B no `draggable` | no | no | uninterrupted (6/16/51) | yes | yes | **yes, on the card** |
| C `draggable` + `dragstart` pd (document) | fires, prevented | no | uninterrupted | yes | yes | yes |
| D `draggable` + `pointerdown` pd | **never fires** | no | uninterrupted | yes | yes | yes |
| E no `draggable` + `setPointerCapture` | no | no | uninterrupted | yes | yes | yes |
| any, no move (click) | no | no | - | yes | no | yes |
| B right button, 50 px | no | no | - | yes | no (button filter) | `contextmenu` only |

Foreign drag with cards **not** draggable: `Input.dispatchDragEvent` (files `["/etc/hosts"]` +
`text/plain`) over a card delivered `dragenter` → `dragover` (types `text/plain|Files`, prevented
by the board swallow) → `drop` (prevented). The page did not navigate.

**Verdict:** a native drag from a `draggable` card kills the pointer stream (pointercancel, no
pointerup), as OQ-4(1) feared. B, C and D all keep an uninterrupted stream. D's cost:
`pointerdown.preventDefault()` also suppresses compat mouse events, focus and text selection.
C is the smallest change that keeps `issue-status-move.feature:49` true; B is the cleanest.
The foreign swallow does not depend on the card being draggable.

## Q2 — OQ-4b: emulated touch long-press on a `draggable` card

| Setup | dragstart | contextmenu | selectstart | pointercancel | click on release |
|---|---|---|---|---|---|
| hold 600 ms still, release | no | no | 0 | no | yes |
| hold 1200 ms still, release | no | no | 0 | no | yes |
| hold 600 / 1200 ms then move 100 px (no pd) | no | no | 0 | yes (browser pan) | no |
| same, NOT draggable | no | no | 0 | yes | no |

**Verdict: inconclusive by construction.** Desktop Chrome's touch emulation, fed by CDP touch
events, runs **no** long-press gesture: no native drag, no context menu, no selection, at any
hold length. Emulation cannot confirm or refute ADR-007's "no HTML5 DnD on touch" (C3).
Real Android Chrome and iOS Safari are the only oracle (device checklist, steps 4, 7, 8).

## Q3 — OQ-2: `touch-action` strategy (390 px, emulated touch)

| # | Card `touch-action` | Gesture | Page/board scrolled? | pointermove | pointercancel | touchmove pd |
|---|---|---|---|---|---|---|
| a | auto | hold 400, then diagonal 120x150 | **no** | 20, flowing | **no** | 19/19, all cancelable |
| a | auto | hold 400, then vertical 300 | **no** | 20 | **no** | 19/19 |
| a | auto | hold 400, then horizontal 120 | **no** | 20 | **no** | 18/18 |
| a | auto, NOT draggable | hold 400, then vertical 300 | **no** | 20 | no | 19/19 |
| a' | auto, **no pd** (control) | hold 400, then vertical 300 | page **285 px** | 2 | **yes** | 0 (12 uncancelable) |
| a" | auto | drift 4/9 px during the hold, then vertical | **no** | flowing | no | all prevented |
| b | auto | immediate vertical swipe | page **285 px** | 2 | yes | none (uncancelable) |
| b | auto | immediate horizontal swipe | board **105 px** | 3 | yes | none |
| c | none | vertical swipe | **no** (swipe from a card is dead) | 20 | no | - |
| c | none | horizontal swipe | **no** | 20 | no | - |
| c | pan-y | vertical swipe | page 285 px | 2 | yes | - |
| c | pan-y | horizontal swipe | **board did not scroll** | 20 | no | - |
| d | auto + `setPointerCapture` | hold, horizontal | no | 20, target always the card | no | 18/18 |
| d | auto, no explicit capture | hold, cross into lane 1 | no | target **always `c0-2`** | no | 19/19 |

Also measured: Chrome withholds `touchmove` until the finger leaves a **~15 px slop** (first
`touchmove` at 16 px), while `pointermove` flows from the first pixel. Track the hold tolerance
on `pointermove`. A tolerance ≤ 15 px means no scroll can begin before the hold decides.

**Verdict:** option **(a) works in Chrome**: swipe scrolls (b), and post-lift moves neither
scroll nor raise `pointercancel`. (c) confirms the predicted trade-offs: `none` kills
swipe-to-scroll from a card; `pan-y` kills horizontal board scroll from a card. (d) explicit
capture changes nothing for touch.

## Q4 — OQ-5: lane resolution under implicit capture

With touch, `pointermove.target` stayed the origin card for the whole gesture, with or without
`setPointerCapture`, even over another lane. `document.elementFromPoint(clientX, clientY)`
returned the real element under the finger (`c0-2 → column → c1-2`) and `null` at the viewport
edge. **Verdict:** resolve lane and slot from the point. Treat `null` as no lane. For mouse,
capture is optional (E behaved like B inside the window).

## Q5 — OQ-3 (emulatable part)

`selectstart` = 0 and `contextmenu` = 0 on a 1200 ms still touch hold, **with and without**
`user-select:none; -webkit-touch-callout:none` and a `contextmenu` preventDefault: emulation
shows no OS long-press behaviour, so it proves nothing. A mouse right-click fires `contextmenu`
and preventDefault suppresses it. **Verdict: device-only.** Keep all three mitigations (CSS pair
+ `contextmenu` pd while a session exists). They are cheap and harmless on desktop.

## Q6 — Post-gesture `click`

| Gesture | click? | Target |
|---|---|---|
| mouse, no move | yes | card |
| mouse drag 100 px, released on the **same card** | **yes** | card → opens the dialog unless suppressed |
| mouse drag 300 px, released in another lane | yes | **common ancestor** `#board-columns` |
| touch tap (80 ms) | yes | card |
| touch hold 400 ms, **lift, release without moving** | **yes** | card → must be suppressed (AC-2.3) |
| touch hold, move 40 px in-card or into another lane | **no** | - (Chrome cancels the tap past slop) |

**Verdict:** arm a one-shot click suppression on release of any **lifted** session (mouse and
touch). Clear it on the next `pointerdown`, because touch drags past slop produce no click to
consume it. After a real drop the card moves before `click` dispatches, so the "common ancestor"
can be the card itself. Never rely on the click target.

## Q7 — OQ-9: driver fidelity

- CDP trusted input drove **both** paths: the mouse threshold lift, the touch hold lift, a
  **real** page/board scroll on an early swipe (`scrollY` 285, `scrollLeft` 105), and
  `pointercancel` on a browser-claimed pan. A driver can prove AC-2.2's "swipe scrolls, no lift"
  in Chrome, not only "no lift". It cannot prove iOS behaviour.
- The harness (`browser_harness.rs`, read only) uses `selenium/standalone-chrome:latest`
  (bundled chromedriver), a 390x844 `mobileEmulation.deviceMetrics` session, and already calls
  `fantoccini::actions` (`KeyActions`). fantoccini 0.21.5 has `MouseActions`, `PenActions` and
  **`TouchActions`** (`PointerType::Touch`). chromedriver implements W3C touch pointer actions via
  `Input.dispatchTouchEvent`, the path measured here. **Not run** against the container (spike
  rule), so this is highly likely but not proven.
- A W3C `pause` of 400 ms between `pointerDown` and `pointerMove` is the hold. CDP touch dispatches
  took ~33 ms each (vsync-paced), so a 20-step move takes ~0.7 s. Budget timeouts accordingly.
- Recommendation: re-point card scenarios to **W3C Actions** (`MouseActions` / `TouchActions`),
  not synthetic `PointerEvent`. Keep synthetic `DragEvent` for foreign drags (D3/D4). Arm the page
  recorder first (automation drags can deliver zero events).

## Recommendations per OQ

| OQ | Recommendation | Evidence |
|---|---|---|
| OQ-2 | **(a)**: cards stay `touch-action: auto`; one non-passive `touchmove` on `document` that pd's only while a lifted session exists (Touch Events beside Pointer Events) | Q3 a, a', b |
| OQ-4 | Stop native drags from own cards. Prefer **removing `draggable`** (B) if the user accepts rewording `issue-status-move.feature:49`; else keep it + **pd `dragstart` on `.issue-card`** in `#board-columns` (C). Avoid pd on `pointerdown` (D). Device-verify long-press either way | Q1, Q2 |
| OQ-5 | Resolve from `elementFromPoint`; the carried visual gets `pointer-events:none`; `null` = no lane; explicit capture optional | Q4 |
| OQ-1 | 350 ms / 10 px behaves as intended in Chrome; 10 px < ~15 px slop. Keep the hold under the ~500 ms OS long-press. The final value is a device feel call | Q3 a" |
| OQ-3 | `user-select:none` + `-webkit-touch-callout:none` on cards + `contextmenu` pd during a session. Device-only proof | Q5 |
| click | One-shot suppression after any lifted release, reset on `pointerdown` | Q6 |
| OQ-9 | W3C Actions via fantoccini `TouchActions` / `MouseActions` | Q7 |

## What emulation CANNOT tell us

1. Whether real Android Chrome / iOS Safari start a **native drag** from a long-press on a
   `draggable` card.
2. iOS selection, magnifier, callout, link preview or haptics; Android context menu or handles.
3. Whether **iOS Safari** honours `touchmove` preventDefault after a still hold like Blink does.
   This is the gating unknown for OQ-2(a).
4. Momentum and fling, iOS edge-swipe back, pull-to-refresh, real 60/120 Hz timing.
5. Whether 350 ms **feels** right against the OS long-press.

## Manual device checklist (iOS Safari AND Android Chrome)

Serve: `cd docs/feature/card-pointer-drag/spike && python3 -m http.server 8000`, then open
`http://<laptop-LAN-IP>:8000/probe.html` on the phone. The log is the bottom panel; the status is
top left. Defaults: draggable on, touch-action auto, pd-after-lift on, contextmenu pd on,
no-select on, hold 350 ms. Record Pass/Fail and the OS/browser version for each step.

1. **Swipe up from a card** (quick). Expect: page scrolls; `hold aborted`, then `pointercancel`; no `LIFT`.
2. **Swipe left from a card** (quick). Expect: board scrolls horizontally; no `LIFT`.
3. **Tap a card.** Expect: `click @c…`; no `LIFT`.
4. **Press and hold still ~0.5 s.** Expect: card turns yellow and `LIFT` appears. No selection,
   magnifier, callout, preview or context menu. No `dragstart` in the log.
5. **Hold until lifted, then drag diagonally across lanes.** Expect: no page/board scroll;
   `pointermove` keeps flowing with `under=` changing lanes; no `pointercancel`; then `pointerup`,
   `END`; no unsuppressed `click`.
6. **Hold until lifted, release without moving.** Expect: `click … SUPPRESSED` or no click.
7. **Repeat 4-5 with `pd contextmenu` and `no-select/callout` unchecked.** Record which OS
   behaviour appears. This proves each mitigation is needed.
8. **Repeat 4-5 with `draggable` unchecked.** Expect the same result. If 4/5 showed `dragstart`
   with draggable on, OQ-4 must remove `draggable`.
9. **touch-action `none`, swipe up from a card.** Expect no scroll. **`pan-y`, swipe left.**
   Expect the board does not scroll.
10. **During a lifted drag: second finger, or pull the notification shade.** Expect
    `pointercancel`, then `END pointercancel`; nothing stuck yellow.
11. **Hold ms 250 / 450**, repeat 1 and 4. Note which value feels right (OQ-1).

If step 5 scrolls on iOS (a `pointercancel` right after `LIFT` when moving), OQ-2(a) fails on
WebKit, and the none/pan-y trade-off returns to the user.
