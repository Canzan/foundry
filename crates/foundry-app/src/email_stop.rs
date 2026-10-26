//! external-integration-plugins US-EIP-11 — a project email's stop link
//! (ADR-EIP-007 decision 5, DDR-16).
//!
//! Every project email names exactly one choice, a (member, project, event)
//! triple, through a signed [`ProjectEmailStopToken`] that never expires
//! (DD-7). The link has three doors:
//!
//! - `GET /email/stop?t=…&sig=…`: the confirm page. It changes nothing, so a
//!   link scanner opening it stops nothing.
//! - `POST /email/stop`: the confirm page's form, CSRF-checked; it deletes
//!   that one choice and says so.
//! - `POST /email/stop/one-click?t=…&sig=…`: the mail client's RFC 8058
//!   one-click, CSRF-exempt (the signature is its authentication), the same
//!   stop, answering 200 with no body.
//!
//! A tampered, unknown or incomplete link gets the shipped uniform refusal
//! page at every door, and changes nothing. Security emails never pass
//! through a choice, so no stop reaches them.

use crate::AppState;
use askama::Template;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use foundry_auth::stop_token::ProjectEmailStopToken;
use foundry_integrations::EmailLines;
use foundry_services::integrations::{EmailStop, IntegrationError};
use foundry_services::Services;
use secrecy::SecretString;
use serde::Deserialize;
use uuid::Uuid;

/// The stop link's confirm page and its form's target.
pub const EMAIL_STOP_PATH: &str = "/email/stop";
/// The RFC 8058 one-click target named by `List-Unsubscribe`.
pub const EMAIL_STOP_ONE_CLICK_PATH: &str = "/email/stop/one-click";
/// `List-Unsubscribe-Post`'s value (RFC 8058).
pub const ONE_CLICK_POST: &str = "List-Unsubscribe=One-Click";

/// The two addresses of one choice's signed stop link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopLinks {
    page: String,
    one_click: String,
}

impl StopLinks {
    /// The links for the choice `(user_id, project_id, event)` on the
    /// Foundry at `public_url`, signed with `secret` (`SESSION_SECRET`).
    pub fn sign(
        public_url: &str,
        secret: &SecretString,
        user_id: Uuid,
        project_id: Uuid,
        event: &str,
    ) -> Option<Self> {
        let token = ProjectEmailStopToken::new(user_id, project_id, event, secret).ok()?;
        let query = format!("?t={}&sig={}", token.choice_param(), token.signature);
        Some(Self {
            page: format!("{public_url}{EMAIL_STOP_PATH}{query}"),
            one_click: format!("{public_url}{EMAIL_STOP_ONE_CLICK_PATH}{query}"),
        })
    }

    /// The email's footer gains its Stop these emails address, just before
    /// its last line (Change what you get).
    pub fn attach(&self, lines: &mut EmailLines) {
        let at = lines.body.len().saturating_sub(1);
        lines
            .body
            .insert(at, format!("Stop these emails: {}", self.page));
    }

    /// `List-Unsubscribe`'s address and `List-Unsubscribe-Post`'s value.
    pub fn unsubscribe(&self) -> (String, String) {
        (self.one_click.clone(), ONE_CLICK_POST.to_string())
    }
}

/// "Stop New comment emails from Identity Platform?"
pub fn stop_question(stop: &EmailStop) -> String {
    format!(
        "Stop {} emails from {}?",
        stop.event.display_name(),
        stop.project_name
    )
}

/// "Stopped: New comment in Identity Platform. Your other choices are unchanged."
pub fn stopped_line(stop: &EmailStop) -> String {
    format!(
        "Stopped: {} in {}. Your other choices are unchanged.",
        stop.event.display_name(),
        stop.project_name
    )
}

/// A stop link's `t` and `sig`, as a query or a form carries them.
#[derive(Debug, Deserialize)]
pub struct StopParams {
    pub t: Option<String>,
    pub sig: Option<String>,
}

impl StopParams {
    /// The choice the link names, when its signature vouches for it.
    fn verified(&self, secret: &SecretString) -> Option<ProjectEmailStopToken> {
        let token = ProjectEmailStopToken::from_params(self.t.as_deref()?, self.sig.as_deref()?)?;
        token.verify(secret).ok()?;
        Some(token)
    }
}

#[derive(Template)]
#[template(path = "email_stop.html")]
struct EmailStopPage {
    question: String,
    stopped: Option<String>,
    csrf: String,
    t: String,
    sig: String,
}

impl EmailStopPage {
    fn html(&self) -> Response {
        Html(self.render().expect("email_stop.html renders")).into_response()
    }
}

/// What a store failure answers.
fn internal_error(err: &foundry_store::StoreError, door: &str) -> Response {
    tracing::error!(%err, door, "email stop: store failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}

/// `GET /email/stop`: the confirm page; nothing changes.
pub async fn show_confirm(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<StopParams>,
) -> Response {
    let Some(token) = params.verified(&state.session_secret) else {
        return crate::unsubscribe::unsubscribe_refusal_page();
    };
    let target = Services::new(state.store.clone())
        .email_stop_target(token.project_id, &token.event)
        .await;
    let stop = match target {
        Ok(stop) => stop,
        Err(IntegrationError::NotFound) => return crate::unsubscribe::unsubscribe_refusal_page(),
        Err(IntegrationError::Store(err)) => return internal_error(&err, "confirm page"),
    };
    let (csrf, set_cookie) = crate::csrf::ensure_csrf_cookie(&state, &headers);
    let page = EmailStopPage {
        question: stop_question(&stop),
        stopped: None,
        csrf,
        t: token.choice_param(),
        sig: token.signature,
    };
    crate::csrf::response_with_optional_cookie(StatusCode::OK, page.html(), set_cookie)
}

/// The stop itself, behind both POST doors; `Err` is the door's answer.
async fn stop(state: &AppState, params: &StopParams, door: &str) -> Result<EmailStop, Response> {
    let token = params
        .verified(&state.session_secret)
        .ok_or_else(crate::unsubscribe::unsubscribe_refusal_page)?;
    Services::new(state.store.clone())
        .stop_project_email(token.user_id, token.project_id, &token.event)
        .await
        .map_err(|err| match err {
            IntegrationError::NotFound => crate::unsubscribe::unsubscribe_refusal_page(),
            IntegrationError::Store(err) => internal_error(&err, door),
        })
}

/// `POST /email/stop` (CSRF-checked): the confirmed stop.
pub async fn confirm(State(state): State<AppState>, Form(params): Form<StopParams>) -> Response {
    match stop(&state, &params, "confirm").await {
        Ok(stopped) => EmailStopPage {
            question: stop_question(&stopped),
            stopped: Some(stopped_line(&stopped)),
            csrf: String::new(),
            t: String::new(),
            sig: String::new(),
        }
        .html(),
        Err(answer) => answer,
    }
}

/// `POST /email/stop/one-click` (CSRF-exempt, RFC 8058): the same stop,
/// 200 with no body.
pub async fn one_click(
    State(state): State<AppState>,
    Query(params): Query<StopParams>,
) -> Response {
    match stop(&state, &params, "one-click").await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(answer) => answer,
    }
}
