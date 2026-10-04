-- 0017_users_provisioned_at.sql — keycloak-sso D3b / DDD-23, DDD-24 (OD-14).
--
-- Provenance: when role provisioning created an account. Written only by the
-- provisioning INSERT (DDD-25, from the caller's clock) and by the backfill below;
-- nothing ever clears it (DDD-27). No default, no index.
--
-- Backfill (OD-14): before this version every non-provisioning users INSERT bound
-- a password hash and no UPDATE sets one NULL, so a password-less account was
-- created by provisioning — mark it at the moment it was created (created_at,
-- never now()). An account with a password stays unmarked, including a
-- provisioned account reset before the upgrade (OD-14's accepted residue). The
-- `provisioned_at IS NULL` guard keeps the statement idempotent.
--
-- Additive and rolling-deploy safe against v0.6.2 (it names columns explicitly
-- and selects no `*`); the read model's `OR password_hash IS NULL` arm (DDD-26)
-- covers an account an old replica provisions during the window. Forward-only: a
-- revert is a forward migration.

ALTER TABLE users ADD COLUMN provisioned_at TIMESTAMPTZ NULL;

UPDATE users
   SET provisioned_at = created_at
 WHERE password_hash IS NULL
   AND provisioned_at IS NULL;
