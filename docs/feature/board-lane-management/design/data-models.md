# Data Models — board-lane-management

Companion to `architecture-design.md` §3–§4. Owns: lanes DDL, the issues-linkage choice, the
fate of the state CHECK, 0012/0013 interplay, and the TOCTOU/transaction analysis for the
two-fate delete.

## 1. `lanes` table (migration 0015)

```sql
CREATE TABLE lanes (
    id            UUID PRIMARY KEY,
    project_id    UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    slug          TEXT NOT NULL CHECK (slug ~ '^[a-z][a-z0-9_]*$'),
    label         TEXT NOT NULL CHECK (length(label) BETWEEN 1 AND 64),
    position      INTEGER NOT NULL CHECK (position >= 0),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, slug),
    UNIQUE (project_id, position) DEFERRABLE INITIALLY IMMEDIATE
);
```

Notes:

- `UNIQUE (project_id, slug)` is the referent of the issues FK (§2) *and* the idempotency key
  for the migration seed (`ON CONFLICT DO NOTHING`).
- `UNIQUE (project_id, position)` is `DEFERRABLE INITIALLY IMMEDIATE` — behaves as a plain
  unique constraint today; a future reorder (D9 successor) can defer it inside one
  transaction. Deliberate extension point, zero cost now.
- `workspace_id` denormalized, mirroring `issues`/`issue_change_events` (0013 precedent) —
  tenancy rides on the row without a JOIN.
- `slug` is immutable identity, `label` a mutable display value — lanes inherit the
  names-are-labels invariant (brief.md) by construction. The slug CHECK admits exactly the
  shape of the five carried-over state values.
- Migration seeds `id` with `gen_random_uuid()` (PG13+ built-in); application inserts (project
  creation) use the house UUIDv7. The mixed versions are inert — `id` is never ordered by.
- No soft delete, no tombstone: a deleted lane is a removed row (D7/D9; the 0006
  comments-tombstone precedent is explicitly comment-scoped).

## 2. Issues linkage — the decision (ADR-board-lane-001)

**Chosen: keep `issues.state` as the lane slug; add a composite FK.**

```sql
ALTER TABLE issues DROP CONSTRAINT issues_state_check;      -- verify name via pg_constraint at DELIVER
ALTER TABLE issues ALTER COLUMN state DROP DEFAULT;          -- landing rule moves to code (D6)
ALTER TABLE issues ADD CONSTRAINT fk_issues_lane
    FOREIGN KEY (project_id, state) REFERENCES lanes (project_id, slug);
```

Why this and not the alternatives (full ADR: `adr-board-lane-001-issues-linkage-state-fk.md`):

- **`state` is already the universal lane identifier** across every surface: 0012 partitions
  positions by `(project_id, state)`; 0013 `status` events store state slugs as
  `old_value`/`new_value`; `data-column` attributes, the dnd POST body, and the API `state`
  field all carry it. Re-pointing to a `lane_id` (Option A) would fracture all of these or
  force a permanent slug↔id mapping layer.
- **The FK converts D8's "zero laneless issues" from a test assertion into a schema fact** —
  no write path in any adapter, present or future, can strand a card. It also blocks
  `DELETE FROM lanes` while cards reference the lane, which the two-fate transaction (§5)
  exploits as its race guard.
- The default FK action (`NO ACTION`) is exactly right: lane deletion must never cascade into
  issues implicitly — card fate is the operator's explicit, counted decision (D7).
- Dropping `DEFAULT 'backlog'` makes a state-less INSERT a loud error, not a silent landing in
  a lane the project may have deleted (US-BLM-03 scenario 4).

What survives untouched: `idx_issues_project_state_position` (0012) — it still serves the
per-lane ordered scan; `priority` CHECK; every `WHERE state = $n` query in
`reposition_issue_with_outbox` and the board read.

## 3. 0012 interplay — positions

- **Migration**: zero issue-row updates; every `(project_id, state, position)` triple is
  byte-identical before/after 0015 (zero-shuffle, the 0012/watch-item-R5 discipline).
- **Bulk move (fate=move)**: destination column's occupied positions are `0..C-1` (0012
  contiguity invariant, maintained by `renumber_column`). Moved cards, read in
  `ORDER BY position ASC, number DESC` from the dying lane, are assigned `C, C+1, …` in that
  order — append-at-bottom preserving relative order (D7). The source column ceases to exist,
  so no gap-closing pass is needed (unlike a single-card cross-state move). Result: both the
  destination's contiguity and the moved cards' relative order hold by construction.
- **Bulk delete (fate=delete)**: the whole `(project_id, dying_state)` partition vanishes;
  no other column's positions are touched.

## 4. 0013 interplay — change events

- **fate=move**: one `field='status'` row per moved card, `old_value = dying slug`,
  `new_value = destination slug`, `actor_id = operator`, written by the same
  `record_issue_change` helper `reposition_issue_with_outbox` uses, **inside the fate
  transaction** — commit-or-nothing (Inherited commitment: append-only, same-transaction).
  No `rank` events: the cards' positions in the destination are new placements, not reorders
  of an existing column — matching the single-card cross-state move, which records `rank`
  only when position changes *within* the tracked lifecycle; here the status row is the
  user-meaningful record (the change report shows "Todo → Backlog ×3", US-BLM-04 scenario 1).
- **fate=delete**: no events written; each deleted card's entire history cascades away with it
  (`issue_change_events.issue_id ON DELETE CASCADE`, 0013 — the accepted D7 shape).
- **Lane deletion itself is not an event**: `issue_change_events.field` CHECK has no lane
  concept and events are per-issue. A lane-audit trail is out of scope (D9-adjacent); noted,
  not built.
- **Outbox parity**: one `IssueUpdated` outbox row per moved card in the same tx (mirrors
  reposition, store/lib.rs:1608-1621) so SSE/board listeners observe the moves; deletions emit
  nothing (parity with `delete_issue_cascade`, which emits nothing).

## 5. TOCTOU / transaction analysis — the two-fate delete

The threat: the dialog renders "N issues" at GET time; the world moves before the confirm POST
(US-BLM-04 scenario 5 — Priya's own automation files AUTH-21 mid-decision). The design closes
every window inside **one transaction** in `Store::delete_lane_with_fate`:

```text
BEGIN;
  1. SELECT id, label, position FROM lanes
       WHERE project_id=$1 AND slug=$2 FOR UPDATE;         -- dying lane locked
       → none: lane already gone (double-submit race) → uniform 404, nothing written
  2. SELECT count(*) FROM lanes WHERE project_id=$1;        -- ≥1 lane invariant (D6)
       → 1: LastLane → 422, nothing written
  3. (move) SELECT id FROM lanes WHERE project_id=$1 AND slug=$dest FOR UPDATE;
       → none or dest == dying: UnknownDestination → 422
  4. SELECT id, number FROM issues
       WHERE project_id=$1 AND state=$2
       ORDER BY position ASC, number DESC
       FOR UPDATE;                                          -- confirm-time membership, locked
  5a. (move) per card: UPDATE issues SET state=$dest, position=C+idx, updated_at=now();
             per card: INSERT issue_change_events (status, old, new, actor);
             per card: INSERT outbox (IssueUpdated);
  5b. (delete) DELETE FROM issues WHERE id = ANY($ids);     -- cascade: comments/attachments/events
  6. DELETE FROM lanes WHERE id=$lane_id;                   -- FK = strand-guard
COMMIT;
```

Race matrix (READ COMMITTED, the pool default):

| Interleaving | Outcome |
|---|---|
| Card filed into dying lane, **committed before step 4** | Step 4 sees it — moved/deleted with the rest. |
| Card INSERT in flight (uncommitted) at step 6 | The inserting tx holds the lane row's `FOR KEY SHARE` (FK check on its side) — but step 1's `FOR UPDATE` on the lane row means such an insert **blocks at its own FK check until we commit**; after our commit its FK check fails (lane gone) → the filer gets a clean refusal, no strand. |
| Card committed into dying lane between steps 4 and 6 (insert began before our step-1 lock — its FK check predates it) | Step 6 raises `foreign_key_violation` → whole tx rolls back → store retries the operation (≤3 attempts), re-resolving membership; the late card is included. Exhaustion → 500, fully rolled back. |
| Concurrent delete of the same lane | Loser blocks at step 1, then sees no row → uniform 404. |
| Concurrent delete of a *different* lane (move destinations crossing) | Lane locks are taken dying-then-destination; a theoretical AB/BA deadlock between two operators deleting two lanes into each other resolves via Postgres deadlock detection → one tx retried by the same bounded-retry loop. Accepted at homelab scale; documented rather than engineered around. |
| ×/Esc cancel | No POST → no transaction → byte-identical lane, cards, positions, history (scenario 4). |

Why not SERIALIZABLE or an advisory lock (ADR-board-lane-002 alternatives): the composite FK
already provides the only guarantee that matters — *a committed world never contains a card
without a lane* — and `FOR UPDATE` + bounded retry turns the residual anomaly window into a
liveness question, not a correctness one. Isolation escalation would buy nothing the FK does
not already prove, at the cost of a project-wide serialization point on every issue write.

"Live count" honesty: the dialog's N is advisory copy; the fate binds to step 4's membership.
The AC language ("all four cards that were in Todo at confirm time") is implemented literally.

## 6. Creation-time data (D4/D6)

- `insert_project` grows lane seeding in its (new) transaction: `backlog/Backlog/0`,
  `in_progress/In-Progress/1`, `done/Done/2` — exactly three (D4; Todo dropped, Cancelled not
  seeded). The template constant lives in `foundry-store::lanes` as the *creation seed*, a
  documented exemption to the no-static-lane-list rule (it writes lanes; it never renders or
  validates against them).
- `insert_issue_with_outbox` resolves `SELECT slug FROM lanes WHERE project_id=$1
  ORDER BY position ASC LIMIT 1` inside its existing tx and INSERTs `state` explicitly;
  returns the landing slug so `CreatedIssue.state` echoes truth (ripple surface 6). A project
  with zero lanes is unreachable (D6 refuses the last delete; creation seeds three) — the
  query returning no row maps to `Internal`, loudly.
