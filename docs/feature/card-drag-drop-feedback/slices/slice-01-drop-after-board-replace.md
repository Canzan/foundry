# Slice 01 — Drops keep working after the board refreshes in place

**Story**: US-CDF-01 | **Estimate**: 0.75 day | **Depends on**: nothing unshipped | **Type**: bug, test-first

## Goal

A card dragged over any lane is accepted after the board has refreshed itself in
place — after each of the 5 in-place refreshes, and after the 1 lane-header
reorder kept as a robustness guard (DISTILL Upstream Issue #1) — with the regression proven
RED on HEAD before the fix.

## IN scope

- **First, before any production change:** a `@needs-browser` regression scenario that replaces `#board-columns` in place (⋯ **Move list right** on Homelab Ops) and then drags **without navigating or reloading**, asserting the synthetic `dragover` on the replacement lane is `defaultPrevented`. Recorded RED on HEAD (`aa8a6f6`) for that reason (D2, AC-1.2).
- A new drag step that does not call `open_board_in_browser`; `drag_a_card` (`feature_board_lane_reorder.rs:1137`) reloads first and must not be reused for this scenario.
- The outline covering the other four in-place refreshes: popup card delete (`87282d3`), lane edit, lane insert (drop into the brand-new lane), lane delete with move fate. The lane header drag is its own guard scenario: it rearranges the lanes without replacing them (DISTILL Upstream Issue #1).
- Event-time lane resolution for accepting and performing a drop; nothing bound per lane at load (D3).
- The guard: only a drag that began on an `.issue-card` on this page moves a card. Every other drop on the board (a file, or text from another app) is **swallowed**, with no move and no navigation, on a fresh load and after each refresh (D4, AC-1.5). The swallow is proven by the outline scenario in the browser lane and by a real Finder drop at dogfood. In-flight state is cleared on every drag end (D5).
- Proof that the fresh-load path and the POST body are byte-identical (AC-1.4, AC-1.7).

## OUT of scope

- Any visual feedback (slices 02–04).
- Any change to the `/state` endpoint, the `after` wire format, ordering or revert (D9).
- The lane drag module (`board-lane-dnd.js`) beyond being one of the triggers.
- A build-time (check-arch) guard against per-lane binding: ruled out (D15). This slice's refresh-then-drag scenarios are the guard.

## Learning hypothesis

**Disproves, if it fails:** that all five in-place refreshes share one cause —
listeners bound to lanes that no longer exist — and that resolving the lane at
event time fixes every one. If a trigger still refuses after the change (e.g.
an htmx swap, or the header drag's pointer handling on the reorder guard, interferes), the RCA's cause
is incomplete and the slice stops to re-investigate rather than patch per
trigger.

**Confirms, if it succeeds:** board scripts can be made replace-proof by
construction, and slices 02–04 can ride the same event-time resolution instead
of each re-solving it.

## Acceptance criteria

AC-1.1 … AC-1.7 (see `feature-delta.md` US-CDF-01).

## Production data

Seeded Identity Platform (AUTH-3/12/19/41/42/43) and Homelab Ops (OPS-3/7/9,
empty Staging) boards in the containerised `selenium/standalone-chrome` the
acceptance lane uses — the same environment the RCA reproduced the refusal in.

## Dogfood moment (manual, real browser)

Same day, on Priya's own instance in Chrome and in Firefox: delete a card from
its popup, then drag another card across lanes **without reloading**; repeat
after ⋯ Move list right and after Insert list after (drop into the new lane).
Drag `keys.png` from Finder onto a lane and onto the gap between lanes, on a
fresh load and again after a refresh: nothing moves and the tab does not open
the file. Record the result in
the delivery notes.

**KPI 2 daily log** (the KPI's only instrument), one row per working day for
5 working days from the day slice 01 is on Priya's instance:

| date | refused drops | pre-emptive reloads | notes |
|---|---|---|---|

Recorded in this slice's delivery notes, then carried into the finalize
`docs/evolution/` entry. Any refused drop reopens US-CDF-01.

## Reference class

`1d91ad8` (the full-page dead close control: a board-surface bug fixed with a
browser-lane regression) and `board-live.js`'s "hold nothing" rule.

## Pre-slice SPIKE

None — the RCA already reproduced both HEAD and the delegated fix.
