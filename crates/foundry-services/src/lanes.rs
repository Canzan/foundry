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
        .map(|lane| BoardLane {
            slug: lane.slug,
            label: lane.label,
        })
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
