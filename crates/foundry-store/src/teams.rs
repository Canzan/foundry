//! Team membership reads and writes, and the default-team setting
//! (team-membership DDD-6, DDD-10, DDD-12, DDD-13, DDD-14, DDD-16).
//!
//! Shipped (slice 01): the reads `team_access_facts`, `list_teams`,
//! `list_team_members`, `list_team_candidates` (DDD-16) and `add_team_member`:
//! `FOR KEY SHARE` on the target's workspace membership, then
//! `INSERT … ON CONFLICT DO NOTHING` as `member` (DDD-10).
//!
//! Shipped (slice 02): `set_team_role` (below).
//!
//! The remaining methods are RED scaffolds created by DISTILL 2026-10-09; they
//! panic until DELIVER implements them, so the store gold tests
//! (`tests/team_lead_guard.rs`, `tests/workspace_default_team.rs`) fail RED, not
//! BROKEN, when un-ignored. Nothing in the app calls them yet.
//!
//! - `remove_team_member` (slice 03): as `set_team_role`, the team row
//!   `FOR NO KEY UPDATE`, the exact pre-read, a typed `LastLead`, and the 0021
//!   trigger's 23514 `team_memberships_team_keeps_a_lead` mapped to `LastLead`
//!   (DDD-6, ADR-TEAM-MEMBERSHIP-001).
//! - `default_team` / `set_default_team` (slice 05): `workspaces.default_team_id`
//!   behind the 0022 composite foreign key (DDD-12, DDD-14).
//!
//! The private `place_on_default_team` helper (DDD-13) is DELIVER's to add.
// SCAFFOLD: true

use crate::{Store, StoreError};
use foundry_core::TeamRole;

/// The two facts the team-authority policy needs, read in one query (DDD-16,
/// ADR-TEAM-MEMBERSHIP-003): the team resolved by (workspace, slug), whether the
/// user is an admin of that workspace, and their role on the team, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamAccessFacts {
    pub team_id: uuid::Uuid,
    pub team_name: String,
    pub team_slug: String,
    pub is_workspace_admin: bool,
    pub role: Option<TeamRole>,
}

/// Which teams [`Store::list_teams`] returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamScope {
    /// Every team of the workspace (a workspace admin).
    All,
    /// Only the teams this user is on.
    MemberOf(uuid::Uuid),
}

/// One team on the teams page, with its member count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamSummary {
    pub slug: String,
    pub name: String,
    pub member_count: i64,
}

/// A person, as team pages show them: display name and email (D8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamPerson {
    pub user_id: uuid::Uuid,
    pub display_name: String,
    pub email: String,
}

/// One row of a team's members list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamMemberRow {
    pub person: TeamPerson,
    pub role: TeamRole,
}

/// The outcome of [`Store::add_team_member`] (D7, D13, INV-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddTeamMemberOutcome {
    /// One `member` row was written.
    Added,
    /// The person was already on the team; nothing was written.
    AlreadyOn,
    /// The person is not (or no longer) a member of the workspace; nothing was
    /// written (the uniform 404 at the door).
    NotWorkspaceMember,
}

/// The outcome of [`Store::set_team_role`] (D6, D13, DDD-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetTeamRoleOutcome {
    Changed,
    /// They already held that role; nothing was written.
    Unchanged,
    /// They are not on the team; nothing was written.
    NotOnTeam,
    /// The demotion would leave a team that has a lead with none; nothing was
    /// written.
    LastLead,
}

/// The outcome of [`Store::remove_team_member`] (D6, D13, DDD-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveTeamMemberOutcome {
    Removed,
    NotOnTeam,
    LastLead,
}

type PersonTuple = (uuid::Uuid, String, String);

fn person_from((user_id, display_name, email): PersonTuple) -> TeamPerson {
    TeamPerson {
        user_id,
        display_name,
        email,
    }
}

/// The 0021 trigger's refusal: SQLSTATE 23514 naming
/// `team_memberships_team_keeps_a_lead` (ADR-TEAM-MEMBERSHIP-001 decision 4).
fn is_lead_rule_refusal(err: &sqlx::Error) -> bool {
    err.as_database_error().is_some_and(|db| {
        db.code().as_deref() == Some("23514")
            && db.constraint() == Some("team_memberships_team_keeps_a_lead")
    })
}

impl Store {
    /// The team `slug` of `workspace_id` and `user_id`'s standing on it, in ONE
    /// SELECT (DDD-16). `None`: no such team in this workspace.
    pub async fn team_access_facts(
        &self,
        workspace_id: uuid::Uuid,
        slug: &str,
        user_id: uuid::Uuid,
    ) -> Result<Option<TeamAccessFacts>, StoreError> {
        let row: Option<(uuid::Uuid, String, String, bool, Option<String>)> = sqlx::query_as(
            "SELECT t.id, t.name, t.slug,
                    EXISTS (SELECT 1 FROM workspace_memberships wm
                             WHERE wm.workspace_id = t.workspace_id
                               AND wm.user_id = $3 AND wm.role = 'admin'),
                    tm.role
               FROM teams t
               LEFT JOIN team_memberships tm ON tm.team_id = t.id AND tm.user_id = $3
              WHERE t.workspace_id = $1 AND t.slug = $2",
        )
        .bind(workspace_id)
        .bind(slug)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(
            |(team_id, team_name, team_slug, is_workspace_admin, role)| TeamAccessFacts {
                team_id,
                team_name,
                team_slug,
                is_workspace_admin,
                role: role.as_deref().and_then(TeamRole::parse),
            },
        ))
    }

    /// The workspace's teams in `scope`, by name, each with its member count
    /// (DDD-16; a member is a `team_memberships` row, DDD-7).
    pub async fn list_teams(
        &self,
        workspace_id: uuid::Uuid,
        scope: TeamScope,
    ) -> Result<Vec<TeamSummary>, StoreError> {
        let member_of = match scope {
            TeamScope::All => None,
            TeamScope::MemberOf(user_id) => Some(user_id),
        };
        let rows: Vec<(String, String, i64)> = sqlx::query_as(
            "SELECT t.slug, t.name,
                    (SELECT count(*) FROM team_memberships m WHERE m.team_id = t.id)
               FROM teams t
              WHERE t.workspace_id = $1
                AND ($2::uuid IS NULL OR EXISTS (
                        SELECT 1 FROM team_memberships m
                         WHERE m.team_id = t.id AND m.user_id = $2))
              ORDER BY t.name, t.slug",
        )
        .bind(workspace_id)
        .bind(member_of)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(slug, name, member_count)| TeamSummary {
                slug,
                name,
                member_count,
            })
            .collect())
    }

    /// Who is on `team_id`: leads first, then by display name, then by email
    /// (DDD-16).
    pub async fn list_team_members(
        &self,
        team_id: uuid::Uuid,
    ) -> Result<Vec<TeamMemberRow>, StoreError> {
        let rows: Vec<(uuid::Uuid, String, String, String)> = sqlx::query_as(
            "SELECT u.id, u.display_name, u.email_display, tm.role
               FROM team_memberships tm
               JOIN users u ON u.id = tm.user_id
              WHERE tm.team_id = $1
              ORDER BY tm.role = 'lead' DESC, u.display_name, u.email_lower",
        )
        .bind(team_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|(user_id, display_name, email, role)| {
                TeamRole::parse(&role).map(|role| TeamMemberRow {
                    person: person_from((user_id, display_name, email)),
                    role,
                })
            })
            .collect())
    }

    /// Members of `workspace_id` who are not on `team_id`, by display name then
    /// email: the Add list (DDD-16, D7).
    pub async fn list_team_candidates(
        &self,
        workspace_id: uuid::Uuid,
        team_id: uuid::Uuid,
    ) -> Result<Vec<TeamPerson>, StoreError> {
        let rows: Vec<PersonTuple> = sqlx::query_as(
            "SELECT u.id, u.display_name, u.email_display
               FROM workspace_memberships wm
               JOIN users u ON u.id = wm.user_id
              WHERE wm.workspace_id = $1
                AND NOT EXISTS (SELECT 1 FROM team_memberships tm
                                 WHERE tm.team_id = $2 AND tm.user_id = wm.user_id)
              ORDER BY u.display_name, u.email_lower",
        )
        .bind(workspace_id)
        .bind(team_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(person_from).collect())
    }

    /// Put a member of `workspace_id` on `team_id` as `member` (slice 01, DDD-10).
    /// The target's workspace membership is locked `FOR KEY SHARE` first, so a
    /// concurrent workspace removal either waits for this add or makes it refuse.
    pub async fn add_team_member(
        &self,
        workspace_id: uuid::Uuid,
        team_id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> Result<AddTeamMemberOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        let in_workspace: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM workspace_memberships
              WHERE workspace_id = $1 AND user_id = $2
                FOR KEY SHARE",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;
        if in_workspace.is_none() {
            tx.rollback().await?;
            return Ok(AddTeamMemberOutcome::NotWorkspaceMember);
        }
        let inserted = sqlx::query(
            "INSERT INTO team_memberships (team_id, user_id, role)
             SELECT t.id, $3, $4 FROM teams t WHERE t.id = $1 AND t.workspace_id = $2
             ON CONFLICT DO NOTHING",
        )
        .bind(team_id)
        .bind(workspace_id)
        .bind(user_id)
        .bind(TeamRole::Member.as_str())
        .execute(&mut *tx)
        .await?
        .rows_affected();
        tx.commit().await?;
        Ok(if inserted == 0 {
            AddTeamMemberOutcome::AlreadyOn
        } else {
            AddTeamMemberOutcome::Added
        })
    }

    /// Make someone on `team_id` a lead or a member (slice 02, DDD-6,
    /// ADR-TEAM-MEMBERSHIP-001). One transaction: the team row `FOR NO KEY
    /// UPDATE` (so lead-removing writes on a team are totally ordered, while adds
    /// and project creates, which take `FOR KEY SHARE`, never wait), then an exact
    /// read of their role and of the other leads. A demotion of the last lead
    /// returns [`SetTeamRoleOutcome::LastLead`] and writes nothing.
    pub async fn set_team_role(
        &self,
        team_id: uuid::Uuid,
        user_id: uuid::Uuid,
        role: TeamRole,
    ) -> Result<SetTeamRoleOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT 1 FROM teams WHERE id = $1 FOR NO KEY UPDATE")
            .bind(team_id)
            .execute(&mut *tx)
            .await?;
        let standing: Option<(String, bool)> = sqlx::query_as(
            "SELECT tm.role,
                    EXISTS (SELECT 1 FROM team_memberships o
                             WHERE o.team_id = tm.team_id AND o.user_id <> tm.user_id
                               AND o.role = 'lead')
               FROM team_memberships tm
              WHERE tm.team_id = $1 AND tm.user_id = $2",
        )
        .bind(team_id)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;
        let outcome = match standing {
            None => SetTeamRoleOutcome::NotOnTeam,
            Some((current, _)) if current == role.as_str() => SetTeamRoleOutcome::Unchanged,
            Some((_, false)) if role == TeamRole::Member => SetTeamRoleOutcome::LastLead,
            Some(_) => SetTeamRoleOutcome::Changed,
        };
        if outcome != SetTeamRoleOutcome::Changed {
            tx.rollback().await?;
            return Ok(outcome);
        }
        let updated = sqlx::query(
            "UPDATE team_memberships SET role = $3 WHERE team_id = $1 AND user_id = $2",
        )
        .bind(team_id)
        .bind(user_id)
        .bind(role.as_str())
        .execute(&mut *tx)
        .await;
        match updated {
            Ok(_) => {
                tx.commit().await?;
                Ok(SetTeamRoleOutcome::Changed)
            }
            Err(err) if is_lead_rule_refusal(&err) => {
                tracing::warn!(%team_id, "the lead guard refused a demotion the store's pre-read allowed");
                tx.rollback().await?;
                Ok(SetTeamRoleOutcome::LastLead)
            }
            Err(err) => Err(err.into()),
        }
    }

    /// Take someone off `team_id` (slice 03).
    pub async fn remove_team_member(
        &self,
        _team_id: uuid::Uuid,
        _user_id: uuid::Uuid,
    ) -> Result<RemoveTeamMemberOutcome, StoreError> {
        panic!("Not yet implemented -- RED scaffold (team-membership 03-01: remove_team_member)")
    }

    /// The workspace's default team for new members, as `(slug, name)` (slice 05).
    pub async fn default_team(
        &self,
        _workspace_id: uuid::Uuid,
    ) -> Result<Option<(String, String)>, StoreError> {
        panic!("Not yet implemented -- RED scaffold (team-membership 05-01: default_team)")
    }

    /// Set (or, with `None`, clear) the workspace's default team (slice 05).
    pub async fn set_default_team(
        &self,
        _workspace_id: uuid::Uuid,
        _team_id: Option<uuid::Uuid>,
    ) -> Result<(), StoreError> {
        panic!("Not yet implemented -- RED scaffold (team-membership 05-01: set_default_team)")
    }
}
