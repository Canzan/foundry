# Data and Migration — the `created_by` audit column + mint persistence

DESIGN (Propose) for the schema delta and how mint persists. The token VALUE is never stored — only
`jti` + metadata. Decisions: ADR-MT04, DD6 (`wave-decisions.md`).

## The schema delta (OD2 — RECOMMENDED: nullable, no backfill, `ON DELETE SET NULL`)

`machine_tokens` today (`0007_machine_tokens.sql`) has: `jti, user_id, workspace_id, scope_team_id,
expires_at, revoked_at, last_used_at, label, created_at`. It has **no `created_by`** (Feature A
deferred it — "no issuer call-site existed"). This feature IS that call-site, so we re-introduce it
with a forward-only migration following the `0006_comments_edit_delete.sql` precedent (three
nullable columns, non-destructive, forward-only per ADR-003):

```sql
-- crates/foundry-store/migrations/0008_machine_tokens_created_by.sql
-- US-MT00 (machine-token admin UX): re-introduce the audit column Feature A
-- deferred. This feature is the issuer call-site, so the registry can finally
-- record WHO minted each token.
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
```

No new index: the list query is already served by `idx_machine_tokens_workspace
(workspace_id, created_at DESC)` (0007); `created_by` is a projected column on that ordered scan,
resolved to a display name with a `LEFT JOIN users` (see below), not a filter.

Rejected alternatives (ADR-MT04): NOT NULL + sentinel backfill (needless for 0 rows; would need a
synthetic `users` row to satisfy the FK); `ON DELETE CASCADE` (destroys audit history);
`ON DELETE RESTRICT` (blocks deleting any admin who minted).

## Repo change (EXTEND `insert_machine_token`)

`insert_machine_token` (`foundry-store/src/lib.rs:1380`) gains a `created_by: uuid::Uuid` parameter,
persisted in the INSERT. This is the ONLY repo signature change; `list/revoke/find/touch` are
unchanged.

```
// foundry-store — insert_machine_token (EXTEND)
pub async fn insert_machine_token(
    &self,
    jti: uuid::Uuid,
    user_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
    scope_team_id: Option<uuid::Uuid>,
    expires_at: time::OffsetDateTime,
    label: &str,
    created_by: uuid::Uuid,          // <-- NEW (the acting admin's user_id)
) -> Result<(), StoreError> {
    // INSERT ... (jti, user_id, workspace_id, scope_team_id, expires_at, label, created_by)
    //   VALUES ($1 ..= $7)
}
```

`created_by` is `NOT NULL` at the call site (every NEW mint records the admin, NFR-MT-SEC-06) even
though the COLUMN is nullable (for the legacy/`ON DELETE SET NULL` cases).

## How mint persists (US-MT01, US-MT04)

The mint use-case (token-admin-services.md) constructs claims, signs, then persists METADATA ONLY —
never the token value:

| Persisted field | Source |
|---|---|
| `jti` | `claims.jti` = `Uuid::now_v7()` (the new token id) |
| `user_id` | `claims.sub` = the acting admin's `user_id` (the principal the credential acts as) |
| `workspace_id` | the acting workspace (from session) |
| `scope_team_id` | `None` (workspace-wide) or the chosen team id (DD9) |
| `expires_at` | `now + ttl` (TTL required, within bounds — DD8) |
| `label` | admin-chosen (CHECK length 1..128, enforced by 0007) |
| `created_by` | the acting admin's `user_id` (NEW, audit) |
| ~~token value~~ | **NEVER persisted** — returned once as `SecretString`, then dropped (DD7) |

`revoked_at` / `last_used_at` start NULL (active, never-used) per the 0007 defaults.

Note `user_id` and `created_by` are the SAME value in v1 (the admin mints a token that acts AS
themselves — the bound principal). They are distinct columns because the model allows a future where
an admin mints a token bound to a service principal (`user_id` = service) while `created_by` records
the admin who issued it. v1 sets both to the acting admin.

## List read (US-MT02, US-MT06)

`list_machine_tokens(workspace_id)` (`foundry-store:1441`) is workspace-scoped and newest-first
already. To surface "minted by {admin}", the list use-case resolves `created_by` → display name.
Two options for the crafter (DESIGN leaves the mechanism, fixes the contract):
- (a) EXTEND the list query with `LEFT JOIN users u ON u.id = m.created_by` and project
  `u.email_display` / `display_name` (so a deleted admin → NULL → "—"); or
- (b) keep `list_machine_tokens` as-is and resolve names in the use-case via
  `find_user_email_by_id` (`foundry-store:1188`) per distinct `created_by`.

Recommend (a) — one round-trip, the `LEFT JOIN` naturally yields "—" for NULL `created_by`
(deleted admin or legacy row), matching `list_comments_for_issue`'s `COALESCE(u.email_display,
'<deleted>')` precedent (`foundry-store:960`). `MachineTokenRow` (`foundry-store:1937`) gains
`created_by: Option<Uuid>` and the resolved name rides in the use-case's view DTO.

`last_used_at` is already on `MachineTokenRow`; the view renders it or "never" (US-MT06).

## Probe / startup substrate check

The boot probe (`Store::probe`, `foundry-store:139`) already asserts the 0007 `machine_tokens`
columns exist in the active schema. The crafter SHOULD extend that column-existence list to include
`created_by` once 0008 ships (the same Earned-Trust substrate-lie guard the 0006/0007 checks use) —
so a binary booting against a pre-0008 database refuses at `/readyz` rather than failing on the
first mint. This is a one-line addition to the existing `mt_cols` check (foundry-store:176-191).
