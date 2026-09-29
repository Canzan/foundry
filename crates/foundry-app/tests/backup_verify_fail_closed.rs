//! fix-backup-verify-fail-open: `foundry doctor backup-verify` must never print
//! `status: OK` unless it actually counted rows.
//!
//! Driven through the real `foundry` binary (the CLI is the driving port). The
//! only double is `FOUNDRY_PG_RESTORE`, a stub `pg_restore` that exits 0 and,
//! for `--list`, prints a table of contents naming the schema the probe holds.
//! That lets each test arrange the "restored" schema directly in the probe
//! database and observe what the verifier reports about it.
//!
//! - Unreachable probe: exit 8, the probe named on stderr (hermetic, no Docker).
//! - A present table whose count fails: exit 9, the table named on stderr.
//! - A schema holding none of the known Foundry tables: exit 10.
//! - A healthy schema: counts for present tables only, absent ones skipped,
//!   `status: OK`, exit 0, and the (mixed-case, quoted) schema dropped after.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Output;
use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};

/// A scratch directory holding the stub `pg_restore` and a placeholder dump.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    /// `toc_schema` is the schema the stub's `--list` output names; `None`
    /// means a TOC with no SCHEMA entry (a dump of `public`).
    fn new(label: &str, toc_schema: Option<&str>) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "foundry-backup-verify-{label}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        let toc = toc_schema
            .map(|s| format!("echo '34; 2615 17225 SCHEMA - {s} postgres'\n"))
            .unwrap_or_else(|| ":\n".to_string());
        let stub = dir.join("pg_restore");
        std::fs::write(
            &stub,
            format!("#!/bin/sh\nif [ \"$1\" = \"--list\" ]; then\n{toc}fi\nexit 0\n"),
        )
        .expect("write stub pg_restore");
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))
            .expect("chmod stub");
        std::fs::write(dir.join("foundry.dump"), b"placeholder").expect("write dump");
        Self { dir }
    }

    fn backup_verify(&self, probe_url: &str) -> Output {
        std::process::Command::new(env!("CARGO_BIN_EXE_foundry"))
            .args(["doctor", "backup-verify"])
            .arg(self.dir.join("foundry.dump"))
            .env("FOUNDRY_PG_RESTORE", self.dir.join("pg_restore"))
            .env("FOUNDRY_DOCTOR_PROBE_URL", probe_url)
            .output()
            .expect("run foundry binary")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

struct Probe {
    url: String,
    pool: PgPool,
    _pg: ContainerAsync<Postgres>,
}

impl Probe {
    async fn start() -> Self {
        let pg = Postgres::default()
            .with_tag("16-alpine")
            .start()
            .await
            .expect("start postgres container");
        let host = pg.get_host().await.expect("host");
        let port = pg.get_host_port_ipv4(5432).await.expect("port");
        let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(Duration::from_secs(10))
            .connect(&url)
            .await
            .expect("connect probe");
        Self { url, pool, _pg: pg }
    }

    async fn exec(&self, sql: &str) {
        sqlx::raw_sql(sql)
            .execute(&self.pool)
            .await
            .unwrap_or_else(|e| panic!("setup SQL failed: {e}\n{sql}"));
    }

    async fn schema_exists(&self, schema: &str) -> bool {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = $1)")
            .bind(schema)
            .fetch_one(&self.pool)
            .await
            .expect("query pg_namespace")
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Assert the run exited with `code`; returns its (stdout, stderr).
#[track_caller]
fn assert_exit(out: &Output, code: i32) -> (String, String) {
    let (stdout, stderr) = (text(&out.stdout), text(&out.stderr));
    assert_eq!(
        out.status.code(),
        Some(code),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    (stdout, stderr)
}

/// Assert the run exited with `code` and never claimed `status: OK`;
/// returns its stderr.
#[track_caller]
fn assert_fails_closed(out: &Output, code: i32) -> String {
    let (stdout, stderr) = assert_exit(out, code);
    assert!(
        !stdout.contains("status: OK"),
        "false OK on stdout:\n{stdout}"
    );
    stderr
}

#[test]
fn unreachable_probe_exits_8_naming_the_probe_and_never_reports_ok() {
    let scratch = Scratch::new("unreachable", None);

    let out = scratch.backup_verify("postgres://nobody:x@127.0.0.1:1/nowhere");

    let stderr = assert_fails_closed(&out, 8);
    assert!(
        stderr.contains("FOUNDRY_DOCTOR_PROBE_URL") && stderr.contains("127.0.0.1:1"),
        "stderr must name the probe:\n{stderr}"
    );
}

#[tokio::test]
async fn failing_count_on_a_present_table_exits_9_naming_the_table() {
    let probe = Probe::start().await;
    // `issues` exists (to_regclass resolves it) but counting it raises.
    probe
        .exec(
            "CREATE SCHEMA verify_s9;
             CREATE TABLE verify_s9.workspaces (id int);
             INSERT INTO verify_s9.workspaces VALUES (1);
             CREATE FUNCTION verify_s9.boom() RETURNS SETOF int LANGUAGE plpgsql
               AS $$ BEGIN RAISE EXCEPTION 'count exploded'; END $$;
             CREATE VIEW verify_s9.issues AS SELECT * FROM verify_s9.boom();",
        )
        .await;
    let scratch = Scratch::new("count-fails", Some("verify_s9"));

    let out = scratch.backup_verify(&probe.url);

    let stderr = assert_fails_closed(&out, 9);
    assert!(
        stderr.contains("issues"),
        "stderr must name the table:\n{stderr}"
    );
}

#[tokio::test]
async fn schema_without_any_foundry_table_exits_10_and_keeps_public() {
    let probe = Probe::start().await;
    probe
        .exec("CREATE TABLE public.not_foundry (id int);")
        .await;
    let scratch = Scratch::new("no-tables", None);

    let out = scratch.backup_verify(&probe.url);

    let stderr = assert_fails_closed(&out, 10);
    assert!(
        stderr.contains("not a Foundry backup"),
        "stderr must say why:\n{stderr}"
    );
    assert!(
        probe.schema_exists("public").await,
        "the public schema must never be dropped"
    );
}

#[tokio::test]
async fn healthy_schema_counts_present_tables_skips_absent_and_drops_the_schema() {
    let probe = Probe::start().await;
    // Mixed case: only a quoted identifier reaches it.
    probe
        .exec(
            r#"CREATE SCHEMA "Verify_OK";
               CREATE TABLE "Verify_OK".issues (id int);
               INSERT INTO "Verify_OK".issues VALUES (1), (2), (3), (4);
               CREATE TABLE "Verify_OK".comments (id int);"#,
        )
        .await;
    let scratch = Scratch::new("healthy", Some("Verify_OK"));

    let out = scratch.backup_verify(&probe.url);

    let (stdout, _) = assert_exit(&out, 0);
    assert!(
        stdout.ends_with("row-counts:\n  issues: 4\n  comments: 0\nstatus: OK\n"),
        "unexpected report:\n{stdout}"
    );
    assert!(
        !probe.schema_exists("Verify_OK").await,
        "the restored schema must be dropped after verification"
    );
}
