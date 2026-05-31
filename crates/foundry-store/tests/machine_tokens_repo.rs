//! US-W05b (step 02-01) — `machine_tokens` registry/denylist repo tests.
//!
//! Machine tokens are JWTs; the JWT itself is the secret, so this table is a
//! REGISTRY of issuance metadata plus a revocation flag — there is deliberately
//! NO token/hash column (see design/auth.md). Revocation works via a `jti`
//! denylist checked per request: `find_by_jti` returns the row and the caller
//! treats `revoked_at IS NULL` as "active". Revocation is therefore a FLAG, not
//! a delete — a revoked credential's row MUST still be findable so the per-
//! request check can refuse it (US-W05b "A revoked credential is refused on its
//! next use").
//!
//! These run against a real Postgres (testcontainers, @real-io): the schema +
//! the SQL behaviour can't be faked. Each test runs the full production
//! migration set on a fresh container, then exercises the repo through its
//! public surface and asserts the observable row state.

use foundry_store::{run_migrations, Store};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Connection, PgPool};
use std::str::FromStr;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ImageExt;

/// Boot a fresh Postgres container and return its base connection URL plus the
/// running container guard (dropped at end of scope tears the container down).
async fn fresh_postgres() -> (
    String,
    testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
) {
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
    (base, container)
}

/// A connection pool against `base`, used as the production binary would: the
/// default `public` search_path, full migration set already applied.
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

/// Seed the FK targets a machine_token row references (workspace, admin user,
/// team, member user) and return `(workspace_id, user_id, team_id)`.
async fn seed_principal(store: &Store) -> (uuid::Uuid, uuid::Uuid, uuid::Uuid) {
    let workspace_id = uuid::Uuid::now_v7();
    let admin_id = uuid::Uuid::now_v7();
    let user_id = uuid::Uuid::now_v7();
    let team_id = uuid::Uuid::now_v7();
    let project_id = uuid::Uuid::now_v7();
    store
        .create_initial_workspace(
            workspace_id,
            "Acme Eng",
            admin_id,
            "devansh@acme.com",
            "devansh@acme.com",
            "Devansh",
            "phc$dummy",
            team_id,
            "Backend",
            "backend",
            project_id,
            "Auth v2",
            "auth-v2",
            "AUTH",
        )
        .await
        .expect("seed workspace");
    // Bind the credential to a distinct member (Mei) so the row's user_id is a
    // real users(id). Insert her directly via the pool.
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, 'mei@acme.com', 'mei@acme.com', 'Mei', 'phc$dummy')",
    )
    .bind(user_id)
    .execute(store.pool())
    .await
    .expect("seed member");
    (workspace_id, user_id, team_id)
}

#[tokio::test]
async fn insert_then_find_by_jti_round_trips_an_active_token() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let (workspace_id, user_id, team_id) = seed_principal(&store).await;

    let jti = uuid::Uuid::now_v7();
    let expires_at = time::OffsetDateTime::now_utc() + time::Duration::days(30);
    store
        .insert_machine_token(
            jti,
            user_id,
            workspace_id,
            Some(team_id),
            expires_at,
            "Devansh's dashboard",
        )
        .await
        .expect("insert machine token");

    let row = store
        .find_machine_token_by_jti(jti)
        .await
        .expect("query by jti")
        .expect("row exists for issued jti");

    assert_eq!(row.jti, jti, "round-trips the jti");
    assert_eq!(row.user_id, user_id, "round-trips the bound principal");
    assert_eq!(row.workspace_id, workspace_id, "round-trips the workspace");
    assert_eq!(
        row.scope_team_id,
        Some(team_id),
        "round-trips the scope team"
    );
    assert_eq!(row.label, "Devansh's dashboard", "round-trips the label");
    assert!(
        row.revoked_at.is_none(),
        "a freshly-issued credential is active (revoked_at IS NULL)"
    );
}

#[tokio::test]
async fn revoke_flips_revoked_at_but_find_still_returns_the_row() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let (workspace_id, user_id, team_id) = seed_principal(&store).await;

    let jti = uuid::Uuid::now_v7();
    let expires_at = time::OffsetDateTime::now_utc() + time::Duration::days(30);
    store
        .insert_machine_token(
            jti,
            user_id,
            workspace_id,
            Some(team_id),
            expires_at,
            "Devansh's dashboard",
        )
        .await
        .expect("insert machine token");

    store.revoke_machine_token(jti).await.expect("revoke");

    // Denylist semantics: revocation is a FLAG, not a delete. The row MUST
    // still be findable so the per-request check can refuse it.
    let row = store
        .find_machine_token_by_jti(jti)
        .await
        .expect("query by jti after revoke")
        .expect("revoked row is still present (denylist, not delete)");
    assert!(
        row.revoked_at.is_some(),
        "revoke sets revoked_at — the per-request check reads this to refuse"
    );
}

#[tokio::test]
async fn list_returns_issued_credentials_for_the_workspace() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let (workspace_id, user_id, team_id) = seed_principal(&store).await;

    let expires_at = time::OffsetDateTime::now_utc() + time::Duration::days(30);
    let jti_a = uuid::Uuid::now_v7();
    let jti_b = uuid::Uuid::now_v7();
    for (jti, label) in [(jti_a, "dashboard"), (jti_b, "ci-runner")] {
        store
            .insert_machine_token(jti, user_id, workspace_id, Some(team_id), expires_at, label)
            .await
            .expect("insert machine token");
    }

    let rows = store
        .list_machine_tokens(workspace_id)
        .await
        .expect("list machine tokens");
    let jtis: std::collections::HashSet<uuid::Uuid> = rows.iter().map(|r| r.jti).collect();
    assert!(
        jtis.contains(&jti_a),
        "list includes the first issued token"
    );
    assert!(
        jtis.contains(&jti_b),
        "list includes the second issued token"
    );
}

#[tokio::test]
async fn touch_last_used_records_use_on_an_active_token() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let (workspace_id, user_id, team_id) = seed_principal(&store).await;

    let jti = uuid::Uuid::now_v7();
    let expires_at = time::OffsetDateTime::now_utc() + time::Duration::days(30);
    store
        .insert_machine_token(
            jti,
            user_id,
            workspace_id,
            Some(team_id),
            expires_at,
            "dash",
        )
        .await
        .expect("insert machine token");

    let before = store
        .find_machine_token_by_jti(jti)
        .await
        .expect("query")
        .expect("row");
    assert!(
        before.last_used_at.is_none(),
        "a never-used credential has no last_used_at"
    );

    store
        .touch_machine_token_last_used(jti)
        .await
        .expect("touch last_used");

    let after = store
        .find_machine_token_by_jti(jti)
        .await
        .expect("query")
        .expect("row");
    assert!(
        after.last_used_at.is_some(),
        "touch records the last_used_at timestamp"
    );
}

/// The startup probe must refuse to boot against a pre-0007 schema — the
/// machine_tokens table (and its denylist columns) is the substrate US-W05b
/// auth reads on every request. Booting without it would surface only on the
/// first authenticated API call; the probe fails fast instead (Earned Trust).
#[tokio::test]
async fn probe_fails_on_a_pre_0007_schema() {
    let (base, _guard) = fresh_postgres().await;

    // Build the schema up to migration 0006 ONLY by replaying the prior
    // migrations' essential substrate, then create a `comments` table with the
    // 0006 columns so the OLD probe assertions still pass — leaving the
    // machine_tokens table ABSENT. The probe's NEW 0007 assertion must fire.
    let mut admin = sqlx::PgConnection::connect(&base)
        .await
        .expect("admin connect");
    for stmt in [
        // Minimal: the probe only inspects information_schema for the
        // comments-0006 columns and (after this step) the machine_tokens
        // columns. A comments table WITH the 0006 columns lets the legacy
        // probe assertion pass; the missing machine_tokens table is what the
        // new assertion catches.
        "CREATE TABLE comments (id uuid PRIMARY KEY, updated_at timestamptz, \
         deleted_at timestamptz, deleted_by uuid)",
    ] {
        sqlx::query(stmt)
            .execute(&mut admin)
            .await
            .unwrap_or_else(|e| panic!("setup `{stmt}`: {e}"));
    }
    drop(admin);

    let store = Store::from_pool(pool_on_public(&base).await);
    let err = store
        .probe()
        .await
        .expect_err("probe must FAIL when the machine_tokens table is absent (pre-0007 schema)");
    let msg = err.to_string();
    assert!(
        msg.contains("machine_tokens") || msg.contains("0007"),
        "expected a missing-machine_tokens probe error, got: {msg}"
    );
}

/// A pool on the default `public` schema (no search_path override).
async fn pool_on_public(base: &str) -> PgPool {
    let opts = PgConnectOptions::from_str(base).expect("parse base url");
    PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(opts)
        .await
        .expect("connect public pool")
}
