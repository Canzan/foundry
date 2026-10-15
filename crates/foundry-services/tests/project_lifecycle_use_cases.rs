//! project-lifecycle — the use-cases: two rename doors on one core, the
//! delete's confirm and write behind `can_manage`, the operator's list and
//! restore, and the team delete counting live projects only (DISCUSS D-B, D3,
//! D4, D7, D10, D16, OQ-1 (b), OQ-3, OQ-4, INV-PL-5, INV-PL-8; DESIGN DDD-10,
//! DDD-12, DDD-16 to DDD-18, DDD-21; ADR-PROJECT-LIFECYCLE-002 decision 5,
//! -003 decision 3, -004 decision 2). DISTILL 2026-10-13.
//!
//! WHY HERE: the authority gate order (session → team facts → `can_manage` →
//! live project → verb) and the shared rename core live in
//! `foundry_services::projects`; the HTTP lane proves the doors, these prove
//! the seam, against a real Postgres. Every refusal before the verb is
//! `NotFound`, the uniform 404, and writes nothing.
//!
//! Authored as RED in DISTILL 2026-10-13: the new use-cases are scaffolds that
//! panic, and tests needing 0025/0026/0027 assert a readable RED gate first.
//! Every test is ignored as `pending: project-lifecycle NN-NN`:
//! 01-02 (the team door and the shared core), 02-03 (delete), 03-01 (restore,
//! deleted list), 04-01 (team delete preview).
//!
//! Real Postgres (testcontainers `postgres:16-alpine`); example-based.

use foundry_core::{ProjectName, ProjectNameError};
use foundry_services::projects::{
    create_project, delete_preview, delete_project, list_deleted_projects, rename_team_project,
    restore_project, CreateProjectError, CreateProjectRequest, ProjectLifecycleError,
    RenameOutcome, RenameProjectError, RenameProjectRequest, TeamProjectRename,
};
use foundry_services::teams::{delete_preview as team_delete_preview, TeamDeletePreview};
use foundry_services::{Principal, Services};
use foundry_store::{ProjectRestoreOutcome, Store};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, PgPool};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};

type Id = uuid::Uuid;

/// Canzan Labs: Backend (Lena leads, Sam member) with Identity Platform (AUTH)
/// and Auth v2 Spike (SPIKE, 3 issues); Ops (nobody but Priya can manage) with
/// Homelab Ops (OPS) and Reading List (READ). Priya is a workspace admin on
/// neither team; Marco is on no team; Ines is an instance admin and a plain
/// member; Hugo is Globex's admin.
#[allow(dead_code)]
struct Labs {
    store: Arc<Store>,
    pool: PgPool,
    ws: Id,
    globex: Id,
    backend: Id,
    ops: Id,
    priya: Id,
    lena: Id,
    sam: Id,
    marco: Id,
    ines: Id,
    hugo: Id,
    auth: Id,
    spike: Id,
    homelab: Id,
    read: Id,
    _c: ContainerAsync<Postgres>,
}

async fn q(pool: &PgPool, sql: &str, binds: &[Id]) {
    let mut query = sqlx::query(sql);
    for b in binds {
        query = query.bind(*b);
    }
    query.execute(pool).await.expect(sql);
}

async fn user(pool: &PgPool, ws: Id, email: &str, role: &str) -> Id {
    let id = Id::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, $2, 'x')",
    )
    .bind(id)
    .bind(email)
    .execute(pool)
    .await
    .expect("user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, $3)",
    )
    .bind(ws)
    .bind(id)
    .bind(role)
    .execute(pool)
    .await
    .expect("membership");
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
        .expect("team");
    id
}

async fn place(pool: &PgPool, team: Id, user: Id, role: &str) {
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, $3)")
        .bind(team)
        .bind(user)
        .bind(role)
        .execute(pool)
        .await
        .expect("team row");
}

async fn project(store: &Store, ws: Id, team: Id, name: &str, slug: &str, key: &str) -> Id {
    let id = Id::now_v7();
    store
        .insert_project(id, ws, team, name, slug, key)
        .await
        .expect("project");
    id
}

async fn labs() -> Labs {
    let c = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let host = c.get_host().await.expect("container host");
    let port = c.get_host_port_ipv4(5432).await.expect("container port");
    let opts = PgConnectOptions::from_str(&format!(
        "postgres://postgres:postgres@{host}:{port}/postgres"
    ))
    .expect("parse url")
    .disable_statement_logging();
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(opts)
        .await
        .expect("connect pool");
    foundry_store::run_migrations(&pool)
        .await
        .expect("run migrations");
    let store = Store::from_pool(pool.clone());
    let ws = Id::now_v7();
    q(
        &pool,
        "INSERT INTO workspaces (id, name) VALUES ($1, 'Canzan Labs')",
        &[ws],
    )
    .await;
    let globex = Id::now_v7();
    q(
        &pool,
        "INSERT INTO workspaces (id, name) VALUES ($1, 'Globex')",
        &[globex],
    )
    .await;
    let priya = user(&pool, ws, "priya.raman@canzan.dev", "admin").await;
    let lena = user(&pool, ws, "lena.park@canzan.dev", "member").await;
    let sam = user(&pool, ws, "sam.okafor@canzan.dev", "member").await;
    let marco = user(&pool, ws, "marco.bianchi@canzan.dev", "member").await;
    let ines = user(&pool, ws, "ines.duarte@canzan.dev", "member").await;
    q(
        &pool,
        "INSERT INTO instance_admins (user_id) VALUES ($1)",
        &[ines],
    )
    .await;
    let hugo = user(&pool, globex, "hugo.weber@globex.example", "admin").await;
    let backend = team(&pool, ws, "Backend", "backend").await;
    let ops = team(&pool, ws, "Ops", "ops").await;
    place(&pool, backend, lena, "lead").await;
    place(&pool, backend, sam, "member").await;
    let auth = project(&store, ws, backend, "Identity Platform", "auth", "AUTH").await;
    let spike = project(
        &store,
        ws,
        backend,
        "Auth v2 Spike",
        "auth-v2-spike",
        "SPIKE",
    )
    .await;
    for title in ["one", "two", "three"] {
        store
            .insert_issue_with_outbox(Id::now_v7(), ws, spike, "SPIKE", lena, title, "")
            .await
            .expect("issue");
    }
    let homelab = project(&store, ws, ops, "Homelab Ops", "homelab-ops", "OPS").await;
    let read = project(&store, ws, ops, "Reading List", "reading-list", "READ").await;
    Labs {
        store: Arc::new(store),
        pool,
        ws,
        globex,
        backend,
        ops,
        priya,
        lena,
        sam,
        marco,
        ines,
        hugo,
        auth,
        spike,
        homelab,
        read,
        _c: c,
    }
}

fn human(user_id: Id, workspace_id: Id) -> Principal {
    Principal::Human {
        user_id,
        workspace_id,
    }
}

async fn require_table(pool: &PgPool, table: &str, gate: &str) {
    let (yes,): (bool,) = sqlx::query_as("SELECT to_regclass($1) IS NOT NULL")
        .bind(table)
        .fetch_one(pool)
        .await
        .expect("look up");
    assert!(yes, "RED gate: {gate}");
}

async fn require_records(pool: &PgPool) {
    require_table(
        pool,
        "project_lifecycle_events",
        "migration 0025 has not created project_lifecycle_events yet",
    )
    .await;
}

async fn require_soft_delete(pool: &PgPool) {
    require_table(
        pool,
        "live_projects",
        "migration 0026 has not created live_projects yet",
    )
    .await;
}

async fn mark_deleted(pool: &PgPool, project: Id, by: Id) {
    sqlx::query("UPDATE projects SET deleted_at = now(), deleted_by = $2 WHERE id = $1")
        .bind(project)
        .bind(by)
        .execute(pool)
        .await
        .expect("mark deleted");
}

/// Every row the use-cases could write, as checksums (refusals write nothing).
async fn everything(pool: &PgPool) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT 'p:' || md5(t::text) FROM projects t
         UNION ALL SELECT 'i:' || md5(t::text) FROM issues t
         UNION ALL SELECT 't:' || md5(t::text) FROM teams t
         ORDER BY 1",
    )
    .fetch_all(pool)
    .await
    .expect("snapshot");
    let mut out: Vec<String> = rows.into_iter().map(|(r,)| r).collect();
    let (has,): (bool,) =
        sqlx::query_as("SELECT to_regclass('project_lifecycle_events') IS NOT NULL")
            .fetch_one(pool)
            .await
            .expect("look up");
    if has {
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

async fn records(pool: &PgPool) -> Vec<(String, Id, String, Option<String>, Option<Id>)> {
    sqlx::query_as(
        "SELECT kind, project_id, name, new_name, actor_id FROM project_lifecycle_events ORDER BY created_at, id",
    )
    .fetch_all(pool)
    .await
    .expect("records")
}

async fn name_of(pool: &PgPool, project: Id) -> (String, String, String) {
    sqlx::query_as("SELECT name, slug, key_prefix FROM projects WHERE id = $1")
        .bind(project)
        .fetch_one(pool)
        .await
        .expect("project")
}

/// Describe a team-door rename outcome (the error type has no `Debug`).
fn said(r: &Result<TeamProjectRename, RenameProjectError>) -> String {
    match r {
        Ok(outcome) => format!("{outcome:?}"),
        Err(RenameProjectError::Forbidden) => "Forbidden".into(),
        Err(RenameProjectError::NotFound) => "NotFound".into(),
        Err(RenameProjectError::InvalidName(e)) => format!("InvalidName({e:?})"),
        Err(RenameProjectError::Store(e)) => format!("Store({e:?})"),
    }
}

// ---------------------------------------------------------------------------
// Slice 01: the team's rename door on the shared core (01-02)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_teams_lead_renames_a_project_name_only_with_one_record() {
    let l = labs().await;
    require_records(&l.pool).await;
    let r = rename_team_project(
        &l.store,
        &human(l.lena, l.ws),
        "backend",
        "auth",
        "Identity & Access",
    )
    .await;
    assert_eq!(
        said(&r),
        format!(
            "{:?}",
            TeamProjectRename::Renamed {
                old_name: "Identity Platform".into(),
                name: "Identity & Access".into()
            }
        )
    );
    assert_eq!(
        name_of(&l.pool, l.auth).await,
        ("Identity & Access".into(), "auth".into(), "AUTH".into())
    );
    assert_eq!(
        records(&l.pool).await,
        vec![(
            "renamed".into(),
            l.auth,
            "Identity Platform".into(),
            Some("Identity & Access".into()),
            Some(l.lena)
        )]
    );
}

#[tokio::test]
async fn a_workspace_admin_renames_a_project_of_a_team_they_are_not_on() {
    let l = labs().await;
    require_records(&l.pool).await;
    let r = rename_team_project(
        &l.store,
        &human(l.priya, l.ws),
        "ops",
        "homelab-ops",
        "Homelab Operations",
    )
    .await;
    assert!(said(&r).starts_with("Renamed"), "{}", said(&r));
    assert_eq!(
        name_of(&l.pool, l.homelab).await.1,
        "homelab-ops",
        "the slug stays"
    );
}

/// D-B, D4, DDD-18: everyone without `Manage` on the project's team, and every
/// unknown or foreign address, is `NotFound` with zero writes.
#[tokio::test]
async fn anyone_without_manage_or_any_unknown_address_is_not_found_and_nothing_changes() {
    let l = labs().await;
    let before = everything(&l.pool).await;
    for (who, ws, team, project, what) in [
        (l.sam, l.ws, "backend", "auth", "a member"),
        (l.marco, l.ws, "backend", "auth", "on no team"),
        (
            l.ines,
            l.ws,
            "backend",
            "auth",
            "an instance admin who is not a workspace admin",
        ),
        (
            l.hugo,
            l.globex,
            "backend",
            "auth",
            "another workspace's admin",
        ),
        (
            l.lena,
            l.ws,
            "ops",
            "reading-list",
            "the lead of another team",
        ),
        (l.priya, l.ws, "no-such-team", "auth", "an unknown team"),
        (
            l.priya,
            l.ws,
            "backend",
            "no-such-project",
            "an unknown project",
        ),
        (l.priya, l.ws, "ops", "auth", "a project of another team"),
    ] {
        let r = rename_team_project(&l.store, &human(who, ws), team, project, "Taken Over").await;
        assert_eq!(said(&r), "NotFound", "{what}");
    }
    assert_eq!(everything(&l.pool).await, before, "zero writes");
}

#[tokio::test]
async fn a_name_that_breaks_the_rule_is_refused_with_its_reason_and_nothing_changes() {
    let l = labs().await;
    let before = everything(&l.pool).await;
    let long = "x".repeat(257);
    for (raw, error) in [
        ("   ", ProjectNameError::Empty),
        ("Bad\u{7}Name", ProjectNameError::ControlCharacter),
        (long.as_str(), ProjectNameError::TooLong),
        ("homelab ops", ProjectNameError::NotUnique),
    ] {
        let r =
            rename_team_project(&l.store, &human(l.priya, l.ws), "ops", "reading-list", raw).await;
        assert_eq!(said(&r), format!("InvalidName({error:?})"), "{raw:?}");
    }
    assert_eq!(everything(&l.pool).await, before);
}

#[tokio::test]
async fn submitting_the_same_name_is_a_no_op_and_a_case_change_is_a_rename() {
    let l = labs().await;
    require_records(&l.pool).await;
    let before = everything(&l.pool).await;
    let r = rename_team_project(
        &l.store,
        &human(l.priya, l.ws),
        "ops",
        "reading-list",
        "Reading List",
    )
    .await;
    assert_eq!(
        said(&r),
        format!(
            "{:?}",
            TeamProjectRename::NoOp {
                name: "Reading List".into()
            }
        )
    );
    assert_eq!(everything(&l.pool).await, before, "no write, no record");
    let r = rename_team_project(
        &l.store,
        &human(l.priya, l.ws),
        "ops",
        "reading-list",
        "Reading list",
    )
    .await;
    assert!(said(&r).starts_with("Renamed"), "{}", said(&r));
}

/// D16, OQ-3: the instance-admin door now runs the same core and writes the
/// same record (DDD-16).
#[tokio::test]
async fn the_instance_admin_door_records_its_rename_too() {
    let l = labs().await;
    require_records(&l.pool).await;
    let services = Services::new(l.store.clone());
    let r = services
        .rename_project(RenameProjectRequest {
            acting_user_id: l.ines,
            project_id: l.read,
            new_name: "Reading Queue",
        })
        .await;
    assert!(
        matches!(r, Ok(RenameOutcome::Renamed { .. })),
        "the instance door renames"
    );
    assert_eq!(
        records(&l.pool).await,
        vec![(
            "renamed".into(),
            l.read,
            "Reading List".into(),
            Some("Reading Queue".into()),
            Some(l.ines)
        )]
    );
}

/// OQ-4, INV-PL-5, D16: a deleted project's name stays taken on both rename
/// doors and on create, and its key on create; a deleted project's id is not
/// found on the instance door.
#[tokio::test]
async fn a_deleted_projects_name_and_key_stay_taken_and_its_id_is_not_found() {
    let l = labs().await;
    require_soft_delete(&l.pool).await;
    mark_deleted(&l.pool, l.spike, l.lena).await;
    let r = rename_team_project(
        &l.store,
        &human(l.lena, l.ws),
        "backend",
        "auth",
        "auth v2 spike",
    )
    .await;
    assert_eq!(
        said(&r),
        format!("InvalidName({:?})", ProjectNameError::NotUnique),
        "team door"
    );
    let created = create_project(
        &l.store,
        CreateProjectRequest {
            workspace_id: l.ws,
            team_id: l.backend,
            name: ProjectName::try_new("auth v2 spike").expect("a valid name"),
            key_prefix: "SPIKY",
        },
    )
    .await;
    assert!(
        matches!(
            created,
            Err(CreateProjectError::InvalidName(ProjectNameError::NotUnique))
        ),
        "{created:?}"
    );
    let created = create_project(
        &l.store,
        CreateProjectRequest {
            workspace_id: l.ws,
            team_id: l.ops,
            name: ProjectName::try_new("Spike Two").expect("a valid name"),
            key_prefix: "SPIKE",
        },
    )
    .await;
    assert!(
        matches!(created, Err(CreateProjectError::DuplicateKey)),
        "{created:?}"
    );
    let services = Services::new(l.store.clone());
    let r = services
        .rename_project(RenameProjectRequest {
            acting_user_id: l.ines,
            project_id: l.spike,
            new_name: "Revived",
        })
        .await;
    assert!(
        matches!(r, Err(RenameProjectError::NotFound)),
        "the instance door finds no deleted project"
    );
    let r = rename_team_project(
        &l.store,
        &human(l.lena, l.ws),
        "backend",
        "auth-v2-spike",
        "Revived",
    )
    .await;
    assert_eq!(
        said(&r),
        "NotFound",
        "the team door finds no deleted project"
    );
}

// ---------------------------------------------------------------------------
// Slice 02: the delete (02-03)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_confirm_preview_counts_the_issues_and_writes_nothing() {
    let l = labs().await;
    let before = everything(&l.pool).await;
    let preview = delete_preview(&l.store, &human(l.lena, l.ws), "backend", "auth-v2-spike")
        .await
        .map_err(|e| format!("{e:?}"))
        .expect("Lena may delete Backend's projects");
    assert_eq!(
        (
            preview.project_name.as_str(),
            preview.key_prefix.as_str(),
            preview.issue_count,
            preview.team_slug.as_str()
        ),
        ("Auth v2 Spike", "SPIKE", 3, "backend")
    );
    let read = delete_preview(&l.store, &human(l.priya, l.ws), "ops", "reading-list")
        .await
        .map_err(|e| format!("{e:?}"))
        .expect("an admin may delete Ops's projects");
    assert_eq!(read.issue_count, 0);
    assert_eq!(
        everything(&l.pool).await,
        before,
        "read-only: zero rows changed"
    );
}

#[tokio::test]
async fn a_teams_lead_deletes_a_project_and_a_second_delete_is_not_found() {
    let l = labs().await;
    require_records(&l.pool).await;
    require_soft_delete(&l.pool).await;
    let deleted = delete_project(&l.store, &human(l.lena, l.ws), "backend", "auth-v2-spike")
        .await
        .map_err(|e| format!("{e:?}"))
        .expect("delete");
    assert_eq!(
        (deleted.name.as_str(), deleted.key_prefix.as_str()),
        ("Auth v2 Spike", "SPIKE")
    );
    let again = delete_project(&l.store, &human(l.lena, l.ws), "backend", "auth-v2-spike").await;
    assert!(
        matches!(again, Err(ProjectLifecycleError::NotFound)),
        "{again:?}"
    );
    let preview = delete_preview(&l.store, &human(l.lena, l.ws), "backend", "auth-v2-spike").await;
    assert!(
        matches!(preview, Err(ProjectLifecycleError::NotFound)),
        "{preview:?}"
    );
    let kinds: Vec<String> = records(&l.pool).await.into_iter().map(|r| r.0).collect();
    assert_eq!(kinds, vec!["deleted".to_string()], "one delete, one record");
}

#[tokio::test]
async fn anyone_without_manage_gets_not_found_on_the_confirm_and_the_delete_and_nothing_changes() {
    let l = labs().await;
    let before = everything(&l.pool).await;
    for (who, ws, team, project, what) in [
        (l.sam, l.ws, "backend", "auth-v2-spike", "a member"),
        (l.marco, l.ws, "backend", "auth-v2-spike", "on no team"),
        (
            l.ines,
            l.ws,
            "backend",
            "auth-v2-spike",
            "an instance admin only",
        ),
        (
            l.hugo,
            l.globex,
            "backend",
            "auth-v2-spike",
            "another workspace's admin",
        ),
        (
            l.lena,
            l.ws,
            "ops",
            "reading-list",
            "the lead of another team",
        ),
        (
            l.priya,
            l.ws,
            "backend",
            "no-such-project",
            "an unknown project",
        ),
        (
            l.priya,
            l.ws,
            "no-such-team",
            "auth-v2-spike",
            "an unknown team",
        ),
    ] {
        let p = delete_preview(&l.store, &human(who, ws), team, project).await;
        assert!(
            matches!(p, Err(ProjectLifecycleError::NotFound)),
            "confirm, {what}: {p:?}"
        );
        let d = delete_project(&l.store, &human(who, ws), team, project).await;
        assert!(
            matches!(d, Err(ProjectLifecycleError::NotFound)),
            "delete, {what}: {d:?}"
        );
    }
    assert_eq!(everything(&l.pool).await, before, "zero writes");
}

// ---------------------------------------------------------------------------
// Slice 03: the operator's doors (03-01)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_operator_lists_and_restores_a_deleted_project_and_refusals_change_nothing() {
    let l = labs().await;
    require_records(&l.pool).await;
    require_soft_delete(&l.pool).await;
    assert!(
        list_deleted_projects(&l.store)
            .await
            .expect("list")
            .is_empty(),
        "status: none"
    );
    delete_project(&l.store, &human(l.lena, l.ws), "backend", "auth-v2-spike")
        .await
        .map_err(|e| format!("{e:?}"))
        .expect("delete");
    let listed = list_deleted_projects(&l.store).await.expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(
        (
            listed[0].project_id,
            listed[0].issue_count,
            listed[0].deleted_by_email.as_str()
        ),
        (l.spike, 3, "lena.park@canzan.dev")
    );
    let before = everything(&l.pool).await;
    assert_eq!(
        restore_project(&l.store, l.auth).await.expect("restore"),
        ProjectRestoreOutcome::NotDeleted
    );
    assert_eq!(
        restore_project(&l.store, Id::now_v7())
            .await
            .expect("restore"),
        ProjectRestoreOutcome::NotFound
    );
    assert_eq!(
        everything(&l.pool).await,
        before,
        "refused restores write nothing"
    );
    assert_eq!(
        restore_project(&l.store, l.spike).await.expect("restore"),
        ProjectRestoreOutcome::Restored
    );
    assert!(list_deleted_projects(&l.store)
        .await
        .expect("list")
        .is_empty());
    let preview = delete_preview(&l.store, &human(l.lena, l.ws), "backend", "auth-v2-spike")
        .await
        .map_err(|e| format!("{e:?}"))
        .expect("the restored project answers again");
    assert_eq!(preview.issue_count, 3);
}

// ---------------------------------------------------------------------------
// Slice 04: only live projects block a team delete (04-01)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_team_delete_preview_refuses_on_live_projects_only_and_names_only_them() {
    let l = labs().await;
    require_soft_delete(&l.pool).await;
    mark_deleted(&l.pool, l.read, l.priya).await;
    let preview = team_delete_preview(&l.store, &human(l.priya, l.ws), "ops")
        .await
        .map_err(|e| format!("{e:?}"))
        .expect("Priya may delete Ops");
    assert_eq!(
        preview,
        TeamDeletePreview::HasProjects {
            team_name: "Ops".into(),
            projects: vec![("Homelab Ops".into(), "OPS".into())]
        },
        "the deleted Reading List is not named"
    );
    mark_deleted(&l.pool, l.homelab, l.priya).await;
    let preview = team_delete_preview(&l.store, &human(l.priya, l.ws), "ops")
        .await
        .map_err(|e| format!("{e:?}"))
        .expect("Priya may delete Ops");
    assert!(
        matches!(preview, TeamDeletePreview::Confirm { .. }),
        "a team whose projects are all deleted can be deleted: {preview:?}"
    );
}

/// Every base table of the current schema, each as `table:count:checksum`
/// (DESIGN Contract Shapes: a read-only use-case changes zero rows anywhere).
async fn every_table(pool: &PgPool) -> Vec<String> {
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT table_name::text FROM information_schema.tables
          WHERE table_schema = current_schema() AND table_type = 'BASE TABLE'
          ORDER BY 1",
    )
    .fetch_all(pool)
    .await
    .expect("list the tables");
    let mut out = Vec::new();
    for (table,) in tables {
        let sql = format!(
            "SELECT count(*), coalesce(md5(string_agg(t::text, '|' ORDER BY t::text)), '')
               FROM \"{table}\" t"
        );
        let (n, digest): (i64, String) = sqlx::query_as(&sql)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|e| panic!("snapshot {table}: {e}"));
        out.push(format!("{table}:{n}:{digest}"));
    }
    out
}

/// DESIGN Contract Shapes (`teams::delete_preview`, read-only): previewing the
/// delete of a team that holds a deleted project, on the refusal branch (a live
/// project remains) and the confirm branch (only deleted ones), changes zero
/// rows in any table.
#[tokio::test]
async fn the_team_delete_preview_changes_zero_rows_with_deleted_projects() {
    let l = labs().await;
    require_soft_delete(&l.pool).await;
    mark_deleted(&l.pool, l.read, l.priya).await;
    let before = every_table(&l.pool).await;
    let refusal = team_delete_preview(&l.store, &human(l.priya, l.ws), "ops").await;
    assert!(refusal.is_ok(), "the preview answers: {:?}", refusal.err());
    assert_eq!(
        every_table(&l.pool).await,
        before,
        "the refusal preview wrote nothing"
    );
    mark_deleted(&l.pool, l.homelab, l.priya).await;
    let before = every_table(&l.pool).await;
    let confirm = team_delete_preview(&l.store, &human(l.priya, l.ws), "ops").await;
    assert!(confirm.is_ok(), "the preview answers: {:?}", confirm.err());
    assert_eq!(
        every_table(&l.pool).await,
        before,
        "the confirm preview wrote nothing"
    );
}
