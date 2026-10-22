-- 0028_outbox_dispatch.sql — external-integration-plugins slice 01 (DESIGN
-- DDR-3, DDR-17, [REF] Persistence Model; ADR-EIP-002 decision 1; DEVOPS
-- "Migrations 0028-0031 under rolling upgrade", U-D12).
--
-- The shipped outbox becomes the delivery log. Three NULLABLE columns, added
-- without defaults (no rewrite), then defaults that apply to new rows only
-- (metadata only): every row written after this migration, by this release or
-- by an older replica still serving, gets an event id and the id of the
-- transaction that wrote it (one board action, DDR-8). Nothing is backfilled:
-- rows at or below `integration_dispatch_start` (all with a NULL event_id) are
-- never dispatched. `notified_at` and `idx_outbox_pending` are untouched.
--
-- Rolling upgrade: the ALTERs need ACCESS EXCLUSIVE on `outbox`, and while the
-- request waits every board write queues behind it. `lock_timeout` bounds that
-- wait at 2 s (U-D12): on timeout the whole migration rolls back, the pod
-- restarts with back-off and retries, and the old pod keeps serving.
--
-- Undo (only before any delivery exists; not needed for a rollback, which the
-- previous release survives, C-9):
--   DROP TABLE IF EXISTS integration_dispatch_start;
--   ALTER TABLE outbox DROP COLUMN IF EXISTS dispatched_at,
--       DROP COLUMN IF EXISTS source_txid, DROP COLUMN IF EXISTS event_id;

SET LOCAL lock_timeout = '2s';

ALTER TABLE outbox
    ADD COLUMN IF NOT EXISTS event_id UUID NULL,
    ADD COLUMN IF NOT EXISTS source_txid XID8 NULL,
    ADD COLUMN IF NOT EXISTS dispatched_at TIMESTAMPTZ NULL;

ALTER TABLE outbox
    ALTER COLUMN event_id SET DEFAULT gen_random_uuid(),
    ALTER COLUMN source_txid SET DEFAULT pg_current_xact_id();

-- The highest outbox id when this migration ran. The ACCESS EXCLUSIVE lock
-- above is held, so no writer is in flight: every row above it is a new event.
CREATE TABLE IF NOT EXISTS integration_dispatch_start (
    only_row   BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (only_row),
    outbox_id  BIGINT NOT NULL
);

INSERT INTO integration_dispatch_start (outbox_id)
SELECT COALESCE(max(id), 0) FROM outbox
ON CONFLICT (only_row) DO NOTHING;
