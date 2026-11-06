-- 0033_instance_integrations.sql — instance-integration-setup, Deliverable A
-- (ADR-IIS-001 r2 and r5, ADR-IIS-002 r5, ADR-IIS-003 r1, ADR-IIS-004 r3 and
-- r8, ADR-IIS-005 r1).
--
-- New tables only, no change to any shipped table. Every query on them lives
-- in crates/foundry-store/src/integrations/instance.rs (check-arch
-- integrations-instance-tables). None holds a plaintext: the secrets key is
-- stored wrapped under the instance key, the Slack app's secrets sealed.
-- instance_slack_app_pending is Deliverable B's, in its own migration.
--
-- Undo (forgets the secrets key, so every credential sealed under it):
--   DROP TABLE IF EXISTS instance_integration_audit_events,
--       integration_replica_states, integration_config_version,
--       instance_slack_app, integration_secrets_key;

-- The one current secrets key, wrapped under an instance key (a second
-- current key is non-representable).
CREATE TABLE IF NOT EXISTS integration_secrets_key (
    id                   SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    key_id               TEXT NOT NULL,
    wrapped_key          BYTEA NOT NULL,
    wrapped_by_key_id    TEXT NOT NULL,
    wrapped_by_source    TEXT NOT NULL CHECK (wrapped_by_source IN ('operator', 'file')),
    rewrapped_from_path  TEXT NULL,
    created_by           UUID NULL REFERENCES users(id) ON DELETE SET NULL,
    created_at           TIMESTAMPTZ NOT NULL
);

-- The Foundry Slack app an instance admin saved; both secrets sealed.
CREATE TABLE IF NOT EXISTS instance_slack_app (
    id                     SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    client_id              TEXT NOT NULL,
    sealed_client_secret   BYTEA NOT NULL,
    sealed_signing_secret  BYTEA NOT NULL,
    saved_by               UUID NULL REFERENCES users(id) ON DELETE SET NULL,
    saved_at               TIMESTAMPTZ NOT NULL
);

-- The config version every instance-level change bumps in its own
-- transaction; each replica's reload polls it.
CREATE TABLE IF NOT EXISTS integration_config_version (
    id       SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    version  BIGINT NOT NULL DEFAULT 0
);
INSERT INTO integration_config_version (id, version) VALUES (1, 0)
    ON CONFLICT (id) DO NOTHING;

-- Each replica's applied version and key mode. Never a key or a key id.
CREATE TABLE IF NOT EXISTS integration_replica_states (
    replica            TEXT PRIMARY KEY,
    applied_version    BIGINT NOT NULL,
    key_mode           TEXT NOT NULL,
    opens_secrets_key  BOOLEAN NOT NULL,
    beat_at            TIMESTAMPTZ NOT NULL
);

-- Instance-level integration activity, append-only. A NULL actor is Foundry.
-- Never a key, a secret, a key id or a length.
CREATE TABLE IF NOT EXISTS instance_integration_audit_events (
    id          UUID PRIMARY KEY,
    actor_id    UUID NULL REFERENCES users(id) ON DELETE SET NULL,
    action      TEXT NOT NULL CHECK (action IN (
                    'secrets_key_generated', 'credentials_moved', 'secrets_key_rewrapped',
                    'slack_app_saved', 'secrets_key_replaced', 'started_over',
                    'slack_app_replaced', 'slack_app_removed')),
    client_id   TEXT NULL,
    count       INTEGER NULL CHECK (count >= 0),
    created_at  TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_instance_integration_audit_events_created
    ON instance_integration_audit_events (created_at);
