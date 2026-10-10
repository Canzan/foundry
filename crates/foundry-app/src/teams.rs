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
//!   GET  /team/{slug}/members    — needs View (members list); Manage adds the form.
//!   POST /team/{slug}/members    — needs Manage; `user_id` names a workspace member.
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
    TeamCandidateOption, TeamIndexRow, TeamMemberLine, TeamMembersPage, TeamsIndexPage,
};
use crate::AppState;
use askama::Template;
use axum::extract::{Form, Path, State};
use axum::http::header::HeaderMap;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use foundry_core::TeamRole;
use foundry_services::teams::{AddMemberResult, TeamError, TeamMembersView};
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
    let index = match services.list_teams(&principal_of(&user)).await {
        Ok(index) => index,
        Err(err) => return refusal(err),
    };
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let page = TeamsIndexPage {
        nav: nav_for(&state, &user, csrf).await,
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
    };
    render(page.render(), StatusCode::OK, set_cookie)
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

// ------------------------------------------------------------------ internals

async fn members_page(
    state: &AppState,
    headers: &HeaderMap,
    user: &SessionUser,
    view: TeamMembersView,
    notice: Option<TeamNotice>,
) -> Response {
    let can_manage = view.access.can_manage();
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
    render(page.render(), StatusCode::OK, set_cookie)
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
    fn role_labels() {
        assert_eq!(role_label(TeamRole::Lead), "Lead");
        assert_eq!(role_label(TeamRole::Member), "Member");
    }
}
