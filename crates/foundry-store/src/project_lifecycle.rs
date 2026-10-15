//! project-lifecycle — the one store module allowed to name the `projects`
//! base table once `live_projects` exists (DESIGN DDD-1, DDD-3;
//! ADR-PROJECT-LIFECYCLE-001 to -004).
//!
//! It holds every read and write of the base table: the team's sibling and
//! live-project reads, the export's `projects` entry, the readiness probe's
//! 0026 and 0027 checks, the audited rename, the soft delete, the operator's
//! list and restore, the live-project write guard every child writer takes
//! ([`lock_live_project`]), and the team delete's detach and
//! `team_deleted_projects`, whose fields `team_delete_facts` folds into
//! `TeamDeleteFacts` (DDD-20, DDD-21; ADR-PROJECT-LIFECYCLE-004 decision 2).
//!
//! The store gold tests in `tests/project_lifecycle.rs`, `tests/team_delete.rs`
//! and the use-case tests in `foundry-services` match on the row and outcome
//! types below.

use crate::{Store, StoreError};

/// The per-workspace export's `projects` entry, label and SQL together
/// (DDD-31, D13): every row of the workspace, a deleted one included and marked
/// by its own `deleted_at` / `deleted_by`.
pub(crate) const EXPORT_PROJECTS: (&str, &str) = (
    "projects",
    "SELECT to_jsonb(t.*)::text FROM projects t WHERE t.workspace_id = $1 ORDER BY 1",
);

/// A team's LIVE projects as `(name, key)`, by name then key: the team delete
/// confirm page's list and the delete's refusal (DDD-8, DDD-10, DDD-21). A
/// deleted project never blocks a team delete: the delete detaches it
/// ([`detach_team_projects`]).
pub(crate) const TEAM_PROJECTS_SQL: &str =
    "SELECT name, key_prefix FROM live_projects WHERE team_id = $1 ORDER BY name, key_prefix";

/// The readiness probe's migration-0026 check (DDD-28): the soft-delete
/// marker columns on `projects` and the `live_projects` view, scoped to the
/// current schema. Lives here because it names the base table.
pub(crate) const SOFT_DELETE_PROBE_SQL: &str = "SELECT (SELECT count(*)::bigint
                       FROM information_schema.columns
                      WHERE table_schema = current_schema()
                        AND table_name = 'projects'
                        AND column_name IN ('deleted_at', 'deleted_by')),
                    EXISTS (SELECT 1 FROM information_schema.views
                             WHERE table_schema = current_schema()
                               AND table_name = 'live_projects')";

/// The readiness probe's migration-0027 check (DDD-28): the detach column
/// `projects.detached_from_team_id`, scoped to the current schema. Lives here
/// because it names the base table.
pub(crate) const DETACH_PROBE_SQL: &str = "SELECT count(*)::bigint
                       FROM information_schema.columns
                      WHERE table_schema = current_schema()
                        AND table_name = 'projects'
                        AND column_name = 'detached_from_team_id'";

/// The outcome of [`Store::rename_project_with_audit`] (DDD-15,
/// ADR-PROJECT-LIFECYCLE-003 decision 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectRenameWrite {
    /// The name changed (never the slug or key) and one `renamed` record was
    /// written with `old_name`, the name read under the lock.
    Renamed { old_name: String },
    /// The new name is byte-equal to the stored one; nothing was written.
    Unchanged,
    /// No live project with that id (unknown, deleted, or deleted while the
    /// rename waited); nothing was written.
    NotFound,
}

/// One live project of a team, for the members page Projects section (DDD-25).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamProjectRow {
    pub name: String,
    pub slug: String,
    pub key: String,
}

/// The outcome of [`Store::soft_delete_project`] (DDD-6,
/// ADR-PROJECT-LIFECYCLE-002 decision 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectDeleteOutcome {
    /// The marker (`deleted_at`, `deleted_by`) is set and one `deleted` record
    /// was written; nothing else changed.
    Deleted { name: String, key_prefix: String },
    /// No live project with that id in that team and workspace (unknown,
    /// foreign, or already deleted); nothing was written.
    NotFound,
}

/// One deleted project, as `foundry doctor list-deleted-projects` prints it
/// (D10, D14; ADR-PROJECT-LIFECYCLE-002 decision 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedProjectRow {
    pub project_id: uuid::Uuid,
    pub workspace_name: String,
    /// The live team's slug, or for a detached project the slug of its team's
    /// `deleted` record.
    pub team_slug: String,
    pub name: String,
    pub key_prefix: String,
    pub issue_count: i64,
    pub deleted_at: time::OffsetDateTime,
    /// `users.email_lower` of `deleted_by`.
    pub deleted_by_email: String,
}

/// `list-deleted-projects`'s read (D10, D14): every deleted project with its
/// workspace's name, its team's slug, its issue count and the deleting user's
/// email, oldest deletion first, `id` breaking ties.
/// A detached project (NULL `team_id`) carries the slug of its team's
/// `deleted` record (DIST-3).
const DELETED_PROJECTS_SQL: &str = "SELECT p.id, w.name,
        coalesce(t.slug, (SELECT e.team_slug FROM team_lifecycle_events e
                           WHERE e.team_id = p.detached_from_team_id AND e.kind = 'deleted'
                           ORDER BY e.created_at DESC LIMIT 1), ''),
        p.name, p.key_prefix,
        (SELECT count(*) FROM issues i WHERE i.project_id = p.id),
        p.deleted_at, u.email_lower
   FROM projects p
   JOIN workspaces w ON w.id = p.workspace_id
   LEFT JOIN teams t ON t.id = p.team_id
   JOIN users u ON u.id = p.deleted_by
  WHERE p.deleted_at IS NOT NULL
  ORDER BY p.deleted_at, p.id";

type DeletedProjectTuple = (
    uuid::Uuid,
    String,
    String,
    String,
    String,
    i64,
    time::OffsetDateTime,
    String,
);

impl From<DeletedProjectTuple> for DeletedProjectRow {
    fn from(
        (project_id, workspace_name, team_slug, name, key_prefix, issue_count, deleted_at, email): DeletedProjectTuple,
    ) -> Self {
        Self {
            project_id,
            workspace_name,
            team_slug,
            name,
            key_prefix,
            issue_count,
            deleted_at,
            deleted_by_email: email,
        }
    }
}

/// The outcome of [`Store::restore_deleted_project`] (DDD-9,
/// ADR-PROJECT-LIFECYCLE-002 decision 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectRestoreOutcome {
    /// The marker is cleared and one `restored` record (actor NULL) was written.
    Restored,
    /// The project exists and is live; nothing was written.
    NotDeleted,
    /// No project with that id; nothing was written.
    NotFound,
    /// The project is detached: its team was deleted (OQ-1 (b)); nothing was
    /// written.
    TeamGone { team_slug: String },
}

/// The team delete page's extra facts (DDD-21): the workspace's name and the
/// team's deleted projects as `(name, key)`, by name then key. Read here
/// because the deleted rows are only on the base table; `team_delete_facts`
/// folds them into `TeamDeleteFacts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamDeletedProjects {
    pub workspace_name: String,
    pub deleted_projects: Vec<(String, String)>,
}

/// The live-project write guard (DDD-7, ADR-PROJECT-LIFECYCLE-002 decision 2;
/// DIST-10): `FOR KEY SHARE` on the project's row through `live_projects`,
/// taken inside the transaction that then performs the write, before its first
/// write. `true` while the project is live, and the row stays locked until the
/// transaction ends, so a delete (`FOR UPDATE`) waits for the write and a write
/// queued behind a delete sees no live row; `false` for an unknown or deleted
/// project. Writes nothing.
pub(crate) async fn lock_live_project(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: uuid::Uuid,
) -> Result<bool, sqlx::Error> {
    let live: Option<(i32,)> =
        sqlx::query_as("SELECT 1 FROM live_projects WHERE id = $1 FOR KEY SHARE")
            .bind(project_id)
            .fetch_optional(&mut **tx)
            .await?;
    Ok(live.is_some())
}

/// The team delete's detach (DDD-20, ADR-PROJECT-LIFECYCLE-004), inside the
/// delete's transaction, after the team `FOR UPDATE` and the live-project
/// check, before `DELETE FROM teams`: EVERY remaining row of the team loses its
/// team and names it in `detached_from_team_id`, so the FK cascade erases
/// none. No `deleted_at` filter: a live row slipping through is refused by the
/// CHECK `projects_team_or_detached` (23514, [`is_detach_backstop`]), loudly,
/// never cascaded silently.
pub(crate) async fn detach_team_projects(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    team_id: uuid::Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE projects SET team_id = NULL, detached_from_team_id = $1 WHERE team_id = $1",
    )
    .bind(team_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// The detach backstop refusing: SQLSTATE 23514 on `projects_team_or_detached`,
/// and only that one (a live project reached the detach: a broken invariant).
pub(crate) fn is_detach_backstop(err: &sqlx::Error) -> bool {
    err.as_database_error().is_some_and(|db| {
        db.code().as_deref() == Some("23514")
            && db.constraint() == Some("projects_team_or_detached")
    })
}

/// An issue-keyed writer's pre-read, inside its transaction and before its
/// guard: the issue's project (`issues.project_id` never changes), scoped to
/// the acting workspace. `None` for an unknown or foreign issue. Writes nothing.
pub(crate) async fn issue_project(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: uuid::Uuid,
    issue_id: uuid::Uuid,
) -> Result<Option<uuid::Uuid>, sqlx::Error> {
    sqlx::query_scalar("SELECT project_id FROM issues WHERE id = $1 AND workspace_id = $2")
        .bind(issue_id)
        .bind(workspace_id)
        .fetch_optional(&mut **tx)
        .await
}

impl Store {
    /// The D4 uniqueness comparison set: `(name, slug)` of every OTHER project
    /// in the same team (self excluded in SQL). Comparison happens app-side —
    /// slug derivation is domain code (`foundry_core::slugify`), not SQL.
    pub async fn list_team_sibling_projects(
        &self,
        team_id: uuid::Uuid,
        exclude_project_id: uuid::Uuid,
    ) -> Result<Vec<(String, String)>, StoreError> {
        Ok(
            sqlx::query_as("SELECT name, slug FROM projects WHERE team_id = $1 AND id <> $2")
                .bind(team_id)
                .bind(exclude_project_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    /// The team's live projects, by name then key (DDD-25).
    pub async fn list_team_projects(
        &self,
        team_id: uuid::Uuid,
    ) -> Result<Vec<TeamProjectRow>, StoreError> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT name, slug, key_prefix FROM live_projects WHERE team_id = $1
              ORDER BY name, key_prefix",
        )
        .bind(team_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(name, slug, key)| TeamProjectRow { name, slug, key })
            .collect())
    }

    /// ADR-PROJECT-LIFECYCLE-003 decision 2, in one transaction: the row
    /// `FOR NO KEY UPDATE` (so concurrent renames serialise and their records
    /// chain); a byte-equal name writes nothing; else `UPDATE … SET name`
    /// (never slug or key) and the `renamed` record (`clock_timestamp()`).
    /// The locking read skips a deleted project (ADR-PROJECT-LIFECYCLE-003
    /// section 2.1): unknown, deleted, or deleted while the rename waited on
    /// the delete's row lock all find no row and answer `NotFound`.
    pub async fn rename_project_with_audit(
        &self,
        project_id: uuid::Uuid,
        actor_id: uuid::Uuid,
        new_name: &str,
    ) -> Result<ProjectRenameWrite, StoreError> {
        let mut tx = self.pool.begin().await?;
        let locked: Option<(uuid::Uuid, uuid::Uuid, String, String, String)> = sqlx::query_as(
            "SELECT workspace_id, team_id, slug, key_prefix, name
               FROM projects WHERE id = $1 AND deleted_at IS NULL FOR NO KEY UPDATE",
        )
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((workspace_id, team_id, slug, key_prefix, old_name)) = locked else {
            tx.rollback().await?;
            return Ok(ProjectRenameWrite::NotFound);
        };
        if old_name == new_name {
            tx.rollback().await?;
            return Ok(ProjectRenameWrite::Unchanged);
        }
        sqlx::query("UPDATE projects SET name = $2 WHERE id = $1")
            .bind(project_id)
            .bind(new_name)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO project_lifecycle_events
                    (id, workspace_id, project_id, team_id, project_slug, key_prefix, kind,
                     actor_id, name, new_name, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, 'renamed', $7, $8, $9, clock_timestamp())",
        )
        .bind(uuid::Uuid::now_v7())
        .bind(workspace_id)
        .bind(project_id)
        .bind(team_id)
        .bind(&slug)
        .bind(&key_prefix)
        .bind(actor_id)
        .bind(&old_name)
        .bind(new_name)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(ProjectRenameWrite::Renamed { old_name })
    }

    /// ADR-PROJECT-LIFECYCLE-002 decision 1, in one transaction: the live row
    /// scoped to workspace and team `FOR UPDATE` (the only row lock that
    /// conflicts with a writer's `FOR KEY SHARE` guard, so the delete waits for
    /// an in-flight write and a write queued behind it finds no live row); set
    /// the marker; the `deleted` record, its `created_at` the marker's
    /// `deleted_at` (one clock read). Nothing else is written: no child row, no
    /// outbox row (D17). Unknown, foreign or already deleted: `NotFound`.
    pub async fn soft_delete_project(
        &self,
        workspace_id: uuid::Uuid,
        team_id: uuid::Uuid,
        project_id: uuid::Uuid,
        actor_id: uuid::Uuid,
    ) -> Result<ProjectDeleteOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        let locked: Option<(String, String, String)> = sqlx::query_as(
            "SELECT slug, key_prefix, name FROM live_projects
              WHERE id = $1 AND workspace_id = $2 AND team_id = $3 FOR UPDATE",
        )
        .bind(project_id)
        .bind(workspace_id)
        .bind(team_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((slug, key_prefix, name)) = locked else {
            tx.rollback().await?;
            return Ok(ProjectDeleteOutcome::NotFound);
        };
        let deleted_at: time::OffsetDateTime = sqlx::query_scalar(
            "UPDATE projects SET deleted_at = clock_timestamp(), deleted_by = $2
              WHERE id = $1 RETURNING deleted_at",
        )
        .bind(project_id)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO project_lifecycle_events
                    (id, workspace_id, project_id, team_id, project_slug, key_prefix, kind,
                     actor_id, name, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, 'deleted', $7, $8, $9)",
        )
        .bind(uuid::Uuid::now_v7())
        .bind(workspace_id)
        .bind(project_id)
        .bind(team_id)
        .bind(&slug)
        .bind(&key_prefix)
        .bind(actor_id)
        .bind(&name)
        .bind(deleted_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(ProjectDeleteOutcome::Deleted { name, key_prefix })
    }

    /// Every deleted project, instance-wide, oldest deletion first (`deleted_at`
    /// then `id`), with its workspace's name, its team's slug, its issue count
    /// and the deleting user's email (D10, D14). Read with no lock; writes
    /// nothing.
    pub async fn list_deleted_projects(&self) -> Result<Vec<DeletedProjectRow>, StoreError> {
        let rows: Vec<DeletedProjectTuple> = sqlx::query_as(DELETED_PROJECTS_SQL)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(DeletedProjectRow::from).collect())
    }

    /// ADR-PROJECT-LIFECYCLE-002 decision 4, DDD-8, DDD-9: an unlocked read
    /// (unknown: `NotFound`; live: `NotDeleted`); then, in one transaction, the
    /// team `FOR KEY SHARE` (gone: `TeamGone`) BEFORE the project `FOR UPDATE`,
    /// which re-checks the project is still deleted and in the same team (a
    /// concurrent restore that won answers `NotDeleted`); clear the marker; the
    /// `restored` record (actor NULL: the operator; `clock_timestamp()`).
    /// Clearing the marker is the whole restore (Changed Assumption 3).
    pub async fn restore_deleted_project(
        &self,
        project_id: uuid::Uuid,
    ) -> Result<ProjectRestoreOutcome, StoreError> {
        type Seen = (Option<uuid::Uuid>, Option<uuid::Uuid>, bool);
        let seen: Option<Seen> = sqlx::query_as(
            "SELECT team_id, detached_from_team_id, deleted_at IS NOT NULL
               FROM projects WHERE id = $1",
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        let team_id = match seen {
            None => return Ok(ProjectRestoreOutcome::NotFound),
            Some((_, _, false)) => return Ok(ProjectRestoreOutcome::NotDeleted),
            Some((Some(team_id), _, true)) => team_id,
            Some((None, detached_from, true)) => {
                return self.team_gone(detached_from.unwrap_or_default()).await
            }
        };
        let mut tx = self.pool.begin().await?;
        let team: Option<(i32,)> =
            sqlx::query_as("SELECT 1 FROM teams WHERE id = $1 FOR KEY SHARE")
                .bind(team_id)
                .fetch_optional(&mut *tx)
                .await?;
        if team.is_none() {
            tx.rollback().await?;
            return self.team_gone(team_id).await;
        }
        let locked: Option<(uuid::Uuid, Option<uuid::Uuid>, String, String, String)> =
            sqlx::query_as(
                "SELECT workspace_id, team_id, slug, key_prefix, name
               FROM projects WHERE id = $1 AND deleted_at IS NOT NULL FOR UPDATE",
            )
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await?;
        let Some((workspace_id, locked_team_id, slug, key_prefix, name)) = locked else {
            tx.rollback().await?;
            return Ok(ProjectRestoreOutcome::NotDeleted);
        };
        if locked_team_id != Some(team_id) {
            tx.rollback().await?;
            return self.team_gone(team_id).await;
        }
        sqlx::query("UPDATE projects SET deleted_at = NULL, deleted_by = NULL WHERE id = $1")
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO project_lifecycle_events
                    (id, workspace_id, project_id, team_id, project_slug, key_prefix, kind,
                     actor_id, name, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, 'restored', NULL, $7, clock_timestamp())",
        )
        .bind(uuid::Uuid::now_v7())
        .bind(workspace_id)
        .bind(project_id)
        .bind(team_id)
        .bind(&slug)
        .bind(&key_prefix)
        .bind(&name)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(ProjectRestoreOutcome::Restored)
    }

    /// A restore refused because the project's team is gone, naming the team
    /// by the slug its `deleted` record carries (DIST-2).
    async fn team_gone(&self, team_id: uuid::Uuid) -> Result<ProjectRestoreOutcome, StoreError> {
        Ok(ProjectRestoreOutcome::TeamGone {
            team_slug: self.deleted_team_slug(team_id).await?,
        })
    }

    /// The slug a gone team's `deleted` record carries (DIST-2); empty when
    /// no record names it.
    async fn deleted_team_slug(&self, team_id: uuid::Uuid) -> Result<String, StoreError> {
        let slug: Option<String> = sqlx::query_scalar(
            "SELECT team_slug FROM team_lifecycle_events
              WHERE team_id = $1 AND kind = 'deleted'
              ORDER BY created_at DESC LIMIT 1",
        )
        .bind(team_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(slug.unwrap_or_default())
    }

    /// The team delete page's deleted-projects line (DDD-21): the workspace's
    /// name (empty for an unknown team) and the team's deleted projects, by
    /// name then key. Read with no lock (advisory); writes nothing.
    pub async fn team_deleted_projects(
        &self,
        team_id: uuid::Uuid,
    ) -> Result<TeamDeletedProjects, StoreError> {
        let workspace_name: Option<String> = sqlx::query_scalar(
            "SELECT w.name FROM teams t JOIN workspaces w ON w.id = t.workspace_id
              WHERE t.id = $1",
        )
        .bind(team_id)
        .fetch_optional(&self.pool)
        .await?;
        let deleted_projects = sqlx::query_as(
            "SELECT name, key_prefix FROM projects
              WHERE team_id = $1 AND deleted_at IS NOT NULL
              ORDER BY name, key_prefix",
        )
        .bind(team_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(TeamDeletedProjects {
            workspace_name: workspace_name.unwrap_or_default(),
            deleted_projects,
        })
    }
}
