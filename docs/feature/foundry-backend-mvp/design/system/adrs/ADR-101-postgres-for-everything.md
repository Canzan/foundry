# ADR-101: Postgres-for-everything (vs. Redis sidecar)

## Status

Accepted (2026-05-23). Confirms a decision originally framed in `recommendation.md`.

## Context

The MVP needs four substrate capabilities that web applications conventionally split across two systems:

1. **Relational data**: workspaces, teams, projects, issues, comments, attachments.
2. **Server-side sessions**: user session rows with expiry.
3. **Background work queue**: e.g., email send for invites, deferred attachment processing.
4. **Pub/sub for realtime fan-out**: cross-replica wake-up so an SSE client on replica B sees an event published by replica A (US-09, NFR-PERF-03).

The conventional stack pairs Postgres (1) with Redis (2-4). This ADR records the decision to put all four in Postgres.

## Decision

Use Postgres for all four capabilities in the MVP:

- **Data**: standard `sqlx` access patterns.
- **Sessions**: `tower-sessions-sqlx-store` writes session rows to a `sessions` table.
- **Queue**: an `outbox` table polled with `pg_notify`-driven wake-up (so polling is rare; wake-up is event-driven).
- **Pub/sub**: `LISTEN/NOTIFY` on a single channel `issue_events`, with per-replica in-process fan-out to local SSE subscribers (see `realtime-infrastructure.md`).

## Alternatives considered

### A — Redis as sidecar for sessions / queue / pubsub

- **Pros**: industry standard; well-known performance; clear separation of concerns; queue libraries like `apalis` and `river-rs` have first-class Redis support.
- **Cons** (decisive):
  - Adds a second stateful service to operate — backups, upgrades, monitoring all double.
  - The "under-an-hour install" promise (US-01) needs a 3-container compose instead of 2.
  - The `pg_dump = complete backup` promise (US-03, NFR-DATA-01/02) dies if sessions live in Redis (operator's restore loses every active session) and gets messy if the outbox lives in Redis (queue state isn't in the dump).
  - The "boring monolith" recommendation (recommendation.md) explicitly subtracts "Redis sidecar" as a T1-Subtraction win.

### B — Postgres + Redis only for pub/sub (the most narrowly scoped Redis use)

- **Pros**: addresses the LISTEN/NOTIFY concern (pgbouncer compatibility, payload caps); pub/sub is Redis's strongest suit.
- **Cons**: still requires running Redis; one more substrate to keep healthy. The payload-cap concern (8000 bytes) is mitigated by sending small wake-up events and re-fetching on the client (the chosen design); the pgbouncer concern is mitigated by a startup probe that refuses pgbouncer-in-transaction-pool mode.

### C — Postgres + NATS / Kafka as the queue + pub/sub layer

- **Pros**: cleaner semantics for guaranteed delivery; replay built in.
- **Cons**: violates the "boring monolith" promise. Two-developer team, MVP scope; NATS adds an operator concept and a deploy artifact.

## Consequences

### Positive

- Single backup primitive (`pg_dump`) captures sessions, queue state, attachments, data. US-03 becomes trivial.
- Single deploy artifact: 2-container compose. US-01 timing target preserved.
- One operator skill (Postgres) instead of two (Postgres + Redis).
- Migration story is single-system; advisory locks (NFR-MIG-01) work cleanly with one DB.
- The Foundry binary's *only* substrate dependency is Postgres — vastly simplifies the probe contract (Principle 9).

### Negative (explicit trade-offs)

- LISTEN/NOTIFY has real limitations: 8000-byte payload cap; per-connection registration; breaks under pgbouncer transaction pooling. Mitigated by (a) small wake-up payloads, (b) dedicated LISTEN connection outside the request pool, (c) `probe.pg.listen_notify` refusing to start under pgbouncer-tx-pool.
- The outbox polling pattern is not Kafka. Throughput ceiling is well above MVP needs (the recommendation.md key-risk note flags this) but a future scaling rung might require `river-rs` or `apalis` with the Postgres backend. The outbox abstraction lives behind a trait so this is a one-file swap.
- Postgres becomes a "more critical" SPOF — but it was already the SPOF for data. Adding sessions/queue/pubsub doesn't change the failure radius; it just makes the SPOF more *visible* (and therefore more honestly accounted for in ADR-105).

### Evidence supporting the decision

- The recommendation.md scoring matrix gave D1 a 5/5 on T1 Subtraction explicitly because of this choice.
- Production examples of the same pattern: Discourse (Postgres + Sidekiq with Postgres), GitLab (Postgres + Redis but Redis is gradually being moved into Postgres), Plausible Analytics (Postgres-only stack).
- The 20-person workload (scaling.md) generates 0.05 mutations/sec peak — orders of magnitude below LISTEN/NOTIFY's throughput ceiling. The 200-person workload generates ~0.4 mutations/sec — still comfortable.

## Review trigger

Revisit this ADR if any of:

1. Realtime fan-out latency exceeds NFR-PERF-03 (median 1 s) under nominal load.
2. Outbox-polling causes Postgres CPU >20% sustained.
3. Foundry installs grow past the 200-person ceiling and we need true pub/sub semantics (guaranteed delivery, replay, multi-instance fan-out).

In any of these cases, the candidate replacement is Redis Streams (for pub/sub) and `river-rs` with Postgres backend (for queue, still no Redis) — not adding Redis.
