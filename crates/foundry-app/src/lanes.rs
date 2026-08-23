//! Lane-delete web surface — `/team/{team}/project/{project}/lanes/{lane}/delete`
//! (board-lane-management, component-boundaries.md §4).
//!
//! Both handlers are mounted in `build_router` UNDER `csrf::csrf_middleware`
//! and `session_layer`: the confirm POST is a mutating htmx form carrying the
//! double-submit `_csrf` (a tokenless POST is refused by the middleware
//! BEFORE this module runs); the dialog GET is a SAFE read and carries none
//! (DESIGN refinement 3).
//!
//! Refusal contract (D10 / DESIGN refinement 4): foreign, absent, non-member
//! and signed-out ALL answer the uniform non-enumerable
//! `resource_not_found_page` on BOTH verbs — deliberately asymmetric to
//! `show_board`'s intra-workspace 403.
//!
//!   - GET  → `foundry_services::lanes::delete_lane_dialog` → render
//!     `partials/delete_lane_modal.html` (confirm-only when `card_count == 0`,
//!     fate dialog otherwise; survivors in board order, leftmost preselected;
//!     close is `data-action="close-modal"` ONLY — BR-4).
//!   - POST → parse `fate` (+ `destination`) → `foundry_services::lanes::
//!     delete_lane` → success: OOB `#board-columns` refresh + empty primary
//!     swap clearing `#modal-root`; `LastLane`/`UnknownDestination`/invalid
//!     form: 422 bare `error_fragment` (marker `delete-lane-error`) into
//!     `[data-error-slot]`; `NotFound`: the uniform 404.

use crate::bootstrap::{resource_not_found_page, SessionUser};
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
use askama::Template;
use axum::extract::{Form, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use foundry_services::lanes::{DeleteLaneError, LaneFate};
use foundry_services::Principal;
use serde::Deserialize;
use tower_sessions::Session;

/// The byte-stable `data-hx-fragment` marker form-errors.js routes into the
/// dialog's `[data-error-slot]` (component-boundaries.md §4).
const ERROR_FRAGMENT_MARKER: &str = "delete-lane-error";

/// The last-lane refusal copy — VERBATIM pin (component-boundaries.md §4).
const LAST_LANE_MESSAGE: &str = "A board needs at least one lane";

/// `GET /team/{team}/project/{project}/lanes/{lane}/delete` — the SAFE dialog
/// read (US-BLM-03/04): no `_csrf` required, nothing written. Renders the
/// confirm dialog with the LIVE advisory count and the survivors in board
/// order. Refusals (foreign, absent, non-member, signed-out) → uniform
/// non-enumerable 404.
pub async fn show_delete_lane_dialog(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((team_slug, project_slug, lane_slug)): Path<(String, String, String)>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return resource_not_found_page();
    };
    let principal = Principal::Human {
        user_id: user.user_id,
        workspace_id: user.workspace_id,
    };
    let view = match foundry_services::lanes::delete_lane_dialog(
        &state.store,
        &principal,
        &team_slug,
        &project_slug,
        &lane_slug,
    )
    .await
    {
        Ok(view) => view,
        Err(DeleteLaneError::NotFound) => return resource_not_found_page(),
        Err(err) => return internal_error("delete_lane_dialog", format!("{err:?}")),
    };
    // Mint (or reuse) the double-submit cookie so the dialog's confirm POST
    // carries a cookie-matched `_csrf` (the new-issue/edit-dialog idiom).
    let (csrf, set_cookie) = crate::csrf::ensure_csrf_cookie(&state, &headers);
    let body = crate::views::DeleteLaneModal {
        action: lane_delete_action(&team_slug, &project_slug, &lane_slug),
        csrf,
        lane_slug: view.lane_slug,
        lane_label: view.lane_label,
        card_count: view.card_count,
        survivors: view.survivors,
    }
    .render()
    .expect("delete_lane_modal partial renders from a fully-resolved, infallible view-model");
    crate::csrf::response_with_optional_cookie(
        StatusCode::OK,
        Html(body).into_response(),
        set_cookie,
    )
}

/// The confirm POST's form: `fate` is the clicked submitter's name/value
/// (htmx includes it); `destination` rides only with the move fate.
#[derive(Debug, Deserialize)]
pub struct DeleteLaneForm {
    #[serde(default)]
    pub fate: String,
    #[serde(default)]
    pub destination: Option<String>,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

/// `POST /team/{team}/project/{project}/lanes/{lane}/delete` — the mutating
/// confirm (the CSRF middleware refuses a tokenless POST before this runs).
/// Success answers the out-of-band `#board-columns` refresh with an empty
/// primary swap (dialog closes, no reload); refusals per the module docs.
pub async fn submit_delete_lane(
    State(state): State<AppState>,
    session: Session,
    Path((team_slug, project_slug, lane_slug)): Path<(String, String, String)>,
    Form(form): Form<DeleteLaneForm>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return resource_not_found_page();
    };
    let principal = Principal::Human {
        user_id: user.user_id,
        workspace_id: user.workspace_id,
    };
    let destination = form.destination.as_deref().map(str::trim);
    let fate = match form.fate.as_str() {
        "delete" => LaneFate::Delete,
        "move" => match destination.filter(|d| !d.is_empty()) {
            Some(destination) => LaneFate::Move { destination },
            None => return validation_fragment("Choose a lane to move the issues to"),
        },
        _ => return validation_fragment("Choose what should happen to this lane"),
    };
    match foundry_services::lanes::delete_lane(
        &state.store,
        &principal,
        &team_slug,
        &project_slug,
        &lane_slug,
        fate,
    )
    .await
    {
        Ok(_success) => oob_columns_response(&state, &principal, &team_slug, &project_slug).await,
        Err(DeleteLaneError::NotFound) => resource_not_found_page(),
        Err(DeleteLaneError::LastLane) => validation_fragment(LAST_LANE_MESSAGE),
        Err(DeleteLaneError::UnknownDestination) => {
            validation_fragment("Choose a lane to move the issues to")
        }
        Err(err) => internal_error("delete_lane", format!("{err:?}")),
    }
}

// ----------------------------------------------------------------- internals

/// Success body: the refreshed board columns as the `hx-swap-oob="true"`
/// `#board-columns` replace (house OOB idiom). Re-reads through the SAME
/// authz-gated `board_view` the board page renders from, so the fragment and
/// the next full render are byte-identical.
async fn oob_columns_response(
    state: &AppState,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
) -> Response {
    let view =
        match foundry_services::board::board_view(&state.store, principal, team_slug, project_slug)
            .await
        {
            Ok(view) => view,
            Err(err) => return internal_error("board_view (post-delete refresh)", err),
        };
    let body = crate::views::BoardColumnsOob {
        team_slug: team_slug.to_string(),
        project_slug: project_slug.to_string(),
        columns: crate::views::board_columns(team_slug, project_slug, &view),
    }
    .render()
    .expect("board_columns_oob partial renders from a fully-resolved, infallible view-model");
    (StatusCode::OK, Html(body)).into_response()
}

fn lane_delete_action(team_slug: &str, project_slug: &str, lane_slug: &str) -> String {
    format!("/team/{team_slug}/project/{project_slug}/lanes/{lane_slug}/delete")
}

/// The bare 422 error fragment (`data-hx-fragment="delete-lane-error"`)
/// form-errors.js routes into the open dialog's `[data-error-slot]`.
fn validation_fragment(message: &str) -> Response {
    let body = crate::views::ErrorFragment {
        fragment_marker: ERROR_FRAGMENT_MARKER.to_string(),
        message: message.to_string(),
    }
    .render()
    .expect("error_fragment.html renders from a fully-resolved, infallible view-model");
    (StatusCode::UNPROCESSABLE_ENTITY, Html(body)).into_response()
}

async fn signed_in_user(session: &Session) -> Option<SessionUser> {
    session
        .get::<SessionUser>(SESSION_KEY_USER_ID)
        .await
        .ok()
        .flatten()
}

fn internal_error<E: std::fmt::Display>(label: &str, err: E) -> Response {
    tracing::error!(error = %err, "{label} failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}
