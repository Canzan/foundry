//! Team membership writes and the default-team setting (team-membership DDD-6,
//! DDD-10, DDD-12, DDD-13, DDD-14) — RED scaffolds created by DISTILL 2026-10-09.
//!
//! Every method here panics until DELIVER implements it, so the store gold tests
//! (`tests/team_lead_guard.rs`, `tests/workspace_default_team.rs`) fail RED, not
//! BROKEN, when un-ignored. Nothing in the app calls them yet: no route is wired,
//! and production behaviour is unchanged.
//!
//! - `add_team_member` (slice 01): `FOR KEY SHARE` on the target's workspace
//!   membership, then `INSERT … ON CONFLICT DO NOTHING` as `member` (DDD-10).
//! - `set_team_role` (slice 02) and `remove_team_member` (slice 03): the team row
//!   `FOR NO KEY UPDATE`, the exact pre-read, a typed `LastLead`, and the 0021
//!   trigger's 23514 `team_memberships_team_keeps_a_lead` mapped to `LastLead`
//!   (DDD-6, ADR-TEAM-MEMBERSHIP-001).
//! - `default_team` / `set_default_team` (slice 05): `workspaces.default_team_id`
//!   behind the 0022 composite foreign key (DDD-12, DDD-14).
//!
//! The reads (`team_access_facts`, `list_teams`, `list_team_members`,
//! `list_team_candidates`, DDD-16) and the private `place_on_default_team` helper
//! (DDD-13) are DELIVER's to add; no test names them yet.
// SCAFFOLD: true

use crate::{Store, StoreError};
use foundry_core::TeamRole;

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

impl Store {
    /// Put a member of `workspace_id` on `team_id` as `member` (slice 01).
    pub async fn add_team_member(
        &self,
        _workspace_id: uuid::Uuid,
        _team_id: uuid::Uuid,
        _user_id: uuid::Uuid,
    ) -> Result<AddTeamMemberOutcome, StoreError> {
        panic!("Not yet implemented -- RED scaffold (team-membership 01-01: add_team_member)")
    }

    /// Make someone on `team_id` a lead or a member (slice 02).
    pub async fn set_team_role(
        &self,
        _team_id: uuid::Uuid,
        _user_id: uuid::Uuid,
        _role: TeamRole,
    ) -> Result<SetTeamRoleOutcome, StoreError> {
        panic!("Not yet implemented -- RED scaffold (team-membership 02-01: set_team_role)")
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
