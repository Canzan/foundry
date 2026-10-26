//! A member's project emails: the projects offered, the choices saved, and
//! the one-click stop (US-EIP-10, ADR-EIP-007).

use crate::project_lifecycle::lock_live_project;
use crate::{Store, StoreError};
use uuid::Uuid;

/// A project a member can open (a member of its team), as the Project emails
/// section offers it (US-EIP-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailProject {
    pub project_id: Uuid,
    pub project_name: String,
    pub workspace_id: Uuid,
    pub workspace_name: String,
}

/// One of a member's email choices: an event of a project, for the issues
/// its scope (`all`, `mine`) covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailChoiceRow {
    pub project_id: Uuid,
    pub event: String,
    pub scope: String,
}

/// The live projects a member can open, by workspace and project name.
const EMAIL_PROJECTS_SQL: &str = "
    SELECT p.id, p.name, w.id, w.name
      FROM live_projects p
      JOIN team_memberships tm ON tm.team_id = p.team_id AND tm.user_id = $1
      JOIN workspaces w ON w.id = p.workspace_id
     ORDER BY w.name, w.id, p.name, p.id";

/// A live project's name.
const EMAIL_STOP_PROJECT_SQL: &str = "SELECT name FROM live_projects WHERE id = $1";

/// A member's choices in live projects.
const EMAIL_CHOICES_SQL: &str = "
    SELECT ch.project_id, ch.event, ch.scope
      FROM integration_email_choices ch
      JOIN live_projects p ON p.id = ch.project_id
     WHERE ch.user_id = $1
     ORDER BY ch.project_id, ch.event";

impl Store {
    /// The live projects `user_id` can open: the projects of their teams.
    pub async fn email_projects(&self, user_id: Uuid) -> Result<Vec<EmailProject>, StoreError> {
        let rows: Vec<(Uuid, String, Uuid, String)> = sqlx::query_as(EMAIL_PROJECTS_SQL)
            .bind(user_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(
                |(project_id, project_name, workspace_id, workspace_name)| EmailProject {
                    project_id,
                    project_name,
                    workspace_id,
                    workspace_name,
                },
            )
            .collect())
    }

    /// `user_id`'s email choices in live projects.
    pub async fn email_choices(&self, user_id: Uuid) -> Result<Vec<EmailChoiceRow>, StoreError> {
        let rows: Vec<(Uuid, String, String)> = sqlx::query_as(EMAIL_CHOICES_SQL)
            .bind(user_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|(project_id, event, scope)| EmailChoiceRow {
                project_id,
                event,
                scope,
            })
            .collect())
    }

    /// The name of the live project `project_id` (a stop link's confirm
    /// page); `None` once it is deleted.
    pub async fn email_stop_project(&self, project_id: Uuid) -> Result<Option<String>, StoreError> {
        Ok(sqlx::query_scalar(EMAIL_STOP_PROJECT_SQL)
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Stop `user_id`'s `event` emails from `project_id` (US-EIP-11): delete
    /// that one choice, in one transaction that opens with
    /// `lock_live_project` (rule 4). Idempotent. The project's name, or
    /// `None`, with nothing written, when the project is not live.
    pub async fn stop_email_choice(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        event: &str,
    ) -> Result<Option<String>, StoreError> {
        let mut tx = self.pool.begin().await?;
        if !lock_live_project(&mut tx, project_id).await? {
            tx.rollback().await?;
            return Ok(None);
        }
        sqlx::query(
            "DELETE FROM integration_email_choices
              WHERE user_id = $1 AND project_id = $2 AND event = $3",
        )
        .bind(user_id)
        .bind(project_id)
        .bind(event)
        .execute(&mut *tx)
        .await?;
        let name: String = sqlx::query_scalar(EMAIL_STOP_PROJECT_SQL)
            .bind(project_id)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(Some(name))
    }

    /// Replace `user_id`'s choices in `project_id` with `choices`
    /// (`(event, scope)` pairs), in one transaction that opens with
    /// `lock_live_project` (rule 4). `false`, with nothing written, when the
    /// project is not live.
    pub async fn save_email_choices(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        choices: &[(String, String)],
    ) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin().await?;
        if !lock_live_project(&mut tx, project_id).await? {
            tx.rollback().await?;
            return Ok(false);
        }
        sqlx::query("DELETE FROM integration_email_choices WHERE user_id = $1 AND project_id = $2")
            .bind(user_id)
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        for (event, scope) in choices {
            sqlx::query(
                "INSERT INTO integration_email_choices (user_id, project_id, event, scope)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT (user_id, project_id, event) DO UPDATE SET scope = EXCLUDED.scope",
            )
            .bind(user_id)
            .bind(project_id)
            .bind(event)
            .bind(scope)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(true)
    }
}
