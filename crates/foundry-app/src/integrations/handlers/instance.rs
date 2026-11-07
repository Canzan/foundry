//! The instance integrations page, `GET /admin/instance/integrations`
//! (US-IIS-01): behind `require_instance_admin`, the uniform 404 for everyone
//! else. It only reads. What it shows and offers is [`page_state`], a pure
//! function of the key mode and where the instance key comes from; the posts
//! its buttons carry land in later steps.

use super::render;
use crate::bootstrap::resource_not_found_page;
use crate::csrf::ensure_csrf_cookie;
use crate::integrations::keys::{KeyMode, NoKeyReason};
use crate::integrations::Integrations;
use crate::AppState;
use askama::Template;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use tower_sessions::Session;

pub const INSTANCE_PATH: &str = "/admin/instance/integrations";
pub const SECRETS_KEY_PATH: &str = "/admin/instance/integrations/secrets-key";
pub const MOVE_PATH: &str = "/admin/instance/integrations/secrets-key/move";

/// The instance key file's name in `FOUNDRY_DATA_DIR`.
const KEY_FILE_NAME: &str = "instance.key";

/// I-25, operator-set.
const KEY_BY_OPERATOR: &str = "Instance key: set by the operator.";
/// I-25, made by Foundry.
const KEY_IN_FILE: &str = "Instance key: made by Foundry in {path}. Back up this file apart from the database. Every Foundry server must use the same file. Without it, saved connections must be reconnected.";
/// I-27.
const KEY_UNWRITABLE: &str = "Foundry has no instance key and cannot make one ({path} cannot be written). An instance operator sets the instance key, or gives Foundry a writable data directory, and restarts Foundry.";
/// I-31.
const KEY_UNREADABLE: &str = "This Foundry cannot read its instance key file ({path}). Slack and Matrix are paused until it can. An instance operator restores the file from backup, or sets FOUNDRY_INSTANCE_KEY, and restarts Foundry.";
/// I-2.
const SECRETS_KEY_SET: &str = "Secrets key: set. Credentials entered here are sealed with it.";
/// I-2e.
const DIRECT_SAVED: &str = "Saved credentials are sealed directly with the instance key. Move them under a secrets key to manage it here.";
/// I-2e0.
const DIRECT_NOTHING_SAVED: &str = "Credentials saved here are sealed directly with the instance key. Move to a secrets key to manage it here.";
/// I-3.
const NO_SECRETS_KEY: &str =
    "This Foundry has no secrets key yet. Foundry needs one to store Slack and Matrix credentials.";
/// I-26 (restore-only in Deliverable A).
const SECRETS_KEY_UNOPENABLE: &str = "This Foundry cannot open its secrets key: the instance key it was sealed with is missing or different on this server. Until it is restored, saved connections cannot be used. An instance operator restores that instance key, or sets it as the previous instance key, and restarts Foundry.";
/// I-8.
const MATRIX_READY: &str = "Matrix: ready. Workspace admins connect their own Matrix bot on their workspace's integrations page.";
/// I-9.
const MATRIX_NEEDS_KEY: &str =
    "Matrix: needs the secrets key (above). Nothing else is set up for Matrix on this Foundry.";

/// What the page is judged over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageFacts {
    pub mode: KeyMode,
    /// The operator set the instance key (either setting name).
    pub operator_key: bool,
    /// The instance key file's path, as the page names it.
    pub key_file: String,
    /// Some stored credential is sealed directly under a held instance key.
    pub saved_direct: bool,
}

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
}

/// What the page shows and offers. Never a key, a key id or a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageState {
    /// I-25, or I-27 / I-31 in its place.
    pub instance_key: String,
    /// I-2, I-2e, I-2e0, I-3 or I-26; none without an instance key.
    pub secrets_key: Option<&'static str>,
    pub offer: Option<KeyOffer>,
    /// I-8 or I-9.
    pub matrix: &'static str,
}

/// The page of `facts` (pure and total).
pub fn page_state(facts: &PageFacts) -> PageState {
    let (secrets_key, offer) = match facts.mode {
        KeyMode::NoInstanceKey(_) => (None, None),
        KeyMode::AwaitingSecretsKey => (Some(NO_SECRETS_KEY), Some(KeyOffer::Generate)),
        KeyMode::Direct if facts.saved_direct => (Some(DIRECT_SAVED), Some(KeyOffer::Move)),
        KeyMode::Direct => (Some(DIRECT_NOTHING_SAVED), Some(KeyOffer::Move)),
        KeyMode::Sealed => (Some(SECRETS_KEY_SET), None),
        KeyMode::Unopenable => (Some(SECRETS_KEY_UNOPENABLE), None),
    };
    PageState {
        instance_key: instance_key_line(facts),
        secrets_key,
        offer,
        matrix: if facts.mode.seals() {
            MATRIX_READY
        } else {
            MATRIX_NEEDS_KEY
        },
    }
}

fn instance_key_line(facts: &PageFacts) -> String {
    let words = match facts.mode {
        KeyMode::NoInstanceKey(NoKeyReason::Unwritable) => KEY_UNWRITABLE,
        KeyMode::NoInstanceKey(NoKeyReason::Unreadable) => KEY_UNREADABLE,
        _ if facts.operator_key => KEY_BY_OPERATOR,
        _ => KEY_IN_FILE,
    };
    words.replace("{path}", &facts.key_file)
}

/// The facts of the integrations in effect on this server.
fn facts_of(integrations: &Integrations) -> PageFacts {
    let settings = integrations.settings();
    PageFacts {
        mode: integrations.key_mode(),
        operator_key: settings.key_setting.is_some(),
        key_file: settings.data_dir.join(KEY_FILE_NAME).display().to_string(),
        saved_direct: integrations.holds_direct_seals(),
    }
}

#[derive(Template)]
#[template(path = "integrations/instance.html")]
struct InstancePage {
    nav: crate::nav::NavContext,
    csrf: String,
    page: PageState,
}

pub async fn show_instance(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(user) = crate::instance_admin::require_instance_admin(&state, &session).await else {
        return resource_not_found_page();
    };
    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let page = page_state(&facts_of(&state.integrations.current()));
    let nav = crate::nav::NavContext::home_for(
        &state,
        user.user_id,
        user.workspace_id,
        true,
        csrf.clone(),
    )
    .await;
    render(InstancePage { nav, csrf, page }.render(), set_cookie)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "/data/instance.key";

    fn facts(mode: KeyMode, operator_key: bool, saved_direct: bool) -> PageFacts {
        PageFacts {
            mode,
            operator_key,
            key_file: FILE.to_string(),
            saved_direct,
        }
    }

    fn file_line() -> String {
        KEY_IN_FILE.replace("{path}", FILE)
    }

    /// Facts in (mode, operator key, direct seals), then the page expected.
    type Row = (
        KeyMode,
        bool,
        bool,
        String,
        Option<&'static str>,
        Option<KeyOffer>,
        &'static str,
    );

    /// Every mode, with the key from the operator and from the file, and with
    /// or without direct seals: the page's lines and its one offer.
    #[test]
    fn the_page_follows_the_mode() {
        use KeyMode::*;
        let unwritable = KEY_UNWRITABLE.replace("{path}", FILE);
        let unreadable = KEY_UNREADABLE.replace("{path}", FILE);
        let op = KEY_BY_OPERATOR.to_string();
        #[rustfmt::skip]
        let table: Vec<Row> = vec![
            (NoInstanceKey(NoKeyReason::Unwritable), false, false, unwritable, None, None, MATRIX_NEEDS_KEY),
            (NoInstanceKey(NoKeyReason::Unreadable), false, false, unreadable, None, None, MATRIX_NEEDS_KEY),
            (AwaitingSecretsKey, true, false, op.clone(), Some(NO_SECRETS_KEY), Some(KeyOffer::Generate), MATRIX_NEEDS_KEY),
            (AwaitingSecretsKey, false, false, file_line(), Some(NO_SECRETS_KEY), Some(KeyOffer::Generate), MATRIX_NEEDS_KEY),
            (Direct, true, false, op.clone(), Some(DIRECT_NOTHING_SAVED), Some(KeyOffer::Move), MATRIX_READY),
            (Direct, true, true, op.clone(), Some(DIRECT_SAVED), Some(KeyOffer::Move), MATRIX_READY),
            (Direct, false, true, file_line(), Some(DIRECT_SAVED), Some(KeyOffer::Move), MATRIX_READY),
            (Sealed, true, false, op.clone(), Some(SECRETS_KEY_SET), None, MATRIX_READY),
            (Sealed, false, true, file_line(), Some(SECRETS_KEY_SET), None, MATRIX_READY),
            (Unopenable, true, false, op.clone(), Some(SECRETS_KEY_UNOPENABLE), None, MATRIX_NEEDS_KEY),
            (Unopenable, false, true, file_line(), Some(SECRETS_KEY_UNOPENABLE), None, MATRIX_NEEDS_KEY),
        ];
        for (mode, operator_key, saved_direct, instance_key, secrets_key, offer, matrix) in table {
            let facts = facts(mode, operator_key, saved_direct);
            assert_eq!(
                page_state(&facts),
                PageState {
                    instance_key,
                    secrets_key,
                    offer,
                    matrix
                },
                "{facts:?}"
            );
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
