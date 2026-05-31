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

use axum::extract::{FromRef, FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, patch, post};
use axum::Router;
use foundry_auth::MachineTokenVerifier;
use foundry_services::{Principal, ServiceError};
use foundry_store::Store;

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
/// `S`: the handler extracts `State<Arc<Store>>` (board read seam) and the
/// `MachinePrincipal` bearer extractor reads `Arc<MachineTokenVerifier>` +
/// `Arc<Store>`, both derived from `S` through `FromRef` (foundry-app
/// implements `FromRef<AppState>` for each), so the sub-router composes into
/// the parent `Router<AppState>` without foundry-api depending on foundry-app.
///
/// Slice 2 (US-W05b): this group is mounted OUTSIDE the session + CSRF tower
/// layers (auth.md §Coexistence). A machine request carries a bearer JWT and
/// NO cookie, so CSRF-exemption is correct by construction. Authentication is
/// the `MachinePrincipal` `FromRequestParts` extractor. Emits JSON only.
pub fn routes<S>() -> Router<S>
where
    Arc<Store>: FromRef<S>,
    Arc<MachineTokenVerifier>: FromRef<S>,
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/issues",
            get(list_issues_handler).post(create_issue_handler),
        )
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}",
            patch(change_issue_state_handler),
        )
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}/comments",
            post(create_comment_handler),
        )
        .route(
            "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}/comments/{comment_id}",
            patch(edit_comment_handler),
        )
}

/// `GET /api/v1/teams/{team}/projects/{project}/issues` (US-W05a/b). The
/// `MachinePrincipal` extractor authenticates the bearer credential (fail-
/// closed 401 on any auth failure) BEFORE the handler body runs; it then calls
/// the shared core seam `list_board_issues` (which decides authorization —
/// 403 on scope/membership) and serializes the neutral result as a JSON array
/// (empty project -> `[]`, 200).
async fn list_issues_handler(
    State(store): State<Arc<Store>>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug)): Path<(String, String)>,
) -> Result<Json<Vec<IssueJson>>, ApiError> {
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

/// `POST /api/v1/teams/{team}/projects/{project}/issues` (US-W05c). Calls the
/// shared `foundry_services::issues::create_issue` write use-case (the SAME
/// path the browser handler uses — identical validation/authz/outbox), then
/// returns `201 Created` with the freshly-allocated issue and a
/// `Location: /api/v1/…/issues/{number}` header (api-contract.md §Create-issue).
/// An empty/over-long title is rejected by the service as `Validation` → 422
/// with the SAME "Title is required" copy the UI shows (rule-parity).
async fn create_issue_handler(
    State(store): State<Arc<Store>>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug)): Path<(String, String)>,
    Json(request): Json<CreateIssueRequest>,
) -> Result<Response, ApiError> {
    let created = foundry_services::issues::create_issue(
        &store,
        &principal,
        &team_slug,
        &project_slug,
        &request.title,
    )
    .await?;
    let body = IssueJson {
        key: created.key,
        number: created.number,
        title: request.title,
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
    State(store): State<Arc<Store>>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug, number)): Path<(String, String, i32)>,
    Json(request): Json<PatchIssueRequest>,
) -> Result<Json<IssueJson>, ApiError> {
    let updated = foundry_services::issues::change_issue_state(
        &store,
        &principal,
        &team_slug,
        &project_slug,
        number,
        &request.state,
    )
    .await?;
    Ok(Json(IssueJson {
        key: updated.key,
        number: updated.number,
        title: updated.title,
        state: updated.state,
    }))
}

/// `POST .../issues/{number}/comments` (US-W05c). Calls
/// `foundry_services::comments::create_comment`, which sanitizes the body in
/// core (`render_comment_markdown`) — the SAME bytes a browser comment stores
/// (NFR-WEB-API-CON-02) — and returns `201` with the sanitized comment. The
/// `body_html` rides inside a JSON string field (allowed, not an HTML body).
async fn create_comment_handler(
    State(store): State<Arc<Store>>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug, number)): Path<(String, String, i32)>,
    Json(request): Json<CommentBodyRequest>,
) -> Result<(StatusCode, Json<CommentJson>), ApiError> {
    let view = foundry_services::comments::create_comment(
        &store,
        &principal,
        &team_slug,
        &project_slug,
        number,
        &request.body,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(view.into())))
}

/// `PATCH .../issues/{number}/comments/{comment_id}` (US-W05c). Calls
/// `foundry_services::comments::edit_comment`, where author-only authz is
/// decided (a non-author edit is `Forbidden` → 403); returns `200` with the
/// updated comment.
async fn edit_comment_handler(
    State(store): State<Arc<Store>>,
    MachinePrincipal(principal): MachinePrincipal,
    Path((team_slug, project_slug, number, comment_id)): Path<(String, String, i32, uuid::Uuid)>,
    Json(request): Json<CommentBodyRequest>,
) -> Result<Json<CommentJson>, ApiError> {
    let view = foundry_services::comments::edit_comment(
        &store,
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
    Arc<Store>: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let verifier = Arc::<MachineTokenVerifier>::from_ref(state);
        let store = Arc::<Store>::from_ref(state);
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok());
        let principal = token_auth::authenticate(header, &verifier, &store).await?;
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
    /// through the `foundry_services::auth` helper (the SINGLE allowed
    /// foundry-store touch for this adapter — routed through services so the
    /// adapter never depends on foundry-store for the denylist, staying ahead
    /// of the Phase-04 boundary guard). A best-effort `last_used` touch is
    /// fire-and-forget and never blocks or fails the request.
    ///
    /// Every failure (missing / malformed / bad-signature / wrong-alg /
    /// expired / forged / revoked / unknown-jti) returns
    /// `ServiceError::Unauthorized` — mapped to an IDENTICAL 401 envelope.
    pub async fn authenticate(
        authorization_header: Option<&str>,
        verifier: &MachineTokenVerifier,
        store: &Store,
    ) -> Result<Principal, ServiceError> {
        let claims = verify_bearer(authorization_header, verifier)?;
        let now = time::OffsetDateTime::now_utc();
        let active = foundry_services::auth::resolve_active_token(store, claims.jti, now).await?;
        Ok(Principal::Machine {
            user_id: active.user_id,
            workspace_id: active.workspace_id,
            jti: claims.jti,
            scope_team_id: active.scope_team_id,
        })
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
