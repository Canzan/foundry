//! Regression test for `Store::probe()` scoping its migration-0006 column
//! check to `current_schema()`.
//!
//! The probe asserts the `comments` table carries the slice-5 migration-0006
//! columns (`updated_at`/`deleted_at`/`deleted_by`). Originally the count query
//! filtered only on `table_name = 'comments'`, with NO `table_schema` filter —
//! so it summed matching columns across *every* schema the role can see. A
//! half-migrated ACTIVE schema would then pass the probe whenever any sibling
//! schema (another tenant, or a per-scenario test schema) still carried the
//! columns, masking the substrate lie the probe exists to catch.
//!
//! This test pins the fix: with a sibling schema that HAS the columns and the
//! active schema MISSING them, the probe MUST fail. Remove the
//! `WHERE table_schema = current_schema()` clause and the sibling's columns
//! mask the gap → the probe wrongly passes → this test fails. (That clause is a
//! SQL string literal, so cargo-mutants can't mutate it; this deterministic
//! cross-schema test is its regression guard instead.)

use foundry_store::Store;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Connection, PgPool};
use std::str::FromStr;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ContainerAsync;
use testcontainers_modules::testcontainers::ImageExt;

/// A pool whose connections pin `search_path` to `schema`, so `current_schema()`
/// resolves to it — mirroring how the app pins the per-deployment schema.
async fn pool_on_schema(base: &str, schema: &str) -> PgPool {
    let opts = PgConnectOptions::from_str(base)
        .expect("parse base url")
        .options([("search_path", schema)]);
    PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(opts)
        .await
        .expect("connect per-schema pool")
}

/// A fresh Postgres matching production (docker-compose / k8s); returns its base
/// URL and the container guard.
async fn postgres() -> (String, ContainerAsync<Postgres>) {
    let container = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let host = container.get_host().await.expect("container host");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("container port");
    (
        format!("postgres://postgres:postgres@{host}:{port}/postgres"),
        container,
    )
}

/// Run `stmts` on an admin connection (schema setup outside any pinned search_path).
async fn setup(base: &str, stmts: &[&str]) {
    let mut admin = sqlx::PgConnection::connect(base)
        .await
        .expect("admin connect");
    for stmt in stmts {
        sqlx::query(stmt)
            .execute(&mut admin)
            .await
            .unwrap_or_else(|e| panic!("setup `{stmt}`: {e}"));
    }
}

#[tokio::test]
async fn probe_column_check_is_scoped_to_current_schema() {
    let (base, _pg) = postgres().await;

    // Two schemas: `sibling` HAS the migration-0006 columns; `active` has a
    // `comments` table MISSING them. (We create the minimal tables the probe
    // inspects rather than running full migrations — the probe only reads
    // information_schema for those three column names.)
    setup(
        &base,
        &[
            "CREATE SCHEMA active",
            "CREATE SCHEMA sibling",
            "CREATE TABLE sibling.comments (id uuid PRIMARY KEY, updated_at timestamptz, \
         deleted_at timestamptz, deleted_by uuid)",
            "CREATE TABLE active.comments (id uuid PRIMARY KEY)",
            // The probe also asserts migration 0007's machine_tokens columns
            // (scoped to current_schema()). Give the HEALTHY `sibling` schema the
            // table so its 0006-scoping assertion still reaches the PASS branch;
            // the `active` schema deliberately omits it (it's already failing the
            // 0006 check, which fires first).
            "CREATE TABLE sibling.machine_tokens (jti uuid PRIMARY KEY, user_id uuid, \
         workspace_id uuid, scope_team_id uuid, expires_at timestamptz, \
         revoked_at timestamptz, last_used_at timestamptz, label text, \
         created_by uuid)",
            // The probe also asserts migration 0017's users.provisioned_at
            // (keycloak-sso DDD-32); the healthy schema carries it.
            "CREATE TABLE sibling.users (id uuid PRIMARY KEY, password_hash text, \
         provisioned_at timestamptz)",
            // ...and migration 0018's workspace_rename_events (DDD-11).
            "CREATE TABLE sibling.workspace_rename_events (id uuid PRIMARY KEY, \
         workspace_id uuid, actor_id uuid, old_name text, new_name text, \
         created_at timestamptz)",
            // ...and migration 0022's workspaces.default_team_id (DDD-15).
            "CREATE TABLE sibling.workspaces (id uuid PRIMARY KEY, default_team_id uuid)",
            // ...and migration 0023's team_lifecycle_events (team-lifecycle DDD-23).
            "CREATE TABLE sibling.team_lifecycle_events (id uuid PRIMARY KEY, \
         workspace_id uuid, team_id uuid, team_slug text, kind text, actor_id uuid, \
         name text, new_name text)",
            // ...and migration 0025's project_lifecycle_events (project-lifecycle DDD-28).
            "CREATE TABLE sibling.project_lifecycle_events (id uuid PRIMARY KEY, \
         workspace_id uuid, project_id uuid, team_id uuid, project_slug text, \
         key_prefix text, kind text, actor_id uuid, name text, new_name text, \
         created_at timestamptz)",
            // ...and migration 0026's deleted marker and live_projects view
            // (project-lifecycle DDD-28).
            "CREATE TABLE sibling.projects (id uuid PRIMARY KEY, deleted_at timestamptz, \
         deleted_by uuid)",
            "CREATE VIEW sibling.live_projects AS SELECT * FROM sibling.projects \
         WHERE deleted_at IS NULL",
        ],
    )
    .await;

    // ACTIVE schema lacks the 0006 columns → probe MUST fail, even though the
    // `sibling` schema has them. Unscoped, the sibling's columns make the count
    // reach 3 and the probe wrongly passes — that regression is what this guards.
    let active = Store::from_pool(pool_on_schema(&base, "active").await);
    let err = active.probe().await.expect_err(
        "probe must FAIL when the ACTIVE schema lacks the migration-0006 columns; \
         a sibling schema having them must not mask the gap",
    );
    let msg = err.to_string();
    assert!(
        msg.contains("migration-0006"),
        "expected a missing-0006-columns probe error, got: {msg}"
    );

    // Healthy side: the `sibling` schema HAS all three columns → probe passes.
    // (Confirms the negative case above is the scoping, not a broken setup.)
    let healthy = Store::from_pool(pool_on_schema(&base, "sibling").await);
    healthy
        .probe()
        .await
        .expect("probe must PASS when the active schema has the migration-0006 columns");
}

/// keycloak-sso DDD-32: a binary that reads `users.provisioned_at` (both sign-in
/// doors do, through `UserRow.provisioned`) refuses readiness against a pre-0017
/// schema instead of serving 500s at both doors. The `pre_0017` schema carries
/// everything the earlier checks want and a `users` table WITHOUT the marker;
/// the `current` schema is the same plus the marker and passes.
#[tokio::test]
async fn probe_refuses_a_schema_without_the_provisioned_marker() {
    let (base, _pg) = postgres().await;
    let mut stmts = vec!["CREATE SCHEMA pre_0017", "CREATE SCHEMA current"];
    let earlier = [
        "CREATE TABLE {s}.comments (id uuid PRIMARY KEY, updated_at timestamptz, \
         deleted_at timestamptz, deleted_by uuid)",
        "CREATE TABLE {s}.machine_tokens (jti uuid PRIMARY KEY, user_id uuid, \
         workspace_id uuid, scope_team_id uuid, expires_at timestamptz, \
         revoked_at timestamptz, created_by uuid)",
    ];
    let owned: Vec<String> = ["pre_0017", "current"]
        .iter()
        .flat_map(|s| earlier.iter().map(move |t| t.replace("{s}", s)))
        .chain([
            "CREATE TABLE pre_0017.users (id uuid PRIMARY KEY, password_hash text)".to_string(),
            "CREATE TABLE current.users (id uuid PRIMARY KEY, password_hash text, \
             provisioned_at timestamptz)"
                .to_string(),
            "CREATE TABLE current.workspace_rename_events (id uuid PRIMARY KEY, \
             workspace_id uuid, actor_id uuid, old_name text, new_name text, \
             created_at timestamptz)"
                .to_string(),
            "CREATE TABLE current.workspaces (id uuid PRIMARY KEY, default_team_id uuid)"
                .to_string(),
            "CREATE TABLE current.team_lifecycle_events (id uuid PRIMARY KEY, \
             workspace_id uuid, team_id uuid, team_slug text, kind text, actor_id uuid, \
             name text, new_name text)"
                .to_string(),
            "CREATE TABLE current.project_lifecycle_events (id uuid PRIMARY KEY, \
             workspace_id uuid, project_id uuid, team_id uuid, project_slug text, \
             key_prefix text, kind text, actor_id uuid, name text, new_name text, \
             created_at timestamptz)"
                .to_string(),
            "CREATE TABLE current.projects (id uuid PRIMARY KEY, deleted_at timestamptz, \
             deleted_by uuid)"
                .to_string(),
            "CREATE VIEW current.live_projects AS SELECT * FROM current.projects \
             WHERE deleted_at IS NULL"
                .to_string(),
        ])
        .collect();
    stmts.extend(owned.iter().map(String::as_str));
    setup(&base, &stmts).await;

    let stale = Store::from_pool(pool_on_schema(&base, "pre_0017").await);
    let msg = stale
        .probe()
        .await
        .expect_err("probe must FAIL against a schema lacking users.provisioned_at")
        .to_string();
    assert!(
        msg.contains("migration-0017"),
        "expected a missing-0017-column probe error, got: {msg}"
    );

    Store::from_pool(pool_on_schema(&base, "current").await)
        .probe()
        .await
        .expect("probe must PASS when the active schema has users.provisioned_at");
}

// ---------------------------------------------------------------------------
// project-lifecycle (DDD-28): one sibling-schema test per new probe line.
// Authored RED in DISTILL 2026-10-13; each is gated on its migration file and
// asserts that a SIBLING schema holding the new object does not satisfy the
// probe for the ACTIVE schema that lacks it, and that the healthy schema passes.
// DELIVER: the new probe lines also make the two tests above need the new
// objects in their healthy schemas (as 0018, 0022 and 0023 did).
// ---------------------------------------------------------------------------

fn migration_exists(file: &str) -> bool {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("migrations")
        .join(file)
        .exists()
}

/// Every object the probe asked for up to 0024, for schema `s`.
fn through_0024(s: &str) -> Vec<String> {
    [
        "CREATE TABLE {s}.comments (id uuid PRIMARY KEY, updated_at timestamptz, \
         deleted_at timestamptz, deleted_by uuid)",
        "CREATE TABLE {s}.machine_tokens (jti uuid PRIMARY KEY, user_id uuid, \
         workspace_id uuid, scope_team_id uuid, expires_at timestamptz, \
         revoked_at timestamptz, last_used_at timestamptz, label text, created_by uuid)",
        "CREATE TABLE {s}.users (id uuid PRIMARY KEY, password_hash text, \
         provisioned_at timestamptz)",
        "CREATE TABLE {s}.workspace_rename_events (id uuid PRIMARY KEY, \
         workspace_id uuid, actor_id uuid, old_name text, new_name text, \
         created_at timestamptz)",
        "CREATE TABLE {s}.workspaces (id uuid PRIMARY KEY, default_team_id uuid)",
        "CREATE TABLE {s}.team_lifecycle_events (id uuid PRIMARY KEY, \
         workspace_id uuid, team_id uuid, team_slug text, kind text, actor_id uuid, \
         name text, new_name text)",
    ]
    .iter()
    .map(|t| t.replace("{s}", s))
    .collect()
}

/// 0025's `project_lifecycle_events` (ADR-PROJECT-LIFECYCLE-003: 11 columns).
fn records_0025(s: &str) -> String {
    format!(
        "CREATE TABLE {s}.project_lifecycle_events (id uuid PRIMARY KEY, \
         workspace_id uuid, project_id uuid, team_id uuid, project_slug text, \
         key_prefix text, kind text, actor_id uuid, name text, new_name text, \
         created_at timestamptz)"
    )
}

/// 0026's marker columns and `live_projects` view (and 0027's detach column
/// when `detach`), over a minimal `projects` table.
fn projects_0026(s: &str, marker: bool, detach: bool) -> Vec<String> {
    let mut cols = String::from(
        "id uuid PRIMARY KEY, team_id uuid, workspace_id uuid, name text, slug text, \
         key_prefix text, next_issue_number integer",
    );
    if marker {
        cols.push_str(", deleted_at timestamptz, deleted_by uuid");
    }
    if detach {
        cols.push_str(", detached_from_team_id uuid");
    }
    let mut out = vec![format!("CREATE TABLE {s}.projects ({cols})")];
    if marker {
        out.push(format!(
            "CREATE VIEW {s}.live_projects AS SELECT * FROM {s}.projects WHERE deleted_at IS NULL"
        ));
    }
    out
}

async fn probe_message(base: &str, schema: &str) -> Result<(), String> {
    Store::from_pool(pool_on_schema(base, schema).await)
        .probe()
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tokio::test]
async fn probe_project_lifecycle_events_check_is_scoped_to_current_schema() {
    assert!(
        migration_exists("0025_project_lifecycle_events.sql"),
        "RED gate: migration 0025 does not exist yet (project-lifecycle DDD-13, DDD-28)"
    );
    let (base, _pg) = postgres().await;
    let mut stmts: Vec<String> = vec![
        "CREATE SCHEMA active".into(),
        "CREATE SCHEMA sibling".into(),
    ];
    stmts.extend(through_0024("active"));
    stmts.extend(through_0024("sibling"));
    stmts.push(records_0025("sibling"));
    stmts.extend(projects_0026("sibling", true, false));
    let refs: Vec<&str> = stmts.iter().map(String::as_str).collect();
    setup(&base, &refs).await;
    let msg = probe_message(&base, "active")
        .await
        .expect_err("probe must FAIL when the ACTIVE schema lacks project_lifecycle_events");
    assert!(
        msg.contains("0025"),
        "expected a missing-0025 probe error, got: {msg}"
    );
    probe_message(&base, "sibling")
        .await
        .expect("probe must PASS when the active schema has project_lifecycle_events");
}

#[tokio::test]
async fn probe_deleted_marker_and_live_view_check_is_scoped_to_current_schema() {
    assert!(
        migration_exists("0026_project_soft_delete.sql"),
        "RED gate: migration 0026 does not exist yet (project-lifecycle DDD-2, DDD-3, DDD-28)"
    );
    let (base, _pg) = postgres().await;
    let mut stmts: Vec<String> = vec![
        "CREATE SCHEMA active".into(),
        "CREATE SCHEMA sibling".into(),
    ];
    for s in ["active", "sibling"] {
        stmts.extend(through_0024(s));
        stmts.push(records_0025(s));
    }
    stmts.extend(projects_0026("active", false, false));
    stmts.extend(projects_0026("sibling", true, false));
    let refs: Vec<&str> = stmts.iter().map(String::as_str).collect();
    setup(&base, &refs).await;
    let msg = probe_message(&base, "active").await.expect_err(
        "probe must FAIL when the ACTIVE schema lacks deleted_at, deleted_by and live_projects",
    );
    assert!(
        msg.contains("0026"),
        "expected a missing-0026 probe error, got: {msg}"
    );
    probe_message(&base, "sibling")
        .await
        .expect("probe must PASS when the active schema has the marker and the view");
}

#[tokio::test]
#[ignore = "pending: project-lifecycle 04-01 — the probe's detached_from_team_id line is scoped to current_schema"]
async fn probe_detach_column_check_is_scoped_to_current_schema() {
    assert!(
        migration_exists("0027_project_detach.sql"),
        "RED gate: migration 0027 does not exist yet (project-lifecycle DDD-19, DDD-28)"
    );
    let (base, _pg) = postgres().await;
    let mut stmts: Vec<String> = vec![
        "CREATE SCHEMA active".into(),
        "CREATE SCHEMA sibling".into(),
    ];
    for s in ["active", "sibling"] {
        stmts.extend(through_0024(s));
        stmts.push(records_0025(s));
    }
    stmts.extend(projects_0026("active", true, false));
    stmts.extend(projects_0026("sibling", true, true));
    let refs: Vec<&str> = stmts.iter().map(String::as_str).collect();
    setup(&base, &refs).await;
    let msg = probe_message(&base, "active")
        .await
        .expect_err("probe must FAIL when the ACTIVE schema lacks detached_from_team_id");
    assert!(
        msg.contains("0027"),
        "expected a missing-0027 probe error, got: {msg}"
    );
    probe_message(&base, "sibling")
        .await
        .expect("probe must PASS when the active schema has the detach column");
}
