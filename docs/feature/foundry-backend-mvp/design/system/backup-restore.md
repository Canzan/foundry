# Backup and Restore

## Audience

Operators (Devansh) and platform engineers planning the disaster-recovery posture. Backed by US-03 and NFR-DATA-01/02. The headline promise is **one `pg_dump` = the entire Foundry instance** — including attachments — because all state lives in Postgres (NFR-DATA-01).

## RTO and RPO targets (MVP)

| Target | Value | Why this and not tighter |
|--------|-------|--------------------------|
| **RPO** (max data loss) | 24 hours | Nightly `pg_dump` is the MVP backup primitive. Operators wanting tighter RPO (1-hour) graduate to Postgres WAL archiving (post-MVP). 24h matches the "single dev evaluating Foundry" use case from US-03. |
| **RTO** (max downtime to restore) | 1 hour for a 20-person team, 4 hours for a 200-person team | Dump size dominates: 5 GB restores in ~10 min; 250 GB restores in ~1-2 hours on commodity hardware. |
| **RTO for app recovery only** (DB intact, replicas down) | 5 minutes | Restart the docker-compose stack. |

These are operator-facing service targets, not technical SLAs. Operators wanting business-grade RPO/RTO should plan for the v0.4+ HA story (ADR-105).

## Backup primitives

The MVP supports exactly two backup operations, both leveraging native Postgres tooling. No Foundry-specific binary on the backup path — this keeps the backup story durable across Foundry versions.

### Operation 1 — Nightly logical dump (`pg_dump -Fc`)

```bash
# Run on the host (or in a sidecar with psql client tooling)
docker compose exec -T postgres \
    pg_dump -U foundry -d foundry -Fc \
    > /backups/foundry-$(date +%F).dump

# Or directly via psql client tooling
PGPASSWORD=$FOUNDRY_DB_PASS pg_dump \
    -h localhost -U foundry -d foundry -Fc \
    -f /backups/foundry-$(date +%F).dump
```

- `-Fc` produces a custom-format dump (compressed, parallelizable restore).
- Output is a single file; copy it offsite using whatever the operator already uses (`rsync`, `restic`, S3 `aws s3 cp`, etc.).
- For a 5 GB DB, dump runs in 5-10 minutes; CPU and IO spike on Postgres during the dump.

A cron entry shipped in the docs:

```cron
# /etc/cron.d/foundry-backup
15 3 * * * root /usr/local/bin/foundry-backup.sh
```

with `foundry-backup.sh` doing dump + offsite-copy + rotation (keep last 7 daily + 4 weekly + 12 monthly).

### Operation 2 — Backup verification (`foundry doctor backup-verify`)

A subcommand of the Foundry binary (solution-architect owns the implementation; infra owns the *contract*):

```bash
docker run --rm -v /backups:/backups \
    foundry/foundry:latest doctor backup-verify /backups/foundry-2026-05-22.dump
```

Output (machine-parseable):

```
backup-file: /backups/foundry-2026-05-22.dump
backup-format: pg_dump custom (v16)
backup-size-bytes: 5421366784
schema-version: 0042
row-counts:
  workspaces: 1
  users: 12
  teams: 3
  projects: 8
  issues: 4823
  comments: 19311
  issue_attachments: 1142 (total bytea bytes: 5183094272)
checks:
  pg_restore --list passes: YES
  schema is at latest known migration: YES
  attachment bytea sums consistent with stored sha256: YES
status: OK
exit-code: 0
```

The verify subcommand does NOT restore — it parses the dump and runs structural checks. Operators can wire this into the cron job to fail loudly if a backup is corrupt.

## Restore drill

The restore drill is the test that proves the backup story is real. The doc operators should run within their first week of running Foundry:

```mermaid
sequenceDiagram
    autonumber
    participant Op as Operator
    participant Old as Production Foundry
    participant Backup as /backups
    participant New as Fresh VM

    Op->>Old: pg_dump -Fc > foundry-$(date).dump
    Old-->>Backup: 5 GB dump
    Op->>Backup: foundry doctor backup-verify (check integrity)
    Backup-->>Op: status: OK

    Note over Op,New: Drill begins
    Op->>New: docker compose up -d (postgres + foundry; first start, empty DB)
    Op->>New: docker compose stop foundry (avoid migration race with restore)
    Op->>New: pg_restore -d foundry --clean --if-exists foundry-$(date).dump
    New-->>Op: restore complete in ~10 min
    Op->>New: docker compose start foundry
    Op->>New: sign in as devansh@acme.com (existing credentials)
    New-->>Op: workspace dashboard with all issues and attachments
    Op->>New: download an issue attachment, sha256 it
    Op->>Old: sha256 the same attachment
    Note over Op: sha256 matches -> drill passes
```

The drill should be re-run after every major version upgrade. It's also the primary test for the "Foundry data sovereignty is real" claim from JTBD outcome #2 (NFR-DATA-02 makes it a CI requirement).

## Backup size growth — the bytea trade-off

NFR-DATA-01 puts attachments in `bytea`. This is the right call for the indie/early-stage segment (single-file backup; no S3 dependency; one operator concept instead of two), but it makes the backup size grow linearly with attachment count.

Estimated dump sizes (from `scaling.md`):

| Workload | Attachments after 1 year | 1-year dump size | 5-year dump size |
|----------|-------------------------|------------------|------------------|
| 20-person team | ~10,000 attachments * 500 KB = 5 GB | ~5 GB | ~25 GB |
| 200-person team | ~100,000 attachments * 500 KB = 50 GB | ~50 GB | ~250 GB |

A 250 GB dump is large but not pathological: `pg_dump -Fc` includes built-in compression; the wire size is typically 50-70% of raw bytea size depending on attachment types. Operators with bandwidth-constrained offsite copies should plan accordingly. The runbook recommends:

- **Up to 50 GB**: nightly full dumps are fine.
- **50-200 GB**: nightly full dumps still tractable; consider Postgres WAL archiving for tighter RPO.
- **>200 GB**: migrate attachments to S3 (the post-MVP feature flag); attachments are then backed up via S3 lifecycle/replication, and `pg_dump` becomes small (metadata only).

The migration path from bytea to S3 is documented as a v0.4+ feature flag; the MVP does not implement it but does not preclude it either. The `issue_attachments` table schema is designed to allow a `storage_backend` column to be added without breaking existing rows.

## Failure modes

| Failure | Effect | Mitigation |
|---------|--------|------------|
| `pg_dump` runs during heavy app traffic | Postgres CPU spikes; app latency degrades | Schedule for 03:00 local time; document this in the operator runbook |
| Backup file corrupted on disk before offsite copy | Restore would fail silently | `foundry doctor backup-verify` after each dump |
| Restore on a different Postgres major version (PG16 dump on PG15) | `pg_restore` errors on version-specific syntax | Document: "use the same Postgres major version for restores"; the `foundry-db` container pins a version |
| Restore on a different Foundry version (e.g., dump from v0.2, restore on v0.5) | Schema mismatch | `sqlx migrate run` on startup catches up the schema; only forward migration paths supported |
| Mid-backup write (operator commits a comment during the dump) | Comment may or may not be in the dump (point-in-time semantics) | Documented behavior, not a bug; `pg_dump` is consistent at dump-start time |
| Disk fills mid-dump | Dump file truncated; useless | Cron job checks free space first; `pg_dump` exits non-zero on partial write |
| `bytea` returns inconsistent data on TOAST corruption (very rare) | Restored attachments may differ from originals | `foundry doctor backup-verify` includes a per-attachment sha256 check against a stored sha256 column |

The sha256-per-attachment check is the **Earned Trust** application of Principle 9 to this component: we do not trust that `bytea` round-trips through `pg_dump`/`pg_restore` cleanly *because Postgres said so*; we verify it empirically per restore.

## What the MVP backup story does NOT cover

Explicitly out of scope for the MVP; documented so operators don't assume:

- **Continuous backup / WAL archiving**: requires `archive_mode=on` + WAL shipping (Barman, pgBackRest). Post-MVP.
- **Point-in-time recovery**: requires WAL archiving + base backups. Post-MVP.
- **Cross-region replication**: streaming replication is post-MVP; ADR-105 documents the path.
- **Backup encryption at rest**: operator's responsibility (`gpg` or `restic` or S3 SSE). Foundry's binary does not encrypt the dump.
- **Backup of `SESSION_SECRET` and other env vars**: operators are responsible for backing up their `.env` file separately. Losing `SESSION_SECRET` does not lose any data (sessions just need to be re-issued) but it does invalidate outstanding bootstrap and invite tokens.

## Cross-references

- `topology.md` — where `pg_dump` runs in each deploy variant.
- `scaling.md` — storage growth projections that feed RTO/RPO.
- `failure-modes.md` — "what if Postgres goes away entirely" overlaps with restore decisions.
- ADR-101 (Postgres-for-everything) — the architectural choice that makes one-file backups possible.
- ADR-105 (single-Postgres SPOF) — the path to tighter RPO/RTO via HA Postgres.
