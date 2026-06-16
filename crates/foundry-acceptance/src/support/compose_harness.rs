//! docker-compose harness for US-01.
//!
//! The harness drives the real `docker compose` CLI rather than
//! testcontainers-rs because US-01's contract IS the compose file
//! itself: scenario 3 inspects `docker-compose.yml`, scenarios 1 and
//! 2 exercise the operator's actual `docker compose up -d` flow.
//!
//! Each [`ComposeStack`] writes a unique COMPOSE_PROJECT_NAME so
//! concurrent scenarios cannot collide. [`Drop`] tears the stack
//! down even when a scenario panics.
//!
//! Image lifecycle: the compose file builds the `foundry` service from
//! source. Left to compose's defaults, each unique project name would mint
//! (and leak) its own `<project>-foundry` image. Instead every stack points
//! the compose file's `${FOUNDRY_IMAGE}` at the single [`SHARED_IMAGE`] tag,
//! so the image is built ONCE (see [`build_shared_image`]) and reused across
//! all scenarios, then removed ONCE at suite end (see [`remove_shared_image`]).

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Single shared tag for the foundry service image across the whole
/// acceptance suite. Every [`ComposeStack`] exports this as `FOUNDRY_IMAGE`,
/// so compose builds/reuses exactly one image regardless of the per-scenario
/// project name — instead of leaving a `<project>-foundry` image behind for
/// each scenario. Built up front by [`build_shared_image`] and torn down by
/// [`remove_shared_image`].
pub const SHARED_IMAGE: &str = "foundry-acceptance:latest";

/// Filesystem location of the workspace root (= the directory that
/// owns `docker-compose.yml`). Computed at compile time from this
/// file's path so it works regardless of `cargo test` cwd.
fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR for foundry-acceptance is .../crates/foundry-acceptance.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent() // .../crates
        .and_then(|p| p.parent()) // workspace root
        .expect("workspace root above crates/foundry-acceptance")
        .to_path_buf()
}

#[derive(Debug)]
pub struct ComposeStack {
    project_name: String,
    root: PathBuf,
    /// Captured logs from the first `up`. Re-read on demand.
    pub initial_bootstrap_lines: Vec<String>,
    /// Count of new `[BOOTSTRAP]` lines observed since the second `up`.
    pub second_run_new_bootstrap_lines: Option<usize>,
}

impl Default for ComposeStack {
    fn default() -> Self {
        Self::new()
    }
}

impl ComposeStack {
    /// Provision a fresh project name. Does NOT call `up` yet.
    pub fn new() -> Self {
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        Self {
            project_name: format!("foundry-at-{}", &suffix[..8]),
            root: workspace_root(),
            initial_bootstrap_lines: Vec::new(),
            second_run_new_bootstrap_lines: None,
        }
    }

    /// Build a `docker compose -p <project>` command, with the env vars
    /// that the compose file expects. `FOUNDRY_HOST_PORT=0` asks docker
    /// for an ephemeral host port per stack so concurrent scenarios don't
    /// collide; `wait_for_foundry_healthy` discovers the assigned port via
    /// `docker compose port`.
    fn compose(&self) -> Command {
        let mut cmd = Command::new("docker");
        cmd.current_dir(&self.root)
            .env("FOUNDRY_HOST_PORT", "0")
            // Pin every stack to the one shared image tag so compose reuses a
            // single pre-built image instead of building a `<project>-foundry`
            // image per scenario.
            .env("FOUNDRY_IMAGE", SHARED_IMAGE)
            .arg("compose")
            .arg("-p")
            .arg(&self.project_name);
        cmd
    }

    /// `docker compose up -d --wait`. Returns when the command exits;
    /// the operator-facing healthcheck is waited on separately.
    pub fn up_detached(&self) -> anyhow::Result<()> {
        let status = self
            .compose()
            .args(["up", "-d", "--wait"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            anyhow::bail!("docker compose up failed (status {status})");
        }
        Ok(())
    }

    /// Wait (up to `timeout`) for `/healthz` on the foundry service to
    /// return 200. Polls every 500ms.
    ///
    /// Async — cucumber-rs steps run inside the tokio runtime, so the
    /// previous `reqwest::blocking` + `thread::sleep` version panicked
    /// with "Cannot start a runtime from within a runtime" on the second
    /// poll attempt.
    pub async fn wait_for_foundry_healthy(&self, timeout: Duration) -> anyhow::Result<()> {
        let port = self.host_port_for("foundry", 3000)?;
        let url = format!("http://127.0.0.1:{port}/healthz");
        let deadline = Instant::now() + timeout;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()?;
        while Instant::now() < deadline {
            if let Ok(resp) = client.get(&url).send().await {
                if resp.status().is_success() {
                    return Ok(());
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        anyhow::bail!("/healthz did not return 200 within {timeout:?}")
    }

    /// Look up the host-side port that the compose stack bound for
    /// `service`'s `container_port`. Uses `docker compose port`.
    pub fn host_port_for(&self, service: &str, container_port: u16) -> anyhow::Result<u16> {
        let out = self
            .compose()
            .args(["port", service, &container_port.to_string()])
            .output()?;
        if !out.status.success() {
            anyhow::bail!(
                "docker compose port failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
        // Format: "0.0.0.0:54321" or "[::]:54321"
        let port = raw
            .rsplit(':')
            .next()
            .and_then(|p| p.parse::<u16>().ok())
            .ok_or_else(|| anyhow::anyhow!("could not parse port from {raw:?}"))?;
        Ok(port)
    }

    /// Confirm the named compose service reports `running` AND its
    /// healthcheck (if declared) is `healthy`.
    pub fn assert_service_healthy(&self, service: &str) -> anyhow::Result<()> {
        // `docker compose ps --format json` returns one JSON object per line.
        let out = self.compose().args(["ps", "--format", "json"]).output()?;
        if !out.status.success() {
            anyhow::bail!(
                "docker compose ps failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
            let value: serde_json::Value = serde_json::from_str(line)?;
            let svc = value.get("Service").and_then(|v| v.as_str()).unwrap_or("");
            if svc != service {
                continue;
            }
            let state = value.get("State").and_then(|v| v.as_str()).unwrap_or("");
            let health = value.get("Health").and_then(|v| v.as_str()).unwrap_or("");
            if state == "running" && (health == "healthy" || health.is_empty()) {
                return Ok(());
            }
            anyhow::bail!("service {service} not healthy (state={state}, health={health})");
        }
        anyhow::bail!("service {service} not present in `compose ps` output");
    }

    /// All log lines from `service` (stdout+stderr, since stack start).
    pub fn logs_for(&self, service: &str) -> anyhow::Result<String> {
        let out = self
            .compose()
            .args(["logs", "--no-color", service])
            .output()?;
        if !out.status.success() {
            anyhow::bail!(
                "docker compose logs failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
        s.push_str(&String::from_utf8_lossy(&out.stderr));
        Ok(s)
    }

    /// Lines in the foundry service log that begin with `[BOOTSTRAP]`.
    pub fn bootstrap_lines(&self) -> anyhow::Result<Vec<String>> {
        let logs = self.logs_for("foundry")?;
        Ok(logs
            .lines()
            .filter(|l| l.contains("[BOOTSTRAP]"))
            .map(|l| l.to_string())
            .collect())
    }

    /// Pre-claim the admin: insert a fake workspace row directly into
    /// Postgres so that the next `up` sees the instance as already
    /// claimed. The bootstrap-token row is not needed; the
    /// `mint_bootstrap_if_needed` predicate keys off `workspaces`.
    ///
    /// Uses `docker compose exec postgres psql` so postgres does not
    /// need a host-port mapping (matching the operator default where
    /// postgres is reachable only via the docker network), and so this
    /// function stays synchronous — earlier versions opened a nested
    /// tokio runtime via sqlx and panicked inside cucumber-rs.
    pub fn pre_claim_admin(&self) -> anyhow::Result<()> {
        let sql = format!(
            "INSERT INTO workspaces (id, name) VALUES (gen_random_uuid(), 'pre-claimed-{}');",
            self.project_name
        );
        let out = self
            .compose()
            .args([
                "exec",
                "-T",
                "postgres",
                "psql",
                "-U",
                "foundry",
                "-d",
                "foundry",
                "-v",
                "ON_ERROR_STOP=1",
                "-c",
                &sql,
            ])
            .output()?;
        if !out.status.success() {
            anyhow::bail!(
                "psql pre-claim insert failed (status {}): stderr={}",
                out.status,
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(())
    }

    /// Restart a single service. Used by US-01 scenario 2 to drive
    /// the startup hook a second time without dropping the database.
    pub fn restart_service(&self, service: &str) -> anyhow::Result<()> {
        let status = self
            .compose()
            .args(["restart", service])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            anyhow::bail!("docker compose restart {service} failed (status {status})");
        }
        Ok(())
    }

    /// Tear the stack down (containers + named volumes).
    pub fn down(&self) {
        let _ = self
            .compose()
            .args(["down", "-v", "--remove-orphans"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

impl Drop for ComposeStack {
    fn drop(&mut self) {
        self.down();
    }
}

/// Read the compose file as text. Used by scenario 3 (volume-shape
/// inspection) — that scenario does NOT spin up the stack.
pub fn read_compose_yml() -> anyhow::Result<String> {
    let path = workspace_root().join("docker-compose.yml");
    Ok(std::fs::read_to_string(path)?)
}

/// Build the foundry service image ONCE, tagged [`SHARED_IMAGE`], before any
/// scenario runs. Every later `docker compose up` then reuses this cached
/// image instead of rebuilding, so the suite produces exactly one image to
/// clean up rather than one per scenario (and concurrent scenarios don't race
/// to build the same tag). The project name here is throwaway — the built tag
/// is fixed by the compose file's `image: ${FOUNDRY_IMAGE}` field.
///
/// Call this from the test runner's setup for any lane that includes the
/// `@docker-compose` scenarios; lanes that exclude them never touch Docker.
pub fn build_shared_image() -> anyhow::Result<()> {
    let status = Command::new("docker")
        .current_dir(workspace_root())
        .env("FOUNDRY_HOST_PORT", "0")
        .env("FOUNDRY_IMAGE", SHARED_IMAGE)
        .args([
            "compose",
            "-p",
            "foundry-acceptance-build",
            "build",
            "foundry",
        ])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    if !status.success() {
        anyhow::bail!("docker compose build foundry failed (status {status})");
    }
    Ok(())
}

/// Remove the [`SHARED_IMAGE`] built by [`build_shared_image`] once the suite
/// has finished. Best-effort: errors are ignored (the image may never have
/// been built because no `@docker-compose` scenario ran). Call this from the
/// test runner's teardown after the run completes.
pub fn remove_shared_image() {
    let _ = Command::new("docker")
        .args(["image", "rm", "-f", SHARED_IMAGE])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}
