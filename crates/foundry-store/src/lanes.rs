//! Per-project board lanes — `lanes` repository (board-lane-management).
//!
//! Port SIGNATURES are the DESIGN contract
//! (`docs/feature/board-lane-management/design/component-boundaries.md` §2,
//! ADR-BOARD-LANE-001/002). `list_project_lanes` is real (step 01-01, backed
//! by migration `0015_project_lanes.sql` — lanes DDL + grandfather seed +
//! CHECK drop + DEFAULT drop + composite FK, data-models.md §1-2).
//!
//! `delete_lane_with_fate` contract (ADR-BOARD-LANE-002): ONE transaction —
//! lock lane (`FOR UPDATE`) → last-lane gate → confirm-time membership
//! (`FOR UPDATE`, `position ASC, number DESC`) → fate arm (move: state+position
//! updates + one 0013 `status` event + one outbox row per card; delete:
//! `DELETE … WHERE id = ANY(ids)`, the `delete_issue_cascade` shape) → delete
//! the lane row (the composite FK is the strand-guard) → commit. Bounded
//! internal retry (≤3) on foreign_key_violation / deadlock, re-resolving
//! membership each attempt. Full TOCTOU analysis: data-models.md §5.
//!
//! Step 03-01 implements the transaction skeleton (lock → last-lane gate →
//! membership → lane-row delete) with the fate arms trivial for ZERO cards.
//! The WITH-CARDS fate arms are step 04-01's: until then a confirm reaching a
//! populated lane rolls back with an explicit honest error (never a silent
//! partial apply, never a faked success).

use crate::{Store, StoreError};

/// Creation-seed template — the documented exemption to the
/// no-static-lane-list rule (component-boundaries.md §2): it WRITES lane rows
/// at project creation; it never renders or validates against them.
///
/// The D4 three defaults (02-01): every FRESHLY CREATED project starts with
/// exactly Backlog, In-Progress, Done, in that order. Grandfathered EXISTING
/// projects keep the four lanes migration 0015 seeded — the migration seed is
/// deliberately NOT this constant.
pub(crate) const CREATION_LANE_SEED: &[(&str, &str, i32)] = &[
    ("backlog", "Backlog", 0),
    ("in_progress", "In-Progress", 1),
    ("done", "Done", 2),
];

/// The freshly-inserted issue: its allocated per-project `number` plus the
/// PERSISTED landing `state` — the project's leftmost lane at insert time
/// (D6, component-boundaries.md §2). Carrying the actual landing slug lets
/// every caller echo the truth instead of a hardcoded `"backlog"`
/// (board-lane-management 02-01, ripple surface 6).
#[derive(Debug, Clone)]
pub struct InsertedIssue {
    pub number: i32,
    pub state: String,
}

/// One lane row, board order by `position` (component-boundaries.md §2).
#[derive(Debug, Clone)]
pub struct LaneRow {
    pub id: uuid::Uuid,
    pub project_id: uuid::Uuid,
    pub slug: String,
    pub label: String,
    pub position: i32,
}

/// The operator's card-fate choice for a lane delete (D7).
#[derive(Debug, Clone)]
pub enum LaneDeleteFate<'a> {
    MoveTo { destination_slug: &'a str },
    DeleteCards,
}

/// Outcome of the single-transaction two-fate delete.
#[derive(Debug, Clone)]
pub enum LaneDeleteOutcome {
    /// Lane row removed; counts for the response copy / logging.
    Deleted { moved: u64, deleted: u64 },
    /// No such lane in this project (incl. double-submit race) → uniform 404.
    LaneNotFound,
    /// `count(lanes) == 1` → 422 "A board needs at least one lane".
    LastLane,
    /// fate=MoveTo and destination absent or == dying lane → 422.
    DestinationNotFound,
}

impl Store {
    /// Project's lanes, board order (`ORDER BY position ASC`). Never empty for
    /// a live project (creation seeds three; delete refuses the last).
    pub async fn list_project_lanes(
        &self,
        project_id: uuid::Uuid,
    ) -> Result<Vec<LaneRow>, StoreError> {
        let rows: Vec<(uuid::Uuid, uuid::Uuid, String, String, i32)> = sqlx::query_as(
            "SELECT id, project_id, slug, label, position
               FROM lanes WHERE project_id = $1 ORDER BY position ASC",
        )
        .bind(project_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, project_id, slug, label, position)| LaneRow {
                id,
                project_id,
                slug,
                label,
                position,
            })
            .collect())
    }

    /// Live count of the cards a lane holds — the dialog's advisory copy
    /// (D7: the fate binds at CONFIRM time inside the transaction, never to
    /// this read).
    pub async fn count_issues_in_lane(
        &self,
        project_id: uuid::Uuid,
        lane_slug: &str,
    ) -> Result<i64, StoreError> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM issues WHERE project_id = $1 AND state = $2")
                .bind(project_id)
                .bind(lane_slug)
                .fetch_one(self.pool())
                .await?;
        Ok(count)
    }

    /// ONE transaction: lock lane → last-lane gate → confirm-time membership →
    /// fate arm → delete lane row (FK strand-guard) → commit. See module docs.
    /// Bounded internal retry (≤3) on foreign_key_violation / deadlock,
    /// re-resolving membership each attempt (a card racing into the dying lane
    /// after the membership snapshot blocks the lane-row DELETE with an FK
    /// violation; the retry re-reads it — US-BLM-04 scenario 5).
    pub async fn delete_lane_with_fate(
        &self,
        project_id: uuid::Uuid,
        lane_slug: &str,
        fate: LaneDeleteFate<'_>,
        actor_id: uuid::Uuid,
    ) -> Result<LaneDeleteOutcome, StoreError> {
        const MAX_ATTEMPTS: u32 = 3;
        let mut attempt = 1;
        loop {
            match self
                .try_delete_lane_with_fate(project_id, lane_slug, &fate, actor_id)
                .await
            {
                Err(StoreError::Sqlx(err)) if attempt < MAX_ATTEMPTS && is_fate_retryable(&err) => {
                    attempt += 1;
                }
                outcome => return outcome,
            }
        }
    }

    /// One attempt of the two-fate delete — the full ADR-BOARD-LANE-002
    /// transaction. Every refusal returns BEFORE any write; the implicit
    /// rollback on drop leaves nothing partially applied.
    async fn try_delete_lane_with_fate(
        &self,
        project_id: uuid::Uuid,
        lane_slug: &str,
        fate: &LaneDeleteFate<'_>,
        actor_id: uuid::Uuid,
    ) -> Result<LaneDeleteOutcome, StoreError> {
        // The actor attributes the per-card 0013 status events the move fate
        // writes — a 04-01 concern; the zero-card arms write no events yet.
        let _ = actor_id;
        let mut tx = self.pool().begin().await?;

        // 1. Lock the dying lane. Absent (incl. the double-submit race) →
        //    the uniform non-enumerable 404.
        let lane: Option<(uuid::Uuid,)> =
            sqlx::query_as("SELECT id FROM lanes WHERE project_id = $1 AND slug = $2 FOR UPDATE")
                .bind(project_id)
                .bind(lane_slug)
                .fetch_optional(&mut *tx)
                .await?;
        let Some((lane_id,)) = lane else {
            return Ok(LaneDeleteOutcome::LaneNotFound);
        };

        // 2. Last-lane gate: a board keeps at least one lane (D6).
        let (lane_count,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM lanes WHERE project_id = $1")
                .bind(project_id)
                .fetch_one(&mut *tx)
                .await?;
        if lane_count == 1 {
            return Ok(LaneDeleteOutcome::LastLane);
        }

        // 3. Confirm-time membership — the fate binds to THESE cards (D7),
        //    locked so no member can slip out mid-fate.
        let cards: Vec<(uuid::Uuid, i32)> = sqlx::query_as(
            "SELECT id, number FROM issues WHERE project_id = $1 AND state = $2
              ORDER BY position ASC, number DESC FOR UPDATE",
        )
        .bind(project_id)
        .bind(lane_slug)
        .fetch_all(&mut *tx)
        .await?;

        // 4. Fate arm. The MoveTo destination is validated inside the
        //    transaction even when there is nothing to move (a race can delete
        //    the destination between the dialog read and this confirm).
        if let LaneDeleteFate::MoveTo { destination_slug } = fate {
            let destination: Option<(uuid::Uuid,)> = sqlx::query_as(
                "SELECT id FROM lanes WHERE project_id = $1 AND slug = $2 FOR UPDATE",
            )
            .bind(project_id)
            .bind(destination_slug)
            .fetch_optional(&mut *tx)
            .await?;
            if destination.is_none() || *destination_slug == lane_slug {
                return Ok(LaneDeleteOutcome::DestinationNotFound);
            }
        }
        if !cards.is_empty() {
            // Honest not-yet path (step 04-01 owns the with-cards arms): roll
            // back and fail loudly — NEVER a silent partial apply or a faked
            // success over unmoved/undeleted cards.
            return Err(StoreError::Sqlx(sqlx::Error::Protocol(
                "delete_lane_with_fate: with-cards fate arms are not yet implemented \
                 (board-lane-management 04-01); transaction rolled back"
                    .into(),
            )));
        }
        let (moved, deleted) = (0u64, 0u64);

        // 5. Delete the lane row — the composite FK is the strand-guard: a
        //    card that raced in after step 3 aborts this DELETE with an FK
        //    violation and the bounded retry re-resolves membership.
        sqlx::query("DELETE FROM lanes WHERE id = $1")
            .bind(lane_id)
            .execute(&mut *tx)
            .await?;

        // 6. Commit.
        tx.commit().await?;
        Ok(LaneDeleteOutcome::Deleted { moved, deleted })
    }
}

/// A fate-transaction attempt is worth retrying when the lane-row DELETE hit
/// the composite-FK strand-guard (a card raced into the dying lane —
/// `foreign_key_violation`, 23503) or Postgres broke a deadlock (40P01).
fn is_fate_retryable(err: &sqlx::Error) -> bool {
    err.as_database_error()
        .and_then(|db| db.code())
        .map(|code| code == "23503" || code == "40P01")
        .unwrap_or(false)
}
