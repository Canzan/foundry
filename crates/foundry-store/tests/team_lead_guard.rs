//! team-membership — the never-leaderless guard at the store and in the database
//! (DISCUSS D6, INV-1 to INV-5, INV-8; DESIGN DDD-6, DDD-7, DDD-10, DDD-11;
//! ADR-TEAM-MEMBERSHIP-001). DISTILL 2026-10-09.
//!
//! The gold tests DESIGN put at store level (OQ-D3): here the races are
//! deterministic (a barrier, two connections) and run 100 times each, which the
//! HTTP lane cannot afford. A run passes only if exactly one of the two writes
//! goes through, the other is the typed `LastLead`, nothing errors, and the team
//! ends with exactly one lead.
//!
//! RED scaffolds: the store methods (`src/teams.rs`) panic until DELIVER lands
//! them, and every test that needs them or migration 0021 is
//! `#[ignore = "pending: team-membership NN-NN"]`, so the suite stays green. Run
//! them with `--include-ignored` to see them RED. Tests that need 0021 first pass
//! `require_lead_guard_installed`, a readable RED gate, never an
//! undefined-object error.
//!
//! GREEN today, not ignored: the two Postgres substrate facts the lock relies on
//! (ADR-001 Earned Trust 3 and 4). They pin the pinned server's behaviour, not
//! this feature's code.
//!
//! The workspace removal is reached through `remove_workspace_member`, whose
//! outcome DDD-11 extends to `Removed | NotMember | LastLead { team }`. Until
//! then [`remove_from_workspace`] reads today's row count; DELIVER 03-01 swaps
//! its body for the new outcome enum (marked `SCAFFOLD`).
//!
//! Real Postgres (testcontainers `postgres:16-alpine`, matching production);
//! integration level, example-based, with the races repeated (Mandates 9/11).

use foundry_core::TeamRole;
use foundry_store::{
    run_migrations, run_migrations_from_dir, RemoveTeamMemberOutcome, SetTeamRoleOutcome, Store,
};
use sqlx::postgres::{PgDatabaseError, PgPoolOptions};
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
use tokio::sync::Barrier;

/// How many times each race runs (DoD 3, KPI-3).
const RACE_RUNS: usize = 100;
/// The rule's name in the database's refusal (DDD-6).
const LEAD_RULE: &str = "team_memberships_team_keeps_a_lead";

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

async fn container() -> ContainerAsync<Postgres> {
    Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container")
}

async fn connect(c: &ContainerAsync<Postgres>) -> PgPool {
    let host = c.get_host().await.expect("container host");
    let port = c.get_host_port_ipv4(5432).await.expect("container port");
    PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&format!(
            "postgres://postgres:postgres@{host}:{port}/postgres"
        ))
        .await
        .expect("connect pool")
}

async fn migrated() -> (Store, ContainerAsync<Postgres>) {
    let c = container().await;
    let pool = connect(&c).await;
    run_migrations(&pool).await.expect("apply every migration");
    (Store::from_pool(pool), c)
}

fn production_migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

/// A private copy of the production migrations up to and including `through`.
fn staged_migrations(through: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("foundry-tm-{}", uuid::Uuid::now_v7()));
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

/// RED gate: migration 0021 has installed the lead guard (one function, two
/// triggers on `team_memberships`).
async fn require_lead_guard_installed(pool: &PgPool) {
    let (function, triggers): (bool, i64) = sqlx::query_as(
        "SELECT to_regprocedure('foundry_team_keeps_a_lead()') IS NOT NULL,
                (SELECT count(*) FROM pg_trigger t JOIN pg_class c ON c.oid = t.tgrelid
                  WHERE c.relname = 'team_memberships'
                    AND c.relnamespace = current_schema()::regnamespace
                    AND t.tgname LIKE 'team_memberships_lead_guard_on_%')",
    )
    .fetch_one(pool)
    .await
    .expect("look the lead guard up");
    assert!(
        function && triggers == 2,
        "RED gate: migration 0021 has not installed the lead guard yet \
         (function {function}, {triggers} of 2 triggers; team-membership DDD-6, ADR-001)"
    );
}

// ---------------------------------------------------------------------------
// Rows
// ---------------------------------------------------------------------------

async fn workspace(pool: &PgPool, name: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(id)
        .bind(name)
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

/// A person in `workspace_id` (as `member`).
async fn person(pool: &PgPool, workspace_id: uuid::Uuid, email: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, $2, 'not-a-real-hash')",
    )
    .bind(id)
    .bind(email)
    .execute(pool)
    .await
    .expect("insert user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(id)
    .execute(pool)
    .await
    .expect("insert workspace membership");
    id
}

/// Put someone on a team with a role, or raise them to it (a promotion is never
/// refused by the guard, so this is a valid reset between race runs).
async fn place(pool: &PgPool, team_id: uuid::Uuid, user_id: uuid::Uuid, role: &str) {
    sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, $3)
             ON CONFLICT (team_id, user_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(team_id)
    .bind(user_id)
    .bind(role)
    .execute(pool)
    .await
    .expect("place someone on a team");
}

/// Every team row: `(team, user) -> role`.
async fn team_rows(pool: &PgPool) -> BTreeMap<(uuid::Uuid, uuid::Uuid), String> {
    let rows: Vec<(uuid::Uuid, uuid::Uuid, String)> =
        sqlx::query_as("SELECT team_id, user_id, role FROM team_memberships")
            .fetch_all(pool)
            .await
            .expect("read team rows");
    rows.into_iter().map(|(t, u, r)| ((t, u), r)).collect()
}

async fn workspace_rows(pool: &PgPool) -> BTreeMap<(uuid::Uuid, uuid::Uuid), String> {
    let rows: Vec<(uuid::Uuid, uuid::Uuid, String)> =
        sqlx::query_as("SELECT workspace_id, user_id, role FROM workspace_memberships")
            .fetch_all(pool)
            .await
            .expect("read workspace rows");
    rows.into_iter().map(|(w, u, r)| ((w, u), r)).collect()
}

async fn leads(pool: &PgPool, team_id: uuid::Uuid) -> i64 {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM team_memberships WHERE team_id = $1 AND role = 'lead'",
    )
    .bind(team_id)
    .fetch_one(pool)
    .await
    .expect("count leads");
    n
}

fn assert_lead_rule(err: sqlx::Error) {
    let db = err
        .as_database_error()
        .unwrap_or_else(|| panic!("expected the database's refusal, got {err:?}"));
    let pg = db.downcast_ref::<PgDatabaseError>();
    assert_eq!(
        pg.code(),
        "23514",
        "the refusal is a check violation: {pg:?}"
    );
    assert_eq!(
        pg.constraint(),
        Some(LEAD_RULE),
        "the refusal names the rule: {pg:?}"
    );
    assert_eq!(
        pg.table(),
        Some("team_memberships"),
        "the refusal names the table: {pg:?}"
    );
    assert!(pg.detail().is_none(), "no row dump in DETAIL: {pg:?}");
}

/// One team in one workspace with two people on it: a lead and a member.
struct Backend {
    pool: PgPool,
    store: Store,
    ws: uuid::Uuid,
    team: uuid::Uuid,
    lena: uuid::Uuid,
    sam: uuid::Uuid,
    _c: ContainerAsync<Postgres>,
}

async fn backend() -> Backend {
    let (store, c) = migrated().await;
    let pool = store.pool().clone();
    let ws = workspace(&pool, "Canzan Labs").await;
    let team = team(&pool, ws, "backend").await;
    let lena = person(&pool, ws, "lena.park@canzan.dev").await;
    let sam = person(&pool, ws, "sam.okafor@canzan.dev").await;
    place(&pool, team, lena, "lead").await;
    place(&pool, team, sam, "member").await;
    Backend {
        pool,
        store,
        ws,
        team,
        lena,
        sam,
        _c: c,
    }
}

// ---------------------------------------------------------------------------
// The workspace-removal door, until DDD-11 lands
// ---------------------------------------------------------------------------

/// The outcome DDD-11 gives `remove_workspace_member`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum WorkspaceRemoval {
    Removed,
    NotMember,
    LastLead(String),
}

/// SCAFFOLD: true — today `remove_workspace_member` returns a row count and can
/// neither refuse a last lead nor delete team rows. DELIVER 03-01 replaces this
/// body with the extended method's outcome enum.
async fn remove_from_workspace(
    store: &Store,
    ws: uuid::Uuid,
    user: uuid::Uuid,
) -> Result<WorkspaceRemoval, foundry_store::StoreError> {
    let removed = store.remove_workspace_member(ws, user).await?;
    Ok(if removed == 0 {
        WorkspaceRemoval::NotMember
    } else {
        WorkspaceRemoval::Removed
    })
}

// ---------------------------------------------------------------------------
// Substrate facts (ADR-001 Earned Trust 3 and 4) — GREEN today
// ---------------------------------------------------------------------------

/// Fact 3: the lock a lead-removing write takes on the team row
/// (`FOR NO KEY UPDATE`) does not block the `FOR KEY SHARE` an insert's foreign
/// key check takes, so adding someone to a team is never held up by a demotion.
#[tokio::test]
async fn an_add_is_not_blocked_while_a_demotion_holds_the_team_lock() {
    let b = backend().await;
    let tomas = person(&b.pool, b.ws, "tomas.rivera@canzan.dev").await;

    let mut demotion = b.pool.begin().await.expect("begin");
    sqlx::query("SELECT id FROM teams WHERE id = $1 FOR NO KEY UPDATE")
        .bind(b.team)
        .execute(&mut *demotion)
        .await
        .expect("take the team lock");

    let insert = sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(b.team)
    .bind(tomas)
    .execute(&b.pool);
    let done = tokio::time::timeout(Duration::from_secs(3), insert).await;
    assert!(
        matches!(done, Ok(Ok(_))),
        "an add must not wait on the team lock: {done:?}"
    );
    demotion.rollback().await.expect("rollback");
}

/// Fact 4: at READ COMMITTED a statement that waited for the team lock sees what
/// the lock holder committed, so the pre-read after the lock is exact.
#[tokio::test]
async fn a_read_after_waiting_for_the_team_lock_sees_the_holders_commit() {
    let b = backend().await;
    place(&b.pool, b.team, b.sam, "lead").await;

    let mut first = b.pool.begin().await.expect("begin first");
    sqlx::query("SELECT id FROM teams WHERE id = $1 FOR NO KEY UPDATE")
        .bind(b.team)
        .execute(&mut *first)
        .await
        .expect("first takes the lock");
    sqlx::query("UPDATE team_memberships SET role = 'member' WHERE team_id = $1 AND user_id = $2")
        .bind(b.team)
        .bind(b.lena)
        .execute(&mut *first)
        .await
        .expect("first demotes Lena");

    let pool = b.pool.clone();
    let team = b.team;
    let second = tokio::spawn(async move {
        let mut tx = pool.begin().await.expect("begin second");
        sqlx::query("SELECT id FROM teams WHERE id = $1 FOR NO KEY UPDATE")
            .bind(team)
            .execute(&mut *tx)
            .await
            .expect("second waits for the lock");
        let (n,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM team_memberships WHERE team_id = $1 AND role = 'lead'",
        )
        .bind(team)
        .fetch_one(&mut *tx)
        .await
        .expect("second counts leads");
        tx.rollback().await.expect("rollback second");
        n
    });
    tokio::time::sleep(Duration::from_millis(200)).await;
    first.commit().await.expect("first commits");
    let seen = second.await.expect("second finished");
    assert_eq!(
        seen, 1,
        "the waiter must see the demotion the holder committed"
    );
}

// ---------------------------------------------------------------------------
// Slice 02: demote and promote (02-01)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "pending: team-membership 02-01"]
async fn making_the_only_lead_a_member_is_refused_as_last_lead_and_changes_nothing() {
    let b = backend().await;
    let before = team_rows(&b.pool).await;
    let outcome = b
        .store
        .set_team_role(b.team, b.lena, TeamRole::Member)
        .await
        .expect("no error");
    assert_eq!(outcome, SetTeamRoleOutcome::LastLead);
    assert_eq!(team_rows(&b.pool).await, before, "a refusal writes nothing");
}

#[tokio::test]
#[ignore = "pending: team-membership 02-01"]
async fn a_promotion_then_a_hand_over_change_exactly_one_role_each_and_a_repeat_changes_nothing() {
    let b = backend().await;
    let mut expected = team_rows(&b.pool).await;

    let promoted = b.store.set_team_role(b.team, b.sam, TeamRole::Lead).await;
    assert_eq!(promoted.expect("no error"), SetTeamRoleOutcome::Changed);
    expected.insert((b.team, b.sam), "lead".into());
    assert_eq!(team_rows(&b.pool).await, expected, "only Sam's role moved");

    let again = b.store.set_team_role(b.team, b.sam, TeamRole::Lead).await;
    assert_eq!(again.expect("no error"), SetTeamRoleOutcome::Unchanged);
    assert_eq!(
        team_rows(&b.pool).await,
        expected,
        "a same-role write changes nothing"
    );

    let handed = b
        .store
        .set_team_role(b.team, b.lena, TeamRole::Member)
        .await;
    assert_eq!(handed.expect("no error"), SetTeamRoleOutcome::Changed);
    expected.insert((b.team, b.lena), "member".into());
    assert_eq!(team_rows(&b.pool).await, expected, "only Lena's role moved");
}

#[tokio::test]
#[ignore = "pending: team-membership 02-01"]
async fn a_role_change_for_someone_not_on_the_team_changes_nothing() {
    let b = backend().await;
    let tomas = person(&b.pool, b.ws, "tomas.rivera@canzan.dev").await;
    let before = team_rows(&b.pool).await;
    let outcome = b.store.set_team_role(b.team, tomas, TeamRole::Lead).await;
    assert_eq!(outcome.expect("no error"), SetTeamRoleOutcome::NotOnTeam);
    assert_eq!(team_rows(&b.pool).await, before);
}

#[tokio::test]
#[ignore = "pending: team-membership 02-01"]
async fn a_team_with_no_lead_can_be_given_one() {
    let b = backend().await;
    let design = team(&b.pool, b.ws, "design").await;
    place(&b.pool, design, b.sam, "member").await;
    let outcome = b.store.set_team_role(design, b.sam, TeamRole::Lead).await;
    assert_eq!(outcome.expect("no error"), SetTeamRoleOutcome::Changed);
    assert_eq!(leads(&b.pool, design).await, 1);
}

/// INV-8: a hand-typed statement that skips the store is refused by the database
/// on its second row (a BEFORE ROW trigger sees the first row's effect, ADR-001
/// fact 1).
#[tokio::test]
#[ignore = "pending: team-membership 02-01"]
async fn one_statement_demoting_both_leads_is_refused_by_the_database_naming_the_rule() {
    let b = backend().await;
    require_lead_guard_installed(&b.pool).await;
    place(&b.pool, b.team, b.sam, "lead").await;
    let before = team_rows(&b.pool).await;
    let err = sqlx::query("UPDATE team_memberships SET role = 'member' WHERE team_id = $1")
        .bind(b.team)
        .execute(&b.pool)
        .await
        .expect_err("the database must refuse leaving the team without a lead");
    assert_lead_rule(err);
    assert_eq!(
        team_rows(&b.pool).await,
        before,
        "the whole statement rolled back"
    );
}

#[tokio::test]
#[ignore = "pending: team-membership 02-01"]
async fn a_hand_delete_of_the_last_lead_is_refused_by_the_database() {
    let b = backend().await;
    require_lead_guard_installed(&b.pool).await;
    let err = sqlx::query("DELETE FROM team_memberships WHERE team_id = $1 AND user_id = $2")
        .bind(b.team)
        .bind(b.lena)
        .execute(&b.pool)
        .await
        .expect_err("the database must refuse removing the last lead");
    assert_lead_rule(err);
    assert_eq!(leads(&b.pool, b.team).await, 1);
}

/// ADR-001 fact 2: a team (or workspace) that is being deleted needs no lead, so
/// the cascade is exempt.
#[tokio::test]
#[ignore = "pending: team-membership 02-01"]
async fn deleting_a_team_or_a_workspace_that_has_a_lead_succeeds() {
    let b = backend().await;
    require_lead_guard_installed(&b.pool).await;
    sqlx::query("DELETE FROM teams WHERE id = $1")
        .bind(b.team)
        .execute(&b.pool)
        .await
        .expect("deleting a team with a lead cascades");
    let other = workspace(&b.pool, "Household").await;
    let chores = team(&b.pool, other, "chores").await;
    let maya = person(&b.pool, other, "maya@household.example").await;
    place(&b.pool, chores, maya, "lead").await;
    sqlx::query("DELETE FROM workspaces WHERE id = $1")
        .bind(other)
        .execute(&b.pool)
        .await
        .expect("deleting a workspace whose team has a lead cascades");
    assert!(
        team_rows(&b.pool).await.is_empty(),
        "both cascades removed every row"
    );
}

/// The 100-run race of INV-2: two demotions of the last two leads.
#[tokio::test]
#[ignore = "pending: team-membership 02-01"]
async fn two_demotions_of_the_last_two_leads_leave_exactly_one_lead_every_time() {
    let b = backend().await;
    for run in 1..=RACE_RUNS {
        place(&b.pool, b.team, b.lena, "lead").await;
        place(&b.pool, b.team, b.sam, "lead").await;
        let barrier = Arc::new(Barrier::new(2));
        let (s1, s2, g1, g2) = (b.store.clone(), b.store.clone(), barrier.clone(), barrier);
        let (team, lena, sam) = (b.team, b.lena, b.sam);
        let one = tokio::spawn(async move {
            g1.wait().await;
            s1.set_team_role(team, lena, TeamRole::Member).await
        });
        let two = tokio::spawn(async move {
            g2.wait().await;
            s2.set_team_role(team, sam, TeamRole::Member).await
        });
        let outcomes = [
            one.await.expect("task").expect("no error"),
            two.await.expect("task").expect("no error"),
        ];
        assert!(
            outcomes.contains(&SetTeamRoleOutcome::Changed)
                && outcomes.contains(&SetTeamRoleOutcome::LastLead),
            "run {run}: exactly one demotion goes through; got {outcomes:?}"
        );
        assert_eq!(leads(&b.pool, b.team).await, 1, "run {run}: one lead left");
    }
}

/// DDD-7: before installing the guard, 0021 deletes team rows whose user is no
/// longer in the team's workspace (left by the pre-OQ-9 removal); every other
/// row is untouched.
#[tokio::test]
#[ignore = "pending: team-membership 02-01"]
async fn migration_0021_deletes_orphan_team_rows_and_leaves_every_other_row_alone() {
    let c = container().await;
    let pool = connect(&c).await;
    run_migrations_from_dir(&pool, &staged_migrations("0020"))
        .await
        .expect("stand the schema at 0020");
    let ws = workspace(&pool, "Canzan Labs").await;
    let backend = team(&pool, ws, "backend").await;
    let lena = person(&pool, ws, "lena.park@canzan.dev").await;
    let gone = person(&pool, ws, "gone@canzan.dev").await;
    place(&pool, backend, lena, "lead").await;
    place(&pool, backend, gone, "lead").await;
    sqlx::query("DELETE FROM workspace_memberships WHERE user_id = $1")
        .bind(gone)
        .execute(&pool)
        .await
        .expect("the pre-OQ-9 removal left a team row behind");
    let mut expected = team_rows(&pool).await;
    expected.remove(&(backend, gone));

    run_migrations(&pool).await.expect("0021 applies");
    require_lead_guard_installed(&pool).await;
    assert_eq!(team_rows(&pool).await, expected, "only the orphan row went");
}

// ---------------------------------------------------------------------------
// Slice 03: remove, and removal from the workspace (03-01)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "pending: team-membership 03-01"]
async fn removing_the_only_lead_is_refused_as_last_lead_and_changes_nothing() {
    let b = backend().await;
    let before = team_rows(&b.pool).await;
    let outcome = b.store.remove_team_member(b.team, b.lena).await;
    assert_eq!(
        outcome.expect("no error"),
        RemoveTeamMemberOutcome::LastLead
    );
    assert_eq!(team_rows(&b.pool).await, before);
}

#[tokio::test]
#[ignore = "pending: team-membership 03-01"]
async fn removing_a_member_deletes_exactly_their_row_and_a_repeat_changes_nothing() {
    let b = backend().await;
    let mut expected = team_rows(&b.pool).await;
    let outcome = b.store.remove_team_member(b.team, b.sam).await;
    assert_eq!(outcome.expect("no error"), RemoveTeamMemberOutcome::Removed);
    expected.remove(&(b.team, b.sam));
    assert_eq!(team_rows(&b.pool).await, expected);
    let again = b.store.remove_team_member(b.team, b.sam).await;
    assert_eq!(again.expect("no error"), RemoveTeamMemberOutcome::NotOnTeam);
    assert_eq!(team_rows(&b.pool).await, expected);
}

/// The 100-run race of INV-3: a removal and a demotion of the last two leads.
#[tokio::test]
#[ignore = "pending: team-membership 03-01"]
async fn a_removal_racing_a_demotion_of_the_last_two_leads_leaves_exactly_one_lead_every_time() {
    let b = backend().await;
    for run in 1..=RACE_RUNS {
        place(&b.pool, b.team, b.lena, "lead").await;
        place(&b.pool, b.team, b.sam, "lead").await;
        let barrier = Arc::new(Barrier::new(2));
        let (s1, s2, g1, g2) = (b.store.clone(), b.store.clone(), barrier.clone(), barrier);
        let (team, lena, sam) = (b.team, b.lena, b.sam);
        let removal = tokio::spawn(async move {
            g1.wait().await;
            s1.remove_team_member(team, lena).await
        });
        let demotion = tokio::spawn(async move {
            g2.wait().await;
            s2.set_team_role(team, sam, TeamRole::Member).await
        });
        let removal = removal.await.expect("task").expect("no error");
        let demotion = demotion.await.expect("task").expect("no error");
        let one_went_through = matches!(
            (removal, demotion),
            (
                RemoveTeamMemberOutcome::Removed,
                SetTeamRoleOutcome::LastLead
            ) | (
                RemoveTeamMemberOutcome::LastLead,
                SetTeamRoleOutcome::Changed
            )
        );
        assert!(
            one_went_through,
            "run {run}: exactly one goes through; got {removal:?} and {demotion:?}"
        );
        assert_eq!(leads(&b.pool, b.team).await, 1, "run {run}: one lead left");
    }
}

/// The 100-run race of INV-3: two removals of the last two leads.
#[tokio::test]
#[ignore = "pending: team-membership 03-01"]
async fn two_removals_of_the_last_two_leads_leave_exactly_one_lead_every_time() {
    let b = backend().await;
    for run in 1..=RACE_RUNS {
        place(&b.pool, b.team, b.lena, "lead").await;
        place(&b.pool, b.team, b.sam, "lead").await;
        let barrier = Arc::new(Barrier::new(2));
        let (s1, s2, g1, g2) = (b.store.clone(), b.store.clone(), barrier.clone(), barrier);
        let (team, lena, sam) = (b.team, b.lena, b.sam);
        let one = tokio::spawn(async move {
            g1.wait().await;
            s1.remove_team_member(team, lena).await
        });
        let two = tokio::spawn(async move {
            g2.wait().await;
            s2.remove_team_member(team, sam).await
        });
        let outcomes = [
            one.await.expect("task").expect("no error"),
            two.await.expect("task").expect("no error"),
        ];
        assert!(
            outcomes.contains(&RemoveTeamMemberOutcome::Removed)
                && outcomes.contains(&RemoveTeamMemberOutcome::LastLead),
            "run {run}: exactly one removal goes through; got {outcomes:?}"
        );
        assert_eq!(leads(&b.pool, b.team).await, 1, "run {run}: one lead left");
    }
}

/// OQ-9: removing someone from a workspace takes them off that workspace's teams
/// and no other workspace's.
#[tokio::test]
#[ignore = "pending: team-membership 03-01"]
async fn removal_from_a_workspace_takes_the_person_off_its_teams_only() {
    let b = backend().await;
    let other = workspace(&b.pool, "Household").await;
    let chores = team(&b.pool, other, "chores").await;
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(other)
    .bind(b.sam)
    .execute(&b.pool)
    .await
    .expect("Sam is also in Household");
    place(&b.pool, chores, b.sam, "member").await;
    let mut teams_after = team_rows(&b.pool).await;
    teams_after.remove(&(b.team, b.sam));
    let mut members_after = workspace_rows(&b.pool).await;
    members_after.remove(&(b.ws, b.sam));

    let outcome = remove_from_workspace(&b.store, b.ws, b.sam).await;
    assert_eq!(outcome.expect("no error"), WorkspaceRemoval::Removed);
    assert_eq!(
        team_rows(&b.pool).await,
        teams_after,
        "Canzan Labs's team rows went; Household's stayed"
    );
    assert_eq!(workspace_rows(&b.pool).await, members_after);
}

#[tokio::test]
#[ignore = "pending: team-membership 03-01"]
async fn removal_from_a_workspace_of_a_teams_only_lead_is_refused_and_changes_nothing() {
    let b = backend().await;
    let (teams_before, members_before) = (team_rows(&b.pool).await, workspace_rows(&b.pool).await);
    let outcome = remove_from_workspace(&b.store, b.ws, b.lena).await;
    assert_eq!(
        outcome.expect("no error"),
        WorkspaceRemoval::LastLead("backend".into())
    );
    assert_eq!(team_rows(&b.pool).await, teams_before);
    assert_eq!(workspace_rows(&b.pool).await, members_before);
}

/// The 100-run race of INV-1/INV-3 through OQ-9's door: removing one of the last
/// two leads from the workspace while the other is demoted.
#[tokio::test]
#[ignore = "pending: team-membership 03-01"]
async fn a_workspace_removal_racing_a_demotion_of_the_last_two_leads_leaves_exactly_one_lead() {
    let b = backend().await;
    for run in 1..=RACE_RUNS {
        sqlx::query(
            "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')
                 ON CONFLICT DO NOTHING",
        )
        .bind(b.ws)
        .bind(b.lena)
        .execute(&b.pool)
        .await
        .expect("Lena is back in the workspace");
        place(&b.pool, b.team, b.lena, "lead").await;
        place(&b.pool, b.team, b.sam, "lead").await;
        let barrier = Arc::new(Barrier::new(2));
        let (s1, s2, g1, g2) = (b.store.clone(), b.store.clone(), barrier.clone(), barrier);
        let (ws, team, lena, sam) = (b.ws, b.team, b.lena, b.sam);
        let removal = tokio::spawn(async move {
            g1.wait().await;
            remove_from_workspace(&s1, ws, lena).await
        });
        let demotion = tokio::spawn(async move {
            g2.wait().await;
            s2.set_team_role(team, sam, TeamRole::Member).await
        });
        let removal = removal.await.expect("task").expect("no error");
        let demotion = demotion.await.expect("task").expect("no error");
        let one_went_through = matches!(
            (&removal, demotion),
            (WorkspaceRemoval::Removed, SetTeamRoleOutcome::LastLead)
                | (WorkspaceRemoval::LastLead(_), SetTeamRoleOutcome::Changed)
        );
        assert!(
            one_went_through,
            "run {run}: exactly one goes through; got {removal:?} and {demotion:?}"
        );
        assert_eq!(leads(&b.pool, b.team).await, 1, "run {run}: one lead left");
    }
}

/// INV-5 (DDD-10 with DDD-11): an add racing a removal from the workspace never
/// leaves anyone on a team of a workspace they are not in, whichever wins.
#[tokio::test]
#[ignore = "pending: team-membership 03-01"]
async fn an_add_racing_a_workspace_removal_never_leaves_someone_on_a_team_outside_their_workspace()
{
    let b = backend().await;
    for run in 1..=RACE_RUNS {
        let tomas = person(&b.pool, b.ws, &format!("tomas.{run}@canzan.dev")).await;
        let barrier = Arc::new(Barrier::new(2));
        let (s1, s2, g1, g2) = (b.store.clone(), b.store.clone(), barrier.clone(), barrier);
        let (ws, team) = (b.ws, b.team);
        let add = tokio::spawn(async move {
            g1.wait().await;
            s1.add_team_member(ws, team, tomas).await
        });
        let removal = tokio::spawn(async move {
            g2.wait().await;
            remove_from_workspace(&s2, ws, tomas).await
        });
        add.await.expect("task").expect("the add never errors");
        assert_eq!(
            removal.await.expect("task").expect("no error"),
            WorkspaceRemoval::Removed,
            "run {run}: the removal always goes through"
        );
        let rows = team_rows(&b.pool).await;
        assert!(
            !rows.contains_key(&(b.team, tomas)),
            "run {run}: nobody stays on a team of a workspace they left"
        );
    }
}
