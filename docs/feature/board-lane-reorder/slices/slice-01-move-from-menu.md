# Slice 01 — Move a lane from the `⋯` menu

**Story**: US-BLR-01 | **Estimate**: 1 day | **Depends on**: nothing unshipped

## Goal

A lane can be moved one position left or right from the shipped `⋯` menu, on
any device and by keyboard alone, through a single atomic position permutation
that touches no card.

## IN scope

- `board_columns.html`: two new menu items — **Move list left**, **Move list right** — between the Insert pair and Delete list, authored once so both render paths inherit them (D5, D14).
- Disabled-end rendering: Move-left disabled on the first lane, Move-right on the last, derived per render from lane rows — never a cached list (D5, check-arch).
- The move **write port**: one use case behind both surfaces (Driving Port 3), addressing its destination by **neighbour slug**, not by index (D7).
- The store transaction: `FOR UPDATE` on the project's lanes → resolve mover and neighbour by identity inside the lock → apply the whole permutation in **one statement** (D8).
- Refusals: vanished mover or neighbour → uniform non-enumerable 404; non-member and signed-out → the same 404; tokenless POST refused by the middleware (D9, D11).
- `hx-post` menu items carrying CSRF, responding with the OOB `#board-columns` refresh — no dialog (D12).
- Acceptance scenarios in the HTTP lane plus a keyboard-only browser scenario; reuse `assert_lane_labels_in_order` rather than re-implementing a lane-order oracle.
- Post-scenario SQL invariant: positions are a contiguous, duplicate-free permutation (AC-1.5).

## OUT of scope

- Any drag interaction (slice 02) or auto-scroll and drop indicator (slice 03).
- Moving more than one position per activation.
- Changing the Edit, Insert or Delete items' behaviour or copy.

## Learning hypothesis

**Disproves, if it fails:** that the `DEFERRABLE` position constraint absorbs a
*permutation* the way it absorbs an *insert shift*. This is not a re-run of the
predecessor's D8 — insert works because its bulk `+1` vacates the target slot,
and a move has no vacancy: the intervening shift collides with the mover still
sitting in its old slot, and end-of-statement checking does not save a
two-statement shape. A failure here falls back to the sentinel-park sequence
(needing a position value outside the live range) or to
`SET CONSTRAINTS <name> DEFERRED` (moving the failure to COMMIT time); only if
all three fail does slice 01 grow a migration and its estimate move.

**Confirms, if it succeeds:** the single-statement `CASE` permutation is the
general shape for any future lane arrangement operation (drag, multi-move,
"sort by"), and slices 02–03 are pure browser work on a proven port.

## Acceptance criteria

AC-1.1 … AC-1.8 (see `feature-delta.md` US-BLR-01).

## Production data

Scenarios run against seeded projects with real lane rows and real issue rows
("Homelab Ops"/OPS, issues OPS-3/7/9), not synthetic fixtures. The zero-laneless
guard query and the position-permutation invariant query both run after every
mutating scenario.

## Dogfood moment

Same day: open the live board, move a real lane left with the keyboard alone,
confirm every card stayed in its lane.

## Pre-slice SPIKE

**Yes — timeboxed, and it gates the slice.** Prove the chosen permutation shape
(D8) against a live postgres:16-alpine with live-shaped lane data, exactly as
the predecessor's D8 was settled. Measure both candidate shapes: the
single-statement `CASE` permutation, and the sentinel-park sequence (which also
requires confirming that no CHECK constrains `position >= 0`). The spike's
output is the statement the store will run, not an opinion about it.
