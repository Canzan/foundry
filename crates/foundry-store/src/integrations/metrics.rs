//! The delivery counters' tallies and the stall and health signals (DDR-22,
//! U-D7).

use crate::{Store, StoreError};
use time::OffsetDateTime;

/// What the connection and route gauges read (U-D8): `(kind, state, count)`
/// rows; a route's state is `active` or `paused`, and routes of deleted
/// projects are not counted.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IntegrationCensus {
    pub connections: Vec<(String, String, i64)>,
    pub routes: Vec<(String, String, i64)>,
}

/// A delivery as the delivery counters count it (DDR-22), read in the same
/// statement that recorded or failed it: its kind (`email` for a recipient's),
/// its event, its status after the write, and its lag `now() - occurred_at`
/// on the database's clock, in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct DeliveryTally {
    pub kind: String,
    pub event_kind: String,
    pub status: String,
    pub lag_seconds: f64,
}

/// What the stall gauges read (DDR-22). Ages are seconds, 0 when none.
#[derive(Debug, Clone, PartialEq)]
pub struct IntegrationBacklog {
    pub undispatched: i64,
    pub undispatched_oldest_seconds: f64,
    /// Pending deliveries per kind (`slack`, `matrix`, `email`); a kind with
    /// none is absent.
    pub pending_by_kind: Vec<(String, i64)>,
    pub pending_oldest_seconds: f64,
}

/// What a write of a delivery row returns for the delivery counters.
pub(super) const TALLY_COLUMNS: &str = "
       COALESCE((SELECT r.kind FROM integration_routes r
                  WHERE r.id = integration_deliveries.route_id), 'email'),
       event_kind, status, EXTRACT(EPOCH FROM now() - occurred_at)::float8";

/// [`TALLY_COLUMNS`] as Postgres answers them.
pub(super) type TallyRow = (String, String, String, f64);

impl Store {
    /// A worker component's heartbeat (DDR-22): `role` on `replica` was alive
    /// at `at`.
    pub async fn beat_worker(
        &self,
        role: &str,
        replica: &str,
        at: OffsetDateTime,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO integration_worker_heartbeats (role, replica, beat_at)
             VALUES ($1, $2, $3)
             ON CONFLICT (role, replica) DO UPDATE SET beat_at = EXCLUDED.beat_at",
        )
        .bind(role)
        .bind(replica)
        .bind(at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The connection and route gauges' facts (U-D8): connections by kind and
    /// state, and live projects' routes by kind and whether they are paused.
    pub async fn integration_census(&self) -> Result<IntegrationCensus, StoreError> {
        let connections = sqlx::query_as(
            "SELECT kind, state, count(*) FROM integration_connections GROUP BY 1, 2",
        )
        .fetch_all(&self.pool)
        .await?;
        let routes = sqlx::query_as(
            "SELECT r.kind, CASE WHEN r.paused_at IS NULL THEN 'active' ELSE 'paused' END,
                    count(*)
               FROM integration_routes r
               JOIN live_projects p ON p.id = r.project_id
              GROUP BY 1, 2",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(IntegrationCensus {
            connections,
            routes,
        })
    }

    /// The stall gauges' facts (DDR-22), on the database's clock: the outbox
    /// rows the dispatcher has yet to read and the age of the oldest, and the
    /// pending deliveries by kind and the age of the oldest.
    pub async fn integration_backlog(&self) -> Result<IntegrationBacklog, StoreError> {
        let (undispatched, undispatched_oldest_seconds): (i64, f64) = sqlx::query_as(
            "SELECT count(*), COALESCE(EXTRACT(EPOCH FROM now() - min(o.created_at)), 0)::float8
               FROM outbox o, integration_dispatch_start s
              WHERE o.dispatched_at IS NULL AND o.event_id IS NOT NULL
                AND o.id > s.outbox_id",
        )
        .fetch_one(&self.pool)
        .await?;
        let pending: Vec<(String, i64, f64)> = sqlx::query_as(
            "SELECT COALESCE(r.kind, 'email'), count(*),
                    EXTRACT(EPOCH FROM now() - min(d.occurred_at))::float8
               FROM integration_deliveries d
               LEFT JOIN integration_routes r ON r.id = d.route_id
              WHERE d.status = 'pending'
              GROUP BY 1",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(IntegrationBacklog {
            undispatched,
            undispatched_oldest_seconds,
            pending_oldest_seconds: pending.iter().map(|p| p.2).fold(0.0, f64::max),
            pending_by_kind: pending.into_iter().map(|(k, n, _)| (k, n)).collect(),
        })
    }

    /// U-D7: every INVALID index in the active schema (a failed
    /// `CREATE INDEX CONCURRENTLY`, e.g. 0029's), as one boot warning each,
    /// naming the remedy (runbook R-7). `Store::migrate` logs them.
    pub async fn invalid_index_warnings(&self) -> Result<Vec<String>, StoreError> {
        let names: Vec<String> = sqlx::query_scalar(
            "SELECT c.relname::text FROM pg_index i
               JOIN pg_class c ON c.oid = i.indexrelid
               JOIN pg_namespace n ON n.oid = c.relnamespace
              WHERE n.nspname = current_schema() AND NOT i.indisvalid
              ORDER BY 1",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(names
            .into_iter()
            .map(|name| {
                format!(
                    "store.index.invalid index={name}: a CREATE INDEX CONCURRENTLY failed and \
                     left it INVALID, and the migration that creates it cannot apply over it; \
                     run DROP INDEX CONCURRENTLY {name}; then restart (runbook R-7)"
                )
            })
            .collect())
    }
}

pub(super) fn delivery_tally(row: TallyRow) -> DeliveryTally {
    let (kind, event_kind, status, lag_seconds) = row;
    DeliveryTally {
        kind,
        event_kind,
        status,
        lag_seconds,
    }
}
