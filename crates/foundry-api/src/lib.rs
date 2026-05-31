//! foundry-api — the JSON API driving adapter (`/api/v1`).
//!
//! SCAFFOLD: true  (RED scaffold created by DISTILL, Mandate 7)
//!
//! Per DESIGN (`docs/feature/web-tier-extraction/design/api-contract.md`,
//! `auth.md`, `architecture.md`, ADR-W01) this crate serves the first-class
//! JSON API: read + write of issues/comments under `/api/v1`, authenticated by
//! a bearer machine token (JWT/Ed25519), emitting JSON only — never HTML.
//!
//! What this scaffold pins (the contract DELIVER implements):
//!   - the route handler entry points (one per api-contract.md route),
//!   - the `token_auth` verification result shape (fail-closed),
//!   - the JSON error envelope + status mapping (error-and-observability.md).
//!
//! What this scaffold deliberately omits (DELIVER adds it, see
//! distill/step-skeletons.md "What DELIVER must wire"):
//!   - the `axum` dependency, the `FromRequestParts` extractor, the
//!     `IntoResponse` impl, and `pub fn routes(state) -> axum::Router`;
//!   - the `foundry-store` machine_tokens repo + `foundry-auth::MachineToken`
//!     verifier calls;
//!   - the `foundry-app::build_router` `.merge(foundry_api::routes(state))`
//!     composition and the `AppState::machine_token_verifier` field.
//!
//! Keeping axum out of the scaffold means adding this crate to the workspace
//! does not pull a heavy build into the otherwise-green tree.
//!
//! Every body `panic!`s, classifying as RED (MISSING_FUNCTIONALITY), not BROKEN.

#![forbid(unsafe_code)]

use std::sync::Arc;

use axum::extract::{FromRef, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::get;
use axum::Router;
use foundry_services::{Principal, ServiceError};
use foundry_store::Store;
use tower_sessions::Session;

const NOT_IMPLEMENTED: &str = "Not yet implemented -- RED scaffold (foundry-api, Feature A)";

/// The stable JSON error envelope (api-contract.md §"Error envelope").
/// Every non-2xx response carries exactly this shape; the `code` is a stable
/// machine token, the `message` carries the same copy the UI shows where one
/// exists. Never contains HTML, SQL, a stack trace, or any credential material.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
}

/// The wire shape of an issue (api-contract.md §"Issue"). Serialized by this
/// adapter from the neutral `foundry_services::BoardIssue`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IssueJson {
    pub key: String,
    pub number: i32,
    pub title: String,
    pub state: String,
}

/// Map a `ServiceError` to its `(status, envelope)` per
/// `api-contract.md` §"Status code conventions". Every variant maps to
/// exactly one HTTP status + JSON envelope code; the envelope never
/// carries HTML, SQL, a stack trace, or any credential material.
pub fn status_for(err: &ServiceError) -> (u16, ErrorBody) {
    let (status, code, message): (u16, &str, String) = match err {
        ServiceError::Unauthorized => (401, "unauthorized", "unauthorized".into()),
        ServiceError::Forbidden => (403, "forbidden", "forbidden".into()),
        ServiceError::NotFound => (404, "not_found", "not found".into()),
        ServiceError::Gone => (410, "gone", "gone".into()),
        ServiceError::Conflict => (409, "conflict", "conflict".into()),
        ServiceError::Validation { code, message } => (422, code.as_str(), message.clone()),
        ServiceError::Internal => (500, "internal", "internal error".into()),
    };
    (
        status,
        ErrorBody {
            error: ErrorDetail {
                code: code.to_string(),
                message,
            },
        },
    )
}

/// The adapter's error type. Wraps a `ServiceError` and renders it as the
/// stable JSON envelope (never HTML — NFR-WEB-API-CON-03 / NFR-WEB-BND-02).
#[derive(Debug)]
pub struct ApiError(pub ServiceError);

impl From<ServiceError> for ApiError {
    fn from(err: ServiceError) -> Self {
        ApiError(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = status_for(&self.0);
        let code = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (code, Json(body)).into_response()
    }
}

/// The `/api/v1` sub-router. Merged into `foundry_app::build_router` via
/// `.merge(foundry_api::routes())`. Generic over the composition root's state
/// `S`: the handler extracts `State<Arc<Store>>`, derived from `S` through
/// `FromRef` (foundry-app implements `FromRef<AppState> for Arc<Store>`), so
/// the sub-router composes into the parent `Router<AppState>` without
/// foundry-api depending on foundry-app (the dependency direction stays
/// foundry-app -> foundry-api).
///
/// The `Session` extractor reads the session populated by the tower-sessions
/// layer (slice-1 transitional browser-session auth, api-contract.md §slice-1
/// note — the machine-token surface lands in Slice 2). Emits JSON only; never
/// constructs an HTML response body.
pub fn routes<S>() -> Router<S>
where
    Arc<Store>: FromRef<S>,
    S: Clone + Send + Sync + 'static,
{
    Router::new().route(
        "/api/v1/teams/{team_slug}/projects/{project_slug}/issues",
        get(list_issues_handler),
    )
}

/// Resolve the slice-1 transitional principal from the browser session.
///
/// Slice 1 (this step) accepts the existing browser session per
/// `api-contract.md` §slice-1 note: the session carries the signed-in
/// `user_id` + `workspace_id` (the SAME shape `foundry-app` stores under
/// the `"user_id"` key). A request with no valid session resolves to
/// `Unauthorized` (401) — fail-closed, leaking no issue data. Slice 2
/// replaces this with the bearer machine-token extractor.
async fn principal_from_session(session: &Session) -> Result<Principal, ServiceError> {
    let user: SessionUser = session
        .get(SESSION_USER_KEY)
        .await
        .map_err(|_| ServiceError::Unauthorized)?
        .ok_or(ServiceError::Unauthorized)?;
    Ok(Principal::Human {
        user_id: user.user_id,
        workspace_id: user.workspace_id,
    })
}

/// The session-data key foundry-app stores the signed-in user under
/// (`foundry_app::session::SESSION_KEY_USER_ID`). Mirrored as a literal here
/// so foundry-api does not depend on foundry-app (the dependency direction is
/// foundry-app -> foundry-api).
const SESSION_USER_KEY: &str = "user_id";

/// The slice-1 session payload shape: mirrors `foundry_app::bootstrap::SessionUser`
/// (which is crate-private). Both serialize the same two fields, so the
/// tower-sessions row written by the browser sign-in deserializes here.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct SessionUser {
    user_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
}

/// `GET /api/v1/teams/{team}/projects/{project}/issues` (US-W05a). Resolves
/// the principal, calls the shared core seam `list_board_issues`, and
/// serializes the neutral result as a JSON array (empty project -> `[]`, 200).
async fn list_issues_handler(
    State(store): State<Arc<Store>>,
    session: Session,
    Path((team_slug, project_slug)): Path<(String, String)>,
) -> Result<Json<Vec<IssueJson>>, ApiError> {
    let principal = principal_from_session(&session).await?;
    let rows =
        foundry_services::board::list_board_issues(&store, &principal, &team_slug, &project_slug)
            .await?;
    let body = rows
        .into_iter()
        .map(|r| IssueJson {
            key: r.key,
            number: r.number,
            title: r.title,
            state: r.state,
        })
        .collect();
    Ok(Json(body))
}

/// The machine-token verification surface (auth.md §"Per-request verification").
/// Fail-closed: every failure path is `Unauthorized` (401) except scope/
/// membership which the service decides as `Forbidden` (403). The reason is
/// logged/counted, never returned (non-enumerable).
pub mod token_auth {
    use super::*;
    use foundry_services::Principal;

    /// Verify a bearer credential and resolve it to a `Principal`.
    ///
    /// DELIVER: parse `Authorization: Bearer <jwt>`; verify the EdDSA signature
    /// with the algorithm allow-list pinned to exactly `[EdDSA]` (reject any
    /// other alg and `alg:none`); validate `exp`; check the `jti` denylist in
    /// the store; build `Principal::Machine`. Missing/malformed/bad-signature/
    /// wrong-alg/expired/forged/revoked all return `ServiceError::Unauthorized`.
    pub fn verify_bearer(_authorization_header: Option<&str>) -> Result<Principal, ServiceError> {
        panic!("{}", NOT_IMPLEMENTED)
    }
}

/// The route handlers. In the real crate each is an `axum` handler wired into
/// `routes(state)`; the scaffold pins the entry points and their service calls.
pub mod routes {
    use super::*;

    /// `GET /api/v1/teams/{team}/projects/{project}/issues` — US-W05a.
    /// Calls `foundry_services::board::list_board_issues` and serializes the
    /// result as a JSON array (empty project -> `[]`, status 200).
    pub fn list_issues(
        _team_slug: &str,
        _project_slug: &str,
    ) -> Result<Vec<IssueJson>, ServiceError> {
        panic!("{}", NOT_IMPLEMENTED)
    }

    /// `POST /api/v1/teams/{team}/projects/{project}/issues` — US-W05c.
    /// 201 + created issue + `Location` header on success.
    pub fn create_issue(
        _team_slug: &str,
        _project_slug: &str,
        _body_json: &str,
    ) -> Result<IssueJson, ServiceError> {
        panic!("{}", NOT_IMPLEMENTED)
    }

    /// `PATCH /api/v1/teams/{team}/projects/{project}/issues/{number}` — US-W05c.
    pub fn change_issue_state(
        _team_slug: &str,
        _project_slug: &str,
        _number: i32,
        _body_json: &str,
    ) -> Result<IssueJson, ServiceError> {
        panic!("{}", NOT_IMPLEMENTED)
    }

    /// `POST .../issues/{number}/comments` — US-W05c.
    pub fn create_comment(
        _team_slug: &str,
        _project_slug: &str,
        _number: i32,
        _body_json: &str,
    ) -> Result<String, ServiceError> {
        panic!("{}", NOT_IMPLEMENTED)
    }

    /// `PATCH .../issues/{number}/comments/{comment_id}` — US-W05c.
    pub fn edit_comment(
        _team_slug: &str,
        _project_slug: &str,
        _number: i32,
        _comment_id: uuid::Uuid,
        _body_json: &str,
    ) -> Result<String, ServiceError> {
        panic!("{}", NOT_IMPLEMENTED)
    }
}
