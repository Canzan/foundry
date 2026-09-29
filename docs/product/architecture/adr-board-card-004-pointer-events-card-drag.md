# ADR-BOARD-CARD-004: Cards drag on Pointer Events, one hand-authored session for mouse, touch and pen; HTML5 drag-and-drop stays only to swallow foreign drags

## Status

**Accepted (2026-09-25)**, card-pointer-drag DESIGN wave. The user chose
Option A over SortableJS for cards (B) and for cards and lanes (C)
(`docs/feature/card-pointer-drag/feature-delta.md` §DESIGN, "User decisions").
D4 (shipped Gherkin byte-identical) stays binding, and no device bake-off against
SortableJS was run. The user also chose to keep `draggable` with `dragstart`
prevented, to record htmx 4 as a successor feature, and to add a `check-arch`
rule forbidding a `keydown` listener in `board-*.js`.
**Accepted, contingent on device evidence for touch.** Slice 01 (mouse parity)
proceeds on this decision. The device checklist (spike steps 1-11, DDD-13) gates
the planning of slice 02; if iOS Safari fails step 5, the mechanism question for
touch returns to the user with the device evidence. (Clarified 2026-09-26 at the
card-pointer-drag DISTILL review.) Acceptance-driver note, same date: touch is
driven by CDP touch events, not W3C `TouchActions` (DDD-12a).
**Amended 2026-09-29: still Accepted.** The device checklist ran on an iPhone.
Step 5 passed, so Option A stands for touch. Step 1 failed the whole-card hold,
and the user replaced it with a grip and a body hold. See *Amendment 2026-09-29*
at the end; Decisions 1, 4 and 8 are annotated.

Supersedes, in part, `adr-board-lane-007-pointer-events-lane-drag.md`:
the phrase "the divergence is deliberate and, for now, permanent", and boundary
leg 1 ("different event families"). It builds on ADR-BOARD-CARD-001, -002 and -003
and amends none of them. (The feature's brief called this ADR "003"; that number
was already taken by the placeholder ADR.)

## Context

Cards drag on native HTML5 drag-and-drop (`board-dnd.js`), and lanes drag on
Pointer Events (ADR-BOARD-LANE-007). A card cannot be dragged by touch, while the
lane header directly above it can. `card-pointer-drag` DISCUSS locked full
convergence of the card drag onto one mechanism for mouse, touch and pen (D2).
HTML5 drag-and-drop is kept only to swallow foreign drags (D3). The Gherkin of
every shipped scenario must stay byte-identical (D4), and everything
`card-drag-drop-feedback` shipped must be preserved as behaviour (D10).

After DISCUSS the user asked for **"the most compatible way to do drag and drop …
consider htmx, and look into using the latest version."** Real-device
compatibility is therefore the top quality attribute. It is followed by
preserving shipped behaviour, testability in the acceptance harness, and the
vendoring posture (`VENDOR.md` sha256 rows, `check-arch` R1-R3, no build step).

The measured facts, from `docs/feature/card-pointer-drag/spike/findings.md`
(Chrome 153, trusted CDP input):

- A native drag from a `draggable` card fires `pointercancel` and no `pointerup`, so a pointer drag cannot coexist with an unprevented native drag (Q1).
- With `touch-action: auto` on cards and a non-passive `touchmove` that calls `preventDefault` only after the lift, a swipe scrolls the board or page, and a lifted move neither scrolls nor raises `pointercancel` (Q3).
- Under implicit touch capture, `event.target` stays the origin card for the whole gesture. `elementFromPoint` returns the real element under the finger (Q4).
- A `click` follows a same-card mouse release and a still hold-and-release. Touch drags past the ~15 px slop produce none (Q6).
- W3C Actions (fantoccini `MouseActions`/`TouchActions`) drive both lift rules and produce real scrolls in Chrome (Q7).
- Emulation cannot tell whether iOS Safari honours `touchmove` preventDefault after a still hold, or whether mobile engines start a native drag on a long-press.

htmx has no drag-and-drop. Its "Sortable" example integrates SortableJS
(latest 1.15.7, MIT). htmx 4.0.0 was released on 2026-08-28 (npm `next`, with 2.x
remaining `latest` until early 2027). The repo pins htmx 2.0.4.

## Decision (Option A)

1. **The card drag runs on Pointer Events**, delegated on `document`, scoped to `.issue-card` inside `#board-columns`. Mouse (primary button) lifts past **6 px**. ~~Touch and pen lift after a **350 ms hold within 10 px**; movement first is a scroll, and release first is a tap. The final values are a device feel call.~~ *Amended 2026-09-29:* touch and pen lift from the card's **grip** after 3 px of travel, with no timer, or from the card **body** after a **500 ms hold within 10 px**. On the body, movement first is a scroll and release first is a tap.
2. **The shipped session is kept.** `CardDragSession`, `Origin`, `slotFor`, lane activation, the zero-footprint marker, landing at the live marker, `dropInto` and the byte-identical `fetch` POST (`state`, `after`, `x-csrf-token`) are unchanged. Only the event layer under them is replaced.
3. **Lane and slot are resolved from the point** (`elementFromPoint`). The carried ghost has `pointer-events: none`, and `null` means no lane. `event.target` is never used for lane resolution during a session.
4. **Touch posture.** Cards keep `touch-action: auto`. One non-passive `touchmove` listener on `document` prevents the default only while a session is lifted. Cards carry `user-select: none` and `-webkit-touch-callout: none`, and `contextmenu` is prevented during a hold or session. *Amended 2026-09-29:* the grip is `touch-action: none`, and its `touchstart` is prevented by one more non-passive listener on `document`.
5. **The carried visual is a fixed clone ghost** offset from the pointer. The origin card stays in its slot, dimmed. No card moves until the drop.
6. **Every exit is explicit.** Escape is a new `keyboard.js::closeTopLayer()` arm directly above the lane-drag arm, found by `html[data-card-dragging]`, and it dispatches `foundry:cancel-card-drag`. `pointercancel` after the lift, a release off every lane, and a card detached by a replace mid-drag each revert to the exact origin with no request. `pointercancel` before the lift abandons the hold.
7. **A one-shot click guard** is armed on any lifted release and reset on the next `pointerdown`, so no drag opens the edit dialog and no stale guard eats a later tap.
8. **Own cards never start a native drag.** Cards keep `draggable="true"`, and `dragstart` on own cards in `#board-columns` is prevented. Removal of `draggable` is a user decision, forced only if the device checklist shows a native drag from a long-press. HTML5 `dragover`/`drop` listeners remain only to swallow foreign drags, as shipped. *Amended 2026-09-29:* the checklist did show one (~655 ms), and removal would not have helped, because iOS then drags the card's text. `draggable="true"` stays.
9. **The two drag modules stay separate and share conventions, not code**: threshold, edge zone and step, the `pointerId` filter, and the arm pattern.
10. **BR-4 is enforced structurally.** `cargo xtask check-arch` gains a rule that no `crates/foundry-app/static/js/board-*.js` contains a `keydown` listener. The input set is found by scanning, and the rule carries an injected-violation gold test. It is a DoD item of card-pointer-drag.
11. **htmx stays at 2.0.4.** The move to htmx 4 is the successor feature `htmx-4-migration`.

### How the boundary holds now

ADR-007's leg 1 (different event families) no longer exists, because both
modules listen for `pointerdown` on `document`. The boundary rests on:

- **Origin.** Each module ignores a `pointerdown` whose target is not inside its own element (`.issue-card` or `[data-lane-drag]`), and neither element contains the other.
- **Thresholds and the lift rule.** A press that never lifts produces no drag.

`board-lane-reorder.feature:230` (unchanged) is the standing proof from the lane
side, and a new US-CPD-01 scenario is the proof from the card side.

## Alternatives Considered

| Alternative | Rejected because |
|---|---|
| **B: SortableJS 1.15.7 for cards**, vendored, initialised by `htmx.onLoad` (`forceFallback`, `delayOnTouchOnly`, `group`, `scroll`) | Its touch compatibility rests on the same techniques as A: pointer/touch events instead of HTML5, a hold delay, a non-passive `touchmove` guard and `elementFromPoint`. It adds no compatibility property A lacks, and the one real unknown (WebKit `touchmove` pd) is shared by both. Against that: (1) it **moves the DOM live** during the drag, which contradicts the zero-footprint marker and landing at the marker (ADR-002). **11 scenario declarations / 17 examples** of `card-drag-drop-feedback.feature` would change (`:215` plus the ten US-CDF-03 marker declarations), or Sortable's sorting would have to be disabled (`onMove` → `false`), leaving a vendored library doing only the ghost and the timer. (2) It **binds per list**, so every `#board-columns` replace needs a re-init, including `applyBoard`'s non-htmx `replaceWith`. That is exactly the alternative ADR-BOARD-CARD-001 rejected. Reasons (1) and (2) follow from Sortable's documented model (live sorting, instance per list element) and are the deciding reasons. **Open risks, unverified, not relied on:** Sortable may have no public cancel-drag API, in which case the Escape arm would fake a release into its internals (BR-4 coupling); and its fallback may evaluate the drag-over on an interval, which would give the acceptance lane a release-timing race. Vendoring cost (one upstream-verbatim `VENDOR.md` row, R1-R3) was **not** a deciding factor. |
| **C: SortableJS for cards and lanes**, retiring `board-lane-dnd.js` | The only single-mechanism answer. It has all of B's costs, and the lane list's host *is* `#board-columns`, the node every replace swaps. It changes `board-lane-reorder.feature:255` and `:263` (the in-flow drop indicator) on top of B's 11/17. It also rewrites a shipped, touch-proven, mutation-tested module for no user-visible gain. |
| **Migrate to htmx 4.0.0 in this feature** | htmx has no drag-and-drop in any version, so it does not serve this goal. It is a cross-cutting migration of every `hx-*` surface (fetch transport, explicit attribute inheritance, morph swaps), and it would fold that regression surface into a feature gated on byte-identical Gherkin. It is also 4 weeks old and published as npm `next`. It goes to its own successor feature. Option A has zero htmx coupling, so nothing waits on it. |
| **Keep HTML5 DnD for mouse; add a touch-only Pointer path** | Two card mechanisms that can drift apart. Rejected at DISCUSS (D2). |
| **Status quo (ADR-007's divergence)** | Leaves cards undraggable on touch, which ADR-007's own Consequences predicted users would read as a bug. |
| **`touch-action: none` or `pan-y` on cards** | Measured: `none` kills swipe-to-scroll that starts on a card. `pan-y` kills horizontal board scroll from a card on a 390px board (spike Q3 c). Kept only as the stop-and-return fallback if WebKit fails device step 5. |
| **`pointerdown.preventDefault()` to stop native drags** | Also suppresses compat mouse events, focus and text selection (spike Q1 D). |

## Consequences

- Positive: one card-drag mechanism serves every pointer. The behaviour contract, including replace-proof delegation, survives by construction because only the listener layer changes. There is no new dependency, and `VENDOR.md` changes only for the stylesheet re-hash.
- Positive: the acceptance lane gains trusted input (W3C Actions) where it had synthetic `DragEvent`s. That is stronger evidence than `card-drag-drop-feedback` could collect.
- Negative: every gesture edge case is ours to own: the click trap, the hold, `pointercancel`, the ghost and auto-scroll. The spike has mapped them, and the lane module is the reference class.
- Negative: iOS Safari behaviour is **unmeasured**. The device checklist (spike steps 1-11) gates slice 02. If WebKit ignores the `touchmove` guard, a lifted drag ends in `pointercancel`, which reverts safely with no request. The `none`/`pan-y` trade-off then returns to the user.
- Neutral: `keyboard.js` gains its third drag-type arm. The ADR-BOARD-LANE-005 pattern generalises again.
- Obligation (DISTILL): OUT-13 and OUT-14 are amended to pointer input. The CDF-KPI-8 instrument is re-scoped to `.feature` files.
- **This decision is provisional on device evidence.** A was chosen on the contract and replace-proof arguments; on compatibility, A and B were argued equal, not measured. The user declined a device bake-off. The device checklist still gates slice 02.
- Deferred successor: **`htmx-4-migration`**, scheduled once htmx 4.x is npm `latest` (expected early 2027). Option A has no htmx coupling, so that migration does not touch the card drag.
- Obligation (DELIVER): the `check-arch` `keydown` rule (Decision 10) ships with its gold test.

## Amendment 2026-09-29: grip and body hold

### Decision

On touch and pen, every card has a **grip**, a 48 px strip on its right edge,
shown on every card at every width and for every pointer. A touch or pen on the
grip lifts the card after 3 px of travel, with no hold, and never scrolls. The card
**body** lifts after a **500 ms hold within 10 px**, with a visible **arming cue**
(the card scales to 0.96 and fades to opacity 0.7 over the hold; under
`prefers-reduced-motion` it only fades). A move, a `touchmove` or
any scroll before the hold completes aborts it, and the browser scrolls. A release
before it is a tap and opens the card. A tap or click on the grip opens nothing,
whatever the pointer. The mouse is otherwise unchanged: 6 px anywhere on the card,
and a click on the body opens it. Everything after the lift is as decided above.

The grip is one empty `span` with `data-card-grip` and `aria-hidden="true"`, the
last child of the card in both card sources. Its CSS is `touch-action: none`,
`-webkit-user-drag: none`, `user-select: none` and `-webkit-touch-callout: none`,
with a dot glyph drawn in CSS. A touch that lands on a grip has its `touchstart`
prevented; a touch anywhere else never does. Like every card listener, the one
that does this is delegated on `document` and never bound inside `#board-columns`
(ADR-BOARD-CARD-001); its code is DELIVER's. Cards keep
`draggable="true"`, and own-card `dragstart` stays prevented. Design detail:
`docs/feature/card-pointer-drag/feature-delta.md` §DESIGN, DDD-23 to DDD-29.

### Evidence

`docs/feature/card-pointer-drag/spike/findings.md`, iPhone 13 Pro Max, iOS 26.7,
Safari and Brave:

- The whole-card hold failed. At 350 ms, 14 of 30 swipes that began on a card lifted it. In each, the thumb rested 407-1085 ms before it moved. First-move times spread evenly from 25 to 1085 ms, so no hold duration separates a resting swipe from a hold (800 ms still lifted 2 of 17).
- The post-lift `touchmove` guard works on WebKit: 14 of 14 lifted gestures kept their `pointermove`, every `touchmove` was prevented, 0 px scrolled, and no `pointercancel` came.
- With `draggable="true"`, iOS fires a native `dragstart` ~655 ms into a still press and holds a grip touch 571-1505 ms for its own drag interaction (`probe-v3.html`). Preventing the grip's `touchstart`, plus `-webkit-user-drag: none`, removed it (`probe-v4.html`).
- The chosen model on the same phone: 13 of 13 grip drags dropped in the lane under the finger (`probe-v5.html`), and 5 of 5 body holds lifted at ~503 ms and dropped where aimed (`probe-v6.html`). The user: "works great".

### Alternatives now rejected

| Alternative | Rejected because |
|---|---|
| **Whole-card press-and-hold, at any duration** (the original Decision 1) | Measured: a resting thumb's first move comes anywhere from 25 ms to over 1 s, so every duration either lifts lazy swipes or makes the hold unusably slow. |
| **A flick-velocity veto** (lift on a hold, but cancel it when the first move is fast) | It decides after the fact: the lift has already happened when the first move arrives, 407-1085 ms in, so the card would visibly lift and drop back. It adds a tuned threshold with no device evidence, and it does nothing for a slow lazy swipe. |
| **Grip only, no body hold** | Rejected by the user in favour of keeping the hold on the text as a second path, with the cue to make it visible. The grip alone was proven (`probe-v5.html`). |
| **Removing `draggable`** | iOS then drags the card's text instead, and `issue-status-move.feature:49` breaks. |
| **A grip only for coarse pointers** (`@media (pointer: coarse)`) | The user chose one look everywhere, which also spares hybrid devices a guess. |
| **The grip's `touchstart` listener on the grip or on `#board-columns`** | A listener inside the replaced subtree is detached by every board replace (ADR-BOARD-CARD-001). The `document` listener it takes instead is an added cost, but a modest one: the page already has a non-passive `document` touch listener (the `touchmove` guard), so scroll start already waits on the main thread once; this adds a second wait of the same kind at touchdown. |

### Consequences

- Positive: the grip is a place on the card where a touch means only drag, so it is reliable whatever the thumb does. The body keeps the familiar hold, and the cue shows it coming.
- Negative, accepted by the user: a swipe whose thumb rests on the body for 500 ms or more lifts the card. Releasing it anywhere off a lane changes nothing. The rate is measured at the slice-02 dogfood, not gated.
- Negative: the card's text column is 44 px narrower (124 px at 390 px), so titles wrap onto more lines. Both card sources change markup, and a test must keep them identical.
- Negative: one **additional** non-passive touch listener on `document` (`touchstart`), beside the existing `touchmove` guard: two non-passive `document` touch listeners in all. Its work off a grip is a single check.
- Neutral: the ADR stays Accepted. The Android Chrome run of the rewritten device checklist is still owed; it gates the start of slice-02 DELIVER unless the user waives it (feature-delta DDD-13, U-1).
