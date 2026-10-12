//! team-lifecycle — a team delete never destroys a project, at the store and in
//! the database (DISCUSS D8, D9, D10, OQ-4 (a), INV-TL-1, INV-TL-2, INV-TL-3,
//! INV-TL-8, INV-TL-9; DESIGN DDD-7 to DDD-11; ADR-TEAM-LIFECYCLE-002/003).
//! DISTILL 2026-10-11.
//!
//! The gold tests DESIGN put at store level (Test Seams): the races are
//! deterministic here (a barrier or a held lock, two connections) and the
//! barrier race runs 100 times, which the HTTP lane cannot afford.
//!
//! Never ignored, and green since DISTILL: the Postgres substrate facts the delete
//! relies on (ADR-002 "Earned Trust"), and the two NEGATIVE CONTROLS DESIGN
//! names. The controls run the two wrong mechanisms by hand and show each one
//! losing a project: (a) `FOR NO KEY UPDATE` then a check, and (b) one
//! `DELETE … WHERE NOT EXISTS (…)`. They pin why `delete_team_if_empty` must
//! lock `FOR UPDATE` and then read the projects in a separate statement. If a
//! control ever stops losing the project, the server's semantics moved and the
//! ADR's argument must be re-made.
//!
//! Authored as RED scaffolds in DISTILL 2026-10-11 (`delete_team_if_empty` and
//! `team_delete_facts` panicked, migration 0023 did not exist, and every test
//! that needed them was ignored as pending); un-ignored in DELIVER 02-01, when
//! 0023, the guarded delete and the 23503 mappings shipped. Tests that need the
//! record table still pass `require_lifecycle_records_installed` first, a
//! readable gate. The INV-TL-2 mapping tests name the `TeamGone` outcome
//! (DDD-9) through its `Debug` name, as written in DISTILL; the test that a
//! 23503 on another foreign key is NOT `TeamGone` was added at finalize.
//!
//! Real Postgres (testcontainers `postgres:16-alpine`, matching production);
//! integration level, example-based, with the race repeated (Mandates 9/11).

use foundry_store::{
    run_migrations, AddTeamMemberOutcome, Store, TeamDeleteFacts, TeamDeleteOutcome,
};
use sqlx::postgres::{PgDatabaseError, PgPoolOptions};
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
use tokio::sync::Barrier;

/// How many times the barrier race runs (DoD 3, KPI-TL-3).
const RACE_RUNS: usize = 100;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

async fn container() -> ContainerAsync<Postgres> {
    Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container")
}

async fn connect(c: &ContainerAsync<Postgres>) -> PgPool {
    let host = c.get_host().await.expect("container host");
    let port = c.get_host_port_ipv4(5432).await.expect("container port");
    PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&format!(
            "postgres://postgres:postgres@{host}:{port}/postgres"
        ))
        .await
        .expect("connect pool")
}

/// RED gate: migration 0023 has created the lifecycle record table.
async fn require_lifecycle_records_installed(pool: &PgPool) {
    let (present,): (bool,) =
        sqlx::query_as("SELECT to_regclass('team_lifecycle_events') IS NOT NULL")
            .fetch_one(pool)
            .await
            .expect("look the record table up");
    assert!(
        present,
        "RED gate: migration 0023 has not created team_lifecycle_events yet \
         (team-lifecycle DDD-11, ADR-TEAM-LIFECYCLE-003)"
    );
}

// ---------------------------------------------------------------------------
// Rows
// ---------------------------------------------------------------------------

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

async fn team(pool: &PgPool, ws: uuid::Uuid, name: &str, slug: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind(ws)
        .bind(name)
        .bind(slug)
        .execute(pool)
        .await
        .expect("insert team");
    id
}

async fn person(pool: &PgPool, ws: uuid::Uuid, email: &str, role: &str) -> uuid::Uuid {
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
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, $3)",
    )
    .bind(ws)
    .bind(id)
    .bind(role)
    .execute(pool)
    .await
    .expect("insert workspace membership");
    id
}

async fn place(pool: &PgPool, team_id: uuid::Uuid, user_id: uuid::Uuid, role: &str) {
    sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, $3)
             ON CONFLICT (team_id, user_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(team_id)
    .bind(user_id)
    .bind(role)
    .execute(pool)
    .await
    .expect("place someone on a team");
}

fn project_sql() -> &'static str {
    "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix) VALUES ($1, $2, $3, $4, $5, $6)"
}

async fn project(pool: &PgPool, ws: uuid::Uuid, team_id: uuid::Uuid, name: &str, key: &str) {
    sqlx::query(project_sql())
        .bind(uuid::Uuid::now_v7())
        .bind(team_id)
        .bind(ws)
        .bind(name)
        .bind(name.to_ascii_lowercase().replace(' ', "-"))
        .bind(key)
        .execute(pool)
        .await
        .expect("insert project");
}

async fn token(
    pool: &PgPool,
    ws: uuid::Uuid,
    user: uuid::Uuid,
    scope: Option<uuid::Uuid>,
    label: &str,
) {
    sqlx::query(
        "INSERT INTO machine_tokens (jti, user_id, workspace_id, scope_team_id, expires_at, label, created_by)
              VALUES ($1, $2, $3, $4, now() + interval '30 days', $5, $2)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(user)
    .bind(ws)
    .bind(scope)
    .bind(label)
    .execute(pool)
    .await
    .expect("insert machine token");
}

async fn set_default(pool: &PgPool, ws: uuid::Uuid, team_id: uuid::Uuid) {
    sqlx::query("UPDATE workspaces SET default_team_id = $2 WHERE id = $1")
        .bind(ws)
        .bind(team_id)
        .execute(pool)
        .await
        .expect("set the default team");
}

async fn count(pool: &PgPool, sql: &str, id: uuid::Uuid) -> i64 {
    let (n,): (i64,) = sqlx::query_as(sql)
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("count rows");
    n
}

async fn projects_of(pool: &PgPool, team_id: uuid::Uuid) -> i64 {
    count(
        pool,
        "SELECT count(*) FROM projects WHERE team_id = $1",
        team_id,
    )
    .await
}

async fn team_exists(pool: &PgPool, team_id: uuid::Uuid) -> bool {
    count(pool, "SELECT count(*) FROM teams WHERE id = $1", team_id).await == 1
}

/// Every row a delete may touch, by table (the universe of DDD's Contract Shapes).
async fn snapshot(pool: &PgPool) -> BTreeMap<&'static str, Vec<String>> {
    let mut out = BTreeMap::new();
    for (table, sql) in [
        ("teams", "SELECT id::text || ':' || name || ':' || slug FROM teams ORDER BY 1"),
        (
            "team_memberships",
            "SELECT team_id::text || ':' || user_id::text || ':' || role FROM team_memberships ORDER BY 1",
        ),
        ("projects", "SELECT id::text || ':' || team_id::text || ':' || name FROM projects ORDER BY 1"),
        (
            "machine_tokens",
            "SELECT jti::text || ':' || coalesce(scope_team_id::text, '-') FROM machine_tokens ORDER BY 1",
        ),
        (
            "defaults",
            "SELECT id::text || ':' || coalesce(default_team_id::text, '-') FROM workspaces ORDER BY 1",
        ),
    ] {
        let rows: Vec<(String,)> = sqlx::query_as(sql).fetch_all(pool).await.expect("snapshot");
        out.insert(table, rows.into_iter().map(|(r,)| r).collect());
    }
    let (has_records,): (bool,) =
        sqlx::query_as("SELECT to_regclass('team_lifecycle_events') IS NOT NULL")
            .fetch_one(pool)
            .await
            .expect("look the records up");
    let records: Vec<(String,)> = if has_records {
        sqlx::query_as(
            "SELECT kind || ':' || team_id::text || ':' || team_slug || ':' || name || ':' ||
                    coalesce(new_name, '-') || ':' || actor_id::text
               FROM team_lifecycle_events ORDER BY 1",
        )
        .fetch_all(pool)
        .await
        .expect("snapshot records")
    } else {
        Vec::new()
    };
    out.insert("records", records.into_iter().map(|(r,)| r).collect());
    out
}

fn assert_fk(err: &sqlx::Error, constraint: &str) {
    let db = err
        .as_database_error()
        .unwrap_or_else(|| panic!("expected the database's refusal, got {err:?}"));
    let pg = db.downcast_ref::<PgDatabaseError>();
    assert_eq!(pg.code(), "23503", "a foreign key violation: {pg:?}");
    assert_eq!(
        pg.constraint(),
        Some(constraint),
        "named constraint: {pg:?}"
    );
}

/// The outcome DDD-9 adds, named through its `Debug` (no shipped enum changes
/// before DELIVER).
fn names_team_gone<T: std::fmt::Debug>(result: &T) -> bool {
    format!("{result:?}").contains("TeamGone")
}

/// Canzan Labs with Design (no projects; Lena leads, Tomás is a member) and
/// Backend (one project), Priya the workspace admin, Globex with Payments.
struct Labs {
    pool: PgPool,
    store: Store,
    ws: uuid::Uuid,
    design: uuid::Uuid,
    backend: uuid::Uuid,
    payments: uuid::Uuid,
    priya: uuid::Uuid,
    lena: uuid::Uuid,
    tomas: uuid::Uuid,
    _c: ContainerAsync<Postgres>,
}

async fn labs() -> Labs {
    let c = container().await;
    let pool = connect(&c).await;
    run_migrations(&pool).await.expect("apply every migration");
    let store = Store::from_pool(pool.clone());
    let ws = workspace(&pool, "Canzan Labs").await;
    let design = team(&pool, ws, "Design", "design").await;
    let backend = team(&pool, ws, "Backend", "backend").await;
    let priya = person(&pool, ws, "priya.raman@canzan.dev", "admin").await;
    let lena = person(&pool, ws, "lena.park@canzan.dev", "member").await;
    let tomas = person(&pool, ws, "tomas.rivera@canzan.dev", "member").await;
    place(&pool, design, lena, "lead").await;
    place(&pool, design, tomas, "member").await;
    place(&pool, backend, lena, "lead").await;
    project(&pool, ws, backend, "Identity Platform", "AUTH").await;
    let globex = workspace(&pool, "Globex").await;
    let payments = team(&pool, globex, "Payments", "payments").await;
    Labs {
        pool,
        store,
        ws,
        design,
        backend,
        payments,
        priya,
        lena,
        tomas,
        _c: c,
    }
}

// ---------------------------------------------------------------------------
// Substrate facts (ADR-002 Earned Trust) — GREEN today
// ---------------------------------------------------------------------------

/// `FOR UPDATE` on the team row conflicts with the `KEY SHARE` a project
/// insert's foreign-key check takes: the insert waits.
#[tokio::test]
async fn a_project_insert_waits_while_a_delete_holds_the_team_row_for_update() {
    let l = labs().await;
    let mut delete = l.pool.begin().await.expect("begin");
    sqlx::query("SELECT 1 FROM teams WHERE id = $1 FOR UPDATE")
        .bind(l.design)
        .execute(&mut *delete)
        .await
        .expect("lock the team row");
    let insert = sqlx::query(project_sql())
        .bind(uuid::Uuid::now_v7())
        .bind(l.design)
        .bind(l.ws)
        .bind("Brand Refresh")
        .bind("brand-refresh")
        .bind("BRAND")
        .execute(&l.pool);
    let waited = tokio::time::timeout(Duration::from_secs(2), insert).await;
    assert!(
        waited.is_err(),
        "the project insert must wait on the team lock: {waited:?}"
    );
    delete.rollback().await.expect("rollback");
}

/// `FOR NO KEY UPDATE` does NOT conflict with `KEY SHARE`: a project insert is
/// not held up by it (why negative control (a) loses a project).
#[tokio::test]
async fn a_project_insert_is_not_held_up_by_for_no_key_update() {
    let l = labs().await;
    let mut weak = l.pool.begin().await.expect("begin");
    sqlx::query("SELECT 1 FROM teams WHERE id = $1 FOR NO KEY UPDATE")
        .bind(l.design)
        .execute(&mut *weak)
        .await
        .expect("take the weaker lock");
    let insert = sqlx::query(project_sql())
        .bind(uuid::Uuid::now_v7())
        .bind(l.design)
        .bind(l.ws)
        .bind("Brand Refresh")
        .bind("brand-refresh")
        .bind("BRAND")
        .execute(&l.pool);
    let done = tokio::time::timeout(Duration::from_secs(3), insert).await;
    assert!(
        matches!(done, Ok(Ok(_))),
        "the insert must not wait: {done:?}"
    );
    weak.rollback().await.expect("rollback");
}

/// At READ COMMITTED, a statement run after the `FOR UPDATE` wait sees the
/// project the `KEY SHARE` holder committed: the delete's fresh read is exact.
#[tokio::test]
async fn a_read_after_the_team_lock_wait_sees_a_project_committed_meanwhile() {
    let l = labs().await;
    let mut create = l.pool.begin().await.expect("begin create");
    sqlx::query(project_sql())
        .bind(uuid::Uuid::now_v7())
        .bind(l.design)
        .bind(l.ws)
        .bind("Brand Refresh")
        .bind("brand-refresh")
        .bind("BRAND")
        .execute(&mut *create)
        .await
        .expect("the create holds KEY SHARE on Design");
    let pool = l.pool.clone();
    let design = l.design;
    let delete = tokio::spawn(async move {
        let mut tx = pool.begin().await.expect("begin delete");
        sqlx::query("SELECT 1 FROM teams WHERE id = $1 FOR UPDATE")
            .bind(design)
            .execute(&mut *tx)
            .await
            .expect("the delete waits for the lock");
        let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM projects WHERE team_id = $1")
            .bind(design)
            .fetch_one(&mut *tx)
            .await
            .expect("fresh read");
        tx.rollback().await.expect("rollback delete");
        n
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    create.commit().await.expect("the create commits");
    assert_eq!(
        delete.await.expect("delete task"),
        1,
        "the fresh read sees the project"
    );
}

/// The constraint names DDD-9 maps, as Postgres reports them on 23503.
#[tokio::test]
async fn every_child_write_to_a_deleted_team_reports_its_foreign_key_by_name() {
    let l = labs().await;
    sqlx::query("DELETE FROM teams WHERE id = $1")
        .bind(l.design)
        .execute(&l.pool)
        .await
        .expect("delete Design by hand");
    let err = sqlx::query(project_sql())
        .bind(uuid::Uuid::now_v7())
        .bind(l.design)
        .bind(l.ws)
        .bind("Brand Refresh")
        .bind("brand-refresh")
        .bind("BRAND")
        .execute(&l.pool)
        .await
        .expect_err("a project on a deleted team");
    assert_fk(&err, "projects_team_id_fkey");
    let err = sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(l.design)
    .bind(l.priya)
    .execute(&l.pool)
    .await
    .expect_err("a member of a deleted team");
    assert_fk(&err, "team_memberships_team_id_fkey");
    let err = sqlx::query("UPDATE workspaces SET default_team_id = $2 WHERE id = $1")
        .bind(l.ws)
        .bind(l.design)
        .execute(&l.pool)
        .await
        .expect_err("a deleted team as the default");
    assert_fk(&err, "workspaces_default_team_same_workspace");
    let err = sqlx::query(
        "INSERT INTO machine_tokens (jti, user_id, workspace_id, scope_team_id, expires_at, label, created_by)
              VALUES ($1, $2, $3, $4, now() + interval '1 day', 'bot', $2)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(l.priya)
    .bind(l.ws)
    .bind(l.design)
    .execute(&l.pool)
    .await
    .expect_err("a token scoped to a deleted team");
    assert_fk(&err, "machine_tokens_scope_team_id_fkey");
}

/// The schema cascade the delete relies on (D10): memberships (0021's
/// exemption, even for a sole lead) and scoped tokens go, the default clears,
/// and nothing of another team moves.
#[tokio::test]
async fn deleting_a_led_team_by_hand_cascades_its_rows_and_clears_only_its_default() {
    let l = labs().await;
    token(&l.pool, l.ws, l.priya, Some(l.design), "design bot").await;
    token(&l.pool, l.ws, l.priya, None, "workspace bot").await;
    set_default(&l.pool, l.ws, l.design).await;
    sqlx::query("DELETE FROM teams WHERE id = $1")
        .bind(l.design)
        .execute(&l.pool)
        .await
        .expect("a sole-lead team deletes (0021 exemption)");
    assert_eq!(
        count(
            &l.pool,
            "SELECT count(*) FROM team_memberships WHERE team_id = $1",
            l.design
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &l.pool,
            "SELECT count(*) FROM machine_tokens WHERE scope_team_id = $1",
            l.design
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &l.pool,
            "SELECT count(*) FROM machine_tokens WHERE workspace_id = $1",
            l.ws
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &l.pool,
            "SELECT count(*) FROM workspaces WHERE id = $1 AND default_team_id IS NULL",
            l.ws
        )
        .await,
        1,
        "the default cleared"
    );
    assert_eq!(
        count(
            &l.pool,
            "SELECT count(*) FROM team_memberships WHERE team_id = $1",
            l.backend
        )
        .await,
        1
    );
    let _ = (l.lena, l.tomas, l.payments, &l.store);
}

// ---------------------------------------------------------------------------
// NEGATIVE CONTROLS (ADR-002 decision 3) — GREEN today: each wrong mechanism
// loses a project
// ---------------------------------------------------------------------------

/// Control (a): `FOR NO KEY UPDATE`, then the "no projects" check, then the
/// delete. A project committed between the check and the delete is cascaded.
#[tokio::test]
async fn negative_control_for_no_key_update_then_check_loses_a_project() {
    let l = labs().await;
    let mut delete = l.pool.begin().await.expect("begin");
    sqlx::query("SELECT 1 FROM teams WHERE id = $1 FOR NO KEY UPDATE")
        .bind(l.design)
        .execute(&mut *delete)
        .await
        .expect("the wrong lock");
    let (seen,): (i64,) = sqlx::query_as("SELECT count(*) FROM projects WHERE team_id = $1")
        .bind(l.design)
        .fetch_one(&mut *delete)
        .await
        .expect("the check");
    assert_eq!(seen, 0, "the check sees no project");
    sqlx::query(project_sql())
        .bind(uuid::Uuid::now_v7())
        .bind(l.design)
        .bind(l.ws)
        .bind("Brand Refresh")
        .bind("brand-refresh")
        .bind("BRAND")
        .execute(&l.pool)
        .await
        .expect("Lena's create commits: the weaker lock does not stop it");
    assert_eq!(
        projects_of(&l.pool, l.design).await,
        1,
        "the project exists"
    );
    sqlx::query("DELETE FROM teams WHERE id = $1")
        .bind(l.design)
        .execute(&mut *delete)
        .await
        .expect("the delete proceeds");
    delete.commit().await.expect("commit");
    assert_eq!(
        projects_of(&l.pool, l.design).await,
        0,
        "control (a) LOSES the project: FOR NO KEY UPDATE must never guard a team delete"
    );
}

/// Control (b): one `DELETE … WHERE NOT EXISTS (projects)`. It waits on the
/// create's KEY SHARE, but its NOT EXISTS keeps the statement's starting
/// snapshot and nothing re-checks it; the RI cascade then deletes the project.
#[tokio::test]
async fn negative_control_single_delete_where_not_exists_loses_a_project() {
    let l = labs().await;
    let mut create = l.pool.begin().await.expect("begin create");
    sqlx::query(project_sql())
        .bind(uuid::Uuid::now_v7())
        .bind(l.design)
        .bind(l.ws)
        .bind("Brand Refresh")
        .bind("brand-refresh")
        .bind("BRAND")
        .execute(&mut *create)
        .await
        .expect("the create holds KEY SHARE on Design");
    let pool = l.pool.clone();
    let design = l.design;
    let delete = tokio::spawn(async move {
        sqlx::query(
            "DELETE FROM teams WHERE id = $1
                AND NOT EXISTS (SELECT 1 FROM projects WHERE team_id = $1)",
        )
        .bind(design)
        .execute(&pool)
        .await
        .expect("the single-statement delete")
        .rows_affected()
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    create.commit().await.expect("the create commits");
    let deleted = delete.await.expect("delete task");
    assert_eq!(deleted, 1, "the single statement deleted the team");
    assert_eq!(
        count(&l.pool, "SELECT count(*) FROM projects WHERE name = 'Brand Refresh' AND workspace_id = $1", l.ws).await,
        0,
        "control (b) LOSES the committed project: the lock and the check must be separate statements"
    );
}

// ---------------------------------------------------------------------------
// Slice 02: the guarded delete (02-01)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_confirm_facts_count_members_default_tokens_and_projects() {
    let l = labs().await;
    require_lifecycle_records_installed(&l.pool).await;
    token(&l.pool, l.ws, l.priya, Some(l.design), "design bot").await;
    set_default(&l.pool, l.ws, l.design).await;
    let before = snapshot(&l.pool).await;
    let facts = l.store.team_delete_facts(l.design).await.expect("no error");
    assert_eq!(
        facts,
        TeamDeleteFacts {
            member_count: 2,
            is_default: true,
            scoped_token_count: 1,
            projects: vec![],
        }
    );
    let backend = l
        .store
        .team_delete_facts(l.backend)
        .await
        .expect("no error");
    assert_eq!(
        backend.projects,
        vec![("Identity Platform".to_string(), "AUTH".to_string())]
    );
    assert_eq!(
        snapshot(&l.pool).await,
        before,
        "the confirm read writes nothing"
    );
}

#[tokio::test]
async fn a_team_with_no_projects_is_deleted_with_its_rows_tokens_and_default_and_one_record() {
    let l = labs().await;
    require_lifecycle_records_installed(&l.pool).await;
    token(&l.pool, l.ws, l.priya, Some(l.design), "design bot").await;
    set_default(&l.pool, l.ws, l.design).await;
    let before = snapshot(&l.pool).await;
    let outcome = l
        .store
        .delete_team_if_empty(l.ws, l.design, l.priya)
        .await
        .expect("no error");
    assert_eq!(
        outcome,
        TeamDeleteOutcome::Deleted {
            name: "Design".to_string(),
            slug: "design".to_string()
        }
    );
    let after = snapshot(&l.pool).await;
    let d = l.design.to_string();
    let mut expected = before.clone();
    expected
        .get_mut("teams")
        .unwrap()
        .retain(|r| !r.starts_with(&d));
    expected
        .get_mut("team_memberships")
        .unwrap()
        .retain(|r| !r.starts_with(&d));
    expected
        .get_mut("machine_tokens")
        .unwrap()
        .retain(|r| !r.ends_with(&d));
    for r in expected.get_mut("defaults").unwrap().iter_mut() {
        *r = r.replace(&d, "-");
    }
    expected.insert(
        "records",
        vec![format!("deleted:{d}:design:Design:-:{}", l.priya)],
    );
    assert_eq!(
        after, expected,
        "exactly the declared cascade plus one record"
    );
}

#[tokio::test]
async fn a_team_with_projects_is_refused_naming_each_by_name_then_key_and_nothing_changes() {
    let l = labs().await;
    require_lifecycle_records_installed(&l.pool).await;
    project(&l.pool, l.ws, l.design, "Reading List", "READ").await;
    project(&l.pool, l.ws, l.design, "Homelab Ops", "OPS").await;
    let before = snapshot(&l.pool).await;
    let outcome = l
        .store
        .delete_team_if_empty(l.ws, l.design, l.priya)
        .await
        .expect("no error");
    assert_eq!(
        outcome,
        TeamDeleteOutcome::HasProjects {
            projects: vec![
                ("Homelab Ops".to_string(), "OPS".to_string()),
                ("Reading List".to_string(), "READ".to_string()),
            ]
        }
    );
    assert_eq!(snapshot(&l.pool).await, before, "a refusal writes nothing");
}

#[tokio::test]
async fn another_workspaces_team_or_an_unknown_team_is_not_found_and_nothing_changes() {
    let l = labs().await;
    require_lifecycle_records_installed(&l.pool).await;
    let before = snapshot(&l.pool).await;
    for team_id in [l.payments, uuid::Uuid::now_v7()] {
        let outcome = l
            .store
            .delete_team_if_empty(l.ws, team_id, l.priya)
            .await
            .expect("no error");
        assert_eq!(outcome, TeamDeleteOutcome::NotFound);
    }
    assert_eq!(snapshot(&l.pool).await, before, "nothing written");
}

/// INV-TL-9: the record commits with the delete or not at all. An actor the
/// record cannot name (no such user) rolls the whole delete back.
#[tokio::test]
async fn a_delete_whose_record_cannot_be_written_deletes_nothing() {
    let l = labs().await;
    require_lifecycle_records_installed(&l.pool).await;
    let before = snapshot(&l.pool).await;
    let result = l
        .store
        .delete_team_if_empty(l.ws, l.design, uuid::Uuid::now_v7())
        .await;
    assert!(result.is_err(), "the record's actor FK refuses: {result:?}");
    assert_eq!(
        snapshot(&l.pool).await,
        before,
        "the delete rolled back with its record"
    );
}

/// INV-TL-9: the record outlives the team (no FK to `teams`) and keeps its id,
/// name and slug as text.
#[tokio::test]
async fn the_delete_record_outlives_the_team_with_its_id_name_and_slug() {
    let l = labs().await;
    require_lifecycle_records_installed(&l.pool).await;
    l.store
        .delete_team_if_empty(l.ws, l.design, l.priya)
        .await
        .expect("no error");
    assert!(!team_exists(&l.pool, l.design).await);
    let row: (uuid::Uuid, String, String, String, Option<String>) = sqlx::query_as(
        "SELECT team_id, team_slug, name, kind, new_name FROM team_lifecycle_events WHERE team_id = $1",
    )
    .bind(l.design)
    .fetch_one(&l.pool)
    .await
    .expect("the record survives the team");
    assert_eq!(
        row,
        (
            l.design,
            "design".into(),
            "Design".into(),
            "deleted".into(),
            None
        )
    );
}

/// INV-TL-1, order "create first": a delete that waits on a create's KEY SHARE
/// reads the committed project afterwards and refuses, naming it.
#[tokio::test]
async fn a_delete_waiting_on_a_project_create_refuses_naming_the_new_project() {
    let l = labs().await;
    require_lifecycle_records_installed(&l.pool).await;
    let mut create = l.pool.begin().await.expect("begin create");
    sqlx::query(project_sql())
        .bind(uuid::Uuid::now_v7())
        .bind(l.design)
        .bind(l.ws)
        .bind("Brand Refresh")
        .bind("brand-refresh")
        .bind("BRAND")
        .execute(&mut *create)
        .await
        .expect("the create holds KEY SHARE on Design");
    let store = l.store.clone();
    let (ws, design, priya) = (l.ws, l.design, l.priya);
    let delete = tokio::spawn(async move { store.delete_team_if_empty(ws, design, priya).await });
    tokio::time::sleep(Duration::from_millis(300)).await;
    create.commit().await.expect("the create commits");
    let outcome = delete.await.expect("task").expect("no error");
    assert_eq!(
        outcome,
        TeamDeleteOutcome::HasProjects {
            projects: vec![("Brand Refresh".to_string(), "BRAND".to_string())]
        }
    );
    assert_eq!(
        projects_of(&l.pool, l.design).await,
        1,
        "the project is intact"
    );
}

/// INV-TL-1/2, order "delete first": a project create that waits on the
/// delete's lock finds the team gone and answers `TeamGone` (the uniform 404),
/// never an unmapped error.
#[tokio::test]
async fn a_project_create_waiting_on_a_delete_answers_team_gone() {
    let l = labs().await;
    let mut delete = l.pool.begin().await.expect("begin delete");
    sqlx::query("SELECT 1 FROM teams WHERE id = $1 FOR UPDATE")
        .bind(l.design)
        .execute(&mut *delete)
        .await
        .expect("lock");
    sqlx::query("DELETE FROM teams WHERE id = $1")
        .bind(l.design)
        .execute(&mut *delete)
        .await
        .expect("delete, not yet committed");
    let store = l.store.clone();
    let (ws, design) = (l.ws, l.design);
    let create = tokio::spawn(async move {
        store
            .insert_project(
                uuid::Uuid::now_v7(),
                ws,
                design,
                "Brand Refresh",
                "brand-refresh",
                "BRAND",
            )
            .await
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    delete.commit().await.expect("the delete commits");
    let result = create.await.expect("task");
    assert!(
        names_team_gone(&result),
        "the create maps 23503 projects_team_id_fkey to TeamGone (DDD-9); got {result:?}"
    );
}

/// INV-TL-2: an add or a default set racing the delete answers its team-gone
/// outcome (the uniform 404), never an unmapped error.
#[tokio::test]
async fn an_add_and_a_default_set_waiting_on_a_delete_answer_team_gone() {
    let l = labs().await;
    for writer in ["add", "default"] {
        let doomed = team(
            &l.pool,
            l.ws,
            &format!("Doomed {writer}"),
            &format!("doomed-{writer}"),
        )
        .await;
        let mut delete = l.pool.begin().await.expect("begin delete");
        sqlx::query("SELECT 1 FROM teams WHERE id = $1 FOR UPDATE")
            .bind(doomed)
            .execute(&mut *delete)
            .await
            .expect("lock");
        sqlx::query("DELETE FROM teams WHERE id = $1")
            .bind(doomed)
            .execute(&mut *delete)
            .await
            .expect("delete, not yet committed");
        let store = l.store.clone();
        let (ws, priya) = (l.ws, l.priya);
        let w = writer.to_string();
        let racing = tokio::spawn(async move {
            if w == "add" {
                format!("{:?}", store.add_team_member(ws, doomed, priya).await)
            } else {
                format!("{:?}", store.set_default_team(ws, Some(doomed)).await)
            }
        });
        tokio::time::sleep(Duration::from_millis(300)).await;
        delete.commit().await.expect("the delete commits");
        let result = racing.await.expect("task");
        assert!(
            result.contains("TeamGone"),
            "{writer}: 23503 maps to TeamGone (DDD-9); got {result}"
        );
    }
    let _ = AddTeamMemberOutcome::Added;
}

/// INV-TL-1 and KPI-TL-3: the barrier race, delete versus project create, 100
/// times. Every run ends with either the project and the team (the delete
/// refused) or neither (the create answered TeamGone). Never a lost project,
/// never an unmapped error.
#[tokio::test]
async fn the_delete_versus_project_create_race_never_loses_a_project_in_100_runs() {
    let l = labs().await;
    require_lifecycle_records_installed(&l.pool).await;
    let mut design = l.design;
    for run in 1..=RACE_RUNS {
        if !team_exists(&l.pool, design).await {
            design = team(&l.pool, l.ws, "Design", "design").await;
            place(&l.pool, design, l.lena, "lead").await;
        }
        sqlx::query("DELETE FROM projects WHERE team_id = $1")
            .bind(design)
            .execute(&l.pool)
            .await
            .expect("reset: no project");
        let barrier = Arc::new(Barrier::new(2));
        let (s1, s2, g1, g2) = (l.store.clone(), l.store.clone(), barrier.clone(), barrier);
        let (ws, priya) = (l.ws, l.priya);
        let project_id = uuid::Uuid::now_v7();
        let delete = tokio::spawn(async move {
            g1.wait().await;
            s1.delete_team_if_empty(ws, design, priya).await
        });
        let create = tokio::spawn(async move {
            g2.wait().await;
            s2.insert_project(
                project_id,
                ws,
                design,
                "Brand Refresh",
                "brand-refresh",
                "BRAND",
            )
            .await
        });
        let deleted = delete
            .await
            .expect("delete task")
            .expect("the delete never errors");
        let created = create.await.expect("create task");
        let project_kept = count(
            &l.pool,
            "SELECT count(*) FROM projects WHERE id = $1",
            project_id,
        )
        .await
            == 1;
        match (&deleted, created.is_ok()) {
            (TeamDeleteOutcome::HasProjects { projects }, true) => {
                assert!(project_kept, "run {run}: the refused delete kept the project");
                assert_eq!(projects, &vec![("Brand Refresh".to_string(), "BRAND".to_string())]);
            }
            (TeamDeleteOutcome::Deleted { .. }, false) => {
                assert!(
                    names_team_gone(&created),
                    "run {run}: the create lost the team and says so: {created:?}"
                );
            }
            other => panic!("run {run}: a project created and then lost, or an unmapped answer: {other:?} / {created:?}"),
        }
    }
}

/// DDD-9 maps exactly `projects_team_id_fkey` to `TeamGone`. A 23503 on another
/// foreign key of the same insert (here `projects_workspace_id_fkey`: a
/// workspace that does not exist) is not "the team is gone".
#[tokio::test]
async fn a_project_insert_refused_by_another_foreign_key_is_not_team_gone() {
    let l = labs().await;
    let result = l
        .store
        .insert_project(
            uuid::Uuid::now_v7(),
            uuid::Uuid::now_v7(),
            l.design,
            "Brand Refresh",
            "brand-refresh",
            "BRAND",
        )
        .await;
    assert!(result.is_err(), "an unknown workspace refuses the insert");
    assert!(
        !names_team_gone(&result),
        "a 23503 on projects_workspace_id_fkey must not read as TeamGone: {result:?}"
    );
    assert!(
        format!("{result:?}").contains("projects_workspace_id_fkey"),
        "the error is the workspace key's: {result:?}"
    );
    assert_eq!(projects_of(&l.pool, l.design).await, 0, "nothing written");
}
