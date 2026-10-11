//! team-lifecycle — a team is born led, at the store (DISCUSS D3, INV-TL-4,
//! INV-TL-7; DESIGN DDD-4, DDD-5; ADR-TEAM-LIFECYCLE-001 decision 4).
//! DISTILL 2026-10-11.
//!
//! `create_team_with_lead` writes the team and its creator's lead row in one
//! transaction, after locking the creator's admin row `FOR KEY SHARE`. These
//! tests pin that write: born led or not at all, an admin only, the slug index
//! as the backstop of a same-name race, and a create racing the creator's
//! removal from the workspace never leaving a lead outside it. The name rule,
//! the slug mint and the bounded fallback retry are the use-case's, proven in
//! `crates/foundry-services/tests/team_lifecycle_use_cases.rs`.
//!
//! RED scaffold: `create_team_with_lead` panics (`src/teams.rs`); every test is
//! `#[ignore = "pending: team-lifecycle 01-01"]`.
//!
//! Real Postgres (testcontainers `postgres:16-alpine`); integration level,
//! example-based, races repeated 100 times (Mandates 9/11).

use foundry_store::{run_migrations, RemoveWorkspaceMemberOutcome, Store, TeamCreateOutcome};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
use tokio::sync::Barrier;

const RACE_RUNS: usize = 100;

async fn migrated() -> (PgPool, ContainerAsync<Postgres>) {
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
    run_migrations(&pool).await.expect("apply every migration");
    (pool, c)
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
    join(pool, ws, id, role).await;
    id
}

async fn join(pool: &PgPool, ws: uuid::Uuid, user: uuid::Uuid, role: &str) {
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, $3)
             ON CONFLICT DO NOTHING",
    )
    .bind(ws)
    .bind(user)
    .bind(role)
    .execute(pool)
    .await
    .expect("insert workspace membership");
}

/// `(team name, slug, Some(lead email))` for every team, and every team row.
async fn teams_and_rows(
    pool: &PgPool,
) -> (
    BTreeSet<(String, String)>,
    BTreeSet<(String, String, String)>,
) {
    let teams: Vec<(String, String)> = sqlx::query_as("SELECT name, slug FROM teams")
        .fetch_all(pool)
        .await
        .expect("read teams");
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT t.slug, u.email_lower, tm.role FROM team_memberships tm
           JOIN teams t ON t.id = tm.team_id JOIN users u ON u.id = tm.user_id",
    )
    .fetch_all(pool)
    .await
    .expect("read team rows");
    (teams.into_iter().collect(), rows.into_iter().collect())
}

struct Labs {
    pool: PgPool,
    store: Store,
    ws: uuid::Uuid,
    priya: uuid::Uuid,
    jeff: uuid::Uuid,
    lena: uuid::Uuid,
    _c: ContainerAsync<Postgres>,
}

async fn labs() -> Labs {
    let (pool, c) = migrated().await;
    let store = Store::from_pool(pool.clone());
    let ws = workspace(&pool, "Canzan Labs").await;
    let priya = person(&pool, ws, "priya.raman@canzan.dev", "admin").await;
    let jeff = person(&pool, ws, "jeffabailey@gmail.com", "admin").await;
    let lena = person(&pool, ws, "lena.park@canzan.dev", "member").await;
    Labs {
        pool,
        store,
        ws,
        priya,
        jeff,
        lena,
        _c: c,
    }
}

#[tokio::test]
#[ignore = "pending: team-lifecycle 01-01"]
async fn an_admins_create_writes_the_team_and_her_lead_row_and_nothing_else() {
    let l = labs().await;
    let (teams_before, rows_before) = teams_and_rows(&l.pool).await;
    let outcome = l
        .store
        .create_team_with_lead(l.ws, l.priya, "Platform", "platform")
        .await
        .expect("no error");
    let TeamCreateOutcome::Created { team_id } = outcome else {
        panic!("an admin's create goes through: {outcome:?}");
    };
    let (teams, rows) = teams_and_rows(&l.pool).await;
    let mut want_teams = teams_before;
    want_teams.insert(("Platform".into(), "platform".into()));
    let mut want_rows = rows_before;
    want_rows.insert((
        "platform".into(),
        "priya.raman@canzan.dev".into(),
        "lead".into(),
    ));
    assert_eq!(
        (teams, rows),
        (want_teams, want_rows),
        "born led: exactly one team and one lead row"
    );
    let (ws,): (uuid::Uuid,) = sqlx::query_as("SELECT workspace_id FROM teams WHERE id = $1")
        .bind(team_id)
        .fetch_one(&l.pool)
        .await
        .expect("the returned id is the team's");
    assert_eq!(ws, l.ws);
}

#[tokio::test]
#[ignore = "pending: team-lifecycle 01-01"]
async fn a_non_admin_or_someone_outside_the_workspace_creates_nothing() {
    let l = labs().await;
    let before = teams_and_rows(&l.pool).await;
    let globex = workspace(&l.pool, "Globex").await;
    let hugo = person(&l.pool, globex, "hugo.weber@globex.example", "admin").await;
    for creator in [l.lena, hugo, uuid::Uuid::now_v7()] {
        let outcome = l
            .store
            .create_team_with_lead(l.ws, creator, "Platform", "platform")
            .await
            .expect("no error");
        assert_eq!(outcome, TeamCreateOutcome::NotAdmin);
    }
    assert_eq!(teams_and_rows(&l.pool).await, before, "nothing written");
}

#[tokio::test]
#[ignore = "pending: team-lifecycle 01-01"]
async fn a_taken_slug_is_a_duplicate_and_nothing_is_written() {
    let l = labs().await;
    l.store
        .create_team_with_lead(l.ws, l.priya, "Backend", "backend")
        .await
        .expect("no error");
    let before = teams_and_rows(&l.pool).await;
    let outcome = l
        .store
        .create_team_with_lead(l.ws, l.jeff, "Back-End", "backend")
        .await
        .expect("no error");
    assert_eq!(outcome, TeamCreateOutcome::DuplicateSlug);
    assert_eq!(
        teams_and_rows(&l.pool).await,
        before,
        "no team and no lead row"
    );
}

/// INV-TL-4 at the store: two creates of the same slug at the same moment give
/// one team (the unique index), the other `DuplicateSlug`, never an error.
#[tokio::test]
#[ignore = "pending: team-lifecycle 01-01"]
async fn two_creates_of_one_slug_at_once_give_one_team_in_100_runs() {
    let l = labs().await;
    for run in 1..=RACE_RUNS {
        sqlx::query("DELETE FROM teams WHERE workspace_id = $1")
            .bind(l.ws)
            .execute(&l.pool)
            .await
            .expect("reset: no team");
        let barrier = Arc::new(Barrier::new(2));
        let (s1, s2, g1, g2) = (l.store.clone(), l.store.clone(), barrier.clone(), barrier);
        let (ws, priya, jeff) = (l.ws, l.priya, l.jeff);
        let one = tokio::spawn(async move {
            g1.wait().await;
            s1.create_team_with_lead(ws, priya, "Infra", "infra").await
        });
        let two = tokio::spawn(async move {
            g2.wait().await;
            s2.create_team_with_lead(ws, jeff, "Infra", "infra").await
        });
        let outcomes = [
            one.await.expect("task").expect("no error"),
            two.await.expect("task").expect("no error"),
        ];
        let created = outcomes
            .iter()
            .filter(|o| matches!(o, TeamCreateOutcome::Created { .. }))
            .count();
        assert!(
            created == 1 && outcomes.contains(&TeamCreateOutcome::DuplicateSlug),
            "run {run}: one created, one duplicate; got {outcomes:?}"
        );
        let (leads,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM team_memberships tm JOIN teams t ON t.id = tm.team_id
              WHERE t.workspace_id = $1 AND tm.role = 'lead'",
        )
        .bind(l.ws)
        .fetch_one(&l.pool)
        .await
        .expect("count leads");
        assert_eq!(
            leads, 1,
            "run {run}: the one team has exactly its creator as lead"
        );
    }
}

/// DDD-5 step 1: a create racing the creator's removal from the workspace never
/// leaves a lead outside the workspace. Either the create commits first (and
/// the removal is refused: she is now a team's last lead) or the removal does
/// (and the create is `NotAdmin`).
#[tokio::test]
#[ignore = "pending: team-lifecycle 01-01"]
async fn a_create_racing_the_creators_removal_never_leaves_a_lead_outside_the_workspace() {
    let l = labs().await;
    for run in 1..=RACE_RUNS {
        sqlx::query("DELETE FROM teams WHERE workspace_id = $1")
            .bind(l.ws)
            .execute(&l.pool)
            .await
            .expect("reset: no team");
        join(&l.pool, l.ws, l.priya, "admin").await;
        let barrier = Arc::new(Barrier::new(2));
        let (s1, s2, g1, g2) = (l.store.clone(), l.store.clone(), barrier.clone(), barrier);
        let (ws, priya) = (l.ws, l.priya);
        let create = tokio::spawn(async move {
            g1.wait().await;
            s1.create_team_with_lead(ws, priya, "Platform", "platform")
                .await
        });
        let removal = tokio::spawn(async move {
            g2.wait().await;
            s2.remove_workspace_member(ws, priya).await
        });
        let created = create.await.expect("task").expect("no error");
        let removed = removal.await.expect("task").expect("no error");
        match (&created, &removed) {
            (TeamCreateOutcome::Created { .. }, RemoveWorkspaceMemberOutcome::LastLead { .. })
            | (TeamCreateOutcome::NotAdmin, RemoveWorkspaceMemberOutcome::Removed) => {}
            other => panic!("run {run}: an inconsistent pair {other:?}"),
        }
        let (strays,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM team_memberships tm JOIN teams t ON t.id = tm.team_id
              WHERE NOT EXISTS (SELECT 1 FROM workspace_memberships wm
                                 WHERE wm.workspace_id = t.workspace_id AND wm.user_id = tm.user_id)",
        )
        .fetch_one(&l.pool)
        .await
        .expect("count strays");
        assert_eq!(
            strays, 0,
            "run {run}: no team row for someone outside the workspace"
        );
    }
}
