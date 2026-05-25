//! `foundry doctor` operator subcommands.
//!
//! Currently provides `foundry doctor backup-verify <file>`, the
//! contract pinned by US-03's `@us-03-cli` acceptance scenarios and
//! by `docs/feature/foundry-backend-mvp/design/system/backup-restore.md`.
//!
//! Contract (acceptance-pinned):
//!
//! - Exit 0 on a healthy `pg_dump -Fc` custom-format archive.
//! - Stdout carries one `<table>: <count>` line per Foundry table
//!   present in the dump (e.g. `issues: 4`, `issue_attachments: 2`).
//! - Stdout ends with a `status: OK` line.
//! - Exit non-zero on a truncated / unreadable / corrupt archive.
//! - Stdout or stderr describes the corruption ("truncated", "could
//!   not read", etc.) so the operator's cron job can grep the
//!   diagnostic without re-running the verification.
//!
//! Implementation strategy:
//!
//! 1. Shell out to `pg_restore --list <file>`. A readable archive
//!    exits 0; a truncated / corrupt archive exits non-zero with a
//!    `pg_restore: error: ...` message on stderr. We forward that
//!    message verbatim to the operator and exit non-zero.
//!
//! 2. For row counts, we need an actual SQL probe — `pg_restore
//!    --list` shows the table-of-contents (objects, not row counts).
//!    The operator points the verifier at a temp Postgres via
//!    `FOUNDRY_DOCTOR_PROBE_URL`; we restore the dump into a fresh
//!    schema there, count rows, and drop the schema. The acceptance
//!    harness provides this URL via the per-scenario restore target
//!    (one container shared across the dump-restore step and the CLI
//!    invocation).
//!
//! 3. The list of tables we count is the hard-coded Foundry schema
//!    (workspaces, users, teams, projects, issues, comments,
//!    issue_attachments, sessions, ...). Tables present in the dump
//!    but unknown to the verifier are silently ignored.
//!
//! Production usage:
//!
//! ```sh
//! # docker compose context
//! export FOUNDRY_DOCTOR_PROBE_URL=postgres://postgres:postgres@verify-db:5432/postgres
//! foundry doctor backup-verify /backups/foundry-2026-05-22.dump
//! ```

use std::path::Path;
use std::process::{Command, Stdio};

/// Entry point invoked from `main.rs` when the CLI sees
/// `foundry doctor backup-verify <file>`.
///
/// Returns the desired process exit code. `main.rs` translates that
/// into the actual process exit so the cucumber harness can observe
/// the same number the operator would.
pub fn run_backup_verify(dump_path: &Path) -> i32 {
    if !dump_path.exists() {
        eprintln!(
            "foundry doctor backup-verify: file does not exist: {}",
            dump_path.display()
        );
        return 2;
    }

    println!("backup-file: {}", dump_path.display());

    // Step 1: structural readability via `pg_restore --list`. A
    // truncated dump returns non-zero with a readable diagnostic.
    let list_output = match Command::new("pg_restore")
        .arg("--list")
        .arg(dump_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(o) => o,
        Err(err) => {
            eprintln!(
                "foundry doctor backup-verify: could not invoke pg_restore: {err}. \
                 Ensure the Postgres client tooling is installed (`apt-get install \
                 postgresql-client-16` / `brew install libpq && brew link --force libpq`).",
            );
            return 3;
        }
    };
    if !list_output.status.success() {
        let stderr = String::from_utf8_lossy(&list_output.stderr);
        eprintln!("foundry doctor backup-verify: backup file is unreadable or truncated: {stderr}",);
        return 4;
    }
    println!("backup-format: pg_dump custom");
    if let Ok(meta) = std::fs::metadata(dump_path) {
        println!("backup-size-bytes: {}", meta.len());
    }

    // Step 2: row counts. Need a probe Postgres URL.
    let probe_url = match std::env::var("FOUNDRY_DOCTOR_PROBE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!(
                "foundry doctor backup-verify: FOUNDRY_DOCTOR_PROBE_URL is required \
                 to count rows. Point it at a writable Postgres instance the \
                 verifier can restore into (e.g. `docker run --rm -d -p 5544:5432 \
                 -e POSTGRES_PASSWORD=postgres postgres:11-alpine` then export \
                 FOUNDRY_DOCTOR_PROBE_URL=postgres://postgres:postgres@127.0.0.1:5544/postgres).",
            );
            return 5;
        }
    };

    // pg_restore --clean --if-exists is idempotent so back-to-back
    // invocations against the same probe DB stay green.
    let restore_output = match Command::new("pg_restore")
        .arg("--clean")
        .arg("--if-exists")
        .arg("--no-owner")
        .arg("--no-privileges")
        .arg("-d")
        .arg(&probe_url)
        .arg(dump_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(o) => o,
        Err(err) => {
            eprintln!("foundry doctor backup-verify: pg_restore spawn failed: {err}");
            return 6;
        }
    };
    if !restore_output.status.success() {
        let stderr = String::from_utf8_lossy(&restore_output.stderr);
        eprintln!("foundry doctor backup-verify: pg_restore into probe DB failed: {stderr}",);
        return 7;
    }

    // Step 3: count rows per known Foundry table. The dump preserves
    // the source schema name; we discover it from pg_restore --list
    // output (look for a `SCHEMA - <name>` TOC entry that is NOT
    // `public`). Falling back to `public` for installations that
    // dumped from the default schema.
    let schema_name = parse_schema_name(&String::from_utf8_lossy(&list_output.stdout))
        .unwrap_or_else(|| "public".to_string());

    println!("schema: {schema_name}");
    println!("row-counts:");
    let tables = [
        "workspaces",
        "users",
        "teams",
        "team_memberships",
        "workspace_memberships",
        "projects",
        "issues",
        "comments",
        "issue_attachments",
        "session",
        "outbox",
    ];

    for table in tables {
        match count_rows(&probe_url, &schema_name, table) {
            Ok(n) => println!("  {table}: {n}"),
            Err(_) => {
                // Skip tables not present in this dump (older
                // Foundry versions, optional features).
            }
        }
    }

    // Step 4: drop the per-invocation schema so the probe DB stays
    // reusable across back-to-back verifications. Best-effort.
    let _ = Command::new("psql")
        .arg(&probe_url)
        .arg("-c")
        .arg(format!("DROP SCHEMA IF EXISTS {schema_name} CASCADE"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    println!("status: OK");
    0
}

/// Best-effort parse of a `pg_restore --list` TOC for the schema name
/// that owns the user-defined objects. The TOC carries a line like
/// `34; 2615 17225 SCHEMA - test_s10_c1269542 postgres`; we pick the
/// first non-`public` schema we see. Returns `None` if no SCHEMA TOC
/// entry exists (the dump was created from the default `public`
/// namespace).
fn parse_schema_name(toc: &str) -> Option<String> {
    for line in toc.lines() {
        let trimmed = line.trim_start();
        // Skip comment / header lines (custom-format TOC prefixes
        // them with `;`).
        if trimmed.starts_with(';') {
            continue;
        }
        // Form: `<num>; <oid> <oid> SCHEMA - <schema_name> <owner>`
        let mut parts = trimmed.split_whitespace();
        // Walk forward to find the literal `SCHEMA -` pair.
        let mut prev: Option<&str> = None;
        let mut prev_prev: Option<&str> = None;
        for part in parts.by_ref() {
            if prev_prev == Some("SCHEMA") && prev == Some("-") && part != "public" {
                return Some(part.to_string());
            }
            prev_prev = prev;
            prev = Some(part);
        }
    }
    None
}

/// Run `psql ... -t -A -c "SELECT count(*) FROM <schema>.<table>"`
/// and return the parsed count. Returns an error if the table does
/// not exist (caller swallows so missing tables don't break the run).
fn count_rows(probe_url: &str, schema: &str, table: &str) -> Result<u64, String> {
    let sql = format!("SELECT count(*) FROM \"{schema}\".\"{table}\"");
    let out = Command::new("psql")
        .arg(probe_url)
        .arg("-t") // tuples only
        .arg("-A") // unaligned
        .arg("-c")
        .arg(&sql)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| format!("psql spawn: {err}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.trim()
        .lines()
        .next()
        .and_then(|first| first.trim().parse::<u64>().ok())
        .ok_or_else(|| format!("could not parse count from {text:?}"))
}
