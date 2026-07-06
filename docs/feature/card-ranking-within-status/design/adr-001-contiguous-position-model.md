# ADR-001 — Contiguous `position` rank model + zero-shuffle migration

**Status**: Accepted (user-ratified 2026-07-06) · **Feature**: card-ranking-within-status · **Slice**: 01

## Context

The board needs a persisted, shared, per-`(project, state)` ordering. Today order is incidental
(`ORDER BY number DESC`, `lib.rs:1246`) and the `issues` table has no ordering column (`0001_init.sql:64`). We
must choose a rank representation and a migration that preserves every existing board's look.

## Decision

**A single `position INTEGER NOT NULL DEFAULT 0` on `issues`, kept contiguous (`0..N-1`) per `(project_id,
state)`, reindexed inside one transaction per move.**

- **Read**: `list_issues_by_project` orders `position ASC, number DESC` (deterministic tiebreak). The existing
  per-state filter in `build_board_page` (`projects.rs:591`) preserves this order into columns — so ordering the
  query is the entire read change; no card attribute or struct field for position is required.
- **Index**: `idx_issues_project_state` → `(project_id, state, position)` (covers the state filter + ordered scan).
- **Move**: in one tx, close the gap in the source `(project, old_state)` and open a slot in the target
  `(project, new_state)`, then set the moved row's `position` (+ `state` if changed). Both columns are contiguous
  afterward. Mechanism (gap-shift `UPDATE` vs `row_number()` recompute of the affected column) is DELIVER's;
  the invariant + single-tx are fixed here.
- **Migration `0012`** (first since `0011`): add the column, then backfill
  `UPDATE issues SET position = sub.rn - 1 FROM (SELECT id, row_number() OVER (PARTITION BY project_id, state
  ORDER BY number DESC) AS rn FROM issues) sub WHERE issues.id = sub.id`. This reproduces the current
  `number DESC` order per status → **zero visible shuffle** on first render.
- **New-issue default slot**: top of Backlog (`position 0`, existing Backlog rows shifted `+1`) in the insert tx,
  preserving today's newest-first feel (ODD-5).

## Why (vs alternatives)

| Model | Move cost | Tail risk | Code | Verdict |
|-------|-----------|-----------|------|---------|
| **Contiguous position, reindex-in-tx** | O(N) rows in the affected column(s) | none (always clean) | low | **Chosen** — simplest *correct* model; N≈dozens per column makes O(N) negligible; trivial deterministic backfill; deterministic read |
| Fractional (float8) | 1 row | precision exhaustion after ~50 midpoint inserts between a pair → needs a renormalize job; float ties | low-med | Rejected — a background renormalize + an exhaustion edge case for zero benefit at this scale |
| Lexorank (string) | 1 row | none | high | Rejected — a whole rank algebra/library; overkill for a small self-hosted board |

At kanban-column scale, correctness-and-simplicity beats write-amplification savings. Contiguous positions make
concurrency reasoning trivial (a column is always a valid permutation) and the migration a one-liner.

## Consequences

- Every move writes multiple rows (the affected column(s)); acceptable and transactional.
- New-issue create now shifts the Backlog column (+1) — a bounded extra UPDATE in the existing insert tx.
- Concurrency is last-writer-wins on the column; the recompute guarantees no corruption (worst case: a lost
  intended slot → re-drag). `SELECT … FOR UPDATE` on the column is the escalation path if ever needed (not v1).
- Consumers that assumed `number DESC` (e.g. the hidden keyboard-nav carrier, which sorts by `number` itself at
  `projects.rs:609-610`) are unaffected — that list sorts independently of the column order.
