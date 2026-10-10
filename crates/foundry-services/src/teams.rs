//! `foundry_services::teams` — team-membership use-cases (team-membership DDD-1,
//! DDD-3, DDD-5, ADR-TEAM-MEMBERSHIP-003).
//!
//! The team-authority policy is applied HERE and nowhere else: each team
//! use-case reads `Store::team_access_facts` once (DDD-22), feeds
//! [`TeamAccess::of`], and answers [`TeamError::NotFound`] for an unknown team
//! and for insufficient access alike, so the adapter's refusal is one uniform
//! 404. Tenancy comes only from the principal's workspace (D10).
//!
//! The read use-cases ([`list_teams`], [`team_members`]) have no write method on
//! their path (Principle 12); [`add_member`] is the only writer.

use crate::Principal;
use foundry_core::TeamAccess;
use foundry_store::teams::{
    AddTeamMemberOutcome, TeamAccessFacts, TeamMemberRow, TeamPerson, TeamScope, TeamSummary,
};
use foundry_store::{Store, StoreError};

/// Why a team use-case did not run.
#[derive(Debug)]
pub enum TeamError {
    /// No such team in the acting workspace, no access to it, or a target that
    /// cannot be named here: all the uniform 404 (D5).
    NotFound,
    Store(StoreError),
}

impl From<StoreError> for TeamError {
    fn from(err: StoreError) -> Self {
        TeamError::Store(err)
    }
}

/// The teams page: which teams the caller sees, and whether they see them as a
/// workspace admin (all teams) or as a member (their own).
#[derive(Debug, Clone)]
pub struct TeamsIndex {
    pub is_workspace_admin: bool,
    pub teams: Vec<TeamSummary>,
}

/// A team's members page, as the caller may see it. `candidates` is empty unless
/// the caller may manage the team.
#[derive(Debug, Clone)]
pub struct TeamMembersView {
    pub team_slug: String,
    pub team_name: String,
    pub access: TeamAccess,
    pub members: Vec<TeamMemberRow>,
    pub candidates: Vec<TeamPerson>,
}

/// What an Add did (D13): one row written, or none because they were already on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddMemberResult {
    Added,
    AlreadyOn,
}

/// The outcome of an Add, with the page re-read after the write (DDD-9).
#[derive(Debug, Clone)]
pub struct AddedMember {
    pub result: AddMemberResult,
    pub person: TeamPerson,
    pub page: TeamMembersView,
}

/// `GET /teams`: every team for a workspace admin, else the caller's own teams.
pub async fn list_teams(store: &Store, principal: &Principal) -> Result<TeamsIndex, TeamError> {
    let is_workspace_admin = store
        .is_workspace_admin(principal.workspace_id(), principal.user_id())
        .await?;
    let scope = if is_workspace_admin {
        TeamScope::All
    } else {
        TeamScope::MemberOf(principal.user_id())
    };
    let teams = store.list_teams(principal.workspace_id(), scope).await?;
    Ok(TeamsIndex {
        is_workspace_admin,
        teams,
    })
}

/// `GET /team/{slug}/members`: needs `View`.
pub async fn team_members(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
) -> Result<TeamMembersView, TeamError> {
    let (facts, access) = resolve_access(store, principal, team_slug).await?;
    if !access.can_view() {
        return Err(TeamError::NotFound);
    }
    members_view(store, principal, facts, access).await
}

/// `POST /team/{slug}/members`: needs `Manage`; the target (a raw form value) must
/// parse as an id and name a member of the acting workspace (DDD-5, DDD-10).
pub async fn add_member(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    raw_user_id: &str,
) -> Result<AddedMember, TeamError> {
    let (facts, access) = resolve_access(store, principal, team_slug).await?;
    if !access.can_manage() {
        return Err(TeamError::NotFound);
    }
    let user_id = uuid::Uuid::parse_str(raw_user_id).map_err(|_| TeamError::NotFound)?;
    let result = match store
        .add_team_member(principal.workspace_id(), facts.team_id, user_id)
        .await?
    {
        AddTeamMemberOutcome::Added => AddMemberResult::Added,
        AddTeamMemberOutcome::AlreadyOn => AddMemberResult::AlreadyOn,
        AddTeamMemberOutcome::NotWorkspaceMember => return Err(TeamError::NotFound),
    };
    // Access is recomputed after the write (DDD-9).
    let page = team_members(store, principal, &facts.team_slug).await?;
    let person = page
        .members
        .iter()
        .find(|row| row.person.user_id == user_id)
        .map(|row| row.person.clone())
        .ok_or(TeamError::NotFound)?;
    Ok(AddedMember {
        result,
        person,
        page,
    })
}

/// The one facts read and the one policy (ADR-TEAM-MEMBERSHIP-003).
async fn resolve_access(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
) -> Result<(TeamAccessFacts, TeamAccess), TeamError> {
    let facts = store
        .team_access_facts(principal.workspace_id(), team_slug, principal.user_id())
        .await?
        .ok_or(TeamError::NotFound)?;
    let access = TeamAccess::of(facts.is_workspace_admin, facts.role);
    Ok((facts, access))
}

async fn members_view(
    store: &Store,
    principal: &Principal,
    facts: TeamAccessFacts,
    access: TeamAccess,
) -> Result<TeamMembersView, TeamError> {
    let members = store.list_team_members(facts.team_id).await?;
    let candidates = if access.can_manage() {
        store
            .list_team_candidates(principal.workspace_id(), facts.team_id)
            .await?
    } else {
        Vec::new()
    };
    Ok(TeamMembersView {
        team_slug: facts.team_slug,
        team_name: facts.team_name,
        access,
        members,
        candidates,
    })
}
