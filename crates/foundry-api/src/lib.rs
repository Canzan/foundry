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

use foundry_services::ServiceError;

/// Marker for the boundary guard's "no scaffolds remain" sweep
/// (`grep -rn "SCAFFOLD: true" crates/`).
pub const __SCAFFOLD__: bool = true;

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

/// Map a `ServiceError` to its `(status, envelope-code, message)` per
/// error-and-observability.md. The real adapter does this inside
/// `impl IntoResponse for ApiError`; the scaffold pins the mapping table so
/// DELIVER's implementation is checked against it.
pub fn status_for(_err: &ServiceError) -> (u16, ErrorBody) {
    panic!("{}", NOT_IMPLEMENTED)
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
