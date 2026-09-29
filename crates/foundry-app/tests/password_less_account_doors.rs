//! keycloak-sso step 01-01 (DDD-19, DDD-21, D7): what the password doors do for
//! an account that has no password (a federated-only account, NULL hash).
//!
//! - The password form refuses it exactly like an unknown address: same status,
//!   same body, a recorded failed attempt, and the known-bad-hash verification
//!   still runs (no early return — that would be a timing oracle for "this
//!   address has an SSO-only account").
//! - Forgot-password renders the usual page AND sends the link; following it
//!   sets a password the password form then accepts.
//! - Change-password refuses it: with no current password to prove, a signed-in
//!   session alone cannot give the account a password (DDD-21).
//!
//! Driven through the real router (driving port) over a real Postgres; the only
//! double is a recording notification provider at the notifier's driven port.

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use foundry_app::notify::{DeliveryError, Notification, NotificationProvider, ProviderKind};
use foundry_app::{build_router, AppState, Notifier, SystemClock};
use foundry_store::{run_migrations, Store};
use secrecy::SecretString;
use sqlx::postgres::PgPoolOptions;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
use tower::ServiceExt;

const CSRF: &str = "password-less-doors-csrf-token";
const PASSWORDLESS_EMAIL: &str = "mei@example.com";
const UNKNOWN_EMAIL: &str = "nobody@example.com";
/// A guess that can match no account: the password-less one has no hash at all.
const ANY_PASSWORD: &str = "any-password-at-all";

/// Records every delivered notification (the notifier's driven port).
#[derive(Default)]
struct RecordingProvider {
    sent: Mutex<Vec<Notification>>,
}

#[async_trait]
impl NotificationProvider for RecordingProvider {
    async fn deliver(&self, notification: &Notification) -> Result<(), DeliveryError> {
        self.sent.lock().unwrap().push(notification.clone());
        Ok(())
    }
    fn kind(&self) -> ProviderKind {
        ProviderKind::Log
    }
    async fn probe(&self) -> Result<(), DeliveryError> {
        Ok(())
    }
}

struct Harness {
    router: Router,
    store: Arc<Store>,
    outbox: Arc<RecordingProvider>,
    _pg: ContainerAsync<Postgres>,
}

async fn harness() -> Harness {
    let pg = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let host = pg.get_host().await.expect("host");
    let port = pg.get_host_port_ipv4(5432).await.expect("port");
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&format!(
            "postgres://postgres:postgres@{host}:{port}/postgres"
        ))
        .await
        .expect("connect pool");
    run_migrations(&pool).await.expect("run migrations");
    let store = Arc::new(Store::from_pool(pool));
    let outbox = Arc::new(RecordingProvider::default());
    let state = AppState {
        oidc: None,
        store: store.clone(),
        session_secret: Arc::new(SecretString::new(
            "password-less-doors-secret-thirty-two-bytes-or-more-yes".into(),
        )),
        machine_token_verifier: Arc::new(
            foundry_auth::MachineTokenVerifier::from_public_keys(&[
                "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAwtFPs8Jcuncc+E7dXqG/oolI3P6Hamrpd8zVKPvRmg0=\n-----END PUBLIC KEY-----\n"
                    .to_string(),
            ])
            .expect("fixed test public key is valid"),
        ),
        machine_token_signer: None,
        session_cookie_secure: false,
        db_schema: "public".into(),
        public_url: "http://foundry.test".into(),
        clock: Arc::new(SystemClock),
        notifier: Arc::new(Notifier::new(vec![outbox.clone()])),
        revoke_rate_limiter: Arc::new(foundry_app::rate_limit::RevokeRateLimiter::default()),
        realtime_tx: foundry_realtime::build_broadcast(),
        sse_heartbeat_ms: foundry_app::DEFAULT_SSE_HEARTBEAT_MS,
        file_upload_max_mb: foundry_app::DEFAULT_FILE_UPLOAD_MAX_MB,
        db_unreachable: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        force_board_render_failure: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        test_migrations_dir: None,
        applied_migrations: Arc::new(std::sync::Mutex::new(
            foundry_store::MigrationReport::default(),
        )),
        test_migration_delay_ms: 0,
    };
    Harness {
        router: build_router(state),
        store,
        outbox,
        _pg: pg,
    }
}

/// A member of a workspace whose `password_hash` is NULL — the account a
/// federated first sign-in provisions (DDD-16), seeded directly here.
async fn seed_password_less_member(store: &Store) {
    let workspace_id = uuid::Uuid::now_v7();
    let user_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Acme')")
        .bind(workspace_id)
        .execute(store.pool())
        .await
        .expect("seed workspace");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, 'Mei', NULL)",
    )
    .bind(user_id)
    .bind(PASSWORDLESS_EMAIL)
    .execute(store.pool())
    .await
    .expect("seed password-less user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .expect("seed membership");
}

async fn post_form(router: &Router, uri: &str, fields: &[(&str, &str)]) -> (StatusCode, String) {
    let (status, body, _) = post_form_as(router, uri, fields, "").await;
    (status, body)
}

/// POST a CSRF-carrying form with `session_cookies` (`name=value; …`, may be
/// empty) and return the status, body and the cookies the response sets.
async fn post_form_as(
    router: &Router,
    uri: &str,
    fields: &[(&str, &str)],
    session_cookies: &str,
) -> (StatusCode, String, String) {
    let mut all: Vec<(&str, &str)> = vec![("_csrf", CSRF)];
    all.extend_from_slice(fields);
    let body = serde_urlencoded::to_string(&all).expect("encode form");
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            [format!("foundry_csrf={CSRF}"), session_cookies.to_string()]
                .into_iter()
                .filter(|c| !c.is_empty())
                .collect::<Vec<_>>()
                .join("; "),
        )
        .body(Body::from(body))
        .expect("request");
    let resp = router.clone().oneshot(req).await.expect("response");
    let status = resp.status();
    let set_cookies = resp
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok()?.split(';').next().map(str::to_string))
        .collect::<Vec<_>>()
        .join("; ");
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body");
    (
        status,
        String::from_utf8_lossy(&bytes).into_owned(),
        set_cookies,
    )
}

async fn password_hash_of(store: &Store, email: &str) -> Option<String> {
    sqlx::query_scalar("SELECT password_hash FROM users WHERE email_lower = $1")
        .bind(email)
        .fetch_one(store.pool())
        .await
        .expect("read password hash")
}

async fn sign_in(router: &Router, email: &str, password: &str) -> (StatusCode, String, Duration) {
    let started = Instant::now();
    let (status, body) = post_form(
        router,
        "/sign-in",
        &[("email", email), ("password", password)],
    )
    .await;
    (status, body, started.elapsed())
}

async fn failed_attempts(store: &Store, email: &str) -> i64 {
    store
        .count_recent_failed_signin_attempts(
            email,
            time::OffsetDateTime::now_utc() - time::Duration::hours(1),
        )
        .await
        .expect("count failed attempts")
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

#[tokio::test]
async fn the_password_form_refuses_a_password_less_account_exactly_like_an_unknown_address() {
    let h = harness().await;
    seed_password_less_member(&h.store).await;

    // Warm-up (discarded): pays the once-per-process known-bad-hash cost.
    let _ = sign_in(&h.router, "warm-up@example.com", ANY_PASSWORD).await;

    let mut unknown_times = Vec::new();
    let mut passwordless_times = Vec::new();
    let mut unknown_answer = None;
    let mut passwordless_answer = None;
    for _ in 0..3 {
        let (status, body, took) = sign_in(&h.router, UNKNOWN_EMAIL, ANY_PASSWORD).await;
        unknown_times.push(took);
        unknown_answer = Some((status, body));
        let (status, body, took) = sign_in(&h.router, PASSWORDLESS_EMAIL, ANY_PASSWORD).await;
        passwordless_times.push(took);
        passwordless_answer = Some((status, body));
    }

    let (unknown_status, unknown_body) = unknown_answer.expect("sampled");
    let (status, body) = passwordless_answer.expect("sampled");
    assert_eq!(unknown_status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        status, unknown_status,
        "a password-less account must get the unknown-address status"
    );
    assert_eq!(
        body, unknown_body,
        "a password-less account must get the unknown-address body byte for byte"
    );
    assert_eq!(
        failed_attempts(&h.store, PASSWORDLESS_EMAIL).await,
        3,
        "every refused attempt against a password-less account is recorded"
    );
    // The known-bad hash is verified on both arms, so both pay the argon2 cost.
    // An early return on a missing hash answers in a small fraction of it.
    let (unknown, passwordless) = (median(unknown_times), median(passwordless_times));
    assert!(
        passwordless * 3 >= unknown,
        "timing oracle: password-less refusal took {passwordless:?}, unknown address \
         {unknown:?} — the known-bad-hash verification must run for a NULL hash"
    );
}

/// DDD-21: a signed-in password-less account has no current password to prove,
/// so the change-password door refuses it whatever it sends — a hijacked SSO
/// session alone cannot give the account a password. Nothing is written and
/// nobody is told a password changed.
#[tokio::test]
async fn a_signed_in_password_less_account_cannot_set_a_password_by_changing_it() {
    let h = harness().await;
    seed_password_less_member(&h.store).await;
    // A signed-in session for the account: sign in while it still had a
    // password, then take the password away — the state a federated sign-in
    // leaves (session for a NULL-hash account) without an identity provider.
    let bootstrap_password = "bootstrap-only-password";
    let hash = foundry_auth::hash_password(&SecretString::new(bootstrap_password.into()))
        .await
        .expect("hash");
    sqlx::query("UPDATE users SET password_hash = $1 WHERE email_lower = $2")
        .bind(&hash)
        .bind(PASSWORDLESS_EMAIL)
        .execute(h.store.pool())
        .await
        .expect("give a temporary password");
    let (status, _, session) = post_form_as(
        &h.router,
        "/sign-in",
        &[
            ("email", PASSWORDLESS_EMAIL),
            ("password", bootstrap_password),
        ],
        "",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "the member signs in");
    sqlx::query("UPDATE users SET password_hash = NULL WHERE email_lower = $1")
        .bind(PASSWORDLESS_EMAIL)
        .execute(h.store.pool())
        .await
        .expect("make the account password-less");

    for current in [ANY_PASSWORD, "", bootstrap_password] {
        let (status, _, _) = post_form_as(
            &h.router,
            "/account/password",
            &[
                ("current_password", current),
                ("new_password", "a-password-of-my-own-choosing"),
            ],
            &session,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "a password-less account cannot reauthenticate with {current:?}"
        );
    }
    assert_eq!(
        password_hash_of(&h.store, PASSWORDLESS_EMAIL).await,
        None,
        "the account is still password-less"
    );
    assert!(
        h.outbox.sent.lock().unwrap().is_empty(),
        "nobody is told a password changed"
    );
}

#[tokio::test]
async fn forgot_password_gives_a_password_less_account_a_password_the_form_then_accepts() {
    let h = harness().await;
    seed_password_less_member(&h.store).await;

    let (_, unknown_page) =
        post_form(&h.router, "/forgot-password", &[("email", UNKNOWN_EMAIL)]).await;
    let (status, page) = post_form(
        &h.router,
        "/forgot-password",
        &[("email", PASSWORDLESS_EMAIL)],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page, unknown_page, "forgot-password renders the usual page");

    let link = {
        let sent = h.outbox.sent.lock().unwrap();
        let mail = sent
            .iter()
            .find(|n| n.recipient == PASSWORDLESS_EMAIL)
            .expect("a reset link is sent to the password-less account");
        let start = mail
            .body
            .find("http://foundry.test/reset-password?token=")
            .expect("the mail carries the reset link");
        mail.body[start..]
            .split_whitespace()
            .next()
            .expect("link")
            .to_string()
    };
    let token = urlencoding::decode(link.split("token=").nth(1).expect("token param"))
        .expect("decode token")
        .into_owned();

    let new_password = "correct horse battery staple";
    let (status, _) = post_form(
        &h.router,
        "/reset-password",
        &[
            ("token", token.as_str()),
            ("password", new_password),
            ("confirm", new_password),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "the reset completes");

    let (status, _, _) = sign_in(&h.router, PASSWORDLESS_EMAIL, new_password).await;
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "the password set through reset is accepted at the password form"
    );
}
