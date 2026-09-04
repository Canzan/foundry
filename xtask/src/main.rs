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

/// One gate in a `ci` / `smoke` run: `(label, cargo-args, env)`.
///
/// The env rides WITH the step (keyboard-shortcut-bindings, ADR-007). `run_steps`
/// previously derived it from a LABEL SUBSTRING (`label.contains("foundry-acceptance")`),
/// which cannot distinguish a SECOND acceptance step — the trap that had to be
/// fixed before the @needs-browser lane could be added beside the existing one.
type Gate<'a> = (&'a str, Vec<&'a str>, Vec<(&'a str, &'a str)>);

fn main() -> ExitCode {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    match cmd.as_str() {
        "" | "help" | "--help" | "-h" => {
            usage();
            ExitCode::SUCCESS
        }
        "ci" => run_ci(),
        "smoke" => run_smoke(),
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
    println!("  ci          Run the full local CI replication (the pre-PUSH gate)");
    println!("  smoke       Fast pre-COMMIT subset: fmt, clippy, boundary guard,");
    println!("              workspace unit/integration tests (no acceptance/deny)");
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

    // Preflight 1 — `.env`. The @docker-compose acceptance group runs
    // `docker compose up`, which reads `.env` as its env-file; without it
    // compose aborts with "env file .env not found". CI does `cp .env.example
    // .env`; mirror that locally so a fresh checkout's `cargo xtask ci` is not
    // a divergence from CI. Only copies when `.env` is MISSING — never clobbers
    // a developer's existing file.
    if include_docker {
        if let Err(err) = ensure_env_file() {
            eprintln!("xtask ci :: could not prepare .env: {err}");
            return ExitCode::from(1);
        }
    }

    // Preflight 2 — RETIRED. The US-03 backup lane used to shell out to the
    // HOST's pg_dump/pg_restore, so this refused to start when they were
    // missing or older than the server. They now run from the pinned
    // `postgres:16-alpine` client image instead (see
    // foundry-acceptance/src/support/pg_backup.rs), which removes the
    // contributor prerequisite AND the whole class of version skew: the
    // client image tag is the same one the server containers use, so a
    // Homebrew Postgres 14 on PATH can no longer refuse a 16.14 server. The
    // docker-daemon check that replaces it is the `include_docker` detection
    // above, which already gates this entire group.

    // Preflight 3 — chromedriver + a MAJOR-VERSION-MATCHED browser. The
    // @needs-browser acceptance lane (keyboard-shortcut-bindings, ADR-007) drives
    // a real headless Chrome against InProcHarness's real origin; chromedriver
    // REFUSES a browser whose major differs ("session not created: This version of
    // ChromeDriver only supports Chrome version N"), which a mere presence check
    // would miss — a `brew upgrade` that moves one and not the other is the usual
    // cause. This PROBES, then REFUSES: it never soft-skips. A browser lane that
    // silently skips is indistinguishable from the bug the lane exists to prevent
    // (a green suite over an absent capability — exactly how seven advertised
    // shortcuts shipped unbound). Same contract pg_dump already has.
    if include_docker {
        if let Err(msg) = chromedriver_matches_browser() {
            eprintln!(
                "xtask ci :: a version-matched chromedriver + Chrome is required for the \
                 @needs-browser lane: {msg}"
            );
            eprintln!(
                "  install it:  macOS -> `brew install --cask chromedriver` \
                 (and keep Chrome on the same major);  Debian/Ubuntu -> \
                 `sudo apt-get install -y chromium-driver chromium`"
            );
            eprintln!(
                "  the lane is NOT optional and is NOT skipped: it runs in `all`, because a gate \
                 that is green without ever pressing a key is the state this feature exists to \
                 make impossible (ADR-007 §4)."
            );
            return ExitCode::from(1);
        }
    }

    let sqlx_offline_cache_present = std::path::Path::new(".sqlx").is_dir();

    let mut steps: Vec<Gate<'_>> = vec![
        (
            "cargo fmt --check",
            vec!["fmt", "--all", "--", "--check"],
            vec![],
        ),
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
            vec![],
        ),
        // US-W06 boundary guard — the AST source-walk layer + the cargo-deny
        // crate-graph dep-direction layer. Cheap (no DB, pure source +
        // crate-graph analysis); runs alongside fmt/clippy so a local
        // `cargo xtask ci` catches a boundary violation before push.
        (
            "xtask check-arch (boundary guard)",
            vec!["run", "-q", "-p", "xtask", "--", "check-arch"],
            vec![],
        ),
        (
            "cargo build --release",
            vec!["build", "--all", "--release"],
            vec![],
        ),
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
            vec![],
        ),
        ("cargo deny check", vec!["deny", "check"], vec![]),
    ];

    // sqlx offline-cache check — mirrors CI's conditional `cargo sqlx prepare
    // --check`. Only meaningful when a `.sqlx/` cache exists (the slice-1
    // binary uses lazy queries, not compile-time `query!`, so there is none
    // yet). Guarded so the gate stays a no-op until compile-time queries land,
    // at which point local and CI verify the cache identically.
    if sqlx_offline_cache_present {
        steps.push((
            "cargo sqlx prepare --workspace --check",
            vec!["sqlx", "prepare", "--workspace", "--check"],
            vec![],
        ));
    }

    if include_docker {
        steps.push((
            "cargo test -p foundry-acceptance (all tags, incl. @docker-compose)",
            vec!["test", "-p", "foundry-acceptance", "--release"],
            // The `all` lane — which INCLUDES @needs-browser (ADR-007 §4).
            // Excluding the browser lane from `all` would rebuild the exact bug
            // this feature closes: a gate that is green without ever pressing a
            // key is precisely the state that let seven advertised shortcuts
            // ship unbound. The preflight above guarantees the driver is there.
            vec![("FOUNDRY_ACCEPTANCE_TAGS", "all")],
        ));
    }

    run_steps("ci", &steps)
}

/// Fast pre-COMMIT smoke: the subset of `ci` that catches the class of failure
/// that must never reach a push — formatting, lints, the web/api boundary
/// guard, and the workspace unit/integration tests (the exact
/// `cargo test --workspace (excl. foundry-acceptance) --release` lane whose red
/// output is the most common avoidable CI break). It SKIPS the heavy tail: the
/// standalone release build (redundant — the test/clippy steps build), the full
/// `@docker-compose` + `@needs-pgclient` acceptance suite, and `cargo deny`.
///
/// This is the tight feedback loop while iterating; the FULL `cargo xtask ci`
/// remains the mandatory pre-PUSH gate (see AGENTS.md). Steps are drawn verbatim
/// from `run_ci`, so smoke can never drift from a strict subset of CI. Like the
/// workspace tests inside `ci`, the test step uses Postgres testcontainers, so a
/// reachable Docker daemon is still required for that step.
fn run_smoke() -> ExitCode {
    let steps: Vec<Gate<'_>> = vec![
        (
            "cargo fmt --check",
            vec!["fmt", "--all", "--", "--check"],
            vec![],
        ),
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
            vec![],
        ),
        (
            "xtask check-arch (boundary guard)",
            vec!["run", "-q", "-p", "xtask", "--", "check-arch"],
            vec![],
        ),
        (
            "cargo test --workspace (excl. foundry-acceptance) --release",
            vec![
                "test",
                "--workspace",
                "--exclude",
                "foundry-acceptance",
                "--release",
            ],
            vec![],
        ),
    ];

    run_steps("smoke", &steps)
}

/// Run an ordered list of `(label, cargo-args, env)` gates, stopping on the first
/// non-zero exit. Shared by `run_ci` and `run_smoke` so the two can never
/// diverge in how a step is launched. `kind` is the log prefix (`ci` / `smoke`).
///
/// Each step carries its OWN env (keyboard-shortcut-bindings, ADR-007). This
/// previously selected the acceptance lane by LABEL SUBSTRING
/// (`label.contains("foundry-acceptance")` => `FOUNDRY_ACCEPTANCE_TAGS=all`),
/// which cannot distinguish a SECOND acceptance step — every acceptance step
/// silently got the same lane. Carrying env per-step also keeps `run_smoke` a
/// strict, drift-proof subset of `run_ci`.
fn run_steps(kind: &str, steps: &[Gate<'_>]) -> ExitCode {
    for (label, args, env_vars) in steps {
        println!("\n=== xtask {kind} :: {label} ===");
        // A few steps shell out to cargo *plugins* (separate binaries). Emit a
        // clear install hint if one isn't on PATH rather than a cryptic exec
        // error — keeps the local gate self-documenting.
        if label.starts_with("cargo deny") && !which("cargo-deny") {
            eprintln!(
                "cargo-deny not found on PATH. Install with: \
                 `cargo install --locked cargo-deny`"
            );
            return ExitCode::from(1);
        }
        if label.starts_with("cargo sqlx") && !which("cargo-sqlx") {
            eprintln!(
                "cargo-sqlx not found on PATH. Install with: \
                 `cargo install --locked sqlx-cli --no-default-features \
                 --features rustls,postgres`"
            );
            return ExitCode::from(1);
        }
        let bin = "cargo";

        let mut cmd = Command::new(bin);
        cmd.args(args);
        for (k, v) in env_vars {
            cmd.env(k, v);
        }
        let status = match cmd.status() {
            Ok(s) => s,
            Err(err) => {
                eprintln!("xtask {kind} :: failed to launch `{bin}`: {err}");
                return ExitCode::from(1);
            }
        };
        if !status.success() {
            eprintln!(
                "\nxtask {kind} :: FAILED at `{label}` with status {}",
                status.code().unwrap_or(-1)
            );
            return ExitCode::from(1);
        }
    }

    println!("\nxtask {kind} :: all gates green");
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

/// Ensure a `.env` exists for the @docker-compose acceptance group, copying
/// `.env.example` into place when it is missing. Never overwrites an existing
/// `.env`. Errors only if `.env` is absent AND `.env.example` cannot be read.
fn ensure_env_file() -> std::io::Result<()> {
    let env = std::path::Path::new(".env");
    if env.exists() {
        return Ok(());
    }
    let example = std::path::Path::new(".env.example");
    if !example.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            ".env is missing and .env.example was not found to seed it",
        ));
    }
    println!("xtask ci :: .env missing, seeding it from .env.example");
    std::fs::copy(example, env).map(|_| ())
}

/// Parse the MAJOR version out of a `--version` line. The version is the first
/// whitespace token STARTING with a digit — do NOT assume it is the last token
/// ("ChromeDriver 150.0.7871.124 (9261fd0a... refs/branch-heads/...)" ends in a
/// build ref, and Homebrew's pg_dump ends in "(Homebrew)"). Mirrors
/// `pg_dump_at_least_16`'s parser in shape.
fn major_version_from(text: &str) -> Option<u32> {
    text.split_whitespace()
        .find(|tok| tok.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .and_then(|v| v.split('.').next())
        .and_then(|m| m.parse::<u32>().ok())
}

/// First `--version` output among `candidates` that runs and exits 0. The browser
/// binary's name varies by platform (google-chrome / chromium / the macOS .app),
/// so probe the known set rather than hardcoding one.
fn first_version_output(candidates: &[&str]) -> Option<(String, String)> {
    for bin in candidates {
        if let Ok(out) = Command::new(bin).arg("--version").output() {
            if out.status.success() {
                return Some((
                    (*bin).to_string(),
                    String::from_utf8_lossy(&out.stdout).to_string(),
                ));
            }
        }
    }
    None
}

/// Verify chromedriver AND a browser are present and their MAJOR versions MATCH
/// (ADR-007 §4). Returns `Err(reason)` describing which half failed — presence is
/// not enough: chromedriver 151 refuses Chrome 150 outright.
fn chromedriver_matches_browser() -> Result<(), String> {
    let out = Command::new("chromedriver")
        .arg("--version")
        .output()
        .map_err(|_| "chromedriver not found on PATH".to_string())?;
    if !out.status.success() {
        return Err("`chromedriver --version` did not succeed".to_string());
    }
    let driver_text = String::from_utf8_lossy(&out.stdout).to_string();
    let driver_major = major_version_from(&driver_text)
        .ok_or_else(|| format!("could not parse chromedriver version from {driver_text:?}"))?;

    let (browser_bin, browser_text) = first_version_output(&[
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
    ])
    .ok_or_else(|| {
        "no Chrome/Chromium browser found (tried google-chrome, google-chrome-stable, chromium, \
         chromium-browser, and the macOS .app bundles)"
            .to_string()
    })?;
    let browser_major = major_version_from(&browser_text)
        .ok_or_else(|| format!("could not parse browser version from {browser_text:?}"))?;

    if driver_major != browser_major {
        return Err(format!(
            "version SKEW: chromedriver is {driver_major}.x but {browser_bin} is \
             {browser_major}.x. chromedriver refuses a browser on a different major, so the lane \
             would die at session creation. Align them (both to {browser_major}.x is usually the \
             quicker fix)."
        ));
    }
    Ok(())
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
