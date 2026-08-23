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
use crate::views::{
    InstanceDashboardPage, InstanceGrantConfirmedFragment, InstanceProjectRowView,
    InstanceProvisionedFragment, InstanceWorkspaceRow,
};
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
    _csrf: Option<String>,
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
    let Some(user) = require_instance_admin(&state, &session).await else {
        return resource_not_found_page();
    };
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let mut projects_by_workspace = match state.store.list_projects_for_instance().await {
        Ok(rows) => group_project_rows_by_workspace(rows, &csrf),
        Err(err) => return internal_error("list_projects_for_instance", err),
    };
    let workspaces = match state.store.list_workspaces().await {
        Ok(rows) => rows
            .into_iter()
            .map(|(id, name)| InstanceWorkspaceRow {
                workspace_id: id.to_string(),
                name,
                projects: projects_by_workspace.remove(&id).unwrap_or_default(),
            })
            .collect(),
        Err(err) => return internal_error("list_workspaces", err),
    };
    // The caller already passed the fail-closed instance-admin gate
    // (`require_instance_admin`), so the rail's Instance-admin item is present (true);
    // the footer sign-out form reuses the SAME `csrf` double-submit token minted for
    // the provision/grant forms above (04-03).
    let nav = crate::nav::NavContext::home_for(
        &state,
        user.user_id,
        user.workspace_id,
        true,
        csrf.clone(),
    )
    .await;
    let page = InstanceDashboardPage {
        csrf,
        workspaces,
        nav,
    };
    match page.render() {
        Ok(html) => html_with_optional_cookie(html, set_cookie),
        Err(err) => internal_error("render instance_dashboard", err),
    }
}

/// Group the ONE instance-wide project read by workspace (no per-workspace
/// N+1, instance-admin-project-rename 01-01); the per-workspace name order
/// falls out of the query's `ORDER BY p.name`.
fn group_project_rows_by_workspace(
    rows: Vec<foundry_store::InstanceProjectRow>,
    csrf: &str,
) -> std::collections::HashMap<uuid::Uuid, Vec<InstanceProjectRowView>> {
    let mut by_workspace: std::collections::HashMap<uuid::Uuid, Vec<InstanceProjectRowView>> =
        std::collections::HashMap::new();
    for row in rows {
        let workspace_id = row.workspace_id;
        by_workspace
            .entry(workspace_id)
            .or_default()
            .push(project_row_view(row, csrf.to_string()));
    }
    by_workspace
}

/// Adapt one store row + the request's CSRF token into the row partial's
/// view-model — the SAME shape whether rendered by the dashboard loop or
/// returned verbatim as the rename-success fragment (the one-partial rule).
fn project_row_view(
    row: foundry_store::InstanceProjectRow,
    csrf: String,
) -> InstanceProjectRowView {
    InstanceProjectRowView {
        project_id: row.project_id.to_string(),
        name: row.name,
        key_prefix: row.key_prefix,
        team_name: row.team_name,
        csrf,
    }
}

/// Assemble the 200 HTML response, attaching the freshly-minted double-submit
/// CSRF cookie when `ensure_csrf_cookie` produced one.
fn html_with_optional_cookie(html: String, set_cookie: Option<String>) -> Response {
    let mut resp = Html(html).into_response();
    if let Some(cookie) = set_cookie {
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            resp.headers_mut().insert(SET_COOKIE, value);
        }
    }
    resp
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

/// The web grant form: the operator `email` to grant super-admin. The
/// double-submit `_csrf` token is enforced by the surrounding `csrf_middleware`
/// BEFORE this handler runs; the field is accepted (and ignored) here.
#[derive(Debug, Deserialize)]
pub struct GrantForm {
    pub email: String,
    #[serde(rename = "_csrf", default)]
    _csrf: Option<String>,
}

/// `POST /admin/instance/super-admins` — grant a user INSTANCE super-admin from
/// the browser (web-provisioning-flow 01-03, ADR-001 / D1; ADR-004 reuse).
///
/// CSRF is enforced by the surrounding `csrf::csrf_middleware` (a POST with no
/// valid `_csrf` is refused BEFORE this handler runs — ADR-002 / G5). The handler
/// resolves the signed-in INSTANCE super-admin from the session
/// (`require_instance_admin`); a signed-out caller OR a signed-in non-super-admin
/// gets the SHIPPED non-enumerable uniform 404 (`resource_not_found_page`,
/// ADR-002) — byte-identical to a never-existed path.
///
/// On a pass it drives the SHIPPED grant path verbatim (the CLI's proven backend
/// legs): resolve the operator by email (`user_id_by_email`), then record the
/// grant via the idempotent `grant_instance_admin` (`INSERT … ON CONFLICT DO
/// NOTHING`). It renders a NON-COMMITTAL confirmation fragment: the SAME response
/// whether or not the email matched a real user (D2 (g) — the grant form is not a
/// user-enumeration oracle), so an unknown email is a silent no-op confirmed
/// identically.
pub async fn submit_grant(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Form(form): axum::extract::Form<GrantForm>,
) -> Response {
    if require_instance_admin(&state, &session).await.is_none() {
        return resource_not_found_page();
    }

    let email = form.email.trim().to_string();
    let email_lower = email.to_ascii_lowercase();

    // Resolve the operator by email, then grant. An unknown email is a silent
    // no-op (no enumeration oracle): we render the SAME confirmation either way.
    match state.store.user_id_by_email(&email_lower).await {
        Ok(Some(operator_id)) => {
            if let Err(err) = state.store.grant_instance_admin(operator_id).await {
                return internal_error("grant_instance_admin", err);
            }
        }
        Ok(None) => {}
        Err(err) => return internal_error("user_id_by_email", err),
    }

    let fragment = InstanceGrantConfirmedFragment { email };
    match fragment.render() {
        Ok(html) => Html(html).into_response(),
        Err(err) => internal_error("render instance_grant_confirmed", err),
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
        Err(err) => {
            tracing::warn!(
                error = %err,
                "is_instance_admin probe failed; failing closed (404)"
            );
            None
        }
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

// ===========================================================================
// instance-admin-project-rename — the rename write surface
// (`docs/feature/instance-admin-project-rename/design/component-boundaries.md`).
//
// Mounted in `build_router` ALONGSIDE the other `/admin/instance/…` routes (so
// UNDER `csrf::csrf_middleware` + `session_layer` — never the CSRF-exempt
// `/api/v1` mount; D5). The handler owns the HTTP mapping ONLY (copy +
// status); classification is service-owned (`foundry_services::projects`).
// ===========================================================================

/// The rename form: the new display `name`. The double-submit `_csrf` token is
/// enforced by the surrounding `csrf_middleware` BEFORE this handler runs; the
/// field is accepted (and ignored) here.
#[derive(Debug, Deserialize)]
pub struct RenameForm {
    pub name: String,
    #[serde(rename = "_csrf", default)]
    _csrf: Option<String>,
}

/// `POST /admin/instance/projects/{project_id}/rename` — correct a project's
/// display name in place (US-IAPR-02/03). Display name ONLY: slug, board and
/// report URLs, key_prefix, and issue keys are byte-identical before and after
/// (D1 / ADR-PROJECT-RENAME-001).
///
/// The path is `Path<String>`, parsed to `Uuid` IN the handler: a malformed id
/// renders the SAME uniform 404 as a non-admin — no 400-vs-404 enumeration
/// oracle (axum's default `Path<Uuid>` rejection would leak a 400). Success
/// and no-op both answer 200 with the bare row partial (one-partial rule);
/// validation refusals answer 422 with the bare `ErrorFragment` (marker
/// `project-rename-error`) carrying the D4 copy verbatim.
pub async fn submit_project_rename(
    State(state): State<AppState>,
    axum::extract::Path(project_id): axum::extract::Path<String>,
    session: Session,
    headers: HeaderMap,
    axum::extract::Form(form): axum::extract::Form<RenameForm>,
) -> Response {
    let Some(admin) = require_instance_admin(&state, &session).await else {
        return resource_not_found_page();
    };
    let Ok(project_id) = project_id.parse::<uuid::Uuid>() else {
        return resource_not_found_page();
    };
    let services = foundry_services::Services::new(state.store.clone());
    let outcome = services
        .rename_project(foundry_services::projects::RenameProjectRequest {
            acting_user_id: admin.user_id,
            project_id,
            new_name: &form.name,
        })
        .await;
    use foundry_services::projects::{RenameOutcome, RenameProjectError};
    match outcome {
        // Quiet no-op and real rename render the SAME re-mounted row (D4).
        Ok(RenameOutcome::Renamed { .. }) | Ok(RenameOutcome::NoOp { .. }) => {
            render_project_row_fragment(&state, &headers, project_id).await
        }
        // Defence-in-depth refusals and unknown ids collapse to the SAME
        // uniform 404 the session gate returns (no enumeration oracle, D5).
        Err(RenameProjectError::Forbidden) | Err(RenameProjectError::NotFound) => {
            resource_not_found_page()
        }
        // Handler-owned copy, service-owned classification (D4/D6 verbatim).
        Err(RenameProjectError::EmptyName) => {
            rename_error_fragment("Project name must not be empty")
        }
        Err(RenameProjectError::NameTooLong) => {
            rename_error_fragment("Project name must be at most 256 characters")
        }
        Err(RenameProjectError::DuplicateName) => {
            rename_error_fragment("Project name must be unique within the team")
        }
        Err(RenameProjectError::Store(err)) => internal_error("rename_project", err),
    }
}

/// Re-render the ONE row partial (`partials/instance_project_row.html`) as the
/// 200 success fragment — the SAME partial the dashboard loop renders, name
/// freshly read, form re-mounted with a `_csrf` token via `ensure_csrf_cookie`.
/// Reuses the shipped instance-wide listing read (no new store port for a
/// homelab-scale row lookup); a row vanished mid-flight (rename racing a
/// delete) collapses to the uniform 404. A BARE fragment — never `base.html`
/// (double-wrap hazard).
async fn render_project_row_fragment(
    state: &AppState,
    headers: &HeaderMap,
    project_id: uuid::Uuid,
) -> Response {
    let rows = match state.store.list_projects_for_instance().await {
        Ok(rows) => rows,
        Err(err) => return internal_error("list_projects_for_instance", err),
    };
    let Some(row) = rows.into_iter().find(|r| r.project_id == project_id) else {
        return resource_not_found_page();
    };
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    match project_row_view(row, csrf).render() {
        Ok(html) => html_with_optional_cookie(html, set_cookie),
        Err(err) => internal_error("render instance_project_row", err),
    }
}

/// The 422 refusal: the SHARED bare `error_fragment.html` parameterized with
/// the byte-stable `project-rename-error` marker (form-errors.js routes it
/// into the submitting row's `[data-error-slot]`, D6).
fn rename_error_fragment(message: &str) -> Response {
    let body = crate::views::ErrorFragment {
        fragment_marker: "project-rename-error".to_string(),
        message: message.to_string(),
    }
    .render()
    .expect("error_fragment.html renders from a fully-resolved, infallible view-model");
    (StatusCode::UNPROCESSABLE_ENTITY, Html(body)).into_response()
}

/// In-crate unit tests for the two PURE response-assembly helpers (the
/// `board_render_tests` precedent — no `AppState`, no store). Added by DELIVER
/// Phase 5 mutation testing: both helpers' no-op `Default::default()` mutants
/// survived the unit layer because their contracts (the 422 fragment shape and
/// the cookie-attach rule) were pinned only by the slow acceptance lane — the
/// @real-io trap. The full HTTP mapping stays acceptance-pinned (@iapr).
#[cfg(test)]
mod response_helper_tests {
    use super::*;

    async fn body_string(resp: Response) -> String {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("read response body");
        String::from_utf8(bytes.to_vec()).expect("utf-8 body")
    }

    /// The rename refusal is a 422 BARE fragment carrying the byte-stable
    /// `project-rename-error` marker (form-errors.js routes it into the row's
    /// `[data-error-slot]`, D6) and the exact copy the handler chose.
    #[tokio::test]
    async fn rename_error_fragment_is_a_422_with_marker_and_copy() {
        let resp = rename_error_fragment("Project name must not be empty");
        assert_eq!(
            resp.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "a validation refusal must answer 422"
        );
        let body = body_string(resp).await;
        assert!(
            body.contains(r#"data-hx-fragment="project-rename-error""#),
            "the fragment must carry the byte-stable scraper marker; body was:\n{body}"
        );
        assert!(
            body.contains("Project name must not be empty"),
            "the fragment must carry the handler's copy verbatim; body was:\n{body}"
        );
    }

    /// The 200 assembly carries the rendered html and attaches SET_COOKIE
    /// exactly when `ensure_csrf_cookie` minted one.
    #[tokio::test]
    async fn html_with_optional_cookie_carries_html_and_attaches_cookie_iff_minted() {
        let minted = html_with_optional_cookie(
            "<div data-project-row>row</div>".to_string(),
            Some("foundry_csrf=abc; Path=/".to_string()),
        );
        assert_eq!(minted.status(), StatusCode::OK);
        assert_eq!(
            minted
                .headers()
                .get(SET_COOKIE)
                .and_then(|v| v.to_str().ok()),
            Some("foundry_csrf=abc; Path=/"),
            "a freshly-minted CSRF cookie must be attached"
        );
        assert!(
            body_string(minted)
                .await
                .contains("<div data-project-row>row</div>"),
            "the response body must be the rendered html"
        );

        let reused = html_with_optional_cookie("<div>row</div>".to_string(), None);
        assert!(
            reused.headers().get(SET_COOKIE).is_none(),
            "no cookie was minted, so none may be attached"
        );
        assert!(
            body_string(reused).await.contains("<div>row</div>"),
            "the response body must still be the rendered html"
        );
    }
}
