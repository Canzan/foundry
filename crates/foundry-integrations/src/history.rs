//! A route's delivery history (US-EIP-05, OD-7): one line per delivery, in
//! Foundry's words. A line is built from stored facts only, never from a
//! vendor's text (UI-7): "14:02 · New issue · AUTH-12 · Delivered".

use crate::{ChannelKind, EventKind, QueuedDelivery, ReasonCode};
use time::OffsetDateTime;

/// One stored delivery (`integration_deliveries`), as the history reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryRecord<'a> {
    /// The route's kind: whose silence an unanswered attempt was.
    pub channel: ChannelKind,
    pub occurred_at: OffsetDateTime,
    /// The stored event wire name, e.g. `issue.created`.
    pub event_kind: &'a str,
    pub issue_key: Option<&'a str>,
    /// `pending`, `delivered`, `failed` or `skipped`.
    pub status: &'a str,
    pub attempts: i32,
    pub first_attempt_at: Option<OffsetDateTime>,
    pub finished_at: Option<OffsetDateTime>,
    pub give_up_at: OffsetDateTime,
    pub lease_until: Option<OffsetDateTime>,
    pub next_attempt_at: Option<OffsetDateTime>,
    /// The vendor's latest reason, as its classifier stored it.
    pub last_vendor_reason: Option<&'a str>,
    /// Why the delivery ended (failed or skipped).
    pub final_reason: Option<&'a str>,
    /// The vendor asked Foundry to slow down at least once.
    pub slowed_down: bool,
}

/// Where a delivery stands at the time the history is read.
enum Standing {
    Delivered,
    GaveUp,
    Ended(&'static str),
    Pending,
}

impl DeliveryRecord<'_> {
    /// The history line for this delivery on the route to `destination`, as
    /// of `now`: time, event, issue key, outcome. The time is when it reached
    /// its outcome (or was first tried, or happened).
    pub fn line(&self, destination: &str, now: OffsetDateTime) -> String {
        let event: &str = match EventKind::from_wire_name(self.event_kind) {
            Some(kind) => kind.display_name(),
            None => self.event_kind,
        };
        let at = self
            .finished_at
            .or(self.first_attempt_at)
            .unwrap_or(self.occurred_at);
        let mut parts = vec![clock(at), event.to_string()];
        if let Some(key) = self.issue_key {
            parts.push(key.to_string());
        }
        parts.push(self.outcome(destination, now));
        parts.join(" · ")
    }

    /// A pending delivery past its give-up time with no live lease has given
    /// up already: the expiry sweep fails it and it is never sent.
    fn standing(&self, now: OffsetDateTime) -> Standing {
        let final_reason = reason(self.final_reason);
        match self.status {
            "delivered" => Standing::Delivered,
            "failed" if final_reason == Some(ReasonCode::GaveUp24h) => Standing::GaveUp,
            "failed" => Standing::Ended("Failed"),
            "skipped" => Standing::Ended("Skipped"),
            _ if self.queued().is_expired_at(now) => Standing::GaveUp,
            _ => Standing::Pending,
        }
    }

    fn outcome(&self, destination: &str, now: OffsetDateTime) -> String {
        match self.standing(now) {
            Standing::Delivered if self.attempts > 1 => format!(
                "Delivered after {} attempts{}",
                self.attempts,
                aside(self.retry_reason(), destination)
            ),
            Standing::Delivered => "Delivered".to_string(),
            Standing::GaveUp => format!(
                "Failed: gave up after 24 hours{}",
                aside(self.retry_reason(), destination)
            ),
            Standing::Ended(word) => match reason(self.final_reason) {
                Some(r) => format!("{word}: {}", r.words(destination)),
                None => word.to_string(),
            },
            Standing::Pending => self.waiting(destination),
        }
    }

    /// Why it took more than one attempt: the vendor's "slow down" first,
    /// then its last recorded reason; an attempt with no recorded answer is
    /// the vendor not answering.
    fn retry_reason(&self) -> Option<ReasonCode> {
        if self.slowed_down {
            return Some(ReasonCode::SlackSlowDown);
        }
        reason(self.last_vendor_reason).or_else(|| {
            if self.attempts > 0 {
                self.channel.no_answer()
            } else {
                None
            }
        })
    }

    /// Not yet tried, being tried, or retrying after a recorded answer.
    fn waiting(&self, destination: &str) -> String {
        if !self.slowed_down && self.last_vendor_reason.is_none() {
            let word = if self.attempts == 0 {
                "Waiting"
            } else {
                "Sending"
            };
            return word.to_string();
        }
        let retrying = format!("Retrying{}", aside(self.retry_reason(), destination));
        match self.next_attempt_at {
            Some(at) => format!("{retrying} · next attempt {}", clock(at)),
            None => retrying,
        }
    }

    fn queued(&self) -> QueuedDelivery {
        QueuedDelivery {
            id: 0,
            next_attempt_at: self.next_attempt_at,
            lease_until: self.lease_until,
            give_up_at: self.give_up_at,
        }
    }
}

fn reason(stored: Option<&str>) -> Option<ReasonCode> {
    stored.and_then(ReasonCode::from_wire_name)
}

/// " (reason)", or nothing.
fn aside(reason: Option<ReasonCode>, destination: &str) -> String {
    reason.map_or(String::new(), |r| format!(" ({})", r.words(destination)))
}

/// "14:02".
pub(crate) fn clock(at: OffsetDateTime) -> String {
    format!("{:02}:{:02}", at.hour(), at.minute())
}
