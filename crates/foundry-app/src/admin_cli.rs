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
use std::str::FromStr;

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
                 -e POSTGRES_PASSWORD=postgres postgres:16-alpine` then export \
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

/// Slice 7 (ADR-016 / D5 = C) — entry point invoked from `main.rs`
/// when the CLI sees `foundry doctor restore-comment <uuid>`.
///
/// Restores a soft-deleted comment by clearing `deleted_at` +
/// `deleted_by`. Operates against the LIVE production database via
/// `DATABASE_URL` (NOT `FOUNDRY_DOCTOR_PROBE_URL` — `backup-verify`
/// restores into a sandbox, this command modifies production).
///
/// Exit codes (per D6 = A consolidated):
///   0 = restored        — UPDATE matched 1 row; `deleted_at` now NULL.
///   2 = invalid UUID    — argument did not parse as a UUID.
///   3 = DB connect fail — DATABASE_URL unreachable or auth failure.
///   4 = not restorable  — UPDATE matched 0 rows (comment not found OR
///                          comment exists but `deleted_at IS NULL`).
///
/// Stdout / stderr distinguish "not found" vs "not tombstoned"
/// diagnostically (so the operator log shows WHICH branch happened),
/// but the EXIT CODE collapses both into 4 — operationally identical
/// ("the UPDATE matched zero rows").
pub fn run_restore_comment(comment_id: &str) -> i32 {
    // (1) Parse the UUID. Exit 2 on malformed input.
    let uuid = match uuid::Uuid::from_str(comment_id) {
        Ok(u) => u,
        Err(err) => {
            eprintln!("foundry doctor restore-comment: invalid UUID {comment_id:?}: {err}");
            return 2;
        }
    };

    // (2) Acquire DATABASE_URL from env. Required for the live DB
    // connection; no default (production operators set it via .env
    // or pod env).
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!(
                "foundry doctor restore-comment: DATABASE_URL is required \
                 to reach the live database. Set it to the same value the \
                 foundry server uses (e.g. postgres://foundry:...@host:5432/foundry)."
            );
            return 3;
        }
    };

    // (3) Build a fresh tokio runtime in a SEPARATE thread so we are
    // not nested inside the outer `#[tokio::main]` runtime that
    // `dispatch_subcommand` runs under. (Nesting `block_on` inside a
    // running runtime panics with "Cannot start a runtime from within
    // a runtime".) The thread-isolated runtime exits when the closure
    // returns.
    //
    // backup-verify avoids this by using std::process::Command and
    // never touching sqlx; restore-comment uses sqlx so we need a
    // tokio context. The std::thread + new_current_thread runtime
    // pair keeps the operator-facing semantics synchronous (the
    // dispatch site still gets back an i32 exit code).
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(err) => {
                eprintln!("foundry doctor restore-comment: could not build tokio runtime: {err}");
                return 3;
            }
        };

        runtime.block_on(async move {
            // (4) Connect to the live DB. Failures here (auth, network,
            // wrong port) map to exit 3.
            let store = match foundry_store::Store::connect(&database_url).await {
                Ok(s) => s,
                Err(err) => {
                    eprintln!(
                        "foundry doctor restore-comment: could not connect to \
                     DATABASE_URL: {err}"
                    );
                    return 3;
                }
            };

            // (5) Run the UPDATE. Returns rows_affected (0 or 1). On a
            // sqlx error mid-UPDATE we surface it as exit 3 (the live DB
            // was reachable to connect but the query itself failed —
            // grouping with "DB-side failure").
            match store.undelete_comment(uuid).await {
                Ok(1) => {
                    println!("comment-id: {uuid}");
                    println!("status: restored");
                    0
                }
                Ok(0) => {
                    // Diagnostic distinguishes "not found" vs "not
                    // tombstoned" via a SELECT round-trip; exit code is
                    // still 4 in both cases per D6 = A. The SELECT is
                    // best-effort — if it fails we still report exit 4
                    // (the primary UPDATE result is the contract).
                    let pool = store.pool();
                    let exists: Result<(bool,), _> =
                        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM comments WHERE id = $1)")
                            .bind(uuid)
                            .fetch_one(pool)
                            .await;
                    eprintln!("comment-id: {uuid}");
                    match exists {
                        Ok((true,)) => {
                            eprintln!(
                                "foundry doctor restore-comment: comment {uuid} is \
                             not currently tombstoned — status: not restorable"
                            );
                        }
                        Ok((false,)) => {
                            eprintln!(
                                "foundry doctor restore-comment: comment {uuid} not \
                             in database — status: not restorable"
                            );
                        }
                        Err(_) => {
                            eprintln!(
                                "foundry doctor restore-comment: UPDATE matched 0 \
                             rows — status: not restorable"
                            );
                        }
                    }
                    4
                }
                Ok(other) => {
                    // Should be unreachable (UPDATE ... WHERE id = $1 can
                    // only affect 0 or 1 rows because `id` is the PK), but
                    // we degrade safely rather than panic on a corrupted DB.
                    eprintln!(
                        "foundry doctor restore-comment: unexpected rows_affected = \
                     {other} for comment {uuid}; status: not restorable"
                    );
                    4
                }
                Err(err) => {
                    eprintln!(
                        "foundry doctor restore-comment: UPDATE against live DB \
                     failed: {err}"
                    );
                    3
                }
            }
        })
    })
    .join()
    .unwrap_or_else(|_| {
        eprintln!(
            "foundry doctor restore-comment: worker thread panicked; \
             see stderr above"
        );
        3
    })
}

/// multi-workspace-provisioning (US-MWT07, ADR-002/003) — entry point invoked
/// from `main.rs` when the CLI sees
/// `foundry doctor provision-workspace --name <name> --admin-email <addr>
/// [--as <super-admin-email>]`.
///
/// CLI-FIRST provisioning surface (ADR-002 / D2). Resolves + verifies the
/// acting super-admin via `is_instance_admin` (FAIL-CLOSED), then atomically
/// creates a NEW workspace + its first admin (mirroring the shipped
/// `create_initial_workspace` seeding tx) and prints the new workspace identity
/// plus a first-admin invite link. Operates against the LIVE database via
/// `DATABASE_URL`, reusing the `run_restore_comment` scaffold (thread-isolated
/// tokio runtime, live DB via the service seam, structured exit codes).
///
/// Exit codes (mirroring `run_restore_comment`'s exit-code discipline):
///
/// - `0` provisioned: workspace + first admin created; stdout reports them.
/// - `2` invalid args: missing `--name`/`--admin-email`, or no acting
///   super-admin resolvable (`--as` required for v1).
/// - `3` DB / infra fail: DATABASE_URL unreachable, SESSION_SECRET unset, or a
///   DB-side failure mid-provision.
/// - `4` not authorized: the acting user is NOT an instance super-admin. The
///   refusal is observationally independent of whether the target already exists.
pub fn run_provision_workspace(name: &str, admin_email: &str, acting_email: &str) -> i32 {
    if name.is_empty() || admin_email.is_empty() || acting_email.is_empty() {
        eprintln!(
            "foundry doctor provision-workspace: --name, --admin-email and --as are required. \
             Usage: foundry doctor provision-workspace --name <name> \
             --admin-email <addr> --as <super-admin-email>"
        );
        return 2;
    }

    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!(
                "foundry doctor provision-workspace: DATABASE_URL is required \
                 to reach the live database. Set it to the same value the \
                 foundry server uses."
            );
            return 3;
        }
    };
    let session_secret = match std::env::var("SESSION_SECRET") {
        Ok(v) if v.len() >= 32 => v,
        _ => {
            eprintln!(
                "foundry doctor provision-workspace: SESSION_SECRET (>= 32 bytes) is \
                 required to sign the first-admin invite link. Set it to the same \
                 value the foundry server uses."
            );
            return 3;
        }
    };
    let public_url =
        std::env::var("FOUNDRY_PUBLIC_URL").unwrap_or_else(|_| "http://localhost".into());

    let name = name.to_string();
    let admin_email = admin_email.to_string();
    let acting_email = acting_email.to_string();

    // Thread-isolated runtime (see `run_restore_comment` for why): we are
    // dispatched from inside the outer `#[tokio::main]` runtime, so nesting a
    // `block_on` would panic.
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(err) => {
                eprintln!(
                    "foundry doctor provision-workspace: could not build tokio runtime: {err}"
                );
                return 3;
            }
        };

        runtime.block_on(async move {
            let store = match foundry_store::Store::connect(&database_url).await {
                Ok(s) => s,
                Err(err) => {
                    eprintln!(
                        "foundry doctor provision-workspace: could not connect to \
                         DATABASE_URL: {err}"
                    );
                    return 3;
                }
            };

            // Resolve the acting super-admin by email. An unresolvable actor is
            // NOT authorized (exit 4) — the same fail-closed refusal a known
            // non-super-admin gets, so it leaks no existence oracle.
            let acting_user_id = match store
                .user_id_by_email(&acting_email.to_ascii_lowercase())
                .await
            {
                Ok(Some(id)) => id,
                Ok(None) => {
                    eprintln!(
                        "foundry doctor provision-workspace: not authorized — status: refused"
                    );
                    return 4;
                }
                Err(err) => {
                    eprintln!(
                        "foundry doctor provision-workspace: failed to resolve acting \
                         operator against live DB: {err}"
                    );
                    return 3;
                }
            };

            let services = foundry_services::Services::new(std::sync::Arc::new(store));
            let now = time::OffsetDateTime::now_utc();
            let request = foundry_services::provisioning::ProvisionRequest {
                acting_user_id,
                workspace_name: &name,
                admin_email: &admin_email,
                admin_password: secrecy::SecretString::new(generate_provisioning_password().into()),
                invite_expires_at: now + time::Duration::days(7),
            };

            match services.provision_workspace(request).await {
                Ok(provisioned) => {
                    let secret = secrecy::SecretString::new(session_secret.into());
                    let invite_url = match foundry_auth::InviteToken::new(
                        provisioned.invite_id,
                        provisioned.invite_expires_at,
                        &secret,
                    ) {
                        Ok(token) => format!(
                            "{}/invites/accept?id={}&sig={}",
                            public_url.trim_end_matches('/'),
                            provisioned.invite_id,
                            urlencoding::encode(&token.signature),
                        ),
                        Err(err) => {
                            eprintln!(
                                "foundry doctor provision-workspace: workspace was \
                                 provisioned but signing the invite link failed: {err}"
                            );
                            return 3;
                        }
                    };
                    println!("workspace-id: {}", provisioned.workspace_id);
                    println!("workspace-name: {name}");
                    println!("first-admin: {admin_email}");
                    println!("invite-link: {invite_url}");
                    println!("status: provisioned");
                    0
                }
                Err(foundry_services::ServiceError::Forbidden) => {
                    // FAIL-CLOSED: the acting user is not an instance super-admin.
                    // The refusal carries no oracle for whether the target exists.
                    eprintln!(
                        "foundry doctor provision-workspace: not authorized — status: refused"
                    );
                    4
                }
                Err(err) => {
                    eprintln!(
                        "foundry doctor provision-workspace: provisioning failed \
                         against live DB: {err}"
                    );
                    3
                }
            }
        })
    })
    .join()
    .unwrap_or_else(|_| {
        eprintln!(
            "foundry doctor provision-workspace: worker thread panicked; \
             see stderr above"
        );
        3
    })
}

/// multi-workspace-provisioning (US-MWT07, ADR-001 / D1) — entry point invoked
/// from `main.rs` when the CLI sees
/// `foundry doctor grant-super-admin --email <operator>`.
///
/// The UPGRADE path: an existing single-workspace install (workspace + admin, but
/// no super-admin yet — i.e. NOT created via the new bootstrap seed) grants its
/// first instance super-admin, who can then provision. Resolves the operator by
/// email to a `users` row, then records the grant via the idempotent
/// `grant_instance_admin` store fn (`INSERT … ON CONFLICT DO NOTHING`), so a
/// second grant for the same operator is a no-op. Reachable ONLY from the operator
/// CLI, never the bearer API. Operates against the LIVE DB via `DATABASE_URL`,
/// reusing the `run_restore_comment` scaffold (thread-isolated tokio runtime,
/// structured exit codes).
///
/// Exit codes (mirroring `run_provision_workspace`'s discipline):
///
/// - `0` granted: the operator is recorded as an instance super-admin (idempotent
///   — a re-grant of an already-super-admin operator also exits 0).
/// - `2` invalid args: missing `--email`, or no `users` row matches the operator
///   (you cannot grant a non-existent user; the operator must already be a user).
/// - `3` DB / infra fail: DATABASE_URL unreachable, or a DB-side failure mid-grant.
pub fn run_grant_super_admin(operator_email: &str) -> i32 {
    if operator_email.is_empty() {
        eprintln!(
            "foundry doctor grant-super-admin: --email is required. \
             Usage: foundry doctor grant-super-admin --email <operator-email>"
        );
        return 2;
    }

    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!(
                "foundry doctor grant-super-admin: DATABASE_URL is required \
                 to reach the live database. Set it to the same value the \
                 foundry server uses."
            );
            return 3;
        }
    };

    let operator_email = operator_email.to_string();

    // Thread-isolated runtime (see `run_restore_comment` for why): we are
    // dispatched from inside the outer `#[tokio::main]` runtime, so nesting a
    // `block_on` would panic.
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(err) => {
                eprintln!("foundry doctor grant-super-admin: could not build tokio runtime: {err}");
                return 3;
            }
        };

        runtime.block_on(async move {
            let store = match foundry_store::Store::connect(&database_url).await {
                Ok(s) => s,
                Err(err) => {
                    eprintln!(
                        "foundry doctor grant-super-admin: could not connect to \
                         DATABASE_URL: {err}"
                    );
                    return 3;
                }
            };

            // Resolve the operator by email. You cannot grant a user who does not
            // exist — exit 2 (invalid argument), distinct from a DB failure (3).
            let operator_id = match store
                .user_id_by_email(&operator_email.to_ascii_lowercase())
                .await
            {
                Ok(Some(id)) => id,
                Ok(None) => {
                    eprintln!(
                        "foundry doctor grant-super-admin: no user matches {operator_email:?}; \
                         the operator must already be a user of this instance."
                    );
                    return 2;
                }
                Err(err) => {
                    eprintln!(
                        "foundry doctor grant-super-admin: failed to resolve operator \
                         against live DB: {err}"
                    );
                    return 3;
                }
            };

            match store.grant_instance_admin(operator_id).await {
                Ok(()) => {
                    println!("operator: {operator_email}");
                    println!("status: super-admin-granted");
                    0
                }
                Err(err) => {
                    eprintln!(
                        "foundry doctor grant-super-admin: grant against live DB \
                         failed: {err}"
                    );
                    3
                }
            }
        })
    })
    .join()
    .unwrap_or_else(|_| {
        eprintln!(
            "foundry doctor grant-super-admin: worker thread panicked; \
             see stderr above"
        );
        3
    })
}

/// per-workspace-backup (US-PWB-01, AC-01.1, DRIFT-1) — entry point invoked from
/// `main.rs` when the CLI sees `foundry doctor list-workspaces`.
///
/// Prints every workspace's identity — id + name (DRIFT-1: `workspaces` has no
/// `slug` column, so `Store::list_workspaces` returns `(id, name)`) — so the
/// operator can pick a target to feed `export-workspace <id|name> <out>`. Operates
/// against the LIVE DB via `DATABASE_URL`, reusing the `run_export_workspace`
/// scaffold (thread-isolated tokio runtime, structured `key: value` + `status:`
/// stdout).
///
/// Output shape: one `workspace-id: <uuid>` line followed by a `workspace-name:
/// <name>` line per workspace, then a trailing `status: OK`.
///
/// Exit codes (mirroring the export scaffold's discipline):
///
/// - `0` OK: the roster was listed; stdout ends with `status: OK`.
/// - `3` DB unreachable / list-read error.
pub fn run_list_workspaces() -> i32 {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!(
                "foundry doctor list-workspaces: DATABASE_URL is required \
                 to reach the live database. Set it to the same value the \
                 foundry server uses."
            );
            return 3;
        }
    };

    // Thread-isolated runtime (see `run_export_workspace`): dispatched from inside
    // the outer `#[tokio::main]` runtime, so a nested `block_on` would panic.
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(err) => {
                eprintln!("foundry doctor list-workspaces: could not build tokio runtime: {err}");
                return 3;
            }
        };

        runtime.block_on(async move {
            let store = match foundry_store::Store::connect(&database_url).await {
                Ok(s) => s,
                Err(err) => {
                    eprintln!(
                        "foundry doctor list-workspaces: could not connect to \
                         DATABASE_URL: {err}"
                    );
                    return 3;
                }
            };

            let workspaces = match store.list_workspaces().await {
                Ok(w) => w,
                Err(err) => {
                    eprintln!(
                        "foundry doctor list-workspaces: failed to list workspaces \
                         against live DB: {err}"
                    );
                    return 3;
                }
            };

            for (id, name) in &workspaces {
                println!("workspace-id: {id}");
                println!("workspace-name: {name}");
            }
            println!("status: OK");
            0
        })
    })
    .join()
    .unwrap_or_else(|_| {
        eprintln!(
            "foundry doctor list-workspaces: worker thread panicked; \
             see stderr above"
        );
        3
    })
}

/// Entry point invoked from `main.rs` when the CLI sees
/// `foundry doctor list-users`.
///
/// Prints every user on the instance — id, email, display name, and whether
/// they hold an `instance_admins` row — so the operator can pick targets for
/// `reset-password` / `grant-super-admin` without a psql session. Mirrors
/// `list-workspaces`: LIVE DB via `DATABASE_URL`, thread-isolated tokio
/// runtime, structured `key: value` + `status:` stdout.
///
/// Output shape: per user, `user-id:` / `user-email:` / `user-name:` /
/// `super-admin: true|false` lines, then a trailing `status: OK`.
///
/// Exit codes (mirroring `run_list_workspaces`):
///
/// - `0` OK: the roster was listed; stdout ends with `status: OK`.
/// - `3` DB unreachable / list-read error.
pub fn run_list_users() -> i32 {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!(
                "foundry doctor list-users: DATABASE_URL is required \
                 to reach the live database. Set it to the same value the \
                 foundry server uses."
            );
            return 3;
        }
    };

    // Thread-isolated runtime (see `run_restore_comment`): dispatched from inside
    // the outer `#[tokio::main]` runtime, so a nested `block_on` would panic.
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(err) => {
                eprintln!("foundry doctor list-users: could not build tokio runtime: {err}");
                return 3;
            }
        };

        runtime.block_on(async move {
            let store = match foundry_store::Store::connect(&database_url).await {
                Ok(s) => s,
                Err(err) => {
                    eprintln!(
                        "foundry doctor list-users: could not connect to \
                         DATABASE_URL: {err}"
                    );
                    return 3;
                }
            };

            let users = match store.list_users().await {
                Ok(u) => u,
                Err(err) => {
                    eprintln!(
                        "foundry doctor list-users: failed to list users \
                         against live DB: {err}"
                    );
                    return 3;
                }
            };

            for (id, email, name, super_admin) in &users {
                println!("user-id: {id}");
                println!("user-email: {email}");
                println!("user-name: {name}");
                println!("super-admin: {super_admin}");
            }
            println!("status: OK");
            0
        })
    })
    .join()
    .unwrap_or_else(|_| {
        eprintln!(
            "foundry doctor list-users: worker thread panicked; \
             see stderr above"
        );
        3
    })
}

/// per-workspace-backup (US-PWB-01, ADR-002/003/005) — entry point invoked from
/// `main.rs` when the CLI sees `foundry doctor export-workspace <id|name> <out>`.
///
/// Exports ONE workspace's data across the ten `foundry_store::TENANT_TABLES` to a
/// single, portable, verifiable tar archive (`manifest.json` +
/// `tables/<table>.jsonl`, whole-row `to_jsonb` JSONL — the slice-05 idiom). The
/// selector resolves the workspace by its id OR by an exact, case-insensitive
/// name (DRIFT-1: `workspaces` has no `slug` column). Reads the scoped rows in ONE
/// `REPEATABLE READ` snapshot (`Store::export_workspace`, the SINGLE place the
/// scope predicate lives), then writes the archive ATOMICALLY via
/// `<out>.partial` → fsync → rename so a failed export never leaves a
/// complete-looking half-archive (NFR-PWB-ATOM-01). Operates against the LIVE DB
/// via `DATABASE_URL`, reusing the `run_provision_workspace` scaffold
/// (thread-isolated tokio runtime, structured `key: value` + `status:` stdout).
///
/// Exit codes (mirroring the shipped scaffold's discipline):
///
/// - `0` OK: archive written; stdout reports a per-table row count for all ten
///   tenant tables, the at-rest sensitivity note (NFR-PWB-SEC-01), and `status: OK`.
/// - `2` unknown/invalid workspace: the selector matched neither an id nor a name
///   (later step wires the redirect to `list-workspaces`).
/// - `3` DB unreachable / mid-read error.
/// - `5` output-path error (parent missing / unwritable) — fails BEFORE any DB read.
pub fn run_export_workspace(selector: &str, out_path: &str) -> i32 {
    if selector.is_empty() || out_path.is_empty() {
        eprintln!(
            "foundry doctor export-workspace: <selector> and <out-path> are required. \
             Usage: foundry doctor export-workspace <id|name> <out-path>"
        );
        return 2;
    }

    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!(
                "foundry doctor export-workspace: DATABASE_URL is required \
                 to reach the live database. Set it to the same value the \
                 foundry server uses."
            );
            return 3;
        }
    };

    let selector = selector.to_string();
    let out_path = std::path::PathBuf::from(out_path);

    // Pre-flight output-path stage (NFR-PWB-ATOM-01, AC-03.2): verify the destination
    // is writable BEFORE any DB read. An unwritable path (parent directory missing or
    // not writable) fails here with exit 5 — so a path error never reads any tenant
    // data and never leaves a half-written archive. This MUST precede `Store::connect`.
    if let Err(err) = preflight_output_path(&out_path) {
        eprintln!(
            "foundry doctor export-workspace: output path {} is not writable: {err}",
            out_path.display()
        );
        return 5;
    }

    // Thread-isolated runtime (see `run_restore_comment` for why): dispatched from
    // inside the outer `#[tokio::main]` runtime, so a nested `block_on` would panic.
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(err) => {
                eprintln!("foundry doctor export-workspace: could not build tokio runtime: {err}");
                return 3;
            }
        };

        runtime.block_on(async move {
            let store = match foundry_store::Store::connect(&database_url).await {
                Ok(s) => s,
                Err(err) => {
                    eprintln!(
                        "foundry doctor export-workspace: could not connect to \
                         DATABASE_URL: {err}"
                    );
                    return 3;
                }
            };

            // Resolve the selector to a workspace id by id OR case-insensitive name.
            let workspaces = match store.list_workspaces().await {
                Ok(w) => w,
                Err(err) => {
                    eprintln!(
                        "foundry doctor export-workspace: failed to list workspaces \
                         against live DB: {err}"
                    );
                    return 3;
                }
            };
            let selector_lower = selector.to_ascii_lowercase();
            let resolved = workspaces.iter().find(|(id, name)| {
                id.to_string() == selector || name.to_ascii_lowercase() == selector_lower
            });
            let Some((workspace_id, _name)) = resolved else {
                eprintln!(
                    "foundry doctor export-workspace: no workspace matches {selector:?}; \
                     run `foundry doctor list-workspaces` to see each workspace's id and name."
                );
                return 2;
            };
            let workspace_id = *workspace_id;

            let export = match store.export_workspace(workspace_id).await {
                Ok(e) => e,
                Err(err) => {
                    eprintln!(
                        "foundry doctor export-workspace: export read against live DB \
                         failed: {err}"
                    );
                    return 3;
                }
            };

            match write_export_archive(&out_path, &export) {
                Ok(()) => {}
                Err(err) => {
                    eprintln!("foundry doctor export-workspace: failed to write archive: {err}");
                    return 5;
                }
            }

            println!("workspace-id: {}", export.workspace_id);
            println!("workspace-name: {}", export.workspace_name);
            println!("archive: {}", out_path.display());
            println!("row-counts:");
            for (table, count) in export.row_counts() {
                println!("  {table}: {count}");
            }
            // Sole-workspace install (AC-03.4): note that this is the only workspace
            // on the instance, so the operator knows the export is a full-instance
            // snapshot — nothing was left behind.
            if workspaces.len() == 1 {
                println!(
                    "note: this is the only workspace on the instance — \
                     the export is a complete snapshot of all tenant data."
                );
            }
            println!(
                "sensitivity-note: this archive contains users.password_hash and \
                 machine_tokens rows — treat it as sensitive at rest."
            );
            println!("status: OK");
            0
        })
    })
    .join()
    .unwrap_or_else(|_| {
        eprintln!(
            "foundry doctor export-workspace: worker thread panicked; \
             see stderr above"
        );
        3
    })
}

/// `main.rs` when the CLI sees `foundry doctor verify-export <path>`.
///
/// Verifies an exported workspace archive (US-PWB-02, AC-02.2) from the PATH ALONE
/// (NFR-PWB-INT-01): reads the self-describing `manifest.json` header for the
/// declared workspace id + per-table row counts, reads every `tables/<table>.jsonl`,
/// then re-applies the SAME §5 scope predicate the export used — offline, with NO
/// database and NO out-of-band workspace argument — via
/// `foundry_store::verify_workspace_export`. Reports:
///
/// - COMPLETENESS: all ten `foundry_store::TENANT_TABLES` present AND per-table
///   JSONL line count equals the manifest's declared `row_counts` (the exit-4
///   truncation tripwire).
/// - ISOLATION: every archived row resolves to the declared workspace and no row
///   resolves to a sibling; the membership-bounded `users` special case (ADR-001)
///   and the transitive `team_memberships` / `comments` FK cross-checks (DRIFT-2)
///   are applied exactly as §5 defines them.
///
/// Exit codes (architecture.md §9):
///
/// - `0` OK: complete AND isolation-clean; stdout reports both confirmations and
///   `status: OK`.
/// - `4` archive missing / unreadable / truncated / incomplete (completeness fails).
/// - non-zero (`6`) isolation failure: a row resolves to a sibling workspace; the
///   message NAMES the foreign row (the falsifiability crux, AC-02.4).
pub fn run_verify_export(path: &str) -> i32 {
    if path.is_empty() {
        eprintln!(
            "foundry doctor verify-export: <path> is required. \
             Usage: foundry doctor verify-export <archive-path>"
        );
        return 4;
    }

    let archive = match read_archive_contents(Path::new(path)) {
        Ok(a) => a,
        Err(err) => {
            eprintln!(
                "foundry doctor verify-export: could not read archive at {path:?}: {err}. \
                 The archive may be missing, unreadable, truncated, or incomplete — \
                 re-run the export."
            );
            return 4;
        }
    };

    let report = foundry_store::verify_workspace_export(&archive);

    if !report.is_complete() {
        for violation in &report.completeness_violations {
            eprintln!("foundry doctor verify-export: completeness check failed: {violation}");
        }
        eprintln!(
            "foundry doctor verify-export: the archive is truncated or incomplete — \
             re-run the export."
        );
        return 4;
    }

    println!("declared-workspace-id: {}", archive.declared_workspace_id);
    println!(
        "completeness: OK — all {} tenant tables are present with matching row counts",
        foundry_store::TENANT_TABLES.len()
    );

    if !report.is_isolation_clean() {
        for violation in &report.isolation_violations {
            eprintln!("foundry doctor verify-export: isolation check failed: {violation}");
        }
        eprintln!(
            "foundry doctor verify-export: the archive contains a row resolving to a \
             workspace other than the declared one — refusing."
        );
        return 6;
    }

    println!("isolation: OK — every row belongs to the declared workspace");
    println!("isolation: OK — no row references a sibling workspace");
    // Transitive FK-chain confirmations (AC-02.3): report that the chain checks
    // actually ran — team_memberships resolved to their owning workspace THROUGH
    // their team (team_memberships has no direct workspace_id), and comments
    // cross-checked against their issue's owning workspace (the DRIFT-2
    // comment.issue_id -> issues.workspace_id corruption cross-check). The counts
    // are the rows the resolver walked, so this is a genuine confirmation, not a
    // constant string.
    println!(
        "isolation: OK — {} team membership(s) resolved to the declared workspace through their team",
        report.team_memberships_resolved
    );
    println!(
        "isolation: OK — {} comment(s) cross-checked against their issue's owning workspace",
        report.comments_cross_checked
    );
    println!("isolation: OK — every transitively-scoped row belongs to the declared workspace");
    println!("status: OK");
    0
}

/// Read a tar archive at `path` into a [`foundry_store::ArchiveContents`]: parse
/// `manifest.json` for the declared workspace id + per-table declared counts, then
/// parse each `tables/<table>.jsonl` into whole-row JSON. Offline, no DB
/// (NFR-PWB-INT-01). Any missing/unparseable manifest or table is an error the
/// caller maps to exit 4.
fn read_archive_contents(path: &Path) -> std::io::Result<foundry_store::ArchiveContents> {
    use std::io::Read;

    let mut manifest: Option<serde_json::Value> = None;
    let mut table_jsonl: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    let file = std::fs::File::open(path)?;
    let mut archive = tar::Archive::new(file);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let name = entry.path()?.to_string_lossy().into_owned();
        let mut buf = String::new();
        entry.read_to_string(&mut buf)?;
        if name == "manifest.json" {
            manifest = Some(serde_json::from_str(&buf).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("manifest.json: {e}"),
                )
            })?);
        } else if let Some(table) = name
            .strip_prefix("tables/")
            .and_then(|n| n.strip_suffix(".jsonl"))
        {
            table_jsonl.insert(table.to_string(), buf);
        }
    }

    let manifest = manifest.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "archive has no manifest.json entry",
        )
    })?;

    let declared_workspace_id = manifest
        .get("declared_workspace_id")
        .and_then(serde_json::Value::as_str)
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "manifest.json has no valid declared_workspace_id",
            )
        })?;

    let declared_counts = manifest
        .get("row_counts")
        .and_then(serde_json::Value::as_object);

    let mut tables = Vec::with_capacity(foundry_store::TENANT_TABLES.len());
    for table in foundry_store::TENANT_TABLES {
        let Some(jsonl) = table_jsonl.get(table) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("archive is missing tables/{table}.jsonl"),
            ));
        };
        let mut rows = Vec::new();
        for line in jsonl.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(line).map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("tables/{table}.jsonl: {e}"),
                )
            })?;
            rows.push(value);
        }
        // Trust the manifest's declared count when present (the truncation tripwire
        // compares it to the actual line count); fall back to the line count when
        // the manifest omits it (then the count check is a tautology, but the
        // isolation pass still reads every row).
        let declared_count = declared_counts
            .and_then(|m| m.get(table))
            .and_then(serde_json::Value::as_u64)
            .map_or(rows.len(), |c| c as usize);
        tables.push(foundry_store::ArchivedTable {
            name: table.to_string(),
            declared_count,
            rows,
        });
    }

    Ok(foundry_store::ArchiveContents {
        declared_workspace_id,
        tables,
    })
}

/// Pre-flight the export output path (NFR-PWB-ATOM-01, AC-03.2): confirm the archive
/// can be written to `out_path` BEFORE any DB read, so an output-path error fails fast
/// (exit 5) without reading any tenant data or leaving a half-written archive. Checks
/// that the parent directory exists and is writable by creating + removing the
/// `<out>.partial` file the atomic writer will use — the cheapest faithful probe of
/// "can the atomic write start here". Returns the underlying I/O error on failure.
fn preflight_output_path(out_path: &Path) -> std::io::Result<()> {
    let partial = out_path.with_extension("partial");
    // Creating the `.partial` probe exercises the exact path the atomic writer opens:
    // a missing parent directory or an unwritable parent fails here, before any DB read.
    std::fs::File::create(&partial)?;
    // Discard the probe so a successful pre-flight leaves the destination pristine for
    // the real atomic write. A best-effort remove is sufficient — the writer recreates it.
    let _ = std::fs::remove_file(&partial);
    Ok(())
}

/// Write a [`foundry_store::WorkspaceExport`] to a single tar archive at `out_path`
/// ATOMICALLY (NFR-PWB-ATOM-01): build the tar at `<out>.partial`, fsync it, then
/// rename into place. A failure mid-write leaves at most a discardable `.partial`
/// file — never a complete-looking archive at the final path. The archive holds
/// `manifest.json` (the self-describing header verify reads first) + one
/// `tables/<table>.jsonl` per tenant table (whole-row JSONL).
fn write_export_archive(
    out_path: &Path,
    export: &foundry_store::WorkspaceExport,
) -> std::io::Result<()> {
    let manifest = serde_json::json!({
        "format_version": 1,
        "declared_workspace_id": export.workspace_id.to_string(),
        "declared_workspace_name": export.workspace_name,
        "exported_at": time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
        "tenant_tables": foundry_store::TENANT_TABLES,
        "row_counts": export
            .row_counts()
            .into_iter()
            .map(|(table, count)| (table, serde_json::Value::from(count)))
            .collect::<serde_json::Map<String, serde_json::Value>>(),
    });
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;

    let partial = out_path.with_extension("partial");
    {
        let file = std::fs::File::create(&partial)?;
        let mut builder = tar::Builder::new(file);

        append_tar_entry(&mut builder, "manifest.json", &manifest_bytes)?;
        for (table, rows) in &export.tables {
            let mut jsonl = String::new();
            for row in rows {
                jsonl.push_str(row);
                jsonl.push('\n');
            }
            append_tar_entry(
                &mut builder,
                &format!("tables/{table}.jsonl"),
                jsonl.as_bytes(),
            )?;
        }

        // Finish the tar (writes the end-of-archive marker), then fsync so the
        // bytes are durable before the atomic rename publishes the final path.
        let file = builder.into_inner()?;
        file.sync_all()?;
    }
    std::fs::rename(&partial, out_path)?;
    Ok(())
}

/// Append one in-memory blob to a tar archive under `name`.
fn append_tar_entry<W: std::io::Write>(
    builder: &mut tar::Builder<W>,
    name: &str,
    bytes: &[u8],
) -> std::io::Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o600);
    header.set_cksum();
    builder.append_data(&mut header, name, bytes)
}

/// Entry point invoked from `main.rs` when the CLI sees
/// `foundry doctor reset-password --email <addr> [--password <new>]`.
///
/// FORCE-resets a user's password: no current-password reauthentication, no
/// reset email — the operator's recovery path when SMTP is unconfigured or the
/// account is simply locked out. Resolves the user by email, hashes the new
/// credential with the SAME argon2 path sign-in verifies against, and writes it
/// via the shipped `update_user_password`. When `--password` is omitted a
/// 32-hex credential is generated and PRINTED — the only copy that will ever
/// exist. A provided `--password` must satisfy the shipped min-12 policy
/// (`foundry_auth::check_password_policy`), the same bar the in-app
/// change-password flow enforces. Operates against the LIVE DB via
/// `DATABASE_URL`, reusing the `run_restore_comment` scaffold (thread-isolated
/// tokio runtime, structured exit codes).
///
/// Exit codes (mirroring the doctor discipline):
///
/// - `0` reset: stdout carries `user: <email>`, `password: <new>` (only when
///   generated), `status: password-reset`.
/// - `2` invalid args: missing `--email`, or a provided `--password` that
///   fails the password policy.
/// - `3` DB / infra fail: DATABASE_URL unreachable, hashing failure, or a
///   DB-side failure mid-update.
/// - `4` no such user: no `users` row matches the email.
pub fn run_reset_password(email: &str, password: Option<String>) -> i32 {
    if email.is_empty() {
        eprintln!(
            "foundry doctor reset-password: --email is required. \
             Usage: foundry doctor reset-password --email <addr> [--password <new>]"
        );
        return 2;
    }

    // Policy-check a provided password BEFORE touching the DB; generate
    // otherwise (32 hex chars ≈ 128 bits, comfortably past the min-12 bar).
    let (password, generated) = match password {
        Some(p) => {
            let candidate = secrecy::SecretString::new(p.clone().into());
            if let Err(err) = foundry_auth::check_password_policy(&candidate) {
                eprintln!("foundry doctor reset-password: refusing weak password: {err}");
                return 2;
            }
            (p, false)
        }
        None => (generate_provisioning_password(), true),
    };

    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!(
                "foundry doctor reset-password: DATABASE_URL is required \
                 to reach the live database. Set it to the same value the \
                 foundry server uses."
            );
            return 3;
        }
    };

    let email = email.to_string();

    // Thread-isolated runtime (see `run_restore_comment` for why): we are
    // dispatched from inside the outer `#[tokio::main]` runtime, so nesting a
    // `block_on` would panic.
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(err) => {
                eprintln!("foundry doctor reset-password: could not build tokio runtime: {err}");
                return 3;
            }
        };

        runtime.block_on(async move {
            let store = match foundry_store::Store::connect(&database_url).await {
                Ok(s) => s,
                Err(err) => {
                    eprintln!(
                        "foundry doctor reset-password: could not connect to \
                         DATABASE_URL: {err}"
                    );
                    return 3;
                }
            };

            let user_id = match store.user_id_by_email(&email.to_ascii_lowercase()).await {
                Ok(Some(id)) => id,
                Ok(None) => {
                    eprintln!(
                        "foundry doctor reset-password: no user matches {email:?} — \
                         status: not-found"
                    );
                    return 4;
                }
                Err(err) => {
                    eprintln!(
                        "foundry doctor reset-password: failed to resolve user \
                         against live DB: {err}"
                    );
                    return 3;
                }
            };

            let secret = secrecy::SecretString::new(password.clone().into());
            let hash = match foundry_auth::hash_password(&secret).await {
                Ok(h) => h,
                Err(err) => {
                    eprintln!("foundry doctor reset-password: hashing failed: {err}");
                    return 3;
                }
            };

            match store.update_user_password(user_id, &hash).await {
                Ok(1) => {
                    println!("user: {email}");
                    if generated {
                        println!("password: {password}");
                    }
                    println!("status: password-reset");
                    0
                }
                Ok(_) => {
                    // The id was resolved a moment ago; a zero-row UPDATE means
                    // the row vanished mid-flight. Operationally "not found".
                    eprintln!(
                        "foundry doctor reset-password: user disappeared before the \
                         update — status: not-found"
                    );
                    4
                }
                Err(err) => {
                    eprintln!(
                        "foundry doctor reset-password: UPDATE against live DB \
                         failed: {err}"
                    );
                    3
                }
            }
        })
    })
    .join()
    .unwrap_or_else(|_| {
        eprintln!(
            "foundry doctor reset-password: worker thread panicked; \
             see stderr above"
        );
        3
    })
}

/// Entry point invoked from `main.rs` when the CLI sees
/// `foundry doctor add-test-user --email <addr> [--password <new>] [--name <display>]`.
///
/// Creates (or tops up) a TEST user who is a `member` of EVERY workspace and
/// EVERY team — board access to every project in the instance, since board
/// reads require team membership. Deliberately NOT an instance super-admin
/// (use `grant-super-admin` separately if the test needs the admin surface).
///
/// Idempotent by design: the membership sweep is `ON CONFLICT DO NOTHING`
/// (`Store::grant_all_memberships`), so RERUN THE COMMAND after creating new
/// workspaces/teams to top up the delta — existing memberships (and their
/// roles) are never touched. If the email already names a user, no credential
/// is minted or changed (use `reset-password` for that); only the sweep runs.
///
/// Exit codes:
///
/// - `0` OK: stdout carries `user: <email>`, `created: true|false`,
///   `password: <new>` (only when a NEW user's credential was generated),
///   `workspaces-added: <n>`, `teams-added: <n>`, `status: OK`.
/// - `2` invalid args: missing `--email`, or a provided `--password` failing
///   the policy, or a `--name` outside the 1..=64 display-name bound.
/// - `3` DB / infra fail: DATABASE_URL unreachable, hashing failure, or a
///   DB-side failure mid-write.
pub fn run_add_test_user(email: &str, password: Option<String>, display_name: &str) -> i32 {
    if email.is_empty() {
        eprintln!(
            "foundry doctor add-test-user: --email is required. \
             Usage: foundry doctor add-test-user --email <addr> \
             [--password <new>] [--name <display>]"
        );
        return 2;
    }
    if display_name.is_empty() || display_name.chars().count() > 64 {
        eprintln!(
            "foundry doctor add-test-user: --name must be 1..=64 characters \
             (the users.display_name bound)."
        );
        return 2;
    }

    let (password, generated) = match password {
        Some(p) => {
            let candidate = secrecy::SecretString::new(p.clone().into());
            if let Err(err) = foundry_auth::check_password_policy(&candidate) {
                eprintln!("foundry doctor add-test-user: refusing weak password: {err}");
                return 2;
            }
            (p, false)
        }
        None => (generate_provisioning_password(), true),
    };

    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!(
                "foundry doctor add-test-user: DATABASE_URL is required \
                 to reach the live database. Set it to the same value the \
                 foundry server uses."
            );
            return 3;
        }
    };

    let email = email.to_string();
    let display_name = display_name.to_string();

    // Thread-isolated runtime (see `run_restore_comment` for why).
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(err) => {
                eprintln!("foundry doctor add-test-user: could not build tokio runtime: {err}");
                return 3;
            }
        };

        runtime.block_on(async move {
            let store = match foundry_store::Store::connect(&database_url).await {
                Ok(s) => s,
                Err(err) => {
                    eprintln!(
                        "foundry doctor add-test-user: could not connect to \
                         DATABASE_URL: {err}"
                    );
                    return 3;
                }
            };

            let email_lower = email.to_ascii_lowercase();
            let (user_id, created) = match store.user_id_by_email(&email_lower).await {
                Ok(Some(id)) => {
                    println!(
                        "note: {email} already exists — password unchanged \
                         (use `foundry doctor reset-password` to change it); \
                         topping up memberships only."
                    );
                    (id, false)
                }
                Ok(None) => {
                    let id = uuid::Uuid::now_v7();
                    let secret = secrecy::SecretString::new(password.clone().into());
                    let hash = match foundry_auth::hash_password(&secret).await {
                        Ok(h) => h,
                        Err(err) => {
                            eprintln!("foundry doctor add-test-user: hashing failed: {err}");
                            return 3;
                        }
                    };
                    if let Err(err) = store
                        .create_user(id, &email_lower, &email, &display_name, &hash)
                        .await
                    {
                        eprintln!(
                            "foundry doctor add-test-user: creating the user \
                             against live DB failed: {err}"
                        );
                        return 3;
                    }
                    (id, true)
                }
                Err(err) => {
                    eprintln!(
                        "foundry doctor add-test-user: failed to resolve email \
                         against live DB: {err}"
                    );
                    return 3;
                }
            };

            match store.grant_all_memberships(user_id).await {
                Ok((ws_added, teams_added)) => {
                    println!("user: {email}");
                    println!("created: {created}");
                    if created && generated {
                        println!("password: {password}");
                    }
                    println!("workspaces-added: {ws_added}");
                    println!("teams-added: {teams_added}");
                    println!("status: OK");
                    0
                }
                Err(err) => {
                    eprintln!(
                        "foundry doctor add-test-user: membership sweep against \
                         live DB failed: {err}"
                    );
                    3
                }
            }
        })
    })
    .join()
    .unwrap_or_else(|_| {
        eprintln!(
            "foundry doctor add-test-user: worker thread panicked; \
             see stderr above"
        );
        3
    })
}

/// Generate a high-entropy initial credential for a provisioned first admin.
/// The operator never sees this; the first admin resets it by accepting the
/// emitted invite link. 32 hex chars ≈ 128 bits of entropy.
fn generate_provisioning_password() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
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
