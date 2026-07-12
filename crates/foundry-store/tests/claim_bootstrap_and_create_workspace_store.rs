//! bootstrap-claim-enumeration-oracle (D1 / NFR-3) — the focused store-seam
//! contract for the NEW atomic `Store::claim_bootstrap_and_create_workspace`.
//! Pins its four load-bearing behaviours directly at the store seam:
//!   1. CONSUMED (happy) — one tx consumes the bootstrap token AND seeds the whole
//!      workspace + the first `instance_admins` row; the token's `used_at` is set.
//!   2. EMAIL COLLISION — when the email already maps to a user, the
//!      `users.email_lower` UNIQUE violation (SQLSTATE 23505) rolls the whole tx
//!      back → the named `EmailCollision` outcome (NOT a generic StoreError/500),
//!      and the bootstrap token stays UNCONSUMED (`used_at IS NULL`).
//!   3. REFUSED — an unknown token yields `Refused` (the guarded-UPDATE saw 0 rows),
//!      seeding nothing.
//!   4. NARROW CATCH (NFR-3, the mutation gate) — a NON-23505 error on the SAME
//!      users INSERT (a CHECK violation, SQLSTATE 23514) propagates as `Err(StoreError)`,
//!      NOT `EmailCollision`. A mutant that broadens the 23505 catch to any
//!      DatabaseError mis-maps this to `EmailCollision` and dies here.
//!
//! WHY-NEW-FILE: crates/foundry-store/tests/claim_bootstrap_and_create_workspace_store.rs
//!   CLOSEST-EXISTING: crates/foundry-store/tests/create_member_and_consume_store.rs
//!   EXTENSION-COST: that file pins the member-accept tx (invite consume + create
//!     member + member membership); this pins a DIFFERENT tx (bootstrap-token consume
//!     + the full workspace/instance-admin seed) with a different fixture lifecycle
//!     (mint a bootstrap token, not seed a workspace+invite) and a different narrow-catch
//!     surface (the non-23505 → StoreError arm the mutation gate targets).
//!   PARALLEL-RATIONALE: `claim_bootstrap_and_create_workspace` seeds a FRESH instance
//!     (workspace 1 + first super-admin) guarded on a `bootstrap_tokens` row, whereas
//!     create_member_and_consume operates on an EXISTING workspace's invite — different
//!     preconditions (no prior workspace vs a seeded one) and different observable rows.
//!
//! Runs against a real Postgres (testcontainers, @real-io): the atomic multi-row tx,
//! the guarded-UPDATE single-use semantics, and the UNIQUE/CHECK rollback arms cannot
//! be faked. Integration-level (adapter ↔ real DB), example-based wiring verification —
//! NOT PBT (the contract is "the seed commits together" / "the collision rolls back to
//! a named outcome, token unconsumed", not "all input shapes").

use foundry_store::{run_migrations, BootstrapClaimOutcome, Store};
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
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(10))
        .connect(base)
        .await
        .expect("connect pool");
    run_migrations(&pool).await.expect("run migrations");
    Store::from_pool(pool)
}

/// An opaque 32-byte token hash. The store treats `token_hash` as raw bytes (real
/// SHA-256 hashing is the HANDLER's concern); at the store seam mint and claim only
/// need to agree on the same bytes, so a distinct-per-`seed` array suffices.
fn token_hash(seed: u8) -> [u8; 32] {
    [seed; 32]
}

/// Mint a live bootstrap token (30-minute TTL from `now`) for the given hash bytes.
async fn mint_token(store: &Store, hash: &[u8], now: time::OffsetDateTime) {
    store
        .insert_bootstrap_token(
            uuid::Uuid::now_v7(),
            hash,
            now + time::Duration::minutes(30),
        )
        .await
        .expect("mint bootstrap token");
}

/// Drive the atomic claim+create with a fixed seed shape, varying only the token +
/// the identity fields the behaviours pivot on.
#[allow(clippy::too_many_arguments)]
async fn claim(
    store: &Store,
    token_hash: &[u8],
    now: time::OffsetDateTime,
    email: &str,
    display_name: &str,
    workspace_name: &str,
) -> Result<BootstrapClaimOutcome, foundry_store::StoreError> {
    store
        .claim_bootstrap_and_create_workspace(
            token_hash,
            now,
            uuid::Uuid::now_v7(),
            workspace_name,
            uuid::Uuid::now_v7(),
            &email.to_ascii_lowercase(),
            email,
            display_name,
            "phc$dummy",
            uuid::Uuid::now_v7(),
            "General",
            "general",
            uuid::Uuid::now_v7(),
            "Sandbox",
            "sandbox",
            "GEN",
        )
        .await
}

/// Behaviour 1 — CONSUMED: a live token + a fresh email seeds the whole workspace
/// (workspace row + first `instance_admins` row) AND marks the token consumed, all in
/// one committed tx. Pinning the seeded rows + the consumed token kills the no-op
/// mutant (skip the seed) and the never-consume mutant (guard omitted).
#[tokio::test]
async fn claim_consumes_token_and_seeds_workspace_and_first_super_admin() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let now = time::OffsetDateTime::now_utc();
    let hash = token_hash(1);
    mint_token(&store, &hash, now).await;

    let outcome = claim(
        &store,
        hash.as_slice(),
        now,
        "ops@acme.com",
        "Ops",
        "Acme Eng",
    )
    .await
    .expect("a live token + fresh email must not error");

    let BootstrapClaimOutcome::Consumed {
        workspace_id,
        user_id,
    } = outcome
    else {
        panic!("a live token + fresh email must yield Consumed; got {outcome:?}");
    };

    // The seeded workspace exists, keyed on the returned id.
    let (ws_name,): (String,) = sqlx::query_as("SELECT name FROM workspaces WHERE id = $1")
        .bind(workspace_id)
        .fetch_one(store.pool())
        .await
        .expect("the seeded workspace row");
    assert_eq!(
        ws_name, "Acme Eng",
        "Consumed must seed the named workspace"
    );

    // The claiming operator is the first instance super-admin (seeded in the SAME tx).
    let (admin_rows,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM instance_admins WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(store.pool())
            .await
            .expect("count the seeded first super-admin");
    assert_eq!(
        admin_rows, 1,
        "the claim must seed its operator as the first instance super-admin"
    );

    // The token was consumed (used_at set) exactly once by the same tx.
    let (consumed,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM bootstrap_tokens WHERE token_hash = $1 AND used_at IS NOT NULL",
    )
    .bind(hash.as_slice())
    .fetch_one(store.pool())
    .await
    .expect("count the consumed token");
    assert_eq!(consumed, 1, "Consumed must mark the bootstrap token used");
}

/// Behaviour 2 — EMAIL COLLISION: when the email already maps to a user, the
/// `users.email_lower` 23505 rolls the whole tx back → `EmailCollision` (NEVER a
/// StoreError/500), and the token stays UNCONSUMED (`used_at IS NULL`). Asserting the
/// named outcome PLUS the unconsumed token kills the broaden-catch mutant and the
/// consume-before-create mutant (which would burn the token).
#[tokio::test]
async fn claim_refuses_email_collision_and_leaves_token_unconsumed() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let now = time::OffsetDateTime::now_utc();

    // A prior account already owns the email (the collision precondition).
    let first = token_hash(1);
    mint_token(&store, &first, now).await;
    claim(
        &store,
        first.as_slice(),
        now,
        "ops@acme.com",
        "Ops",
        "Acme Eng",
    )
    .await
    .expect("seed the first account");

    // A second live token claimed with the ALREADY-registered email.
    let second = token_hash(2);
    mint_token(&store, &second, now).await;
    let outcome = claim(
        &store,
        second.as_slice(),
        now,
        "ops@acme.com",
        "Ops Two",
        "Collision WS",
    )
    .await
    .expect("an email collision must be a NAMED outcome, never a bubbled StoreError/500");
    assert_eq!(
        outcome,
        BootstrapClaimOutcome::EmailCollision,
        "a colliding email must yield EmailCollision (NOT Consumed, NOT a 500)"
    );

    // The whole tx rolled back: the SECOND token stays UNCONSUMED (reusable).
    let (unconsumed,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM bootstrap_tokens WHERE token_hash = $1 AND used_at IS NULL",
    )
    .bind(second.as_slice())
    .fetch_one(store.pool())
    .await
    .expect("count the still-unconsumed token");
    assert_eq!(
        unconsumed, 1,
        "the collision must roll back the tx — the token stays unconsumed"
    );
    // No second workspace was seeded (the rollback undid any partial seed).
    let (ws_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM workspaces WHERE name = $1")
        .bind("Collision WS")
        .fetch_one(store.pool())
        .await
        .expect("count the colliding workspace");
    assert_eq!(
        ws_count, 0,
        "the collision must leave no partial workspace seed"
    );
}

/// Behaviour 3 — REFUSED: an unknown token (never minted) yields `Refused` from the
/// 0-row guarded UPDATE and seeds nothing. Kills the "guard never fires" mutant.
#[tokio::test]
async fn claim_refuses_unknown_token_without_seeding() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let now = time::OffsetDateTime::now_utc();
    let unknown = token_hash(9); // never minted

    let outcome = claim(
        &store,
        unknown.as_slice(),
        now,
        "nobody@acme.com",
        "Nobody",
        "Ghost WS",
    )
    .await
    .expect("an unknown token must be a NAMED outcome, never an error");
    assert_eq!(
        outcome,
        BootstrapClaimOutcome::Refused,
        "an unknown token must yield Refused"
    );

    let (ws_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM workspaces")
        .fetch_one(store.pool())
        .await
        .expect("count all workspaces");
    assert_eq!(ws_count, 0, "a refused claim must seed no workspace");
}

/// Behaviour 4 — NARROW CATCH (NFR-3, the store-scope mutation gate): a NON-23505
/// error on the SAME users INSERT must propagate as `Err(StoreError)`, NOT map to a
/// refusal. An empty `display_name` violates the users CHECK
/// (`length(display_name) BETWEEN 1 AND 64`) → SQLSTATE 23514 (check_violation), a
/// DIFFERENT code from the 23505 email collision. A mutant that broadens the catch to
/// any `sqlx::Error::Database` would mis-map this to `EmailCollision`; asserting `Err`
/// here kills it.
#[tokio::test]
async fn claim_propagates_non_23505_error_as_store_error() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let now = time::OffsetDateTime::now_utc();
    let hash = token_hash(1);
    mint_token(&store, &hash, now).await;

    // Fresh email (so NOT a 23505) but an empty display_name → the users CHECK fires
    // (23514) on the users INSERT itself.
    let result = claim(&store, hash.as_slice(), now, "ops@acme.com", "", "Acme Eng").await;

    match result {
        Err(_) => {}
        Ok(other) => panic!(
            "a non-23505 error on the users INSERT must propagate as Err(StoreError), \
             never a named refusal; got Ok({other:?})"
        ),
    }
}
