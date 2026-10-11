//! team-lifecycle — migrations 0023 (the lifecycle record) and 0024 (the team
//! name rule in the database), and the readiness probe line (DISCUSS OQ-6,
//! OQ-7, DoD 8 as corrected; DESIGN DDD-6, DDD-11, DDD-23, DDD-24;
//! ADR-TEAM-LIFECYCLE-001 decision 7, ADR-TEAM-LIFECYCLE-003 decisions 1 and 6;
//! ADR-NAME-DB-001). DISTILL 2026-10-11.
//!
//! - 0023 is additive: a schema staged at 0022 holding rows migrates with no
//!   row touched; the record's shape CHECK refuses an incoherent row; the
//!   record has no foreign key to `teams`; it cascades with its workspace.
//! - The probe refuses readiness without the record table (the 0018 precedent).
//! - 0024 adds `teams_name_rule_on_insert` / `_on_rename` at cap 64 over
//!   0019's unchanged verdict function, and REPLACES the shared trigger function
//!   `foundry_enforce_name_rule()` with an identical body plus one HINT arm
//!   (`teams` → `TeamName`). Legacy rows are never scanned; workspaces and
//!   projects keep their rule and their HINT (parity); a re-run is a no-op.
//!
//! GREEN today and never ignored: the substrate fact that Postgres refuses
//! U+0000 in `text` (22021) before any trigger runs, which is why the DB-rule
//! scenarios use U+0001 (DESIGN Changed Assumption 2).
//!
//! RED gates: `require_0023` / `require_0024` assert the migration file exists
//! first, a readable failure, never a missing-file panic. Every other test is
//! `#[ignore = "pending: team-lifecycle 02-01 | 03-01"]`.
//!
//! The existing `name_rule_in_database.rs` asserts exactly the four 0019/0020
//! triggers (`all_four_enabled`); 0024 makes it six. Extending that file's
//! parity to `teams` is DELIVER 03-01's (DESIGN Component Decomposition).
//!
//! Real Postgres (testcontainers `postgres:16-alpine`); example-based.

use foundry_store::{
    run_boot_migrations_from_dir, run_migrations, run_migrations_from_dir, upgrade_warnings,
    MigrationReport, Store, UPGRADE_WARNING_0023,
};
use sqlx::postgres::{PgDatabaseError, PgPoolOptions};
use sqlx::PgPool;
use std::path::PathBuf;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};

const M0023: &str = "0023_team_lifecycle_events.sql";
const M0024: &str = "0024_team_name_rule.sql";

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
    let dir = std::env::temp_dir().join(format!("foundry-tl-{}", uuid::Uuid::now_v7()));
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

fn require_0023() {
    require_migration(M0023, "the team lifecycle record, DDD-11, slice 02");
}

fn require_0024() {
    require_migration(M0024, "the team name rule in the database, DDD-6, slice 03");
}

async fn workspace(pool: &PgPool, name: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .expect("insert workspace");
    id
}

async fn user(pool: &PgPool, email: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, $2, 'not-a-real-hash')",
    )
    .bind(id)
    .bind(email)
    .execute(pool)
    .await
    .expect("insert user");
    id
}

async fn insert_team(
    pool: &PgPool,
    ws: uuid::Uuid,
    name: &str,
    slug: &str,
) -> Result<uuid::Uuid, sqlx::Error> {
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind(ws)
        .bind(name)
        .bind(slug)
        .execute(pool)
        .await
        .map(|_| id)
}

async fn every_row(pool: &PgPool) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT 'w:' || id || ':' || name FROM workspaces
         UNION ALL SELECT 't:' || id || ':' || name || ':' || slug FROM teams
         UNION ALL SELECT 'u:' || id || ':' || email_lower FROM users
         ORDER BY 1",
    )
    .fetch_all(pool)
    .await
    .expect("read every row");
    rows.into_iter().map(|(r,)| r).collect()
}

fn pg(err: &sqlx::Error) -> &PgDatabaseError {
    err.as_database_error()
        .unwrap_or_else(|| panic!("expected the database's refusal, got {err:?}"))
        .downcast_ref::<PgDatabaseError>()
}

fn assert_team_rule(err: &sqlx::Error, arm: &str) {
    let pg = pg(err);
    assert_eq!(pg.code(), "23514", "a check violation: {pg:?}");
    assert_eq!(pg.constraint(), Some(arm), "names the arm: {pg:?}");
    assert_eq!(pg.table(), Some("teams"), "names the table: {pg:?}");
    assert_eq!(pg.column(), Some("name"), "names the column: {pg:?}");
    assert!(pg.detail().is_none(), "no row dump in DETAIL: {pg:?}");
    assert!(
        pg.hint()
            .is_some_and(|h| h.contains("foundry_core::TeamName") && h.contains("64")),
        "the HINT names the mirrored type and its cap: {pg:?}"
    );
}

// ---------------------------------------------------------------------------
// Substrate fact — GREEN today
// ---------------------------------------------------------------------------

/// Postgres refuses U+0000 in `text` with 22021 before any trigger can run, so
/// a NUL can never reach the name rule (DESIGN Changed Assumption 2).
#[tokio::test]
async fn a_nul_in_a_team_name_is_refused_by_postgres_itself_before_any_rule() {
    let (pool, _c) = fresh().await;
    run_migrations(&pool).await.expect("apply every migration");
    let ws = workspace(&pool, "Canzan Labs").await;
    let err = insert_team(&pool, ws, "Platform\u{0}Ops", "platform-ops")
        .await
        .expect_err("a NUL is not storable in text");
    assert_eq!(
        pg(&err).code(),
        "22021",
        "character_not_in_repertoire: {:?}",
        pg(&err)
    );
}

// ---------------------------------------------------------------------------
// 0023 and the probe (02-01)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn migration_0023_is_additive_over_a_schema_holding_rows() {
    require_0023();
    let (pool, _c) = fresh().await;
    run_migrations_from_dir(&pool, &staged_migrations("0022"))
        .await
        .expect("stand at 0022");
    let ws = workspace(&pool, "Canzan Labs").await;
    insert_team(&pool, ws, "Backend", "backend")
        .await
        .expect("a team");
    user(&pool, "priya.raman@canzan.dev").await;
    let before = every_row(&pool).await;
    run_migrations(&pool).await.expect("0023 applies over 0022");
    assert_eq!(every_row(&pool).await, before, "no existing row touched");
    let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM team_lifecycle_events")
        .fetch_one(&pool)
        .await
        .expect("the table exists and is empty");
    assert_eq!(n, 0);
}

#[tokio::test]
async fn the_record_shape_check_refuses_an_incoherent_record_and_has_no_link_to_teams() {
    require_0023();
    let (pool, _c) = fresh().await;
    run_migrations(&pool).await.expect("apply every migration");
    let ws = workspace(&pool, "Canzan Labs").await;
    let actor = user(&pool, "priya.raman@canzan.dev").await;
    let record = |kind: &'static str, name: &'static str, new_name: Option<&'static str>| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                "INSERT INTO team_lifecycle_events (id, workspace_id, team_id, team_slug, kind, actor_id, name, new_name)
                      VALUES ($1, $2, $3, 'backend', $4, $5, $6, $7)",
            )
            .bind(uuid::Uuid::now_v7())
            .bind(ws)
            .bind(uuid::Uuid::now_v7())
            .bind(kind)
            .bind(actor)
            .bind(name)
            .bind(new_name)
            .execute(&pool)
            .await
        }
    };
    for (kind, name, new_name) in [
        ("renamed", "Backend", Some("Backend")),
        ("renamed", "Backend", None),
        ("deleted", "Backend", Some("Platform")),
        ("archived", "Backend", None),
    ] {
        let err = record(kind, name, new_name)
            .await
            .expect_err("an incoherent record");
        assert_eq!(
            pg(&err).code(),
            "23514",
            "{kind} {name} {new_name:?}: {:?}",
            pg(&err)
        );
    }
    record("renamed", "Backend", Some("backend"))
        .await
        .expect("a case-only rename is coherent");
    record("deleted", "Backend", None)
        .await
        .expect("a delete record names a team that no longer exists: no FK to teams");
    sqlx::query("DELETE FROM workspaces WHERE id = $1")
        .bind(ws)
        .execute(&pool)
        .await
        .expect("the records cascade with their workspace");
}

#[tokio::test]
async fn the_probe_refuses_a_schema_without_the_lifecycle_record() {
    require_0023();
    let (pool, _c) = fresh().await;
    run_migrations_from_dir(&pool, &staged_migrations("0022"))
        .await
        .expect("stand at 0022");
    let store = Store::from_pool(pool.clone());
    let err = store
        .probe()
        .await
        .expect_err("a schema without team_lifecycle_events must not be ready");
    assert!(
        err.to_string().contains("team_lifecycle_events"),
        "names the missing table: {err}"
    );
    run_migrations(&pool).await.expect("0023 applies over 0022");
    store.probe().await.expect("a schema at 0023 is ready");
}

// ---------------------------------------------------------------------------
// 0024 (03-01)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_new_team_name_that_breaks_the_rule_is_refused_under_its_arm() {
    require_0024();
    let (pool, _c) = fresh().await;
    run_migrations(&pool).await.expect("apply every migration");
    let ws = workspace(&pool, "Canzan Labs").await;
    for (name, arm) in [
        ("Platform\u{1}Ops", "teams_name_no_control_chars"),
        ("Platform\u{202E}Ops", "teams_name_no_control_chars"),
        (&*"a".repeat(65), "teams_name_max_64_chars"),
        (" ", "teams_name_not_empty"),
        (" Platform", "teams_name_trimmed"),
    ] {
        let err = insert_team(&pool, ws, name, "hand-typed")
            .await
            .expect_err("the rule refuses a new bad name");
        assert_team_rule(&err, arm);
    }
    insert_team(&pool, ws, &"日".repeat(64), "team")
        .await
        .expect("64 scalars are accepted (never stricter than TeamName)");
    let backend = insert_team(&pool, ws, "Backend", "backend")
        .await
        .expect("a team");
    let err = sqlx::query("UPDATE teams SET name = $1 WHERE id = $2")
        .bind("Backend\u{202E}")
        .bind(backend)
        .execute(&pool)
        .await
        .expect_err("a rename to a bad name is refused");
    assert_team_rule(&err, "teams_name_no_control_chars");
}

#[tokio::test]
async fn a_legacy_team_name_is_never_scanned_and_survives_writes_that_keep_it() {
    require_0024();
    let (pool, _c) = fresh().await;
    run_migrations_from_dir(&pool, &staged_migrations("0023"))
        .await
        .expect("stand at 0023");
    let ws = workspace(&pool, "Canzan Labs").await;
    let legacy = insert_team(&pool, ws, "Legacy\tTeam", "legacy")
        .await
        .expect("before 0024 any name was storable");
    run_migrations(&pool)
        .await
        .expect("0024 applies over a legacy row");
    sqlx::query("UPDATE teams SET slug = 'legacy-team' WHERE id = $1")
        .bind(legacy)
        .execute(&pool)
        .await
        .expect("a write of another column never meets the rule");
    sqlx::query("UPDATE teams SET name = name WHERE id = $1")
        .bind(legacy)
        .execute(&pool)
        .await
        .expect("a same-value name write never meets the rule");
    let (name,): (String,) = sqlx::query_as("SELECT name FROM teams WHERE id = $1")
        .bind(legacy)
        .fetch_one(&pool)
        .await
        .expect("read the legacy name");
    assert_eq!(name, "Legacy\tTeam", "the legacy name is untouched");
    let err = sqlx::query("UPDATE teams SET name = 'Legacy\tTeam 2' WHERE id = $1")
        .bind(legacy)
        .execute(&pool)
        .await
        .expect_err("a legacy row renamed to another bad name is refused");
    assert_team_rule(&err, "teams_name_no_control_chars");
}

/// 0024 replaces the shared trigger function: workspaces and projects keep
/// their rule, their caps and their HINT; the teams triggers are installed and
/// enabled; and re-applying the migration's body is a no-op.
#[tokio::test]
async fn migration_0024_keeps_the_other_rules_installs_both_triggers_and_reruns_cleanly() {
    require_0024();
    let (pool, _c) = fresh().await;
    run_migrations(&pool).await.expect("apply every migration");
    let triggers: Vec<(String, String)> = sqlx::query_as(
        "SELECT t.tgname::text, t.tgenabled::text FROM pg_trigger t JOIN pg_class c ON c.oid = t.tgrelid
          WHERE c.relname = 'teams' AND c.relnamespace = current_schema()::regnamespace
            AND NOT t.tgisinternal AND t.tgname LIKE '%name_rule%' ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .expect("read the teams triggers");
    assert_eq!(
        triggers,
        vec![
            ("teams_name_rule_on_insert".to_string(), "O".to_string()),
            ("teams_name_rule_on_rename".to_string(), "O".to_string()),
        ]
    );
    let err = sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(uuid::Uuid::now_v7())
        .bind("a".repeat(25))
        .execute(&pool)
        .await
        .expect_err("the workspace rule still holds at 24");
    let pg = pg(&err);
    assert_eq!(pg.constraint(), Some("workspaces_name_max_24_chars"));
    assert!(
        pg.hint()
            .is_some_and(|h| h.contains("foundry_core::WorkspaceName")),
        "the workspace HINT is unchanged: {pg:?}"
    );
    let sql = std::fs::read_to_string(production_migrations_dir().join(M0024)).expect("read 0024");
    sqlx::raw_sql(&sql)
        .execute(&pool)
        .await
        .expect("0024's body is re-runnable");
    let ws = workspace(&pool, "Canzan Labs").await;
    let err = insert_team(&pool, ws, "Platform\u{1}Ops", "platform-ops")
        .await
        .expect_err("still refused after a re-run");
    assert_team_rule(&err, "teams_name_no_control_chars");
}

// ---------------------------------------------------------------------------
// The upgrade warning for 0023 (user decision 2026-10-11, DELIVER 02-01)
// ---------------------------------------------------------------------------
//
// 0023's foreign keys from `team_lifecycle_events` to `workspaces` and `users`
// make `pg_restore --clean` of a pre-0023 dump over an upgraded database fail.
// The user kept the keys; "handled during upgrading as a warning": a boot that
// UPGRADES a database through 0023 (applies 23 while earlier migrations were
// already present before that boot) warns that such a dump is restored into a
// fresh database (or the table dropped first). A fresh install that applies
// 1..23 in one boot has no older dump and does not warn.
// `foundry_store::upgrade_warnings(&MigrationReport) -> Vec<&'static str>` is
// the contract (`applied` = this boot's versions, `already_applied` = the
// versions present before it); `UPGRADE_WARNING_0023` is its one source of
// text, and the boot logs each returned warning at `warn`.

/// The warning text says what happened and what the operator does about it.
#[test]
fn the_0023_warning_names_the_migration_the_table_and_the_supported_restore() {
    for phrase in [
        "0023",
        "team_lifecycle_events",
        "fresh database",
        "drop team_lifecycle_events",
    ] {
        assert!(
            UPGRADE_WARNING_0023.contains(phrase),
            "the 0023 upgrade warning must say {phrase:?}: {UPGRADE_WARNING_0023}"
        );
    }
}

/// The pure contract over a report: exactly one warning when this boot applied
/// 0023 on top of migrations that were already present; none for a fresh
/// install, a boot that applied nothing, or a boot that did not apply 0023.
#[test]
fn a_report_warns_once_exactly_when_this_boot_upgraded_through_0023() {
    let report = |applied: Vec<i64>, already_applied: Vec<i64>| MigrationReport {
        applied,
        already_applied,
    };
    assert_eq!(
        upgrade_warnings(&report(vec![22, 23], (1..=21).collect())),
        vec![UPGRADE_WARNING_0023],
        "an upgrade from 0021 through 0023 warns once"
    );
    assert_eq!(
        upgrade_warnings(&report(vec![23], (1..=22).collect())),
        vec![UPGRADE_WARNING_0023],
        "an upgrade from 0022 to 0023 warns once"
    );
    assert!(
        upgrade_warnings(&report((1..=23).collect(), vec![])).is_empty(),
        "a fresh install applies 0023 with nothing before it: no old dump, no warning"
    );
    assert!(
        upgrade_warnings(&report(vec![], (1..=23).collect())).is_empty(),
        "a boot that applies nothing warns nothing"
    );
    assert!(
        upgrade_warnings(&report(vec![24], (1..=23).collect())).is_empty(),
        "a later migration alone warns nothing"
    );
    assert!(
        upgrade_warnings(&report(vec![21, 22], (1..=20).collect())).is_empty(),
        "an upgrade that stops before 0023 warns nothing"
    );
}

/// Through the boot loop on real databases: a fresh boot straight to the latest
/// warns nothing; the staged upgrade (0021, then 0022, then the boot that
/// applies 0023) warns exactly once, on that boot; the next boot warns nothing.
#[tokio::test]
async fn only_the_boot_that_upgrades_through_0023_warns_and_a_fresh_install_does_not() {
    require_0023();
    let (fresh_pool, _fresh) = fresh().await;
    let install = run_boot_migrations_from_dir(&fresh_pool, &production_migrations_dir())
        .await
        .expect("a fresh install boots to the latest");
    assert!(
        install.applied.contains(&23) && install.already_applied.is_empty(),
        "{install:?}"
    );
    assert!(
        upgrade_warnings(&install).is_empty(),
        "a fresh install warns nothing: {install:?}"
    );

    let (pool, _c) = fresh().await;
    let to_0021 = run_boot_migrations_from_dir(&pool, &staged_migrations("0021"))
        .await
        .expect("boot at 0021");
    assert!(upgrade_warnings(&to_0021).is_empty(), "{to_0021:?}");
    let to_0022 = run_boot_migrations_from_dir(&pool, &staged_migrations("0022"))
        .await
        .expect("boot at 0022");
    assert_eq!(to_0022.applied, vec![22]);
    assert!(
        upgrade_warnings(&to_0022).is_empty(),
        "applying only 0022 warns nothing"
    );
    let upgrade = run_boot_migrations_from_dir(&pool, &production_migrations_dir())
        .await
        .expect("the upgrade boot");
    assert!(upgrade.applied.contains(&23), "{upgrade:?}");
    assert_eq!(
        upgrade_warnings(&upgrade),
        vec![UPGRADE_WARNING_0023],
        "the boot that upgrades through 0023 warns exactly once"
    );
    let again = run_boot_migrations_from_dir(&pool, &production_migrations_dir())
        .await
        .expect("the next boot");
    assert!(again.applied.is_empty(), "{again:?}");
    assert!(
        upgrade_warnings(&again).is_empty(),
        "a boot that applies nothing warns nothing"
    );
}
