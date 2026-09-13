//! Issue deletion — the ONE primitive every issue delete routes through.
//!
//! The signatures are the DESIGN port shapes (feature-delta DDD-1/DDD-3/DDD-4,
//! `adr-issue-delete-001-one-hard-delete-primitive.md`).
//!
//! The shape this module honours:
//!
//! - `delete_issues_with_outbox` takes the CALLER's transaction, so both
//!   transaction owners (the single-card delete below, and `lanes.rs`'s
//!   `DeleteCards` fate) commit atomically with their own work.
//! - It performs ZERO lookups: the caller hands it a fully-resolved
//!   [`IssueDeleteContext`]. Both callers already hold one.
//! - It emits one `IssueDeleted` outbox row per row ACTUALLY deleted, never
//!   one per card requested (slice 03 — step 03-01). `rows_affected()` says
//!   how many rows went, never WHICH, so the emit is driven by
//!   `DELETE ... RETURNING id, number` — a card that vanished between
//!   resolution and delete is silently absent from the announcement rather
//!   than falsely announced. The insert rides the caller's transaction, so a
//!   committed delete cannot fail to announce itself and a rolled-back one
//!   announces nothing (AC-3.1/AC-3.7).
//! - The payload uses ONLY fields `EventPayload` already declares: this is a
//!   zero-field addition (realtime-roadmap invariant 4 / DDD-12).
//!   `schema_version` stays 1 and `foundry-realtime` is untouched. `key` is
//!   composed as `{key_prefix}-{number}` from the caller's context plus the
//!   number `RETURNING` hands back — after the delete there is no row left to
//!   compose it from.
//! - The schema's `ON DELETE CASCADE` (migrations 0004/0005/0013) carries
//!   comments, attachments and change events. No child delete is written here.
//! - No migration: this module adds no column and no table.

use crate::StoreError;

/// Everything the emit needs, resolved by the caller before the transaction
/// opens. Holding this off the primitive is what keeps it lookup-free.
#[derive(Debug, Clone)]
pub struct IssueDeleteContext {
    pub workspace_id: uuid::Uuid,
    pub project_id: uuid::Uuid,
    /// The project's key prefix (e.g. `AUTH`) — composes the payload's `key`
    /// as `{key_prefix}-{number}`, exactly as `move_cards_to_destination` does.
    pub key_prefix: String,
}

/// Delete the named issues on the caller's transaction and announce each one
/// actually removed. THE shared primitive (DDD-1/DDD-2): the single-card path
/// and the lane `DeleteCards` fate are its only two callers.
///
/// `cards` is `(issue_id, number)` — the number rides along so the payload's
/// `key` needs no second read.
///
/// Returns the number of rows deleted, which is also the number of
/// `IssueDeleted` outbox rows written.
pub async fn delete_issues_with_outbox(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ctx: &IssueDeleteContext,
    cards: &[(uuid::Uuid, i32)],
) -> Result<u64, StoreError> {
    if cards.is_empty() {
        return Ok(0);
    }
    let ids: Vec<uuid::Uuid> = cards.iter().map(|(id, _)| *id).collect();
    let removed: Vec<(uuid::Uuid, i32)> =
        sqlx::query_as("DELETE FROM issues WHERE id = ANY($1) RETURNING id, number")
            .bind(&ids)
            .fetch_all(&mut **tx)
            .await?;

    for (issue_id, number) in &removed {
        announce_deleted(tx, ctx, *issue_id, *number).await?;
    }
    Ok(removed.len() as u64)
}

/// One `IssueDeleted` row for one card that is already gone from `issues`.
/// Mirrors `lanes.rs::move_cards_to_destination`'s emit shape, with
/// `deleted: true` in place of `state`/`author_id`.
async fn announce_deleted(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ctx: &IssueDeleteContext,
    issue_id: uuid::Uuid,
    number: i32,
) -> Result<(), StoreError> {
    let key_prefix = &ctx.key_prefix;
    let payload = serde_json::json!({
        "issue_id": issue_id,
        "project_id": ctx.project_id,
        "workspace_id": ctx.workspace_id,
        "number": number,
        "key": format!("{key_prefix}-{number}"),
        "deleted": true,
    });
    sqlx::query("INSERT INTO outbox (event_type, payload) VALUES ('IssueDeleted', $1)")
        .bind(payload)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

impl crate::Store {
    /// Single-card delete — owns its own transaction and delegates the work to
    /// [`delete_issues_with_outbox`] (DDD-4: one primitive, two transaction
    /// owners).
    ///
    /// `Ok(0)` means the issue was not there — the handler maps that to the
    /// uniform non-enumerable 404, and NO outbox row is written (DDD-10).
    pub async fn delete_issue_with_outbox(
        &self,
        ctx: &IssueDeleteContext,
        issue_id: uuid::Uuid,
        number: i32,
    ) -> Result<u64, StoreError> {
        let mut tx = self.pool().begin().await?;
        let deleted = delete_issues_with_outbox(&mut tx, ctx, &[(issue_id, number)]).await?;
        tx.commit().await?;
        Ok(deleted)
    }
}
