-- no-transaction
-- 0029_outbox_dispatch_index.sql — external-integration-plugins slice 01
-- (ADR-EIP-002 decision 1; DEVOPS U-D7, runbook R-7).
--
-- The dispatcher's read path, built without blocking board writes: CREATE
-- INDEX CONCURRENTLY, which refuses to run inside a transaction, hence the
-- `-- no-transaction` first line (sqlx 0.8 applies the file outside one) and
-- exactly ONE statement (a multi-statement simple query is an implicit
-- transaction). No IF NOT EXISTS: it would accept an INVALID index left by a
-- failed build and record the migration as applied.
--
-- Observed failure mode (eip_store `invalid_0029_index_is_reported_at_boot`):
-- a build cancelled while it waits leaves the index INVALID and NO row in
-- `_sqlx_migrations` (so never `Dirty`); every later boot refuses with
-- 42P07 "relation already exists" and `Store::migrate` warns
-- `store.index.invalid`. Remedy (R-7): `DROP INDEX CONCURRENTLY
-- idx_outbox_undispatched;` then restart the pod.
CREATE INDEX CONCURRENTLY idx_outbox_undispatched ON outbox (id)
    WHERE dispatched_at IS NULL AND event_id IS NOT NULL;
