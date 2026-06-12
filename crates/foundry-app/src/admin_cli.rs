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
