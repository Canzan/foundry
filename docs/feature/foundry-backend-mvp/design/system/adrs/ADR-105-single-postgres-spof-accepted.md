# ADR-105: Single-Postgres SPOF accepted for v0.1

## Status

Accepted (2026-05-23). Explicit acknowledgement, not an oversight.

## Context

The MVP design puts everything in Postgres (ADR-101). The deploy variants (`topology.md`) run a single Postgres instance — there is no replica, no failover, no automatic recovery. If Postgres dies, the entire Foundry instance is down until the operator restores or restarts it.

This is the largest single accepted risk in the v0.1 architecture. This ADR records that the risk is acknowledged, why it's accepted now, and what removes it later.

## Decision

The MVP runs a single Postgres instance. There is no HA Postgres in v0.1. The risk is mitigated by:

1. Documenting it explicitly here, in `topology.md`, and in `failure-modes.md` (Failure 1).
2. Making the backup story (`backup-restore.md`) credible: `pg_dump` + verified restore is the recovery primitive.
3. Designing the app to fail safely when Postgres is unavailable (`/readyz` flips to 503; LB drains; replicas wait for Postgres to return; `realtime.listen` reconnects with backoff).
4. Choosing the substrate (Postgres) such that the v0.4+ HA upgrade is mechanical, not architectural.

## Alternatives considered

### A — HA Postgres in v0.1 via Patroni or pg_auto_failover

- **Pros**: removes the SPOF; matches enterprise expectations.
- **Cons** (decisive):
  - Adds 2 Postgres replicas + a consensus layer (etcd/Consul for Patroni). Container count goes from 2 → 5+ in the baseline compose.
  - Operator must understand replication lag, fencing, split-brain. Violates the "boring monolith" promise.
  - The "under an hour install" budget (US-01) is consumed by HA configuration alone.
  - The MVP target segment (5-50 person teams) does not yet need HA Postgres — a 5-minute Postgres restart per quarter is acceptable. The 200-person ceiling (`scaling.md`) is where it becomes a real ask.

### B — Cloud-managed Postgres (RDS, Cloud SQL, Aiven) only

- **Pros**: HA is the cloud provider's problem.
- **Cons**:
  - Violates the self-host ethos (JTBD outcome #2: data sovereignty).
  - Makes the docker-compose deploy second-class.
  - Lock-in concerns for AGPL-loving operators.

### C — Streaming replication only (no automatic failover)

- **Pros**: warm standby that can be promoted manually; RPO drops to seconds.
- **Cons**:
  - Manual failover means RTO is still operator-driven (15-30 minutes).
  - All the operational complexity of replication without the upside of automatic recovery.
  - Acceptable for operators with mature on-call but doesn't reduce the SPOF in any meaningful sense — and adds a container to the default compose.

### D — Sharded Postgres (e.g., Citus)

- Rejected immediately: massive complexity for a workload that doesn't yet justify it. The 200-person ceiling fits comfortably in a single Postgres.

## Consequences

### Positive

- 2-container compose stays 2-container.
- Operator-facing complexity stays minimal.
- The HA upgrade path is preserved cleanly — same Postgres protocol, same SQL, just more processes.
- The backup story (`backup-restore.md`) is honest about the recovery scenario: this is the RTO/RPO you get with one Postgres, here are the targets, here's the path to tighter targets when you need them.

### Negative (explicit trade-offs, with named impact)

- **Operator-facing**: A single Postgres failure is a full outage. For the 20-person team, this is 5-15 minutes of downtime (process restart) or up to a few hours (restore from backup). For the 200-person team, this is the same downtime but ~200x more user impact.
- **Enterprise procurement**: HA Postgres is often a checkbox requirement. Foundry v0.1 fails that checkbox. The pitch is "v0.4 ships HA; v0.1 is for teams whose business continuity tolerates short outages." Some enterprise sales will be lost to this; we accept it.
- **Backup-as-recovery is slow**: restoring a 250 GB dump is a 1-2-hour operation. The 200-person team running at the design ceiling cannot rely on backup for tight RTO.

### Mitigations baked into the MVP

- `/readyz` flips to 503 within 10 s of Postgres unreachability (NFR-OBS-02), so the LB drains traffic and the user sees a clean maintenance page instead of broken pages.
- Replicas auto-recover when Postgres returns; no manual restart of the app needed.
- LISTEN connection reconnects with backoff; realtime fan-out resumes without operator intervention (`realtime-infrastructure.md`).
- Bytea attachment sha256 verification (`backup-restore.md`) makes restored data trustworthy.

### Path to remove this SPOF (post-MVP)

Three options, in increasing complexity:

1. **Streaming replication + manual failover (v0.3)**: add a `foundry-db-replica` to the compose; document a `foundry doctor promote-replica` procedure. Reduces RPO to seconds; RTO still operator-driven.
2. **Patroni or pg_auto_failover (v0.4)**: full HA with automatic leader election. Adds operator complexity but removes the SPOF. Documented as a separate compose variant.
3. **Cloud-managed Postgres (always available)**: Foundry doesn't care; the operator sets `DATABASE_URL` to RDS / Cloud SQL / Aiven and gets HA for free (modulo cost and lock-in).

Whichever the operator picks, no Foundry code change is required — Postgres is Postgres.

## Probe contract (Principle 9)

The MVP cannot fully verify "is Postgres actually durable?" — that's a multi-day chaos-test, not a startup probe. But it does verify the *substrate honesty* that affects durability:

- `probe.pg.fsync` refuses to start if Postgres reports `fsync=off` (silent data loss on power failure).
- `probe.pg.synchronous_commit` refuses to start if `synchronous_commit=off` (small window of data loss).

These are partial defenses; they catch the egregious misconfiguration but not subtle storage-layer corruption. Operators wanting full assurance should run a separate Postgres testing tool (`pg_amcheck`, `pgbackrest verify`) on a schedule.

## Review trigger

Promote this ADR to "Removed" when *any* of:

1. A target operator is blocked on enterprise procurement because of the SPOF and the deal is large enough to justify HA work.
2. The MVP user base grows past 200 people such that a 1-hour restore is unacceptable downtime.
3. A Postgres outage in the field causes a memorable incident — the cost of HA work becomes obvious in retrospect.

The replacement is not a guess: Patroni is the recommended HA option for v0.4+ based on its operational maturity and the existing Postgres-only stance.
