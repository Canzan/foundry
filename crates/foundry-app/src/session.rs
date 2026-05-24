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
