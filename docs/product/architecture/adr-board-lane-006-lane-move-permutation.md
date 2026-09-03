# ADR-BOARD-LANE-006: A lane move is one CASE permutation statement, serialized by FOR UPDATE

## Status

Accepted (board-lane-reorder DESIGN wave, 2026-09-03)

## Context

`board-lane-reorder` adds "move this lane left/right" — by dragging a column
header, and from the `⋯` menu. DISCUSS (D8) flagged the position permutation as
the feature's only real uncertainty, and flagged it for a specific reason:
**the shipped insert shuffle cannot be reused.**

ADR-BOARD-LANE-003 established that `UNIQUE (project_id, position) DEFERRABLE
INITIALLY IMMEDIATE` (`0015:22`) checks at end-of-*statement*, which is why
`insert_lane_at` can shift later positions with a plain bulk `UPDATE`. That
works because an insert's shift **vacates** the target slot and the new row
then fills it. A move has no vacancy: shifting the intervening range toward the
mover's old slot collides with the mover, which is still sitting in it, so the
shift statement itself ends dirty and end-of-statement checking does not save
it.

DISCUSS named three candidate shapes and instructed DESIGN to **measure all
three rather than confirm a favourite** — explicitly because the predecessor's
D8 had assumed `SET CONSTRAINTS` was required and measurement showed it was
not. The assumption deserved the same scepticism in the other direction.

The measurements below were **run**, not reasoned about: a disposable
`postgres:16-alpine` container (PostgreSQL 16.14, the tag `harness.rs:76` pins
to production) carrying a faithful 0015 reproduction — `lanes` with both unique
constraints and all three CHECKs, a non-deferrable twin table, `issues` with the
`fk_issues_lane` composite FK, and the DISCUSS journey's exact wrong-order
board (`backlog@0 done@1 staging@2 in_progress@3`) with 6 issues across 4 lanes.

## Decision

**Apply the whole permutation in ONE `UPDATE … SET position = CASE …`
statement, inside one transaction that first takes `FOR UPDATE` on the
project's lane rows and resolves both the mover and its destination neighbour
by identity inside the lock.**

```sql
BEGIN;
  SELECT 1 FROM lanes WHERE project_id = $1 ORDER BY position FOR UPDATE;
  SELECT position INTO f FROM lanes WHERE project_id = $1 AND slug = $mover;      -- identity
  SELECT position INTO t FROM lanes WHERE project_id = $1 AND slug = $before;     -- identity
  IF t > f THEN t := t - 1; END IF;   -- removing the mover shifts the neighbour left
  UPDATE lanes SET position = CASE
      WHEN slug = $mover                                     THEN t
      WHEN t < f AND position >= t AND position <  f         THEN position + 1
      WHEN t > f AND position >  f AND position <= t         THEN position - 1
      ELSE position END
    WHERE project_id = $1;
COMMIT;
```

`$before` omitted means "place last" (`t := max(position)`).

### Finding 1 — the insert shape does NOT generalise (the central claim, confirmed)

Reusing `insert_lane_at`'s two-statement shuffle for a move fails:

```
ERROR:  duplicate key value violates unique constraint "lanes_project_id_position_key"
DETAIL:  Key (project_id, "position")=(…, 3) already exists.
```

This is the finding that most shapes this ADR, because the failing shape is
exactly what a reader of `insert_lane_at` would reach for first.

### Finding 2 — all three candidates work, and all three ride on `DEFERRABLE`

| Candidate | Against `DEFERRABLE` (0015) | Against a non-deferrable twin |
|---|---|---|
| (a) one `CASE` permutation statement | **works** — `UPDATE 4`, both directions | `ERROR: duplicate key … =(…, 2) already exists` |
| (b) park at a sentinel, shift, place | **works** — 3 statements | `ERROR: duplicate key … =(…, 2) already exists` |
| (c) `SET CONSTRAINTS … DEFERRED` + the naive two-statement shape | **works** | `ERROR: constraint "…" is not deferrable` |

So the `DEFERRABLE` keyword is not merely convenient for one shape — **it is a
precondition for reordering a lane in any shape we would write.** ADR-003
called the keyword load-bearing for insert; it is load-bearing for move as
well, by three independent routes.

### Finding 3 — a negative sentinel is illegal, which constrains (b)

`position INTEGER NOT NULL CHECK (position >= 0)` (`0015:19`). DISCUSS asked
DESIGN to confirm whether such a CHECK existed. It does:

```
ERROR:  new row for relation "lanes" violates check constraint "lanes_position_check"
```

(b) therefore needs a *high positive* sentinel outside the live range — a
magic number requiring a justification the other candidates do not need.

### Finding 4 — without the lock the failure is SILENT, which is worse than insert's

ADR-003 measured the unlocked insert race as a loud `duplicate key` — a
500-shaped failure the operator should never see, but one that is impossible to
miss. **The unlocked move race is not loud.** Two concurrent moves, with the
second resolving positions inside the first's window:

```
with FOR UPDATE:     staging@0 backlog@1 in_progress@2 done@3   contiguous ✓
without FOR UPDATE:  staging@0 in_progress@1 backlog@2 done@3   contiguous ✓
```

No error is raised. Every invariant holds — contiguous, no duplicates, zero
laneless issues. Both operators' stated intents are even satisfiable on a loose
reading. But `backlog`, which **neither operator mentioned**, has been shoved
past `in_progress`, and the board has settled into an arrangement nobody asked
for. Reproduced 5/5 under a deterministic interleaving; in production the
interleaving would be nondeterministic, making this a heisenbug that no
invariant query can catch.

This makes `FOR UPDATE` *more* necessary for move than it was for insert, not
less: for insert the lock prevents a visible error, for move it prevents a
silent wrong answer.

### Finding 5 — zero issue rows, across every measurement

An `EXCEPT`-based diff against a pre-spike snapshot of all 6 issue rows
returned **0 differing rows** after every candidate, every direction, and every
concurrency run. A lane move is a lane-set operation only (D1).

## Alternatives Considered

| Alternative | Rejected because |
|---|---|
| (b) Sentinel park, three statements | Works, but needs a high-positive magic sentinel (Finding 3), triples the statement count, and still depends on `DEFERRABLE` for its shift — so it buys no robustness for its extra machinery. |
| (c) `SET CONSTRAINTS lanes_project_id_position_key DEFERRED` | Works, and was taken seriously — DISCUSS explicitly forbade dismissing it, since the predecessor's D8 guessed wrong in the opposite direction. Rejected on two counts: it moves the failure from the offending statement to `COMMIT`, which degrades attribution when something *else* in the transaction is at fault; and it hard-codes the constraint's auto-generated name into Rust, so a future migration renaming the constraint breaks lane reorder at runtime with every test still green. (a) needs neither. |
| Reusing insert's two-statement shuffle | Measured to fail (Finding 1). |
| Dropping the position unique constraint | Would make every shape work by removing the only DB-level guard against two lanes in one slot. The constraint is doing real work, and this repo's lane design consistently prefers schema facts to test assertions. |
| Fractional / lexicographic ranking (LexoRank-style) | Genuinely the strongest answer at scale: one row written per move, no shuffle ever. Rejected on blast radius — it requires migrating off `UNIQUE (project_id, position)`, which the shipped insert depends on, and it would split the repo's two ordering systems (`0012` chose contiguous integers for cards) into two disciplines. Revisit if lane counts ever leave homelab scale. |
| Optimistic retry instead of `FOR UPDATE` | The retry idiom exists here (ADR-BOARD-LANE-002), but retry keys off a raised error, and Finding 4 shows the unlocked move race **raises none**. There is nothing for a retry to trigger on. |
| Resolving the destination as a numeric index rather than a neighbour slug | The index is stale the instant another operator inserts or deletes a lane. Same trap ADR-003 recorded for insert; the `IF t > f THEN t := t - 1` adjustment is only correct *because* both ends are resolved by identity inside the lock. |

## Consequences

- Slice 01 needs **no migration**; the counter stays at 0015. AC-1.5 and AC-1.6 are satisfied by measurement rather than hope.
- **The `check-arch` rule pinning `DEFERRABLE` on `0015:22` is no longer optional.** ADR-003 recommended it and it was carried forward unimplemented. Finding 2 raises the stakes: a migration dropping that keyword now breaks lane insert *and* lane move, by three routes, while every existing test stays green. This ADR treats the rule as a DoD item for `board-lane-reorder`, not a suggestion.
- The store gains a concurrency test whose oracle is **the resulting order**, not the absence of an error — the shipped insert concurrency test can assert "no error raised", and for move that assertion would pass on the corrupt case (Finding 4).
- Contiguity remains a convention with no DB constraint; the acceptance oracles stay its only enforcement, which is why AC-1.5 asserts it explicitly after every move.
- `move_lane`'s `NOOP` arm (destination equals current position) returns without writing, so a drag that lands where it started costs no transaction.
