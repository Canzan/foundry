//! Tombstone-factory test helper — direct SQL insertion of tombstoned
//! comments at controllable `deleted_at` ages.
//!
//! Per `docs/feature/comment-tombstone-gc/distill/driver.md` § 2a +
//! `wave-decisions.md` D4 = A: the production soft-delete handler always
//! sets `deleted_at = now()`, useless for testing the 90-day threshold.
//! The slice-7 GC scenarios seed tombstoned rows directly via SQL with
//! `deleted_at = now() - interval '<N> days'` to span the boundary.
//!
//! This is a TEST-ONLY FIXTURE helper — not production code. Mirrors
//! slice-3's `pg_backup.rs` shape (small focused module with a narrow
//! direct-SQL surface). NO new crate dep; vanilla `sqlx::query` calls.

#![allow(dead_code)]

use sqlx::PgPool;
use uuid::Uuid;

/// Insert a single tombstoned comment whose `deleted_at` is set to
/// `now() - interval '<deletion_age_days> days'`. Returns the inserted
/// UUID so the calling scenario can reference it (e.g. the admin-undelete
/// CLI scenario needs the UUID to pass as the subprocess argument).
///
/// Callers resolve `issue_id` + `author_id` from the slice-1 Background-
/// seeded state via the in-process pool. `workspace_id` is looked up
/// from the issue row — comments are workspace-scoped per the slice-2
/// schema (FK to workspaces).
///
/// The inserted row carries a synthetic `body_markdown` + `body_html`
/// pair derived from `body`. We do NOT run the production sanitizer
/// against `body` — these rows are tombstoned at insert time and the
/// live-comment renderer will never show them; only the admin-undelete
/// CLI's restored-row assertion (slice-7 WS #7) re-reads the body, and
/// it asserts on substring match in the HTML.
pub async fn insert_tombstoned_comment(
    pool: &PgPool,
    issue_id: Uuid,
    author_id: Uuid,
    body: &str,
    deletion_age_days: i64,
) -> Uuid {
    let comment_id = Uuid::now_v7();
    let workspace_id: (Uuid,) = sqlx::query_as("SELECT workspace_id FROM issues WHERE id = $1")
        .bind(issue_id)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|err| panic!("tombstone_factory: issue {issue_id} not found: {err}"));
    // body_html is a trivial HTML wrapping for the slice-7 WS #7
    // assertion (the issue page renders this raw — we want the
    // substring "abandoned-thought" to land in the rendered DOM).
    let body_html = format!("<p>{}</p>", html_escape(body));
    sqlx::query(
        "INSERT INTO comments
              (id, workspace_id, issue_id, author_id,
               body_markdown, body_html, deleted_at, deleted_by)
          VALUES ($1, $2, $3, $4, $5, $6,
                  now() - ($7 || ' days')::interval,
                  $4)",
    )
    .bind(comment_id)
    .bind(workspace_id.0)
    .bind(issue_id)
    .bind(author_id)
    .bind(body)
    .bind(&body_html)
    .bind(deletion_age_days.to_string())
    .execute(pool)
    .await
    .unwrap_or_else(|err| panic!("tombstone_factory: insert tombstoned comment: {err}"));
    comment_id
}

/// Bulk-insert N tombstoned comments at the same `deleted_at` age.
/// Used by the cap scenario (#3, 11k rows). Pre-generates UUIDs in
/// Rust (uuidv7) and ships them through an `INSERT ... SELECT ...
/// FROM UNNEST(...)` so 11k rows complete in ~1-3s rather than ~30s
/// for 11k individual round trips.
///
/// We pre-generate the UUIDs application-side (NOT `gen_random_uuid()`
/// in SQL) because the testcontainers default Postgres image (11-alpine)
/// does not ship the pgcrypto extension and the project's migration
/// policy avoids installing extensions (slice 1, ADR-001 / 0001_init.sql
/// line 5: "We do NOT install the uuid-ossp extension"). Slice 7 honors
/// the same constraint.
///
/// Returns the inserted UUIDs in insertion order (length = count).
pub async fn bulk_insert_tombstoned_comments(
    pool: &PgPool,
    issue_id: Uuid,
    author_id: Uuid,
    count: u64,
    deletion_age_days: i64,
) -> Vec<Uuid> {
    let workspace_id: (Uuid,) = sqlx::query_as("SELECT workspace_id FROM issues WHERE id = $1")
        .bind(issue_id)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|err| panic!("tombstone_factory: issue {issue_id} not found: {err}"));

    // Pre-generate UUIDs app-side; Postgres UNNEST'es them into the
    // INSERT. This is the standard sqlx bulk-insert idiom that
    // tolerates the absence of pgcrypto.
    let ids: Vec<Uuid> = (0..count).map(|_| Uuid::now_v7()).collect();
    sqlx::query(
        "INSERT INTO comments
              (id, workspace_id, issue_id, author_id,
               body_markdown, body_html, deleted_at, deleted_by)
          SELECT id, $2, $3, $4,
                 'bulk-tombstone',
                 '<p>bulk-tombstone</p>',
                 now() - ($5 || ' days')::interval,
                 $4
            FROM UNNEST($1::uuid[]) AS t(id)",
    )
    .bind(&ids)
    .bind(workspace_id.0)
    .bind(issue_id)
    .bind(author_id)
    .bind(deletion_age_days.to_string())
    .execute(pool)
    .await
    .unwrap_or_else(|err| panic!("tombstone_factory: bulk insert: {err}"));
    ids
}

/// Count tombstoned comments on a given issue, optionally filtered to
/// those older than a threshold (days). Used by Then-step assertions
/// that avoid a HTTP round-trip — cheaper than re-rendering the issue
/// page when the scenario only cares about the row count.
///
/// `older_than_days = None`  → count all tombstones on this issue.
/// `older_than_days = Some(d)` → count tombstones whose `deleted_at <
/// now() - interval 'd days'`.
pub async fn count_tombstoned_comments_on_issue(
    pool: &PgPool,
    issue_id: Uuid,
    older_than_days: Option<i64>,
) -> u64 {
    let row: (i64,) = match older_than_days {
        None => sqlx::query_as(
            "SELECT count(*)::bigint FROM comments
              WHERE issue_id = $1 AND deleted_at IS NOT NULL",
        )
        .bind(issue_id)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|err| panic!("tombstone_factory: count: {err}")),
        Some(days) => sqlx::query_as(
            "SELECT count(*)::bigint FROM comments
              WHERE issue_id = $1
                AND deleted_at IS NOT NULL
                AND deleted_at < now() - ($2 || ' days')::interval",
        )
        .bind(issue_id)
        .bind(days.to_string())
        .fetch_one(pool)
        .await
        .unwrap_or_else(|err| panic!("tombstone_factory: count older_than: {err}")),
    };
    row.0.max(0) as u64
}

/// Minimal HTML escape sufficient for the slice-7 WS #7 body content
/// (`abandoned-thought` — no special chars). We escape the four
/// dangerous-in-HTML characters so accidental special-char content
/// in a scenario body doesn't break the DOM.
fn html_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}
