-- 0034_instance_slack_app_pending.sql — instance-integration-setup,
-- Deliverable B (ADR-IIS-003 r4, ADR-IIS-004 r2).
--
-- The Slack app an instance admin submitted while workspaces are still
-- connected through the app in effect: held sealed until she confirms it
-- (I-14), so the confirm page carries no secret. One new table, no change to
-- any shipped table. Every query on it lives in
-- crates/foundry-store/src/integrations/instance.rs (check-arch
-- integrations-instance-tables). It holds no plaintext: both Slack secrets
-- are sealed, as instance_slack_app's are.
--
-- The hold lasts ten minutes: `expires_at` is written by the caller's clock,
-- never now(), and the confirm and the retention sweep judge it against that
-- same clock. A second pending app is non-representable (CHECK (id = 1)); a
-- new submission overwrites the row.
--
-- Undo (forgets a submission nobody confirmed yet):
--   DROP TABLE IF EXISTS instance_slack_app_pending;

CREATE TABLE IF NOT EXISTS instance_slack_app_pending (
    id                     SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    client_id              TEXT NOT NULL,
    sealed_client_secret   BYTEA NOT NULL,
    sealed_signing_secret  BYTEA NOT NULL,
    requested_by           UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at             TIMESTAMPTZ NOT NULL
);
