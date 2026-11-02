//! external-integration-plugins — the EIP-KPI-4 guardrail bench (DEVOPS
//! "EIP-KPI-4 method", UI-9, gate G3; `eip-kpi4-bench`, manual, one bounded
//! invocation per repeat):
//!
//! `timeout 90m cargo test -p foundry-acceptance --release --test eip_kpi4_bench -- --ignored --nocapture`
//!
//! One release-profile `foundry` subprocess on its own fresh `postgres:16-alpine`
//! (256 MB /dev/shm, 1 GB WAL files, label `foundry-acceptance=eip`); arm A is a project with no
//! routes, arm B a project with one Slack, one Matrix and one email route, every
//! destination black-holed and the worker active. Per concurrency (1, then 4):
//! 50 warm-up calls per arm, discarded, then 500 issue creates and 500 lane moves
//! per arm, arms interleaved in blocks of 50 (ABAB…). Statistic: a 2,000-resample
//! bootstrap 95% interval on p95(B)/p95(A), fixed seed. Verdict PASS (upper bound
//! <= 1.10), FAIL (lower bound > 1.10) or INCONCLUSIVE; an INCONCLUSIVE run is
//! repeated three more times and the verdict taken from the pooled samples of
//! every run on the same commit. Output `target/eip-kpi4/<sha>.json` (`verdict`
//! field, read by G3) plus each run's raw samples under `target/eip-kpi4/<sha>/`.
//!
//! Modes (`EIP_KPI4_MODE`):
//! - `guardrail` (default): arms A and B, with the worker sending to three
//!   black holes (listeners that accept and never answer); a run in which any
//!   route kind saw no attempt fails, rather than report a verdict it did not
//!   measure.
//! - `baseline`: arm A only (task T-1); verdict `BASELINE`, the arm-A
//!   p50/p95/p99 per operation and concurrency go into `kpi-contracts.yaml`.
//! - `worker-off`: arms A and B with routes seeded and the worker stopped, so a
//!   B/A difference cannot be the worker's (FU-24). Its verdict is `CONTROL`, so
//!   it can never satisfy G3.
//!
//! `EIP_KPI4_DIAGNOSE=1` (always on for `worker-off`) also records each call's
//! start time, gaps in a thread that only sleeps (a stall of the whole process
//! or VM), and Postgres's slow statements, lock waits, checkpoints and autovacuums,
//! all in unix or UTC time so they line up, and the WAL bytes written.
//! `EIP_KPI4_PG_SET="name=value;…"` adds Postgres settings to a diagnosed run
//! (e.g. `log_min_duration_statement=3`). Those runs go under
//! `target/eip-kpi4-diagnose/`, never into G3's results.
//!
//! Latencies are client-side wall-clock per request (send to full body read)
//! over loopback; the board's htmx forms are what is driven.

use foundry_acceptance::steps::handler_instrumentation::FoundrySubprocess;
use foundry_acceptance::support::external_integration_plugins::SECRETS_KEY;
use foundry_acceptance::support::matrix_homeserver::{MAILPIT_IMAGE, MAILPIT_TAG};
use foundry_app::integrations::credential_slot;
use foundry_auth::secret_box::SecretBox;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use secrecy::SecretString;
use serde_json::{json, Value};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::core::{IntoContainerPort, Mount};
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};

const POSTGRES_TAG: &str = "16-alpine";
const SHM_BYTES: u64 = 256 * 1024 * 1024;
/// 1 GB WAL files, so a run (about 530 MB of WAL) never starts a new one: in a
/// fresh database each new 16 MB file is zero-filled while commits wait, and
/// that wait landed in arm A or B by chance (G3, 2026-11-02).
const WAL_SEGMENT_INITDB_ARG: &str = "--wal-segsize=1024";
const SCHEMA: &str = "eip_kpi4";
const WARM_UP_CALLS: usize = 50;
const CALLS_PER_OPERATION: usize = 500;
const BLOCK: usize = 50;
const CONCURRENCIES: [usize; 2] = [1, 4];
const RESAMPLES: usize = 2_000;
const SEED: u64 = 0x00E1_9C41;
const RATIO_LIMIT: f64 = 1.10;
const POOLED_RUNS: usize = 4;
const PASSWORD: &str = "eip-kpi4-bench-password";
const EMAIL: &str = "bench@example.test";
const TEAM: &str = "bench";
const MOVE_TARGET: &str = "in_progress";
const RECIPIENT_EMAIL: &str = "reader@example.test";
const BENCH_EVENTS: [&str; 2] = ["issue.created", "issue.moved"];
/// A sleeping thread's wake-up this late means the process did not get a CPU.
const STALL_THRESHOLD_MS: f64 = 50.0;
const STALL_PROBE_SLEEP: Duration = Duration::from_millis(5);
const POSTGRES_DIAGNOSTIC_SETTINGS: [(&str, &str); 5] = [
    ("log_min_duration_statement", "100"),
    ("log_lock_waits", "on"),
    ("deadlock_timeout", "50"),
    ("log_checkpoints", "on"),
    ("log_autovacuum_min_duration", "0"),
];

#[test]
#[ignore = "manual: EIP-KPI-4 bench (run via eip-kpi4-bench)"]
fn eip_kpi4_bench() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
        .block_on(run_bench());
}

async fn run_bench() {
    let started = Instant::now();
    let mode = Mode::from_env();
    let arms = mode.arms();

    let database = BenchDatabase::start().await;
    let diagnostics = mode.diagnosed().then(Diagnostics::start);
    if diagnostics.is_some() {
        database.log_slow_work().await;
    }
    for arm in &arms {
        database.seed_project(arm).await;
    }
    let holes = match mode {
        Mode::Guardrail | Mode::WorkerOff => Some(BlackHoles::bind().await),
        Mode::Baseline => None,
    };
    if let Some(holes) = &holes {
        database.seed_routes(&Arm::B, holes).await;
    }
    let mut env = holes.as_ref().map(BlackHoles::env).unwrap_or_default();
    if mode == Mode::WorkerOff {
        env.push(("FOUNDRY_INTEGRATIONS_WORKER", "off".to_string()));
    }
    let foundry = FoundrySubprocess::spawn_with_env_overrides(
        &database.url_with_schema(),
        SCHEMA.to_string(),
        5,
        &env,
    )
    .await
    .expect("spawn the release foundry subprocess");
    if let Some(holes) = &holes {
        holes.set_relay_paused(true);
    }
    let base = format!("http://{}", foundry.main_addr);
    let calls = diagnostics.as_ref().map(|d| d.calls.clone());
    let client = BoardClient::sign_in(&base, calls).await;

    let mut samples = Samples::default();
    let mut cursors: Vec<IssueCursor> = arms.iter().map(|_| IssueCursor::default()).collect();
    for concurrency in CONCURRENCIES {
        for (arm, cursor) in arms.iter().zip(cursors.iter_mut()) {
            client.warm_up(arm, cursor, concurrency).await;
        }
        for _ in 0..CALLS_PER_OPERATION / BLOCK {
            for (arm, cursor) in arms.iter().zip(cursors.iter_mut()) {
                let block = client.run_block(arm, cursor, concurrency).await;
                samples.extend(arm.label, concurrency, block);
            }
        }
    }
    match (mode, &holes) {
        (Mode::Guardrail, Some(holes)) => database.assert_every_kind_attempted(holes).await,
        (Mode::WorkerOff, _) => database.assert_nothing_dispatched().await,
        _ => {}
    }
    drop(foundry);
    if let Some(holes) = holes {
        holes.stop().await;
    }
    let diagnostics = match diagnostics {
        Some(diagnostics) => Some(diagnostics.finish(database.postgres_log().await)),
        None => None,
    };
    database.stop().await;

    let run = RunRecord::new(mode, samples, diagnostics, started.elapsed());
    let report = ResultStore::for_commit(&run.commit, mode.diagnosed()).record(run);
    println!("{}", report.summary_line());
}

// ------------------------------------------------------------------ modes, arms

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Guardrail,
    Baseline,
    WorkerOff,
}

impl Mode {
    fn from_env() -> Self {
        match std::env::var("EIP_KPI4_MODE").as_deref() {
            Ok("baseline") => Self::Baseline,
            Ok("worker-off") => Self::WorkerOff,
            Ok("guardrail") | Err(_) => Self::Guardrail,
            Ok(other) => {
                panic!("EIP_KPI4_MODE must be guardrail, baseline or worker-off; got {other:?}")
            }
        }
    }

    fn arms(self) -> Vec<Arm> {
        match self {
            Self::Baseline => vec![Arm::A],
            Self::Guardrail | Self::WorkerOff => vec![Arm::A, Arm::B],
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Guardrail => "guardrail",
            Self::Baseline => "baseline",
            Self::WorkerOff => "worker-off",
        }
    }

    fn diagnosed(self) -> bool {
        self == Self::WorkerOff || std::env::var_os("EIP_KPI4_DIAGNOSE").is_some()
    }
}

// ------------------------------------------------------------------ diagnostics

/// Each measured call: when it started (unix ms), its path, its latency (ms).
type CallTrace = Arc<Mutex<Vec<(u64, String, f64)>>>;

/// What lets a slow call be placed against everything else going on.
struct Diagnostics {
    calls: CallTrace,
    stall_probe_stop: Arc<AtomicBool>,
    stall_probe: std::thread::JoinHandle<Vec<(u64, f64)>>,
}

impl Diagnostics {
    fn start() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let stall_probe = std::thread::spawn(move || {
            let mut stalls = Vec::new();
            while !stopping.load(Ordering::Relaxed) {
                let before = Instant::now();
                std::thread::sleep(STALL_PROBE_SLEEP);
                let gap = before.elapsed().as_secs_f64() * 1_000.0;
                if gap >= STALL_THRESHOLD_MS {
                    stalls.push((unix_ms(), gap));
                }
            }
            stalls
        });
        Self {
            calls: CallTrace::default(),
            stall_probe_stop: stop,
            stall_probe,
        }
    }

    fn finish(self, postgres_log: String) -> Value {
        self.stall_probe_stop.store(true, Ordering::Relaxed);
        let stalls = self.stall_probe.join().expect("the stall probe");
        let calls = self.calls.lock().unwrap_or_else(|p| p.into_inner());
        json!({
            "calls": *calls,
            "stalls_unix_ms_gap_ms": stalls,
            "postgres_log": postgres_log,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Arm {
    label: &'static str,
    project_slug: &'static str,
    key_prefix: &'static str,
}

impl Arm {
    /// No routes.
    const A: Self = Self {
        label: "A",
        project_slug: "arm-a",
        key_prefix: "ARMA",
    };
    /// One Slack, one Matrix and one email route, every destination black-holed.
    const B: Self = Self {
        label: "B",
        project_slug: "arm-b",
        key_prefix: "ARMB",
    };
}

// ------------------------------------------------------------- the black holes

/// A listener for Slack or Matrix: it accepts, counts and holds each connection
/// and never writes, so a send lasts until its own timeout.
struct BlackHole {
    port: u16,
    connections: Arc<AtomicUsize>,
}

impl BlackHole {
    async fn bind(address: IpAddr) -> Self {
        let listener = TcpListener::bind((address, 0))
            .await
            .expect("bind a black hole");
        let port = listener.local_addr().expect("a black hole's port").port();
        let connections = Arc::new(AtomicUsize::new(0));
        let counted = connections.clone();
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((stream, _)) = listener.accept().await {
                counted.fetch_add(1, Ordering::SeqCst);
                held.push(stream);
            }
        });
        Self { port, connections }
    }

    fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }
}

/// SMTP is the pinned relay, paused once Foundry has booted: the shipped
/// provider requires STARTTLS at its startup probe, so a silent socket would
/// refuse the boot, and a paused relay answers nothing after it.
struct BlackHoles {
    slack: BlackHole,
    matrix: BlackHole,
    matrix_address: IpAddr,
    relay: ContainerAsync<GenericImage>,
    smtp_port: u16,
}

impl BlackHoles {
    async fn bind() -> Self {
        let tls = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tls");
        let relay = GenericImage::new(MAILPIT_IMAGE, MAILPIT_TAG)
            .with_exposed_port(1025.tcp())
            .with_mount(Mount::bind_mount(tls, "/tls"))
            .with_env_var("MP_SMTP_TLS_CERT", "/tls/mailpit-test.crt")
            .with_env_var("MP_SMTP_TLS_KEY", "/tls/mailpit-test.private-key")
            .with_env_var("MP_SMTP_REQUIRE_STARTTLS", "true")
            .with_label("foundry-acceptance", "eip")
            .start()
            .await
            .expect("start the bench's relay");
        let smtp_port = relay
            .get_host_port_ipv4(1025.tcp())
            .await
            .expect("the relay's smtp port");
        await_smtp_greeting(smtp_port).await;
        let matrix_address = lan_address();
        Self {
            slack: BlackHole::bind(IpAddr::V4(Ipv4Addr::LOCALHOST)).await,
            matrix: BlackHole::bind(matrix_address).await,
            matrix_address,
            relay,
            smtp_port,
        }
    }

    fn set_relay_paused(&self, paused: bool) {
        let verb = if paused { "pause" } else { "unpause" };
        let status = std::process::Command::new("docker")
            .args([verb, self.relay.id()])
            .stdout(std::process::Stdio::null())
            .status()
            .expect("run docker pause");
        assert!(status.success(), "docker {verb} the bench's relay");
    }

    async fn stop(self) {
        self.set_relay_paused(false);
        if let Err(err) = self.relay.rm().await {
            eprintln!("warning: could not remove the bench relay: {err}");
        }
    }

    /// The operator's settings for this instance; poll and lease stay default.
    fn env(&self) -> Vec<(&'static str, String)> {
        vec![
            ("FOUNDRY_SECRETS_KEY", SECRETS_KEY.to_string()),
            (
                "FOUNDRY_PUBLIC_URL",
                "https://foundry.bench.test".to_string(),
            ),
            (
                "FOUNDRY_OUTBOUND_ALLOW",
                format!("{}/32", self.matrix_address),
            ),
            ("SLACK_CLIENT_ID", "bench-client".to_string()),
            ("SLACK_CLIENT_SECRET", "bench-secret".to_string()),
            ("SLACK_SIGNING_SECRET", "bench-signing".to_string()),
            (
                "SLACK_API_BASE_URL",
                format!("http://127.0.0.1:{}/api", self.slack.port),
            ),
            ("NOTIFICATION_PROVIDERS", "smtp".to_string()),
            ("SMTP_HOST", "127.0.0.1".to_string()),
            ("SMTP_PORT", self.smtp_port.to_string()),
            ("SMTP_FROM", "foundry@bench.test".to_string()),
            ("SMTP_TLS_VERIFY", "false".to_string()),
        ]
    }
}

// ---------------------------------------------------------------- the database

struct BenchDatabase {
    container: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    base_url: String,
    pool: PgPool,
}

impl BenchDatabase {
    async fn start() -> Self {
        let container = Postgres::default()
            .with_tag(POSTGRES_TAG)
            .with_shm_size(SHM_BYTES)
            .with_env_var("POSTGRES_INITDB_ARGS", WAL_SEGMENT_INITDB_ARG)
            .with_label("foundry-acceptance", "eip")
            .start()
            .await
            .expect("start the bench's own postgres container");
        let host = container.get_host().await.expect("postgres host");
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("postgres port");
        let base_url = format!("postgres://postgres:postgres@{host}:{port}/postgres");
        let pool = Self::schema_pool(&base_url).await;
        foundry_store::run_migrations(&pool)
            .await
            .expect("run migrations");
        Self {
            container,
            base_url,
            pool,
        }
    }

    async fn schema_pool(base_url: &str) -> PgPool {
        let options = PgConnectOptions::from_str(base_url)
            .expect("parse postgres url")
            .ssl_mode(PgSslMode::Disable);
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await
            .expect("connect to postgres");
        sqlx::query(&format!("CREATE SCHEMA {SCHEMA}"))
            .execute(&admin)
            .await
            .expect("create the bench schema");
        admin.close().await;
        PgPoolOptions::new()
            .max_connections(2)
            .connect_with(options.options([("search_path", SCHEMA)]))
            .await
            .expect("connect with the bench search_path")
    }

    fn url_with_schema(&self) -> String {
        format!("{}?options=-csearch_path%3D{SCHEMA}", self.base_url)
    }

    /// FU-24's evidence: what Postgres itself was doing around a slow call.
    async fn log_slow_work(&self) {
        // `EIP_KPI4_PG_SET="name=value;…"` adds or overrides settings, e.g. a
        // 3 ms slow-statement threshold to see where a slow call's time goes.
        let extra = std::env::var("EIP_KPI4_PG_SET").unwrap_or_default();
        let extra: Vec<(&str, &str)> = extra
            .split(';')
            .filter(|pair| !pair.trim().is_empty())
            .map(|pair| {
                let (name, value) = pair
                    .split_once('=')
                    .expect("EIP_KPI4_PG_SET entries are name=value");
                assert!(
                    name.trim()
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_')
                        && !value.contains('\''),
                    "EIP_KPI4_PG_SET entry {pair:?} is not a plain setting"
                );
                (name.trim(), value.trim())
            })
            .collect();
        for (name, value) in POSTGRES_DIAGNOSTIC_SETTINGS.into_iter().chain(extra) {
            sqlx::query(&format!("ALTER SYSTEM SET {name} = '{value}'"))
                .execute(&self.pool)
                .await
                .expect("set a postgres logging setting");
        }
        sqlx::query("SELECT pg_reload_conf()")
            .execute(&self.pool)
            .await
            .expect("reload the postgres settings");
    }

    /// The log, headed by the WAL bytes written since initdb and the file size.
    async fn postgres_log(&self) -> String {
        let wal_bytes: String =
            sqlx::query_scalar("SELECT pg_wal_lsn_diff(pg_current_wal_lsn(), '0/0')::text")
                .fetch_one(&self.pool)
                .await
                .expect("read the WAL position");
        let segment: String = sqlx::query_scalar("SHOW wal_segment_size")
            .fetch_one(&self.pool)
            .await
            .expect("read the WAL file size");
        let log = self
            .container
            .stderr_to_vec()
            .await
            .expect("read the postgres log");
        format!(
            "wal_bytes={wal_bytes} wal_segment_size={segment}\n{}",
            String::from_utf8_lossy(&log)
        )
    }

    /// One workspace, team and member, plus one project with its lanes per arm.
    async fn seed_project(&self, arm: &Arm) {
        let workspace = self.ensure_member_and_team().await;
        let project = uuid::Uuid::now_v7();
        sqlx::query(
            "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
             SELECT $1, t.id, t.workspace_id, $2, $2, $3 FROM teams t WHERE t.slug = $4",
        )
        .bind(project)
        .bind(arm.project_slug)
        .bind(arm.key_prefix)
        .bind(TEAM)
        .execute(&self.pool)
        .await
        .expect("seed an arm's project");
        sqlx::query(
            "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
             SELECT gen_random_uuid(), $1, $2, seed.slug, seed.label, seed.position
               FROM (VALUES ('backlog', 'Backlog', 0), ('todo', 'Todo', 1),
                            ('in_progress', 'In-Progress', 2), ('done', 'Done', 3))
                    AS seed (slug, label, position)",
        )
        .bind(project)
        .bind(workspace)
        .execute(&self.pool)
        .await
        .expect("seed an arm's lanes");
    }

    /// Arm B's Slack and Matrix connections and routes, and a second member's
    /// email choices, all for the two events the bench drives.
    async fn seed_routes(&self, arm: &Arm, holes: &BlackHoles) {
        let (workspace, project): (uuid::Uuid, uuid::Uuid) =
            sqlx::query_as("SELECT workspace_id, id FROM projects WHERE slug = $1")
                .bind(arm.project_slug)
                .fetch_one(&self.pool)
                .await
                .expect("read the arm's project");
        let matrix_address = format!("https://{}:{}", holes.matrix_address, holes.matrix.port);
        for (kind, external_id, destination) in [
            ("slack", "T0BENCH", "C0BENCH"),
            ("matrix", matrix_address.as_str(), "!bench:bench.test"),
        ] {
            let connection = uuid::Uuid::now_v7();
            sqlx::query(
                "INSERT INTO integration_connections
                        (id, workspace_id, kind, state, external_id, app_id, bot_user_id,
                         label, sealed_credential, credential_saved_at)
                 VALUES ($1, $2, $3, 'connected', $4, 'A0BENCH', 'bench-bot', 'Bench', $5, now())",
            )
            .bind(connection)
            .bind(workspace)
            .bind(kind)
            .bind(external_id)
            .bind(sealed_credential(connection))
            .execute(&self.pool)
            .await
            .expect("seed a connection");
            sqlx::query(
                "INSERT INTO integration_routes
                        (id, workspace_id, project_id, connection_id, kind, destination_ref,
                         destination_label, events, content_level, receives_from, created_via)
                 VALUES ($1, $2, $3, $4, $5, $6, 'bench', $7, 'summary', now(), 'web')",
            )
            .bind(uuid::Uuid::now_v7())
            .bind(workspace)
            .bind(project)
            .bind(connection)
            .bind(kind)
            .bind(destination)
            .bind(BENCH_EVENTS)
            .execute(&self.pool)
            .await
            .expect("seed a route");
        }

        let recipient = uuid::Uuid::now_v7();
        sqlx::query(
            "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
             VALUES ($1, $2, $2, 'Bench Reader', 'unused')",
        )
        .bind(recipient)
        .bind(RECIPIENT_EMAIL)
        .execute(&self.pool)
        .await
        .expect("seed the email recipient");
        sqlx::query(
            "INSERT INTO workspace_memberships (workspace_id, user_id, role)
             VALUES ($1, $2, 'member')",
        )
        .bind(workspace)
        .bind(recipient)
        .execute(&self.pool)
        .await
        .expect("seed the recipient's workspace membership");
        sqlx::query(
            "INSERT INTO team_memberships (team_id, user_id, role)
             SELECT team_id, $1, 'member' FROM projects WHERE id = $2",
        )
        .bind(recipient)
        .bind(project)
        .execute(&self.pool)
        .await
        .expect("seed the recipient's team membership");
        for event in BENCH_EVENTS {
            sqlx::query(
                "INSERT INTO integration_email_choices (user_id, project_id, event, scope)
                 VALUES ($1, $2, $3, 'all')",
            )
            .bind(recipient)
            .bind(project)
            .bind(event)
            .execute(&self.pool)
            .await
            .expect("seed an email choice");
        }
    }

    /// The verdict means nothing unless the worker really was busy: each kind
    /// has queued deliveries with attempts, and each black hole saw a connection.
    async fn assert_every_kind_attempted(&self, holes: &BlackHoles) {
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT COALESCE(r.kind, 'email'), COUNT(*), COUNT(*) FILTER (WHERE d.attempts > 0)
               FROM integration_deliveries d
               LEFT JOIN integration_routes r ON r.id = d.route_id
              GROUP BY 1",
        )
        .fetch_all(&self.pool)
        .await
        .expect("read the delivery census");
        println!("EIP-KPI-4 deliveries (kind, queued, attempted): {rows:?}");
        for kind in ["slack", "matrix", "email"] {
            assert!(
                rows.iter().any(|row| row.0 == kind && row.2 > 0),
                "EIP-KPI-4: no {kind} delivery was attempted; arm B measured an idle worker"
            );
        }
        let connections = [
            ("slack", holes.slack.connections()),
            ("matrix", holes.matrix.connections()),
        ];
        println!("EIP-KPI-4 black-hole connections: {connections:?}");
        assert!(
            connections.iter().all(|(_, count)| *count > 0),
            "EIP-KPI-4: a black hole saw no connection: {connections:?}"
        );
    }

    /// The control means nothing unless the worker really was stopped.
    async fn assert_nothing_dispatched(&self) {
        let (deliveries,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM integration_deliveries")
            .fetch_one(&self.pool)
            .await
            .expect("read the delivery count");
        assert_eq!(deliveries, 0, "EIP-KPI-4: the worker-off run dispatched");
    }

    async fn ensure_member_and_team(&self) -> uuid::Uuid {
        if let Some(id) = sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM workspaces LIMIT 1")
            .fetch_optional(&self.pool)
            .await
            .expect("read the workspace")
        {
            return id;
        }
        let workspace = uuid::Uuid::now_v7();
        let team = uuid::Uuid::now_v7();
        let user = uuid::Uuid::now_v7();
        let hash = foundry_auth::hash_password(&SecretString::new(PASSWORD.to_string().into()))
            .await
            .expect("hash the bench password");
        let statements: [(&str, Vec<Bind>); 5] = [
            (
                "INSERT INTO workspaces (id, name) VALUES ($1, 'EIP bench')",
                vec![Bind::Id(workspace)],
            ),
            (
                "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Bench', $3)",
                vec![Bind::Id(team), Bind::Id(workspace), Bind::Text(TEAM.into())],
            ),
            (
                "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
                 VALUES ($1, $2, $2, 'Bench Member', $3)",
                vec![Bind::Id(user), Bind::Text(EMAIL.into()), Bind::Text(hash)],
            ),
            (
                "INSERT INTO workspace_memberships (workspace_id, user_id, role)
                 VALUES ($1, $2, 'member')",
                vec![Bind::Id(workspace), Bind::Id(user)],
            ),
            (
                "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')",
                vec![Bind::Id(team), Bind::Id(user)],
            ),
        ];
        for (sql, binds) in statements {
            let mut query = sqlx::query(sql);
            for bind in binds {
                query = match bind {
                    Bind::Id(id) => query.bind(id),
                    Bind::Text(text) => query.bind(text),
                };
            }
            query
                .execute(&self.pool)
                .await
                .expect("seed the bench member");
        }
        workspace
    }

    async fn stop(self) {
        self.pool.close().await;
        if let Err(err) = self.container.rm().await {
            eprintln!("warning: could not remove the bench postgres container: {err}");
        }
    }
}

enum Bind {
    Id(uuid::Uuid),
    Text(String),
}

/// This machine's private address. The outbound policy refuses loopback outright,
/// so the Matrix hole must sit where `FOUNDRY_OUTBOUND_ALLOW` can admit it. A UDP
/// `connect` only picks the route; nothing is sent.
fn lan_address() -> IpAddr {
    let socket = std::net::UdpSocket::bind(("0.0.0.0", 0)).expect("bind a UDP socket");
    socket
        .connect(("10.255.255.255", 9))
        .expect("this machine has a route to a private address");
    socket.local_addr().expect("the route's address").ip()
}

/// The relay's published port answers before the relay does, so wait for "220".
async fn await_smtp_greeting(port: u16) {
    for _ in 0..150 {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)).await {
            let mut greeting = [0u8; 3];
            let read = stream.read_exact(&mut greeting);
            if let Ok(Ok(_)) = tokio::time::timeout(Duration::from_secs(1), read).await {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!("the bench's relay never greeted on {port}");
}

/// A bot token sealed the way the app seals one, so the worker opens it.
fn sealed_credential(connection: uuid::Uuid) -> Vec<u8> {
    let key = SecretString::new(SECRETS_KEY.to_string().into());
    SecretBox::from_settings(&key, None)
        .expect("the bench's secrets key")
        .seal(
            &SecretString::new("bench-bot-token".to_string().into()),
            &credential_slot(connection),
        )
        .expect("seal the bench's bot token")
        .as_bytes()
        .to_vec()
}

// ------------------------------------------------------------ the board client

#[derive(Clone)]
struct BoardClient {
    http: reqwest::Client,
    base: String,
    cookie: String,
    csrf: String,
    calls: Option<CallTrace>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Operation {
    Create,
    Move,
}

impl Operation {
    const ALL: [Self; 2] = [Self::Create, Self::Move];

    fn name(self) -> &'static str {
        match self {
            Self::Create => "issue_create",
            Self::Move => "lane_move",
        }
    }
}

/// One block's measured latencies, in milliseconds.
struct BlockLatencies {
    creates: Vec<f64>,
    moves: Vec<f64>,
}

impl BoardClient {
    async fn sign_in(base: &str, calls: Option<CallTrace>) -> Self {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .pool_max_idle_per_host(8)
            .build()
            .expect("http client");
        let page = http
            .get(format!("{base}/sign-in"))
            .send()
            .await
            .expect("GET /sign-in");
        let csrf = cookie_value(&page, "foundry_csrf").expect("/sign-in mints foundry_csrf");
        let signed_in = http
            .post(format!("{base}/sign-in"))
            .header(reqwest::header::COOKIE, format!("foundry_csrf={csrf}"))
            .form(&[
                ("email", EMAIL),
                ("password", PASSWORD),
                ("_csrf", csrf.as_str()),
            ])
            .send()
            .await
            .expect("POST /sign-in");
        let session =
            cookie_value(&signed_in, "foundry_session").expect("sign-in issues a session");
        Self {
            http,
            base: base.to_string(),
            cookie: format!("foundry_session={session}; foundry_csrf={csrf}"),
            csrf,
            calls,
        }
    }

    fn project_url(&self, arm: &Arm) -> String {
        format!("{}/team/{TEAM}/project/{}", self.base, arm.project_slug)
    }

    /// Half creates, half moves of the issues just created; discarded.
    async fn warm_up(&self, arm: &Arm, cursor: &mut IssueCursor, concurrency: usize) {
        let half = WARM_UP_CALLS / 2;
        self.creates(arm, half, concurrency).await;
        self.moves(arm, cursor.advance(half), concurrency).await;
    }

    /// 50 creates, then a lane move of each of the 50 new issues.
    async fn run_block(
        &self,
        arm: &Arm,
        cursor: &mut IssueCursor,
        concurrency: usize,
    ) -> BlockLatencies {
        let creates = self.creates(arm, BLOCK, concurrency).await;
        let moves = self.moves(arm, cursor.advance(BLOCK), concurrency).await;
        BlockLatencies { creates, moves }
    }

    async fn creates(&self, arm: &Arm, calls: usize, concurrency: usize) -> Vec<f64> {
        let shares = split(calls, concurrency);
        let workers = shares.into_iter().map(|share| {
            let client = self.clone();
            let url = format!("{}/issues", self.project_url(arm));
            tokio::spawn(async move {
                let mut latencies = Vec::with_capacity(share);
                for _ in 0..share {
                    latencies.push(client.timed_post(&url, &[("title", "bench issue")]).await);
                }
                latencies
            })
        });
        join_latencies(workers).await
    }

    async fn moves(&self, arm: &Arm, numbers: Vec<i32>, concurrency: usize) -> Vec<f64> {
        let mut lists: Vec<Vec<i32>> = vec![Vec::new(); concurrency];
        for (index, number) in numbers.into_iter().enumerate() {
            lists[index % concurrency].push(number);
        }
        let workers = lists.into_iter().map(|list| {
            let client = self.clone();
            let project = self.project_url(arm);
            tokio::spawn(async move {
                let mut latencies = Vec::with_capacity(list.len());
                for number in list {
                    let url = format!("{project}/issues/{number}/state");
                    latencies.push(client.timed_post(&url, &[("state", MOVE_TARGET)]).await);
                }
                latencies
            })
        });
        join_latencies(workers).await
    }

    async fn timed_post(&self, url: &str, form: &[(&str, &str)]) -> f64 {
        let mut fields: Vec<(&str, &str)> = form.to_vec();
        fields.push(("_csrf", self.csrf.as_str()));
        let started_unix_ms = unix_ms();
        let start = Instant::now();
        let response = self
            .http
            .post(url)
            .header(reqwest::header::COOKIE, &self.cookie)
            .header("HX-Request", "true")
            .form(&fields)
            .send()
            .await
            .unwrap_or_else(|err| panic!("POST {url}: {err}"));
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let elapsed = start.elapsed().as_secs_f64() * 1_000.0;
        assert!(status.is_success(), "POST {url} answered {status}: {body}");
        if let Some(calls) = &self.calls {
            let path = url.trim_start_matches(&self.base).to_string();
            calls
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push((started_unix_ms, path, elapsed));
        }
        elapsed
    }
}

/// Issue numbers run 1..=N per project and every create succeeds, so the
/// numbers a block created are the next ones after the previous block's.
#[derive(Default)]
struct IssueCursor {
    created: i32,
}

impl IssueCursor {
    fn advance(&mut self, count: usize) -> Vec<i32> {
        let first = self.created + 1;
        self.created += count as i32;
        (first..=self.created).collect()
    }
}

fn cookie_value(response: &reqwest::Response, name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|value| value.strip_prefix(&prefix))
        .and_then(|rest| rest.split(';').next())
        .map(str::to_string)
}

fn split(calls: usize, workers: usize) -> Vec<usize> {
    (0..workers)
        .map(|index| calls / workers + usize::from(index < calls % workers))
        .collect()
}

async fn join_latencies(
    workers: impl Iterator<Item = tokio::task::JoinHandle<Vec<f64>>>,
) -> Vec<f64> {
    let mut all = Vec::new();
    for worker in workers {
        all.extend(worker.await.expect("bench worker"));
    }
    all
}

// ------------------------------------------------------------------ statistics

/// Raw latencies keyed by (arm, operation, concurrency).
#[derive(Default)]
struct Samples(BTreeMap<(String, Operation, usize), Vec<f64>>);

impl Samples {
    fn extend(&mut self, arm: &str, concurrency: usize, block: BlockLatencies) {
        self.cell(arm, Operation::Create, concurrency)
            .extend(block.creates);
        self.cell(arm, Operation::Move, concurrency)
            .extend(block.moves);
    }

    fn cell(&mut self, arm: &str, operation: Operation, concurrency: usize) -> &mut Vec<f64> {
        self.0
            .entry((arm.to_string(), operation, concurrency))
            .or_default()
    }

    fn get(&self, arm: &str, operation: Operation, concurrency: usize) -> &[f64] {
        self.0
            .get(&(arm.to_string(), operation, concurrency))
            .map_or(&[], Vec::as_slice)
    }

    fn to_json(&self) -> Value {
        let mut out = serde_json::Map::new();
        for ((arm, operation, concurrency), values) in &self.0 {
            out.insert(cell_key(arm, *operation, *concurrency), json!(values));
        }
        Value::Object(out)
    }

    fn from_json(value: &Value) -> Self {
        let mut samples = Self::default();
        for arm in ["A", "B"] {
            for operation in Operation::ALL {
                for concurrency in CONCURRENCIES {
                    let key = cell_key(arm, operation, concurrency);
                    if let Some(values) = value.get(&key).and_then(Value::as_array) {
                        samples
                            .cell(arm, operation, concurrency)
                            .extend(values.iter().filter_map(Value::as_f64));
                    }
                }
            }
        }
        samples
    }

    fn pooled(runs: &[Self]) -> Self {
        let mut pooled = Self::default();
        for run in runs {
            for (key, values) in &run.0 {
                pooled.0.entry(key.clone()).or_default().extend(values);
            }
        }
        pooled
    }
}

fn cell_key(arm: &str, operation: Operation, concurrency: usize) -> String {
    format!("{arm}/{}/c{concurrency}", operation.name())
}

/// Nearest-rank percentile: the value at index ceil(q * n) - 1 of the sorted sample.
fn percentile(values: &[f64], q: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let index = ((sorted.len() as f64) * q).ceil() as usize;
    sorted[index.saturating_sub(1).min(sorted.len() - 1)]
}

#[derive(Debug, Clone, Copy)]
struct RatioInterval {
    ratio: f64,
    lower: f64,
    upper: f64,
}

impl RatioInterval {
    /// 2,000-resample percentile bootstrap of p95(B)/p95(A), fixed seed.
    fn bootstrap(a: &[f64], b: &[f64]) -> Self {
        let mut rng = StdRng::seed_from_u64(SEED);
        let mut ratios: Vec<f64> = (0..RESAMPLES)
            .map(|_| {
                let a_star = resample(a, &mut rng);
                let b_star = resample(b, &mut rng);
                percentile(&b_star, 0.95) / percentile(&a_star, 0.95)
            })
            .collect();
        ratios.sort_by(f64::total_cmp);
        Self {
            ratio: percentile(b, 0.95) / percentile(a, 0.95),
            lower: percentile(&ratios, 0.025),
            upper: percentile(&ratios, 0.975),
        }
    }

    fn verdict(self) -> Verdict {
        if self.upper <= RATIO_LIMIT {
            Verdict::Pass
        } else if self.lower > RATIO_LIMIT {
            Verdict::Fail
        } else {
            Verdict::Inconclusive
        }
    }
}

fn resample(values: &[f64], rng: &mut StdRng) -> Vec<f64> {
    (0..values.len())
        .map(|_| values[rng.gen_range(0..values.len())])
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    Pass,
    Inconclusive,
    Fail,
    Baseline,
    Control,
}

impl Verdict {
    fn name(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Inconclusive => "INCONCLUSIVE",
            Self::Fail => "FAIL",
            Self::Baseline => "BASELINE",
            Self::Control => "CONTROL",
        }
    }

    /// Every cell must pass; any failing cell fails the run.
    fn combine(cells: impl Iterator<Item = Self>) -> Self {
        cells.max().unwrap_or(Self::Inconclusive)
    }
}

/// The guardrail statistic over every (operation, concurrency) cell.
fn guardrail(samples: &Samples) -> (Verdict, Value) {
    let mut cells = serde_json::Map::new();
    let mut verdicts = Vec::new();
    for operation in Operation::ALL {
        for concurrency in CONCURRENCIES {
            let a = samples.get("A", operation, concurrency);
            let b = samples.get("B", operation, concurrency);
            let interval = RatioInterval::bootstrap(a, b);
            verdicts.push(interval.verdict());
            cells.insert(
                format!("{}/c{concurrency}", operation.name()),
                json!({
                    "p95_a_ms": percentile(a, 0.95),
                    "p95_b_ms": percentile(b, 0.95),
                    "ratio": interval.ratio,
                    "ci95": [interval.lower, interval.upper],
                    "verdict": interval.verdict().name(),
                }),
            );
        }
    }
    (Verdict::combine(verdicts.into_iter()), Value::Object(cells))
}

/// p50/p95/p99 and n per arm, operation and concurrency.
fn distribution(samples: &Samples) -> Value {
    let mut out = serde_json::Map::new();
    for ((arm, operation, concurrency), values) in &samples.0 {
        out.insert(
            cell_key(arm, *operation, *concurrency),
            json!({
                "n": values.len(),
                "p50_ms": round3(percentile(values, 0.50)),
                "p95_ms": round3(percentile(values, 0.95)),
                "p99_ms": round3(percentile(values, 0.99)),
            }),
        );
    }
    Value::Object(out)
}

fn round3(value: f64) -> f64 {
    (value * 1_000.0).round() / 1_000.0
}

// --------------------------------------------------------------------- results

struct RunRecord {
    mode: Mode,
    commit: String,
    samples: Samples,
    diagnostics: Option<Value>,
    wall_clock: Duration,
}

impl RunRecord {
    fn new(mode: Mode, samples: Samples, diagnostics: Option<Value>, wall_clock: Duration) -> Self {
        Self {
            mode,
            commit: command_output("git", &["rev-parse", "HEAD"]),
            samples,
            diagnostics,
            wall_clock,
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "mode": self.mode.name(),
            "commit": self.commit,
            "production_code_dirty": !command_output(
                "git",
                &["status", "--porcelain", "--", "crates", ":(exclude)crates/foundry-acceptance"],
            ).is_empty(),
            "recorded_at_unix": unix_now(),
            "wall_clock_seconds": self.wall_clock.as_secs(),
            "machine": machine(),
            "postgres_image": format!("postgres:{POSTGRES_TAG}"),
            "profile": "release",
            "method": {
                "warm_up_calls": WARM_UP_CALLS,
                "calls_per_operation": CALLS_PER_OPERATION,
                "block": BLOCK,
                "concurrencies": CONCURRENCIES,
                "resamples": RESAMPLES,
                "seed": SEED,
                "ratio_limit": RATIO_LIMIT,
                "latency": "client-side wall-clock per request, loopback, ms",
            },
            "distribution": distribution(&self.samples),
            "samples_ms": self.samples.to_json(),
            "diagnostics": self.diagnostics,
        })
    }
}

struct ResultStore {
    dir: PathBuf,
    commit: String,
}

struct Report {
    path: PathBuf,
    summary: Value,
}

impl Report {
    fn summary_line(&self) -> String {
        format!(
            "EIP-KPI-4 verdict={} commit={} runs={} -> {}",
            self.summary["verdict"].as_str().unwrap_or("?"),
            self.summary["commit"].as_str().unwrap_or("?"),
            self.summary["runs"],
            self.path.display(),
        )
    }
}

impl ResultStore {
    fn for_commit(commit: &str, diagnosed: bool) -> Self {
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));
        Self {
            dir: target.join(if diagnosed {
                "eip-kpi4-diagnose"
            } else {
                "eip-kpi4"
            }),
            commit: commit.to_string(),
        }
    }

    /// Writes this run's raw record, then the commit's summary with its verdict.
    fn record(&self, run: RunRecord) -> Report {
        let runs_dir = self.dir.join(&self.commit);
        std::fs::create_dir_all(&runs_dir).expect("create the bench result directory");
        let record = run.to_json();
        let run_path = runs_dir.join(format!("{}-{}.json", run.mode.name(), unix_now()));
        write_json(&run_path, &record);

        let mut summary = record;
        summary
            .as_object_mut()
            .expect("record object")
            .remove("samples_ms");
        summary
            .as_object_mut()
            .expect("record object")
            .remove("diagnostics");
        let (verdict, runs, cells) = match run.mode {
            Mode::Baseline => (Verdict::Baseline, 1, Value::Null),
            Mode::Guardrail => self.guardrail_verdict(&run.samples),
            Mode::WorkerOff => (Verdict::Control, 1, guardrail(&run.samples).1),
        };
        summary["verdict"] = json!(verdict.name());
        summary["runs"] = json!(runs);
        summary["ratio_cells"] = cells;
        summary["run_record"] = json!(run_path.display().to_string());
        let path = self.dir.join(format!("{}.json", self.commit));
        write_json(&path, &summary);
        Report { path, summary }
    }

    /// One run decides unless INCONCLUSIVE; then the pooled samples of every
    /// guardrail run on this commit decide once there are four (one plus three).
    fn guardrail_verdict(&self, latest: &Samples) -> (Verdict, usize, Value) {
        let (verdict, cells) = guardrail(latest);
        if verdict != Verdict::Inconclusive {
            return (verdict, 1, cells);
        }
        let runs = self.guardrail_runs();
        if runs.len() < POOLED_RUNS {
            return (verdict, runs.len(), cells);
        }
        let (pooled, pooled_cells) = guardrail(&Samples::pooled(&runs));
        (pooled, runs.len(), pooled_cells)
    }

    fn guardrail_runs(&self) -> Vec<Samples> {
        let Ok(entries) = std::fs::read_dir(self.dir.join(&self.commit)) else {
            return Vec::new();
        };
        entries
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("guardrail-")
            })
            .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
            .filter_map(|text| serde_json::from_str::<Value>(&text).ok())
            .map(|record| Samples::from_json(&record["samples_ms"]))
            .collect()
    }
}

fn write_json(path: &PathBuf, value: &Value) {
    let text = serde_json::to_string_pretty(value).expect("serialise bench json");
    std::fs::write(path, text).unwrap_or_else(|err| panic!("write {}: {err}", path.display()));
}

fn command_output(program: &str, args: &[&str]) -> String {
    std::process::Command::new(program)
        .args(args)
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_default()
}

/// `sysctl` on the Mac; a Linux guest answers none of these names.
fn machine() -> Value {
    let or_else = |answer: String, fallback: String| {
        if answer.is_empty() {
            fallback
        } else {
            answer
        }
    };
    json!({
        "model": or_else(
            command_output("sysctl", &["-n", "hw.model"]),
            command_output("uname", &["-srm"]),
        ),
        "cores": std::thread::available_parallelism().map_or(0, std::num::NonZero::get),
        "load_average": or_else(
            command_output("sysctl", &["-n", "vm.loadavg"]),
            std::fs::read_to_string("/proc/loadavg")
                .unwrap_or_default()
                .trim()
                .to_string(),
        ),
    })
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}
