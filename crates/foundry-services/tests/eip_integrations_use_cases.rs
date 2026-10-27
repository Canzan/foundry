//! external-integration-plugins — the integrations use-cases through the
//! `Services` seam over a real Postgres (DELIVER Phase 5: closing the mutation
//! survivors the crate's own suite left). Example-based, one container per
//! test (testcontainers `postgres:16-alpine`; Docker required).
//!
//! What these pin, in Foundry's words:
//! - only a workspace admin passes the integrations admin check;
//! - a reconnect seals for the workspace's existing connection row, and a
//!   first connection gets a fresh row of its own;
//! - a member's project email choices: one per event, "all issues" wins,
//!   each project's choices stay in that project, and a project the member
//!   can no longer open drops out of their section;
//! - a stop link names a live project and an email event, stops exactly that
//!   one choice, and anything else is refused with nothing changed;
//! - a route's events and content level are checked before anything is saved;
//! - a route's history is that route's, not a sibling's;
//! - a disconnect skips every delivery its routes were waiting to send;
//! - the workspace page lists what was done to its integrations.
//!
//! `timeout 900 cargo test -p foundry-services --test eip_integrations_use_cases`

use foundry_integrations::{EmailScope, EventKind};
use foundry_services::integrations::{EmailChoiceInput, EmailStop, IntegrationError, RouteDraft};
use foundry_services::{Principal, Services};
use foundry_store::integrations::{
    DeliveryTarget, DisconnectOutcome, DispatchBatch, NewConnection, NewDelivery,
};
use foundry_store::Store;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, PgPool};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
use time::OffsetDateTime;

type Id = uuid::Uuid;

/// Canzan Labs: Priya (workspace admin), Sam (member) on Backend, whose
/// Identity Platform (AUTH) and Auth v2 Spike (SPIKE) he can open, and on
/// Ops, whose Homelab Ops (OPS) he can open too.
struct Labs {
    services: Services,
    store: Arc<Store>,
    pool: PgPool,
    ws: Id,
    priya: Id,
    sam: Id,
    ops: Id,
    auth: Id,
    spike: Id,
    homelab: Id,
    _c: ContainerAsync<Postgres>,
}

async fn q(pool: &PgPool, sql: &str, binds: &[Id]) {
    let mut query = sqlx::query(sql);
    for b in binds {
        query = query.bind(*b);
    }
    query.execute(pool).await.expect(sql);
}

async fn user(pool: &PgPool, ws: Id, email: &str, role: &str) -> Id {
    let id = Id::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, $2, 'x')",
    )
    .bind(id)
    .bind(email)
    .execute(pool)
    .await
    .expect("user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, $3)",
    )
    .bind(ws)
    .bind(id)
    .bind(role)
    .execute(pool)
    .await
    .expect("membership");
    id
}

async fn team(pool: &PgPool, ws: Id, name: &str, slug: &str) -> Id {
    let id = Id::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind(ws)
        .bind(name)
        .bind(slug)
        .execute(pool)
        .await
        .expect("team");
    id
}

async fn place(pool: &PgPool, team: Id, user: Id, role: &str) {
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, $3)")
        .bind(team)
        .bind(user)
        .bind(role)
        .execute(pool)
        .await
        .expect("team row");
}

async fn project(store: &Store, ws: Id, team: Id, name: &str, slug: &str, key: &str) -> Id {
    let id = Id::now_v7();
    store
        .insert_project(id, ws, team, name, slug, key)
        .await
        .expect("project");
    id
}

async fn labs() -> Labs {
    let c = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let host = c.get_host().await.expect("container host");
    let port = c.get_host_port_ipv4(5432).await.expect("container port");
    let opts = PgConnectOptions::from_str(&format!(
        "postgres://postgres:postgres@{host}:{port}/postgres"
    ))
    .expect("parse url")
    .disable_statement_logging();
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(20))
        .connect_with(opts)
        .await
        .expect("connect pool");
    foundry_store::run_migrations(&pool)
        .await
        .expect("run migrations");
    let store = Arc::new(Store::from_pool(pool.clone()));
    let ws = Id::now_v7();
    q(
        &pool,
        "INSERT INTO workspaces (id, name) VALUES ($1, 'Canzan Labs')",
        &[ws],
    )
    .await;
    let priya = user(&pool, ws, "priya.raman@canzan.dev", "admin").await;
    let sam = user(&pool, ws, "sam.okafor@canzan.dev", "member").await;
    let backend = team(&pool, ws, "Backend", "backend").await;
    let ops = team(&pool, ws, "Ops", "ops").await;
    place(&pool, backend, sam, "member").await;
    place(&pool, ops, sam, "member").await;
    let auth = project(&store, ws, backend, "Identity Platform", "auth", "AUTH").await;
    let spike = project(
        &store,
        ws,
        backend,
        "Auth v2 Spike",
        "auth-v2-spike",
        "SPIKE",
    )
    .await;
    let homelab = project(&store, ws, ops, "Homelab Ops", "homelab", "OPS").await;
    Labs {
        services: Services::new(store.clone()),
        store,
        pool,
        ws,
        priya,
        sam,
        ops,
        auth,
        spike,
        homelab,
        _c: c,
    }
}

impl Labs {
    fn as_user(&self, user_id: Id) -> Principal {
        Principal::Human {
            user_id,
            workspace_id: self.ws,
        }
    }

    /// The workspace's `kind` connection, saved for the row `id`.
    async fn connect(&self, id: Id, kind: &str, external_id: &str, label: &str) {
        self.store
            .upsert_connection(&NewConnection {
                id,
                workspace_id: self.ws,
                kind: kind.into(),
                external_id: external_id.into(),
                app_id: None,
                bot_user_id: Some("U0FOUNDRYBOT".into()),
                label: label.into(),
                detail: None,
                sealed_credential: vec![0x01, 0x02, 0x03],
                connected_by: self.priya,
                connected_at: OffsetDateTime::now_utc(),
            })
            .await
            .expect("save the connection");
    }

    /// A Slack route of Identity Platform to `channel`, saved by Priya.
    async fn route(&self, channel: &str, label: &str) -> Id {
        self.services
            .create_route(
                &self.as_user(self.priya),
                "backend",
                "auth",
                draft(channel, label, &["issue.created"], "summary"),
                OffsetDateTime::now_utc(),
            )
            .await
            .expect("the route is saved")
    }

    /// One event of Identity Platform, dispatched to `route` only; returns
    /// the delivery's id.
    async fn delivery_to(&self, route: Id) -> i64 {
        sqlx::query("INSERT INTO outbox (event_type, payload) VALUES ('IssueCreated', $1::jsonb)")
            .bind(format!(
                r#"{{"project_id": "{}", "key": "AUTH-1"}}"#,
                self.auth
            ))
            .execute(&self.pool)
            .await
            .expect("an event");
        let (ws, auth) = (self.ws, self.auth);
        self.store
            .dispatch_tick(
                100,
                OffsetDateTime::now_utc(),
                Duration::from_secs(24 * 3600),
                move |batch: &DispatchBatch| {
                    batch
                        .events
                        .iter()
                        .map(|event| NewDelivery {
                            workspace_id: ws,
                            project_id: auth,
                            target: DeliveryTarget::Route(route),
                            event_kind: "issue.created".to_string(),
                            event_id: event.event_id,
                            outbox_ids: vec![event.id],
                            issue_id: None,
                            issue_key: None,
                            occurred_at: event.created_at,
                        })
                        .collect()
                },
            )
            .await
            .expect("dispatch");
        sqlx::query_scalar("SELECT max(id) FROM integration_deliveries WHERE route_id = $1")
            .bind(route)
            .fetch_one(&self.pool)
            .await
            .expect("the delivery")
    }

    /// Sam's saved choices as `(project, event, scope)`, as stored.
    async fn stored_choices(&self) -> Vec<(Id, String, String)> {
        sqlx::query_as(
            "SELECT project_id, event, scope FROM integration_email_choices
              WHERE user_id = $1 ORDER BY project_id, event",
        )
        .bind(self.sam)
        .fetch_all(&self.pool)
        .await
        .expect("choices")
    }
}

fn draft(channel: &str, label: &str, events: &[&str], content_level: &str) -> RouteDraft {
    RouteDraft {
        kind: "slack".into(),
        destination_ref: channel.into(),
        destination_label: label.into(),
        events: events.iter().map(|e| e.to_string()).collect(),
        content_level: content_level.into(),
        lanes: vec![],
        shared_confirmed: false,
    }
}

fn choice(project_id: Id, event: EventKind, scope: EmailScope) -> EmailChoiceInput {
    EmailChoiceInput {
        project_id,
        event,
        scope,
    }
}

/// `(project, event wire name, scope wire name)`, sorted.
fn shown(choices: &foundry_services::integrations::ProjectEmails) -> Vec<(Id, String, String)> {
    let mut rows: Vec<(Id, String, String)> = choices
        .choices
        .iter()
        .map(|c| (c.project_id, c.event.clone(), c.scope.clone()))
        .collect();
    rows.sort();
    rows
}

fn row(project: Id, event: EventKind, scope: EmailScope) -> (Id, String, String) {
    (
        project,
        event.wire_name().to_string(),
        scope.wire_name().to_string(),
    )
}

// SECURITY (US-EIP-01, SC-6): the integrations admin check lets a workspace
// admin through and refuses a member with the uniform not-found.
#[tokio::test]
async fn only_a_workspace_admin_passes_the_integrations_admin_check() {
    let labs = labs().await;
    assert!(
        matches!(
            labs.services
                .require_workspace_admin(&labs.as_user(labs.sam))
                .await,
            Err(IntegrationError::NotFound)
        ),
        "Sam is a member, not an admin"
    );
    labs.services
        .require_workspace_admin(&labs.as_user(labs.priya))
        .await
        .expect("Priya is an admin");
}

// US-EIP-06: a reconnect seals the new credential for the workspace's existing
// connection row; a first connection gets a fresh row id of its own.
#[tokio::test]
async fn a_reconnect_seals_for_the_existing_connection_row() {
    let labs = labs().await;
    let priya = labs.as_user(labs.priya);
    for kind in ["slack", "matrix"] {
        let fresh = match kind {
            "slack" => labs.services.slack_connection_id(&priya).await,
            _ => labs.services.matrix_connection_id(&priya).await,
        }
        .expect("a fresh row id");
        assert_ne!(fresh, Id::nil(), "{kind}: a first connection has a real id");
        labs.connect(fresh, kind, &format!("{kind}-external"), "Canzan Labs")
            .await;
        let again = match kind {
            "slack" => labs.services.slack_connection_id(&priya).await,
            _ => labs.services.matrix_connection_id(&priya).await,
        }
        .expect("the existing row id");
        assert_eq!(again, fresh, "{kind}: a reconnect reuses the row");
    }
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM integration_connections")
        .fetch_one(&labs.pool)
        .await
        .expect("count");
    assert_eq!(rows, 2, "one row per kind");
}

// US-EIP-10: a member's choices are kept one per event and project; ticking
// both scopes of one event keeps "all issues", ticking one scope twice keeps
// it, and a project's choices never land in another project.
#[tokio::test]
async fn project_email_choices_are_one_per_event_and_stay_in_their_project() {
    let labs = labs().await;
    let saved = labs
        .services
        .save_project_emails(
            labs.sam,
            &[
                choice(labs.auth, EventKind::IssueCreated, EmailScope::Mine),
                choice(labs.auth, EventKind::IssueCreated, EmailScope::All),
                choice(labs.auth, EventKind::IssueMoved, EmailScope::Mine),
                choice(labs.auth, EventKind::IssueMoved, EmailScope::Mine),
                choice(labs.spike, EventKind::CommentAdded, EmailScope::All),
            ],
        )
        .await
        .expect("save");
    assert_eq!(saved.len(), 3, "one saved choice per project and event");
    let mut expected = vec![
        row(labs.auth, EventKind::IssueCreated, EmailScope::All),
        row(labs.auth, EventKind::IssueMoved, EmailScope::Mine),
        row(labs.spike, EventKind::CommentAdded, EmailScope::All),
    ];
    expected.sort();
    assert_eq!(labs.stored_choices().await, expected);
    let section = labs
        .services
        .project_emails(labs.sam)
        .await
        .expect("section");
    assert_eq!(shown(&section), expected, "the section reads them back");
}

// US-EIP-10: a project the member can no longer open drops out of their
// section, with its choices; the others stay.
#[tokio::test]
async fn a_project_the_member_can_no_longer_open_leaves_their_section() {
    let labs = labs().await;
    labs.services
        .save_project_emails(
            labs.sam,
            &[
                choice(labs.auth, EventKind::IssueCreated, EmailScope::All),
                choice(labs.homelab, EventKind::IssueDeleted, EmailScope::Mine),
            ],
        )
        .await
        .expect("save");
    q(
        &labs.pool,
        "DELETE FROM team_memberships WHERE team_id = $1 AND user_id = $2",
        &[labs.ops, labs.sam],
    )
    .await;
    let section = labs
        .services
        .project_emails(labs.sam)
        .await
        .expect("section");
    assert_eq!(
        shown(&section),
        vec![row(labs.auth, EventKind::IssueCreated, EmailScope::All)],
        "Homelab Ops' choice is not shown once Sam left Ops"
    );
    assert!(
        section
            .projects
            .iter()
            .all(|p| p.project_id != labs.homelab),
        "nor is Homelab Ops offered"
    );
}

// US-EIP-11: a stop link names its live project and email event; stopping
// removes exactly that one choice, and stopping twice stops once.
#[tokio::test]
async fn a_stop_link_stops_exactly_its_one_choice() {
    let labs = labs().await;
    labs.services
        .save_project_emails(
            labs.sam,
            &[
                choice(labs.auth, EventKind::IssueCreated, EmailScope::All),
                choice(labs.auth, EventKind::CommentAdded, EmailScope::Mine),
                choice(labs.spike, EventKind::IssueCreated, EmailScope::All),
            ],
        )
        .await
        .expect("save");
    let created = EventKind::IssueCreated.wire_name();
    let named = EmailStop {
        project_name: "Identity Platform".into(),
        event: EventKind::IssueCreated,
    };
    assert_eq!(
        labs.services
            .email_stop_target(labs.auth, created)
            .await
            .expect("the confirm page's facts"),
        named
    );
    assert_eq!(
        labs.stored_choices().await.len(),
        3,
        "the confirm page changes nothing"
    );
    for _ in 0..2 {
        assert_eq!(
            labs.services
                .stop_project_email(labs.sam, labs.auth, created)
                .await
                .expect("stop"),
            named
        );
    }
    let mut left = vec![
        row(labs.auth, EventKind::CommentAdded, EmailScope::Mine),
        row(labs.spike, EventKind::IssueCreated, EmailScope::All),
    ];
    left.sort();
    assert_eq!(labs.stored_choices().await, left, "only that choice went");
}

// US-EIP-11: a stop link naming an event no member can choose by email, or a
// project since deleted, is the uniform refusal and changes nothing.
#[tokio::test]
async fn a_stop_link_for_a_non_email_event_or_a_deleted_project_changes_nothing() {
    let labs = labs().await;
    labs.services
        .save_project_emails(
            labs.sam,
            &[
                choice(labs.auth, EventKind::IssueCreated, EmailScope::All),
                choice(labs.spike, EventKind::IssueCreated, EmailScope::All),
            ],
        )
        .await
        .expect("save");
    let before = labs.stored_choices().await;

    let edited = EventKind::CommentEdited.wire_name();
    assert!(
        matches!(
            labs.services.email_stop_target(labs.auth, edited).await,
            Err(IntegrationError::NotFound)
        ),
        "a comment's edit is never emailed, so no link names it"
    );
    assert!(matches!(
        labs.services
            .stop_project_email(labs.sam, labs.auth, edited)
            .await,
        Err(IntegrationError::NotFound)
    ));

    let backend: Id = sqlx::query_scalar("SELECT team_id FROM projects WHERE id = $1")
        .bind(labs.spike)
        .fetch_one(&labs.pool)
        .await
        .expect("team");
    labs.store
        .soft_delete_project(labs.ws, backend, labs.spike, labs.priya)
        .await
        .expect("delete Auth v2 Spike");
    let created = EventKind::IssueCreated.wire_name();
    assert!(
        matches!(
            labs.services.email_stop_target(labs.spike, created).await,
            Err(IntegrationError::NotFound)
        ),
        "a deleted project's link is refused"
    );
    assert!(matches!(
        labs.services
            .stop_project_email(labs.sam, labs.spike, created)
            .await,
        Err(IntegrationError::NotFound)
    ));
    assert_eq!(labs.stored_choices().await, before, "nothing changed");
}

// US-EIP-02: a route needs at least one chat event and a known content level;
// anything else is refused before a row is written.
#[tokio::test]
async fn a_route_without_events_or_with_an_unknown_content_level_is_not_saved() {
    let labs = labs().await;
    labs.connect(Id::now_v7(), "slack", "T0CANZAN", "Canzan Labs")
        .await;
    let priya = labs.as_user(labs.priya);
    for (events, level) in [(&[][..], "summary"), (&["issue.created"][..], "everything")] {
        let refused = labs
            .services
            .create_route(
                &priya,
                "backend",
                "auth",
                draft("C0AUTHDEV", "#auth-dev", events, level),
                OffsetDateTime::now_utc(),
            )
            .await;
        assert!(
            matches!(refused, Err(IntegrationError::NotFound)),
            "{events:?} at {level}: {refused:?}"
        );
    }
    let routes: i64 = sqlx::query_scalar("SELECT count(*) FROM integration_routes")
        .fetch_one(&labs.pool)
        .await
        .expect("count");
    assert_eq!(routes, 0, "nothing was saved");
    labs.route("C0AUTHDEV", "#auth-dev").await;
}

// US-EIP-05: a route's history is its own deliveries, under its own channel,
// even when the project has another route.
#[tokio::test]
async fn a_routes_history_is_its_own() {
    let labs = labs().await;
    labs.connect(Id::now_v7(), "slack", "T0CANZAN", "Canzan Labs")
        .await;
    let general = labs.route("C0GENERAL", "#general").await;
    let auth_dev = labs.route("C0AUTHDEV", "#auth-dev").await;
    labs.delivery_to(auth_dev).await;
    let history = labs
        .services
        .route_history(&labs.as_user(labs.priya), "backend", "auth", auth_dev)
        .await
        .expect("history");
    assert_eq!(history.destination_label, "#auth-dev");
    assert_eq!(history.deliveries.len(), 1, "the one delivery to #auth-dev");
    let quiet = labs
        .services
        .route_history(&labs.as_user(labs.priya), "backend", "auth", general)
        .await
        .expect("history");
    assert_eq!(quiet.destination_label, "#general");
    assert!(quiet.deliveries.is_empty(), "#general was sent nothing");
}

// US-EIP-07: disconnecting skips every delivery its routes were waiting to
// send, across its routes.
#[tokio::test]
async fn a_disconnect_skips_everything_its_routes_were_waiting_to_send() {
    let labs = labs().await;
    labs.connect(Id::now_v7(), "slack", "T0CANZAN", "Canzan Labs")
        .await;
    let general = labs.route("C0GENERAL", "#general").await;
    let auth_dev = labs.route("C0AUTHDEV", "#auth-dev").await;
    let waiting = [
        labs.delivery_to(general).await,
        labs.delivery_to(auth_dev).await,
        labs.delivery_to(auth_dev).await,
    ];
    let outcome = labs
        .services
        .disconnect(
            &labs.as_user(labs.priya),
            "slack",
            OffsetDateTime::now_utc(),
        )
        .await
        .expect("disconnect");
    assert!(
        matches!(
            outcome,
            DisconnectOutcome::Disconnected { cancelled: 3, .. }
        ),
        "{outcome:?}"
    );
    let statuses: Vec<String> = sqlx::query_scalar(
        "SELECT status FROM integration_deliveries WHERE id = ANY($1) ORDER BY id",
    )
    .bind(&waiting[..])
    .fetch_all(&labs.pool)
    .await
    .expect("statuses");
    assert_eq!(statuses, vec!["skipped"; 3]);
}

// R-4: the workspace integrations page lists what was done, newest first.
#[tokio::test]
async fn the_workspace_page_lists_its_integration_activity() {
    let labs = labs().await;
    labs.connect(Id::now_v7(), "slack", "T0CANZAN", "Canzan Labs")
        .await;
    labs.route("C0AUTHDEV", "#auth-dev").await;
    let page = labs
        .services
        .workspace_integrations(&labs.as_user(labs.priya))
        .await
        .expect("page");
    assert_eq!(page.activity.len(), 2, "{:?}", page.activity);
    assert!(
        page.activity[0].ends_with("created the route to #auth-dev"),
        "{:?}",
        page.activity
    );
    assert!(
        page.activity[1].ends_with("connected Slack (Canzan Labs)"),
        "{:?}",
        page.activity
    );
}
