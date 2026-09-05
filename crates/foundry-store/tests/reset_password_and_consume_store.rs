//! The store-seam contract for `Store::reset_password_and_consume` — the
//! authoritative single-use guard behind `/reset-password`.
//!
//! WHY-NEW-FILE: crates/foundry-store/tests/reset_password_and_consume_store.rs
//!   CLOSEST-EXISTING: crates/foundry-store/tests/create_member_and_consume_store.rs
//!   EXTENSION-COST: that file pins the member-accept tx (consume an invite +
//!     CREATE a user + add a membership, with a UNIQUE-collision rollback arm).
//!     This is a different transaction over a different table with a different
//!     precondition (a live `reset_tokens` row for an EXISTING user) and a
//!     different observable surface (the rewritten credential + every sibling
//!     token burned). Folding them together would entangle two unrelated
//!     transactions whose only shared idea is "guarded UPDATE".
//!   PARALLEL-RATIONALE: the properties here are expiry, single-use under a
//!     RACE, and sibling invalidation — none of which the invite tx has.
//!
//! Runs against a real Postgres (testcontainers, @real-io) because every
//! property under test is a property of the DATABASE: a guarded UPDATE's
//! 0-row/1-row semantics, and two concurrent transactions racing one row.
//! A fake store would assert only that the fake behaves like the fake.

use foundry_store::{run_migrations, ResetOutcome, Store};
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
        .with_tag("16-alpine")
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
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(10))
        .connect(base)
        .await
        .expect("connect pool");
    run_migrations(&pool).await.expect("run migrations");
    Store::from_pool(pool)
}

async fn seed_user(store: &Store, email: &str) -> uuid::Uuid {
    let user_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, 'Mei', 'phc$original')",
    )
    .bind(user_id)
    .bind(email)
    .execute(store.pool())
    .await
    .expect("seed user");
    user_id
}

async fn stored_hash(store: &Store, user_id: uuid::Uuid) -> String {
    let (hash,): (String,) = sqlx::query_as("SELECT password_hash FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(store.pool())
        .await
        .expect("read password hash");
    hash
}

fn hash_of(token: &str) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    h.finalize().to_vec()
}

#[tokio::test]
async fn consuming_a_live_token_rewrites_the_credential_and_burns_every_sibling() {
    let (base, _c) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let now = time::OffsetDateTime::now_utc();
    let user_id = seed_user(&store, "mei@northwind.example").await;

    // TWO outstanding requests for the same user — the shape you get when
    // someone clicks "forgot password" twice.
    store
        .insert_reset_token(
            uuid::Uuid::now_v7(),
            user_id,
            &hash_of("older-token"),
            now + time::Duration::hours(1),
        )
        .await
        .expect("insert the older token");
    store
        .insert_reset_token(
            uuid::Uuid::now_v7(),
            user_id,
            &hash_of("newer-token"),
            now + time::Duration::hours(1),
        )
        .await
        .expect("insert the newer token");

    let outcome = store
        .reset_password_and_consume(&hash_of("newer-token"), "phc$rotated", now)
        .await
        .expect("the consume runs");
    assert_eq!(outcome, ResetOutcome::Consumed { user_id });
    assert_eq!(stored_hash(&store, user_id).await, "phc$rotated");

    // The OTHER outstanding link must be dead too. Leaving it live would mean
    // whoever triggered the earlier reset still holds a working link after the
    // legitimate owner has recovered the account.
    let sibling = store
        .reset_password_and_consume(&hash_of("older-token"), "phc$attacker", now)
        .await
        .expect("the sibling consume runs");
    assert_eq!(
        sibling,
        ResetOutcome::Refused,
        "a sibling token must not survive a completed reset"
    );
    assert_eq!(
        stored_hash(&store, user_id).await,
        "phc$rotated",
        "the refused sibling must not have written a credential"
    );
}

#[tokio::test]
async fn a_token_is_single_use_and_an_expired_or_unknown_one_is_refused() {
    let (base, _c) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let now = time::OffsetDateTime::now_utc();
    let user_id = seed_user(&store, "mei@northwind.example").await;

    store
        .insert_reset_token(
            uuid::Uuid::now_v7(),
            user_id,
            &hash_of("live"),
            now + time::Duration::hours(1),
        )
        .await
        .expect("insert a live token");
    // Already past its hour when it is offered.
    store
        .insert_reset_token(
            uuid::Uuid::now_v7(),
            user_id,
            &hash_of("stale"),
            now - time::Duration::minutes(1),
        )
        .await
        .expect("insert an expired token");

    assert_eq!(
        store
            .reset_password_and_consume(&hash_of("stale"), "phc$expired", now)
            .await
            .expect("the expired consume runs"),
        ResetOutcome::Refused,
        "an expired token must be refused"
    );
    assert_eq!(
        store
            .reset_password_and_consume(&hash_of("never-issued"), "phc$unknown", now)
            .await
            .expect("the unknown consume runs"),
        ResetOutcome::Refused,
        "a token that was never issued must be refused"
    );
    assert_eq!(
        stored_hash(&store, user_id).await,
        "phc$original",
        "neither refusal may touch the credential"
    );

    // First use wins; the second is refused even though nothing else changed.
    assert_eq!(
        store
            .reset_password_and_consume(&hash_of("live"), "phc$first", now)
            .await
            .expect("the first consume runs"),
        ResetOutcome::Consumed { user_id }
    );
    assert_eq!(
        store
            .reset_password_and_consume(&hash_of("live"), "phc$second", now)
            .await
            .expect("the replay runs"),
        ResetOutcome::Refused,
        "replaying a consumed token must be refused"
    );
    assert_eq!(
        stored_hash(&store, user_id).await,
        "phc$first",
        "the replay must not overwrite the credential set by the first use"
    );
}

#[tokio::test]
async fn two_requests_racing_one_token_produce_exactly_one_consume() {
    // The reason the guard lives in the UPDATE's WHERE clause rather than in a
    // read-then-write: check-then-update lets both callers pass the check. This
    // is the test that would fail if anyone "simplified" it back.
    let (base, _c) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let now = time::OffsetDateTime::now_utc();
    let user_id = seed_user(&store, "mei@northwind.example").await;
    store
        .insert_reset_token(
            uuid::Uuid::now_v7(),
            user_id,
            &hash_of("contended"),
            now + time::Duration::hours(1),
        )
        .await
        .expect("insert the contended token");

    let contended = hash_of("contended");
    let (left, right) = tokio::join!(
        store.reset_password_and_consume(&contended, "phc$left", now),
        store.reset_password_and_consume(&contended, "phc$right", now),
    );
    let outcomes = [
        left.expect("the left consume runs"),
        right.expect("the right consume runs"),
    ];
    let consumed = outcomes
        .iter()
        .filter(|o| matches!(o, ResetOutcome::Consumed { .. }))
        .count();
    assert_eq!(
        consumed, 1,
        "exactly one of two racing consumes may succeed, got {outcomes:?}"
    );
}
