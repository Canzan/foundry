//! Rotating the secrets key of a Foundry an instance admin has come to doubt
//! (US-IIS-07, D6): how many credentials a rotation touches and the words
//! that say so ([`SavedCredentials`]), and what a replacement stores
//! ([`ReplacePlan`]).
//!
//! The count is the I-22 counting rule the store already applies: each stored
//! connection credential one, the saved Slack app one however many of its
//! secrets are sealed (K2). The app Foundry's environment sets is nobody's
//! saved credential, and the app held for its confirm is re-sealed with the
//! rest and counted with the app.
//!
//! Nothing here reads a row or writes one: the doors hand it the facts and
//! the store hands it the count it sealed again, so the words and the plan
//! are decided in one place (L10).

use super::keys::{plan_generate, GeneratePlan};
use super::sealed_slot;
use foundry_auth::secret_box::SecretBox;
use foundry_store::integrations::{KeyCheck, KeyWrapper, NewSecretsKey, SealedSlot};
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
        }
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
            prop_assert_eq!(SavedCredentials::counted(n).replaced_notice(), notice(n));
        }
    }
}
