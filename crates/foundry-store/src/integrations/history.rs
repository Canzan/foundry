//! A project's routes as the project page, the route form and a route's
//! history read them (K-9, US-EIP-05).

use super::StoredKind;
use crate::{Store, StoreError};
use time::OffsetDateTime;
use uuid::Uuid;

/// `route_settings`' row: kind, destination ref and label, events, content
/// level, lane slugs.
type SettingsRow = (
    StoredKind,
    String,
    String,
    Vec<String>,
    String,
    Vec<String>,
    Option<String>,
);

/// A route as a project's integrations page lists it, with the facts its
/// status is derived from (DDR-19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRoute {
    pub id: Uuid,
    pub kind: String,
    pub destination_label: String,
    pub paused: bool,
    /// `integration_connections.state`; `None` for a webhook route.
    pub connection_state: Option<String>,
    /// The connection's bot account (`integration_connections.bot_user_id`).
    pub bot_user_id: Option<String>,
    pub problem_reason: Option<String>,
    /// The label at save of a filter lane the project no longer has.
    pub missing_lane: Option<String>,
    pub newest_finished: Option<FinishedDeliveryRow>,
    /// The head pending delivery, when it has failed at least once, and the
    /// vendor's latest reason for it.
    pub head_failed: Option<Option<String>>,
    pub last_delivered_at: Option<OffsetDateTime>,
}

/// A route as its edit form shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteSettings {
    pub id: Uuid,
    /// `slack`, `matrix`, …
    pub kind: String,
    pub destination_ref: String,
    pub destination_label: String,
    pub events: Vec<String>,
    pub content_level: String,
    /// Slugs of the filter lanes the project still has.
    pub lanes: Vec<String>,
    /// Why a vendor refusal stopped the route, if one did.
    pub problem_reason: Option<String>,
}

/// The newest finished delivery of a route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedDeliveryRow {
    pub status: String,
    pub final_reason: Option<String>,
    pub issue_key: Option<String>,
}

/// A project's routes with their status facts, oldest first.
const PROJECT_ROUTES_SQL: &str = "
    SELECT r.id, r.kind, r.destination_label, r.paused_at IS NOT NULL AS paused,
           c.state AS connection_state, c.bot_user_id, r.problem_reason,
           (SELECT l.label_at_save FROM integration_route_lanes l
             WHERE l.route_id = r.id
               AND NOT EXISTS (SELECT 1 FROM lanes x
                                WHERE x.id = l.lane_id AND x.project_id = r.project_id)
             ORDER BY l.label_at_save LIMIT 1) AS missing_lane,
           f.status AS finished_status, f.final_reason, f.issue_key,
           h.id IS NOT NULL AND (h.next_attempt_at IS NOT NULL
                                 OR h.last_vendor_reason IS NOT NULL) AS head_failed,
           h.last_vendor_reason,
           (SELECT max(d.finished_at) FROM integration_deliveries d
             WHERE d.route_id = r.id AND d.status = 'delivered') AS last_delivered_at
      FROM integration_routes r
      LEFT JOIN integration_connections c ON c.id = r.connection_id
      LEFT JOIN LATERAL (
            SELECT d.status, d.final_reason, d.issue_key FROM integration_deliveries d
             WHERE d.route_id = r.id AND d.status <> 'pending'
             ORDER BY d.finished_at DESC, d.id DESC LIMIT 1) f ON TRUE
      LEFT JOIN LATERAL (
            SELECT d.id, d.next_attempt_at, d.last_vendor_reason FROM integration_deliveries d
             WHERE d.route_id = r.id AND d.status = 'pending'
             ORDER BY d.id LIMIT 1) h ON TRUE
     WHERE r.project_id = $1
     ORDER BY r.created_at, r.id";

/// One delivery of a route, as its history lists it (US-EIP-05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryHistoryRow {
    pub occurred_at: OffsetDateTime,
    pub event_kind: String,
    pub issue_key: Option<String>,
    pub status: String,
    pub attempts: i32,
    pub first_attempt_at: Option<OffsetDateTime>,
    pub finished_at: Option<OffsetDateTime>,
    pub give_up_at: OffsetDateTime,
    pub lease_until: Option<OffsetDateTime>,
    pub next_attempt_at: Option<OffsetDateTime>,
    pub last_vendor_reason: Option<String>,
    pub final_reason: Option<String>,
    pub slowed_down: bool,
}

/// A route's deliveries, newest first.
const ROUTE_HISTORY_SQL: &str = "
    SELECT occurred_at, event_kind, issue_key, status, attempts, first_attempt_at,
           finished_at, give_up_at, lease_until, next_attempt_at, last_vendor_reason,
           final_reason, slowed_down
      FROM integration_deliveries
     WHERE route_id = $1
     ORDER BY id DESC
     LIMIT $2";

fn delivery_history_row(row: &sqlx::postgres::PgRow) -> Result<DeliveryHistoryRow, sqlx::Error> {
    use sqlx::Row;
    Ok(DeliveryHistoryRow {
        occurred_at: row.try_get("occurred_at")?,
        event_kind: row.try_get("event_kind")?,
        issue_key: row.try_get("issue_key")?,
        status: row.try_get("status")?,
        attempts: row.try_get("attempts")?,
        first_attempt_at: row.try_get("first_attempt_at")?,
        finished_at: row.try_get("finished_at")?,
        give_up_at: row.try_get("give_up_at")?,
        lease_until: row.try_get("lease_until")?,
        next_attempt_at: row.try_get("next_attempt_at")?,
        last_vendor_reason: row.try_get("last_vendor_reason")?,
        final_reason: row.try_get("final_reason")?,
        slowed_down: row.try_get("slowed_down")?,
    })
}

fn project_route(row: &sqlx::postgres::PgRow) -> Result<ProjectRoute, sqlx::Error> {
    use sqlx::Row;
    let finished_status: Option<String> = row.try_get("finished_status")?;
    let head_failed: bool = row.try_get("head_failed")?;
    let head_reason: Option<String> = row.try_get("last_vendor_reason")?;
    Ok(ProjectRoute {
        id: row.try_get("id")?,
        kind: row.try_get::<StoredKind, _>("kind")?.wire(),
        destination_label: row.try_get("destination_label")?,
        paused: row.try_get("paused")?,
        connection_state: row.try_get("connection_state")?,
        bot_user_id: row.try_get("bot_user_id")?,
        problem_reason: row.try_get("problem_reason")?,
        missing_lane: row.try_get("missing_lane")?,
        newest_finished: match finished_status {
            Some(status) => Some(FinishedDeliveryRow {
                status,
                final_reason: row.try_get("final_reason")?,
                issue_key: row.try_get("issue_key")?,
            }),
            None => None,
        },
        head_failed: head_failed.then_some(head_reason),
        last_delivered_at: row.try_get("last_delivered_at")?,
    })
}

impl Store {
    /// A project's routes with their status facts, oldest first.
    pub async fn project_routes(&self, project_id: Uuid) -> Result<Vec<ProjectRoute>, StoreError> {
        let rows = sqlx::query(PROJECT_ROUTES_SQL)
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(project_route)
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// What the edit form shows of the project's route `route_id`: its
    /// destination, events, content level and the slugs of its filter lanes
    /// the project still has. `None` for a route of another project.
    pub async fn route_settings(
        &self,
        project_id: Uuid,
        route_id: Uuid,
    ) -> Result<Option<RouteSettings>, StoreError> {
        let row: Option<SettingsRow> = sqlx::query_as(
            "SELECT r.kind, r.destination_ref, r.destination_label, r.events, r.content_level,
                    COALESCE(array_agg(x.slug ORDER BY x.position)
                             FILTER (WHERE x.slug IS NOT NULL), '{}'),
                    r.problem_reason
               FROM integration_routes r
               LEFT JOIN integration_route_lanes l ON l.route_id = r.id
               LEFT JOIN lanes x ON x.id = l.lane_id AND x.project_id = r.project_id
              WHERE r.id = $1 AND r.project_id = $2
              GROUP BY r.id",
        )
        .bind(route_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(
            |(
                kind,
                destination_ref,
                destination_label,
                events,
                content_level,
                lanes,
                problem_reason,
            )| {
                RouteSettings {
                    id: route_id,
                    kind: kind.wire(),
                    destination_ref,
                    destination_label,
                    events,
                    content_level,
                    lanes,
                    problem_reason,
                }
            },
        ))
    }

    /// The route's newest `limit` deliveries, newest first (its history).
    pub async fn route_history(
        &self,
        route_id: Uuid,
        limit: i64,
    ) -> Result<Vec<DeliveryHistoryRow>, StoreError> {
        let rows = sqlx::query(ROUTE_HISTORY_SQL)
            .bind(route_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(delivery_history_row)
            .collect::<Result<Vec<_>, _>>()?)
    }
}
