# Foundry MVP — Data Access Strategy

Owner: `foundry-store` crate. All Postgres interaction is here.

## Crate Choice: `sqlx` (confirmed)

DIVERGE recommended `sqlx`; this design wave confirms. Current evidence (2026):

- **sqlx** is on `0.8.x` as of 2026, MIT/Apache-2.0 dual licensed, active maintenance (release cadence ~2 months). Postgres support is first-class with native `LISTEN/NOTIFY` integration on a `PgListener`. Compile-time query checking via `sqlx::query!` macro.
- **sea-orm** is more abstracted (active record). Adds an ORM concept (T2 cost) without solving a real problem we have. Rejected.
- **diesel** is sync-first; `diesel-async` exists but lags behind. Compile-time safety is excellent but the macro-heavy DSL adds learning curve (Approachability taste filter fails). Rejected.

See `adrs/ADR-003.md` for the full rationale.

### Choice within sqlx: macros or query strings?

**Recommend: `sqlx::query!` and `sqlx::query_as!` macros where the SQL is static, plain `sqlx::query` for dynamic SQL.** Macros require a live database at build time (or the `SQLX_OFFLINE=true` mode with `sqlx prepare` cached `.sqlx/` directory committed to repo). We will:

1. Run `cargo sqlx prepare --workspace` as a pre-commit hook and in CI to refresh the offline cache.
2. Commit `.sqlx/` to the repo so contributors can build without a database (NFR-DEV-01: cold-start <= 10min).
3. Use macros for >90% of queries; plain `query` for the rare dynamic case (filter builders).

This balances compile-time safety with NFR-DEV-01's contributor onboarding promise.

## Connection Pool Strategy

Per replica:

- **One main pool** of size `DATABASE_MAX_CONNECTIONS` (default 10, per NFR-PERF-04). Used for all query handlers.
- **One dedicated single connection** for `LISTEN issue_events`, held by a background task (`foundry-realtime::listener`). A listening connection cannot be returned to the pool.
- Pool config: `min_connections=1, max_connections=10, acquire_timeout=5s, idle_timeout=10min, max_lifetime=30min`.

Total Postgres connections per replica: `10 + 1 = 11`. With 3 replicas = 33; Postgres default `max_connections=100` is comfortable. (System-designer owns capacity planning.)

## Migration Strategy

### Tooling

`sqlx-cli` is the migration tool. Migrations are plain SQL files in `crates/foundry-store/migrations/`, named with the `sqlx migrate add` convention: `0001_init.sql`, `0002_add_comments.sql`, etc.

### Forward-only rule

**No down migrations.** Rationale:
- Down migrations are routinely wrong (forgotten data, schema drift) and we have never reverted a production migration in real systems.
- The recovery story is `pg_restore` from yesterday's backup (NFR-DATA-02), not a `migrate down`.
- Eliminating down migrations halves the surface area to review per PR.

### Migration runner: invoked at startup, advisory-locked

The binary calls `sqlx::migrate!("./migrations").run(&pool)` during startup, wrapped in a Postgres advisory lock so concurrent replicas serialize migration runs (NFR-MIG-01).

```rust
// crates/foundry-store/src/lib.rs (illustrative; final form may differ)
const MIGRATION_LOCK_ID: i64 = 0x_F0_0D_BA_BE_F0_0D_BA_BE_u64 as i64;

pub async fn run_migrations(pool: &PgPool) -> Result<(), MigrateError> {
    let mut conn = pool.acquire().await?;
    // Block until we hold the lock. Released automatically when conn is dropped.
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(MIGRATION_LOCK_ID)
        .execute(&mut *conn)
        .await?;

    // Even though we hold the lock, sqlx's migrator also checks the _sqlx_migrations
    // table and skips already-applied migrations idempotently. So replicas that wait
    // for the lock and then proceed do not re-apply anything.
    sqlx::migrate!("./migrations").run(&mut *conn).await?;

    // explicit unlock for clarity; would also happen on conn drop
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(MIGRATION_LOCK_ID)
        .execute(&mut *conn)
        .await?;
    Ok(())
}
```

(Above is a sketch for the design discussion only — software-crafter writes the final form.)

### Why advisory lock, not just `_sqlx_migrations` table

sqlx's built-in migrator already skips applied migrations, but it does *not* serialize concurrent applications. Two replicas starting simultaneously could both try to apply migration `0042` at once: one succeeds, the other gets a duplicate-key error on `_sqlx_migrations`, and the replica fails to start. The advisory lock makes the race deterministic: one replica runs, the others wait then no-op.

### What about non-transactional migrations?

Some Postgres operations cannot run inside a transaction (`CREATE INDEX CONCURRENTLY`, `ALTER TYPE ... ADD VALUE`). For these, the migration file starts with a header comment:

```sql
-- sqlx-migrate: no-transaction
-- This migration uses CREATE INDEX CONCURRENTLY and runs outside a transaction.
-- Manual recovery on failure: see docs/operations/migration-recovery.md.
CREATE INDEX CONCURRENTLY idx_issues_by_state ON issues (project_id, state);
```

sqlx supports this via the `-- sqlx migrate: no-transaction` directive (parsed from the file header). NFR-MIG-02 documents the manual recovery path.

### Naming convention

`NNNN_short_snake_case_description.sql`:
- `NNNN` = zero-padded sequence number (`0001`, `0042`).
- Description names the change, not the ticket.
- No dates in the filename (the migration order is by number, not by chronology).

Examples:
- `0001_init.sql` — base schema
- `0002_add_issue_priority.sql`
- `0003_create_comments_table.sql` (slice 2)
- `0004_add_issue_attachments.sql` (slice 3)

## Example Migration: `0001_init.sql`

This is the slice-1 base schema. Annotated with rationale where non-obvious.

```sql
-- crates/foundry-store/migrations/0001_init.sql
-- Slice 1 base schema for Foundry MVP.

-- UUIDv7: time-ordered, cache-friendly inserts. Use the in-app generator
-- (uuid crate's v7) and pass UUIDs as parameters; we do NOT use the
-- uuid-ossp extension because it requires superuser to install.

CREATE TABLE workspaces (
    id           UUID PRIMARY KEY,
    name         TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- I-W1: at most one workspace per instance.
CREATE UNIQUE INDEX uniq_one_workspace ON workspaces ((true));

CREATE TABLE users (
    id                 UUID PRIMARY KEY,
    email_lower        TEXT NOT NULL UNIQUE,
    email_display      TEXT NOT NULL,
    display_name       TEXT NOT NULL CHECK (length(display_name) BETWEEN 1 AND 64),
    password_hash      TEXT NOT NULL,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- workspace_memberships: workspace-level role (admin or member).
CREATE TABLE workspace_memberships (
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role          TEXT NOT NULL CHECK (role IN ('admin', 'member')),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, user_id)
);

CREATE TABLE teams (
    id            UUID PRIMARY KEY,
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    slug          TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, slug)
);

CREATE TABLE team_memberships (
    team_id     UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role        TEXT NOT NULL CHECK (role IN ('lead', 'member')),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (team_id, user_id)
);

CREATE TABLE projects (
    id                 UUID PRIMARY KEY,
    team_id            UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    workspace_id       UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name               TEXT NOT NULL,
    slug               TEXT NOT NULL,
    key_prefix         TEXT NOT NULL CHECK (key_prefix ~ '^[A-Z]{2,6}$'),
    next_issue_number  INTEGER NOT NULL DEFAULT 1 CHECK (next_issue_number >= 1),
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (team_id, slug),
    UNIQUE (workspace_id, key_prefix)
);

CREATE TABLE issues (
    id                  UUID PRIMARY KEY,
    project_id          UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    workspace_id        UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    number              INTEGER NOT NULL,
    title               TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 256),
    description_md      TEXT NOT NULL DEFAULT '' CHECK (length(description_md) <= 262144),
    state               TEXT NOT NULL DEFAULT 'backlog'
                        CHECK (state IN ('backlog','todo','in_progress','done','cancelled')),
    priority            TEXT NOT NULL DEFAULT 'medium'
                        CHECK (priority IN ('urgent','high','medium','low','no_priority')),
    assignee_id         UUID REFERENCES users(id) ON DELETE SET NULL,
    author_id           UUID NOT NULL REFERENCES users(id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, number)
);

CREATE INDEX idx_issues_project_state ON issues (project_id, state);
CREATE INDEX idx_issues_assignee ON issues (assignee_id) WHERE assignee_id IS NOT NULL;

CREATE TABLE bootstrap_tokens (
    id          UUID PRIMARY KEY,
    token_hash  BYTEA NOT NULL UNIQUE,        -- SHA-256 of the raw token; raw never stored
    expires_at  TIMESTAMPTZ NOT NULL,
    used_at     TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE invites (
    id              UUID PRIMARY KEY,
    workspace_id    UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    invitee_email   TEXT,                          -- NULL for link-only invites
    created_by      UUID REFERENCES users(id),
    expires_at      TIMESTAMPTZ NOT NULL,
    used_at         TIMESTAMPTZ,
    used_by         UUID REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Outbox: durable record of every DomainEvent. Slice-1 writers insert here;
-- slice-2 SSE may also poll for events that pg_notify dropped on the floor.
CREATE TABLE outbox (
    id            BIGSERIAL PRIMARY KEY,
    event_type    TEXT NOT NULL,
    payload       JSONB NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    notified_at   TIMESTAMPTZ                       -- set when pg_notify was issued
);
CREATE INDEX idx_outbox_pending ON outbox (id) WHERE notified_at IS NULL;

-- tower-sessions table is created by tower-sessions-sqlx-store's own migration.
-- We invoke its migrator after ours runs (see foundry-auth::sessions).

-- Brute-force tracking: rolling counter keyed by email_lower.
CREATE TABLE signin_attempts (
    email_lower    TEXT NOT NULL,
    attempt_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    success        BOOLEAN NOT NULL
);
CREATE INDEX idx_signin_attempts_email_time ON signin_attempts (email_lower, attempt_at);
```

(Total: 9 tables + tower-sessions's own. Roughly 130 lines of SQL. Reviewable in one sitting.)

## Reading from the Store: One Example

For "GET /projects/auth-v2" (project board view), the handler invokes:

```rust
// crates/foundry-store/src/issues.rs (illustrative)
pub async fn list_by_project(
    pool: &PgPool,
    project_id: ProjectId,
) -> Result<Vec<Issue>, StoreError> {
    let rows = sqlx::query_as!(IssueRow,
        r#"
        SELECT id, project_id, number, title, description_md,
               state as "state: _", priority as "priority: _",
               assignee_id, author_id, created_at, updated_at
          FROM issues
         WHERE project_id = $1
         ORDER BY state, number
        "#,
        project_id.0
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(Issue::from).collect())
}
```

Key properties:

- `IssueRow` is a private struct in the store crate; it has sqlx derives.
- `Issue` is the domain type from `foundry-core`; it has no sqlx awareness.
- `Issue::from(IssueRow)` is a `From` impl in the store crate that does the field-by-field translation including enum parsing.
- The handler receives `Vec<Issue>` and never sees a `Row`.

## Transactions

Use `sqlx::Transaction` for any write that touches >1 row. The `create_issue` example from `architecture.md` looks like:

```rust
// pseudocode for service code; lives in foundry-app::services
pub async fn create_issue(
    state: AppState,
    actor: AuthUser,
    project_id: ProjectId,
    title: NonEmptyString,
) -> Result<Issue, ServiceError> {
    let mut tx = state.pool.begin().await?;
    state.store.projects.authorize_member(&mut tx, project_id, actor.user_id).await?;
    let number = state.store.projects.allocate_next_number(&mut tx, project_id).await?;
    let issue = Issue::new(project_id, number, title, actor.user_id);
    state.store.issues.insert(&mut tx, &issue).await?;
    state.store.outbox.insert(&mut tx, &DomainEvent::IssueCreated{..}).await?;
    tx.commit().await?;
    state.publisher.notify("issue_events", &issue).await?;  // outside tx
    Ok(issue)
}
```

## CI Discipline

In CI:

1. Spin up `postgres:16-alpine` container.
2. Run `cargo sqlx prepare --workspace --check` to verify `.sqlx/` cache matches actual queries.
3. Run `cargo test --workspace` against the live Postgres.
4. Run `cargo deny check` (licenses + security advisories).

The `cargo sqlx prepare --check` step catches "query was edited but cache not updated" PRs at CI time.

## What is NOT in the Store

- **No business logic.** All authorization checks happen in services or middleware, then the store executes the query. The store does not know "user X is allowed to write to project Y."
- **No domain construction with invariants.** `IssueRow -> Issue` should be infallible (rows in the DB are presumed already-valid because we wrote them through `Issue::new()`).
- **No event publication.** `pg_notify` is owned by `foundry-realtime`, called by the service after `tx.commit()`.
