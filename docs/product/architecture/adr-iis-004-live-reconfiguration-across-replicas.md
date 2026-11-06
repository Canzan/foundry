# ADR-IIS-004: Instance integration changes are serialised by one lock, versioned in the same transaction, and picked up by a polling reload on every replica

## Status

Proposed (instance-integration-setup DESIGN, 2026-11-05). Options 3-A (no separate spike) and 4-A
were chosen by the user on 2026-11-05
(`docs/feature/instance-integration-setup/design/options.md`).

## Context

- D10: every change takes effect without a restart, on the serving replica at once and on every
  replica within 60 s. While replicas catch up, nothing is marked lost and no email is sent.
- D2 refinement: Generate is create-if-absent across replicas.
- D4: re-seals are all-or-nothing.
- `Integrations` is built once at boot. `IntegrationsHandle` (`Arc<RwLock<Arc<Integrations>>>`)
  already supports `install`.
- Every handler reads `current()` per request, and the worker reads it per poll round.
- Postgres LISTEN/NOTIFY exists for the board (`foundry-realtime`) but loses messages across a
  reconnect.

## Decision

1. **One lock.**
   - Every instance-level key or app write takes `pg_advisory_xact_lock` on one fixed key named
     `foundry.instance-integrations`: Generate, Move, Replace, Start over, re-wrap, `doctor
     rotate-secrets`, Slack save, Slack confirm and Slack remove.
2. **The seal fence.**
   - Every other write that stores a sealed value takes the same lock **shared**. These are:
     - Slack return (connect and reconnect);
     - Matrix connect;
     - the I-14 pending-app insert (`instance_slack_app_pending`, ADR-IIS-003 r4).

     Slack save itself holds it exclusive (rule 1).
   - **Lock order.** The advisory lock is always taken first, before any row lock. That includes
     every `SELECT … FOR UPDATE`, such as `reseal_credentials`'s lock over
     `integration_connections` (`keys.rs`), which is called only from within a transaction that
     already holds the exclusive lock.
     - A connect write takes the shared lock before touching its connection row.
     - No path takes a row lock and then the advisory lock, so the two cannot deadlock.
     - The dispatcher's own advisory lock is a different key and never nests with this one.
   - In its transaction it checks that the key id it sealed under is still the sealing key: the
     stored secrets key id, or no stored key in `Direct` mode.
   - On a mismatch the store refuses with a stale-key error. The handler reloads its integrations
     and retries once.
   - Nothing is ever sealed under a discarded key, which is why Replace needs no grace window.
   - The fence lives in the store transaction, not in the handlers. Today's sealed writes reach the
     store through the connections store and the services connection use-cases.
   - check-arch `integrations-seal-fence` (ADR-IIS-001) keeps every sealed-column write inside
     fenced store functions.
   - Previous-release replicas during a roll write unfenced, but they seal only under the instance
     key they hold. That key stays in every new replica's credential box, so those values open.
     The next Replace or Move re-seals every stored value, because the re-seal set is "all sealed
     values", not "values under the old key". Move is pressed after the roll (D5).
3. **Version.**
   - Single-row `integration_config_version` (`id = 1`, `version bigint`), bumped in the same
     transaction as every write under rule 1.
4. **Reload.**
   - Each replica runs one reload task. Every `FOUNDRY_INTEGRATIONS_RELOAD_MS` (default 5000) it
     reads the version.
   - On a change it rebuilds `Integrations` from the boot-time settings and held keys plus the
     database state, through the same shared boot function (ADR-IIS-001 decision 9), and
     `install`s it. Reload never touches the key file (ADR-IIS-002).
   - The version and the state it rebuilds from are read in **one `REPEATABLE READ` transaction**,
     version first. A commit landing between the reads can then never be recorded as applied
     without being seen.
   - A gold test injects a write between the two reads and asserts the next tick applies it.
   - The snapshot covers, in that one transaction:
     - `integration_config_version`;
     - `integration_secrets_key` (key id, wrapped key, wrapping key id and source,
       `rewrapped_from_path`);
     - `instance_slack_app` (client ID, sealed secrets, saved by and at);
     - `integration_key_checks`;
     - the key ids of every stored sealed value: `integration_connections.sealed_credential`,
       `instance_slack_app`, `instance_slack_app_pending`.

     Not covered:
     - the environment settings and held instance keys (boot-time, held in memory);
     - activity lines and pending-row expiry (read by the page on demand).
   - The serving replica does the same right after its POST commits, before rendering the answer.
   - A failed read or rebuild keeps the last good integrations, logs `integrations.reload.failed`
     (warn), and retries on the next tick.
5. **Unknown key id triggers a reload.**
   - A replica that meets a sealed value whose key id it does not hold reloads at once, at most once
     per second by the injected `Clock`, before judging it unreadable.
6. **Held keys survive.** A rebuild whose stored secrets key id equals the one held keeps the held
   key without unwrapping. A re-wrap under a new instance key therefore never blinds a replica
   that has not restarted.
7. **In flight.**
   - A request keeps the `Arc<Integrations>` it read.
   - The worker's send in flight finishes under its snapshot, and the next round uses the new one.
   - An open failure in the window is the existing `Transient(secret_unavailable)`.

8. **Replica state.**
   - Each replica upserts one row in `integration_replica_states` (the
     `integration_worker_heartbeats` pattern). Columns: `replica`, `applied_version`, `key_mode`,
     `opens_secrets_key`, `beat_at`.
   - It writes the row at boot, on every applied reload, and every 10 s.
   - It never holds a key or key id.
   - Uses:
     - Start over's "no live replica opens it" gate (ADR-IIS-001 decision 7);
     - an operator-visible signal that a replica is stale or holds a different key;
     - the acceptance observable for "applied on every replica".
   - Rows older than 1 hour are pruned by the existing retention tick.
   - The retention tick also deletes expired `instance_slack_app_pending` rows.
   - Every time comparison uses the injected `Clock` (`AppState.clock`; `MockClock` or
     `WallTickingClock` in acceptance, as `multi_replica_harness` shares one) and is passed into
     SQL as a parameter, never `now()`. That covers:
     - `beat_at` and the 30 s freshness gate;
     - pruning;
     - the unknown-key-id rate limit;
     - pending expiry.

## Alternatives considered

- **LISTEN/NOTIFY** (3-B). Rejected: it still needs a poll to be correct after a reconnect, plus a
  dedicated connection per replica and a caveat for transaction poolers. It would be the poll plus
  more code.
- **Reload on read with a TTL** (3-C). Rejected: a query, and sometimes a Slack client rebuild, on
  the request path, and the worker needs its own trigger.
- **Constraints only** (4-B). Rejected: there is no fence for concurrent connect handlers, so
  Replace would need a previous-key row and a straggler sweep.
- **SERIALIZABLE with retry** (4-C). Rejected: every connect path must handle serialization
  failures.

## Consequences

- Positive: a worst-case 5 s lag, well inside 60 s.
- Positive: it works through any pooler and cannot miss a change.
- Positive: it is testable in process (`multi_replica_harness`, short interval).
- Negative: one trivial query per replica per 5 s, and a shared advisory lock on connect writes.
- Probe (Earned Trust): the reload task's first read at boot is part of wire, probe, use. A reload
  that cannot read keeps serving the last good state and says so.
- Spike: none separate. The walking-skeleton scenario runs on two replicas and is the spike.
