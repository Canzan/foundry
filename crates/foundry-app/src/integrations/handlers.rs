//! The integration doors (DESIGN [REF] Driving Ports; ADR-EIP-005 decisions
//! 2 and 4). Every door answers the uniform 404 to a caller without the right
//! (SC-6), decided by the use-case before anything else.
//!
//! - `GET /workspace/integrations`: the Slack connection (K-1), what is
//!   missing (K-2, K-20) or Add to Slack, and the activity list (R-4).
//! - `GET /workspace/integrations/slack/setup`: the manifest page (DD-9).
//! - `GET /workspace/integrations/slack/install`: keeps a fresh state in the
//!   session and sends the admin to Slack's approval screen.
//! - `GET /workspace/integrations/slack/return`: takes the state back out
//!   (single use), and only for a matching, fresh state exchanges the code,
//!   seals the bot token for its connection row and saves the connection.
//!   Anything else shows K-3 and stores nothing.
//! - `GET /team/{t}/project/{p}/integrations`: the project's routes (Manage).

use super::slack::{self, InstallState, SlackGrant};
use super::{Integrations, SlackOffer};
use crate::bootstrap::{resource_not_found_page, SessionUser};
use crate::csrf::{ensure_csrf_cookie, response_with_optional_cookie};
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use foundry_auth::secret_box::SecretSlot;
use foundry_services::integrations::{
    IntegrationError, ProjectIntegrations, SlackInstall, WorkspaceIntegrations,
};
use foundry_services::{Principal, Services};
use foundry_store::integrations::{ConnectionWriteOutcome, ProjectRoute};
use serde::Deserialize;
use tower_sessions::Session;

pub const WORKSPACE_PATH: &str = "/workspace/integrations";
pub const SETUP_PATH: &str = "/workspace/integrations/slack/setup";
pub const INSTALL_PATH: &str = "/workspace/integrations/slack/install";
pub const RETURN_PATH: &str = slack::RETURN_PATH;

/// K-3.
pub const NOT_CONNECTED: &str = "Slack was not connected. Nothing changed.";

#[derive(Template)]
#[template(path = "integrations/workspace.html")]
struct WorkspacePage {
    nav: crate::nav::NavContext,
    notice: Option<String>,
    slack_line: Option<String>,
    not_set_up: bool,
    no_secrets_key: bool,
    activity: Vec<String>,
}

#[derive(Template)]
#[template(path = "integrations/slack_setup.html")]
struct SetupPage {
    nav: crate::nav::NavContext,
    manifest: String,
}

#[derive(Template)]
#[template(path = "integrations/project.html")]
struct ProjectPage {
    nav: crate::nav::NavContext,
    project_name: String,
    routes: Vec<ProjectRoute>,
}

/// Slack's return: a code and our state, or Slack's `error`.
#[derive(Debug, Default, Deserialize)]
pub struct SlackReturn {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// The signed-in caller as the use-cases see them.
struct Caller {
    user: SessionUser,
    services: Services,
}

impl Caller {
    async fn of(state: &AppState, session: &Session) -> Option<Self> {
        let user = session
            .get::<SessionUser>(SESSION_KEY_USER_ID)
            .await
            .ok()
            .flatten()?;
        Some(Self {
            user,
            services: Services::new(state.store.clone()),
        })
    }

    fn principal(&self) -> Principal {
        Principal::Human {
            user_id: self.user.user_id,
            workspace_id: self.user.workspace_id,
        }
    }

    async fn nav(&self, state: &AppState, csrf: String) -> crate::nav::NavContext {
        let is_instance_admin =
            crate::nav::resolve_is_instance_admin(state, self.user.user_id).await;
        crate::nav::NavContext::home_for(
            state,
            self.user.user_id,
            self.user.workspace_id,
            is_instance_admin,
            csrf,
        )
        .await
    }
}

pub async fn show_workspace(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return resource_not_found_page();
    };
    workspace_page(&state, &caller, &headers, None).await
}

pub async fn show_slack_setup(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return resource_not_found_page();
    };
    if let Err(err) = caller
        .services
        .require_workspace_admin(&caller.principal())
        .await
    {
        return refusal(err);
    }
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let page = SetupPage {
        nav: caller.nav(&state, csrf).await,
        manifest: slack::manifest(&state.integrations.current().settings().public_url),
    };
    render(page.render(), set_cookie)
}

pub async fn start_slack_install(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return resource_not_found_page();
    };
    if let Err(err) = caller
        .services
        .require_workspace_admin(&caller.principal())
        .await
    {
        return refusal(err);
    }
    let integrations = state.integrations.current();
    let (Some(app), SlackOffer::Ready) = (integrations.slack(), integrations.slack_offer()) else {
        return workspace_page(&state, &caller, &headers, None).await;
    };
    let held = InstallState::issue(caller.user.workspace_id, state.clock.now());
    if let Err(err) = session.insert(InstallState::SESSION_KEY, &held).await {
        tracing::warn!(error = %err, "slack install: the session could not hold the install state");
        return workspace_page(&state, &caller, &headers, Some(NOT_CONNECTED)).await;
    }
    Redirect::to(&app.approval_url(held.state(), &integrations.slack_return_url())).into_response()
}

pub async fn finish_slack_install(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Query(returned): Query<SlackReturn>,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return resource_not_found_page();
    };
    let principal = caller.principal();
    if let Err(err) = caller.services.require_workspace_admin(&principal).await {
        return refusal(err);
    }
    let held = session
        .remove::<InstallState>(InstallState::SESSION_KEY)
        .await
        .ok()
        .flatten();
    let code = match (held, &returned) {
        (
            Some(held),
            SlackReturn {
                code: Some(code),
                state: Some(returned_state),
                error: None,
            },
        ) if held.accepts(returned_state, caller.user.workspace_id, state.clock.now()) => code,
        _ => return workspace_page(&state, &caller, &headers, Some(NOT_CONNECTED)).await,
    };
    match connect(&state, &caller, &principal, code).await {
        Ok(_) => Redirect::to(WORKSPACE_PATH).into_response(),
        Err(why) => {
            tracing::warn!(reason = why, "slack install: not connected");
            workspace_page(&state, &caller, &headers, Some(NOT_CONNECTED)).await
        }
    }
}

pub async fn show_project(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((team_slug, project_slug)): Path<(String, String)>,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return resource_not_found_page();
    };
    let view = caller
        .services
        .project_integrations(&caller.principal(), &team_slug, &project_slug)
        .await;
    let ProjectIntegrations {
        project_name,
        routes,
    } = match view {
        Ok(view) => view,
        Err(err) => return refusal(err),
    };
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let page = ProjectPage {
        nav: caller.nav(&state, csrf).await,
        project_name,
        routes,
    };
    render(page.render(), set_cookie)
}

/// Exchange the code, read the domain, seal the token for its row, save.
async fn connect(
    state: &AppState,
    caller: &Caller,
    principal: &Principal,
    code: &str,
) -> Result<ConnectionWriteOutcome, &'static str> {
    let integrations: std::sync::Arc<Integrations> = state.integrations.current();
    let (Some(app), Some(secrets)) = (
        integrations.slack(),
        integrations.settings().secrets.as_ref(),
    ) else {
        return Err("slack_not_offered");
    };
    let grant: SlackGrant = app
        .exchange(code, &integrations.slack_return_url())
        .await
        .map_err(|_| "oauth_exchange")?;
    let domain = app
        .domain(&grant.access_token)
        .await
        .map_err(|_| "auth_test")?;
    let connection_id = caller
        .services
        .slack_connection_id(principal)
        .await
        .map_err(|_| "connection_row")?;
    let slot = SecretSlot::new(
        "integration_connections",
        &connection_id.to_string(),
        "sealed_credential",
    );
    let sealed = secrets
        .seal(&grant.access_token, &slot)
        .map_err(|_| "seal")?;
    let install = SlackInstall {
        team_id: grant.team_id,
        team_name: grant.team_name,
        domain,
        app_id: grant.app_id,
        bot_user_id: grant.bot_user_id,
    };
    match caller
        .services
        .connect_slack(principal, install, connection_id, sealed, state.clock.now())
        .await
    {
        Ok(ConnectionWriteOutcome::StaleRow) => Err("stale_row"),
        Ok(outcome) => Ok(outcome),
        Err(_) => Err("store"),
    }
}

async fn workspace_page(
    state: &AppState,
    caller: &Caller,
    headers: &HeaderMap,
    notice: Option<&str>,
) -> Response {
    let WorkspaceIntegrations { slack, activity } = match caller
        .services
        .workspace_integrations(&caller.principal())
        .await
    {
        Ok(view) => view,
        Err(err) => return refusal(err),
    };
    let offer = state.integrations.current().slack_offer();
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let page = WorkspacePage {
        nav: caller.nav(state, csrf).await,
        notice: notice.map(str::to_string),
        not_set_up: offer == SlackOffer::NotSetUp,
        no_secrets_key: offer == SlackOffer::NoSecretsKey,
        slack_line: slack,
        activity,
    };
    render(page.render(), set_cookie)
}

fn refusal(err: IntegrationError) -> Response {
    if let IntegrationError::Store(err) = err {
        tracing::error!(error = %err, "integrations: store read failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
    }
    resource_not_found_page()
}

fn render(rendered: Result<String, askama::Error>, set_cookie: Option<String>) -> Response {
    match rendered {
        Ok(html) => {
            response_with_optional_cookie(StatusCode::OK, Html(html).into_response(), set_cookie)
        }
        Err(err) => {
            tracing::error!(error = %err, "integrations: render failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
        }
    }
}
