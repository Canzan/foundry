//! external-integration-plugins — foundry-app gold tests below the acceptance
//! lane (DESIGN [REF] Driven Ports and Contract Shapes, check-arch rule 5,
//! Earned Trust; DEVOPS C-7, C-13). Pending stubs written by DISTILL 2026-10-21
//! (ADR-025 scaffolded RED): each is `#[ignore]`d with its DELIVER step id and
//! panics with the RED marker until DELIVER replaces its body (through
//! `foundry_app::build_router`, like `api_docs_page.rs`).

macro_rules! pending {
    ($name:ident, $step:literal, $what:literal) => {
        #[test]
        #[ignore = concat!("pending: external-integration-plugins ", $step, " ", $what)]
        fn $name() {
            panic!(concat!(
                "Not yet implemented -- RED scaffold (external-integration-plugins ",
                $step,
                ": ",
                $what,
                ")"
            ));
        }
    };
}

// Handlers (slices 01-03).
pending!(
    preview_calls_no_port_and_writes_nothing,
    "02-02",
    "DDR-25: routes/preview re-renders compose output; no vendor call, no integration_* row"
);
pending!(
    reconnect_to_another_slack_team_changes_nothing,
    "03-02",
    "connection upsert: a different team id on the return leaves the row and audit untouched"
);
pending!(csrf_exempt_set_is_exactly_three_paths, "03-02", "check-arch rule 5: is_exempt_path is /bootstrap, /integrations/slack/events, /email/stop/one-click and nothing else");
pending!(connection_lost_event_is_registered_at_zero, "03-02", "DDR-23: NotificationEvent::IntegrationConnectionLost is suppressible and its series register at 0");

// Outbound address policy (slice 04).
pending!(
    outbound_refuses_a_name_that_rebinds_to_loopback,
    "04-01",
    "DNS rebinding: the judging resolver refuses at connect time"
);
pending!(
    outbound_ignores_an_https_proxy_pointed_at_metadata,
    "04-01",
    "no_proxy(): HTTPS_PROXY set to a forwarder for 169.254.169.254 is never used"
);
pending!(
    outbound_follows_no_redirect_to_metadata,
    "04-01",
    "redirect::Policy::none(): a 302 to 169.254.169.254 is an error, not a request"
);
pending!(
    outbound_without_the_test_seam_refuses_loopback,
    "04-01",
    "C-7: with the test-support seam off, a 127.0.0.1 homeserver is refused"
);

// D-16, SC-3 (01-05): a Slack install through the router leaves one audit row,
// and no column of it holds the bot token, the code, a Foundry Slack app
// setting, the secrets key or the sealed value. The only double is a local
// Slack answering `oauth.v2.access` and `auth.test` (the driven port).
mod audit {
    use axum::body::Body;
    use axum::http::{header, Request, StatusCode};
    use axum::routing::post;
    use axum::Router;
    use foundry_app::integrations::config::IntegrationSettings;
    use foundry_app::integrations::{Integrations, IntegrationsHandle};
    use foundry_app::{build_router, AppState, Notifier, SystemClock};
    use foundry_store::{run_migrations, Store};
    use secrecy::SecretString;
    use sqlx::postgres::PgPoolOptions;
    use std::sync::Arc;
    use std::time::Duration;
    use testcontainers_modules::postgres::Postgres;
    use testcontainers_modules::testcontainers::runners::AsyncRunner;
    use testcontainers_modules::testcontainers::ImageExt;
    use tower::ServiceExt;

    const CSRF: &str = "eip-audit-csrf-token";
    const PASSWORD: &str = "eip-audit-password-long-enough";
    const ADMIN: &str = "priya@canzan.dev";
    const TOKEN: &str = "xoxb-eip-audit-sentinel-token";
    const CODE: &str = "eip-audit-sentinel-code";
    const CLIENT_SECRET: &str = "eip-audit-sentinel-client-secret";
    const SIGNING_SECRET: &str = "eip-audit-sentinel-signing-secret";
    const SECRETS_KEY: &str = "ZWlwLXNlbnRpbmVsLXNlY3JldHMta2V5LTMyYnl0ZXM";

    async fn slack() -> String {
        let app = Router::new()
            .route(
                "/oauth.v2.access",
                post(|| async {
                    axum::Json(serde_json::json!({
                        "ok": true, "access_token": TOKEN, "token_type": "bot",
                        "bot_user_id": "U0FOUNDRYBOT", "app_id": "A0EIPFOUNDRY",
                        "team": { "id": "T0CANZANLABS", "name": "Canzan Labs" }
                    }))
                }),
            )
            .route(
                "/auth.test",
                post(|| async {
                    axum::Json(serde_json::json!({
                        "ok": true, "url": "https://canzanlabs.slack.com/", "team_id": "T0CANZANLABS"
                    }))
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{addr}")
    }

    fn settings(slack: &str) -> IntegrationSettings {
        let env = [
            ("SLACK_CLIENT_ID", "4100000000.eipclientid".to_string()),
            ("SLACK_CLIENT_SECRET", CLIENT_SECRET.to_string()),
            ("SLACK_SIGNING_SECRET", SIGNING_SECRET.to_string()),
            ("SLACK_API_BASE_URL", slack.to_string()),
            ("SLACK_AUTHORIZE_URL", format!("{slack}/oauth/v2/authorize")),
            ("FOUNDRY_SECRETS_KEY", SECRETS_KEY.to_string()),
        ];
        IntegrationSettings::from_lookup(
            |name| env.iter().find(|(k, _)| *k == name).map(|(_, v)| v.clone()),
            "http://foundry.test",
        )
        .expect("complete settings")
    }

    async fn seed_admin(store: &Store) {
        let (workspace_id, user_id) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
        let hash = foundry_auth::hash_password(&SecretString::new(PASSWORD.into()))
            .await
            .expect("hash");
        sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Canzan Labs')")
            .bind(workspace_id)
            .execute(store.pool())
            .await
            .expect("seed workspace");
        sqlx::query(
            "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
                  VALUES ($1, $2, $2, 'Priya Raman', $3)",
        )
        .bind(user_id)
        .bind(ADMIN)
        .bind(&hash)
        .execute(store.pool())
        .await
        .expect("seed user");
        sqlx::query(
            "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'admin')",
        )
        .bind(workspace_id)
        .bind(user_id)
        .execute(store.pool())
        .await
        .expect("seed admin");
    }

    async fn send(router: &Router, req: Request<Body>) -> axum::response::Response {
        router.clone().oneshot(req).await.expect("response")
    }

    fn cookies(resp: &axum::response::Response) -> Vec<String> {
        resp.headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok()?.split(';').next().map(str::to_string))
            .collect()
    }

    fn get(uri: &str, cookie: &str) -> Request<Body> {
        Request::builder()
            .uri(uri)
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .expect("request")
    }

    #[tokio::test]
    async fn audit_rows_carry_no_secret() {
        let pg = Postgres::default()
            .with_tag("16-alpine")
            .start()
            .await
            .expect("start postgres container");
        let url = format!(
            "postgres://postgres:postgres@{}:{}/postgres",
            pg.get_host().await.expect("host"),
            pg.get_host_port_ipv4(5432).await.expect("port")
        );
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .acquire_timeout(Duration::from_secs(10))
            .connect(&url)
            .await
            .expect("connect pool");
        run_migrations(&pool).await.expect("run migrations");
        let store = Arc::new(Store::from_pool(pool));
        seed_admin(&store).await;
        let state = AppState {
            oidc: None,
            store: store.clone(),
            session_secret: Arc::new(SecretString::new(
                "eip-audit-secret-thirty-two-bytes-or-more-yes-please".into(),
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
            integrations: IntegrationsHandle::new(Integrations::new(settings(&slack().await))),
            notifier: Arc::new(Notifier::new(vec![])),
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
        let router = build_router(state);

        let form = serde_urlencoded::to_string([
            ("_csrf", CSRF),
            ("email", ADMIN),
            ("password", PASSWORD),
        ])
        .expect("form");
        let signed_in = send(
            &router,
            Request::builder()
                .method("POST")
                .uri("/sign-in")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, format!("foundry_csrf={CSRF}"))
                .body(Body::from(form))
                .expect("request"),
        )
        .await;
        assert_eq!(signed_in.status(), StatusCode::SEE_OTHER, "Priya signs in");
        let mut jar = cookies(&signed_in);
        jar.push(format!("foundry_csrf={CSRF}"));
        let cookie = jar.join("; ");

        let start = send(
            &router,
            get("/workspace/integrations/slack/install", &cookie),
        )
        .await;
        assert!(
            start.status().is_redirection(),
            "Add to Slack goes to Slack"
        );
        let to_slack = start.headers()[header::LOCATION]
            .to_str()
            .expect("location")
            .to_string();
        let state = to_slack
            .split("state=")
            .nth(1)
            .expect("the approval address carries the state")
            .to_string();
        let back = send(
            &router,
            get(
                &format!("/workspace/integrations/slack/return?code={CODE}&state={state}"),
                &cookie,
            ),
        )
        .await;
        assert_eq!(back.status(), StatusCode::SEE_OTHER, "the return connects");

        let rows: Vec<String> =
            sqlx::query_scalar("SELECT row_to_json(a)::text FROM integration_audit_events a")
                .fetch_all(store.pool())
                .await
                .expect("audit rows");
        assert_eq!(rows.len(), 1, "one audit row for the connection: {rows:?}");
        assert!(rows[0].contains("connection_connected"), "{}", rows[0]);
        let sealed: Vec<u8> = sqlx::query_scalar(
            "SELECT sealed_credential FROM integration_connections WHERE kind = 'slack'",
        )
        .fetch_one(store.pool())
        .await
        .expect("the sealed credential");
        assert!(
            !String::from_utf8_lossy(&sealed).contains(TOKEN),
            "the stored credential is sealed"
        );
        let hex: String = sealed.iter().map(|b| format!("{b:02x}")).collect();
        for secret in [
            TOKEN,
            CODE,
            CLIENT_SECRET,
            SIGNING_SECRET,
            SECRETS_KEY,
            &hex[2..34],
        ] {
            assert!(
                !rows[0].contains(secret),
                "no audit column holds a credential: {}",
                rows[0]
            );
        }
    }
}

// DDR-25 (01-06): Send test, its per-person limit, and the route save, through
// the router over a real Postgres. The only double is a local Slack answering
// `conversations.list` and `chat.postMessage` (the driven ports), recording
// every call. The universe each test compares is every integration_* row.
mod routes {
    use axum::body::Body;
    use axum::extract::State;
    use axum::http::{header, Request, StatusCode};
    use axum::routing::{get, post};
    use axum::Router;
    use foundry_app::integrations::config::IntegrationSettings;
    use foundry_app::integrations::{Integrations, IntegrationsHandle};
    use foundry_app::{build_router, AppState, Notifier, SystemClock};
    use foundry_auth::secret_box::SecretSlot;
    use foundry_store::integrations::NewConnection;
    use foundry_store::{run_migrations, Store};
    use secrecy::SecretString;
    use serde_json::{json, Value};
    use sqlx::postgres::PgPoolOptions;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use testcontainers_modules::postgres::Postgres;
    use testcontainers_modules::testcontainers::runners::AsyncRunner;
    use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
    use tower::ServiceExt;
    use uuid::Uuid;

    const CSRF: &str = "eip-routes-csrf-token";
    const PASSWORD: &str = "eip-routes-password-long-enough";
    const LEAD: &str = "lena.park@canzan.dev";
    const TOKEN: &str = "xoxb-eip-routes-sentinel-token";
    const SECRETS_KEY: &str = "ZWlwLXNlbnRpbmVsLXNlY3JldHMta2V5LTMyYnl0ZXM";
    const AUTH_DEV: &str = "C0AUTHDEV";
    const PAGE: &str = "/team/backend/project/identity-platform/integrations";
    const K7: &str =
        "Test message from Foundry for Identity Platform (AUTH). This route will post here.";

    /// Slack's calls as the double received them: (method, body).
    type Calls = Arc<Mutex<Vec<(String, Value)>>>;

    async fn slack() -> (String, Calls) {
        let calls: Calls = Arc::default();
        let app = Router::new()
            .route(
                "/conversations.list",
                get(|State(calls): State<Calls>| async move {
                    calls
                        .lock()
                        .unwrap()
                        .push(("conversations.list".into(), Value::Null));
                    axum::Json(json!({
                        "ok": true,
                        "channels": [{ "id": AUTH_DEV, "name": "auth-dev", "is_private": false,
                                       "is_member": true, "is_archived": false }],
                        "response_metadata": { "next_cursor": "" }
                    }))
                }),
            )
            .route(
                "/chat.postMessage",
                post(|State(calls): State<Calls>, body: String| async move {
                    let body: Value = serde_json::from_str(&body).unwrap_or_default();
                    calls
                        .lock()
                        .unwrap()
                        .push(("chat.postMessage".into(), body));
                    axum::Json(
                        json!({ "ok": true, "channel": AUTH_DEV, "ts": "1761055320.000100" }),
                    )
                }),
            )
            .with_state(calls.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (format!("http://{addr}"), calls)
    }

    fn settings(slack: &str) -> IntegrationSettings {
        let env = [
            ("SLACK_CLIENT_ID", "4100000000.eipclientid".to_string()),
            (
                "SLACK_CLIENT_SECRET",
                "eip-routes-client-secret".to_string(),
            ),
            (
                "SLACK_SIGNING_SECRET",
                "eip-routes-signing-secret".to_string(),
            ),
            ("SLACK_API_BASE_URL", slack.to_string()),
            ("SLACK_AUTHORIZE_URL", format!("{slack}/oauth/v2/authorize")),
            ("FOUNDRY_SECRETS_KEY", SECRETS_KEY.to_string()),
            ("FOUNDRY_INTEGRATIONS_WORKER", "off".to_string()),
        ];
        IntegrationSettings::from_lookup(
            |name| env.iter().find(|(k, _)| *k == name).map(|(_, v)| v.clone()),
            "http://foundry.test",
        )
        .expect("complete settings")
    }

    async fn exec(store: &Store, sql: &str, ids: &[Uuid]) {
        let mut query = sqlx::query(sql);
        for id in ids {
            query = query.bind(*id);
        }
        query.execute(store.pool()).await.expect(sql);
    }

    /// Lena leads Backend, whose Identity Platform (AUTH) has a Backlog lane;
    /// Canzan Labs is connected to Slack (a precondition, sealed for its row).
    async fn seed(store: &Store, settings: &IntegrationSettings) {
        let (ws, lena, team, project, connection) = (
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
        );
        let hash = foundry_auth::hash_password(&SecretString::new(PASSWORD.into()))
            .await
            .expect("hash");
        exec(
            store,
            "INSERT INTO workspaces (id, name) VALUES ($1, 'Canzan Labs')",
            &[ws],
        )
        .await;
        sqlx::query(
            "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
                  VALUES ($1, $2, $2, 'Lena Park', $3)",
        )
        .bind(lena)
        .bind(LEAD)
        .bind(&hash)
        .execute(store.pool())
        .await
        .expect("seed user");
        exec(
            store,
            "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
            &[ws, lena],
        )
        .await;
        exec(
            store,
            "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Backend', 'backend')",
            &[team, ws],
        )
        .await;
        exec(
            store,
            "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'lead')",
            &[team, lena],
        )
        .await;
        exec(
            store,
            "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
                  VALUES ($1, $2, $3, 'Identity Platform', 'identity-platform', 'AUTH')",
            &[project, team, ws],
        )
        .await;
        exec(
            store,
            "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
                  VALUES (gen_random_uuid(), $1, $2, 'backlog', 'Backlog', 0)",
            &[project, ws],
        )
        .await;
        let slot = SecretSlot::new(
            "integration_connections",
            &connection.to_string(),
            "sealed_credential",
        );
        let sealed = settings
            .secrets
            .as_ref()
            .expect("the secrets key")
            .seal(&SecretString::from(TOKEN), &slot)
            .expect("seal");
        store
            .upsert_connection(&NewConnection {
                id: connection,
                workspace_id: ws,
                kind: "slack".into(),
                external_id: "T0CANZANLABS".into(),
                app_id: Some("A0EIPFOUNDRY".into()),
                bot_user_id: Some("U0FOUNDRYBOT".into()),
                label: "Canzan Labs".into(),
                detail: Some("canzanlabs.slack.com".into()),
                sealed_credential: sealed.as_bytes().to_vec(),
                connected_by: lena,
                connected_at: time::OffsetDateTime::now_utc(),
            })
            .await
            .expect("seed the Slack connection");
    }

    struct Lab {
        _pg: ContainerAsync<Postgres>,
        store: Arc<Store>,
        router: Router,
        cookie: String,
        calls: Calls,
    }

    impl Lab {
        async fn start() -> Self {
            let pg = Postgres::default()
                .with_tag("16-alpine")
                .start()
                .await
                .expect("start postgres container");
            let url = format!(
                "postgres://postgres:postgres@{}:{}/postgres",
                pg.get_host().await.expect("host"),
                pg.get_host_port_ipv4(5432).await.expect("port")
            );
            let pool = PgPoolOptions::new()
                .max_connections(8)
                .acquire_timeout(Duration::from_secs(10))
                .connect(&url)
                .await
                .expect("connect pool");
            run_migrations(&pool).await.expect("run migrations");
            let store = Arc::new(Store::from_pool(pool));
            let (slack, calls) = slack().await;
            let settings = settings(&slack);
            seed(&store, &settings).await;
            let state = AppState {
                oidc: None,
                store: store.clone(),
                session_secret: Arc::new(SecretString::new(
                    "eip-routes-secret-thirty-two-bytes-or-more-yes-please".into(),
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
                integrations: IntegrationsHandle::new(Integrations::new(settings)),
                notifier: Arc::new(Notifier::new(vec![])),
                revoke_rate_limiter: Arc::new(
                    foundry_app::rate_limit::RevokeRateLimiter::default(),
                ),
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
            let router = build_router(state);
            let signed_in = router
                .clone()
                .oneshot(form(
                    "/sign-in",
                    &format!("foundry_csrf={CSRF}"),
                    &[("email", LEAD), ("password", PASSWORD)],
                ))
                .await
                .expect("sign in");
            assert_eq!(signed_in.status(), StatusCode::SEE_OTHER, "Lena signs in");
            let mut jar: Vec<String> = signed_in
                .headers()
                .get_all(header::SET_COOKIE)
                .iter()
                .filter_map(|v| v.to_str().ok()?.split(';').next().map(str::to_string))
                .collect();
            jar.push(format!("foundry_csrf={CSRF}"));
            Lab {
                _pg: pg,
                store,
                router,
                cookie: jar.join("; "),
                calls,
            }
        }

        /// POST the route form to `door` (`routes` or `routes/test`).
        async fn submit(&self, door: &str, destination: &str) -> (StatusCode, String) {
            let response = self
                .router
                .clone()
                .oneshot(form(
                    &format!("{PAGE}/{door}"),
                    &self.cookie,
                    &[
                        ("kind", "slack"),
                        ("destination", destination),
                        ("events", "issue.created"),
                        ("content_level", "summary"),
                    ],
                ))
                .await
                .expect("response");
            let status = response.status();
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body");
            (status, String::from_utf8_lossy(&body).into_owned())
        }

        fn posts(&self) -> Vec<Value> {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .filter(|(method, _)| method == "chat.postMessage")
                .map(|(_, body)| body.clone())
                .collect()
        }

        /// Every integration_* row, as JSON, table by table.
        async fn universe(&self) -> Vec<String> {
            let mut rows = Vec::new();
            for table in [
                "integration_connections",
                "integration_routes",
                "integration_route_lanes",
                "integration_deliveries",
                "integration_destination_pacing",
                "integration_audit_events",
            ] {
                let json: Vec<String> = sqlx::query_scalar(&format!(
                    "SELECT row_to_json(t)::text FROM {table} t ORDER BY 1"
                ))
                .fetch_all(self.store.pool())
                .await
                .expect("read the universe");
                rows.extend(json.into_iter().map(|row| format!("{table}: {row}")));
            }
            rows
        }
    }

    fn form(uri: &str, cookie: &str, fields: &[(&str, &str)]) -> Request<Body> {
        let mut all = vec![("_csrf", CSRF)];
        all.extend_from_slice(fields);
        Request::builder()
            .method("POST")
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::COOKIE, cookie)
            .body(Body::from(serde_urlencoded::to_string(all).expect("form")))
            .expect("request")
    }

    fn texts(post: &Value) -> String {
        post.to_string()
    }

    #[tokio::test]
    async fn send_test_posts_once_and_writes_no_row() {
        let lab = Lab::start().await;
        let before = lab.universe().await;
        let (status, page) = lab.submit("routes/test", AUTH_DEV).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        let posts = lab.posts();
        assert_eq!(posts.len(), 1, "exactly one message: {posts:?}");
        assert_eq!(posts[0]["channel"], AUTH_DEV);
        assert!(texts(&posts[0]).contains(K7), "{}", posts[0]);
        assert_eq!(lab.universe().await, before, "no integration_* row changed");
    }

    #[tokio::test]
    async fn seventh_send_test_in_a_minute_is_refused_without_a_vendor_call() {
        let lab = Lab::start().await;
        for n in 1..=6 {
            let (status, page) = lab.submit("routes/test", AUTH_DEV).await;
            assert_eq!(status, StatusCode::OK, "test {n}: {page}");
        }
        let before = lab.universe().await;
        let (status, page) = lab.submit("routes/test", AUTH_DEV).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert!(
            page.contains("Too many test messages. Try again in a minute."),
            "{page}"
        );
        assert_eq!(lab.posts().len(), 6, "the seventh asks Slack nothing");
        assert_eq!(lab.universe().await, before, "no integration_* row changed");
    }

    #[tokio::test]
    async fn save_never_sends() {
        let lab = Lab::start().await;
        let before = lab.universe().await;
        let (status, page) = lab.submit("routes", AUTH_DEV).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "{page}");
        assert!(lab.posts().is_empty(), "saving sends nothing");
        let after = lab.universe().await;
        let added: Vec<&String> = after.iter().filter(|row| !before.contains(row)).collect();
        assert_eq!(
            added.len(),
            2,
            "one route row and one audit row: {added:#?}"
        );
        assert!(added
            .iter()
            .any(|row| row.starts_with("integration_routes: ")
                && row.contains("\"destination_label\":\"#auth-dev\"")));
        assert!(added
            .iter()
            .any(|row| row.starts_with("integration_audit_events: ")
                && row.contains("route_created")));
        assert!(
            before.iter().all(|row| after.contains(row)),
            "nothing else changed"
        );
    }

    #[tokio::test]
    async fn refused_destination_check_writes_nothing() {
        let lab = Lab::start().await;
        let before = lab.universe().await;
        let (status, page) = lab.submit("routes", "C0NOTLISTED").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{page}");
        assert!(lab.posts().is_empty(), "a refused check sends nothing");
        assert_eq!(lab.universe().await, before, "no route, lane or audit row");
    }
}
