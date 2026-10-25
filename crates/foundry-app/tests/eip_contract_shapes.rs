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
pending!(csrf_exempt_set_is_exactly_three_paths, "03-02", "check-arch rule 5: is_exempt_path is /bootstrap, /integrations/slack/events, /email/stop/one-click and nothing else");

// DDR-23 (03-02): the connection-lost admin email is a suppressible account
// event (D-15: a muted admin is not emailed), and the composition root's
// register-at-0 series name it for every active provider and outcome.
#[test]
fn connection_lost_event_is_registered_at_zero() {
    use foundry_app::notify::{delivery_zero_series, suppressions_zero_series};
    use foundry_app::{DeliveryOutcome, NotificationEvent, ProviderKind};
    let event = NotificationEvent::IntegrationConnectionLost;
    assert_eq!(event.as_str(), "integration_connection_lost");
    assert!(
        event.is_suppressible(),
        "a muted admin is not emailed (D-15)"
    );
    assert!(NotificationEvent::ALL.contains(&event));
    assert!(suppressions_zero_series().contains(&event));
    let series = delivery_zero_series(&ProviderKind::ALL);
    for provider in ProviderKind::ALL {
        for outcome in DeliveryOutcome::ALL {
            assert!(
                series.contains(&(provider, event, outcome)),
                "{} {} {} registers at 0",
                provider.as_str(),
                event.as_str(),
                outcome.as_str()
            );
        }
    }
}

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

    /// The app's state over `store`, with `integrations` and `notifier`.
    fn app_state(
        store: Arc<Store>,
        integrations: IntegrationsHandle,
        notifier: Arc<Notifier>,
    ) -> AppState {
        AppState {
            oidc: None,
            store,
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
            integrations,
            notifier,
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
        }
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
        let state = app_state(
            store.clone(),
            IntegrationsHandle::new(Integrations::new(settings(&slack().await))),
            Arc::new(Notifier::new(vec![])),
        );
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

    /// Records every notification a provider is handed.
    #[derive(Default)]
    struct Mailbox(std::sync::Mutex<Vec<(String, String, String)>>);

    /// The provider handing every notification to a shared [`Mailbox`].
    struct Inbox(Arc<Mailbox>);

    #[async_trait::async_trait]
    impl foundry_app::NotificationProvider for Inbox {
        async fn deliver(
            &self,
            n: &foundry_app::Notification,
        ) -> Result<(), foundry_app::DeliveryError> {
            self.0 .0.lock().unwrap().push((
                n.recipient.clone(),
                n.subject.clone(),
                n.body.clone(),
            ));
            Ok(())
        }
        fn kind(&self) -> foundry_app::ProviderKind {
            foundry_app::ProviderKind::Log
        }
        async fn probe(&self) -> Result<(), foundry_app::DeliveryError> {
            Ok(())
        }
    }

    /// Slack's `v0` request signature over `v0:{ts}:{body}`.
    fn slack_signature(secret: &str, ts: i64, body: &str) -> String {
        use hmac::Mac;
        let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(format!("v0:{ts}:{body}").as_bytes());
        let hex: String = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        format!("v0={hex}")
    }

    /// A post to the events address, as Slack (or someone) sends it: no
    /// cookie, no CSRF token; signed with `secret` `age` seconds ago.
    fn event(body: &str, secret: Option<&str>, age: i64) -> Request<Body> {
        let ts = time::OffsetDateTime::now_utc().unix_timestamp() - age;
        let mut req = Request::builder()
            .method("POST")
            .uri("/integrations/slack/events")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(secret) = secret {
            req = req
                .header("x-slack-request-timestamp", ts.to_string())
                .header("x-slack-signature", slack_signature(secret, ts, body));
        }
        req.body(Body::from(body.to_string())).expect("request")
    }

    fn uninstall(team: &str, app: &str) -> String {
        serde_json::json!({
            "token": "deprecated", "team_id": team, "api_app_id": app,
            "type": "event_callback", "event_id": "Ev0EIP", "event_time": 1_761_055_320,
            "event": { "type": "app_uninstalled" }
        })
        .to_string()
    }

    async fn text(resp: axum::response::Response) -> String {
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .expect("body");
        String::from_utf8_lossy(&bytes).to_string()
    }

    // DDR-11, DDR-23 (03-02): Slack's events address, through the router with
    // no session and no CSRF token, believes only Slack's signed word: a post
    // that is not Slack's answers 401 and changes nothing; Slack's address
    // check is answered; the Foundry app's removal from the connected Slack
    // workspace turns the connection lost and emails each workspace admin
    // once (a repeat tells no one), and the workspace page then says what
    // broke, with Reconnect.
    #[tokio::test]
    async fn slack_events_believe_only_slack_and_tell_admins_once() {
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
        sqlx::query(
            "INSERT INTO integration_connections
                    (id, workspace_id, kind, state, external_id, app_id, label,
                     sealed_credential, credential_saved_at)
             SELECT $1, id, 'slack', 'connected', 'T0CANZANLABS', 'A0EIPFOUNDRY',
                    'Canzan Labs', '\\x00'::bytea, now() FROM workspaces",
        )
        .bind(uuid::Uuid::now_v7())
        .execute(store.pool())
        .await
        .expect("seed connection");
        let mailbox = Arc::new(Mailbox::default());
        let notifier = Arc::new(Notifier::new(vec![Arc::new(Inbox(mailbox.clone()))]));
        let integrations = IntegrationsHandle::new(Integrations::new(settings(&slack().await)));
        integrations.admin_mail().use_notifier(notifier.clone());
        let state = app_state(store.clone(), integrations, notifier);
        let router = build_router(state);
        let connection = || async {
            sqlx::query_as::<_, (String, Option<String>)>(
                "SELECT state, lost_reason FROM integration_connections",
            )
            .fetch_one(store.pool())
            .await
            .expect("connection")
        };
        let connected = ("connected".to_string(), None);
        let mails = || mailbox.0.lock().unwrap().clone();
        let labs = uninstall("T0CANZANLABS", "A0EIPFOUNDRY");

        for (req, why) in [
            (event(&labs, None, 0), "unsigned"),
            (
                event(&labs, Some("not-the-signing-secret"), 0),
                "wrong secret",
            ),
            (event(&labs, Some(SIGNING_SECRET), 6 * 60), "stale"),
        ] {
            let resp = send(&router, req).await;
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{why}");
            assert_eq!(connection().await, connected, "{why} changes nothing");
        }
        for (team, app) in [
            ("T0CANZANPERSONAL", "A0EIPFOUNDRY"),
            ("T0CANZANLABS", "A0SOMEOTHERAPP"),
        ] {
            let resp = send(
                &router,
                event(&uninstall(team, app), Some(SIGNING_SECRET), 0),
            )
            .await;
            assert_eq!(
                resp.status(),
                StatusCode::OK,
                "{team} {app} is acknowledged"
            );
            assert_eq!(
                connection().await,
                connected,
                "{team} {app} changes nothing"
            );
        }
        // The CSRF exemption is that one path: a cookie-less post anywhere
        // else is refused before any route.
        let mut beside = event(&labs, Some(SIGNING_SECRET), 0);
        *beside.uri_mut() = "/integrations/slack/events/".parse().expect("uri");
        assert_eq!(send(&router, beside).await.status(), StatusCode::FORBIDDEN);
        let check = r#"{"token":"x","challenge":"eip-challenge-1","type":"url_verification"}"#;
        let resp = send(&router, event(check, Some(SIGNING_SECRET), 0)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(text(resp).await.contains("eip-challenge-1"));
        assert_eq!(
            send(&router, event("not json", Some(SIGNING_SECRET), 0))
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
        assert!(mails().is_empty(), "nobody is emailed: {:?}", mails());

        for _ in 0..2 {
            let resp = send(&router, event(&labs, Some(SIGNING_SECRET), 0)).await;
            assert_eq!(
                resp.status(),
                StatusCode::OK,
                "Slack's word is acknowledged"
            );
        }
        assert_eq!(
            connection().await,
            ("lost".to_string(), Some("app_removed".to_string()))
        );
        let told = mails();
        assert_eq!(told.len(), 1, "one email, once: {told:?}");
        assert_eq!(
            (told[0].0.as_str(), told[0].1.as_str()),
            (ADMIN, "Foundry lost its Slack connection for Canzan Labs")
        );
        assert_eq!(
            told[0].2,
            "Foundry lost its Slack connection for Canzan Labs: the Foundry app was removed \
             from Slack. Routes are paused. Reconnect: http://foundry.test/workspace/integrations"
        );

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
        let mut jar = cookies(&signed_in);
        jar.push(format!("foundry_csrf={CSRF}"));
        let page = text(send(&router, get("/workspace/integrations", &jar.join("; "))).await).await;
        assert!(
            page.contains(
                "Foundry lost its Slack connection for Canzan Labs: the Foundry app was removed \
                 from Slack. Routes are paused. <a class=\"button\" \
                 href=\"/workspace/integrations/slack/install\">Reconnect</a>"
            ),
            "{page}"
        );
        assert!(!page.contains("Connected to"), "{page}");
    }

    /// A local Slack whose approval is for the team `team` holds now.
    async fn slack_of(team: Arc<std::sync::Mutex<(&'static str, &'static str)>>) -> String {
        let exchange = {
            let team = team.clone();
            move || {
                let (id, name) = *team.lock().unwrap();
                async move {
                    axum::Json(serde_json::json!({
                        "ok": true, "access_token": TOKEN, "token_type": "bot",
                        "bot_user_id": "U0FOUNDRYBOT", "app_id": "A0EIPFOUNDRY",
                        "team": { "id": id, "name": name }
                    }))
                }
            }
        };
        let app = Router::new()
            .route("/oauth.v2.access", post(exchange))
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

    /// Add to Slack, approved, back on Foundry's return address.
    async fn install(router: &Router, cookie: &str) -> axum::response::Response {
        let start = send(router, get("/workspace/integrations/slack/install", cookie)).await;
        let to_slack = start.headers()[header::LOCATION]
            .to_str()
            .expect("location")
            .to_string();
        let state = to_slack
            .split("state=")
            .nth(1)
            .expect("the approval address carries the state")
            .to_string();
        send(
            router,
            get(
                &format!("/workspace/integrations/slack/return?code={CODE}&state={state}"),
                cookie,
            ),
        )
        .await
    }

    // US-EIP-06 example 5 (03-04): reconnecting a lost Slack connection to
    // another Slack workspace, through the router, says so and leaves the
    // connection row and the audit as they were; reconnecting to the same
    // Slack workspace restores it with one record.
    #[tokio::test]
    async fn reconnect_to_another_slack_team_changes_nothing() {
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
        let team = Arc::new(std::sync::Mutex::new(("T0CANZANLABS", "Canzan Labs")));
        let router = build_router(app_state(
            store.clone(),
            IntegrationsHandle::new(Integrations::new(settings(&slack_of(team.clone()).await))),
            Arc::new(Notifier::new(vec![])),
        ));
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
        let mut jar = cookies(&signed_in);
        jar.push(format!("foundry_csrf={CSRF}"));
        let cookie = jar.join("; ");
        let connected = install(&router, &cookie).await;
        assert_eq!(connected.status(), StatusCode::SEE_OTHER, "Priya connects");
        sqlx::query(
            "UPDATE integration_connections
                SET state = 'lost', lost_reason = 'app_removed', lost_at = now()",
        )
        .execute(store.pool())
        .await
        .expect("the connection is lost");
        let universe = || async {
            let connection: String =
                sqlx::query_scalar("SELECT row_to_json(c)::text FROM integration_connections c")
                    .fetch_one(store.pool())
                    .await
                    .expect("connection");
            let audit: Vec<String> = sqlx::query_scalar(
                "SELECT action FROM integration_audit_events ORDER BY created_at, id",
            )
            .fetch_all(store.pool())
            .await
            .expect("audit");
            (connection, audit)
        };
        let before = universe().await;

        *team.lock().unwrap() = ("T0CANZANPERS", "Canzan Personal");
        let other = install(&router, &cookie).await;
        assert_eq!(other.status(), StatusCode::OK, "Priya stays on the page");
        let page = text(other).await;
        assert!(
            page.contains(
                "This is a different Slack workspace (Canzan Personal). Routes stay paused. \
                 Reconnect to Canzan Labs, or disconnect."
            ),
            "{page}"
        );
        assert_eq!(universe().await, before, "nothing changed");

        *team.lock().unwrap() = ("T0CANZANLABS", "Canzan Labs");
        let same = install(&router, &cookie).await;
        assert_eq!(same.status(), StatusCode::SEE_OTHER, "Priya reconnects");
        let (connection, audit) = universe().await;
        assert!(
            connection.contains("\"state\":\"connected\""),
            "{connection}"
        );
        assert!(connection.contains("\"lost_reason\":null"), "{connection}");
        assert_eq!(
            audit,
            vec!["connection_connected", "connection_reconnected"]
        );
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
    use foundry_integrations::{compose, ContentLevel, EventKind, EventView};
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

    /// Whether the Foundry bot is a member of #auth-dev, as
    /// `conversations.info` answers.
    type Member = Arc<std::sync::atomic::AtomicBool>;

    async fn slack() -> (String, Calls, Member) {
        let calls: Calls = Arc::default();
        let member: Member = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let info = {
            let (calls, member) = (calls.clone(), member.clone());
            move || async move {
                calls
                    .lock()
                    .unwrap()
                    .push(("conversations.info".into(), Value::Null));
                let is_member = member.load(std::sync::atomic::Ordering::SeqCst);
                axum::Json(json!({
                    "ok": true,
                    "channel": { "id": AUTH_DEV, "name": "auth-dev", "is_private": false,
                                 "is_member": is_member, "is_archived": false }
                }))
            }
        };
        let app = Router::new()
            .route("/conversations.info", get(info.clone()).post(info))
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
                "/apps.uninstall",
                post(
                    |State(calls): State<Calls>, headers: axum::http::HeaderMap| async move {
                        let bearer = headers
                            .get(header::AUTHORIZATION)
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or_default()
                            .to_string();
                        calls
                            .lock()
                            .unwrap()
                            .push(("apps.uninstall".into(), Value::String(bearer)));
                        axum::Json(json!({ "ok": true }))
                    },
                ),
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
        (format!("http://{addr}"), calls, member)
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
        member: Member,
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
            let (slack, calls, member) = slack().await;
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
                member,
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

    // US-EIP-06 (03-04), D-8: Resume on a route a refusal stopped asks Slack
    // about its channel first. While Foundry is still out of the channel the
    // route keeps its problem and nothing is recorded; once invited back, the
    // problem clears with one record and the route's boundary stays, so what
    // waited is sent. A route with no problem resumes without asking Slack.
    #[tokio::test]
    async fn resume_rechecks_a_stopped_route_with_slack() {
        let lab = Lab::start().await;
        let (status, _) = lab.submit("routes", AUTH_DEV).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "Lena saves the route");
        let route: Uuid = sqlx::query_scalar("SELECT id FROM integration_routes")
            .fetch_one(lab.store.pool())
            .await
            .expect("the route");
        exec(
            &lab.store,
            "UPDATE integration_routes SET problem_reason = 'not_in_channel', problem_at = now()
              WHERE id = $1",
            &[route],
        )
        .await;
        let resume = || async {
            lab.router
                .clone()
                .oneshot(form(
                    &format!("{PAGE}/routes/{route}/resume"),
                    &lab.cookie,
                    &[],
                ))
                .await
                .expect("resume")
                .status()
        };
        let asked = || {
            lab.calls
                .lock()
                .unwrap()
                .iter()
                .filter(|(method, _)| method == "conversations.info")
                .count()
        };
        let before = lab.universe().await;

        lab.member.store(false, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(resume().await, StatusCode::SEE_OTHER);
        assert_eq!(asked(), 1, "Slack is asked about the channel");
        assert_eq!(lab.universe().await, before, "the route stays stopped");

        lab.member.store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(resume().await, StatusCode::SEE_OTHER);
        assert_eq!(asked(), 2);
        let (problem, kept): (Option<String>, bool) = sqlx::query_as(
            "SELECT problem_reason, receives_from = created_at FROM integration_routes",
        )
        .fetch_one(lab.store.pool())
        .await
        .expect("the route");
        assert_eq!(problem, None, "the problem is cleared");
        assert!(kept, "what waited is still the route's to send");
        let audit: Vec<String> = sqlx::query_scalar(
            "SELECT action FROM integration_audit_events
              WHERE action LIKE 'route_%' ORDER BY created_at, id",
        )
        .fetch_all(lab.store.pool())
        .await
        .expect("audit");
        assert_eq!(audit, vec!["route_created", "route_resumed"]);

        let paused = lab
            .router
            .clone()
            .oneshot(form(
                &format!("{PAGE}/routes/{route}/pause"),
                &lab.cookie,
                &[],
            ))
            .await
            .expect("pause");
        assert_eq!(paused.status(), StatusCode::SEE_OTHER);
        assert_eq!(resume().await, StatusCode::SEE_OTHER);
        assert_eq!(asked(), 2, "a paused route resumes without asking Slack");
        assert_eq!(lab.posts().len(), 0, "resuming sends nothing itself");
    }

    // US-EIP-07 (03-05): Disconnect Slack is an admin's door (a lead gets the
    // uniform 404 and nothing changes); its GET counts what stops and writes
    // nothing; a POST without the page's token is turned away; the confirm
    // forgets the credential, skips what the routes waited to send, records
    // it, and only then asks Slack once, with that credential, to withdraw
    // Foundry's access. A second confirm changes nothing and asks no more.
    #[tokio::test]
    async fn disconnect_asks_first_then_forgets_the_credential_and_revokes_once() {
        let lab = Lab::start().await;
        let (status, _) = lab.submit("routes", AUTH_DEV).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "Lena saves the route");
        let route: Uuid = sqlx::query_scalar("SELECT id FROM integration_routes")
            .fetch_one(lab.store.pool())
            .await
            .expect("the route");
        sqlx::query(
            "INSERT INTO integration_deliveries
                    (workspace_id, project_id, route_id, event_kind, event_id, outbox_ids,
                     occurred_at, give_up_at)
             SELECT workspace_id, project_id, id, 'issue.created', gen_random_uuid(), '{1}',
                    now(), now() + interval '1 day'
               FROM integration_routes WHERE id = $1",
        )
        .bind(route)
        .execute(lab.store.pool())
        .await
        .expect("a waiting delivery");
        const DOOR: &str = "/workspace/integrations/slack/disconnect";
        let open = || async {
            let response = lab
                .router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(DOOR)
                        .header(header::COOKIE, &lab.cookie)
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("dialog");
            let status = response.status();
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body");
            (status, String::from_utf8_lossy(&body).into_owned())
        };
        let confirm = |token: bool| {
            let request = if token {
                form(DOOR, &lab.cookie, &[])
            } else {
                Request::builder()
                    .method("POST")
                    .uri(DOOR)
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .header(header::COOKIE, &lab.cookie)
                    .body(Body::empty())
                    .expect("request")
            };
            let router = lab.router.clone();
            async move { router.oneshot(request).await.expect("confirm") }
        };
        let uninstalls = || {
            lab.calls
                .lock()
                .unwrap()
                .iter()
                .filter(|(method, _)| method == "apps.uninstall")
                .map(|(_, bearer)| bearer.clone())
                .collect::<Vec<_>>()
        };
        let before = lab.universe().await;

        assert_eq!(
            open().await.0,
            StatusCode::NOT_FOUND,
            "a lead is not an admin"
        );
        assert_eq!(confirm(true).await.status(), StatusCode::NOT_FOUND);
        assert_eq!(lab.universe().await, before, "nothing changed");

        exec(
            &lab.store,
            "UPDATE workspace_memberships SET role = 'admin'",
            &[],
        )
        .await;
        let (status, page) = open().await;
        assert_eq!(status, StatusCode::OK, "the admin is asked first");
        let question = "Disconnect Slack? 1 route in 1 project stops posting. \
                        Foundry&#x27;s access to Canzan Labs&#x27; Slack is removed.";
        assert!(page.contains(question), "{page}");
        assert!(page.contains(&format!("action=\"{DOOR}\"")), "{page}");
        let forged = confirm(false).await;
        assert_eq!(
            forged.status(),
            StatusCode::FORBIDDEN,
            "no token, no change"
        );
        assert_eq!(
            lab.universe().await,
            before,
            "asking and forging write nothing"
        );
        assert!(uninstalls().is_empty(), "Slack is asked nothing yet");

        let done = confirm(true).await;
        assert_eq!(done.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            done.headers()[header::LOCATION],
            "/workspace/integrations",
            "back to the workspace page"
        );
        assert_eq!(
            uninstalls(),
            vec![Value::String(format!("Bearer {TOKEN}"))],
            "Slack is asked once, with the forgotten credential"
        );
        let (state, sealed): (String, Option<Vec<u8>>) =
            sqlx::query_as("SELECT state, sealed_credential FROM integration_connections")
                .fetch_one(lab.store.pool())
                .await
                .expect("the connection");
        assert_eq!((state.as_str(), sealed), ("disconnected", None));
        let delivery: String = sqlx::query_scalar("SELECT status FROM integration_deliveries")
            .fetch_one(lab.store.pool())
            .await
            .expect("the delivery");
        assert_eq!(delivery, "skipped", "what waited is never sent");
        let audit: Vec<String> = sqlx::query_scalar(
            "SELECT action FROM integration_audit_events
              WHERE action LIKE 'connection_%' ORDER BY created_at, id",
        )
        .fetch_all(lab.store.pool())
        .await
        .expect("audit");
        assert_eq!(
            audit,
            vec!["connection_connected", "connection_disconnected"]
        );
        let workspace = lab
            .router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/workspace/integrations")
                    .header(header::COOKIE, &lab.cookie)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("page");
        let workspace = axum::body::to_bytes(workspace.into_body(), usize::MAX)
            .await
            .expect("body");
        let workspace = String::from_utf8_lossy(&workspace);
        assert!(
            workspace.contains("Add to Slack") && !workspace.contains(DOOR),
            "Add to Slack is offered again, Disconnect no more: {workspace}"
        );

        let after = lab.universe().await;
        let again = confirm(true).await;
        assert_eq!(again.status(), StatusCode::SEE_OTHER);
        assert_eq!(uninstalls().len(), 1, "Slack is not asked again");
        assert_eq!(
            lab.universe().await,
            after,
            "a second confirm changes nothing"
        );
        assert!(
            open().await.0.is_redirection(),
            "nothing left to disconnect: back to the workspace page"
        );
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

    /// The preview section's paragraphs (tags stripped) and its links.
    fn preview_of(page: &str) -> (Vec<String>, Vec<String>) {
        let start = page.find("data-route-preview").expect("a preview section");
        let section = &page[start..start + page[start..].find("</section>").expect("its end")];
        let paragraphs = section
            .split("<p>")
            .skip(1)
            .map(|p| {
                let inner = &p[..p.find("</p>").expect("a closed paragraph")];
                let mut text = String::new();
                let mut in_tag = false;
                for c in inner.chars() {
                    match c {
                        '<' => in_tag = true,
                        '>' => in_tag = false,
                        c if !in_tag => text.push(c),
                        _ => {}
                    }
                }
                text
            })
            .collect();
        let links = section
            .split("href=\"")
            .skip(1)
            .map(|rest| rest[..rest.find('"').expect("a quoted href")].to_string())
            .collect();
        (paragraphs, links)
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

    /// DDR-25 (02-03): the preview re-renders the form with `compose`'s lines
    /// for the project's newest issue, string for string, at every level; it
    /// asks Slack nothing (not even the channel list) and changes no
    /// integration_* row.
    #[tokio::test]
    async fn preview_calls_no_port_and_writes_nothing() {
        let lab = Lab::start().await;
        sqlx::query(
            "INSERT INTO issues (id, project_id, workspace_id, number, title, state, author_id)
             SELECT gen_random_uuid(), p.id, p.workspace_id, 12, 'Rotate LLDAP bind password',
                    'backlog', u.id
               FROM projects p, users u",
        )
        .execute(lab.store.pool())
        .await
        .expect("seed AUTH-12");
        let view = EventView {
            kind: EventKind::IssueCreated,
            issue_key: "AUTH-12".into(),
            title: "Rotate LLDAP bind password".into(),
            project_name: "Identity Platform".into(),
            actor_name: "Lena Park".into(),
            lane_label: "Backlog".into(),
            from_lane_label: String::new(),
            issue_url: "http://foundry.test/team/backend/project/identity-platform/issues/12"
                .into(),
            issue_count: 1,
            comment_text: String::new(),
        };
        let before = lab.universe().await;
        for (wire, level) in [
            ("link_only", ContentLevel::LinkOnly),
            ("summary", ContentLevel::Summary),
            ("summary_with_comment", ContentLevel::SummaryWithComment),
        ] {
            let response = lab
                .router
                .clone()
                .oneshot(form(
                    &format!("{PAGE}/routes/preview"),
                    &lab.cookie,
                    &[
                        ("kind", "slack"),
                        ("destination", AUTH_DEV),
                        ("events", "issue.created"),
                        ("content_level", wire),
                    ],
                ))
                .await
                .expect("response");
            assert_eq!(response.status(), StatusCode::OK, "{wire}");
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body");
            let page = String::from_utf8_lossy(&body);
            let (shown, hrefs) = preview_of(&page);
            let lines = compose(&view, level);
            let mut expected = vec![lines.headline.clone()];
            expected.extend(lines.detail.clone());
            expected.extend(lines.link.as_ref().map(|_| "Open in Foundry".to_string()));
            assert_eq!(shown, expected, "{wire}: the preview is compose's lines");
            assert_eq!(hrefs, lines.link.into_iter().collect::<Vec<_>>());
        }
        assert!(
            lab.calls.lock().unwrap().is_empty(),
            "the preview asked Slack nothing: {:?}",
            lab.calls.lock().unwrap()
        );
        assert_eq!(lab.universe().await, before, "no integration_* row changed");
    }

    /// The destination picker of a preview page: (value, selected) per option.
    fn picker_of(page: &str) -> Vec<(String, bool)> {
        let start = page
            .find("name=\"destination\"")
            .expect("the destination picker");
        let picker = &page[start..start + page[start..].find("</select>").expect("its end")];
        picker
            .split("<option value=\"")
            .skip(1)
            .map(|rest| {
                let value = rest[..rest.find('"').expect("a quoted value")].to_string();
                let tag = &rest[..rest.find('>').expect("a closed option")];
                (value, tag.contains("selected"))
            })
            .collect()
    }

    /// DDR-25 (02-03): a preview asks Slack nothing, so its picker offers only
    /// the channel the form names, chosen — and with no channel chosen it
    /// offers none (no empty channel, no audience notice).
    #[tokio::test]
    async fn preview_picker_offers_only_the_named_channel() {
        let lab = Lab::start().await;
        let mut pickers = Vec::new();
        for destination in ["", AUTH_DEV] {
            let response = lab
                .router
                .clone()
                .oneshot(form(
                    &format!("{PAGE}/routes/preview"),
                    &lab.cookie,
                    &[
                        ("kind", "slack"),
                        ("destination", destination),
                        ("events", "issue.created"),
                        ("content_level", "summary"),
                    ],
                ))
                .await
                .expect("response");
            assert_eq!(response.status(), StatusCode::OK, "{destination:?}");
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body");
            let page = String::from_utf8_lossy(&body).into_owned();
            pickers.push((picker_of(&page), page.contains("data-audience-notice")));
        }
        assert_eq!(
            pickers[0],
            (vec![], false),
            "no channel chosen: the picker offers none and no audience notice shows"
        );
        assert_eq!(
            pickers[1],
            (vec![(AUTH_DEV.to_string(), true)], true),
            "the named channel alone, chosen, with its audience notice"
        );
        assert!(
            lab.calls.lock().unwrap().is_empty(),
            "the preview asked Slack nothing: {:?}",
            lab.calls.lock().unwrap()
        );
    }

    /// The hidden `channel_label.{id}` fields a rendered route form carries.
    fn channel_labels_of(page: &str) -> Vec<(String, String)> {
        page.split("<input type=\"hidden\" name=\"channel_label.")
            .skip(1)
            .map(|rest| {
                let id = rest[..rest.find('"').expect("a quoted name")].to_string();
                let value = rest.split("value=\"").nth(1).expect("a value");
                (
                    format!("channel_label.{id}"),
                    value[..value.find('"').expect("a quoted value")].to_string(),
                )
            })
            .collect()
    }

    /// The picker's option labels and the K-5 audience notice of a page.
    fn picker_labels_and_audience(page: &str) -> (Vec<String>, Option<String>) {
        let start = page
            .find("name=\"destination\"")
            .expect("the destination picker");
        let picker = &page[start..start + page[start..].find("</select>").expect("its end")];
        let labels = picker
            .split("<option ")
            .skip(1)
            .map(|rest| {
                let after = &rest[rest.find('>').expect("a closed option") + 1..];
                after[..after.find("</option>").expect("its end")].to_string()
            })
            .collect();
        let audience = page.find("data-audience-notice>").map(|at| {
            let rest = &page[at + "data-audience-notice>".len()..];
            rest[..rest.find("</p>").expect("a closed notice")].to_string()
        });
        (labels, audience)
    }

    /// 02-03: the preview names the chosen channel as the rendered form named
    /// it — the form carries each offered channel's name in a hidden
    /// `channel_label.{id}` field, and the preview's single-channel picker and
    /// K-5 read that name, never another channel's; without it they fall back
    /// to the channel id. Slack is asked nothing by any preview.
    #[tokio::test]
    async fn preview_names_the_chosen_channel_from_the_rendered_form() {
        let lab = Lab::start().await;
        let response = lab
            .router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "{PAGE}/routes/new?kind=slack&destination={AUTH_DEV}"
                    ))
                    .header(header::COOKIE, &lab.cookie)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK, "the route form");
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let rendered = channel_labels_of(&String::from_utf8_lossy(&body));
        assert_eq!(
            rendered,
            vec![(format!("channel_label.{AUTH_DEV}"), "#auth-dev".to_string())],
            "the form carries the channel's name"
        );
        lab.calls.lock().unwrap().clear();
        let decoy = vec![("channel_label.C0DECOY".to_string(), "#decoy".to_string())];
        let mut answers = Vec::new();
        for labels in [
            rendered.clone(),
            [decoy.clone(), rendered].concat(),
            decoy,
            vec![],
        ] {
            let mut fields: Vec<(&str, &str)> = vec![
                ("kind", "slack"),
                ("destination", AUTH_DEV),
                ("events", "issue.created"),
                ("content_level", "summary"),
            ];
            fields.extend(labels.iter().map(|(n, v)| (n.as_str(), v.as_str())));
            let response = lab
                .router
                .clone()
                .oneshot(form(
                    &format!("{PAGE}/routes/preview"),
                    &lab.cookie,
                    &fields,
                ))
                .await
                .expect("response");
            assert_eq!(response.status(), StatusCode::OK, "{labels:?}");
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body");
            answers.push(picker_labels_and_audience(&String::from_utf8_lossy(&body)));
        }
        let named = |name: &str| {
            (
                vec![name.to_string()],
                Some(format!(
                    "Everyone who can read {name} will see what this route sends. \
                     Foundry cannot check who that is."
                )),
            )
        };
        assert_eq!(
            answers,
            vec![
                named("#auth-dev"),
                named("#auth-dev"),
                named(AUTH_DEV),
                named(AUTH_DEV),
            ],
            "the rendered name; never another channel's; else the channel id"
        );
        assert!(
            lab.calls.lock().unwrap().is_empty(),
            "the preview asked Slack nothing: {:?}",
            lab.calls.lock().unwrap()
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

// DDR-22, U-D8, C-6 (03-06): the operator's delivery metrics register at 0
// over their closed label sets, count what a record or sweep reports, carry
// the lag histogram's explicit buckets, and no label names a channel, room,
// project, workspace, address or person.
mod delivery_metrics {
    use foundry_app::integrations::worker::{
        count_delivery, register_delivery_metrics, register_stall_gauges, report_backlog,
        report_census,
    };
    use foundry_store::integrations::{DeliveryTally, IntegrationBacklog, IntegrationCensus};
    use std::collections::{BTreeMap, BTreeSet};

    type Labels = BTreeMap<String, String>;

    /// `(name, labels, value)` of every sample line of a Prometheus render.
    fn samples(text: &str) -> Vec<(String, Labels, f64)> {
        text.lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|line| {
                let (series, value) = line.rsplit_once(' ').expect("sample line");
                let (name, labels) = match series.split_once('{') {
                    Some((name, rest)) => {
                        let labels = rest
                            .trim_end_matches('}')
                            .split(',')
                            .filter(|p| !p.is_empty())
                            .map(|pair| {
                                let (k, v) = pair.split_once('=').expect("label");
                                (k.to_string(), v.trim_matches('"').to_string())
                            })
                            .collect();
                        (name.to_string(), labels)
                    }
                    None => (series.to_string(), Labels::new()),
                };
                (name, labels, value.parse().expect("value"))
            })
            .collect()
    }

    fn of<'a>(all: &'a [(String, Labels, f64)], name: &str) -> Vec<&'a (String, Labels, f64)> {
        all.iter().filter(|(n, _, _)| n == name).collect()
    }

    fn value(all: &[(String, Labels, f64)], name: &str, labels: &[(&str, &str)]) -> Option<f64> {
        let want: Labels = labels
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        of(all, name)
            .into_iter()
            .find(|(_, l, _)| *l == want)
            .map(|(_, _, v)| *v)
    }

    const KINDS: [&str; 3] = ["slack", "matrix", "email"];
    const EVENTS: [&str; 4] = [
        "issue.created",
        "issue.moved",
        "issue.deleted",
        "comment.added",
    ];
    const OUTCOMES: [&str; 4] = ["delivered", "retried", "failed", "skipped"];

    fn tally(kind: &str, event: &str, status: &str, lag: f64) -> DeliveryTally {
        DeliveryTally {
            kind: kind.to_string(),
            event_kind: event.to_string(),
            status: status.to_string(),
            lag_seconds: lag,
        }
    }

    #[test]
    fn delivery_metrics_register_at_zero_and_count_over_closed_labels() {
        let recorder = foundry_app::metrics_server::prometheus_builder()
            .expect("builder")
            .build_recorder();
        let handle = recorder.handle();
        metrics::with_local_recorder(&recorder, || {
            register_stall_gauges();
            register_delivery_metrics();
        });
        let boot = samples(&handle.render());

        let deliveries = of(&boot, "foundry_integration_deliveries_total");
        assert_eq!(deliveries.len(), 48, "3 kinds x 4 events x 4 outcomes");
        for kind in KINDS {
            for event in EVENTS {
                for outcome in OUTCOMES {
                    assert_eq!(
                        value(
                            &boot,
                            "foundry_integration_deliveries_total",
                            &[("event", event), ("kind", kind), ("outcome", outcome)]
                        ),
                        Some(0.0),
                        "{kind} {event} {outcome} registers at 0"
                    );
                }
            }
        }
        for (name, series) in [
            ("foundry_integration_connections", 6),
            ("foundry_integration_routes", 4),
            ("foundry_integration_deliveries_pending", 3),
            ("foundry_outbox_undispatched", 1),
            ("foundry_outbox_undispatched_oldest_seconds", 1),
            ("foundry_integration_deliveries_pending_oldest_seconds", 1),
        ] {
            let found = of(&boot, name);
            assert_eq!(found.len(), series, "{name} registers {series} series");
            assert!(found.iter().all(|(_, _, v)| *v == 0.0), "{name} reads 0");
        }
        for kind in ["slack", "matrix"] {
            for state in ["connected", "lost", "disconnected"] {
                assert_eq!(
                    value(
                        &boot,
                        "foundry_integration_connections",
                        &[("kind", kind), ("state", state)]
                    ),
                    Some(0.0)
                );
            }
            for state in ["active", "paused"] {
                assert_eq!(
                    value(
                        &boot,
                        "foundry_integration_routes",
                        &[("kind", kind), ("state", state)]
                    ),
                    Some(0.0)
                );
            }
        }
        for kind in KINDS {
            assert_eq!(
                value(
                    &boot,
                    "foundry_integration_deliveries_pending",
                    &[("kind", kind)]
                ),
                Some(0.0)
            );
        }

        metrics::with_local_recorder(&recorder, || {
            count_delivery(&tally("slack", "issue.created", "delivered", 3.0));
            count_delivery(&tally("matrix", "comment.added", "pending", 1.0));
            count_delivery(&tally("email", "issue.moved", "failed", 1.0));
            count_delivery(&tally("slack", "issue.deleted", "skipped", 1.0));
            // Outside the closed sets: not counted, no new series.
            count_delivery(&tally("webhook", "issue.created", "delivered", 1.0));
            count_delivery(&tally("slack", "comment.edited", "delivered", 1.0));
            count_delivery(&tally("slack", "issue.created", "leased", 1.0));
            report_census(&IntegrationCensus {
                connections: vec![("slack".into(), "lost".into(), 2)],
                routes: vec![("matrix".into(), "paused".into(), 5)],
            });
            report_backlog(&IntegrationBacklog {
                undispatched: 3,
                undispatched_oldest_seconds: 7.5,
                pending_by_kind: vec![("matrix".to_string(), 4)],
                pending_oldest_seconds: 9.5,
            });
        });
        let after = samples(&handle.render());
        let counted: Vec<(&Labels, f64)> = of(&after, "foundry_integration_deliveries_total")
            .into_iter()
            .filter(|(_, _, v)| *v != 0.0)
            .map(|(_, l, v)| (l, *v))
            .collect();
        let label = |kind: &str, event: &str, outcome: &str| -> Labels {
            [("event", event), ("kind", kind), ("outcome", outcome)]
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        };
        let want = [
            label("slack", "issue.created", "delivered"),
            label("matrix", "comment.added", "retried"),
            label("email", "issue.moved", "failed"),
            label("slack", "issue.deleted", "skipped"),
        ];
        assert_eq!(counted.len(), 4, "exactly the four closed tallies count");
        for w in &want {
            assert!(counted.contains(&(w, 1.0)), "{w:?} counts once");
        }
        assert_eq!(of(&after, "foundry_integration_deliveries_total").len(), 48);

        // The lag histogram: delivered ones only, with the explicit buckets.
        let buckets = of(&after, "foundry_integration_delivery_lag_seconds_bucket");
        let les: BTreeSet<&str> = buckets.iter().map(|(_, l, _)| l["le"].as_str()).collect();
        assert_eq!(buckets.len(), 12, "one kind observed, 11 buckets and +Inf");
        for le in [
            "1", "2", "5", "10", "30", "60", "300", "900", "3600", "21600", "86400", "+Inf",
        ] {
            assert!(les.contains(le), "bucket le={le}; {les:?}");
        }
        assert!(buckets.iter().all(|(_, l, _)| l["kind"] == "slack"));
        assert_eq!(
            value(
                &after,
                "foundry_integration_delivery_lag_seconds_bucket",
                &[("kind", "slack"), ("le", "2")]
            ),
            Some(0.0)
        );
        assert_eq!(
            value(
                &after,
                "foundry_integration_delivery_lag_seconds_bucket",
                &[("kind", "slack"), ("le", "5")]
            ),
            Some(1.0)
        );
        assert_eq!(
            value(
                &after,
                "foundry_integration_delivery_lag_seconds_sum",
                &[("kind", "slack")]
            ),
            Some(3.0)
        );

        for (name, labels, v) in [
            (
                "foundry_integration_connections",
                vec![("kind", "slack"), ("state", "lost")],
                2.0,
            ),
            (
                "foundry_integration_connections",
                vec![("kind", "slack"), ("state", "connected")],
                0.0,
            ),
            (
                "foundry_integration_routes",
                vec![("kind", "matrix"), ("state", "paused")],
                5.0,
            ),
            (
                "foundry_integration_routes",
                vec![("kind", "matrix"), ("state", "active")],
                0.0,
            ),
            (
                "foundry_integration_deliveries_pending",
                vec![("kind", "matrix")],
                4.0,
            ),
            (
                "foundry_integration_deliveries_pending",
                vec![("kind", "slack")],
                0.0,
            ),
            ("foundry_outbox_undispatched", vec![], 3.0),
            ("foundry_outbox_undispatched_oldest_seconds", vec![], 7.5),
            (
                "foundry_integration_deliveries_pending_oldest_seconds",
                vec![],
                9.5,
            ),
        ] {
            assert_eq!(value(&after, name, &labels), Some(v), "{name} {labels:?}");
        }

        // Criterion 3: no label names a person, place or address.
        let allowed = ["kind", "event", "outcome", "state", "le"];
        for (name, labels, _) in &after {
            if name.starts_with("foundry_integration_") || name.starts_with("foundry_outbox_") {
                for (k, v) in labels {
                    assert!(allowed.contains(&k.as_str()), "{name} carries {k}");
                    let closed = KINDS.contains(&v.as_str())
                        || EVENTS.contains(&v.as_str())
                        || OUTCOMES.contains(&v.as_str())
                        || ["connected", "lost", "disconnected", "active", "paused"]
                            .contains(&v.as_str())
                        || k == "le";
                    assert!(closed, "{name} {k}={v} is from a closed set");
                }
            }
        }
    }
}
