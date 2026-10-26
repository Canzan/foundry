//! external-integration-plugins — the integration use-cases (DESIGN [REF]
//! Component Decomposition slice 01; ADR-EIP-005 decision 4; DDR-21).
//!
//! - [`workspace_view`]: the workspace integrations page (admins only): the
//!   Slack connection line (K-1) and the activity list (R-4).
//! - [`connect_slack`]: a completed Slack install becomes one connection row
//!   and one audit row. It receives the credential already sealed: no
//!   plaintext crosses a use-case (ADR-EIP-004).
//! - [`connect_matrix`]: a bot token the homeserver accepted becomes the
//!   workspace's Matrix connection row and one audit row (ADR-EIP-006
//!   decision 1), its token already sealed.
//! - [`project_view`]: a project's integrations page (Manage only).
//! - [`route_target`]: what the route form, Send test and the route save
//!   act on (Manage only): the project and the workspace's Slack connection,
//!   its credential still sealed (the adapters open it, ADR-EIP-004).
//! - [`create_route`]: a route the destination check accepted becomes one
//!   route row, its lane rows and one audit row (DDR-25: saving never sends).
//! - [`route_history`]: a route's deliveries, newest first (Manage only).
//! - [`managed_route`]: what the edit form and the delete dialog show of a
//!   route (Manage only).
//! - [`change_route`]: pause, resume, edit or delete a route, one change and
//!   its audit record (Manage only). A pause skips the route's waiting
//!   deliveries the pure `cancel_set` names (DD-8); asking for what already
//!   is so writes nothing. Resuming a route a vendor refusal stopped first
//!   asks the vendor about its destination again ([`DestinationCatalogs`]);
//!   while the problem lasts the route stays stopped (D-8).
//!
//! - [`room_still_routed`]: whether another route of the workspace still
//!   sends to a destination (a deleted Matrix route's bot leaves a room no
//!   route uses).
//! - [`disconnect_preview`]: what disconnecting Slack or Matrix stops (admins).
//! - [`disconnect`]: the workspace's Slack or Matrix connection disconnected,
//!   its credential forgotten and its routes' waiting deliveries skipped (the
//!   pure `cancel_set` per route), with its audit record (admins; R-9: a
//!   deliberate disconnect queues nothing). The credential it held comes
//!   back still sealed, for the Slack adapter to withdraw at Slack.
//!
//! - [`project_emails`]: a member's Project emails section: the projects
//!   they can open and their choices there (US-EIP-10; nothing is chosen
//!   for them, OD-10).
//! - [`save_project_emails`]: a member's choices replace theirs in every
//!   project they can open, each project in one transaction that takes the
//!   live-project lock; a choice naming any other project refuses the whole
//!   save, nothing written (ADR-EIP-007 decision 1).
//!
//! Every refusal is [`IntegrationError::NotFound`], which each door answers
//! with its uniform 404. The use-cases read and write through
//! [`IntegrationRecords`], implemented over the store in `store_impls.rs`
//! (check-arch `integrations-store-capability`).

mod store_impls;

use crate::Principal;
use foundry_auth::secret_box::SealedSecret;
use foundry_core::TeamAccess;
use foundry_integrations::{
    cancel_set, DestinationCatalog, DestinationRef, ReasonCode, TargetedDelivery,
};
use foundry_integrations::{email_events, EmailScope, EventKind};
use foundry_store::integrations::{
    AuditRow, CancelCandidate, ConnectionChange, ConnectionRow, ConnectionWriteOutcome,
    DeliveryHistoryRow, DeliveryIssue, DisconnectOutcome, DisconnectPreview, EmailChoiceRow,
    EmailProject, NewConnection, NewRoute, ProjectRoute, RouteChange, RouteChangeOutcome,
    RouteEdit, RouteLane, RouteSettings, RouteWriteOutcome, SealedConnection,
};
use foundry_store::lanes::LaneRow;
use foundry_store::teams::TeamAccessFacts;
use foundry_store::{ProjectRow, StoreError};
use time::OffsetDateTime;
use uuid::Uuid;

/// The connection kinds this module connects.
pub const SLACK: &str = "slack";
pub const MATRIX: &str = "matrix";

/// How many activity lines the workspace page lists.
pub const ACTIVITY_LIMIT: i64 = 50;

/// How many deliveries a route's history lists, newest first. Above the
/// retained minimum of 100 (NFR-EIP-7), so the 30-day tail stays visible.
pub const HISTORY_LIMIT: i64 = 500;

/// Opens the read-only destination catalogue of a connection, its credential
/// opened inside the adapter (ADR-EIP-004); `None` when this Foundry cannot.
pub trait DestinationCatalogs: Sync {
    fn catalogue(&self, connection: &SealedConnection) -> Option<Box<dyn DestinationCatalog>>;
}

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
    async fn latest_issue(&self, project_id: Uuid) -> Result<Option<DeliveryIssue>, StoreError>;
    async fn create_route(&self, route: &NewRoute) -> Result<RouteWriteOutcome, StoreError>;
    async fn route_history(
        &self,
        route_id: Uuid,
        limit: i64,
    ) -> Result<Vec<DeliveryHistoryRow>, StoreError>;
    async fn route_settings(
        &self,
        project_id: Uuid,
        route_id: Uuid,
    ) -> Result<Option<RouteSettings>, StoreError>;
    async fn pause_route(
        &self,
        change: &RouteChange,
        plan: impl FnOnce(&[CancelCandidate]) -> Vec<i64> + Send,
    ) -> Result<RouteChangeOutcome, StoreError>;
    async fn resume_route(&self, change: &RouteChange) -> Result<RouteChangeOutcome, StoreError>;
    async fn edit_route(
        &self,
        change: &RouteChange,
        edit: &RouteEdit,
    ) -> Result<RouteChangeOutcome, StoreError>;
    async fn delete_route(&self, change: &RouteChange) -> Result<RouteChangeOutcome, StoreError>;
    async fn destination_routed(
        &self,
        workspace_id: Uuid,
        kind: &str,
        destination_ref: &str,
    ) -> Result<bool, StoreError>;
    async fn disconnect_preview(
        &self,
        workspace_id: Uuid,
        kind: &str,
    ) -> Result<Option<DisconnectPreview>, StoreError>;
    async fn disconnect_connection(
        &self,
        change: &ConnectionChange,
        plan: impl FnOnce(&[CancelCandidate]) -> Vec<i64> + Send,
    ) -> Result<DisconnectOutcome, StoreError>;
    async fn email_projects(&self, user_id: Uuid) -> Result<Vec<EmailProject>, StoreError>;
    async fn email_choices(&self, user_id: Uuid) -> Result<Vec<EmailChoiceRow>, StoreError>;
    async fn save_email_choices(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        choices: &[(String, String)],
    ) -> Result<bool, StoreError>;
    async fn email_stop_project(&self, project_id: Uuid) -> Result<Option<String>, StoreError>;
    async fn stop_email_choice(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        event: &str,
    ) -> Result<Option<String>, StoreError>;
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
    /// K-1 for the workspace's Slack connection, when it has a connected one.
    pub slack: Option<String>,
    /// The workspace's Slack connection, when it is lost (K-10's facts).
    pub slack_lost: Option<LostSlack>,
    /// The workspace's Matrix connection, when it has a live one.
    pub matrix: Option<MatrixLine>,
    /// R-4 activity lines, newest first.
    pub activity: Vec<String>,
}

/// A Matrix connection as its workspace's page tells it (US-EIP-08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixLine {
    /// "Connected as {bot} on {homeserver host} · by {admin} · {date}".
    pub connected: String,
    /// The access token field: "saved {date}" (never the token).
    pub token_saved: String,
}

/// What the homeserver said of an accepted bot token (never the token).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixLogin {
    /// The homeserver's address, as Foundry reaches it.
    pub homeserver: String,
    /// Its host, as the page and the activity list name it.
    pub host: String,
    /// The bot's user id, from `whoami`.
    pub bot_user_id: String,
}

/// A lost Slack connection as its workspace's page tells it (K-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LostSlack {
    pub workspace_name: String,
    /// Why it was lost (`lost_reason`), when Foundry knows.
    pub reason: Option<String>,
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
    /// The workspace's Slack connection.
    pub connection: Option<SealedConnection>,
    /// The workspace's Matrix connection.
    pub matrix_connection: Option<SealedConnection>,
    /// The project's lanes, board order: what Issue moved can be limited to.
    pub lanes: Vec<TargetLane>,
}

/// What the route form's preview renders (DDR-25): the route door's target
/// and the project's newest issue, if it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePreview {
    pub target: RouteTarget,
    pub latest_issue: Option<DeliveryIssue>,
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
    /// `slack` or `matrix`.
    pub kind: String,
    pub destination_ref: String,
    pub destination_label: String,
    pub events: Vec<String>,
    pub content_level: String,
    /// Lane slugs Issue moved is limited to (empty: any lane).
    pub lanes: Vec<String>,
    pub shared_confirmed: bool,
}

/// A route as its edit form and delete dialog show it, with the door's
/// target (the project, its lanes and the workspace's Slack connection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRoute {
    pub target: RouteTarget,
    pub route: RouteSettings,
}

/// A route edit as its form submitted it (the destination stays).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteEditDraft {
    pub events: Vec<String>,
    pub content_level: String,
    /// Lane slugs Issue moved is limited to (empty: any lane).
    pub lanes: Vec<String>,
}

/// The four route changes a lead may ask for.
#[derive(Clone)]
pub enum RouteAction<'a> {
    Pause,
    /// Resume, re-checking a stopped route's destination through the
    /// connection's catalogue first.
    Resume(&'a dyn DestinationCatalogs),
    Edit(RouteEditDraft),
    Delete,
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
    let connections = records.connections(workspace_id).await?;
    let current = connections
        .iter()
        .find(|c| c.kind == SLACK && c.state != "disconnected");
    let slack_lost = current.filter(|c| c.state == "lost").map(|c| LostSlack {
        workspace_name: c.workspace_name.clone(),
        reason: c.lost_reason.clone(),
    });
    let slack = current.filter(|c| c.state != "lost").map(connected_line);
    let matrix = connections
        .iter()
        .find(|c| c.kind == MATRIX && c.state != "disconnected")
        .map(matrix_line);
    let activity = records
        .activity(workspace_id, ACTIVITY_LIMIT)
        .await?
        .iter()
        .map(activity_line)
        .collect();
    Ok(WorkspaceIntegrations {
        slack,
        slack_lost,
        matrix,
        activity,
    })
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

/// The row a new Matrix bot token is sealed for: the workspace's existing
/// Matrix connection, or a fresh id.
pub(crate) async fn matrix_connection_id(
    records: &impl IntegrationRecords,
    principal: &Principal,
) -> Result<Uuid, IntegrationError> {
    require_admin(records, principal).await?;
    Ok(records
        .connection_id(principal.workspace_id(), MATRIX)
        .await?
        .unwrap_or_else(Uuid::now_v7))
}

/// Save the workspace's Matrix connection and its audit record (admins).
pub(crate) async fn connect_matrix(
    records: &impl IntegrationRecords,
    principal: &Principal,
    login: MatrixLogin,
    connection_id: Uuid,
    sealed: SealedSecret,
    now: OffsetDateTime,
) -> Result<ConnectionWriteOutcome, IntegrationError> {
    require_admin(records, principal).await?;
    let connection = NewConnection {
        id: connection_id,
        workspace_id: principal.workspace_id(),
        kind: MATRIX.to_string(),
        external_id: login.homeserver,
        app_id: None,
        bot_user_id: Some(login.bot_user_id.clone()),
        label: login.host,
        detail: Some(login.bot_user_id),
        sealed_credential: sealed.as_bytes().to_vec(),
        connected_by: principal.user_id(),
        connected_at: now,
    };
    Ok(records.upsert_connection(&connection).await?)
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
    let matrix_connection = records
        .sealed_connection(principal.workspace_id(), MATRIX)
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
        matrix_connection,
        lanes,
    })
}

/// What a preview renders (Manage): reads only, writes nothing, asks no
/// vendor.
pub(crate) async fn route_preview(
    records: &impl IntegrationRecords,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
) -> Result<RoutePreview, IntegrationError> {
    let target = route_target(records, principal, team_slug, project_slug).await?;
    let latest_issue = records.latest_issue(target.project_id).await?;
    Ok(RoutePreview {
        target,
        latest_issue,
    })
}

/// Save a Slack or Matrix route (Manage only) on the workspace's connection
/// of its kind: its events within the chat events and at least one, a known
/// content level, lanes of the project; else not found and nothing written.
pub(crate) async fn create_route(
    records: &impl IntegrationRecords,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    draft: RouteDraft,
    now: OffsetDateTime,
) -> Result<Uuid, IntegrationError> {
    let target = route_target(records, principal, team_slug, project_slug).await?;
    let connection = match draft.kind.as_str() {
        SLACK => target.connection.clone(),
        MATRIX => target.matrix_connection.clone(),
        _ => None,
    }
    .ok_or(IntegrationError::NotFound)?;
    let lanes = checked_choices(&target, &draft.events, &draft.content_level, &draft.lanes)?;
    let route = NewRoute {
        workspace_id: target.workspace_id,
        project_id: target.project_id,
        connection_id: Some(connection.id),
        kind: draft.kind,
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

/// Events within the chat events and at least one, a known content level,
/// and lanes of the project (by slug), else not found.
fn checked_choices(
    target: &RouteTarget,
    events: &[String],
    content_level: &str,
    lane_slugs: &[String],
) -> Result<Vec<RouteLane>, IntegrationError> {
    let events_ok = !events.is_empty()
        && events
            .iter()
            .all(|event| CHAT_EVENTS.contains(&event.as_str()));
    if !events_ok || !CONTENT_LEVELS.contains(&content_level) {
        return Err(IntegrationError::NotFound);
    }
    lane_slugs
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
        .collect()
}

/// Manage only: the project's route `route_id` as its edit form and delete
/// dialog show it; a route of another project is not found.
pub(crate) async fn managed_route(
    records: &impl IntegrationRecords,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    route_id: Uuid,
) -> Result<ManagedRoute, IntegrationError> {
    let target = route_target(records, principal, team_slug, project_slug).await?;
    let route = records
        .route_settings(target.project_id, route_id)
        .await?
        .ok_or(IntegrationError::NotFound)?;
    Ok(ManagedRoute { target, route })
}

/// Manage only: one change of the project's route `route_id`, with its audit
/// record. A pause skips exactly what [`cancel_set`] names at `now`; an edit
/// is checked as a new route is. A route already so (or already gone)
/// changes nothing.
pub(crate) async fn change_route(
    records: &impl IntegrationRecords,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    route_id: Uuid,
    action: RouteAction<'_>,
    now: OffsetDateTime,
) -> Result<RouteChangeOutcome, IntegrationError> {
    let target = route_target(records, principal, team_slug, project_slug).await?;
    let change = RouteChange {
        project_id: target.project_id,
        route_id,
        actor_id: principal.user_id(),
        via: "web".to_string(),
        at: now,
    };
    let outcome = match action {
        RouteAction::Pause => {
            records
                .pause_route(&change, |candidates| {
                    cancel_set(route_id, now, &targeted(candidates))
                })
                .await?
        }
        RouteAction::Resume(catalogs) => {
            if !problem_cleared(records, catalogs, &target, route_id).await? {
                return Ok(RouteChangeOutcome::Unchanged);
            }
            records.resume_route(&change).await?
        }
        RouteAction::Edit(draft) => {
            let lanes =
                checked_choices(&target, &draft.events, &draft.content_level, &draft.lanes)?;
            let edit = RouteEdit {
                events: draft.events,
                content_level: draft.content_level,
                lanes,
            };
            records.edit_route(&change, &edit).await?
        }
        RouteAction::Delete => records.delete_route(&change).await?,
    };
    match outcome {
        RouteChangeOutcome::ProjectDeleted => Err(IntegrationError::NotFound),
        outcome => Ok(outcome),
    }
}

/// Whether a route of the principal's workspace, on its connection of
/// `kind`, still sends to `destination_ref` (in a live project, paused or
/// not).
pub(crate) async fn room_still_routed(
    records: &impl IntegrationRecords,
    principal: &Principal,
    kind: &str,
    destination_ref: &str,
) -> Result<bool, IntegrationError> {
    Ok(records
        .destination_routed(principal.workspace_id(), kind, destination_ref)
        .await?)
}

/// Admins only: what disconnecting the workspace's connection of `kind`
/// stops; `None` when there is nothing to disconnect.
pub(crate) async fn disconnect_preview(
    records: &impl IntegrationRecords,
    principal: &Principal,
    kind: &str,
) -> Result<Option<DisconnectPreview>, IntegrationError> {
    require_admin(records, principal).await?;
    Ok(records
        .disconnect_preview(principal.workspace_id(), kind)
        .await?)
}

/// Admins only: disconnect the workspace's connection of `kind` at `now`,
/// skipping exactly what [`cancel_set`] names for each of its routes.
pub(crate) async fn disconnect(
    records: &impl IntegrationRecords,
    principal: &Principal,
    kind: &str,
    now: OffsetDateTime,
) -> Result<DisconnectOutcome, IntegrationError> {
    require_admin(records, principal).await?;
    let change = ConnectionChange {
        workspace_id: principal.workspace_id(),
        kind: kind.to_string(),
        actor_id: principal.user_id(),
        via: "web".to_string(),
        at: now,
    };
    Ok(records
        .disconnect_connection(&change, |candidates| {
            let deliveries = targeted(candidates);
            let mut routes: Vec<Uuid> = candidates.iter().map(|c| c.route_id).collect();
            routes.sort_unstable();
            routes.dedup();
            routes
                .into_iter()
                .flat_map(|route| cancel_set(route, now, &deliveries))
                .collect()
        })
        .await?)
}

/// Whether the route `route_id` may resume: it has no channel problem, or
/// the vendor now shows that problem gone.
async fn problem_cleared(
    records: &impl IntegrationRecords,
    catalogs: &dyn DestinationCatalogs,
    target: &RouteTarget,
    route_id: Uuid,
) -> Result<bool, IntegrationError> {
    let Some(settings) = records.route_settings(target.project_id, route_id).await? else {
        return Ok(true);
    };
    let Some(problem) = settings.problem_reason else {
        return Ok(true);
    };
    let Some(catalog) = target
        .connection
        .as_ref()
        .and_then(|c| catalogs.catalogue(c))
    else {
        return Ok(false);
    };
    let destination = DestinationRef(settings.destination_ref);
    Ok(match catalog.check_destination(&destination).await {
        Ok(check) => ReasonCode::from_wire_name(&problem)
            .map_or(check.can_post, |problem| check.clears(problem)),
        Err(_) => false,
    })
}

fn targeted(candidates: &[CancelCandidate]) -> Vec<TargetedDelivery> {
    candidates
        .iter()
        .map(|candidate| TargetedDelivery {
            id: candidate.id,
            target: candidate.route_id,
            finished: candidate.finished,
            lease_until: candidate.lease_until,
        })
        .collect()
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

/// US-EIP-08: "Connected as {bot} on {host} · by {admin} · {date}" and the
/// token field's "saved {date}".
fn matrix_line(connection: &ConnectionRow) -> MatrixLine {
    let bot = connection.detail.as_deref().unwrap_or_default();
    let by = connection
        .connected_by_name
        .as_deref()
        .map(|name| format!(" · by {name}"))
        .unwrap_or_default();
    let date = display_date(connection.connected_at);
    MatrixLine {
        connected: format!("Connected as {bot} on {}{by} · {date}", connection.label),
        token_saved: format!("saved {date}"),
    }
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
            lost_reason: None,
            workspace_name: "Canzan Labs".into(),
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
    fn the_matrix_line_names_the_bot_and_the_homeserver() {
        let connection = ConnectionRow {
            kind: MATRIX.into(),
            state: "connected".into(),
            lost_reason: None,
            workspace_name: "Canzan Labs".into(),
            label: "matrix.canzan.dev".into(),
            detail: Some("@foundry:canzan.dev".into()),
            connected_by_name: Some("Priya Raman".into()),
            connected_at: datetime!(2026-10-21 14:02 UTC),
        };
        assert_eq!(
            matrix_line(&connection),
            MatrixLine {
                connected: "Connected as @foundry:canzan.dev on matrix.canzan.dev · by Priya Raman · 21 Oct 2026".into(),
                token_saved: "saved 21 Oct 2026".into(),
            }
        );
        assert_eq!(
            activity_line(&row("connection_connected", "matrix", "matrix.canzan.dev")),
            "21 Oct 2026 · Priya Raman connected Matrix (matrix.canzan.dev)"
        );
    }

    #[test]
    fn single_digit_days_are_not_padded() {
        assert_eq!(display_date(datetime!(2026-09-05 08:00 UTC)), "5 Sep 2026");
    }
}

/// A member's Project emails section: the projects they can open, and their
/// choices in them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectEmails {
    pub projects: Vec<EmailProject>,
    pub choices: Vec<EmailChoiceRow>,
}

/// One choice as a member submits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmailChoiceInput {
    pub project_id: Uuid,
    pub event: EventKind,
    pub scope: EmailScope,
}

/// One saved choice, as the confirmation names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedEmailChoice {
    pub project_name: String,
    pub event: EventKind,
    pub scope: EmailScope,
}

/// The member's Project emails: only projects they can open, with only the
/// choices they made.
pub(crate) async fn project_emails(
    records: &impl IntegrationRecords,
    user_id: Uuid,
) -> Result<ProjectEmails, IntegrationError> {
    let projects = records.email_projects(user_id).await?;
    let choices = records
        .email_choices(user_id)
        .await?
        .into_iter()
        .filter(|c| projects.iter().any(|p| p.project_id == c.project_id))
        .collect();
    Ok(ProjectEmails { projects, choices })
}

/// Replace the member's choices with `submitted` in every project they can
/// open. A choice for any other project refuses the save, nothing written.
/// Ticking both scopes of one event keeps "all issues".
pub(crate) async fn save_project_emails(
    records: &impl IntegrationRecords,
    user_id: Uuid,
    submitted: &[EmailChoiceInput],
) -> Result<Vec<SavedEmailChoice>, IntegrationError> {
    let projects = records.email_projects(user_id).await?;
    if !submitted
        .iter()
        .all(|c| projects.iter().any(|p| p.project_id == c.project_id))
    {
        return Err(IntegrationError::NotFound);
    }
    let mut saved = Vec::new();
    for project in &projects {
        let mut chosen: Vec<(EventKind, EmailScope)> = Vec::new();
        for choice in submitted
            .iter()
            .filter(|c| c.project_id == project.project_id)
        {
            match chosen.iter_mut().find(|(event, _)| *event == choice.event) {
                Some((_, scope)) if choice.scope == EmailScope::All => *scope = EmailScope::All,
                Some(_) => {}
                None => chosen.push((choice.event, choice.scope)),
            }
        }
        let rows: Vec<(String, String)> = chosen
            .iter()
            .map(|(event, scope)| (event.wire_name().to_string(), scope.wire_name().to_string()))
            .collect();
        if records
            .save_email_choices(user_id, project.project_id, &rows)
            .await?
        {
            saved.extend(chosen.into_iter().map(|(event, scope)| SavedEmailChoice {
                project_name: project.project_name.clone(),
                event,
                scope,
            }));
        }
    }
    Ok(saved)
}

/// The one choice a stop link names, as its pages say it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailStop {
    pub project_name: String,
    pub event: EventKind,
}

/// The event a stop link may name: one a member can choose by email.
fn stoppable(event: &str) -> Result<EventKind, IntegrationError> {
    EventKind::from_wire_name(event)
        .filter(|kind| email_events().any(|e| e == *kind))
        .ok_or(IntegrationError::NotFound)
}

/// What a stop link's confirm page names (it changes nothing): the live
/// project and the event; anything else is the uniform refusal.
pub(crate) async fn email_stop_target(
    records: &impl IntegrationRecords,
    project_id: Uuid,
    event: &str,
) -> Result<EmailStop, IntegrationError> {
    let event = stoppable(event)?;
    let project_name = records
        .email_stop_project(project_id)
        .await?
        .ok_or(IntegrationError::NotFound)?;
    Ok(EmailStop {
        project_name,
        event,
    })
}

/// Stop the member's `event` emails from `project_id` (US-EIP-11): that one
/// choice goes, the others stay. Stopping twice stops once.
pub(crate) async fn stop_project_email(
    records: &impl IntegrationRecords,
    user_id: Uuid,
    project_id: Uuid,
    event: &str,
) -> Result<EmailStop, IntegrationError> {
    let event = stoppable(event)?;
    let project_name = records
        .stop_email_choice(user_id, project_id, event.wire_name())
        .await?
        .ok_or(IntegrationError::NotFound)?;
    Ok(EmailStop {
        project_name,
        event,
    })
}
