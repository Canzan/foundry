//! US-02 multi-replica harness — Option A driving adapter.
//!
//! Per `distill/driver.md` §2b + `wave-decisions.md` §US-02:
//!
//! Spawns N foundry-app instances (TestApp) sharing ONE per-scenario
//! Postgres schema, fronted by the round-robin proxy. The shared
//! schema is the linchpin: it lets sessions issued by replica A
//! validate on replica B because the `session` table lives in the
//! database, not in any replica's memory.
//!
//! Daemon-pressure constraint: NO new Postgres containers per replica.
//! The 3 replicas share the same per-scenario schema in the same
//! shared Postgres container that slice 1 stood up. Per-replica
//! pg_listener tasks each open their own listen connection — that is
//! a connection budget concern (3 listeners + 3 pools, each
//! max_connections = 4 from `fresh_schema_pool_with_url`), not a
//! container concern. The Postgres default max_connections of 100
//! easily handles the maths.
//!
//! Per-replica state separation:
//! - One broadcast channel per replica (so the "fan-out via different
//!   replica than Mei's subscription" scenario works — events
//!   produced on replica A's broadcast must reach replica B's SSE
//!   stream only through the pg_listener → notify path).
//! - One pg_listener task per replica (each LISTENs on `issue_events`
//!   over its OWN connection; postgres fans the NOTIFY to every
//!   listener).
//! - SAME `session_secret` everywhere (cookies sign uniformly).
//! - SAME shared MockClock + FakeEmailSender (Arc-cloned) so all
//!   replicas observe consistent time / outbound email.
//! - SAME `db_unreachable: Arc<AtomicBool>` across all replicas (the
//!   "DB unreachable" scenario flips one flag and every replica's
//!   /readyz observes it within one /readyz poll).

use crate::support::file_upload_env;
use crate::support::harness::{
    ensure_postgres, fresh_schema_pool_no_migrations, fresh_schema_pool_with_url, InProcHarness,
};
use crate::support::heartbeat_env;
use crate::support::round_robin_proxy::{spawn_round_robin_proxy, ProxyHandle};
use crate::support::test_migration::TestMigrationsDir;
use foundry_app::clock::MockClock;
use foundry_app::email::FakeEmailSender;
use foundry_app::test_support::{boot_test_migrations, spawn_app_with_listener, TestApp};
use foundry_app::{AppState, DEFAULT_FILE_UPLOAD_MAX_MB, DEFAULT_SSE_HEARTBEAT_MS};
use foundry_store::{MigrationReport, Store};
use secrecy::SecretString;
use sqlx::PgPool;
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// Captured handle to a per-scenario schema that's already been
/// provisioned by an earlier step (typically the slice-1 `ensure_harness`
/// in a Background block).
struct SharedSchema {
    schema: String,
    pool: PgPool,
    listen_url: String,
    /// The address of the existing replica, if the caller wants the
    /// multi-replica harness to *reuse* it as one of its N replicas
    /// instead of double-counting. Currently informational; the
    /// inner_spawn path simply runs N fresh `spawn_app_with_listener`
    /// calls (the existing single-replica is kept alive separately on
    /// `world.harness` for any inherited steps that reach into it).
    #[allow(dead_code)]
    existing_addr: Option<SocketAddr>,
}

/// The full N-replica harness for a single US-02 scenario.
///
/// Holds the proxy in front and the N replicas behind. `replicas[i]`
/// is the TestApp at slot i; `proxy.upstream_addrs()[i]` is the addr
/// the proxy will route to for slot i (`None` when failed).
pub struct MultiReplicaHarness {
    /// The N replicas in spawn order.
    pub replicas: Vec<TestApp>,
    /// Per-replica references to the shared `db_unreachable` flag.
    /// Conceptually the SAME `Arc<AtomicBool>` (the harness shares one
    /// across all replicas); this is kept as a flat vec so tests that
    /// want to verify "every replica observes the flag" can iterate.
    pub db_unreachable_flags: Vec<Arc<AtomicBool>>,
    /// Shared per-scenario schema. Every replica's pool runs with
    /// `search_path` pinned to this name.
    pub schema: String,
    /// The round-robin proxy in front of all replicas.
    pub proxy: ProxyHandle,
    /// Shared fake clock — Arc'd so every replica observes the same now.
    pub fake_clock: Arc<MockClock>,
    /// Shared fake email — Arc'd so every replica's send() lands in
    /// the same inbox.
    pub fake_email: Arc<FakeEmailSender>,
    /// The pool every replica's Store shares. Tests use this for
    /// direct SQL seeding (the same path slice-1 scenarios use through
    /// `harness.app.state.store.pool()`).
    pub shared_pool: PgPool,
    /// The listen URL the per-replica pg_listener tasks were spawned
    /// against (pinned to the per-scenario schema's search_path).
    /// Stored so tests can spawn additional listeners if needed.
    #[allow(dead_code)]
    pub listen_url: String,
    /// US-04 only: the staged migrations dir, kept alive on the harness
    /// so the underlying `tempfile::TempDir` lives at least as long as
    /// the replicas that point at it. `None` for the legacy
    /// `spawn`/`spawn_sharing_schema` paths.
    #[allow(dead_code)]
    pub migrations_dir: Option<TestMigrationsDir>,
    /// US-04 only: per-replica boot durations captured by
    /// `spawn_concurrent`. Indexed by spawn-order slot. The
    /// slow-lock-race scenario reads these to assert "the second
    /// replica's boot is blocked for between N and M ms" against an
    /// observable; the value is the wall-clock between the parallel
    /// `boot_test_migrations` call starting and returning.
    pub boot_durations: Vec<std::time::Duration>,
}

impl std::fmt::Debug for MultiReplicaHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiReplicaHarness")
            .field("schema", &self.schema)
            .field("proxy_addr", &self.proxy.addr)
            .field(
                "replica_addrs",
                &self.replicas.iter().map(|r| r.addr).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl MultiReplicaHarness {
    /// Spawn N replicas sharing ONE per-scenario schema + a round-robin
    /// proxy in front. Booting is sequential — concurrent boot is
    /// only needed for the US-04 advisory-lock race (different harness
    /// entry point in slice-4).
    pub async fn spawn(n: usize, now: time::OffsetDateTime) -> Self {
        // No existing single-replica harness; provision a fresh schema.
        Self::spawn_inner(n, now, None).await
    }

    /// US-04: Spawn N replicas in PARALLEL against a fresh per-scenario
    /// schema that has NOT yet had migrations applied. Each replica's
    /// boot path races for `pg_advisory_lock(MIGRATION_LOCK_ID)` and
    /// applies migrations from `migrations_dir` (typically a
    /// `tempfile::TempDir` staged via `support::test_migration::stage`).
    ///
    /// One replica wins the lock and applies the migrations; the others
    /// block on the lock, then observe the migrations as already-applied.
    /// Each replica's per-invocation [`MigrationReport`] is recorded into
    /// its `AppState::applied_migrations` so the per-replica
    /// "exactly-one-applied" assertion can read it.
    ///
    /// Returns the harness on success. If any replica fails its
    /// migration boot, returns `Err(SpawnConcurrentError)` carrying the
    /// failure details — the broken-migration acceptance scenario
    /// asserts against this Err path.
    pub async fn spawn_concurrent(
        n: usize,
        migrations_dir: TestMigrationsDir,
        now: time::OffsetDateTime,
    ) -> Result<Self, SpawnConcurrentError> {
        // Fresh per-scenario schema with NO migrations applied — the
        // replicas race for the advisory lock to apply them.
        Self::spawn_concurrent_inner(n, migrations_dir, 0, now, None).await
    }

    /// US-04 variant of [`Self::spawn_concurrent`] that REUSES the
    /// per-scenario schema + pool + listen URL of an already-running
    /// `InProcHarness` (which typically applied production base
    /// migrations + seeded workspace state during the slice-1
    /// Background). The N replicas race for the advisory lock on top of
    /// that existing schema — migrations already present are observed
    /// as already-applied, and only the staged 0099 racing migration
    /// remains for the winner to apply.
    pub async fn spawn_concurrent_sharing_schema(
        n: usize,
        migrations_dir: TestMigrationsDir,
        existing: &InProcHarness,
        now: time::OffsetDateTime,
    ) -> Result<Self, SpawnConcurrentError> {
        Self::spawn_concurrent_sharing_schema_with_delay(n, migrations_dir, existing, 0, now).await
    }

    /// As [`Self::spawn_concurrent_sharing_schema`] but with an
    /// explicit per-replica slow-migration delay (ms). The delay is
    /// applied per-call inside `run_migrations_from_dir_with_delay`
    /// AND gated on `has_work` — the winner of the advisory-lock race
    /// pays the delay; the loser observes no work and skips it.
    pub async fn spawn_concurrent_sharing_schema_with_delay(
        n: usize,
        migrations_dir: TestMigrationsDir,
        existing: &InProcHarness,
        delay_ms: u64,
        now: time::OffsetDateTime,
    ) -> Result<Self, SpawnConcurrentError> {
        let shared = SharedSchema {
            schema: existing.schema.clone(),
            pool: existing.app.state.store.pool().clone(),
            listen_url: format!(
                "{base}?options=-csearch_path%3D{schema}",
                base = ensure_postgres().await,
                schema = existing.schema,
            ),
            existing_addr: Some(existing.app.addr),
        };
        Self::spawn_concurrent_inner(n, migrations_dir, delay_ms, now, Some(shared)).await
    }

    async fn spawn_concurrent_inner(
        n: usize,
        migrations_dir: TestMigrationsDir,
        delay_ms: u64,
        now: time::OffsetDateTime,
        shared: Option<SharedSchema>,
    ) -> Result<Self, SpawnConcurrentError> {
        assert!(n >= 1, "MultiReplicaHarness requires n >= 1");
        let _ = ensure_postgres().await;

        let (schema, pool, listen_url) = match shared {
            Some(s) => (s.schema, s.pool, s.listen_url),
            None => fresh_schema_pool_no_migrations().await,
        };
        let fake_clock = MockClock::new(now);
        let fake_email = FakeEmailSender::new();
        let heartbeat_ms =
            heartbeat_env::current_heartbeat_ms().unwrap_or(DEFAULT_SSE_HEARTBEAT_MS);
        let file_upload_max_mb =
            file_upload_env::current_file_upload_max_mb().unwrap_or(DEFAULT_FILE_UPLOAD_MAX_MB);
        let db_unreachable = Arc::new(AtomicBool::new(false));

        // Build one AppState per replica with the shared schema + pool.
        let migrations_path = migrations_dir.path().to_path_buf();
        let states: Vec<AppState> = (0..n)
            .map(|_| {
                let realtime_tx = foundry_realtime::build_broadcast();
                let store = Arc::new(Store::from_pool(pool.clone()));
                AppState {
                    store,
                    session_secret: Arc::new(SecretString::new(
                        "test-only-secret-must-be-at-least-32-bytes-long-please-yes".into(),
                    )),
                    machine_token_verifier: Arc::new(foundry_auth::test_keys::verifier()),
                    // machine-token-admin-ux: multi-replica scenarios verify-only.
                    machine_token_signer: None,
                    session_cookie_secure: true,
                    db_schema: schema.clone(),
                    public_url: "http://localhost".into(),
                    clock: fake_clock.clone(),
                    email: fake_email.clone(),
                    realtime_tx,
                    sse_heartbeat_ms: heartbeat_ms,
                    file_upload_max_mb,
                    db_unreachable: db_unreachable.clone(),
                    force_board_render_failure: Arc::new(AtomicBool::new(false)),
                    test_migrations_dir: Some(migrations_path.clone()),
                    applied_migrations: Arc::new(Mutex::new(MigrationReport::default())),
                    test_migration_delay_ms: delay_ms,
                }
            })
            .collect();

        // Race the migration boots in PARALLEL via join_all. Each call
        // contends for `pg_advisory_lock(MIGRATION_LOCK_ID)` inside
        // `boot_test_migrations`; the winner runs the migrator, the
        // others block, observe already-applied, and release. Each
        // call's wall-clock duration is recorded so the slow-lock-race
        // scenario can assert "second replica blocked for between N
        // and M ms" against a real observable.
        let timed_results: Vec<(Result<(), foundry_store::StoreError>, std::time::Duration)> =
            futures::future::join_all(states.iter().map(|s| async move {
                let started = std::time::Instant::now();
                let r = boot_test_migrations(s).await;
                (r, started.elapsed())
            }))
            .await;
        let mut migration_results: Vec<Result<(), foundry_store::StoreError>> =
            Vec::with_capacity(n);
        let mut boot_durations: Vec<std::time::Duration> = Vec::with_capacity(n);
        for (r, d) in timed_results {
            migration_results.push(r);
            boot_durations.push(d);
        }

        // Surface the FIRST failure as the harness error. The remaining
        // replicas may have raced through fine; we still abort the
        // harness because the failed-migration scenario expects a boot
        // failure.
        for (idx, result) in migration_results.iter().enumerate() {
            if let Err(e) = result {
                return Err(SpawnConcurrentError::MigrationFailed {
                    replica_idx: idx,
                    detail: e.to_string(),
                });
            }
        }

        // All migrations succeeded; now bind HTTP listeners for each
        // replica. Boot order here is irrelevant — the advisory-lock
        // race was the production-meaningful step.
        let mut replicas: Vec<TestApp> = Vec::with_capacity(n);
        let mut flags: Vec<Arc<AtomicBool>> = Vec::with_capacity(n);
        for state in states {
            flags.push(db_unreachable.clone());
            // Move the populated applied_migrations Arc onto the
            // TestApp's AppState so per-replica getters work.
            let app = spawn_app_with_listener(state, listen_url.clone())
                .await
                .map_err(|e| SpawnConcurrentError::BindFailed {
                    detail: e.to_string(),
                })?;
            replicas.push(app);
        }

        let proxy =
            spawn_round_robin_proxy(replicas.iter().map(|r| r.addr).collect::<Vec<_>>()).await;

        Ok(Self {
            replicas,
            db_unreachable_flags: flags,
            schema,
            proxy,
            fake_clock,
            fake_email,
            shared_pool: pool,
            listen_url,
            migrations_dir: Some(migrations_dir),
            boot_durations,
        })
    }

    /// Spawn N replicas reusing the per-scenario schema + pool that the
    /// existing single-replica `InProcHarness` already provisioned.
    /// Used by the US-02 step phrase that fires after the inherited
    /// Background steps have already seeded a workspace/team/project
    /// through `world.harness`. The seeded rows are immediately
    /// visible to every spawned replica because they all share the
    /// same `search_path`-pinned pool.
    pub async fn spawn_sharing_schema(
        n: usize,
        existing: &InProcHarness,
        now: time::OffsetDateTime,
    ) -> Self {
        let shared = SharedSchema {
            schema: existing.schema.clone(),
            pool: existing.app.state.store.pool().clone(),
            // Reconstruct the listen URL the same way the harness did.
            // Tests that need this rare value can rebuild it explicitly.
            listen_url: format!(
                "{base}?options=-csearch_path%3D{schema}",
                base = ensure_postgres().await,
                schema = existing.schema,
            ),
            existing_addr: Some(existing.app.addr),
        };
        Self::spawn_inner(n, now, Some(shared)).await
    }

    async fn spawn_inner(
        n: usize,
        now: time::OffsetDateTime,
        shared: Option<SharedSchema>,
    ) -> Self {
        assert!(n >= 1, "MultiReplicaHarness requires n >= 1");
        // Force the shared Postgres container to exist before any of
        // the parallel spawns race the lazy initialiser.
        let _ = ensure_postgres().await;

        let (schema, pool, listen_url, _existing_addr) = match shared {
            Some(s) => (s.schema, s.pool, s.listen_url, s.existing_addr),
            None => {
                let (schema, pool, listen_url) = fresh_schema_pool_with_url().await;
                (schema, pool, listen_url, None)
            }
        };
        let fake_clock = MockClock::new(now);
        let fake_email = FakeEmailSender::new();
        let heartbeat_ms =
            heartbeat_env::current_heartbeat_ms().unwrap_or(DEFAULT_SSE_HEARTBEAT_MS);
        let file_upload_max_mb =
            file_upload_env::current_file_upload_max_mb().unwrap_or(DEFAULT_FILE_UPLOAD_MAX_MB);

        let db_unreachable = Arc::new(AtomicBool::new(false));

        let mut replicas: Vec<TestApp> = Vec::with_capacity(n);
        let mut flags: Vec<Arc<AtomicBool>> = Vec::with_capacity(n);
        for _ in 0..n {
            // EACH replica gets its OWN broadcast Sender — production
            // replicas do too. The pg_listener fans NOTIFY to all
            // listeners; the in-process broadcast is per-replica.
            let realtime_tx = foundry_realtime::build_broadcast();
            let store = Arc::new(Store::from_pool(pool.clone()));
            let state = AppState {
                store,
                session_secret: Arc::new(SecretString::new(
                    "test-only-secret-must-be-at-least-32-bytes-long-please-yes".into(),
                )),
                machine_token_verifier: Arc::new(foundry_auth::test_keys::verifier()),
                // machine-token-admin-ux: multi-replica scenarios verify-only.
                machine_token_signer: None,
                session_cookie_secure: true,
                db_schema: schema.clone(),
                public_url: "http://localhost".into(),
                clock: fake_clock.clone(),
                email: fake_email.clone(),
                realtime_tx,
                sse_heartbeat_ms: heartbeat_ms,
                file_upload_max_mb,
                db_unreachable: db_unreachable.clone(),
                force_board_render_failure: Arc::new(AtomicBool::new(false)),
                test_migrations_dir: None,
                applied_migrations: Arc::new(std::sync::Mutex::new(
                    foundry_store::MigrationReport::default(),
                )),
                test_migration_delay_ms: 0,
            };
            let app = spawn_app_with_listener(state, listen_url.clone())
                .await
                .expect("spawn multi-replica app + listener");
            flags.push(db_unreachable.clone());
            replicas.push(app);
        }

        let proxy =
            spawn_round_robin_proxy(replicas.iter().map(|r| r.addr).collect::<Vec<_>>()).await;

        Self {
            replicas,
            db_unreachable_flags: flags,
            schema,
            proxy,
            fake_clock,
            fake_email,
            shared_pool: pool,
            listen_url,
            migrations_dir: None,
            boot_durations: Vec::new(),
        }
    }

    /// Base URL the test reqwest client should hit — the proxy, NOT
    /// any individual replica.
    pub fn base_url(&self) -> String {
        self.proxy.base_url()
    }

    /// Direct addresses of the underlying replicas, in spawn order.
    pub fn replica_addrs(&self) -> Vec<SocketAddr> {
        self.replicas.iter().map(|r| r.addr).collect()
    }

    /// Mark every replica's `/readyz` as 503 by flipping the shared
    /// injection flag. Used by the NFR-OBS-02 scenario.
    pub fn mark_all_db_unreachable(&self) {
        for flag in &self.db_unreachable_flags {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// Per-replica `applied_migrations` accessor. Returns the
    /// [`MigrationReport`] captured by replica `idx` during its boot.
    /// US-04 assertions read this to verify exactly-one-applied semantics.
    pub fn applied_migrations(&self, idx: usize) -> MigrationReport {
        let app = self
            .replicas
            .get(idx)
            .expect("replica idx in range for applied_migrations()");
        app.state
            .applied_migrations
            .lock()
            .expect("applied_migrations mutex")
            .clone()
    }

    /// Stop the replica at slot `idx` by aborting its underlying tokio
    /// serve task AND removing it from the proxy's rotation. The
    /// per-replica TestApp's Drop will reclaim the listener task; the
    /// HTTP serve task winds down when its TcpListener drops (we send
    /// the shutdown oneshot first to be explicit).
    pub fn stop_replica(&mut self, idx: usize) {
        // (1) Flip the proxy slot to None so subsequent requests don't
        //     pick this replica.
        self.proxy.fail_replica(idx);
        // (2) Drop the TestApp's shutdown channel by replacing the
        //     TestApp with a sentinel — actually, we KEEP the TestApp
        //     around (its listener_task is also owned by it), but we
        //     send the shutdown signal so the axum serve task exits.
        //     This mimics SIGTERM behaviour: the listener stops
        //     accepting new connections, in-flight requests drain via
        //     axum's with_graceful_shutdown.
        if let Some(app) = self.replicas.get_mut(idx) {
            // Move the shutdown sender out via std::mem::replace with
            // a dummy oneshot that's already-dropped, then drop the
            // real one to fire shutdown.
            let (dummy_tx, _dummy_rx) = tokio::sync::oneshot::channel::<()>();
            let real_tx = std::mem::replace(&mut app.shutdown, dummy_tx);
            // Sending on a dropped Receiver is fine — `with_graceful_shutdown`
            // already polled the rx; if it's already won, this is a noop.
            let _ = real_tx.send(());
        }
    }
}

/// Failure modes for [`MultiReplicaHarness::spawn_concurrent`]. The
/// broken-migration US-04 scenario asserts against
/// `MigrationFailed` — the production-meaningful observable is
/// "the replica that attempted the broken migration fails to start".
#[derive(Debug, thiserror::Error)]
pub enum SpawnConcurrentError {
    #[error("replica {replica_idx} migration failed: {detail}")]
    MigrationFailed { replica_idx: usize, detail: String },
    #[error("replica HTTP listener failed to bind: {detail}")]
    BindFailed { detail: String },
}
