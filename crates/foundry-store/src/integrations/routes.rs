//! A project's route writers, each holding the live-project guard and writing
//! its audit record in the same transaction (ADR-EIP-002 decision 3, DD-8,
//! DDR-21, DDR-27).

use crate::project_lifecycle::lock_live_project;
use crate::{Store, StoreError};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteLane {
    pub lane_id: Uuid,
    pub label_at_save: String,
}

/// A route as the create use-case saves it; `receives_from` is the database's
/// `now()` of the write (DDR-27).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRoute {
    pub workspace_id: Uuid,
    pub project_id: Uuid,
    pub connection_id: Option<Uuid>,
    pub kind: String,
    pub destination_ref: String,
    pub destination_label: String,
    pub events: Vec<String>,
    pub content_level: String,
    pub shared_confirmed: bool,
    pub lanes: Vec<RouteLane>,
    pub created_by: Uuid,
    pub created_via: String,
    /// The injected clock's time of the save (the audit record's date).
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteWriteOutcome {
    Created(Uuid),
    /// The project is deleted (or being deleted); nothing was written.
    ProjectDeleted,
}

/// Who changes a route, and when (the injected clock's time: the audit
/// record's date, and the "now" a lease is judged against).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteChange {
    pub project_id: Uuid,
    pub route_id: Uuid,
    pub actor_id: Uuid,
    pub via: String,
    pub at: OffsetDateTime,
}

/// What a route's edit sets (its destination stays).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteEdit {
    pub events: Vec<String>,
    pub content_level: String,
    pub lanes: Vec<RouteLane>,
}

/// One delivery of a route as a pause's cancel plan sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelCandidate {
    pub id: i64,
    pub route_id: Uuid,
    pub finished: bool,
    pub lease_until: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteChangeOutcome {
    /// The route changed and its audit record was written; a pause also
    /// skipped `cancelled` deliveries.
    Changed { cancelled: u64 },
    /// The route already was so (paused, live, gone): nothing was written.
    Unchanged,
    /// The project is deleted (or being deleted); nothing was written.
    ProjectDeleted,
}

/// A pause's compare-and-set on the plan's ids: still the route's, pending,
/// and holding no live lease at `$3`.
const CANCEL_SQL: &str = "
    UPDATE integration_deliveries
       SET status = 'skipped', final_reason = $4, finished_at = $3,
           next_attempt_at = NULL, lease_until = NULL
     WHERE id = ANY($1) AND route_id = $2 AND status = 'pending'
       AND (lease_until IS NULL OR lease_until <= $3)";

/// A route's audit record (DDR-21), its subject the route's destination.
const ROUTE_AUDIT_SQL: &str = "
    INSERT INTO integration_audit_events
           (id, workspace_id, project_id, actor_id, via, action, kind, subject, created_at)
    SELECT $1, r.workspace_id, r.project_id, $2, $3, $4, r.kind, r.destination_label, $5
      FROM integration_routes r WHERE r.id = $6";

impl Store {
    /// Save a route, its lane filter and its audit record (ADR-EIP-002
    /// decision 3, DDR-21), one transaction. Opens with `lock_live_project`, so
    /// a project delete in flight makes it wait and a deleted project refuses
    /// it ([`RouteWriteOutcome::ProjectDeleted`]). `receives_from` is the
    /// transaction's `now()`; the audit record is dated `created_at`.
    pub async fn create_route(&self, route: &NewRoute) -> Result<RouteWriteOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        if !lock_live_project(&mut tx, route.project_id).await? {
            tx.rollback().await?;
            return Ok(RouteWriteOutcome::ProjectDeleted);
        }
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO integration_routes
                    (id, workspace_id, project_id, connection_id, kind, destination_ref,
                     destination_label, events, content_level, shared_confirmed,
                     receives_from, created_by, created_via)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, now(), $11, $12)",
        )
        .bind(id)
        .bind(route.workspace_id)
        .bind(route.project_id)
        .bind(route.connection_id)
        .bind(&route.kind)
        .bind(&route.destination_ref)
        .bind(&route.destination_label)
        .bind(&route.events)
        .bind(&route.content_level)
        .bind(route.shared_confirmed)
        .bind(route.created_by)
        .bind(&route.created_via)
        .execute(&mut *tx)
        .await?;
        insert_route_lanes(&mut tx, id, &route.lanes).await?;
        sqlx::query(
            "INSERT INTO integration_audit_events
                    (id, workspace_id, project_id, actor_id, via, action, kind, subject, created_at)
             VALUES ($1, $2, $3, $4, $5, 'route_created', $6, $7, $8)",
        )
        .bind(Uuid::now_v7())
        .bind(route.workspace_id)
        .bind(route.project_id)
        .bind(route.created_by)
        .bind(&route.created_via)
        .bind(&route.kind)
        .bind(&route.destination_label)
        .bind(route.created_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(RouteWriteOutcome::Created(id))
    }

    /// Pause a live route (DD-8), one transaction opening with
    /// `lock_live_project`: stamps it paused, reads its pending deliveries,
    /// skips exactly the ids `plan` names that are still pending and hold no
    /// live lease at `change.at` (a leased one is left to its sender), and
    /// writes its `route_paused` record. A paused or missing route changes
    /// nothing.
    pub async fn pause_route<P>(
        &self,
        change: &RouteChange,
        plan: P,
    ) -> Result<RouteChangeOutcome, StoreError>
    where
        P: FnOnce(&[CancelCandidate]) -> Vec<i64>,
    {
        let mut tx = self.pool.begin().await?;
        if !lock_live_project(&mut tx, change.project_id).await? {
            tx.rollback().await?;
            return Ok(RouteChangeOutcome::ProjectDeleted);
        }
        let paused = sqlx::query(
            "UPDATE integration_routes SET paused_at = $3, paused_by = $4, updated_at = now()
              WHERE id = $1 AND project_id = $2 AND paused_at IS NULL",
        )
        .bind(change.route_id)
        .bind(change.project_id)
        .bind(change.at)
        .bind(change.actor_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if paused == 0 {
            tx.rollback().await?;
            return Ok(RouteChangeOutcome::Unchanged);
        }
        let rows: Vec<(i64, Option<OffsetDateTime>)> = sqlx::query_as(
            "SELECT id, lease_until FROM integration_deliveries
              WHERE route_id = $1 AND status = 'pending' ORDER BY id",
        )
        .bind(change.route_id)
        .fetch_all(&mut *tx)
        .await?;
        let candidates: Vec<CancelCandidate> = rows
            .into_iter()
            .map(|(id, lease_until)| CancelCandidate {
                id,
                route_id: change.route_id,
                finished: false,
                lease_until,
            })
            .collect();
        let ids = plan(&candidates);
        let cancelled = sqlx::query(CANCEL_SQL)
            .bind(&ids)
            .bind(change.route_id)
            .bind(change.at)
            .bind("route_paused")
            .execute(&mut *tx)
            .await?
            .rows_affected();
        route_audit(&mut tx, change, "route_paused").await?;
        tx.commit().await?;
        Ok(RouteChangeOutcome::Changed { cancelled })
    }

    /// Resume a paused route, or one a refusal stopped, and clear its
    /// problem; one transaction opening with `lock_live_project`, with its
    /// `route_resumed` record. A paused route receives the events from this
    /// transaction's `now()` on, never what it skipped (DDR-27); a route only
    /// a refusal stopped keeps its waiting deliveries, which are sent now
    /// (D-8). A live route changes nothing.
    pub async fn resume_route(
        &self,
        change: &RouteChange,
    ) -> Result<RouteChangeOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        if !lock_live_project(&mut tx, change.project_id).await? {
            tx.rollback().await?;
            return Ok(RouteChangeOutcome::ProjectDeleted);
        }
        let resumed = sqlx::query(
            "UPDATE integration_routes
                SET paused_at = NULL, paused_by = NULL, problem_reason = NULL,
                    problem_at = NULL, updated_at = now(),
                    receives_from = CASE WHEN paused_at IS NULL THEN receives_from
                                         ELSE now() END
              WHERE id = $1 AND project_id = $2
                AND (paused_at IS NOT NULL OR problem_reason IS NOT NULL)",
        )
        .bind(change.route_id)
        .bind(change.project_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if resumed == 0 {
            tx.rollback().await?;
            return Ok(RouteChangeOutcome::Unchanged);
        }
        route_audit(&mut tx, change, "route_resumed").await?;
        tx.commit().await?;
        Ok(RouteChangeOutcome::Changed { cancelled: 0 })
    }

    /// Edit a route's events, content level and lane filter, one transaction
    /// opening with `lock_live_project`, with its `route_edited` record. The
    /// next event the dispatcher reads is judged by the edited route.
    pub async fn edit_route(
        &self,
        change: &RouteChange,
        edit: &RouteEdit,
    ) -> Result<RouteChangeOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        if !lock_live_project(&mut tx, change.project_id).await? {
            tx.rollback().await?;
            return Ok(RouteChangeOutcome::ProjectDeleted);
        }
        let edited = sqlx::query(
            "UPDATE integration_routes SET events = $3, content_level = $4, updated_at = now()
              WHERE id = $1 AND project_id = $2",
        )
        .bind(change.route_id)
        .bind(change.project_id)
        .bind(&edit.events)
        .bind(&edit.content_level)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if edited == 0 {
            tx.rollback().await?;
            return Ok(RouteChangeOutcome::Unchanged);
        }
        sqlx::query("DELETE FROM integration_route_lanes WHERE route_id = $1")
            .bind(change.route_id)
            .execute(&mut *tx)
            .await?;
        insert_route_lanes(&mut tx, change.route_id, &edit.lanes).await?;
        route_audit(&mut tx, change, "route_edited").await?;
        tx.commit().await?;
        Ok(RouteChangeOutcome::Changed { cancelled: 0 })
    }

    /// Delete a route with its deliveries (messages already sent stay where
    /// they are), one transaction opening with `lock_live_project`, with its
    /// `route_deleted` record. A route already gone changes nothing.
    pub async fn delete_route(
        &self,
        change: &RouteChange,
    ) -> Result<RouteChangeOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        if !lock_live_project(&mut tx, change.project_id).await? {
            tx.rollback().await?;
            return Ok(RouteChangeOutcome::ProjectDeleted);
        }
        let exists: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM integration_routes WHERE id = $1 AND project_id = $2 FOR UPDATE",
        )
        .bind(change.route_id)
        .bind(change.project_id)
        .fetch_optional(&mut *tx)
        .await?;
        if exists.is_none() {
            tx.rollback().await?;
            return Ok(RouteChangeOutcome::Unchanged);
        }
        route_audit(&mut tx, change, "route_deleted").await?;
        sqlx::query("DELETE FROM integration_routes WHERE id = $1")
            .bind(change.route_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(RouteChangeOutcome::Changed { cancelled: 0 })
    }

    /// Whether a route of the workspace's connection of `kind`, in a live
    /// project (paused or not), sends to `destination_ref`: a deleted Matrix
    /// route's bot leaves only a room no route uses. Reads only.
    pub async fn destination_routed(
        &self,
        workspace_id: Uuid,
        kind: &str,
        destination_ref: &str,
    ) -> Result<bool, StoreError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (
                 SELECT 1 FROM integration_routes r
                   JOIN integration_connections c ON c.id = r.connection_id
                   JOIN live_projects p ON p.id = r.project_id
                  WHERE c.workspace_id = $1 AND c.kind = $2 AND r.destination_ref = $3)",
        )
        .bind(workspace_id)
        .bind(kind)
        .bind(destination_ref)
        .fetch_one(&self.pool)
        .await?)
    }
}

/// A route's filter lanes, in the write's transaction.
async fn insert_route_lanes(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    route_id: Uuid,
    lanes: &[RouteLane],
) -> Result<(), StoreError> {
    for lane in lanes {
        sqlx::query(
            "INSERT INTO integration_route_lanes (route_id, lane_id, label_at_save)
             VALUES ($1, $2, $3)",
        )
        .bind(route_id)
        .bind(lane.lane_id)
        .bind(&lane.label_at_save)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// A route change's audit record, in the change's transaction.
async fn route_audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    change: &RouteChange,
    action: &str,
) -> Result<(), StoreError> {
    sqlx::query(ROUTE_AUDIT_SQL)
        .bind(Uuid::now_v7())
        .bind(change.actor_id)
        .bind(&change.via)
        .bind(action)
        .bind(change.at)
        .bind(change.route_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
