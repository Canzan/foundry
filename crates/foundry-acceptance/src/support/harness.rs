//! In-process acceptance harness for US-05+.
//!
//! Per `distill/driver.md` §3-4:
//! - One Postgres container per `cargo test` invocation (testcontainers-rs).
//! - Per-scenario PG schema, search_path rotated on the pool.
//! - Per-scenario AppState with `FakeEmailSender` + `MockClock`.
//!
//! Containers and pools are deliberately leaked (`Box::leak`) so they
//! outlive every scenario — `cargo test` exits and the docker daemon
//! reaps the container.

use crate::support::heartbeat_env;
use foundry_app::clock::MockClock;
use foundry_app::email::FakeEmailSender;
use foundry_app::test_support::{spawn_app_with_listener, TestApp};
use foundry_app::{AppState, DEFAULT_SSE_HEARTBEAT_MS};
use foundry_store::Store;
use once_cell::sync::OnceCell;
use secrecy::SecretString;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Connection, PgPool};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ContainerAsync;
use tokio::sync::OnceCell as AsyncOnceCell;

static PG_CONTAINER: OnceCell<&'static ContainerAsync<Postgres>> = OnceCell::new();
static PG_CONTAINER_INIT: AsyncOnceCell<()> = AsyncOnceCell::const_new();
static PG_BASE_URL: OnceCell<String> = OnceCell::new();
static SCHEMA_COUNTER: Mutex<u64> = Mutex::new(0);

/// Boot the shared Postgres container if not already up. Returns the
/// `postgres://...` URL pointing at the default `postgres` database
/// (callers create per-scenario schemas inside it).
pub async fn ensure_postgres() -> &'static str {
    PG_CONTAINER_INIT
        .get_or_init(|| async {
            let container: ContainerAsync<Postgres> = Postgres::default()
                .start()
                .await
                .expect("start postgres container");
            let host = container.get_host().await.expect("postgres container host");
            let port = container
                .get_host_port_ipv4(5432)
                .await
                .expect("postgres container port");
            // Default testcontainers postgres user/password/db.
            let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");
            PG_BASE_URL.set(url).ok();
            PG_CONTAINER
                .set(Box::leak(Box::new(container)))
                .expect("set PG_CONTAINER once");
        })
        .await;
    PG_BASE_URL.get().expect("postgres base URL set").as_str()
}

/// Provision a fresh per-scenario schema, run migrations into it, and
/// return a pool whose connections have `search_path` pinned to that
/// schema.
pub async fn fresh_schema_pool() -> (String, PgPool) {
    let (schema, pool, _) = fresh_schema_pool_with_url().await;
    (schema, pool)
}

/// As [`fresh_schema_pool`] but also returns a `postgres://...` URL
/// whose connect options pin `search_path=<schema>`. The realtime
/// listener (US-09) needs a URL — `PgListener::connect` does not
/// accept a PgPool.
pub async fn fresh_schema_pool_with_url() -> (String, PgPool, String) {
    let base = ensure_postgres().await;
    let counter = {
        let mut g = SCHEMA_COUNTER.lock().expect("schema counter mutex");
        *g += 1;
        *g
    };
    // Postgres schema names must start with a letter and avoid hyphens.
    let schema = format!("test_s{}_{}", counter, hex_suffix());

    // Open a one-shot connection to create the schema.
    let mut admin = sqlx::PgConnection::connect(base)
        .await
        .expect("connect to base postgres");
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&mut admin)
        .await
        .expect("create schema");
    drop(admin);

    let options = PgConnectOptions::from_str(base)
        .expect("parse postgres URL")
        .options([("search_path", schema.as_str())]);
    let pool = PgPoolOptions::new()
        .min_connections(1)
        .max_connections(4)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect_with(options)
        .await
        .expect("build per-scenario pool");

    foundry_store::run_migrations(&pool)
        .await
        .expect("run migrations into per-scenario schema");

    // Build a URL the LISTEN task can pass to PgListener::connect that
    // pins the same search_path. Append `?options=-csearch_path%3D<schema>`
    // — sqlx 0.8 parses this correctly.
    let listen_url = format!(
        "{base}?options=-csearch_path%3D{schema}",
        base = base,
        schema = schema
    );

    (schema, pool, listen_url)
}

fn hex_suffix() -> String {
    let mut bytes = [0u8; 4];
    use rand::RngCore;
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Drop a per-scenario schema. Best-effort; ignores failures because
/// the After hook runs on a process-wide pool that may be torn down.
pub async fn drop_schema(schema: &str) {
    let Some(base) = PG_BASE_URL.get() else {
        return;
    };
    if let Ok(mut conn) = sqlx::PgConnection::connect(base).await {
        let _ = sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&mut conn)
            .await;
    }
}

/// The full in-process harness for a single scenario.
pub struct InProcHarness {
    pub app: TestApp,
    pub fake_clock: Arc<MockClock>,
    pub fake_email: Arc<FakeEmailSender>,
    pub schema: String,
}

impl std::fmt::Debug for InProcHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InProcHarness")
            .field("addr", &self.app.addr)
            .field("schema", &self.schema)
            .finish_non_exhaustive()
    }
}

impl InProcHarness {
    pub async fn spawn(now: time::OffsetDateTime) -> Self {
        let (schema, pool, listen_url) = fresh_schema_pool_with_url().await;
        let store = Arc::new(Store::from_pool(pool));
        let fake_clock = MockClock::new(now);
        let fake_email = FakeEmailSender::new();
        let realtime_tx = foundry_realtime::build_broadcast();
        let heartbeat_ms =
            heartbeat_env::current_heartbeat_ms().unwrap_or(DEFAULT_SSE_HEARTBEAT_MS);
        let state = AppState {
            store,
            session_secret: Arc::new(SecretString::new(
                "test-only-secret-must-be-at-least-32-bytes-long-please-yes".into(),
            )),
            // The US-05 happy-path scenario asserts the cookie carries
            // Secure. The harness binds to 127.0.0.1 (plain HTTP) but
            // we still emit Secure in the Set-Cookie header — the test
            // only inspects the header text, not whether the browser
            // would send the cookie back over HTTP.
            session_cookie_secure: true,
            db_schema: schema.clone(),
            public_url: "http://localhost".into(),
            clock: fake_clock.clone(),
            email: fake_email.clone(),
            realtime_tx,
            sse_heartbeat_ms: heartbeat_ms,
        };
        let app = spawn_app_with_listener(state, listen_url)
            .await
            .expect("spawn app + pg listener");
        Self {
            app,
            fake_clock,
            fake_email,
            schema,
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.app.addr)
    }
}

// ---------------------------------------------------------------- HTTP helpers

/// Outcome of a `signed_in_post` call. Carries the bits subsequent
/// `Then` steps need to introspect (status / headers / body) without
/// reaching back into the world struct.
#[derive(Debug)]
pub struct PostOutcome {
    pub status: reqwest::StatusCode,
    pub headers: reqwest::header::HeaderMap,
    pub body: String,
}

/// Sign in as `email` / `password`, then POST `form` to `url` with a
/// freshly-fetched CSRF token. Returns the full response shape so the
/// caller can drive assertions.
///
/// This dedupes the boilerplate that US-06 sign-out and the new US-07
/// project-create scenarios both need: (1) authenticate, (2) capture
/// the session cookie, (3) GET a form page to receive a CSRF cookie,
/// (4) POST with cookie + form `_csrf` token.
pub async fn signed_in_post(
    harness: &InProcHarness,
    http: &reqwest::Client,
    email: &str,
    password: &str,
    url: &str,
    form: &[(&str, &str)],
) -> PostOutcome {
    let base = harness.base_url();

    // TODO(slice-2): extract a `fetch_csrf_token(http, base, path) -> (cookie, token)`
    // helper. The same Set-Cookie → strip-prefix → strip-attributes dance lives
    // here AND in two places inside us_08_file_issue.rs (sign-in step + perf
    // scenario). Refactor when slice 2 adds a third CSRF-aware POST handler.

    // (1) GET /sign-in to mint a CSRF cookie + token.
    let signin_get = http
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("get /sign-in for csrf");
    let csrf_cookie = signin_get
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(|s| s.to_string())
        .expect("/sign-in must mint foundry_csrf cookie");
    let csrf_token = csrf_cookie
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();
    let signin_cookie_header = format!("foundry_csrf={csrf_token}");

    // (2) POST /sign-in to authenticate. Captures the session cookie.
    let mut signin_form: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    signin_form.insert("email", email.to_string());
    signin_form.insert("password", password.to_string());
    signin_form.insert("_csrf", csrf_token.clone());
    let signin_resp = http
        .post(format!("{base}/sign-in"))
        .header(reqwest::header::COOKIE, signin_cookie_header)
        .form(&signin_form)
        .send()
        .await
        .expect("post /sign-in");
    let session_cookie = signin_resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .map(|s| s.to_string())
        .expect("sign-in must issue a foundry_session cookie");
    let session_pair = session_cookie
        .split(';')
        .next()
        .unwrap_or(&session_cookie)
        .to_string();

    // (3) GET /sign-in again presenting the session cookie so the CSRF
    // cookie minted in step 1 also rides with the session in step 4.
    // tower-sessions binds the session row to the cookie value; the
    // CSRF cookie is independent. We can reuse the existing csrf token.
    let combined_cookie = format!("{session_pair}; foundry_csrf={csrf_token}");

    // (4) POST `url` with the form + matching CSRF token.
    let mut full_form: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    for (k, v) in form {
        full_form.insert(k, (*v).to_string());
    }
    full_form.insert("_csrf", csrf_token);
    let resp = http
        .post(format!("{base}{url}"))
        .header(reqwest::header::COOKIE, combined_cookie)
        .form(&full_form)
        .send()
        .await
        .expect("post target url");

    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp.text().await.unwrap_or_default();
    PostOutcome {
        status,
        headers,
        body,
    }
}
