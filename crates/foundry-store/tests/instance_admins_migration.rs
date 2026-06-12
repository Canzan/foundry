//! multi-workspace-provisioning — Slice 5 (step 01-01): the additive,
//! forward-only `0011_instance_admins.sql` migration.
//!
//! ADR-003/ADR-004 (D6): `0011` adds an instance-level super-admin role table —
//! `instance_admins(user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE
//! CASCADE, created_at TIMESTAMPTZ NOT NULL DEFAULT now())`. It is purely
//! additive (creates one empty table, rewrites no prior row) and idempotent
//! (`CREATE TABLE IF NOT EXISTS`), so re-applying the full migration set is a
//! no-op for it.
//!
//! These tests run the REAL migration runner (`run_migrations_from_dir`, the
//! same advisory-lock path production boot uses) against a real Postgres
//! (testcontainers, @real-io) — the schema shape and idempotence cannot be
//! faked. They pin the step-01-01 contract that underpins the slice-05
//! "re-running the upgrade does not duplicate or alter anything" acceptance
//! scenario.

use foundry_store::run_migrations_from_dir;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Row};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ImageExt;

/// Resolve the absolute path to this crate's canonical `migrations/` dir.
fn production_migrations_dir() -> PathBuf {
    let manifest =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is always set by cargo");
    PathBuf::from(manifest).join("migrations")
}

async fn fresh_pool() -> (
    PgPool,
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
    let opts = PgConnectOptions::from_str(&base).expect("parse base url");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(opts)
        .await
        .expect("connect pool");
    (pool, container)
}

async fn instance_admins_exists(pool: &PgPool) -> bool {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables \
         WHERE table_schema = current_schema() AND table_name = 'instance_admins')",
    )
    .fetch_one(pool)
    .await
    .expect("query table existence")
}

/// The single-column primary key of `instance_admins`, as reported by the
/// catalog — must be exactly `user_id`.
async fn instance_admins_pk_columns(pool: &PgPool) -> Vec<String> {
    let rows = sqlx::query(
        "SELECT a.attname AS col \
         FROM pg_index i \
         JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey) \
         WHERE i.indrelid = 'instance_admins'::regclass AND i.indisprimary \
         ORDER BY a.attname",
    )
    .fetch_all(pool)
    .await
    .expect("query primary key columns");
    rows.into_iter()
        .map(|r| r.get::<String, _>("col"))
        .collect()
}

async fn instance_admins_row_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT count(*) FROM instance_admins")
        .fetch_one(pool)
        .await
        .expect("count instance_admins rows")
}

/// Applying the canonical forward-only migration set yields an
/// `instance_admins` table keyed on `user_id`, and it starts empty (AC 1, 5).
#[tokio::test]
async fn migration_set_creates_empty_instance_admins_keyed_on_user_id() {
    let (pool, _guard) = fresh_pool().await;
    let dir: &Path = &production_migrations_dir();

    run_migrations_from_dir(&pool, dir)
        .await
        .expect("apply canonical forward-only migration set");

    assert!(
        instance_admins_exists(&pool).await,
        "0011 must create the instance_admins table"
    );
    assert_eq!(
        instance_admins_pk_columns(&pool).await,
        vec!["user_id".to_string()],
        "instance_admins must be keyed on user_id as its primary key"
    );
    assert_eq!(
        instance_admins_row_count(&pool).await,
        0,
        "the newly created instance_admins table is empty until a super-admin is seeded"
    );
}

/// Re-applying the full migration set a second time neither errors nor
/// re-runs (duplicates) the 0011 migration: the second invocation reports it
/// as already-applied, and the table is unchanged (AC 3, idempotence).
#[tokio::test]
async fn re_applying_migration_set_is_idempotent_for_instance_admins() {
    let (pool, _guard) = fresh_pool().await;
    let dir: &Path = &production_migrations_dir();

    let first = run_migrations_from_dir(&pool, dir)
        .await
        .expect("first apply succeeds");
    assert!(
        first.applied.contains(&11),
        "the first apply must run migration 0011"
    );

    let second = run_migrations_from_dir(&pool, dir)
        .await
        .expect("second apply must not error (IF NOT EXISTS idempotence)");
    assert!(
        !second.applied.contains(&11),
        "the second apply must NOT re-run 0011"
    );
    assert!(
        second.already_applied.contains(&11),
        "the second apply must report 0011 as already-applied"
    );

    assert!(
        instance_admins_exists(&pool).await,
        "instance_admins still exists after a second apply"
    );
    assert_eq!(
        instance_admins_row_count(&pool).await,
        0,
        "re-applying must not duplicate or populate instance_admins"
    );
}
