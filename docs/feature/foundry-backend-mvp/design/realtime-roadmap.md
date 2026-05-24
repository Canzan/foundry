# Foundry MVP — Realtime Roadmap (Slice 1 -> Slice 2)

Slice 1 does **not** ship any SSE endpoints to the browser. But the architecture decisions in slice 1 must preserve the slice 2 SSE/LISTEN/NOTIFY story (US-09) without rework. This document lists what slice 1 establishes, what slice 2 layers on, and the invariants the boundary must hold.

## What Slice 1 Ships

1. **An outbox table** (`outbox`, see `data-access.md`) with `id BIGSERIAL`, `event_type TEXT`, `payload JSONB`, `notified_at TIMESTAMPTZ`. Every domain event-producing service writes to this table in the same transaction as its primary write.

2. **A `Publisher` type** in `foundry-realtime/src/publisher.rs` with one method:
   ```text
   pub async fn notify(&self, channel: &str, event: &DomainEvent) -> Result<(), PublishError>
   ```
   Implementation in slice 1: after the service's `tx.commit()`, the service calls `publisher.notify("issue_events", &event)`, which:
   - Serializes the event to a compact JSON payload (≤ 8000 bytes, Postgres NOTIFY hard limit).
   - Calls `SELECT pg_notify($1, $2)` on a pool connection.
   - Updates `outbox.notified_at = now()` for the corresponding outbox row.

3. **Domain event publication for `IssueCreated`** (US-08). Other events are written to outbox but not yet notified — slice 2 turns on notification for `IssueUpdated`, `IssueCommented`.

4. **No SSE endpoint, no per-replica listener.** These are slice-2 additions.

## What Slice 2 Adds

US-09 (realtime updates) and US-10 (comments) add:

1. **`SseHandler`** in `foundry-realtime/src/sse.rs` — an axum handler at `GET /projects/:project_id/events` that:
   - Authenticates the request (workspace-member middleware).
   - Authorizes that the user can read the project.
   - Opens an `axum::response::sse::Sse` stream backed by a `tokio::sync::broadcast::Receiver<DomainEvent>` filtered by project_id.
   - Sends a `:heartbeat\n\n` comment every 15 seconds to keep LBs from killing the connection (system-designer coordination question #3).

2. **Per-replica `PgListener`** in `foundry-realtime/src/listener.rs` — a background tokio task spawned once at startup that:
   - Holds a dedicated Postgres connection (not from the pool).
   - Executes `LISTEN issue_events`.
   - Loops: on each `NOTIFY` payload, deserializes the `DomainEvent`, sends it into the local `tokio::sync::broadcast::Sender<DomainEvent>`.
   - On connection error: reconnects with exponential backoff (1s, 2s, 4s, 8s capped).

3. **Wiring**: the broadcast Sender is held in `AppState`. Each new SSE client subscribes (`.subscribe()` -> Receiver), filters by their project_id, and streams.

4. **Outbox-poll fallback (optional, slice 2.5)**: a background task scans `outbox WHERE notified_at IS NULL AND created_at < now() - interval '5 seconds'` and re-notifies. Catches the rare case where `pg_notify` was lost (e.g., during a Postgres restart). Not in MVP-MVP, but the column already exists if we need it.

## Cross-Replica Fanout (How LISTEN/NOTIFY scales)

```mermaid
sequenceDiagram
    participant W as Writer browser
    participant R1 as Replica 1 (writer)
    participant PG as Postgres
    participant R2 as Replica 2
    participant R3 as Replica 3
    participant V as Viewer browser
    Note over R1,R3: All replicas have LISTEN issue_events on their dedicated connection

    W->>R1: POST /issues (create AUTH-9)
    R1->>PG: BEGIN; INSERT issues; INSERT outbox; COMMIT
    R1->>PG: SELECT pg_notify('issue_events', '{event:IssueCreated,project_id:auth-v2,...}')
    PG-->>R1: notification delivered to R1's listening conn
    PG-->>R2: notification delivered to R2's listening conn
    PG-->>R3: notification delivered to R3's listening conn
    R1->>R1: filter; no local subscribers for project auth-v2
    R3->>R3: filter; no local subscribers
    R2->>R2: filter; V is subscribed to project auth-v2 on R2
    R2->>V: SSE event {type:IssueCreated, html:<li id=AUTH-9>...</li>}
    V->>V: htmx swaps into #backlog-column
```

The key property: **Postgres is the broker; the app replicas are stateless w.r.t. routing.** Any replica can write; every replica receives; each replica forwards only to its locally-connected SSE clients.

Cost of LISTEN/NOTIFY at scale (rough): NOTIFY fans out in Postgres' notify backend in O(N_listeners). With 3-10 replicas, this is trivial. The bottleneck is the per-message JSON payload size (8000 byte hard cap, ~few hundred typical). If we ever need bigger payloads, we put the payload in the outbox and notify with `{outbox_id: 12345}` — listeners fetch from the table.

## Slice-1 Invariants the Boundary Must Hold

These are the *non-obvious* things slice 1 must get right so slice 2 lights up without surgery:

1. **Every issue-mutating service writes to the outbox in the same transaction.** Even though slice-1 only notifies for `IssueCreated`, ALL mutations must outbox. Otherwise slice-2 will need a per-service audit. The discipline is: "if you `INSERT`, `UPDATE`, or soft-delete an issue, you also `INSERT INTO outbox`."

2. **DomainEvent is in `foundry-core`, not in `foundry-store`.** The event types are domain concepts (they describe what happened in business terms, not what changed in SQL). This means the `Publisher` does not synthesize events from SQL diffs — services explicitly construct events.

3. **`pg_notify` is called *after* `tx.commit()`, not inside the transaction.** Postgres docs are clear: NOTIFY inside a transaction queues messages until commit. Calling NOTIFY then aborting the tx is a no-op. We avoid that ambiguity by calling NOTIFY explicitly post-commit.

4. **Payload JSON is forward-compatible.** The event payload is `{event_type, schema_version: 1, ...}`. Slice 2 listeners parse based on `schema_version`. We will never re-purpose a field name; new info goes in new fields.

5. **Per-project filtering is the publisher's responsibility.** The NOTIFY payload always carries `project_id` at the top level. Listeners filter; the channel is always `issue_events`. If slice 3 introduces non-project-scoped events, it gets its own channel (`workspace_events`).

6. **No app-tier state required for SSE correctness.** A client that reconnects to a different replica gets the same correctness (it joins that replica's broadcast); only missed-while-disconnected events are lost. NFR-AVAIL-03 explicitly accepts this for MVP; replay is a slice-4 add via `Last-Event-Id` + outbox lookup.

## Per-Replica Connection Budget Implication

Adds 1 long-lived Postgres connection per replica (LISTEN). Documented in `architecture.md` coordination question #2 for system-designer.

## Why SSE (and not WebSockets)

- SSE is HTTP. Goes through any HTTP-aware LB without WebSocket upgrade quirks.
- One-way (server-to-client) matches the use case (the client writes via POST, not over the realtime channel).
- Browser `EventSource` auto-reconnects. No client-side reconnect library.
- htmx has `sse-swap` built in: `<ul hx-ext="sse" sse-connect="/projects/auth-v2/events" sse-swap="IssueCreated">` swaps the named event into the element.

The DIVERGE recommendation already locked this; we confirm.

## Why LISTEN/NOTIFY (and not Redis pubsub / Kafka)

- NFR-DATA-01 says all state in Postgres. Adding a broker contradicts this.
- The outbox is in Postgres anyway (atomic with the write). Adding Redis means two-phase coordination for no perceived benefit at the 5-50 person team scale.
- If we outgrow LISTEN/NOTIFY (e.g., 1000s of LISTEN connections per Postgres), the migration is documented in DIVERGE's R-mitigation: swap the Publisher implementation to a broker. The outbox table remains the durable record.

## Client-side rendering decision (deferred to slice 2)

The DIVERGE recommendation noted "htmx `sse-swap` or vanilla EventSource + alpine-managed DOM update — choose one in DESIGN wave." Recommendation: **start with htmx `sse-swap`** for the project board's card rendering (server sends the rendered `<li>` HTML). Fallback to vanilla EventSource + alpine for cases where the server cannot easily produce the right fragment (e.g., comment count badges that need to increment across N different DOM locations).

This keeps the slice-1 templates reusable in slice 2 — the same `issue_card.html` partial renders both for full-page board GETs and for SSE-pushed updates.
