//! Regression test for `Store::probe()` scoping its migration-0006 column
//! check to `current_schema()`.
//!
//! The probe asserts the `comments` table carries the slice-5 migration-0006
//! columns (`updated_at`/`deleted_at`/`deleted_by`). Originally the count query
//! filtered only on `table_name = 'comments'`, with NO `table_schema` filter —
//! so it summed matching columns across *every* schema the role can see. A
//! half-migrated ACTIVE schema would then pass the probe whenever any sibling
//! schema (another tenant, or a per-scenario test schema) still carried the
//! columns, masking the substrate lie the probe exists to catch.
//!
//! This test pins the fix: with a sibling schema that HAS the columns and the
//! active schema MISSING them, the probe MUST fail. Remove the
//! `WHERE table_schema = current_schema()` clause and the sibling's columns
//! mask the gap → the probe wrongly passes → this test fails. (That clause is a
//! SQL string literal, so cargo-mutants can't mutate it; this deterministic
//! cross-schema test is its regression guard instead.)

use foundry_store::Store;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Connection, PgPool};
use std::str::FromStr;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ImageExt;

/// A pool whose connections pin `search_path` to `schema`, so `current_schema()`
/// resolves to it — mirroring how the app pins the per-deployment schema.
async fn pool_on_schema(base: &str, schema: &str) -> PgPool {
    let opts = PgConnectOptions::from_str(base)
        .expect("parse base url")
        .options([("search_path", schema)]);
    PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(opts)
        .await
        .expect("connect per-schema pool")
}

#[tokio::test]
async fn probe_column_check_is_scoped_to_current_schema() {
    let container = Postgres::default()
        .with_tag("16-alpine") // match production Postgres (docker-compose / k8s)
        .start()
        .await
        .expect("start postgres container");
    let host = container.get_host().await.expect("container host");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("container port");
    let base = format!("postgres://postgres:postgres@{host}:{port}/postgres");

    // Two schemas: `sibling` HAS the migration-0006 columns; `active` has a
    // `comments` table MISSING them. (We create the minimal tables the probe
    // inspects rather than running full migrations — the probe only reads
    // information_schema for those three column names.)
    let mut admin = sqlx::PgConnection::connect(&base)
        .await
        .expect("admin connect");
    for stmt in [
        "CREATE SCHEMA active",
        "CREATE SCHEMA sibling",
        "CREATE TABLE sibling.comments (id uuid PRIMARY KEY, updated_at timestamptz, \
         deleted_at timestamptz, deleted_by uuid)",
        "CREATE TABLE active.comments (id uuid PRIMARY KEY)",
    ] {
        sqlx::query(stmt)
            .execute(&mut admin)
            .await
            .unwrap_or_else(|e| panic!("setup `{stmt}`: {e}"));
    }
    drop(admin);

    // ACTIVE schema lacks the 0006 columns → probe MUST fail, even though the
    // `sibling` schema has them. Unscoped, the sibling's columns make the count
    // reach 3 and the probe wrongly passes — that regression is what this guards.
    let active = Store::from_pool(pool_on_schema(&base, "active").await);
    let err = active.probe().await.expect_err(
        "probe must FAIL when the ACTIVE schema lacks the migration-0006 columns; \
         a sibling schema having them must not mask the gap",
    );
    let msg = err.to_string();
    assert!(
        msg.contains("migration-0006"),
        "expected a missing-0006-columns probe error, got: {msg}"
    );

    // Healthy side: the `sibling` schema HAS all three columns → probe passes.
    // (Confirms the negative case above is the scoping, not a broken setup.)
    let healthy = Store::from_pool(pool_on_schema(&base, "sibling").await);
    healthy
        .probe()
        .await
        .expect("probe must PASS when the active schema has the migration-0006 columns");
}
