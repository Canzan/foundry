//! team-lifecycle — a rename is a locked, audited, name-only write (DISCUSS D4,
//! D15, OQ-6, INV-TL-5, INV-TL-8, INV-TL-9; DESIGN DDD-13, DDD-14;
//! ADR-TEAM-LIFECYCLE-003 decision 2). DISTILL 2026-10-11.
//!
//! `rename_team_with_audit` locks the team row `FOR NO KEY UPDATE`, writes
//! nothing for a byte-equal name, else updates the name (never the slug) and
//! writes one `renamed` record carrying the name read under the lock, all in
//! one transaction. These tests pin: the slug and every team row untouched, the
//! no-op, the case-only rename, atomicity with the record, chained old names
//! under concurrent renames, and the total order against a delete.
//!
//! Shipped at team-lifecycle 04-01 (written RED by DISTILL). Every test still
//! passes `require_lifecycle_records_installed` first, so a tree without
//! migration 0023 fails for that reason and no other.
//!
//! Real Postgres (testcontainers `postgres:16-alpine`); integration level,
//! example-based, races repeated 100 times (Mandates 9/11).

use foundry_store::{run_migrations, Store, TeamDeleteOutcome, TeamRenameWrite};
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

async fn require_lifecycle_records_installed(pool: &PgPool) {
    let (present,): (bool,) =
        sqlx::query_as("SELECT to_regclass('team_lifecycle_events') IS NOT NULL")
            .fetch_one(pool)
            .await
            .expect("look the record table up");
    assert!(
        present,
        "RED gate: migration 0023 has not created team_lifecycle_events yet (DDD-11)"
    );
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

/// `(id, name, slug)` of every team, every team row, and every record as
/// `(kind, team_id, slug, name, new_name, actor)`.
type Snapshot = (
    BTreeSet<(uuid::Uuid, String, String)>,
    BTreeSet<(uuid::Uuid, uuid::Uuid, String)>,
    Vec<(
        String,
        uuid::Uuid,
        String,
        String,
        Option<String>,
        uuid::Uuid,
    )>,
);

async fn snapshot(pool: &PgPool) -> Snapshot {
    let teams: Vec<(uuid::Uuid, String, String)> =
        sqlx::query_as("SELECT id, name, slug FROM teams")
            .fetch_all(pool)
            .await
            .expect("read teams");
    let rows: Vec<(uuid::Uuid, uuid::Uuid, String)> =
        sqlx::query_as("SELECT team_id, user_id, role FROM team_memberships")
            .fetch_all(pool)
            .await
            .expect("read team rows");
    let records: Vec<(
        String,
        uuid::Uuid,
        String,
        String,
        Option<String>,
        uuid::Uuid,
    )> = sqlx::query_as(
        "SELECT kind, team_id, team_slug, name, new_name, actor_id FROM team_lifecycle_events
          ORDER BY created_at, id",
    )
    .fetch_all(pool)
    .await
    .expect("read records");
    (
        teams.into_iter().collect(),
        rows.into_iter().collect(),
        records,
    )
}

struct Labs {
    pool: PgPool,
    store: Store,
    ws: uuid::Uuid,
    backend: uuid::Uuid,
    payments: uuid::Uuid,
    priya: uuid::Uuid,
    lena: uuid::Uuid,
    _c: ContainerAsync<Postgres>,
}

async fn labs() -> Labs {
    let (pool, c) = migrated().await;
    let store = Store::from_pool(pool.clone());
    let ws = workspace(&pool, "Canzan Labs").await;
    let backend = team(&pool, ws, "Backend", "backend").await;
    team(&pool, ws, "General", "general").await;
    let priya = person(&pool, ws, "priya.raman@canzan.dev", "admin").await;
    let lena = person(&pool, ws, "lena.park@canzan.dev", "member").await;
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'lead')")
        .bind(backend)
        .bind(lena)
        .execute(&pool)
        .await
        .expect("Lena leads Backend");
    let globex = workspace(&pool, "Globex").await;
    let payments = team(&pool, globex, "Payments", "payments").await;
    Labs {
        pool,
        store,
        ws,
        backend,
        payments,
        priya,
        lena,
        _c: c,
    }
}

#[tokio::test]
async fn a_rename_changes_the_name_only_and_records_the_old_name() {
    let l = labs().await;
    require_lifecycle_records_installed(&l.pool).await;
    let (teams, rows, records) = snapshot(&l.pool).await;
    let outcome = l
        .store
        .rename_team_with_audit(l.ws, l.backend, l.priya, "Platform Engineering")
        .await
        .expect("no error");
    assert_eq!(
        outcome,
        TeamRenameWrite::Renamed {
            old_name: "Backend".into()
        }
    );
    let mut want_teams = teams.clone();
    want_teams.remove(&(l.backend, "Backend".into(), "backend".into()));
    want_teams.insert((l.backend, "Platform Engineering".into(), "backend".into()));
    let mut want_records = records;
    want_records.push((
        "renamed".into(),
        l.backend,
        "backend".into(),
        "Backend".into(),
        Some("Platform Engineering".into()),
        l.priya,
    ));
    assert_eq!(
        snapshot(&l.pool).await,
        (want_teams, rows, want_records),
        "the name only (the slug stays), no team row touched (INV-TL-8), one record"
    );
}

#[tokio::test]
async fn a_byte_equal_name_writes_nothing_and_a_case_only_change_is_a_rename() {
    let l = labs().await;
    require_lifecycle_records_installed(&l.pool).await;
    let before = snapshot(&l.pool).await;
    let same = l
        .store
        .rename_team_with_audit(l.ws, l.backend, l.lena, "Backend")
        .await
        .expect("no error");
    assert_eq!(same, TeamRenameWrite::Unchanged);
    assert_eq!(
        snapshot(&l.pool).await,
        before,
        "a byte-equal name writes nothing (D15)"
    );
    let case = l
        .store
        .rename_team_with_audit(l.ws, l.backend, l.lena, "backend")
        .await
        .expect("no error");
    assert_eq!(
        case,
        TeamRenameWrite::Renamed {
            old_name: "Backend".into()
        }
    );
}

#[tokio::test]
async fn another_workspaces_team_or_an_unknown_team_is_not_found_and_nothing_changes() {
    let l = labs().await;
    require_lifecycle_records_installed(&l.pool).await;
    let before = snapshot(&l.pool).await;
    for team_id in [l.payments, uuid::Uuid::now_v7()] {
        let outcome = l
            .store
            .rename_team_with_audit(l.ws, team_id, l.priya, "Platform")
            .await
            .expect("no error");
        assert_eq!(outcome, TeamRenameWrite::NotFound);
    }
    assert_eq!(snapshot(&l.pool).await, before);
}

/// INV-TL-9: a rename whose record cannot be written (an actor the record
/// cannot name) rolls the name back.
#[tokio::test]
async fn a_rename_whose_record_cannot_be_written_keeps_the_old_name() {
    let l = labs().await;
    require_lifecycle_records_installed(&l.pool).await;
    let before = snapshot(&l.pool).await;
    let result = l
        .store
        .rename_team_with_audit(l.ws, l.backend, uuid::Uuid::now_v7(), "Platform")
        .await;
    assert!(result.is_err(), "the record's actor FK refuses: {result:?}");
    assert_eq!(
        snapshot(&l.pool).await,
        before,
        "the rename rolled back with its record"
    );
}

/// ADR-003 decision 2: concurrent renames are serialised by the lock, so the
/// two records chain: Backend → first, first → second.
#[tokio::test]
async fn two_renames_at_once_record_chained_old_names_in_100_runs() {
    let l = labs().await;
    require_lifecycle_records_installed(&l.pool).await;
    for run in 1..=RACE_RUNS {
        sqlx::query("UPDATE teams SET name = 'Backend' WHERE id = $1")
            .bind(l.backend)
            .execute(&l.pool)
            .await
            .expect("reset: the name is Backend");
        sqlx::query("DELETE FROM team_lifecycle_events")
            .execute(&l.pool)
            .await
            .expect("reset: no record (test-only)");
        let barrier = Arc::new(Barrier::new(2));
        let (s1, s2, g1, g2) = (l.store.clone(), l.store.clone(), barrier.clone(), barrier);
        let (ws, backend, priya, lena) = (l.ws, l.backend, l.priya, l.lena);
        let one = tokio::spawn(async move {
            g1.wait().await;
            s1.rename_team_with_audit(ws, backend, priya, "Platform")
                .await
        });
        let two = tokio::spawn(async move {
            g2.wait().await;
            s2.rename_team_with_audit(ws, backend, lena, "Infrastructure")
                .await
        });
        one.await.expect("task").expect("no error");
        two.await.expect("task").expect("no error");
        let (_, _, records) = snapshot(&l.pool).await;
        let chain: Vec<(String, Option<String>)> = records
            .into_iter()
            .map(|(_, _, _, name, new_name, _)| (name, new_name))
            .collect();
        let ok = chain
            == vec![
                ("Backend".to_string(), Some("Platform".to_string())),
                ("Platform".to_string(), Some("Infrastructure".to_string())),
            ]
            || chain
                == vec![
                    ("Backend".to_string(), Some("Infrastructure".to_string())),
                    ("Infrastructure".to_string(), Some("Platform".to_string())),
                ];
        assert!(
            ok,
            "run {run}: the records chain from Backend; got {chain:?}"
        );
    }
}

/// ADR-003 decision 2: a rename and a delete are totally ordered. Either the
/// rename commits first (and the delete's record carries the new name) or the
/// delete does (and the rename is `NotFound`).
#[tokio::test]
async fn a_rename_racing_a_delete_is_ordered_in_100_runs() {
    let l = labs().await;
    require_lifecycle_records_installed(&l.pool).await;
    let mut ops = l.backend;
    for run in 1..=RACE_RUNS {
        sqlx::query("DELETE FROM teams WHERE id = $1")
            .bind(ops)
            .execute(&l.pool)
            .await
            .expect("reset: remove last run's team");
        ops = team(&l.pool, l.ws, "Ops", "ops").await;
        let barrier = Arc::new(Barrier::new(2));
        let (s1, s2, g1, g2) = (l.store.clone(), l.store.clone(), barrier.clone(), barrier);
        let (ws, priya) = (l.ws, l.priya);
        let team_id = ops;
        let rename = tokio::spawn(async move {
            g1.wait().await;
            s1.rename_team_with_audit(ws, team_id, priya, "Operations")
                .await
        });
        let delete = tokio::spawn(async move {
            g2.wait().await;
            s2.delete_team_if_empty(ws, team_id, priya).await
        });
        let renamed = rename.await.expect("task").expect("no error");
        let deleted = delete.await.expect("task").expect("no error");
        match (&renamed, &deleted) {
            (TeamRenameWrite::Renamed { old_name }, TeamDeleteOutcome::Deleted { name, slug }) => {
                assert_eq!(
                    (old_name.as_str(), name.as_str(), slug.as_str()),
                    ("Ops", "Operations", "ops"),
                    "run {run}: rename first, so the delete saw the new name"
                );
            }
            (TeamRenameWrite::NotFound, TeamDeleteOutcome::Deleted { name, .. }) => {
                assert_eq!(
                    name, "Ops",
                    "run {run}: delete first, so the rename found nothing"
                );
            }
            other => panic!("run {run}: an unordered pair {other:?}"),
        }
        let (slugs,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM teams WHERE workspace_id = $1 AND slug = 'ops'")
                .bind(l.ws)
                .fetch_one(&l.pool)
                .await
                .expect("count");
        assert_eq!(slugs, 0, "run {run}: the team is gone either way");
    }
}
