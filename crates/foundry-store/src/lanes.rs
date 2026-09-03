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

use crate::{IssueInsertError, Store, StoreError};

/// Creation-seed template — the documented exemption to the
/// no-static-lane-list rule (component-boundaries.md §2): it WRITES lane rows
/// at project creation; it never renders or validates against them.
///
/// The D4 three defaults (02-01): every FRESHLY CREATED project starts with
/// exactly Backlog, In-Progress, Done, in that order. Grandfathered EXISTING
/// projects keep the four lanes migration 0015 seeded — the migration seed is
/// deliberately NOT this constant.
const CREATION_LANE_SEED: &[(&str, &str, i32)] = &[
    ("backlog", "Backlog", 0),
    ("in_progress", "In-Progress", 1),
    ("done", "Done", 2),
];

/// Write the creation-seed lane rows for a freshly-inserted project — the ONE
/// writer both creation paths (`insert_project`, `seed_initial_workspace`)
/// call, inside THEIR transaction, so a committed project can never exist
/// laneless (post-0015 the composite FK `fk_issues_lane` would strand every
/// subsequent issue INSERT).
pub(crate) async fn seed_creation_lanes(
    conn: &mut sqlx::PgConnection,
    project_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
) -> Result<(), sqlx::Error> {
    for (lane_slug, label, position) in CREATION_LANE_SEED {
        sqlx::query(
            "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
                  VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(uuid::Uuid::now_v7())
        .bind(project_id)
        .bind(workspace_id)
        .bind(lane_slug)
        .bind(label)
        .bind(position)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

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
        let (moved, deleted) = match fate {
            LaneDeleteFate::MoveTo { destination_slug } => {
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
                let moved = self
                    .move_cards_to_destination(
                        &mut tx,
                        project_id,
                        lane_slug,
                        destination_slug,
                        &cards,
                        actor_id,
                    )
                    .await?;
                (moved, 0u64)
            }
            LaneDeleteFate::DeleteCards => (0u64, delete_cards_permanently(&mut tx, &cards).await?),
        };

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

    /// MOVE fate (data-models.md §3-§4): append the dying lane's cards at the
    /// destination's bottom — positions `C..C+N-1` in the captured
    /// `(position ASC, number DESC)` order, `C` the destination's occupied
    /// count (0012 contiguity: occupied positions are `0..C-1`; the source
    /// column vanishes whole, so no gap-closing pass). Per card, in the SAME
    /// transaction: one 0013 `status` event (`old` = dying slug, `new` =
    /// destination slug, actor = operator) via the shared
    /// `record_issue_change` writer, plus one `IssueUpdated` outbox row
    /// (mirrors `reposition_issue_with_outbox`) so SSE/board listeners
    /// observe the moves. Commit-or-nothing rides the caller's `tx`.
    async fn move_cards_to_destination(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        project_id: uuid::Uuid,
        dying_slug: &str,
        destination_slug: &str,
        cards: &[(uuid::Uuid, i32)],
        actor_id: uuid::Uuid,
    ) -> Result<u64, StoreError> {
        let (workspace_id, key_prefix): (uuid::Uuid, String) =
            sqlx::query_as("SELECT workspace_id, key_prefix FROM projects WHERE id = $1")
                .bind(project_id)
                .fetch_one(&mut **tx)
                .await?;
        let (occupied,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM issues WHERE project_id = $1 AND state = $2")
                .bind(project_id)
                .bind(destination_slug)
                .fetch_one(&mut **tx)
                .await?;
        for (index, (issue_id, number)) in cards.iter().enumerate() {
            sqlx::query(
                "UPDATE issues SET state = $1, position = $2, updated_at = now() WHERE id = $3",
            )
            .bind(destination_slug)
            .bind(occupied as i32 + index as i32)
            .bind(issue_id)
            .execute(&mut **tx)
            .await?;
            let payload = serde_json::json!({
                "issue_id": issue_id,
                "project_id": project_id,
                "workspace_id": workspace_id,
                "number": number,
                "key": format!("{key_prefix}-{number}"),
                "state": destination_slug,
                "author_id": actor_id,
            });
            sqlx::query("INSERT INTO outbox (event_type, payload) VALUES ('IssueUpdated', $1)")
                .bind(payload)
                .execute(&mut **tx)
                .await?;
            self.record_issue_change(
                tx,
                workspace_id,
                project_id,
                *issue_id,
                actor_id,
                "status",
                Some(dying_slug),
                destination_slug,
            )
            .await
            .map_err(issue_change_error)?;
        }
        Ok(cards.len() as u64)
    }
}

/// DELETE fate — the `delete_issue_cascade` shape, batched and in-transaction:
/// one `DELETE … WHERE id = ANY($ids)`; comments, attachments and change
/// events cascade away at the schema level (0006/0011/0013 `ON DELETE
/// CASCADE`). No events, no outbox, no tombstone (D7 — parity with
/// `delete_issue_cascade`, which emits nothing).
async fn delete_cards_permanently(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    cards: &[(uuid::Uuid, i32)],
) -> Result<u64, StoreError> {
    if cards.is_empty() {
        return Ok(0);
    }
    let ids: Vec<uuid::Uuid> = cards.iter().map(|(id, _)| *id).collect();
    let result = sqlx::query("DELETE FROM issues WHERE id = ANY($1)")
        .bind(&ids)
        .execute(&mut **tx)
        .await?;
    Ok(result.rows_affected())
}

/// `record_issue_change` speaks `IssueInsertError`; inside the lane-fate
/// transaction only its sqlx arm is reachable (no project resolution here).
fn issue_change_error(err: IssueInsertError) -> StoreError {
    match err {
        IssueInsertError::Store(sqlx_error) => StoreError::Sqlx(sqlx_error),
        other => StoreError::Sqlx(sqlx::Error::Protocol(other.to_string())),
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

#[cfg(test)]
mod retry_classifier_tests {
    //! The retry gates' PURE hearts — `is_fate_retryable` (this module) and
    //! `crate::is_lane_fk_violation` (the insert envelope) — pinned over a
    //! fake `DatabaseError` carrying arbitrary SQLSTATE/constraint pairs.
    //! Their signatures ARE the driving ports (pure single-output
    //! classifiers, state-delta exempt category). Added at DELIVER Phase 5:
    //! mutation testing showed every code/constraint comparison survived —
    //! the retry path only fires under a non-deterministic race, so the
    //! classifiers must be killed at the fast unit level.

    use super::is_fate_retryable;
    use crate::is_lane_fk_violation;
    use std::borrow::Cow;

    #[derive(Debug)]
    struct FakeDbError {
        code: Option<&'static str>,
        constraint: Option<&'static str>,
    }

    impl std::fmt::Display for FakeDbError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "fake database error")
        }
    }

    impl std::error::Error for FakeDbError {}

    impl sqlx::error::DatabaseError for FakeDbError {
        fn message(&self) -> &str {
            "fake database error"
        }
        fn code(&self) -> Option<Cow<'_, str>> {
            self.code.map(Cow::Borrowed)
        }
        fn constraint(&self) -> Option<&str> {
            self.constraint
        }
        fn kind(&self) -> sqlx::error::ErrorKind {
            sqlx::error::ErrorKind::Other
        }
        fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
            self
        }
    }

    fn db_error(code: Option<&'static str>, constraint: Option<&'static str>) -> sqlx::Error {
        sqlx::Error::Database(Box::new(FakeDbError { code, constraint }))
    }

    /// A fate attempt retries on EXACTLY foreign_key_violation (23503) or
    /// deadlock_detected (40P01) — never on other SQLSTATEs, a codeless
    /// database error, or a non-database error.
    #[test]
    fn fate_retry_fires_only_on_fk_violation_or_deadlock() {
        for (code, expected) in [
            (Some("23503"), true),
            (Some("40P01"), true),
            (Some("23505"), false), // unique_violation is NOT worth retrying
            (Some("42P01"), false), // undefined_table is a bug, not a race
            (None, false),
        ] {
            assert_eq!(
                is_fate_retryable(&db_error(code, None)),
                expected,
                "SQLSTATE {code:?} retryability"
            );
        }
        assert!(
            !is_fate_retryable(&sqlx::Error::RowNotFound),
            "a non-database error is never a retryable race"
        );
    }

    /// The insert envelope's single re-resolve retry fires on EXACTLY the
    /// `fk_issues_lane` strand-guard: 23503 AND that constraint — never on
    /// another constraint, another SQLSTATE, or a non-database error.
    #[test]
    fn insert_retry_fires_only_on_the_lane_strand_guard() {
        for (code, constraint, expected) in [
            (Some("23503"), Some("fk_issues_lane"), true),
            (Some("23503"), Some("fk_comments_issue"), false),
            (Some("23505"), Some("fk_issues_lane"), false),
            (Some("23503"), None, false),
            (None, Some("fk_issues_lane"), false),
        ] {
            assert_eq!(
                is_lane_fk_violation(&db_error(code, constraint)),
                expected,
                "SQLSTATE {code:?} / constraint {constraint:?}"
            );
        }
        assert!(
            !is_lane_fk_violation(&sqlx::Error::RowNotFound),
            "a non-database error is never the strand-guard"
        );
    }
}

// ===========================================================================
// board-lane-overflow-menu — shape-in-place write ports (DISTILL scaffolds)
//
// SCAFFOLD: true (ADR-025). Bodies panic; the SIGNATURES are the DESIGN
// contract (component-boundaries.md §4). DELIVER slices 02/03 replace them.
// ===========================================================================

/// Which side of the anchor lane a new lane lands on (`insert_lane_at`).
///
/// An unrecognised side never reaches here: the adapter answers the uniform
/// non-enumerable 404, so "insert after a lane that does not exist" and
/// "insert sideways" are indistinguishable to a prober (DD6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneSide {
    Before,
    After,
}

/// Why an insert was refused. Every arm returns BEFORE any write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaneInsertOutcome {
    Inserted {
        position: i32,
    },
    /// The anchor lane is absent → uniform 404 at the adapter.
    AnchorNotFound,
    /// The minted slug already exists on this project. Pre-checked INSIDE the
    /// transaction's lock so the operator sees D7's refusal copy rather than a
    /// raw `duplicate key value violates unique constraint
    /// "lanes_project_id_slug_key"` — which is what surfaces otherwise
    /// (measured, adr-board-lane-003).
    SlugTaken,
}

impl Store {
    /// SCAFFOLD: true — rename a lane's LABEL. DELIVER slice 02.
    ///
    /// Label-only by inherited invariant, not by choice: `issues.state` holds
    /// the lane **slug** under composite FK `fk_issues_lane`, and `brief.md`
    /// §lanes pins "lane slugs are immutable identity, labels mutable display".
    /// This statement must therefore touch `label` and nothing else — verified
    /// against a real Postgres: `slug`, `position` and every `issues.state`
    /// are unchanged by it.
    ///
    /// No transaction and no lock (DD7): last-write-wins on a display label is
    /// acceptable, and no invariant depends on the ordering of two renames.
    pub async fn rename_lane(
        &self,
        project_id: uuid::Uuid,
        lane_slug: &str,
        new_label: &str,
    ) -> Result<bool, StoreError> {
        // `SET label` and NOTHING else. The column list here is the whole
        // guarantee: `slug` is identity under composite FK `fk_issues_lane`,
        // so touching it would rewrite every issue in the lane; `position`
        // belongs to the insert path. Verified against a real Postgres — after
        // this statement, slug, position and every issues.state are unchanged.
        let affected =
            sqlx::query("UPDATE lanes SET label = $3 WHERE project_id = $1 AND slug = $2")
                .bind(project_id)
                .bind(lane_slug)
                .bind(new_label)
                .execute(self.pool())
                .await?
                .rows_affected();
        Ok(affected == 1)
    }

    /// SCAFFOLD: true — insert a lane beside an anchor. DELIVER slice 03.
    ///
    /// ONE transaction, in this exact order (ADR-BOARD-LANE-003, every step
    /// measured against a real Postgres 16 during the DESIGN spike):
    ///
    /// 1. `SELECT … FOR UPDATE` on **this project's lane rows** — the house
    ///    idiom `delete_lane_with_fate` already uses. Without it, two
    ///    concurrent inserts at one anchor hand the loser a raw
    ///    `duplicate key` error; with it, both commit cleanly.
    /// 2. Resolve the anchor by **identity** (slug), inside the lock. NEVER
    ///    from a position captured when the dialog was rendered — "insert
    ///    before Done" must keep meaning *before Done* if another operator
    ///    inserts meanwhile (the D7 "count is advisory" shape).
    /// 3. Capture the target position BEFORE any shift (`+1` for `After`).
    ///    Reading the anchor back after shifting is the trap that made the
    ///    first spike attempt fail — twice.
    /// 4. Pre-check the minted slug for collision → `SlugTaken`.
    /// 5. `UPDATE lanes SET position = position + 1 WHERE position >= at`.
    ///    Safe with **no** `SET CONSTRAINTS` call: the shipped constraint is
    ///    `UNIQUE (project_id, position) DEFERRABLE INITIALLY IMMEDIATE`, and
    ///    DEFERRABLE means checked at end-of-**statement**, not per row. The
    ///    identical statement against a non-deferrable constraint FAILS — so
    ///    that keyword is load-bearing for this function (adr-board-lane-003).
    /// 6. Insert at the captured slot.
    ///
    /// Writes ZERO issue rows and ZERO 0013 change events (AC-3.3).
    pub async fn insert_lane_at(
        &self,
        project_id: uuid::Uuid,
        anchor_slug: &str,
        side: LaneSide,
        new_slug: &str,
        new_label: &str,
    ) -> Result<LaneInsertOutcome, StoreError> {
        let mut tx = self.pool().begin().await?;

        // 1. Serialize on THIS project's lane rows. Without this, two
        //    concurrent inserts at one anchor hand the loser a raw
        //    `duplicate key` error — measured, not hypothesised. The same
        //    `FOR UPDATE` idiom `delete_lane_with_fate` uses.
        sqlx::query("SELECT 1 FROM lanes WHERE project_id = $1 ORDER BY position FOR UPDATE")
            .bind(project_id)
            .fetch_all(&mut *tx)
            .await?;

        // 2. Resolve the anchor by IDENTITY, inside the lock — never from a
        //    position captured when the dialog was rendered. "Insert before
        //    Done" must keep meaning *before Done* if another operator inserts
        //    meanwhile (the D7 "the count is advisory" shape).
        let anchor: Option<(i32,)> =
            sqlx::query_as("SELECT position FROM lanes WHERE project_id = $1 AND slug = $2")
                .bind(project_id)
                .bind(anchor_slug)
                .fetch_optional(&mut *tx)
                .await?;
        let Some((anchor_position,)) = anchor else {
            return Ok(LaneInsertOutcome::AnchorNotFound);
        };

        // 3. Capture the target slot BEFORE any shift. Reading the anchor back
        //    AFTER shifting is the trap that made the DESIGN spike's first
        //    attempt fail — twice.
        let at = match side {
            LaneSide::Before => anchor_position,
            LaneSide::After => anchor_position + 1,
        };

        // 4. Pre-check the slug INSIDE the lock, so the operator gets the D7
        //    refusal copy instead of a raw
        //    `duplicate key value violates unique constraint
        //     "lanes_project_id_slug_key"`.
        let taken: Option<(i32,)> =
            sqlx::query_as("SELECT 1 FROM lanes WHERE project_id = $1 AND slug = $2")
                .bind(project_id)
                .bind(new_slug)
                .fetch_optional(&mut *tx)
                .await?;
        if taken.is_some() {
            return Ok(LaneInsertOutcome::SlugTaken);
        }

        // 5. The shift. NO `SET CONSTRAINTS` call is needed and none is made:
        //    `UNIQUE (project_id, position) DEFERRABLE INITIALLY IMMEDIATE`
        //    (0015:22) checks at end-of-STATEMENT, not per row, so the bulk
        //    increment is already safe. The identical statement against a
        //    non-deferrable constraint FAILS — that keyword is load-bearing
        //    for this function (ADR-BOARD-LANE-003).
        sqlx::query(
            "UPDATE lanes SET position = position + 1 WHERE project_id = $1 AND position >= $2",
        )
        .bind(project_id)
        .bind(at)
        .execute(&mut *tx)
        .await?;

        // 6. Land at the captured slot. Zero issue rows and zero 0013 change
        //    events are written anywhere in this transaction: inserting a lane
        //    is a lane-set operation only (AC-3.3).
        let workspace_id: (uuid::Uuid,) =
            sqlx::query_as("SELECT workspace_id FROM projects WHERE id = $1")
                .bind(project_id)
                .fetch_one(&mut *tx)
                .await?;
        sqlx::query(
            "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
                  VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(uuid::Uuid::now_v7())
        .bind(project_id)
        .bind(workspace_id.0)
        .bind(new_slug)
        .bind(new_label)
        .bind(at)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(LaneInsertOutcome::Inserted { position: at })
    }

    /// Relocate a lane so it sits immediately before `before_slug`, or last when
    /// `before_slug` is `None` (D7). Both ends are resolved by IDENTITY inside
    /// the lock, never from a position captured when the drag began.
    ///
    /// ONE `UPDATE … SET position = CASE …` statement applies the whole
    /// permutation (ADR-BOARD-LANE-006). `insert_lane_at`'s two-statement
    /// shuffle CANNOT be reused: insert is safe only because its bulk `+1`
    /// VACATES the target slot, and a move has no vacancy — the intervening
    /// shift collides with the mover still occupying its old position, and
    /// `DEFERRABLE INITIALLY IMMEDIATE` (end-of-STATEMENT) does not save it.
    /// Measured against postgres:16-alpine, not reasoned about.
    ///
    /// The `FOR UPDATE` is load-bearing for a subtler reason than it was for
    /// insert: the unlocked insert race raises a duplicate-key error, but the
    /// unlocked MOVE race raises NOTHING — it keeps contiguity, keeps
    /// uniqueness, and still leaves a lane nobody mentioned shoved past
    /// another (ADR-BOARD-LANE-006 Finding 4).
    pub async fn move_lane_before(
        &self,
        project_id: uuid::Uuid,
        mover_slug: &str,
        before_slug: Option<&str>,
    ) -> Result<LaneMoveOutcome, StoreError> {
        let mut tx = self.pool().begin().await?;

        // 1. Serialize on THIS project's lane rows — the same idiom
        //    `insert_lane_at` and `delete_lane_with_fate` use.
        sqlx::query("SELECT 1 FROM lanes WHERE project_id = $1 ORDER BY position FOR UPDATE")
            .bind(project_id)
            .fetch_all(&mut *tx)
            .await?;

        // 2. Resolve the mover by IDENTITY, inside the lock.
        let mover: Option<(i32,)> =
            sqlx::query_as("SELECT position FROM lanes WHERE project_id = $1 AND slug = $2")
                .bind(project_id)
                .bind(mover_slug)
                .fetch_optional(&mut *tx)
                .await?;
        let Some((from,)) = mover else {
            return Ok(LaneMoveOutcome::MoverNotFound);
        };

        // 3. Resolve the destination, also by identity inside the lock. An
        //    absent `before_slug` means "place last".
        let to = match before_slug {
            None => {
                let (max,): (i32,) =
                    sqlx::query_as("SELECT max(position) FROM lanes WHERE project_id = $1")
                        .bind(project_id)
                        .fetch_one(&mut *tx)
                        .await?;
                max
            }
            Some(slug) => {
                let neighbour: Option<(i32,)> = sqlx::query_as(
                    "SELECT position FROM lanes WHERE project_id = $1 AND slug = $2",
                )
                .bind(project_id)
                .bind(slug)
                .fetch_optional(&mut *tx)
                .await?;
                let Some((at,)) = neighbour else {
                    return Ok(LaneMoveOutcome::NeighbourNotFound);
                };
                // Lifting the mover out shifts every later lane left by one, so
                // landing "before" a neighbour to our RIGHT means landing at
                // its position minus one.
                if at > from {
                    at - 1
                } else {
                    at
                }
            }
        };

        if to == from {
            // Nothing to do, and nothing written (DDD-7). A drag that lands
            // where it started costs no write.
            return Ok(LaneMoveOutcome::NoOp);
        }

        // 4. The whole permutation, in ONE statement. Two statements would end
        //    the first one with a duplicate position — see the doc comment.
        sqlx::query(
            "UPDATE lanes SET position = CASE
                    WHEN slug = $2                                          THEN $4
                    WHEN $4 < $3 AND position >= $4 AND position <  $3      THEN position + 1
                    WHEN $4 > $3 AND position >  $3 AND position <= $4      THEN position - 1
                    ELSE position END
               WHERE project_id = $1",
        )
        .bind(project_id)
        .bind(mover_slug)
        .bind(from)
        .bind(to)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(LaneMoveOutcome::Moved { from, to })
    }
}

/// Why a move was refused, or what it did. Every refusal arm returns BEFORE any
/// write. `MoverNotFound` and `NeighbourNotFound` BOTH map to the uniform
/// non-enumerable 404 at the adapter and must stay indistinguishable there
/// (DDD-6) — they are separate arms here only so the store's own tests can tell
/// which lookup failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaneMoveOutcome {
    Moved {
        from: i32,
        to: i32,
    },
    /// Destination equals the lane's current position — commits nothing (DDD-7).
    NoOp,
    MoverNotFound,
    NeighbourNotFound,
}
