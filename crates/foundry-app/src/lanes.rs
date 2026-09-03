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
use foundry_services::lanes::{
    DeleteLaneError, InsertLaneError, LaneFate, MoveLaneError, RenameLaneError,
};
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

#[cfg(test)]
mod response_helper_tests {
    //! The `instance_admin::response_helper_tests` idiom, added at DELIVER
    //! Phase 5: mutation testing showed the pure response helpers were only
    //! pinned through the browser lane (the @real-io trap).

    use super::*;

    async fn body_string(resp: Response) -> String {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("read response body");
        String::from_utf8(bytes.to_vec()).expect("utf-8 body")
    }

    /// A lane-delete refusal is a 422 BARE fragment carrying the byte-stable
    /// `delete-lane-error` marker (form-errors.js routes it into the open
    /// dialog's `[data-error-slot]`) and the handler's copy verbatim.
    #[tokio::test]
    async fn validation_fragment_is_a_422_with_marker_and_copy() {
        let resp = validation_fragment(LAST_LANE_MESSAGE);
        assert_eq!(
            resp.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "a lane-delete refusal must answer 422"
        );
        let body = body_string(resp).await;
        assert!(
            body.contains(r#"data-hx-fragment="delete-lane-error""#),
            "the fragment must carry the byte-stable scraper marker; body was:\n{body}"
        );
        assert!(
            body.contains("A board needs at least one lane"),
            "the fragment must carry the refusal copy verbatim; body was:\n{body}"
        );
    }

    /// The dialog's confirm form posts back to the lane-delete route built
    /// from the REQUEST-PATH slugs (D2 — never a render-time derivation).
    #[test]
    fn lane_delete_action_is_the_confirm_post_route() {
        assert_eq!(
            lane_delete_action("general", "sandbox", "in_progress"),
            "/team/general/project/sandbox/lanes/in_progress/delete",
            "the confirm POST must target the lane-delete route for THESE slugs"
        );
    }

    /// An internal failure answers 500 — never a silent 200.
    #[tokio::test]
    async fn internal_error_answers_500() {
        let resp = internal_error("delete_lane", "boom");
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}

// ===========================================================================
// board-lane-overflow-menu — edit + insert web surface (DISTILL scaffolds)
//
// SCAFFOLD: true (ADR-025). Each handler answers a clean 501 rather than
// panicking: a panic aborts the axum connection and MASKS the assertion the
// scenario is making (the `admin_tokens` precedent). Mounting the routes now
// also keeps the authz scenarios honest — an unrouted path would answer the
// exact uniform 404 they assert, and would pass for the wrong reason.
//
// The refusal contract is inherited verbatim from the delete surface above:
// foreign, absent, non-member and signed-out all answer the uniform
// non-enumerable 404 on BOTH verbs. An unrecognised insert `{side}` joins
// that set — it must be indistinguishable from an unknown lane, never a 400
// (DD6), or the pair becomes an enumeration oracle for which lanes exist.
// ===========================================================================

/// The refusal copy, pinned here so the fragment and the acceptance oracle
/// cannot drift apart.
const LABEL_BLANK_MESSAGE: &str = "Enter a name using letters or numbers";
const LABEL_TOO_LONG_MESSAGE: &str = "Use 64 characters or fewer";

fn lane_edit_action(team_slug: &str, project_slug: &str, lane_slug: &str) -> String {
    format!("/team/{team_slug}/project/{project_slug}/lanes/{lane_slug}/edit")
}

fn lane_insert_action(team_slug: &str, project_slug: &str, lane_slug: &str, side: &str) -> String {
    format!("/team/{team_slug}/project/{project_slug}/lanes/{lane_slug}/insert/{side}")
}

/// The signed-in principal, or `None` → the uniform 404. Shared by all four
/// handlers so the signed-out arm cannot diverge between them.
async fn lane_principal(session: &Session) -> Option<Principal> {
    signed_in_user(session).await.map(|user| Principal::Human {
        user_id: user.user_id,
        workspace_id: user.workspace_id,
    })
}

/// The `{side}` path segment. Anything but `before`/`after` resolves to
/// `None`, which the handlers answer with the uniform 404 (DD6).
pub fn parse_lane_side(raw: &str) -> Option<foundry_services::lanes::LaneSideView> {
    match raw {
        "before" => Some(foundry_services::lanes::LaneSideView::Before),
        "after" => Some(foundry_services::lanes::LaneSideView::After),
        _ => None,
    }
}

/// The rename confirm's form. `_csrf` rides as a body field like every other
/// mutating form in this app (the double-submit idiom).
#[derive(Debug, Deserialize)]
pub struct EditLaneForm {
    #[serde(default)]
    pub label: String,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

/// The insert confirm's form.
#[derive(Debug, Deserialize)]
pub struct InsertLaneForm {
    #[serde(default)]
    pub label: String,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

/// `GET …/lanes/{lane}/edit` — the SAFE dialog read (no `_csrf`; nothing
/// written), pre-filled with the lane's current label.
pub async fn show_edit_lane_dialog(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((team_slug, project_slug, lane_slug)): Path<(String, String, String)>,
) -> Response {
    let Some(principal) = lane_principal(&session).await else {
        return resource_not_found_page();
    };
    let view = match foundry_services::lanes::edit_lane_dialog(
        &state.store,
        &principal,
        &team_slug,
        &project_slug,
        &lane_slug,
    )
    .await
    {
        Ok(view) => view,
        Err(RenameLaneError::NotFound) => return resource_not_found_page(),
        Err(err) => return internal_error("edit_lane_dialog", format!("{err:?}")),
    };
    let (csrf, set_cookie) = crate::csrf::ensure_csrf_cookie(&state, &headers);
    let body = crate::views::EditLaneModal {
        action: lane_edit_action(&team_slug, &project_slug, &lane_slug),
        csrf,
        lane_slug: view.lane_slug,
        lane_label: view.lane_label,
    }
    .render()
    .expect("edit_lane_modal partial renders from a fully-resolved, infallible view-model");
    crate::csrf::response_with_optional_cookie(
        StatusCode::OK,
        Html(body).into_response(),
        set_cookie,
    )
}

/// `POST …/lanes/{lane}/edit` — the rename confirm (the CSRF middleware
/// refuses a tokenless POST before this runs).
pub async fn submit_edit_lane(
    State(state): State<AppState>,
    session: Session,
    Path((team_slug, project_slug, lane_slug)): Path<(String, String, String)>,
    Form(form): Form<EditLaneForm>,
) -> Response {
    let Some(principal) = lane_principal(&session).await else {
        return resource_not_found_page();
    };
    match foundry_services::lanes::rename_lane(
        &state.store,
        &principal,
        &team_slug,
        &project_slug,
        &lane_slug,
        &form.label,
    )
    .await
    {
        Ok(()) => oob_columns_response(&state, &principal, &team_slug, &project_slug).await,
        Err(RenameLaneError::NotFound) => resource_not_found_page(),
        Err(RenameLaneError::LabelBlank) => validation_fragment(LABEL_BLANK_MESSAGE),
        Err(RenameLaneError::LabelTooLong) => validation_fragment(LABEL_TOO_LONG_MESSAGE),
        Err(err) => internal_error("rename_lane", format!("{err:?}")),
    }
}

/// `GET …/lanes/{lane}/insert/{side}` — the SAFE dialog read.
///
/// An unrecognised `{side}` takes the SAME uniform 404 an unknown lane takes.
/// That is deliberate and load-bearing: a 400 here would tell a prober that the
/// lane exists and only the side was wrong, turning the pair into an
/// enumeration oracle for a project's lane set (DD6).
pub async fn show_insert_lane_dialog(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((team_slug, project_slug, lane_slug, side)): Path<(String, String, String, String)>,
) -> Response {
    let Some(principal) = lane_principal(&session).await else {
        return resource_not_found_page();
    };
    let Some(side_view) = parse_lane_side(&side) else {
        return resource_not_found_page();
    };
    let view = match foundry_services::lanes::insert_lane_dialog(
        &state.store,
        &principal,
        &team_slug,
        &project_slug,
        &lane_slug,
        side_view,
    )
    .await
    {
        Ok(view) => view,
        Err(InsertLaneError::NotFound) => return resource_not_found_page(),
        Err(err) => return internal_error("insert_lane_dialog", format!("{err:?}")),
    };
    let (csrf, set_cookie) = crate::csrf::ensure_csrf_cookie(&state, &headers);
    let body = crate::views::InsertLaneModal {
        action: lane_insert_action(&team_slug, &project_slug, &lane_slug, &side),
        csrf,
        anchor_slug: view.anchor_slug,
        anchor_label: view.anchor_label,
        side: side.clone(),
    }
    .render()
    .expect("insert_lane_modal partial renders from a fully-resolved, infallible view-model");
    crate::csrf::response_with_optional_cookie(
        StatusCode::OK,
        Html(body).into_response(),
        set_cookie,
    )
}

/// The move confirm's form. `before` names the lane the mover lands
/// immediately before; ABSENT or empty means "place last" (D7). Deliberately a
/// neighbour SLUG and never a numeric index — an index captured when the drag
/// began is stale the instant another operator inserts or deletes a lane
/// (ADR-BOARD-LANE-006).
#[derive(Debug, Deserialize)]
pub struct MoveLaneForm {
    #[serde(default)]
    pub before: String,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

/// `POST …/lanes/{lane}/move` — the move confirm (the CSRF middleware refuses a
/// tokenless POST before this runs).
///
/// Both callers land here: the `⋯` menu's two Move items (`hx-post`) and the
/// column-header drag (`fetch` + `x-csrf-token`). There is deliberately no GET
/// counterpart — a move needs no dialog (D12).
pub async fn submit_move_lane(
    State(state): State<AppState>,
    session: Session,
    Path((team_slug, project_slug, lane_slug)): Path<(String, String, String)>,
    Form(form): Form<MoveLaneForm>,
) -> Response {
    let Some(principal) = lane_principal(&session).await else {
        return resource_not_found_page();
    };
    // An absent or empty `before` means "place last" (D7) — an empty form field
    // and an omitted one must mean the same thing, since a browser sends the
    // former and a script may send either.
    let before = form.before.trim();
    let before = if before.is_empty() {
        None
    } else {
        Some(before)
    };
    match foundry_services::lanes::move_lane(
        &state.store,
        &principal,
        &team_slug,
        &project_slug,
        &lane_slug,
        before,
    )
    .await
    {
        Ok(()) => oob_columns_response(&state, &principal, &team_slug, &project_slug).await,
        Err(MoveLaneError::NotFound) => resource_not_found_page(),
        Err(err) => internal_error("move_lane", format!("{err:?}")),
    }
}

/// `POST …/lanes/{lane}/insert/{side}` — the insert confirm (the CSRF
/// middleware refuses a tokenless POST before this runs).
pub async fn submit_insert_lane(
    State(state): State<AppState>,
    session: Session,
    Path((team_slug, project_slug, lane_slug, side)): Path<(String, String, String, String)>,
    Form(form): Form<InsertLaneForm>,
) -> Response {
    let Some(principal) = lane_principal(&session).await else {
        return resource_not_found_page();
    };
    let Some(side_view) = parse_lane_side(&side) else {
        return resource_not_found_page();
    };
    match foundry_services::lanes::insert_lane(
        &state.store,
        &principal,
        &team_slug,
        &project_slug,
        &lane_slug,
        side_view,
        &form.label,
    )
    .await
    {
        Ok(()) => oob_columns_response(&state, &principal, &team_slug, &project_slug).await,
        Err(InsertLaneError::NotFound) => resource_not_found_page(),
        Err(InsertLaneError::LabelBlank) => validation_fragment(LABEL_BLANK_MESSAGE),
        Err(InsertLaneError::LabelTooLong) => validation_fragment(LABEL_TOO_LONG_MESSAGE),
        // The name held no letters or digits ("...", "!!!"). The copy asks for
        // what IS usable rather than reporting that a slug came out empty —
        // the operator never sees slugs.
        Err(InsertLaneError::SlugEmpty) => validation_fragment(LABEL_BLANK_MESSAGE),
        // Names the conflict. Never auto-suffixed: a silent `done_2` would be
        // permanent identity drifting from its label forever (D7).
        Err(InsertLaneError::SlugTaken) => validation_fragment(&format!(
            "A lane called {} already exists",
            form.label.trim()
        )),
        Err(err) => internal_error("insert_lane", format!("{err:?}")),
    }
}
