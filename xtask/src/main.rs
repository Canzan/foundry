//! xtask — developer-only task runner.
//!
//! Subcommands:
//!   `ci`         — runs every gate that CI runs, in the same order, against
//!                  the local checkout. Exits non-zero on the first failure.
//!   `check-arch` — the US-W06 boundary guard (boundary-guard.md): the AST
//!                  source-walk layer (api≠HTML, api≠ad-hoc-authz, JWT alg pin)
//!                  PLUS the `cargo-deny` crate-graph dependency-direction
//!                  layer. Passes on a clean tree with zero manual steps; on a
//!                  violation it NAMES the offender and exits non-zero.
//!   `help`       — list subcommands.

mod check_arch;

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    match cmd.as_str() {
        "" | "help" | "--help" | "-h" => {
            usage();
            ExitCode::SUCCESS
        }
        "ci" => run_ci(),
        "check-arch" => check_arch::run(std::env::args().skip(2).collect()),
        other => {
            eprintln!("unknown subcommand: {other}");
            usage();
            ExitCode::from(2)
        }
    }
}

fn usage() {
    println!("foundry xtask");
    println!();
    println!("Subcommands:");
    println!("  ci          Run the full local CI replication");
    println!("  check-arch  Run the web/api boundary guard (US-W06)");
    println!("  help        Show this message");
}

/// Run every gate the remote CI runs, in roughly the same order, and
/// stop on the first non-zero exit. Mirrors the gates in
/// `.github/workflows/ci.yml` and `CONTRIBUTING.md`.
///
/// The @docker-compose acceptance group adds ~50s to the run and
/// requires a reachable Docker daemon. By default we auto-detect:
/// if `docker info` succeeds, the group runs; otherwise it's skipped
/// with a visible note. The env var `FOUNDRY_XTASK_INCLUDE_DOCKER`
/// forces the decision: `1` / `true` always include, `0` / `false`
/// always skip. CI sets this explicitly; humans don't need to.
fn run_ci() -> ExitCode {
    // Export DOCKER_HOST for child processes if the user is on a non-default
    // socket (Colima / OrbStack / Lima). testcontainers-rs reads DOCKER_HOST
    // directly and does NOT consult `docker context`, so the cargo-test
    // acceptance run fails with "/var/run/docker.sock not found" even when
    // `docker` itself works fine via context. Mirror what the CLI uses.
    if std::env::var_os("DOCKER_HOST").is_none() {
        if let Some(host) = current_docker_context_host() {
            println!("xtask ci :: exporting DOCKER_HOST={host} (from docker context)");
            unsafe {
                std::env::set_var("DOCKER_HOST", host);
            }
        }
    }

    let include_docker = match std::env::var("FOUNDRY_XTASK_INCLUDE_DOCKER")
        .ok()
        .as_deref()
    {
        Some(v) if v == "1" || v.eq_ignore_ascii_case("true") => true,
        Some(v) if v == "0" || v.eq_ignore_ascii_case("false") => false,
        _ => {
            let detected = docker_daemon_reachable();
            if detected {
                println!(
                    "xtask ci :: docker daemon detected, \
                     including @docker-compose acceptance group"
                );
            } else {
                eprintln!(
                    "xtask ci :: docker daemon not reachable, \
                     skipping @docker-compose acceptance group. \
                     Start Colima/OrbStack/Docker Desktop and re-run, \
                     or force-skip with FOUNDRY_XTASK_INCLUDE_DOCKER=0"
                );
            }
            detected
        }
    };

    let mut steps: Vec<(&str, Vec<&str>)> = vec![
        ("cargo fmt --check", vec!["fmt", "--all", "--", "--check"]),
        (
            "cargo clippy",
            vec![
                "clippy",
                "--all-targets",
                "--release",
                "--",
                "-D",
                "warnings",
            ],
        ),
        // US-W06 boundary guard — the AST source-walk layer + the cargo-deny
        // crate-graph dep-direction layer. Cheap (no DB, pure source +
        // crate-graph analysis); runs alongside fmt/clippy so a local
        // `cargo xtask ci` catches a boundary violation before push.
        (
            "xtask check-arch (boundary guard)",
            vec!["run", "-q", "-p", "xtask", "--", "check-arch"],
        ),
        ("cargo build --release", vec!["build", "--all", "--release"]),
        (
            // Exclude foundry-acceptance here: it is a heavy integration
            // suite (Postgres testcontainer + many spawned foundry
            // subprocesses) covered by its OWN dedicated step below with
            // FOUNDRY_ACCEPTANCE_TAGS=all (a superset of the default lane),
            // so running it inside `--workspace` is redundant. Worse, the
            // `--workspace` run executed it CONCURRENTLY with foundry-app's
            // own container tests; the combined memory footprint OOM-killed
            // (SIGKILL) spawned foundry subprocesses ("did not bind both
            // ports within 30s"). Excluding it keeps full coverage (the @all
            // step) while letting the acceptance suite run alone. Safe now
            // that foundry-app self-enables `test-support` in its
            // dev-dependencies (previously it relied on foundry-acceptance
            // to enable it transitively via feature unification).
            "cargo test --workspace (excl. foundry-acceptance) --release",
            vec![
                "test",
                "--workspace",
                "--exclude",
                "foundry-acceptance",
                "--release",
            ],
        ),
        ("cargo deny check", vec!["deny", "check"]),
    ];

    if include_docker {
        steps.push((
            "cargo test -p foundry-acceptance (all tags, incl. @docker-compose)",
            vec!["test", "-p", "foundry-acceptance", "--release"],
        ));
    }

    for (label, args) in &steps {
        println!("\n=== xtask ci :: {label} ===");
        let env_vars: Vec<(&str, &str)> = if label.contains("foundry-acceptance") {
            vec![("FOUNDRY_ACCEPTANCE_TAGS", "all")]
        } else {
            vec![]
        };
        let bin = if label.starts_with("cargo deny") {
            // cargo-deny is a separate binary; emit a clear message if
            // it isn't installed yet rather than a cryptic exec error.
            if !which("cargo-deny") {
                eprintln!(
                    "cargo-deny not found on PATH. Install with: \
                     `cargo install --locked cargo-deny`"
                );
                return ExitCode::from(1);
            }
            "cargo"
        } else {
            "cargo"
        };

        let mut cmd = Command::new(bin);
        cmd.args(args);
        for (k, v) in env_vars {
            cmd.env(k, v);
        }
        let status = match cmd.status() {
            Ok(s) => s,
            Err(err) => {
                eprintln!("xtask ci :: failed to launch `{bin}`: {err}");
                return ExitCode::from(1);
            }
        };
        if !status.success() {
            eprintln!(
                "\nxtask ci :: FAILED at `{label}` with status {}",
                status.code().unwrap_or(-1)
            );
            return ExitCode::from(1);
        }
    }

    println!("\nxtask ci :: all gates green");
    ExitCode::SUCCESS
}

fn which(bin: &str) -> bool {
    Command::new(bin)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// `docker info` round-trips through the daemon, so a 0-exit means
/// the daemon is alive and reachable from this process's environment
/// (DOCKER_HOST, current docker context, etc.). `docker --version`
/// alone would lie: the CLI is installed even when the VM is stopped.
fn docker_daemon_reachable() -> bool {
    Command::new("docker")
        .arg("info")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Ask the docker CLI which socket the current context points at, so
/// we can mirror that into DOCKER_HOST for testcontainers-rs (which
/// only reads the env var). Returns None if docker isn't installed or
/// there's no current context.
fn current_docker_context_host() -> Option<String> {
    let out = Command::new("docker")
        .args([
            "context",
            "inspect",
            "--format",
            "{{.Endpoints.docker.Host}}",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let host = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}
