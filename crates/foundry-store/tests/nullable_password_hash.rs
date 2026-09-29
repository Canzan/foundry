//! keycloak-sso step 01-01 (DDD-13, DDD-21): an account may exist without a
//! password. Migration 0016 drops `NOT NULL` from `users.password_hash`; the
//! store's user reads surface that as a password-less account (`None`), never as
//! a decode error, and the reset UPDATE fills the NULL like any other hash.
//!
//! WHY-NEW-FILE: crates/foundry-store/tests/nullable_password_hash.rs
//!   CLOSEST-EXISTING: crates/foundry-store/tests/reset_password_and_consume_store.rs
//!   EXTENSION-COST: that file pins the single-use reset transaction against
//!     users that all HAVE a hash; this pins a schema change (0016) and the
//!     read seams' handling of a NULL hash, of which reset is one consumer.
//!   PARALLEL-RATIONALE: the property here is "NULL hash is a first-class
//!     account state"; reset's properties (expiry, race, siblings) are unrelated.
//!
//! Real Postgres (testcontainers): nullability, UNIQUE and NULL decoding are
//! properties of the database and the driver, not of a fake.

use foundry_store::{run_migrations, ResetOutcome, Store};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ImageExt;

async fn migrated_store() -> (
    Store,
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
    run_migrations(&pool).await.expect("run migrations");
    (Store::from_pool(pool), container)
}

async fn insert_user(
    store: &Store,
    email: &str,
    password_hash: Option<&str>,
) -> Result<uuid::Uuid, sqlx::Error> {
    let user_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, 'Mei', $3)",
    )
    .bind(user_id)
    .bind(email)
    .bind(password_hash)
    .execute(store.pool())
    .await?;
    Ok(user_id)
}

#[tokio::test]
async fn an_account_without_a_password_is_found_and_can_be_given_one_through_reset() {
    let (store, _pg) = migrated_store().await;

    // An account with a password and an account without one live side by side.
    let hashed_id = insert_user(&store, "ana@example.com", Some("phc$ana"))
        .await
        .expect("an account with a password is created as before");
    let federated_id = insert_user(&store, "mei@example.com", None)
        .await
        .expect("an account without a password can exist (0016)");

    // Found by email: the password-less account is an account, not a decode error.
    let federated = store
        .find_user_by_email("mei@example.com")
        .await
        .expect("a NULL hash decodes as a password-less account")
        .expect("the password-less account is found by email");
    assert_eq!(federated.id, federated_id);
    assert_eq!(federated.password_hash, None);

    // Found by id, likewise.
    let by_id = store
        .find_user_by_id(federated_id)
        .await
        .expect("a NULL hash decodes by id")
        .expect("the password-less account is found by id");
    assert_eq!(by_id.id, federated_id);
    assert_eq!(by_id.password_hash, None);

    // The existing account keeps its hash untouched.
    let hashed = store
        .find_user_by_email("ana@example.com")
        .await
        .expect("lookup")
        .expect("found");
    assert_eq!(hashed.id, hashed_id);
    assert_eq!(hashed.password_hash.as_deref(), Some("phc$ana"));

    // One account per address still holds, with or without a password.
    let duplicate = insert_user(&store, "mei@example.com", None).await;
    let is_unique_violation = matches!(
        &duplicate,
        Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some("23505")
    );
    assert!(
        is_unique_violation,
        "email_lower UNIQUE must still refuse a second account: {duplicate:?}"
    );

    // Reset fills the NULL: the account now has exactly the new password hash.
    let token_hash = vec![7u8; 32];
    let now = time::OffsetDateTime::now_utc();
    store
        .insert_reset_token(
            uuid::Uuid::now_v7(),
            federated_id,
            &token_hash,
            now + time::Duration::hours(1),
        )
        .await
        .expect("insert reset token");
    let outcome = store
        .reset_password_and_consume(&token_hash, "phc$mei-new", now)
        .await
        .expect("reset");
    assert_eq!(
        outcome,
        ResetOutcome::Consumed {
            user_id: federated_id
        },
        "reset applies"
    );
    let after = store
        .find_user_by_id(federated_id)
        .await
        .expect("lookup")
        .expect("found");
    assert_eq!(after.password_hash.as_deref(), Some("phc$mei-new"));
}
