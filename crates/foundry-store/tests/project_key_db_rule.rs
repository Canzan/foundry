//! The stored project key rule, on a real Postgres.
//!
//! L3 / T2: the database admits a key_prefix write exactly when the rule the
//! application enforces accepts the same text. The two verdicts are read
//! separately for the same text — the domain answer from the construction
//! point, the database answer from a real insert — and only then compared, so
//! neither is derived from the other.
//!
//! L6: applying the widening leaves every project row written before it
//! untouched. The rows are seeded while the database still carries the earlier
//! rule (the migration set staged up to the one before the widening), so a key
//! left alone and a key re-minted are distinguishable; seeding afterwards
//! cannot tell the two apart.

use foundry_core::ProjectKey;
use foundry_store::{run_migrations, run_migrations_from_dir};
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use proptest::test_runner::TestRunner;
use sqlx::postgres::{PgDatabaseError, PgPoolOptions};
use sqlx::PgPool;
use std::path::PathBuf;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};

/// The last migration written before the key rule was widened.
const BEFORE_THE_WIDENING: &str = "0034";
/// Generated probe keys per run, beside the named ones.
const GENERATED_PROBES: usize = 64;

type Id = uuid::Uuid;

async fn fresh() -> (PgPool, ContainerAsync<Postgres>) {
    let container = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let host = container.get_host().await.expect("container host");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("container port");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&format!(
            "postgres://postgres:postgres@{host}:{port}/postgres"
        ))
        .await
        .expect("connect pool");
    (pool, container)
}

fn production_migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

/// A private copy of the production migrations up to and including `through`.
fn staged_migrations(through: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("foundry-pkl-{}", Id::now_v7()));
    std::fs::create_dir_all(&dir).expect("stage dir");
    let mut names: Vec<String> = std::fs::read_dir(production_migrations_dir())
        .expect("read migrations")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".sql") && n.get(..4).is_some_and(|v| v <= through))
        .collect();
    names.sort();
    for name in names {
        std::fs::copy(production_migrations_dir().join(&name), dir.join(&name))
            .expect("stage file");
    }
    dir
}

/// A workspace and a team for the project rows to hang from.
async fn seed_parents(pool: &PgPool) -> (Id, Id) {
    let workspace = Id::now_v7();
    let team = Id::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(workspace)
        .bind("Canzan Labs")
        .execute(pool)
        .await
        .expect("insert workspace");
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
        .bind(team)
        .bind(workspace)
        .bind("Backend")
        .bind("backend")
        .execute(pool)
        .await
        .expect("insert team");
    (workspace, team)
}

/// Offer the database one key_prefix on a project row of its own and report
/// whether the write was admitted. A row that lands is removed again, so the
/// unique key per workspace never decides a later probe.
async fn the_database_admits(pool: &PgPool, workspace: Id, team: Id, key: &str) -> bool {
    let id = Id::now_v7();
    let name = format!("Probe {id}");
    let slug = format!("probe-{id}");
    let written = sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(team)
    .bind(workspace)
    .bind(&name)
    .bind(&slug)
    .bind(key)
    .execute(pool)
    .await;
    match written {
        Ok(_) => {
            sqlx::query("DELETE FROM projects WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await
                .expect("remove the probe row");
            true
        }
        Err(err) => {
            let database = err
                .as_database_error()
                .unwrap_or_else(|| panic!("the database refused {key:?} with {err:?}"))
                .downcast_ref::<PgDatabaseError>();
            assert_eq!(
                database.code(),
                "23514",
                "the probe of {key:?} was refused for {reason:?}, which is not the stored key \
                 rule; the probe itself is at fault",
                reason = database.message()
            );
            false
        }
    }
}

fn probe_keys() -> Vec<String> {
    let mut keys: Vec<String> = ["AU", "PLATFORMOP", "PLATFORMOPS", "auth", "AU1", ""]
        .iter()
        .map(|k| (*k).to_string())
        .collect();
    let characters = prop_oneof![
        30 => prop::char::range('A', 'Z'),
        10 => prop::char::range('a', 'z'),
        6 => prop::char::range('0', '9'),
        4 => prop::sample::select(vec!['-', '_', '.', ' ', '#', '/', '+']),
        4 => prop::sample::select(vec!['é', 'Ä', 'ß', '日', '語', 'ボ']),
    ];
    let strategy =
        prop::collection::vec(characters, 0..=14).prop_map(|cs| cs.into_iter().collect::<String>());
    let mut runner = TestRunner::default();
    for _ in 0..GENERATED_PROBES {
        let drawn = strategy
            .new_tree(&mut runner)
            .expect("draw a probe key")
            .current();
        keys.push(drawn);
    }
    keys
}

#[tokio::test]
async fn the_stored_rule_admits_exactly_the_keys_the_application_accepts() {
    let (pool, _container) = fresh().await;
    run_migrations(&pool).await.expect("apply every migration");
    let (workspace, team) = seed_parents(&pool).await;

    let mut disagreements: Vec<String> = Vec::new();
    for key in probe_keys() {
        let application_accepts = ProjectKey::try_new(&key).is_ok();
        let database_admits = the_database_admits(&pool, workspace, team, &key).await;
        if application_accepts != database_admits {
            disagreements.push(format!(
                "{key:?}: the application {a} it, the database {d} it",
                a = if application_accepts {
                    "accepts"
                } else {
                    "refuses"
                },
                d = if database_admits { "admits" } else { "refuses" },
            ));
        }
    }
    assert!(
        disagreements.is_empty(),
        "WHAT: the stored rule and the rule the application enforces disagree on {n} key(s): \
         {disagreements:?}. WHY: a stored rule that is stricter turns an accepted create into a \
         server error, and one that is looser leaves the key shape unguarded for a writer that \
         never passes the application. HOW: state the same rule on both sides. \
         The empty key and the keys AU, PLATFORMOP, PLATFORMOPS, auth and AU1 are among the \
         keys probed here.",
        n = disagreements.len()
    );
}

#[tokio::test]
async fn widening_the_stored_rule_leaves_every_earlier_project_untouched() {
    let (pool, _container) = fresh().await;
    run_migrations_from_dir(&pool, &staged_migrations(BEFORE_THE_WIDENING))
        .await
        .expect("stand where the database stood before the widening");
    let (workspace, team) = seed_parents(&pool).await;

    // The two ends of what the earlier rule allowed, each with an issue.
    for (key, name, slug) in [
        ("AU", "Auth v1", "auth-v1"),
        ("AUTHWS", "Auth Website", "auth-website"),
    ] {
        let project = Id::now_v7();
        sqlx::query(
            "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(project)
        .bind(team)
        .bind(workspace)
        .bind(name)
        .bind(slug)
        .bind(key)
        .execute(&pool)
        .await
        .unwrap_or_else(|err| panic!("seed the project keyed {key} before the widening: {err}"));
    }
    let before: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT key_prefix, name, slug FROM projects ORDER BY key_prefix, name, slug",
    )
    .fetch_all(&pool)
    .await
    .expect("read the earlier projects");

    run_migrations(&pool)
        .await
        .expect("apply the widening over the earlier database");

    let after: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT key_prefix, name, slug FROM projects ORDER BY key_prefix, name, slug",
    )
    .fetch_all(&pool)
    .await
    .expect("read the projects after the widening");
    assert_eq!(
        after, before,
        "WHAT: the projects written before the widening read differently afterwards. \
         WHY: an operator who wrote down a key, a board address or an issue key of an older \
         project must still find it. HOW: widen the rule without rewriting a row."
    );
    for (key, _, _) in &before {
        assert!(
            ProjectKey::try_new(key).is_ok(),
            "WHAT: the stored key {key:?} is no longer one the application accepts. \
             WHY: every key kept in the database must still be readable on its board and in \
             its issue keys. HOW: keep the shorter keys acceptable."
        );
    }
    assert!(
        the_database_admits(&pool, workspace, team, "PLATFORMOP").await,
        "WHAT: after the widening the database still refuses the ten-letter key PLATFORMOP. \
         WHY: the operator may now give a project a key of up to ten capital letters, and the \
         stored rule is what decides the write. HOW: widen the stored rule in the same change."
    );
}
