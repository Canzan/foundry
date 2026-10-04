//! keycloak-sso US-07 (D3b, D9; DESIGN DDD-23..27): foundry knows, permanently,
//! which accounts role provisioning created. Migration 0017 adds
//! `users.provisioned_at` and backfills it onto every password-less account
//! (OD-14); `UserRow.provisioned` reads `provisioned_at IS NOT NULL OR
//! password_hash IS NULL` (DDD-26); nothing ever clears the marker (DDD-27).
//!
//! WHY-NEW-FILE: crates/foundry-store/tests/users_provisioned_at.rs
//!   CLOSEST-EXISTING: crates/foundry-store/tests/nullable_password_hash.rs
//!   EXTENSION-COST: that file pins 0016 (an account MAY lack a password) and the
//!     NULL-hash decode; this pins 0017 (provenance), its backfill and the
//!     provisioned read — a different migration with a different observable.
//!   PARALLEL-RATIONALE: "a NULL hash is a first-class account state" and
//!     "provenance survives every password write" are unrelated properties.
//!
//! Real Postgres (testcontainers): the backfill, the computed read and the
//! migration runner's per-version application cannot be faked. Integration-level,
//! example-based (Mandate 9/11).

use foundry_store::{run_migrations, run_migrations_from_dir, ResetOutcome, Store};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::path::PathBuf;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ImageExt;

const MIGRATION_0017: &str = "0017_users_provisioned_at.sql";

fn production_migrations_dir() -> PathBuf {
    let manifest =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is always set by cargo");
    PathBuf::from(manifest).join("migrations")
}

async fn empty_pool() -> (
    PgPool,
    testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
) {
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

/// A private copy of the production migrations up to and including version
/// `through` (the four-digit file prefix), so a schema can be stood up at a past
/// version and then moved forward by the real runner.
fn staged_migrations(through: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("foundry-0017-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).expect("stage dir");
    let mut names: Vec<String> = std::fs::read_dir(production_migrations_dir())
        .expect("read migrations")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".sql") && n.get(..4).is_some_and(|v| v <= through))
        .collect();
    names.sort();
    for n in names {
        std::fs::copy(production_migrations_dir().join(&n), dir.join(&n)).expect("stage file");
    }
    dir
}

async fn insert_user(
    pool: &PgPool,
    email: &str,
    password_hash: Option<&str>,
    created_at: time::OffsetDateTime,
) -> uuid::Uuid {
    let user_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash, created_at)
              VALUES ($1, $2, $2, 'Someone', $3, $4)",
    )
    .bind(user_id)
    .bind(email)
    .bind(password_hash)
    .bind(created_at)
    .execute(pool)
    .await
    .expect("insert a user as a pre-0017 binary does: no provisioned_at named");
    user_id
}

/// (provisioned_at, created_at) of one account, read through SQL.
async fn marker(
    pool: &PgPool,
    id: uuid::Uuid,
) -> (Option<time::OffsetDateTime>, time::OffsetDateTime) {
    sqlx::query_as("SELECT provisioned_at, created_at FROM users WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("read provisioned_at (0017 adds the column)")
}

/// OQ-6 / DoD 4 / OD-14 (DDD-24): applying 0017 onto a 0016 schema marks every
/// password-less account as provisioned at the moment it was created, and only
/// those — an account with a password (bootstrap, invite, test user) stays
/// unmarked.
#[tokio::test]
async fn the_upgrade_marks_exactly_the_password_less_accounts_as_provisioned_when_they_were_created(
) {
    assert!(
        production_migrations_dir().join(MIGRATION_0017).exists(),
        "migration {MIGRATION_0017} (DDD-24) is missing"
    );
    let (pool, _pg) = empty_pool().await;
    let at_0016 = staged_migrations("0016");
    run_migrations_from_dir(&pool, &at_0016)
        .await
        .expect("stand the schema up at 0016");

    let early = time::macros::datetime!(2026-02-01 09:00 UTC);
    let later = time::macros::datetime!(2026-03-15 17:30 UTC);
    let provisioned_early = insert_user(&pool, "nia@example.test", None, early).await;
    let provisioned_later = insert_user(&pool, "noor@example.test", None, later).await;
    let invited = insert_user(&pool, "pat@example.test", Some("phc$pat"), early).await;
    let reset_after_provisioning =
        insert_user(&pool, "rae@example.test", Some("phc$rae-chose"), later).await;

    let at_0017 = staged_migrations("0017");
    run_migrations_from_dir(&pool, &at_0017)
        .await
        .expect("apply 0017 onto the 0016 schema");

    assert_eq!(marker(&pool, provisioned_early).await, (Some(early), early));
    assert_eq!(marker(&pool, provisioned_later).await, (Some(later), later));
    assert_eq!(
        marker(&pool, invited).await.0,
        None,
        "an account with a password was marked provisioned"
    );
    // OD-14's accepted residue: reset before the upgrade looks invited.
    assert_eq!(marker(&pool, reset_after_provisioning).await.0, None);

    // Re-running the upgrade changes nothing (the runner skips applied versions,
    // and the backfill's `provisioned_at IS NULL` guard keeps it idempotent).
    run_migrations_from_dir(&pool, &at_0017)
        .await
        .expect("re-run the upgrade");
    assert_eq!(marker(&pool, provisioned_early).await, (Some(early), early));
    assert_eq!(marker(&pool, invited).await.0, None);

    let _ = std::fs::remove_dir_all(at_0016);
    let _ = std::fs::remove_dir_all(at_0017);
}

/// OQ-7 / DDD-26: the read model calls an account provisioned when it carries the
/// marker OR has no password. The no-password arm covers an account a pre-0017
/// replica provisioned during a rolling deploy (no marker written); the marker
/// arm covers a provisioned account that has since chosen a password; an account
/// with a password and no marker is not provisioned. Read by email and by id.
#[tokio::test]
async fn an_account_is_read_as_provisioned_when_marked_or_password_less() {
    assert!(
        production_migrations_dir().join(MIGRATION_0017).exists(),
        "migration {MIGRATION_0017} (DDD-24) is missing"
    );
    let (pool, _pg) = empty_pool().await;
    run_migrations(&pool).await.expect("run migrations");
    let store = Store::from_pool(pool.clone());
    let now = time::macros::datetime!(2026-10-04 12:00 UTC);

    // The rolling-deploy window: no password, no marker.
    let window = insert_user(&pool, "win@example.test", None, now).await;
    // Marked, then given a password.
    let marked = insert_user(&pool, "mark@example.test", Some("phc$mark"), now).await;
    sqlx::query("UPDATE users SET provisioned_at = $2 WHERE id = $1")
        .bind(marked)
        .bind(now)
        .execute(&pool)
        .await
        .expect("mark the account");
    // An invited account: a password, no marker.
    let invited = insert_user(&pool, "pat@example.test", Some("phc$pat"), now).await;

    for (email, id, expected) in [
        ("win@example.test", window, true),
        ("mark@example.test", marked, true),
        ("pat@example.test", invited, false),
    ] {
        let by_email = store
            .find_user_by_email(email)
            .await
            .expect("lookup by email")
            .expect("found by email");
        let by_id = store
            .find_user_by_id(id)
            .await
            .expect("lookup by id")
            .expect("found by id");
        assert_eq!(by_email.provisioned, expected, "{email} by email");
        assert_eq!(by_id.provisioned, expected, "{email} by id");
    }
}

/// DDD-25 / DDD-27 / D9 (AC-7.5 at the store): the provisioning write sets the
/// marker, and neither password writer — the reset link nor a signed-in or
/// operator password change — clears it.
#[tokio::test]
async fn no_password_write_clears_the_provisioned_marker() {
    assert!(
        production_migrations_dir().join(MIGRATION_0017).exists(),
        "migration {MIGRATION_0017} (DDD-24) is missing"
    );
    let (pool, _pg) = empty_pool().await;
    run_migrations(&pool).await.expect("run migrations");
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Cluster')")
        .bind(uuid::Uuid::now_v7())
        .execute(&pool)
        .await
        .expect("seed the original workspace");
    let store = Store::from_pool(pool.clone());

    let provisioned_at = time::macros::datetime!(2026-10-04 08:15 UTC);
    let user_id = match store
        .provision_federated_member(
            "nia@example.test",
            "nia@example.test",
            "Nia",
            provisioned_at,
        )
        .await
        .expect("provision")
    {
        foundry_store::FederatedProvisionOutcome::Created { user_id, .. } => user_id,
        other => panic!("expected a new account, got {other:?}"),
    };
    let first = marker(&pool, user_id).await.0;
    assert!(first.is_some(), "the provisioning write left no marker");

    let token_hash = vec![9u8; 32];
    let now = time::OffsetDateTime::now_utc();
    store
        .insert_reset_token(
            uuid::Uuid::now_v7(),
            user_id,
            &token_hash,
            now + time::Duration::hours(1),
        )
        .await
        .expect("insert reset token");
    assert_eq!(
        store
            .reset_password_and_consume(&token_hash, "phc$nia-chose", now)
            .await
            .expect("reset"),
        ResetOutcome::Consumed { user_id }
    );
    assert_eq!(
        marker(&pool, user_id).await.0,
        first,
        "a reset cleared the marker"
    );

    store
        .update_user_password(user_id, "phc$nia-changed")
        .await
        .expect("change password");
    assert_eq!(
        marker(&pool, user_id).await.0,
        first,
        "a password change cleared the marker"
    );
    let row = store
        .find_user_by_id(user_id)
        .await
        .expect("lookup")
        .expect("found");
    assert!(
        row.provisioned,
        "a provisioned account with a password reads as not provisioned"
    );
}
