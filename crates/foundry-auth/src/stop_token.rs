//! external-integration-plugins — the project email stop link's token
//! (ADR-EIP-007 decision 5, DDR-16).
//!
//! A [`ProjectEmailStopToken`] names exactly one email choice, a (member,
//! project, event) triple, and an HMAC over
//! `stop|v1|{user_id}|{project_id}|{event}` keyed on `SESSION_SECRET`: the
//! [`crate::UnsubscribeToken`] shape, with its own `stop` domain prefix so a
//! workspace unsubscribe signature never verifies as a stop, nor the reverse.
//! It carries no time: a stop link works for as long as its email exists
//! (DD-7). Verification is constant time ([`crate::verify`]).

use crate::{sign, verify, AuthError};
use secrecy::SecretString;
use uuid::Uuid;

/// One email choice and the signature that vouches for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectEmailStopToken {
    pub user_id: Uuid,
    pub project_id: Uuid,
    /// The event's wire name, e.g. `comment.added`.
    pub event: String,
    pub signature: String,
}

impl ProjectEmailStopToken {
    /// Sign the choice `(user_id, project_id, event)` with `secret` (the
    /// application's `SESSION_SECRET`).
    pub fn new(
        user_id: Uuid,
        project_id: Uuid,
        event: &str,
        secret: &SecretString,
    ) -> Result<Self, AuthError> {
        let signature = sign(secret, stop_payload(user_id, project_id, event).as_bytes())?;
        Ok(Self {
            user_id,
            project_id,
            event: event.to_string(),
            signature,
        })
    }

    /// Whether the signature vouches for exactly this choice. Any change to
    /// a field or to the signature fails.
    pub fn verify(&self, secret: &SecretString) -> Result<(), AuthError> {
        let payload = stop_payload(self.user_id, self.project_id, &self.event);
        verify(secret, payload.as_bytes(), &self.signature)
    }

    /// The link's opaque `t` parameter: `{user_id}.{project_id}.{event}`.
    pub fn choice_param(&self) -> String {
        format!("{}.{}.{}", self.user_id, self.project_id, self.event)
    }

    /// The token a link's `t` and `sig` parameters carry, unverified; `None`
    /// when `t` does not name a choice.
    pub fn from_params(choice: &str, signature: &str) -> Option<Self> {
        let mut parts = choice.splitn(3, '.');
        let user_id = parts.next()?.parse().ok()?;
        let project_id = parts.next()?.parse().ok()?;
        let event = parts.next().filter(|e| !e.is_empty())?;
        Some(Self {
            user_id,
            project_id,
            event: event.to_string(),
            signature: signature.to_string(),
        })
    }
}

/// The domain-separated, versioned stop payload.
fn stop_payload(user_id: Uuid, project_id: Uuid, event: &str) -> String {
    format!("stop|v1|{user_id}|{project_id}|{event}")
}
