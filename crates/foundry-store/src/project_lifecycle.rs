//! project-lifecycle — the one store module allowed to name the `projects`
//! base table once `live_projects` exists (DESIGN DDD-1, DDD-3;
//! ADR-PROJECT-LIFECYCLE-001 to -004).
//!
//! SCAFFOLD: true — RED scaffold written by DISTILL (2026-10-13). The row and
//! outcome types are real (the store gold tests in `tests/project_lifecycle.rs`,
//! `tests/team_delete.rs` and the use-case tests in `foundry-services` match on
//! them); every method not yet shipped panics until its DELIVER step.
//! Shipped (01-02): `list_team_sibling_projects` (moved in unchanged),
//! `list_team_projects` and `rename_project_with_audit`. Signatures are
//! DISTILL's working choice within what DESIGN pinned (names, arguments where
//! the ADRs give them, and outcomes): if DELIVER changes one, it changes the
//! gold tests with it.
//!
//! DELIVER also moves the export's `projects` entry and `TEAM_PROJECTS_SQL`
//! into this module (DDD-31, DDD-20), and folds [`TeamDeletedProjects`] into
//! `TeamDeleteFacts` (ADR-PROJECT-LIFECYCLE-004 decision 2).

use crate::{Store, StoreError};

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
/// team's deleted projects as `(name, key)`, by name then key. DELIVER folds
/// these into `TeamDeleteFacts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamDeletedProjects {
    pub workspace_name: String,
    pub deleted_projects: Vec<(String, String)>,
}

/// The live-project write guard (DDD-7, ADR-PROJECT-LIFECYCLE-002 decision 2):
/// `SELECT 1 FROM projects WHERE id = $1 AND deleted_at IS NULL FOR KEY SHARE`,
/// the first statement of every project-scoped writer's transaction.
#[allow(dead_code)]
pub(crate) async fn lock_live_project(
    _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    _project_id: uuid::Uuid,
) -> Result<bool, sqlx::Error> {
    panic!("Not yet implemented -- RED scaffold (project-lifecycle 02-01: lock_live_project)")
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

    /// The team's live projects, by name then key (DDD-25). Reads `projects`
    /// until 0026, then `live_projects`.
    pub async fn list_team_projects(
        &self,
        team_id: uuid::Uuid,
    ) -> Result<Vec<TeamProjectRow>, StoreError> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT name, slug, key_prefix FROM projects WHERE team_id = $1
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
    /// Until 0026 there is no deleted marker to exclude; 02-02 adds it.
    pub async fn rename_project_with_audit(
        &self,
        project_id: uuid::Uuid,
        actor_id: uuid::Uuid,
        new_name: &str,
    ) -> Result<ProjectRenameWrite, StoreError> {
        let mut tx = self.pool.begin().await?;
        let locked: Option<(uuid::Uuid, uuid::Uuid, String, String, String)> = sqlx::query_as(
            "SELECT workspace_id, team_id, slug, key_prefix, name
               FROM projects WHERE id = $1 FOR NO KEY UPDATE",
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
    /// scoped to workspace and team `FOR UPDATE`; set the marker; the `deleted`
    /// record with `created_at` equal to `deleted_at`.
    pub async fn soft_delete_project(
        &self,
        _workspace_id: uuid::Uuid,
        _team_id: uuid::Uuid,
        _project_id: uuid::Uuid,
        _actor_id: uuid::Uuid,
    ) -> Result<ProjectDeleteOutcome, StoreError> {
        panic!("Not yet implemented -- RED scaffold (project-lifecycle 02-03: soft_delete_project)")
    }

    /// Every deleted project, instance-wide, oldest deletion first (`deleted_at`
    /// then `id`).
    pub async fn list_deleted_projects(&self) -> Result<Vec<DeletedProjectRow>, StoreError> {
        panic!(
            "Not yet implemented -- RED scaffold (project-lifecycle 03-01: list_deleted_projects)"
        )
    }

    /// ADR-PROJECT-LIFECYCLE-002 decision 4: unlocked read; team `FOR KEY
    /// SHARE`; project `FOR UPDATE` (still deleted, same team); clear the
    /// marker; the `restored` record.
    pub async fn restore_deleted_project(
        &self,
        _project_id: uuid::Uuid,
    ) -> Result<ProjectRestoreOutcome, StoreError> {
        panic!(
            "Not yet implemented -- RED scaffold (project-lifecycle 03-01: restore_deleted_project)"
        )
    }

    /// The team delete page's deleted-projects line (DDD-21), read with no lock.
    pub async fn team_deleted_projects(
        &self,
        _team_id: uuid::Uuid,
    ) -> Result<TeamDeletedProjects, StoreError> {
        panic!(
            "Not yet implemented -- RED scaffold (project-lifecycle 04-01: team_delete_facts deleted_projects)"
        )
    }
}
