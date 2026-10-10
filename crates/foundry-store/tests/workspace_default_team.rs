//! team-membership — the default team for new members at the store and in the
//! database (DISCUSS OQ-4 to OQ-8, D17, INV-7; DESIGN DDD-12, DDD-13, DDD-14;
//! ADR-TEAM-MEMBERSHIP-002). DISTILL 2026-10-09.
//!
//! - INV-7 is a schema fact after migration 0022: a workspace's default team is
//!   one of its own teams or none (the composite foreign key), and deleting the
//!   team clears only the default.
//! - Placement (DDD-13) happens inside the newcomer's own transaction, as
//!   `member`, on invite accept (`create_member_and_consume`) and on a first
//!   provisioning (`provision_federated_member`'s `Created` branch) — never on a
//!   link (`Existing`). It is best-effort: a placement that fails rolls back its
//!   savepoint and the account and workspace membership still commit (D17).
//!
//! RED scaffolds: `default_team` / `set_default_team` panic until DELIVER 05-01;
//! every test that needs them or 0022 is `#[ignore = "pending: team-membership
//! NN-NN"]` and passes `require_default_column` first (a readable RED gate). The
//! two guards that pin today's behaviour with no default (D17's floor) are GREEN
//! now and not ignored.
//!
//! Real Postgres (testcontainers `postgres:16-alpine`); integration level,
//! example-based (Mandates 9/11).

use foundry_store::{run_migrations, FederatedProvisionOutcome, MemberConsumeOutcome, Store};
use sqlx::postgres::{PgDatabaseError, PgPoolOptions};
use sqlx::PgPool;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};

const DEFAULT_FK: &str = "workspaces_default_team_same_workspace";

fn ts(raw: &str) -> time::OffsetDateTime {
    time::OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339)
        .expect("timestamp parses")
}

async fn migrated() -> (Store, ContainerAsync<Postgres>) {
    let c = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let host = c.get_host().await.expect("container host");
    let port = c.get_host_port_ipv4(5432).await.expect("container port");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&format!(
            "postgres://postgres:postgres@{host}:{port}/postgres"
        ))
        .await
        .expect("connect pool");
    run_migrations(&pool).await.expect("apply every migration");
    (Store::from_pool(pool), c)
}

/// RED gate: migration 0022 has added `workspaces.default_team_id`.
async fn require_default_column(pool: &PgPool) {
    let (present,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns
                         WHERE table_schema = current_schema()
                           AND table_name = 'workspaces' AND column_name = 'default_team_id')",
    )
    .fetch_one(pool)
    .await
    .expect("look the column up");
    assert!(
        present,
        "RED gate: migration 0022 has not added workspaces.default_team_id yet \
         (team-membership DDD-12, ADR-002)"
    );
}

async fn workspace(pool: &PgPool, name: &str, created_at: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name, created_at) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(name)
        .bind(ts(created_at))
        .execute(pool)
        .await
        .expect("insert workspace");
    id
}

async fn team(pool: &PgPool, workspace_id: uuid::Uuid, slug: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $3)")
        .bind(id)
        .bind(workspace_id)
        .bind(slug)
        .execute(pool)
        .await
        .expect("insert team");
    id
}

async fn admin(pool: &PgPool, workspace_id: uuid::Uuid) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, 'priya.raman@canzan.dev', 'priya.raman@canzan.dev', 'Priya Raman', 'x')",
    )
    .bind(id)
    .execute(pool)
    .await
    .expect("insert admin");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'admin')",
    )
    .bind(workspace_id)
    .bind(id)
    .execute(pool)
    .await
    .expect("admin membership");
    id
}

/// A live invite for `email` into `workspace_id`.
async fn invite(
    store: &Store,
    workspace_id: uuid::Uuid,
    by: uuid::Uuid,
    email: &str,
) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    store
        .insert_invite(
            id,
            workspace_id,
            Some(email),
            by,
            ts("2026-10-16T12:00:00Z"),
        )
        .await
        .expect("insert invite");
    id
}

/// `(team slug, role)` of every team row of the account holding `email`.
async fn places_of(pool: &PgPool, email: &str) -> Vec<(String, String)> {
    sqlx::query_as(
        "SELECT t.slug, tm.role FROM team_memberships tm
           JOIN teams t ON t.id = tm.team_id JOIN users u ON u.id = tm.user_id
          WHERE u.email_lower = $1 ORDER BY t.slug",
    )
    .bind(email)
    .fetch_all(pool)
    .await
    .expect("read places")
}

async fn in_workspace(pool: &PgPool, workspace_id: uuid::Uuid, email: &str) -> bool {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM workspace_memberships wm JOIN users u ON u.id = wm.user_id
          WHERE wm.workspace_id = $1 AND u.email_lower = $2",
    )
    .bind(workspace_id)
    .bind(email)
    .fetch_one(pool)
    .await
    .expect("count membership");
    n == 1
}

async fn set_default_by_hand(
    pool: &PgPool,
    ws: uuid::Uuid,
    team: Option<uuid::Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE workspaces SET default_team_id = $2 WHERE id = $1")
        .bind(ws)
        .bind(team)
        .execute(pool)
        .await
        .map(|_| ())
}

/// Make every team placement fail inside the database, to drive D17's savepoint.
async fn sabotage_team_placement(pool: &PgPool) {
    sqlx::query(
        "CREATE FUNCTION test_refuse_team_rows() RETURNS trigger LANGUAGE plpgsql AS
         $$ BEGIN RAISE EXCEPTION 'placement sabotaged by the test'; END $$",
    )
    .execute(pool)
    .await
    .expect("create sabotage function");
    sqlx::query(
        "CREATE TRIGGER test_refuse_team_rows BEFORE INSERT ON team_memberships
         FOR EACH ROW EXECUTE FUNCTION test_refuse_team_rows()",
    )
    .execute(pool)
    .await
    .expect("create sabotage trigger");
}

// ---------------------------------------------------------------------------
// D17's floor: no default, today's behaviour — GREEN today
// ---------------------------------------------------------------------------

#[tokio::test]
async fn with_no_default_an_invite_accept_places_the_newcomer_on_no_team() {
    let (store, _c) = migrated().await;
    let pool = store.pool().clone();
    let ws = workspace(&pool, "Canzan Labs", "2025-06-01T00:00:00Z").await;
    team(&pool, ws, "general").await;
    let priya = admin(&pool, ws).await;
    let id = invite(&store, ws, priya, "tomas.rivera@canzan.dev").await;
    let outcome = store
        .create_member_and_consume(id, "hash", ts("2026-10-09T12:00:00Z"))
        .await
        .expect("accept");
    assert!(
        matches!(outcome, MemberConsumeOutcome::Consumed { .. }),
        "{outcome:?}"
    );
    assert!(places_of(&pool, "tomas.rivera@canzan.dev").await.is_empty());
}

#[tokio::test]
async fn with_no_default_a_first_provisioning_places_the_newcomer_on_no_team() {
    let (store, _c) = migrated().await;
    let pool = store.pool().clone();
    let ws = workspace(&pool, "Canzan Labs", "2025-06-01T00:00:00Z").await;
    team(&pool, ws, "general").await;
    let outcome = store
        .provision_federated_member(
            "tomas.rivera@canzan.dev",
            "tomas.rivera@canzan.dev",
            "Tomás Rivera",
            ts("2026-10-09T12:00:00Z"),
        )
        .await
        .expect("provision");
    assert!(
        matches!(outcome, FederatedProvisionOutcome::Created { .. }),
        "{outcome:?}"
    );
    assert!(places_of(&pool, "tomas.rivera@canzan.dev").await.is_empty());
}

// ---------------------------------------------------------------------------
// Slice 05: the setting (05-01)
// ---------------------------------------------------------------------------

/// INV-7: a default naming another workspace's team is refused by the database.
#[tokio::test]
#[ignore = "pending: team-membership 05-01"]
async fn a_default_team_from_another_workspace_is_refused_by_the_database() {
    let (store, _c) = migrated().await;
    let pool = store.pool().clone();
    require_default_column(&pool).await;
    let canzan = workspace(&pool, "Canzan Labs", "2025-06-01T00:00:00Z").await;
    let globex = workspace(&pool, "Globex", "2026-01-01T00:00:00Z").await;
    let payments = team(&pool, globex, "payments").await;
    let err = set_default_by_hand(&pool, canzan, Some(payments))
        .await
        .expect_err("a foreign default must be refused");
    let db = err.as_database_error().expect("a database refusal");
    let pg = db.downcast_ref::<PgDatabaseError>();
    assert_eq!(pg.code(), "23503", "a foreign-key violation: {pg:?}");
    assert_eq!(
        pg.constraint(),
        Some(DEFAULT_FK),
        "names the composite key: {pg:?}"
    );
}

/// OQ-6: deleting the default team clears only the default; the workspace and
/// its other teams are untouched.
#[tokio::test]
#[ignore = "pending: team-membership 05-01"]
async fn deleting_the_default_team_clears_only_the_default() {
    let (store, _c) = migrated().await;
    let pool = store.pool().clone();
    require_default_column(&pool).await;
    let ws = workspace(&pool, "Canzan Labs", "2025-06-01T00:00:00Z").await;
    let onboarding = team(&pool, ws, "onboarding").await;
    team(&pool, ws, "general").await;
    store
        .set_default_team(ws, Some(onboarding))
        .await
        .expect("set");
    sqlx::query("DELETE FROM teams WHERE id = $1")
        .bind(onboarding)
        .execute(&pool)
        .await
        .expect("delete the default team by hand");
    assert_eq!(store.default_team(ws).await.expect("read"), None);
    let (name, teams): (String, i64) = sqlx::query_as(
        "SELECT w.name, (SELECT count(*) FROM teams WHERE workspace_id = w.id) FROM workspaces w WHERE w.id = $1",
    )
    .bind(ws)
    .fetch_one(&pool)
    .await
    .expect("the workspace is still there");
    assert_eq!((name.as_str(), teams), ("Canzan Labs", 1));
}

#[tokio::test]
#[ignore = "pending: team-membership 05-01"]
async fn a_workspace_with_a_default_set_can_still_be_deleted() {
    let (store, _c) = migrated().await;
    let pool = store.pool().clone();
    require_default_column(&pool).await;
    let ws = workspace(&pool, "Household", "2025-06-01T00:00:00Z").await;
    let chores = team(&pool, ws, "chores").await;
    store.set_default_team(ws, Some(chores)).await.expect("set");
    sqlx::query("DELETE FROM workspaces WHERE id = $1")
        .bind(ws)
        .execute(&pool)
        .await
        .expect("the cascade is not blocked by the default");
}

#[tokio::test]
#[ignore = "pending: team-membership 05-01"]
async fn the_default_team_is_set_read_back_and_cleared() {
    let (store, _c) = migrated().await;
    let pool = store.pool().clone();
    require_default_column(&pool).await;
    let ws = workspace(&pool, "Canzan Labs", "2025-06-01T00:00:00Z").await;
    let general = team(&pool, ws, "general").await;
    assert_eq!(
        store.default_team(ws).await.expect("read"),
        None,
        "unset on upgrade (OQ-4)"
    );
    store
        .set_default_team(ws, Some(general))
        .await
        .expect("set");
    assert_eq!(
        store.default_team(ws).await.expect("read"),
        Some(("general".to_string(), "general".to_string()))
    );
    store.set_default_team(ws, None).await.expect("clear");
    assert_eq!(store.default_team(ws).await.expect("read"), None);
}

/// OQ-8: setting a default puts nobody already in the workspace on it.
#[tokio::test]
#[ignore = "pending: team-membership 05-01"]
async fn setting_a_default_does_not_backfill_existing_members() {
    let (store, _c) = migrated().await;
    let pool = store.pool().clone();
    require_default_column(&pool).await;
    let ws = workspace(&pool, "Canzan Labs", "2025-06-01T00:00:00Z").await;
    let general = team(&pool, ws, "general").await;
    admin(&pool, ws).await;
    store
        .set_default_team(ws, Some(general))
        .await
        .expect("set");
    assert!(places_of(&pool, "priya.raman@canzan.dev").await.is_empty());
}

// ---------------------------------------------------------------------------
// Slice 05: placement on invite accept (05-01)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "pending: team-membership 05-01"]
async fn an_invite_accept_places_the_newcomer_on_the_default_team_as_member() {
    let (store, _c) = migrated().await;
    let pool = store.pool().clone();
    require_default_column(&pool).await;
    let ws = workspace(&pool, "Canzan Labs", "2025-06-01T00:00:00Z").await;
    let general = team(&pool, ws, "general").await;
    let priya = admin(&pool, ws).await;
    set_default_by_hand(&pool, ws, Some(general))
        .await
        .expect("set the default");
    let id = invite(&store, ws, priya, "tomas.rivera@canzan.dev").await;
    let outcome = store
        .create_member_and_consume(id, "hash", ts("2026-10-09T12:00:00Z"))
        .await
        .expect("accept");
    assert!(
        matches!(outcome, MemberConsumeOutcome::Consumed { .. }),
        "{outcome:?}"
    );
    assert_eq!(
        places_of(&pool, "tomas.rivera@canzan.dev").await,
        vec![("general".to_string(), "member".to_string())]
    );
}

/// D17: a placement that fails never fails the accept.
#[tokio::test]
#[ignore = "pending: team-membership 05-01"]
async fn a_failed_placement_never_fails_the_invite_accept() {
    let (store, _c) = migrated().await;
    let pool = store.pool().clone();
    require_default_column(&pool).await;
    let ws = workspace(&pool, "Canzan Labs", "2025-06-01T00:00:00Z").await;
    let general = team(&pool, ws, "general").await;
    let priya = admin(&pool, ws).await;
    set_default_by_hand(&pool, ws, Some(general))
        .await
        .expect("set the default");
    sabotage_team_placement(&pool).await;
    let id = invite(&store, ws, priya, "tomas.rivera@canzan.dev").await;
    let outcome = store
        .create_member_and_consume(id, "hash", ts("2026-10-09T12:00:00Z"))
        .await
        .expect("the accept never errors because of the default");
    assert!(
        matches!(outcome, MemberConsumeOutcome::Consumed { .. }),
        "{outcome:?}"
    );
    assert!(
        in_workspace(&pool, ws, "tomas.rivera@canzan.dev").await,
        "the membership committed"
    );
    assert!(
        places_of(&pool, "tomas.rivera@canzan.dev").await.is_empty(),
        "on no team"
    );
}

// ---------------------------------------------------------------------------
// Slice 06: placement on a first Keycloak provisioning (06-01)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "pending: team-membership 06-01"]
async fn a_first_provisioning_places_the_newcomer_on_the_original_workspaces_default_team() {
    let (store, _c) = migrated().await;
    let pool = store.pool().clone();
    require_default_column(&pool).await;
    let canzan = workspace(&pool, "Canzan Labs", "2025-06-01T00:00:00Z").await;
    let general = team(&pool, canzan, "general").await;
    let globex = workspace(&pool, "Globex", "2026-01-01T00:00:00Z").await;
    let payments = team(&pool, globex, "payments").await;
    set_default_by_hand(&pool, canzan, Some(general))
        .await
        .expect("Canzan's default");
    set_default_by_hand(&pool, globex, Some(payments))
        .await
        .expect("Globex's default");
    let outcome = store
        .provision_federated_member(
            "tomas.rivera@canzan.dev",
            "tomas.rivera@canzan.dev",
            "Tomás Rivera",
            ts("2026-10-09T12:00:00Z"),
        )
        .await
        .expect("provision");
    assert!(
        matches!(outcome, FederatedProvisionOutcome::Created { .. }),
        "{outcome:?}"
    );
    assert_eq!(
        places_of(&pool, "tomas.rivera@canzan.dev").await,
        vec![("general".to_string(), "member".to_string())],
        "only the original workspace's default applies (OQ-5)"
    );
}

#[tokio::test]
#[ignore = "pending: team-membership 06-01"]
async fn linking_an_existing_account_places_no_one() {
    let (store, _c) = migrated().await;
    let pool = store.pool().clone();
    require_default_column(&pool).await;
    let ws = workspace(&pool, "Canzan Labs", "2025-06-01T00:00:00Z").await;
    let general = team(&pool, ws, "general").await;
    admin(&pool, ws).await;
    set_default_by_hand(&pool, ws, Some(general))
        .await
        .expect("set the default");
    let outcome = store
        .provision_federated_member(
            "priya.raman@canzan.dev",
            "priya.raman@canzan.dev",
            "Priya Raman",
            ts("2026-10-09T12:00:00Z"),
        )
        .await
        .expect("link");
    assert!(
        matches!(outcome, FederatedProvisionOutcome::Existing { .. }),
        "{outcome:?}"
    );
    assert!(places_of(&pool, "priya.raman@canzan.dev").await.is_empty());
}

#[tokio::test]
#[ignore = "pending: team-membership 06-01"]
async fn a_failed_placement_never_fails_provisioning() {
    let (store, _c) = migrated().await;
    let pool = store.pool().clone();
    require_default_column(&pool).await;
    let ws = workspace(&pool, "Canzan Labs", "2025-06-01T00:00:00Z").await;
    let general = team(&pool, ws, "general").await;
    set_default_by_hand(&pool, ws, Some(general))
        .await
        .expect("set the default");
    sabotage_team_placement(&pool).await;
    let outcome = store
        .provision_federated_member(
            "tomas.rivera@canzan.dev",
            "tomas.rivera@canzan.dev",
            "Tomás Rivera",
            ts("2026-10-09T12:00:00Z"),
        )
        .await
        .expect("provisioning never errors because of the default");
    assert!(
        matches!(outcome, FederatedProvisionOutcome::Created { .. }),
        "{outcome:?}"
    );
    assert!(
        in_workspace(&pool, ws, "tomas.rivera@canzan.dev").await,
        "the membership committed"
    );
    assert!(
        places_of(&pool, "tomas.rivera@canzan.dev").await.is_empty(),
        "on no team"
    );
}
