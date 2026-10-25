//! What Slack's (and a Matrix homeserver's) answer to a call means (ADR-EIP-005 decision 5): Slack's `ok`
//! field decides, never the status alone, and every answer Foundry cannot
//! read is transient, bounded by the delivery's give-up time.
//!
//! The closed table: `ok: true` is delivered; a 429 (or `ratelimited`) asks
//! Foundry to slow down, for its `Retry-After`; a refusal naming the channel
//! stops that route; a refusal naming the credential or the Slack workspace
//! loses the connection; anything else (no answer, an error page, an unknown
//! code) is Slack not answering, retried.

use crate::{ChannelOutcome, ReasonCode};
use std::time::Duration;

/// One answer Slack gave: its HTTP status, its `Retry-After` and its body
/// (`None` when the body is not JSON).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlackAnswer<'a> {
    pub status: u16,
    pub retry_after: Option<Duration>,
    pub body: Option<&'a serde_json::Value>,
}

/// HTTP 429.
const TOO_MANY_REQUESTS: u16 = 429;

/// `(Slack error code, what it means)`: Slack's documented refusals that
/// concern the destination or the connection.
const REFUSALS: [(&str, ChannelOutcome); 9] = [
    ("is_archived", route(ReasonCode::ChannelArchived)),
    ("channel_is_archived", route(ReasonCode::ChannelArchived)),
    ("not_in_channel", route(ReasonCode::NotInChannel)),
    ("channel_not_found", route(ReasonCode::ChannelNotFound)),
    ("token_revoked", connection(ReasonCode::TokenRevoked)),
    ("invalid_auth", connection(ReasonCode::TokenRevoked)),
    ("not_authed", connection(ReasonCode::TokenRevoked)),
    ("account_inactive", connection(ReasonCode::AppRemoved)),
    (
        "team_access_not_granted",
        connection(ReasonCode::AppRemoved),
    ),
];

const fn route(reason: ReasonCode) -> ChannelOutcome {
    ChannelOutcome::RouteRefused { reason }
}

const fn connection(reason: ReasonCode) -> ChannelOutcome {
    ChannelOutcome::ConnectionRefused { reason }
}

fn no_answer() -> ChannelOutcome {
    ChannelOutcome::Transient {
        reason: ReasonCode::SlackNoAnswer,
        retry_after: None,
    }
}

fn slow_down(retry_after: Option<Duration>) -> ChannelOutcome {
    ChannelOutcome::Transient {
        reason: ReasonCode::SlackSlowDown,
        retry_after,
    }
}

/// One answer to a call, `None` when Slack gave none.
pub fn classify_slack(answer: Option<SlackAnswer<'_>>) -> ChannelOutcome {
    let Some(answer) = answer else {
        return no_answer();
    };
    if answer.status == TOO_MANY_REQUESTS {
        return slow_down(answer.retry_after);
    }
    let Some(body) = answer.body else {
        return no_answer();
    };
    match body["ok"].as_bool() {
        Some(true) => ChannelOutcome::Delivered,
        Some(false) => refusal(body["error"].as_str(), answer.retry_after),
        None => no_answer(),
    }
}

fn refusal(code: Option<&str>, retry_after: Option<Duration>) -> ChannelOutcome {
    if code == Some("ratelimited") {
        return slow_down(retry_after);
    }
    REFUSALS
        .iter()
        .find(|(known, _)| Some(*known) == code)
        .map_or_else(no_answer, |(_, outcome)| *outcome)
}

/// One answer a Matrix homeserver gave (ADR-EIP-005's rule, ADR-EIP-006
/// decision 5): its HTTP status and its body (`None` when not JSON).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatrixAnswer<'a> {
    pub status: u16,
    pub body: Option<&'a serde_json::Value>,
}

/// HTTP 401 and 403.
const UNAUTHORIZED: u16 = 401;
const FORBIDDEN: u16 = 403;

/// One answer to a Matrix call, `None` when the homeserver gave none. The
/// closed table: a success is delivered; 429 or `M_LIMIT_EXCEEDED` asks
/// Foundry to wait its `retry_after_ms`; `M_UNKNOWN_TOKEN` loses the
/// connection; `M_FORBIDDEN` stops the route (the bot is not in the room);
/// anything else is the homeserver not answering, retried.
pub fn classify_matrix(answer: Option<MatrixAnswer<'_>>) -> ChannelOutcome {
    let homeserver_no_answer = |retry_after| ChannelOutcome::Transient {
        reason: ReasonCode::HomeserverNoAnswer,
        retry_after,
    };
    let Some(answer) = answer else {
        return homeserver_no_answer(None);
    };
    if (200..300).contains(&answer.status) {
        return ChannelOutcome::Delivered;
    }
    let code = answer.body.and_then(|b| b["errcode"].as_str());
    match (answer.status, code) {
        (TOO_MANY_REQUESTS, _) | (_, Some("M_LIMIT_EXCEEDED")) => {
            let wait = answer
                .body
                .and_then(|b| b["retry_after_ms"].as_u64())
                .map(Duration::from_millis);
            homeserver_no_answer(wait)
        }
        (UNAUTHORIZED, Some("M_UNKNOWN_TOKEN")) => connection(ReasonCode::TokenRevoked),
        (FORBIDDEN, Some("M_FORBIDDEN")) => route(ReasonCode::BotNotInRoom),
        _ => homeserver_no_answer(None),
    }
}
