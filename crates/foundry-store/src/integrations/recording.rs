//! Recording an attempt: the fenced record of a claimed delivery, and a lost
//! connection's `connected → lost` compare-and-set (DDR-5, DDR-21).

use super::metrics::{delivery_tally, DeliveryTally, TALLY_COLUMNS};
use super::StoredKind;
use crate::project_lifecycle::lock_live_project;
use crate::{Store, StoreError};
use time::OffsetDateTime;
use uuid::Uuid;

const STATUS_PENDING: &str = "pending";

/// What a sender learned from one attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryResult {
    Delivered,
    Retry {
        next_attempt_at: OffsetDateTime,
        vendor_reason: Option<String>,
    },
    Failed {
        final_reason: String,
        vendor_reason: Option<String>,
    },
    Skipped {
        final_reason: String,
    },
    /// The vendor asked Foundry to wait: retry at `next_attempt_at`, and send
    /// nothing to the destination before `not_before` (its "retry after").
    SlowedDown {
        next_attempt_at: OffsetDateTime,
        not_before: Option<OffsetDateTime>,
        vendor_reason: String,
    },
    /// The vendor refused the destination: the delivery waits, unscheduled,
    /// and its route gets `vendor_reason` as its problem.
    RouteRefused {
        vendor_reason: String,
    },
    /// The vendor refused the connection: the delivery waits, unscheduled,
    /// and a connected connection becomes lost for `vendor_reason`.
    ConnectionRefused {
        vendor_reason: String,
    },
}

/// The columns a record writes.
struct RecordColumns<'a> {
    status: &'a str,
    /// The record finishes the row (`finished_at`).
    finishes: bool,
    next_attempt_at: Option<OffsetDateTime>,
    vendor_reason: Option<&'a str>,
    final_reason: Option<&'a str>,
    /// The vendor asked Foundry to slow down.
    slowed_down: bool,
}

impl<'a> RecordColumns<'a> {
    /// A finished row: `status`, for `final_reason` if any.
    fn finished(status: &'a str, final_reason: Option<&'a str>) -> Self {
        Self {
            status,
            finishes: true,
            next_attempt_at: None,
            vendor_reason: None,
            final_reason,
            slowed_down: false,
        }
    }

    /// A row left pending, to be tried at `next_attempt_at` (unscheduled when
    /// `None`).
    fn pending(next_attempt_at: Option<OffsetDateTime>, vendor_reason: Option<&'a str>) -> Self {
        Self {
            status: STATUS_PENDING,
            finishes: false,
            next_attempt_at,
            vendor_reason,
            final_reason: None,
            slowed_down: false,
        }
    }
}

impl DeliveryResult {
    fn columns(&self) -> RecordColumns<'_> {
        match self {
            Self::Delivered => RecordColumns::finished("delivered", None),
            Self::Retry {
                next_attempt_at,
                vendor_reason,
            } => RecordColumns::pending(Some(*next_attempt_at), vendor_reason.as_deref()),
            Self::SlowedDown {
                next_attempt_at,
                vendor_reason,
                ..
            } => RecordColumns {
                slowed_down: true,
                ..RecordColumns::pending(Some(*next_attempt_at), Some(vendor_reason))
            },
            Self::RouteRefused { vendor_reason } | Self::ConnectionRefused { vendor_reason } => {
                RecordColumns::pending(None, Some(vendor_reason))
            }
            Self::Failed {
                final_reason,
                vendor_reason,
            } => RecordColumns {
                vendor_reason: vendor_reason.as_deref(),
                ..RecordColumns::finished("failed", Some(final_reason))
            },
            Self::Skipped { final_reason } => {
                RecordColumns::finished("skipped", Some(final_reason))
            }
        }
    }
}

/// A connection that has just turned `connected → lost`: what its
/// workspace's admins are told, and their addresses (DDR-23).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LostConnection {
    pub workspace_id: Uuid,
    pub workspace_name: String,
    pub kind: String,
    pub reason: Option<String>,
    pub admins: Vec<String>,
}

/// The rest of a `connected → lost` transition, in its transaction: the
/// audit record, and what the admins are told.
async fn connection_lost(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    connection_id: Uuid,
    via: &str,
    now: OffsetDateTime,
) -> Result<LostConnection, StoreError> {
    let (workspace_id,): (Uuid,) = sqlx::query_as(CONNECTION_LOST_AUDIT_SQL)
        .bind(Uuid::now_v7())
        .bind(via)
        .bind(now)
        .bind(connection_id)
        .fetch_one(&mut **tx)
        .await?;
    let (workspace_name, kind, reason): (String, StoredKind, Option<String>) =
        sqlx::query_as(LOST_CONNECTION_SQL)
            .bind(connection_id)
            .fetch_one(&mut **tx)
            .await?;
    let admins: Vec<(String,)> = sqlx::query_as(WORKSPACE_ADMINS_SQL)
        .bind(workspace_id)
        .fetch_all(&mut **tx)
        .await?;
    Ok(LostConnection {
        workspace_id,
        workspace_name,
        kind: kind.wire(),
        reason,
        admins: admins.into_iter().map(|(email,)| email).collect(),
    })
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecordOutcome {
    Recorded(DeliveryTally),
    /// Recorded, and this record's refusal turned the route's connection
    /// from connected to lost: its admins are to be told, once.
    ConnectionLost {
        tally: DeliveryTally,
        lost: LostConnection,
    },
    /// The claim is no longer the row's (taken over, or already finished).
    LeaseLost,
}

impl RecordOutcome {
    /// What the record counts, unless it wrote nothing.
    pub fn tally(&self) -> Option<&DeliveryTally> {
        match self {
            Self::Recorded(tally) | Self::ConnectionLost { tally, .. } => Some(tally),
            Self::LeaseLost => None,
        }
    }
}

/// A recorded row: its route and project, then its tally.
type RecordedRow = (Option<Uuid>, Uuid, String, String, String, f64);

/// The fenced record: only the current claim of a pending row writes.
const RECORD_SQL: &str = "
    UPDATE integration_deliveries
       SET status = $3, lease_until = NULL, next_attempt_at = $4,
           last_vendor_reason = COALESCE($5, last_vendor_reason), final_reason = $6,
           finished_at = CASE WHEN $7 THEN $8 END,
           slowed_down = slowed_down OR $9
     WHERE id = $1 AND claim_id = $2 AND status = 'pending'
    RETURNING route_id, project_id,";

/// A "retry after": the route's destination waits at least until $2.
const SLOW_DOWN_SQL: &str = "
    INSERT INTO integration_destination_pacing (connection_id, destination_ref, not_before)
    SELECT r.connection_id, r.destination_ref, $2 FROM integration_routes r
     WHERE r.id = $1 AND r.connection_id IS NOT NULL
    ON CONFLICT (connection_id, destination_ref)
    DO UPDATE SET not_before = GREATEST(integration_destination_pacing.not_before,
                                        EXCLUDED.not_before)";

/// A refused destination: the route needs attention (kept if already set).
const ROUTE_PROBLEM_SQL: &str = "
    UPDATE integration_routes SET problem_reason = $2, problem_at = $3
     WHERE id = $1 AND problem_reason IS NULL";

/// A refused connection: `connected → lost`, once (compare-and-set).
const CONNECTION_LOST_SQL: &str = "
    UPDATE integration_connections c SET state = 'lost', lost_reason = $2, lost_at = $3
      FROM integration_routes r
     WHERE r.id = $1 AND c.id = r.connection_id AND c.state = 'connected'
    RETURNING c.id";

/// Slack's word that the Foundry app left a Slack workspace: that
/// workspace's connection of this app, `connected → lost`, once.
const SLACK_APP_LOST_SQL: &str = "
    UPDATE integration_connections SET state = 'lost', lost_reason = $3, lost_at = $4
     WHERE kind = 'slack' AND external_id = $1 AND app_id = $2 AND state = 'connected'
    RETURNING id";

/// The audit record of a connection that was lost (DDR-21), by Foundry.
const CONNECTION_LOST_AUDIT_SQL: &str = "
    INSERT INTO integration_audit_events
           (id, workspace_id, project_id, actor_id, via, action, kind, subject, created_at)
    SELECT $1, c.workspace_id, NULL, NULL, $2, 'connection_lost', c.kind, c.label, $3
      FROM integration_connections c WHERE c.id = $4
    RETURNING workspace_id";

/// What a lost connection's admins are told: the workspace's name, the
/// connection's kind and why it was lost.
const LOST_CONNECTION_SQL: &str = "
    SELECT w.name, c.kind, c.lost_reason
      FROM integration_connections c JOIN workspaces w ON w.id = c.workspace_id
     WHERE c.id = $1";

/// A workspace's admins' addresses.
const WORKSPACE_ADMINS_SQL: &str = "
    SELECT u.email_display FROM workspace_memberships m JOIN users u ON u.id = m.user_id
     WHERE m.workspace_id = $1 AND m.role = 'admin'
     ORDER BY u.email_lower";

impl Store {
    /// The fenced record of one attempt (ADR-EIP-002 decision 4): writes only
    /// while `claim_id` is still the pending row's claim, releasing the lease,
    /// and in the same transaction applies what the answer means beyond the
    /// row: a "retry after" on the destination's pacing, a refused
    /// destination as its route's problem, a refused connection as its
    /// `connected → lost` transition.
    /// A recorder whose lease was taken over gets [`RecordOutcome::LeaseLost`]
    /// and has changed nothing.
    pub async fn record_delivery(
        &self,
        delivery_id: i64,
        claim_id: Uuid,
        now: OffsetDateTime,
        result: DeliveryResult,
    ) -> Result<RecordOutcome, StoreError> {
        let columns = result.columns();
        let mut tx = self.pool.begin().await?;
        let written: Option<RecordedRow> = sqlx::query_as(&format!("{RECORD_SQL}{TALLY_COLUMNS}"))
            .bind(delivery_id)
            .bind(claim_id)
            .bind(columns.status)
            .bind(columns.next_attempt_at)
            .bind(columns.vendor_reason)
            .bind(columns.final_reason)
            .bind(columns.finishes)
            .bind(now)
            .bind(columns.slowed_down)
            .fetch_optional(&mut *tx)
            .await?;
        let Some((route_id, project_id, kind, event_kind, status, lag_seconds)) = written else {
            tx.rollback().await?;
            return Ok(RecordOutcome::LeaseLost);
        };
        let tally = delivery_tally((kind, event_kind, status, lag_seconds));
        match (&result, route_id) {
            (
                DeliveryResult::SlowedDown {
                    not_before: Some(not_before),
                    ..
                },
                Some(route_id),
            ) => {
                sqlx::query(SLOW_DOWN_SQL)
                    .bind(route_id)
                    .bind(*not_before)
                    .execute(&mut *tx)
                    .await?;
            }
            // A route of a project deleted meanwhile is left as it is.
            (DeliveryResult::RouteRefused { vendor_reason }, Some(route_id)) => {
                if lock_live_project(&mut tx, project_id).await? {
                    sqlx::query(ROUTE_PROBLEM_SQL)
                        .bind(route_id)
                        .bind(vendor_reason)
                        .bind(now)
                        .execute(&mut *tx)
                        .await?;
                }
            }
            (DeliveryResult::ConnectionRefused { vendor_reason }, Some(route_id)) => {
                let lost: Option<(Uuid,)> = sqlx::query_as(CONNECTION_LOST_SQL)
                    .bind(route_id)
                    .bind(vendor_reason)
                    .bind(now)
                    .fetch_optional(&mut *tx)
                    .await?;
                if let Some((connection_id,)) = lost {
                    let lost = connection_lost(&mut tx, connection_id, "system", now).await?;
                    tx.commit().await?;
                    return Ok(RecordOutcome::ConnectionLost { tally, lost });
                }
            }
            _ => {}
        }
        tx.commit().await?;
        Ok(RecordOutcome::Recorded(tally))
    }

    /// Slack's signed word that the Foundry app `app_id` left the Slack
    /// workspace `team_id` (ADR-EIP-005 decision 6): that connection,
    /// `connected → lost` for `reason`, with its audit record, in one
    /// transaction. Only the call that turns it gets the
    /// [`LostConnection`]; a connection already lost, another app's or
    /// another Slack workspace's changes nothing.
    pub async fn slack_app_lost(
        &self,
        team_id: &str,
        app_id: &str,
        reason: &str,
        now: OffsetDateTime,
    ) -> Result<Option<LostConnection>, StoreError> {
        let mut tx = self.pool.begin().await?;
        let lost: Option<(Uuid,)> = sqlx::query_as(SLACK_APP_LOST_SQL)
            .bind(team_id)
            .bind(app_id)
            .bind(reason)
            .bind(now)
            .fetch_optional(&mut *tx)
            .await?;
        let Some((connection_id,)) = lost else {
            tx.rollback().await?;
            return Ok(None);
        };
        let lost = connection_lost(&mut tx, connection_id, "slack", now).await?;
        tx.commit().await?;
        Ok(Some(lost))
    }
}
