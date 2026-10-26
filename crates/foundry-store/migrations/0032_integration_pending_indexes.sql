-- 0032_integration_pending_indexes.sql — external-integration-plugins
-- (ADR-EIP-002 decision 4, ADR-EIP-007 decision 2): the senders' and the
-- expiry sweep's reads of pending deliveries, without a scan of the history.
--
-- - A member's email claim (`recipient_user_id = $1 AND status = 'pending'
--   ORDER BY id`) and the senders' email candidates (pending, a recipient,
--   grouped by recipient in `min(id)` order). Route deliveries are left out:
--   0030's `(route_id, id)` index serves them.
-- - The expiry sweep (`status = 'pending' AND give_up_at <= now`).
--
-- Plain CREATE INDEX, in a transaction: `integration_deliveries` ships in
-- this release (0030), so no board write waits on a build over old rows.
-- Undo: DROP INDEX IF EXISTS idx_integration_deliveries_recipient_pending,
--     idx_integration_deliveries_give_up_pending;
CREATE INDEX IF NOT EXISTS idx_integration_deliveries_recipient_pending
    ON integration_deliveries (recipient_user_id, id)
    WHERE status = 'pending' AND recipient_user_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_integration_deliveries_give_up_pending
    ON integration_deliveries (give_up_at)
    WHERE status = 'pending';
