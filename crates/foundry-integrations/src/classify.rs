//! What Slack's answer to a call means (ADR-EIP-005 decision 5): Slack's `ok`
//! field decides, never the status alone, and every answer Foundry cannot
//! read is transient, bounded by the delivery's give-up time.
//!
//! Slice 01 reads `ok: true` as delivered and anything else as Slack not
//! answering; the closed table of refusal codes (channel archived, not in
//! channel, token revoked, a request to slow down) is 03-01's
//! (`classify_slack_recorded_answers`).

use crate::{ChannelOutcome, ReasonCode};

/// One answer to a call: its JSON body, `None` when no readable answer came.
pub fn classify_slack(body: Option<&serde_json::Value>) -> ChannelOutcome {
    match body.and_then(|answer| answer["ok"].as_bool()) {
        Some(true) => ChannelOutcome::Delivered,
        _ => ChannelOutcome::Transient {
            reason: ReasonCode::SlackNoAnswer,
            retry_after: None,
        },
    }
}
