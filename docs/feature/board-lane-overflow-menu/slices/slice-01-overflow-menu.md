# Slice 01 — The `⋯` overflow menu replaces the armed `×`

**Story**: US-BLO-01 | **Estimate**: 1 day | **Depends on**: nothing

## Goal

Every board column header carries one unobtrusive `⋯` menu trigger instead of a
permanently-armed `×`, and the menu's four items route to their handlers —
Delete list reaching the shipped delete dialog unchanged.

## IN scope

- `board_columns.html`: replace the `button.lane-delete` with the menu trigger + menu markup (authored once; both render paths inherit it — D14).
- Menu open/close/focus behaviour in `keyboard.js`, using the existing layer mechanism — **no second `Escape` listener** (D9, BR-4).
- Full menu keyboard semantics: trigger activation, item traversal, `Escape` closes and returns focus to the trigger (D10).
- Menu CSS in the content-hashed stylesheet, using existing canzan tokens, correct in both palettes; `static/VENDOR.md` row updated in the same commit.
- Delete list wired to the shipped `show_delete_lane_dialog` GET — behaviour, copy and fate arms unchanged.
- Edit / Insert-before / Insert-after items rendered and reachable, routed to their (not-yet-built) handlers.
- Re-premise the two shipped browser scenarios that click `button[data-lane-delete]` directly (`feature_board_lane_management.rs:2427, 2565`) to open the menu first (D13).

## OUT of scope

- Any rename behaviour (slice 02) or insert behaviour (slice 03).
- Changing the delete dialog itself.
- Reordering lanes; archive in any form.

## Learning hypothesis

**Disproves, if it fails:** that a popup menu can be added to the board without
violating BR-4 — i.e. that `closeTopLayer()`'s single-`Escape`-owner model
extends to a non-modal layer. A failure here means the menu needs its own
dismissal mechanism, which is an architecture change (a second layer kind) and
would force an ADR before slices 02–03 can land their dialogs.

**Confirms, if it succeeds:** the layer model generalises, and every later
board affordance (reorder, sort, WIP limits) has a home that costs template +
CSS only.

## Acceptance criteria

AC-1.1 … AC-1.7 (see `feature-delta.md` US-BLO-01).

## Production data

The board scenarios run against seeded projects with real lane rows and real
issue rows ("Homelab Ops"/OPS), not synthetic fixtures. The zero-laneless guard
query runs after every mutating scenario.

## Dogfood moment

Same day: open the live board, click `⋯` on a real column, delete a real empty
lane through the menu.

## Pre-slice SPIKE

None — every mechanism this slice touches is shipped.
