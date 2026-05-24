-- crates/foundry-store/migrations/0002_sessions_and_reset.sql
-- US-06: session storage + password-reset tokens.
--
-- `session` table layout matches tower-sessions-sqlx-store 0.14 schema
-- (id TEXT primary key, data BYTEA, expiry_date TIMESTAMPTZ). Inlining
-- the migration here (instead of calling PostgresStore::migrate()) keeps
-- all schema changes in one ordered migration set and avoids creating a
-- separate `tower_sessions` Postgres schema — both production and the
-- per-scenario test schema get the table in their normal search_path.

CREATE TABLE session (
    id           TEXT PRIMARY KEY NOT NULL,
    data         BYTEA NOT NULL,
    expiry_date  TIMESTAMPTZ NOT NULL
);
CREATE INDEX idx_session_expiry ON session (expiry_date);

-- Password-reset tokens. The raw token is never persisted; only its
-- SHA-256 hash is stored, mirroring bootstrap_tokens.
CREATE TABLE reset_tokens (
    id          UUID PRIMARY KEY,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  BYTEA NOT NULL UNIQUE,
    expires_at  TIMESTAMPTZ NOT NULL,
    used_at     TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_reset_tokens_user ON reset_tokens (user_id);
