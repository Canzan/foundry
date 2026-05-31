-- crates/foundry-store/migrations/0007_machine_tokens.sql
-- US-W05b (Feature A, slice 2): machine-token registry + revocation denylist.
--
-- Machine tokens are JWTs (Ed25519, alg pinned to EdDSA — see design/auth.md).
-- The JWT itself IS the secret, presented as a bearer credential; the server
-- verifies the signature offline. This table is therefore a REGISTRY of
-- issuance metadata plus a revocation flag — there is DELIBERATELY NO
-- token/hash/secret column. Persisting the token would defeat the point of a
-- self-contained signed credential and create an exfiltration target.
--
-- Revocation works via a `jti` (JWT ID) denylist checked on every request:
--   find_by_jti(jti) -> revoked_at IS NULL  means the credential is active.
-- Revocation is a FLAG, not a delete: the row stays so the per-request check
-- can keep refusing a revoked credential until it expires and is GC'd. (The
-- US-W05b "A revoked credential is refused on its next use" scenario relies on
-- the row surviving revocation.)
--
-- `scope_team_id` binds the credential to a team scope; NULL means
-- workspace-wide. `user_id` is the principal the credential acts as — the
-- machine "is" that user for authorisation. `expires_at` mirrors the JWT's
-- exp claim so an expiry sweep can prune dead rows without decoding tokens.
-- `last_used_at` is a touch timestamp updated on use (operational visibility).
--
-- Forward-only per ADR-003: never edit a prior migration. Applied under the
-- migration runner's MIGRATION_LOCK_ID advisory lock (see foundry-store::migrate).

CREATE TABLE machine_tokens (
    jti            UUID PRIMARY KEY,
    user_id        UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id   UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    scope_team_id  UUID NULL REFERENCES teams(id) ON DELETE CASCADE,
    expires_at     TIMESTAMPTZ NOT NULL,
    revoked_at     TIMESTAMPTZ NULL,
    last_used_at   TIMESTAMPTZ NULL,
    label          TEXT NOT NULL CHECK (length(label) BETWEEN 1 AND 128),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- List view: a workspace admin lists the credentials they have issued,
-- newest first.
CREATE INDEX idx_machine_tokens_workspace
    ON machine_tokens (workspace_id, created_at DESC);
