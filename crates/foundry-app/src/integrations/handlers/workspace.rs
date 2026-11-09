//! The workspace integrations page and the disconnect doors, Slack's and
//! Matrix's (K-1, K-2, K-20, R-4, US-EIP-07).

use super::{
    refusal, render, sign_in, unavailable_words, Caller, CANNOT_REMOVE_ACCESS, DISCONNECT_PATH,
    LOG_TARGET, MATRIX_CONNECT_PATH, MATRIX_DISCONNECT_PATH, WORKSPACE_PATH,
};
use crate::csrf::ensure_csrf_cookie;
use crate::integrations::no_connection::{ConnectionWords, NoConnectionCause, Viewer};
use crate::integrations::{MatrixOffer, SlackOffer};
use crate::AppState;
use askama::Template;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use foundry_integrations::{ChannelKind, ConnectionLostNotice, ReasonCode};
use foundry_services::integrations::{LostSlack, MatrixLine, WorkspaceIntegrations};
use foundry_store::integrations::{DisconnectOutcome, DisconnectPreview};
use tower_sessions::Session;

#[derive(Template)]
#[template(path = "integrations/workspace.html")]
struct WorkspacePage {
    nav: crate::nav::NavContext,
    notice: Option<String>,
    slack_line: Option<String>,
    /// K-10: the connection was lost; offer to reconnect it.
    reconnect: bool,
    /// W-2 or W-3 for Slack, and the ways on it offers, as every door words
    /// them: the one source for the sentence and its links (ADR-IW-001).
    slack_no_connection: Option<ConnectionWords>,
    /// K-19: the saved connection cannot be read by this Foundry (DD-2);
    /// Disconnect is offered beside it, as a repair.
    unavailable: bool,
    /// K-19 for Slack and for Matrix, as every door words it.
    slack_unavailable_words: String,
    matrix_unavailable_words: String,
    /// The Matrix connection line and its token field, when connected.
    matrix: Option<MatrixLine>,
    /// Matrix's connect form (Connect Matrix, or Replace token): the secrets
    /// key is set, so a token can be sealed.
    matrix_offered: bool,
    /// K-18 / W-2 Matrix: no secrets key, so no bot token can be sealed; the
    /// same sentence and offers the route form shows.
    matrix_no_connection: Option<ConnectionWords>,
    /// K-19 for Matrix: the saved connection cannot be read by this Foundry
    /// (DD-2); Disconnect and, with a key, Replace token beside it.
    matrix_unavailable: bool,
    matrix_connect_path: &'static str,
    matrix_disconnect_path: &'static str,
    csrf: String,
    activity: Vec<String>,
}

#[derive(Template)]
#[template(path = "integrations/slack_disconnect.html")]
struct DisconnectPage {
    nav: crate::nav::NavContext,
    csrf: String,
    /// "Slack" or "Matrix".
    kind: &'static str,
    question: String,
    disconnect_path: &'static str,
    workspace_path: &'static str,
}

pub async fn show_workspace(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(caller) = Caller::of(&state, &session).await else {
        return sign_in();
    };
    workspace_page(&state, &caller, &headers, None).await
}

pub async fn show_slack_disconnect(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    show_disconnect(&state, &session, &headers, ChannelKind::Slack).await
}

pub async fn show_matrix_disconnect(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    show_disconnect(&state, &session, &headers, ChannelKind::Matrix).await
}

/// The disconnect question for the workspace's connection of `kind`
/// (admins; anyone else gets the uniform not-found page).
async fn show_disconnect(
    state: &AppState,
    session: &Session,
    headers: &HeaderMap,
    kind: ChannelKind,
) -> Response {
    let Some(caller) = Caller::of(state, session).await else {
        return sign_in();
    };
    let preview = match caller
        .services
        .disconnect_preview(&caller.principal(), kind.wire_name())
        .await
    {
        Ok(Some(preview)) => preview,
        Ok(None) => return Redirect::to(WORKSPACE_PATH).into_response(),
        Err(err) => return refusal(err),
    };
    let unreadable = match caller.unreadable_connections(state).await {
        Ok(unreadable) => unreadable,
        Err(err) => return refusal(err),
    };
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let page = DisconnectPage {
        nav: caller.nav(state, csrf.clone()).await,
        csrf,
        kind: kind.display_name(),
        question: disconnect_question(kind, &preview, unreadable.slack),
        disconnect_path: disconnect_path(kind),
        workspace_path: WORKSPACE_PATH,
    };
    render(page.render(), set_cookie)
}

fn disconnect_path(kind: ChannelKind) -> &'static str {
    match kind {
        ChannelKind::Matrix => MATRIX_DISCONNECT_PATH,
        _ => DISCONNECT_PATH,
    }
}

/// The workspace's connection of `kind` disconnected (admins; anyone else
/// gets the uniform not-found page): its credential forgotten and its routes
/// paused, in one transaction.
async fn disconnect(
    state: &AppState,
    session: &Session,
    kind: ChannelKind,
) -> Result<DisconnectOutcome, Response> {
    let caller = Caller::of(state, session).await.ok_or_else(sign_in)?;
    caller
        .services
        .disconnect(&caller.principal(), kind.wire_name(), state.clock.now())
        .await
        .map_err(refusal)
}

/// The Matrix disconnect: nothing is asked of the homeserver (the bot stays
/// in its rooms).
pub async fn disconnect_matrix(State(state): State<AppState>, session: Session) -> Response {
    match disconnect(&state, &session, ChannelKind::Matrix).await {
        Ok(_) => Redirect::to(WORKSPACE_PATH).into_response(),
        Err(response) => response,
    }
}

/// The Slack disconnect: Foundry forgets the credential first, then asks
/// Slack to withdraw the access it granted, so a Slack failure only goes to
/// the log.
pub async fn disconnect_slack(State(state): State<AppState>, session: Session) -> Response {
    let outcome = match disconnect(&state, &session, ChannelKind::Slack).await {
        Ok(outcome) => outcome,
        Err(response) => return response,
    };
    if let DisconnectOutcome::Disconnected {
        connection_id,
        sealed_credential,
        ..
    } = outcome
    {
        let withdrawn = state
            .integrations
            .current()
            .withdraw_slack_access(connection_id, sealed_credential)
            .await;
        if !withdrawn {
            tracing::warn!(
                target: LOG_TARGET,
                reason = "apps_uninstall",
                "slack disconnect: Slack did not withdraw Foundry's access; the credential is forgotten"
            );
        }
    }
    Redirect::to(WORKSPACE_PATH).into_response()
}

/// US-EIP-07's disconnect question: "Disconnect Slack? 3 routes in 2
/// projects stop posting. Foundry's access to Canzan Labs' Slack is removed."
/// For Matrix (DIST-25): "… Foundry forgets the bot's access token." For a
/// Slack connection this Foundry cannot read (`slack_unreadable`), R-13 in
/// place of the last sentence: without the token Slack cannot be asked.
fn disconnect_question(
    kind: ChannelKind,
    preview: &DisconnectPreview,
    slack_unreadable: bool,
) -> String {
    let count = |n: i64, noun: &str| {
        if n == 1 {
            format!("1 {noun}")
        } else {
            format!("{n} {noun}s")
        }
    };
    let verb = if preview.routes == 1 { "stops" } else { "stop" };
    let consequence = match kind {
        ChannelKind::Matrix => "Foundry forgets the bot's access token.".to_string(),
        _ => {
            let owner = if preview.label.ends_with('s') {
                format!("{}'", preview.label)
            } else {
                format!("{}'s", preview.label)
            };
            if slack_unreadable {
                CANNOT_REMOVE_ACCESS.replace("{owner}", &owner)
            } else {
                format!("Foundry's access to {owner} Slack is removed.")
            }
        }
    };
    format!(
        "Disconnect {}? {} in {} {verb} posting. {consequence}",
        kind.display_name(),
        count(preview.routes, "route"),
        count(preview.projects, "project"),
    )
}

pub(super) async fn workspace_page(
    state: &AppState,
    caller: &Caller,
    headers: &HeaderMap,
    notice: Option<&str>,
) -> Response {
    let WorkspaceIntegrations {
        slack,
        slack_lost,
        matrix,
        activity,
    } = match caller
        .services
        .workspace_integrations(&caller.principal())
        .await
    {
        Ok(view) => view,
        Err(err) => return refusal(err),
    };
    let unreadable = match caller.unreadable_connections(state).await {
        Ok(unreadable) => unreadable,
        Err(err) => return refusal(err),
    };
    let integrations = state.integrations.current();
    let offer = integrations.slack_offer(unreadable);
    let matrix_offer = integrations.matrix_offer(unreadable);
    let reconnect = slack_lost.is_some();
    let slack = slack.or_else(|| slack_lost.map(|lost| lost_words(ChannelKind::Slack, &lost)));
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let nav = caller.nav(state, csrf.clone()).await;
    // The services view above answers only a workspace admin, so the
    // workspace's own links are this viewer's; whether this Foundry's setup
    // is too is the nav's fact, fail-closed (US-IIS-04).
    let viewer = Viewer::new(true, nav.is_instance_admin);
    // W-2 and W-3 only: a connection the workspace can make, has lost, or
    // this Foundry cannot read keeps the page's own arms below.
    let no_connection = |cause: Option<NoConnectionCause>| {
        cause
            .filter(|cause| cause.awaits_this_foundry())
            .map(|cause| cause.words(viewer))
    };
    let slack_no_connection = no_connection(NoConnectionCause::of_slack(offer, slack.is_some()));
    let matrix_no_connection =
        no_connection(NoConnectionCause::of_matrix(matrix_offer, matrix.is_some()));
    let page = WorkspacePage {
        nav,
        csrf,
        matrix_offered: integrations.sealing_box().is_some(),
        matrix_no_connection,
        matrix_unavailable: matrix_offer == MatrixOffer::Unavailable,
        matrix,
        matrix_connect_path: MATRIX_CONNECT_PATH,
        matrix_disconnect_path: MATRIX_DISCONNECT_PATH,
        notice: notice.map(str::to_string),
        slack_no_connection,
        unavailable: offer == SlackOffer::Unavailable,
        slack_unavailable_words: unavailable_words(ChannelKind::Slack),
        matrix_unavailable_words: unavailable_words(ChannelKind::Matrix),
        slack_line: slack,
        reconnect,
        activity,
    };
    render(page.render(), set_cookie)
}

/// K-10 for the workspace's lost connection of `kind`, up to its Reconnect
/// action: the workspace page's words, and the project page's.
pub(crate) fn lost_words(kind: ChannelKind, lost: &LostSlack) -> String {
    ConnectionLostNotice {
        kind,
        workspace: lost.workspace_name.clone(),
        reason: lost.reason.as_deref().and_then(ReasonCode::from_wire_name),
    }
    .words()
}
