# Slice 02 — Drag a card on a phone by its grip, or hold it to lift it; swipe and tap unchanged

**Story**: US-CPD-02 | **Estimate**: ~1.25 days | **Depends on**: slice 01; the DESIGN touch spike (OQ-2/3/4, run 2026-09-29) | **Type**: new capability

*Amended 2026-09-29 after the DDD-13 device run (D5 as amended). The slice was
"Press and hold to lift a card on a phone". A whole-card hold could not tell
Priya's resting-thumb swipe from a hold, so touch and pen now lift from a grip
at once, or from the card body by a 500 ms hold with an arming cue. The file
name is kept because the roadmap references it.*

## Goal

At 390px, Priya puts her thumb on a card's grip and moves, and it lifts at
once. Or she holds the card's text for half a second, sees it arm, and it
lifts. She carries it to an exact slot and releases, with the same lit lane,
marker, POST and revert as the mouse. A quick swipe across a card's text still
scrolls the board, and a tap on it still opens the card.

## IN scope

- ~~The hold rule for `pointerType` touch and pen: lift after the hold duration within the hold tolerance (OQ-1). Moving past the tolerance first leaves the gesture to the browser (scroll). Releasing first is a tap (the edit dialog opens) (D5).~~ *Amended 2026-09-29:*
- The grip, on every card, in both card sources (`issue_card.html`, `issues.rs:731`), fresh and after an in-place refresh. A touch or pen on it lifts the card after a few pixels of travel, with no hold, and never scrolls. A tap on it opens nothing (D5, AC-2.10, 2.11, 2.13).
- The body hold for `pointerType` touch and pen: lift after 500 ms within the hold tolerance (OQ-1, resolved). Moving past the tolerance first leaves the gesture to the browser (scroll). Releasing first is a tap (the edit dialog opens) (D5).
- The arming cue over a body hold, cleared on every abort (AC-2.12).
- After the lift, finger movement carries the card and does not scroll (the `touch-action` strategy chosen by the spike, OQ-2).
- Suppression of OS long-press behaviours on cards: selection, callout, context menu, native drag, and the iOS long-press drag that stalls the grip (OQ-3, OQ-4; System Constraints).
- A visible lift announcement at the moment of lift (D15; haptics optional, OQ-10).
- One pointer, one session: other pointers are ignored (D16).
- The 7 `@us-cpd-02 @mobile` scenarios (was 5). Stylesheet re-hash (D18). ~~If OQ-4 changes `draggable`, both card sources change (`issue_card.html`, `issues.rs:731`), and only after a user decision about `issue-status-move.feature:49`.~~ Both card sources change for the grip; `draggable` stays as DDD-19 decided.

## OUT of scope

- Edge auto-scroll while carrying, and touch `pointercancel` scenarios (slice 03). The revert code path already exists from slice 01.
- Any change to lane-header touch behaviour (AC-2.8 guards it).
- Any change to the mouse gesture (D6, slice 01). Whether a mouse click on the grip opens the card is DESIGN's (OQ-13).
- Keyboard card move (D17).

## Learning hypothesis

~~**Disproves, if it fails:** that one element can mean three things to a finger
(swipe = scroll, tap = open, hold = lift) on both mobile engines without
giving any of them up.~~ *Disproved on 2026-09-29 on a real iPhone: 14 of 30
swipes lifted (`spike/findings.md`).*

**Disproves, if it fails (amended 2026-09-29):** that a grip gives the drag a
reliable place of its own, while the card's text keeps swipe = scroll and
tap = open, with a 500 ms hold as a second way in. If the grip lifts late or
scrolls on either engine, or the lazy-swipe misfire rate at 500 ms is one the
user will not live with, D5 returns to the user.

**Confirms, if it succeeds:** the touch drag is the mouse session plus two lift
rules. Slice 03 only adds reach and interruption handling.

## Acceptance criteria

AC-2.1 … AC-2.14 (see `feature-delta.md` US-CPD-02; AC-2.10 to AC-2.14 added
2026-09-29). AC-2.2's real scroll, AC-2.5 and AC-2.14 are **dogfood ACs**:
synthetic `PointerEvent`s cannot make a browser scroll, show a callout or
reproduce a resting thumb (D19). If DESIGN adopts WebDriver touch Actions
(OQ-9), part of AC-2.2 may move into the lane.

## Production data

Identity Platform and Homelab Ops at the `@mobile` 390px viewport, the
board-lane-reorder precedent.

## Dogfood moment (manual, real devices)

On a real iPhone (Safari) and a real Android phone (Chrome), on Priya's
instance:

| check | iOS Safari | Android Chrome |
|---|---|---|
| 5 grip drags land at the marker, lift at once; a reload agrees *(added 2026-09-29)* | | |
| 5 body-hold drops land at the marker; the arming cue shows over the hold; a reload agrees | | |
| 20 prompt swipes across card text: lifts (target 0) | | |
| 20 natural swipes across card text: resting-thumb lifts at 500 ms (recorded, not gated; KPI 3) *(added 2026-09-29)* | | |
| 10 taps on card text: dialog opens every time | | |
| 10 taps on the grip: nothing opens *(added 2026-09-29)* | | |
| Body hold then release in place: no dialog, card unmoved | | |
| No selection / callout / context menu / native drag on hold or on the grip | | |
| Body hold feels neither sluggish nor twitchy at 500 ms | | |

**KPI 7 log**, one row per working day for 5 days from the day slice 02 is on
the instance: cards moved by touch / moves deferred to the desk.

## Pre-slice SPIKE (DESIGN, gating)

*Run 2026-09-29 on an iPhone 13 Pro Max, iOS 26.7 (`spike/findings.md`). Android
Chrome is still owed at dogfood. Result: (1) option (a) holds after the lift on
WebKit; (2) no hold value separates a resting swipe from a hold, which led to
the grip; (3) with `draggable="true"` iOS starts its own drag ~655 ms into a
still press, and a grip needs its `touchstart` prevented.*

On real iOS Safari and Android Chrome, with a throwaway page carrying the real
card markup:

1. `touch-action` candidates (a) auto plus a non-passive `touchmove` `preventDefault` after lift, (b) `none`, (c) `pan-y`. Does a swipe that starts on a card scroll, and does a post-lift move stay put?
2. Hold timing vs the OS long-press: which fires first at 300 / 350 / 400 ms?
3. `draggable="true"`: does a long-press start a native drag on either engine?
4. Callout, selection and context menu under `-webkit-touch-callout: none` + `user-select: none` + `contextmenu` suppression.

Record the findings in the ADR (OQ-8). If no candidate satisfies (1), stop and
return to the user before planning this slice.
