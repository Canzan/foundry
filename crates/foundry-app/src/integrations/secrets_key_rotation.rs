//! Rotating the secrets key of a Foundry an instance admin has come to doubt
//! (US-IIS-07, D6): how many credentials a rotation touches and the words
//! that say so ([`SavedCredentials`]), what the page offers while the stored
//! key cannot be opened ([`UnopenableOffer`]) over the live-replica gate
//! ([`LiveOpeners`]) and what a confirmed start over does in each of those
//! states ([`ConfirmedStartOver`]), the typed confirmation a start over cannot be applied
//! without ([`StartOverConfirmation`]), and what a replacement
//! ([`ReplacePlan`]) and a start over ([`StartOverPlan`]) store.
//!
//! The count is the I-22 counting rule the store already applies: each stored
//! connection credential one, the saved Slack app one however many of its
//! secrets are sealed (K2). The app Foundry's environment sets is nobody's
//! saved credential, and the app held for its confirm is re-sealed with the
//! rest and counted with the app.
//!
//! Nothing here reads a row or writes one: the doors hand it the facts and
//! the store hands it the count it sealed again, so the words, the offer and
//! the plan are decided in one place (L6, L7, L10).

use super::keys::{plan_generate, GeneratePlan, KeyFile, KeyMode};
use super::sealed_slot;
use foundry_auth::secret_box::SecretBox;
use foundry_store::integrations::{KeyCheck, KeyWrapper, NewSecretsKey, ReplicaState, SealedSlot};
use std::time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;

/// I-23, n ≥ 2.
const REPLACE_MANY: &str = "Replace the secrets key? Foundry seals its {n} saved credentials again under a new key. Nobody needs to reconnect.";
/// I-23, n = 1.
const REPLACE_ONE: &str = "Replace the secrets key? Foundry seals its 1 saved credential again under a new key. Nobody needs to reconnect.";
/// I-23, n = 0: nothing is sealed, and Replace stays offered as I-2 does.
const REPLACE_NONE: &str =
    "Replace the secrets key? This Foundry has no saved credentials yet, so nothing needs to be sealed again.";
/// I-24, n ≥ 2.
const REPLACED_MANY: &str = "Replaced. {n} saved credentials are sealed under the new key.";
/// I-24, n = 1.
const REPLACED_ONE: &str = "Replaced. 1 saved credential is sealed under the new key.";
/// I-24, n = 0.
const REPLACED_NONE: &str = "Replaced. This Foundry has a new secrets key.";
/// I-28, n ≥ 2.
const START_OVER_MANY: &str = "Start over with a new secrets key? The {n} saved credentials this Foundry cannot open are discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again.";
/// I-28, n = 1.
const START_OVER_ONE: &str = "Start over with a new secrets key? The 1 saved credential this Foundry cannot open is discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again.";
/// I-28, n = 0: nothing is discarded, and Start over stays offered as I-2e0's
/// Move is.
const START_OVER_NONE: &str =
    "Start over with a new secrets key? This Foundry has no saved credentials to discard.";
/// The one sentence I-26 gains while another live Foundry server still opens
/// the stored secrets key (approved 2026-11-11). It is appended to the
/// shipped restore words and never replaces them.
const LIVE_REPLICA_OPENS: &str =
    "Another Foundry server still opens the secrets key, so Start over is not offered.";

/// How many credentials a rotation of the secrets key touches, counted by the
/// I-22 rule, and the words that count selects: the n ≥ 2 form, the n = 1
/// form and the n = 0 form, for I-23 and I-24 alike (L10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavedCredentials(usize);

impl SavedCredentials {
    /// The credentials stored now: each connection one, the saved Slack app
    /// one however many of its secrets are sealed.
    pub fn of(connections: usize, saved_app: bool) -> Self {
        Self::counted(connections + usize::from(saved_app))
    }

    /// A count another owner made by that same rule: the store's `resealed`,
    /// which counts each connection one and the saved Slack app one.
    pub fn counted(count: usize) -> Self {
        Self(count)
    }

    pub fn count(self) -> usize {
        self.0
    }

    /// I-23: what a replacement seals again, read before she decides.
    pub fn replace_question(self) -> String {
        self.form(REPLACE_NONE, REPLACE_ONE, REPLACE_MANY)
    }

    /// I-24: what the replacement sealed under the new key.
    pub fn replaced_notice(self) -> String {
        self.form(REPLACED_NONE, REPLACED_ONE, REPLACED_MANY)
    }

    /// I-28: what starting over discards, read before she types the
    /// confirmation it asks for. Under I-26 no held key opens any of them,
    /// which is why the same count reads as the credentials this Foundry
    /// cannot open.
    pub fn start_over_question(self) -> String {
        self.form(START_OVER_NONE, START_OVER_ONE, START_OVER_MANY)
    }

    /// The form the count selects, with the number substituted independently
    /// of the selection: no count but 0 reads `none`, and no count but 1
    /// reads `one`.
    fn form(self, none: &str, one: &str, many: &str) -> String {
        match self.0 {
            0 => none.to_string(),
            1 => one.to_string(),
            n => many.replace("{n}", &n.to_string()),
        }
    }
}

/// The live-replica gate (US-IIS-07 AC 2, D5): whether some Foundry server of
/// this database still opens the stored secrets key. A server counts while its
/// own state row says `opens_secrets_key` and its last beat is no older than
/// the window, which is three reload intervals - long enough that a running
/// server always beats inside it, short enough that a server that has stopped
/// stops counting within seconds. A beat ahead of this server's clock counts
/// as current: skew that large is the operator's (Y5), and the safe reading of
/// it is that the other server is still there.
///
/// A server of an earlier release that never beats a state row at all cannot
/// be seen here, which is why I-28a asks the human to rule it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveOpeners {
    /// The instant the beats are judged against: the caller's clock, never
    /// `now()`.
    at: OffsetDateTime,
    window: time::Duration,
}

impl LiveOpeners {
    /// How many reload intervals a beat may be old and still count (D5).
    const INTERVALS: u32 = 3;

    /// The gate at `now`, over a replica reload period of `reload`
    /// (`FOUNDRY_INTEGRATIONS_RELOAD_MS`).
    pub fn at(now: OffsetDateTime, reload: Duration) -> Self {
        Self {
            at: now,
            window: time::Duration::try_from(reload.saturating_mul(Self::INTERVALS))
                .unwrap_or(time::Duration::MAX),
        }
    }

    /// Whether any of `states` is a server that still opens the stored key.
    pub fn still_opening(self, states: &[ReplicaState]) -> bool {
        states.iter().any(|state| self.opens(state))
    }

    fn opens(self, state: &ReplicaState) -> bool {
        state.opens_secrets_key && self.at - state.beat_at <= self.window
    }
}

/// What the page offers while a secrets key is stored and this server cannot
/// open it (I-26, US-IIS-07 AC 2): the three states of that one line, decided
/// from the key mode, the gate and the key file and nowhere else, so no
/// template decides which of them she reads (L6, L7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnopenableOffer {
    /// I-26 with Start over appended: no running server opens the stored key,
    /// and this one holds an instance key to seal a new secrets key with.
    StartOver,
    /// I-26 restore-only, with the one sentence that another live server
    /// still opens the key: starting over would discard credentials that
    /// server is serving with.
    RestoreOnlyLiveReplica,
    /// I-26 restore-only, claiming nothing more - and every state that is not
    /// I-26 at all, I-31 included, where the page reads its own words and
    /// offers neither way out.
    RestoreOnly,
}

impl UnopenableOffer {
    /// The offer in `mode`, with `live_opener` as the gate read it and the
    /// instance key file in `key_file` (pure and total).
    ///
    /// Only a stored key no held key opens is I-26 at all: I-27 and I-31 read
    /// their own instance-key words, and a key that opens is Replace's own
    /// state, so the two ways out are never offered together (L7). With no key
    /// file made and no operator key (`Withheld`, ADR-IIS-002 r3) this server
    /// holds nothing to seal a new secrets key with, so the way out could not
    /// work and nothing is claimed about another server either.
    pub fn decide(mode: KeyMode, live_opener: bool, key_file: KeyFile) -> Self {
        match (mode, key_file, live_opener) {
            (KeyMode::Unopenable, KeyFile::Withheld, _) => Self::RestoreOnly,
            (KeyMode::Unopenable, _, true) => Self::RestoreOnlyLiveReplica,
            (KeyMode::Unopenable, _, false) => Self::StartOver,
            _ => Self::RestoreOnly,
        }
    }

    /// The offer over the gate as it could be read: an unread replica list is
    /// neither a server that opens the key nor the absence of one, so the page
    /// stays restore-only and claims nothing - the way out is withheld rather
    /// than offered over a fact this Foundry does not hold (B6).
    pub fn of_gate(mode: KeyMode, live_opener: Option<bool>, key_file: KeyFile) -> Self {
        match live_opener {
            Some(live) => Self::decide(mode, live, key_file),
            None => Self::RestoreOnly,
        }
    }

    /// Whether the way out is offered: the page's Start over link and the
    /// start-over door read this one answer, so neither can offer what the
    /// other refuses.
    pub fn offers_start_over(self) -> bool {
        matches!(self, Self::StartOver)
    }

    /// What a confirmed start over does in this state (D8): the one place the
    /// gate's refusal and a state that offers no way out at all are told
    /// apart. A live server still opening the stored key is the one reason to
    /// ask the cost again; everywhere else the way out is not this Foundry's
    /// to offer, so the post changes nothing and answers the instance page,
    /// exactly as the start-over page does.
    pub fn confirmed_post(self) -> ConfirmedStartOver {
        match self {
            Self::StartOver => ConfirmedStartOver::Applies,
            Self::RestoreOnlyLiveReplica => ConfirmedStartOver::AsksAgain,
            Self::RestoreOnly => ConfirmedStartOver::NotOffered,
        }
    }

    /// Whether the replica gate decides anything in `mode` with the instance
    /// key file in `key_file`: the one owner of that fact, read beside
    /// [`Self::decide`], whose table it follows. Only a stored key no held key
    /// opens is I-26 at all, and with no key file made and no operator key
    /// nothing is held to seal a new secrets key with, so everywhere else the
    /// offer is restore-only whatever the gate says - which is what
    /// [`Self::of_gate`] answers over an unread list - and a page that asks
    /// after the other servers of this database there would read rows that
    /// change no word it shows (L6, L7).
    pub fn reads_gate(mode: KeyMode, key_file: KeyFile) -> bool {
        match (mode, key_file) {
            (KeyMode::Unopenable, KeyFile::Withheld) => false,
            (KeyMode::Unopenable, _) => true,
            _ => false,
        }
    }

    /// The sentence appended to the shipped I-26 restore words, which it never
    /// rewrites.
    pub fn appended(self) -> Option<&'static str> {
        match self {
            Self::RestoreOnlyLiveReplica => Some(LIVE_REPLICA_OPENS),
            Self::StartOver | Self::RestoreOnly => None,
        }
    }
}

/// What a confirmed start over does, decided from [`UnopenableOffer`] alone
/// and nowhere else (D8, L6, L7): the way out is carried out only where this
/// Foundry offers it, the cost is asked again only where the live-replica gate
/// is the reason it is withheld, and a state that offers no way out at all -
/// a stored key this Foundry opens, I-27, I-31, nothing held to seal a new key
/// with, a gate that could not be read - changes nothing and answers the
/// instance integrations page.
///
/// Telling the last two apart is what keeps the discard-everything control off
/// a page a healthy Foundry draws: a tab left open from a lost instance key
/// that posts after the operator restored it is answered her own page, not the
/// cost again with a live control on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmedStartOver {
    /// The way out applies here: the words she typed and the gate read again
    /// inside the write are all that is left between her and the new key.
    Applies,
    /// Another live Foundry server still opens the stored key: nothing is
    /// discarded, and the cost she was reading is asked again rather than
    /// erased.
    AsksAgain,
    /// Starting over is not offered in this state: nothing is discarded and
    /// nothing is asked.
    NotOffered,
}

/// The I-28a Confirmation as she typed it, admitted (D6): the value exists
/// only where the field held the exact words the page asked for, so the apply
/// that takes it cannot be reached without them. It carries no text: what it
/// says is that this one was typed, and nothing it could be printed beside
/// needs the field back.
///
/// Its one constructor is [`Self::typed`]; there is no public raw constructor,
/// no setter and no deserializer, so no caller in this crate builds what the
/// constructor refuses. A caller outside it could not either, because the
/// field is private. Nothing here claims intent or authority: that she is the
/// instance admin is the guard's, and that no server of an earlier release is
/// still running is what the words assert on her behalf (I-28a, Y5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartOverConfirmation(());

impl StartOverConfirmation {
    /// The words the page asks her to type (I-28a).
    const WORDS: &'static str = "START OVER";

    /// `field` as the form posted it, admitted only where it is the exact
    /// words START OVER after the spaces a paste brings along are removed;
    /// `None` for anything else, an empty field and an absent one included.
    pub fn typed(field: &str) -> Option<Self> {
        (field.trim() == Self::WORDS).then_some(Self(()))
    }
}

/// What a start over stores (US-IIS-07, D6): a fresh secrets key wrapped under
/// the instance key this server holds with its check value, and the stored key
/// id it replaces (the compare-and-set the store is judged over). Nothing is
/// sealed again - the credentials this Foundry cannot open are discarded - so
/// this plan holds no key in effect, which is all that tells it from its
/// sibling [`ReplacePlan`]. No `Debug`: it carries the new key.
pub struct StartOverPlan {
    plan: GeneratePlan,
    from_key_id: String,
}

impl StartOverPlan {
    /// The plan that starts over from the stored key `from_key_id` with a
    /// fresh key wrapped under the held `instance` key; `None` when the random
    /// source or the wrapping fails. It is exactly a generated key and its
    /// canary: a start over needs nothing re-sealed.
    pub fn of(
        instance: &SecretBox,
        from_key_id: String,
        wrapped_by: KeyWrapper,
        actor: Uuid,
    ) -> Option<Self> {
        Some(Self {
            plan: plan_generate(instance, wrapped_by, Some(actor))?,
            from_key_id,
        })
    }

    /// The stored key id this plan starts over from.
    pub fn from_key_id(&self) -> &str {
        &self.from_key_id
    }

    /// The new key, as the store writes it.
    pub fn key(&self) -> &NewSecretsKey {
        &self.plan.key
    }

    /// The new key's check value.
    pub fn canary(&self) -> &KeyCheck {
        &self.plan.canary
    }
}

/// What a replacement stores (US-IIS-07, D2): a new secrets key wrapped under
/// the instance key this server holds with its check value, the stored key id
/// it replaces (the compare-and-set the store is judged over), and the key
/// that opens what is stored now. No `Debug`: it carries the new key.
pub struct ReplacePlan<'a> {
    plan: GeneratePlan,
    /// The secrets key in effect: what every stored sealed value opens under
    /// until this plan commits.
    opening: &'a SecretBox,
    from_key_id: String,
}

impl<'a> ReplacePlan<'a> {
    /// The plan that replaces the stored key `from_key_id`, which `opening`
    /// opens, with a fresh key wrapped under the held `instance` key;
    /// `None` when the random source or the wrapping fails.
    pub fn of(
        opening: &'a SecretBox,
        instance: &SecretBox,
        from_key_id: String,
        wrapped_by: KeyWrapper,
        actor: Uuid,
    ) -> Option<Self> {
        Some(Self {
            plan: plan_generate(instance, wrapped_by, Some(actor))?,
            opening,
            from_key_id,
        })
    }

    /// The stored key id this plan replaces.
    pub fn from_key_id(&self) -> &str {
        &self.from_key_id
    }

    /// The new key, as the store writes it.
    pub fn key(&self) -> &NewSecretsKey {
        &self.plan.key
    }

    /// The new key's check value.
    pub fn canary(&self) -> &KeyCheck {
        &self.plan.canary
    }

    /// `sealed` in `slot`, opened under the key in effect and sealed again
    /// under the new one; `None` when the key in effect does not open it,
    /// which refuses the whole write (B1, L9).
    pub fn reseal(&self, slot: SealedSlot, sealed: &[u8]) -> Option<Vec<u8>> {
        self.plan
            .reseal_under(self.opening, &sealed_slot(slot), sealed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// The locked copy table, read independently of the selector: the
    /// sentence each count calls for, built from the words and the number.
    fn question(n: usize) -> String {
        match n {
            0 => "Replace the secrets key? This Foundry has no saved credentials yet, so nothing needs to be sealed again.".to_string(),
            1 => "Replace the secrets key? Foundry seals its 1 saved credential again under a new key. Nobody needs to reconnect.".to_string(),
            n => format!("Replace the secrets key? Foundry seals its {n} saved credentials again under a new key. Nobody needs to reconnect."),
        }
    }

    fn notice(n: usize) -> String {
        match n {
            0 => "Replaced. This Foundry has a new secrets key.".to_string(),
            1 => "Replaced. 1 saved credential is sealed under the new key.".to_string(),
            n => format!("Replaced. {n} saved credentials are sealed under the new key."),
        }
    }

    /// The locked I-28 table, read independently of the selector.
    fn discards(n: usize) -> String {
        match n {
            0 => "Start over with a new secrets key? This Foundry has no saved credentials to discard.".to_string(),
            1 => "Start over with a new secrets key? The 1 saved credential this Foundry cannot open is discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again.".to_string(),
            n => format!("Start over with a new secrets key? The {n} saved credentials this Foundry cannot open are discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again."),
        }
    }

    /// L10's must-include cases, with the locked words in full: nothing
    /// saved, the saved app alone, one connection alone, and one connection
    /// with the app. A selector that counted each Slack app secret separately
    /// would read 2 where the copy reads 1, and one that read the n = 1 form
    /// for n = 0 would promise a credential this Foundry does not hold.
    #[test]
    fn the_count_and_the_copy_follow_the_i22_rule() {
        #[rustfmt::skip]
        let table = [
            (0, false, 0),
            (0, true,  1),
            (1, false, 1),
            (1, true,  2),
            (8, true,  9),
        ];
        for (connections, saved_app, count) in table {
            let saved = SavedCredentials::of(connections, saved_app);
            assert_eq!(saved.count(), count, "{connections} + app {saved_app}");
            assert_eq!(saved.replace_question(), question(count));
            assert_eq!(saved.replaced_notice(), notice(count));
            assert_eq!(saved.start_over_question(), discards(count));
        }
        // I-28's own three forms, in the locked words: the count the sentence
        // carries is what this Foundry holds, so the connection alone and the
        // app alone read the n = 1 form and never the plural one.
        assert_eq!(
            SavedCredentials::of(0, false).start_over_question(),
            "Start over with a new secrets key? This Foundry has no saved credentials to discard."
        );
        assert_eq!(
            SavedCredentials::of(0, true).start_over_question(),
            "Start over with a new secrets key? The 1 saved credential this Foundry cannot open is discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again."
        );
        assert_eq!(
            SavedCredentials::of(1, false).start_over_question(),
            "Start over with a new secrets key? The 1 saved credential this Foundry cannot open is discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again."
        );
        assert_eq!(
            SavedCredentials::of(1, true).start_over_question(),
            "Start over with a new secrets key? The 2 saved credentials this Foundry cannot open are discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again."
        );
        assert_eq!(
            SavedCredentials::of(0, false).replace_question(),
            "Replace the secrets key? This Foundry has no saved credentials yet, so nothing needs to be sealed again."
        );
        assert_eq!(
            SavedCredentials::of(0, true).replace_question(),
            "Replace the secrets key? Foundry seals its 1 saved credential again under a new key. Nobody needs to reconnect."
        );
        assert_eq!(
            SavedCredentials::of(2, true).replace_question(),
            "Replace the secrets key? Foundry seals its 3 saved credentials again under a new key. Nobody needs to reconnect."
        );
        assert_eq!(
            SavedCredentials::of(0, false).replaced_notice(),
            "Replaced. This Foundry has a new secrets key."
        );
        assert_eq!(
            SavedCredentials::of(0, true).replaced_notice(),
            "Replaced. 1 saved credential is sealed under the new key."
        );
        assert_eq!(
            SavedCredentials::of(2, true).replaced_notice(),
            "Replaced. 3 saved credentials are sealed under the new key."
        );
    }

    proptest! {
        /// L10: over 0 to 8 connections crossed with the saved Slack app
        /// present or absent, the count is the I-22 one and I-23 and I-24
        /// read the form that count calls for, the number substituted
        /// independently of the selection. A count the store made by the same
        /// rule reads the same words.
        #[test]
        fn the_copy_reads_the_form_the_count_calls_for(
            connections in 0usize..=8,
            saved_app in any::<bool>(),
        ) {
            let saved = SavedCredentials::of(connections, saved_app);
            let n = connections + usize::from(saved_app);
            prop_assert_eq!(saved.count(), n);
            prop_assert_eq!(saved.replace_question(), question(n));
            prop_assert_eq!(saved.replaced_notice(), notice(n));
            prop_assert_eq!(saved.start_over_question(), discards(n));
            prop_assert_eq!(SavedCredentials::counted(n).replaced_notice(), notice(n));
        }
    }

    use crate::integrations::keys::NoKeyReason;

    const ALL_MODES: [KeyMode; 6] = [
        KeyMode::NoInstanceKey(NoKeyReason::Unwritable),
        KeyMode::NoInstanceKey(NoKeyReason::Unreadable),
        KeyMode::Direct,
        KeyMode::AwaitingSecretsKey,
        KeyMode::Sealed,
        KeyMode::Unopenable,
    ];

    const ALL_FILES: [KeyFile; 4] = [
        KeyFile::Held,
        KeyFile::Unwritable,
        KeyFile::Unreadable,
        KeyFile::Withheld,
    ];

    /// L6 and L7: the three I-26 states, over the key mode, the gate and the
    /// key file. The way out is offered exactly where its own state holds -
    /// a stored key this server cannot open, a key held to seal a new one
    /// with, and no running server still opening the old one - and the one
    /// sentence that says why it is withheld is read only where another live
    /// server is the reason. Every other mode, I-31 and I-27 among them, is
    /// restore-only with nothing appended, so no state offers both ways out
    /// and none offers the sentence beside the way out it explains away.
    #[test]
    fn the_table_of_the_three_i26_states() {
        use UnopenableOffer::{RestoreOnly, RestoreOnlyLiveReplica, StartOver};
        // The key file, whether a live server opens the key, then the offer.
        #[rustfmt::skip]
        let table = [
            (KeyFile::Held,       false, StartOver),
            (KeyFile::Held,       true,  RestoreOnlyLiveReplica),
            (KeyFile::Unwritable, false, StartOver),
            (KeyFile::Unwritable, true,  RestoreOnlyLiveReplica),
            (KeyFile::Unreadable, false, StartOver),
            (KeyFile::Unreadable, true,  RestoreOnlyLiveReplica),
            // Nothing is held to seal a new secrets key with (ADR-IIS-002 r3).
            (KeyFile::Withheld,   false, RestoreOnly),
            (KeyFile::Withheld,   true,  RestoreOnly),
        ];
        for (key_file, live, offer) in table {
            let decided = UnopenableOffer::decide(KeyMode::Unopenable, live, key_file);
            assert_eq!(decided, offer, "{key_file:?}, live opener {live}");
            // The gate is read exactly where it decides between two offers:
            // where it is not, the offer is the same whatever it would have
            // said, so a page that skips the read shows the same words.
            assert_eq!(
                UnopenableOffer::reads_gate(KeyMode::Unopenable, key_file),
                decided != UnopenableOffer::decide(KeyMode::Unopenable, !live, key_file),
                "{key_file:?} decides on the gate"
            );
            assert_eq!(decided.offers_start_over(), offer == StartOver);
            assert_eq!(
                decided.appended(),
                (offer == RestoreOnlyLiveReplica).then_some(
                    "Another Foundry server still opens the secrets key, so Start over is not \
                     offered."
                ),
            );
            // An unread replica list is neither state: restore-only, with the
            // way out withheld and nothing claimed about another server.
            let unread = UnopenableOffer::of_gate(KeyMode::Unopenable, None, key_file);
            assert_eq!(unread, RestoreOnly, "{key_file:?} with the gate unread");
            assert_eq!(
                UnopenableOffer::of_gate(KeyMode::Unopenable, Some(live), key_file),
                decided
            );
        }
        // Every mode that is not I-26: its own words, neither way out and no
        // sentence about another server, whatever the gate and the file say.
        for mode in ALL_MODES.into_iter().filter(|m| *m != KeyMode::Unopenable) {
            for key_file in ALL_FILES {
                for live in [false, true] {
                    let decided = UnopenableOffer::decide(mode, live, key_file);
                    assert_eq!(decided, RestoreOnly, "{mode:?}, {key_file:?}, live {live}");
                    assert!(!decided.offers_start_over());
                    // Nothing to read the rows for: the offer is restore-only
                    // whatever the gate says, and the unread reading is it.
                    assert!(!UnopenableOffer::reads_gate(mode, key_file));
                    assert_eq!(
                        UnopenableOffer::of_gate(mode, None, key_file),
                        decided,
                        "{mode:?}, {key_file:?} with the gate unread"
                    );
                    assert_eq!(decided.appended(), None);
                    // Replace's own state and Start over's are disjoint (L7).
                    assert!(!(mode.replaces(false) && decided.offers_start_over()));
                }
            }
        }
    }

    /// D5: a server counts as still opening the stored key while its own row
    /// says so and its last beat is inside three reload intervals. A row that
    /// says it opens nothing never counts, however fresh; a row a window and
    /// a tick old never counts, however recently it opened the key; and a beat
    /// ahead of this server's clock counts, because skew that large is not a
    /// server that has gone.
    #[test]
    fn the_table_of_the_beats_the_gate_counts_as_a_live_opener() {
        let reload = Duration::from_millis(200);
        let window = time::Duration::milliseconds(600);
        let now = OffsetDateTime::UNIX_EPOCH + time::Duration::hours(9);
        let gate = LiveOpeners::at(now, reload);
        let row = |opens: bool, beat_at: OffsetDateTime| ReplicaState {
            replica: "foundry-0".to_string(),
            applied_version: 7,
            key_mode: "unopenable".to_string(),
            opens_secrets_key: opens,
            beat_at,
        };
        // Whether the row says it opens the key, how old its beat is, then
        // whether the gate counts it.
        #[rustfmt::skip]
        let table = [
            (true,  time::Duration::ZERO,               true),
            (true,  window,                             true),
            (true,  window + time::Duration::milliseconds(1), false),
            (true,  time::Duration::minutes(1),         false),
            (false, time::Duration::ZERO,               false),
            (false, window,                             false),
            (false, time::Duration::minutes(1),         false),
            (true,  -time::Duration::minutes(1),        true),
        ];
        for (opens, age, counts) in table {
            let states = [row(opens, now - age)];
            assert_eq!(
                gate.still_opening(&states),
                counts,
                "opens {opens}, beat {age} old"
            );
        }
        // No server has beaten at all: nobody opens the key.
        assert!(!gate.still_opening(&[]));
        // One live opener among stale and closed rows is still a live opener.
        let mixed = [
            row(false, now),
            row(true, now - time::Duration::minutes(1)),
            row(true, now - window),
        ];
        assert!(gate.still_opening(&mixed));
    }
}
