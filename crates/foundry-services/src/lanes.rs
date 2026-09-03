//! Lane-delete use-case (board-lane-management).
//!
//! Port SIGNATURES are the DESIGN contract (component-boundaries.md §3),
//! implemented on the `classify_rename` idiom: a PURE classification heart
//! ([`classify_lane_delete`]) — property-testable without a store — wrapped
//! by thin async shells (reads → classify → store tx via
//! [`foundry_store::Store::delete_lane_with_fate`], whose in-transaction
//! locks re-check every arm authoritatively).
//!
//! Error mapping (architecture-design.md §5.4): `NotFound` → the uniform
//! non-enumerable 404 (foreign/absent lane|project|team, non-member,
//! double-submit — D10, on BOTH the dialog GET and the confirm POST);
//! `LastLane` / `UnknownDestination` → 422 bare fragment into the dialog's
//! `[data-error-slot]`.

use crate::{BoardLane, Principal};
use foundry_store::{LaneDeleteFate, LaneDeleteOutcome, Store};

/// The operator's fate choice as the adapter parsed it (`fate` + optional
/// `destination` form fields; htmx submits the clicked button's name/value).
#[derive(Debug, Clone)]
pub enum LaneFate<'a> {
    Move { destination: &'a str },
    Delete,
}

/// The dialog GET's view-model: lane identity, LIVE advisory count, and the
/// surviving lanes in board order (`survivors[0]` is the picker preselect).
#[derive(Debug, Clone)]
pub struct LaneDialogView {
    pub lane_slug: String,
    pub lane_label: String,
    /// Advisory copy only; the fate binds at confirm time (D7).
    pub card_count: i64,
    pub survivors: Vec<BoardLane>,
}

#[derive(Debug, Clone)]
pub struct DeleteLaneSuccess {
    pub surviving: Vec<BoardLane>,
    pub moved: u64,
    pub deleted: u64,
}

#[derive(Debug)]
pub enum DeleteLaneError {
    /// Foreign/absent lane|project|team, non-member, double-submit → uniform 404.
    NotFound,
    /// Sole remaining lane → 422 "A board needs at least one lane".
    LastLane,
    /// Move destination unknown or == dying lane → 422.
    UnknownDestination,
    Internal,
}

/// The pure decision the thin shells act on (the `classify_rename` idiom).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaneDeleteDecision {
    ProceedDeleteCards,
    ProceedMoveTo { destination_slug: String },
}

/// GET arm: authz (`resolve_member_project` idiom) → lane + live count +
/// survivors. Foreign/absent/non-member → `NotFound` (handler renders the
/// uniform 404).
pub async fn delete_lane_dialog(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    lane_slug: &str,
) -> Result<LaneDialogView, DeleteLaneError> {
    let project = resolve_lane_project(store, principal, team_slug, project_slug).await?;
    let lanes = list_lanes(store, project.id).await?;
    let lane = lanes
        .iter()
        .find(|lane| lane.slug == lane_slug)
        .ok_or(DeleteLaneError::NotFound)?
        .clone();
    let card_count = store
        .count_issues_in_lane(project.id, lane_slug)
        .await
        .map_err(|_| DeleteLaneError::Internal)?;
    let survivors = survivors_of(&lanes, lane_slug);
    Ok(LaneDialogView {
        lane_slug: lane.slug,
        lane_label: lane.label,
        card_count,
        survivors,
    })
}

/// POST arm: authz → reads → [`classify_lane_delete`] (the PURE heart) →
/// `Store::delete_lane_with_fate` (the authoritative in-transaction re-check)
/// → outcome mapping.
pub async fn delete_lane(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    lane_slug: &str,
    fate: LaneFate<'_>,
) -> Result<DeleteLaneSuccess, DeleteLaneError> {
    let project = resolve_lane_project(store, principal, team_slug, project_slug).await?;
    let lanes = list_lanes(store, project.id).await?;
    let lane_exists = lanes.iter().any(|lane| lane.slug == lane_slug);
    let destination_is_survivor = match &fate {
        LaneFate::Move { destination } => {
            *destination != lane_slug && lanes.iter().any(|lane| lane.slug == *destination)
        }
        LaneFate::Delete => false,
    };
    let decision = classify_lane_delete(lane_exists, lanes.len(), &fate, destination_is_survivor)?;
    let store_fate = match &decision {
        LaneDeleteDecision::ProceedDeleteCards => LaneDeleteFate::DeleteCards,
        LaneDeleteDecision::ProceedMoveTo { destination_slug } => {
            LaneDeleteFate::MoveTo { destination_slug }
        }
    };
    let outcome = store
        .delete_lane_with_fate(project.id, lane_slug, store_fate, principal.user_id())
        .await
        .map_err(|_| DeleteLaneError::Internal)?;
    match outcome {
        LaneDeleteOutcome::Deleted { moved, deleted } => {
            let surviving = list_lanes(store, project.id).await?;
            Ok(DeleteLaneSuccess {
                surviving,
                moved,
                deleted,
            })
        }
        // In-transaction re-checks are authoritative (double-submit race,
        // concurrent last-lane, racing destination delete).
        LaneDeleteOutcome::LaneNotFound => Err(DeleteLaneError::NotFound),
        LaneDeleteOutcome::LastLane => Err(DeleteLaneError::LastLane),
        LaneDeleteOutcome::DestinationNotFound => Err(DeleteLaneError::UnknownDestination),
    }
}

/// The lane routes' authz gate — the `resolve_member_project` idiom with the
/// D10 mapping: foreign/absent team|project AND non-member (incl. a
/// team-scoped machine credential aimed elsewhere) ALL collapse to `NotFound`,
/// the uniform non-enumerable 404 on BOTH verbs (DESIGN refinement 4 pins the
/// deliberate asymmetry vs `show_board`'s intra-workspace 403).
async fn resolve_lane_project(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
) -> Result<foundry_store::ProjectRow, DeleteLaneError> {
    let team = store
        .find_team_by_slug(principal.workspace_id(), team_slug)
        .await
        .map_err(|_| DeleteLaneError::Internal)?
        .ok_or(DeleteLaneError::NotFound)?;
    if let Principal::Machine {
        scope_team_id: Some(scoped_team),
        ..
    } = principal
    {
        if *scoped_team != team.id {
            return Err(DeleteLaneError::NotFound);
        }
    }
    let is_member = store
        .is_team_member(team.id, principal.user_id())
        .await
        .map_err(|_| DeleteLaneError::Internal)?;
    if !is_member {
        return Err(DeleteLaneError::NotFound);
    }
    store
        .find_project_by_slug(team.id, project_slug)
        .await
        .map_err(|_| DeleteLaneError::Internal)?
        .ok_or(DeleteLaneError::NotFound)
}

async fn list_lanes(
    store: &Store,
    project_id: uuid::Uuid,
) -> Result<Vec<BoardLane>, DeleteLaneError> {
    Ok(store
        .list_project_lanes(project_id)
        .await
        .map_err(|_| DeleteLaneError::Internal)?
        .into_iter()
        .map(BoardLane::from)
        .collect())
}

fn survivors_of(lanes: &[BoardLane], dying_slug: &str) -> Vec<BoardLane> {
    lanes
        .iter()
        .filter(|lane| lane.slug != dying_slug)
        .cloned()
        .collect()
}

/// PURE heart — property-testable without a store: given what the reads saw
/// (does the lane exist, how many lanes does the project have, which fate,
/// is the destination among the survivors), decide
/// Proceed{arm} | NotFound | LastLane | UnknownDestination.
pub fn classify_lane_delete(
    lane_exists: bool,
    lane_count: usize,
    fate: &LaneFate<'_>,
    destination_is_survivor: bool,
) -> Result<LaneDeleteDecision, DeleteLaneError> {
    // Ordering mirrors the transaction (ADR-BOARD-LANE-002): the lane row is
    // locked FIRST, so an absent lane refuses NotFound before any other arm.
    if !lane_exists {
        return Err(DeleteLaneError::NotFound);
    }
    if lane_count < 2 {
        return Err(DeleteLaneError::LastLane);
    }
    match fate {
        LaneFate::Delete => Ok(LaneDeleteDecision::ProceedDeleteCards),
        LaneFate::Move { destination } => {
            if !destination_is_survivor {
                return Err(DeleteLaneError::UnknownDestination);
            }
            Ok(LaneDeleteDecision::ProceedMoveTo {
                destination_slug: (*destination).to_string(),
            })
        }
    }
}

#[cfg(test)]
mod classify_lane_delete_properties {
    //! PBT over the PURE classification heart (board-lane-management 03-01,
    //! the `classify_rename` idiom). The function IS its own driving port
    //! (port-to-port at domain scope); the observable universe is the single
    //! return value — pure function, no adjacent slots (state-delta exempt
    //! category, single-output pure function).
    //!
    //! Partial-arm properties pinned by the step contract:
    //!   - lane absent → ALWAYS NotFound (dominates every other arm — the
    //!     transaction locks the lane row FIRST, ADR-BOARD-LANE-002);
    //!   - lane present and lane_count == 1 → ALWAYS LastLane;
    //!   - NEVER Proceed while either refusal condition holds.

    use super::*;
    use proptest::prelude::*;

    /// An arbitrary operator fate: `Delete`, or `Move` with whether the parsed
    /// destination is among the survivors.
    fn any_fate() -> impl Strategy<Value = (bool, bool)> {
        // (is_move, destination_is_survivor) — survivor flag meaningless for
        // Delete but generated anyway (the classifier must ignore it there).
        (any::<bool>(), any::<bool>())
    }

    fn fate_of(is_move: bool) -> LaneFate<'static> {
        if is_move {
            LaneFate::Move {
                destination: "some-survivor",
            }
        } else {
            LaneFate::Delete
        }
    }

    proptest! {
        /// Lane absent → NotFound, whatever the count or fate (the uniform
        /// non-enumerable 404 arm; covers the double-submit race).
        #[test]
        fn absent_lane_is_always_not_found(
            lane_count in 0usize..=8,
            (is_move, survivor) in any_fate(),
        ) {
            let fate = fate_of(is_move);
            let result = classify_lane_delete(false, lane_count, &fate, survivor);
            prop_assert!(
                matches!(result, Err(DeleteLaneError::NotFound)),
                "absent lane must classify NotFound; got {result:?}"
            );
        }

        /// Sole remaining lane → LastLane, whatever the fate (the 422
        /// "A board needs at least one lane" arm).
        #[test]
        fn sole_lane_is_always_last_lane(
            (is_move, survivor) in any_fate(),
        ) {
            let fate = fate_of(is_move);
            let result = classify_lane_delete(true, 1, &fate, survivor);
            prop_assert!(
                matches!(result, Err(DeleteLaneError::LastLane)),
                "the sole lane must classify LastLane; got {result:?}"
            );
        }

        /// Joint safety + proceed-arm correctness: Proceed is ONLY reachable
        /// when the lane exists among >= 2 lanes; a proceeding Move requires a
        /// surviving destination, and the decision arm mirrors the fate.
        #[test]
        fn proceed_only_when_deletable_and_arm_matches_fate(
            lane_exists in any::<bool>(),
            lane_count in 0usize..=8,
            (is_move, survivor) in any_fate(),
        ) {
            // A present lane counts itself: (exists, count == 0) is
            // unconstructible from the reads (precondition, Hebert ch.10).
            prop_assume!(!(lane_exists && lane_count == 0));
            let fate = fate_of(is_move);
            let result = classify_lane_delete(lane_exists, lane_count, &fate, survivor);
            let deletable = lane_exists && lane_count >= 2;
            if !deletable {
                prop_assert!(
                    result.is_err(),
                    "never Proceed when the lane is absent or the last; got {result:?}"
                );
            } else if is_move && !survivor {
                prop_assert!(
                    matches!(result, Err(DeleteLaneError::UnknownDestination)),
                    "a move to a non-survivor must classify UnknownDestination; got {result:?}"
                );
            } else if is_move {
                prop_assert!(
                    matches!(
                        &result,
                        Ok(LaneDeleteDecision::ProceedMoveTo { destination_slug })
                            if destination_slug == "some-survivor"
                    ),
                    "a surviving-destination move must proceed with THAT slug; got {result:?}"
                );
            } else {
                prop_assert!(
                    matches!(result, Ok(LaneDeleteDecision::ProceedDeleteCards)),
                    "a delete fate must proceed to the delete arm; got {result:?}"
                );
            }
        }
    }
}

// ===========================================================================
// board-lane-overflow-menu — shape-in-place use cases (DISTILL scaffolds)
//
// SCAFFOLD: true (ADR-025). Bodies panic; the SIGNATURES are the DESIGN
// contract (component-boundaries.md §3). DELIVER slices 02/03 replace them.
// ===========================================================================

/// The edit dialog's view-model — the lane's CURRENT label, pre-filled.
#[derive(Debug, Clone)]
pub struct EditLaneView {
    pub lane_slug: String,
    /// Pre-fill for the name field. Advisory in exactly the sense the delete
    /// dialog's count is advisory: the write re-resolves the lane at confirm.
    pub lane_label: String,
}

/// The insert dialog's view-model — which lane the new one lands beside.
#[derive(Debug, Clone)]
pub struct InsertLaneView {
    pub anchor_slug: String,
    pub anchor_label: String,
    pub side: crate::lanes::LaneSideView,
}

/// Adapter-facing mirror of `foundry_store::lanes::LaneSide`, so the HTTP
/// layer never depends on the store crate's enum directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneSideView {
    Before,
    After,
}

/// Why a rename was refused.
#[derive(Debug)]
pub enum RenameLaneError {
    /// Foreign/absent lane|project|team, non-member, signed-out → uniform 404.
    NotFound,
    /// Blank after trim → 422 into `[data-error-slot]`.
    LabelBlank,
    /// Longer than the 64-character DB bound → 422. Enforced HERE as well as
    /// by the CHECK: a bound only the database knows surfaces as a 500.
    LabelTooLong,
    Store(crate::lanes::LaneStoreFailure),
}

/// Why an insert was refused.
#[derive(Debug)]
pub enum InsertLaneError {
    /// Anchor lane absent, unrecognised side, non-member, signed-out →
    /// uniform 404. An unrecognised side MUST be indistinguishable from an
    /// unknown lane (DD6) — never a 400.
    NotFound,
    LabelBlank,
    LabelTooLong,
    /// The name normalises to no usable characters ("...", "!!!", "   ") →
    /// 422 asking for letters or numbers (D7).
    SlugEmpty,
    /// The minted slug already names a lane on this project → 422 naming the
    /// conflict. Never auto-suffixed: slugs are immutable identity, so a
    /// silent `done_2` would drift from its label forever (D7).
    SlugTaken,
    Store(crate::lanes::LaneStoreFailure),
}

/// Opaque store-failure carrier, so the two error enums above stay
/// adapter-facing without leaking `StoreError`'s shape.
#[derive(Debug)]
pub struct LaneStoreFailure(pub String);

/// The DB bound, mirrored in code. `lanes.label` is
/// `CHECK (length(label) BETWEEN 1 AND 64)`; a bound only the database knows
/// surfaces to the operator as a 500 instead of a reason.
pub const LANE_LABEL_MAX: usize = 64;

/// Why a lane label was rejected. ONE seam serves rename and insert (Driving
/// Port 3 — the DD10 "one normalisation shared by both adapters" property that
/// already holds for state validation). Two label validators would drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelRejection {
    Blank,
    TooLong,
}

/// Trim, then bound. Returns the trimmed label the caller should store, so the
/// stored value and the validated value cannot diverge.
pub fn validate_lane_label(raw: &str) -> Result<&str, LabelRejection> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(LabelRejection::Blank);
    }
    // Counted in CHARACTERS, not bytes — the DB CHECK uses `length()`, which is
    // characters in Postgres. Counting bytes would reject a 40-character
    // accented label the database would happily accept.
    if trimmed.chars().count() > LANE_LABEL_MAX {
        return Err(LabelRejection::TooLong);
    }
    Ok(trimmed)
}

/// SCAFFOLD: true — the edit dialog's read. DELIVER slice 02.
pub async fn edit_lane_dialog(
    store: &foundry_store::Store,
    principal: &crate::Principal,
    team_slug: &str,
    project_slug: &str,
    lane_slug: &str,
) -> Result<EditLaneView, RenameLaneError> {
    let project = resolve_lane_project(store, principal, team_slug, project_slug)
        .await
        .map_err(rename_from_delete_error)?;
    let lanes = list_lanes(store, project.id)
        .await
        .map_err(rename_from_delete_error)?;
    let lane = lanes
        .iter()
        .find(|lane| lane.slug == lane_slug)
        .ok_or(RenameLaneError::NotFound)?;
    Ok(EditLaneView {
        lane_slug: lane.slug.clone(),
        lane_label: lane.label.clone(),
    })
}

/// The lane routes share ONE authz gate, so an absent lane, a foreign project
/// and a non-member all collapse to the same uniform 404 (D11). These two
/// adapters map that gate's error onto their own enum rather than duplicating
/// the gate — a second copy is a second place for the 404-vs-403 asymmetry to
/// drift.
fn rename_from_delete_error(err: DeleteLaneError) -> RenameLaneError {
    match err {
        DeleteLaneError::NotFound => RenameLaneError::NotFound,
        other => RenameLaneError::Store(LaneStoreFailure(format!("{other:?}"))),
    }
}

/// Why a lane move was refused. `NotFound` covers mover-absent,
/// neighbour-absent, non-member and signed-out — all one uniform 404 at the
/// adapter (DDD-6), never distinguishable by a prober.
#[derive(Debug)]
pub enum MoveLaneError {
    NotFound,
    Store(crate::lanes::LaneStoreFailure),
}

/// Move a lane beside a named neighbour. `before_slug == None` means "place
/// last" (D7).
///
/// BOTH surfaces call this — the `⋯` menu's Move items and the column-header
/// drag (DDD-8). Two seams would be two places for the 404-vs-403 asymmetry to
/// drift, which is the whole reason there is one.
///
/// Every refusal collapses to `NotFound`: a vanished mover, a vanished
/// neighbour, a non-member and a signed-out visitor must be indistinguishable
/// at the adapter, or the pair enumerates which lanes and boards exist (DDD-6).
pub async fn move_lane(
    store: &foundry_store::Store,
    principal: &crate::Principal,
    team_slug: &str,
    project_slug: &str,
    mover_slug: &str,
    before_slug: Option<&str>,
) -> Result<(), MoveLaneError> {
    let project = resolve_lane_project(store, principal, team_slug, project_slug)
        .await
        .map_err(move_from_delete_error)?;
    match store
        .move_lane_before(project.id, mover_slug, before_slug)
        .await
        .map_err(|err| MoveLaneError::Store(LaneStoreFailure(format!("{err:?}"))))?
    {
        // A no-op still answers with the board refresh, so an optimistic DOM
        // move that landed where it started is re-synced rather than left to
        // drift (DDD-7).
        foundry_store::lanes::LaneMoveOutcome::Moved { .. }
        | foundry_store::lanes::LaneMoveOutcome::NoOp => Ok(()),
        foundry_store::lanes::LaneMoveOutcome::MoverNotFound
        | foundry_store::lanes::LaneMoveOutcome::NeighbourNotFound => Err(MoveLaneError::NotFound),
    }
}

fn move_from_delete_error(err: DeleteLaneError) -> MoveLaneError {
    match err {
        DeleteLaneError::NotFound => MoveLaneError::NotFound,
        other => MoveLaneError::Store(LaneStoreFailure(format!("{other:?}"))),
    }
}

fn insert_from_delete_error(err: DeleteLaneError) -> InsertLaneError {
    match err {
        DeleteLaneError::NotFound => InsertLaneError::NotFound,
        other => InsertLaneError::Store(LaneStoreFailure(format!("{other:?}"))),
    }
}

/// SCAFFOLD: true — rename a lane's label. DELIVER slice 02.
///
/// Validation runs through the SAME seam `insert_lane` uses (Driving Port 3 —
/// the DD10 "one normalisation shared by both adapters" property that already
/// holds for state validation). Two label validators would drift.
pub async fn rename_lane(
    store: &foundry_store::Store,
    principal: &crate::Principal,
    team_slug: &str,
    project_slug: &str,
    lane_slug: &str,
    new_label: &str,
) -> Result<(), RenameLaneError> {
    let project = resolve_lane_project(store, principal, team_slug, project_slug)
        .await
        .map_err(rename_from_delete_error)?;
    let label = validate_lane_label(new_label).map_err(|rejection| match rejection {
        LabelRejection::Blank => RenameLaneError::LabelBlank,
        LabelRejection::TooLong => RenameLaneError::LabelTooLong,
    })?;
    // No uniqueness check: labels are DISPLAY, and two lanes may legitimately
    // read "Doing" (AC-2.6). Only slugs are identity, and a rename never
    // touches one.
    let renamed = store
        .rename_lane(project.id, lane_slug, label)
        .await
        .map_err(|err| RenameLaneError::Store(LaneStoreFailure(format!("{err:?}"))))?;
    if renamed {
        Ok(())
    } else {
        // Zero rows means the lane is gone (or never existed) — the same
        // uniform 404 a foreign project gets.
        Err(RenameLaneError::NotFound)
    }
}

/// SCAFFOLD: true — the insert dialog's read. DELIVER slice 03.
pub async fn insert_lane_dialog(
    store: &foundry_store::Store,
    principal: &crate::Principal,
    team_slug: &str,
    project_slug: &str,
    anchor_slug: &str,
    side: LaneSideView,
) -> Result<InsertLaneView, InsertLaneError> {
    let project = resolve_lane_project(store, principal, team_slug, project_slug)
        .await
        .map_err(insert_from_delete_error)?;
    let lanes = list_lanes(store, project.id)
        .await
        .map_err(insert_from_delete_error)?;
    let anchor = lanes
        .iter()
        .find(|lane| lane.slug == anchor_slug)
        .ok_or(InsertLaneError::NotFound)?;
    Ok(InsertLaneView {
        anchor_slug: anchor.slug.clone(),
        anchor_label: anchor.label.clone(),
        side,
    })
}

/// SCAFFOLD: true — mint the slug and insert the lane. DELIVER slice 03.
///
/// Mints via `foundry_core::lane_slug`, NEVER `foundry_core::slugify` — the
/// latter emits hyphens, which `lanes_slug_check` rejects
/// (adr-board-lane-004). Wraps the store's locked transaction; the slug
/// collision is pre-checked inside that lock so the raw `duplicate key` error
/// never reaches the operator.
pub async fn insert_lane(
    store: &foundry_store::Store,
    principal: &crate::Principal,
    team_slug: &str,
    project_slug: &str,
    anchor_slug: &str,
    side: LaneSideView,
    new_label: &str,
) -> Result<(), InsertLaneError> {
    let project = resolve_lane_project(store, principal, team_slug, project_slug)
        .await
        .map_err(insert_from_delete_error)?;
    // The SAME label seam the rename uses (Driving Port 3 / DD10).
    let label = validate_lane_label(new_label).map_err(|rejection| match rejection {
        LabelRejection::Blank => InsertLaneError::LabelBlank,
        LabelRejection::TooLong => InsertLaneError::LabelTooLong,
    })?;
    // Minted by `lane_slug`, NEVER `slugify` — the latter emits hyphens, which
    // `lanes_slug_check` rejects (ADR-BOARD-LANE-004).
    let slug = foundry_core::lane_slug(label);
    if slug.is_empty() {
        return Err(InsertLaneError::SlugEmpty);
    }
    let store_side = match side {
        LaneSideView::Before => foundry_store::lanes::LaneSide::Before,
        LaneSideView::After => foundry_store::lanes::LaneSide::After,
    };
    let outcome = store
        .insert_lane_at(project.id, anchor_slug, store_side, &slug, label)
        .await
        .map_err(|err| InsertLaneError::Store(LaneStoreFailure(format!("{err:?}"))))?;
    match outcome {
        foundry_store::lanes::LaneInsertOutcome::Inserted { .. } => Ok(()),
        // In-transaction re-checks are authoritative: the anchor may have been
        // deleted, or the slug taken, since the dialog was rendered.
        foundry_store::lanes::LaneInsertOutcome::AnchorNotFound => Err(InsertLaneError::NotFound),
        foundry_store::lanes::LaneInsertOutcome::SlugTaken => Err(InsertLaneError::SlugTaken),
    }
}

#[cfg(test)]
mod lane_label_tests {
    use super::{validate_lane_label, LabelRejection, LANE_LABEL_MAX};

    /// The seam is shared by rename AND insert (Driving Port 3), so its
    /// boundaries are tested here once rather than through two adapters. These
    /// are fast unit tests on purpose: the acceptance lane proves the refusal
    /// reaches `[data-error-slot]`, but a bound covered ONLY through acceptance
    /// lets `>` / `>=` mutants survive — the `@real-io` trap the predecessor
    /// wave hit and fixed the same way.
    #[test]
    fn trims_and_accepts_an_ordinary_label() {
        assert_eq!(validate_lane_label("  Staging  "), Ok("Staging"));
    }

    #[test]
    fn returns_the_trimmed_value_so_stored_and_validated_cannot_diverge() {
        // If this returned the RAW input, a label could pass validation at 64
        // trimmed characters and then be stored at 68 with padding.
        assert_eq!(validate_lane_label(" Doing "), Ok("Doing"));
    }

    #[test]
    fn rejects_empty_and_whitespace_only() {
        assert_eq!(validate_lane_label(""), Err(LabelRejection::Blank));
        assert_eq!(validate_lane_label("   "), Err(LabelRejection::Blank));
        assert_eq!(validate_lane_label("\t\n "), Err(LabelRejection::Blank));
    }

    #[test]
    fn the_bound_is_inclusive_at_64_and_exclusive_at_65() {
        let at = "x".repeat(LANE_LABEL_MAX);
        let over = "x".repeat(LANE_LABEL_MAX + 1);
        assert!(
            validate_lane_label(&at).is_ok(),
            "64 characters is ACCEPTED"
        );
        assert_eq!(validate_lane_label(&over), Err(LabelRejection::TooLong));
    }

    #[test]
    fn one_character_is_accepted() {
        assert_eq!(validate_lane_label("x"), Ok("x"));
    }

    /// The DB CHECK is `length(label)`, which Postgres counts in CHARACTERS.
    /// Counting bytes here would reject a label the database accepts — a
    /// refusal the operator could not act on.
    #[test]
    fn the_bound_counts_characters_not_bytes() {
        let accented = "é".repeat(LANE_LABEL_MAX);
        assert_eq!(
            accented.len(),
            LANE_LABEL_MAX * 2,
            "precondition: 2 bytes each"
        );
        assert!(
            validate_lane_label(&accented).is_ok(),
            "64 characters must pass even at 128 bytes"
        );
    }
}
