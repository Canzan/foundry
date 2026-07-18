//! foundry-api — the JSON API driving adapter (`/api/v1`).
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

use axum::extract::{FromRef, FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{any, delete, get, patch, post};
use axum::Router;
use foundry_auth::MachineTokenVerifier;
use foundry_services::{Principal, ServiceError, Services};

/// US-TMA05 (NFR-TMA-SEC-07) — the per-principal revoke-storm guardrail PORT.
///
/// A driven-port abstraction the DELETE token handler consults AFTER auth and
/// BEFORE `Services::revoke_token`. The concrete in-process token bucket lives
/// in the composition root (`foundry-app`), which implements this trait and
/// exposes it through `AppState`'s `FromRef` seam — so this adapter reads the
/// guardrail WITHOUT depending on foundry-app and WITHOUT gaining a new crate
/// dependency. The guardrail is a transport-rate policy, not a domain rule: the
/// 429 it drives rides adapter-local here, leaving the cross-adapter
/// `ServiceError` contract unchanged.
pub trait RevokeRateGuard: Send + Sync {
    /// Charge one revoke against the calling principal's per-principal budget.
    /// Returns `true` when the revoke is within the guardrail (proceed) or
    /// `false` when the principal's budget is exhausted (refuse 429). The
    /// implementation reads its own clock seam, so callers pass no time.
    fn check_revoke(&self, principal_user_id: uuid::Uuid) -> bool;
}

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

/// `POST .../issues` request body (api-contract.md §"Create-issue request").
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateIssueRequest {
    pub title: String,
    /// The optional issue description (new-issue-dialog-description US-02,
    /// NFR-WEB-API-CON-02). Mirrors the shipped title handling: `#[serde(default)]`
    /// keeps existing clients byte-compatible — a request omitting the key
    /// deserializes to `""`, so the JSON create stores an empty description
    /// exactly as before. When present, it threads through the SAME shared
    /// `create_issue` write use-case the browser handler uses (rule-parity).
    #[serde(default)]
    pub description: String,
}

/// `PATCH .../issues/{number}` request body — a partial state mutation
/// (api-contract.md §"PATCH vs PUT").
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PatchIssueRequest {
    pub state: String,
}

/// `POST .../issues/{number}/comments` request body
/// (api-contract.md §"Create-comment request").
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CommentBodyRequest {
    pub body: String,
}

/// The wire shape of a comment (api-contract.md §"Comment"). `body_html` is the
/// core-sanitized markup (`render_comment_markdown`) — the SAME bytes the UI
/// stores; serializing it inside this JSON *string field* does NOT violate
/// api≠HTML (the boundary guard forbids an HTML *response body*, not a JSON
/// string that happens to contain markup — boundary-guard.md). Serialized from
/// the neutral `foundry_services::comments::CommentView`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommentJson {
    pub id: String,
    pub author_email: String,
    pub body_html: String,
    pub edited: bool,
}

impl From<foundry_services::comments::CommentView> for CommentJson {
    fn from(view: foundry_services::comments::CommentView) -> Self {
        CommentJson {
            id: view.id.to_string(),
            author_email: view.author_email,
            body_html: view.body_html,
            edited: view.edited,
        }
    }
}

/// The wire shape of one issue change event (issue-change-history ADR-002 §2).
/// Serialized by this adapter from the neutral
/// `foundry_services::issues::IssueChangeHistoryEntry`. `actor` is the acting
/// user's email (a stable identifier for integrators, matching the sibling
/// `CommentJson.author_email`); `old` is `null` where the event has no old value
/// (present-but-null key); `at` is an RFC3339 (ISO-8601 UTC) timestamp. The
/// array is ordered oldest→newest (stable audit order for programs — the human
/// timeline is newest-first; ADR-002 watch-item R7).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IssueHistoryJson {
    pub actor: String,
    pub field: String,
    pub old: Option<String>,
    pub new: String,
    pub at: String,
}

impl From<foundry_services::issues::IssueChangeHistoryEntry> for IssueHistoryJson {
    fn from(entry: foundry_services::issues::IssueChangeHistoryEntry) -> Self {
        use time::format_description::well_known::Rfc3339;
        let at = entry
            .at
            .format(&Rfc3339)
            .unwrap_or_else(|_| entry.at.unix_timestamp().to_string());
        IssueHistoryJson {
            actor: entry.actor_email,
            field: entry.field,
            old: entry.old_value,
            new: entry.new_value,
            at,
        }
    }
}

/// The wire shape of a machine-token registry row (api-contract.md §2
/// "TokenJson"). Serialized by this adapter from the neutral
/// `foundry_services::tokens::TokenView`. Field names are the VERBATIM
/// `TokenView` snake_case names. There is deliberately NO `value` / `token` /
/// `secret` / `hash` field — by construction this struct cannot carry a token
/// value (NFR-TMA-SEC-02). `scope_team_id` / `scope_team_name` / `last_used_at`
/// / `minted_by` serialize as JSON `null` when absent; timestamps are RFC3339
/// strings (the conventional JSON timestamp encoding).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenJson {
    pub jti: String,
    pub label: String,
    pub scope_team_id: Option<String>,
    pub scope_team_name: Option<String>,
    pub expires_at: String,
    pub revoked: bool,
    pub last_used_at: Option<String>,
    pub minted_by: Option<String>,
}

impl From<foundry_services::tokens::TokenView> for TokenJson {
    fn from(view: foundry_services::tokens::TokenView) -> Self {
        use time::format_description::well_known::Rfc3339;
        let format_ts = |ts: time::OffsetDateTime| {
            ts.format(&Rfc3339)
                .unwrap_or_else(|_| ts.unix_timestamp().to_string())
        };
        TokenJson {
            jti: view.jti.to_string(),
            label: view.label,
            scope_team_id: view.scope_team_id.map(|id| id.to_string()),
            scope_team_name: view.scope_team_name,
            expires_at: format_ts(view.expires_at),
            revoked: view.revoked,
            last_used_at: view.last_used_at.map(format_ts),
            minted_by: view.minted_by,
        }
    }
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

/// The adapter-local 429 `rate_limited` response (US-TMA05). The guardrail is a
/// transport-rate concern that never reaches the domain, so it does NOT route
/// through `ServiceError` (no `TooManyRequests` variant is added — OD-TMA-5);
/// instead it builds the SHIPPED `ErrorBody` envelope directly with the stable
/// `rate_limited` code, so US-TMA04's "every refusal is a stable machine-readable
/// code" contract still holds.
fn rate_limited_response() -> Response {
    let body = ErrorBody {
        error: ErrorDetail {
            code: "rate_limited".to_string(),
            message: "too many token revocations; slow down".to_string(),
        },
    };
    (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response()
}

/// The `/api/v1` sub-router. Merged into `foundry_app::build_router` via
/// `.merge(foundry_api::routes())`. Generic over the composition root's state
/// `S`: the handler extracts `State<Services>` (the shared seam) and the
/// `MachinePrincipal` bearer extractor reads `Arc<MachineTokenVerifier>` +
/// `Services`, both derived from `S` through `FromRef` (foundry-app implements
/// `FromRef<AppState>` for each), so the sub-router composes into the parent
/// `Router<AppState>` without foundry-api depending on foundry-app — and
/// WITHOUT foundry-api naming `foundry_store::Store` (the `Services` handle
/// owns the only `Store`, satisfying the `foundry-api ⊀ foundry-store`
/// boundary-guard ban, LAYER 2).
///
/// US-TMA05: the DELETE token route additionally reads the per-principal revoke
/// guardrail via `State<Arc<dyn RevokeRateGuard>>` (also `FromRef`-derived from
/// `S`), so the rate policy stays adapter-local — foundry-api never names the
/// concrete bucket and gains no new crate dependency.
///
/// Slice 2 (US-W05b): this group is mounted OUTSIDE the session + CSRF tower
/// layers (auth.md §Coexistence). A machine request carries a bearer JWT and
/// NO cookie, so CSRF-exemption is correct by construction. Authentication is
/// the `MachinePrincipal` `FromRequestParts` extractor. Emits JSON only.
pub fn routes<S>() -> Router<S>
where
    Services: FromRef<S>,
    Arc<MachineTokenVerifier>: FromRef<S>,
    Arc<dyn RevokeRateGuard>: FromRef<S>,
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        // web-provisioning-flow regression fix — a CSRF-EXEMPT, route-absent
        // 404 catch-all scoped to `/api/v1`. The composition root's base
        // `uniform_not_found` fallback sits UNDER `csrf_middleware`, so an
        // UNROUTED `/api/v1/...` request that fell through to it was refused
        // with a CSRF 403 ("CSRF token missing or mismatched") — wrong for the
        // CSRF-exempt bearer surface (slice-06 `@us-mwt07`), which never does
        // CSRF and must answer route-absence with the API's own non-enumerable
        // 404 (the same `ServiceError::NotFound` -> 404 JSON envelope the bearer
        // surface returns for a foreign/unknown resource). Mounting this catch-
        // all on the api router (merged OUTSIDE the csrf/session layers) keeps an
        // unrouted `/api/v1` POST CSRF-exempt: a real api route matches more
        // specifically and wins; only genuinely-absent `/api/v1` paths land here.
        // The HTML base fallback stays CSRF-wrapped (preserving the web POST
        // non-enumerability oracle steps 02-02/02-03 close).
        .route("/api/v1/{*rest}", any(api_not_found_handler))
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/issues",
            get(list_issues_handler).post(create_issue_handler),
        )
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}",
            patch(change_issue_state_handler),
        )
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}/history",
            get(list_issue_history_handler),
        )
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}/comments",
            post(create_comment_handler),
        )
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}/comments/{comment_id}",
            patch(edit_comment_handler),
        )
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/tokens",
            get(list_tokens_handler),
        )
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/tokens/{jti}",
            delete(revoke_token_handler),
        )
}

/// CSRF-exempt route-absent `404` for any unrouted `/api/v1/...` path
/// (web-provisioning-flow regression fix). Returns the API's own non-enumerable
/// NotFound envelope — the SAME `(404, {"error":{"code":"not_found",...}})` the
/// bearer surface returns for a foreign/unknown resource — so a never-existed
/// `/api/v1` path is byte-identical to a gated one (non-enumerability) and never
/// leaks through to the CSRF-wrapped HTML base fallback (which would 403).
async fn api_not_found_handler() -> ApiError {
    ApiError(ServiceError::NotFound)
}

/// `GET /api/v1/teams/{team}/projects/{project}/issues` (US-W05a/b). The
/// `MachinePrincipal` extractor authenticates the bearer credential (fail-
/// closed 401 on any auth failure) BEFORE the handler body runs; it then calls
/// the shared core seam `list_board_issues` (which decides authorization —
/// 403 on scope/membership) and serializes the neutral result as a JSON array
/// (empty project -> `[]`, 200).
async fn list_issues_handler(
    State(services): State<Services>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug)): Path<(String, String)>,
) -> Result<Json<Vec<IssueJson>>, ApiError> {
    let rows = services
        .list_board_issues(&principal, &team_slug, &project_slug)
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

/// `POST /api/v1/teams/{team}/projects/{project}/issues` (US-W05c). Calls the
/// shared `foundry_services::issues::create_issue` write use-case (the SAME
/// path the browser handler uses — identical validation/authz/outbox), then
/// returns `201 Created` with the freshly-allocated issue and a
/// `Location: /api/v1/…/issues/{number}` header (api-contract.md §Create-issue).
/// An empty/over-long title is rejected by the service as `Validation` → 422
/// with the SAME "Title is required" copy the UI shows (rule-parity).
async fn create_issue_handler(
    State(services): State<Services>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug)): Path<(String, String)>,
    Json(request): Json<CreateIssueRequest>,
) -> Result<Response, ApiError> {
    let created = services
        // Rule-parity (NFR-WEB-API-CON-02): thread the optional description
        // through the SAME shared `create_issue` the browser handler uses
        // (identical validation/authz/outbox). An omitted key deserializes to
        // "" (CreateIssueRequest `#[serde(default)]`), preserving the shipped
        // API contract for existing clients.
        .create_issue(
            &principal,
            &team_slug,
            &project_slug,
            &request.title,
            &request.description,
        )
        .await?;
    let body = IssueJson {
        key: created.key,
        number: created.number,
        // Echo the TRIMMED title the service persisted, NOT the raw request
        // title — the returned representation must equal a subsequent read
        // (NFR-WEB-API-CON-02).
        title: created.title,
        state: created.state,
    };
    let location = format!(
        "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{}",
        body.number
    );
    let mut response = (StatusCode::CREATED, Json(body)).into_response();
    if let Ok(value) = HeaderValue::from_str(&location) {
        response.headers_mut().insert(header::LOCATION, value);
    }
    Ok(response)
}

/// `PATCH /api/v1/teams/{team}/projects/{project}/issues/{number}` (US-W05c).
/// Calls `foundry_services::issues::change_issue_state` (which normalizes the
/// state through the SAME `normalize_state` the UI uses) and returns `200` with
/// the updated issue.
async fn change_issue_state_handler(
    State(services): State<Services>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug, number)): Path<(String, String, i32)>,
    Json(request): Json<PatchIssueRequest>,
) -> Result<Json<IssueJson>, ApiError> {
    let updated = services
        .change_issue_state(
            &principal,
            &team_slug,
            &project_slug,
            number,
            &request.state,
            // The JSON API changes state only; positioning is a board concern.
            None,
        )
        .await?;
    Ok(Json(IssueJson {
        key: updated.key,
        number: updated.number,
        title: updated.title,
        state: updated.state,
    }))
}

/// `GET .../issues/{number}/history` (issue-change-history US-03, ADR-002 §2).
/// The `MachinePrincipal` extractor authenticates the bearer credential (fail-
/// closed 401) BEFORE the handler body runs; it then calls the shared core seam
/// `list_issue_change_history` (which decides authorization via the SAME
/// `resolve_member_project` gate the comments/PATCH routes use — 403 on scope/
/// membership, and a foreign/absent issue → the uniform non-enumerable 404 JSON,
/// never a 500). Serializes the neutral entries as a JSON array ordered
/// oldest→newest (stable audit order); an issue with no recorded changes returns
/// `[]` (200). The events are the SAME stored rows the human timeline renders
/// (one source of truth, AC-03.4).
async fn list_issue_history_handler(
    State(services): State<Services>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug, number)): Path<(String, String, i32)>,
) -> Result<Json<Vec<IssueHistoryJson>>, ApiError> {
    let entries = services
        .list_issue_change_history(&principal, &team_slug, &project_slug, number)
        .await?;
    let body = entries.into_iter().map(IssueHistoryJson::from).collect();
    Ok(Json(body))
}

/// `POST .../issues/{number}/comments` (US-W05c). Calls
/// `foundry_services::comments::create_comment`, which sanitizes the body in
/// core (`render_comment_markdown`) — the SAME bytes a browser comment stores
/// (NFR-WEB-API-CON-02) — and returns `201` with the sanitized comment. The
/// `body_html` rides inside a JSON string field (allowed, not an HTML body).
async fn create_comment_handler(
    State(services): State<Services>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug, number)): Path<(String, String, i32)>,
    Json(request): Json<CommentBodyRequest>,
) -> Result<(StatusCode, Json<CommentJson>), ApiError> {
    let view = services
        .create_comment(&principal, &team_slug, &project_slug, number, &request.body)
        .await?;
    Ok((StatusCode::CREATED, Json(view.into())))
}

/// `PATCH .../issues/{number}/comments/{comment_id}` (US-W05c). Calls
/// `foundry_services::comments::edit_comment`, where author-only authz is
/// decided (a non-author edit is `Forbidden` → 403); returns `200` with the
/// updated comment.
async fn edit_comment_handler(
    State(services): State<Services>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug, number, comment_id)): Path<(String, String, i32, uuid::Uuid)>,
    Json(request): Json<CommentBodyRequest>,
) -> Result<Json<CommentJson>, ApiError> {
    let view = services
        .edit_comment(
            &principal,
            &team_slug,
            &project_slug,
            number,
            comment_id,
            &request.body,
        )
        .await?;
    Ok(Json(view.into()))
}

/// `GET /api/v1/teams/{team}/projects/{project}/tokens` (US-TMA01). The
/// `MachinePrincipal` extractor authenticates the bearer credential (fail-
/// closed 401 on any auth failure) BEFORE the handler body runs; it then calls
/// the shared core seam `list_tokens` (which decides authorization — 403 when
/// the bound user is not a workspace admin) and serializes the neutral
/// `TokenView` rows as a JSON array, newest-first (the use-case ORDERs by
/// `created_at DESC`). An empty registry returns `[]` with 200 (never a 404).
/// The `{team}`/`{project}` path segments mirror the issue/comment shape; the
/// use-case is workspace-scoped via the bound principal, so the path is the
/// addressing convention, not an authorization input. No `TokenJson` carries a
/// token value by construction (NFR-TMA-SEC-02).
async fn list_tokens_handler(
    State(services): State<Services>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((_team_slug, _project_slug)): Path<(String, String)>,
) -> Result<Json<Vec<TokenJson>>, ApiError> {
    let views = services.list_tokens(&principal).await?;
    let body = views.into_iter().map(TokenJson::from).collect();
    Ok(Json(body))
}

/// `DELETE /api/v1/teams/{team}/projects/{project}/tokens/{jti}` (US-TMA02/03).
/// The `MachinePrincipal` extractor authenticates the bearer credential (fail-
/// closed 401) BEFORE the handler body runs; the `{jti}` is extracted as a
/// `uuid::Uuid` path param, so a malformed id fails axum extraction before the
/// handler — leaking no existence (api-contract.md §3). It then calls the shared
/// core seam `revoke_token`, where the SHIPPED, mutation-hardened use-case
/// decides authorization (403 when the bound user is not a workspace admin),
/// workspace-confined non-enumerable NotFound (404 — identical for a foreign or
/// unknown jti), and idempotency (a re-revoke of an already-revoked credential
/// is a harmless success). The use-case returns `()` (there is no representation
/// of a deletion), so a success maps to `204 No Content` with no body — keeping
/// the contract minimal and the read-after-write (US-TMA04) honest.
///
/// Kill-switch effectiveness is NOT a new mechanism here: after a successful
/// revoke, the credential's very next `/api/v1` call is refused 401 by the
/// SHIPPED per-request jti denylist (`token_auth::authenticate` ->
/// `Services::resolve_active_token`), unchanged by this handler.
///
/// US-TMA05: AFTER authentication (so there is a `Principal` to key on) and
/// BEFORE `Services::revoke_token`, the bound `user_id` is charged against the
/// per-principal revoke guardrail. An exhausted budget is refused adapter-local
/// as 429 `rate_limited` (never reaching the use-case), bounding a revoke storm
/// from a single leaked bearer. The guardrail keys on the accountable identity
/// (bound `user_id`), so sibling tokens of the same admin share one budget. The
/// read path (LIST) is deliberately unguarded.
async fn revoke_token_handler(
    State(services): State<Services>,
    State(rate_guard): State<Arc<dyn RevokeRateGuard>>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((_team_slug, _project_slug, jti)): Path<(String, String, uuid::Uuid)>,
) -> Result<Response, ApiError> {
    // DELIBERATE ordering (ACCEPTED design decision, not an oversight): the rate
    // guard fires AFTER authn (`MachinePrincipal` resolved a bound identity) and
    // BEFORE the use-case authz (`is_workspace_admin`, inside `revoke_token`). So
    // a throttled NON-admin may receive 429 before it would have received 403.
    // This is accepted because:
    //   (a) it protects the authz DB lookup itself from a revoke-storm — the
    //       guard is the cheap, identity-keyed front line ahead of any query;
    //   (b) the 429-vs-403 distinction leaks NOTHING about token existence — the
    //       non-enumerability contract concerns jtis, which remain byte-identical
    //       404 regardless of this ordering;
    //   (c) the reviewer's alternative (an adapter-side `is_workspace_admin`
    //       pre-check so authz precedes throttling) would VIOLATE the
    //       `check_api_no_adhoc_authz` boundary guard — authz must stay in
    //       foundry-services, never in this adapter. We do NOT add adapter-side
    //       authz.
    if !rate_guard.check_revoke(principal.user_id()) {
        return Ok(rate_limited_response());
    }
    services.revoke_token(&principal, jti).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// The authenticated machine principal, recovered by the bearer-token
/// extractor. Wrapping `Principal` lets axum run authentication via
/// `FromRequestParts` BEFORE the handler body — every `/api/v1` handler that
/// names this in its signature is guarded.
///
/// Fail-closed (auth.md §"Per-request verification"): missing / malformed /
/// bad-signature / wrong-alg / `alg:none` / expired / forged / revoked all
/// reject as `ServiceError::Unauthorized` -> 401 with an IDENTICAL,
/// non-enumerable JSON envelope (the caller is never told WHICH check failed,
/// error-and-observability.md). Scope/membership is decided later by the
/// service as 403.
pub struct MachinePrincipal(pub Principal);

impl<S> FromRequestParts<S> for MachinePrincipal
where
    Arc<MachineTokenVerifier>: FromRef<S>,
    Services: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let verifier = Arc::<MachineTokenVerifier>::from_ref(state);
        let services = Services::from_ref(state);
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok());
        let principal = token_auth::authenticate(header, &verifier, &services).await?;
        Ok(MachinePrincipal(principal))
    }
}

/// The machine-token verification surface (auth.md §"Per-request verification").
/// Fail-closed: every failure path is `Unauthorized` (401) except scope/
/// membership which the service decides as `Forbidden` (403). The reason is
/// logged/counted, never returned (non-enumerable).
pub mod token_auth {
    use super::*;
    use foundry_auth::MachineTokenClaims;

    /// Parse `Authorization: Bearer <jwt>` and verify the credential's
    /// cryptography (auth.md §"Per-request verification" steps 1-3): extract
    /// the bearer token, verify the EdDSA signature against the configured
    /// Ed25519 public key(s) with the algorithm allow-list pinned to exactly
    /// `[EdDSA]` (so any other `alg` — RS256/HS256/… — or `alg:none` is
    /// rejected before any key is consulted), and validate `exp`.
    ///
    /// This is the PURE-CRYPTO half (no DB): missing / malformed / bad-
    /// signature / wrong-alg / `alg:none` / expired / signature-forged all
    /// collapse to `ServiceError::Unauthorized`. The reason is never returned
    /// (non-enumerable refusal). The `jti` denylist check is the separate
    /// store-touching half — see [`authenticate`].
    pub fn verify_bearer(
        authorization_header: Option<&str>,
        verifier: &MachineTokenVerifier,
    ) -> Result<MachineTokenClaims, ServiceError> {
        let jwt = bearer_token(authorization_header).ok_or(ServiceError::Unauthorized)?;
        verifier.verify(jwt).map_err(|_| ServiceError::Unauthorized)
    }

    /// Extract the `<jwt>` from an `Authorization: Bearer <jwt>` header value,
    /// case-insensitively on the scheme. Returns `None` when the header is
    /// absent or not a non-empty bearer credential.
    fn bearer_token(authorization_header: Option<&str>) -> Option<&str> {
        let raw = authorization_header?;
        let (scheme, token) = raw.split_once(' ')?;
        if !scheme.eq_ignore_ascii_case("bearer") {
            return None;
        }
        let token = token.trim();
        if token.is_empty() {
            None
        } else {
            Some(token)
        }
    }

    /// Authenticate a bearer credential end-to-end and resolve it to a
    /// `Principal::Machine` (auth.md §"Per-request verification" steps 1-5):
    /// verify the crypto via [`verify_bearer`], then check the `jti` denylist
    /// through `Services::resolve_active_token` (the SINGLE store touch for this
    /// adapter — routed through the `Services` handle so foundry-api never names
    /// `foundry_store::Store`, satisfying the Phase-04 boundary guard's
    /// `foundry-api ⊀ foundry-store` ban). A best-effort `last_used` touch is
    /// fire-and-forget and never blocks or fails the request.
    ///
    /// Every failure (missing / malformed / bad-signature / wrong-alg /
    /// expired / forged / revoked / unknown-jti) returns
    /// `ServiceError::Unauthorized` — mapped to an IDENTICAL 401 envelope.
    pub async fn authenticate(
        authorization_header: Option<&str>,
        verifier: &MachineTokenVerifier,
        services: &Services,
    ) -> Result<Principal, ServiceError> {
        let claims = verify_bearer(authorization_header, verifier)?;
        let now = time::OffsetDateTime::now_utc();
        let active = services.resolve_active_token(claims.jti, now).await?;
        Ok(Principal::Machine {
            user_id: active.user_id,
            workspace_id: active.workspace_id,
            jti: claims.jti,
            scope_team_id: active.scope_team_id,
        })
    }
}

#[cfg(test)]
mod create_issue_request_tests {
    //! Port-to-port serde test for the `CreateIssueRequest` wire shape
    //! (new-issue-dialog-description US-02). `CreateIssueRequest`'s
    //! deserialization IS the driving port — it is how a JSON body crosses into
    //! `create_issue_handler`. This pins the `#[serde(default)]` back-compat
    //! contract in isolation: an omitted `description` key deserializes to `""`
    //! (existing clients unaffected — S9), a present key is carried verbatim
    //! (S8). It is the fast, Postgres-free complement to the `@real-io` S8/S9
    //! acceptance scenarios (which prove the full store round-trip).
    //!
    //! Behaviour budget: 1 distinct behaviour ("description deserializes:
    //! absent -> \"\", present -> value") x2 = 2. Authored: 1, table-driven over
    //! the equivalence classes (Mandate 5). Falsifiable: dropping
    //! `#[serde(default)]` makes the omitted-key case fail to deserialize.

    use super::*;

    #[test]
    fn description_defaults_when_absent_and_is_carried_when_present() {
        let cases: [(&str, &str, &str); 3] = [
            // (request body, expected title, expected description)
            (r#"{"title":"No body"}"#, "No body", ""),
            (
                r#"{"title":"Rate limit","description":"Return 429."}"#,
                "Rate limit",
                "Return 429.",
            ),
            (
                r#"{"title":"Empty body","description":""}"#,
                "Empty body",
                "",
            ),
        ];
        for (body, want_title, want_description) in cases {
            let request: CreateIssueRequest = serde_json::from_str(body)
                .unwrap_or_else(|err| panic!("deserialize {body:?}: {err}"));
            assert_eq!(request.title, want_title, "title for {body:?}");
            assert_eq!(
                request.description, want_description,
                "description for {body:?} must honour the serde(default) back-compat contract"
            );
        }
    }
}

#[cfg(test)]
mod token_json_tests {
    //! Port-to-port serde test for the `TokenJson` wire shape. `TokenJson` IS
    //! the driving port here — its serialized JSON is the API's observable
    //! contract. This compensates for the LAYER-3 (real-Postgres) example-based
    //! acceptance test with a fast, deterministic assertion that the serialized
    //! key set is EXACTLY the api-contract.md §2 field list and carries no
    //! token/secret/hash/value key by construction (NFR-TMA-SEC-02).
    //!
    //! Behaviour budget: 1 distinct behaviour ("`TokenJson` serializes the
    //! value-free contract key set, with `None` rendered as JSON null") ×2 = 2.
    //! Authored: 1.

    use super::*;
    use std::collections::BTreeSet;

    /// The serialized object exposes EXACTLY the contract keys (api-contract.md
    /// §2) — no more, no fewer — and NONE of the credential-material keys. A
    /// whole-workspace, never-used, unattributed row renders its `Option` fields
    /// as JSON `null` (the keys are still present, value-free).
    #[test]
    fn serializes_exactly_the_value_free_contract_key_set() {
        let token = TokenJson {
            jti: "0190a5c1-2b3d-7e4f-8a9b-1c2d3e4f5a6b".to_string(),
            label: "ci-issue-filer".to_string(),
            scope_team_id: None,
            scope_team_name: None,
            expires_at: "2026-09-05T00:00:00Z".to_string(),
            revoked: false,
            last_used_at: None,
            minted_by: None,
        };

        let value: serde_json::Value = serde_json::to_value(&token).expect("serialize TokenJson");
        let object = value
            .as_object()
            .expect("TokenJson serializes to an object");

        let got: BTreeSet<&str> = object.keys().map(String::as_str).collect();
        let want: BTreeSet<&str> = [
            "jti",
            "label",
            "scope_team_id",
            "scope_team_name",
            "expires_at",
            "revoked",
            "last_used_at",
            "minted_by",
        ]
        .into_iter()
        .collect();
        assert_eq!(
            got, want,
            "TokenJson key set must be exactly the api-contract.md §2 fields"
        );

        for forbidden in ["value", "token", "secret", "hash"] {
            assert!(
                !object.contains_key(forbidden),
                "TokenJson must never carry a {forbidden:?} key (NFR-TMA-SEC-02)"
            );
        }

        // Absent Option fields are present-but-null (the key exists, value-free).
        assert!(object["scope_team_id"].is_null());
        assert!(object["scope_team_name"].is_null());
        assert!(object["last_used_at"].is_null());
        assert!(object["minted_by"].is_null());
    }
}

#[cfg(test)]
mod token_auth_tests {
    //! Port-to-port unit tests for the PURE-CRYPTO half of bearer
    //! authentication (`token_auth::verify_bearer`). The function signature IS
    //! the driving port. The DB-touching half (denylist: forged-jti / revoked /
    //! expired-registry) and the 403 scope branch require real Postgres and are
    //! covered by the `@real-io` us-w05b acceptance scenarios — testing them
    //! here would require mocking inside the hexagon (forbidden).
    //!
    //! Behaviour budget: 2 distinct behaviours of `verify_bearer`
    //!   (1) accept a well-formed EdDSA credential -> recover its claims,
    //!   (2) refuse every malformed/invalid credential -> identical Unauthorized.
    //! Budget = 2 x 2 = 4 unit tests. Authored: 2 (happy + parametrized-refusal),
    //! well within budget. The refusal catalogue is a SINGLE parametrized test
    //! (Mandate 5) over the equivalence classes, also asserting non-enumerability
    //! (every refusal yields the byte-identical `ServiceError::Unauthorized`).

    use super::*;
    use foundry_auth::{test_keys, MachineTokenClaims, MachineTokenSigner};
    use jsonwebtoken::{encode, EncodingKey, Header};
    use secrecy::ExposeSecret;

    fn now() -> i64 {
        time::OffsetDateTime::now_utc().unix_timestamp()
    }

    fn claims(exp_offset_secs: i64) -> MachineTokenClaims {
        let iat = now();
        MachineTokenClaims {
            sub: uuid::Uuid::now_v7(),
            scope: None,
            iat,
            exp: iat + exp_offset_secs,
            jti: uuid::Uuid::now_v7(),
            iss: foundry_auth::MACHINE_TOKEN_ISS.to_string(),
            aud: foundry_auth::MACHINE_TOKEN_AUD.to_string(),
        }
    }

    fn mint_valid(signer: &MachineTokenSigner, claims: &MachineTokenClaims) -> String {
        signer
            .mint(claims)
            .expect("mint")
            .expose_secret()
            .to_string()
    }

    /// Behaviour (1): a well-formed, unexpired EdDSA credential minted by the
    /// matching signing key is accepted and its claims (jti/sub/scope) are
    /// recovered verbatim — the data the extractor binds into `Principal`.
    #[test]
    fn accepts_valid_eddsa_credential_and_recovers_claims() {
        let verifier = test_keys::verifier();
        let signer = test_keys::signer();
        let want = claims(3600);
        let header = format!("Bearer {}", mint_valid(&signer, &want));

        let got = token_auth::verify_bearer(Some(&header), &verifier)
            .expect("a valid EdDSA credential authenticates");

        assert_eq!(got.jti, want.jti, "recovered jti must match");
        assert_eq!(got.sub, want.sub, "recovered sub must match");
        assert_eq!(got.scope, want.scope, "recovered scope must match");
    }

    /// Behaviour (2): the NON-ENUMERABLE refusal catalogue. Every malformed or
    /// invalid credential — across all equivalence classes — is refused with
    /// the byte-identical `ServiceError::Unauthorized`. No class leaks WHY it
    /// failed (auth.md / error-and-observability.md). The HS256 and `alg:none`
    /// cases prove the alg-confusion footgun is closed (the verifier pins
    /// exactly `[EdDSA]`).
    #[test]
    fn refuses_every_invalid_credential_non_enumerably() {
        let verifier = test_keys::verifier();
        let signer = test_keys::signer();

        // A bad-signature token: forge with a DIFFERENT Ed25519 key the
        // verifier does not trust.
        let other_signer = MachineTokenSigner::from_pkcs8_pem(&secrecy::SecretString::new(
            wrong_signing_key_pem().into(),
        ))
        .expect("build a second valid signer");
        let bad_sig = format!("Bearer {}", mint_valid(&other_signer, &claims(3600)));

        // An expired-but-validly-signed token (exp in the past).
        let expired = format!("Bearer {}", mint_valid(&signer, &claims(-3600)));

        // A wrong-algorithm token: HS256 (the classic public-key-as-HMAC-secret
        // attack). The verifier pins `[EdDSA]`, so it is rejected before any
        // key is consulted.
        let hs256 = {
            let header = Header::new(jsonwebtoken::Algorithm::HS256);
            let key = EncodingKey::from_secret(b"attacker-chosen-hmac-secret");
            format!(
                "Bearer {}",
                encode(&header, &claims(3600), &key).expect("hs256 encode")
            )
        };

        // An `alg:none` token: header alg=none, empty signature.
        let alg_none = {
            let header = "{\"alg\":\"none\",\"typ\":\"JWT\"}";
            let payload = serde_json::to_string(&claims(3600)).expect("payload json");
            let b64 = |b: &[u8]| {
                use base64::Engine;
                base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
            };
            format!(
                "Bearer {}.{}.",
                b64(header.as_bytes()),
                b64(payload.as_bytes())
            )
        };

        let cases: Vec<(&str, Option<String>)> = vec![
            ("missing header", None),
            ("empty header", Some(String::new())),
            ("wrong scheme (Basic)", Some("Basic abc".into())),
            ("bearer with no token", Some("Bearer ".into())),
            ("malformed jwt", Some("Bearer not.a.jwt".into())),
            (
                "garbage",
                Some("Bearer !!!not-a-valid-credential!!!".into()),
            ),
            ("bad signature (foreign key)", Some(bad_sig)),
            ("expired", Some(expired)),
            ("wrong alg (HS256)", Some(hs256)),
            ("alg:none", Some(alg_none)),
        ];

        for (label, header) in cases {
            let result = token_auth::verify_bearer(header.as_deref(), &verifier);
            match result {
                Err(ServiceError::Unauthorized) => {}
                other => panic!(
                    "case {label:?} must refuse as the identical Unauthorized, got {other:?}"
                ),
            }
        }
    }

    /// A second, valid Ed25519 PKCS#8 key distinct from the fixed test key —
    /// used only to forge a bad-signature token. (Generated offline; any valid
    /// Ed25519 key the verifier does not hold works.)
    fn wrong_signing_key_pem() -> String {
        // ring/ed25519 PKCS#8 v2 for a throwaway keypair (NOT the test key).
        "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEINTuctv5E1hK1bbY8fdp+K06/nwoy/HU++CXqI9EdVhC\n-----END PRIVATE KEY-----\n".to_string()
    }
}
