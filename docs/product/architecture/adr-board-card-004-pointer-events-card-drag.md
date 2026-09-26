# ADR-BOARD-CARD-004: Cards drag on Pointer Events, one hand-authored session for mouse, touch and pen; HTML5 drag-and-drop stays only to swallow foreign drags

## Status

**Accepted (2026-09-25)**, card-pointer-drag DESIGN wave. The user chose
Option A over SortableJS for cards (B) and for cards and lanes (C)
(`docs/feature/card-pointer-drag/feature-delta.md` §DESIGN, "User decisions").
D4 (shipped Gherkin byte-identical) stays binding, and no device bake-off against
SortableJS was run. The user also chose to keep `draggable` with `dragstart`
prevented, to record htmx 4 as a successor feature, and to add a `check-arch`
rule forbidding a `keydown` listener in `board-*.js`.
**Provisional on the device checklist:** if iOS Safari fails spike step 5, the
mechanism question returns to the user with the device evidence.

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

1. **The card drag runs on Pointer Events**, delegated on `document`, scoped to `.issue-card` inside `#board-columns`. Mouse (primary button) lifts past **6 px**. Touch and pen lift after a **350 ms hold within 10 px**; movement first is a scroll, and release first is a tap. The final values are a device feel call.
2. **The shipped session is kept.** `CardDragSession`, `Origin`, `slotFor`, lane activation, the zero-footprint marker, landing at the live marker, `dropInto` and the byte-identical `fetch` POST (`state`, `after`, `x-csrf-token`) are unchanged. Only the event layer under them is replaced.
3. **Lane and slot are resolved from the point** (`elementFromPoint`). The carried ghost has `pointer-events: none`, and `null` means no lane. `event.target` is never used for lane resolution during a session.
4. **Touch posture.** Cards keep `touch-action: auto`. One non-passive `touchmove` listener on `document` prevents the default only while a session is lifted. Cards carry `user-select: none` and `-webkit-touch-callout: none`, and `contextmenu` is prevented during a hold or session.
5. **The carried visual is a fixed clone ghost** offset from the pointer. The origin card stays in its slot, dimmed. No card moves until the drop.
6. **Every exit is explicit.** Escape is a new `keyboard.js::closeTopLayer()` arm directly above the lane-drag arm, found by `html[data-card-dragging]`, and it dispatches `foundry:cancel-card-drag`. `pointercancel` after the lift, a release off every lane, and a card detached by a replace mid-drag each revert to the exact origin with no request. `pointercancel` before the lift abandons the hold.
7. **A one-shot click guard** is armed on any lifted release and reset on the next `pointerdown`, so no drag opens the edit dialog and no stale guard eats a later tap.
8. **Own cards never start a native drag.** Cards keep `draggable="true"`, and `dragstart` on own cards in `#board-columns` is prevented. Removal of `draggable` is a user decision, forced only if the device checklist shows a native drag from a long-press. HTML5 `dragover`/`drop` listeners remain only to swallow foreign drags, as shipped.
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
