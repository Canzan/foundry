# ADR-EIP-002: Durable after-commit delivery: the outbox becomes a dispatched log, a deliveries table is the queue, and every replica runs a fenced, leased sender

## Status

Accepted (2026-10-27, by the user at the end of external-integration-plugins DELIVER).
Implemented (finalized 2026-10-27 at `8c681b2`; not yet released). DD-2 was amended on
2026-10-27 (FU-11, fixed in `c7694b2`). Departures from this record are listed in `docs/evolution/2026-10-27-external-integration-plugins.md` (Key decisions).

Proposed (2026-10-21, external-integration-plugins DESIGN; revised after peer review iteration 1).
Changes notification-delivery-providers ADR-007 for routes and project emails only (CA-1, OD-7
accepted); account notifications keep ADR-007.

## Context

- OD-7 (accepted): routes and project emails are queued after commit, retried with backoff for up to
  24 h honouring "retry after", at-least-once with a stable event id. NFR-EIP-3: every event delivered
  or shown failed within 24 h; at most one attempt per destination per second. US-EIP-05: per route,
  one issue's messages arrive in order.
- SC-1 / NFR-EIP-2 / EIP-KPI-4: a board action's p95 stays within 10% of the no-routes baseline with
  every destination unreachable. SC-2: no event without its change, none silently dropped.
- CA-5 / R3: the shipped `outbox` (0001, trigger 0003) is written in the same transaction as every
  board change, but nothing consumes it durably: the NOTIFY is fire-and-forget and `notified_at` is
  never written. `outbox_pending_jobs` (`count_pending_outbox`, `WHERE notified_at IS NULL`) is a
  shipped gauge whose acceptance scenario asserts it rises with writes.
- Migrations run at replica boot (`run_migrations`), while older replicas keep serving and writing
  outbox rows (us-04 rolling-upgrade harness). The outbox is never pruned today, so it can be large.
- Background-task precedent: `tokio::spawn` loops in `main.rs`; an advisory lock where only one replica
  may act (tombstone GC, `TOMBSTONE_GC_LOCK_ID`).
- Volume (NFR-EIP-7): up to 50 routes per workspace, about 1,000 events per day.

## Options considered

1. **Consume the outbox through LISTEN/NOTIFY.** Rejected: a NOTIFY sent while no replica listens is
   lost (restart, deploy), so SC-2 fails.
2. **Every writer also inserts integration rows in its transaction** (a second outbox). Rejected: six
   writers in three modules must each remember it, route lookups would run inside the board
   transaction (EIP-KPI-4), and the shipped outbox already is the transactional event record.
3. **A trigger copies outbox rows into a queue table.** Workable, but puts route matching and the burst
   rule in PL/pgSQL, outside the property-tested core, and adds route reads to every outbox insert.
4. **Outbox as the durable log, read by a dispatcher that marks rows it has expanded; a deliveries
   table is the work queue (chosen).**
5. **An external broker (NATS, Redis streams, RabbitMQ).** Rejected: new infrastructure for 1,000
   events a day, and it reintroduces the dual-write problem the outbox solves.

## Decision

1. **Migrations that never rewrite or long-lock `outbox`** (slice 01). Every statement sets
   `lock_timeout` (5 s) so a busy table fails the migration instead of stalling board writes:
   - 0028 adds three **nullable columns without defaults** (`event_id uuid`, `source_txid xid8`,
     `dispatched_at timestamptz`), then `ALTER … SET DEFAULT gen_random_uuid()` and
     `SET DEFAULT pg_current_xact_id()` (metadata only; applies to new rows). Nothing is backfilled.
     It records the dispatch start: one row in `integration_dispatch_start` holding the highest
     outbox `id` at migration time;
   - 0029 (`-- no-transaction`) creates the partial index on `outbox (id) WHERE dispatched_at IS NULL
     AND event_id IS NOT NULL` with `CREATE INDEX CONCURRENTLY`.
   Old rows have a NULL `event_id`, sit at or below the dispatch start, and are never dispatched. Rows
   written by an old replica after 0028 get the defaults and are real new events. `notified_at` and
   the `outbox_pending_jobs` gauge keep their shipped meaning (SC-7). A gold test runs board writes
   while 0028 and 0029 apply.
2. **Dispatcher (one active replica, transaction-scoped advisory lock):** each tick it selects whole
   source transactions (a subquery picks up to N `source_txid` values in `min(id)` order, then all
   their undispatched rows; a group is never split). It hands them to the pure `DispatchPlan`, which
   maps rows to `EventKind`, matches live routes and email choices, and coalesces bursts of more than 5
   same-kind events per route. It then inserts the plan's `integration_deliveries` rows and sets
   `dispatched_at`, all in **one** transaction. The store exposes this as one call that holds the
   transaction and takes the plan as a closure returning store-level `NewDelivery` rows. Expansion is
   exactly-once. Paused routes, routes created after the event, deleted projects and disconnected
   connections get no delivery rows.
3. **Route boundary:** a route receives an event only if the outbox row's `created_at` is at or
   after the route's `receives_from`. `receives_from` is set by the database's `now()` in the create
   or resume transaction, the same clock as `outbox.created_at`, never by the app `Clock`. A
   lagging dispatcher therefore never sends a new route events from before it existed, and a
   resumed route never receives what was skipped while paused (D-8). Accepted edge: `now()` is the
   transaction start time, so an event whose transaction started before the route's but committed
   after it is not delivered to that route.
4. **Sender (every replica): claim, send, record, all fenced.**
   - The claim runs in one short transaction. It locks a candidate route row
     (`FOR UPDATE SKIP LOCKED`) and its destination pacing row. It reads that route's non-terminal
     deliveries and asks the pure `claimable_head(route, deliveries, pacing, now) -> Option<id>`,
     which returns the **head** (the lowest-id non-terminal delivery) only if the head has no live
     lease, `next_attempt_at <= now` and the destination's `not_before <= now`. The store then
     writes, by compare-and-set on that id, a fresh `claim_id` (uuid), `attempts + 1`,
     `lease_until = now + lease`, and the destination's `not_before = now + 1 s`, and commits. Because
     the head stays the head while its lease lives, a second replica can never take row 2 of a route
     while row 1 is in flight. Per-route order holds across replicas.
   - The send happens **outside any transaction**, with a 10 s timeout.
   - The record runs in a second short transaction, as a compare-and-set
     `WHERE id = … AND claim_id = …`. A recorder whose lease was taken over writes nothing.
   - Lease = `FOUNDRY_INTEGRATIONS_LEASE_SECONDS`, default 45. Boot refuses a value under the
     worst-case claim-to-record path plus 10 s. That path is the sum of every timeout the adapter may
     spend: resolve 5 s, pre-send read 10 s (Matrix encryption), send 10 s. That gives 25 s, so the
     minimum is 35 s.
   - A crash leaves an expired lease; the next claim re-sends with the same `event_id`
     (at-least-once). Matrix sends use the delivery id as `txnId`, so the homeserver de-duplicates a
     repeat. Slack has no idempotency key and may rarely show one.
5. **Pacing is per destination, not per route** (NFR-EIP-3): `integration_destination_pacing`
   `(connection_id, destination_ref, not_before)`. Two routes to one channel share a row. A
   connection-wide "retry after" moves every destination row of the connection (and the connection's
   own `not_before`). Email paces per recipient.
6. **Retry policy is a pure function** of (attempts, outcome, retry_after, give_up_at, now). Slice 01
   retries every 30 s; slice 03 backs off from 10 s, doubling to a 15-minute cap with ±20% jitter, and
   waits longer when `retry_after` says so. It never schedules an attempt after `give_up_at`.
   **Give-up:** each delivery stores `give_up_at = max(occurred_at, dispatched_at) + window`
   (window 24 h), fixed at dispatch and never NULL. With a running dispatcher this equals
   `occurred_at + 24 h` to within a poll interval. After a dispatcher stall the window counts from
   dispatch, so a stall delays messages instead of failing them unseen.
   **The guarantee:** every delivery is delivered, skipped with a reason, or failed with a
   `final_reason` by its `give_up_at`, **whatever the route or connection state**. A blocked delivery
   (connection lost, route needs attention, credential unavailable) is attempted when its blocker
   clears. If the blocker does not clear in time, it fails unattempted, and `final_reason` names the
   blocker.
7. **Outcomes:** `RouteRefused` and permanent answers fail at once. `ConnectionRefused` marks the
   connection lost (ADR-EIP-005/006) and leaves its deliveries pending, so a reconnect within the
   window drains them (D-8). A local fault (a credential this replica cannot open, ADR-EIP-004) is
   `Transient` with reason `secret_unavailable` and never changes connection state.
8. **Send-time checks:** the sender re-reads the route, connection, project (through `live_projects`),
   issue and, for email, the member's access, mute and choice. A delivery whose project was deleted,
   route paused or deleted, connection disconnected, issue or comment deleted since the event, or
   recipient who lost access, ends `skipped` with a reason and is never sent.
9. **Sweeps** run on one replica at a time under their own advisory lock. Each one is a
   **plan-returning pure function** in the core (`expiry_set(now, deliveries)`,
   `cancel_set(target, deliveries)`, `prune_set(now, deliveries, outbox_refs)`, each returning ids).
   The store then applies one compare-and-set update on exactly those ids (non-terminal status, no
   live lease), so a sweep and a claim can never both win:
   - the expiry sweep applies the give-up rule;
   - the cancel step (pause, disconnect, DD-8) marks the route's or connection's pending deliveries
     `skipped`;
   - the retention sweep deletes finished deliveries older than 30 days beyond the newest 100 per
     route, and dispatched outbox rows older than 30 days that no non-terminal delivery references
     (this bounds the deleted-issue titles and comment-author addresses the outbox holds).
10. **Stall visibility:** gauges `foundry_outbox_undispatched_oldest_seconds` and
    `foundry_integration_deliveries_pending_oldest_seconds`. The dispatcher writes a heartbeat
    (`integration_worker_heartbeats`). A warn-level `integrations.dispatch.stalled` log fires when the
    oldest undispatched row is over 5 minutes old. Readiness is unaffected (the board must not depend
    on integrations).
11. **Timing comes from the injected `Clock`,** passed into SQL as a parameter (the
    `count_unclaimed_bootstrap_tokens(now)` precedent), so acceptance can shorten windows. The settings
    are `FOUNDRY_INTEGRATIONS_GIVE_UP_SECONDS` (86400), `FOUNDRY_INTEGRATIONS_POLL_MS` (1000) and
    `FOUNDRY_INTEGRATIONS_LEASE_SECONDS` (45). Exceptions are `outbox.created_at`, `dispatched_at`
    and `receives_from`, which are database time on both sides of their comparison (decision 3).
12. **Board latency is protected structurally:** board writers gain no query (two column defaults
    and three payload fields, ADR-EIP-003). The worker never holds a pooled connection across an
    outbound call, and in-flight sends per replica are capped (default 4).

## Consequences

- Positive: SC-2 holds by construction. The event row commits with the change; expansion and the mark
  commit together; sends are leased, fenced and retried. Every delivery ends with a visible outcome
  by its `give_up_at`.
- Positive: no new infrastructure: an ordinary transactional outbox plus a `SKIP LOCKED` queue on the
  Postgres 16 the app already runs. Deploys never rewrite the outbox.
- Positive: multi-replica safe without leader election for sending; only the cheap dispatch step and
  the sweeps are single-active.
- Negative: head-of-line ordering means one message stuck in transient failure delays later messages
  of the same route, for up to 24 h. Accepted: the route shows "Retrying", and a route's messages
  share one destination, so they usually fail together (DD-4).
- Negative: per-route order is by outbox `id`. Issue writers hold the issue row (`UPDATE` or `DELETE`
  on `issues`) before inserting their outbox row, so one issue's create, moves and delete keep commit
  order. A comment insert does not lock the issue, so a comment racing a move on the same issue can be
  ordered by `id` draw rather than commit, within milliseconds. A store test pins the issue-writer
  invariant.
- Negative: polling adds up to one poll interval of latency (1 s default, inside NFR-EIP-1's 10 s).
- Negative: after a dispatcher stall, messages arrive late rather than failing unseen; the window
  counts from dispatch. "Within 24 h" (NFR-EIP-3) holds whenever the dispatcher runs, and the stall
  gauges make a stall visible within minutes.
- Earned Trust: `Store::probe` refuses a schema without the new columns and tables. The gold tests:
  - kill a replica mid-send: the lease expires and the delivery is re-sent with the same `event_id`;
  - lease expiry during a slow send: the late recorder writes nothing;
  - two replicas with a slow send: row 2 never overtakes row 1;
  - two routes to one channel: at most one attempt per second;
  - roll back a board transaction: no delivery;
  - board writes during migrations 0028 and 0029;
  - a 25-hour dispatcher stall: deliveries get a fresh window from dispatch and are attempted;
  - connection lost and never reconnected: the delivery fails unattempted at `give_up_at`, naming
    the blocker.
