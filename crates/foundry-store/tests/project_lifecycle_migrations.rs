//! project-lifecycle — migrations 0025 (the lifecycle record), 0026 (the soft
//! delete marker and `live_projects`) and 0027 (the detach), the readiness
//! probe lines and the two upgrade warnings (DESIGN DDD-2, DDD-3, DDD-13,
//! DDD-14, DDD-19, DDD-28, DDD-29; ADR-PROJECT-LIFECYCLE-001 decision 2,
//! -003 decision 1, -004 decisions 1 and 5; DEVOPS "Migrations, probe and
//! upgrade warnings"). DISTILL 2026-10-13.
//!
//! - Each migration is additive: a schema staged at the previous version
//!   holding rows migrates with no existing row touched.
//! - The record's shape CHECK refuses every incoherent kind; the record has no
//!   foreign key to `projects` or `teams`, and cascades with its workspace.
//! - After 0026 every existing project is live (in the view).
//! - 0027 keeps `projects_team_id_fkey` ON DELETE CASCADE.
//! - The probe refuses readiness without each slice's objects (the 0018 and
//!   0023 precedent).
//! - `UPGRADE_WARNING_0025` and `_0026` are logged once by the boot that
//!   upgrades through them, never by a fresh install; 0027 warns nothing.
//!
//! Authored as RED in DISTILL 2026-10-13: ignored as pending, behind the
//! `require_0025` / `require_0026` / `require_0027` gates, which assert the
//! migration file exists first (a readable failure, never a missing-file
//! panic). The two warning constants are empty scaffolds until DELIVER 01-01
//! and 02-01. Un-ignored in DELIVER 01-01 (0025), 02-01 (0026), 04-01 (0027).
//!
//! Real Postgres (testcontainers `postgres:16-alpine`); example-based.

use foundry_store::{
    run_boot_migrations_from_dir, run_migrations, run_migrations_from_dir, upgrade_warnings,
    MigrationReport, Store, UPGRADE_WARNING_0023, UPGRADE_WARNING_0025, UPGRADE_WARNING_0026,
};
use sqlx::postgres::{PgConnectOptions, PgDatabaseError, PgPoolOptions};
use sqlx::PgPool;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};

const M0025: &str = "0025_project_lifecycle_events.sql";
const M0026: &str = "0026_project_soft_delete.sql";
const M0027: &str = "0027_project_detach.sql";

type Id = uuid::Uuid;

async fn fresh() -> (PgPool, ContainerAsync<Postgres>) {
    let c = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let host = c.get_host().await.expect("container host");
    let port = c.get_host_port_ipv4(5432).await.expect("container port");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&format!(
            "postgres://postgres:postgres@{host}:{port}/postgres"
        ))
        .await
        .expect("connect pool");
    (pool, c)
}

fn production_migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

/// A private copy of the production migrations up to and including `through`.
fn staged_migrations(through: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("foundry-pl-{}", Id::now_v7()));
    std::fs::create_dir_all(&dir).expect("stage dir");
    let mut names: Vec<String> = std::fs::read_dir(production_migrations_dir())
        .expect("read migrations")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".sql") && n.get(..4).is_some_and(|v| v <= through))
        .collect();
    names.sort();
    for n in names {
        std::fs::copy(production_migrations_dir().join(&n), dir.join(&n)).expect("stage file");
    }
    dir
}

fn require_migration(file: &str, what: &str) {
    assert!(
        production_migrations_dir().join(file).exists(),
        "RED gate: migration {file} does not exist yet ({what})"
    );
}

fn require_0025() {
    require_migration(M0025, "project_lifecycle_events, DDD-13, slice 01");
}

fn require_0026() {
    require_migration(
        M0026,
        "the deleted marker and live_projects, DDD-2/DDD-3, slice 02",
    );
}

fn require_0027() {
    require_migration(M0027, "the detach, DDD-19, slice 04");
}

fn pg(err: &sqlx::Error) -> &PgDatabaseError {
    err.as_database_error()
        .unwrap_or_else(|| panic!("expected the database's refusal, got {err:?}"))
        .downcast_ref::<PgDatabaseError>()
}

/// Canzan Labs, Backend, Priya, and Identity Platform (AUTH) with one issue —
/// rows a migration must not touch.
async fn seed(pool: &PgPool) -> (Id, Id, Id, Id) {
    let (ws, team, user, project) = (Id::now_v7(), Id::now_v7(), Id::now_v7(), Id::now_v7());
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Canzan Labs')")
        .bind(ws)
        .execute(pool)
        .await
        .expect("workspace");
    sqlx::query(
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Backend', 'backend')",
    )
    .bind(team)
    .bind(ws)
    .execute(pool)
    .await
    .expect("team");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, 'priya.raman@canzan.dev', 'priya.raman@canzan.dev', 'Priya', 'x')",
    )
    .bind(user)
    .execute(pool)
    .await
    .expect("user");
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, 'Identity Platform', 'auth', 'AUTH')",
    )
    .bind(project)
    .bind(team)
    .bind(ws)
    .execute(pool)
    .await
    .expect("project");
    (ws, team, user, project)
}

async fn every_row(pool: &PgPool) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT 'w:' || id || ':' || name FROM workspaces
         UNION ALL SELECT 't:' || id || ':' || name || ':' || slug FROM teams
         UNION ALL SELECT 'u:' || id || ':' || email_lower FROM users
         UNION ALL SELECT 'p:' || id || ':' || team_id || ':' || name || ':' || slug || ':' || key_prefix
                          || ':' || next_issue_number FROM projects
         ORDER BY 1",
    )
    .fetch_all(pool)
    .await
    .expect("read every row");
    rows.into_iter().map(|(r,)| r).collect()
}

async fn has(pool: &PgPool, sql: &str) -> bool {
    let (yes,): (bool,) = sqlx::query_as(sql).fetch_one(pool).await.expect("look up");
    yes
}

// ---------------------------------------------------------------------------
// 0025 and its probe line (01-01)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn migration_0025_is_additive_over_a_schema_holding_rows() {
    require_0025();
    let (pool, _c) = fresh().await;
    run_migrations_from_dir(&pool, &staged_migrations("0024"))
        .await
        .expect("stand at 0024");
    seed(&pool).await;
    let before = every_row(&pool).await;
    run_migrations(&pool).await.expect("0025 applies over 0024");
    assert_eq!(every_row(&pool).await, before, "no existing row touched");
    let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM project_lifecycle_events")
        .fetch_one(&pool)
        .await
        .expect("the table exists and is empty");
    assert_eq!(n, 0);
}

/// ADR-PROJECT-LIFECYCLE-003 decision 1: `project_lifecycle_events_kind_shape`.
#[tokio::test]
async fn the_record_shape_check_refuses_every_incoherent_record_and_has_no_link_to_projects_or_teams(
) {
    require_0025();
    let (pool, _c) = fresh().await;
    run_migrations(&pool).await.expect("apply every migration");
    let (ws, _team, actor, _project) = seed(&pool).await;
    let record = |kind: &'static str,
                  actor: Option<Id>,
                  name: &'static str,
                  new_name: Option<&'static str>| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                "INSERT INTO project_lifecycle_events
                        (id, workspace_id, project_id, team_id, project_slug, key_prefix, kind, actor_id, name, new_name)
                 VALUES ($1, $2, $3, $4, 'auth', 'AUTH', $5, $6, $7, $8)",
            )
            .bind(Id::now_v7())
            .bind(ws)
            .bind(Id::now_v7())
            .bind(Id::now_v7())
            .bind(kind)
            .bind(actor)
            .bind(name)
            .bind(new_name)
            .execute(&pool)
            .await
        }
    };
    for (kind, who, name, new_name) in [
        (
            "renamed",
            Some(actor),
            "Identity Platform",
            Some("Identity Platform"),
        ),
        ("renamed", Some(actor), "Identity Platform", None),
        (
            "renamed",
            None,
            "Identity Platform",
            Some("Identity & Access"),
        ),
        ("deleted", Some(actor), "Identity Platform", Some("Other")),
        ("deleted", None, "Identity Platform", None),
        ("restored", Some(actor), "Identity Platform", None),
        ("restored", None, "Identity Platform", Some("Other")),
        ("archived", Some(actor), "Identity Platform", None),
    ] {
        let err = record(kind, who, name, new_name)
            .await
            .expect_err("an incoherent record");
        assert_eq!(
            pg(&err).code(),
            "23514",
            "{kind} {who:?} {new_name:?}: {:?}",
            pg(&err)
        );
    }
    record(
        "renamed",
        Some(actor),
        "Identity Platform",
        Some("identity platform"),
    )
    .await
    .expect("a case-only rename is coherent");
    record("deleted", Some(actor), "Identity Platform", None)
        .await
        .expect("a delete record names a project id that no row has: no FK to projects or teams");
    record("restored", None, "Identity Platform", None)
        .await
        .expect("a restore is the operator's: actor NULL");
    sqlx::query("DELETE FROM workspaces WHERE id = $1")
        .bind(ws)
        .execute(&pool)
        .await
        .expect("the records cascade with their workspace");
}

#[tokio::test]
async fn the_probe_refuses_a_schema_without_the_project_lifecycle_record() {
    require_0025();
    let (pool, _c) = fresh().await;
    run_migrations_from_dir(&pool, &staged_migrations("0024"))
        .await
        .expect("stand at 0024");
    let store = Store::from_pool(pool.clone());
    let err = store
        .probe()
        .await
        .expect_err("a schema without project_lifecycle_events must not be ready");
    assert!(
        err.to_string().contains("project_lifecycle_events"),
        "names the table: {err}"
    );
    run_migrations_from_dir(&pool, &staged_migrations("0025"))
        .await
        .expect("0025 applies over 0024");
    if !production_migrations_dir().join(M0026).exists() {
        store
            .probe()
            .await
            .expect("a schema at 0025 is ready (slice 01 alone)");
    }
}

/// The eleven columns of `project_lifecycle_events` the probe counts (DDD-28).
const RECORD_COLUMNS: [&str; 11] = [
    "id",
    "workspace_id",
    "project_id",
    "team_id",
    "project_slug",
    "key_prefix",
    "kind",
    "actor_id",
    "name",
    "new_name",
    "created_at",
];

/// Review TG-7 (DDD-28): the probe counts all eleven record columns. For each
/// one, a scratch schema migrated to head and then missing that column alone
/// is refused, the probe naming "(found 10 of 11)". Each schema is first shown
/// ready, so the refusal is the dropped column's doing.
#[tokio::test]
async fn the_probe_refuses_a_record_table_missing_any_one_of_its_eleven_columns() {
    require_0025();
    let (admin, c) = fresh().await;
    let host = c.get_host().await.expect("container host");
    let port = c.get_host_port_ipv4(5432).await.expect("container port");
    let base = format!("postgres://postgres:postgres@{host}:{port}/postgres");
    let mut survived = Vec::new();
    for column in RECORD_COLUMNS {
        let schema = format!("pl_missing_{column}");
        exec(&admin, &format!("CREATE SCHEMA {schema}")).await;
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(
                PgConnectOptions::from_str(&base)
                    .expect("parse the base url")
                    .options([("search_path", schema.as_str())]),
            )
            .await
            .expect("connect to the scratch schema");
        run_migrations(&pool)
            .await
            .expect("migrate the scratch schema");
        let store = Store::from_pool(pool.clone());
        store
            .probe()
            .await
            .unwrap_or_else(|e| panic!("precondition: {schema} at head is ready: {e}"));
        exec(
            &pool,
            &format!("ALTER TABLE project_lifecycle_events DROP COLUMN {column} CASCADE"),
        )
        .await;
        match store.probe().await {
            Ok(_) => survived.push(format!("{column}: ready")),
            Err(e) => {
                let said = e.to_string();
                if !said.contains(
                    "project_lifecycle_events missing migration-0025 columns (found 10 of 11)",
                ) {
                    survived.push(format!("{column}: {said}"));
                }
            }
        }
        pool.close().await;
    }
    assert!(
        survived.is_empty(),
        "the probe must refuse each missing record column, naming (found 10 of 11):\n{}",
        survived.join("\n")
    );
}

// ---------------------------------------------------------------------------
// 0026 and its probe lines (02-01)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn migration_0026_is_additive_and_every_existing_project_is_live() {
    require_0026();
    let (pool, _c) = fresh().await;
    run_migrations_from_dir(&pool, &staged_migrations("0025"))
        .await
        .expect("stand at 0025");
    let (_ws, _team, _user, project) = seed(&pool).await;
    let before = every_row(&pool).await;
    run_migrations_from_dir(&pool, &staged_migrations("0026"))
        .await
        .expect("0026 applies over 0025");
    assert_eq!(every_row(&pool).await, before, "no existing row touched");
    let (live,): (i64,) = sqlx::query_as("SELECT count(*) FROM live_projects WHERE id = $1")
        .bind(project)
        .fetch_one(&pool)
        .await
        .expect("the view exists");
    assert_eq!(live, 1, "an existing project is live");
    assert!(
        has(
            &pool,
            "SELECT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'projects_deleted_marker_shape')"
        )
        .await,
        "the marker shape CHECK is installed"
    );
}

/// DEVOPS: 0026 "self-checks view parity; a failed self-check fails the
/// migration and the boot". Fault injection on a staged copy: the view built
/// with a narrower column list must make 0026 fail, so a later migration that
/// forgets to recreate `live_projects` is a refused boot, never a silent drift.
/// Kills the hand-table fault "0026 without its parity self-check".
#[tokio::test]
async fn migration_0026_refuses_a_live_view_whose_columns_differ_from_projects() {
    require_0026();
    let (pool, _c) = fresh().await;
    let dir = staged_migrations("0026");
    let file = dir.join(M0026);
    let sql = std::fs::read_to_string(&file).expect("read the staged 0026");
    let pattern = regex_lite_view(&sql)
        .expect("0026 creates live_projects AS SELECT * FROM projects (DESIGN Data Model)");
    let drifted = sql.replacen(&pattern, &pattern.replacen('*', "id, name", 1), 1);
    std::fs::write(&file, drifted).expect("inject the drift");
    let err = run_migrations_from_dir(&pool, &dir).await.expect_err(
        "a live_projects view whose columns differ from projects must fail 0026's self-check",
    );
    assert!(
        matches!(err, foundry_store::StoreError::MigrationFailed(_)),
        "the self-check fails the migration itself (MigrationFailed), not a later query; got {err:?}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("live_projects"),
        "the self-check's error names live_projects, so the operator sees which object drifted; got {msg}"
    );
}

/// The `CREATE [OR REPLACE] VIEW live_projects AS SELECT * …` span, as written.
fn regex_lite_view(sql: &str) -> Option<String> {
    let lower = sql.to_ascii_lowercase();
    let start = lower.find("view live_projects")?;
    let star = start + lower[start..].find("select *")?;
    Some(sql[start..star + "select *".len()].to_string())
}

#[tokio::test]
async fn the_probe_refuses_a_schema_without_the_deleted_marker_or_the_live_view() {
    require_0026();
    let (pool, _c) = fresh().await;
    run_migrations_from_dir(&pool, &staged_migrations("0025"))
        .await
        .expect("stand at 0025");
    let store = Store::from_pool(pool.clone());
    let err = store
        .probe()
        .await
        .expect_err("a schema without the soft-delete objects must not be ready");
    let said = err.to_string();
    assert!(
        said.contains("deleted_at") || said.contains("live_projects"),
        "names the missing object: {said}"
    );
    run_migrations_from_dir(&pool, &staged_migrations("0026"))
        .await
        .expect("0026");
    exec(&pool, "DROP VIEW live_projects").await;
    let err = store
        .probe()
        .await
        .expect_err("a schema without live_projects is not ready");
    assert!(err.to_string().contains("live_projects"), "{err}");
}

/// Phase 5 mutation gap S-1: the marker is counted, not only the view. A
/// schema at head that keeps `live_projects` but has lost `projects.deleted_by`
/// is refused, naming the one marker column found.
#[tokio::test]
async fn the_probe_refuses_a_schema_keeping_the_live_view_but_missing_deleted_by() {
    require_0027();
    let (pool, _c) = fresh().await;
    run_migrations(&pool).await.expect("stand at head");
    let store = Store::from_pool(pool.clone());
    store.probe().await.expect("ready at head");
    exec(
        &pool,
        "DROP VIEW live_projects;
         ALTER TABLE projects DROP COLUMN deleted_by;
         CREATE VIEW live_projects AS SELECT * FROM projects WHERE deleted_at IS NULL;",
    )
    .await;
    let err = store
        .probe()
        .await
        .expect_err("a schema missing deleted_by is not ready, view or no view");
    let said = err.to_string();
    assert!(
        said.contains("deleted_at/deleted_by found 1 of 2, live_projects view present true"),
        "names the one marker column found and the view present: {said}"
    );
}

// ---------------------------------------------------------------------------
// 0027 and its probe line (04-01)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn migration_0027_is_additive_installs_its_check_and_keeps_the_team_fk_cascading() {
    require_0027();
    let (pool, _c) = fresh().await;
    run_migrations_from_dir(&pool, &staged_migrations("0026"))
        .await
        .expect("stand at 0026");
    seed(&pool).await;
    let before = every_row(&pool).await;
    run_migrations(&pool).await.expect("0027 applies over 0026");
    assert_eq!(every_row(&pool).await, before, "no existing row touched");
    assert!(
        has(&pool, "SELECT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'projects_team_or_detached')").await,
        "the detach CHECK is installed"
    );
    assert!(
        has(
            &pool,
            "SELECT confdeltype = 'c' FROM pg_constraint WHERE conname = 'projects_team_id_fkey'"
        )
        .await,
        "projects_team_id_fkey is still ON DELETE CASCADE"
    );
    assert!(
        has(
            &pool,
            "SELECT is_nullable = 'YES' FROM information_schema.columns
              WHERE table_schema = current_schema() AND table_name = 'projects' AND column_name = 'team_id'"
        )
        .await,
        "team_id is nullable"
    );
}

#[tokio::test]
async fn the_probe_refuses_a_schema_without_the_detach_column() {
    require_0027();
    let (pool, _c) = fresh().await;
    run_migrations_from_dir(&pool, &staged_migrations("0026"))
        .await
        .expect("stand at 0026");
    let store = Store::from_pool(pool.clone());
    let err = store
        .probe()
        .await
        .expect_err("not ready without the detach");
    assert!(err.to_string().contains("detached_from_team_id"), "{err}");
    run_migrations(&pool).await.expect("0027");
    store.probe().await.expect("ready at 0027");
}

async fn exec(pool: &PgPool, sql: &str) {
    sqlx::raw_sql(sql).execute(pool).await.expect(sql);
}

// ---------------------------------------------------------------------------
// The upgrade warnings (01-01: 0025; 02-01: 0026; DDD-29, DEVOPS)
// ---------------------------------------------------------------------------

#[test]
fn the_0025_warning_names_the_migration_the_table_and_the_supported_restore() {
    for phrase in [
        "0025",
        "project_lifecycle_events",
        "fresh database",
        "pg_restore --clean",
    ] {
        assert!(
            UPGRADE_WARNING_0025.contains(phrase),
            "the 0025 upgrade warning must say {phrase:?}: {UPGRADE_WARNING_0025:?}"
        );
    }
}

#[test]
fn the_0026_warning_names_the_migration_the_view_and_the_supported_restore() {
    for phrase in [
        "0026",
        "live_projects",
        "fresh database",
        "pg_restore --clean",
    ] {
        assert!(
            UPGRADE_WARNING_0026.contains(phrase),
            "the 0026 upgrade warning must say {phrase:?}: {UPGRADE_WARNING_0026:?}"
        );
    }
}

/// The pure contract, generalised from one version to a list (DEVOPS): one
/// warning per warned version this boot applied on top of earlier ones, in
/// version order; none for a fresh install; none for 0027.
#[test]
fn a_report_warns_once_per_warned_version_this_boot_upgraded_through() {
    let report = |applied: Vec<i64>, already_applied: Vec<i64>| MigrationReport {
        applied,
        already_applied,
    };
    assert_eq!(
        upgrade_warnings(&report(vec![25, 26], (1..=24).collect())),
        vec![UPGRADE_WARNING_0025, UPGRADE_WARNING_0026],
        "v0.14.1 straight to 0026 logs both, in version order"
    );
    assert_eq!(
        upgrade_warnings(&report(vec![25], (1..=24).collect())),
        vec![UPGRADE_WARNING_0025],
        "a slice-01 release logs 0025 only"
    );
    assert_eq!(
        upgrade_warnings(&report(vec![26, 27], (1..=25).collect())),
        vec![UPGRADE_WARNING_0026],
        "0027 warns nothing"
    );
    assert_eq!(
        upgrade_warnings(&report(vec![23, 24, 25, 26], (1..=22).collect())),
        vec![
            UPGRADE_WARNING_0023,
            UPGRADE_WARNING_0025,
            UPGRADE_WARNING_0026
        ],
        "every warned version, in order"
    );
    assert!(
        upgrade_warnings(&report((1..=27).collect(), vec![])).is_empty(),
        "a fresh install"
    );
    assert!(
        upgrade_warnings(&report(vec![], (1..=27).collect())).is_empty(),
        "nothing applied"
    );
    assert!(
        upgrade_warnings(&report(vec![27], (1..=26).collect())).is_empty(),
        "0027 alone"
    );
}

/// Through the boot loop on real databases: a fresh boot warns nothing; a boot
/// from 0024 through the latest warns 0025 and 0026 exactly once; the next boot
/// warns nothing.
#[tokio::test]
async fn only_the_boot_that_upgrades_through_0025_and_0026_warns_and_a_fresh_install_does_not() {
    require_0025();
    require_0026();
    let (fresh_pool, _fresh) = fresh().await;
    let install = run_boot_migrations_from_dir(&fresh_pool, &production_migrations_dir())
        .await
        .expect("a fresh install boots to the latest");
    assert!(
        upgrade_warnings(&install).is_empty(),
        "a fresh install warns nothing: {install:?}"
    );
    let (pool, _c) = fresh().await;
    run_boot_migrations_from_dir(&pool, &staged_migrations("0024"))
        .await
        .expect("boot at 0024");
    let upgrade = run_boot_migrations_from_dir(&pool, &production_migrations_dir())
        .await
        .expect("the upgrade boot");
    let warned = upgrade_warnings(&upgrade);
    assert!(
        warned.contains(&UPGRADE_WARNING_0025) && warned.contains(&UPGRADE_WARNING_0026),
        "{upgrade:?}"
    );
    let again = run_boot_migrations_from_dir(&pool, &production_migrations_dir())
        .await
        .expect("the next boot");
    assert!(
        upgrade_warnings(&again).is_empty(),
        "a boot that applies nothing warns nothing"
    );
}

// ---------------------------------------------------------------------------
// The previous release on the new schema (DEVOPS rollback floor, 04-01)
// ---------------------------------------------------------------------------

/// DEVOPS Deployment strategy: "an older image boots and goes Ready on the newer
/// schema", because the boot loop skips applied versions it does not know.
/// Migrate through 0027, then boot the previous release's loop over a dir staged
/// to 0024 (the name_rule_in_database.rs `previous_release_boot` idiom): it
/// applies nothing, errors on nothing, and the probe's verdict is unchanged.
#[tokio::test]
async fn the_previous_release_boots_on_the_migrated_schema_and_applies_nothing() {
    require_0025();
    require_0026();
    require_0027();
    let (pool, _c) = fresh().await;
    run_migrations_from_dir(&pool, &staged_migrations("0027"))
        .await
        .expect("migrate through 0027");
    let store = Store::from_pool(pool.clone());
    let probe_before = store.probe().await.map(|_| ()).map_err(|e| e.to_string());
    let report = run_boot_migrations_from_dir(&pool, &staged_migrations("0024"))
        .await
        .expect("the previous release's boot loop accepts the newer schema");
    assert!(
        report.applied.is_empty(),
        "the previous release applies nothing; applied {:?}",
        report.applied
    );
    let probe_after = store.probe().await.map(|_| ()).map_err(|e| e.to_string());
    assert_eq!(
        probe_after, probe_before,
        "the previous release's boot leaves the probe's verdict unchanged"
    );
}
