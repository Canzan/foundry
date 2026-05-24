# Database Migrations Under Rolling Deploy

## Audience

Operators rolling out a new Foundry version (US-04). Companion `solution-architect` owns the migration *file format* and sqlx integration; this document owns the *rollout sequence*, the advisory-lock contract, and what happens when migrations fail mid-rolling-deploy.

## Recommendation — application-startup migration with advisory lock

The MVP runs migrations at app startup, gated by `pg_advisory_lock(MIGRATION_LOCK_ID)`. The first replica to start during a deploy acquires the lock, applies new migrations, and releases. Other replicas block on the lock, then observe the schema is current and skip. This is NFR-MIG-01 and US-04 acceptance criteria.

**Why startup-time over pre-deploy-step:**

| Dimension | App-startup with advisory lock (chosen) | Separate pre-deploy migration job |
|-----------|----------------------------------------|------------------------------------|
| Operator complexity | Single `docker compose up -d` does everything | Operator must run `foundry migrate` then deploy |
| Failure isolation | Failed migration -> replica exits non-zero; LB drains it; old replicas keep serving | Failed migration job blocks deploy entirely (better isolation) |
| K8s portability | Same K8s; uses `livenessProbe` + replica restart | K8s `Job` resource (more ceremony) |
| Behavior under simultaneous replica startup | Advisory lock serializes; well-defined | N/A (job runs once) |
| Bad-migration blast radius | First replica exits; if all replicas fail, full outage | One job fails; cluster stays on old version |
| "Under-an-hour install" promise (US-01) | Preserved | Adds a step to the README |

The MVP picks startup-with-advisory-lock because (1) preserving the one-step install dominates, (2) the K8s `Job` form is the post-MVP refinement, and (3) the advisory lock makes the race well-defined.

A future v0.4 deploy variant can add an optional `foundry migrate` CLI subcommand that operators with stricter change-management can call in a pre-deploy step. Same binary, different entry point — no infrastructure change required.

## Rollout sequence (multi-replica rolling deploy)

```mermaid
sequenceDiagram
    autonumber
    participant Op as Operator
    participant LB as Caddy
    participant R1 as Replica 1 (old)
    participant R2 as Replica 2 (old)
    participant R3 as Replica 3 (old)
    participant PG as Postgres

    Note over R1,R3: v0.2.1 running; LB round-robins to all three
    Op->>R1: docker compose pull && docker compose up -d (rolling)

    Note over R1: Docker recreates R1 with v0.3.0 image
    R1->>PG: pg_advisory_lock(MIGRATION_LOCK_ID)
    PG-->>R1: lock acquired
    R1->>PG: sqlx migrate run (4 new migrations, ~3s)
    PG-->>R1: ok
    R1->>PG: pg_advisory_unlock
    R1->>LB: /readyz returns 200
    LB-->>R1: starts routing requests

    Note over R2: Docker recreates R2 with v0.3.0 image
    R2->>PG: pg_advisory_lock(MIGRATION_LOCK_ID)
    PG-->>R2: lock acquired (instant; nothing to migrate)
    R2->>PG: sqlx migrate run (0 new migrations)
    R2->>PG: pg_advisory_unlock
    R2->>LB: /readyz returns 200

    Note over R3: same as R2

    Note over R1,R3: All replicas on v0.3.0; rolling deploy complete
```

The interleaving constraint: while R1 is running migrations, R2 and R3 are still serving old-version traffic against the in-progress new schema. **This requires every migration to be forward-compatible with the previous app version for the duration of the rolling deploy.** The rule of thumb is the well-known "expand-contract" pattern:

| Migration type | Safe in rolling deploy? | Pattern |
|----------------|------------------------|---------|
| Add nullable column | Yes (old app ignores it) | Single migration |
| Add column with default | Yes if default is constant and add+backfill is fast | Single migration |
| Drop column | No — must be 2 releases: release N removes app references, release N+1 drops the column | Expand-contract |
| Rename column | No — same as drop: 2 releases minimum, with a temporary duplicate | Expand-contract |
| Add NOT NULL constraint to existing column | No — backfill first as a separate release | Expand-contract |
| Add index | Yes (use `CREATE INDEX CONCURRENTLY` outside a transaction; see NFR-MIG-02 exception) | Single migration with header comment |
| Drop index | Yes (no app dependency) | Single migration |
| Change column type | Usually no; same pattern as rename | Expand-contract |

The release-notes requirement (NFR-MIG-03) captures the operator-visible side: every release that includes a non-trivially-safe migration must call it out.

## Advisory lock semantics

The lock ID is a single fixed integer constant in the codebase, e.g., `MIGRATION_LOCK_ID = 0x_F0_5D_71_4D` (chosen to be memorable and unlikely to clash with other tooling). `pg_advisory_lock` is per-session; sqlx's migration runner acquires it inside a session, runs migrations, then releases. Properties we rely on:

- **Serialization**: only one session holds the lock at a time; others block.
- **Auto-release on session end**: if the replica crashes mid-migration, the Postgres backend closing releases the lock automatically. The next replica picks up.
- **Cooperative — not enforced on writes**: the advisory lock does *not* prevent other replicas from running queries; it only serializes migration runs. This is the desired behavior — non-migrating replicas keep serving requests during a migration.

We do NOT use `pg_try_advisory_lock` and skip on contention. Skipping is wrong: the replica needs to wait until the migration is done before it can safely accept traffic on the new schema.

## Failure modes

### Migration fails mid-rollout, only some replicas upgraded

R1 acquires the lock, the migration errors, R1 exits non-zero. R2 and R3 are still on the old version, serving requests. The new schema state is unchanged (transaction rolled back per NFR-MIG-02). Operator sees `R1` in a restart loop with the migration error in logs.

Recovery procedure:

1. Operator diagnoses the failed migration (looks at R1's logs).
2. Two paths:
   - Path A (preferred): pin the image tag back to the old version; R1 restarts cleanly on old schema; old version continues running. Then fix the migration in a hotfix release and re-deploy.
   - Path B: hand-fix the database (e.g., backfill the column the migration was failing on), then re-deploy the new version.
3. In both cases, R2 and R3 never went down; user-visible impact is "1/3 of the cluster temporarily offline" — well within the rolling-deploy contract.

The advisory lock guarantees no *partial* migration state: either the migration committed entirely or it rolled back entirely (with the documented `CREATE INDEX CONCURRENTLY` exception, which must be flagged in the migration file header — NFR-MIG-02).

### Migration that cannot be transactional (e.g., `CREATE INDEX CONCURRENTLY`)

These migrations run *outside* a transaction. If they fail partway, Postgres may be left with an `INVALID` index. The migration file header must:

```sql
-- migration: 0042_add_issues_state_index.sql
-- non-transactional: uses CREATE INDEX CONCURRENTLY
-- recovery: if this migration fails, the operator must run
--   DROP INDEX IF EXISTS idx_issues_state_invalid;
-- and then re-deploy.
```

The header comment is part of NFR-MIG-02. Failed non-transactional migrations leave the schema in a defined-but-invalid state; the recovery instruction is part of the migration file itself, not a separate runbook.

### All replicas die during deploy (catastrophic)

If every replica fails to start due to the new migration, the cluster is fully down. Mitigation: operators are documented to deploy one replica at a time (Docker's default for `compose up -d` with multiple replicas honors this if `update_config.parallelism: 1` is set in the compose file; the MVP compose file ships with this default). Even with all-at-once deploys, the advisory lock at least guarantees only one replica is running the migration at a time, so the failure is reproducible (replica 1 fails, exits, restart back-off, replica 2 picks up the lock and fails identically, etc.) — not a corrupted partial state.

### Concurrent deployment of two different Foundry versions

The advisory lock prevents two migrations from running simultaneously, but it does NOT prevent two app versions from coexisting against the same Postgres. This is normal during the rolling-deploy window (mid-deploy, half the replicas are old). It becomes a problem if the operator runs `docker compose up` with two *different* image tags simultaneously (e.g., manual experimentation). The MVP design accepts this hazard — the operator who does this is outside the contract — but the release notes (NFR-MIG-03) flag any schema change that would be unsafe under prolonged mixed-version operation.

### Migration takes >LB-health-check-grace seconds

A long-running migration delays the new replica becoming ready. Caddy's default health check polls `/readyz` every 30 s and waits for the replica to be reachable; if migration takes 2 minutes, the new replica is unhealthy for 2 minutes but the old replicas keep serving. No outage; only the rolling deploy slows down.

NFR-MIG-03 requires release notes to document expected migration runtime against a 100k-issue / 10k-user baseline. Operators with much larger databases should plan accordingly; migrations expected to take >5 minutes are flagged in the release notes as "maintenance-window candidates."

## Probe contract (Principle 9)

Migrations rely on substrate properties that can lie. Probes verified at startup (run before `sqlx migrate run`):

1. **`probe.pg.advisory_lock`** — acquire `pg_advisory_lock(PROBE_LOCK_ID)` with a 2-second timeout, then release. Refuses to start if it times out (indicates another replica is stuck holding the migration lock, or pgbouncer is dropping advisory-lock semantics).

2. **`probe.pg.transactional_ddl`** — open a transaction, `CREATE TABLE _foundry_probe_ddl(x int); ROLLBACK;`, then verify `_foundry_probe_ddl` does not exist. This catches the "Postgres compiled without transactional DDL" lie (vanishingly rare on Postgres, common on MySQL — important for operators who try to point Foundry at an unsupported substrate).

3. **`probe.pg.migrations_table_writable`** — verify the `_sqlx_migrations` table can be written and queried. Catches read-only-Postgres misconfiguration.

These probes are part of the broader `probe()` contract (see `topology.md`). They run *before* `sqlx migrate run` so a failing probe gives a clearer diagnostic than a cryptic migration error.

## Cross-references

- Deploy sequence (where this fits): `topology.md` Variant 2.
- ADR-102 (docker-compose primacy) — why we don't use a Helm pre-install hook.
- `failure-modes.md` for the broader "what if Postgres goes away" picture.
- solution-architect's `migrations.md` for migration file format conventions.
