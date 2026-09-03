# ADR-BOARD-LANE-003: Lane insert shuffles positions under the deferrable constraint, serialized by FOR UPDATE

## Status

Accepted (board-lane-overflow-menu DESIGN wave, 2026-09-02)

## Context

`board-lane-overflow-menu` adds "Insert list before/after", which must place a
new lane mid-board and move every later lane one position right. `lanes` carries
`UNIQUE (project_id, position) DEFERRABLE INITIALLY IMMEDIATE`
(`0015_project_lanes.sql:22`).

DISCUSS (D8) flagged this as the feature's only real uncertainty and assumed a
`SET CONSTRAINTS ... DEFERRED` window would be required, with migration 0016 as
the fallback. DESIGN was required to prove the mechanism against live-shaped data
before slice 03 could be planned.

The measurements below were **run**, not reasoned about: a disposable
`postgres:16-alpine` container (the tag `harness.rs:76` pins to production)
carrying a faithful 0015 reproduction — `lanes` with both unique constraints and
the CHECKs, `issues` with the `fk_issues_lane` composite FK, 3 projects, 2–5
lanes each, 8 issues.

## Decision

**Shuffle with a plain `UPDATE`, inside one transaction that first takes
`FOR UPDATE` on the project's lane rows, resolving the anchor by identity and
capturing its position before the shift. No `SET CONSTRAINTS`. No migration.**

```sql
BEGIN;
  SELECT 1 FROM lanes WHERE project_id = $1 ORDER BY position FOR UPDATE;
  SELECT position INTO at FROM lanes WHERE project_id = $1 AND slug = $2;  -- identity
  at := at + (side = 'after')::int;                                        -- capture BEFORE
  -- slug uniqueness pre-checked here, inside the lock
  UPDATE lanes SET position = position + 1 WHERE project_id = $1 AND position >= at;
  INSERT INTO lanes (project_id, slug, label, position) VALUES ($1, $3, $4, at);
COMMIT;
```

Three findings make this the decision:

**1. `DEFERRABLE INITIALLY IMMEDIATE` checks after each *statement*, not each
row.** The naive bulk shift therefore commits: at end-of-statement the positions
are already unique.

**2. The `DEFERRABLE` keyword is load-bearing.** The identical statement against
a non-deferrable `UNIQUE (project_id, position)` fails:

```
ERROR:  duplicate key value violates unique constraint "lanes_nd_project_id_position_key"
DETAIL:  Key (project_id, "position")=(…, 2) already exists.
```

**3. Concurrency needs the lock.** Two concurrent inserts at the same anchor
without one: the loser aborts with a raw `duplicate key` error (no corruption,
but a 500-shaped failure the operator should never see). With `FOR UPDATE` and
an identity-resolved anchor: **both commit**, positions contiguous 0–6, zero
issue rows touched.

Consequently **no migration is required; the counter stays at 0015.**

## Alternatives Considered

| Alternative | Rejected because |
|---|---|
| `SET CONSTRAINTS ALL DEFERRED` in the insert transaction | Measured as unnecessary — the constraint already defers to end-of-statement. Adding it implies a constraint that does not exist and invites the reader to believe per-row checking was the problem. |
| Migration 0016 making the constraint `INITIALLY DEFERRED` | Solves nothing that is broken, and spends a forward-only migration on live homelab data for no observable gain. |
| Migration 0016 dropping the position unique constraint entirely | Removes the only DB-level guard against two lanes occupying one slot. The constraint is doing real work. |
| Sparse positions (gaps of 100) to avoid shuffling | Trades a solved problem for an unsolved one: gaps still collide eventually, contiguity assertions become untrue, and the delete path (which preserves contiguity today) would need reworking. |
| Optimistic retry on duplicate-key instead of `FOR UPDATE` | The retry idiom exists in this codebase (ADR-BOARD-LANE-002 uses bounded retry), but here the contended resource is one small per-project row set and the lock is uncontended at homelab scale. `FOR UPDATE` is the simpler correct thing and matches `delete_lane_with_fate`. |
| Capturing the insert position at dialog-render time | The trap that made the first spike attempt fail. "Insert before Done" must keep meaning *before Done*, not *at slot 3*, if another operator inserts meanwhile. Same shape as D7's "the count is advisory; the fate binds at confirm time". |

## Consequences

- Slice 03 needs no migration; AC-3.9 is satisfied by measurement rather than hope.
- **A future migration that drops `DEFERRABLE` from `0015:22` silently breaks lane
  insert while every existing test stays green** (nothing today shifts positions).
  `architecture-design.md` §6 recommends a `check-arch` rule pinning the keyword,
  in the spirit of the no-static-lane-list and `fn slugify(` rules. Until that
  rule exists, this ADR is the only guard.
- The store gains a genuine concurrency test (the spike's test 5b), not just a
  happy-path insert test.
- Contiguity remains a convention with no DB constraint; the acceptance oracles
  are its only enforcement, which is why AC-3.2 asserts it explicitly.
