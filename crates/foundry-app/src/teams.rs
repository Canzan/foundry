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
//!                                  same 409; a stale target re-renders with no copy.
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
    CreateTeamForm, DefaultTeamForm, DefaultTeamOption, TeamCandidateOption, TeamDeletePage,
    TeamIndexRow, TeamMemberLine, TeamMembersPage, TeamsIndexPage,
};
use crate::AppState;
use askama::Template;
use axum::extract::{Form, Path, State};
use axum::http::header::HeaderMap;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use foundry_core::TeamRole;
use foundry_services::teams::{
    AddMemberResult, CreateTeamError, RemoveMemberResult, RoleChangeResult, TeamDeletePreview,
    TeamDeletion, TeamError, TeamMembersView, TeamsIndex,
};
use foundry_services::{Principal, Services};
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
    /// team-lifecycle D12: "Deleted {team}."
    Deleted { team: String },
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
            Self::Deleted { team } => write!(f, "Deleted {team}."),
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
    let removal = match services
        .remove_member_from_team(&principal_of(&user), &team_slug, &raw_user_id)
        .await
    {
        Ok(removal) => removal,
        Err(err) => return refusal(err),
    };
    let team = removal.page.team_name.clone();
    let (status, notice) = match removal.result {
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
    members_page_with_status(&state, &headers, &user, removal.page, notice, status).await
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
                    team: team_name,
                    members: facts.member_count,
                }
                .to_string(),
            };
            render(page.render(), StatusCode::OK, set_cookie)
        }
        Ok(TeamDeletePreview::HasProjects { .. }) => projects_refusal_not_yet_rendered(),
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
        Ok(TeamDeletion::HasProjects { .. }) => projects_refusal_not_yet_rendered(),
        Err(err) => refusal(err),
    }
}

/// team-lifecycle 02-04 renders the projects refusal (the 200 page and the
/// 409); until then a team with projects keeps a logged 500 and is never
/// deleted.
fn projects_refusal_not_yet_rendered() -> Response {
    tracing::error!("team delete refused for projects; the refusal page lands in 02-04");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
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

async fn members_page_with_status(
    state: &AppState,
    headers: &HeaderMap,
    user: &SessionUser,
    view: TeamMembersView,
    notice: Option<TeamNotice>,
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
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let page = TeamMembersPage {
        nav: nav_for(state, user, csrf.clone()).await,
        csrf,
        team_slug: view.team_slug,
        team_name: view.team_name,
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
    fn deleted() {
        let notice = TeamNotice::Deleted {
            team: "Bakcend".into(),
        };
        assert_eq!(notice.to_string(), "Deleted Bakcend.");
    }

    #[test]
    fn role_labels() {
        assert_eq!(role_label(TeamRole::Lead), "Lead");
        assert_eq!(role_label(TeamRole::Member), "Member");
    }
}
