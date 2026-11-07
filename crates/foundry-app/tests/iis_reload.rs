//! instance-integration-setup step 02-04 — each replica's reload task
//! (ADR-IIS-004 r4, r8) over a real Postgres (testcontainers), through the
//! reload task's own store capability.
//!
//! - The gold test: a write committed between the version read and the state
//!   reads is installed by the next tick, and the replica state row's
//!   `applied_version` reaches it.
//! - A failed reload keeps serving the last good integrations and logs
//!   `integrations.reload.failed`.
//!
//! Example-based: the reload is an effect shell and the gold test is one fixed
//! interleaving.

use async_trait::async_trait;
use foundry_app::clock::MockClock;
use foundry_app::integrations::config::IntegrationSettings;
use foundry_app::integrations::keys::NoKeyFile;
use foundry_app::integrations::reload::Reload;
use foundry_app::integrations::store_capabilities::{IntegrationsReload, StoreCapabilities};
use foundry_app::integrations::{Integrations, IntegrationsHandle};
use foundry_app::Clock;
use foundry_store::integrations::{InstanceSnapshot, ReplicaState, SlackAppSave};
use foundry_store::{run_migrations, Store, StoreError};
use sqlx::postgres::PgPoolOptions;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
use uuid::Uuid;

/// The reload capability over the store, with the gold test's seam: an armed
/// Slack save commits between the snapshot's version read and its state
/// reads, and a switch that makes the snapshot read fail.
struct Interleaved {
    store: Arc<Store>,
    between_reads: Mutex<Option<SlackAppSave>>,
    snapshot_fails: AtomicBool,
}

#[async_trait]
impl IntegrationsReload for Interleaved {
    async fn config_version(&self) -> Result<i64, StoreError> {
        self.store.config_version().await
    }

    async fn instance_snapshot(&self) -> Result<InstanceSnapshot, StoreError> {
        if self.snapshot_fails.load(Ordering::SeqCst) {
            return Err(sqlx::Error::PoolTimedOut.into());
        }
        let armed = self.between_reads.lock().expect("seam").take();
        let store = self.store.clone();
        self.store
            .instance_snapshot_with(|_version| async move {
                if let Some(app) = armed {
                    store
                        .save_slack_app(&app)
                        .await
                        .expect("the interleaved save");
                }
            })
            .await
    }

    async fn beat_replica_state(&self, state: &ReplicaState) -> Result<(), StoreError> {
        self.store.beat_replica_state(state).await
    }
}

struct Lab {
    _pg: ContainerAsync<Postgres>,
    store: Arc<Store>,
    capability: Arc<Interleaved>,
    handle: IntegrationsHandle,
    clock: Arc<MockClock>,
    priya: Uuid,
}

impl Lab {
    async fn start() -> Self {
        let pg = Postgres::default()
            .with_tag("16-alpine")
            .start()
            .await
            .expect("start postgres container");
        let url = format!(
            "postgres://postgres:postgres@{}:{}/postgres",
            pg.get_host().await.expect("host"),
            pg.get_host_port_ipv4(5432).await.expect("port")
        );
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .acquire_timeout(Duration::from_secs(20))
            .connect(&url)
            .await
            .expect("connect pool");
        run_migrations(&pool).await.expect("run migrations");
        let store = Arc::new(Store::from_pool(pool));
        let priya = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
                  VALUES ($1, 'priya@canzan.dev', 'priya@canzan.dev', 'Priya Raman', 'x')",
        )
        .bind(priya)
        .execute(store.pool())
        .await
        .expect("seed Priya");
        let booted = Integrations::boot(
            IntegrationSettings::off("https://foundry.example.org"),
            &*StoreCapabilities::new(store.clone()),
            &NoKeyFile,
        )
        .await
        .expect("boot");
        let clock = MockClock::new(
            time::OffsetDateTime::from_unix_timestamp(1_800_000_000).expect("a time"),
        );
        Self {
            _pg: pg,
            capability: Arc::new(Interleaved {
                store: store.clone(),
                between_reads: Mutex::new(None),
                snapshot_fails: AtomicBool::new(false),
            }),
            store,
            handle: IntegrationsHandle::new(booted),
            clock,
            priya,
        }
    }

    fn reload(&self) -> Reload {
        let clock: Arc<dyn Clock> = self.clock.clone();
        Reload::new(self.capability.clone(), self.handle.clone(), clock)
    }

    fn slack_save(&self, client_id: &str) -> SlackAppSave {
        SlackAppSave {
            client_id: client_id.to_string(),
            sealed_client_secret: vec![1, 2, 3],
            sealed_signing_secret: vec![4, 5, 6],
            sealed_under: None,
            saved_by: self.priya,
            saved_at: self.clock.now(),
        }
    }

    async fn applied_versions(&self) -> Vec<i64> {
        self.store
            .replica_states()
            .await
            .expect("replica states")
            .into_iter()
            .map(|row| row.applied_version)
            .collect()
    }
}

#[tokio::test]
async fn a_write_between_the_version_read_and_the_state_reads_is_installed_by_the_next_tick() {
    let lab = Lab::start().await;
    let booted_at = lab.handle.current().applied_version().expect("boot read");
    lab.store
        .save_slack_app(&lab.slack_save("6123456789012.1"))
        .await
        .expect("a first save");
    let first = lab.store.config_version().await.expect("version");
    *lab.capability.between_reads.lock().expect("seam") = Some(lab.slack_save("6123456789012.2"));
    let mut reload = lab.reload();

    reload.tick().await;
    let second = lab.store.config_version().await.expect("version");
    assert!(
        first > booted_at && second > first,
        "both saves bumped the version"
    );
    assert_eq!(
        lab.handle.current().applied_version(),
        Some(first),
        "the tick applies the version its snapshot read, not the later commit"
    );
    assert_eq!(lab.applied_versions().await, vec![first]);

    reload.tick().await;
    assert_eq!(lab.handle.current().applied_version(), Some(second));
    assert_eq!(
        lab.applied_versions().await,
        vec![second],
        "this replica's one state row reaches the interleaved write"
    );
}

#[tokio::test]
async fn a_failed_reload_keeps_the_last_good_integrations_and_says_so() {
    let lab = Lab::start().await;
    let last_good = lab.handle.current();
    lab.store
        .save_slack_app(&lab.slack_save("6123456789012.1"))
        .await
        .expect("a save");
    lab.capability.snapshot_fails.store(true, Ordering::SeqCst);
    let lines = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = {
        let lines = lines.clone();
        move || Captured(lines.clone())
    };
    let subscriber = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    lab.reload().tick().await;

    assert!(
        Arc::ptr_eq(&lab.handle.current(), &last_good),
        "the last good integrations keep serving"
    );
    let logged = String::from_utf8(lines.lock().expect("lines").clone()).expect("utf-8");
    assert!(
        logged.contains("integrations.reload.failed"),
        "the failure is logged: {logged}"
    );
    assert_eq!(
        lab.applied_versions().await,
        vec![last_good.applied_version().expect("boot read")],
        "the state row still names the version served"
    );
}

/// A log line sink for one test.
struct Captured(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("lines").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
