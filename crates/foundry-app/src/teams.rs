//! Team membership web surface — `/teams` and `/team/{team_slug}/members`
//! (team-membership US-TM-01, US-TM-02; DESIGN DDD-2, DDD-5, DDD-9, DDD-17, DDD-18).
//!
//! WHY-NEW-FILE: crates/foundry-app/src/teams.rs
//!   CLOSEST-EXISTING: crates/foundry-app/src/member_invites.rs
//!   EXTENSION-COST: `member_invites.rs` gates every route on `is_workspace_admin`
//!     itself; these routes must not compute authority in the adapter at all — they
//!     call `foundry_services::teams`, which applies the one team-access policy
//!     (ADR-TEAM-MEMBERSHIP-003), and they grow to six routes over slices 01-05.
//!   PARALLEL-RATIONALE: a different authz boundary (the services-owned
//!     `TeamAccess` policy, not an admin check), a different refusal source
//!     (`TeamError::NotFound`), and its own copy enum (`TeamNotice`), as DESIGN's
//!     component table (DDD-1, DDD-18) records.
//!
//!   GET  /teams                  — a workspace admin sees every team; anyone else
//!                                  their own teams, or the shipped no-team copy.
//!   POST /teams                  — workspace admins only (team-lifecycle DDD-16):
//!                                  creates the team led by the caller, then
//!                                  renders its members page; a name refusal is a
//!                                  422 teams page with the text kept.
//!   GET  /team/{slug}/members    — needs View (members list); Manage adds the form.
//!   POST /teams/default          — workspace admins only; `team` is a slug of the
//!                                  acting workspace, or empty for "No team" (DDD-14).
//!   POST /team/{slug}/members    — needs Manage; `user_id` names a workspace member.
//!   POST /team/{slug}/members/{user_id}/role — needs Manage; `role` is lead|member.
//!                                  A last lead is a 409 with the members page (DDD-8).
//!   POST /team/{slug}/members/{user_id}/remove — needs Manage; a last lead is the
//!                                  same 409; a stale target re-renders with no copy;
//!                                  someone who removed themselves and can no longer
//!                                  view the team gets their 200 teams page with
//!                                  "You left {team}." (DDD-9 amendment, 2026-10-15).
//!   POST /team/{slug}/rename     — needs Manage (team-lifecycle DDD-17/22): Renamed
//!                                  → 200 members page with "Renamed {old} to
//!                                  {new}."; a byte-equal name → 200, no notice;
//!                                  a name refusal → 422 members page with the
//!                                  create door's words and the text kept.
//!   POST /team/{slug}/project/{project}/rename — needs Manage (project-lifecycle
//!                                  DDD-23/24): Renamed → 200 members page with
//!                                  "Renamed {old} to {new}."; a byte-equal name →
//!                                  200, no notice; a name refusal → 422 members
//!                                  page with the reason and the text kept in that
//!                                  project's row.
//!   GET  /team/{slug}/project/{project}/delete — needs Manage (project-lifecycle
//!                                  DDD-12/23): the confirm page; it writes nothing.
//!   POST /team/{slug}/project/{project}/delete — needs Manage (project-lifecycle
//!                                  DDD-23/27): Deleted → 200 members page with
//!                                  "Deleted {project}." (no redirect).
//!   GET  /team/{slug}/delete     — needs Administer (team-lifecycle DDD-10): the
//!                                  confirm page; it writes nothing.
//!   POST /team/{slug}/delete     — needs Administer (DDD-18): Deleted → 200 teams
//!                                  page with "Deleted {team}." (no redirect).
//!
//! Gate order (DDD-5): session (none → 404) → the services use-case (facts by
//! session workspace + slug, access, target id parse, target in workspace) → verb.
//! Every refusal is `resource_not_found_page()` byte for byte (D5), and logs
//! nothing (DDD-20). Writes re-render the members page with 200 and the D15 copy;
//! there is no post-redirect-get (DDD-9). CSRF is enforced by `csrf_middleware`.

use crate::bootstrap::{resource_not_found_page, SessionUser};
use crate::csrf::{ensure_csrf_cookie, response_with_optional_cookie};
use crate::session::SESSION_KEY_USER_ID;
use crate::views::{
    CreateTeamForm, DefaultTeamForm, DefaultTeamOption, ProjectDeletePage, TeamCandidateOption,
    TeamDeletePage, TeamIndexRow, TeamMemberLine, TeamMembersPage, TeamProjectLine, TeamsIndexPage,
};
use crate::AppState;
use askama::Template;
use axum::extract::{Form, Path, State};
use axum::http::header::HeaderMap;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use foundry_core::TeamRole;
use foundry_services::projects::{
    ProjectDeletePreview, ProjectLifecycleError, RenameProjectError, TeamProjectRename,
    TeamProjectRenaming,
};
use foundry_services::teams::{
    AddMemberResult, CreateTeamError, MemberRemoval, RemoveMemberResult, RenameTeamError,
    RoleChangeResult, TeamDeletePreview, TeamDeletion, TeamError, TeamMembersView, TeamRenaming,
    TeamsIndex,
};
use foundry_services::{Principal, Services};
use foundry_store::TeamProjectRow;
use serde::Deserialize;
use std::fmt;
use tower_sessions::Session;

/// The team-membership copy (D15), byte for byte. A variant lands with its verb.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeamNotice {
    /// "Added {name} ({email}) to {team} as a member."
    Added {
        name: String,
        email: String,
        team: String,
    },
    /// "{name} ({email}) is already on {team}."
    AlreadyOn {
        name: String,
        email: String,
        team: String,
    },
    /// "Everyone in this workspace is already on {team}."
    EveryoneOn { team: String },
    /// "{name} ({email}) is now a lead of {team}." / "... a member of {team}."
    NowA {
        name: String,
        email: String,
        role: TeamRole,
        team: String,
    },
    /// "{team} needs at least one lead. Make someone else a lead first."
    NeedsALead { team: String },
    /// "Removed {name} ({email}) from {team}."
    Removed {
        name: String,
        email: String,
        team: String,
    },
    /// "You left {team}." — a lead who removed themselves and can no longer
    /// view the team, on their teams list (DDD-9 amendment, 2026-10-15).
    Left { team: String },
    /// OQ-9: "{name} ({email}) is the last lead of {team}. Make someone else a
    /// lead first." (`POST /workspace/members/remove`'s 409).
    LastLeadOf {
        name: String,
        email: String,
        team: String,
    },
    /// "New members will join {team}."
    DefaultSet { team: String },
    /// "New members join no team."
    DefaultCleared,
    /// team-lifecycle D12: "Created {team}. You're its lead."
    Created { team: String },
    /// team-lifecycle D12: a workspace admin's teams page with no team.
    NoTeamsYet,
    /// team-lifecycle D12: the delete page's question, "Delete {team}?"
    DeleteQuestion { team: String },
    /// team-lifecycle D12: "{team} has {n} member(s) and no projects. They lose
    /// access to {team}. This can't be undone."
    DeleteMembers { team: String, members: i64 },
    /// team-lifecycle D12: "{team} is the default team for new members. After
    /// it is deleted, new members will join no team." (only when the default)
    DeleteDefault { team: String },
    /// team-lifecycle D12: "{k} machine token(s) scoped to {team} will stop
    /// working." (only when k > 0)
    DeleteTokens { team: String, tokens: i64 },
    /// team-lifecycle D12: "Deleted {team}."
    Deleted { team: String },
    /// team-lifecycle D12: "Renamed {old} to {new}."
    Renamed { old: String, new: String },
    /// project-lifecycle D14/DDD-26: "Renamed {old} to {new}." for a project.
    ProjectRenamed { old: String, new: String },
    /// team-lifecycle D9/DDD-10: "{team} still has {n} project(s): {list}. A
    /// team with projects can't be deleted." — the list is "Name (KEY)" by name
    /// then key, joined by ", ".
    HasProjects {
        team: String,
        projects: Vec<(String, String)>,
    },
    /// project-lifecycle DDD-25: "{team} has no projects yet."
    NoProjectsYet { team: String },
    /// project-lifecycle D14/DDD-26: the delete page's question, "Delete {project}?"
    ProjectDeleteHeading { project: String },
    /// project-lifecycle D14/DDD-26: "{project} ({KEY}) has {n} issue(s). After
    /// it is deleted, nobody can open its board, its issues or its report, and
    /// links to {KEY} issues stop working."
    ProjectDeleteCount {
        project: String,
        key: String,
        issues: i64,
    },
    /// project-lifecycle D14/DDD-26: "Nothing is erased. The instance operator
    /// can restore {project} with every issue, comment and attachment."
    ProjectDeleteNothingErased { project: String },
    /// project-lifecycle D14/DDD-26: "Deleted {project}."
    ProjectDeleted { project: String },
    /// project-lifecycle D14/DDD-21: "{team} also has {k} deleted project(s):
    /// {list}. They are kept, but can't be restored after {team} is deleted,
    /// and their keys stay taken in {workspace}." — the list as in
    /// [`TeamNotice::HasProjects`].
    TeamAlsoHasDeletedProjects {
        team: String,
        workspace: String,
        projects: Vec<(String, String)>,
    },
}

/// "Name (KEY)" per project, by name then key, joined by ", " (DDD-10).
fn project_list(projects: &[(String, String)]) -> String {
    let mut ordered: Vec<&(String, String)> = projects.iter().collect();
    ordered.sort();
    ordered
        .iter()
        .map(|(name, key)| format!("{name} ({key})"))
        .collect::<Vec<_>>()
        .join(", ")
}

impl fmt::Display for TeamNotice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Added { name, email, team } => {
                write!(f, "Added {name} ({email}) to {team} as a member.")
            }
            Self::AlreadyOn { name, email, team } => {
                write!(f, "{name} ({email}) is already on {team}.")
            }
            Self::EveryoneOn { team } => {
                write!(f, "Everyone in this workspace is already on {team}.")
            }
            Self::NowA {
                name,
                email,
                role,
                team,
            } => {
                let role = role.as_str();
                write!(f, "{name} ({email}) is now a {role} of {team}.")
            }
            Self::NeedsALead { team } => {
                write!(
                    f,
                    "{team} needs at least one lead. Make someone else a lead first."
                )
            }
            Self::Removed { name, email, team } => {
                write!(f, "Removed {name} ({email}) from {team}.")
            }
            Self::Left { team } => write!(f, "You left {team}."),
            Self::LastLeadOf { name, email, team } => {
                write!(
                    f,
                    "{name} ({email}) is the last lead of {team}. Make someone else a lead first."
                )
            }
            Self::DefaultSet { team } => write!(f, "New members will join {team}."),
            Self::DefaultCleared => f.write_str("New members join no team."),
            Self::Created { team } => write!(f, "Created {team}. You're its lead."),
            Self::NoTeamsYet => {
                f.write_str("This workspace has no teams yet. Create one to start a project.")
            }
            Self::DeleteQuestion { team } => write!(f, "Delete {team}?"),
            Self::DeleteMembers { team, members } => write!(
                f,
                "{team} has {members} member(s) and no projects. They lose access to {team}. \
                 This can't be undone."
            ),
            Self::DeleteDefault { team } => write!(
                f,
                "{team} is the default team for new members. After it is deleted, \
                 new members will join no team."
            ),
            Self::DeleteTokens { team, tokens } => {
                write!(
                    f,
                    "{tokens} machine token(s) scoped to {team} will stop working."
                )
            }
            Self::Deleted { team } | Self::ProjectDeleted { project: team } => {
                write!(f, "Deleted {team}.")
            }
            Self::Renamed { old, new } | Self::ProjectRenamed { old, new } => {
                write!(f, "Renamed {old} to {new}.")
            }
            Self::NoProjectsYet { team } => write!(f, "{team} has no projects yet."),
            Self::ProjectDeleteHeading { project } => write!(f, "Delete {project}?"),
            Self::ProjectDeleteCount {
                project,
                key,
                issues,
            } => write!(
                f,
                "{project} ({key}) has {issues} issue(s). After it is deleted, nobody can open \
                 its board, its issues or its report, and links to {key} issues stop working."
            ),
            Self::ProjectDeleteNothingErased { project } => write!(
                f,
                "Nothing is erased. The instance operator can restore {project} with every \
                 issue, comment and attachment."
            ),
            Self::HasProjects { team, projects } => write!(
                f,
                "{team} still has {} project(s): {}. A team with projects can't be deleted.",
                projects.len(),
                project_list(projects)
            ),
            Self::TeamAlsoHasDeletedProjects {
                team,
                workspace,
                projects,
            } => write!(
                f,
                "{team} also has {} deleted project(s): {}. They are kept, but can't be \
                 restored after {team} is deleted, and their keys stay taken in {workspace}.",
                projects.len(),
                project_list(projects)
            ),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AddMemberForm {
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(rename = "_csrf", default)]
    _csrf: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTeamFormData {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "_csrf", default)]
    _csrf: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RenameTeamFormData {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "_csrf", default)]
    _csrf: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RenameProjectFormData {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "_csrf", default)]
    _csrf: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DefaultTeamFormData {
    #[serde(default)]
    pub team: Option<String>,
    #[serde(rename = "_csrf", default)]
    _csrf: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RoleForm {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(rename = "_csrf", default)]
    _csrf: Option<String>,
}

// ------------------------------------------------------------------ GET /teams

pub async fn show_teams(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return resource_not_found_page();
    };
    let services = Services::new(state.store.clone());
    match services.list_teams(&principal_of(&user)).await {
        Ok(index) => teams_page(&state, &headers, &user, index, None).await,
        Err(err) => refusal(err),
    }
}

// ----------------------------------------------------------------- POST /teams

/// Gate order (DDD-16): session (none → 404) → the use-case (workspace admin,
/// else the uniform 404) → the name rule. Created → 200 members page of the new
/// team; a name refusal → 422 teams page with the text kept. Refusals log
/// nothing (DDD-25).
pub async fn submit_create(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<CreateTeamFormData>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return resource_not_found_page();
    };
    let services = Services::new(state.store.clone());
    let principal = principal_of(&user);
    let typed = form.name.unwrap_or_default();
    match services.create_team(&principal, &typed).await {
        Ok(created) => match services.team_members(&principal, &created.slug).await {
            Ok(view) => {
                let notice = TeamNotice::Created { team: created.name };
                members_page(&state, &headers, &user, view, Some(notice)).await
            }
            Err(err) => refusal(err),
        },
        Err(CreateTeamError::NotFound) => resource_not_found_page(),
        Err(CreateTeamError::InvalidName(reason)) => match services.list_teams(&principal).await {
            Ok(index) => {
                let create = CreateTeamForm {
                    typed,
                    error: Some(reason.to_string()),
                };
                let status = StatusCode::UNPROCESSABLE_ENTITY;
                teams_page_with(&state, &headers, &user, index, None, create, status).await
            }
            Err(err) => refusal(err),
        },
        Err(CreateTeamError::SlugContention) => {
            tracing::error!("team create lost its fallback address on every attempt");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
        }
        Err(CreateTeamError::Store(err)) => refusal(TeamError::Store(err)),
    }
}

// -------------------------------------------------------- POST /teams/default

pub async fn submit_default(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<DefaultTeamFormData>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return resource_not_found_page();
    };
    let services = Services::new(state.store.clone());
    let raw_team_slug = form.team.as_deref().unwrap_or("");
    let choice = match services
        .choose_default_team(&principal_of(&user), raw_team_slug)
        .await
    {
        Ok(choice) => choice,
        Err(err) => return refusal(err),
    };
    let notice = match choice.chosen {
        Some(team) => TeamNotice::DefaultSet { team: team.name },
        None => TeamNotice::DefaultCleared,
    };
    teams_page(&state, &headers, &user, choice.index, Some(notice)).await
}

// ---------------------------------------------------- GET /team/{slug}/members

pub async fn show_members(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(team_slug): Path<String>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return resource_not_found_page();
    };
    let services = Services::new(state.store.clone());
    match services
        .team_members(&principal_of(&user), &team_slug)
        .await
    {
        Ok(view) => members_page(&state, &headers, &user, view, None).await,
        Err(err) => refusal(err),
    }
}

// --------------------------------------------------- POST /team/{slug}/members

pub async fn submit_add(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(team_slug): Path<String>,
    Form(form): Form<AddMemberForm>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return resource_not_found_page();
    };
    let services = Services::new(state.store.clone());
    let raw_user_id = form.user_id.as_deref().unwrap_or("");
    let added = match services
        .add_member_to_team(&principal_of(&user), &team_slug, raw_user_id)
        .await
    {
        Ok(added) => added,
        Err(err) => return refusal(err),
    };
    let (name, email, team) = (
        added.person.display_name,
        added.person.email,
        added.page.team_name.clone(),
    );
    let notice = match added.result {
        AddMemberResult::Added => TeamNotice::Added { name, email, team },
        AddMemberResult::AlreadyOn => TeamNotice::AlreadyOn { name, email, team },
    };
    members_page(&state, &headers, &user, added.page, Some(notice)).await
}

// ------------------------------------ POST /team/{slug}/members/{user_id}/role

/// The path id stays a `String` so a non-UUID is the uniform 404, never axum's
/// 400 (DDD-5).
pub async fn submit_role(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((team_slug, raw_user_id)): Path<(String, String)>,
    Form(form): Form<RoleForm>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return resource_not_found_page();
    };
    let services = Services::new(state.store.clone());
    let raw_role = form.role.as_deref().unwrap_or("");
    let change = match services
        .change_member_role(&principal_of(&user), &team_slug, &raw_user_id, raw_role)
        .await
    {
        Ok(change) => change,
        Err(err) => return refusal(err),
    };
    let team = change.page.team_name.clone();
    let (status, notice) = match change.result {
        RoleChangeResult::Changed(person) | RoleChangeResult::Unchanged(person) => (
            StatusCode::OK,
            Some(TeamNotice::NowA {
                name: person.display_name,
                email: person.email,
                role: change.role,
                team,
            }),
        ),
        RoleChangeResult::NotOnTeam => (StatusCode::OK, None),
        RoleChangeResult::LastLead => (StatusCode::CONFLICT, Some(TeamNotice::NeedsALead { team })),
    };
    members_page_with_status(&state, &headers, &user, change.page, notice, status).await
}

// ---------------------------------- POST /team/{slug}/members/{user_id}/remove

/// The path id stays a `String` so a non-UUID is the uniform 404, never axum's
/// 400 (DDD-5).
pub async fn submit_remove(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((team_slug, raw_user_id)): Path<(String, String)>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return resource_not_found_page();
    };
    let services = Services::new(state.store.clone());
    let (result, page) = match services
        .remove_member_from_team(&principal_of(&user), &team_slug, &raw_user_id)
        .await
    {
        Ok(MemberRemoval::Members { result, page }) => (result, page),
        Ok(MemberRemoval::Left { team_name, index }) => {
            let notice = TeamNotice::Left { team: team_name };
            return teams_page(&state, &headers, &user, index, Some(notice)).await;
        }
        Err(err) => return refusal(err),
    };
    let team = page.team_name.clone();
    let (status, notice) = match result {
        RemoveMemberResult::Removed(person) => (
            StatusCode::OK,
            Some(TeamNotice::Removed {
                name: person.display_name,
                email: person.email,
                team,
            }),
        ),
        RemoveMemberResult::NotOnTeam => (StatusCode::OK, None),
        RemoveMemberResult::LastLead => {
            (StatusCode::CONFLICT, Some(TeamNotice::NeedsALead { team }))
        }
    };
    members_page_with_status(&state, &headers, &user, page, notice, status).await
}

// ---------------------------------------------------- POST /team/{slug}/rename

/// Gate order (DDD-16): session (none → 404) → the use-case (facts by session
/// workspace + slug, `Manage`, else the uniform 404) → the name rule → the
/// write. Renamed → 200 members page with the D12 copy; a byte-equal name →
/// 200 members page with no notice (D15). The slug, and so every URL, stays.
pub async fn submit_rename(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(team_slug): Path<String>,
    Form(form): Form<RenameTeamFormData>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return resource_not_found_page();
    };
    let services = Services::new(state.store.clone());
    let principal = principal_of(&user);
    let typed = form.name.unwrap_or_default();
    match services.rename_team(&principal, &team_slug, &typed).await {
        Ok(TeamRenaming::Renamed {
            old_name,
            new_name,
            page,
        }) => {
            let notice = TeamNotice::Renamed {
                old: old_name,
                new: new_name,
            };
            members_page(&state, &headers, &user, page, Some(notice)).await
        }
        Ok(TeamRenaming::Unchanged { page }) => {
            members_page(&state, &headers, &user, page, None).await
        }
        Err(RenameTeamError::NotFound) => resource_not_found_page(),
        // A name refusal is a 422 members page (DDD-18) with the create door's
        // words (`TeamNameError`'s Display) and the typed text kept.
        Err(RenameTeamError::InvalidName(reason)) => {
            match services.team_members(&principal, &team_slug).await {
                Ok(view) => {
                    let fields = PageFields {
                        team_rename: Some(RenameField {
                            typed,
                            error: Some(reason.to_string()),
                        }),
                        project_rename: None,
                    };
                    let status = StatusCode::UNPROCESSABLE_ENTITY;
                    members_page_rendered(&state, &headers, &user, view, None, fields, status).await
                }
                Err(err) => refusal(err),
            }
        }
        Err(RenameTeamError::Store(err)) => refusal(TeamError::Store(err)),
    }
}

// ------------------------------------- POST /team/{slug}/project/{p}/rename

/// Gate order (DDD-18): session (none → 404) → the use-case (team facts by
/// session workspace + slug, `Manage` read now (INV-PL-8), the project by
/// (team, slug), else the uniform 404) → the name rule → the write (DDD-24).
/// Renamed → 200 members page with "Renamed {old} to {new}."; a byte-equal
/// name → 200 members page with no notice; a name refusal → 422 with its words
/// and the typed text in that project's row. Refusals log nothing (DDD-30).
pub async fn submit_project_rename(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((team_slug, project_slug)): Path<(String, String)>,
    Form(form): Form<RenameProjectFormData>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return resource_not_found_page();
    };
    let services = Services::new(state.store.clone());
    let principal = principal_of(&user);
    let typed = form.name.unwrap_or_default();
    let outcome = services
        .rename_team_project(&principal, &team_slug, &project_slug, &typed)
        .await;
    match outcome {
        // A committed (or no-op) rename renders the page the use-case built
        // from the authority it read once (F-14) — no second authority read.
        Ok(TeamProjectRenaming { outcome, page }) => {
            let notice = match outcome {
                TeamProjectRename::Renamed { old_name, name } => Some(TeamNotice::ProjectRenamed {
                    old: old_name,
                    new: name,
                }),
                TeamProjectRename::NoOp { .. } => None,
            };
            members_page(&state, &headers, &user, page, notice).await
        }
        // No write committed: the 422 reads the page afresh.
        Err(RenameProjectError::InvalidName(reason)) => {
            let refused = ProjectRenameRefusal {
                project_slug,
                typed,
                error: reason.to_string(),
            };
            let fields = PageFields {
                team_rename: None,
                project_rename: Some(refused),
            };
            match services.team_members(&principal, &team_slug).await {
                Ok(view) => {
                    let status = StatusCode::UNPROCESSABLE_ENTITY;
                    members_page_rendered(&state, &headers, &user, view, None, fields, status).await
                }
                Err(err) => refusal(err),
            }
        }
        Err(RenameProjectError::Forbidden | RenameProjectError::NotFound) => {
            resource_not_found_page()
        }
        Err(RenameProjectError::Store(err)) => refusal(TeamError::Store(err)),
    }
}

// ------------------------------------- GET /team/{slug}/project/{p}/delete

/// Gate order (DDD-18): session (none → 404) → the use-case (team facts by
/// session workspace + slug, `can_manage`, the live project by (team, slug),
/// else the uniform 404) → the confirm page (DDD-12). Read-only; the issue
/// count is advisory (the POST decides).
pub async fn show_project_delete(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((team_slug, project_slug)): Path<(String, String)>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return resource_not_found_page();
    };
    let services = Services::new(state.store.clone());
    let preview = match services
        .project_delete_preview(&principal_of(&user), &team_slug, &project_slug)
        .await
    {
        Ok(preview) => preview,
        Err(err) => return project_refusal(err),
    };
    let ProjectDeletePreview {
        team_slug,
        project_name,
        key_prefix,
        issue_count,
        ..
    } = preview;
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let page = ProjectDeletePage {
        nav: nav_for(&state, &user, csrf.clone()).await,
        csrf,
        team_slug,
        project_slug,
        heading: TeamNotice::ProjectDeleteHeading {
            project: project_name.clone(),
        }
        .to_string(),
        count_line: TeamNotice::ProjectDeleteCount {
            project: project_name.clone(),
            key: key_prefix,
            issues: issue_count,
        }
        .to_string(),
        nothing_erased_line: TeamNotice::ProjectDeleteNothingErased {
            project: project_name,
        }
        .to_string(),
    };
    render(page.render(), StatusCode::OK, set_cookie)
}

// ------------------------------------- POST /team/{slug}/project/{p}/delete

/// Gate order (DDD-18/23): session (none → 404) → the use-case (team facts by
/// session workspace + slug, `Manage` read now (INV-PL-8), the live project by
/// (team, slug), the locked soft delete, else the uniform 404) → the 200
/// members page with "Deleted {project}." (DDD-27: no post-redirect-get, so a
/// reload re-POSTs and meets the 404). Refusals and races log nothing (DDD-30).
pub async fn submit_project_delete(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((team_slug, project_slug)): Path<(String, String)>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return resource_not_found_page();
    };
    let services = Services::new(state.store.clone());
    let principal = principal_of(&user);
    let deletion = match services
        .delete_project(&principal, &team_slug, &project_slug)
        .await
    {
        Ok(deletion) => deletion,
        Err(err) => return project_refusal(err),
    };
    let notice = TeamNotice::ProjectDeleted {
        project: deletion.name,
    };
    // The page the use-case built from the authority it read once (F-14).
    members_page(&state, &headers, &user, deletion.page, Some(notice)).await
}

// ----------------------------------------------------- GET /team/{slug}/delete

/// Gate order (DDD-5): session (none → 404) → the use-case (facts by session
/// workspace + slug, `can_delete`, else the uniform 404) → the confirm page.
/// Writes nothing.
pub async fn show_delete(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(team_slug): Path<String>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return resource_not_found_page();
    };
    let services = Services::new(state.store.clone());
    match services
        .team_delete_preview(&principal_of(&user), &team_slug)
        .await
    {
        Ok(TeamDeletePreview::Confirm {
            team_slug,
            team_name,
            facts,
        }) => {
            let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
            let page = TeamDeletePage {
                nav: nav_for(&state, &user, csrf.clone()).await,
                csrf,
                team_slug,
                question: TeamNotice::DeleteQuestion {
                    team: team_name.clone(),
                }
                .to_string(),
                member_line: TeamNotice::DeleteMembers {
                    team: team_name.clone(),
                    members: facts.member_count,
                }
                .to_string(),
                default_line: facts.is_default.then(|| {
                    TeamNotice::DeleteDefault {
                        team: team_name.clone(),
                    }
                    .to_string()
                }),
                token_line: (facts.scoped_token_count > 0).then(|| {
                    TeamNotice::DeleteTokens {
                        team: team_name.clone(),
                        tokens: facts.scoped_token_count,
                    }
                    .to_string()
                }),
                deleted_projects_line: (facts.projects.is_empty()
                    && !facts.deleted_projects.is_empty())
                .then(|| {
                    TeamNotice::TeamAlsoHasDeletedProjects {
                        team: team_name,
                        workspace: facts.workspace_name,
                        projects: facts.deleted_projects,
                    }
                    .to_string()
                }),
                refusal: None,
            };
            render(page.render(), StatusCode::OK, set_cookie)
        }
        Ok(TeamDeletePreview::HasProjects {
            team_name,
            projects,
        }) => {
            projects_refusal(
                &state,
                &headers,
                &user,
                team_slug,
                team_name,
                projects,
                StatusCode::OK,
            )
            .await
        }
        Err(err) => refusal(err),
    }
}

// ---------------------------------------------------- POST /team/{slug}/delete

/// Same gate order as [`show_delete`]; Deleted → 200 teams page with "Deleted
/// {team}." and no redirect (DDD-18): a reload meets the uniform 404.
pub async fn submit_delete(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path(team_slug): Path<String>,
) -> Response {
    let Some(user) = signed_in_user(&session).await else {
        return resource_not_found_page();
    };
    let services = Services::new(state.store.clone());
    match services.delete_team(&principal_of(&user), &team_slug).await {
        Ok(TeamDeletion::Deleted { team_name, index }) => {
            let notice = TeamNotice::Deleted { team: team_name };
            teams_page(&state, &headers, &user, index, Some(notice)).await
        }
        Ok(TeamDeletion::HasProjects {
            team_name,
            projects,
        }) => {
            projects_refusal(
                &state,
                &headers,
                &user,
                team_slug,
                team_name,
                projects,
                StatusCode::CONFLICT,
            )
            .await
        }
        Err(err) => refusal(err),
    }
}

/// The delete page's refusal variant (DDD-10, D9): the team's projects and no
/// Delete team. 200 on GET; 409 when the commit-time read found projects.
async fn projects_refusal(
    state: &AppState,
    headers: &HeaderMap,
    user: &SessionUser,
    team_slug: String,
    team_name: String,
    projects: Vec<(String, String)>,
    status: StatusCode,
) -> Response {
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let page = TeamDeletePage {
        nav: nav_for(state, user, csrf.clone()).await,
        csrf,
        team_slug,
        question: TeamNotice::DeleteQuestion {
            team: team_name.clone(),
        }
        .to_string(),
        member_line: String::new(),
        default_line: None,
        token_line: None,
        deleted_projects_line: None,
        refusal: Some(
            TeamNotice::HasProjects {
                team: team_name,
                projects,
            }
            .to_string(),
        ),
    };
    render(page.render(), status, set_cookie)
}

// ------------------------------------------------------------------ internals

async fn teams_page(
    state: &AppState,
    headers: &HeaderMap,
    user: &SessionUser,
    index: TeamsIndex,
    notice: Option<TeamNotice>,
) -> Response {
    let create = CreateTeamForm::default();
    teams_page_with(state, headers, user, index, notice, create, StatusCode::OK).await
}

/// The teams page; a workspace admin also gets `create` (the create form) and,
/// when the workspace has no team, the no-teams-yet copy.
async fn teams_page_with(
    state: &AppState,
    headers: &HeaderMap,
    user: &SessionUser,
    index: TeamsIndex,
    notice: Option<TeamNotice>,
    create: CreateTeamForm,
    status: StatusCode,
) -> Response {
    let is_admin = index.is_workspace_admin;
    let default_form = is_admin.then(|| default_form(&index));
    let no_teams_yet =
        (is_admin && index.teams.is_empty()).then(|| TeamNotice::NoTeamsYet.to_string());
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let page = TeamsIndexPage {
        nav: nav_for(state, user, csrf.clone()).await,
        csrf,
        is_workspace_admin: index.is_workspace_admin,
        teams: index
            .teams
            .into_iter()
            .map(|t| TeamIndexRow {
                slug: t.slug,
                name: t.name,
                member_count: t.member_count,
            })
            .collect(),
        default_form,
        create_form: is_admin.then_some(create),
        no_teams_yet,
        notice: notice.map(|n| n.to_string()),
    };
    render(page.render(), status, set_cookie)
}

/// "No team" (empty value), then every team of the workspace, with the current
/// default selected (DDD-14, DDD-17).
fn default_form(index: &TeamsIndex) -> DefaultTeamForm {
    let current = index.default_team.as_ref().map(|t| t.slug.as_str());
    let no_team = DefaultTeamOption {
        value: String::new(),
        label: "No team".to_string(),
        selected: current.is_none(),
    };
    let teams = index.teams.iter().map(|t| DefaultTeamOption {
        value: t.slug.clone(),
        label: t.name.clone(),
        selected: current == Some(t.slug.as_str()),
    });
    DefaultTeamForm {
        options: std::iter::once(no_team).chain(teams).collect(),
    }
}

async fn members_page(
    state: &AppState,
    headers: &HeaderMap,
    user: &SessionUser,
    view: TeamMembersView,
    notice: Option<TeamNotice>,
) -> Response {
    members_page_with_status(state, headers, user, view, notice, StatusCode::OK).await
}

/// The Rename field: the team's name, or — on a refused rename (DDD-18) — the
/// text typed and the rule's own words.
struct RenameField {
    typed: String,
    error: Option<String>,
}

impl RenameField {
    fn showing(team_name: &str) -> Self {
        Self {
            typed: team_name.to_string(),
            error: None,
        }
    }
}

/// A refused project rename (DDD-24): the row it belongs to, the text typed and
/// the rule's own words, shown in THAT row only.
struct ProjectRenameRefusal {
    project_slug: String,
    typed: String,
    error: String,
}

/// What a refused write puts back in the page's fields; the default is every
/// field showing its current name.
#[derive(Default)]
struct PageFields {
    team_rename: Option<RenameField>,
    project_rename: Option<ProjectRenameRefusal>,
}

impl PageFields {
    fn project_line(&self, p: TeamProjectRow) -> TeamProjectLine {
        let refused = self
            .project_rename
            .as_ref()
            .filter(|r| r.project_slug == p.slug);
        TeamProjectLine {
            typed: refused.map_or_else(|| p.name.clone(), |r| r.typed.clone()),
            error: refused.map(|r| r.error.clone()),
            name: p.name,
            slug: p.slug,
            key: p.key,
        }
    }
}

async fn members_page_with_status(
    state: &AppState,
    headers: &HeaderMap,
    user: &SessionUser,
    view: TeamMembersView,
    notice: Option<TeamNotice>,
    status: StatusCode,
) -> Response {
    members_page_rendered(
        state,
        headers,
        user,
        view,
        notice,
        PageFields::default(),
        status,
    )
    .await
}

async fn members_page_rendered(
    state: &AppState,
    headers: &HeaderMap,
    user: &SessionUser,
    view: TeamMembersView,
    notice: Option<TeamNotice>,
    fields: PageFields,
    status: StatusCode,
) -> Response {
    let can_manage = view.access.can_manage();
    let can_delete = view.access.can_delete();
    let on_team = view.on_team;
    let nobody_to_add = (can_manage && view.candidates.is_empty()).then(|| {
        TeamNotice::EveryoneOn {
            team: view.team_name.clone(),
        }
        .to_string()
    });
    let no_projects = view.projects.is_empty().then(|| {
        TeamNotice::NoProjectsYet {
            team: view.team_name.clone(),
        }
        .to_string()
    });
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let projects = view
        .projects
        .into_iter()
        .map(|p| fields.project_line(p))
        .collect();
    let rename = fields
        .team_rename
        .unwrap_or_else(|| RenameField::showing(&view.team_name));
    let page = TeamMembersPage {
        nav: nav_for(state, user, csrf.clone()).await,
        csrf,
        team_slug: view.team_slug,
        team_name: view.team_name,
        rename_typed: rename.typed,
        rename_error: rename.error,
        can_manage,
        can_delete,
        on_team,
        members: view
            .members
            .into_iter()
            .map(|row| TeamMemberLine {
                user_id: row.person.user_id.to_string(),
                name: row.person.display_name,
                email: row.person.email,
                role_label: role_label(row.role),
            })
            .collect(),
        candidates: view
            .candidates
            .into_iter()
            .map(|p| TeamCandidateOption {
                user_id: p.user_id.to_string(),
                name: p.display_name,
                email: p.email,
            })
            .collect(),
        notice: notice.map(|n| n.to_string()),
        nobody_to_add,
        projects,
        no_projects,
    };
    render(page.render(), status, set_cookie)
}

/// The D15 role labels.
fn role_label(role: TeamRole) -> &'static str {
    match role {
        TeamRole::Lead => "Lead",
        TeamRole::Member => "Member",
    }
}

/// `NotFound` is the uniform 404 and logs nothing (DDD-20); a store failure is a
/// 500.
fn refusal(err: TeamError) -> Response {
    match err {
        TeamError::NotFound => resource_not_found_page(),
        TeamError::Store(err) => {
            tracing::error!(error = %err, "team membership use-case failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
        }
    }
}

/// A project lifecycle refusal, answered as [`refusal`] answers a team one.
fn project_refusal(err: ProjectLifecycleError) -> Response {
    refusal(match err {
        ProjectLifecycleError::NotFound => TeamError::NotFound,
        ProjectLifecycleError::Store(err) => TeamError::Store(err),
    })
}

fn render(
    rendered: Result<String, askama::Error>,
    status: StatusCode,
    set_cookie: Option<String>,
) -> Response {
    match rendered {
        Ok(html) => response_with_optional_cookie(status, Html(html).into_response(), set_cookie),
        Err(err) => {
            tracing::error!(error = %err, "render team page failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
        }
    }
}

async fn nav_for(state: &AppState, user: &SessionUser, csrf: String) -> crate::nav::NavContext {
    let is_instance_admin = crate::nav::resolve_is_instance_admin(state, user.user_id).await;
    crate::nav::NavContext::home_for(
        state,
        user.user_id,
        user.workspace_id,
        is_instance_admin,
        csrf,
    )
    .await
}

fn principal_of(user: &SessionUser) -> Principal {
    Principal::Human {
        user_id: user.user_id,
        workspace_id: user.workspace_id,
    }
}

async fn signed_in_user(session: &Session) -> Option<SessionUser> {
    session
        .get::<SessionUser>(SESSION_KEY_USER_ID)
        .await
        .ok()
        .flatten()
}

#[cfg(test)]
mod team_notice_copy {
    //! D15, byte for byte (DDD-18). bypass: exact-string contracts.
    use super::*;

    #[test]
    fn added() {
        let notice = TeamNotice::Added {
            name: "Tomás Rivera".into(),
            email: "tomas.rivera@canzan.dev".into(),
            team: "Backend".into(),
        };
        assert_eq!(
            notice.to_string(),
            "Added Tomás Rivera (tomas.rivera@canzan.dev) to Backend as a member."
        );
    }

    #[test]
    fn already_on() {
        let notice = TeamNotice::AlreadyOn {
            name: "Tomás Rivera".into(),
            email: "tomas.rivera@canzan.dev".into(),
            team: "Backend".into(),
        };
        assert_eq!(
            notice.to_string(),
            "Tomás Rivera (tomas.rivera@canzan.dev) is already on Backend."
        );
    }

    #[test]
    fn everyone_on() {
        let notice = TeamNotice::EveryoneOn {
            team: "Backend".into(),
        };
        assert_eq!(
            notice.to_string(),
            "Everyone in this workspace is already on Backend."
        );
    }

    #[test]
    fn now_a_lead() {
        let notice = TeamNotice::NowA {
            name: "Sam Okafor".into(),
            email: "sam.okafor@canzan.dev".into(),
            role: TeamRole::Lead,
            team: "Backend".into(),
        };
        assert_eq!(
            notice.to_string(),
            "Sam Okafor (sam.okafor@canzan.dev) is now a lead of Backend."
        );
    }

    #[test]
    fn now_a_member() {
        let notice = TeamNotice::NowA {
            name: "Lena Park".into(),
            email: "lena.park@canzan.dev".into(),
            role: TeamRole::Member,
            team: "Backend".into(),
        };
        assert_eq!(
            notice.to_string(),
            "Lena Park (lena.park@canzan.dev) is now a member of Backend."
        );
    }

    #[test]
    fn needs_a_lead() {
        let notice = TeamNotice::NeedsALead {
            team: "Backend".into(),
        };
        assert_eq!(
            notice.to_string(),
            "Backend needs at least one lead. Make someone else a lead first."
        );
    }

    #[test]
    fn last_lead_of() {
        let notice = TeamNotice::LastLeadOf {
            name: "Lena Park".into(),
            email: "lena.park@canzan.dev".into(),
            team: "Backend".into(),
        };
        assert_eq!(
            notice.to_string(),
            "Lena Park (lena.park@canzan.dev) is the last lead of Backend. Make someone else a lead first."
        );
    }

    #[test]
    fn removed() {
        let notice = TeamNotice::Removed {
            name: "Tomás Rivera".into(),
            email: "tomas.rivera@canzan.dev".into(),
            team: "Backend".into(),
        };
        assert_eq!(
            notice.to_string(),
            "Removed Tomás Rivera (tomas.rivera@canzan.dev) from Backend."
        );
    }

    #[test]
    fn left() {
        let notice = TeamNotice::Left {
            team: "Backend".into(),
        };
        assert_eq!(notice.to_string(), "You left Backend.");
    }

    #[test]
    fn default_set() {
        let notice = TeamNotice::DefaultSet {
            team: "General".into(),
        };
        assert_eq!(notice.to_string(), "New members will join General.");
    }

    #[test]
    fn default_cleared() {
        assert_eq!(
            TeamNotice::DefaultCleared.to_string(),
            "New members join no team."
        );
    }

    #[test]
    fn created() {
        let notice = TeamNotice::Created { team: "Ops".into() };
        assert_eq!(notice.to_string(), "Created Ops. You're its lead.");
    }

    #[test]
    fn no_teams_yet() {
        assert_eq!(
            TeamNotice::NoTeamsYet.to_string(),
            "This workspace has no teams yet. Create one to start a project."
        );
    }

    #[test]
    fn delete_question() {
        let notice = TeamNotice::DeleteQuestion {
            team: "Bakcend".into(),
        };
        assert_eq!(notice.to_string(), "Delete Bakcend?");
    }

    #[test]
    fn delete_members() {
        let notice = TeamNotice::DeleteMembers {
            team: "Bakcend".into(),
            members: 2,
        };
        assert_eq!(
            notice.to_string(),
            "Bakcend has 2 member(s) and no projects. They lose access to Bakcend. This can't be undone."
        );
    }

    #[test]
    fn delete_default() {
        let notice = TeamNotice::DeleteDefault { team: "Ops".into() };
        assert_eq!(
            notice.to_string(),
            "Ops is the default team for new members. After it is deleted, new members will join no team."
        );
    }

    #[test]
    fn delete_tokens() {
        let notice = TeamNotice::DeleteTokens {
            team: "Design".into(),
            tokens: 1,
        };
        assert_eq!(
            notice.to_string(),
            "1 machine token(s) scoped to Design will stop working."
        );
    }

    #[test]
    fn deleted() {
        let notice = TeamNotice::Deleted {
            team: "Bakcend".into(),
        };
        assert_eq!(notice.to_string(), "Deleted Bakcend.");
    }

    #[test]
    fn renamed() {
        let notice = TeamNotice::Renamed {
            old: "Backend".into(),
            new: "Platform Engineering".into(),
        };
        assert_eq!(
            notice.to_string(),
            "Renamed Backend to Platform Engineering."
        );
    }

    #[test]
    fn has_one_project() {
        let notice = TeamNotice::HasProjects {
            team: "Backend".into(),
            projects: vec![("Identity Platform".into(), "AUTH".into())],
        };
        assert_eq!(
            notice.to_string(),
            "Backend still has 1 project(s): Identity Platform (AUTH). A team with projects can't be deleted."
        );
    }

    #[test]
    fn has_two_projects() {
        let notice = TeamNotice::HasProjects {
            team: "Ops".into(),
            projects: vec![
                ("Reading List".into(), "READ".into()),
                ("Homelab Ops".into(), "OPS".into()),
            ],
        };
        assert_eq!(
            notice.to_string(),
            "Ops still has 2 project(s): Homelab Ops (OPS), Reading List (READ). A team with projects can't be deleted."
        );
    }

    #[test]
    fn role_labels() {
        assert_eq!(role_label(TeamRole::Lead), "Lead");
        assert_eq!(role_label(TeamRole::Member), "Member");
    }
}

#[cfg(test)]
mod team_projects_list {
    //! The refusal's list: any non-empty set of (name, key) renders every pair
    //! once, ordered by name then key, joined by ", " (DDD-10).
    use super::*;
    use proptest::prelude::*;
    use std::collections::BTreeSet;

    fn pairs() -> impl Strategy<Value = BTreeSet<(String, String)>> {
        prop::collection::btree_set(("[A-Z][a-z]{0,6}( [A-Z][a-z]{0,6})?", "[A-Z]{2,6}"), 1..8)
    }

    proptest! {
        #[test]
        fn every_project_named_once_by_name_then_key(set in pairs(), seed in any::<u64>()) {
            let mut given: Vec<(String, String)> = set.iter().cloned().collect();
            let len = given.len();
            given.rotate_left(usize::try_from(seed).unwrap_or(0) % len);
            given.reverse();
            let text = TeamNotice::HasProjects { team: "Ops".into(), projects: given }.to_string();
            let prefix = format!("Ops still has {len} project(s): ");
            let suffix = ". A team with projects can't be deleted.";
            prop_assert!(text.starts_with(&prefix), "{text}");
            prop_assert!(text.ends_with(suffix), "{text}");
            let list = &text[prefix.len()..text.len() - suffix.len()];
            let rendered: Vec<&str> = list.split(", ").collect();
            let expected: Vec<String> =
                set.iter().map(|(name, key)| format!("{name} ({key})")).collect();
            prop_assert_eq!(rendered, expected);
        }
    }
}
