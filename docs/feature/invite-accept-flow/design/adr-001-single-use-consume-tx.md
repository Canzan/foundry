# ADR-001 — Single-use consume: no migration, guarded-UPDATE in one TX

## Status
Proposed (DESIGN wave, Propose mode). Resolves OD-1. Needs a trivial user nod (uses shipped schema).

## Context
NFR-2 requires an invite consumable **exactly once**, race-safe, with the password write and the
consume in the **same transaction**. The task brief, `../discuss/requirements.md`, and
`../discuss/shared-artifacts-registry.md` all recorded the `invites` columns as
`id, workspace_id, invitee_email, created_by, expires_at` with **NO** `used_at` — and framed OD-1 as
"add an additive migration for a `used_at`/`consumed_at` column + write a `consume_invite` fn."

**Grounding overturns that premise.** `crates/foundry-store/migrations/0001_init.sql:93-102`:

```sql
CREATE TABLE invites (
    id              UUID PRIMARY KEY,
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    invitee_email   TEXT,
    created_by      UUID REFERENCES users(id),
    expires_at      TIMESTAMPTZ NOT NULL,
    used_at         TIMESTAMPTZ,            -- the single-use marker — ALREADY PRESENT
    used_by         UUID REFERENCES users(id),  -- who consumed it — ALREADY PRESENT
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

The marker shipped in the FIRST migration. `insert_invite` (`store/lib.rs:491`) and the provision tx
(`:1254`) simply do not write `used_at`/`used_by`; they default NULL — exactly the "unconsumed" state
the guard needs. Furthermore, the exact race-safe idiom is ALREADY proven in-tree:
`claim_bootstrap_token` (`store/lib.rs:258-276`):

```sql
UPDATE bootstrap_tokens SET used_at = $2
 WHERE token_hash = $1 AND used_at IS NULL AND expires_at > $2
 RETURNING id
```

The provision tx (`store/lib.rs:1216-1266`) binds the first-admin `admin_user_id` BOTH as the `users`
PK AND as the invite's `created_by`. So `invites.created_by` IS the first-admin user_id — the consume
can name the exact user row to write the password onto, with no extra lookup.

## Options considered
- **(a) NO migration; reuse `used_at`/`used_by`; mirror `claim_bootstrap_token` (RECOMMENDED).** One
  new store fn does the guarded-UPDATE; a wrapping fn runs it + the password write in one TX. Zero
  schema change. Uses an idiom already proven and mutation-relevant in-tree.
- **(b) Additive migration `0012` adding a fresh `consumed_at` column.** Rejected: the column already
  exists under the name `used_at`. Adding a second, differently-named marker would be redundant and
  confusing, and would orphan the shipped `used_by` audit column. (This was the task's assumed plan,
  invalidated by grounding.)
- **(c) A separate `consumed_invites` table (event-style).** Rejected: over-engineered for a boolean-ish
  single-use marker; the column + guarded-UPDATE is the simplest correct mechanism and matches the
  shipped bootstrap-token design.

## Decision
**(a)** — NO migration. Reuse the shipped `invites.used_at` (marker) and `used_by` (audit) columns.

Two new `foundry-store` functions:

```
// the guarded single-use consume — mirrors claim_bootstrap_token
consume_invite(id, used_by, now) -> Option<(workspace_id, created_by)>:
    UPDATE invites SET used_at = $now, used_by = $used_by
     WHERE id = $id AND used_at IS NULL AND expires_at > $now
     RETURNING workspace_id, created_by
    // None (0 rows) => unknown / already-used / expired => caller refuses

// the one-TX wrapper: consume + credential write are atomic (NFR-2, BR-3)
set_first_admin_password_and_consume(id, password_hash, now) -> ConsumeOutcome:
    BEGIN
      row = <guarded-UPDATE above; RETURNING workspace_id, created_by>
      if 0 rows: ROLLBACK; return Refused
      UPDATE users SET password_hash = $password_hash WHERE id = row.created_by
      COMMIT
      return Consumed { workspace_id: row.workspace_id, user_id: row.created_by }
```

`used_by` is set to `created_by` (the first-admin claims their own invite in v1). Exact SQL parameter
binding and the `ConsumeOutcome` type are the software-crafter's to finalize during GREEN.

## Consequences
- **Positive**: ZERO migration; reuses a proven in-tree race-safe idiom; the `RETURNING created_by`
  removes a second query (no extra race surface); `used_by` gives a free audit trail; single-use +
  expiry enforced in ONE statement (TOCTOU-safe, NFR-2/AC-02.6/02.7).
- **Negative**: the consume fn is genuinely new backend code (not a thin adapter) and must be
  concurrency-probed (the @property two-concurrent-POSTs test is the oracle).
- **Security**: the guarded-UPDATE — not a read-then-write — is the single-use guarantee; the GET-side
  liveness read is advisory only (D6).

## Relationship
Mirrors the shipped `claim_bootstrap_token` single-use design. Corrects the grounding miss recorded in
`upstream-changes.md` Finding 1. Realizes the `web-provisioning-flow` ADR-005 deferred accept vertical.
