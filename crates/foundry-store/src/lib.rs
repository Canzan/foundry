//! foundry-store — Postgres adapter for slice 1.
//!
//! Public surface kept minimal: [`Store`] owns the [`sqlx::PgPool`] and
//! exposes [`Store::connect`], [`Store::migrate`], [`Store::probe`].
//! Per-aggregate repository modules land as US-05..US-08 require them.

#![forbid(unsafe_code)]
#![deny(clippy::all)]

pub mod attachments;

pub use attachments::{AttachmentInsertError, AttachmentRow, AttachmentSummary};

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;
use thiserror::Error;

/// Advisory-lock key used to serialize migrations across replicas.
/// (`data-access.md` §"Migration runner".)
const MIGRATION_LOCK_ID: i64 = 0x_F0_0D_BA_BE_F0_0D_BA_BE_u64 as i64;

/// Slice 7 (ADR-015) — advisory-lock key for the daily tombstone GC
/// sweep. Distinct literal from [`MIGRATION_LOCK_ID`] so `pg_locks`
/// output distinguishes the GC lock from the migration lock during
/// operational triage. Non-blocking acquisition via
/// `pg_try_advisory_lock` ensures sibling replicas exit gracefully
/// with `Ok(0)` when another replica is mid-sweep.
pub const TOMBSTONE_GC_LOCK_ID: i64 = 0x_60_C0_DE_60_C0_DE_60_60_u64 as i64;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration failed: {0}")]
    MigrationFailed(#[from] sqlx::migrate::MigrateError),
}

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("probe failed: {0}")]
    Failed(String),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

#[derive(Debug, Clone)]
pub struct ProbeReport {
    pub select_one_ok: bool,
    pub round_trip_ms: u128,
}

/// Read-only snapshot of the underlying sqlx connection pool's state.
///
/// Slice 6 (handler-instrumentation, ADR-012): the background poll task
/// in `foundry-app::main` reads this every 5 seconds and updates the
/// `db_connections_in_use` Prometheus gauge. The snapshot is cheap —
/// `Pool::size()` and `Pool::num_idle()` are non-blocking atomic loads.
///
/// Invariant: `in_use + idle == size` at the instant the snapshot is
/// taken (the values may individually shift between the two reads, but
/// the relationship holds when interpreted as "what sqlx thinks the
/// pool looks like right now"). The gauge consumes `in_use` only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolStats {
    pub in_use: i32,
    pub idle: i32,
    pub size: i32,
}

/// Postgres-backed store. Wraps a [`sqlx::PgPool`].
#[derive(Debug, Clone)]
pub struct Store {
    pool: PgPool,
}

impl Store {
    /// Open a connection pool against `database_url`.
    ///
    /// Pool sized to 10 by default (NFR-PERF-04). Callers may rebuild
    /// the pool with bespoke options for tests.
    pub async fn connect(database_url: &str) -> Result<Self, StoreError> {
        let pool = PgPoolOptions::new()
            .min_connections(1)
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(5))
            .idle_timeout(Duration::from_secs(600))
            .max_lifetime(Duration::from_secs(1800))
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    /// Construct a store from an externally-built pool (used by tests
    /// that drive a [`testcontainers`] Postgres with bespoke search_path).
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Run sqlx migrations under an advisory lock so concurrent replicas
    /// serialize on application startup.
    pub async fn migrate(&self) -> Result<(), StoreError> {
        let mut conn = self.pool.acquire().await?;
        sqlx::query("SELECT pg_advisory_lock($1)")
            .bind(MIGRATION_LOCK_ID)
            .execute(&mut *conn)
            .await?;
        let result = sqlx::migrate!("./migrations").run(&mut *conn).await;
        // Always try to release the lock — even on migration failure.
        let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(MIGRATION_LOCK_ID)
            .execute(&mut *conn)
            .await;
        result.map_err(StoreError::from)
    }

    /// Liveness probe: `SELECT 1` round-trip + slice-5 migration-0006
    /// column-existence assertion (Earned Trust per architecture.md).
    /// The "comments.updated_at + comments.deleted_at exist" check
    /// catches the substrate-lie where the binary boots against a
    /// pre-0006 database (would otherwise crash only on first PATCH).
    pub async fn probe(&self) -> Result<ProbeReport, ProbeError> {
        let started = std::time::Instant::now();
        let one: (i32,) = sqlx::query_as("SELECT 1").fetch_one(&self.pool).await?;
        if one.0 != 1 {
            return Err(ProbeError::Failed("SELECT 1 returned non-1".to_string()));
        }
        // Slice-5 substrate check: assert migration 0006 columns exist.
        // information_schema is per-search_path so this works under the
        // per-scenario schema rotation used by the acceptance harness.
        let cols: (i64,) = sqlx::query_as(
            "SELECT count(*)::bigint
               FROM information_schema.columns
              WHERE table_name = 'comments'
                AND column_name IN ('updated_at', 'deleted_at', 'deleted_by')",
        )
        .fetch_one(&self.pool)
        .await?;
        if cols.0 < 3 {
            return Err(ProbeError::Failed(format!(
                "comments table missing migration-0006 columns (found {} of 3)",
                cols.0
            )));
        }
        Ok(ProbeReport {
            select_one_ok: true,
            round_trip_ms: started.elapsed().as_millis(),
        })
    }

    /// Has any workspace been provisioned? Used by the bootstrap flow
    /// (US-01 / US-05) to decide whether to mint a bootstrap token.
    pub async fn any_workspace_exists(&self) -> Result<bool, StoreError> {
        let row: (bool,) = sqlx::query_as("SELECT EXISTS (SELECT 1 FROM workspaces)")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Insert a bootstrap-token row. Caller passes the SHA-256 of the raw token.
    pub async fn insert_bootstrap_token(
        &self,
        id: uuid::Uuid,
        token_hash: &[u8],
        expires_at: time::OffsetDateTime,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO bootstrap_tokens (id, token_hash, expires_at) VALUES ($1, $2, $3)",
        )
        .bind(id)
        .bind(token_hash)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Read-only snapshot of the underlying pool's size + idle/in-use
    /// breakdown. Used by the slice-6 background poll task in
    /// `foundry-app::main` to refresh the `db_connections_in_use`
    /// Prometheus gauge every 5 seconds (ADR-012).
    ///
    /// Cheap — sqlx exposes both `size()` and `num_idle()` as
    /// non-blocking atomic loads. Saturating cast to `i32` is safe at
    /// our pool sizes (NFR-PERF-04 caps at 10 connections per replica).
    pub fn pool_stats(&self) -> PoolStats {
        let size = self.pool.size() as i32;
        let idle = self.pool.num_idle() as i32;
        // Guard against the (very rare) race window where num_idle is
        // observed marginally after size — saturate at 0 to keep the
        // gauge a non-negative integer.
        let in_use = (size - idle).max(0);
        PoolStats { in_use, idle, size }
    }

    /// Atomically claim a bootstrap token: mark it consumed if-and-only-if
    /// it has not been consumed and has not expired. Returns the row's
    /// `id` on success; `None` if the token is unknown, already used, or
    /// expired. This is the single-use enforcement point — concurrent
    /// claim attempts race on this UPDATE; one wins, the others see
    /// `None` and surface 410 Gone.
    pub async fn claim_bootstrap_token(
        &self,
        token_hash: &[u8],
        now: time::OffsetDateTime,
    ) -> Result<Option<uuid::Uuid>, StoreError> {
        let row: Option<(uuid::Uuid,)> = sqlx::query_as(
            "UPDATE bootstrap_tokens
                SET used_at = $2
              WHERE token_hash = $1
                AND used_at IS NULL
                AND expires_at > $2
              RETURNING id",
        )
        .bind(token_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.0))
    }

    /// Why a bootstrap token lookup might fail. Drives the explanatory
    /// page rendered for invalid `/bootstrap?token=...` GETs.
    pub async fn bootstrap_token_status(
        &self,
        token_hash: &[u8],
        now: time::OffsetDateTime,
    ) -> Result<BootstrapTokenStatus, StoreError> {
        let row: Option<(Option<time::OffsetDateTime>, time::OffsetDateTime)> = sqlx::query_as(
            "SELECT used_at, expires_at FROM bootstrap_tokens WHERE token_hash = $1",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(match row {
            None => BootstrapTokenStatus::Unknown,
            Some((Some(_), _)) => BootstrapTokenStatus::AlreadyUsed,
            Some((None, expires_at)) if expires_at <= now => BootstrapTokenStatus::Expired,
            Some(_) => BootstrapTokenStatus::Valid,
        })
    }

    /// Create the initial workspace + admin user + default team + default
    /// project + admin membership in a single transaction. Returns the
    /// newly-minted `(workspace_id, user_id)` so the handler can attach
    /// the user_id to the session cookie.
    ///
    /// Caller is responsible for ensuring the bootstrap token was claimed
    /// (single-use guard) before invoking this.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_initial_workspace(
        &self,
        workspace_id: uuid::Uuid,
        workspace_name: &str,
        user_id: uuid::Uuid,
        email_lower: &str,
        email_display: &str,
        display_name: &str,
        password_hash: &str,
        team_id: uuid::Uuid,
        team_name: &str,
        team_slug: &str,
        project_id: uuid::Uuid,
        project_name: &str,
        project_slug: &str,
        project_key_prefix: &str,
    ) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
            .bind(workspace_id)
            .bind(workspace_name)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
                  VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(user_id)
        .bind(email_lower)
        .bind(email_display)
        .bind(display_name)
        .bind(password_hash)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO workspace_memberships (workspace_id, user_id, role)
                  VALUES ($1, $2, 'admin')",
        )
        .bind(workspace_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
            .bind(team_id)
            .bind(workspace_id)
            .bind(team_name)
            .bind(team_slug)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'lead')",
        )
        .bind(team_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
                  VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(project_id)
        .bind(team_id)
        .bind(workspace_id)
        .bind(project_name)
        .bind(project_slug)
        .bind(project_key_prefix)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Count workspaces. Used by the second-workspace-create handler to
    /// short-circuit with 409 Conflict before hitting the unique index.
    pub async fn workspace_count(&self) -> Result<i64, StoreError> {
        let row: (i64,) = sqlx::query_as("SELECT count(*) FROM workspaces")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Find the workspace's id + name (slice-1 has at most one).
    pub async fn first_workspace(&self) -> Result<Option<(uuid::Uuid, String)>, StoreError> {
        let row: Option<(uuid::Uuid, String)> =
            sqlx::query_as("SELECT id, name FROM workspaces LIMIT 1")
                .fetch_optional(&self.pool)
                .await?;
        Ok(row)
    }

    /// Record an invite row. Returns the row id (which the caller signs
    /// into the URL).
    pub async fn insert_invite(
        &self,
        id: uuid::Uuid,
        workspace_id: uuid::Uuid,
        invitee_email: Option<&str>,
        created_by: uuid::Uuid,
        expires_at: time::OffsetDateTime,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO invites (id, workspace_id, invitee_email, created_by, expires_at)
                  VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(invitee_email)
        .bind(created_by)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Look up an invite by id; used by tests to assert expiry windows.
    pub async fn invite_expires_at(
        &self,
        id: uuid::Uuid,
    ) -> Result<Option<time::OffsetDateTime>, StoreError> {
        let row: Option<(time::OffsetDateTime,)> =
            sqlx::query_as("SELECT expires_at FROM invites WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    // ----- US-06 sign-in -------------------------------------------------

    /// Look up a user row by lower-cased email. Returns the bits the
    /// sign-in handler needs (id + PHC-encoded password hash).
    pub async fn find_user_by_email(
        &self,
        email_lower: &str,
    ) -> Result<Option<UserRow>, StoreError> {
        let row: Option<(uuid::Uuid, String)> =
            sqlx::query_as("SELECT id, password_hash FROM users WHERE email_lower = $1")
                .bind(email_lower)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(id, password_hash)| UserRow { id, password_hash }))
    }

    /// Count failed sign-in attempts for `email_lower` since
    /// `window_start`. Drives the NFR-SEC-02 brute-force delay.
    pub async fn count_recent_failed_signin_attempts(
        &self,
        email_lower: &str,
        window_start: time::OffsetDateTime,
    ) -> Result<i64, StoreError> {
        let row: (i64,) = sqlx::query_as(
            "SELECT count(*) FROM signin_attempts
              WHERE email_lower = $1 AND success = FALSE AND attempt_at >= $2",
        )
        .bind(email_lower)
        .bind(window_start)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    /// Record one sign-in attempt outcome.
    pub async fn record_signin_attempt(
        &self,
        email_lower: &str,
        success: bool,
        attempt_at: time::OffsetDateTime,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO signin_attempts (email_lower, attempt_at, success) VALUES ($1, $2, $3)",
        )
        .bind(email_lower)
        .bind(attempt_at)
        .bind(success)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Insert a password-reset token row. The caller passes the
    /// SHA-256 hash; the raw token is never persisted.
    pub async fn insert_reset_token(
        &self,
        id: uuid::Uuid,
        user_id: uuid::Uuid,
        token_hash: &[u8],
        expires_at: time::OffsetDateTime,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO reset_tokens (id, user_id, token_hash, expires_at)
                  VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind(user_id)
        .bind(token_hash)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Does a tower-sessions `session` row exist? Used by US-06's
    /// sign-out scenario to assert server-side invalidation, not just
    /// cookie clearing.
    pub async fn session_row_exists(&self, session_id: &str) -> Result<bool, StoreError> {
        let row: (bool,) = sqlx::query_as("SELECT EXISTS (SELECT 1 FROM session WHERE id = $1)")
            .bind(session_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    // ----- US-07 project create ------------------------------------------

    /// Look up a team by `(workspace_id, slug)`. Returns the team id +
    /// human-readable name; the latter is rendered back into the empty
    /// board view's heading.
    pub async fn find_team_by_slug(
        &self,
        workspace_id: uuid::Uuid,
        slug: &str,
    ) -> Result<Option<TeamRow>, StoreError> {
        let row: Option<(uuid::Uuid, String)> =
            sqlx::query_as("SELECT id, name FROM teams WHERE workspace_id = $1 AND slug = $2")
                .bind(workspace_id)
                .bind(slug)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(id, name)| TeamRow { id, name }))
    }

    /// Is `user_id` a member of `team_id`? Drives the 403 path when a
    /// workspace member tries to create a project in a team they don't
    /// belong to (US-07 scenario 4).
    pub async fn is_team_member(
        &self,
        team_id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> Result<bool, StoreError> {
        let row: (bool,) = sqlx::query_as(
            "SELECT EXISTS (SELECT 1 FROM team_memberships WHERE team_id = $1 AND user_id = $2)",
        )
        .bind(team_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    /// Insert a project row. Caller is responsible for: (a) validating
    /// the key prefix through [`foundry_core::ProjectKey`], (b) computing
    /// the slug, (c) verifying authorisation.
    ///
    /// Surfaces the distinct uniqueness errors (project name within
    /// team vs key prefix within workspace) so the handler can render
    /// the correct inline error.
    pub async fn insert_project(
        &self,
        project_id: uuid::Uuid,
        workspace_id: uuid::Uuid,
        team_id: uuid::Uuid,
        name: &str,
        slug: &str,
        key_prefix: &str,
    ) -> Result<(), ProjectInsertError> {
        let result = sqlx::query(
            "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
                  VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(project_id)
        .bind(team_id)
        .bind(workspace_id)
        .bind(name)
        .bind(slug)
        .bind(key_prefix)
        .execute(&self.pool)
        .await;
        match result {
            Ok(_) => Ok(()),
            Err(sqlx::Error::Database(db_err)) => {
                // PostgreSQL "23505" = unique_violation. Constraint name
                // tells us *which* uniqueness was violated; we mapped
                // both constraints in migration 0001_init.sql.
                if db_err.code().as_deref() == Some("23505") {
                    let constraint = db_err.constraint().unwrap_or("");
                    if constraint.contains("key_prefix") {
                        return Err(ProjectInsertError::DuplicateKey);
                    }
                    if constraint.contains("slug") {
                        return Err(ProjectInsertError::DuplicateName);
                    }
                    // Fallback when the constraint name is generic (older
                    // Postgres versions strip the index name): look at
                    // the message body.
                    let msg = db_err.message().to_ascii_lowercase();
                    if msg.contains("key_prefix") {
                        return Err(ProjectInsertError::DuplicateKey);
                    }
                    if msg.contains("slug") {
                        return Err(ProjectInsertError::DuplicateName);
                    }
                    return Err(ProjectInsertError::Other(StoreError::Sqlx(
                        sqlx::Error::Database(db_err),
                    )));
                }
                Err(ProjectInsertError::Other(StoreError::Sqlx(
                    sqlx::Error::Database(db_err),
                )))
            }
            Err(err) => Err(ProjectInsertError::Other(StoreError::Sqlx(err))),
        }
    }

    /// Lookup project by `(team_id, slug)`. Used by the board view
    /// handler `GET /team/{team}/project/{slug}` so the page heading
    /// reflects the freshly-created project.
    pub async fn find_project_by_slug(
        &self,
        team_id: uuid::Uuid,
        slug: &str,
    ) -> Result<Option<ProjectRow>, StoreError> {
        let row: Option<(uuid::Uuid, String, String)> = sqlx::query_as(
            "SELECT id, name, key_prefix FROM projects WHERE team_id = $1 AND slug = $2",
        )
        .bind(team_id)
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(id, name, key_prefix)| ProjectRow {
            id,
            name,
            key_prefix,
        }))
    }

    // ----- US-08 file issue ----------------------------------------------

    /// Insert one issue, allocating its per-project sequential number in
    /// the same transaction as the outbox `IssueCreated` event.
    ///
    /// Sequential numbering uses
    /// `UPDATE projects SET next_issue_number = next_issue_number + 1
    ///  RETURNING next_issue_number` which takes a row-level lock on the
    /// project row, serialising concurrent allocations on the same project
    /// without bottlenecking other projects. The returned value is the
    /// number assigned to the NEW issue (we allocate "the next number to
    /// give out" as the issue number, then bump it for the following insert).
    ///
    /// The outbox row is inserted in the same transaction so an
    /// `IssueCreated` event is visible to LISTEN/NOTIFY consumers only
    /// once the issue itself is committed. The realtime crate's
    /// publisher hook (slice 2) consumes the row.
    ///
    /// Returns `(issue_id, number)` so the caller can render the
    /// freshly-minted issue key in the response without a re-fetch.
    pub async fn insert_issue_with_outbox(
        &self,
        issue_id: uuid::Uuid,
        workspace_id: uuid::Uuid,
        project_id: uuid::Uuid,
        project_key_prefix: &str,
        author_id: uuid::Uuid,
        title: &str,
    ) -> Result<i32, IssueInsertError> {
        let mut tx = self.pool.begin().await?;

        // Allocate the next per-project number. Row-level lock prevents
        // duplicate numbers under concurrent inserts on the same project.
        // The returned value is the number we hand to the NEW issue —
        // the column's invariant is "the next number to give out", so
        // before the update we have N (and assign N to this issue), and
        // after the update the column holds N+1. We model that by
        // capturing `next_issue_number` BEFORE incrementing:
        let row: Option<(i32,)> = sqlx::query_as(
            "UPDATE projects
                SET next_issue_number = next_issue_number + 1
              WHERE id = $1
              RETURNING next_issue_number - 1",
        )
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?;
        let number = match row {
            Some((n,)) => n,
            None => return Err(IssueInsertError::ProjectNotFound),
        };

        sqlx::query(
            "INSERT INTO issues
                  (id, project_id, workspace_id, number, title, author_id)
              VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(issue_id)
        .bind(project_id)
        .bind(workspace_id)
        .bind(number)
        .bind(title)
        .bind(author_id)
        .execute(&mut *tx)
        .await?;

        // Outbox event. Payload is JSON the LISTEN/NOTIFY consumer
        // (foundry-realtime, slice 2) will decode. Keep keys terse and
        // stable — adding fields is fine, renaming would be a breaking
        // change for any subscribed consumer.
        let payload = serde_json::json!({
            "issue_id": issue_id,
            "project_id": project_id,
            "workspace_id": workspace_id,
            "number": number,
            "key": format!("{project_key_prefix}-{number}"),
            "author_id": author_id,
        });
        sqlx::query("INSERT INTO outbox (event_type, payload) VALUES ('IssueCreated', $1)")
            .bind(payload)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(number)
    }

    /// List issues in a project ordered by `number DESC` (most recent
    /// first — matches the Linear-style "newest at the top" UX).
    pub async fn list_issues_by_project(
        &self,
        project_id: uuid::Uuid,
    ) -> Result<Vec<IssueRow>, StoreError> {
        let rows: Vec<(uuid::Uuid, i32, String, String, String)> = sqlx::query_as(
            "SELECT id, number, title, state, priority
               FROM issues
              WHERE project_id = $1
              ORDER BY number DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, number, title, state, priority)| IssueRow {
                id,
                number,
                title,
                state,
                priority,
            })
            .collect())
    }

    /// Update an issue's `state` and write a matching `IssueUpdated`
    /// outbox row in the same transaction. The trigger
    /// `notify_outbox_event` then fans the event out to all replicas
    /// LISTENing on `issue_events` (slice 2; see
    /// `migrations/0003_outbox_notify.sql`).
    ///
    /// `actor_id` is the user driving the change — surfaces in the
    /// event payload as `author_id` so the realtime layer can decide
    /// whether to suppress echo back to the originator (out of scope
    /// for slice 2).
    pub async fn update_issue_state_with_outbox(
        &self,
        project_key_prefix: &str,
        issue_number: i32,
        new_state: &str,
        actor_id: uuid::Uuid,
    ) -> Result<Option<()>, IssueInsertError> {
        let mut tx = self.pool.begin().await?;

        // Lookup the issue row + project context. Single round trip so
        // the trigger payload contains the project_id without a second
        // query.
        let row: Option<(uuid::Uuid, uuid::Uuid, uuid::Uuid)> = sqlx::query_as(
            "SELECT i.id, i.project_id, i.workspace_id
               FROM issues i
               JOIN projects p ON p.id = i.project_id
              WHERE p.key_prefix = $1 AND i.number = $2",
        )
        .bind(project_key_prefix)
        .bind(issue_number)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((issue_id, project_id, workspace_id)) = row else {
            return Ok(None);
        };

        sqlx::query("UPDATE issues SET state = $1, updated_at = now() WHERE id = $2")
            .bind(new_state)
            .bind(issue_id)
            .execute(&mut *tx)
            .await?;

        let payload = serde_json::json!({
            "issue_id": issue_id,
            "project_id": project_id,
            "workspace_id": workspace_id,
            "number": issue_number,
            "key": format!("{project_key_prefix}-{issue_number}"),
            "state": new_state,
            "author_id": actor_id,
        });
        sqlx::query("INSERT INTO outbox (event_type, payload) VALUES ('IssueUpdated', $1)")
            .bind(payload)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(Some(()))
    }

    /// Count issues in a project. Acceptance assertion helper for the
    /// "no issue is created" path.
    pub async fn count_issues_in_project(&self, project_id: uuid::Uuid) -> Result<i64, StoreError> {
        let row: (i64,) = sqlx::query_as("SELECT count(*) FROM issues WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Count projects in this workspace with the given name across all
    /// teams. Used only by the acceptance suite (assertions about
    /// "no second project is created"); the create handler relies on
    /// the unique INSERT for uniqueness, not a pre-check.
    pub async fn count_projects_by_name(
        &self,
        workspace_id: uuid::Uuid,
        name: &str,
    ) -> Result<i64, StoreError> {
        let row: (i64,) =
            sqlx::query_as("SELECT count(*) FROM projects WHERE workspace_id = $1 AND name = $2")
                .bind(workspace_id)
                .bind(name)
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0)
    }

    // ----- US-10 comments ------------------------------------------------

    /// Resolve `(project, issue)` from a `(team_id, project_slug,
    /// issue_number)` triple. Returns the project + issue ids and the
    /// project's key_prefix so the comment handler can render the issue
    /// key without a second round trip.
    pub async fn find_issue_by_team_project_number(
        &self,
        team_id: uuid::Uuid,
        project_slug: &str,
        issue_number: i32,
    ) -> Result<Option<IssueLookupRow>, StoreError> {
        let row: Option<(uuid::Uuid, String, uuid::Uuid, uuid::Uuid)> = sqlx::query_as(
            "SELECT p.id, p.key_prefix, i.id, i.workspace_id
               FROM projects p
               JOIN issues   i ON i.project_id = p.id
              WHERE p.team_id = $1 AND p.slug = $2 AND i.number = $3",
        )
        .bind(team_id)
        .bind(project_slug)
        .bind(issue_number)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(
            |(project_id, key_prefix, issue_id, workspace_id)| IssueLookupRow {
                project_id,
                project_key_prefix: key_prefix,
                issue_id,
                workspace_id,
            },
        ))
    }

    /// Insert a comment row and a `CommentAdded` outbox row in the same
    /// transaction. The Postgres trigger `notify_outbox_event` then
    /// fans the event out to every replica `LISTEN`ing on
    /// `issue_events` — no separate publisher hook needed.
    ///
    /// `author_email` rides in the outbox payload so subscribers can
    /// render the comment author without a JOIN against `users`.
    /// `body_html` is the pre-rendered (sanitized) HTML; `body_markdown`
    /// is the original input, persisted alongside so we can re-render
    /// the table if the sanitizer ever changes its allowlist.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_comment_with_outbox(
        &self,
        comment_id: uuid::Uuid,
        workspace_id: uuid::Uuid,
        project_id: uuid::Uuid,
        project_key_prefix: &str,
        issue_id: uuid::Uuid,
        issue_number: i32,
        author_id: uuid::Uuid,
        author_email: &str,
        body_markdown: &str,
        body_html: &str,
    ) -> Result<(), CommentInsertError> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO comments
                  (id, workspace_id, issue_id, author_id, body_markdown, body_html)
              VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(comment_id)
        .bind(workspace_id)
        .bind(issue_id)
        .bind(author_id)
        .bind(body_markdown)
        .bind(body_html)
        .execute(&mut *tx)
        .await?;

        // Outbox payload. Keys mirror IssueCreated/IssueUpdated where
        // possible (issue_id, project_id, workspace_id, number, key,
        // author_id) and add the comment-specific fields. Adding fields
        // is forward-compatible — the LISTEN consumer's EventPayload
        // declares them `#[serde(default)]`.
        let payload = serde_json::json!({
            "issue_id": issue_id,
            "project_id": project_id,
            "workspace_id": workspace_id,
            "number": issue_number,
            "key": format!("{project_key_prefix}-{issue_number}"),
            "author_id": author_id,
            "comment_id": comment_id,
            "author_email": author_email,
        });
        sqlx::query("INSERT INTO outbox (event_type, payload) VALUES ('CommentAdded', $1)")
            .bind(payload)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    /// List comments on an issue ordered by `created_at ASC` (oldest
    /// first — matches the conventional thread reading order).
    /// `author_email` is joined from the `users` table; if the author
    /// has been deleted (cascade does NOT cascade on users), we surface
    /// `<deleted>` as a sentinel.
    ///
    /// Slice-5 (ADR-007 soft-delete invariant): tombstoned rows
    /// (`deleted_at IS NOT NULL`) are filtered out of the public list
    /// view. The `idx_comments_issue_live` partial index covers this
    /// query shape. The `edited` flag on `CommentRow` is derived from
    /// `updated_at IS NOT NULL` so the renderer can paint the "edited"
    /// indicator (Q4 = A) without a second query.
    pub async fn list_comments_for_issue(
        &self,
        issue_id: uuid::Uuid,
    ) -> Result<Vec<CommentRow>, StoreError> {
        let rows: Vec<(
            uuid::Uuid,
            uuid::Uuid,
            String,
            String,
            time::OffsetDateTime,
            Option<time::OffsetDateTime>,
        )> = sqlx::query_as(
            "SELECT c.id, c.author_id, COALESCE(u.email_display, '<deleted>'), c.body_html, c.created_at, c.updated_at
               FROM comments c
               LEFT JOIN users u ON u.id = c.author_id
              WHERE c.issue_id = $1 AND c.deleted_at IS NULL
              ORDER BY c.created_at ASC, c.id ASC",
        )
        .bind(issue_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(id, author_id, author_email, body_html, created_at, updated_at)| CommentRow {
                    id,
                    author_id,
                    author_email,
                    body_html,
                    created_at,
                    edited: updated_at.is_some(),
                },
            )
            .collect())
    }

    // ----- US-10 (slice 5) edit + soft-delete -----------------------------

    /// Look up a comment by id within a workspace, including soft-deleted
    /// rows. Returns `None` when no row exists (the handler maps this to
    /// 404), or `Some(CommentLookupRow)` carrying `deleted: bool` so the
    /// caller can distinguish tombstoned (`deleted == true` → 410 Gone)
    /// from live (`deleted == false` → proceed with authz check).
    ///
    /// Per ADR-008 the 404-vs-410 dispatch lives in the handler; this
    /// method just surfaces the bits required for the decision. Scoped
    /// by `workspace_id` so a comment id from another workspace presents
    /// as 404, not as a cross-workspace leak.
    pub async fn find_comment_by_id(
        &self,
        workspace_id: uuid::Uuid,
        comment_id: uuid::Uuid,
    ) -> Result<Option<CommentLookupRow>, StoreError> {
        let row: Option<(
            uuid::Uuid,
            uuid::Uuid,
            uuid::Uuid,
            String,
            Option<time::OffsetDateTime>,
        )> = sqlx::query_as(
            "SELECT c.id, c.issue_id, c.author_id, c.body_markdown, c.deleted_at
               FROM comments c
              WHERE c.id = $1 AND c.workspace_id = $2",
        )
        .bind(comment_id)
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(
            |(id, issue_id, author_id, body_markdown, deleted_at)| CommentLookupRow {
                id,
                issue_id,
                author_id,
                body_markdown,
                deleted: deleted_at.is_some(),
            },
        ))
    }

    /// Is `user_id` a workspace admin in `workspace_id`? Used by the
    /// DELETE-comment handler to authorize the admin-moderation path
    /// (author OR admin can delete; only author can edit). Per ADR-006
    /// edit is author-only; per ADR-007 delete extends to admin.
    pub async fn is_workspace_admin(
        &self,
        workspace_id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> Result<bool, StoreError> {
        let row: (bool,) = sqlx::query_as(
            "SELECT EXISTS (SELECT 1 FROM workspace_memberships
                             WHERE workspace_id = $1 AND user_id = $2 AND role = 'admin')",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    /// Update a comment's body + write a `CommentEdited` outbox row in
    /// one transaction. `now` lands in `updated_at` (drives the "edited"
    /// indicator per Q4 = A). The Postgres trigger `notify_outbox_event`
    /// fans the event out to every LISTEN-ing replica — no per-handler
    /// publisher hook needed.
    ///
    /// Returns `Ok(false)` when no live row matches (the handler should
    /// re-fetch via `find_comment_by_id` to distinguish 404 vs 410). The
    /// edit path never touches tombstoned rows; the handler's pre-check
    /// short-circuits to 410 before this method is called.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_comment_with_outbox(
        &self,
        workspace_id: uuid::Uuid,
        comment_id: uuid::Uuid,
        new_markdown: &str,
        new_html: &str,
        actor_user_id: uuid::Uuid,
        author_email: &str,
    ) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin().await?;

        // Update only LIVE rows. The handler is responsible for the
        // 404-vs-410 distinction before getting here; this WHERE clause
        // is a defense-in-depth guard against a concurrent delete racing
        // an edit in flight (would surface as "no rows updated" → false).
        let updated: Option<(uuid::Uuid, uuid::Uuid, i32, String)> = sqlx::query_as(
            "UPDATE comments
                SET body_markdown = $3, body_html = $4, updated_at = now()
              WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL
              RETURNING issue_id,
                        (SELECT project_id FROM issues WHERE id = comments.issue_id),
                        (SELECT number FROM issues WHERE id = comments.issue_id),
                        (SELECT key_prefix FROM projects
                          WHERE id = (SELECT project_id FROM issues
                                       WHERE id = comments.issue_id))",
        )
        .bind(comment_id)
        .bind(workspace_id)
        .bind(new_markdown)
        .bind(new_html)
        .fetch_optional(&mut *tx)
        .await?;

        let Some((issue_id, project_id, number, key_prefix)) = updated else {
            tx.rollback().await?;
            return Ok(false);
        };

        let payload = serde_json::json!({
            "issue_id": issue_id,
            "project_id": project_id,
            "workspace_id": workspace_id,
            "number": number,
            "key": format!("{key_prefix}-{number}"),
            "author_id": actor_user_id,
            "comment_id": comment_id,
            "author_email": author_email,
        });
        sqlx::query("INSERT INTO outbox (event_type, payload) VALUES ('CommentEdited', $1)")
            .bind(payload)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(true)
    }

    /// Soft-delete a comment + write a `CommentDeleted` outbox row in
    /// one transaction. Sets `deleted_at = now()` and `deleted_by =
    /// actor`. The outbox payload sets `deleted: true` per ADR-008 so
    /// receivers can detect tombstones without parsing `event_type`.
    ///
    /// Returns `Ok(false)` when no live row matches — the handler should
    /// re-fetch to disambiguate (a re-DELETE on an already-tombstoned
    /// row is a 410, not a 200). The handler's pre-check short-circuits
    /// to 410 before calling this method on tombstoned rows, so the
    /// returned `false` mostly catches the race-condition / cross-
    /// workspace cases.
    pub async fn soft_delete_comment_with_outbox(
        &self,
        workspace_id: uuid::Uuid,
        comment_id: uuid::Uuid,
        actor_user_id: uuid::Uuid,
    ) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin().await?;

        let updated: Option<(uuid::Uuid, uuid::Uuid, i32, String)> = sqlx::query_as(
            "UPDATE comments
                SET deleted_at = now(), deleted_by = $3
              WHERE id = $1 AND workspace_id = $2 AND deleted_at IS NULL
              RETURNING issue_id,
                        (SELECT project_id FROM issues WHERE id = comments.issue_id),
                        (SELECT number FROM issues WHERE id = comments.issue_id),
                        (SELECT key_prefix FROM projects
                          WHERE id = (SELECT project_id FROM issues
                                       WHERE id = comments.issue_id))",
        )
        .bind(comment_id)
        .bind(workspace_id)
        .bind(actor_user_id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some((issue_id, project_id, number, key_prefix)) = updated else {
            tx.rollback().await?;
            return Ok(false);
        };

        let payload = serde_json::json!({
            "issue_id": issue_id,
            "project_id": project_id,
            "workspace_id": workspace_id,
            "number": number,
            "key": format!("{key_prefix}-{number}"),
            "author_id": actor_user_id,
            "comment_id": comment_id,
            "deleted": true,
        });
        sqlx::query("INSERT INTO outbox (event_type, payload) VALUES ('CommentDeleted', $1)")
            .bind(payload)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(true)
    }

    /// Count comments on an issue. Acceptance assertion helper for the
    /// "no comment is recorded" path used by the @error scenarios.
    pub async fn count_comments_for_issue(&self, issue_id: uuid::Uuid) -> Result<i64, StoreError> {
        let row: (i64,) = sqlx::query_as("SELECT count(*) FROM comments WHERE issue_id = $1")
            .bind(issue_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Look up a user's display email by id. Used by the comment-create
    /// handler so the actor's email rides through the outbox payload
    /// (wave-decisions.md — no JOIN at fan-out time).
    pub async fn find_user_email_by_id(
        &self,
        user_id: uuid::Uuid,
    ) -> Result<Option<String>, StoreError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT email_display FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    // ----- US-10 (slice 7) tombstone GC + admin-undelete -----------------

    /// Hard-delete comment tombstones older than `older_than`, in
    /// batches of `batch` rows per DELETE, up to `cap` rows per
    /// invocation. Returns the total rows removed.
    ///
    /// Per ADR-015 (slice 7):
    ///   - Acquires [`TOMBSTONE_GC_LOCK_ID`] via `pg_try_advisory_lock`
    ///     (non-blocking). When contended, returns `Ok(0)` without
    ///     touching any rows — sibling replicas exit gracefully.
    ///   - Loops `DELETE ... WHERE id IN (SELECT id ... LIMIT batch)`
    ///     until rows_affected < batch OR cumulative >= cap.
    ///   - Releases the advisory lock in ALL paths (including error)
    ///     so a transient sqlx failure mid-sweep doesn't pin the lock.
    ///
    /// `older_than` becomes a `now() - $1::interval` SQL fragment so
    /// the cutoff is computed inside Postgres against the same clock
    /// the soft-delete handler used (no client-side now() skew).
    pub async fn gc_tombstoned_comments(
        &self,
        older_than: Duration,
        batch: usize,
        cap: usize,
    ) -> Result<u64, StoreError> {
        // Derive the lock id from search_path so per-scenario PG
        // schemas inside the shared testcontainers Postgres do NOT
        // serialise on each other. Same pattern as slice-1's
        // `scoped_migration_lock_id`: production binaries (search_path
        // = "public") return the canonical TOMBSTONE_GC_LOCK_ID so
        // operational triage on `pg_locks` still works; acceptance
        // scenarios with per-schema search_path get a derived literal.
        let lock_id = scoped_tombstone_gc_lock_id(&self.pool)
            .await
            .unwrap_or(TOMBSTONE_GC_LOCK_ID);
        let mut conn = self.pool.acquire().await?;
        let lock: (bool,) = sqlx::query_as("SELECT pg_try_advisory_lock($1)")
            .bind(lock_id)
            .fetch_one(&mut *conn)
            .await?;
        if !lock.0 {
            // Contended — another replica is mid-sweep. Graceful no-op.
            return Ok(0);
        }
        let older_than_seconds = older_than.as_secs() as i64;
        let result: Result<u64, sqlx::Error> = async {
            let mut total: u64 = 0;
            loop {
                if total >= cap as u64 {
                    break;
                }
                // Remaining capacity for this iteration; never exceed
                // `batch` per round-trip. Saturating-sub keeps the
                // arithmetic safe at the boundary.
                let remaining = (cap as u64).saturating_sub(total);
                let this_batch = remaining.min(batch as u64) as i64;
                let outcome = sqlx::query(
                    "DELETE FROM comments
                      WHERE id IN (
                          SELECT id FROM comments
                           WHERE deleted_at IS NOT NULL
                             AND deleted_at < now() - ($1 || ' seconds')::interval
                           LIMIT $2
                      )",
                )
                .bind(older_than_seconds.to_string())
                .bind(this_batch)
                .execute(&mut *conn)
                .await?;
                let affected = outcome.rows_affected();
                total += affected;
                if affected < this_batch as u64 {
                    // Drained — fewer rows matched than we asked for.
                    break;
                }
            }
            Ok(total)
        }
        .await;
        // ALWAYS release the lock, even on error.
        let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(lock_id)
            .execute(&mut *conn)
            .await;
        Ok(result?)
    }

    /// Count comment tombstones older than `older_than`. Feeds the
    /// `comments_tombstones_pending` gauge (slice 7 / ADR-016). Pure
    /// read — no lock, no mutation.
    pub async fn count_pending_tombstones(&self, older_than: Duration) -> Result<u64, StoreError> {
        let older_than_seconds = older_than.as_secs() as i64;
        let row: (i64,) = sqlx::query_as(
            "SELECT count(*)::bigint
               FROM comments
              WHERE deleted_at IS NOT NULL
                AND deleted_at < now() - ($1 || ' seconds')::interval",
        )
        .bind(older_than_seconds.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0.max(0) as u64)
    }

    /// Restore a soft-deleted comment by clearing `deleted_at` +
    /// `deleted_by`. Returns the number of rows affected:
    ///   - `1` — the comment existed AND was tombstoned (now restored).
    ///   - `0` — the comment doesn't exist OR is already live.
    ///
    /// Idempotent: re-invoking on an already-restored row is a no-op
    /// zero-return, not an error. Per ADR-016 / D6 = A the CLI dispatch
    /// maps `0` to exit code 4 ("not restorable") and `1` to exit code
    /// 0 ("restored"). NO outbox event is emitted — operator-driven
    /// restoration is an out-of-band moderation reversal, not a
    /// user-visible state change worth fanning out (the issue page's
    /// next render reflects the restored row naturally).
    pub async fn undelete_comment(&self, comment_id: uuid::Uuid) -> Result<u64, StoreError> {
        let outcome = sqlx::query(
            "UPDATE comments
                SET deleted_at = NULL, deleted_by = NULL
              WHERE id = $1 AND deleted_at IS NOT NULL",
        )
        .bind(comment_id)
        .execute(&self.pool)
        .await?;
        Ok(outcome.rows_affected())
    }
}

/// Minimal team projection.
#[derive(Debug, Clone)]
pub struct TeamRow {
    pub id: uuid::Uuid,
    pub name: String,
}

/// Minimal project projection used by the board view handler.
#[derive(Debug, Clone)]
pub struct ProjectRow {
    pub id: uuid::Uuid,
    pub name: String,
    pub key_prefix: String,
}

/// Minimal issue projection used by the board view + acceptance
/// assertions. `state` and `priority` are kept as `String` for slice 1
/// (they are CHECK-constrained at the schema level); a typed enum is a
/// slice-2 hardening item.
#[derive(Debug, Clone)]
pub struct IssueRow {
    pub id: uuid::Uuid,
    pub number: i32,
    pub title: String,
    pub state: String,
    pub priority: String,
}

/// Errors specific to issue insert.
#[derive(Debug, Error)]
pub enum IssueInsertError {
    #[error("project not found")]
    ProjectNotFound,
    #[error(transparent)]
    Store(#[from] sqlx::Error),
}

/// Errors specific to project insert. Splits the uniqueness violations
/// so the handler can render the correct user-facing inline error.
#[derive(Debug, Error)]
pub enum ProjectInsertError {
    #[error("project key already exists in workspace")]
    DuplicateKey,
    #[error("project name already exists in team")]
    DuplicateName,
    #[error(transparent)]
    Other(#[from] StoreError),
}

/// Minimal user projection used by the sign-in handler.
#[derive(Debug, Clone)]
pub struct UserRow {
    pub id: uuid::Uuid,
    pub password_hash: String,
}

/// What state a bootstrap-token lookup found. Drives the explanatory
/// 410 page distinguishing "already used" from "expired".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapTokenStatus {
    Valid,
    AlreadyUsed,
    Expired,
    Unknown,
}

/// Run sqlx migrations against an externally-built pool. Used by the
/// acceptance harness which builds a pool with a per-scenario search_path.
pub async fn run_migrations(pool: &PgPool) -> Result<(), StoreError> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

/// Slice 7 — derive the tombstone-GC advisory-lock id from the active
/// `search_path`, in the same shape as [`scoped_migration_lock_id`].
/// The acceptance suite shares ONE Postgres container across per-
/// scenario schemas; without scoping, two concurrent slice-7 scenarios
/// would serialise on the global lock and observe each other's "lock
/// contended" no-ops, breaking the scenario-isolation invariant the
/// slice-1 harness establishes. Production binaries use `public` and
/// return the canonical [`TOMBSTONE_GC_LOCK_ID`].
///
/// Returns the canonical id on any error (cannot read search_path,
/// transient sqlx failure) — preserves strict-serialisation semantics
/// when the scope can't be determined honestly.
pub async fn scoped_tombstone_gc_lock_id(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let row: (String,) = sqlx::query_as("SHOW search_path").fetch_one(pool).await?;
    let normalised = row.0.trim();
    if normalised.is_empty()
        || normalised == "\"$user\", public"
        || normalised == "public"
        || normalised == "\"$user\""
    {
        return Ok(TOMBSTONE_GC_LOCK_ID);
    }
    // FNV-1a hash, truncated to i64. Same construction as
    // `scoped_migration_lock_id` (different seed makes the resulting
    // lock id space disjoint per-schema between the two
    // production-meaningful locks). Identical inputs map to identical
    // outputs across replicas — that's the production-meaningful
    // invariant for the advisory-lock pattern.
    let mut hash: u64 = 0x0123_4567_89AB_CDEF; // distinct seed from migration variant
    for b in normalised.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    // XOR with TOMBSTONE_GC_LOCK_ID so the canonical literal is
    // preserved as a "salt" — distinguishes slice-7 lock space from
    // slice-1 migration lock space even at the unlikely event of
    // FNV-1a hash collisions across the two families.
    Ok((hash as i64) ^ TOMBSTONE_GC_LOCK_ID)
}

/// Derive the advisory-lock id from the active `search_path` so the
/// US-04 acceptance scenarios — which use per-scenario Postgres schemas
/// inside ONE shared testcontainers container — do not serialise on
/// each other. The production binary uses `public` (or no override),
/// for which this returns the canonical [`MIGRATION_LOCK_ID`] so the
/// runbook-documented value is preserved.
///
/// Failure modes (cannot read search_path, etc.) fall back to the
/// canonical id at the caller — preserving strict serialisation if the
/// scope query fails.
async fn scoped_migration_lock_id(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let row: (String,) = sqlx::query_as("SHOW search_path").fetch_one(pool).await?;
    let raw = row.0;
    // Postgres returns search_path like `"$user", public` or
    // `test_s17_ab12` depending on `search_path` overrides on the
    // connection. For the production binary, this is `"$user", public`
    // — we return the canonical id to preserve runbook semantics.
    // For the acceptance harness, the pool's connect_options pins
    // `search_path=<schema>` so this is the schema name.
    let normalised = raw.trim();
    if normalised.is_empty()
        || normalised == "\"$user\", public"
        || normalised == "public"
        || normalised == "\"$user\""
    {
        return Ok(MIGRATION_LOCK_ID);
    }
    // FNV-1a hash, truncated to i64. Identical inputs map to identical
    // outputs across replicas — that's the production-meaningful
    // invariant for the advisory-lock pattern.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in normalised.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    Ok(hash as i64)
}

/// Report returned by [`run_migrations_from_dir`] so US-04 acceptance
/// scenarios can observe per-replica migration outcomes.
///
/// `applied` holds the version numbers this invocation actually executed
/// (took the advisory lock, applied SQL, recorded a row in
/// `_sqlx_migrations`). `already_applied` holds the version numbers this
/// invocation observed as already applied (no SQL run; row already
/// present). Together they let the test "exactly one replica reports
/// having applied schema update '0099' and the other reports
/// already-applied" be expressed against an observable.
///
/// Production replicas use [`run_migrations`] (compile-time `migrate!`)
/// and never see this type. The runtime variant is the test path.
#[derive(Debug, Clone, Default)]
pub struct MigrationReport {
    pub applied: Vec<i64>,
    pub already_applied: Vec<i64>,
}

/// Runtime sibling of [`run_migrations`]: load migrations from a
/// directory at runtime (instead of compile-time `migrate!` macro) and
/// run them under the SAME `pg_advisory_lock(MIGRATION_LOCK_ID)` guard
/// that the production path uses. Used by the US-04 acceptance suite
/// to stage per-scenario test migrations into a `tempfile::TempDir`
/// without touching production `crates/foundry-store/migrations/`.
///
/// Returns a [`MigrationReport`] enumerating which migrations were
/// newly applied vs already-applied by this invocation.
///
/// Convenience wrapper that delegates to
/// [`run_migrations_from_dir_with_delay`] with `delay_ms = 0`.
pub async fn run_migrations_from_dir(
    pool: &PgPool,
    dir: &std::path::Path,
) -> Result<MigrationReport, StoreError> {
    run_migrations_from_dir_with_delay(pool, dir, 0).await
}

/// As [`run_migrations_from_dir`] but with an explicit per-call
/// `delay_ms` slept AFTER acquiring the advisory lock and BEFORE
/// snapshotting `_sqlx_migrations`. This is the slow-migration seam
/// the US-04 lock-race scenario uses to keep the winner holding the
/// lock long enough that the loser's blocking time is observable.
///
/// Passing this per-call (instead of a process-global atomic) keeps
/// parallel scenarios isolated — each AppState carries its own value.
pub async fn run_migrations_from_dir_with_delay(
    pool: &PgPool,
    dir: &std::path::Path,
    delay_ms: u64,
) -> Result<MigrationReport, StoreError> {
    use sqlx::migrate::Migrator;

    // The advisory lock id is derived from the current Postgres
    // `search_path` so per-scenario schemas inside the shared
    // testcontainers container do NOT serialise on each other (the
    // production-meaningful invariant is "concurrent replicas against
    // the SAME database serialise", which means SAME schema in the
    // test setup). The base prod lock id is preserved when search_path
    // is `public` (or unset) so the production binary keeps using
    // the canonical id.
    let lock_id = scoped_migration_lock_id(pool)
        .await
        .unwrap_or(MIGRATION_LOCK_ID);

    let mut conn = pool.acquire().await?;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(lock_id)
        .execute(&mut *conn)
        .await?;

    // Pre-snapshot of `_sqlx_migrations` AFTER acquiring the advisory
    // lock. This is the linchpin of per-invocation accounting: the
    // loser replica sees the winner's applied rows here and correctly
    // classifies them as already-applied. Snapshotting BEFORE the
    // lock would race — both replicas would see empty pre-state and
    // both would falsely claim to have applied the migration.
    //
    // The table may not yet exist on a virgin pool — the very first
    // `Migrator::run` call creates it. We tolerate that by treating a
    // missing table as "no versions applied yet".
    let pre_versions: Vec<i64> =
        match sqlx::query_as::<_, (i64,)>("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&mut *conn)
            .await
        {
            Ok(rows) => rows.into_iter().map(|(v,)| v).collect(),
            // Table not yet present (virgin DB) → no priors.
            Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("42P01") => {
                Vec::new()
            }
            Err(e) => {
                // Release the lock before propagating.
                let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
                    .bind(lock_id)
                    .execute(&mut *conn)
                    .await;
                return Err(StoreError::Sqlx(e));
            }
        };

    // Slow-migration seam: if delay_ms > 0 AND there is work to do
    // (i.e., the migrations dir contains versions not yet in
    // `_sqlx_migrations`), sleep before running the migrator. The
    // race-winner has work; the loser has none (the winner already
    // applied 0099). This way only ONE caller pays the delay even
    // though both share the same per-call delay_ms value — exactly
    // the production semantic the slow-lock-race scenario models.
    let pre_set_for_delay: std::collections::HashSet<i64> = pre_versions.iter().copied().collect();
    // `set_locking(false)` disables sqlx's internal advisory lock
    // (which uses a GLOBAL key across the whole Postgres instance and
    // would re-serialise every parallel test scenario). We already
    // hold our scoped advisory lock above; sqlx's lock would be a
    // double-hold against a shared key, which is what was hammering
    // the slice-1+2 timing-sensitive scenarios under load.
    let mut migrator_raw = Migrator::new(dir).await?;
    let migrator = migrator_raw.set_locking(false);
    let has_work = migrator
        .iter()
        .any(|m| !pre_set_for_delay.contains(&m.version));
    if delay_ms > 0 && has_work {
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
    }

    let migrator_result = migrator.run(&mut *conn).await;

    // Post-snapshot taken WHILE we still hold the lock (so another
    // concurrent boot can't race a new row in between).
    let post_versions: Vec<i64> = match migrator_result.as_ref() {
        Ok(_) => {
            sqlx::query_as::<_, (i64,)>("SELECT version FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&mut *conn)
                .await?
                .into_iter()
                .map(|(v,)| v)
                .collect()
        }
        // On failure we don't trust the snapshot; return empty.
        Err(_) => Vec::new(),
    };

    // Always release the advisory lock, even on migration failure.
    let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(lock_id)
        .execute(&mut *conn)
        .await;

    migrator_result?;

    let pre_set: std::collections::HashSet<i64> = pre_versions.iter().copied().collect();
    let mut applied: Vec<i64> = Vec::new();
    let mut already_applied: Vec<i64> = Vec::new();
    for v in &post_versions {
        if pre_set.contains(v) {
            already_applied.push(*v);
        } else {
            applied.push(*v);
        }
    }

    Ok(MigrationReport {
        applied,
        already_applied,
    })
}

/// Joined lookup of an issue by `(team_id, project_slug, issue_number)`.
/// Carries the project key prefix so the comment handler can build the
/// issue key in the outbox payload without a second query.
#[derive(Debug, Clone)]
pub struct IssueLookupRow {
    pub project_id: uuid::Uuid,
    pub project_key_prefix: String,
    pub issue_id: uuid::Uuid,
    pub workspace_id: uuid::Uuid,
}

/// Minimal comment projection used by the issue-detail page renderer
/// and by the acceptance suite's HTML structural assertions.
///
/// `author_email` is the display form (`email_display`) — the renderer
/// uses it as the `data-author=` attribute on each comment card so the
/// acceptance scraper can target individual comments by author.
/// `author_id` lets the renderer conditionally emit the Edit/Delete
/// affordances when `author_id == actor.user_id` (server-side gating per
/// ADR-006). `edited` derives from `updated_at IS NOT NULL` and drives
/// the "edited" indicator (Q4 = A).
#[derive(Debug, Clone)]
pub struct CommentRow {
    pub id: uuid::Uuid,
    pub author_id: uuid::Uuid,
    pub author_email: String,
    pub body_html: String,
    pub created_at: time::OffsetDateTime,
    pub edited: bool,
}

/// Slice-5 lookup projection for `find_comment_by_id`. Carries the
/// `deleted` flag (derived from `deleted_at IS NOT NULL`) so the handler
/// can dispatch 404-vs-410-vs-403 per ADR-008. `body_markdown` is the
/// raw source the edit-form GET handler returns in the textarea (Q5 = A
/// inline-replace requires the original characters the author typed,
/// not the rendered HTML).
#[derive(Debug, Clone)]
pub struct CommentLookupRow {
    pub id: uuid::Uuid,
    pub issue_id: uuid::Uuid,
    pub author_id: uuid::Uuid,
    pub body_markdown: String,
    pub deleted: bool,
}

/// Errors specific to comment insert.
#[derive(Debug, Error)]
pub enum CommentInsertError {
    #[error("issue not found")]
    IssueNotFound,
    #[error(transparent)]
    Store(#[from] sqlx::Error),
}
