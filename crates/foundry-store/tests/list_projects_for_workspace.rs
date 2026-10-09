//! dashboard-enhancements US-05 (AC-05.1/.2) — the dashboard project-index query.
//!
//! The signed-in dashboard lists the acting workspace's projects via ONE
//! tenant-scoped read: `Store::list_projects_for_workspace(workspace_id)` returns
//! `(team_slug, project_slug, name, key_prefix)` rows ordered by name. This
//! backfills the coverage the base dashboard shipped without (`51ba981`, D6),
//! pinning that query's contract directly at the store boundary:
//!
//! - a workspace's projects come back ordered by name (AC-05.1);
//! - a SECOND coexisting workspace's project never leaks into the result
//!   (tenant isolation — the crux, AC-05.1);
//! - a project-less workspace yields an empty vec (AC-05.2).
//!
//! Runs against a real Postgres (testcontainers, @real-io): the JOIN + the
//! `WHERE workspace_id` scoping + the `ORDER BY name` can't be faked. Each test
//! runs the full production migration set on a fresh container.

use foundry_store::{run_migrations, Store};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ImageExt;

async fn fresh_postgres() -> (
    String,
    testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
) {
    let container = Postgres::default()
        .with_tag("16-alpine") // match production Postgres
        .start()
        .await
        .expect("start postgres container");
    let host = container.get_host().await.expect("container host");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("container port");
    let base = format!("postgres://postgres:postgres@{host}:{port}/postgres");
    (base, container)
}

async fn migrated_store(base: &str) -> Store {
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(10))
        .connect(base)
        .await
        .expect("connect pool");
    run_migrations(&pool).await.expect("run migrations");
    Store::from_pool(pool)
}

async fn seed_workspace(store: &Store, name: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(id)
        .bind(name)
        .execute(store.pool())
        .await
        .expect("insert workspace");
    id
}

async fn seed_team(store: &Store, workspace_id: uuid::Uuid, name: &str, slug: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind(workspace_id)
        .bind(name)
        .bind(slug)
        .execute(store.pool())
        .await
        .expect("insert team");
    id
}

async fn seed_project(
    store: &Store,
    team_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
    name: &str,
    slug: &str,
    key_prefix: &str,
) {
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(team_id)
    .bind(workspace_id)
    .bind(name)
    .bind(slug)
    .bind(key_prefix)
    .execute(store.pool())
    .await
    .expect("insert project");
}

/// A workspace's projects come back ordered by name, and a SECOND workspace's
/// project never leaks in — the two assertions the base dashboard rests on
/// (ordering for a stable render, tenant isolation for ADR-002 scoping).
#[tokio::test]
async fn lists_workspace_projects_ordered_by_name_and_isolated() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    // Workspace A (the acting tenant): projects seeded OUT of name order so a
    // missing ORDER BY would surface them insertion-ordered.
    let acme = seed_workspace(&store, "Acme").await;
    let acme_team = seed_team(&store, acme, "General", "general").await;
    seed_project(&store, acme_team, acme, "Zebra", "zebra", "ZEB").await;
    seed_project(&store, acme_team, acme, "Alpha", "alpha", "ALP").await;

    // Workspace B (a FOREIGN tenant): its project must never appear in A's list.
    let globex = seed_workspace(&store, "Globex").await;
    let globex_team = seed_team(&store, globex, "General", "general").await;
    seed_project(&store, globex_team, globex, "Foreign", "foreign", "FGN").await;

    // The acting user is on BOTH teams, so only the workspace scope can keep
    // the foreign tenant's project out.
    let user = seed_user(&store, acme, "ada@acme.example").await;
    join_team(&store, acme_team, user).await;
    join_team(&store, globex_team, user).await;

    let projects = store
        .list_projects_for_workspace(acme, user)
        .await
        .expect("list projects query succeeds");

    let names: Vec<&str> = projects.iter().map(|p| p.2.as_str()).collect();
    assert_eq!(
        names,
        vec!["Alpha", "Zebra"],
        "the acting workspace's projects must come back ordered by name"
    );
    assert!(
        !names.contains(&"Foreign"),
        "a foreign workspace's project must never leak into the tenant-scoped list: {projects:?}"
    );
}

/// A workspace with no projects resolves to an empty vec (AC-05.2) — the
/// dashboard then renders its "no projects yet" empty state rather than erroring.
#[tokio::test]
async fn empty_workspace_yields_no_projects() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    let acme = seed_workspace(&store, "Acme").await;
    let user = seed_user(&store, acme, "ada@acme.example").await;

    let projects = store
        .list_projects_for_workspace(acme, user)
        .await
        .expect("list projects query succeeds");

    assert!(
        projects.is_empty(),
        "a project-less workspace must yield an empty list, got {projects:?}"
    );
}

async fn seed_user(store: &Store, workspace_id: uuid::Uuid, email: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, $2, 'unused-hash')",
    )
    .bind(id)
    .bind(email)
    .execute(store.pool())
    .await
    .expect("insert user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(id)
    .execute(store.pool())
    .await
    .expect("insert workspace membership");
    id
}

async fn join_team(store: &Store, team_id: uuid::Uuid, user_id: uuid::Uuid) {
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')")
        .bind(team_id)
        .bind(user_id)
        .execute(store.pool())
        .await
        .expect("insert team membership");
}

/// fix-hide-unreachable-boards — the dashboard list and the rail Board link must only
/// offer boards the member can open. The board gate is team-scoped
/// (`Store::is_team_member`), so the list must be too: a user on ONE of two teams sees
/// only that team's projects, and a workspace member on NO team (the invite-accept /
/// OIDC-provisioned shape) sees none.
///
/// RED in 01-01 against the unfiltered `WHERE p.workspace_id = $1` query: the other
/// team's project leaks in. 01-02 adds the member scope (the user id) and un-ignores.
#[tokio::test]
async fn lists_only_projects_of_the_members_teams() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    let acme = seed_workspace(&store, "Acme").await;
    let general = seed_team(&store, acme, "General", "general").await;
    let platform = seed_team(&store, acme, "Platform", "platform").await;
    seed_project(&store, general, acme, "Sandbox", "sandbox", "SBX").await;
    seed_project(&store, platform, acme, "Infra", "infra", "INF").await;

    let general_member = seed_user(&store, acme, "gail@acme.example").await;
    join_team(&store, general, general_member).await;
    let no_team_member = seed_user(&store, acme, "nora@acme.example").await;

    let gails = store
        .list_projects_for_workspace(acme, general_member)
        .await
        .expect("list projects query succeeds");
    let gail_names: Vec<&str> = gails.iter().map(|p| p.2.as_str()).collect();
    assert_eq!(
        gail_names,
        vec!["Sandbox"],
        "a member of team general only must be offered only general's projects \
         (the board gate 404s the rest): {gails:?}"
    );

    let noras = store
        .list_projects_for_workspace(acme, no_team_member)
        .await
        .expect("list projects query succeeds");
    assert!(
        noras.is_empty(),
        "a workspace member on no team must be offered no project: {noras:?}"
    );
}
