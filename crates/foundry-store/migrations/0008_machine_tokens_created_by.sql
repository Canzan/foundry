-- 0008_machine_tokens_created_by.sql
-- US-MT00 (machine-token admin UX): re-introduce the audit column Feature A
-- deferred. This feature is the issuer call-site, so the registry can finally
-- record WHO minted each token (DD6 / ADR-MT04 / OD2-RATIFIED).
--
-- Nullable: there are 0 existing rows today, and any pre-feature row back-fills
-- NULL and surfaces as "minted by —" in the list (US-MT06 edge path). New mints
-- always record the acting admin (NFR-MT-SEC-06).
--
-- ON DELETE SET NULL (NOT CASCADE): deleting an admin user must NOT vaporize the
-- token registry rows — audit history survives, degrading to "minted by —".
-- (CASCADE would destroy the record of who issued still-live credentials.)
--
-- NFR-MT-DATA-02: this migration adds NO token/secret/hash column. The JWT is
-- the secret; the table stays a registry of metadata + the revocation flag.
--
-- Forward-only per ADR-003: never edit 0007_machine_tokens.sql. Applied under
-- the migration runner's MIGRATION_LOCK_ID advisory lock (foundry-store::migrate).

ALTER TABLE machine_tokens
    ADD COLUMN created_by UUID NULL REFERENCES users(id) ON DELETE SET NULL;
