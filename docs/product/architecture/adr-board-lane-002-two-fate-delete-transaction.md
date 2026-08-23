# ADR-BOARD-LANE-002: Lane delete + card fate is one transaction; the lane FK is the strand-guard, with bounded retry

## Status

Accepted (board-lane-management DESIGN wave, 2026-08-22)

## Context

Deleting a lane holding N ≥ 1 cards applies an operator-chosen fate — move-all
to a surviving lane (append-at-bottom, one 0013 `status` event per card) or
delete-all (hard cascade, `delete_issue_cascade` shape) — and D7/D8 pin: one
atomic operation, no observable laneless intermediate state, and the fate binds
to the lane's membership **at confirm time** (a card filed while the dialog is
open must never be stranded — US-BLM-04 scenario 5). The dialog's card count is
therefore advisory copy only. Store runs Postgres at READ COMMITTED; homelab
scale (single node, single-digit concurrent writers).

## Decision

`Store::delete_lane_with_fate` executes one transaction: lock the dying lane
row (`FOR UPDATE`) → refuse if it is the project's last lane → lock and resolve
confirm-time membership (`FOR UPDATE`, `position ASC, number DESC`) → apply the
fate arm (move: `state`+`position` updates + one same-tx 0013 `status` event +
one outbox `IssueUpdated` per card; delete: `DELETE … WHERE id = ANY(ids)`,
cascades take comments/attachments/history) → delete the lane row → commit.

The composite FK from ADR-BOARD-LANE-001 makes the final `DELETE FROM lanes`
the race guard: a card committed into the dying lane after the membership
snapshot raises `foreign_key_violation` on that statement, rolling back the
whole operation. The store retries the operation (≤3 attempts), re-resolving
membership, so the late card is included in the fate. A filer whose INSERT
lands after the lane delete commits gets a clean FK refusal — never a stranded
card. Retry exhaustion → internal error, fully rolled back. Cancel (×/Esc)
sends no request: zero writes, zero events.

## Alternatives Considered

- **A. SERIALIZABLE isolation for the fate transaction** — Rejected. Buys no
  correctness the FK does not already prove (a committed world can never hold a
  laneless card), still requires a retry loop (serialization failures), and
  escalates every concurrent issue write in the project into a potential
  aborter. Strictly more machinery for the same guarantee.
- **B. Postgres advisory lock per project around lane deletes and issue
  writes** — Rejected. Correct only if *every* issue-writing path takes the
  lock — a convention spanning all current and future adapters, exactly the
  kind of unenforced discipline this feature exists to remove. The FK holds
  regardless of who forgets.
- **C. Two-phase UX: bulk-move/bulk-delete cards first, then delete the empty
  lane as a separate operation** — Rejected. Creates the observable
  intermediate state D7 forbids (an emptied-but-present lane, or a deleted lane
  with a failed second phase), doubles the confirm ceremony, and still needs
  the race guard for the window between phases.

## Consequences

- Positive: atomicity, confirm-time-truth, and no-strand hold by schema
  construction plus a small bounded retry; the change report shows exactly one
  `status` row per moved card, attributed to the operator, same-tx (0013
  invariants intact); 0012 contiguity holds by construction (destination
  append `C..C+N-1`; source partition vanishes whole).
- Negative: a pathological sustained writer into a dying lane can exhaust the
  3 retries → 500 (operator retries; acceptable at homelab scale, documented).
  Cross-lane AB/BA deadlocks between two concurrent deletes resolve via
  Postgres deadlock detection into the same retry path. N cards produce N
  events + N outbox rows in one tx — fine for homelab card counts; a future
  bulk-events optimisation would be a store-internal change behind the same
  port.
- The design is exercised empirically (Earned Trust): a gold HTTP-lane test
  injects a concurrent filing between dialog render and confirm and asserts
  the zero-laneless guard query after commit.
