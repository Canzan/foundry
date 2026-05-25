-- crates/foundry-store/migrations/0006_comments_edit_delete.sql
-- Slice 5 (US-10 deferred ACs): comment edit + soft-delete + admin moderation.
--
-- Adds three nullable columns to support:
--   * "edited" indicator           (updated_at IS NOT NULL — ADR-006 / Q4 = A)
--   * soft-delete tombstone        (deleted_at IS NOT NULL — ADR-007)
--   * admin moderation audit trail (deleted_by points to the deleting user)
--
-- All columns are nullable so the migration is non-destructive for
-- existing rows. The 90-day GC task (ADR-007 follow-up alternative C) is a
-- strict superset of this schema; no further migration is needed when v0.2
-- adds it — the GC task just adds a deletion path, not a schema change.
--
-- Forward-only per ADR-003: never edit 0004_comments.sql.

ALTER TABLE comments
    ADD COLUMN updated_at TIMESTAMPTZ NULL,
    ADD COLUMN deleted_at TIMESTAMPTZ NULL,
    ADD COLUMN deleted_by UUID NULL REFERENCES users(id);

-- Partial index to keep the "live comments for issue" hot path narrow.
-- The existing idx_comments_issue_created covers all rows; this partial
-- index skips tombstones, which is the dominant access shape for the
-- issue-page list query (`WHERE deleted_at IS NULL ORDER BY created_at`).
CREATE INDEX idx_comments_issue_live ON comments (issue_id, created_at)
    WHERE deleted_at IS NULL;
