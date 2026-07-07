-- crates/foundry-store/migrations/0013_issue_change_events.sql
-- issue-change-history slice 01 (ADR-001): a dedicated, append-only record of
-- every tracked-field change to an issue — `actor · field · old → new · when`.
--
-- Mirrors the `comments` precedent (0004): per-issue sub-record owned by a
-- workspace, with cascade deletes so history vanishes only when its issue /
-- project / workspace is itself removed. `project_id` is denormalized (it is
-- reachable via `issue_id → project_id`) so the project change-report (US-04)
-- can read the `(project_id, created_at)` index without a JOIN.
--
-- Invariants: APPEND-ONLY — no code path UPDATEs or DELETEs a row (audit
-- integrity); written in the SAME transaction as the mutation (no phantom / no
-- drop); one row per CHANGED field (a no-op save records nothing). `old_value`
-- is nullable (reserved for a future creation-event kind); v1 field-change rows
-- carry both old + new. The `field` CHECK grows as new editable fields land.

CREATE TABLE issue_change_events (
    id            UUID PRIMARY KEY,
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id    UUID NOT NULL REFERENCES projects(id)   ON DELETE CASCADE,
    issue_id      UUID NOT NULL REFERENCES issues(id)     ON DELETE CASCADE,
    actor_id      UUID NOT NULL REFERENCES users(id),
    field         TEXT NOT NULL CHECK (field IN ('status', 'title', 'description', 'rank')),
    old_value     TEXT,
    new_value     TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Timeline (US-01/02) + program feed (US-03): "the changes for issue X, in order".
CREATE INDEX idx_issue_change_events_issue_created
    ON issue_change_events (issue_id, created_at);

-- Project change report (US-04): "the changes across project P, in order".
CREATE INDEX idx_issue_change_events_project_created
    ON issue_change_events (project_id, created_at);
