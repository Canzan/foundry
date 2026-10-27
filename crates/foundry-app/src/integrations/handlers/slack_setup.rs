//! Slack's setup: the manifest page, the install round trip, and Slack's
//! Events API (DD-9, DDR-11, ADR-EIP-005 decisions 4 and 6).

use super::workspace::workspace_page;
use super::{
    internal_error, refusal, render, Caller, LOG_TARGET, NOT_CONNECTED, OTHER_SLACK_WORKSPACE,
    WORKSPACE_PATH,
};
use crate::csrf::ensure_csrf_cookie;
use crate::integrations::slack::{InstallState, SlackEvent};
use crate::integrations::store_capabilities::{ConnectionHealth, StoreCapabilities};
use crate::integrations::{credential_slot, slack, SlackOffer};
use crate::AppState;
use askama::Template;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use foundry_integrations::ReasonCode;
use foundry_services::integrations::SlackInstall;
use foundry_store::integrations::ConnectionWriteOutcome;
use serde::Deserialize;
use tower_sessions::Session;

#[derive(Template)]
#[template(path = "integrations/slack_setup.html")]
struct SetupPage {
    nav: crate::nav::NavContext,
    manifest: String,
}

/// Slack's return: a code and our state, or Slack's `error`.
#[derive(Debug, Default, Deserialize)]
pub struct SlackReturn {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

pub async fn show_slack_setup(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let caller = match Caller::admin(&state, &session).await {
        Ok(caller) => caller,
        Err(response) => return response,
    };
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
    let caller = match Caller::admin(&state, &session).await {
        Ok(caller) => caller,
        Err(response) => return response,
    };
    let unreadable = match caller.unreadable_connections(&state).await {
        Ok(unreadable) => unreadable,
        Err(err) => return refusal(err),
    };
    let integrations = state.integrations.current();
    let (Some(app), SlackOffer::Ready) =
        (integrations.slack(), integrations.slack_offer(unreadable))
    else {
        return workspace_page(&state, &caller, &headers, None).await;
    };
    let held = InstallState::issue(caller.user.workspace_id, state.clock.now());
    if let Err(err) = session.insert(InstallState::SESSION_KEY, &held).await {
        tracing::warn!(
            target: LOG_TARGET,
            error = %err,
            "slack install: the session could not hold the install state",
        );
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
    let caller = match Caller::admin(&state, &session).await {
        Ok(caller) => caller,
        Err(response) => return response,
    };
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
    match connect(&state, &caller, code).await {
        Ok(()) => Redirect::to(WORKSPACE_PATH).into_response(),
        Err(NotConnected::OtherWorkspace {
            approved,
            connected,
        }) => {
            tracing::warn!(
                target: LOG_TARGET,
                reason = "other_workspace",
                "slack install: not connected",
            );
            let words = OTHER_SLACK_WORKSPACE
                .replace("{approved}", &approved)
                .replace("{connected}", &connected);
            workspace_page(&state, &caller, &headers, Some(&words)).await
        }
        Err(NotConnected::Failed(why)) => {
            tracing::warn!(target: LOG_TARGET, reason = why, "slack install: not connected");
            workspace_page(&state, &caller, &headers, Some(NOT_CONNECTED)).await
        }
    }
}

/// Why a Slack install left the workspace's connection as it was.
enum NotConnected {
    /// The approval was for another Slack workspace than the connected one.
    OtherWorkspace {
        approved: String,
        connected: String,
    },
    Failed(&'static str),
}

impl From<&'static str> for NotConnected {
    fn from(why: &'static str) -> Self {
        Self::Failed(why)
    }
}

/// Exchange the code, read the domain, seal the token for its row, save;
/// only while the workspace is offered Slack (not over a connection this
/// Foundry cannot read, DD-2).
async fn connect(state: &AppState, caller: &Caller, code: &str) -> Result<(), NotConnected> {
    let principal = &caller.principal();
    let unreadable = caller
        .unreadable_connections(state)
        .await
        .map_err(|_| "connection_row")?;
    let integrations = state.integrations.current();
    let (Some(app), Some(secrets), SlackOffer::Ready) = (
        integrations.slack(),
        integrations.settings().secrets.as_ref(),
        integrations.slack_offer(unreadable),
    ) else {
        return Err("slack_not_offered".into());
    };
    let grant = app
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
    let sealed = secrets
        .seal(&grant.access_token, &credential_slot(connection_id))
        .map_err(|_| "seal")?;
    let approved = grant.team_name.clone();
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
        Ok(ConnectionWriteOutcome::StaleRow) => Err("stale_row".into()),
        Ok(ConnectionWriteOutcome::OtherWorkspace { connected_label }) => {
            Err(NotConnected::OtherWorkspace {
                approved,
                connected: connected_label,
            })
        }
        Ok(_) => Ok(()),
        Err(_) => Err("store".into()),
    }
}

/// `POST /integrations/slack/events`: Slack's Events API. The signature's
/// freshness is judged on the wall clock, as Slack stamps it.
pub async fn slack_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let integrations = state.integrations.current();
    let Some(app) = integrations.slack() else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let header = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
    let event = app.read_event(
        header("x-slack-request-timestamp"),
        header("x-slack-signature"),
        &body,
        time::OffsetDateTime::now_utc(),
    );
    match event {
        SlackEvent::Unsigned => StatusCode::UNAUTHORIZED.into_response(),
        SlackEvent::Unreadable => StatusCode::BAD_REQUEST.into_response(),
        SlackEvent::UrlVerification { challenge } => {
            Json(serde_json::json!({ "challenge": challenge })).into_response()
        }
        SlackEvent::AppLost {
            team_id,
            app_id,
            reason,
        } => {
            let public_url = &integrations.settings().public_url;
            slack_app_lost(&state, public_url, &team_id, &app_id, reason).await
        }
        SlackEvent::Other => StatusCode::OK.into_response(),
    }
}

/// The Foundry app left the Slack workspace `team_id`: its connection turns
/// lost, and the record that turned it tells the workspace's admins, with the
/// address at `public_url` that reconnects it.
async fn slack_app_lost(
    state: &AppState,
    public_url: &str,
    team_id: &str,
    app_id: &str,
    reason: ReasonCode,
) -> Response {
    let health = StoreCapabilities::new(state.store.clone());
    let lost = health
        .slack_app_lost(team_id, app_id, reason.wire_name(), state.clock.now())
        .await;
    match lost {
        Ok(Some(lost)) => {
            state
                .integrations
                .admin_mail()
                .connection_lost(&lost, public_url)
                .await;
            StatusCode::OK.into_response()
        }
        Ok(None) => StatusCode::OK.into_response(),
        Err(err) => {
            tracing::error!(
                target: LOG_TARGET,
                error = %err,
                "integrations: Slack event not recorded",
            );
            internal_error()
        }
    }
}
