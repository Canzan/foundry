//! name-db-checks (DISCUSS D1-D12; DESIGN DDD-4/5/7/8/10/14/15, ADR-NAME-DB-001):
//! the name rule is wired to every NEW name write on `workspaces` and `projects`,
//! and to nothing else.
//!
//! - Migrations 0019 (verdict + enforce functions, UTF8 guard, apply-time
//!   self-check, the two `workspaces` triggers at `'24'`) and 0020 (the two
//!   `projects` triggers at `'256'`, self-check at 256) apply over legacy rows
//!   without rewriting them, and re-running them is a no-op (DoD 3).
//! - A refused write is SQLSTATE 23514 with `CONSTRAINT = <table>_name_<arm>`,
//!   `TABLE`, `COLUMN = name`, `SCHEMA`, the native message, a HINT naming the
//!   mirrored Rust type and no DETAIL (DDD-4); nothing changes.
//! - A write that does not change the name never meets the rule: another column
//!   on a legacy row (the issue counter, `lib.rs` `insert_issue_attempt`), a
//!   same-value name write, the store's no-op rename (D6, DDD-5).
//! - The rule holds under `search_path = ''` (pg_restore's), DDD-4.
//! - Earned Trust at apply time: a non-UTF8 database and a drifted verdict
//!   function are refused (DDD-8).
//! - The legacy seam (DDD-10): bounded change — the one caller row, with the
//!   table's two triggers enabled again before commit, even when the write fails.
//! - Slice 03 (DDD-14): a post-0020 dump with legacy rows restores byte-identical
//!   with the rule on; a pre-0019 dump restores over a migrated schema and the next
//!   boot re-applies the rule; the previous release's boot loop ignores 0019/0020,
//!   its probe passes and it writes valid names.
//!
//! Legacy rows are seeded the staged way (DDD-13, the F8 model): stand the schema
//! at 0018, insert plainly, migrate on. Only the seam tests and the post-0020
//! dump (DDD-14 1) go through the seam.
//!
//! [`seam_insert_workspace`] / [`seam_insert_project`] call the DDD-10
//! test-support seam; [`previous_release_boot`] calls the DDD-14 4 test-support
//! boot-path migrator entry over a staged dir. Every database test starts at
//! [`require_rule_installed`] (or the 0019 file lookup), the RED gate that fails
//! with a readable message, never an undefined-object error, if 0019/0020 are
//! missing.
//!
//! WHY-NEW-FILE: crates/foundry-store/tests/name_rule_in_database.rs
//!   CLOSEST-EXISTING: crates/foundry-store/tests/workspace_rename_with_audit.rs
//!   EXTENSION-COST: that file pins the rename transaction and 0018; this pins two
//!     new migrations, two tables' triggers, restore and rollback.
//!   PARALLEL-RATIONALE: "a rename and its record commit together" and "every new
//!     name write meets the rule, and nothing else does" are unrelated properties.
//!
//! Real Postgres (testcontainers `postgres:16-alpine`; its own `pg_dump` and
//! `pg_restore` run inside the container, so the client always matches the
//! server). Integration level, example-based (Mandates 9/11).

use foundry_store::{
    run_boot_migrations_from_dir, run_migrations, run_migrations_from_dir,
    seed_row_predating_name_rule, NameRuleTable, Store, WorkspaceRenameWrite,
};
use sqlx::postgres::{PgDatabaseError, PgPoolOptions};
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::core::ExecCommand;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};

/// The iawr legacy over-long name (53 characters).
const LEGACY_LONG: &str = "Canzan Labs Platform Engineering and Site Reliability";
const LEGACY_TAB_WORKSPACE: &str = "Canzan\tLabs";
const LEGACY_TAB_PROJECT: &str = "Homelab\tOps";

/// The four triggers DDD-5 names, `(table, trigger)`.
const TRIGGERS: [(&str, &str); 4] = [
    ("projects", "projects_name_rule_on_insert"),
    ("projects", "projects_name_rule_on_rename"),
    ("workspaces", "workspaces_name_rule_on_insert"),
    ("workspaces", "workspaces_name_rule_on_rename"),
];

// ---------------------------------------------------------------------------
// The legacy seam and the previous release's boot
// ---------------------------------------------------------------------------

/// The DDD-10 test-support legacy seam storing a workspace "that predates the
/// rule" (reached through the crate's self dev-dependency with `test-support`);
/// `Err` carries the write's error text.
async fn seam_insert_workspace(pool: &PgPool, id: uuid::Uuid, name: &str) -> Result<(), String> {
    let write = sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(id)
        .bind(name);
    seed_row_predating_name_rule(pool, NameRuleTable::Workspaces, write)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// As [`seam_insert_workspace`], for a project.
async fn seam_insert_project(pool: &PgPool, row: &ProjectSeed) -> Result<(), String> {
    let write = sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(row.id)
    .bind(row.team_id)
    .bind(row.workspace_id)
    .bind(&row.name)
    .bind(&row.slug)
    .bind(&row.key_prefix);
    seed_row_predating_name_rule(pool, NameRuleTable::Projects, write)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// DDD-14 (4): what the previous release (v0.11.0) does at boot — the boot loop
/// (`run_migrator_timed`, which skips applied versions it does not know) over
/// migrations up to 0018 only. NOT `run_migrations_from_dir`: its
/// `Migrator::run` errors on applied-but-unknown versions, which an old binary's
/// boot does not. Returns what that boot saw, as `(applied, already_applied)`
/// version lists.
async fn previous_release_boot(
    pool: &PgPool,
    staged: &Path,
) -> Result<(Vec<i64>, Vec<i64>), String> {
    run_boot_migrations_from_dir(pool, staged)
        .await
        .map(|report| (report.applied, report.already_applied))
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

fn production_migrations_dir() -> PathBuf {
    let manifest =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is always set by cargo");
    PathBuf::from(manifest).join("migrations")
}

/// A private copy of the production migrations up to and including `through`.
fn staged_migrations(through: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("foundry-ndc-{}", uuid::Uuid::now_v7()));
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

async fn container() -> ContainerAsync<Postgres> {
    Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container")
}

async fn connect(container: &ContainerAsync<Postgres>, database: &str) -> PgPool {
    let host = container.get_host().await.expect("container host");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("container port");
    PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&format!(
            "postgres://postgres:postgres@{host}:{port}/{database}"
        ))
        .await
        .expect("connect pool")
}

async fn create_database(admin: &PgPool, name: &str, options: &str) {
    sqlx::query(&format!("CREATE DATABASE {name} {options}"))
        .execute(admin)
        .await
        .unwrap_or_else(|e| panic!("create database {name}: {e}"));
}

async fn migrated() -> (PgPool, ContainerAsync<Postgres>) {
    let c = container().await;
    let pool = connect(&c, "postgres").await;
    run_migrations(&pool).await.expect("apply every migration");
    (pool, c)
}

/// A schema at 0018 holding the legacy rows, then migrated forward (the F8 model).
async fn staged_with_legacy_rows() -> (PgPool, ContainerAsync<Postgres>, Legacy) {
    let c = container().await;
    let pool = connect(&c, "postgres").await;
    stand_at_0018(&pool).await;
    let legacy = insert_legacy_rows(&pool).await;
    run_migrations(&pool)
        .await
        .expect("0019/0020 apply over legacy rows");
    (pool, c, legacy)
}

async fn stand_at_0018(pool: &PgPool) {
    run_migrations_from_dir(pool, &staged_migrations("0018"))
        .await
        .expect("stand the schema at 0018");
}

/// RED gate: 0019 and 0020 are installed. Until they land this fails with a
/// readable RED-gate message, never with an undefined-object database error.
async fn require_rule_installed(pool: &PgPool) {
    let (verdict, enforce, triggers): (bool, bool, i64) = sqlx::query_as(
        "SELECT to_regprocedure('foundry_name_rule_violation(text, integer)') IS NOT NULL,
                to_regprocedure('foundry_enforce_name_rule()') IS NOT NULL,
                (SELECT count(*) FROM pg_trigger t JOIN pg_class c ON c.oid = t.tgrelid
                  WHERE c.relnamespace = current_schema()::regnamespace
                    AND t.tgname LIKE '%_name_rule_on_%')",
    )
    .fetch_one(pool)
    .await
    .expect("look the rule up");
    assert!(
        verdict && enforce && triggers == 4,
        "RED gate: migrations 0019/0020 have not installed the name rule yet \
         (verdict fn {verdict}, enforce fn {enforce}, {triggers} of 4 triggers; \
         name-db-checks DDD-1/4/5/7)"
    );
}

// ---------------------------------------------------------------------------
// Rows
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ProjectSeed {
    id: uuid::Uuid,
    workspace_id: uuid::Uuid,
    team_id: uuid::Uuid,
    name: String,
    slug: String,
    key_prefix: String,
}

/// The rows stored before the upgrade, and the fit rows beside them.
#[derive(Debug, Clone, Copy)]
struct Legacy {
    priya: uuid::Uuid,
    long_workspace: uuid::Uuid,
    tab_workspace: uuid::Uuid,
    household: uuid::Uuid,
    team: uuid::Uuid,
    tab_project: uuid::Uuid,
    sandbox: uuid::Uuid,
}

async fn insert_workspace(pool: &PgPool, name: &str) -> Result<uuid::Uuid, sqlx::Error> {
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .map(|_| id)
}

async fn insert_user(pool: &PgPool, email: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, 'Priya Raman', 'not-a-real-hash')",
    )
    .bind(id)
    .bind(email)
    .execute(pool)
    .await
    .expect("insert user");
    id
}

async fn insert_team(pool: &PgPool, workspace_id: uuid::Uuid) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Backend', 'backend')",
    )
    .bind(id)
    .bind(workspace_id)
    .execute(pool)
    .await
    .expect("insert team");
    id
}

async fn insert_project(pool: &PgPool, row: &ProjectSeed) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(row.id)
    .bind(row.team_id)
    .bind(row.workspace_id)
    .bind(&row.name)
    .bind(&row.slug)
    .bind(&row.key_prefix)
    .execute(pool)
    .await
    .map(|_| ())
}

async fn seed_lanes(pool: &PgPool, project_id: uuid::Uuid) {
    sqlx::query(
        "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
         SELECT gen_random_uuid(), p.id, p.workspace_id, seed.slug, seed.label, seed.position
           FROM projects p
          CROSS JOIN (VALUES ('backlog', 'Backlog', 0), ('done', 'Done', 1))
                AS seed (slug, label, position)
          WHERE p.id = $1",
    )
    .bind(project_id)
    .execute(pool)
    .await
    .expect("seed lanes");
}

fn project_seed(
    workspace_id: uuid::Uuid,
    team_id: uuid::Uuid,
    name: &str,
    key: &str,
) -> ProjectSeed {
    ProjectSeed {
        id: uuid::Uuid::now_v7(),
        workspace_id,
        team_id,
        name: name.to_string(),
        slug: format!("p-{}", key.to_ascii_lowercase()),
        key_prefix: key.to_string(),
    }
}

/// Plain inserts: only valid BEFORE 0019 (the staged model).
async fn insert_legacy_rows(pool: &PgPool) -> Legacy {
    let priya = insert_user(pool, "priya@canzan.test").await;
    let household = insert_workspace(pool, "Household")
        .await
        .expect("Household");
    let long_workspace = insert_workspace(pool, LEGACY_LONG)
        .await
        .expect("legacy long");
    let tab_workspace = insert_workspace(pool, LEGACY_TAB_WORKSPACE)
        .await
        .expect("legacy tab");
    let team = insert_team(pool, household).await;
    let tab = project_seed(household, team, LEGACY_TAB_PROJECT, "OPS");
    insert_project(pool, &tab).await.expect("legacy project");
    seed_lanes(pool, tab.id).await;
    let sandbox = project_seed(household, team, "Sandbox", "SBX");
    insert_project(pool, &sandbox).await.expect("Sandbox");
    seed_lanes(pool, sandbox.id).await;
    Legacy {
        priya,
        long_workspace,
        tab_workspace,
        household,
        team,
        tab_project: tab.id,
        sandbox: sandbox.id,
    }
}

/// Every name in both tables plus each project's issue counter (the universe).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Names {
    workspaces: BTreeMap<uuid::Uuid, String>,
    projects: BTreeMap<uuid::Uuid, (String, i32)>,
}

async fn names(pool: &PgPool) -> Names {
    let workspaces: Vec<(uuid::Uuid, String)> = sqlx::query_as("SELECT id, name FROM workspaces")
        .fetch_all(pool)
        .await
        .expect("read workspaces");
    let projects: Vec<(uuid::Uuid, String, i32)> =
        sqlx::query_as("SELECT id, name, next_issue_number FROM projects")
            .fetch_all(pool)
            .await
            .expect("read projects");
    Names {
        workspaces: workspaces.into_iter().collect(),
        projects: projects
            .into_iter()
            .map(|(id, name, n)| (id, (name, n)))
            .collect(),
    }
}

/// `(table, trigger, tgenabled)` of every name-rule trigger in the schema.
async fn triggers(pool: &PgPool) -> Vec<(String, String, String)> {
    sqlx::query_as(
        "SELECT c.relname::text, t.tgname::text, t.tgenabled::text
           FROM pg_trigger t JOIN pg_class c ON c.oid = t.tgrelid
          WHERE c.relnamespace = current_schema()::regnamespace
            AND NOT t.tgisinternal AND t.tgname LIKE '%name_rule%'
          ORDER BY 1, 2",
    )
    .fetch_all(pool)
    .await
    .expect("read the name-rule triggers")
}

fn all_four_enabled() -> Vec<(String, String, String)> {
    TRIGGERS
        .iter()
        .map(|(t, n)| (t.to_string(), n.to_string(), "O".to_string()))
        .collect()
}

// ---------------------------------------------------------------------------
// The refusal oracle (DDD-4)
// ---------------------------------------------------------------------------

fn assert_refused(result: Result<sqlx::postgres::PgQueryResult, sqlx::Error>, arm: &str) {
    let table = if arm.starts_with("workspaces_") {
        "workspaces"
    } else {
        "projects"
    };
    let rust_type = if table == "workspaces" {
        "WorkspaceName"
    } else {
        "ProjectName"
    };
    let err = match result {
        Ok(done) => panic!(
            "the write must be refused under {arm:?}; it touched {} row(s)",
            done.rows_affected()
        ),
        Err(err) => err,
    };
    let sqlx::Error::Database(db) = &err else {
        panic!("the refusal must come from the database; got {err}");
    };
    let pg = db
        .try_downcast_ref::<PgDatabaseError>()
        .unwrap_or_else(|| panic!("a Postgres error; got {db}"));
    assert_eq!(pg.code(), "23514", "check_violation (D5); got {pg:?}");
    assert_eq!(
        pg.constraint(),
        Some(arm),
        "the arm is named (DDD-4); got {pg:?}"
    );
    assert_eq!(pg.table(), Some(table), "TABLE is set (DDD-4); got {pg:?}");
    assert_eq!(
        pg.column(),
        Some("name"),
        "COLUMN is set (DDD-4); got {pg:?}"
    );
    assert!(pg.schema().is_some(), "SCHEMA is set (DDD-4); got {pg:?}");
    assert_eq!(
        pg.message(),
        format!("new row for relation \"{table}\" violates check constraint \"{arm}\""),
        "the native check-violation wording (DDD-4)"
    );
    assert!(
        pg.hint().is_some_and(|h| h.contains(rust_type)),
        "a one-line HINT names the mirrored type {rust_type} (DDD-4, OQ-D6); got {pg:?}"
    );
    assert_eq!(pg.detail(), None, "no DETAIL row dump (DDD-4); got {pg:?}");
}

async fn rename(
    pool: &PgPool,
    table: &str,
    id: uuid::Uuid,
    name: &str,
) -> Result<sqlx::postgres::PgQueryResult, sqlx::Error> {
    sqlx::query(&format!("UPDATE {table} SET name = $1 WHERE id = $2"))
        .bind(name)
        .bind(id)
        .execute(pool)
        .await
}

// ---------------------------------------------------------------------------
// Migrations 0019 / 0020 (DoD 3, DDD-7)
// ---------------------------------------------------------------------------

/// DoD 3: the rule applies over a database already holding legacy names, scans
/// and rewrites none of them, and re-running the set is a no-op.
#[tokio::test]
async fn migrations_0019_and_0020_apply_over_legacy_rows_rewrite_none_and_rerun_is_a_noop() {
    let c = container().await;
    let pool = connect(&c, "postgres").await;
    stand_at_0018(&pool).await;
    insert_legacy_rows(&pool).await;
    let before = names(&pool).await;

    run_migrations(&pool)
        .await
        .expect("0019/0020 apply forward");
    run_migrations(&pool)
        .await
        .expect("re-running the set is a no-op");

    require_rule_installed(&pool).await;
    assert_eq!(
        names(&pool).await,
        before,
        "no legacy row is rewritten (D4)"
    );
    assert_eq!(triggers(&pool).await, all_four_enabled());
    let (applied,): (Vec<i64>,) = sqlx::query_as(
        "SELECT array_agg(version ORDER BY version) FROM _sqlx_migrations WHERE version >= 19",
    )
    .fetch_one(&pool)
    .await
    .expect("read the applied versions");
    assert_eq!(
        applied,
        vec![19, 20],
        "0019 and 0020 are recorded once each"
    );
}

/// DDD-4/5/7: the triggers fire BEFORE INSERT and BEFORE UPDATE OF name only when
/// the name changes, carry each table's cap, and every object says what it mirrors.
#[tokio::test]
async fn the_triggers_fire_only_on_new_name_writes_with_each_tables_cap_and_say_what_they_mirror() {
    let (pool, _c) = migrated().await;
    require_rule_installed(&pool).await;
    assert_eq!(triggers(&pool).await, all_four_enabled());
    for (table, trigger) in TRIGGERS {
        let (def, comment): (String, Option<String>) = sqlx::query_as(
            "SELECT pg_get_triggerdef(t.oid), obj_description(t.oid, 'pg_trigger')
               FROM pg_trigger t JOIN pg_class c ON c.oid = t.tgrelid
              WHERE c.relname = $1 AND t.tgname = $2
                AND c.relnamespace = current_schema()::regnamespace",
        )
        .bind(table)
        .bind(trigger)
        .fetch_one(&pool)
        .await
        .unwrap_or_else(|e| panic!("read {trigger}: {e}"));
        let cap = if table == "workspaces" {
            "'24'"
        } else {
            "'256'"
        };
        let rust_type = if table == "workspaces" {
            "foundry_core::WorkspaceName"
        } else {
            "foundry_core::ProjectName"
        };
        assert!(
            def.contains("BEFORE"),
            "{trigger} runs BEFORE the write: {def}"
        );
        assert!(def.contains("FOR EACH ROW"), "{trigger} is per row: {def}");
        assert!(
            def.contains(&format!("foundry_enforce_name_rule({cap})")),
            "{trigger} passes the table's cap {cap} (DDD-4): {def}"
        );
        if trigger.ends_with("_on_insert") {
            assert!(def.contains("BEFORE INSERT ON"), "{trigger}: {def}");
        } else {
            assert!(
                def.contains("BEFORE UPDATE OF name ON"),
                "{trigger} fires on an UPDATE that lists name, and on no other UPDATE (D6): {def}"
            );
            assert!(
                def.contains("(old.name IS DISTINCT FROM new.name)"),
                "{trigger} fires only when the name changes (DDD-5): {def}"
            );
        }
        assert!(
            comment.as_deref().is_some_and(|c| c.contains(rust_type)),
            "{trigger} says it mirrors {rust_type} (DDD-7); comment = {comment:?}"
        );
    }
    for function in [
        "foundry_name_rule_violation(text, integer)",
        "foundry_enforce_name_rule()",
    ] {
        let (comment,): (Option<String>,) =
            sqlx::query_as("SELECT obj_description($1::regprocedure, 'pg_proc')")
                .bind(function)
                .fetch_one(&pool)
                .await
                .unwrap_or_else(|e| panic!("read the comment on {function}: {e}"));
        assert!(
            comment
                .as_deref()
                .is_some_and(|c| c.contains("foundry_core::")),
            "{function} says what it mirrors (DDD-7); comment = {comment:?}"
        );
        let (source,): (String,) =
            sqlx::query_as("SELECT prosrc FROM pg_proc WHERE oid = $1::regprocedure")
                .bind(function)
                .fetch_one(&pool)
                .await
                .unwrap_or_else(|e| panic!("read the body of {function}: {e}"));
        assert!(
            !source.contains("current_setting"),
            "{function} has no switch any session could flip (DDD-11: no GUC bypass)"
        );
    }
    for (_, trigger) in TRIGGERS {
        let (def,): (String,) =
            sqlx::query_as("SELECT pg_get_triggerdef(oid) FROM pg_trigger WHERE tgname = $1")
                .bind(trigger)
                .fetch_one(&pool)
                .await
                .unwrap_or_else(|e| panic!("read {trigger}: {e}"));
        assert!(
            !def.contains("current_setting"),
            "{trigger}'s WHEN has no session switch (DDD-11): {def}"
        );
    }
}

// ---------------------------------------------------------------------------
// New name writes (DDD-4/6, the DoD-2 shell examples)
// ---------------------------------------------------------------------------

/// Every arm, on INSERT and on UPDATE, on both tables: refused with the full
/// error contract, and nothing moves.
#[tokio::test]
async fn a_bad_name_write_is_refused_per_arm_and_table_and_nothing_changes() {
    let (pool, _c, legacy) = staged_with_legacy_rows().await;
    require_rule_installed(&pool).await;
    let before = names(&pool).await;
    let ws_cases: [(&str, &str); 5] = [
        ("", "workspaces_name_not_empty"),
        (" Globex", "workspaces_name_trimmed"),
        ("Globex\u{00A0}", "workspaces_name_trimmed"),
        ("House\tHold", "workspaces_name_no_control_chars"),
        ("Canzan Labs Platform Ops!", "workspaces_name_max_24_chars"),
    ];
    for (name, arm) in ws_cases {
        assert_refused(
            rename(&pool, "workspaces", legacy.household, name).await,
            arm,
        );
        assert_refused(
            sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
                .bind(uuid::Uuid::now_v7())
                .bind(name)
                .execute(&pool)
                .await,
            arm,
        );
    }
    let long = "x".repeat(257);
    let pr_cases: [(&str, &str); 5] = [
        ("", "projects_name_not_empty"),
        ("Homelab Ops ", "projects_name_trimmed"),
        ("\u{3000}Sandbox", "projects_name_trimmed"),
        ("Sand\tbox", "projects_name_no_control_chars"),
        (long.as_str(), "projects_name_max_256_chars"),
    ];
    for (name, arm) in pr_cases {
        assert_refused(rename(&pool, "projects", legacy.sandbox, name).await, arm);
        let seed = project_seed(legacy.household, legacy.team, name, "NEW");
        assert_refused(
            sqlx::query(
                "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
                      VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(seed.id)
            .bind(seed.team_id)
            .bind(seed.workspace_id)
            .bind(&seed.name)
            .bind(&seed.slug)
            .bind(&seed.key_prefix)
            .execute(&pool)
            .await,
            arm,
        );
    }
    assert_eq!(names(&pool).await, before, "a refusal moves nothing");
}

/// D3 (never stricter) and D1 (uniqueness stays in the app): fit names, a
/// case-duplicate project and a same-named workspace are stored byte for byte.
#[tokio::test]
async fn fit_names_are_stored_byte_exact_and_uniqueness_is_left_to_the_app() {
    let (pool, _c, legacy) = staged_with_legacy_rows().await;
    require_rule_installed(&pool).await;
    for name in [
        "👨\u{200D}👩\u{200D}👧 Bailey",
        "Ångström Øresund Société",
        "\u{FEFF}Globex",
        "Glo\u{00A0}bex",
        "Household",
    ] {
        rename(&pool, "workspaces", legacy.household, name)
            .await
            .unwrap_or_else(|e| panic!("{name:?} must be accepted: {e}"));
        let stored = names(&pool).await.workspaces[&legacy.household].clone();
        assert_eq!(stored, name, "stored byte for byte");
    }
    insert_workspace(&pool, "Household")
        .await
        .expect("two workspaces may share a name");
    let duplicate = project_seed(legacy.household, legacy.team, "sandbox", "SBD");
    insert_project(&pool, &duplicate)
        .await
        .expect("a case-duplicate project name is the app's to refuse, not the database's (D1)");
    rename(&pool, "projects", legacy.sandbox, &"日".repeat(256))
        .await
        .expect("256 three-byte characters are accepted");
}

// ---------------------------------------------------------------------------
// Legacy rows keep every capability (D6, DDD-5)
// ---------------------------------------------------------------------------

/// Writes that do not change a legacy name never meet the rule: the issue
/// counter (through the store's own new-issue path), another workspace column,
/// a same-value name write, the store's no-op rename.
#[tokio::test]
async fn writes_that_do_not_change_a_legacy_name_never_meet_the_rule() {
    let (pool, _c, legacy) = staged_with_legacy_rows().await;
    require_rule_installed(&pool).await;
    let store = Store::from_pool(pool.clone());
    let before = names(&pool).await;

    let issue = store
        .insert_issue_with_outbox(
            uuid::Uuid::now_v7(),
            legacy.household,
            legacy.tab_project,
            "OPS",
            legacy.priya,
            "Replace UPS battery",
            "",
        )
        .await
        .expect("a legacy project takes new issues (D6; lib.rs insert_issue_attempt)");
    assert_eq!(issue.number, 1, "the first issue of the legacy project");
    sqlx::query("UPDATE workspaces SET created_at = created_at WHERE id = $1")
        .bind(legacy.long_workspace)
        .execute(&pool)
        .await
        .expect("another column of a legacy workspace");
    rename(
        &pool,
        "workspaces",
        legacy.tab_workspace,
        LEGACY_TAB_WORKSPACE,
    )
    .await
    .expect("a same-value name write on a legacy workspace");
    rename(&pool, "projects", legacy.tab_project, LEGACY_TAB_PROJECT)
        .await
        .expect("a same-value name write on a legacy project");
    let quiet = store
        .rename_workspace_with_audit(legacy.long_workspace, legacy.priya, LEGACY_LONG)
        .await
        .expect("the store's no-op rename of a legacy name");
    assert!(
        matches!(quiet, WorkspaceRenameWrite::Unchanged),
        "{quiet:?}"
    );

    let mut expected = before.clone();
    expected
        .projects
        .get_mut(&legacy.tab_project)
        .expect("legacy project")
        .1 += 1;
    assert_eq!(
        names(&pool).await,
        expected,
        "only the legacy project's issue counter moved; every legacy name is byte-identical"
    );
}

/// A legacy row is not exempt from the rule on a NEW name: another bad name is
/// refused, a fit name is accepted through the store's own renames.
#[tokio::test]
async fn a_legacy_row_renamed_to_another_bad_name_is_refused_and_to_a_fit_name_is_accepted() {
    let (pool, _c, legacy) = staged_with_legacy_rows().await;
    require_rule_installed(&pool).await;
    assert_refused(
        rename(
            &pool,
            "workspaces",
            legacy.tab_workspace,
            "Canzan\tLabs Ops",
        )
        .await,
        "workspaces_name_no_control_chars",
    );
    assert_refused(
        rename(
            &pool,
            "workspaces",
            legacy.long_workspace,
            "Canzan Labs Platform Engineering",
        )
        .await,
        "workspaces_name_max_24_chars",
    );
    assert_refused(
        rename(&pool, "projects", legacy.tab_project, "Homelab\tOps 2").await,
        "projects_name_no_control_chars",
    );
    let store = Store::from_pool(pool.clone());
    let renamed = store
        .rename_workspace_with_audit(legacy.long_workspace, legacy.priya, "Canzan Labs")
        .await
        .expect("a legacy workspace renames to a fit name");
    assert!(
        matches!(renamed, WorkspaceRenameWrite::Renamed { .. }),
        "{renamed:?}"
    );
    assert_eq!(
        store
            .update_project_name(legacy.tab_project, "Homelab Ops")
            .await
            .expect("a legacy project renames to a fit name"),
        1
    );
}

/// DDD-4: `SET search_path FROM CURRENT` keeps the rule bound when the caller's
/// session has an empty search_path (pg_restore's), with qualified statements.
#[tokio::test]
async fn the_rule_holds_under_an_empty_search_path() {
    let (pool, _c, legacy) = staged_with_legacy_rows().await;
    require_rule_installed(&pool).await;
    let mut conn = pool.acquire().await.expect("one session");
    sqlx::query("SET search_path = ''")
        .execute(&mut *conn)
        .await
        .expect("empty search_path");
    assert_refused(
        sqlx::query("UPDATE public.workspaces SET name = $1 WHERE id = $2")
            .bind("House\tHold")
            .bind(legacy.household)
            .execute(&mut *conn)
            .await,
        "workspaces_name_no_control_chars",
    );
    assert_refused(
        sqlx::query("UPDATE public.projects SET name = $1 WHERE id = $2")
            .bind("Sandbox ")
            .bind(legacy.sandbox)
            .execute(&mut *conn)
            .await,
        "projects_name_trimmed",
    );
}

// ---------------------------------------------------------------------------
// Earned Trust at apply time (DDD-8)
// ---------------------------------------------------------------------------

/// DDD-8 / OQ-D1: 0019 refuses a database whose encoding is not UTF8, inside its
/// own transaction: nothing of the rule is left and version 19 is not recorded.
#[tokio::test]
async fn migration_0019_refuses_a_database_that_is_not_utf8() {
    let c = container().await;
    let admin = connect(&c, "postgres").await;
    create_database(
        &admin,
        "latin",
        "ENCODING 'SQL_ASCII' LC_COLLATE 'C' LC_CTYPE 'C' TEMPLATE template0",
    )
    .await;
    let pool = connect(&c, "latin").await;
    stand_at_0018(&pool).await;
    assert!(
        production_migrations_dir()
            .join("0019_workspace_name_rule.sql")
            .is_file(),
        "RED gate: migration 0019_workspace_name_rule.sql does not exist yet (DDD-7/8)"
    );
    let outcome = run_migrations(&pool).await;
    assert!(
        outcome.is_err(),
        "0019 must refuse a SQL_ASCII database (DDD-8); it applied"
    );
    let (verdict, latest): (bool, i64) = sqlx::query_as(
        "SELECT to_regprocedure('foundry_name_rule_violation(text, integer)') IS NOT NULL,
                (SELECT max(version) FROM _sqlx_migrations WHERE success)",
    )
    .fetch_one(&pool)
    .await
    .expect("read what was left");
    assert!(
        !verdict,
        "the refused migration leaves no verdict function behind"
    );
    assert_eq!(latest, 18, "version 19 is not recorded");
}

/// Every `DO $tag$ … $tag$;` block of a migration file, verbatim.
fn do_blocks(sql: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut rest = sql;
    while let Some(at) = rest.find("DO $") {
        let after = &rest[at + 3..];
        let Some(tag_len) = after[1..].find('$').map(|i| i + 2) else {
            break;
        };
        let tag = &after[..tag_len];
        let body_start = at + 3 + tag_len;
        let Some(close) = rest[body_start..].find(tag) else {
            break;
        };
        let end = body_start + close + tag_len;
        blocks.push(format!("{};", &rest[at..end]));
        rest = &rest[end..];
    }
    blocks
}

/// DDD-8: the apply-time self-check really checks the verdict function. Replaced
/// by one that passes everything (a drifted substrate's answer), each migration's
/// self-check must refuse.
#[tokio::test]
async fn the_apply_time_self_check_refuses_a_verdict_function_that_passes_everything() {
    let (pool, _c) = migrated().await;
    require_rule_installed(&pool).await;
    for file in ["0019_workspace_name_rule.sql", "0020_project_name_rule.sql"] {
        let sql = std::fs::read_to_string(production_migrations_dir().join(file))
            .unwrap_or_else(|e| panic!("read {file}: {e}"));
        let blocks = do_blocks(&sql);
        assert!(
            !blocks.is_empty(),
            "{file} carries a self-check DO block (DDD-8)"
        );
        let mut tx = pool.begin().await.expect("begin");
        sqlx::query(
            "CREATE OR REPLACE FUNCTION foundry_name_rule_violation(name text, max_chars integer)
             RETURNS text LANGUAGE sql IMMUTABLE AS $$ SELECT NULL::text $$",
        )
        .execute(&mut *tx)
        .await
        .expect("drift the verdict function inside the transaction");
        let mut refused = false;
        for block in &blocks {
            if sqlx::raw_sql(block).execute(&mut *tx).await.is_err() {
                refused = true;
                break;
            }
        }
        tx.rollback().await.expect("rollback the drift");
        assert!(
            refused,
            "{file}'s self-check must refuse a verdict function that passes everything"
        );
    }
}

// ---------------------------------------------------------------------------
// The legacy seam (DDD-10)
// ---------------------------------------------------------------------------

/// DDD-10, bounded change: the seam stores exactly the caller's row — even one the
/// rule refuses — and both of the table's triggers are enabled again afterwards,
/// also when the write itself fails.
#[tokio::test]
async fn the_legacy_seam_stores_exactly_the_callers_row_and_leaves_both_triggers_enabled() {
    let (pool, _c) = migrated().await;
    require_rule_installed(&pool).await;
    let before = names(&pool).await;
    let id = uuid::Uuid::now_v7();
    seam_insert_workspace(&pool, id, LEGACY_LONG)
        .await
        .expect("the seam stores a name the rule refuses");
    let mut expected = before.clone();
    expected.workspaces.insert(id, LEGACY_LONG.to_string());
    assert_eq!(names(&pool).await, expected, "exactly the caller's row");
    assert_eq!(triggers(&pool).await, all_four_enabled());

    let team = insert_team(&pool, id).await;
    let project = project_seed(id, team, LEGACY_TAB_PROJECT, "OPS");
    seam_insert_project(&pool, &project)
        .await
        .expect("the seam stores a legacy project");
    assert_eq!(triggers(&pool).await, all_four_enabled());

    let orphan = project_seed(id, uuid::Uuid::now_v7(), "Orphan", "ORP");
    assert!(
        seam_insert_project(&pool, &orphan).await.is_err(),
        "the seam switches off the two name triggers only, never the foreign keys \
         (DDD-10: the two name triggers by name, never all of them)"
    );

    let failing = seam_insert_workspace(&pool, id, "Duplicate id").await;
    assert!(failing.is_err(), "a duplicate id fails inside the seam");
    assert_eq!(
        triggers(&pool).await,
        all_four_enabled(),
        "a failed seam write leaves both triggers enabled (the transaction rolls back)"
    );
    assert_refused(
        rename(&pool, "workspaces", id, "Another\tname").await,
        "workspaces_name_no_control_chars",
    );
}

// ---------------------------------------------------------------------------
// Slice 03 — restore and rollback (DDD-14)
// ---------------------------------------------------------------------------

/// Run a client tool inside the database container; `(exit, stdout, stderr)`.
async fn in_container(c: &ContainerAsync<Postgres>, args: &[&str]) -> (i64, String, String) {
    let mut exec = c
        .exec(ExecCommand::new(args.iter().map(|a| a.to_string())))
        .await
        .unwrap_or_else(|e| panic!("exec {args:?}: {e}"));
    let stdout = exec.stdout_to_vec().await.expect("stdout");
    let stderr = exec.stderr_to_vec().await.expect("stderr");
    let mut code = exec.exit_code().await.expect("exit code");
    for _ in 0..50 {
        if code.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        code = exec.exit_code().await.expect("exit code");
    }
    (
        code.unwrap_or(-1),
        String::from_utf8_lossy(&stdout).into_owned(),
        String::from_utf8_lossy(&stderr).into_owned(),
    )
}

async fn dump(c: &ContainerAsync<Postgres>, database: &str, file: &str) {
    let (code, _, err) = in_container(
        c,
        &[
            "pg_dump", "-U", "postgres", "-Fc", "-f", file, "-d", database,
        ],
    )
    .await;
    assert_eq!(code, 0, "pg_dump {database}: {err}");
}

async fn restore(c: &ContainerAsync<Postgres>, database: &str, file: &str) {
    let (code, _, err) = in_container(
        c,
        &[
            "pg_restore",
            "-U",
            "postgres",
            "--clean",
            "--if-exists",
            "-d",
            database,
            file,
        ],
    )
    .await;
    assert_eq!(
        code, 0,
        "pg_restore --clean --if-exists into {database} must exit 0 (D9): {err}"
    );
}

/// DDD-14 (1): a post-0020 dump holding legacy rows (seeded through the seam)
/// restores with exit 0, names byte-identical, the four triggers enabled, and a
/// bad write is still refused.
#[tokio::test]
async fn a_post_upgrade_dump_holding_legacy_rows_restores_byte_identical_with_the_rule_on() {
    let (pool, c) = migrated().await;
    require_rule_installed(&pool).await;
    let ws = uuid::Uuid::now_v7();
    seam_insert_workspace(&pool, ws, LEGACY_LONG)
        .await
        .expect("legacy workspace");
    let team = insert_team(&pool, ws).await;
    let project = project_seed(ws, team, LEGACY_TAB_PROJECT, "OPS");
    seam_insert_project(&pool, &project)
        .await
        .expect("legacy project");
    let source = names(&pool).await;

    dump(&c, "postgres", "/tmp/post.dump").await;
    create_database(&pool, "restored", "").await;
    restore(&c, "restored", "/tmp/post.dump").await;
    let restored = connect(&c, "restored").await;

    assert_eq!(names(&restored).await, source, "names byte-identical (D9)");
    assert_eq!(triggers(&restored).await, all_four_enabled());
    assert_refused(
        rename(&restored, "workspaces", ws, "House\tHold").await,
        "workspaces_name_no_control_chars",
    );
}

/// DDD-14 (2): a pre-0019 dump (schema at 0018, legacy rows) restores over a
/// post-0020 database; the leftover functions are not in its archive, and the next
/// boot re-applies 0019/0020 over them cleanly. The legacy rows are untouched and
/// the rule holds.
#[tokio::test]
async fn a_pre_upgrade_dump_restores_over_a_migrated_database_and_the_next_boot_reapplies_the_rule()
{
    let c = container().await;
    let admin = connect(&c, "postgres").await;
    create_database(&admin, "before_upgrade", "").await;
    let old = connect(&c, "before_upgrade").await;
    stand_at_0018(&old).await;
    let legacy = insert_legacy_rows(&old).await;
    let source = names(&old).await;
    dump(&c, "before_upgrade", "/tmp/pre.dump").await;

    run_migrations(&admin)
        .await
        .expect("the target is fully migrated");
    restore(&c, "postgres", "/tmp/pre.dump").await;
    run_migrations(&admin)
        .await
        .expect("the next boot re-applies 0019/0020 over the leftover functions");
    run_migrations(&admin)
        .await
        .expect("and is a no-op after that");

    require_rule_installed(&admin).await;
    assert_eq!(
        names(&admin).await,
        source,
        "the legacy rows are untouched (D9)"
    );
    assert_eq!(triggers(&admin).await, all_four_enabled());
    assert_refused(
        rename(&admin, "workspaces", legacy.household, "House\tHold").await,
        "workspaces_name_no_control_chars",
    );
}

/// DDD-14 (4), D10: the previous release (v0.11.0) boots against the migrated
/// database — its boot loop ignores 19 and 20 — its probe passes, and it writes
/// valid names.
#[tokio::test]
async fn the_previous_release_boots_against_the_migrated_database_and_writes_valid_names() {
    let (pool, _c) = migrated().await;
    require_rule_installed(&pool).await;
    let (applied, already_applied) = previous_release_boot(&pool, &staged_migrations("0018"))
        .await
        .expect("the old boot loop is a no-op success over 0019/0020");
    assert_eq!(
        applied,
        Vec::<i64>::new(),
        "the old boot applies nothing over a database migrated through 0020"
    );
    assert_eq!(
        already_applied,
        (1..=18).collect::<Vec<i64>>(),
        "the old boot finds every migration it knows already applied, and never sees 0019/0020"
    );
    let store = Store::from_pool(pool.clone());
    store.probe().await.expect("the old probe passes (DDD-9)");
    let workspace = uuid::Uuid::now_v7();
    store
        .provision_workspace(
            workspace,
            "Globex",
            uuid::Uuid::now_v7(),
            "dana@globex.test",
            "dana@globex.test",
            "Dana",
            "not-a-real-hash",
            uuid::Uuid::now_v7(),
            time::OffsetDateTime::now_utc() + time::Duration::days(7),
        )
        .await
        .expect("the old release provisions a valid workspace");
    let team = insert_team(&pool, workspace).await;
    store
        .insert_project(
            uuid::Uuid::now_v7(),
            workspace,
            team,
            "Homelab Ops",
            "homelab-ops",
            "HLO",
        )
        .await
        .expect("the old release creates a valid project");
    assert_eq!(
        triggers(&pool).await,
        all_four_enabled(),
        "the rule is untouched"
    );
}

/// DDD-15: the documented undo (drop the four triggers, then both functions)
/// leaves a healthy store that writes any name; the documented re-arm (forget
/// versions 19 and 20, boot) brings the rule back over the rows written meanwhile.
#[tokio::test]
async fn the_documented_undo_and_rearm_work() {
    let (pool, _c) = migrated().await;
    require_rule_installed(&pool).await;
    sqlx::raw_sql(
        "DROP TRIGGER IF EXISTS workspaces_name_rule_on_insert ON workspaces;
         DROP TRIGGER IF EXISTS workspaces_name_rule_on_rename ON workspaces;
         DROP TRIGGER IF EXISTS projects_name_rule_on_insert ON projects;
         DROP TRIGGER IF EXISTS projects_name_rule_on_rename ON projects;
         DROP FUNCTION IF EXISTS foundry_enforce_name_rule(), foundry_name_rule_violation(text, integer);",
    )
    .execute(&pool)
    .await
    .expect("the documented undo runs as written");
    let store = Store::from_pool(pool.clone());
    store
        .probe()
        .await
        .expect("the probe does not require the rule (DDD-9)");
    let during = insert_workspace(&pool, "Written\tduring the undo")
        .await
        .expect("with the rule undone, any name is stored");

    sqlx::query("DELETE FROM _sqlx_migrations WHERE version IN (19, 20)")
        .execute(&pool)
        .await
        .expect("forget 19 and 20");
    run_migrations(&pool)
        .await
        .expect("the re-runnable bodies re-arm the rule (DDD-7)");
    require_rule_installed(&pool).await;
    assert_eq!(triggers(&pool).await, all_four_enabled());
    assert_eq!(
        names(&pool).await.workspaces[&during],
        "Written\tduring the undo",
        "re-arming scans and rewrites nothing"
    );
    assert_refused(
        rename(&pool, "workspaces", during, "Still\tbad").await,
        "workspaces_name_no_control_chars",
    );
}

#[test]
fn do_blocks_are_cut_at_their_own_dollar_tag() {
    let sql = "CREATE FUNCTION f() RETURNS int AS $$ SELECT 1 $$;\n\
               DO $check$ BEGIN PERFORM $x$a$x$; END $check$;\n\
               DO $$ BEGIN NULL; END $$;\n";
    assert_eq!(
        do_blocks(sql),
        vec![
            "DO $check$ BEGIN PERFORM $x$a$x$; END $check$;".to_string(),
            "DO $$ BEGIN NULL; END $$;".to_string(),
        ]
    );
}
