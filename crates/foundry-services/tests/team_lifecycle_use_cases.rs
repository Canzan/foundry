//! team-lifecycle — the create use-case under concurrency, and the token mint
//! racing a delete (DISCUSS D3, D6, D7, D16, OQ-3, INV-TL-2, INV-TL-4; DESIGN
//! DDD-4, DDD-9; ADR-TEAM-LIFECYCLE-001 decisions 3 and 5,
//! ADR-TEAM-LIFECYCLE-002 decision 5). DISTILL 2026-10-11.
//!
//! WHY HERE AND NOT IN THE STORE TESTS: the bounded fallback-slug retry lives in
//! `teams::create_team` (read siblings → check → mint → `create_team_with_lead`,
//! at most three attempts). Its races are proven against a real Postgres, 100
//! runs each, as DESIGN's Test Seams require (the HTTP lane repeats them 5
//! times):
//!   1. two creates of "Infra" at once → one team, one `NotUnique`;
//!   2. two creates of "インフラ" at once → one team, one `NotUnique` (DESIGN
//!      Changed Assumption 1: the retry re-runs the sibling check);
//!   3. "インフラ" racing "運用" → both created, at `team` and `team-2`.
//!
//! Plus the plain contract (creator becomes lead; a non-admin is the uniform
//! 404) and INV-TL-2's token mint: a mint scoped to a team whose delete commits
//! under it answers the shipped 422 `scope_team_not_in_workspace`, never the
//! 500 it answers today (DESIGN Changed Assumption 3).
//!
//! RED scaffold: `teams::create_team` panics (`src/teams.rs`); every create
//! test is `#[ignore = "pending: team-lifecycle 01-01 | 03-01"]`, the token
//! test `#[ignore = "pending: team-lifecycle 02-01"]`.

use foundry_core::TeamNameError;
use foundry_services::teams::{create_team, CreateTeamError};
use foundry_services::tokens::{mint_token, MintInput, ScopeChoice};
use foundry_services::{Principal, ServiceError};
use foundry_store::Store;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, PgPool};
use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
use tokio::sync::Barrier;

const RACE_RUNS: usize = 100;

struct Labs {
    store: Arc<Store>,
    pool: PgPool,
    ws: uuid::Uuid,
    priya: uuid::Uuid,
    jeff: uuid::Uuid,
    lena: uuid::Uuid,
    _c: ContainerAsync<Postgres>,
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
    let ws = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Canzan Labs')")
        .bind(ws)
        .execute(&pool)
        .await
        .expect("workspace");
    let mut ids = Vec::new();
    for (email, role) in [
        ("priya.raman@canzan.dev", "admin"),
        ("jeffabailey@gmail.com", "admin"),
        ("lena.park@canzan.dev", "member"),
    ] {
        let id = uuid::Uuid::now_v7();
        sqlx::query(
            "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
                  VALUES ($1, $2, $2, $2, 'x')",
        )
        .bind(id)
        .bind(email)
        .execute(&pool)
        .await
        .expect("user");
        sqlx::query(
            "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, $3)",
        )
        .bind(ws)
        .bind(id)
        .bind(role)
        .execute(&pool)
        .await
        .expect("membership");
        ids.push(id);
    }
    Labs {
        store: Arc::new(Store::from_pool(pool.clone())),
        pool,
        ws,
        priya: ids[0],
        jeff: ids[1],
        lena: ids[2],
        _c: c,
    }
}

fn human(l: &Labs, user_id: uuid::Uuid) -> Principal {
    Principal::Human {
        user_id,
        workspace_id: l.ws,
    }
}

/// `name -> slugs` for the given names.
async fn teams_named(l: &Labs, names: &[&str]) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for name in names {
        let slugs: Vec<(String,)> = sqlx::query_as(
            "SELECT slug FROM teams WHERE workspace_id = $1 AND name = $2 ORDER BY slug",
        )
        .bind(l.ws)
        .bind(*name)
        .fetch_all(&l.pool)
        .await
        .expect("read teams");
        out.push((name.to_string(), slugs.into_iter().map(|(s,)| s).collect()));
    }
    out
}

async fn reset(l: &Labs) {
    sqlx::query("DELETE FROM teams WHERE workspace_id = $1")
        .bind(l.ws)
        .execute(&l.pool)
        .await
        .expect("reset: no team");
}

async fn race(
    l: &Labs,
    first: (uuid::Uuid, &'static str),
    second: (uuid::Uuid, &'static str),
) -> [Result<foundry_services::teams::CreatedTeam, CreateTeamError>; 2] {
    let barrier = Arc::new(Barrier::new(2));
    let (s1, s2, g1, g2) = (l.store.clone(), l.store.clone(), barrier.clone(), barrier);
    let (p1, p2) = (human(l, first.0), human(l, second.0));
    let one = tokio::spawn(async move {
        g1.wait().await;
        create_team(&s1, &p1, first.1).await
    });
    let two = tokio::spawn(async move {
        g2.wait().await;
        create_team(&s2, &p2, second.1).await
    });
    [one.await.expect("task"), two.await.expect("task")]
}

#[tokio::test]
async fn an_admin_creates_a_team_she_leads_at_its_derived_address() {
    let l = labs().await;
    let created = create_team(&l.store, &human(&l, l.priya), "  Platform  ")
        .await
        .expect("an admin's create goes through");
    assert_eq!(
        (created.name.as_str(), created.slug.as_str()),
        ("Platform", "platform")
    );
    let (role,): (String,) =
        sqlx::query_as("SELECT role FROM team_memberships WHERE team_id = $1 AND user_id = $2")
            .bind(created.team_id)
            .bind(l.priya)
            .fetch_one(&l.pool)
            .await
            .expect("the creator's row");
    assert_eq!(role, "lead", "the creator is the lead (D3)");
}

#[tokio::test]
async fn a_non_admin_is_the_uniform_not_found_and_a_bad_name_is_its_reason() {
    let l = labs().await;
    let refused = create_team(&l.store, &human(&l, l.lena), "Platform").await;
    assert!(
        matches!(refused, Err(CreateTeamError::NotFound)),
        "{refused:?}"
    );
    for (raw, reason) in [
        ("   ", TeamNameError::Empty),
        ("Platform\u{202E}Ops", TeamNameError::ControlCharacter),
        (&*"a".repeat(65), TeamNameError::TooLong),
    ] {
        let result = create_team(&l.store, &human(&l, l.priya), raw).await;
        assert!(
            matches!(result, Err(CreateTeamError::InvalidName(r)) if r == reason),
            "{raw:?}: {result:?}"
        );
    }
    create_team(&l.store, &human(&l, l.priya), "Backend")
        .await
        .expect("Backend");
    // "Backend!" is a different name case-insensitively, but slugify drops the
    // trailing "!" and derives `backend`, Backend's stored slug (D6's slug arm).
    // ("Back-End" would not collide: slugify keeps the hyphen, `back-end`.)
    let duplicate = create_team(&l.store, &human(&l, l.jeff), "Backend!").await;
    assert!(
        matches!(
            duplicate,
            Err(CreateTeamError::InvalidName(TeamNameError::NotUnique))
        ),
        "a colliding derived slug is not unique (D6): {duplicate:?}"
    );
    assert_eq!(
        teams_named(&l, &["Backend!"]).await,
        vec![("Backend!".to_string(), vec![])]
    );
}

#[tokio::test]
#[ignore = "pending: team-lifecycle 03-01"]
async fn names_with_no_latin_letters_take_the_lowest_free_fallback_address() {
    let l = labs().await;
    let a = create_team(&l.store, &human(&l, l.priya), "インフラ")
        .await
        .expect("first");
    let b = create_team(&l.store, &human(&l, l.priya), "運用")
        .await
        .expect("second");
    assert_eq!((a.slug.as_str(), b.slug.as_str()), ("team", "team-2"));
}

/// INV-TL-4, Latin: one team, one `NotUnique`, never an error, 100 runs.
#[tokio::test]
#[ignore = "pending: team-lifecycle 03-01"]
async fn two_creates_of_the_same_name_at_once_give_one_team_in_100_runs() {
    let l = labs().await;
    for run in 1..=RACE_RUNS {
        reset(&l).await;
        let results = race(&l, (l.priya, "Infra"), (l.jeff, "Infra")).await;
        let created = results.iter().filter(|r| r.is_ok()).count();
        let refused = results
            .iter()
            .filter(|r| {
                matches!(
                    r,
                    Err(CreateTeamError::InvalidName(TeamNameError::NotUnique))
                )
            })
            .count();
        assert_eq!((created, refused), (1, 1), "run {run}: {results:?}");
        assert_eq!(
            teams_named(&l, &["Infra"]).await[0].1,
            vec!["infra".to_string()],
            "run {run}"
        );
    }
}

/// INV-TL-4, fallback, same name (DESIGN Changed Assumption 1): the retry
/// re-runs the sibling check, so the second create is `NotUnique`, never a
/// second team at `team-2`.
#[tokio::test]
#[ignore = "pending: team-lifecycle 03-01"]
async fn two_creates_of_the_same_non_latin_name_at_once_give_one_team_in_100_runs() {
    let l = labs().await;
    for run in 1..=RACE_RUNS {
        reset(&l).await;
        let results = race(&l, (l.priya, "インフラ"), (l.jeff, "インフラ")).await;
        let created = results.iter().filter(|r| r.is_ok()).count();
        let refused = results
            .iter()
            .filter(|r| {
                matches!(
                    r,
                    Err(CreateTeamError::InvalidName(TeamNameError::NotUnique))
                )
            })
            .count();
        assert_eq!((created, refused), (1, 1), "run {run}: {results:?}");
        assert_eq!(
            teams_named(&l, &["インフラ"]).await[0].1,
            vec!["team".to_string()],
            "run {run}"
        );
    }
}

/// INV-TL-4, fallback, different names: both succeed, at distinct addresses;
/// never a false duplicate, never `SlugContention`.
#[tokio::test]
#[ignore = "pending: team-lifecycle 03-01"]
async fn two_creates_of_different_non_latin_names_at_once_both_succeed_in_100_runs() {
    let l = labs().await;
    for run in 1..=RACE_RUNS {
        reset(&l).await;
        let results = race(&l, (l.priya, "インフラ"), (l.jeff, "運用")).await;
        assert!(results.iter().all(Result::is_ok), "run {run}: {results:?}");
        let slugs: BTreeSet<String> = teams_named(&l, &["インフラ", "運用"])
            .await
            .into_iter()
            .flat_map(|(_, s)| s)
            .collect();
        assert_eq!(
            slugs,
            BTreeSet::from(["team".to_string(), "team-2".to_string()]),
            "run {run}"
        );
    }
}

/// INV-TL-2 (DESIGN Changed Assumption 3): a token mint scoped to a team whose
/// delete commits while the mint waits on the foreign key answers the shipped
/// 422 `scope_team_not_in_workspace`, never a 500.
#[tokio::test]
async fn a_token_mint_racing_a_team_delete_answers_the_shipped_422() {
    let l = labs().await;
    let design = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Design', 'design')",
    )
    .bind(design)
    .bind(l.ws)
    .execute(&l.pool)
    .await
    .expect("Design");
    let mut delete = l.pool.begin().await.expect("begin delete");
    sqlx::query("SELECT 1 FROM teams WHERE id = $1 FOR UPDATE")
        .bind(design)
        .execute(&mut *delete)
        .await
        .expect("lock");
    sqlx::query("DELETE FROM teams WHERE id = $1")
        .bind(design)
        .execute(&mut *delete)
        .await
        .expect("delete, not yet committed");
    let store = l.store.clone();
    let principal = human(&l, l.priya);
    let mint = tokio::spawn(async move {
        let signer = foundry_auth::test_keys::signer();
        mint_token(
            &store,
            &signer,
            &principal,
            MintInput {
                label: "Design deploy bot".to_string(),
                scope: ScopeChoice::Team(design),
                ttl_days: 30,
            },
        )
        .await
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    delete.commit().await.expect("the delete commits");
    let result = mint.await.expect("task");
    assert!(
        matches!(&result, Err(ServiceError::Validation { code, .. }) if code == "scope_team_not_in_workspace"),
        "the mint answers its shipped 422, never Internal: {:?}",
        result.as_ref().map(|_| "minted")
    );
}
