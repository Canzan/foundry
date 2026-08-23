//! Integration-style unit tests for the `lanes::{delete_lane_dialog,
//! delete_lane}` use-case shells (board-lane-management), driven through the
//! services driving port against a REAL Postgres harness (@real-io) — the
//! `rename_project_use_case` idiom.
//!
//! The PURE classification heart is proptest-pinned in
//! `src/lanes.rs::classify_lane_delete_properties`; what only a real store can
//! exercise is the composition AROUND it. DELIVER Phase 5 mutation testing
//! showed these shells' guards survived when covered only by the @blm
//! acceptance lane (the @real-io trap): the `!is_member` gate inversion, the
//! machine-scope `!=` inversion, the dialog's lane-find `==` inversion, the
//! survivors filter, and the lane-exists/destination prechecks feeding
//! `classify_lane_delete`. These tests kill them at the service seam.
//!
//! Test budget: 5 distinct behaviours (dialog view content, authz refusal
//! uniformity, delete-fate success, move-fate success, absent-lane 404
//! precedence) × 2 = 10 max; 5 written.
//!
//! Integration level: single-example tests verify WIRING (paradigm matrix —
//! integration layer is example-based by design).

use foundry_services::lanes::{delete_lane, delete_lane_dialog, DeleteLaneError, LaneFate};
use foundry_services::Principal;
use foundry_store::Store;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::ConnectOptions;
use std::str::FromStr;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ImageExt;

struct Harness {
    _container: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    store: Store,
    workspace_id: uuid::Uuid,
    team_id: uuid::Uuid,
    project_id: uuid::Uuid,
    /// The bootstrap operator — a 'lead' member of team General.
    operator_id: uuid::Uuid,
    /// A workspace user who is NOT a member of team General.
    outsider_id: uuid::Uuid,
}

impl Harness {
    fn operator(&self) -> Principal {
        Principal::Human {
            user_id: self.operator_id,
            workspace_id: self.workspace_id,
        }
    }

    fn outsider(&self) -> Principal {
        Principal::Human {
            user_id: self.outsider_id,
            workspace_id: self.workspace_id,
        }
    }

    /// A machine credential bound to the OPERATOR (a genuine member) but
    /// team-scoped to `scope_team_id`.
    fn machine_scoped_to(&self, scope_team_id: uuid::Uuid) -> Principal {
        Principal::Machine {
            user_id: self.operator_id,
            workspace_id: self.workspace_id,
            jti: uuid::Uuid::now_v7(),
            scope_team_id: Some(scope_team_id),
        }
    }
}

/// Spin a real Postgres, migrate it, claim the instance (workspace "Acme",
/// team "General"/`general`, project "Sandbox"/`sandbox`/`GEN` with the THREE
/// creation-seed lanes backlog(0)/in_progress(1)/done(2)), then insert a plain
/// non-member user as the unauthorized actor.
async fn seeded_harness() -> Harness {
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

    let opts = PgConnectOptions::from_str(&base)
        .expect("parse base url")
        .disable_statement_logging();
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(opts)
        .await
        .expect("connect pool");
    foundry_store::run_migrations(&pool)
        .await
        .expect("run migrations");
    let store = Store::from_pool(pool);

    let workspace_id = uuid::Uuid::now_v7();
    let operator_id = uuid::Uuid::now_v7();
    let team_id = uuid::Uuid::now_v7();
    let project_id = uuid::Uuid::now_v7();
    store
        .create_initial_workspace(
            workspace_id,
            "Acme",
            operator_id,
            "ops@acme.com",
            "ops@acme.com",
            "Ops",
            "phc$dummy",
            team_id,
            "General",
            "general",
            project_id,
            "Sandbox",
            "sandbox",
            "GEN",
        )
        .await
        .expect("bootstrap claim");

    let outsider_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(outsider_id)
    .bind("mallory@acme.com")
    .bind("mallory@acme.com")
    .bind("Mallory")
    .bind("phc$dummy")
    .execute(store.pool())
    .await
    .expect("insert non-member user");

    Harness {
        _container: container,
        store,
        workspace_id,
        team_id,
        project_id,
        operator_id,
        outsider_id,
    }
}

/// Seed a card directly at `(state, position)` — the driven-port-side fixture
/// (preconditions only; the use-case under test does all lane work).
async fn seed_card(h: &Harness, number: i32, state: &str, position: i32) {
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md,
                             state, position, author_id)
              VALUES ($1, $2, $3, $4, 'seed', '', $5, $6, $7)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(h.project_id)
    .bind(h.workspace_id)
    .bind(number)
    .bind(state)
    .bind(position)
    .bind(h.operator_id)
    .execute(h.store.pool())
    .await
    .expect("seed card");
}

async fn states_by_number(h: &Harness) -> Vec<(i32, String)> {
    sqlx::query_as("SELECT number, state FROM issues WHERE project_id = $1 ORDER BY number")
        .bind(h.project_id)
        .fetch_all(h.store.pool())
        .await
        .expect("read issue states")
}

/// Behaviour 1 — the dialog view: the DYING lane's identity, the LIVE
/// advisory count, and the survivors in board order (leftmost first). Kills
/// the lane-find `==`→`!=` inversion (wrong lane picked), `list_lanes` →
/// `Ok(vec![])` (dialog would 404), `survivors_of` → `vec![]`, and the
/// survivors filter `!=`→`==` (which would offer ONLY the dying lane).
#[tokio::test]
async fn dialog_shows_the_dying_lane_with_live_count_and_survivors_in_board_order() {
    let h = seeded_harness().await;
    seed_card(&h, 1, "in_progress", 0).await;
    seed_card(&h, 2, "in_progress", 1).await;

    let view = delete_lane_dialog(&h.store, &h.operator(), "general", "sandbox", "in_progress")
        .await
        .expect("a member's dialog read for a real lane must succeed");

    assert_eq!(
        (view.lane_slug.as_str(), view.lane_label.as_str()),
        ("in_progress", "In-Progress"),
        "the dialog must describe the DYING lane, not another"
    );
    assert_eq!(view.card_count, 2, "the advisory count must be LIVE");
    let survivor_slugs: Vec<&str> = view.survivors.iter().map(|l| l.slug.as_str()).collect();
    assert_eq!(
        survivor_slugs,
        vec!["backlog", "done"],
        "survivors must be every OTHER lane, in board order (leftmost preselect)"
    );
}

/// Behaviour 2 — the D10 uniform non-enumerable 404: a non-member is refused
/// on BOTH verbs, a machine credential scoped to ANOTHER team is refused, and
/// a machine scoped to THIS team succeeds. Kills the `!is_member` gate
/// inversion and the machine-scope `!=`→`==` inversion (which would refuse
/// the correctly-scoped credential and admit the foreign-scoped one).
#[tokio::test]
async fn non_member_and_foreign_scoped_machine_are_refused_the_uniform_not_found() {
    let h = seeded_harness().await;

    let dialog = delete_lane_dialog(&h.store, &h.outsider(), "general", "sandbox", "done").await;
    assert!(
        matches!(dialog, Err(DeleteLaneError::NotFound)),
        "a non-member's dialog GET must be the uniform NotFound; got {dialog:?}"
    );

    let confirm = delete_lane(
        &h.store,
        &h.outsider(),
        "general",
        "sandbox",
        "done",
        LaneFate::Delete,
    )
    .await;
    assert!(
        matches!(confirm, Err(DeleteLaneError::NotFound)),
        "a non-member's confirm POST must be the uniform NotFound; got {confirm:?}"
    );

    let foreign_scope = h.machine_scoped_to(uuid::Uuid::now_v7());
    let scoped_out =
        delete_lane_dialog(&h.store, &foreign_scope, "general", "sandbox", "done").await;
    assert!(
        matches!(scoped_out, Err(DeleteLaneError::NotFound)),
        "a machine credential scoped to ANOTHER team must be refused NotFound; got {scoped_out:?}"
    );

    let home_scope = h.machine_scoped_to(h.team_id);
    let scoped_in = delete_lane_dialog(&h.store, &home_scope, "general", "sandbox", "done").await;
    assert!(
        scoped_in.is_ok(),
        "a machine credential scoped to THIS team must pass the gate; got {scoped_in:?}"
    );

    // Neither refusal wrote anything: all three creation lanes still stand.
    let lanes = h
        .store
        .list_project_lanes(h.project_id)
        .await
        .expect("list lanes");
    assert_eq!(lanes.len(), 3, "a refusal must not touch the board");
}

/// Behaviour 3 — the delete fate through the driving port: the cards are gone
/// for good, the lane row is gone, and the success carries truthful counts +
/// the surviving lanes for the columns re-render.
#[tokio::test]
async fn delete_fate_removes_the_cards_and_reports_truthful_counts() {
    let h = seeded_harness().await;
    seed_card(&h, 1, "in_progress", 0).await;
    seed_card(&h, 2, "in_progress", 1).await;
    seed_card(&h, 3, "backlog", 0).await;

    let success = delete_lane(
        &h.store,
        &h.operator(),
        "general",
        "sandbox",
        "in_progress",
        LaneFate::Delete,
    )
    .await
    .expect("a member's delete-fate confirm on a populated lane must succeed");

    assert_eq!(
        (success.moved, success.deleted),
        (0, 2),
        "the delete fate must report 0 moved / 2 deleted"
    );
    let surviving: Vec<&str> = success.surviving.iter().map(|l| l.slug.as_str()).collect();
    assert_eq!(
        surviving,
        vec!["backlog", "done"],
        "the success must carry the SURVIVING lanes in board order"
    );
    assert_eq!(
        states_by_number(&h).await,
        vec![(3, "backlog".to_string())],
        "the dying lane's cards must be deleted; other lanes' cards untouched"
    );
}

/// Behaviour 4 — the move fate through the driving port: the cards land in
/// the chosen destination and the success carries truthful counts. Kills the
/// destination precheck `!=`→`==` inversion (which would refuse every
/// LEGITIMATE move as UnknownDestination).
#[tokio::test]
async fn move_fate_relocates_the_cards_into_the_chosen_destination() {
    let h = seeded_harness().await;
    seed_card(&h, 1, "in_progress", 0).await;
    seed_card(&h, 2, "in_progress", 1).await;

    let success = delete_lane(
        &h.store,
        &h.operator(),
        "general",
        "sandbox",
        "in_progress",
        LaneFate::Move {
            destination: "done",
        },
    )
    .await
    .expect("a move to a surviving destination must succeed");

    assert_eq!(
        (success.moved, success.deleted),
        (2, 0),
        "the move fate must report 2 moved / 0 deleted"
    );
    assert_eq!(
        states_by_number(&h).await,
        vec![(1, "done".to_string()), (2, "done".to_string())],
        "every card of the dying lane must sit in the destination"
    );
}

/// Behaviour 5 — refusal PRECEDENCE (ADR-BOARD-LANE-002 ordering): an ABSENT
/// lane answers the uniform 404 even on a single-lane board, never the
/// LastLane 422. Kills the lane-exists precheck `==`→`!=` inversion, which
/// would misclassify the ghost as existing and answer LastLane.
#[tokio::test]
async fn absent_lane_on_a_single_lane_board_is_not_found_never_last_lane() {
    let h = seeded_harness().await;
    // Shrink the board to ONE lane (fixture precondition, driven-port side).
    sqlx::query("DELETE FROM lanes WHERE project_id = $1 AND slug <> 'backlog'")
        .bind(h.project_id)
        .execute(h.store.pool())
        .await
        .expect("shrink board to one lane");

    let outcome = delete_lane(
        &h.store,
        &h.operator(),
        "general",
        "sandbox",
        "ghost",
        LaneFate::Delete,
    )
    .await;

    assert!(
        matches!(outcome, Err(DeleteLaneError::NotFound)),
        "an absent lane must refuse NotFound (the lane lock comes FIRST), not LastLane; got {outcome:?}"
    );
}
