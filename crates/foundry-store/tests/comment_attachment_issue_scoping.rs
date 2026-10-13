//! fix-issue-attachment-scoping — the store's issue-scoped comment and
//! attachment primitives, pinned at the store boundary against a real
//! Postgres (@real-io).
//!
//! WHY-NEW-FILE: crates/foundry-store/tests/comment_attachment_issue_scoping.rs
//!   CLOSEST-EXISTING: crates/foundry-app/tests/comment_scoping.rs
//!   EXTENSION-COST: the app suite exercises these primitives through the
//!     router, but a store mutant is judged by the store crate's own tests;
//!     only a `foundry-store` test binary can show the store returning the
//!     row, or changing it, for the issue the comment or attachment is on.
//!   PARALLEL-RATIONALE: different crate, different test-binary lifecycle.
//!
//! What is pinned: each lookup returns the row through its own issue and
//! nothing through a sibling issue; the comment update and soft-delete report
//! `true`, change the row and write their outbox event through its own issue.

use foundry_store::Store;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};

const BODY: &str = "rotate the zebra-42 signing key";
const EDITED: &str = "rotated the zebra-42 signing key";
const AUTHOR_EMAIL: &str = "yuki@scoping.test";
const FILE_BYTES: &[u8] = b"\x89PNG\r\n\x1a\nissue-scoped-bytes";

/// One workspace, one project with issues 1 and 2; a comment and an
/// attachment on issue 1.
struct World {
    store: Store,
    _pg: ContainerAsync<Postgres>,
    workspace_id: uuid::Uuid,
    author_id: uuid::Uuid,
    issue_1: uuid::Uuid,
    issue_2: uuid::Uuid,
    comment_id: uuid::Uuid,
    attachment_id: uuid::Uuid,
}

async fn exec(store: &Store, sql: &str, binds: &[uuid::Uuid], what: &str) {
    let mut q = sqlx::query(sql);
    for b in binds {
        q = q.bind(*b);
    }
    q.execute(store.pool()).await.expect(what);
}

async fn world() -> World {
    let pg = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let host = pg.get_host().await.expect("container host");
    let port = pg.get_host_port_ipv4(5432).await.expect("container port");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&format!(
            "postgres://postgres:postgres@{host}:{port}/postgres"
        ))
        .await
        .expect("connect pool");
    foundry_store::run_migrations(&pool)
        .await
        .expect("run migrations");
    let store = Store::from_pool(pool);

    let workspace_id = uuid::Uuid::now_v7();
    let author_id = uuid::Uuid::now_v7();
    let team_id = uuid::Uuid::now_v7();
    let project_id = uuid::Uuid::now_v7();
    exec(
        &store,
        "INSERT INTO workspaces (id, name) VALUES ($1, 'Acme')",
        &[workspace_id],
        "seed workspace",
    )
    .await;
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, 'Yuki', 'x')",
    )
    .bind(author_id)
    .bind(AUTHOR_EMAIL)
    .execute(store.pool())
    .await
    .expect("seed user");
    exec(
        &store,
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Backend', 'backend')",
        &[team_id, workspace_id],
        "seed team",
    )
    .await;
    exec(
        &store,
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, 'Api', 'api', 'AUTH')",
        &[project_id, team_id, workspace_id],
        "seed project",
    )
    .await;
    exec(
        &store,
        "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
         SELECT gen_random_uuid(), $1, $2, v.slug, v.label, v.position
           FROM (VALUES ('backlog', 'Backlog', 0), ('todo', 'Todo', 1),
                        ('in_progress', 'In-Progress', 2), ('done', 'Done', 3))
                AS v (slug, label, position)",
        &[project_id, workspace_id],
        "seed lanes",
    )
    .await;
    let mut issues = Vec::new();
    for number in [1, 2] {
        let issue_id = uuid::Uuid::now_v7();
        sqlx::query(
            "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md,
                                 state, position, author_id)
                  VALUES ($1, $2, $3, $4, 'seed', '', 'todo', $4, $5)",
        )
        .bind(issue_id)
        .bind(project_id)
        .bind(workspace_id)
        .bind(number)
        .bind(author_id)
        .execute(store.pool())
        .await
        .expect("seed issue");
        issues.push(issue_id);
    }
    let (issue_1, issue_2) = (issues[0], issues[1]);

    let comment_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO comments (id, workspace_id, issue_id, author_id, body_markdown, body_html)
              VALUES ($1, $2, $3, $4, $5, $5)",
    )
    .bind(comment_id)
    .bind(workspace_id)
    .bind(issue_1)
    .bind(author_id)
    .bind(BODY)
    .execute(store.pool())
    .await
    .expect("seed comment");

    let attachment_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO issue_attachments (id, issue_id, workspace_id, uploader_id, filename,
                                        content_type, size_bytes, sha256_hex, content)
              VALUES ($1, $2, $3, $4, 'screenshot.png', 'image/png', $5, $6, $7)",
    )
    .bind(attachment_id)
    .bind(issue_1)
    .bind(workspace_id)
    .bind(author_id)
    .bind(FILE_BYTES.len() as i64)
    .bind("0".repeat(64))
    .bind(FILE_BYTES.to_vec())
    .execute(store.pool())
    .await
    .expect("seed attachment");

    World {
        store,
        _pg: pg,
        workspace_id,
        author_id,
        issue_1,
        issue_2,
        comment_id,
        attachment_id,
    }
}

/// The stored comment: its markdown and whether it is tombstoned.
async fn stored_comment(w: &World) -> (String, bool) {
    sqlx::query_as("SELECT body_markdown, deleted_at IS NOT NULL FROM comments WHERE id = $1")
        .bind(w.comment_id)
        .fetch_one(w.store.pool())
        .await
        .expect("the comment row")
}

/// The comment ids named by outbox events of `event_type`.
async fn outbox_comment_ids(w: &World, event_type: &str) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT payload->>'comment_id' FROM outbox WHERE event_type = $1 ORDER BY id",
    )
    .bind(event_type)
    .fetch_all(w.store.pool())
    .await
    .expect("the outbox rows")
}

#[tokio::test]
async fn a_comment_is_found_through_its_own_issue_and_not_its_sibling() {
    let w = world().await;

    let found = w
        .store
        .find_comment_for_issue(w.comment_id, w.issue_1)
        .await
        .expect("the lookup runs")
        .expect("the comment is found through its own issue");
    assert_eq!(found.id, w.comment_id);
    assert_eq!(found.issue_id, w.issue_1);
    assert_eq!(found.author_id, w.author_id);
    assert_eq!(found.body_markdown, BODY);
    assert!(!found.deleted, "a live comment is not deleted");

    let sibling = w
        .store
        .find_comment_for_issue(w.comment_id, w.issue_2)
        .await
        .expect("the lookup runs");
    assert!(sibling.is_none(), "nothing through the sibling issue");
}

#[tokio::test]
async fn an_attachment_is_found_through_its_own_issue_and_not_its_sibling() {
    let w = world().await;

    let found = w
        .store
        .find_attachment_for_issue(w.attachment_id, w.issue_1)
        .await
        .expect("the lookup runs")
        .expect("the attachment is found through its own issue");
    assert_eq!(found.id, w.attachment_id);
    assert_eq!(found.issue_id, w.issue_1);
    assert_eq!(found.workspace_id, w.workspace_id);
    assert_eq!(found.filename, "screenshot.png");
    assert_eq!(found.content_type, "image/png");
    assert_eq!(found.size_bytes, FILE_BYTES.len() as i64);
    assert_eq!(found.content, FILE_BYTES, "the stored bytes come back");

    let sibling = w
        .store
        .find_attachment_for_issue(w.attachment_id, w.issue_2)
        .await
        .expect("the lookup runs");
    assert!(sibling.is_none(), "nothing through the sibling issue");
}

#[tokio::test]
async fn updating_a_comment_through_its_own_issue_changes_it_and_announces_the_edit() {
    let w = world().await;

    let updated = w
        .store
        .update_comment_with_outbox(
            w.workspace_id,
            w.issue_1,
            w.comment_id,
            EDITED,
            EDITED,
            w.author_id,
            AUTHOR_EMAIL,
        )
        .await
        .expect("the update runs");

    assert!(updated, "the comment's own issue matches");
    assert_eq!(stored_comment(&w).await, (EDITED.to_string(), false));
    assert_eq!(
        outbox_comment_ids(&w, "CommentEdited").await,
        vec![w.comment_id.to_string()],
        "one CommentEdited event names the comment"
    );
}

#[tokio::test]
async fn soft_deleting_a_comment_through_its_own_issue_tombstones_it_and_announces_the_delete() {
    let w = world().await;

    let deleted = w
        .store
        .soft_delete_comment_with_outbox(w.workspace_id, w.issue_1, w.comment_id, w.author_id)
        .await
        .expect("the soft-delete runs");

    assert!(deleted, "the comment's own issue matches");
    assert_eq!(stored_comment(&w).await, (BODY.to_string(), true));
    assert_eq!(
        outbox_comment_ids(&w, "CommentDeleted").await,
        vec![w.comment_id.to_string()],
        "one CommentDeleted event names the comment"
    );
}
