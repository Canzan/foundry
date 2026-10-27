//! external-integration-plugins — DELIVER Phase 5: the doors and helpers the
//! mutation measurement found untested at crate level. Crate-level tests, no
//! Gherkin; each is named for the behaviour it pins.
//!
//! - Pure (no Docker): the delivery worker's lease floor and off switch, the
//!   Project emails section and its form parse, the project mail's transport
//!   and stop-link signing, and a stop link's addresses.
//! - Through the router over a real Postgres (testcontainers
//!   `postgres:16-alpine`), with a local Slack double recording every call:
//!   the admin-only doors refuse a member, the Slack return refuses a wrong,
//!   missing or stale state, a stop link's three doors, the route form's
//!   heading and channel picker, and the Project emails save without email.
//!
//! `timeout 900 cargo test -p foundry-app --test eip_phase5_doors`

use foundry_app::email_stop::{StopLinks, ONE_CLICK_POST};
use foundry_app::integrations::config::{
    IntegrationSettings, LEASE_SECONDS, MIN_LEASE_SECONDS, WORKER,
};
use foundry_app::integrations::project_emails::{
    parse_choices, section_of, ProjectMail, NO_EMAIL_TRANSPORT,
};
use foundry_integrations::{email_events, EmailLines, EmailScope, EventKind};
use foundry_services::integrations::ProjectEmails;
use foundry_store::integrations::{EmailChoiceRow, EmailProject};
use secrecy::SecretString;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

const PUBLIC_URL: &str = "https://foundry.canzan.dev";

fn worker_settings(
    env: &[(&str, &str)],
) -> Result<
    foundry_app::integrations::config::WorkerSettings,
    foundry_app::integrations::config::ConfigError,
> {
    IntegrationSettings::from_lookup(
        |name| {
            env.iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_string())
        },
        PUBLIC_URL,
    )
    .map(|settings| settings.worker)
}

// DDR-4: the shortest lease an operator may set is MIN_LEASE_SECONDS itself;
// one second under it is refused at startup.
#[test]
fn the_shortest_lease_an_operator_may_set_is_the_floor_itself() {
    let floor = MIN_LEASE_SECONDS.to_string();
    let worker = worker_settings(&[(LEASE_SECONDS, &floor)]).expect("the floor is accepted");
    assert_eq!(worker.lease, Duration::from_secs(MIN_LEASE_SECONDS));
    let under = (MIN_LEASE_SECONDS - 1).to_string();
    assert!(
        matches!(
            worker_settings(&[(LEASE_SECONDS, &under)]),
            Err(foundry_app::integrations::config::ConfigError::LeaseTooShort)
        ),
        "a lease under the floor could expire during its send"
    );
    let over = (MIN_LEASE_SECONDS + 1).to_string();
    assert_eq!(
        worker_settings(&[(LEASE_SECONDS, &over)])
            .expect("over the floor")
            .lease,
        Duration::from_secs(MIN_LEASE_SECONDS + 1)
    );
}

// DEVOPS C-13: FOUNDRY_INTEGRATIONS_WORKER=off stops this process running the
// worker; unset (or anything else) runs it.
#[test]
fn the_worker_runs_unless_switched_off() {
    assert!(worker_settings(&[]).expect("defaults").enabled);
    assert!(worker_settings(&[(WORKER, "on")]).expect("on").enabled);
    for off in ["off", " OFF ", "Off"] {
        assert!(
            !worker_settings(&[(WORKER, off)]).expect("off").enabled,
            "{off:?} switches the worker off"
        );
    }
}

/// Canzan Labs (muted by Sam) with Identity Platform and Auth v2 Spike, and
/// Globex with Billing; `choices` name a project by its place in that list.
struct Offered {
    labs: Uuid,
    ids: [Uuid; 3],
    emails: ProjectEmails,
}

fn offered(choices: &[(usize, EventKind, EmailScope)]) -> Offered {
    let (labs, globex) = (Uuid::now_v7(), Uuid::now_v7());
    let ids = [Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7()];
    let project =
        |project_id, project_name: &str, workspace_id, workspace_name: &str| EmailProject {
            project_id,
            project_name: project_name.into(),
            workspace_id,
            workspace_name: workspace_name.into(),
        };
    let emails = ProjectEmails {
        projects: vec![
            project(ids[0], "Identity Platform", labs, "Canzan Labs"),
            project(ids[1], "Auth v2 Spike", labs, "Canzan Labs"),
            project(ids[2], "Billing", globex, "Globex"),
        ],
        choices: choices
            .iter()
            .map(|(at, event, scope)| EmailChoiceRow {
                project_id: ids[*at],
                event: event.wire_name().into(),
                scope: scope.wire_name().into(),
            })
            .collect(),
    };
    Offered { labs, ids, emails }
}

// US-EIP-10 (OD-10): the section ticks exactly the boxes the member chose, by
// project, event and scope; each box names its scope in K-12's words.
#[test]
fn the_project_emails_section_ticks_exactly_the_chosen_boxes() {
    let offered = offered(&[
        (0, EventKind::IssueCreated, EmailScope::All),
        (1, EventKind::CommentAdded, EmailScope::Mine),
    ]);
    let section = section_of(&offered.emails, &HashSet::new(), true, None);
    assert!(section.can_send);
    assert_eq!(section.unavailable, NO_EMAIL_TRANSPORT);
    let mut ticked = Vec::new();
    let mut words = HashSet::new();
    for (project, id) in section.projects.iter().zip(offered.ids) {
        assert_eq!(project.events.len(), email_events().count());
        for (event, kind) in project.events.iter().zip(email_events()) {
            assert_eq!(event.label, kind.display_name());
            for (tick, scope) in event.boxes.iter().zip(EmailScope::ALL) {
                assert_eq!(
                    tick.value,
                    format!("{id}:{}:{}", kind.wire_name(), scope.wire_name())
                );
                words.insert((scope.wire_name(), tick.words));
                if tick.checked {
                    ticked.push((project.name.clone(), kind, scope));
                }
            }
        }
    }
    assert_eq!(
        ticked,
        vec![
            (
                "Identity Platform".to_string(),
                EventKind::IssueCreated,
                EmailScope::All
            ),
            (
                "Auth v2 Spike".to_string(),
                EventKind::CommentAdded,
                EmailScope::Mine
            ),
        ]
    );
    assert_eq!(
        words,
        HashSet::from([
            ("all", "All issues"),
            ("mine", "Only issues I created or commented on")
        ])
    );
}

// D-9: one note per muted workspace among the offered projects', once, in
// their order; none when nothing is muted.
#[test]
fn a_muted_workspace_is_noted_once() {
    let offered = offered(&[]);
    assert!(section_of(&offered.emails, &HashSet::new(), true, None)
        .muted
        .is_empty());
    let muted = HashSet::from([offered.labs]);
    assert_eq!(
        section_of(&offered.emails, &muted, false, None).muted,
        vec!["Canzan Labs is muted. These choices apply when you resubscribe.".to_string()]
    );
}

// US-EIP-10: the form's other fields are not choices; a choice naming an
// event no member can choose by email refuses the whole form.
#[test]
fn the_form_parse_reads_only_choices() {
    let project = Uuid::now_v7();
    let fields = |choices: &[&str]| -> Vec<(String, String)> {
        let mut all = vec![("_csrf".to_string(), "token".to_string())];
        all.extend(
            choices
                .iter()
                .map(|c| ("choice".to_string(), format!("{project}:{c}"))),
        );
        all
    };
    let parsed = parse_choices(&fields(&["issue.created:all", "comment.added:mine"]))
        .expect("the choices parse");
    assert_eq!(
        parsed
            .iter()
            .map(|c| (c.project_id, c.event, c.scope))
            .collect::<Vec<_>>(),
        vec![
            (project, EventKind::IssueCreated, EmailScope::All),
            (project, EventKind::CommentAdded, EmailScope::Mine),
        ]
    );
    assert_eq!(parse_choices(&fields(&[])).map(|c| c.len()), Some(0));
    assert!(parse_choices(&fields(&["comment.edited:all"])).is_none());
}

// K-13 and ADR-EIP-007 decision 5: project mail cannot send until it has a
// transport, and signs no stop link until it has the secret.
#[test]
fn project_mail_sends_and_signs_only_once_set_up() {
    let mail = ProjectMail::default();
    assert!(!mail.can_send(), "no transport: K-13");
    let (user, project) = (Uuid::now_v7(), Uuid::now_v7());
    assert!(
        mail.stop_links(PUBLIC_URL, user, project, "issue.created")
            .is_none(),
        "no secret, no link (so no email)"
    );
    let notifier = foundry_app::Notifier::new(vec![Arc::new(Relay)]);
    mail.use_transport(notifier.email_transport());
    assert!(mail.can_send());
    let secret = Arc::new(SecretString::from(SESSION_SECRET));
    mail.sign_stop_links_with(secret.clone());
    let links = mail
        .stop_links(PUBLIC_URL, user, project, "issue.created")
        .expect("signed links");
    assert_eq!(
        Some(links),
        StopLinks::sign(PUBLIC_URL, &secret, user, project, "issue.created")
    );
}

/// An SMTP relay that accepts everything.
struct Relay;

#[async_trait::async_trait]
impl foundry_app::NotificationProvider for Relay {
    async fn deliver(
        &self,
        _notification: &foundry_app::Notification,
    ) -> Result<(), foundry_app::DeliveryError> {
        Ok(())
    }

    fn kind(&self) -> foundry_app::ProviderKind {
        foundry_app::ProviderKind::Smtp
    }

    async fn probe(&self) -> Result<(), foundry_app::DeliveryError> {
        Ok(())
    }
}

const SESSION_SECRET: &str = "eip-phase5-secret-thirty-two-bytes-or-more-yes-please";

/// The confirm page's address and the one-click address of a signed link.
fn addresses(links: &StopLinks) -> (String, String) {
    let mut lines = EmailLines {
        subject: "[AUTH-7] New issue".into(),
        body: vec![
            "Lena Park created AUTH-7".into(),
            "Change what you get: https://foundry.canzan.dev/account/settings".into(),
        ],
    };
    links.attach(&mut lines);
    assert_eq!(lines.body.len(), 3, "{:?}", lines.body);
    assert_eq!(lines.body[0], "Lena Park created AUTH-7");
    assert!(
        lines.body[2].starts_with("Change what you get:"),
        "the stop line goes just before the last line: {:?}",
        lines.body
    );
    let page = lines.body[1]
        .strip_prefix("Stop these emails: ")
        .expect("the stop line")
        .to_string();
    let (one_click, post) = links.unsubscribe();
    assert_eq!(post, ONE_CLICK_POST);
    (page, one_click)
}

// US-EIP-11: a signed link's confirm page and one-click addresses carry the
// same signed choice, on this Foundry's address.
#[test]
fn a_stop_link_names_its_confirm_page_and_its_one_click() {
    let secret = SecretString::from(SESSION_SECRET);
    let links = StopLinks::sign(
        PUBLIC_URL,
        &secret,
        Uuid::now_v7(),
        Uuid::now_v7(),
        "issue.created",
    )
    .expect("signed");
    let (page, one_click) = addresses(&links);
    let query = page
        .strip_prefix(&format!("{PUBLIC_URL}/email/stop?"))
        .expect("the confirm page");
    assert!(query.starts_with("t=") && query.contains("&sig="), "{page}");
    assert_eq!(
        one_click,
        format!("{PUBLIC_URL}/email/stop/one-click?{query}")
    );
}

// ADR-EIP-007 decision 5: a project email through the hosted email API is
// posted once, with its recipient, subject and text, as a project email, and
// with the one-click unsubscribe headers it names.
#[tokio::test]
async fn a_project_email_through_the_email_api_carries_its_unsubscribe_headers() {
    use axum::routing::post;
    use foundry_app::notify::EmailApiConfig;
    use foundry_app::{EmailApiProvider, NotificationProvider};
    use foundry_integrations::OutboundEmail;
    let posted: Arc<std::sync::Mutex<Vec<serde_json::Value>>> = Arc::default();
    let app = axum::Router::new().route(
        "/send",
        post({
            let posted = posted.clone();
            move |body: String| async move {
                posted
                    .lock()
                    .unwrap()
                    .push(serde_json::from_str(&body).expect("a JSON body"));
                axum::http::StatusCode::ACCEPTED
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let url = format!("http://{addr}/send");
    let config = EmailApiConfig::from_lookup(|key| match key {
        "EMAIL_API_URL" => Some(url.clone()),
        "EMAIL_API_KEY" => Some("eip-phase5-api-key".to_string()),
        "EMAIL_API_FROM" => Some("foundry@canzan.dev".to_string()),
        _ => None,
    })
    .expect("config");
    let provider = EmailApiProvider::new(config).expect("provider");
    let one_click = "https://foundry.canzan.dev/email/stop/one-click?t=x&sig=y";
    provider
        .send_email(&OutboundEmail {
            recipient: "sam.okafor@canzan.dev".into(),
            subject: "[AUTH-7] New issue: Move Grafana sign-in to Keycloak".into(),
            text_body: "Lena Park created AUTH-7".into(),
            unsubscribe: Some((one_click.into(), ONE_CLICK_POST.into())),
        })
        .await
        .expect("sent");
    assert_eq!(
        *posted.lock().unwrap(),
        vec![serde_json::json!({
            "from": "foundry@canzan.dev",
            "to": "sam.okafor@canzan.dev",
            "subject": "[AUTH-7] New issue: Move Grafana sign-in to Keycloak",
            "body": "Lena Park created AUTH-7",
            "event": "project_email",
            "headers": {
                "List-Unsubscribe": format!("<{one_click}>"),
                "List-Unsubscribe-Post": ONE_CLICK_POST,
            },
        })]
    );
}

mod router {
    use super::{addresses, SESSION_SECRET};
    use axum::body::Body;
    use axum::http::{header, Request, StatusCode};
    use axum::routing::{self, post};
    use axum::Router;
    use foundry_app::clock::MockClock;
    use foundry_app::email_stop::StopLinks;
    use foundry_app::integrations::config::IntegrationSettings;
    use foundry_app::integrations::{Integrations, IntegrationsHandle};
    use foundry_app::{build_router, AppState, Notifier};
    use foundry_auth::secret_box::SecretSlot;
    use foundry_services::integrations::RouteDraft;
    use foundry_services::{Principal, Services};
    use foundry_store::integrations::NewConnection;
    use foundry_store::{run_migrations, Store};
    use secrecy::SecretString;
    use serde_json::json;
    use sqlx::postgres::PgPoolOptions;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use testcontainers_modules::postgres::Postgres;
    use testcontainers_modules::testcontainers::runners::AsyncRunner;
    use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
    use tower::ServiceExt;
    use uuid::Uuid;

    const CSRF: &str = "eip-phase5-csrf-token";
    const PASSWORD: &str = "eip-phase5-password-long-enough";
    const PRIYA: &str = "priya.raman@canzan.dev";
    const SAM: &str = "sam.okafor@canzan.dev";
    const TOKEN: &str = "xoxb-eip-phase5-sentinel-token";
    const SECRETS_KEY: &str = "ZWlwLXNlbnRpbmVsLXNlY3JldHMta2V5LTMyYnl0ZXM";
    const PAGE: &str = "/team/backend/project/identity-platform/integrations";
    const NOT_FOUND: &str = "The requested resource does not exist or is not available.";
    const K3: &str = "Slack was not connected. Nothing changed.";
    const LINK_REFUSED: &str = "This unsubscribe link is no longer valid";

    /// Slack's calls as the double received them, by method.
    type Calls = Arc<Mutex<Vec<String>>>;

    /// A local Slack: the install exchange, `auth.test`, and a channel list
    /// with one public channel Foundry is not in, one private channel it is
    /// in, one private channel it is not in, and one archived channel.
    async fn slack() -> (String, Calls) {
        let calls: Calls = Arc::default();
        let record = |calls: &Calls, method: &str| calls.lock().unwrap().push(method.into());
        let app = Router::new()
            .route(
                "/oauth.v2.access",
                post({
                    let calls = calls.clone();
                    move || async move {
                        record(&calls, "oauth.v2.access");
                        axum::Json(json!({
                            "ok": true, "access_token": TOKEN, "token_type": "bot",
                            "bot_user_id": "U0FOUNDRYBOT", "app_id": "A0EIPFOUNDRY",
                            "team": { "id": "T0CANZANLABS", "name": "Canzan Labs" }
                        }))
                    }
                }),
            )
            .route(
                "/auth.test",
                post({
                    let calls = calls.clone();
                    move || async move {
                        record(&calls, "auth.test");
                        axum::Json(json!({
                            "ok": true, "url": "https://canzanlabs.slack.com/",
                            "team_id": "T0CANZANLABS"
                        }))
                    }
                }),
            )
            .route(
                "/conversations.list",
                routing::get({
                    let calls = calls.clone();
                    move || async move {
                        record(&calls, "conversations.list");
                        axum::Json(json!({
                            "ok": true,
                            "channels": [
                                { "id": "C0GENERAL", "name": "general", "is_private": false,
                                  "is_member": false, "is_archived": false },
                                { "id": "C0SECRETIN", "name": "secret-in", "is_private": true,
                                  "is_member": true, "is_archived": false },
                                { "id": "C0SECRETOUT", "name": "secret-out", "is_private": true,
                                  "is_member": false, "is_archived": false },
                                { "id": "C0ARCHIVED", "name": "archived", "is_private": false,
                                  "is_member": true, "is_archived": true }
                            ],
                            "response_metadata": { "next_cursor": "" }
                        }))
                    }
                }),
            );
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
                "eip-phase5-client-secret".to_string(),
            ),
            (
                "SLACK_SIGNING_SECRET",
                "eip-phase5-signing-secret".to_string(),
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

    async fn person(store: &Store, ws: Uuid, email: &str, name: &str, role: &str) -> Uuid {
        let id = Uuid::now_v7();
        let hash = foundry_auth::hash_password(&SecretString::new(PASSWORD.into()))
            .await
            .expect("hash");
        sqlx::query(
            "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
                  VALUES ($1, $2, $2, $3, $4)",
        )
        .bind(id)
        .bind(email)
        .bind(name)
        .bind(&hash)
        .execute(store.pool())
        .await
        .expect("seed user");
        sqlx::query(
            "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, $3)",
        )
        .bind(ws)
        .bind(id)
        .bind(role)
        .execute(store.pool())
        .await
        .expect("seed membership");
        id
    }

    /// Canzan Labs: Priya (workspace admin) and Sam (a member on Backend),
    /// whose Identity Platform (AUTH) Sam can open but not manage.
    struct Lab {
        _pg: ContainerAsync<Postgres>,
        store: Arc<Store>,
        router: Router,
        clock: Arc<MockClock>,
        calls: Calls,
        settings_key: IntegrationSettings,
        url: String,
        ws: Uuid,
        priya: Uuid,
        sam: Uuid,
        auth: Uuid,
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
                .acquire_timeout(Duration::from_secs(20))
                .connect(&url)
                .await
                .expect("connect pool");
            run_migrations(&pool).await.expect("run migrations");
            let store = Arc::new(Store::from_pool(pool));
            let ws = Uuid::now_v7();
            exec(
                &store,
                "INSERT INTO workspaces (id, name) VALUES ($1, 'Canzan Labs')",
                &[ws],
            )
            .await;
            let priya = person(&store, ws, PRIYA, "Priya Raman", "admin").await;
            let sam = person(&store, ws, SAM, "Sam Okafor", "member").await;
            let team = Uuid::now_v7();
            exec(
                &store,
                "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Backend', 'backend')",
                &[team, ws],
            )
            .await;
            exec(
                &store,
                "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')",
                &[team, sam],
            )
            .await;
            let auth = Uuid::now_v7();
            store
                .insert_project(
                    auth,
                    ws,
                    team,
                    "Identity Platform",
                    "identity-platform",
                    "AUTH",
                )
                .await
                .expect("project");
            let (slack, calls) = slack().await;
            let clock = MockClock::new(time::OffsetDateTime::now_utc());
            let state = AppState {
                oidc: None,
                store: store.clone(),
                session_secret: Arc::new(SecretString::new(SESSION_SECRET.into())),
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
                clock: clock.clone(),
                integrations: IntegrationsHandle::new(Integrations::new(settings(&slack))),
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
            Lab {
                _pg: pg,
                store,
                router: build_router(state),
                clock,
                calls,
                settings_key: settings(&slack),
                url,
                ws,
                priya,
                sam,
                auth,
            }
        }

        /// `email` signs in; their cookie jar (with the CSRF cookie).
        async fn sign_in(&self, email: &str) -> String {
            let signed_in = self
                .send(form(
                    "/sign-in",
                    &format!("foundry_csrf={CSRF}"),
                    &[("email", email), ("password", PASSWORD)],
                ))
                .await;
            assert_eq!(
                signed_in.status(),
                StatusCode::SEE_OTHER,
                "{email} signs in"
            );
            let mut jar: Vec<String> = signed_in
                .headers()
                .get_all(header::SET_COOKIE)
                .iter()
                .filter_map(|v| v.to_str().ok()?.split(';').next().map(str::to_string))
                .collect();
            jar.push(format!("foundry_csrf={CSRF}"));
            jar.join("; ")
        }

        async fn send(&self, request: Request<Body>) -> axum::response::Response {
            self.router
                .clone()
                .oneshot(request)
                .await
                .expect("response")
        }

        /// The status and body of `request`.
        async fn answer(&self, request: Request<Body>) -> (StatusCode, String) {
            let response = self.send(request).await;
            let status = response.status();
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("body");
            (status, String::from_utf8_lossy(&body).into_owned())
        }

        /// Canzan Labs' Slack connection, its token sealed for its row.
        async fn connect_slack(&self) {
            let connection = Uuid::now_v7();
            let sealed = self
                .settings_key
                .secrets
                .as_ref()
                .expect("the secrets key")
                .seal(
                    &SecretString::from(TOKEN),
                    &SecretSlot::new(
                        "integration_connections",
                        &connection.to_string(),
                        "sealed_credential",
                    ),
                )
                .expect("seal");
            self.store
                .upsert_connection(&NewConnection {
                    id: connection,
                    workspace_id: self.ws,
                    kind: "slack".into(),
                    external_id: "T0CANZANLABS".into(),
                    app_id: Some("A0EIPFOUNDRY".into()),
                    bot_user_id: Some("U0FOUNDRYBOT".into()),
                    label: "Canzan Labs".into(),
                    detail: Some("canzanlabs.slack.com".into()),
                    sealed_credential: sealed.as_bytes().to_vec(),
                    connected_by: self.priya,
                    connected_at: time::OffsetDateTime::now_utc(),
                })
                .await
                .expect("seed the Slack connection");
        }

        /// Every integration row and email choice, as JSON, table by table.
        async fn universe(&self) -> Vec<String> {
            let mut rows = Vec::new();
            for table in [
                "integration_connections",
                "integration_routes",
                "integration_route_lanes",
                "integration_deliveries",
                "integration_audit_events",
                "integration_email_choices",
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

        fn slack_calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }

        /// Add to Slack: the approval address's state, kept in the session.
        async fn start_install(&self, cookie: &str) -> String {
            let start = self
                .send(get("/workspace/integrations/slack/install", cookie))
                .await;
            assert!(
                start.status().is_redirection(),
                "Add to Slack goes to Slack"
            );
            let to_slack = start.headers()[header::LOCATION]
                .to_str()
                .expect("location")
                .to_string();
            to_slack
                .split("state=")
                .nth(1)
                .expect("the approval address carries the state")
                .split('&')
                .next()
                .expect("the state")
                .to_string()
        }

        async fn seed_choice(&self, event: &str) {
            sqlx::query(
                "INSERT INTO integration_email_choices (user_id, project_id, event, scope)
                 VALUES ($1, $2, $3, 'all')",
            )
            .bind(self.sam)
            .bind(self.auth)
            .bind(event)
            .execute(self.store.pool())
            .await
            .expect("seed choice");
        }

        async fn choices(&self) -> Vec<String> {
            sqlx::query_scalar(
                "SELECT event FROM integration_email_choices WHERE user_id = $1 ORDER BY event",
            )
            .bind(self.sam)
            .fetch_all(self.store.pool())
            .await
            .expect("choices")
        }
    }

    fn get(uri: &str, cookie: &str) -> Request<Body> {
        Request::builder()
            .uri(uri)
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .expect("request")
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

    // SECURITY (US-EIP-01, SC-6): a workspace member who is not an admin gets
    // the uniform not-found at every admin-only integrations door, Slack is
    // never asked anything, and nothing is written. Priya, the admin, gets
    // the same doors.
    #[tokio::test]
    async fn a_member_is_refused_at_every_admin_door() {
        let lab = Lab::start().await;
        let sam = lab.sign_in(SAM).await;
        let before = lab.universe().await;
        let doors = [
            get("/workspace/integrations", &sam),
            get("/workspace/integrations/slack/setup", &sam),
            get("/workspace/integrations/slack/install", &sam),
            get(
                "/workspace/integrations/slack/return?code=eip-code&state=eip-state",
                &sam,
            ),
            form(
                "/workspace/integrations/matrix",
                &sam,
                &[
                    ("homeserver", "https://matrix.canzan.dev"),
                    ("access_token", "syt_eip_phase5"),
                ],
            ),
            get("/workspace/integrations/slack/disconnect", &sam),
            form("/workspace/integrations/slack/disconnect", &sam, &[]),
            get(&format!("{PAGE}/routes/new?kind=slack"), &sam),
            form(
                &format!("{PAGE}/routes"),
                &sam,
                &[
                    ("kind", "slack"),
                    ("destination", "C0GENERAL"),
                    ("events", "issue.created"),
                    ("content_level", "summary"),
                ],
            ),
        ];
        for door in doors {
            let what = format!("{} {}", door.method(), door.uri());
            let (status, body) = lab.answer(door).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{what}: {body}");
            assert!(body.contains(NOT_FOUND), "{what}: {body}");
        }
        assert!(
            lab.slack_calls().is_empty(),
            "Slack was asked nothing: {:?}",
            lab.slack_calls()
        );
        assert_eq!(lab.universe().await, before, "nothing was written");

        let priya = lab.sign_in(PRIYA).await;
        let (status, setup) = lab
            .answer(get("/workspace/integrations/slack/setup", &priya))
            .await;
        assert_eq!(status, StatusCode::OK, "{setup}");
        assert!(setup.contains("/integrations/slack/events"), "{setup}");
        lab.start_install(&priya).await;
    }

    // ADR-EIP-005 decision 4: Slack's return is accepted only with the state
    // this session was given, for this workspace, under ten minutes old. A
    // wrong, missing or stale state shows K-3, asks Slack for nothing and
    // connects nothing; the right one connects.
    #[tokio::test]
    async fn the_slack_return_connects_only_with_the_state_it_was_given() {
        let lab = Lab::start().await;
        let priya = lab.sign_in(PRIYA).await;
        let before = lab.universe().await;
        let refused = |lab: &Lab, case: &str, status: StatusCode, page: &str| {
            assert_eq!(status, StatusCode::OK, "{case}: {page}");
            assert!(page.contains(K3), "{case}: {page}");
            assert!(
                lab.slack_calls().is_empty(),
                "{case}: no exchange: {:?}",
                lab.slack_calls()
            );
        };

        let state = lab.start_install(&priya).await;
        let (status, page) = lab
            .answer(get(
                &format!("/workspace/integrations/slack/return?code=eip-code&state={state}x"),
                &priya,
            ))
            .await;
        refused(&lab, "a wrong state", status, &page);

        lab.start_install(&priya).await;
        let (status, page) = lab
            .answer(get(
                "/workspace/integrations/slack/return?code=eip-code",
                &priya,
            ))
            .await;
        refused(&lab, "no state", status, &page);

        let state = lab.start_install(&priya).await;
        lab.clock.advance(Duration::from_secs(10 * 60 + 1));
        let (status, page) = lab
            .answer(get(
                &format!("/workspace/integrations/slack/return?code=eip-code&state={state}"),
                &priya,
            ))
            .await;
        refused(&lab, "a stale state", status, &page);
        assert_eq!(lab.universe().await, before, "nothing connected");

        let state = lab.start_install(&priya).await;
        let back = lab
            .send(get(
                &format!("/workspace/integrations/slack/return?code=eip-code&state={state}"),
                &priya,
            ))
            .await;
        assert_eq!(
            back.status(),
            StatusCode::SEE_OTHER,
            "the right state connects"
        );
        let connected: i64 =
            sqlx::query_scalar("SELECT count(*) FROM integration_connections WHERE kind = 'slack'")
                .fetch_one(lab.store.pool())
                .await
                .expect("count");
        assert_eq!(connected, 1);
    }

    // US-EIP-11: a stop link's confirm page names the choice and changes
    // nothing; its confirm stops that one choice and says so; the one-click
    // does the same with an empty 200; a tampered link is refused and stops
    // nothing; a store failure answers 500.
    #[tokio::test]
    async fn a_stop_links_three_doors() {
        let lab = Lab::start().await;
        for event in ["comment.added", "issue.created", "issue.moved"] {
            lab.seed_choice(event).await;
        }
        let secret = SecretString::from(SESSION_SECRET);
        let link = |event: &str| {
            let links = StopLinks::sign("", &secret, lab.sam, lab.auth, event).expect("signed");
            addresses(&links)
        };

        let (page, _) = link("issue.created");
        let (status, confirm) = lab.answer(get(&page, "")).await;
        assert_eq!(status, StatusCode::OK, "{confirm}");
        assert!(
            confirm.contains("<h1>Stop New issue emails from Identity Platform?</h1>"),
            "{confirm}"
        );
        assert_eq!(
            lab.choices().await.len(),
            3,
            "the confirm page stops nothing"
        );

        let (tampered, _) = link("issue.moved");
        let (status, refusal) = lab
            .answer(get(&tampered.replace("sig=", "sig=0"), ""))
            .await;
        assert_eq!(status, StatusCode::OK);
        assert!(refusal.contains(LINK_REFUSED), "{refusal}");

        let query = page.split('?').nth(1).expect("the link's query");
        let (status, stopped) = lab
            .answer(
                Request::builder()
                    .method("POST")
                    .uri("/email/stop")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .header(header::COOKIE, format!("foundry_csrf={CSRF}"))
                    .body(Body::from(format!("_csrf={CSRF}&{query}")))
                    .expect("request"),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{stopped}");
        assert!(
            stopped.contains(
                "<h1>Stopped: New issue in Identity Platform. Your other choices are unchanged.</h1>"
            ),
            "{stopped}"
        );
        assert_eq!(lab.choices().await, vec!["comment.added", "issue.moved"]);

        let (_, one_click) = link("comment.added");
        let response = lab
            .send(
                Request::builder()
                    .method("POST")
                    .uri(&one_click)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            lab.choices().await,
            vec!["issue.moved"],
            "one-click stops it"
        );

        sqlx::query(
            "ALTER TABLE integration_email_choices RENAME TO integration_email_choices_gone",
        )
        .execute(lab.store.pool())
        .await
        .expect("break the store");
        let (_, one_click) = link("issue.moved");
        let failed = lab
            .send(
                Request::builder()
                    .method("POST")
                    .uri(&one_click)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await;
        assert_eq!(failed.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    /// `foundry doctor rotate-secrets` against the lab's database with the
    /// given key settings: its exit code, stdout and stderr.
    fn rotate(database_url: &str, current: &str, previous: Option<&str>) -> (i32, String, String) {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_foundry"));
        command
            .args(["doctor", "rotate-secrets"])
            .env("DATABASE_URL", database_url)
            .env("FOUNDRY_SECRETS_KEY", current)
            .env_remove("FOUNDRY_SECRETS_KEY_PREVIOUS");
        if let Some(previous) = previous {
            command.env("FOUNDRY_SECRETS_KEY_PREVIOUS", previous);
        }
        let out = command.output().expect("run the foundry binary");
        (
            out.status.code().unwrap_or(-100),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    // C-11: `foundry doctor rotate-secrets` exits 2 without a new key, 4 when
    // a stored credential opens under neither key (nothing changed), and 0
    // once every credential is sealed again under the new key.
    #[tokio::test]
    async fn rotate_secrets_answers_with_its_exit_codes() {
        const NEW_KEY: &str = "ZWlwLXBoYXNlNS1yb3RhdGVkLW5ldy1rZXktMzJieXQ";
        const UNRELATED_KEY: &str = "ZWlwLXBoYXNlNS11bnJlbGF0ZWQta2V5LTMyYnl0ZXM";
        let lab = Lab::start().await;
        lab.connect_slack().await;
        let url = lab.url.clone();
        let sealed = || async {
            sqlx::query_scalar::<_, Vec<u8>>(
                "SELECT sealed_credential FROM integration_connections WHERE kind = 'slack'",
            )
            .fetch_one(lab.store.pool())
            .await
            .expect("the credential")
        };
        let before = sealed().await;

        let (code, _, stderr) = tokio::task::spawn_blocking({
            let url = url.clone();
            move || rotate(&url, "   ", None)
        })
        .await
        .expect("join");
        assert_eq!(code, 2, "{stderr}");
        assert!(
            stderr.contains("FOUNDRY_SECRETS_KEY is not set"),
            "{stderr}"
        );

        let (code, _, stderr) = tokio::task::spawn_blocking({
            let url = url.clone();
            move || rotate(&url, UNRELATED_KEY, None)
        })
        .await
        .expect("join");
        assert_eq!(code, 4, "{stderr}");
        assert_eq!(sealed().await, before, "nothing changed");

        let (code, stdout, stderr) = tokio::task::spawn_blocking({
            let url = url.clone();
            move || rotate(&url, NEW_KEY, Some(SECRETS_KEY))
        })
        .await
        .expect("join");
        assert_eq!(code, 0, "{stderr}");
        assert!(
            stdout.starts_with("rotate-secrets: 1 credential(s) sealed again under key "),
            "{stdout}"
        );
        let id: Uuid = sqlx::query_scalar("SELECT id FROM integration_connections")
            .fetch_one(lab.store.pool())
            .await
            .expect("id");
        let new_key =
            foundry_auth::secret_box::SecretBox::from_settings(&SecretString::from(NEW_KEY), None)
                .expect("the new key");
        let opened = new_key
            .open(
                &foundry_auth::secret_box::SealedSecret::from_bytes(sealed().await),
                &foundry_app::integrations::credential_slot(id),
            )
            .expect("the credential opens under the new key alone");
        assert_eq!(secrecy::ExposeSecret::expose_secret(&opened), TOKEN);
    }

    /// The destination picker's channel ids, in order.
    fn picker_of(page: &str) -> Vec<String> {
        let start = page
            .find("name=\"destination\"")
            .expect("the destination picker");
        let picker = &page[start..start + page[start..].find("</select>").expect("its end")];
        picker
            .split("<option value=\"")
            .skip(1)
            .map(|rest| rest[..rest.find('"').expect("a quoted value")].to_string())
            .filter(|value| !value.is_empty())
            .collect()
    }

    // US-EIP-02: the new route form is headed for its kind and project, and
    // its picker offers the channels Foundry can post to: a public one, and a
    // private one Foundry is in; never a private one it is not in, nor an
    // archived one. The edit form is headed for the route's channel.
    #[tokio::test]
    async fn the_route_form_heading_and_its_postable_channels() {
        let lab = Lab::start().await;
        lab.connect_slack().await;
        let priya = lab.sign_in(PRIYA).await;
        let (status, page) = lab
            .answer(get(&format!("{PAGE}/routes/new?kind=slack"), &priya))
            .await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert!(
            page.contains("<h1>New Slack route for Identity Platform</h1>"),
            "{page}"
        );
        assert_eq!(picker_of(&page), vec!["C0GENERAL", "C0SECRETIN"]);

        let route = Services::new(lab.store.clone())
            .create_route(
                &Principal::Human {
                    user_id: lab.priya,
                    workspace_id: lab.ws,
                },
                "backend",
                "identity-platform",
                RouteDraft {
                    kind: "slack".into(),
                    destination_ref: "C0GENERAL".into(),
                    destination_label: "#general".into(),
                    events: vec!["issue.created".into()],
                    content_level: "summary".into(),
                    lanes: vec![],
                    shared_confirmed: false,
                },
                time::OffsetDateTime::now_utc(),
            )
            .await
            .expect("a route to #general");
        let (status, page) = lab
            .answer(get(&format!("{PAGE}/routes/{route}/edit"), &priya))
            .await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert!(
            page.contains("<h1>Edit the route to #general</h1>"),
            "{page}"
        );
        // The edit form offers every chat event and content level, with the
        // route's own ticked.
        let ticks = |name: &str, kind: &str, offered: &[(&str, &str, bool)]| {
            for (value, label, chosen) in offered {
                let tick = if *chosen { " checked" } else { "" };
                let input = format!(
                    "<input type=\"{kind}\" name=\"{name}\" value=\"{value}\"{tick}> {label}</label>"
                );
                assert!(page.contains(&input), "{input}: {page}");
            }
        };
        ticks(
            "events",
            "checkbox",
            &[
                ("issue.created", "New issue", true),
                ("issue.moved", "Issue moved", false),
                ("comment.added", "New comment", false),
                ("issue.deleted", "Issue deleted", false),
            ],
        );
        ticks(
            "content_level",
            "radio",
            &[
                ("link_only", "Link only", false),
                ("summary", "Summary", true),
                ("summary_with_comment", "Summary with comment text", false),
            ],
        );
    }

    // K-13: without email delivery the Project emails save saves nothing and
    // shows the settings page for projects the member can open; a choice for
    // a project they cannot open is the uniform not-found.
    #[tokio::test]
    async fn the_project_emails_save_without_email_saves_nothing() {
        let lab = Lab::start().await;
        let sam = lab.sign_in(SAM).await;
        let offered = format!("{}:issue.created:all", lab.auth);
        let (status, page) = lab
            .answer(form(
                "/account/settings/project-emails",
                &sam,
                &[("choice", &offered)],
            ))
            .await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert!(page.contains(super::NO_EMAIL_TRANSPORT), "{page}");
        let elsewhere = format!("{}:issue.created:all", Uuid::now_v7());
        let (status, page) = lab
            .answer(form(
                "/account/settings/project-emails",
                &sam,
                &[("choice", &offered), ("choice", &elsewhere)],
            ))
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{page}");
        assert!(lab.choices().await.is_empty(), "nothing saved");
    }
}
