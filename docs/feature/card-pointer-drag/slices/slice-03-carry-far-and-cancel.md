# Slice 03 — Carry a card off-screen, and never strand it when interrupted

**Story**: US-CPD-03 | **Estimate**: ~0.75 day | **Depends on**: slice 02 (touch lift) | **Type**: reach + robustness

## Goal

On a 390px phone, a carried card reaches a lane off to the side and a slot
below the fold. A system interruption mid-drag (a call, an OS gesture, a pinch)
puts the card back exactly where it was, with nothing left on screen.

## IN scope

- Horizontal edge auto-scroll of `#board-columns` while a card is carried (the `board-lane-dnd.js::autoScroll` shape: edge zone, step, clamped to the board's extent) (D14).
- Vertical edge auto-scroll of the **page** while a card is carried, if OQ-6 confirms it. Lanes do not scroll internally.
- Marker and activation tracking across any auto-scroll. The POST `after` always equals the marker's slot (AC-3.4).
- Touch `pointercancel` after the lift reverts exactly, with zero residue. Before the lift, the hold is abandoned (D12).
- Release off every lane on touch behaves as the mouse `cancelled` path.
- The 5 `@us-cpd-03 @mobile` scenarios.

## OUT of scope

- Auto-scroll speed curves or acceleration beyond the lane precedent's constant step (tuning is DESIGN's).
- Lane-drag auto-scroll (shipped, unchanged).
- Keyboard card move (D17).

## Learning hypothesis

**Disproves, if it fails:** that edge auto-scroll plus event-time,
point-resolved slotting keeps the marker honest while the content moves under a
still finger. If the marker and the landing disagree after a scroll, the slot
computation reads stale geometry, and CDF's "marker = landing = after"
invariant (OUT-14) is broken on touch.

**Confirms, if it succeeds:** a touch card drag reaches the whole board, and
every way it can end leaves the board as trustworthy as the desk drag.

## Acceptance criteria

AC-3.1 … AC-3.7 (see `feature-delta.md` US-CPD-03).

## Production data

Homelab Ops with **eight** lanes (the board-lane-reorder slice-03 fixture). An
Identity Platform Backlog seeded to **20** cards so the last is below the fold
at 390px. Both are at the `@mobile` viewport.

## Dogfood moment (manual, real devices)

On a real iPhone (Safari) and a real Android phone (Chrome):

| check | iOS Safari | Android Chrome |
|---|---|---|
| Far-lane drop on the 8-lane board; a reload agrees | | |
| Bottom-of-long-lane drop; a reload agrees | | |
| Mid-drag: pull the notification shade / OS back gesture: card back, nothing lit | | |
| Mid-drag: second finger pinch: card back, nothing lit | | |
| Mid-drag: lock the screen and unlock: card back, nothing lit | | |

Also on a narrow desktop window with a real mouse: a far-lane drop by edge
scroll (AC-3.7).

## Reference class

board-lane-reorder `slices/slice-03-edge-autoscroll.md`: the same edge zone,
step and clamping, and the teardown-on-every-exit AC.

## Pre-slice SPIKE

None. OQ-6 (vertical scope) was answered in DESIGN: vertical page auto-scroll
is in scope (DDD-11), so AC-3.3 is unconditional and scenario "Holding a carried
card near the bottom reaches the end of a long lane" stands.
