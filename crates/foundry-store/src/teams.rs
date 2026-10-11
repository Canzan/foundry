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
//! Shipped (slice 03): `remove_team_member` (below) and the guarded workspace
//! removal `Store::remove_workspace_member` (in `lib.rs`, DDD-11), whose outcome
//! type lives here.
//!
//! Shipped (slice 05): the default team for new members, `default_team` and
//! `set_default_team`: `workspaces.default_team_id` behind the 0022 composite
//! foreign key (DDD-12, DDD-14), and the crate-private placement helper
//! `place_on_default_team` (DDD-13), run in a savepoint by the newcomer paths.

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

/// The outcome of [`Store::remove_workspace_member`] (OQ-9, DDD-11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoveWorkspaceMemberOutcome {
    /// Their workspace membership and their rows on its teams were deleted.
    Removed,
    /// They are not a member of the workspace; nothing was written.
    NotMember,
    /// They are the last lead of `team_name` (the first such team by name);
    /// nothing was written. `person` is them, as the refusal copy names them.
    LastLead {
        team_name: String,
        person: TeamPerson,
    },
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

/// Their role on `team_id` and whether anyone else leads it, read exactly: the
/// team row is locked `FOR NO KEY UPDATE` first, so every lead-removing write on
/// a team is totally ordered and this read sees the previous holder's commit
/// (DDD-6, ADR-TEAM-MEMBERSHIP-001). `None`: they are not on the team.
async fn lock_team_and_read_standing(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    team_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<Option<(String, bool)>, StoreError> {
    sqlx::query("SELECT 1 FROM teams WHERE id = $1 FOR NO KEY UPDATE")
        .bind(team_id)
        .execute(&mut **tx)
        .await?;
    Ok(sqlx::query_as(
        "SELECT tm.role,
                EXISTS (SELECT 1 FROM team_memberships o
                         WHERE o.team_id = tm.team_id AND o.user_id <> tm.user_id
                           AND o.role = 'lead')
           FROM team_memberships tm
          WHERE tm.team_id = $1 AND tm.user_id = $2",
    )
    .bind(team_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?)
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
        let standing = lock_team_and_read_standing(&mut tx, team_id, user_id).await?;
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

    /// Take someone off `team_id` (slice 03, DDD-6, ADR-TEAM-MEMBERSHIP-001). The
    /// same shape as [`Store::set_team_role`]: the team row `FOR NO KEY UPDATE`,
    /// an exact read of their role and of the other leads, and a removal of the
    /// last lead returns [`RemoveTeamMemberOutcome::LastLead`] and writes nothing.
    pub async fn remove_team_member(
        &self,
        team_id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> Result<RemoveTeamMemberOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        let standing = lock_team_and_read_standing(&mut tx, team_id, user_id).await?;
        let outcome = match standing {
            None => RemoveTeamMemberOutcome::NotOnTeam,
            Some((role, false)) if role == TeamRole::Lead.as_str() => {
                RemoveTeamMemberOutcome::LastLead
            }
            Some(_) => RemoveTeamMemberOutcome::Removed,
        };
        if outcome != RemoveTeamMemberOutcome::Removed {
            tx.rollback().await?;
            return Ok(outcome);
        }
        let deleted =
            sqlx::query("DELETE FROM team_memberships WHERE team_id = $1 AND user_id = $2")
                .bind(team_id)
                .bind(user_id)
                .execute(&mut *tx)
                .await;
        match deleted {
            Ok(_) => {
                tx.commit().await?;
                Ok(RemoveTeamMemberOutcome::Removed)
            }
            Err(err) if is_lead_rule_refusal(&err) => {
                tracing::warn!(%team_id, "the lead guard refused a removal the store's pre-read allowed");
                tx.rollback().await?;
                Ok(RemoveTeamMemberOutcome::LastLead)
            }
            Err(err) => Err(err.into()),
        }
    }

    /// The workspace's default team for new members, as `(slug, name)` (slice 05).
    pub async fn default_team(
        &self,
        workspace_id: uuid::Uuid,
    ) -> Result<Option<(String, String)>, StoreError> {
        Ok(sqlx::query_as(
            "SELECT t.slug, t.name FROM workspaces w JOIN teams t ON t.id = w.default_team_id
              WHERE w.id = $1",
        )
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Set (or, with `None`, clear) the workspace's default team (slice 05). One
    /// UPDATE; the last writer wins. A team of another workspace is refused by
    /// the database (`workspaces_default_team_same_workspace`, INV-7).
    pub async fn set_default_team(
        &self,
        workspace_id: uuid::Uuid,
        team_id: Option<uuid::Uuid>,
    ) -> Result<(), StoreError> {
        sqlx::query("UPDATE workspaces SET default_team_id = $2 WHERE id = $1")
            .bind(workspace_id)
            .bind(team_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Put a newcomer on their workspace's default team, as `member`, inside the
/// caller's transaction (DDD-13, ADR-TEAM-MEMBERSHIP-002). Best-effort (D17):
/// the statement runs in a savepoint; if it fails, the savepoint is rolled back,
/// one warning carrying ids only is logged (DDD-20), and the caller's
/// transaction goes on to commit the account and membership. No default set
/// places nobody. Only a savepoint that cannot be opened or rolled back (a dead
/// connection) is an error.
pub(crate) async fn place_on_default_team(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<(), StoreError> {
    let mut savepoint = sqlx::Connection::begin(&mut **tx).await?;
    let placed = sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role)
         SELECT w.default_team_id, $2, 'member' FROM workspaces w
          WHERE w.id = $1 AND w.default_team_id IS NOT NULL
         ON CONFLICT (team_id, user_id) DO NOTHING",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(&mut *savepoint)
    .await;
    match placed {
        Ok(_) => savepoint.commit().await?,
        Err(err) => {
            savepoint.rollback().await?;
            tracing::warn!(
                %workspace_id,
                %user_id,
                error = %err,
                "placing a newcomer on the default team failed; they joined on no team"
            );
        }
    }
    Ok(())
}

// ===========================================================================
// team-lifecycle (DESIGN DDD-5, DDD-7, DDD-8, DDD-10, DDD-13, DDD-21;
// ADR-TEAM-LIFECYCLE-001/002/003)
//
// Shipped (team-lifecycle 01-01): `list_team_siblings` and
// `create_team_with_lead` (DDD-4, DDD-5).
//
// SCAFFOLD: true — RED scaffold written by DISTILL (2026-10-11). The outcome
// types are real (the store gold tests in `tests/team_create.rs`,
// `tests/team_delete.rs` and `tests/team_rename.rs` match on them); every
// method not yet shipped panics until its DELIVER step. Nothing in production
// calls those yet, so no shipped behaviour changes. Signatures are DISTILL's
// working choice: if DELIVER changes one, it changes the store tests with it.
// ===========================================================================

/// The outcome of [`Store::create_team_with_lead`] (DDD-5, INV-TL-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamCreateOutcome {
    /// One `teams` row and the creator's `lead` row were written together.
    Created { team_id: uuid::Uuid },
    /// `teams_workspace_id_slug_key` refused the slug; nothing was written.
    DuplicateSlug,
    /// The creator is not (or no longer) an admin of the workspace; nothing was
    /// written.
    NotAdmin,
}

/// What the delete confirm page states (DDD-10, INV-TL-3): read with no lock,
/// so every number here is advisory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamDeleteFacts {
    pub member_count: i64,
    /// The team is its workspace's default team for new members.
    pub is_default: bool,
    /// Machine tokens scoped to the team.
    pub scoped_token_count: i64,
    /// The team's projects as `(name, key)`, by name then key.
    pub projects: Vec<(String, String)>,
}

/// The outcome of [`Store::delete_team_if_empty`] (DDD-7, DDD-8, INV-TL-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeamDeleteOutcome {
    /// The team, its memberships and its scoped tokens are gone, the default
    /// cleared if it pointed here, and one `deleted` record was written.
    Deleted { name: String, slug: String },
    /// The team had projects at commit time, as `(name, key)` by name then key;
    /// nothing was written.
    HasProjects { projects: Vec<(String, String)> },
    /// No such team in the workspace (or it is already gone); nothing written.
    NotFound,
}

/// The outcome of [`Store::rename_team_with_audit`] (DDD-13, D15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeamRenameWrite {
    /// The name changed (never the slug) and one `renamed` record was written
    /// with `old_name`, the name read under the lock.
    Renamed { old_name: String },
    /// The new name is byte-equal to the stored one; nothing was written.
    Unchanged,
    /// No such team in the workspace (or it was deleted first); nothing written.
    NotFound,
}

/// `teams_workspace_id_slug_key` refusing an insert: SQLSTATE 23505 on that
/// constraint, and only that one.
fn is_team_slug_taken(err: &sqlx::Error) -> bool {
    err.as_database_error().is_some_and(|db| {
        db.code().as_deref() == Some("23505")
            && db.constraint() == Some("teams_workspace_id_slug_key")
    })
}

impl Store {
    /// The workspace's teams as `(name, stored slug)`, for the uniqueness rule
    /// and the slug mint (DDD-4).
    pub async fn list_team_siblings(
        &self,
        workspace_id: uuid::Uuid,
    ) -> Result<Vec<(String, String)>, StoreError> {
        Ok(
            sqlx::query_as("SELECT name, slug FROM teams WHERE workspace_id = $1")
                .bind(workspace_id)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    /// One transaction (DDD-5): the creator's admin row `FOR KEY SHARE`, then
    /// `INSERT teams` (app-minted v7 id), then the creator's `lead` row.
    ///
    /// The `KEY SHARE` lock orders this create against a removal of the creator
    /// (`remove_workspace_member` takes `FOR UPDATE`): either the removal waits
    /// and then sees her lead row (`LastLead`), or this create waits and then
    /// finds no admin row (`NotAdmin`). A taken slug is the
    /// `teams_workspace_id_slug_key` unique index refusing (`DuplicateSlug`),
    /// with nothing written (INV-TL-7: born led or not at all).
    pub async fn create_team_with_lead(
        &self,
        workspace_id: uuid::Uuid,
        creator_id: uuid::Uuid,
        name: &str,
        slug: &str,
    ) -> Result<TeamCreateOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        let is_admin: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM workspace_memberships
              WHERE workspace_id = $1 AND user_id = $2 AND role = 'admin'
                FOR KEY SHARE",
        )
        .bind(workspace_id)
        .bind(creator_id)
        .fetch_optional(&mut *tx)
        .await?;
        if is_admin.is_none() {
            tx.rollback().await?;
            return Ok(TeamCreateOutcome::NotAdmin);
        }
        let team_id = uuid::Uuid::now_v7();
        let inserted =
            sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
                .bind(team_id)
                .bind(workspace_id)
                .bind(name)
                .bind(slug)
                .execute(&mut *tx)
                .await;
        match inserted {
            Ok(_) => {}
            Err(err) if is_team_slug_taken(&err) => {
                tx.rollback().await?;
                return Ok(TeamCreateOutcome::DuplicateSlug);
            }
            Err(err) => return Err(err.into()),
        }
        sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, $3)")
            .bind(team_id)
            .bind(creator_id)
            .bind(TeamRole::Lead.as_str())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(TeamCreateOutcome::Created { team_id })
    }

    /// The first team the user is on, by name then slug (DDD-21); `None` when
    /// they are on no team. Picks the dashboard's empty state and link.
    pub async fn first_team_slug(
        &self,
        workspace_id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> Result<Option<String>, StoreError> {
        let slug: Option<String> = sqlx::query_scalar(
            "SELECT t.slug FROM teams t \
               JOIN team_memberships tm ON tm.team_id = t.id AND tm.user_id = $2 \
              WHERE t.workspace_id = $1 \
              ORDER BY t.name, t.slug \
              LIMIT 1",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(slug)
    }

    /// The confirm page's advisory facts, read with no lock (DDD-10).
    pub async fn team_delete_facts(
        &self,
        _team_id: uuid::Uuid,
    ) -> Result<TeamDeleteFacts, StoreError> {
        panic!("Not yet implemented -- RED scaffold (team-lifecycle 02-01: team_delete_facts)")
    }

    /// ADR-TEAM-LIFECYCLE-002 decision 1, in exactly this order: the team row
    /// `FOR UPDATE`; then, in a new statement, its projects; then `DELETE FROM
    /// teams`; then the `deleted` record; then commit.
    pub async fn delete_team_if_empty(
        &self,
        _workspace_id: uuid::Uuid,
        _team_id: uuid::Uuid,
        _actor_id: uuid::Uuid,
    ) -> Result<TeamDeleteOutcome, StoreError> {
        panic!("Not yet implemented -- RED scaffold (team-lifecycle 02-01: delete_team_if_empty)")
    }

    /// ADR-TEAM-LIFECYCLE-003 decision 2: the team row `FOR NO KEY UPDATE`; a
    /// byte-equal name writes nothing; else `UPDATE teams SET name` (never the
    /// slug) and the `renamed` record, in one transaction.
    pub async fn rename_team_with_audit(
        &self,
        _workspace_id: uuid::Uuid,
        _team_id: uuid::Uuid,
        _actor_id: uuid::Uuid,
        _new_name: &str,
    ) -> Result<TeamRenameWrite, StoreError> {
        panic!("Not yet implemented -- RED scaffold (team-lifecycle 04-01: rename_team_with_audit)")
    }
}
