# ADR-003: Export store-fn design + read consistency

## Status
Accepted

## Context
The export reads all 10 tenant tables for workspace W. If the reads are independent (10 separate
queries, no shared transaction), a concurrent writer could insert a comment between the `issues` read
and the `comments` read, producing an archive whose `comments.issue_id` references an issue the
archive does NOT contain — a broken referential closure that verify would flag, even though the
instance is perfectly consistent. The scope predicate (the crux) must also live in exactly ONE place
to guarantee selection == isolation.

## Decision
- A single new store fn `Store::export_workspace(W) -> WorkspaceExport` that opens ONE transaction at
  **`REPEATABLE READ`**, runs all 10 scoped SELECTs inside it (each returning whole-row
  `to_jsonb(t.*)::text`), and returns the per-table row sets + counts. Read-only — the tx never
  writes (read-only feature, no migration).
- The 10 `WHERE` clauses (the scope predicate, architecture.md Section 5) live in this one fn — the
  single source of truth for "belongs to W". The same predicate definition is what verify re-applies.
- Invoked from `admin_cli.rs` via the shipped thread-isolated tokio runtime + `Store::connect`
  pattern (mirrors `run_provision_workspace`).

## Alternatives Considered
1. **Ten independent queries, no shared tx.** Rejected: no consistent cut; a concurrent write can
   break referential closure and red verify spuriously.
2. **`SERIALIZABLE` isolation.** Rejected: unnecessary for a read-only snapshot; `REPEATABLE READ`
   gives a consistent cut without serialization-failure retries, and Postgres' `REPEATABLE READ` is
   a true snapshot.
3. **One generic `dump_scoped(table, predicate)` called 10 times from admin_cli.** Rejected:
   scatters the predicate across call sites (crux risk) and pushes SQL construction into the adapter;
   keeping it in `foundry-store` keeps the seam testable and the predicate centralized.

## Consequences
- Positive: archive is a single consistent cut; referential closure holds.
- Positive: the scope predicate is in ONE function — selection and isolation cannot diverge by
  construction.
- Positive: unit-testable at the store seam (seed two workspaces, export one, assert no sibling rows).
- Negative: a long-running export holds a read snapshot — acceptable for an operator batch job at
  operator cadence (not a hot path).
