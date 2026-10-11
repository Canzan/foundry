//! `foundry_services::teams` — team-membership use-cases (team-membership DDD-1,
//! DDD-3, DDD-5, ADR-TEAM-MEMBERSHIP-003).
//!
//! The team-authority policy is applied HERE and nowhere else: each team
//! use-case reads `Store::team_access_facts` once (DDD-22), feeds
//! [`TeamAccess::of`], and answers [`TeamError::NotFound`] for an unknown team
//! and for insufficient access alike, so the adapter's refusal is one uniform
//! 404. Tenancy comes only from the principal's workspace (D10).
//!
//! The read use-cases ([`list_teams`], [`team_members`], [`delete_preview`]) have
//! no write method on their path (Principle 12); [`add_member`], [`change_role`],
//! [`remove_member`], [`set_default_team`], [`create_team`] and [`delete_team`]
//! are the writers.

use crate::Principal;
use foundry_core::{mint_team_slug, MintedTeamSlug, TeamAccess, TeamName, TeamNameError, TeamRole};
use foundry_store::teams::{
    AddTeamMemberOutcome, RemoveTeamMemberOutcome, SetDefaultTeamOutcome, SetTeamRoleOutcome,
    TeamAccessFacts, TeamCreateOutcome, TeamDeleteFacts, TeamDeleteOutcome, TeamMemberRow,
    TeamPerson, TeamScope, TeamSummary,
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
/// workspace admin (all teams) or as a member (their own). `default_team` is
/// read only for a workspace admin, the one caller offered the setting (OQ-3).
#[derive(Debug, Clone)]
pub struct TeamsIndex {
    pub is_workspace_admin: bool,
    pub teams: Vec<TeamSummary>,
    pub default_team: Option<DefaultTeam>,
}

/// The team an invited newcomer lands on (ADR-TEAM-MEMBERSHIP-002).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultTeam {
    pub slug: String,
    pub name: String,
}

/// The outcome of setting the default: the team now chosen (`None` for "No
/// team"), with the teams page re-read after the write (DDD-9, DDD-14).
#[derive(Debug, Clone)]
pub struct DefaultTeamChoice {
    pub chosen: Option<DefaultTeam>,
    pub index: TeamsIndex,
}

/// A team's members page, as the caller may see it. `candidates` is empty unless
/// the caller may manage the team.
#[derive(Debug, Clone)]
pub struct TeamMembersView {
    pub team_slug: String,
    pub team_name: String,
    pub access: TeamAccess,
    /// The caller has a row on this team (lead or member): the members page
    /// offers New project (DDD-21).
    pub on_team: bool,
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

/// What a role change did (D6, D13, DDD-8/9). `Changed` and `Unchanged` carry
/// the person, as the page now lists them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleChangeResult {
    Changed(TeamPerson),
    /// They already held the role; nothing was written.
    Unchanged(TeamPerson),
    /// A workspace member no longer on the team; nothing was written.
    NotOnTeam,
    /// The demotion would leave the team without a lead; nothing was written.
    LastLead,
}

/// The outcome of a role change, with the page re-read after it (DDD-9).
#[derive(Debug, Clone)]
pub struct RoleChange {
    pub result: RoleChangeResult,
    pub role: TeamRole,
    pub page: TeamMembersView,
}

/// What a removal did (D13, DDD-8/9). `Removed` carries the person as the page
/// listed them before the write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoveMemberResult {
    Removed(TeamPerson),
    /// A workspace member no longer on the team; nothing was written.
    NotOnTeam,
    /// The removal would leave the team without a lead; nothing was written.
    LastLead,
}

/// The outcome of a removal, with the page re-read after it (DDD-9).
#[derive(Debug, Clone)]
pub struct MemberRemoval {
    pub result: RemoveMemberResult,
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
    let default_team = if is_workspace_admin {
        store
            .default_team(principal.workspace_id())
            .await?
            .map(|(slug, name)| DefaultTeam { slug, name })
    } else {
        None
    };
    Ok(TeamsIndex {
        is_workspace_admin,
        teams,
        default_team,
    })
}

/// `POST /teams/default` (DDD-14): workspace admins only. `raw_team_slug` is a
/// team slug resolved in the acting workspace, or empty for "No team"; an
/// unknown or foreign slug is `NotFound` with no write. Nobody already in the
/// workspace is moved (OQ-8).
pub async fn set_default_team(
    store: &Store,
    principal: &Principal,
    raw_team_slug: &str,
) -> Result<DefaultTeamChoice, TeamError> {
    let is_workspace_admin = store
        .is_workspace_admin(principal.workspace_id(), principal.user_id())
        .await?;
    if !is_workspace_admin {
        return Err(TeamError::NotFound);
    }
    let chosen = if raw_team_slug.is_empty() {
        None
    } else {
        Some(
            store
                .team_access_facts(principal.workspace_id(), raw_team_slug, principal.user_id())
                .await?
                .ok_or(TeamError::NotFound)?,
        )
    };
    let written = store
        .set_default_team(
            principal.workspace_id(),
            chosen.as_ref().map(|facts| facts.team_id),
        )
        .await?;
    if written == SetDefaultTeamOutcome::TeamGone {
        return Err(TeamError::NotFound);
    }
    let index = list_teams(store, principal).await?;
    Ok(DefaultTeamChoice {
        chosen: chosen.map(|facts| DefaultTeam {
            slug: facts.team_slug,
            name: facts.team_name,
        }),
        index,
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
        AddTeamMemberOutcome::NotWorkspaceMember | AddTeamMemberOutcome::TeamGone => {
            return Err(TeamError::NotFound)
        }
    };
    // Access is recomputed after the write (DDD-9).
    let page = team_members(store, principal, &facts.team_slug).await?;
    let person = page.member(user_id)?;
    Ok(AddedMember {
        result,
        person,
        page,
    })
}

/// `POST /team/{slug}/members/{user_id}/role`: needs `Manage`; the path id and
/// the role (raw values) must parse, and the target must be a member of the
/// acting workspace (DDD-5). Every refusal before the verb is `NotFound`.
pub async fn change_role(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    raw_user_id: &str,
    raw_role: &str,
) -> Result<RoleChange, TeamError> {
    let (facts, access) = resolve_access(store, principal, team_slug).await?;
    if !access.can_manage() {
        return Err(TeamError::NotFound);
    }
    let user_id = uuid::Uuid::parse_str(raw_user_id).map_err(|_| TeamError::NotFound)?;
    let role = TeamRole::parse(raw_role).ok_or(TeamError::NotFound)?;
    let before = members_view(store, principal, facts.clone(), access).await?;
    if !before.lists_in_workspace(user_id) {
        return Err(TeamError::NotFound);
    }
    let outcome = store.set_team_role(facts.team_id, user_id, role).await?;
    // Access is recomputed after the write (DDD-9).
    let page = team_members(store, principal, &facts.team_slug).await?;
    let result = match outcome {
        SetTeamRoleOutcome::Changed => RoleChangeResult::Changed(page.member(user_id)?),
        SetTeamRoleOutcome::Unchanged => RoleChangeResult::Unchanged(page.member(user_id)?),
        SetTeamRoleOutcome::NotOnTeam => RoleChangeResult::NotOnTeam,
        SetTeamRoleOutcome::LastLead => RoleChangeResult::LastLead,
    };
    Ok(RoleChange { result, role, page })
}

/// `POST /team/{slug}/members/{user_id}/remove`: needs `Manage`; the path id (a
/// raw value) must parse and name a member of the acting workspace (DDD-5).
/// Every refusal before the verb is `NotFound`.
pub async fn remove_member(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    raw_user_id: &str,
) -> Result<MemberRemoval, TeamError> {
    let (facts, access) = resolve_access(store, principal, team_slug).await?;
    if !access.can_manage() {
        return Err(TeamError::NotFound);
    }
    let user_id = uuid::Uuid::parse_str(raw_user_id).map_err(|_| TeamError::NotFound)?;
    let before = members_view(store, principal, facts.clone(), access).await?;
    let person = before.listed(user_id).ok_or(TeamError::NotFound)?;
    let result = match store.remove_team_member(facts.team_id, user_id).await? {
        RemoveTeamMemberOutcome::Removed => RemoveMemberResult::Removed(person),
        RemoveTeamMemberOutcome::NotOnTeam => RemoveMemberResult::NotOnTeam,
        RemoveTeamMemberOutcome::LastLead => RemoveMemberResult::LastLead,
    };
    // Access is recomputed after the write (DDD-9).
    let page = team_members(store, principal, &facts.team_slug).await?;
    Ok(MemberRemoval { result, page })
}

impl TeamMembersView {
    /// Whether `user_id` is on the team or offered to it, i.e. a member of the
    /// acting workspace (a manager's view lists both).
    fn lists_in_workspace(&self, user_id: uuid::Uuid) -> bool {
        self.listed(user_id).is_some()
    }

    /// The person with `user_id` as the page lists them, on the team or offered.
    fn listed(&self, user_id: uuid::Uuid) -> Option<TeamPerson> {
        self.members
            .iter()
            .map(|row| &row.person)
            .chain(self.candidates.iter())
            .find(|p| p.user_id == user_id)
            .cloned()
    }

    /// The person on the team with `user_id`, as the page lists them.
    fn member(&self, user_id: uuid::Uuid) -> Result<TeamPerson, TeamError> {
        self.members
            .iter()
            .find(|row| row.person.user_id == user_id)
            .map(|row| row.person.clone())
            .ok_or(TeamError::NotFound)
    }
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
        on_team: facts.role.is_some(),
        members,
        candidates,
    })
}

// ===========================================================================
// team-lifecycle: the create use-case (DESIGN DDD-4, ADR-TEAM-LIFECYCLE-001
// decision 3). Its error stays its own type, `CreateTeamError`, so no shipped
// `match` on `TeamError` changes.
// ===========================================================================

/// Why a team create did not happen.
#[derive(Debug)]
pub enum CreateTeamError {
    /// The caller is not an admin of the acting workspace: the uniform 404.
    NotFound,
    /// The name broke the rule, or is not unique within the workspace (422).
    InvalidName(TeamNameError),
    /// The fallback slug was lost on every bounded retry (a logged 500).
    SlugContention,
    Store(StoreError),
}

impl From<StoreError> for CreateTeamError {
    fn from(err: StoreError) -> Self {
        CreateTeamError::Store(err)
    }
}

/// The team a create wrote, with its minted slug.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedTeam {
    pub team_id: uuid::Uuid,
    pub name: String,
    pub slug: String,
}

/// How many create attempts a fallback address gets before the create gives up
/// with [`CreateTeamError::SlugContention`] (ADR-PROJECT-NAME-002 §3's idiom).
/// Counts the first attempt.
pub const TEAM_SLUG_ATTEMPTS: usize = 3;

/// `POST /teams`: admin check, the one name rule, then per attempt (at most 3):
/// siblings, uniqueness, mint, `Store::create_team_with_lead`. A derived-slug
/// clash is `NotUnique`; a fallback-slug clash is retried. The ONE place a team
/// slug is minted.
pub async fn create_team(
    store: &Store,
    principal: &Principal,
    raw_name: &str,
) -> Result<CreatedTeam, CreateTeamError> {
    let workspace_id = principal.workspace_id();
    if !store
        .is_workspace_admin(workspace_id, principal.user_id())
        .await?
    {
        return Err(CreateTeamError::NotFound);
    }
    let name = TeamName::try_new(raw_name).map_err(CreateTeamError::InvalidName)?;
    for _ in 0..TEAM_SLUG_ATTEMPTS {
        let siblings = store.list_team_siblings(workspace_id).await?;
        name.ensure_unique_among(&siblings)
            .map_err(CreateTeamError::InvalidName)?;
        let taken: Vec<String> = siblings.into_iter().map(|(_, slug)| slug).collect();
        let minted = mint_team_slug(&name, &taken);
        let outcome = store
            .create_team_with_lead(
                workspace_id,
                principal.user_id(),
                name.as_str(),
                minted.as_str(),
            )
            .await?;
        match (outcome, &minted) {
            (TeamCreateOutcome::Created { team_id }, _) => {
                return Ok(CreatedTeam {
                    team_id,
                    name: name.as_str().to_string(),
                    slug: minted.as_str().to_string(),
                })
            }
            (TeamCreateOutcome::NotAdmin, _) => return Err(CreateTeamError::NotFound),
            (TeamCreateOutcome::DuplicateSlug, MintedTeamSlug::Derived(_)) => {
                return Err(CreateTeamError::InvalidName(TeamNameError::NotUnique))
            }
            (TeamCreateOutcome::DuplicateSlug, MintedTeamSlug::Fallback(_)) => continue,
        }
    }
    Err(CreateTeamError::SlugContention)
}

// ===========================================================================
// team-lifecycle: the delete use-cases (DESIGN DDD-7, DDD-10, DDD-18,
// ADR-TEAM-LIFECYCLE-002, ADR-TEAM-LIFECYCLE-004). Both need `Administer`
// (`can_delete`); anything less is the uniform 404.
// ===========================================================================

/// What the delete page shows (DDD-10): the confirm, or why the team cannot go.
/// Every number is advisory; the delete re-checks under its lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeamDeletePreview {
    /// The team has no project: offer the delete.
    Confirm {
        team_slug: String,
        team_name: String,
        facts: TeamDeleteFacts,
    },
    /// The team has projects, as `(name, key)` by name then key.
    HasProjects {
        team_name: String,
        projects: Vec<(String, String)>,
    },
}

/// What a delete did (DDD-18).
#[derive(Debug, Clone)]
pub enum TeamDeletion {
    /// The team is gone; the teams page is re-read after the write.
    Deleted {
        team_name: String,
        index: TeamsIndex,
    },
    /// The team had projects at commit time; nothing was written.
    HasProjects {
        team_name: String,
        projects: Vec<(String, String)>,
    },
}

/// `GET /team/{slug}/delete`: needs `Administer`. Read-only: no write method on
/// its path (Principle 12).
pub async fn delete_preview(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
) -> Result<TeamDeletePreview, TeamError> {
    let facts = administered_team(store, principal, team_slug).await?;
    let delete_facts = store.team_delete_facts(facts.team_id).await?;
    if !delete_facts.projects.is_empty() {
        return Ok(TeamDeletePreview::HasProjects {
            team_name: facts.team_name,
            projects: delete_facts.projects,
        });
    }
    Ok(TeamDeletePreview::Confirm {
        team_slug: facts.team_slug,
        team_name: facts.team_name,
        facts: delete_facts,
    })
}

/// `POST /team/{slug}/delete`: needs `Administer`, read once at the start
/// (ADR-TEAM-LIFECYCLE-004 decision 6); the store's locked transaction decides
/// between deleted and has-projects (ADR-TEAM-LIFECYCLE-002).
pub async fn delete_team(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
) -> Result<TeamDeletion, TeamError> {
    let facts = administered_team(store, principal, team_slug).await?;
    let outcome = store
        .delete_team_if_empty(principal.workspace_id(), facts.team_id, principal.user_id())
        .await?;
    match outcome {
        TeamDeleteOutcome::Deleted { name, .. } => Ok(TeamDeletion::Deleted {
            team_name: name,
            index: list_teams(store, principal).await?,
        }),
        TeamDeleteOutcome::HasProjects { projects } => Ok(TeamDeletion::HasProjects {
            team_name: facts.team_name,
            projects,
        }),
        TeamDeleteOutcome::NotFound => Err(TeamError::NotFound),
    }
}

/// The team's facts when the caller may delete it (`Administer`); anything else
/// is the uniform 404 (ADR-TEAM-LIFECYCLE-004 decision 2).
async fn administered_team(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
) -> Result<TeamAccessFacts, TeamError> {
    let (facts, access) = resolve_access(store, principal, team_slug).await?;
    if !access.can_delete() {
        return Err(TeamError::NotFound);
    }
    Ok(facts)
}
