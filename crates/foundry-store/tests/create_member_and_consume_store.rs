//! workspace-member-invites — Slice 01: the focused store-seam contract for the
//! NEW account-creating member-accept transaction `Store::create_member_and_consume`
//! (ADR-002). Pins its two load-bearing behaviours directly at the store seam:
//!   1. ATOMIC happy path — consume the invite + CREATE the user + ADD a `member`
//!      membership + set `used_by`, all committed together (one tx).
//!   2. EMAIL COLLISION — when `invitee_email` already maps to an existing user, the
//!      `users.email_lower` UNIQUE violation (SQLSTATE 23505) aborts the tx → the
//!      named `EmailCollision` outcome (NOT a generic StoreError/500), and the invite
//!      stays UNCONSUMED (the whole tx rolled back).
//!
//! WHY-NEW-FILE: crates/foundry-store/tests/create_member_and_consume_store.rs
//!   CLOSEST-EXISTING: crates/foundry-store/tests/provision_workspace_store.rs
//!   EXTENSION-COST: that file pins the additional-workspace PROVISIONING tx
//!     (create workspace + first-admin + admin membership + first-admin invite); the
//!     member-accept tx is a DIFFERENT transaction with a guarded-UPDATE consume +
//!     a UNIQUE-collision rollback arm + a `member`-role membership — folding it in
//!     would entangle two unrelated store transactions with different preconditions
//!     and observable surfaces.
//!   PARALLEL-RATIONALE: `create_member_and_consume` operates on an EXISTING
//!     workspace's pending invite (consume + create-member), and owns the named
//!     `EmailCollision` outcome via a SQLSTATE-23505 catch — a different fixture
//!     lifecycle (seed-workspace-and-invite) and a different observable surface (the
//!     created user + member membership + consumed invite, OR the rolled-back
//!     no-side-effect collision) from the provisioning seed contract.
//!
//! Runs against a real Postgres (testcontainers, @real-io): the atomic multi-row
//! transaction, the guarded-UPDATE single-use semantics, and the UNIQUE-constraint
//! collision rollback cannot be faked. Integration-level (adapter ↔ real DB),
//! example-based wiring verification — NOT PBT (the contract is "the four rows commit
//! together" and "the collision rolls back to a named outcome", not "all input
//! shapes").

use foundry_store::{run_migrations, InviteAcceptView, MemberConsumeOutcome, Store};
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

/// Seed a workspace + an admin (the inviter) + a pending MEMBER invite for
/// `invitee_email`. Returns `(workspace_id, admin_id, invite_id)`.
async fn seed_workspace_with_member_invite(
    store: &Store,
    workspace_name: &str,
    invitee_email: &str,
    expires_at: time::OffsetDateTime,
) -> (uuid::Uuid, uuid::Uuid, uuid::Uuid) {
    let workspace_id = uuid::Uuid::now_v7();
    let admin_id = uuid::Uuid::now_v7();
    let invite_id = uuid::Uuid::now_v7();

    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(workspace_id)
        .bind(workspace_name)
        .execute(store.pool())
        .await
        .expect("seed workspace");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, 'admin@northwind.example', 'admin@northwind.example', 'Admin', 'phc$dummy')",
    )
    .bind(admin_id)
    .execute(store.pool())
    .await
    .expect("seed inviting admin");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'admin')",
    )
    .bind(workspace_id)
    .bind(admin_id)
    .execute(store.pool())
    .await
    .expect("seed admin membership");
    // The member invite: created_by = the admin (≠ invitee — the member discriminator).
    store
        .insert_invite(
            invite_id,
            workspace_id,
            Some(invitee_email),
            admin_id,
            expires_at,
        )
        .await
        .expect("seed the pending member invite");

    (workspace_id, admin_id, invite_id)
}

/// Behaviour 1 — the ATOMIC happy path: `create_member_and_consume` consumes the
/// live invite AND creates the invitee's user AND adds a `member`-role membership AND
/// sets `used_by` to the new user, all in ONE committed tx.
///
/// Asserting the `Consumed { workspace_id, user_id }` outcome PLUS all four DB rows
/// (the new user keyed on the invitee email, the `member` membership on the inviting
/// workspace, and the invite consumed with `used_by = new user`) kills the no-op
/// mutant (skip the tx → no user/membership), the wrong-role mutant (`admin` instead
/// of `member`), and the unconsumed mutant (the guard never fires).
#[tokio::test]
async fn create_member_and_consume_atomically_creates_user_membership_and_consumes() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let now = time::OffsetDateTime::now_utc();
    let invitee = "sam.okafor@northwind.example";
    let (workspace_id, _admin_id, invite_id) = seed_workspace_with_member_invite(
        &store,
        "Northwind",
        invitee,
        now + time::Duration::days(7),
    )
    .await;

    let outcome = store
        .create_member_and_consume(invite_id, "phc$member-hash", now)
        .await
        .expect("create_member_and_consume must not error on a live member invite");

    let MemberConsumeOutcome::Consumed {
        workspace_id: consumed_ws,
        user_id: new_user_id,
    } = outcome
    else {
        panic!("a live member invite must yield Consumed; got {outcome:?}");
    };
    assert_eq!(
        consumed_ws, workspace_id,
        "Consumed must carry the invite's workspace id"
    );

    // (1) the new user exists, keyed on the invitee email, with the given hash.
    let (user_email, user_hash): (String, String) =
        sqlx::query_as("SELECT email_lower, password_hash FROM users WHERE id = $1")
            .bind(new_user_id)
            .fetch_one(store.pool())
            .await
            .expect("the created member user row");
    assert_eq!(
        user_email, invitee,
        "the created user's email_lower must be the invitee email"
    );
    assert_eq!(
        user_hash, "phc$member-hash",
        "the created user must carry the supplied argon2id password hash"
    );

    // (2) the membership exists with role 'member' (NOT admin) on the inviting ws.
    let (membership_role,): (String,) = sqlx::query_as(
        "SELECT role FROM workspace_memberships WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(workspace_id)
    .bind(new_user_id)
    .fetch_one(store.pool())
    .await
    .expect("the created member membership row");
    assert_eq!(
        membership_role, "member",
        "the invitee must join as a MEMBER (not admin)"
    );

    // (3) the invite is consumed exactly once, used_by = the new user.
    let (consumed_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NOT NULL AND used_by = $2",
    )
    .bind(invite_id)
    .bind(new_user_id)
    .fetch_one(store.pool())
    .await
    .expect("count the consumed invite row");
    assert_eq!(
        consumed_rows, 1,
        "the invite must be consumed exactly once with used_by = the new member"
    );

    // (4) the created user's `display_name` is the REAL derivation from the invitee
    // email's local-part (`display_name_from_email`, ADR-002): the part before `@`.
    // Pinning the exact derived value here kills the `display_name_from_email →
    // "xyzzy"` mutant (a hard-coded placeholder would make this assert fail).
    let (created_display_name,): (String,) =
        sqlx::query_as("SELECT display_name FROM users WHERE id = $1")
            .bind(new_user_id)
            .fetch_one(store.pool())
            .await
            .expect("the created member user's display_name");
    assert_eq!(
        created_display_name, "sam.okafor",
        "display_name must be derived from the invitee email local-part (NOT a placeholder)"
    );
}

/// Behaviour 2 — the EMAIL-COLLISION arm (OD-1 / A-E9, the HIGH-risk row): when the
/// invitee email already maps to an existing user, the `users.email_lower` UNIQUE
/// violation aborts the tx → the named `EmailCollision` outcome (NEVER a generic
/// StoreError/500), and the whole tx rolls back so the invite stays UNCONSUMED and NO
/// second account is created.
///
/// Asserting `EmailCollision` PLUS the invite still live PLUS exactly one user for
/// the colliding email kills the broaden-the-catch mutant (mapping every error to a
/// refusal), the consume-before-create mutant (which would leave the invite consumed
/// on collision), and the create-anyway mutant (a second account).
#[tokio::test]
async fn create_member_and_consume_refuses_email_collision_without_consuming() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let now = time::OffsetDateTime::now_utc();
    let invitee = "taken@northwind.example";
    let (_workspace_id, _admin_id, invite_id) = seed_workspace_with_member_invite(
        &store,
        "Northwind",
        invitee,
        now + time::Duration::days(7),
    )
    .await;

    // The invitee email ALREADY maps to an existing user (the collision precondition).
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, 'Existing', 'phc$existing')",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(invitee)
    .execute(store.pool())
    .await
    .expect("seed the colliding existing user");
    let users_before: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE email_lower = $1")
        .bind(invitee)
        .fetch_one(store.pool())
        .await
        .expect("count colliding users before");
    assert_eq!(
        users_before, 1,
        "precondition: exactly one user for the email"
    );

    let outcome = store
        .create_member_and_consume(invite_id, "phc$member-hash", now)
        .await
        .expect("an email collision must be a NAMED outcome, never a bubbled StoreError/500");
    assert_eq!(
        outcome,
        MemberConsumeOutcome::EmailCollision,
        "a colliding invitee email must yield EmailCollision (NOT Consumed, NOT a 500)"
    );

    // The whole tx rolled back: the invite stays LIVE (unconsumed) and NO second
    // account was created.
    let (live_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL AND expires_at > $2",
    )
    .bind(invite_id)
    .bind(now)
    .fetch_one(store.pool())
    .await
    .expect("count the still-live invite row");
    assert_eq!(
        live_rows, 1,
        "the collision must roll back the tx — the invite stays live (unconsumed)"
    );
    let users_after: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE email_lower = $1")
        .bind(invitee)
        .fetch_one(store.pool())
        .await
        .expect("count colliding users after");
    assert_eq!(
        users_after, 1,
        "no second account may be created for the colliding email (still exactly one)"
    );
}

/// `invite_accept_view` GET-side liveness read (ADR-001 / D6): for a real seeded
/// invite it must return `Some(view)` projecting the invite's `invitee_email` +
/// `created_by` + `expires_at` joined to its `workspace_name` — NOT `None`.
///
/// Pinning the `Some(view)` projection (the four observable fields the accept page
/// needs) kills the `invite_accept_view → Ok(None)` mutant: a blanket `Ok(None)`
/// would make the `Some(view)` destructure panic.
#[tokio::test]
async fn invite_accept_view_returns_the_joined_projection_for_a_live_invite() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let now = time::OffsetDateTime::now_utc();
    let expires_at = now + time::Duration::days(7);
    let invitee = "sam.okafor@northwind.example";
    let (workspace_id, admin_id, invite_id) =
        seed_workspace_with_member_invite(&store, "Northwind", invitee, expires_at).await;
    // `seed_workspace_with_member_invite` sets created_by = the inviting admin.
    let _ = workspace_id;

    let view: Option<InviteAcceptView> = store
        .invite_accept_view(invite_id)
        .await
        .expect("invite_accept_view must not error for a seeded invite");

    let Some(view) = view else {
        panic!("a seeded live invite must yield Some(view), not None");
    };
    assert_eq!(
        view.invitee_email.as_deref(),
        Some(invitee),
        "the view must project the invite's invitee_email"
    );
    assert_eq!(
        view.created_by,
        Some(admin_id),
        "the view must project the invite's created_by (the inviting admin)"
    );
    assert_eq!(
        view.workspace_name, "Northwind",
        "the view must join in the invite's workspace name"
    );
    assert_eq!(
        view.expires_at, expires_at,
        "the view must project the invite's expires_at"
    );
    assert!(
        view.used_at.is_none(),
        "a freshly-seeded invite is unconsumed (used_at is NULL)"
    );
}

/// `is_workspace_admin` (ADR-007 admin-moderation authz): true for a user holding the
/// `admin` role in the workspace, FALSE for a `member`-role user in the same workspace.
///
/// The FALSE case is the load-bearing assertion — it kills the `is_workspace_admin →
/// Ok(true)` mutant (a blanket `true` would authorize a non-admin member).
#[tokio::test]
async fn is_workspace_admin_is_true_for_admin_and_false_for_a_member() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let now = time::OffsetDateTime::now_utc();
    let invitee = "sam.okafor@northwind.example";
    // Seeds a workspace + an `admin`-role user (the inviter) + a pending member invite.
    let (workspace_id, admin_id, invite_id) = seed_workspace_with_member_invite(
        &store,
        "Northwind",
        invitee,
        now + time::Duration::days(7),
    )
    .await;

    // Consume the invite → creates the invitee user with a `member`-role membership.
    let outcome = store
        .create_member_and_consume(invite_id, "phc$member-hash", now)
        .await
        .expect("consume the live member invite");
    let MemberConsumeOutcome::Consumed {
        user_id: member_id, ..
    } = outcome
    else {
        panic!("a live member invite must yield Consumed; got {outcome:?}");
    };

    let admin_is_admin = store
        .is_workspace_admin(workspace_id, admin_id)
        .await
        .expect("is_workspace_admin must not error for the admin");
    assert!(
        admin_is_admin,
        "the admin-role user must be reported as a workspace admin"
    );

    let member_is_admin = store
        .is_workspace_admin(workspace_id, member_id)
        .await
        .expect("is_workspace_admin must not error for the member");
    assert!(
        !member_is_admin,
        "a member-role user must NOT be reported as a workspace admin (kills the always-true mutant)"
    );
}
