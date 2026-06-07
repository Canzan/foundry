//! Admin machine-token surface — `/admin/tokens` (machine-token-admin-ux).
//!
//! RED scaffold (DISTILL, Mandate 7 / ADR-025):
//! These three handlers are mounted in `build_router` ALONGSIDE the HTML routes
//! (so they sit UNDER `csrf::csrf_middleware` + `session_layer` — NOT the
//! CSRF-exempt `/api/v1` mount; ADR-MT03/DD5). For RED they each RETURN a clean
//! `501 Not Implemented` response carrying a `RED scaffold` marker — NOT a
//! `panic!` (a panic aborts the axum connection, surfacing at the test client as
//! a transport error that masks the assertion; a returned 501 lets the Then step
//! capture a real status and fail RED for MISSING_FUNCTIONALITY on the absent
//! page/value/list, which is the correct RED signal). The route EXISTS on every
//! binary (the point of OD1/DD2 graceful degradation: verifier-only differs by
//! the signer Option, not by route presence), so there is no 404.
//!
//! DELIVER replaces these bodies following `projects.rs` verbatim:
//!   - `signed_in_user(&session)` → resolve workspace from session → render.
//!   - `is_workspace_admin` gate → non-admin = generic 404 (non-enumerable,
//!     NFR-MT-SEC-03).
//!   - `state.machine_token_signer.is_none()` → "issuing not enabled" notice on
//!     GET, 403-style page on POST (graceful, OD1/DD2) — never a 500, never a
//!     partial token (all-or-nothing render, NFR-MT-REL-01).
//!   - mint reads the signer from `state` and passes it to
//!     `services.mint_token(signer, …)` (DD4); the returned `SecretString` is
//!     rendered EXACTLY ONCE into `TokenMintedPage` and dropped (DD7).
//!
//! Contract + view-models in `docs/feature/machine-token-admin-ux/design/admin-routes.md`
//! and the precise wiring in `distill/step-skeletons.md`.
//!
//! SCAFFOLD: true

use crate::AppState;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use tower_sessions::Session;

const RED_SCAFFOLD_BODY: &str =
    "machine-token-admin-ux: /admin/tokens not yet implemented — RED scaffold";

fn not_implemented() -> Response {
    (StatusCode::NOT_IMPLEMENTED, RED_SCAFFOLD_BODY).into_response()
}

/// `GET /admin/tokens` — US-MT02/US-MT06 list + the mint form (or the "issuing
/// not enabled" notice on a verifier-only binary).
pub async fn show_index(
    State(_state): State<AppState>,
    _session: Session,
    _headers: HeaderMap,
) -> Response {
    not_implemented()
}

/// `POST /admin/tokens` — US-MT01/US-MT04 mint. On success renders the one-time
/// value page (TokenMintedPage); on validation failure re-renders the form.
pub async fn submit_mint(
    State(_state): State<AppState>,
    _session: Session,
    _headers: HeaderMap,
    _form: axum::extract::RawForm,
) -> Response {
    not_implemented()
}

/// `POST /admin/tokens/{jti}/revoke` — US-MT03 revoke. Workspace-isolated,
/// idempotent; redirects to the list (or swaps the row fragment).
pub async fn submit_revoke(
    State(_state): State<AppState>,
    _session: Session,
    _headers: HeaderMap,
    Path(_jti): Path<uuid::Uuid>,
    _form: axum::extract::RawForm,
) -> Response {
    not_implemented()
}
