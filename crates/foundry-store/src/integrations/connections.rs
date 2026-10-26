//! A workspace's connections: connect, disconnect, the workspace page's reads
//! and the sealed credential's reads (ADR-EIP-005 decision 4, US-EIP-07, R-9).

use super::routes::CancelCandidate;
use super::StoredKind;
use crate::project_lifecycle::lock_live_project;
use crate::{Store, StoreError};
use time::OffsetDateTime;
use uuid::Uuid;

/// Who changes a workspace's connection of `kind`, and when (the injected
/// clock's time: the audit record's date, and the "now" a lease is judged
/// against).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionChange {
    pub workspace_id: Uuid,
    pub kind: String,
    pub actor_id: Uuid,
    pub via: String,
    pub at: OffsetDateTime,
}

/// What a disconnect stops (US-EIP-07): the connection's label and its
/// routes that post now (not paused, in a live project), and their projects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisconnectPreview {
    pub label: String,
    pub routes: i64,
    pub projects: i64,
}

/// What a disconnect did.
#[derive(Clone, PartialEq, Eq)]
pub enum DisconnectOutcome {
    /// The connection is disconnected and holds no credential; the credential
    /// it held, still sealed for the row `connection_id`, is handed back once
    /// so the vendor can be asked to withdraw it.
    Disconnected {
        connection_id: Uuid,
        sealed_credential: Vec<u8>,
        cancelled: u64,
    },
    /// No connection of the kind, or it already was disconnected.
    Unchanged,
}

impl std::fmt::Debug for DisconnectOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected {
                connection_id,
                cancelled,
                ..
            } => f
                .debug_struct("Disconnected")
                .field("connection_id", connection_id)
                .field("cancelled", cancelled)
                .finish_non_exhaustive(),
            Self::Unchanged => f.write_str("Unchanged"),
        }
    }
}

/// A disconnect's compare-and-set on the plan's ids: still a pending
/// delivery of one of the connection's routes, holding no live lease at `$3`.
const DISCONNECT_CANCEL_SQL: &str = "
    UPDATE integration_deliveries d
       SET status = 'skipped', final_reason = 'route_paused', finished_at = $3,
           next_attempt_at = NULL, lease_until = NULL
      FROM integration_routes r
     WHERE d.id = ANY($1) AND r.id = d.route_id AND r.connection_id = $2
       AND d.status = 'pending' AND (d.lease_until IS NULL OR d.lease_until <= $3)";

/// The routes a disconnect stops: not paused, in a live project.
const DISCONNECT_PREVIEW_SQL: &str = "
    SELECT c.label, count(r.id), count(DISTINCT r.project_id)
      FROM integration_connections c
      LEFT JOIN integration_routes r ON r.connection_id = c.id AND r.paused_at IS NULL
            AND EXISTS (SELECT 1 FROM live_projects p WHERE p.id = r.project_id)
     WHERE c.workspace_id = $1 AND c.kind = $2 AND c.state <> 'disconnected'
     GROUP BY c.id, c.label";

/// A connection as the connect use-case saves it (ADR-EIP-005 decision 4).
/// `sealed_credential` was sealed for the row `id` (row-bound AAD), so the
/// write refuses a different row.
pub struct NewConnection {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub kind: String,
    pub external_id: String,
    pub app_id: Option<String>,
    pub bot_user_id: Option<String>,
    pub label: String,
    pub detail: Option<String>,
    pub sealed_credential: Vec<u8>,
    pub connected_by: Uuid,
    pub connected_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionWriteOutcome {
    /// No connection of the kind existed: one row and its audit row.
    Connected,
    /// The workspace's connection of the kind was replaced in place.
    Reconnected,
    /// The workspace's connection of the kind is another row than the one the
    /// credential was sealed for; nothing was written.
    StaleRow,
    /// The workspace's connection of the kind is to another vendor workspace
    /// (`external_id`) than the one just approved, labelled
    /// `connected_label`; nothing was written (US-EIP-06: reconnecting to a
    /// different Slack workspace changes nothing).
    OtherWorkspace { connected_label: String },
}

/// A workspace's connection as its pages show it. Never the credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionRow {
    pub kind: String,
    pub state: String,
    /// Why a lost connection was lost (`lost_reason`).
    pub lost_reason: Option<String>,
    pub workspace_name: String,
    pub label: String,
    pub detail: Option<String>,
    pub connected_by_name: Option<String>,
    pub connected_at: OffsetDateTime,
}

/// One integration audit record as the activity list shows it (DDR-21).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRow {
    pub created_at: OffsetDateTime,
    pub actor_name: Option<String>,
    pub action: String,
    pub kind: String,
    pub subject: String,
}

/// A workspace's connection of one kind with its sealed credential, for the
/// adapters that open it (route form, Send test, route save).
#[derive(Clone, PartialEq, Eq)]
pub struct SealedConnection {
    pub id: Uuid,
    pub state: String,
    /// The vendor's id for what is connected (a Slack team id, a Matrix
    /// homeserver's address).
    pub external_id: String,
    /// The bot's vendor id (a Slack bot user, a Matrix user id).
    pub bot_user_id: Option<String>,
    /// `None` only for a disconnected connection.
    pub sealed_credential: Option<Vec<u8>>,
}

impl std::fmt::Debug for SealedConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealedConnection")
            .field("id", &self.id)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

/// `workspace_connections`' row as Postgres answers it.
type SealedConnectionTuple = (Uuid, String, String, Option<String>, Option<Vec<u8>>);
type ConnectionTuple = (
    StoredKind,
    String,
    Option<String>,
    String,
    String,
    Option<String>,
    Option<String>,
    OffsetDateTime,
);

impl Store {
    /// The id of the workspace's connection of `kind`, if one exists.
    pub async fn connection_id(
        &self,
        workspace_id: Uuid,
        kind: &str,
    ) -> Result<Option<Uuid>, StoreError> {
        Ok(sqlx::query_scalar(
            "SELECT id FROM integration_connections WHERE workspace_id = $1 AND kind = $2",
        )
        .bind(workspace_id)
        .bind(kind)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Save a connection and its audit record in one transaction: insert the
    /// workspace's first connection of the kind, or replace the existing row
    /// in place when it is the row the credential was sealed for.
    pub async fn upsert_connection(
        &self,
        connection: &NewConnection,
    ) -> Result<ConnectionWriteOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        let existing: Option<(Uuid, String, String, String)> = sqlx::query_as(
            "SELECT id, external_id, label, state FROM integration_connections
              WHERE workspace_id = $1 AND kind = $2 FOR UPDATE",
        )
        .bind(connection.workspace_id)
        .bind(&connection.kind)
        .fetch_optional(&mut *tx)
        .await?;
        let was_disconnected =
            matches!(&existing, Some((_, _, _, state)) if state == "disconnected");
        let refused = match &existing {
            Some((id, ..)) if *id != connection.id => Some(ConnectionWriteOutcome::StaleRow),
            Some((_, external_id, label, _))
                if *external_id != connection.external_id && !was_disconnected =>
            {
                Some(ConnectionWriteOutcome::OtherWorkspace {
                    connected_label: label.clone(),
                })
            }
            _ => None,
        };
        if let Some(refused) = refused {
            tx.rollback().await?;
            return Ok(refused);
        }
        let outcome = if existing.is_some() {
            ConnectionWriteOutcome::Reconnected
        } else {
            ConnectionWriteOutcome::Connected
        };
        sqlx::query(
            "INSERT INTO integration_connections
                    (id, workspace_id, kind, state, external_id, app_id, bot_user_id, label,
                     detail, sealed_credential, credential_saved_at, connected_by, connected_at)
             VALUES ($1, $2, $3, 'connected', $4, $5, $6, $7, $8, $9, $10, $11, $10)
             ON CONFLICT (workspace_id, kind) DO UPDATE
                SET state = 'connected', external_id = EXCLUDED.external_id,
                    app_id = EXCLUDED.app_id, bot_user_id = EXCLUDED.bot_user_id,
                    label = EXCLUDED.label, detail = EXCLUDED.detail,
                    sealed_credential = EXCLUDED.sealed_credential,
                    credential_saved_at = EXCLUDED.credential_saved_at,
                    lost_reason = NULL, lost_at = NULL,
                    connected_by = EXCLUDED.connected_by, connected_at = EXCLUDED.connected_at",
        )
        .bind(connection.id)
        .bind(connection.workspace_id)
        .bind(&connection.kind)
        .bind(&connection.external_id)
        .bind(&connection.app_id)
        .bind(&connection.bot_user_id)
        .bind(&connection.label)
        .bind(&connection.detail)
        .bind(&connection.sealed_credential)
        .bind(connection.connected_at)
        .bind(connection.connected_by)
        .execute(&mut *tx)
        .await?;
        let action = match outcome {
            ConnectionWriteOutcome::Reconnected => "connection_reconnected",
            _ => "connection_connected",
        };
        sqlx::query(
            "INSERT INTO integration_audit_events
                    (id, workspace_id, project_id, actor_id, via, action, kind, subject, created_at)
             VALUES ($1, $2, NULL, $3, 'web', $4, $5, $6, $7)",
        )
        .bind(Uuid::now_v7())
        .bind(connection.workspace_id)
        .bind(connection.connected_by)
        .bind(action)
        .bind(&connection.kind)
        .bind(&connection.label)
        .bind(connection.connected_at)
        .execute(&mut *tx)
        .await?;
        if was_disconnected {
            routes_receive_from_now(&mut tx, connection.id).await?;
        }
        tx.commit().await?;
        Ok(outcome)
    }

    /// What disconnecting the workspace's connection of `kind` stops; `None`
    /// when it has none, or it already is disconnected. Reads only.
    pub async fn disconnect_preview(
        &self,
        workspace_id: Uuid,
        kind: &str,
    ) -> Result<Option<DisconnectPreview>, StoreError> {
        let row: Option<(String, i64, i64)> = sqlx::query_as(DISCONNECT_PREVIEW_SQL)
            .bind(workspace_id)
            .bind(kind)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|(label, routes, projects)| DisconnectPreview {
            label,
            routes,
            projects,
        }))
    }

    /// Disconnect the workspace's connection of `change.kind` (US-EIP-07,
    /// R-9), one transaction: the connection `FOR UPDATE`; the pending
    /// deliveries of its routes, of which exactly the ids `plan` names that
    /// still are pending and hold no live lease at `change.at` are skipped;
    /// the connection `disconnected`, its credential forgotten; its
    /// `connection_disconnected` record. Its routes read "Paused: … was
    /// disconnected" and nothing is dispatched to them until a reconnect,
    /// which receives from then on. No connection, or one already
    /// disconnected, changes nothing.
    pub async fn disconnect_connection<P>(
        &self,
        change: &ConnectionChange,
        plan: P,
    ) -> Result<DisconnectOutcome, StoreError>
    where
        P: FnOnce(&[CancelCandidate]) -> Vec<i64>,
    {
        let mut tx = self.pool.begin().await?;
        let held: Option<(Uuid, Option<Vec<u8>>)> = sqlx::query_as(
            "SELECT id, sealed_credential FROM integration_connections
              WHERE workspace_id = $1 AND kind = $2 AND state <> 'disconnected' FOR UPDATE",
        )
        .bind(change.workspace_id)
        .bind(&change.kind)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((connection_id, Some(sealed_credential))) = held else {
            tx.rollback().await?;
            return Ok(DisconnectOutcome::Unchanged);
        };
        let rows: Vec<(i64, Uuid, Option<OffsetDateTime>)> = sqlx::query_as(
            "SELECT d.id, d.route_id, d.lease_until
               FROM integration_deliveries d JOIN integration_routes r ON r.id = d.route_id
              WHERE r.connection_id = $1 AND d.status = 'pending' ORDER BY d.id",
        )
        .bind(connection_id)
        .fetch_all(&mut *tx)
        .await?;
        let candidates: Vec<CancelCandidate> = rows
            .into_iter()
            .map(|(id, route_id, lease_until)| CancelCandidate {
                id,
                route_id,
                finished: false,
                lease_until,
            })
            .collect();
        let ids = plan(&candidates);
        let cancelled = sqlx::query(DISCONNECT_CANCEL_SQL)
            .bind(&ids)
            .bind(connection_id)
            .bind(change.at)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        sqlx::query(
            "UPDATE integration_connections
                SET state = 'disconnected', sealed_credential = NULL, credential_saved_at = NULL,
                    lost_reason = NULL, lost_at = NULL
              WHERE id = $1",
        )
        .bind(connection_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO integration_audit_events
                    (id, workspace_id, project_id, actor_id, via, action, kind, subject, created_at)
             SELECT $1, c.workspace_id, NULL, $2, $3, 'connection_disconnected', c.kind, c.label, $4
               FROM integration_connections c WHERE c.id = $5",
        )
        .bind(Uuid::now_v7())
        .bind(change.actor_id)
        .bind(&change.via)
        .bind(change.at)
        .bind(connection_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(DisconnectOutcome::Disconnected {
            connection_id,
            sealed_credential,
            cancelled,
        })
    }

    /// The workspace's connections, oldest kind first. Never the credential.
    pub async fn workspace_connections(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<ConnectionRow>, StoreError> {
        let rows: Vec<ConnectionTuple> = sqlx::query_as(
            "SELECT c.kind, c.state, c.lost_reason, w.name, c.label, c.detail, u.display_name,
                        c.connected_at
                   FROM integration_connections c
                   JOIN workspaces w ON w.id = c.workspace_id
                   LEFT JOIN users u ON u.id = c.connected_by
                  WHERE c.workspace_id = $1
                  ORDER BY c.kind",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(
                    kind,
                    state,
                    lost_reason,
                    workspace_name,
                    label,
                    detail,
                    connected_by_name,
                    connected_at,
                )| ConnectionRow {
                    kind: kind.wire(),
                    state,
                    lost_reason,
                    workspace_name,
                    label,
                    detail,
                    connected_by_name,
                    connected_at,
                },
            )
            .collect())
    }

    /// The workspace's latest integration audit records, newest first.
    pub async fn integration_activity(
        &self,
        workspace_id: Uuid,
        limit: i64,
    ) -> Result<Vec<AuditRow>, StoreError> {
        let rows: Vec<(OffsetDateTime, Option<String>, String, String, String)> = sqlx::query_as(
            "SELECT a.created_at, u.display_name, a.action, a.kind, a.subject
               FROM integration_audit_events a
               LEFT JOIN users u ON u.id = a.actor_id
              WHERE a.workspace_id = $1
              ORDER BY a.created_at DESC, a.id DESC
              LIMIT $2",
        )
        .bind(workspace_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(created_at, actor_name, action, kind, subject)| AuditRow {
                created_at,
                actor_name,
                action,
                kind,
                subject,
            })
            .collect())
    }

    /// The workspace's connection of `kind` with its sealed credential.
    pub async fn sealed_connection(
        &self,
        workspace_id: Uuid,
        kind: &str,
    ) -> Result<Option<SealedConnection>, StoreError> {
        let row: Option<SealedConnectionTuple> = sqlx::query_as(
            "SELECT id, state, external_id, bot_user_id, sealed_credential
               FROM integration_connections
              WHERE workspace_id = $1 AND kind = $2",
        )
        .bind(workspace_id)
        .bind(kind)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(
            |(id, state, external_id, bot_user_id, sealed_credential)| SealedConnection {
                id,
                state,
                external_id,
                bot_user_id,
                sealed_credential,
            },
        ))
    }

    /// The vendor id of the connection `connection_id` (a Matrix
    /// connection's homeserver address); `None` when there is no such row.
    pub async fn connection_external_id(
        &self,
        connection_id: Uuid,
    ) -> Result<Option<String>, StoreError> {
        Ok(
            sqlx::query_scalar("SELECT external_id FROM integration_connections WHERE id = $1")
                .bind(connection_id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    /// The sealed credential of the connection `connection_id` (none once
    /// disconnected).
    pub async fn sealed_credential(
        &self,
        connection_id: Uuid,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(sqlx::query_scalar::<_, Option<Vec<u8>>>(
            "SELECT sealed_credential FROM integration_connections WHERE id = $1",
        )
        .bind(connection_id)
        .fetch_optional(&self.pool)
        .await?
        .flatten())
    }
}

/// A connection reconnected after a disconnect: each of its routes in a live
/// project receives the events from this transaction's `now()` on, so
/// nothing that happened while it was disconnected is sent (R-9). A deleted
/// project's routes are left as they are (`lock_live_project`, one project
/// at a time).
async fn routes_receive_from_now(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    connection_id: Uuid,
) -> Result<(), StoreError> {
    let projects: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT project_id FROM integration_routes
          WHERE connection_id = $1 ORDER BY project_id",
    )
    .bind(connection_id)
    .fetch_all(&mut **tx)
    .await?;
    for project_id in projects {
        if lock_live_project(tx, project_id).await? {
            sqlx::query(
                "UPDATE integration_routes SET receives_from = now(), updated_at = now()
                  WHERE connection_id = $1 AND project_id = $2",
            )
            .bind(connection_id)
            .bind(project_id)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}
