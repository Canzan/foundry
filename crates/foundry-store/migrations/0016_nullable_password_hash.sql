-- 0016_nullable_password_hash.sql — keycloak-sso D3a / DDD-13.
--
-- An account may exist without a password: a federated first sign-in provisions
-- a user whose only door is the identity provider. Additive: no backfill, no
-- default, every existing row keeps its hash; users.email_lower UNIQUE is
-- untouched and remains the one-account-per-address guarantee.
--
-- ONE-WAY once any NULL-hash row exists (DDD-13 addendum): restoring NOT NULL
-- fails while a password-less account remains. A revert is a FORWARD migration
-- (delete those accounts or assign them hashes first), never a down migration.

ALTER TABLE users ALTER COLUMN password_hash DROP NOT NULL;
