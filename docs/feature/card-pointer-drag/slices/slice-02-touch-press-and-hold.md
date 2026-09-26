# Slice 02 — Press and hold to lift a card on a phone; swipe and tap unchanged

**Story**: US-CPD-02 | **Estimate**: ~1 day (after the spike) | **Depends on**: slice 01; the DESIGN touch spike (OQ-2/3/4) | **Type**: new capability

## Goal

At 390px, Priya holds a card for about a third of a second and it lifts. She
carries it to an exact slot and releases, with the same lit lane, marker, POST
and revert as the mouse. A quick swipe across a card still scrolls the board,
and a tap still opens the card.

## IN scope

- The hold rule for `pointerType` touch and pen: lift after the hold duration within the hold tolerance (OQ-1). Moving past the tolerance first leaves the gesture to the browser (scroll). Releasing first is a tap (the edit dialog opens) (D5).
- After the lift, finger movement carries the card and does not scroll (the `touch-action` strategy chosen by the spike, OQ-2).
- Suppression of OS long-press behaviours on cards: selection, callout, context menu, native drag (OQ-3, OQ-4).
- A visible lift announcement at the moment of lift (D15; haptics optional, OQ-10).
- One pointer, one session: other pointers are ignored (D16).
- The 5 `@us-cpd-02 @mobile` scenarios. Stylesheet re-hash (D18). If OQ-4 changes `draggable`, both card sources change (`issue_card.html`, `issues.rs:731`), and only after a user decision about `issue-status-move.feature:49`.

## OUT of scope

- Edge auto-scroll while carrying, and touch `pointercancel` scenarios (slice 03). The revert code path already exists from slice 01.
- Any change to lane-header touch behaviour (AC-2.8 guards it).
- Keyboard card move (D17).

## Learning hypothesis

**Disproves, if it fails:** that one element can mean three things to a finger
(swipe = scroll, tap = open, hold = lift) on both mobile engines without
giving any of them up. If the spike or dogfood shows a swipe starting on a card
no longer scrolls, or the OS long-press wins the race, then press-and-hold on
the whole card is the wrong affordance for this board. The fallback, a grip
handle or axis-restricted `touch-action`, is a trade-off that returns to the
user.

**Confirms, if it succeeds:** the touch drag is the mouse session plus one lift
rule. Slice 03 only adds reach and interruption handling.

## Acceptance criteria

AC-2.1 … AC-2.9 (see `feature-delta.md` US-CPD-02). AC-2.2's real scroll and
AC-2.5 are **dogfood ACs**: synthetic `PointerEvent`s cannot make a browser
scroll or show a callout (D19). If DESIGN adopts WebDriver touch Actions
(OQ-9), part of AC-2.2 may move into the lane.

## Production data

Identity Platform and Homelab Ops at the `@mobile` 390px viewport, the
board-lane-reorder precedent.

## Dogfood moment (manual, real devices)

On a real iPhone (Safari) and a real Android phone (Chrome), on Priya's
instance:

| check | iOS Safari | Android Chrome |
|---|---|---|
| 5 hold-and-drop moves land at the marker; a reload agrees | | |
| 20 swipes across cards: accidental lifts (target 0) | | |
| 10 taps: dialog opens every time | | |
| Hold then release in place: no dialog, card unmoved | | |
| No selection / callout / context menu / native drag on hold | | |
| Hold feels neither sluggish nor twitchy (note the ms value) | | |

**KPI 7 log**, one row per working day for 5 days from the day slice 02 is on
the instance: cards moved by touch / moves deferred to the desk.

## Pre-slice SPIKE (DESIGN, gating)

On real iOS Safari and Android Chrome, with a throwaway page carrying the real
card markup:

1. `touch-action` candidates (a) auto plus a non-passive `touchmove` `preventDefault` after lift, (b) `none`, (c) `pan-y`. Does a swipe that starts on a card scroll, and does a post-lift move stay put?
2. Hold timing vs the OS long-press: which fires first at 300 / 350 / 400 ms?
3. `draggable="true"`: does a long-press start a native drag on either engine?
4. Callout, selection and context menu under `-webkit-touch-callout: none` + `user-select: none` + `contextmenu` suppression.

Record the findings in the ADR (OQ-8). If no candidate satisfies (1), stop and
return to the user before planning this slice.
