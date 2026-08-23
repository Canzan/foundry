//! Per-project board lanes — `lanes` repository (board-lane-management).
//!
//! Port SIGNATURES are the DESIGN contract
//! (`docs/feature/board-lane-management/design/component-boundaries.md` §2,
//! ADR-BOARD-LANE-001/002). `list_project_lanes` is real (step 01-01, backed
//! by migration `0015_project_lanes.sql` — lanes DDL + grandfather seed +
//! CHECK drop + DEFAULT drop + composite FK, data-models.md §1-2).
//!
//! Still RED-scaffolded (DISTILL, Mandate 7 / ADR-025) — the body panics with
//! a `RED scaffold` marker so the acceptance suite classifies failures as
//! MISSING_FUNCTIONALITY (RED), never as an import/wiring error (BROKEN):
//!   - [`Store::delete_lane_with_fate`] (slice 03/04);
//!
//! and DELIVER later steps own:
//!   - lane seeding inside `insert_project` (slice 02; the creation seed
//!     constant lives HERE, the documented exemption to the
//!     no-static-lane-list rule);
//!   - leftmost-lane resolution inside `insert_issue_with_outbox` (return type
//!     grows to carry the ACTUAL landing state — ripple surface 6).
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
//! SCAFFOLD: true

use crate::{Store, StoreError};

/// Creation-seed template — the documented exemption to the
/// no-static-lane-list rule (component-boundaries.md §2): it WRITES lane rows
/// at project creation; it never renders or validates against them.
///
/// 01-01 bridge shape: the four grandfathered lanes, so every pre-BLM suite
/// (us-07's four-column board, us-08's Backlog landing) stays byte-green while
/// boards render from lane ROWS. 02-01 narrows this to the D4 three
/// (backlog, in_progress, done) together with its own @us-blm-02 scenarios.
pub(crate) const CREATION_LANE_SEED: &[(&str, &str, i32)] = &[
    ("backlog", "Backlog", 0),
    ("todo", "Todo", 1),
    ("in_progress", "In-Progress", 2),
    ("done", "Done", 3),
];

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

    /// ONE transaction: lock lane → last-lane gate → confirm-time membership →
    /// fate arm → delete lane row (FK strand-guard) → commit. See module docs.
    pub async fn delete_lane_with_fate(
        &self,
        _project_id: uuid::Uuid,
        _lane_slug: &str,
        _fate: LaneDeleteFate<'_>,
        _actor_id: uuid::Uuid,
    ) -> Result<LaneDeleteOutcome, StoreError> {
        panic!("Store::delete_lane_with_fate not yet implemented — RED scaffold (board-lane-management)")
    }
}
