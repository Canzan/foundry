# Realtime Infrastructure — pg_notify + SSE Fan-out

## Audience

Designers and operators of the multi-replica deployment. This is the single hardest infrastructure problem in the MVP: keeping `LISTEN/NOTIFY` honest across replicas without painting ourselves into a corner.

Companion `solution-architect` owns the application-layer SSE handler trait, the htmx wiring, and the event payload schema. This document owns the *infrastructure mechanics*: connection-pool layout, Postgres LISTEN semantics, fan-out topology, reconnect behavior, and what happens when the substrate lies.

## Problem statement

US-09 demands: an issue mutation on replica A becomes visible to a client connected to replica B within 1 second (median, NFR-PERF-03). The MVP refuses to add Redis (recommendation.md, ADR-101), so the cross-replica wakeup must travel through Postgres.

The shape is **fan-out on read with a pub/sub wake-up signal**: each replica maintains its own in-process map of `project_id -> [local SSE subscribers]`; cross-replica replication is just a wake-up notification on a shared Postgres channel; the actual event payload is small and ships with the NOTIFY.

## Fan-out topology

```mermaid
sequenceDiagram
    autonumber
    participant U1 as Hiroshi (browser)
    participant U2 as Mei (browser)
    participant LB as Caddy LB
    participant R1 as Replica 1 (writer)
    participant R2 as Replica 2 (Mei's home)
    participant PG as Postgres

    Note over R1,R2: Both replicas hold one dedicated LISTEN connection on channel "issue_events"
    Note over R2: Mei's SSE stream is on Replica 2 (sticky to the connection, not to sessions)

    U1->>LB: POST /issues/AUTH-3 state=in-progress
    LB->>R1: forward
    R1->>PG: BEGIN; UPDATE issues SET state=...; SELECT pg_notify('issue_events', json); COMMIT
    PG-->>R1: ok
    R1-->>U1: 200 OK (htmx swap)

    par PG fans out NOTIFY to every LISTENer
        PG-->>R1: NOTIFY issue_events {project_id, issue_id, ...}
        PG-->>R2: NOTIFY issue_events {project_id, issue_id, ...}
    end

    Note over R1: Local SSE subscribers for that project_id: 0 -> drop
    R2->>R2: Look up local subs for project_id -> [Mei]
    R2-->>U2: SSE event: issue_updated AUTH-3
```

Key properties of this topology:

- **Single Postgres channel**, not channel-per-project. Per-project channels would explode (one project = one LISTEN registration; 50 projects = 50 LISTENs per replica), and Postgres caps channel name length at NAMEDATALEN-1 (63 bytes). Single channel + in-process filter is the right granularity.
- **Per-replica in-process fan-out** via `tokio::sync::broadcast` (or an axum `Sse` stream backed by a watch). Each request handler that accepts an SSE connection subscribes to the local broadcast; the dedicated LISTEN task is the sole publisher.
- **NOTIFY payload carries the event**, not just a "go look at the database" wake-up. This avoids a follow-up SELECT per event. Postgres caps NOTIFY payload at 8000 bytes; our payload is `{event_type, project_id, issue_id, actor_id, timestamp, version}` — well under cap. The full issue/comment HTML re-render happens lazily on the client (htmx `hx-trigger` triggers a `hx-get` for the specific card). This keeps the NOTIFY small and avoids stuffing rendered HTML through Postgres.

## Connection-pool layout (the critical detail)

Each replica holds **two distinct Postgres connection groups**. Confusing them is the dominant failure mode in LISTEN/NOTIFY-based systems.

| Pool | Size | Lifecycle | Purpose |
|------|------|-----------|---------|
| Request pool | `DATABASE_MAX_CONNECTIONS` (default 10) | sqlx `PgPool`; connections borrowed and returned per request | All SELECT/INSERT/UPDATE/DELETE in request handlers |
| LISTEN connection | 1, dedicated | Single `PgConnection`, owned by a background task for the lifetime of the replica, never returned to the pool | `LISTEN issue_events`, drives the in-process broadcast |

**Why dedicated**: a LISTEN registration is per-connection. If the connection is returned to the pool, the next borrower inherits the LISTEN (sometimes silently). Worse: pgbouncer in transaction-pooling mode breaks LISTEN entirely (the LISTEN gets dropped when the transaction releases the backend). The probe `probe.pg.listen_notify` (see `topology.md`) detects this lie at startup by self-NOTIFYing on a probe channel and verifying receipt within 1 second.

**Reconnect behavior**: the LISTEN task watches its own connection. On connection loss:

1. Log `realtime.listen.disconnected` with the underlying error.
2. Exponential backoff: 100 ms, 200 ms, 400 ms, ..., capped at 5 seconds.
3. On reconnect: re-issue `LISTEN issue_events`.
4. Emit `realtime.listen.reconnected` with the disconnect duration.
5. **Do not** attempt to replay events for the gap. The MVP design accepts a small SSE gap (US-09 acceptance criterion explicitly: "No event replay on reconnect in MVP"). Clients receive a "Reconnected — refresh for latest" toast (US-09 scenario "Stale-state warning on reconnect").

Event replay during gaps is deferred to v0.4 (`event_log` table with `Last-Event-Id` resume). The MVP design doesn't pre-build it but doesn't preclude it: the existing schema already has a monotonic version field on issues, so v0.4 can replay by `WHERE updated_at > $last_seen`.

## Per-replica connection budget

For an N-replica deployment against one Postgres:

```
Total Postgres connections used = N * (DATABASE_MAX_CONNECTIONS + 1)
                                = N * 11   (default)

3 replicas:  33 connections
5 replicas:  55 connections
10 replicas: 110 connections (above Postgres default 100; raise max_connections)
```

NFR-PERF-04 sizes this. The +1 per replica for the LISTEN is the easy-to-miss number; documenting it here prevents the "I bumped to 5 replicas and Postgres started rejecting connections" surprise.

## Failure modes specific to LISTEN/NOTIFY

| Failure | Effect on system | Detection | Mitigation |
|---------|------------------|-----------|------------|
| Postgres restarted, LISTEN connection drops | All replicas lose realtime; SSE clients keep connection but receive no events | LISTEN task's reconnect loop logs disconnect; `/readyz` flips to 503 on Postgres unreachability | Backoff + reconnect (see above); document acceptable 1-5s gap |
| pgbouncer in transaction-pooling mode in front of Postgres | NOTIFYs work, LISTENs drop silently after first transaction | `probe.pg.listen_notify` at startup; replica refuses to start | Operator either configures pgbouncer for session pooling or removes it |
| Single replica has its LISTEN drop but the rest are fine | Subset of users see no realtime; others see normal | Per-replica `realtime.listen.disconnected` metric + Prometheus alert at >5 sec disconnect | Auto-recovers via reconnect loop; replica drops out of `/readyz` if disconnect >30 s |
| NOTIFY payload >8000 bytes | Postgres truncates / errors; event lost | The serializer enforces the cap; payloads beyond cap are reduced to "go re-fetch" and the client falls back to fetching the affected card. Logged as `realtime.payload.too_large` | Schema enforces fixed-size payload; full rendering is client-driven |
| Two replicas write near-simultaneously with conflicting state | Last-writer-wins at Postgres; both NOTIFYs fan out; clients see final state | `version` field on issues monotonically increases; clients ignore out-of-order events with lower version | Standard optimistic-concurrency pattern (owned by solution-architect's issue model) |
| Listen connection appears healthy but Postgres silently dropped NOTIFYs (rare) | Realtime appears to work but events go missing | Periodic self-NOTIFY heartbeat every 30 s; absence triggers reconnect | Heartbeat detection runs in the LISTEN task |

The heartbeat self-probe (last row) is the **Earned Trust** application of Principle 9 to this component: the LISTEN connection cannot be trusted just because it has not raised an error. The probe verifies the substrate is *actually* delivering, not just *appears connected*.

## Probe contract (Principle 9)

At startup the replica runs three probes before flipping `/readyz` to 200:

1. **`probe.pg.listen_notify`** — open the dedicated LISTEN connection, `LISTEN _foundry_probe`, `pg_notify('_foundry_probe', 'ping')`, wait up to 1 s for receipt. Refuse to start if not received. Diagnostic: "LISTEN/NOTIFY does not work against this Postgres — check for pgbouncer in transaction mode or Postgres version too old."

2. **`probe.pg.notify_payload_cap`** — confirm payloads up to 7800 bytes succeed and 8001 bytes fail (Postgres-version-dependent; we assume 16+). Refuse if behavior differs from documented. Diagnostic: "Postgres reports a different NOTIFY payload limit than expected; payload encoder may corrupt large events."

3. **`probe.pg.advisory_lock`** — acquire and release the migration-lock ID. Refuse if it hangs >2 s. Diagnostic: "Another replica holds the migration lock; check for a stuck migration."

Each probe emits a structured `health.startup.refused` event naming the specific failure mode and a remediation. Probes that pass emit `health.startup.probe.ok`.

The heartbeat self-NOTIFY at runtime (above) is the *self-application* of Principle 9: it verifies the probes themselves remain honest after every Postgres upgrade.

## What this design does NOT do (and why that's OK for MVP)

- **No guaranteed delivery**: NOTIFYs are best-effort. A client offline at the moment of a NOTIFY misses it. The "refresh for latest" toast acknowledges this. The Slice-2 US-09 acceptance criteria explicitly waive replay.
- **No cross-instance subscription** (Foundry instance A sending events to Foundry instance B): out of scope; each instance is its own pub/sub island.
- **No per-user notification queue**: events are pushed to currently-connected SSE clients only. Email/in-app notification for offline users is post-MVP.
- **No event sourcing**: events are derived from row mutations, not the source of truth. The `issues` table is the source of truth; events are a derived projection. This means `pg_dump` captures the truth without needing to capture the event stream (NFR-DATA-02 holds).

## Cross-references

- Deploy variants where this runs: `topology.md`.
- Pool sizing in the broader capacity context: `scaling.md`.
- What happens when Postgres goes away entirely: `failure-modes.md`.
- ADR-101 (why we're not using Redis); the pg_notify path is the justification.
