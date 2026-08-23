//! Lane-delete use-case (board-lane-management).
//!
//! RED scaffold (DISTILL, Mandate 7 / ADR-025): the port SIGNATURES are the
//! DESIGN contract (component-boundaries.md §3). Bodies panic with a `RED
//! scaffold` marker so failures classify as MISSING_FUNCTIONALITY (RED), not
//! BROKEN. DELIVER replaces the bodies following the `classify_rename` idiom:
//! a PURE classification heart ([`classify_lane_delete`]) — property-testable
//! without a store — wrapped by thin async shells (reads → classify → store
//! tx via [`foundry_store::Store::delete_lane_with_fate`]).
//!
//! Error mapping (architecture-design.md §5.4): `NotFound` → the uniform
//! non-enumerable 404 (foreign/absent lane|project|team, non-member,
//! double-submit — D10, on BOTH the dialog GET and the confirm POST);
//! `LastLane` / `UnknownDestination` → 422 bare fragment into the dialog's
//! `[data-error-slot]`.
//!
//! SCAFFOLD: true

use crate::{BoardLane, Principal};
use foundry_store::Store;

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
    _store: &Store,
    _principal: &Principal,
    _team_slug: &str,
    _project_slug: &str,
    _lane_slug: &str,
) -> Result<LaneDialogView, DeleteLaneError> {
    panic!("lanes::delete_lane_dialog not yet implemented — RED scaffold (board-lane-management)")
}

/// POST arm: authz → `Store::delete_lane_with_fate` → outcome mapping.
pub async fn delete_lane(
    _store: &Store,
    _principal: &Principal,
    _team_slug: &str,
    _project_slug: &str,
    _lane_slug: &str,
    _fate: LaneFate<'_>,
) -> Result<DeleteLaneSuccess, DeleteLaneError> {
    panic!("lanes::delete_lane not yet implemented — RED scaffold (board-lane-management)")
}

/// PURE heart — property-testable without a store: given what the reads saw
/// (does the lane exist, how many lanes does the project have, which fate,
/// is the destination among the survivors), decide
/// Proceed{arm} | NotFound | LastLane | UnknownDestination.
pub fn classify_lane_delete(
    _lane_exists: bool,
    _lane_count: usize,
    _fate: &LaneFate<'_>,
    _destination_is_survivor: bool,
) -> Result<LaneDeleteDecision, DeleteLaneError> {
    panic!("lanes::classify_lane_delete not yet implemented — RED scaffold (board-lane-management)")
}
