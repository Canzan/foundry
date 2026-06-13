//! Instance super-admin web surface — `/admin/instance/…` (web-provisioning-flow).
//!
//! A NEW WEB DRIVING ADAPTER over the ALREADY-SHIPPED `Services::provision_workspace`
//! use-case + `is_instance_admin` authz (the CLI legs shipped in
//! us-mwt-slice-06-provision-and-prove; this is the browser surface CLI-first v1
//! deferred — multi-workspace-provisioning ADR-002 D2 → realised HERE).
//!
//! Mounted in `build_router` ALONGSIDE the HTML routes (so under
//! `csrf::csrf_middleware` + `session_layer` — NOT the CSRF-exempt `/api/v1`
//! mount). The super-admin is a browser human, so the session cookie +
//! double-submit `_csrf` field both apply (ADR-002 / G5).
//!
//! Step 01-01 implements the WALKING SKELETON: the thinnest end-to-end web
//! vertical proving a signed-in super-admin can provision a NEW isolated
//! workspace from the browser:
//!   - `POST /admin/instance/workspaces` (`submit_provision`): the
//!     `require_instance_admin` session gate (the web analogue of the CLI's
//!     fail-closed `is_instance_admin` gate — a non-super-admin / signed-out
//!     caller gets the SHIPPED non-enumerable uniform 404, ADR-002); parse the
//!     form (workspace name + first-admin email) with a valid `_csrf`; call the
//!     SHIPPED `Services::provision_workspace`; render an htmx success fragment
//!     reporting the new workspace id + the (informational, D5) first-admin
//!     invite link.
//!
//! The instance dashboard GET (01-02), the grant surface (01-03/04), the
//! non-enumerable refusal matrix (02-xx), and the legacy-route retirement (03-01)
//! are LATER steps — not implemented here.
//!
//! LAYER-1e (ADR-004 / D6): this file is INSTANCE-scoped, not tenant-scoped — it
//! legitimately drives the super-admin provisioning path that creates a brand-new
//! workspace id (it does not scope a tenant query by a request-parsed workspace
//! id). Its stem is on the `check_arch.rs` tenant-scoping allow-list.

use crate::bootstrap::{resource_not_found_page, SessionUser};
use crate::csrf::{build_csrf_cookie, generate_token};
use crate::session::SESSION_KEY_USER_ID;
use crate::views::{InstanceDashboardPage, InstanceProvisionedFragment, InstanceWorkspaceRow};
use crate::AppState;
use askama::Template;
use axum::extract::State;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use foundry_services::provisioning::ProvisionRequest;
use foundry_services::ServiceError;
use secrecy::SecretString;
use serde::Deserialize;
use tower_sessions::Session;

/// How long a freshly-provisioned first-admin invite is valid. Mirrors the CLI
/// provisioning leg (`admin_cli.rs` — `now + 7 days`).
const INVITE_TTL_DAYS: i64 = 7;

/// The web provision form: a workspace `name` + the first admin's `email`. The
/// double-submit `_csrf` token is enforced by the surrounding `csrf_middleware`
/// BEFORE this handler runs; the field is accepted (and ignored) here.
#[derive(Debug, Deserialize)]
pub struct ProvisionForm {
    pub name: String,
    pub email: String,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

/// `GET /admin/instance/workspaces` — the instance super-admin DASHBOARD: the
/// full-page (no-JS) entry point of the surface (web-provisioning-flow 01-02,
/// ADR-001 / D1). Resolves the signed-in INSTANCE super-admin from the session
/// (`require_instance_admin`); a signed-out caller OR a signed-in non-super-admin
/// gets the SHIPPED non-enumerable uniform 404 (`resource_not_found_page`,
/// ADR-002) — byte-identical to a never-existed path. On a pass it renders the
/// existing-workspace list (the thin `list_workspaces` read, D4) plus BOTH
/// state-changing forms (provision + grant), each carrying the double-submit
/// `_csrf` field the surrounding `csrf_middleware` enforces on the POST.
pub async fn show_dashboard(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    if require_instance_admin(&state, &session).await.is_none() {
        return resource_not_found_page();
    }
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let workspaces = match state.store.list_workspaces().await {
        Ok(rows) => rows
            .into_iter()
            .map(|(id, name)| InstanceWorkspaceRow {
                workspace_id: id.to_string(),
                name,
            })
            .collect(),
        Err(err) => return internal_error("list_workspaces", err),
    };
    let page = InstanceDashboardPage { csrf, workspaces };
    match page.render() {
        Ok(html) => {
            let mut resp = Html(html).into_response();
            if let Some(cookie) = set_cookie {
                if let Ok(value) = HeaderValue::from_str(&cookie) {
                    resp.headers_mut().insert(SET_COOKIE, value);
                }
            }
            resp
        }
        Err(err) => internal_error("render instance_dashboard", err),
    }
}

/// `POST /admin/instance/workspaces` — provision a NEW isolated workspace + first
/// admin from the browser (US-MWT07 web leg, walking skeleton).
///
/// CSRF is enforced by the surrounding `csrf::csrf_middleware` (a POST with no
/// valid `_csrf` is refused BEFORE this handler runs — ADR-002 / G5). The handler
/// resolves the signed-in INSTANCE super-admin from the session
/// (`require_instance_admin`); a signed-out caller OR a signed-in non-super-admin
/// gets the SHIPPED non-enumerable uniform 404 (`resource_not_found_page`,
/// ADR-002) — byte-identical to a never-existed path, no 403/401/redirect oracle.
/// On a pass it drives the SHIPPED `Services::provision_workspace` use-case (which
/// re-checks `is_instance_admin`, defence-in-depth) and renders the htmx success
/// fragment carrying the new workspace id + the informational first-admin invite
/// link (D5 — no sign-in via it).
pub async fn submit_provision(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Form(form): axum::extract::Form<ProvisionForm>,
) -> Response {
    let Some(admin) = require_instance_admin(&state, &session).await else {
        return resource_not_found_page();
    };

    let now = state.clock.now();
    let request = ProvisionRequest {
        acting_user_id: admin.user_id,
        workspace_name: form.name.trim(),
        admin_email: form.email.trim(),
        admin_password: SecretString::new(generate_initial_password().into()),
        invite_expires_at: now + time::Duration::days(INVITE_TTL_DAYS),
    };

    let services = foundry_services::Services::new(state.store.clone());
    match services.provision_workspace(request).await {
        Ok(provisioned) => {
            let invite_link = match build_invite_link(&state, &provisioned) {
                Ok(link) => link,
                Err(err) => return internal_error("sign invite link", err),
            };
            let fragment = InstanceProvisionedFragment {
                workspace_id: provisioned.workspace_id.to_string(),
                workspace_name: form.name.trim().to_string(),
                first_admin_email: form.email.trim().to_string(),
                invite_link,
            };
            match fragment.render() {
                Ok(html) => Html(html).into_response(),
                Err(err) => internal_error("render instance_provisioned", err),
            }
        }
        // Defence-in-depth: the use-case re-checks the super-admin gate and
        // refuses non-enumerably — collapse to the SAME uniform 404 the session
        // gate returns (no oracle that the surface or target exists).
        Err(ServiceError::Forbidden) | Err(ServiceError::NotFound) => resource_not_found_page(),
        Err(other) => internal_error("provision_workspace", other),
    }
}

// ----------------------------------------------------------------- internals

/// Resolve the signed-in INSTANCE super-admin from the session (the web analogue
/// of the CLI's fail-closed `is_instance_admin` gate). Returns `None` — driving a
/// SHIPPED non-enumerable uniform 404 — when there is no session OR the signed-in
/// user is not an `instance_admins` row (ADR-002). A session-store ERROR fails
/// closed identically but is logged so an operator can tell a broken store from
/// ordinary non-admin traffic (mirrors `admin_tokens::resolve_admin`).
async fn require_instance_admin(state: &AppState, session: &Session) -> Option<SessionUser> {
    let user = match session.get::<SessionUser>(SESSION_KEY_USER_ID).await {
        Ok(maybe_user) => maybe_user?,
        Err(err) => {
            tracing::warn!(
                error = %err,
                "session store error resolving super-admin for /admin/instance; failing closed (404)"
            );
            return None;
        }
    };
    match state.store.is_instance_admin(user.user_id).await {
        Ok(true) => Some(user),
        _ => None,
    }
}

/// Build the (informational) first-admin invite link from the provisioned invite
/// row, mirroring the CLI provisioning leg (`admin_cli.rs`): the signed
/// `/invites/accept?id=…&sig=…` URL rooted at `state.public_url`. Per D5 the link
/// is RENDERED for the operator to relay — there is NO accept route in v1.
fn build_invite_link(
    state: &AppState,
    provisioned: &foundry_services::provisioning::Provisioned,
) -> Result<String, foundry_auth::AuthError> {
    let token = foundry_auth::InviteToken::new(
        provisioned.invite_id,
        provisioned.invite_expires_at,
        &state.session_secret,
    )?;
    Ok(format!(
        "{}/invites/accept?id={}&sig={}",
        state.public_url.trim_end_matches('/'),
        provisioned.invite_id,
        urlencoding::encode(&token.signature),
    ))
}

/// A random initial credential for the provisioned first admin. The first admin
/// resets it by accepting the emitted invite link; it is never shown to the
/// super-admin (it exists only so the admin row is well-formed). Mirrors the
/// CLI's `generate_provisioning_password`.
fn generate_initial_password() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Reuse the request's existing double-submit `foundry_csrf` cookie when present,
/// else mint one (mirrors `admin_tokens::ensure_csrf_cookie`). The returned token
/// is rendered into the dashboard forms' hidden `_csrf` field; the optional cookie
/// is attached to the response so a fresh visitor's first POST has a matching pair.
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
