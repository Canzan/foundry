//! project-lifecycle — the store's soft delete, write guard, restore, rename
//! record and live-project readers, on a real Postgres (DISCUSS D-A, D2, D3,
//! D5, D6, D10-D12, D17, OQ-3, OQ-4, INV-PL-1 to INV-PL-5, INV-PL-7, INV-PL-9;
//! DESIGN DDD-2 to DDD-9, DDD-13, DDD-15, DDD-17, DDD-25, DDD-31, Changed
//! Assumption 3; ADR-PROJECT-LIFECYCLE-001 to -003). DISTILL 2026-10-13.
//!
//! The gold tests DESIGN put at store level (Test Seams): the races are
//! deterministic here (a held lock or a barrier, two connections) and the
//! barrier race runs 100 times, which the HTTP lane cannot afford.
//!
//! Never ignored, and green since DISTILL: the Postgres SUBSTRATE facts the
//! design relies on (ADR-PROJECT-LIFECYCLE-001/002 "Earned Trust"), proven on
//! scratch tables because 0026 does not exist yet, and the NEGATIVE CONTROL:
//! a delete that takes `FOR NO KEY UPDATE` lets a guarded child write commit
//! after it. If a control ever stops losing, the server's semantics moved and
//! the ADR's argument must be re-made.
//!
//! Authored as RED in DISTILL 2026-10-13: every test that needs the new schema
//! (0025 `project_lifecycle_events`, 0026 `deleted_at`/`deleted_by` and
//! `live_projects`, 0027 the detach) or a scaffolded store method is ignored as
//! `pending: project-lifecycle NN-NN`, and still asserts its readable RED gate
//! first (`require_records` / `require_soft_delete` / `require_detach`).
//! DELIVER un-ignores each at its step:
//! - 01-01 the record table (0025), `rename_project_with_audit`, `list_team_projects`;
//! - 02-01 0026, the view, the probe and `lock_live_project` in every writer;
//! - 02-02 the reader sweep onto `live_projects`;
//! - 02-03 `soft_delete_project`;
//! - 03-01 `restore_deleted_project`, `list_deleted_projects`.
//!
//! Signatures are DISTILL's working choice (see `src/project_lifecycle.rs`).
//! The project-deleted outcome of `undelete_comment` is named through its
//! `Debug` (`ProjectDeleted`), so no shipped return type changes before DELIVER.
//!
//! Real Postgres (testcontainers `postgres:16-alpine`, matching production);
//! integration level, example-based, races repeated (Mandates 9/11).

use foundry_store::issue_delete::IssueDeleteContext;
use foundry_store::lanes::LaneSide;
use foundry_store::{
    run_migrations, LaneDeleteFate, ProjectDeleteOutcome, ProjectRenameWrite,
    ProjectRestoreOutcome, Store, TeamProjectRow,
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

/// How many times each barrier race runs (DoD 4).
const RACE_RUNS: usize = 100;

type Id = uuid::Uuid;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

async fn fresh() -> (PgPool, ContainerAsync<Postgres>) {
    let c = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let host = c.get_host().await.expect("container host");
    let port = c.get_host_port_ipv4(5432).await.expect("container port");
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&format!(
            "postgres://postgres:postgres@{host}:{port}/postgres"
        ))
        .await
        .expect("connect pool");
    (pool, c)
}

async fn present(pool: &PgPool, sql: &str) -> bool {
    let (yes,): (bool,) = sqlx::query_as(sql)
        .fetch_one(pool)
        .await
        .expect("look the schema up");
    yes
}

/// RED gate: migration 0025 has created `project_lifecycle_events`.
async fn require_records(pool: &PgPool) {
    assert!(
        present(
            pool,
            "SELECT to_regclass('project_lifecycle_events') IS NOT NULL"
        )
        .await,
        "RED gate: migration 0025 has not created project_lifecycle_events yet \
         (project-lifecycle DDD-13, ADR-PROJECT-LIFECYCLE-003)"
    );
}

/// RED gate: migration 0026 has added the deleted marker and `live_projects`.
async fn require_soft_delete(pool: &PgPool) {
    assert!(
        present(
            pool,
            "SELECT EXISTS (SELECT 1 FROM information_schema.columns
                             WHERE table_schema = current_schema()
                               AND table_name = 'projects' AND column_name = 'deleted_at')
                AND to_regclass('live_projects') IS NOT NULL"
        )
        .await,
        "RED gate: migration 0026 has not added projects.deleted_at and the live_projects view yet \
         (project-lifecycle DDD-2, DDD-3, ADR-PROJECT-LIFECYCLE-001)"
    );
}

/// RED gate: migration 0027 has added the detach column.
async fn require_detach(pool: &PgPool) {
    assert!(
        present(
            pool,
            "SELECT EXISTS (SELECT 1 FROM information_schema.columns
                             WHERE table_schema = current_schema()
                               AND table_name = 'projects'
                               AND column_name = 'detached_from_team_id')"
        )
        .await,
        "RED gate: migration 0027 has not added projects.detached_from_team_id yet \
         (project-lifecycle DDD-19, ADR-PROJECT-LIFECYCLE-004)"
    );
}

fn pg(err: &sqlx::Error) -> &PgDatabaseError {
    err.as_database_error()
        .unwrap_or_else(|| panic!("expected the database's refusal, got {err:?}"))
        .downcast_ref::<PgDatabaseError>()
}

async fn exec(pool: &PgPool, sql: &str) {
    sqlx::raw_sql(sql).execute(pool).await.expect(sql);
}

// ---------------------------------------------------------------------------
// The lab: Canzan Labs, Backend (Lena leads) with Auth v2 Spike (SPIKE: 3
// issues, 2 live comments and 1 tombstone on SPIKE-2, 1 attachment, a change
// event, 4 lanes) and Identity Platform (AUTH); Ops with Reading List (READ);
// Globex with Payments. Every child row is written by the shipped writers.
// ---------------------------------------------------------------------------

#[allow(dead_code)]
struct Lab {
    pool: PgPool,
    store: Store,
    ws: Id,
    globex: Id,
    backend: Id,
    ops: Id,
    payments: Id,
    priya: Id,
    lena: Id,
    sam: Id,
    spike: Id,
    auth: Id,
    read: Id,
    /// SPIKE-1..3 as `(issue id, number)`.
    issues: Vec<(Id, i32)>,
    live_comments: Vec<Id>,
    tombstone: Id,
    attachment: Id,
    _c: ContainerAsync<Postgres>,
}

async fn workspace(pool: &PgPool, name: &str) -> Id {
    let id = Id::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .expect("insert workspace");
    id
}

async fn team(pool: &PgPool, ws: Id, name: &str, slug: &str) -> Id {
    let id = Id::now_v7();
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

async fn person(pool: &PgPool, ws: Id, email: &str, role: &str) -> Id {
    let id = Id::now_v7();
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

async fn place(pool: &PgPool, team_id: Id, user_id: Id, role: &str) {
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, $3)")
        .bind(team_id)
        .bind(user_id)
        .bind(role)
        .execute(pool)
        .await
        .expect("place someone on a team");
}

async fn project(store: &Store, ws: Id, team_id: Id, name: &str, slug: &str, key: &str) -> Id {
    let id = Id::now_v7();
    store
        .insert_project(id, ws, team_id, name, slug, key)
        .await
        .expect("insert project with its lanes");
    id
}

async fn issue(
    store: &Store,
    ws: Id,
    project_id: Id,
    key: &str,
    author: Id,
    title: &str,
) -> (Id, i32) {
    let id = Id::now_v7();
    let inserted = store
        .insert_issue_with_outbox(id, ws, project_id, key, author, title, "")
        .await
        .expect("insert issue");
    (id, inserted.number)
}

#[allow(clippy::too_many_arguments)]
async fn comment(
    store: &Store,
    ws: Id,
    project_id: Id,
    key: &str,
    on: (Id, i32),
    author: Id,
    email: &str,
    body: &str,
) -> Id {
    let id = Id::now_v7();
    store
        .insert_comment_with_outbox(
            id, ws, project_id, key, on.0, on.1, author, email, body, body,
        )
        .await
        .expect("insert comment");
    id
}

async fn lab() -> Lab {
    let (pool, c) = fresh().await;
    run_migrations(&pool).await.expect("apply every migration");
    let store = Store::from_pool(pool.clone());
    let ws = workspace(&pool, "Canzan Labs").await;
    let backend = team(&pool, ws, "Backend", "backend").await;
    let ops = team(&pool, ws, "Ops", "ops").await;
    let priya = person(&pool, ws, "priya.raman@canzan.dev", "admin").await;
    let lena = person(&pool, ws, "lena.park@canzan.dev", "member").await;
    let sam = person(&pool, ws, "sam.okafor@canzan.dev", "member").await;
    place(&pool, backend, lena, "lead").await;
    place(&pool, backend, sam, "member").await;
    place(&pool, backend, priya, "member").await;
    let spike = project(
        &store,
        ws,
        backend,
        "Auth v2 Spike",
        "auth-v2-spike",
        "SPIKE",
    )
    .await;
    let auth = project(&store, ws, backend, "Identity Platform", "auth", "AUTH").await;
    let read = project(&store, ws, ops, "Reading List", "reading-list", "READ").await;
    let mut issues = Vec::new();
    for title in ["Token rotation", "Session store", "Refresh flow"] {
        issues.push(issue(&store, ws, spike, "SPIKE", lena, title).await);
    }
    store
        .update_issue_details(spike, issues[0].1, "Token rotation v2", "", lena)
        .await
        .expect("an edit writes a change event");
    let mut live_comments = Vec::new();
    for body in ["Looks good", "Ship it"] {
        live_comments.push(
            comment(
                &store,
                ws,
                spike,
                "SPIKE",
                issues[1],
                sam,
                "sam.okafor@canzan.dev",
                body,
            )
            .await,
        );
    }
    let tombstone = comment(
        &store,
        ws,
        spike,
        "SPIKE",
        issues[1],
        sam,
        "sam.okafor@canzan.dev",
        "typo",
    )
    .await;
    assert!(
        store
            .soft_delete_comment_with_outbox(ws, issues[1].0, tombstone, sam)
            .await
            .expect("tombstone a comment"),
        "the tombstone is written"
    );
    let attachment = Id::now_v7();
    store
        .insert_attachment(
            attachment,
            issues[1].0,
            ws,
            sam,
            "spike-notes.png",
            "image/png",
            "0000000000000000000000000000000000000000000000000000000000000000",
            b"\x89PNG spike notes",
        )
        .await
        .expect("insert attachment");
    issue(&store, ws, auth, "AUTH", lena, "Login page").await;
    let globex = workspace(&pool, "Globex").await;
    let payments = team(&pool, globex, "Payments", "payments").await;
    Lab {
        pool,
        store,
        ws,
        globex,
        backend,
        ops,
        payments,
        priya,
        lena,
        sam,
        spike,
        auth,
        read,
        issues,
        live_comments,
        tombstone,
        attachment,
        _c: c,
    }
}

/// Every row under a project, as `id:checksum` of the whole row, by table
/// (INV-PL-1's universe: issues, comments live and tombstoned, attachments,
/// change events, lanes), plus the whole outbox and the project row minus its
/// deleted marker (name, slug, key, counter, team, workspace).
async fn under(pool: &PgPool, project_id: Id) -> BTreeMap<&'static str, Vec<String>> {
    let mut out = BTreeMap::new();
    for (table, sql) in [
        ("issues", "SELECT t.id::text || ':' || md5(t::text) FROM issues t WHERE t.project_id = $1 ORDER BY 1"),
        (
            "comments",
            "SELECT c.id::text || ':' || md5(c::text) FROM comments c JOIN issues i ON i.id = c.issue_id
              WHERE i.project_id = $1 ORDER BY 1",
        ),
        (
            "issue_attachments",
            "SELECT a.id::text || ':' || md5(a::text) FROM issue_attachments a JOIN issues i ON i.id = a.issue_id
              WHERE i.project_id = $1 ORDER BY 1",
        ),
        (
            "issue_change_events",
            "SELECT e.id::text || ':' || md5(e::text) FROM issue_change_events e WHERE e.project_id = $1 ORDER BY 1",
        ),
        ("lanes", "SELECT l.id::text || ':' || md5(l::text) FROM lanes l WHERE l.project_id = $1 ORDER BY 1"),
        ("outbox", "SELECT o.id::text || ':' || md5(o::text) FROM outbox o WHERE $1::uuid IS NOT NULL ORDER BY o.id"),
        (
            "project",
            "SELECT p.id::text || ':' || md5(row(p.name, p.slug, p.key_prefix, p.next_issue_number,
                                                 p.team_id, p.workspace_id)::text)
               FROM projects p WHERE p.id = $1",
        ),
    ] {
        let rows: Vec<(String,)> = sqlx::query_as(sql)
            .bind(project_id)
            .fetch_all(pool)
            .await
            .expect("snapshot");
        out.insert(table, rows.into_iter().map(|(r,)| r).collect());
    }
    out
}

/// Every row of every table a lifecycle write could touch, whole-database
/// (read-only use-cases change zero rows).
async fn everything(pool: &PgPool) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT 'p:' || md5(t::text) FROM projects t
         UNION ALL SELECT 'i:' || md5(t::text) FROM issues t
         UNION ALL SELECT 'c:' || md5(t::text) FROM comments t
         UNION ALL SELECT 'a:' || md5(t::text) FROM issue_attachments t
         UNION ALL SELECT 'e:' || md5(t::text) FROM issue_change_events t
         UNION ALL SELECT 'l:' || md5(t::text) FROM lanes t
         UNION ALL SELECT 'o:' || md5(t::text) FROM outbox t
         UNION ALL SELECT 't:' || md5(t::text) FROM teams t
         ORDER BY 1",
    )
    .fetch_all(pool)
    .await
    .expect("whole-database snapshot");
    let mut out: Vec<String> = rows.into_iter().map(|(r,)| r).collect();
    if present(
        pool,
        "SELECT to_regclass('project_lifecycle_events') IS NOT NULL",
    )
    .await
    {
        let records: Vec<(String,)> = sqlx::query_as(
            "SELECT 'r:' || md5(t::text) FROM project_lifecycle_events t ORDER BY 1",
        )
        .fetch_all(pool)
        .await
        .expect("records");
        out.extend(records.into_iter().map(|(r,)| r));
    }
    out
}

/// One lifecycle record, as the operator reads it with psql.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Record {
    kind: String,
    project_id: Id,
    project_slug: String,
    key_prefix: String,
    name: String,
    new_name: Option<String>,
    actor_id: Option<Id>,
}

/// `(kind, project id, slug, key, name, new name, actor)` as psql returns it.
type RecordRow = (
    String,
    Id,
    String,
    String,
    String,
    Option<String>,
    Option<Id>,
);

async fn records(pool: &PgPool) -> Vec<Record> {
    let rows: Vec<RecordRow> = sqlx::query_as(
        "SELECT kind, project_id, project_slug, key_prefix, name, new_name, actor_id
           FROM project_lifecycle_events ORDER BY created_at, id",
    )
    .fetch_all(pool)
    .await
    .expect("read the lifecycle records");
    rows.into_iter()
        .map(
            |(kind, project_id, project_slug, key_prefix, name, new_name, actor_id)| Record {
                kind,
                project_id,
                project_slug,
                key_prefix,
                name,
                new_name,
                actor_id,
            },
        )
        .collect()
}

/// Install a refusal on every insert into `table` (atomicity tests).
async fn refuse_inserts_into(pool: &PgPool, table: &str) {
    exec(
        pool,
        &format!(
            "CREATE OR REPLACE FUNCTION test_refuse() RETURNS trigger LANGUAGE plpgsql AS $f$
             BEGIN RAISE EXCEPTION 'injected refusal' USING ERRCODE = '23514'; END $f$;
             CREATE TRIGGER test_refuse BEFORE INSERT ON {table}
                 FOR EACH ROW EXECUTE FUNCTION test_refuse();"
        ),
    )
    .await;
}

/// Mark a project deleted by hand, as the operator's psql would: a precondition
/// for the reader and guard tests that must not depend on `soft_delete_project`.
async fn mark_deleted_by_hand(pool: &PgPool, project_id: Id, by: Id) {
    sqlx::query("UPDATE projects SET deleted_at = now(), deleted_by = $2 WHERE id = $1")
        .bind(project_id)
        .bind(by)
        .execute(pool)
        .await
        .expect("mark the project deleted");
}

/// A held delete: the project row `FOR UPDATE` and the marker set, uncommitted.
async fn hold_delete(
    pool: &PgPool,
    project_id: Id,
    by: Id,
) -> sqlx::Transaction<'static, sqlx::Postgres> {
    let mut tx = pool.begin().await.expect("begin the delete");
    sqlx::query("SELECT 1 FROM projects WHERE id = $1 FOR UPDATE")
        .bind(project_id)
        .execute(&mut *tx)
        .await
        .expect("lock the project row");
    sqlx::query("UPDATE projects SET deleted_at = now(), deleted_by = $2 WHERE id = $1")
        .bind(project_id)
        .bind(by)
        .execute(&mut *tx)
        .await
        .expect("set the marker");
    tx
}

async fn count(pool: &PgPool, sql: &str, id: Id) -> i64 {
    let (n,): (i64,) = sqlx::query_as(sql)
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("count rows");
    n
}

// ===========================================================================
// SUBSTRATE FACTS (ADR-PROJECT-LIFECYCLE-001/002 Earned Trust) — GREEN today,
// on scratch tables shaped like `projects` / `issues` / `live_projects`
// ===========================================================================

async fn scratch() -> (PgPool, ContainerAsync<Postgres>) {
    let (pool, c) = fresh().await;
    exec(
        &pool,
        "CREATE TABLE s_team (id int PRIMARY KEY);
         CREATE TABLE s_project (id int PRIMARY KEY, team_id int REFERENCES s_team(id) ON DELETE CASCADE,
                                 slug text NOT NULL, n int NOT NULL DEFAULT 1, deleted_at timestamptz,
                                 detached int, UNIQUE (team_id, slug));
         CREATE TABLE s_issue (id serial PRIMARY KEY, project_id int NOT NULL REFERENCES s_project(id));
         CREATE VIEW s_live AS SELECT * FROM s_project WHERE deleted_at IS NULL;
         INSERT INTO s_team VALUES (1);
         INSERT INTO s_project (id, team_id, slug) VALUES (1, 1, 'spike');",
    )
    .await;
    (pool, c)
}

/// A guarded writer's `FOR KEY SHARE` on the live row waits while a delete
/// holds the row `FOR UPDATE`.
#[tokio::test]
async fn a_key_share_guard_waits_while_a_delete_holds_the_project_row_for_update() {
    let (pool, _c) = scratch().await;
    let mut delete = pool.begin().await.expect("begin");
    sqlx::query("SELECT 1 FROM s_project WHERE id = 1 FOR UPDATE")
        .execute(&mut *delete)
        .await
        .expect("lock");
    let guard =
        sqlx::query("SELECT 1 FROM s_project WHERE id = 1 AND deleted_at IS NULL FOR KEY SHARE")
            .fetch_optional(&pool);
    let waited = tokio::time::timeout(Duration::from_secs(2), guard).await;
    assert!(
        waited.is_err(),
        "the guard must wait on FOR UPDATE: {waited:?}"
    );
    delete.rollback().await.expect("rollback");
}

/// At READ COMMITTED a guard that waited re-evaluates `deleted_at IS NULL`
/// against the committed delete and finds no row; after a rollback it finds it.
#[tokio::test]
async fn a_guard_that_waited_finds_no_live_row_after_the_delete_commits_and_one_after_a_rollback() {
    let (pool, _c) = scratch().await;
    for commit in [true, false] {
        exec(&pool, "UPDATE s_project SET deleted_at = NULL WHERE id = 1").await;
        let mut delete = pool.begin().await.expect("begin");
        sqlx::query("SELECT 1 FROM s_project WHERE id = 1 FOR UPDATE")
            .execute(&mut *delete)
            .await
            .expect("lock");
        sqlx::query("UPDATE s_project SET deleted_at = now() WHERE id = 1")
            .execute(&mut *delete)
            .await
            .expect("mark");
        let p = pool.clone();
        let guard = tokio::spawn(async move {
            sqlx::query("SELECT 1 FROM s_project WHERE id = 1 AND deleted_at IS NULL FOR KEY SHARE")
                .fetch_optional(&p)
                .await
                .expect("guard")
                .is_some()
        });
        tokio::time::sleep(Duration::from_millis(300)).await;
        if commit {
            delete.commit().await.expect("commit");
        } else {
            delete.rollback().await.expect("rollback");
        }
        assert_eq!(
            guard.await.expect("guard task"),
            !commit,
            "commit={commit}: the waited guard re-evaluates the predicate"
        );
    }
}

/// `UPDATE <simple view> … RETURNING` passes through to the base row, waits on
/// a held delete, and returns no row once the delete commits (the issue
/// counter is its own guard, DDD-7).
#[tokio::test]
async fn an_update_through_the_live_view_that_waited_on_a_delete_returns_no_row() {
    let (pool, _c) = scratch().await;
    let (n,): (i32,) = sqlx::query_as("UPDATE s_live SET n = n + 1 WHERE id = 1 RETURNING n - 1")
        .fetch_one(&pool)
        .await
        .expect("a simple view is automatically updatable");
    assert_eq!(n, 1);
    let mut delete = pool.begin().await.expect("begin");
    sqlx::query("SELECT 1 FROM s_project WHERE id = 1 FOR UPDATE")
        .execute(&mut *delete)
        .await
        .expect("lock");
    sqlx::query("UPDATE s_project SET deleted_at = now() WHERE id = 1")
        .execute(&mut *delete)
        .await
        .expect("mark");
    let p = pool.clone();
    let counter = tokio::spawn(async move {
        sqlx::query_as::<_, (i32,)>("UPDATE s_live SET n = n + 1 WHERE id = 1 RETURNING n - 1")
            .fetch_optional(&p)
            .await
            .expect("the counter never errors")
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    delete.commit().await.expect("commit");
    assert_eq!(
        counter.await.expect("task"),
        None,
        "no row: ProjectNotFound, no 40P01"
    );
    let (n,): (i32,) = sqlx::query_as("SELECT n FROM s_project WHERE id = 1")
        .fetch_one(&pool)
        .await
        .expect("read");
    assert_eq!(n, 2, "the counter did not move after the delete");
}

/// A row lock taken through the view lands on the base row: a guard through
/// `live_projects` blocks the delete's `FOR UPDATE` on `projects`.
#[tokio::test]
async fn a_row_lock_through_the_live_view_blocks_a_for_update_on_the_base_row() {
    let (pool, _c) = scratch().await;
    let mut guard = pool.begin().await.expect("begin");
    sqlx::query("SELECT 1 FROM s_live WHERE id = 1 FOR KEY SHARE")
        .execute(&mut *guard)
        .await
        .expect("lock through the view");
    let delete =
        sqlx::query("SELECT 1 FROM s_project WHERE id = 1 FOR UPDATE").fetch_optional(&pool);
    let waited = tokio::time::timeout(Duration::from_secs(2), delete).await;
    assert!(
        waited.is_err(),
        "the delete must wait on the view's lock: {waited:?}"
    );
    guard.rollback().await.expect("rollback");
}

/// KEY SHARE is compatible with KEY SHARE and with FOR NO KEY UPDATE: the
/// guard serialises writers against a delete only, never against each other,
/// a rename or the issue counter. FOR NO KEY UPDATE (a rename) conflicts with
/// FOR UPDATE (a delete): rename and delete are totally ordered.
#[tokio::test]
async fn the_guard_does_not_serialise_writers_or_renames_but_a_rename_and_a_delete_are_ordered() {
    let (pool, _c) = scratch().await;
    let mut one = pool.begin().await.expect("begin");
    sqlx::query("SELECT 1 FROM s_project WHERE id = 1 FOR KEY SHARE")
        .execute(&mut *one)
        .await
        .expect("first guard");
    for lock in ["FOR KEY SHARE", "FOR NO KEY UPDATE"] {
        let q = format!("SELECT 1 FROM s_project WHERE id = 1 {lock}");
        let mut other = pool.begin().await.expect("begin");
        let done =
            tokio::time::timeout(Duration::from_secs(3), sqlx::query(&q).execute(&mut *other))
                .await;
        assert!(
            matches!(done, Ok(Ok(_))),
            "{lock} must not wait on a guard: {done:?}"
        );
        other.rollback().await.expect("rollback");
    }
    one.rollback().await.expect("rollback");
    let mut rename = pool.begin().await.expect("begin");
    sqlx::query("SELECT 1 FROM s_project WHERE id = 1 FOR NO KEY UPDATE")
        .execute(&mut *rename)
        .await
        .expect("rename lock");
    let delete =
        sqlx::query("SELECT 1 FROM s_project WHERE id = 1 FOR UPDATE").fetch_optional(&pool);
    let waited = tokio::time::timeout(Duration::from_secs(2), delete).await;
    assert!(
        waited.is_err(),
        "a delete must wait on a rename: {waited:?}"
    );
    rename.rollback().await.expect("rollback");
}

/// `CREATE VIEW … AS SELECT *` fixes its column list at creation: a column
/// added later is not in the view until it is recreated (hence the parity
/// self-checks and "every migration that adds a projects column recreates
/// live_projects").
#[tokio::test]
async fn a_select_star_view_does_not_pick_up_a_column_added_later() {
    let (pool, _c) = scratch().await;
    exec(&pool, "ALTER TABLE s_project ADD COLUMN later text").await;
    let err = sqlx::query("SELECT later FROM s_live")
        .fetch_all(&pool)
        .await
        .expect_err("the view's columns were fixed at creation");
    assert_eq!(pg(&err).code(), "42703", "undefined column: {:?}", pg(&err));
    exec(
        &pool,
        "CREATE OR REPLACE VIEW s_live AS SELECT * FROM s_project WHERE deleted_at IS NULL",
    )
    .await;
    sqlx::query("SELECT later FROM s_live")
        .fetch_all(&pool)
        .await
        .expect("a recreated view has it");
}

/// `UNIQUE (team_id, slug)` treats NULL team ids as distinct: detached projects
/// never collide on their slug (ADR-PROJECT-LIFECYCLE-004 decision 1).
#[tokio::test]
async fn unique_team_and_slug_treats_two_detached_rows_as_distinct() {
    let (pool, _c) = scratch().await;
    exec(
        &pool,
        "INSERT INTO s_project (id, team_id, slug, detached) VALUES (2, NULL, 'proto', 1), (3, NULL, 'proto', 1)",
    )
    .await;
}

/// NEGATIVE CONTROL (ADR-PROJECT-LIFECYCLE-002): a delete that takes `FOR NO KEY
/// UPDATE` does not stop a guarded child write, which commits under a project
/// the delete then marks deleted. Only `FOR UPDATE` may guard the delete.
#[tokio::test]
async fn negative_control_a_delete_with_for_no_key_update_lets_a_guarded_write_commit_after_it() {
    let (pool, _c) = scratch().await;
    let mut delete = pool.begin().await.expect("begin");
    sqlx::query("SELECT 1 FROM s_project WHERE id = 1 FOR NO KEY UPDATE")
        .execute(&mut *delete)
        .await
        .expect("the wrong lock");
    let mut write = pool.begin().await.expect("begin write");
    let guarded = tokio::time::timeout(
        Duration::from_secs(3),
        sqlx::query("SELECT 1 FROM s_project WHERE id = 1 AND deleted_at IS NULL FOR KEY SHARE")
            .fetch_optional(&mut *write),
    )
    .await;
    assert!(
        matches!(guarded, Ok(Ok(Some(_)))),
        "the guard is not held up: {guarded:?}"
    );
    sqlx::query("INSERT INTO s_issue (project_id) VALUES (1)")
        .execute(&mut *write)
        .await
        .expect("the child write");
    write
        .commit()
        .await
        .expect("the write commits while the delete is in flight");
    sqlx::query("UPDATE s_project SET deleted_at = now() WHERE id = 1")
        .execute(&mut *delete)
        .await
        .expect("the marker");
    delete.commit().await.expect("the delete commits");
    let (n,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM s_issue i JOIN s_project p ON p.id = i.project_id WHERE p.deleted_at IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .expect("count");
    assert_eq!(
        n, 1,
        "the control writes into a project deleted around it: FOR NO KEY UPDATE must never guard a delete"
    );
}

// ===========================================================================
// 0026: the view and its marker (02-01)
// ===========================================================================

async fn columns_of(pool: &PgPool, relation: &str) -> Vec<(String, String)> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT column_name::text, data_type::text FROM information_schema.columns
          WHERE table_schema = current_schema() AND table_name = $1 ORDER BY ordinal_position",
    )
    .bind(relation)
    .fetch_all(pool)
    .await
    .expect("read columns");
    rows
}

#[tokio::test]
async fn live_projects_has_exactly_the_columns_of_projects_in_order() {
    let (pool, _c) = fresh().await;
    run_migrations(&pool).await.expect("apply every migration");
    require_soft_delete(&pool).await;
    let table = columns_of(&pool, "projects").await;
    assert!(
        table.iter().any(|(c, _)| c == "deleted_at")
            && table.iter().any(|(c, _)| c == "deleted_by")
    );
    assert_eq!(
        columns_of(&pool, "live_projects").await,
        table,
        "view parity (0026)"
    );
}

#[tokio::test]
async fn live_projects_still_has_exactly_the_columns_of_projects_after_the_detach() {
    let (pool, _c) = fresh().await;
    run_migrations(&pool).await.expect("apply every migration");
    require_detach(&pool).await;
    let table = columns_of(&pool, "projects").await;
    assert!(table.iter().any(|(c, _)| c == "detached_from_team_id"));
    assert_eq!(
        columns_of(&pool, "live_projects").await,
        table,
        "view parity (0027)"
    );
}

#[tokio::test]
async fn the_deleted_marker_is_both_columns_or_neither() {
    let l = lab().await;
    require_soft_delete(&l.pool).await;
    for (sql, what) in [
        (
            "UPDATE projects SET deleted_at = now() WHERE id = $1",
            "a time without a who",
        ),
        (
            "UPDATE projects SET deleted_by = $2 WHERE id = $1",
            "a who without a time",
        ),
    ] {
        let err = sqlx::query(sql)
            .bind(l.spike)
            .bind(l.lena)
            .execute(&l.pool)
            .await
            .expect_err(what);
        assert_eq!(pg(&err).code(), "23514", "{what}: {:?}", pg(&err));
        assert_eq!(
            pg(&err).constraint(),
            Some("projects_deleted_marker_shape"),
            "{what}"
        );
    }
    mark_deleted_by_hand(&l.pool, l.spike, l.lena).await;
}

// ===========================================================================
// The write guard (02-01)
// ===========================================================================

/// Every project-scoped writer DESIGN names (DDD-7, Component Decomposition),
/// each with the not-found outcome it already has.
#[derive(Debug, Clone, Copy)]
enum Writer {
    IssueCreate,
    IssueEdit,
    IssueMove,
    IssueDelete,
    CommentAdd,
    CommentEdit,
    CommentDelete,
    CommentRestore,
    AttachmentAdd,
    LaneRename,
    LaneInsert,
    LaneMove,
    LaneDelete,
}

const WRITERS: [Writer; 13] = [
    Writer::IssueCreate,
    Writer::IssueEdit,
    Writer::IssueMove,
    Writer::IssueDelete,
    Writer::CommentAdd,
    Writer::CommentEdit,
    Writer::CommentDelete,
    Writer::CommentRestore,
    Writer::AttachmentAdd,
    Writer::LaneRename,
    Writer::LaneInsert,
    Writer::LaneMove,
    Writer::LaneDelete,
];

/// One fresh project per writer row: its issue SPIKE-1 with a live comment and
/// a tombstone, and its four seeded lanes.
struct Target {
    ws: Id,
    project: Id,
    key: String,
    issue: (Id, i32),
    comment: Id,
    tombstone: Id,
    actor: Id,
}

async fn target(l: &Lab, n: usize) -> Target {
    let key = key_for(n);
    let project = project(
        &l.store,
        l.ws,
        l.backend,
        &format!("Writer {n}"),
        &format!("writer-{n}"),
        &key,
    )
    .await;
    let on = issue(&l.store, l.ws, project, &key, l.lena, "target").await;
    let comment = comment(
        &l.store,
        l.ws,
        project,
        &key,
        on,
        l.sam,
        "sam.okafor@canzan.dev",
        "live",
    )
    .await;
    let tombstone = comment_tombstone(l, project, &key, on).await;
    Target {
        ws: l.ws,
        project,
        key,
        issue: on,
        comment,
        tombstone,
        actor: l.lena,
    }
}

/// A distinct key prefix per writer row: "W" and three letters.
fn key_for(n: usize) -> String {
    let letter = |d: usize| (b'A' + (d % 26) as u8) as char;
    format!("W{}{}{}", letter(n / 676), letter(n / 26), letter(n))
}

async fn comment_tombstone(l: &Lab, project: Id, key: &str, on: (Id, i32)) -> Id {
    let id = comment(
        &l.store,
        l.ws,
        project,
        key,
        on,
        l.sam,
        "sam.okafor@canzan.dev",
        "gone",
    )
    .await;
    l.store
        .soft_delete_comment_with_outbox(l.ws, on.0, id, l.sam)
        .await
        .expect("tombstone");
    id
}

/// Run the writer and describe its outcome; `true` when it is the writer's
/// shipped not-found outcome.
async fn run_writer(store: Store, w: Writer, t: Arc<Target>) -> (bool, String) {
    match w {
        Writer::IssueCreate => {
            let r = store
                .insert_issue_with_outbox(
                    Id::now_v7(),
                    t.ws,
                    t.project,
                    &t.key,
                    t.actor,
                    "late",
                    "",
                )
                .await;
            (
                matches!(r, Err(foundry_store::IssueInsertError::ProjectNotFound)),
                format!("{r:?}"),
            )
        }
        Writer::IssueEdit => {
            let r = store
                .update_issue_details(t.project, t.issue.1, "late", "late", t.actor)
                .await;
            (matches!(r, Ok(None)), format!("{r:?}"))
        }
        Writer::IssueMove => {
            let r = store
                .reposition_issue_with_outbox(t.project, t.issue.1, "done", None, t.actor)
                .await;
            (
                matches!(r, Ok(foundry_store::RepositionOutcome::IssueNotFound)),
                format!("{r:?}"),
            )
        }
        Writer::IssueDelete => {
            let ctx = IssueDeleteContext {
                workspace_id: t.ws,
                project_id: t.project,
                key_prefix: t.key.clone(),
            };
            let r = store
                .delete_issue_with_outbox(&ctx, t.issue.0, t.issue.1)
                .await;
            (matches!(r, Ok(0)), format!("{r:?}"))
        }
        Writer::CommentAdd => {
            let r = store
                .insert_comment_with_outbox(
                    Id::now_v7(),
                    t.ws,
                    t.project,
                    &t.key,
                    t.issue.0,
                    t.issue.1,
                    t.actor,
                    "lena.park@canzan.dev",
                    "late",
                    "late",
                )
                .await;
            (
                matches!(r, Err(foundry_store::CommentInsertError::IssueNotFound)),
                format!("{r:?}"),
            )
        }
        Writer::CommentEdit => {
            let r = store
                .update_comment_with_outbox(
                    t.ws,
                    t.issue.0,
                    t.comment,
                    "late",
                    "late",
                    t.actor,
                    "lena.park@canzan.dev",
                )
                .await;
            (matches!(r, Ok(false)), format!("{r:?}"))
        }
        Writer::CommentDelete => {
            let r = store
                .soft_delete_comment_with_outbox(t.ws, t.issue.0, t.comment, t.actor)
                .await;
            (matches!(r, Ok(false)), format!("{r:?}"))
        }
        Writer::CommentRestore => {
            let r = store.undelete_comment(t.tombstone).await;
            let said = format!("{r:?}");
            (said.contains("ProjectDeleted"), said)
        }
        Writer::AttachmentAdd => {
            let r = store
                .insert_attachment(
                    Id::now_v7(),
                    t.issue.0,
                    t.ws,
                    t.actor,
                    "late.png",
                    "image/png",
                    "1111111111111111111111111111111111111111111111111111111111111111",
                    b"late",
                )
                .await;
            (
                matches!(r, Err(foundry_store::AttachmentInsertError::IssueNotFound)),
                format!("{r:?}"),
            )
        }
        Writer::LaneRename => {
            let r = store.rename_lane(t.project, "todo", "To do").await;
            (matches!(r, Ok(false)), format!("{r:?}"))
        }
        Writer::LaneInsert => {
            let r = store
                .insert_lane_at(t.project, "todo", LaneSide::After, "review", "Review")
                .await;
            (
                matches!(
                    r,
                    Ok(foundry_store::lanes::LaneInsertOutcome::AnchorNotFound)
                ),
                format!("{r:?}"),
            )
        }
        Writer::LaneMove => {
            let r = store
                .move_lane_before(t.project, "done", Some("todo"))
                .await;
            (
                matches!(r, Ok(foundry_store::lanes::LaneMoveOutcome::MoverNotFound)),
                format!("{r:?}"),
            )
        }
        Writer::LaneDelete => {
            let r = store
                .delete_lane_with_fate(
                    t.project,
                    "done",
                    LaneDeleteFate::MoveTo {
                        destination_slug: "todo",
                    },
                    t.actor,
                )
                .await;
            (
                matches!(r, Ok(foundry_store::LaneDeleteOutcome::LaneNotFound)),
                format!("{r:?}"),
            )
        }
    }
}

/// DDD-7, INV-PL-3, D17: every guarded writer, started while a delete holds the
/// project, waits; once the delete commits it returns its shipped not-found
/// outcome (never an error), and nothing under the project changed.
#[tokio::test]
async fn every_guarded_writer_waits_on_a_held_delete_then_answers_its_not_found_and_writes_nothing()
{
    let l = lab().await;
    require_soft_delete(&l.pool).await;
    let mut failures = Vec::new();
    for (n, w) in WRITERS.into_iter().enumerate() {
        let t = Arc::new(target(&l, n).await);
        let before = under(&l.pool, t.project).await;
        let delete = hold_delete(&l.pool, t.project, l.lena).await;
        let task = tokio::spawn(run_writer(l.store.clone(), w, t.clone()));
        tokio::time::sleep(Duration::from_millis(400)).await;
        let waited = !task.is_finished();
        delete.commit().await.expect("the delete commits");
        let (not_found, said) = task.await.expect("writer task");
        let after = under(&l.pool, t.project).await;
        let outbox_moved = before.get("outbox") != after.get("outbox");
        let mut children_before = before.clone();
        let mut children_after = after.clone();
        children_before.remove("outbox");
        children_after.remove("outbox");
        if !waited || !not_found || children_before != children_after || outbox_moved {
            failures.push(format!(
                "{w:?}: waited={waited} not_found={not_found} ({said}) children_unchanged={} outbox_unchanged={}",
                children_before == children_after,
                !outbox_moved
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "unguarded writers:\n{}",
        failures.join("\n")
    );
}

/// The same writers, with the held delete rolled back: each proceeds (the guard
/// only refuses a COMMITTED delete). Issue create and comment add stand for the
/// family.
#[tokio::test]
async fn a_write_waiting_on_a_delete_that_rolls_back_goes_through() {
    let l = lab().await;
    require_soft_delete(&l.pool).await;
    for (n, w) in [(20, Writer::IssueCreate), (21, Writer::CommentAdd)] {
        let t = Arc::new(target(&l, n).await);
        let delete = hold_delete(&l.pool, t.project, l.lena).await;
        let task = tokio::spawn(run_writer(l.store.clone(), w, t.clone()));
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert!(!task.is_finished(), "{w:?} waits on the held delete");
        delete.rollback().await.expect("the delete rolls back");
        let (not_found, said) = task.await.expect("writer task");
        assert!(
            !not_found && !said.contains("Err"),
            "{w:?} goes through: {said}"
        );
    }
}

/// `restore-comment` inside a deleted project changes nothing and names the
/// reason (ADR-PROJECT-LIFECYCLE-002 decision 2; exit 4 at the CLI).
#[tokio::test]
async fn restoring_a_comment_inside_a_deleted_project_changes_nothing_and_says_why() {
    let l = lab().await;
    require_soft_delete(&l.pool).await;
    mark_deleted_by_hand(&l.pool, l.spike, l.lena).await;
    let before = under(&l.pool, l.spike).await;
    let r = l.store.undelete_comment(l.tombstone).await;
    assert!(
        format!("{r:?}").contains("ProjectDeleted"),
        "the typed outcome: {r:?}"
    );
    assert_eq!(
        under(&l.pool, l.spike).await,
        before,
        "the tombstone stays a tombstone"
    );
}

// ===========================================================================
// The reader sweep: a deleted project is "never existed" to every reader (02-02)
// ===========================================================================

#[tokio::test]
async fn every_project_reader_treats_a_deleted_project_as_one_that_never_existed() {
    let l = lab().await;
    require_soft_delete(&l.pool).await;
    mark_deleted_by_hand(&l.pool, l.spike, l.lena).await;
    let mut leaks = Vec::new();
    if l.store
        .find_project_by_slug(l.backend, "auth-v2-spike")
        .await
        .expect("read")
        .is_some()
    {
        leaks.push("find_project_by_slug");
    }
    if l.store
        .find_issue_by_team_project_number(l.backend, "auth-v2-spike", 2)
        .await
        .expect("read")
        .is_some()
    {
        leaks.push("find_issue_by_team_project_number");
    }
    let listed = l
        .store
        .list_projects_for_workspace(l.ws, l.lena)
        .await
        .expect("read");
    if listed.iter().any(|(_, slug, _, _)| slug == "auth-v2-spike") {
        leaks.push("list_projects_for_workspace");
    }
    if !listed.iter().any(|(_, slug, _, _)| slug == "auth") {
        leaks.push("list_projects_for_workspace dropped a live project");
    }
    let instance = l.store.list_projects_for_instance().await.expect("read");
    if instance.iter().any(|p| p.project_id == l.spike) {
        leaks.push("list_projects_for_instance");
    }
    if l.store
        .project_rename_context(l.spike)
        .await
        .expect("read")
        .is_some()
    {
        leaks.push("project_rename_context");
    }
    assert!(
        leaks.is_empty(),
        "readers that still see the deleted project: {leaks:?}"
    );
}

#[tokio::test]
async fn the_team_project_list_omits_a_deleted_project() {
    let l = lab().await;
    require_soft_delete(&l.pool).await;
    mark_deleted_by_hand(&l.pool, l.spike, l.lena).await;
    let rows = l.store.list_team_projects(l.backend).await.expect("read");
    assert_eq!(
        rows,
        vec![TeamProjectRow {
            name: "Identity Platform".into(),
            slug: "auth".into(),
            key: "AUTH".into()
        }]
    );
}

/// D13, DDD-31: the per-workspace export still carries a deleted project,
/// marked by its own `deleted_at` / `deleted_by`.
#[tokio::test]
async fn the_workspace_export_keeps_a_deleted_project_marked_deleted() {
    let l = lab().await;
    require_soft_delete(&l.pool).await;
    mark_deleted_by_hand(&l.pool, l.spike, l.lena).await;
    let export = l.store.export_workspace(l.ws).await.expect("export");
    let projects = export
        .tables
        .iter()
        .find(|(t, _)| t == "projects")
        .map(|(_, rows)| rows.clone())
        .expect("the projects table is exported");
    let spike = projects
        .iter()
        .find(|line| line.contains(&l.spike.to_string()))
        .expect("the deleted project is in the export");
    assert!(
        spike.contains("\"deleted_at\"")
            && !spike.contains("\"deleted_at\": null")
            && spike.contains(&l.lena.to_string()),
        "marked deleted, by whom: {spike}"
    );
    let issues = export
        .tables
        .iter()
        .find(|(t, _)| t == "issues")
        .expect("issues")
        .1
        .len();
    assert_eq!(
        issues, 4,
        "every issue of the deleted project is still exported"
    );
}

// ===========================================================================
// Slice 01: the rename write and its record (01-01)
// ===========================================================================

async fn name_slug_key(pool: &PgPool, project: Id) -> (String, String, String) {
    sqlx::query_as("SELECT name, slug, key_prefix FROM projects WHERE id = $1")
        .bind(project)
        .fetch_one(pool)
        .await
        .expect("read the project")
}

#[tokio::test]
async fn a_rename_writes_the_name_only_and_one_record_with_the_old_name() {
    let l = lab().await;
    require_records(&l.pool).await;
    let before = everything(&l.pool).await;
    let out = l
        .store
        .rename_project_with_audit(l.auth, l.lena, "Identity & Access")
        .await
        .expect("rename");
    assert_eq!(
        out,
        ProjectRenameWrite::Renamed {
            old_name: "Identity Platform".into()
        }
    );
    assert_eq!(
        name_slug_key(&l.pool, l.auth).await,
        ("Identity & Access".into(), "auth".into(), "AUTH".into()),
        "name only: slug and key never change (D2)"
    );
    assert_eq!(
        records(&l.pool).await,
        vec![Record {
            kind: "renamed".into(),
            project_id: l.auth,
            project_slug: "auth".into(),
            key_prefix: "AUTH".into(),
            name: "Identity Platform".into(),
            new_name: Some("Identity & Access".into()),
            actor_id: Some(l.lena),
        }]
    );
    let after = everything(&l.pool).await;
    let changed: Vec<&String> = after.iter().filter(|r| !before.contains(r)).collect();
    assert_eq!(
        changed.len(),
        2,
        "exactly the project row and one record changed: {changed:?}"
    );
}

#[tokio::test]
async fn a_byte_equal_rename_writes_nothing_and_a_case_only_change_is_a_rename() {
    let l = lab().await;
    require_records(&l.pool).await;
    let before = everything(&l.pool).await;
    assert_eq!(
        l.store
            .rename_project_with_audit(l.read, l.priya, "Reading List")
            .await
            .expect("no-op"),
        ProjectRenameWrite::Unchanged
    );
    assert_eq!(
        everything(&l.pool).await,
        before,
        "a byte-equal rename writes nothing, records nothing"
    );
    assert_eq!(
        l.store
            .rename_project_with_audit(l.read, l.priya, "Reading list")
            .await
            .expect("case"),
        ProjectRenameWrite::Renamed {
            old_name: "Reading List".into()
        }
    );
    assert_eq!(
        records(&l.pool).await.len(),
        1,
        "a case-only change is recorded"
    );
}

#[tokio::test]
async fn a_rename_of_an_unknown_project_is_not_found_and_writes_nothing() {
    let l = lab().await;
    require_records(&l.pool).await;
    let before = everything(&l.pool).await;
    assert_eq!(
        l.store
            .rename_project_with_audit(Id::now_v7(), l.priya, "Anything")
            .await
            .expect("rename"),
        ProjectRenameWrite::NotFound
    );
    assert_eq!(everything(&l.pool).await, before);
}

/// INV-PL-7: the record commits with the rename or not at all.
#[tokio::test]
async fn a_rename_whose_record_cannot_be_written_renames_nothing() {
    let l = lab().await;
    require_records(&l.pool).await;
    refuse_inserts_into(&l.pool, "project_lifecycle_events").await;
    let before = everything(&l.pool).await;
    let r = l
        .store
        .rename_project_with_audit(l.auth, l.lena, "Identity & Access")
        .await;
    assert!(r.is_err(), "the refused record fails the rename: {r:?}");
    assert_eq!(
        everything(&l.pool).await,
        before,
        "no partial rename (NFR-PL-4)"
    );
}

/// DDD-15: concurrent renames of one project are serialised, so the records
/// chain (each old name is the previous new name), 100 runs.
#[tokio::test]
async fn concurrent_renames_of_one_project_record_chained_old_names_in_100_runs() {
    let l = lab().await;
    require_records(&l.pool).await;
    for run in 1..=RACE_RUNS {
        let barrier = Arc::new(Barrier::new(2));
        let mut tasks = Vec::new();
        for name in [format!("Ops A {run}"), format!("Ops B {run}")] {
            let (s, g, id, actor) = (l.store.clone(), barrier.clone(), l.read, l.priya);
            tasks.push(tokio::spawn(async move {
                g.wait().await;
                s.rename_project_with_audit(id, actor, &name).await
            }));
        }
        for t in tasks {
            assert!(
                matches!(
                    t.await.expect("task"),
                    Ok(ProjectRenameWrite::Renamed { .. })
                ),
                "run {run}: both renames go through"
            );
        }
    }
    let chain = records(&l.pool).await;
    assert_eq!(chain.len(), 2 * RACE_RUNS);
    for pair in chain.windows(2) {
        assert_eq!(
            Some(pair[1].name.clone()),
            pair[0].new_name,
            "each record's old name is the previous record's new name"
        );
    }
    let (current, _, _) = name_slug_key(&l.pool, l.read).await;
    assert_eq!(chain.last().and_then(|r| r.new_name.clone()), Some(current));
}

#[tokio::test]
async fn the_team_project_list_is_by_name_then_key() {
    let l = lab().await;
    project(
        &l.store,
        l.ws,
        l.backend,
        "Identity Platform",
        "identity-platform-2",
        "IDP",
    )
    .await;
    let rows = l.store.list_team_projects(l.backend).await.expect("read");
    let shown: Vec<(String, String)> = rows.into_iter().map(|r| (r.name, r.key)).collect();
    assert_eq!(
        shown,
        vec![
            ("Auth v2 Spike".into(), "SPIKE".into()),
            ("Identity Platform".into(), "AUTH".into()),
            ("Identity Platform".into(), "IDP".into()),
        ]
    );
}

// ===========================================================================
// Slice 02: the delete (02-03)
// ===========================================================================

/// INV-PL-1, D6, PL-KPI-2: a delete writes the marker and one record and
/// nothing else; the record's time equals the marker's.
#[tokio::test]
async fn a_delete_writes_only_the_marker_and_one_record_and_erases_nothing() {
    let l = lab().await;
    require_records(&l.pool).await;
    require_soft_delete(&l.pool).await;
    let before = under(&l.pool, l.spike).await;
    let counts_before = (
        count(&l.pool, "SELECT count(*) FROM issues WHERE project_id = $1", l.spike).await,
        count(
            &l.pool,
            "SELECT count(*) FROM comments c JOIN issues i ON i.id = c.issue_id WHERE i.project_id = $1",
            l.spike,
        )
        .await,
    );
    let out = l
        .store
        .soft_delete_project(l.ws, l.backend, l.spike, l.lena)
        .await
        .expect("delete");
    assert_eq!(
        out,
        ProjectDeleteOutcome::Deleted {
            name: "Auth v2 Spike".into(),
            key_prefix: "SPIKE".into()
        }
    );
    assert_eq!(
        under(&l.pool, l.spike).await,
        before,
        "every row under it, the outbox and the counter unchanged"
    );
    assert_eq!(
        counts_before,
        (3, 3),
        "3 issues; 2 live comments and 1 tombstone"
    );
    let (by, same_time): (Option<Id>, bool) = sqlx::query_as(
        "SELECT p.deleted_by, p.deleted_at = e.created_at
           FROM projects p JOIN project_lifecycle_events e ON e.project_id = p.id AND e.kind = 'deleted'
          WHERE p.id = $1",
    )
    .bind(l.spike)
    .fetch_one(&l.pool)
    .await
    .expect("the marker and its record");
    assert_eq!(by, Some(l.lena));
    assert!(same_time, "the record's created_at equals deleted_at");
    assert_eq!(
        records(&l.pool).await,
        vec![Record {
            kind: "deleted".into(),
            project_id: l.spike,
            project_slug: "auth-v2-spike".into(),
            key_prefix: "SPIKE".into(),
            name: "Auth v2 Spike".into(),
            new_name: None,
            actor_id: Some(l.lena),
        }]
    );
}

/// DDD-6 on the production path: `soft_delete_project` takes the live row
/// `FOR UPDATE`, which conflicts with a writer's `FOR KEY SHARE` guard, so a
/// guard held open makes the delete wait (a `FOR NO KEY UPDATE` delete would
/// not). Kills the DEVOPS hand-table fault "delete with FOR NO KEY UPDATE",
/// which the HTTP and 100-run races may let survive (a write that lands first
/// is a legal outcome there).
#[tokio::test]
async fn the_production_delete_waits_while_a_writer_holds_the_guard() {
    let l = lab().await;
    require_records(&l.pool).await;
    require_soft_delete(&l.pool).await;
    let mut writer = l.pool.begin().await.expect("begin the writer");
    sqlx::query("SELECT 1 FROM live_projects WHERE id = $1 FOR KEY SHARE")
        .bind(l.spike)
        .execute(&mut *writer)
        .await
        .expect("hold the guard");
    let store = l.store.clone();
    let (ws, team, spike, lena) = (l.ws, l.backend, l.spike, l.lena);
    let delete =
        tokio::spawn(async move { store.soft_delete_project(ws, team, spike, lena).await });
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !delete.is_finished(),
        "the delete must wait while a writer holds the guard (FOR UPDATE conflicts with FOR KEY SHARE)"
    );
    let (waiting,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM pg_stat_activity
          WHERE datname = current_database() AND wait_event_type = 'Lock'",
    )
    .fetch_one(&l.pool)
    .await
    .expect("read the waiting sessions");
    assert!(waiting >= 1, "the delete is waiting on a row lock");
    writer.commit().await.expect("release the guard");
    let out = tokio::time::timeout(Duration::from_secs(10), delete)
        .await
        .expect("the delete finishes once the guard is released")
        .expect("join the delete")
        .expect("delete");
    assert_eq!(
        out,
        ProjectDeleteOutcome::Deleted {
            name: "Auth v2 Spike".into(),
            key_prefix: "SPIKE".into()
        }
    );
}

/// D17, US-PL-03 AC 6, DDD-11: a notification queued before the delete is
/// delivered as usual and none arises after it. The outbox is fire-and-forget:
/// each row is announced by migration 0003's COMMIT-time NOTIFY trigger, and
/// nothing marks `notified_at` today (`Store::count_pending_outbox`, store
/// `lib.rs`, "today nothing writes notified_at"). So "still deliverable" is: the
/// row queued before the delete is byte-unchanged and still pending
/// (`notified_at IS NULL`); and "none after": a refused write through a guarded
/// writer adds no outbox row about the project.
#[tokio::test]
async fn a_notification_queued_before_the_delete_stays_pending_and_none_arises_after_it() {
    let l = lab().await;
    require_records(&l.pool).await;
    require_soft_delete(&l.pool).await;
    let queued_sql = "SELECT id, event_type, payload::text, notified_at IS NULL
                        FROM outbox WHERE payload->>'project_id' = $1 ORDER BY id";
    let read = |pool: PgPool, project: String| async move {
        sqlx::query_as::<_, (i64, String, String, bool)>(queued_sql)
            .bind(project)
            .fetch_all(&pool)
            .await
            .expect("read the project's queued notifications")
    };
    let queued = read(l.pool.clone(), l.spike.to_string()).await;
    assert!(
        !queued.is_empty(),
        "the lab queued notifications about SPIKE (issues, an edit, comments)"
    );
    assert!(
        queued.iter().all(|(_, _, _, pending)| *pending),
        "precondition: every queued row is pending"
    );
    let pending_before = l.store.count_pending_outbox().await.expect("count pending");
    l.store
        .soft_delete_project(l.ws, l.backend, l.spike, l.lena)
        .await
        .expect("delete");
    assert_eq!(
        read(l.pool.clone(), l.spike.to_string()).await,
        queued,
        "every notification queued before the delete is unchanged and still pending"
    );
    let filed = l
        .store
        .insert_issue_with_outbox(Id::now_v7(), l.ws, l.spike, "SPIKE", l.lena, "After", "")
        .await;
    assert!(filed.is_err(), "an issue filed after the delete is refused");
    let commented = l
        .store
        .insert_comment_with_outbox(
            Id::now_v7(),
            l.ws,
            l.spike,
            "SPIKE",
            l.issues[1].0,
            l.issues[1].1,
            l.sam,
            "sam.okafor@canzan.dev",
            "After",
            "After",
        )
        .await;
    assert!(commented.is_err(), "a comment after the delete is refused");
    assert_eq!(
        read(l.pool.clone(), l.spike.to_string()).await,
        queued,
        "no notification about the project arises after the delete"
    );
    assert_eq!(
        l.store.count_pending_outbox().await.expect("count pending"),
        pending_before,
        "the outbox as a whole gained nothing"
    );
}

#[tokio::test]
async fn a_second_delete_a_foreign_one_and_an_unknown_one_are_not_found_and_write_nothing() {
    let l = lab().await;
    require_records(&l.pool).await;
    require_soft_delete(&l.pool).await;
    l.store
        .soft_delete_project(l.ws, l.backend, l.spike, l.lena)
        .await
        .expect("first delete");
    let before = everything(&l.pool).await;
    for (ws, team, project, what) in [
        (l.ws, l.backend, l.spike, "a double submit"),
        (l.ws, l.ops, l.auth, "another team's address"),
        (l.globex, l.payments, l.auth, "another workspace"),
        (l.ws, l.backend, Id::now_v7(), "an unknown project"),
    ] {
        assert_eq!(
            l.store
                .soft_delete_project(ws, team, project, l.priya)
                .await
                .expect(what),
            ProjectDeleteOutcome::NotFound,
            "{what}"
        );
    }
    assert_eq!(everything(&l.pool).await, before, "one delete, one record");
}

#[tokio::test]
async fn a_delete_whose_record_cannot_be_written_deletes_nothing() {
    let l = lab().await;
    require_records(&l.pool).await;
    require_soft_delete(&l.pool).await;
    refuse_inserts_into(&l.pool, "project_lifecycle_events").await;
    let before = everything(&l.pool).await;
    let r = l
        .store
        .soft_delete_project(l.ws, l.backend, l.spike, l.lena)
        .await;
    assert!(r.is_err(), "{r:?}");
    assert_eq!(
        everything(&l.pool).await,
        before,
        "no delete without its record (NFR-PL-4)"
    );
}

/// INV-PL-3, US-PL-03 AC 4, PL-KPI-2: delete versus issue create, 100 runs.
/// Either the issue committed first (kept, hidden with the project) or the
/// create answered `ProjectNotFound`. Never an unmapped error, never a lost
/// issue.
#[tokio::test]
async fn the_delete_versus_issue_create_race_never_errors_or_loses_an_issue_in_100_runs() {
    let l = lab().await;
    require_records(&l.pool).await;
    require_soft_delete(&l.pool).await;
    for run in 1..=RACE_RUNS {
        let t = Arc::new(target(&l, 100 + run).await);
        let barrier = Arc::new(Barrier::new(2));
        let (s1, s2, g1, g2) = (l.store.clone(), l.store.clone(), barrier.clone(), barrier);
        let (ws, team, lena) = (l.ws, l.backend, l.lena);
        let t1 = t.clone();
        let issue_id = Id::now_v7();
        let delete = tokio::spawn(async move {
            g1.wait().await;
            s1.soft_delete_project(ws, team, t1.project, lena).await
        });
        let t2 = t.clone();
        let create = tokio::spawn(async move {
            g2.wait().await;
            s2.insert_issue_with_outbox(
                issue_id, t2.ws, t2.project, &t2.key, t2.actor, "racing", "",
            )
            .await
        });
        let deleted = delete.await.expect("delete task");
        let created = create.await.expect("create task");
        assert!(
            matches!(deleted, Ok(ProjectDeleteOutcome::Deleted { .. })),
            "run {run}: the delete always goes through: {deleted:?}"
        );
        let kept = count(
            &l.pool,
            "SELECT count(*) FROM issues WHERE id = $1",
            issue_id,
        )
        .await
            == 1;
        match created {
            Ok(_) => assert!(kept, "run {run}: a committed issue is kept"),
            Err(foundry_store::IssueInsertError::ProjectNotFound) => {
                assert!(!kept, "run {run}: a refused create wrote nothing")
            }
            Err(other) => panic!("run {run}: an unmapped error (a 500): {other:?}"),
        }
    }
}

/// DDD-15 vs DDD-6: a rename and a delete of one project are totally ordered.
/// Either the rename committed first (the delete record carries the new name)
/// or the delete did (the rename is NotFound, the uniform 404).
#[tokio::test]
async fn a_rename_and_a_delete_of_one_project_are_totally_ordered_in_100_runs() {
    let l = lab().await;
    require_records(&l.pool).await;
    require_soft_delete(&l.pool).await;
    let (mut renamed_first, mut deleted_first) = (0, 0);
    for run in 1..=RACE_RUNS {
        let t = target(&l, 300 + run).await;
        let old = format!("Writer {}", 300 + run);
        let barrier = Arc::new(Barrier::new(2));
        let (s1, s2, g1, g2) = (l.store.clone(), l.store.clone(), barrier.clone(), barrier);
        let (ws, team, lena, project) = (l.ws, l.backend, l.lena, t.project);
        let rename = tokio::spawn(async move {
            g1.wait().await;
            s1.rename_project_with_audit(project, lena, "Renamed meanwhile")
                .await
        });
        let delete = tokio::spawn(async move {
            g2.wait().await;
            s2.soft_delete_project(ws, team, project, lena).await
        });
        let renamed = rename
            .await
            .expect("rename task")
            .expect("the rename never errors");
        let deleted = delete
            .await
            .expect("delete task")
            .expect("the delete never errors");
        match (&renamed, &deleted) {
            (
                ProjectRenameWrite::Renamed { old_name },
                ProjectDeleteOutcome::Deleted { name, .. },
            ) if *old_name == old && name == "Renamed meanwhile" => renamed_first += 1,
            (ProjectRenameWrite::NotFound, ProjectDeleteOutcome::Deleted { name, .. })
                if *name == old =>
            {
                deleted_first += 1
            }
            other => panic!("run {run}: not a total order: {other:?}"),
        }
    }
    assert_eq!(renamed_first + deleted_first, RACE_RUNS);
}

// ===========================================================================
// Slice 03: restore and the deleted list (03-01)
// ===========================================================================

#[tokio::test]
async fn a_restore_puts_back_exactly_what_was_there_except_tombstones_the_gc_purged_meanwhile() {
    let l = lab().await;
    require_records(&l.pool).await;
    require_soft_delete(&l.pool).await;
    // A second tombstone that will pass its 90-day window while the project is deleted.
    let aged = comment_tombstone(&l, l.spike, "SPIKE", l.issues[2]).await;
    let before = under(&l.pool, l.spike).await;
    l.store
        .soft_delete_project(l.ws, l.backend, l.spike, l.lena)
        .await
        .expect("delete");
    sqlx::query("UPDATE comments SET deleted_at = now() - interval '91 days' WHERE id = $1")
        .bind(aged)
        .execute(&l.pool)
        .await
        .expect("time passes for one tombstone");
    l.store
        .gc_tombstoned_comments(Duration::from_secs(90 * 24 * 3600), 100, 1000)
        .await
        .expect("the D12 GC runs as usual");
    assert_eq!(
        l.store
            .restore_deleted_project(l.spike)
            .await
            .expect("restore"),
        ProjectRestoreOutcome::Restored
    );
    let mut expected = before.clone();
    expected
        .get_mut("comments")
        .expect("comments")
        .retain(|row| !row.starts_with(&aged.to_string()));
    let mut after = under(&l.pool, l.spike).await;
    // The GC sweep may have announced itself on the outbox; INV-PL-4 is about rows under the project.
    after.remove("outbox");
    expected.remove("outbox");
    assert_eq!(
        after, expected,
        "live rows and in-window tombstones exactly as at the delete"
    );
    let (next,): (i32,) = sqlx::query_as("SELECT next_issue_number FROM projects WHERE id = $1")
        .bind(l.spike)
        .fetch_one(&l.pool)
        .await
        .expect("counter");
    assert_eq!(next, 4, "the next issue filed is SPIKE-4");
    let kinds: Vec<(String, Option<Id>)> = records(&l.pool)
        .await
        .into_iter()
        .map(|r| (r.kind, r.actor_id))
        .collect();
    assert_eq!(
        kinds,
        vec![("deleted".into(), Some(l.lena)), ("restored".into(), None)]
    );
}

/// DDD-8, DDD-9 (lock order team -> project): the restore takes the team's
/// `KEY SHARE` before the project's `FOR UPDATE`. While the test holds the team
/// row `FOR UPDATE`, the restore waits, and the project row is still free (a
/// third session takes it `FOR UPDATE NOWAIT`), so it was not locked first.
#[tokio::test]
async fn a_restore_takes_the_team_lock_before_the_project_lock() {
    let l = lab().await;
    require_records(&l.pool).await;
    require_soft_delete(&l.pool).await;
    l.store
        .soft_delete_project(l.ws, l.backend, l.spike, l.lena)
        .await
        .expect("delete");
    let mut team = l.pool.begin().await.expect("begin the team holder");
    sqlx::query("SELECT 1 FROM teams WHERE id = $1 FOR UPDATE")
        .bind(l.backend)
        .execute(&mut *team)
        .await
        .expect("hold the team row");
    let store = l.store.clone();
    let spike = l.spike;
    let restore = tokio::spawn(async move { store.restore_deleted_project(spike).await });
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !restore.is_finished(),
        "the restore waits on the team lock (KEY SHARE vs FOR UPDATE)"
    );
    let mut probe = l.pool.begin().await.expect("begin the probe");
    let free = sqlx::query("SELECT 1 FROM projects WHERE id = $1 FOR UPDATE NOWAIT")
        .bind(l.spike)
        .execute(&mut *probe)
        .await;
    assert!(
        free.is_ok(),
        "the project row is not locked while the restore waits on the team (team before project); got {free:?}"
    );
    probe.rollback().await.expect("release the probe");
    team.commit().await.expect("release the team");
    let out = tokio::time::timeout(Duration::from_secs(10), restore)
        .await
        .expect("the restore finishes once the team is released")
        .expect("join the restore")
        .expect("restore");
    assert_eq!(out, ProjectRestoreOutcome::Restored);
}

#[tokio::test]
async fn restoring_a_live_project_or_an_unknown_id_says_so_and_writes_nothing() {
    let l = lab().await;
    require_records(&l.pool).await;
    require_soft_delete(&l.pool).await;
    let before = everything(&l.pool).await;
    assert_eq!(
        l.store
            .restore_deleted_project(l.auth)
            .await
            .expect("restore"),
        ProjectRestoreOutcome::NotDeleted
    );
    assert_eq!(
        l.store
            .restore_deleted_project(Id::now_v7())
            .await
            .expect("restore"),
        ProjectRestoreOutcome::NotFound
    );
    assert_eq!(everything(&l.pool).await, before);
}

#[tokio::test]
async fn two_restores_at_once_restore_once_and_record_once() {
    let l = lab().await;
    require_records(&l.pool).await;
    require_soft_delete(&l.pool).await;
    l.store
        .soft_delete_project(l.ws, l.backend, l.spike, l.lena)
        .await
        .expect("delete");
    let barrier = Arc::new(Barrier::new(2));
    let mut tasks = Vec::new();
    for _ in 0..2 {
        let (s, g, id) = (l.store.clone(), barrier.clone(), l.spike);
        tasks.push(tokio::spawn(async move {
            g.wait().await;
            s.restore_deleted_project(id).await
        }));
    }
    let mut outcomes = Vec::new();
    for t in tasks {
        outcomes.push(t.await.expect("task").expect("never errors"));
    }
    outcomes.sort_by_key(|o| format!("{o:?}"));
    assert_eq!(
        outcomes,
        vec![
            ProjectRestoreOutcome::NotDeleted,
            ProjectRestoreOutcome::Restored
        ]
    );
    let restored = records(&l.pool)
        .await
        .into_iter()
        .filter(|r| r.kind == "restored")
        .count();
    assert_eq!(restored, 1);
}

#[tokio::test]
async fn the_deleted_list_is_oldest_deletion_first_with_who_and_how_many_issues() {
    let l = lab().await;
    require_records(&l.pool).await;
    require_soft_delete(&l.pool).await;
    assert!(
        l.store
            .list_deleted_projects()
            .await
            .expect("empty list")
            .is_empty(),
        "status: none"
    );
    l.store
        .soft_delete_project(l.ws, l.ops, l.read, l.priya)
        .await
        .expect("delete READ");
    l.store
        .soft_delete_project(l.ws, l.backend, l.spike, l.lena)
        .await
        .expect("delete SPIKE");
    let before = everything(&l.pool).await;
    let rows = l.store.list_deleted_projects().await.expect("list");
    assert_eq!(
        everything(&l.pool).await,
        before,
        "read-only: zero rows changed"
    );
    let shown: Vec<(Id, &str, &str, &str, &str, i64, &str)> = rows
        .iter()
        .map(|r| {
            (
                r.project_id,
                r.workspace_name.as_str(),
                r.team_slug.as_str(),
                r.name.as_str(),
                r.key_prefix.as_str(),
                r.issue_count,
                r.deleted_by_email.as_str(),
            )
        })
        .collect();
    assert_eq!(
        shown,
        vec![
            (
                l.read,
                "Canzan Labs",
                "ops",
                "Reading List",
                "READ",
                0,
                "priya.raman@canzan.dev"
            ),
            (
                l.spike,
                "Canzan Labs",
                "backend",
                "Auth v2 Spike",
                "SPIKE",
                3,
                "lena.park@canzan.dev"
            ),
        ]
    );
    assert!(rows[0].deleted_at <= rows[1].deleted_at);
}

/// OQ-4, INV-PL-5: while deleted, the project's slug (in its team) and key (in
/// its workspace) stay taken at the database, so a restore never collides.
#[tokio::test]
async fn a_deleted_projects_slug_and_key_stay_taken() {
    let l = lab().await;
    require_soft_delete(&l.pool).await;
    l.store
        .soft_delete_project(l.ws, l.backend, l.spike, l.lena)
        .await
        .expect("delete");
    let slug = l
        .store
        .insert_project(
            Id::now_v7(),
            l.ws,
            l.backend,
            "Auth v2 Spike",
            "auth-v2-spike",
            "SPIKY",
        )
        .await;
    assert!(
        matches!(slug, Err(foundry_store::ProjectInsertError::DuplicateSlug)),
        "{slug:?}"
    );
    let key = l
        .store
        .insert_project(Id::now_v7(), l.ws, l.ops, "Spike Two", "spike-two", "SPIKE")
        .await;
    assert!(
        matches!(key, Err(foundry_store::ProjectInsertError::DuplicateKey)),
        "{key:?}"
    );
}

// ===========================================================================
// The record stays out of the per-workspace export (DDD-14, 01-01)
// ===========================================================================

/// DDD-14, ADR-WORKSPACE-RENAME-002: `project_lifecycle_events` is not a tenant
/// table. `TENANT_TABLES` still lists exactly ten tables without it, and a
/// workspace export after a recorded rename carries no record.
#[tokio::test]
async fn project_lifecycle_events_is_not_a_tenant_table() {
    let l = lab().await;
    require_records(&l.pool).await;
    assert_eq!(
        foundry_store::TENANT_TABLES.len(),
        10,
        "TENANT_TABLES is still ten"
    );
    assert!(
        !foundry_store::TENANT_TABLES.contains(&"project_lifecycle_events"),
        "the record is not a tenant table"
    );
    l.store
        .rename_project_with_audit(l.auth, l.lena, "Identity & Access")
        .await
        .expect("a recorded rename");
    let export = l.store.export_workspace(l.ws).await.expect("export");
    assert!(
        export
            .tables
            .iter()
            .all(|(table, _)| table != "project_lifecycle_events"),
        "the export carries no project_lifecycle_events table"
    );
    assert!(
        export
            .tables
            .iter()
            .flat_map(|(_, rows)| rows.iter())
            .all(|row| !row.contains("\"new_name\"")),
        "no exported row is a lifecycle record"
    );
}
