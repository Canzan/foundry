-- 0031_integration_email_choices.sql — external-integration-plugins slice 05
-- (DESIGN [REF] Persistence Model, DDR-15, DDR-17; ADR-EIP-007 decision 1).
--
-- A member's project email choices: one row per (member, project, event),
-- with the issues it covers (`all`, or `mine`: issues the member created or
-- commented on). No default rows (OD-10). New table only: no `projects`
-- column (so `live_projects` is not recreated) and no change to any shipped
-- table, so the previous release boots on this schema (C-9). Every Rust
-- reader selects projects through `live_projects`; writers open with
-- lock_live_project. Not added to TENANT_TABLES in v1 (DD-5).
--
-- Undo (forgets every member's project email choices):
--   DROP TABLE IF EXISTS integration_email_choices;

CREATE TABLE IF NOT EXISTS integration_email_choices (
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    project_id  UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    event       TEXT NOT NULL CHECK (event IN (
                    'issue.created', 'issue.moved', 'issue.deleted', 'comment.added')),
    scope       TEXT NOT NULL CHECK (scope IN ('all', 'mine')),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, project_id, event)
);
CREATE INDEX IF NOT EXISTS idx_integration_email_choices_project
    ON integration_email_choices (project_id);
