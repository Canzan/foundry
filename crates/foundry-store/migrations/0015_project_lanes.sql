-- 0015_project_lanes.sql — board-lane-management slice 01 (D5, ADR-BOARD-LANE-001).
--
-- Forward-only, additive, one-shot; runs on live homelab data. Zero issue-row
-- updates (the 0012 zero-shuffle discipline): positions, states, numbers all
-- untouched. The FK ADD in step 5 is the migration's built-in verification —
-- Postgres validates every existing row, so a project holding an issue in a
-- state the seed failed to cover aborts the whole migration atomically.
-- Full analysis: docs/feature/board-lane-management/design/data-models.md §1-2,
-- architecture-design.md §4.

-- 1. The lanes table (data-models.md §1).
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

-- 2. Grandfather seed (D5) — idempotent by construction (ON CONFLICT DO
--    NOTHING on the (project_id, slug) idempotency key). Every existing
--    project gets its four rendered lanes, labels byte-equal to today's
--    headers; Cancelled ONLY where a cancelled issue exists (the one
--    deliberate visible outcome, D11). Ids via gen_random_uuid() (PG13+
--    built-in; app-side inserts keep the house UUIDv7 idiom — inert mix,
--    id is never ordered by).
INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
SELECT gen_random_uuid(), p.id, p.workspace_id, seed.slug, seed.label, seed.position
  FROM projects p
 CROSS JOIN (VALUES
        ('backlog',     'Backlog',     0),
        ('todo',        'Todo',        1),
        ('in_progress', 'In-Progress', 2),
        ('done',        'Done',        3)
      ) AS seed (slug, label, position)
    ON CONFLICT (project_id, slug) DO NOTHING;

INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
SELECT gen_random_uuid(), p.id, p.workspace_id, 'cancelled', 'Cancelled', 4
  FROM projects p
 WHERE EXISTS (SELECT 1 FROM issues i
                WHERE i.project_id = p.id AND i.state = 'cancelled')
    ON CONFLICT (project_id, slug) DO NOTHING;

-- 3. Drop the static CHECK. Constraint name verified against pg_constraint on
--    a live-data copy at DELIVER (Earned Trust, architecture-design.md §8):
--    the inline column CHECK in 0001 auto-names as issues_state_check. No
--    IF EXISTS: a differently-named CHECK must abort the migration loudly
--    (leaving it behind would silently double-constrain issues.state).
ALTER TABLE issues DROP CONSTRAINT issues_state_check;

-- 4. Drop the DEFAULT — the landing rule moves to code (D6); a state-less
--    INSERT now fails loudly instead of silently minting 'backlog'.
ALTER TABLE issues ALTER COLUMN state DROP DEFAULT;

-- 5. The composite FK — issues.state stays the lane slug; the DB now enforces
--    "zero laneless issues" as a schema fact (KPI 2). Default NO ACTION is
--    exactly right: lane deletion must never cascade into issues implicitly
--    (D7). This ADD validates every live row — the built-in probe.
ALTER TABLE issues ADD CONSTRAINT fk_issues_lane
    FOREIGN KEY (project_id, state) REFERENCES lanes (project_id, slug);
