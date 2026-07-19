//! US-03 backup-restore harness — wraps the system `pg_dump` and
//! `pg_restore` binaries plus a per-scenario "restore target"
//! Postgres container.
//!
//! Per `distill/driver.md` §2c and `wave-decisions.md` §US-03:
//!
//! - The system `pg_dump` and `pg_restore` binaries MUST be on PATH;
//!   missing tools panic at first call with a contributor-friendly
//!   message (F-004 anti-flake — no silent skip).
//! - The slice-1 shared Postgres container is the dump SOURCE; the
//!   per-scenario second container is the restore TARGET. Restore is
//!   destructive, so the target cannot be shared across scenarios.
//! - `pg_dump -Fc` produces a custom-format dump file. The `foundry
//!   doctor backup-verify` subcommand also parses this format.
//!
//! Mac+Colima caveat: total containers per US-03 scenario = 1 source
//! (shared) + 1 target. With six US-03 scenarios that is six extra
//! containers over the course of the suite, plus one ephemeral
//! container per `foundry doctor backup-verify` invocation to host the
//! row-count probe. `RestoreTarget` is a cheap clone-able handle (admin
//! URL + serialising mutex); the container itself is process-wide and is
//! removed by [`shutdown_restore_target`] at the end of the run, not at
//! scenario teardown.

use once_cell::sync::OnceCell;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ContainerAsync;
use testcontainers_modules::testcontainers::ImageExt;
use tokio::sync::{Mutex, OnceCell as AsyncOnceCell};

static PG_TOOLS_PROBE: OnceCell<()> = OnceCell::new();

/// Probe `pg_dump --version` and `pg_restore --version` once per
/// process. Panics with a clear contributor-facing message if either
/// is missing. The wave-decisions doc requires no silent skip
/// (F-004 anti-flake); contributors must install the Postgres client
/// tooling to run the US-03 lane.
pub fn probe_pg_tools_on_path() {
    PG_TOOLS_PROBE.get_or_init(|| {
        for tool in ["pg_dump", "pg_restore"] {
            let out = Command::new(tool)
                .arg("--version")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();
            match out {
                Ok(o) if o.status.success() => {}
                Ok(o) => panic!(
                    "US-03 requires `{tool}` on PATH but `{tool} --version` exited \
                     non-zero (status={status}, stderr={stderr}). \
                     Install the Postgres client tooling (macOS: `brew install \
                     libpq && brew link --force libpq`; Debian/Ubuntu: \
                     `apt-get install postgresql-client-16`) or skip the US-03 \
                     lane with `--tags 'not @us-03'`.",
                    tool = tool,
                    status = o.status,
                    stderr = String::from_utf8_lossy(&o.stderr),
                ),
                Err(err) => panic!(
                    "US-03 requires `{tool}` on PATH but spawning it failed: {err}. \
                     Install the Postgres client tooling (macOS: `brew install \
                     libpq && brew link --force libpq`; Debian/Ubuntu: \
                     `apt-get install postgresql-client-16`) or skip the US-03 \
                     lane with `--tags 'not @us-03'`.",
                    tool = tool,
                ),
            }
        }
    });
}

/// Handle returned by [`spawn_restore_target`]. Holds the connection
/// URL pointing at the per-scenario restore target Postgres plus the
/// `restore_mutex` the scenario MUST acquire around every `pg_restore`
/// + assertion sequence — see the wave-decisions note below.
///
/// Wave-decisions evolution (2026-05-24 mid-DELIVER): the original
/// plan was one fresh testcontainers Postgres per scenario, so the
/// destructive `pg_restore --clean` could not corrupt sibling
/// scenarios. Mac+Colima could not sustain that — the daemon ran out
/// of memory and started OOM-killing fresh container boots. Mitigation:
/// share ONE process-wide restore target across US-03 scenarios + a
/// global `restore_mutex` that callers acquire across the restore +
/// assertions so two scenarios never observe each other's data.
/// `--clean --if-exists` makes back-to-back restores idempotent.
#[derive(Clone)]
pub struct RestoreTarget {
    admin_url: String,
    restore_mutex: Arc<Mutex<()>>,
}

impl std::fmt::Debug for RestoreTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RestoreTarget")
            .field("admin_url", &self.admin_url)
            .finish_non_exhaustive()
    }
}

impl RestoreTarget {
    /// Connection URL pointing at the default `postgres` database with
    /// admin credentials. Suitable as the `-d` argument to `pg_restore`.
    pub fn admin_url(&self) -> &str {
        &self.admin_url
    }

    /// Acquire the process-wide restore-serialisation lock. Callers
    /// MUST hold this across the `pg_restore` + subsequent assertion
    /// sequence so two scenarios do not see each other's data.
    pub async fn lock_restore(&self) -> tokio::sync::OwnedMutexGuard<()> {
        self.restore_mutex.clone().lock_owned().await
    }
}

static SHARED_RESTORE_TARGET: AsyncOnceCell<RestoreTarget> = AsyncOnceCell::const_new();
/// Keep-alive slot for the shared restore-target container.
/// `Mutex<Option<_>>` rather than `OnceCell` so
/// [`shutdown_restore_target`] can take ownership and call the consuming
/// `rm()`; the value is never read otherwise.
/// `std::sync::Mutex`, explicitly: the bare `Mutex` in this module is
/// `tokio::sync::Mutex`, whose `new` is not `const` and so cannot
/// initialise a `static`. Nothing here is held across an await.
static SHARED_RESTORE_CONTAINER: std::sync::Mutex<Option<ContainerAsync<Postgres>>> =
    std::sync::Mutex::new(None);

/// Return the process-wide restore target. The container is removed
/// EXPLICITLY by [`shutdown_restore_target`] at the end of
/// `tests/acceptance.rs` main — `Drop` cannot do it, because testcontainers
/// defers removal to an async task and a static drops after the tokio
/// runtime is gone. See `harness`'s module header for the full rationale.
/// Mac+Colima could not sustain a fresh per-scenario second container under
/// the slice-3 load, so we share one across all US-03 scenarios and
/// serialise with `restore_mutex`.
pub async fn spawn_restore_target() -> RestoreTarget {
    SHARED_RESTORE_TARGET
        .get_or_init(|| async {
            let container: ContainerAsync<Postgres> = Postgres::default()
                .with_tag("16-alpine") // match production (see harness::ensure_postgres)
                .start()
                .await
                .expect("spawn US-03 shared restore-target postgres");
            let host = container.get_host().await.expect("restore-target host");
            let port = container
                .get_host_port_ipv4(5432)
                .await
                .expect("restore-target port");
            let admin_url = format!("postgres://postgres:postgres@{host}:{port}/postgres");
            *SHARED_RESTORE_CONTAINER
                .lock()
                .expect("restore container mutex") = Some(container);
            RestoreTarget {
                admin_url,
                restore_mutex: Arc::new(Mutex::new(())),
            }
        })
        .await
        .clone()
}

/// Stop and remove the shared US-03 restore-target container.
///
/// Counterpart to [`harness::shutdown_postgres`]; called from
/// `tests/acceptance.rs` at the end of `main` while the tokio runtime is
/// still alive. Idempotent, and a no-op when no US-03 scenario ran.
pub async fn shutdown_restore_target() {
    let container = SHARED_RESTORE_CONTAINER
        .lock()
        .expect("restore container mutex")
        .take();
    if let Some(container) = container {
        if let Err(e) = container.rm().await {
            eprintln!("warning: failed to remove US-03 restore-target container: {e}");
        }
    }
}

/// Invoke `pg_dump -Fc` against `source_url`, restricting to objects
/// in `schema`, and write the dump bytes to `out_path`. The slice-3
/// harness pins each scenario to its own search-path-scoped schema in
/// the shared source container, so the dump file is scoped to that
/// scenario's rows — sibling scenarios in other schemas are not
/// included.
///
/// Returns the dump file's size in bytes (a sanity-check value the
/// caller can log for diagnostics).
pub async fn dump_schema_to_file(
    source_url: &str,
    schema: &str,
    out_path: &Path,
) -> anyhow::Result<u64> {
    probe_pg_tools_on_path();
    let owned_source = source_url.to_string();
    let owned_schema = schema.to_string();
    let owned_out = out_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let out = Command::new("pg_dump")
            .arg("-Fc")
            .arg("--schema")
            .arg(&owned_schema)
            .arg("--no-owner")
            .arg("--no-privileges")
            .arg("-f")
            .arg(&owned_out)
            .arg(&owned_source)
            // Defensive: a future password/tty prompt must not block on
            // an inherited stdin. The password is in the URL today, so
            // this is permanent hardening, not the current fix.
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|err| anyhow::anyhow!("spawn pg_dump: {err}"))?;
        if !out.status.success() {
            return Err(anyhow::anyhow!(
                "pg_dump failed (status={status}) stderr={stderr}",
                status = out.status,
                stderr = String::from_utf8_lossy(&out.stderr),
            ));
        }
        let size = std::fs::metadata(&owned_out).map(|m| m.len()).unwrap_or(0);
        Ok(size)
    })
    .await
    .map_err(|err| anyhow::anyhow!("dump_schema_to_file join: {err}"))?
}

/// `pg_restore --clean --if-exists` of `dump_file` against
/// `target_url`. The dump file was produced by [`dump_schema_to_file`]
/// and carries schema-qualified DDL.
///
/// We do NOT pass `--no-owner` here — by passing it on `pg_dump` the
/// archive omits ownership clauses entirely, so `pg_restore` happily
/// applies as the postgres superuser. `--clean --if-exists` lets us
/// re-restore an already-populated target idempotently (matters for
/// the "no state outside the DB" scenario which restores into a target
/// that already has the same schema from a prior phase).
pub async fn restore_file_to_schema(target_url: &str, dump_file: &Path) -> anyhow::Result<()> {
    probe_pg_tools_on_path();
    let owned_target = target_url.to_string();
    let owned_file = dump_file.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let out = Command::new("pg_restore")
            .arg("--clean")
            .arg("--if-exists")
            .arg("--no-owner")
            .arg("--no-privileges")
            .arg("-d")
            .arg(&owned_target)
            .arg(&owned_file)
            // Fail-fast lock guard: if a sibling scenario still holds a
            // connection (relation lock) against the shared restore
            // target, the `--clean` DROP TABLE would otherwise block
            // forever. `lock_timeout=30000` makes the blocked statement
            // error ("canceling statement due to lock timeout") after
            // 30s, so the `.expect("pg_restore into target")` in the
            // calling step panics fast and the lane FAILS VISIBLY in
            // ≤30s instead of hanging at 0% CPU. `PGCONNECT_TIMEOUT`
            // bounds a stuck TCP connect for the same fail-fast reason.
            .env("PGOPTIONS", "-c lock_timeout=30000")
            .env("PGCONNECT_TIMEOUT", "30")
            // Defensive: never block on an inherited stdin (see pg_dump).
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|err| anyhow::anyhow!("spawn pg_restore: {err}"))?;
        // pg_restore exits 0 even when individual DROP-IF-EXISTS
        // statements warn about missing objects. We only fail when
        // the binary reports a hard non-zero exit AND stderr does not
        // look like the benign "schema/role does not exist" warning
        // tide. The data round-trip is verified by the calling
        // scenario's assertions, not by parsing pg_restore stderr.
        if !out.status.success() {
            return Err(anyhow::anyhow!(
                "pg_restore failed (status={status}) stderr={stderr}",
                status = out.status,
                stderr = String::from_utf8_lossy(&out.stderr),
            ));
        }
        Ok(())
    })
    .await
    .map_err(|err| anyhow::anyhow!("restore_file_to_schema join: {err}"))?
}

/// Truncate `path` to its first `keep_bytes` bytes. Used by the
/// `@us-03-cli @error` scenario which expects `foundry doctor
/// backup-verify` to fail on a partial / corrupt dump file.
pub fn truncate_dump(path: &Path, keep_bytes: u64) -> anyhow::Result<()> {
    let file = std::fs::OpenOptions::new().write(true).open(path)?;
    file.set_len(keep_bytes)?;
    Ok(())
}

/// Shared scratch dir for dump files produced inside scenarios. We
/// keep one process-wide temp dir (lazily initialised) so dump files
/// have stable paths inside one cargo-test invocation, and the OS
/// reaps the dir at process exit. Per-scenario uniqueness is provided
/// by [`fresh_dump_path`].
pub fn dump_scratch_dir() -> PathBuf {
    static DIR: OnceCell<PathBuf> = OnceCell::new();
    DIR.get_or_init(|| {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "foundry-acceptance-us03-{pid}",
            pid = std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create us03 scratch dir");
        dir
    })
    .clone()
}

/// Allocate a fresh dump-file path inside the scratch dir, keyed by
/// `name` (e.g. the per-scenario schema name). The file does not
/// exist yet; the caller writes it via [`dump_schema_to_file`].
pub fn fresh_dump_path(name: &str) -> PathBuf {
    static COUNTER: AsyncOnceCell<()> = AsyncOnceCell::const_new();
    let _ = &COUNTER;
    let mut p = dump_scratch_dir();
    p.push(format!("{name}.dump"));
    p
}
