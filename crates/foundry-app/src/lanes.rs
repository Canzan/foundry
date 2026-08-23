//! Lane-delete web surface — `/team/{team}/project/{project}/lanes/{lane}/delete`
//! (board-lane-management).
//!
//! RED scaffold (DISTILL, Mandate 7 / ADR-025):
//! Both handlers are mounted in `build_router` ALONGSIDE the HTML routes (so
//! they sit UNDER `csrf::csrf_middleware` + `session_layer` — the confirm POST
//! is a mutating htmx form carrying the double-submit `_csrf`; the dialog GET
//! is a SAFE read and carries none — DESIGN refinement 3). For RED they each
//! RETURN a clean `501 Not Implemented` carrying a `RED scaffold` marker — NOT
//! a `panic!` (a panic aborts the axum connection and masks the assertion; a
//! returned 501 lets the Then step capture a real status and fail RED for
//! MISSING_FUNCTIONALITY on the absent dialog/refresh, the correct signal —
//! the `admin_tokens` scaffold precedent). Mounting the routes NOW also keeps
//! the authz scenarios honest: an unrouted path would answer the uniform 404,
//! which is exactly the byte-identical refusal those scenarios assert — a
//! false green over an absent gate.
//!
//! DELIVER replaces these bodies per component-boundaries.md §4:
//!   - GET  → `foundry_services::lanes::delete_lane_dialog` → render
//!     `partials/delete_lane_modal.html` (confirm-only when `card_count == 0`,
//!     fate dialog otherwise; survivors in board order, leftmost preselected;
//!     close is `data-action="close-modal"` ONLY — BR-4).
//!   - POST → parse `fate` (+ `destination`) → `foundry_services::lanes::
//!     delete_lane` → success: OOB `#board-columns` refresh + empty primary
//!     swap clearing `#modal-root`; `LastLane`/`UnknownDestination`: 422 bare
//!     `error_fragment` (marker `delete-lane-error`) into `[data-error-slot]`;
//!     `NotFound`: the uniform `resource_not_found_page` (D10, BOTH verbs).
//!
//! SCAFFOLD: true

use crate::AppState;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use tower_sessions::Session;

const RED_SCAFFOLD_BODY: &str =
    "board-lane-management: /team/{team}/project/{project}/lanes/{lane}/delete not yet implemented — RED scaffold";

fn not_implemented() -> Response {
    (StatusCode::NOT_IMPLEMENTED, RED_SCAFFOLD_BODY).into_response()
}

/// `GET /team/{team}/project/{project}/lanes/{lane}/delete` — the SAFE dialog
/// read (US-BLM-03/04). Refusals (foreign, absent, non-member, signed-out) →
/// uniform non-enumerable 404.
pub async fn show_delete_lane_dialog(
    State(_state): State<AppState>,
    _session: Session,
    _headers: HeaderMap,
    Path((_team_slug, _project_slug, _lane_slug)): Path<(String, String, String)>,
) -> Response {
    not_implemented()
}

/// `POST /team/{team}/project/{project}/lanes/{lane}/delete` — the mutating
/// confirm (form `_csrf`, `fate` = `move|delete`, `destination` iff move; the
/// CSRF middleware refuses a tokenless POST before this handler runs).
pub async fn submit_delete_lane(
    State(_state): State<AppState>,
    _session: Session,
    _headers: HeaderMap,
    Path((_team_slug, _project_slug, _lane_slug)): Path<(String, String, String)>,
    _form: axum::extract::RawForm,
) -> Response {
    not_implemented()
}
