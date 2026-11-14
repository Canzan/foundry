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
//!
//! Replace, `GET`/`POST …/secrets-key/replace` (US-IIS-07 AC 1), where
//! [`KeyMode::replaces`] holds: the control on the page opens the `GET`,
//! which reads I-23 over the credentials stored now and offers the one
//! control that carries the replacement out. That page only reads
//! ([`ReadOnlyDoor`]), so a prefetch and a link preview replace nothing. The
//! `POST` wraps a new secrets key under the instance key this server holds,
//! seals every stored value again under it in one locked transaction,
//! rebuilds this server's integrations and sends her to the page itself to
//! read I-24 once ([`SavedNotice`]). A refused re-seal leaves the old key in
//! effect with every credential still opening and asks the question again; a
//! replacement another admin made first answers the page as it is.
//!
//! Start over, `GET …/secrets-key/start-over` (US-IIS-07 AC 3), where the
//! stored key cannot be opened at all (I-26) and no running Foundry server of
//! this database still opens it ([`UnopenableOffer`]): the page appends the
//! way out to the shipped restore words, and this door reads I-28 over the
//! credentials this Foundry cannot open with the I-28a Confirmation it asks
//! her to type first. It only reads ([`ReadOnlyDoor`]), so a prefetch, a link
//! preview and a reload discard nothing. Anywhere the way out does not apply
//! (a key that opens, a live server that still opens it, or no instance key
//! read at all) it changes nothing and answers the instance page, as the
//! replace door does. Its `POST`, which discards what it cannot open, lands
//! with the typed confirmation and the live-replica gate read again inside the
//! write. Why the way out is withheld decides what that `POST` answers
//! ([`ConfirmedStartOver`]): a live server that still opens the key is asked
//! the cost again, while a state that offers no way out at all - a stored key
//! this Foundry opens first among them - changes nothing and answers the
//! instance page, so a tab left open never reads the discard-everything
//! control on a Foundry whose key works.

use super::instance::{
    answer, gate_of, ReadOnlyDoor, SavedNotice, MOVE_PATH, REPLACE_PATH, SECRETS_KEY_PATH,
    START_OVER_PATH,
};
use super::{reinstall, render};
use crate::bootstrap::{resource_not_found_page, SessionUser};
use crate::csrf::ensure_csrf_cookie;
use crate::integrations::keys::{plan_generate, KeyMode};
use crate::integrations::sealed_slot;
use crate::integrations::secrets_key_rotation::{
    ConfirmedStartOver, ReplacePlan, StartOverConfirmation, StartOverPlan, UnopenableOffer,
};
use crate::integrations::store_capabilities::{
    InstanceKeyWrites, KeyProbe, ResealRefused, StoreCapabilities,
};
use crate::integrations::Integrations;
use crate::AppState;
use askama::Template;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::Form;
use foundry_store::integrations::{ReplaceOutcome, SecretsKeyOutcome, StartOverOutcome};
use time::OffsetDateTime;
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
pub(super) const SECRETS_KEY_GENERATED: &str = "Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it.";
/// The words on the way out of a stored key this Foundry cannot open: the
/// page's link to the confirm, and the confirm's own control (I-26, I-28).
pub(super) const START_OVER_CONTROL: &str = "Start over";
/// I-28a: what the Confirmation field asks her to type, and the one thing the
/// gate cannot see for her - a Foundry server of an earlier release that beats
/// no state row at all.
const TYPE_START_OVER: &str =
    "Type START OVER to confirm that no Foundry server of an earlier release is still running.";
/// I-28a's field label, as she reads it beside the box she types in.
const CONFIRMATION: &str = "Confirmation";

/// The key action the page offers: a control carrying the copy, the door it
/// reaches and when it applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOffer {
    /// I-3's `Generate secrets key`.
    Generate,
    /// I-2e / I-2e0's `Move to a secrets key`.
    Move,
    /// I-2's `Replace secrets key` (US-IIS-07 AC 1): it opens the page that
    /// reads I-23 first, so nothing is re-sealed before she confirms.
    Replace,
}

impl KeyOffer {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Generate => "Generate secrets key",
            Self::Move => "Move to a secrets key",
            Self::Replace => "Replace secrets key",
        }
    }

    pub fn door(&self) -> &'static str {
        match self {
            Self::Generate => SECRETS_KEY_PATH,
            Self::Move => MOVE_PATH,
            Self::Replace => REPLACE_PATH,
        }
    }

    /// The door this control opens by itself, where choosing it only asks a
    /// question: Replace opens the page that reads I-23 before anything is
    /// sealed again. Generate and Move open none, because each is the submit
    /// control of its own write.
    pub fn opens(self) -> Option<&'static str> {
        match self {
            Self::Generate | Self::Move => None,
            Self::Replace => Some(self.door()),
        }
    }

    /// Whether this door applies in `mode`, with I-4 holding or not.
    pub fn applies(self, mode: KeyMode, unopenable_values: bool) -> bool {
        match self {
            Self::Generate => mode.generates(unopenable_values),
            Self::Move => mode.moves(unopenable_values),
            Self::Replace => mode.replaces(unopenable_values),
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
    let wrapped_by = settings.key_wrapper();
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

/// I-23: how many saved credentials a replacement seals again, and the one
/// control that carries it out. Never a key and never a form field: the
/// replacement needs nothing typed.
#[derive(Template)]
#[template(path = "integrations/secrets_key_replace.html")]
struct SecretsKeyReplacePage {
    nav: crate::nav::NavContext,
    csrf: String,
    /// Where the one control posts.
    replace_door: &'static str,
    /// I-23, in the form the saved-credential count selects.
    line: String,
    /// The control's copy.
    control: &'static str,
}

/// I-28: what starting over discards, with the I-28a Confirmation she types
/// before the one control that carries it out. Never a key and never a
/// credential: the page states a cost and holds nothing.
#[derive(Template)]
#[template(path = "integrations/secrets_key_start_over.html")]
struct SecretsKeyStartOverPage {
    nav: crate::nav::NavContext,
    csrf: String,
    /// Where the one control posts.
    start_over_door: &'static str,
    /// I-28, in the form the count of credentials this Foundry cannot open
    /// selects.
    line: String,
    /// I-28a's field label.
    confirmation: &'static str,
    /// I-28a: what she types in it, and why.
    asks: &'static str,
    /// The control's copy.
    control: &'static str,
}

/// What a replacement answers (D7, L9): only a committed replacement earns
/// I-24 and the redirect onto the page itself (post, redirect, get, so a
/// reload posts nothing again); a refused re-seal left everything as it was,
/// so the question is asked again; anything that stored nothing answers the
/// page in place, with nothing to tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplaceAnswer {
    Replaced(SavedNotice),
    AskAgain,
    Unchanged,
}

/// `GET /admin/instance/integrations/secrets-key/replace`: I-23 over the
/// credentials stored now, with the one control that replaces the key. It
/// only reads ([`ReadOnlyDoor`]), so a prefetch, a link preview and a reload
/// replace nothing (L4, C2).
pub async fn show_secrets_key_replace(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(user) = crate::instance_admin::require_instance_admin(&state, &session).await else {
        return resource_not_found_page();
    };
    replace_page(&state, &user, &headers).await
}

/// `POST /admin/instance/integrations/secrets-key/replace`: the replacement
/// where Replace applies over the stored rows as they are at the post, and
/// the admin sent to the page itself to read I-24 once. A refused re-seal
/// asks the question again with nothing changed; a replacement another admin
/// made first, and one anywhere Replace does not apply, change nothing and
/// answer the page as it is (B1, B2, D8).
pub async fn replace_secrets_key(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(user) = crate::instance_admin::require_instance_admin(&state, &session).await else {
        return resource_not_found_page();
    };
    let store = StoreCapabilities::new(state.store.clone());
    match replaced(&state, &*store, user.user_id).await {
        ReplaceAnswer::Replaced(notice) => notice.answered(&session).await,
        ReplaceAnswer::AskAgain => replace_page(&state, &user, &headers).await,
        ReplaceAnswer::Unchanged => answer(&state, &user, &headers, None, String::new()).await,
    }
}

/// The replace page over the credentials stored now. Where Replace does not
/// apply, and where the count could not be read - an unread count is not none
/// saved (B6) - it answers the instance page instead of asking about a change
/// it would refuse or claiming a count it does not have.
async fn replace_page(state: &AppState, user: &SessionUser, headers: &HeaderMap) -> Response {
    let integrations = state.integrations.current();
    let store = StoreCapabilities::new(state.store.clone());
    let saved = match replaceable(&integrations) {
        true => {
            ReadOnlyDoor::SecretsKeyReplace
                .read(&store, gate_of(state))
                .await
                .saved
        }
        false => None,
    };
    let Some(saved) = saved else {
        return answer(state, user, headers, None, String::new()).await;
    };
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let nav = crate::nav::NavContext::home_for(
        state,
        user.user_id,
        user.workspace_id,
        true,
        csrf.clone(),
    )
    .await;
    render(
        SecretsKeyReplacePage {
            nav,
            csrf,
            replace_door: REPLACE_PATH,
            line: saved.replace_question(),
            control: KeyOffer::Replace.label(),
        }
        .render(),
        set_cookie,
    )
}

/// Whether the secrets key of this server is Priya's to replace: the same
/// offer the page makes, so the door and the page cannot disagree (D4, L7).
fn replaceable(integrations: &Integrations) -> bool {
    KeyOffer::Replace.applies(
        integrations.key_mode(),
        integrations.secrets_check().degraded(),
    )
}

/// `GET /admin/instance/integrations/secrets-key/start-over`: I-28 over the
/// credentials this Foundry cannot open, with the I-28a Confirmation and the
/// one control that carries the start over out. It only reads
/// ([`ReadOnlyDoor`]), so a prefetch, a link preview and a reload discard
/// nothing (L4, C2).
pub async fn show_secrets_key_start_over(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some(user) = crate::instance_admin::require_instance_admin(&state, &session).await else {
        return resource_not_found_page();
    };
    start_over_page(&state, &user, &headers, StartOverCost::WhereOffered).await
}

/// What the start-over form carries: the Confirmation she typed.
#[derive(serde::Deserialize)]
pub struct StartOverForm {
    confirmation: Option<String>,
}

/// What a start over answers: a committed one earns I-21 once; a wrong
/// confirmation, and a live replica that still opens the stored key, ask the
/// cost again with nothing discarded; where the way out is not offered at all
/// the instance page is answered as it is, with no cost asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartOverAnswer {
    StartedOver,
    AskAgain,
    Unchanged,
}

/// `POST /admin/instance/integrations/secrets-key/start-over`: the start over
/// where the way out applies and the Confirmation is the words the page asked
/// for. A wrong or empty Confirmation asks the cost again and changes
/// nothing, and so does a live replica that speaks again before the write.
/// Where starting over is not offered - a stored key this Foundry opens, the
/// confirm posted from a tab left open on it - nothing changes and the
/// instance page is answered, never the cost of a discard it would refuse.
pub async fn start_over_secrets_key(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<StartOverForm>,
) -> Response {
    let Some(user) = crate::instance_admin::require_instance_admin(&state, &session).await else {
        return resource_not_found_page();
    };
    let Some(confirmed) =
        StartOverConfirmation::typed(form.confirmation.as_deref().unwrap_or_default())
    else {
        return start_over_page(&state, &user, &headers, StartOverCost::WhereOffered).await;
    };
    let store = StoreCapabilities::new(state.store.clone());
    match started_over(&state, &store, user.user_id, confirmed).await {
        StartOverAnswer::StartedOver => SavedNotice::StartedOver.answered(&session).await,
        StartOverAnswer::AskAgain => {
            start_over_page(&state, &user, &headers, StartOverCost::Again).await
        }
        StartOverAnswer::Unchanged => answer(&state, &user, &headers, None, String::new()).await,
    }
}

/// Start over where the way out applies over the stored rows as they are at
/// the post, with `confirmed` the words she typed: no caller reaches the apply
/// without them, because there is no other way to hold the value (D6). The
/// live-replica reading is taken again inside the locked write.
async fn started_over(
    state: &AppState,
    store: &StoreCapabilities,
    actor: Uuid,
    confirmed: StartOverConfirmation,
) -> StartOverAnswer {
    let snapshot = match store.instance_snapshot().await {
        Ok(snapshot) => snapshot,
        Err(err) => {
            tracing::warn!(error = %err, "reading the stored credentials failed");
            return StartOverAnswer::Unchanged;
        }
    };
    let integrations = state.integrations.current();
    let facts = ReadOnlyDoor::SecretsKeyStartOver
        .read(store, gate_of(state))
        .await;
    match offered(&integrations, facts.live_opener).confirmed_post() {
        ConfirmedStartOver::Applies => {}
        ConfirmedStartOver::AsksAgain => return StartOverAnswer::AskAgain,
        ConfirmedStartOver::NotOffered => return StartOverAnswer::Unchanged,
    }
    let settings = integrations.settings();
    let wrapped_by = settings.key_wrapper();
    let (Some(instance), Some(from_key_id)) = (
        settings.secrets.as_ref(),
        snapshot
            .secrets_key
            .as_ref()
            .map(|stored| stored.key_id.clone()),
    ) else {
        return StartOverAnswer::Unchanged;
    };
    let Some(plan) = StartOverPlan::of(instance, from_key_id, wrapped_by, actor) else {
        tracing::warn!(probe = "secrets", "generating the new secrets key failed");
        return StartOverAnswer::Unchanged;
    };
    let gate = gate_of(state);
    let answered = match store
        .start_over_secrets_key(confirmed, &plan, actor, state.clock.now(), &|states| {
            gate.still_opening(states)
        })
        .await
    {
        Ok(StartOverOutcome::StartedOver { .. }) => StartOverAnswer::StartedOver,
        Ok(StartOverOutcome::LiveReplica) => StartOverAnswer::AskAgain,
        Ok(StartOverOutcome::KeyChanged) | Err(_) => StartOverAnswer::Unchanged,
    };
    reinstall(state, &integrations).await;
    answered
}

/// When the start-over cost is asked: the one thing the page she opens and
/// the page a refused post lands on do not share (D8, B6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartOverCost {
    /// Asked only where the way out applies on this server: the page the
    /// control opens, and the one a wrong Confirmation asks again.
    WhereOffered,
    /// The answer to a start over the live-replica gate refused, read at the
    /// door or inside the write: asked whatever that gate now says, because
    /// the refusal must not erase the question she was reading. Asked only
    /// where that gate is what withholds the way out, so no refusal can draw
    /// the cost of a discard this Foundry would not take (D8, L6).
    Again,
}

impl StartOverCost {
    /// Whether the cost is asked, over the facts this door read.
    fn asked(self, integrations: &Integrations, live_opener: Option<bool>) -> bool {
        match self {
            Self::WhereOffered => offered(integrations, live_opener).offers_start_over(),
            Self::Again => {
                UnopenableOffer::reads_gate(integrations.key_mode(), integrations.key_file())
            }
        }
    }
}

/// The start-over confirm page over the credentials stored now, asked as
/// `cost` says. Where it is not asked - a stored key that opens, a live server
/// that still opens it, no instance key held to seal a new one with - and where
/// the count could not be read, it answers the instance page instead of asking
/// her to confirm a cost it would refuse to take or claiming a count it does
/// not have (D8, B6).
async fn start_over_page(
    state: &AppState,
    user: &SessionUser,
    headers: &HeaderMap,
    cost: StartOverCost,
) -> Response {
    let integrations = state.integrations.current();
    let store = StoreCapabilities::new(state.store.clone());
    let facts = ReadOnlyDoor::SecretsKeyStartOver
        .read(&store, gate_of(state))
        .await;
    let Some(saved) = facts
        .saved
        .filter(|_| cost.asked(&integrations, facts.live_opener))
    else {
        return answer(state, user, headers, None, String::new()).await;
    };
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let nav = crate::nav::NavContext::home_for(
        state,
        user.user_id,
        user.workspace_id,
        true,
        csrf.clone(),
    )
    .await;
    render(
        SecretsKeyStartOverPage {
            nav,
            csrf,
            start_over_door: START_OVER_PATH,
            line: saved.start_over_question(),
            confirmation: CONFIRMATION,
            asks: TYPE_START_OVER,
            control: START_OVER_CONTROL,
        }
        .render(),
        set_cookie,
    )
}

/// The offer this server makes on the way out of a stored secrets key: the
/// same one the page appends to I-26, over the key mode, the gate as it was
/// read and the key file. The page's link, the confirm page and the post read
/// this one answer, so none of them can offer or carry out what another
/// refuses (D5, D8, L6, L7).
fn offered(integrations: &Integrations, live_opener: Option<bool>) -> UnopenableOffer {
    UnopenableOffer::of_gate(
        integrations.key_mode(),
        live_opener,
        integrations.key_file(),
    )
}

/// Replace (US-IIS-07 AC 1): judged over the stored rows as they are at the
/// post, never the last rebuild's, a fresh secrets key is wrapped under the
/// instance key this server holds and every stored sealed value is opened
/// under the key in effect and sealed again under the new one - the key, the
/// values, the canary, the activity line and the version bump in one locked
/// transaction (L1, C3). After it, this server's integrations are rebuilt and
/// installed through the shared boot probe, so the acting replica opens every
/// credential before it answers; the others follow on their reload (D3).
async fn replaced<S>(state: &AppState, store: &S, actor: Uuid) -> ReplaceAnswer
where
    S: KeyProbe + InstanceKeyWrites,
{
    let integrations = match store.instance_snapshot().await {
        Ok(snapshot) => state.integrations.current().rebuilt(Ok(snapshot)),
        Err(err) => {
            tracing::warn!(error = %err, "reading the stored credentials failed");
            return ReplaceAnswer::Unchanged;
        }
    };
    if !replaceable(&integrations) {
        return ReplaceAnswer::Unchanged;
    }
    let settings = integrations.settings();
    let wrapped_by = settings.key_wrapper();
    let (Some(instance), Some(opening), Some(from_key_id)) = (
        settings.secrets.as_ref(),
        integrations.sealing_box(),
        integrations.sealed_under(),
    ) else {
        tracing::warn!(
            probe = "secrets",
            "replacing the secrets key found no key in effect"
        );
        return ReplaceAnswer::Unchanged;
    };
    let Some(plan) = ReplacePlan::of(opening, instance, from_key_id, wrapped_by, actor) else {
        tracing::warn!(probe = "secrets", "generating the new secrets key failed");
        return ReplaceAnswer::Unchanged;
    };
    let answered = written(store, &plan, actor, state.clock.now()).await;
    reinstall(state, &integrations).await;
    answered
}

/// `plan` written, and what that answers: a commit earns I-24 over the
/// credentials it sealed again, a refusal asks the question again, and a
/// compare-and-set another admin's replacement won leaves the page as it is
/// (L9, B1, B2).
async fn written(
    store: &dyn InstanceKeyWrites,
    plan: &ReplacePlan<'_>,
    actor: Uuid,
    at: OffsetDateTime,
) -> ReplaceAnswer {
    let reseal = |slot, sealed: &[u8]| plan.reseal(slot, sealed).map(Some).ok_or(ResealRefused);
    let outcome = store
        .replace_secrets_key(
            plan.from_key_id(),
            plan.key(),
            plan.canary(),
            actor,
            at,
            &reseal,
        )
        .await;
    match outcome {
        Ok(ReplaceOutcome::Replaced { resealed, .. }) => {
            ReplaceAnswer::Replaced(SavedNotice::Replaced(resealed))
        }
        Ok(ReplaceOutcome::Refused(ResealRefused)) => {
            tracing::warn!(
                probe = "secrets",
                "a stored credential could not be sealed again under the new secrets key; \
                 nothing changed"
            );
            ReplaceAnswer::AskAgain
        }
        Ok(ReplaceOutcome::KeyChanged) => {
            tracing::info!(
                probe = "secrets",
                "the secrets key was replaced by another request first; nothing changed"
            );
            ReplaceAnswer::Unchanged
        }
        Err(err) => {
            tracing::warn!(error = %err, "replacing the secrets key failed");
            ReplaceAnswer::Unchanged
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundry_store::integrations::KeyWrapper;

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
        assert_eq!(KeyOffer::Replace.label(), "Replace secrets key");
        assert_eq!(KeyOffer::Replace.door(), REPLACE_PATH);
        // Generate and Move are the submit control of their own write;
        // Replace opens the page that asks the question first (L4).
        assert_eq!(KeyOffer::Generate.opens(), None);
        assert_eq!(KeyOffer::Move.opens(), None);
        assert_eq!(KeyOffer::Replace.opens(), Some(REPLACE_PATH));
    }

    /// A store that answers one replacement with `outcome`, recording the
    /// re-seal it was handed. Every other instance write is unreachable from
    /// the replace door.
    struct OneReplacement {
        outcome: std::sync::Mutex<Option<ReplaceOutcome<ResealRefused>>>,
        asked: std::sync::atomic::AtomicUsize,
    }

    impl OneReplacement {
        fn answering(outcome: ReplaceOutcome<ResealRefused>) -> Self {
            Self {
                outcome: std::sync::Mutex::new(Some(outcome)),
                asked: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        fn asked(&self) -> usize {
            self.asked.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl InstanceKeyWrites for OneReplacement {
        async fn store_secrets_key(
            &self,
            _key: &foundry_store::integrations::NewSecretsKey,
            _canary: &foundry_store::integrations::KeyCheck,
            _at: OffsetDateTime,
            _reseal: crate::integrations::store_capabilities::Resealer<'_>,
        ) -> Result<SecretsKeyOutcome<ResealRefused>, foundry_store::StoreError> {
            unreachable!("the replace door generates no key")
        }

        async fn replace_secrets_key(
            &self,
            _from_key_id: &str,
            _key: &foundry_store::integrations::NewSecretsKey,
            _canary: &foundry_store::integrations::KeyCheck,
            _actor: Uuid,
            _at: OffsetDateTime,
            _reseal: crate::integrations::store_capabilities::Resealer<'_>,
        ) -> Result<ReplaceOutcome<ResealRefused>, foundry_store::StoreError> {
            self.asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(self
                .outcome
                .lock()
                .expect("the outcome")
                .take()
                .expect("one replacement"))
        }

        async fn start_over_secrets_key(
            &self,
            _confirmed: StartOverConfirmation,
            _plan: &StartOverPlan,
            _actor: Uuid,
            _at: OffsetDateTime,
            _gate: crate::integrations::store_capabilities::LiveReplicaGate<'_>,
        ) -> Result<StartOverOutcome, foundry_store::StoreError> {
            unreachable!("the replace door starts over with no secrets key")
        }

        async fn store_slack_submission(
            &self,
            _gate: crate::integrations::store_capabilities::SubmissionGate<'_>,
        ) -> Result<foundry_store::integrations::SubmissionStored, foundry_store::StoreError>
        {
            unreachable!("the replace door writes no Slack submission")
        }

        async fn apply_pending_slack_app(
            &self,
            _actor: Uuid,
            _now: OffsetDateTime,
            _sealed_under: Option<&str>,
            _reseal: crate::integrations::store_capabilities::HeldResealer<'_>,
        ) -> Result<
            foundry_store::integrations::PendingApplied<ResealRefused>,
            foundry_store::StoreError,
        > {
            unreachable!("the replace door applies no held Slack app")
        }

        async fn remove_slack_app(
            &self,
            _actor: Uuid,
            _now: OffsetDateTime,
        ) -> Result<foundry_store::integrations::RemoveOutcome, foundry_store::StoreError> {
            unreachable!("the replace door removes no Slack app")
        }

        async fn connected_slack_workspaces(&self) -> Result<i64, foundry_store::StoreError> {
            unreachable!("the replace door counts no connection")
        }

        async fn replica_states(
            &self,
        ) -> Result<Vec<foundry_store::integrations::ReplicaState>, foundry_store::StoreError>
        {
            unreachable!("the replace door reads no replica state")
        }

        async fn prune_pending_slack_app(
            &self,
            _now: OffsetDateTime,
        ) -> Result<u64, foundry_store::StoreError> {
            unreachable!("the replace door prunes nothing")
        }

        async fn instance_activity(
            &self,
            _limit: i64,
        ) -> Result<Vec<foundry_store::integrations::InstanceActivityRow>, foundry_store::StoreError>
        {
            unreachable!("the replace door reads no activity")
        }

        async fn display_name(
            &self,
            _user_id: Uuid,
        ) -> Result<Option<String>, foundry_store::StoreError> {
            unreachable!("the replace door reads no name")
        }
    }

    /// The instance key of a test server, with a plan over it.
    fn a_box() -> foundry_auth::secret_box::SecretBox {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        let text = secrecy::SecretString::from(URL_SAFE_NO_PAD.encode([7u8; 32]));
        let key = foundry_auth::secret_box::SecretsKey::parse(
            &text,
            foundry_auth::secret_box::INSTANCE_KEY_SETTING,
        )
        .expect("32 bytes");
        foundry_auth::secret_box::SecretBox::new(key, Vec::new())
    }

    /// P3 and L9: a replacement the store refuses leaves the admin reading
    /// the confirm question again, and earns no I-24 - the refusal is no
    /// notice, so nothing she lands on can claim a replacement that was
    /// rolled back. A commit is the one answer that earns the notice, with
    /// the credentials the write sealed again. A replacement another admin
    /// won answers the page as it is.
    #[tokio::test]
    async fn only_a_committed_replacement_earns_i24_and_a_refused_one_asks_again() {
        let keys = a_box();
        let plan = ReplacePlan::of(
            &keys,
            &keys,
            "sk-old".to_string(),
            KeyWrapper::Operator,
            Uuid::nil(),
        )
        .expect("a plan");
        let at = OffsetDateTime::UNIX_EPOCH;

        let refusing = OneReplacement::answering(ReplaceOutcome::Refused(ResealRefused));
        let answered = written(&refusing, &plan, Uuid::nil(), at).await;
        assert_eq!(answered, ReplaceAnswer::AskAgain);
        assert_eq!(refusing.asked(), 1, "the write was asked once");

        let committing = OneReplacement::answering(ReplaceOutcome::Replaced {
            resealed: 3,
            version: 9,
        });
        let answered = written(&committing, &plan, Uuid::nil(), at).await;
        assert_eq!(
            answered,
            ReplaceAnswer::Replaced(SavedNotice::Replaced(3)),
            "a commit earns I-24 over the credentials it sealed again"
        );

        let lost = OneReplacement::answering(ReplaceOutcome::KeyChanged);
        let answered = written(&lost, &plan, Uuid::nil(), at).await;
        assert_eq!(answered, ReplaceAnswer::Unchanged);
    }
}
