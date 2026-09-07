//! issue-card-delete 01-01 — `issue_delete`, the ONE hard-delete primitive,
//! pinned at the store boundary against a real Postgres (@real-io).
//!
//! WHY-NEW-FILE: crates/foundry-store/tests/issue_delete.rs
//!   CLOSEST-EXISTING: crates/foundry-store/tests/delete_lane_with_fate.rs
//!   EXTENSION-COST: each file under `tests/` is its own `cargo test` binary;
//!     folding these in would put the issue-delete primitive's coverage inside
//!     the lane-fate target, so `--test delete_lane_with_fate` would boot and
//!     migrate a container for issue-delete work and vice versa.
//!   PARALLEL-RATIONALE: different test-binary lifecycle — the two suites own
//!     independent containers and independent `cargo test --test <name>`
//!     selection, which a shared file structurally cannot give.
//!
//! What is pinned here (ADR-ISSUE-DELETE-001, DDD-1/DDD-3/DDD-4/DDD-10/DDD-11):
//!
//! - The delete is HARD and the CASCADE is the schema's, not the
//!   application's — comments (0004), attachments (0005) and change events
//!   (0013) vanish with the issue because their FKs say so. Only a real
//!   Postgres can prove that; an in-memory double would be asserting our own
//!   assumption back at us.
//! - Siblings are byte-identical afterwards — the `ANY($1)` id set is the
//!   whole blast radius.
//! - `Ok(0)` is the not-there case and writes NOTHING (DDD-10).
//! - The batch arm rides the CALLER's transaction (DDD-4) — this slice's
//!   learning hypothesis. Proven by rolling the caller's transaction back and
//!   observing the delete undone.
//!
//! Test budget: 4 behaviours (single-card cascade + sibling isolation; the
//! vanished-id zero; the caller-transaction seam — which step 03-01 extended
//! to cover the emit riding that same transaction; and the partially-vanished
//! id set) → 4 integration tests. Per the layered discipline, integration
//! tests verify WIRING with one representative call, so no property framing
//! here; the primitive's input-space properties are owned by the acceptance
//! lane in slice 02.
//!
//! Step 03-01 added the outbox emit. What it did NOT have to do is rewrite a
//! single assertion 01-01 wrote: 01-01 deliberately asserted an empty outbox
//! only on the `Ok(0)` path, where DDD-10 makes it permanent. The two things
//! 03-01 adds here are the two the acceptance lane structurally cannot reach:
//!
//! - the emit lives INSIDE the caller's transaction, not on a side
//!   connection — proven by counting the rows through the open transaction and
//!   then rolling it back;
//! - the emit is driven by `RETURNING`, not by `rows_affected()` — proven by
//!   handing the primitive an id that vanished out-of-band and watching it
//!   announce the one card it actually removed and no other.

use foundry_store::issue_delete::{delete_issues_with_outbox, IssueDeleteContext};
use foundry_store::Store;
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
    foundry_store::run_migrations(&pool)
        .await
        .expect("run migrations");
    Store::from_pool(pool)
}

/// A seeded project plus the actor who owns everything in it.
struct Seed {
    workspace_id: uuid::Uuid,
    project_id: uuid::Uuid,
    actor_id: uuid::Uuid,
}

impl Seed {
    fn context(&self) -> IssueDeleteContext {
        IssueDeleteContext {
            workspace_id: self.workspace_id,
            project_id: self.project_id,
            key_prefix: "GEN".to_string(),
        }
    }
}

/// Seed a workspace + user + team + project with the four grandfathered lanes.
async fn seed_project(store: &Store) -> Seed {
    let workspace_id = uuid::Uuid::now_v7();
    let actor_id = uuid::Uuid::now_v7();
    let team_id = uuid::Uuid::now_v7();
    let project_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Acme')")
        .bind(workspace_id)
        .execute(store.pool())
        .await
        .expect("insert workspace");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, 'op@delete.test', 'op@delete.test', 'Operator', 'x')",
    )
    .bind(actor_id)
    .execute(store.pool())
    .await
    .expect("insert user");
    sqlx::query(
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'General', 'general')",
    )
    .bind(team_id)
    .bind(workspace_id)
    .execute(store.pool())
    .await
    .expect("insert team");
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, 'Sandbox', 'sandbox', 'GEN')",
    )
    .bind(project_id)
    .bind(team_id)
    .bind(workspace_id)
    .execute(store.pool())
    .await
    .expect("insert project");
    sqlx::query(
        "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
         SELECT gen_random_uuid(), $1, $2, v.slug, v.label, v.position
           FROM (VALUES ('backlog', 'Backlog', 0), ('todo', 'Todo', 1),
                        ('in_progress', 'In-Progress', 2), ('done', 'Done', 3))
                AS v (slug, label, position)",
    )
    .bind(project_id)
    .bind(workspace_id)
    .execute(store.pool())
    .await
    .expect("seed lanes");
    Seed {
        workspace_id,
        project_id,
        actor_id,
    }
}

/// Seed one issue in `todo` and return its id.
async fn seed_issue(store: &Store, seed: &Seed, number: i32) -> uuid::Uuid {
    let issue_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md,
                             state, position, author_id)
              VALUES ($1, $2, $3, $4, 'seed', '', 'todo', $4, $5)",
    )
    .bind(issue_id)
    .bind(seed.project_id)
    .bind(seed.workspace_id)
    .bind(number)
    .bind(seed.actor_id)
    .execute(store.pool())
    .await
    .expect("insert issue");
    issue_id
}

/// Hang one comment, one attachment and one change event off an issue, so the
/// schema's `ON DELETE CASCADE` has something to carry away.
async fn seed_children(store: &Store, seed: &Seed, issue_id: uuid::Uuid) {
    sqlx::query(
        "INSERT INTO comments (id, workspace_id, issue_id, author_id, body_markdown, body_html)
              VALUES ($1, $2, $3, $4, 'a comment', '<p>a comment</p>')",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(seed.workspace_id)
    .bind(issue_id)
    .bind(seed.actor_id)
    .execute(store.pool())
    .await
    .expect("insert comment");
    sqlx::query(
        "INSERT INTO issue_attachments (id, issue_id, workspace_id, uploader_id, filename,
                                        content_type, size_bytes, sha256_hex, content)
              VALUES ($1, $2, $3, $4, 'note.txt', 'text/plain', 3, $5, $6)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(issue_id)
    .bind(seed.workspace_id)
    .bind(seed.actor_id)
    .bind("0".repeat(64))
    .bind(vec![1u8, 2, 3])
    .execute(store.pool())
    .await
    .expect("insert attachment");
    sqlx::query(
        "INSERT INTO issue_change_events (id, workspace_id, project_id, issue_id, actor_id,
                                          field, old_value, new_value)
              VALUES ($1, $2, $3, $4, $5, 'status', 'backlog', 'todo')",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(seed.workspace_id)
    .bind(seed.project_id)
    .bind(issue_id)
    .bind(seed.actor_id)
    .execute(store.pool())
    .await
    .expect("insert change event");
}

async fn count(store: &Store, sql: &str, issue_id: uuid::Uuid) -> i64 {
    let row: (i64,) = sqlx::query_as(sql)
        .bind(issue_id)
        .fetch_one(store.pool())
        .await
        .expect("count query");
    row.0
}

async fn issue_exists(store: &Store, issue_id: uuid::Uuid) -> bool {
    count(store, "SELECT count(*) FROM issues WHERE id = $1", issue_id).await == 1
}

async fn child_counts(store: &Store, issue_id: uuid::Uuid) -> (i64, i64, i64) {
    (
        count(
            store,
            "SELECT count(*) FROM comments WHERE issue_id = $1",
            issue_id,
        )
        .await,
        count(
            store,
            "SELECT count(*) FROM issue_attachments WHERE issue_id = $1",
            issue_id,
        )
        .await,
        count(
            store,
            "SELECT count(*) FROM issue_change_events WHERE issue_id = $1",
            issue_id,
        )
        .await,
    )
}

async fn outbox_len(store: &Store) -> i64 {
    let row: (i64,) = sqlx::query_as("SELECT count(*) FROM outbox")
        .fetch_one(store.pool())
        .await
        .expect("count outbox");
    row.0
}

/// Every `IssueDeleted` payload written so far, oldest first. The announcement
/// oracle counts BOTH ways — a missing row and a spurious row are different
/// bugs, and a return value of `1` distinguishes neither.
async fn announced_deletes(store: &Store) -> Vec<serde_json::Value> {
    sqlx::query_as::<_, (serde_json::Value,)>(
        "SELECT payload FROM outbox WHERE event_type = 'IssueDeleted' ORDER BY id ASC",
    )
    .fetch_all(store.pool())
    .await
    .expect("read IssueDeleted payloads")
    .into_iter()
    .map(|(payload,)| payload)
    .collect()
}

/// The single-card arm owns its own transaction, removes exactly one `issues`
/// row, and the schema carries its comment, attachment and change event away
/// with it. The sibling card and the sibling's children are untouched — the
/// id set is the whole blast radius (DDD-11: no application-level child
/// delete is written, and none is needed).
#[tokio::test]
async fn deleting_one_card_cascades_its_children_and_leaves_its_sibling_intact() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let seed = seed_project(&store).await;

    let doomed = seed_issue(&store, &seed, 10).await;
    let sibling = seed_issue(&store, &seed, 20).await;
    seed_children(&store, &seed, doomed).await;
    seed_children(&store, &seed, sibling).await;

    let deleted = store
        .delete_issue_with_outbox(&seed.context(), doomed, 10)
        .await
        .expect("single-card delete must not error");

    assert_eq!(deleted, 1, "exactly one issues row must be removed");
    assert!(
        !issue_exists(&store, doomed).await,
        "the named issue must be gone — the delete is hard, not a tombstone"
    );
    assert_eq!(
        child_counts(&store, doomed).await,
        (0, 0, 0),
        "comments, attachments and change events must cascade away with the issue"
    );
    assert!(
        issue_exists(&store, sibling).await,
        "a sibling issue must survive a delete that did not name it"
    );
    assert_eq!(
        child_counts(&store, sibling).await,
        (1, 1, 1),
        "the sibling's own children must be untouched"
    );
}

/// `Ok(0)` is the not-there case: an id that resolved and then vanished (or
/// never existed) deletes nothing, touches nothing, and — DDD-10 — announces
/// nothing. A card that is not there must never be falsely announced.
#[tokio::test]
async fn deleting_a_vanished_card_returns_zero_and_writes_nothing() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let seed = seed_project(&store).await;

    let bystander = seed_issue(&store, &seed, 10).await;
    seed_children(&store, &seed, bystander).await;
    let outbox_before = outbox_len(&store).await;

    let deleted = store
        .delete_issue_with_outbox(&seed.context(), uuid::Uuid::now_v7(), 99)
        .await
        .expect("a vanished card is Ok(0), never an error");

    assert_eq!(deleted, 0, "nothing was there, so nothing was deleted");
    assert_eq!(
        outbox_len(&store).await,
        outbox_before,
        "Ok(0) must write NO outbox row (DDD-10) — never announce a card that was not there"
    );
    assert!(
        issue_exists(&store, bystander).await,
        "an unnamed issue must survive a miss"
    );
    assert_eq!(
        child_counts(&store, bystander).await,
        (1, 1, 1),
        "a miss must not disturb anyone's children"
    );
}

/// The slice's learning hypothesis (DDD-4): ONE primitive serves BOTH
/// transaction owners. The batch arm takes the caller's `&mut Transaction`, so
/// it commits — or unwinds — with the caller's own work. Rolling the caller's
/// transaction back must undo the delete entirely; that is what will let slice
/// 03's `DeleteCards` fate ride the fate transaction unchanged.
#[tokio::test]
async fn the_batch_arm_rides_the_callers_transaction_and_unwinds_with_it() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let seed = seed_project(&store).await;

    let first = seed_issue(&store, &seed, 10).await;
    let second = seed_issue(&store, &seed, 20).await;
    let bystander = seed_issue(&store, &seed, 30).await;
    seed_children(&store, &seed, first).await;

    // The caller owns the transaction; the primitive borrows it.
    let mut tx = store
        .pool()
        .begin()
        .await
        .expect("begin caller transaction");
    let deleted = delete_issues_with_outbox(&mut tx, &seed.context(), &[(first, 10), (second, 20)])
        .await
        .expect("batch delete must not error on the caller's transaction");
    assert_eq!(
        deleted, 2,
        "both named cards must be deleted in one statement"
    );

    // Read the announcement THROUGH the still-open transaction. A side
    // connection cannot see it if the emit is where it belongs — and if the
    // emit were taken on a pool connection of its own, this would read 0 here
    // and the rollback below would leave the rows stranded, announcing a
    // delete that never happened.
    let announced_inside: (i64,) =
        sqlx::query_as("SELECT count(*) FROM outbox WHERE event_type = 'IssueDeleted'")
            .fetch_one(&mut *tx)
            .await
            .expect("count announcements inside the caller's transaction");
    assert_eq!(
        announced_inside.0, 2,
        "the emit must happen INSIDE the caller's transaction — one row per card \
         actually deleted, visible only from within it (AC-3.1)"
    );

    tx.rollback().await.expect("roll the caller's work back");

    assert_eq!(
        announced_deletes(&store).await.len(),
        0,
        "a rolled-back delete takes its announcement with it — a card still on \
         the board must never have been announced as gone (AC-3.7)"
    );

    assert!(
        issue_exists(&store, first).await && issue_exists(&store, second).await,
        "rolling the CALLER's transaction back must undo the delete — the primitive \
         must not own a transaction of its own (DDD-4)"
    );
    assert_eq!(
        child_counts(&store, first).await,
        (1, 1, 1),
        "the cascaded children must come back with their issue"
    );
    assert!(
        issue_exists(&store, bystander).await,
        "an unnamed issue is outside the batch's blast radius"
    );
}

/// `rows_affected()` says how many rows went, never WHICH. The primitive is
/// handed two ids, one of which vanished out-of-band between the caller's
/// resolution and the delete. Exactly one row goes, so exactly ONE card is
/// announced — the one Postgres actually removed. The vanished card must be
/// silently absent rather than falsely announced (DDD-3/DDD-10). No acceptance
/// scenario can stage this race; only the store can.
#[tokio::test]
async fn only_the_rows_postgres_actually_removed_are_announced() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let seed = seed_project(&store).await;

    let present = seed_issue(&store, &seed, 10).await;
    let vanishing = seed_issue(&store, &seed, 20).await;
    seed_children(&store, &seed, present).await;

    // Someone else deletes it after the caller resolved it and before the
    // primitive's DELETE lands.
    sqlx::query("DELETE FROM issues WHERE id = $1")
        .bind(vanishing)
        .execute(store.pool())
        .await
        .expect("delete the vanishing card out-of-band");

    let mut tx = store
        .pool()
        .begin()
        .await
        .expect("begin caller transaction");
    let deleted =
        delete_issues_with_outbox(&mut tx, &seed.context(), &[(present, 10), (vanishing, 20)])
            .await
            .expect("a half-vanished id set is not an error");
    tx.commit().await.expect("commit the caller's work");

    assert_eq!(
        deleted, 1,
        "only one of the two ids was still there to delete"
    );

    let announced = announced_deletes(&store).await;
    assert_eq!(
        announced.len(),
        1,
        "one announcement per row ACTUALLY deleted — announcing the vanished \
         card too would tell every open board a card went that was already gone"
    );
    let payload = &announced[0];
    assert_eq!(
        payload.get("key").and_then(|v| v.as_str()),
        Some("GEN-10"),
        "the payload names the card that went, composed from the key_prefix the \
         caller supplied and the number RETURNING gave back — after the DELETE \
         there is no row left to compose it from"
    );
    assert_eq!(
        payload.get("issue_id").and_then(|v| v.as_str()),
        Some(present.to_string().as_str()),
        "the announcement must identify the card that went"
    );
    assert_eq!(
        payload.get("number").and_then(|v| v.as_i64()),
        Some(10),
        "the number must come from the row Postgres removed, not from the request"
    );
    assert_eq!(
        payload.get("project_id").and_then(|v| v.as_str()),
        Some(seed.project_id.to_string().as_str()),
        "project_id is the tenancy filter the SSE handler applies (AC-3.5)"
    );
    assert_eq!(
        payload.get("workspace_id").and_then(|v| v.as_str()),
        Some(seed.workspace_id.to_string().as_str()),
        "workspace_id rides along exactly as every other event carries it"
    );
    assert_eq!(
        payload.get("deleted").and_then(|v| v.as_bool()),
        Some(true),
        "the tombstone marker ADR-008 added — a receiver matching on payload \
         shape can tell a removal from an update without parsing event_type"
    );
    assert!(
        !issue_exists(&store, present).await,
        "the card that was announced must actually be gone"
    );
}
