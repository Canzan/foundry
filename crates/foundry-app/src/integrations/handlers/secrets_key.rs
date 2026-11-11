//! The secrets-key doors of the instance integrations page: what the page
//! offers for the key ([`KeyOffer`]), the one write use-case behind Generate
//! and Move, and the words a post that stored the key answers
//! ([`stored_notice`]).
//!
//! Generate applies only where [`KeyMode::generates`] holds; anywhere else it
//! changes nothing and answers the page as it is. It builds the plan (a new
//! key wrapped under the instance key, and its canary), hands it to the one
//! write use-case, and after the commit rebuilds this server's integrations
//! through the shared boot probe
//! ([`Integrations::reboot`](crate::integrations::Integrations::reboot)) and
//! installs them before it answers I-21. Other replicas follow on their
//! reload (ADR-IIS-004).
//!
//! Move, `POST …/secrets-key/move` (US-IIS-06), is the same use-case where
//! [`KeyMode::moves`] holds: every value sealed directly under a held
//! instance key is re-sealed under the new key in the same transaction, and
//! it answers I-22 (or I-21 with nothing saved). Both gates are judged over
//! the stored rows as they are at the post, never the last rebuild's, and
//! both are withheld under I-4.

use super::instance::{answer, MOVE_PATH, SECRETS_KEY_PATH};
use super::reinstall;
use crate::bootstrap::resource_not_found_page;
use crate::integrations::keys::{plan_generate, KeyMode};
use crate::integrations::sealed_slot;
use crate::integrations::store_capabilities::{
    InstanceKeyWrites, KeyProbe, ResealRefused, StoreCapabilities,
};
use crate::AppState;
use axum::http::HeaderMap;
use axum::response::Response;
use foundry_store::integrations::{KeyWrapper, SecretsKeyOutcome};
use tower_sessions::Session;
use uuid::Uuid;

/// I-3.
pub(super) const NO_SECRETS_KEY: &str =
    "This Foundry has no secrets key yet. Foundry needs one to store Slack and Matrix credentials.";
/// I-22, n = 1.
const MOVED_ONE: &str =
    "Moved. 1 saved credential is sealed under a new secrets key. Nobody needs to reconnect.";
/// I-22, n ≥ 2.
const MOVED_MANY: &str =
    "Moved. {n} saved credentials are sealed under a new secrets key. Nobody needs to reconnect.";
/// I-21.
const SECRETS_KEY_GENERATED: &str = "Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it.";

/// The key action the page offers: a submit control carrying the copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOffer {
    /// I-3's `Generate secrets key`.
    Generate,
    /// I-2e / I-2e0's `Move to a secrets key`.
    Move,
}

impl KeyOffer {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Generate => "Generate secrets key",
            Self::Move => "Move to a secrets key",
        }
    }

    pub fn door(&self) -> &'static str {
        match self {
            Self::Generate => SECRETS_KEY_PATH,
            Self::Move => MOVE_PATH,
        }
    }

    /// Whether this door applies in `mode`, with I-4 holding or not.
    pub fn applies(self, mode: KeyMode, unopenable_values: bool) -> bool {
        match self {
            Self::Generate => mode.generates(unopenable_values),
            Self::Move => mode.moves(unopenable_values),
        }
    }
}

/// What a Generate or Move that stored the key answers: I-21 when nothing
/// was re-sealed, else I-22 over the `moved` credentials.
pub fn stored_notice(moved: usize) -> String {
    match moved {
        0 => SECRETS_KEY_GENERATED.to_string(),
        1 => MOVED_ONE.to_string(),
        n => MOVED_MANY.replace("{n}", &n.to_string()),
    }
}

/// Generate's and Move's door: the uniform 404 past the guard, else `door`
/// applied and the page answered.
pub(super) async fn key_door(
    state: &AppState,
    session: &Session,
    headers: &HeaderMap,
    door: KeyOffer,
) -> Response {
    let Some(user) = crate::instance_admin::require_instance_admin(state, session).await else {
        return resource_not_found_page();
    };
    let notice = store_secrets_key(state, user.user_id, door).await;
    answer(state, &user, headers, notice, String::new()).await
}

/// Generate or Move (one use-case, ADR-IIS-001 decision 5) where `door`
/// applies over the stored rows as they are now, then this server's
/// integrations rebuilt and installed: I-21 or I-22 when this post stored
/// the key, else nothing to tell.
async fn store_secrets_key(state: &AppState, actor: Uuid, door: KeyOffer) -> Option<String> {
    let store = StoreCapabilities::new(state.store.clone());
    let integrations = match store.instance_snapshot().await {
        Ok(snapshot) => state.integrations.current().rebuilt(Ok(snapshot)),
        Err(err) => {
            tracing::warn!(error = %err, "reading the stored credentials failed");
            return None;
        }
    };
    if !door.applies(
        integrations.key_mode(),
        integrations.secrets_check().degraded(),
    ) {
        return None;
    }
    let settings = integrations.settings();
    let wrapped_by = match settings.key_setting {
        Some(_) => KeyWrapper::Operator,
        None => KeyWrapper::File,
    };
    let instance = settings.secrets.as_ref()?;
    let Some(plan) = plan_generate(instance, wrapped_by, Some(actor)) else {
        tracing::warn!(probe = "secrets", "generating the secrets key failed");
        return None;
    };
    let reseal = |slot, sealed: &[u8]| {
        plan.reseal(instance, &sealed_slot(slot), sealed)
            .map(Some)
            .ok_or(ResealRefused)
    };
    let now = state.clock.now();
    let outcome = store
        .store_secrets_key(&plan.key, &plan.canary, now, &reseal)
        .await;
    let notice = match outcome {
        Ok(SecretsKeyOutcome::Stored { resealed, .. }) => Some(stored_notice(resealed)),
        Ok(SecretsKeyOutcome::AlreadyStored { .. }) => None,
        Ok(SecretsKeyOutcome::Refused(ResealRefused)) => {
            tracing::warn!(
                probe = "secrets",
                "a stored credential could not be re-sealed; nothing changed"
            );
            None
        }
        Err(err) => {
            tracing::warn!(error = %err, "storing the secrets key failed");
            return None;
        }
    };
    reinstall(state, &integrations).await;
    notice
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a Generate or Move that stored the key answers, over n in
    /// {0, 1, 2, many}: I-21 for none, I-22 singular for one, plural after.
    #[test]
    fn the_answer_names_how_many_credentials_moved() {
        let i21 = "Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it.";
        let table = [
            (0, i21.to_string()),
            (1, "Moved. 1 saved credential is sealed under a new secrets key. Nobody needs to reconnect.".to_string()),
            (2, "Moved. 2 saved credentials are sealed under a new secrets key. Nobody needs to reconnect.".to_string()),
            (137, "Moved. 137 saved credentials are sealed under a new secrets key. Nobody needs to reconnect.".to_string()),
        ];
        for (moved, words) in table {
            assert_eq!(stored_notice(moved), words, "n = {moved}");
        }
    }

    #[test]
    fn each_offer_posts_to_its_own_door() {
        assert_eq!(KeyOffer::Generate.label(), "Generate secrets key");
        assert_eq!(KeyOffer::Generate.door(), SECRETS_KEY_PATH);
        assert_eq!(KeyOffer::Move.label(), "Move to a secrets key");
        assert_eq!(KeyOffer::Move.door(), MOVE_PATH);
    }
}
