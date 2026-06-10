//! Session backend: tower-sessions over a Postgres-backed `session`
//! table (per `design/auth.md` § Sessions).
//!
//! The `session` table is created by migration `0002_sessions_and_reset.sql`
//! so all schema changes live in one ordered set; we do NOT call
//! `PostgresStore::migrate()` (which would create a separate
//! `tower_sessions` schema). Instead, [`build_session_layer`] hands
//! `PostgresStore` the schema name we already migrated into.
//!
//! Cookie shape (NFR-SEC-03):
//!   - name `foundry_session`
//!   - `HttpOnly`, `SameSite=Lax`, `Path=/`, `Secure` toggled by
//!     `SESSION_COOKIE_SECURE` env var (default `true`)
//!   - 30-day Max-Age (`Expiry::OnInactivity(30 days)`)

use sqlx::PgPool;
use std::time::Duration as StdDuration;
use time::Duration as TimeDuration;
use tower_sessions::cookie::{time::Duration as CookieDuration, SameSite};
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;

/// The RESOLVED acting workspace for a request (ADR-002, multi-workspace-tenancy).
///
/// A newtype over the workspace `Uuid` that a handler must scope every
/// tenant-scoped read/write by. It is produced ONLY by the request-workspace
/// resolution seam (ADR-001 / `SessionUser::acting_workspace`), never parsed from
/// a path/query/body parameter — so "the workspace came from the trusted seam" is
/// the only well-typed path into a tenant-scoped store call. A client-supplied id
/// is then a type mismatch at the call boundary, making "forgot to scope" (or
/// "scoped by an attacker-controlled id") structurally hard rather than a matter
/// of convention. The `check-arch` LAYER-1e tenant-scoping guard locks this in at
/// build time (xtask/src/check_arch.rs).
///
/// `Copy` so threading it through handler call sites is frictionless; it carries
/// no secret beyond the workspace id the session already holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActingWorkspace(uuid::Uuid);

impl ActingWorkspace {
    /// Wrap a workspace id that came from the trusted resolution seam.
    ///
    /// Intentionally `pub(crate)` and named for the seam: the ONLY production
    /// caller is `SessionUser::acting_workspace`, which reads the
    /// session-resolved `workspace_id` (stamped at sign-in by
    /// `resolve_active_workspace`, ADR-005). Handlers never call this with a
    /// path/query/body id.
    pub(crate) fn from_resolved(workspace_id: uuid::Uuid) -> Self {
        Self(workspace_id)
    }

    /// The underlying workspace id, for the `WHERE … AND workspace_id = $n`
    /// scoping clause every tenant-scoped store method takes.
    pub fn workspace_id(self) -> uuid::Uuid {
        self.0
    }
}

pub const SESSION_COOKIE_NAME: &str = "foundry_session";
/// Key under which we store the signed-in user id inside the session
/// data map. Workspace + team memberships are looked up per-request
/// (design/auth.md: keep session data thin so memberships can rotate
/// without invalidating sessions).
pub const SESSION_KEY_USER_ID: &str = "user_id";

/// Build the tower-sessions middleware layer pointing at the Postgres
/// `session` table inside `schema_name` (e.g. "public" in production,
/// or a per-scenario schema in tests).
pub fn build_session_layer(
    pool: PgPool,
    schema_name: &str,
    cookie_secure: bool,
) -> SessionManagerLayer<PostgresStore> {
    let store = PostgresStore::new(pool)
        .with_schema_name(schema_name)
        .expect("schema name is a valid identifier");
    SessionManagerLayer::new(store)
        .with_name(SESSION_COOKIE_NAME)
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        .with_path("/".to_string())
        .with_secure(cookie_secure)
        // NFR-SEC-03: cookie + server-side row valid for 30 days.
        .with_expiry(Expiry::OnInactivity(CookieDuration::days(30)))
}

/// 30 days — exposed for assertion / store-side helpers.
pub const SESSION_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;

#[allow(dead_code)]
pub const SESSION_TTL_TIME: TimeDuration = TimeDuration::seconds(SESSION_TTL_SECONDS);

#[allow(dead_code)]
pub const SESSION_TTL_STD: StdDuration = StdDuration::from_secs(SESSION_TTL_SECONDS as u64);

// ----------------------------------------------------------- POST /workspace/switch

use crate::bootstrap::{resource_not_found_page, SessionUser};
use crate::AppState;
use axum::extract::{Form, State};
use axum::http::header::{HeaderMap, HeaderValue, LOCATION};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use tower_sessions::Session;

#[derive(Debug, Deserialize)]
pub struct SwitchForm {
    /// The workspace the user wants to act on next. A membership of the
    /// signed-in user — NOT a free-form scoping id. It is validated against the
    /// user's `workspace_memberships` by [`crate::Store::set_active_workspace`]
    /// before anything is re-stamped (privilege boundary).
    pub workspace_id: uuid::Uuid,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

/// POST `/workspace/switch` — change the session's ACTIVE workspace for a
/// multi-membership user (ADR-005, step 02-05).
///
/// Runs UNDER the session + double-submit CSRF layers (registered alongside the
/// other browser POSTs in `build_router`), so it requires a signed-in
/// `foundry_session` cookie and a matching `_csrf` token like every other web
/// write.
///
/// SECURITY — fail-closed privilege boundary: the target is accepted ONLY if the
/// signed-in user is a MEMBER of it. [`crate::Store::set_active_workspace`] does
/// the membership-guarded write atomically and returns `false` for a non-member;
/// we then refuse with the SAME non-enumerable 404 a cross-tenant resource reach
/// returns (ADR-003), so switching to a workspace the user is not a member of
/// neither succeeds NOR reveals that the workspace exists.
///
/// On success we persist the choice (so a subsequent — even fresh — sign-in
/// resolves to it via `resolve_active_workspace`) AND re-stamp THIS session's
/// `SessionUser.workspace_id` so the very next request on the current cookie
/// already scopes to the new tenant through the SHIPPED `acting_workspace` seam —
/// no scoping logic is re-implemented here.
pub async fn submit_switch(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<SwitchForm>,
) -> Response {
    let Some(user) = session
        .get::<SessionUser>(SESSION_KEY_USER_ID)
        .await
        .ok()
        .flatten()
    else {
        // Not signed in — same redirect-to-sign-in shape the other web surfaces use.
        let mut hdrs = HeaderMap::new();
        hdrs.insert(LOCATION, HeaderValue::from_static("/sign-in"));
        return (StatusCode::SEE_OTHER, hdrs, "").into_response();
    };

    // Membership-guarded, fail-closed: a non-member write returns false (no row
    // touched) and we refuse non-enumerably.
    match state
        .store
        .set_active_workspace(user.user_id, form.workspace_id)
        .await
    {
        Ok(true) => {}
        Ok(false) => return resource_not_found_page(),
        Err(err) => {
            tracing::error!(%err, "set_active_workspace failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    }

    // Re-stamp the CURRENT session so the next request on this cookie already
    // acts on the switched workspace (the persisted column covers fresh sign-ins;
    // this covers the live session). Reuses the same SessionUser shape sign-in
    // stamps — the scoped reads downstream go through `acting_workspace` unchanged.
    if let Err(err) = session
        .insert(
            SESSION_KEY_USER_ID,
            SessionUser {
                user_id: user.user_id,
                workspace_id: form.workspace_id,
            },
        )
        .await
    {
        tracing::error!(%err, "session.insert failed during workspace switch");
        return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
    }

    let mut hdrs = HeaderMap::new();
    hdrs.insert(LOCATION, HeaderValue::from_static("/"));
    (StatusCode::SEE_OTHER, hdrs, "").into_response()
}
