-- crates/foundry-store/migrations/0012_issue_position.sql
-- card-ranking-within-status — Slice 01 (ADR-001): a persisted, shared,
-- per-(project, state) rank.
--
-- The board had no ordering column (0001_init.sql:64); order was incidental
-- (`ORDER BY number DESC`). We add a single contiguous `position` per
-- (project_id, state), read `ORDER BY position ASC, number DESC`.
--
-- Backfill is ZERO-SHUFFLE: `row_number() OVER (PARTITION BY project_id, state
-- ORDER BY number DESC) - 1` reproduces the current number-DESC order per
-- status, so every existing board's first render is unchanged (watch-item R5).
--
-- The state index is widened to cover the ordered scan: the old
-- `(project_id, state)` is a prefix of the new `(project_id, state, position)`,
-- so the new index serves BOTH the per-state filter and the ordered read.
--
-- Forward-only, additive: one new column (DEFAULT 0, so the ALTER needs no
-- table rewrite of NULLs) plus a deterministic one-shot backfill.

ALTER TABLE issues ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

UPDATE issues i
   SET position = sub.rn
  FROM (
    SELECT id,
           row_number() OVER (PARTITION BY project_id, state ORDER BY number DESC) - 1 AS rn
      FROM issues
  ) sub
 WHERE i.id = sub.id;

DROP INDEX idx_issues_project_state;
CREATE INDEX idx_issues_project_state_position ON issues (project_id, state, position);
