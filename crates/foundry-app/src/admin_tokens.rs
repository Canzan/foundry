//! Admin machine-token surface — `/admin/tokens` (machine-token-admin-ux).
//!
//! Mounted in `build_router` ALONGSIDE the HTML routes (so under
//! `csrf::csrf_middleware` + `session_layer` — NOT the CSRF-exempt `/api/v1`
//! mount; ADR-MT03/DD5). The admin is a browser human, so the session cookie +
//! double-submit `_csrf` field both apply (NFR-MT-SEC-07).
//!
//! Step 01-02 implements the MINT slice (the walking skeleton):
//!   - `GET /admin/tokens` (`show_index`): resolve the signed-in admin from the
//!     session (non-admin → non-enumerable 404, NFR-MT-SEC-03); render the mint
//!     form on an issuer binary or the "issuing not enabled" notice on a
//!     verifier-only binary (OD1/DD2, signer.md); list the workspace's issued
//!     tokens as METADATA ONLY (no value field anywhere — NFR-MT-SEC-02).
//!   - `POST /admin/tokens` (`submit_mint`): admin gate → signer present (else a
//!     clean "not enabled" page, never a 500/partial token) → parse the form →
//!     `services.mint_token(signer, …)` → render `TokenMintedPage` exposing the
//!     `SecretString` value EXACTLY ONCE (DD7) then dropping it. A missing label
//!     is refused 422 with NO value shown (all-or-nothing, NFR-MT-REL-01).
//!
//! `submit_revoke` stays a 501 RED scaffold (step 03-01).
//!
//! The minted `SecretString` is rendered once via `expose_secret()` into the
//! one-time page's owned `String` field and dropped when the handler returns —
//! never stored, never put in a list/detail view, never logged.

use crate::bootstrap::SessionUser;
use crate::csrf::{build_csrf_cookie, generate_token};
use crate::session::SESSION_KEY_USER_ID;
use crate::views::{TokenListPage, TokenMintedPage, TokenRow};
use crate::AppState;
use askama::Template;
use axum::extract::{Path, State};
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use foundry_services::tokens::{MintInput, ScopeChoice};
use foundry_services::{Principal, ServiceError};
use secrecy::ExposeSecret;
use tower_sessions::Session;

/// A non-enumerable refusal: a non-admin (or a non-member) gets the SAME generic
/// 404 whether the surface exists or not (NFR-MT-SEC-03). Nothing in status/body
/// reveals that `/admin/tokens` is a real surface.
fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Not found").into_response()
}

/// `GET /admin/tokens` — the mint form (or the "issuing not enabled" notice on a
/// verifier-only binary) plus the workspace's issued tokens (metadata only).
pub async fn show_index(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(admin) = resolve_admin(&state, &session).await else {
        return not_found();
    };
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let tokens = match load_token_rows(&state, &admin).await {
        Ok(rows) => rows,
        Err(err) => return internal_error("list_tokens", err),
    };
    let page = TokenListPage {
        mint_enabled: state.machine_token_signer.is_some(),
        csrf,
        error: None,
        tokens,
    };
    render_list(page, StatusCode::OK, set_cookie)
}

/// `POST /admin/tokens` — mint. Admin-gated, signer-present-gated, CSRF-enforced
/// by the middleware. On success renders the one-time value page; a missing label
/// is refused 422 with NO value shown.
pub async fn submit_mint(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    axum::extract::Form(form): axum::extract::Form<MintForm>,
) -> Response {
    let Some(admin) = resolve_admin(&state, &session).await else {
        return not_found();
    };

    // Signer-absent (verifier-only binary): a clean "not enabled" page BEFORE any
    // claims are built — never a 500, never a partial token (OD1/DD2).
    let Some(signer) = state.machine_token_signer.clone() else {
        let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
        let tokens = load_token_rows(&state, &admin).await.unwrap_or_default();
        let page = TokenListPage {
            mint_enabled: false,
            csrf,
            error: None,
            tokens,
        };
        return render_list(page, StatusCode::FORBIDDEN, set_cookie);
    };

    let label = form.label.trim().to_string();
    let ttl_days = form.ttl_days.unwrap_or(DEFAULT_TTL_DAYS);

    // A label is required (US-MT01 — "issuing without a label is refused"). We
    // refuse it here, BEFORE minting, so no token value is ever produced for an
    // invalid request (all-or-nothing, NFR-MT-REL-01). A 422 re-renders the form
    // with the error.
    if label.is_empty() {
        return mint_error_response(
            &state,
            &headers,
            &admin,
            ServiceError::Validation {
                code: "label_required".to_string(),
                message: "A label is required".to_string(),
            },
        )
        .await;
    }

    let principal = Principal::Human {
        user_id: admin.user_id,
        workspace_id: admin.workspace_id,
    };
    let scope = resolve_scope(&state, &admin, &form).await;
    let input = MintInput {
        label: label.clone(),
        scope,
        ttl_days,
    };

    let services = foundry_services::Services::new(state.store.clone());
    match services.mint_token(&signer, &principal, input).await {
        Ok(minted) => {
            // Expose the SecretString EXACTLY ONCE into the owned page field,
            // then drop `minted` (and its SecretString) when this scope ends —
            // never stored, never logged (DD7 / NFR-MT-SEC-01).
            let page = TokenMintedPage {
                value_once: minted.value.expose_secret().to_string(),
                jti: minted.jti.to_string(),
                label: minted.label.clone(),
                scope_label: scope_label(minted.scope_team_id, minted.scope_team_name.as_deref()),
                expires_at: format_ts(minted.expires_at),
            };
            match page.render() {
                Ok(html) => Html(html).into_response(),
                Err(err) => internal_error("render token_minted", err),
            }
        }
        Err(err) => mint_error_response(&state, &headers, &admin, err).await,
    }
}

/// `POST /admin/tokens/{jti}/revoke` — revoke (US-MT03). The kill-switch.
///
/// CSRF is enforced by the surrounding `csrf::csrf_middleware` (a POST with no
/// valid `_csrf` is refused 403 BEFORE this handler runs — NFR-MT-SEC-07). The
/// handler resolves the signed-in admin (non-admin → non-enumerable 404,
/// NFR-MT-SEC-03), then drives the `revoke_token` use-case which gates on the
/// admin role again (defense in depth, DD3), enforces workspace isolation with a
/// NON-ENUMERABLE `NotFound` (a missing OR foreign jti reads identically), and
/// flips `revoked_at` (idempotent). Effectiveness is the SHIPPED per-request
/// denylist: the credential's very next `/api/v1` call is refused 401 by the
/// `token_auth` extractor — this handler stamps no new refusal state. On success
/// it redirects back to the list (303) so the row reads "revoked" on reload.
pub async fn submit_revoke(
    State(state): State<AppState>,
    session: Session,
    _headers: HeaderMap,
    Path(jti): Path<uuid::Uuid>,
    _form: axum::extract::RawForm,
) -> Response {
    let Some(admin) = resolve_admin(&state, &session).await else {
        return not_found();
    };
    let principal = Principal::Human {
        user_id: admin.user_id,
        workspace_id: admin.workspace_id,
    };
    let services = foundry_services::Services::new(state.store.clone());
    match services.revoke_token(&principal, jti).await {
        // Idempotent: a fresh revoke and a re-revoke of an already-revoked token
        // both succeed and land back on the list (the row now reads "revoked").
        Ok(()) => Redirect::to("/admin/tokens").into_response(),
        // A missing OR foreign-workspace jti is a non-enumerable 404 — never a
        // 403/200 that would confirm the credential exists.
        Err(ServiceError::NotFound) | Err(ServiceError::Forbidden) => not_found(),
        Err(other) => internal_error("revoke_token", other),
    }
}

// ----------------------------------------------------------------- internals

/// Default rotation cadence when the admin does not pick a TTL (DD8 / OD4).
const DEFAULT_TTL_DAYS: i64 = foundry_services::tokens::DEFAULT_TTL_DAYS;

#[derive(Debug, serde::Deserialize)]
pub struct MintForm {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub ttl_days: Option<i64>,
    /// `"team"` selects a team-scoped grant; anything else (or absent) is a
    /// whole-workspace grant (DD9). The default radio is "workspace".
    #[serde(default)]
    pub scope: Option<String>,
    /// The chosen team when `scope == "team"` — either the team's `Uuid` (the
    /// real picker submits ids) or its name/slug (resolved to an id below). A
    /// value that resolves to no team in the acting workspace stays a foreign
    /// id, which `mint_token` refuses as `scope_team_not_in_workspace` (422).
    #[serde(default)]
    pub team: Option<String>,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

/// Map the mint form's scope/team fields to a [`ScopeChoice`] (DD9). Anything
/// other than `scope == "team"` is a whole-workspace grant. For a team grant the
/// `team` field is either the team's `Uuid` (the real picker submits ids) or its
/// name/slug — resolved to the acting workspace's team id when it matches, else
/// left as the submitted/zero id so `mint_token` refuses it non-enumerably as
/// `scope_team_not_in_workspace` (a foreign team never resolves here). The
/// workspace-membership of the resolved id is re-validated inside the service —
/// this only translates the form choice.
async fn resolve_scope(state: &AppState, admin: &SessionUser, form: &MintForm) -> ScopeChoice {
    if form.scope.as_deref() != Some("team") {
        return ScopeChoice::Workspace;
    }
    let raw = form.team.as_deref().unwrap_or("").trim();
    if let Ok(team_id) = uuid::Uuid::parse_str(raw) {
        return ScopeChoice::Team(team_id);
    }
    let resolved = state
        .store
        .find_team_by_slug(admin.workspace_id, &slugify(raw))
        .await
        .ok()
        .flatten()
        .map(|team| team.id)
        .unwrap_or_else(uuid::Uuid::nil);
    ScopeChoice::Team(resolved)
}

/// Lowercase ASCII-alphanumeric slug (matches `projects::slugify` + the team
/// seed): non-alphanumerics collapse to single hyphens, leading/trailing
/// hyphens trimmed.
fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_hyphen = true;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
            last_hyphen = false;
        } else if !last_hyphen {
            out.push('-');
            last_hyphen = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Resolve the signed-in admin from the session. Returns `None` (→ a generic,
/// non-enumerable 404) when there is no session, the user is not a workspace
/// member, or the user is not a workspace admin (NFR-MT-SEC-03). The service
/// re-checks `is_workspace_admin` too (defense in depth, DD3).
async fn resolve_admin(state: &AppState, session: &Session) -> Option<SessionUser> {
    let user = session
        .get::<SessionUser>(SESSION_KEY_USER_ID)
        .await
        .ok()
        .flatten()?;
    match state
        .store
        .is_workspace_admin(user.workspace_id, user.user_id)
        .await
    {
        Ok(true) => Some(user),
        _ => None,
    }
}

/// Build the metadata-only list rows for the acting admin (newest first) by
/// going THROUGH the `list_tokens` use-case (the driving port — it re-checks the
/// admin gate and scopes the read to the principal's workspace, NFR-MT-REL-03),
/// never the store directly. Each `TokenView` is mapped to a render `TokenRow`
/// with its display status derived (revoked → expired → active) and the minting
/// admin's name resolved ("—" when unattributed, US-MT06). NO value is ever read
/// or rendered (NFR-MT-SEC-02 — `TokenView` has no value field).
async fn load_token_rows(
    state: &AppState,
    admin: &SessionUser,
) -> Result<Vec<TokenRow>, ServiceError> {
    let principal = Principal::Human {
        user_id: admin.user_id,
        workspace_id: admin.workspace_id,
    };
    let services = foundry_services::Services::new(state.store.clone());
    let views = services.list_tokens(&principal).await?;
    let now = time::OffsetDateTime::now_utc();
    Ok(views.into_iter().map(|view| token_row(view, now)).collect())
}

/// Map a value-free [`TokenView`] to a render [`TokenRow`]. Status is derived in
/// priority order: a `revoked` token reads "revoked"; an un-revoked token whose
/// `expires_at` is in the past reads "expired"; otherwise "active". A missing
/// `minted_by` renders "—"; a never-used token renders "never".
fn token_row(view: foundry_services::tokens::TokenView, now: time::OffsetDateTime) -> TokenRow {
    let status = token_status(&view, now);
    let scope = scope_label(view.scope_team_id, view.scope_team_name.as_deref());
    TokenRow {
        jti: view.jti.to_string(),
        label: view.label,
        scope_label: scope,
        expires_at: format_ts(view.expires_at),
        status,
        minted_by: view.minted_by.unwrap_or_else(|| "—".to_string()),
        last_used: match view.last_used_at {
            Some(ts) => format_ts(ts),
            None => "never".to_string(),
        },
    }
}

/// Derive the display status: revoked wins over expired wins over active.
fn token_status(view: &foundry_services::tokens::TokenView, now: time::OffsetDateTime) -> String {
    if view.revoked {
        return "revoked".to_string();
    }
    if view.expires_at < now {
        return "expired".to_string();
    }
    "active".to_string()
}

/// Map a mint `ServiceError` to its HTML response. A `Validation` (e.g. a
/// missing/empty label, or TTL over cap) re-renders the surface with the error
/// and a 422 — NEVER a partial token (all-or-nothing, NFR-MT-REL-01). `Forbidden`
/// collapses to the non-enumerable 404; everything else is a clean 500.
async fn mint_error_response(
    state: &AppState,
    headers: &HeaderMap,
    admin: &SessionUser,
    err: ServiceError,
) -> Response {
    match err {
        ServiceError::Validation { message, .. } => {
            let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
            let tokens = load_token_rows(state, admin).await.unwrap_or_default();
            let page = TokenListPage {
                mint_enabled: state.machine_token_signer.is_some(),
                csrf,
                error: Some(message),
                tokens,
            };
            render_list(page, StatusCode::UNPROCESSABLE_ENTITY, set_cookie)
        }
        ServiceError::Forbidden | ServiceError::NotFound => not_found(),
        other => internal_error("mint_token", other),
    }
}

/// Render a [`TokenListPage`] to a full HTML response at `status`, attaching the
/// CSRF cookie when one was freshly minted. A render `Err` is a clean 500 (the
/// page renders to a complete String before any bytes hit the response, so no
/// half-page can leak).
fn render_list(page: TokenListPage, status: StatusCode, set_cookie: Option<String>) -> Response {
    match page.render() {
        Ok(html) => {
            let mut resp = (status, Html(html)).into_response();
            if let Some(cookie) = set_cookie {
                if let Ok(value) = HeaderValue::from_str(&cookie) {
                    resp.headers_mut().insert(SET_COOKIE, value);
                }
            }
            resp
        }
        Err(err) => internal_error("render token_mint_form", err),
    }
}

/// The scope display label (DD9): a whole-workspace grant reads "Whole
/// workspace"; a team grant reads the resolved team NAME (so the surface names
/// "Backend", never an opaque id). A team that no longer resolves falls back to
/// the id so the row is never blank.
fn scope_label(scope_team_id: Option<uuid::Uuid>, scope_team_name: Option<&str>) -> String {
    match (scope_team_id, scope_team_name) {
        (None, _) => "Whole workspace".to_string(),
        (Some(_), Some(name)) => name.to_string(),
        (Some(team_id), None) => format!("Team {team_id}"),
    }
}

fn format_ts(ts: time::OffsetDateTime) -> String {
    ts.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| ts.unix_timestamp().to_string())
}

fn ensure_csrf_cookie(state: &AppState, headers: &HeaderMap) -> (String, Option<String>) {
    let existing = headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(crate::csrf::extract_csrf_cookie);
    if let Some(token) = existing {
        return (token, None);
    }
    let token = generate_token();
    let cookie = build_csrf_cookie(&token, state.session_cookie_secure);
    (token, Some(cookie))
}

fn internal_error<E: std::fmt::Display>(label: &str, err: E) -> Response {
    tracing::error!(error = %err, "{label} failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}
