//! Workspace role changes for the operator CLI (`foundry doctor set-role`).
//!
//! The only write to `workspace_memberships.role` after a membership is
//! created. Reachable from the CLI adapter only, never the web tier: the
//! operator names the workspace explicitly.

use crate::{Store, StoreError};
use foundry_core::WorkspaceRole;

/// The outcome of [`Store::set_workspace_role`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetWorkspaceRoleOutcome {
    Changed,
    /// They already held that role; nothing was written.
    Unchanged,
    /// They are not a member of the workspace; nothing was written.
    NotMember,
    /// The demotion would leave the workspace with no admin; nothing was
    /// written.
    LastAdmin,
}

impl Store {
    /// Make a member of `workspace_id` an admin or a member. One transaction:
    /// the workspace row `FOR NO KEY UPDATE`, so two demotions in one workspace
    /// are ordered and cannot each see the other as the remaining admin; then
    /// an exact read of their role and of the other admins. A demotion of the
    /// last admin returns [`SetWorkspaceRoleOutcome::LastAdmin`] and writes
    /// nothing.
    pub async fn set_workspace_role(
        &self,
        workspace_id: uuid::Uuid,
        user_id: uuid::Uuid,
        role: WorkspaceRole,
    ) -> Result<SetWorkspaceRoleOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT 1 FROM workspaces WHERE id = $1 FOR NO KEY UPDATE")
            .bind(workspace_id)
            .execute(&mut *tx)
            .await?;
        let standing: Option<(String, bool)> = sqlx::query_as(
            "SELECT m.role,
                    EXISTS (SELECT 1 FROM workspace_memberships o
                             WHERE o.workspace_id = m.workspace_id
                               AND o.user_id <> m.user_id
                               AND o.role = $3) AS another_admin
               FROM workspace_memberships m
              WHERE m.workspace_id = $1 AND m.user_id = $2",
        )
        .bind(workspace_id)
        .bind(user_id)
        .bind(WorkspaceRole::Admin.as_str())
        .fetch_optional(&mut *tx)
        .await?;
        let outcome = match standing {
            None => SetWorkspaceRoleOutcome::NotMember,
            Some((current, _)) if current == role.as_str() => SetWorkspaceRoleOutcome::Unchanged,
            Some((_, false)) if role == WorkspaceRole::Member => SetWorkspaceRoleOutcome::LastAdmin,
            Some(_) => SetWorkspaceRoleOutcome::Changed,
        };
        if outcome != SetWorkspaceRoleOutcome::Changed {
            tx.rollback().await?;
            return Ok(outcome);
        }
        sqlx::query(
            "UPDATE workspace_memberships SET role = $3 WHERE workspace_id = $1 AND user_id = $2",
        )
        .bind(workspace_id)
        .bind(user_id)
        .bind(role.as_str())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(SetWorkspaceRoleOutcome::Changed)
    }
}
