-- crates/foundry-store/migrations/0005_issue_attachments.sql
-- Slice 3 (US-11): attachments stored inline in Postgres bytea
-- (NFR-DATA-01 — attachments survive a single pg_dump backup).
--
-- A row owns its file content directly via the bytea column. Cascading
-- delete on issue removes its attachments — the US-11 "delete issue
-- removes attachments" scenario relies on this.
--
-- `size_bytes` is denormalised so the issue-detail render can show
-- size labels (e.g. "9 MB") without scanning the bytea. `sha256_hex`
-- is captured at insert time so the US-03 backup-restore round-trip
-- can prove byte-for-byte integrity without re-reading the column.
--
-- Indexed on (issue_id, created_at) so the issue page lists newest-
-- first attachments cheaply.

CREATE TABLE issue_attachments (
    id             UUID PRIMARY KEY,
    issue_id       UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    workspace_id   UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    uploader_id    UUID NOT NULL REFERENCES users(id),
    filename       TEXT NOT NULL CHECK (length(filename) BETWEEN 1 AND 256),
    content_type   TEXT NOT NULL CHECK (length(content_type) BETWEEN 1 AND 128),
    size_bytes     BIGINT NOT NULL CHECK (size_bytes >= 0),
    sha256_hex     TEXT NOT NULL CHECK (length(sha256_hex) = 64),
    content        BYTEA NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_issue_attachments_issue_created
    ON issue_attachments (issue_id, created_at);
