//! external-integration-plugins — the integration use-cases (DESIGN [REF]
//! Component Decomposition slice 01; ADR-EIP-005 decision 4; DDR-21).
//!
//! - [`workspace_view`]: the workspace integrations page (admins only): the
//!   Slack connection line (K-1) and the activity list (R-4).
//! - [`connect_slack`]: a completed Slack install becomes one connection row
//!   and one audit row. It receives the credential already sealed: no
//!   plaintext crosses a use-case (ADR-EIP-004).
//! - [`project_view`]: a project's integrations page (Manage only).
//! - [`route_target`]: what the route form, Send test and the route save
//!   act on (Manage only): the project and the workspace's Slack connection,
//!   its credential still sealed (the adapters open it, ADR-EIP-004).
//! - [`create_route`]: a route the destination check accepted becomes one
//!   route row, its lane rows and one audit row (DDR-25: saving never sends).
//! - [`route_history`]: a route's deliveries, newest first (Manage only).
//!
//! Every refusal is [`IntegrationError::NotFound`], which each door answers
//! with its uniform 404. The use-cases read and write through
//! [`IntegrationRecords`], implemented over the store in `store_impls.rs`
//! (check-arch `integrations-store-capability`).

mod store_impls;

use crate::Principal;
use foundry_auth::secret_box::SealedSecret;
use foundry_core::TeamAccess;
use foundry_store::integrations::{
    AuditRow, ConnectionRow, ConnectionWriteOutcome, DeliveryHistoryRow, NewConnection, NewRoute,
    ProjectRoute, RouteLane, RouteWriteOutcome, SealedConnection,
};
use foundry_store::lanes::LaneRow;
use foundry_store::teams::TeamAccessFacts;
use foundry_store::{ProjectRow, StoreError};
use time::OffsetDateTime;
use uuid::Uuid;

/// The connection kind this module connects today.
pub const SLACK: &str = "slack";

/// How many activity lines the workspace page lists.
pub const ACTIVITY_LIMIT: i64 = 50;

/// How many deliveries a route's history lists, newest first. Above the
/// retained minimum of 100 (NFR-EIP-7), so the 30-day tail stays visible.
pub const HISTORY_LIMIT: i64 = 500;

/// The persistence the integration use-cases need, and nothing more.
pub(crate) trait IntegrationRecords: Sync {
    async fn is_workspace_admin(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, StoreError>;
    async fn connections(&self, workspace_id: Uuid) -> Result<Vec<ConnectionRow>, StoreError>;
    async fn activity(&self, workspace_id: Uuid, limit: i64) -> Result<Vec<AuditRow>, StoreError>;
    async fn connection_id(
        &self,
        workspace_id: Uuid,
        kind: &str,
    ) -> Result<Option<Uuid>, StoreError>;
    async fn upsert_connection(
        &self,
        connection: &NewConnection,
    ) -> Result<ConnectionWriteOutcome, StoreError>;
    async fn team_access(
        &self,
        workspace_id: Uuid,
        team_slug: &str,
        user_id: Uuid,
    ) -> Result<Option<TeamAccessFacts>, StoreError>;
    async fn live_project(
        &self,
        team_id: Uuid,
        project_slug: &str,
    ) -> Result<Option<ProjectRow>, StoreError>;
    async fn project_routes(&self, project_id: Uuid) -> Result<Vec<ProjectRoute>, StoreError>;
    async fn sealed_connection(
        &self,
        workspace_id: Uuid,
        kind: &str,
    ) -> Result<Option<SealedConnection>, StoreError>;
    async fn project_lanes(&self, project_id: Uuid) -> Result<Vec<LaneRow>, StoreError>;
    async fn create_route(&self, route: &NewRoute) -> Result<RouteWriteOutcome, StoreError>;
    async fn route_history(
        &self,
        route_id: Uuid,
        limit: i64,
    ) -> Result<Vec<DeliveryHistoryRow>, StoreError>;
}

/// The events a chat route may choose (D-2).
pub const CHAT_EVENTS: [&str; 4] = [
    "issue.created",
    "issue.moved",
    "comment.added",
    "issue.deleted",
];

/// The content levels (D-3).
pub const CONTENT_LEVELS: [&str; 3] = ["link_only", "summary", "summary_with_comment"];

#[derive(Debug, thiserror::Error)]
pub enum IntegrationError {
    /// No right, or nothing there: the uniform 404.
    #[error("not found")]
    NotFound,
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// The workspace integrations page's facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceIntegrations {
    /// K-1 for the workspace's Slack connection, when it has one.
    pub slack: Option<String>,
    /// R-4 activity lines, newest first.
    pub activity: Vec<String>,
}

/// What Slack's install answered (ADR-EIP-005 decision 4). Never the token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlackInstall {
    pub team_id: String,
    pub team_name: String,
    pub domain: String,
    pub app_id: String,
    pub bot_user_id: String,
}

/// A project's integrations page facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectIntegrations {
    pub project_name: String,
    pub routes: Vec<ProjectRoute>,
}

/// What a route door acts on: a project the caller manages and the
/// workspace's Slack connection, if it has one that is not disconnected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteTarget {
    pub workspace_id: Uuid,
    pub project_id: Uuid,
    pub project_name: String,
    pub key_prefix: String,
    pub connection: Option<SealedConnection>,
    /// The project's lanes, board order: what Issue moved can be limited to.
    pub lanes: Vec<TargetLane>,
}

/// One lane a route's Issue moved can be limited to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLane {
    pub id: Uuid,
    pub slug: String,
    pub label: String,
}

/// A route's history page facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteHistory {
    pub project_name: String,
    /// The route's kind (`slack`, `matrix`, …).
    pub kind: String,
    pub destination_label: String,
    /// Newest first.
    pub deliveries: Vec<DeliveryHistoryRow>,
}

/// A route as its form submitted it, its destination already checked and
/// labelled by the destination catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDraft {
    pub destination_ref: String,
    pub destination_label: String,
    pub events: Vec<String>,
    pub content_level: String,
    /// Lane slugs Issue moved is limited to (empty: any lane).
    pub lanes: Vec<String>,
    pub shared_confirmed: bool,
}

/// Admins only, else [`IntegrationError::NotFound`].
pub(crate) async fn require_admin(
    records: &impl IntegrationRecords,
    principal: &Principal,
) -> Result<(), IntegrationError> {
    if records
        .is_workspace_admin(principal.workspace_id(), principal.user_id())
        .await?
    {
        Ok(())
    } else {
        Err(IntegrationError::NotFound)
    }
}

pub(crate) async fn workspace_view(
    records: &impl IntegrationRecords,
    principal: &Principal,
) -> Result<WorkspaceIntegrations, IntegrationError> {
    require_admin(records, principal).await?;
    let workspace_id = principal.workspace_id();
    let slack = records
        .connections(workspace_id)
        .await?
        .iter()
        .find(|c| c.kind == SLACK && c.state != "disconnected")
        .map(connected_line);
    let activity = records
        .activity(workspace_id, ACTIVITY_LIMIT)
        .await?
        .iter()
        .map(activity_line)
        .collect();
    Ok(WorkspaceIntegrations { slack, activity })
}

/// The row a new Slack credential is sealed for: the workspace's existing
/// Slack connection, or a fresh id.
pub(crate) async fn slack_connection_id(
    records: &impl IntegrationRecords,
    principal: &Principal,
) -> Result<Uuid, IntegrationError> {
    require_admin(records, principal).await?;
    Ok(records
        .connection_id(principal.workspace_id(), SLACK)
        .await?
        .unwrap_or_else(Uuid::now_v7))
}

/// Save the workspace's Slack connection and its audit record.
pub(crate) async fn connect_slack(
    records: &impl IntegrationRecords,
    principal: &Principal,
    install: SlackInstall,
    connection_id: Uuid,
    sealed: SealedSecret,
    now: OffsetDateTime,
) -> Result<ConnectionWriteOutcome, IntegrationError> {
    require_admin(records, principal).await?;
    let connection = NewConnection {
        id: connection_id,
        workspace_id: principal.workspace_id(),
        kind: SLACK.to_string(),
        external_id: install.team_id,
        app_id: Some(install.app_id),
        bot_user_id: Some(install.bot_user_id),
        label: install.team_name,
        detail: Some(install.domain),
        sealed_credential: sealed.as_bytes().to_vec(),
        connected_by: principal.user_id(),
        connected_at: now,
    };
    Ok(records.upsert_connection(&connection).await?)
}

/// The live project `team_slug`/`project_slug` when the caller may Manage it
/// (a workspace admin or a lead of the project's team), else not found.
async fn managed_project(
    records: &impl IntegrationRecords,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
) -> Result<ProjectRow, IntegrationError> {
    let facts = records
        .team_access(principal.workspace_id(), team_slug, principal.user_id())
        .await?
        .ok_or(IntegrationError::NotFound)?;
    if !TeamAccess::of(facts.is_workspace_admin, facts.role).can_manage() {
        return Err(IntegrationError::NotFound);
    }
    records
        .live_project(facts.team_id, project_slug)
        .await?
        .ok_or(IntegrationError::NotFound)
}

/// Manage only.
pub(crate) async fn project_view(
    records: &impl IntegrationRecords,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
) -> Result<ProjectIntegrations, IntegrationError> {
    let project = managed_project(records, principal, team_slug, project_slug).await?;
    let routes = records.project_routes(project.id).await?;
    Ok(ProjectIntegrations {
        project_name: project.name,
        routes,
    })
}

/// Manage only: the route `route_id` of the project and its deliveries,
/// newest first; a route of another project is not found.
pub(crate) async fn route_history(
    records: &impl IntegrationRecords,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    route_id: Uuid,
) -> Result<RouteHistory, IntegrationError> {
    let project = managed_project(records, principal, team_slug, project_slug).await?;
    let route = records
        .project_routes(project.id)
        .await?
        .into_iter()
        .find(|route| route.id == route_id)
        .ok_or(IntegrationError::NotFound)?;
    let deliveries = records.route_history(route.id, HISTORY_LIMIT).await?;
    Ok(RouteHistory {
        project_name: project.name,
        kind: route.kind,
        destination_label: route.destination_label,
        deliveries,
    })
}

/// Manage only: the project and its workspace's usable Slack connection.
pub(crate) async fn route_target(
    records: &impl IntegrationRecords,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
) -> Result<RouteTarget, IntegrationError> {
    let project = managed_project(records, principal, team_slug, project_slug).await?;
    let connection = records
        .sealed_connection(principal.workspace_id(), SLACK)
        .await?
        .filter(|c| c.state != "disconnected");
    let lanes = records
        .project_lanes(project.id)
        .await?
        .into_iter()
        .map(|lane| TargetLane {
            id: lane.id,
            slug: lane.slug,
            label: lane.label,
        })
        .collect();
    Ok(RouteTarget {
        workspace_id: principal.workspace_id(),
        project_id: project.id,
        project_name: project.name,
        key_prefix: project.key_prefix,
        connection,
        lanes,
    })
}

/// Save a Slack route (Manage only): its events within the chat events and at
/// least one, a known content level, lanes of the project; else not found
/// and nothing written.
pub(crate) async fn create_route(
    records: &impl IntegrationRecords,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    draft: RouteDraft,
    now: OffsetDateTime,
) -> Result<Uuid, IntegrationError> {
    let target = route_target(records, principal, team_slug, project_slug).await?;
    let connection = target.connection.ok_or(IntegrationError::NotFound)?;
    let events_ok = !draft.events.is_empty()
        && draft
            .events
            .iter()
            .all(|event| CHAT_EVENTS.contains(&event.as_str()));
    if !events_ok || !CONTENT_LEVELS.contains(&draft.content_level.as_str()) {
        return Err(IntegrationError::NotFound);
    }
    let lanes = draft
        .lanes
        .iter()
        .map(|slug| {
            target
                .lanes
                .iter()
                .find(|lane| &lane.slug == slug)
                .map(|lane| RouteLane {
                    lane_id: lane.id,
                    label_at_save: lane.label.clone(),
                })
                .ok_or(IntegrationError::NotFound)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let route = NewRoute {
        workspace_id: target.workspace_id,
        project_id: target.project_id,
        connection_id: Some(connection.id),
        kind: SLACK.to_string(),
        destination_ref: draft.destination_ref,
        destination_label: draft.destination_label,
        events: draft.events,
        content_level: draft.content_level,
        shared_confirmed: draft.shared_confirmed,
        lanes,
        created_by: principal.user_id(),
        created_via: "web".to_string(),
        created_at: now,
    };
    match records.create_route(&route).await? {
        RouteWriteOutcome::Created(id) => Ok(id),
        RouteWriteOutcome::ProjectDeleted => Err(IntegrationError::NotFound),
    }
}

/// K-1: "Connected to {slack_team} ({slack_domain}) · by {admin_name} · {date}".
fn connected_line(connection: &ConnectionRow) -> String {
    let detail = connection
        .detail
        .as_deref()
        .map(|d| format!(" ({d})"))
        .unwrap_or_default();
    let by = connection
        .connected_by_name
        .as_deref()
        .map(|name| format!(" · by {name}"))
        .unwrap_or_default();
    format!(
        "Connected to {}{detail}{by} · {}",
        connection.label,
        display_date(connection.connected_at)
    )
}

/// R-4: one pattern for every activity line.
fn activity_line(row: &AuditRow) -> String {
    let date = display_date(row.created_at);
    let actor = row.actor_name.as_deref().unwrap_or("Foundry");
    let kind = kind_name(&row.kind);
    let subject = &row.subject;
    let what = match row.action.as_str() {
        "connection_connected" | "connection_reconnected" => {
            format!("connected {kind} ({subject})")
        }
        "connection_disconnected" => format!("disconnected {kind} ({subject})"),
        "connection_lost" => format!("lost {kind} ({subject})"),
        "route_created" => format!("created the route to {subject}"),
        "route_edited" => format!("edited the route to {subject}"),
        "route_paused" => format!("paused the route to {subject}"),
        "route_resumed" => format!("resumed the route to {subject}"),
        "route_deleted" => format!("deleted the route to {subject}"),
        other => format!("{} ({subject})", other.replace('_', " ")),
    };
    format!("{date} · {actor} {what}")
}

fn kind_name(kind: &str) -> &str {
    match kind {
        "slack" => "Slack",
        "matrix" => "Matrix",
        "webhook" => "Webhook",
        "email" => "Email",
        other => other,
    }
}

/// "21 Oct 2026".
fn display_date(at: OffsetDateTime) -> String {
    let month = at.month().to_string();
    format!("{} {} {}", at.day(), &month[..3], at.year())
}

#[cfg(test)]
mod copy {
    //! bypass: exact-string contracts (K-1, R-4), example-based.
    use super::*;
    use time::macros::datetime;

    fn row(action: &str, kind: &str, subject: &str) -> AuditRow {
        AuditRow {
            created_at: datetime!(2026-10-21 14:02 UTC),
            actor_name: Some("Priya Raman".into()),
            action: action.into(),
            kind: kind.into(),
            subject: subject.into(),
        }
    }

    #[test]
    fn the_connection_line_is_k1() {
        let connection = ConnectionRow {
            kind: SLACK.into(),
            state: "connected".into(),
            label: "Canzan Labs".into(),
            detail: Some("canzanlabs.slack.com".into()),
            connected_by_name: Some("Priya Raman".into()),
            connected_at: datetime!(2026-10-21 14:02 UTC),
        };
        assert_eq!(
            connected_line(&connection),
            "Connected to Canzan Labs (canzanlabs.slack.com) · by Priya Raman · 21 Oct 2026"
        );
    }

    #[test]
    fn activity_lines_follow_r4() {
        assert_eq!(
            activity_line(&row("connection_connected", "slack", "Canzan Labs")),
            "21 Oct 2026 · Priya Raman connected Slack (Canzan Labs)"
        );
        assert_eq!(
            activity_line(&row("connection_disconnected", "slack", "Canzan Labs")),
            "21 Oct 2026 · Priya Raman disconnected Slack (Canzan Labs)"
        );
        assert_eq!(
            activity_line(&row("route_created", "slack", "#auth-dev")),
            "21 Oct 2026 · Priya Raman created the route to #auth-dev"
        );
    }

    #[test]
    fn single_digit_days_are_not_padded() {
        assert_eq!(display_date(datetime!(2026-09-05 08:00 UTC)), "5 Sep 2026");
    }
}
