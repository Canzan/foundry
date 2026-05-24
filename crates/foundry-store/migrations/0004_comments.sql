-- crates/foundry-store/migrations/0004_comments.sql
-- Slice 2 (US-10): comments table.
--
-- A comment is owned by an issue, which lives in a workspace. Both
-- foreign keys cascade on delete (deleting an issue removes its
-- comments; deleting a workspace cascades through issues -> comments).
--
-- We persist BOTH `body_markdown` (the original user input — re-render
-- on demand, e.g. for editing or for migrating renderer versions) and
-- `body_html` (the sanitized output rendered at insert time). Storing
-- the HTML inline avoids re-rendering on every page load; storing the
-- markdown alongside makes it possible to re-render the whole table if
-- the renderer ever changes its allowlist.
--
-- Edit/delete columns are deferred per wave-decisions.md; the table
-- intentionally has no `updated_at` or `deleted_at` yet. Slice-3 may
-- add them with a follow-up migration.

CREATE TABLE comments (
    id             UUID PRIMARY KEY,
    workspace_id   UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    issue_id       UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    author_id      UUID NOT NULL REFERENCES users(id),
    body_markdown  TEXT NOT NULL CHECK (length(body_markdown) BETWEEN 1 AND 65536),
    body_html      TEXT NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Comments are loaded as "the comment thread for issue X, in order".
-- Index on (issue_id, created_at) supports that query without a scan.
CREATE INDEX idx_comments_issue_created ON comments (issue_id, created_at);
