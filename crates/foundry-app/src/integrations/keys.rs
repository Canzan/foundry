//! The key mode and the credential box (ADR-IIS-001 decisions 3, 4 and 9).
//!
//! Two tiers: an instance key this server holds (the operator's
//! `FOUNDRY_INSTANCE_KEY`, its legacy alias `FOUNDRY_SECRETS_KEY`, later a key
//! file) wraps the secrets key stored in `integration_secrets_key`; the
//! secrets key seals credentials. Until a secrets key is stored, a legacy
//! instance seals directly under the instance key (`Direct`).
//!
//! [`KeyMode::decide`] is pure and total over [`KeyFacts`]. [`Keyring::resolve`]
//! gathers those facts from the held keys and the stored rows (unwrapping is
//! crypto, not IO); the boot function in [`super`] reads the rows. Nothing
//! here implements `Debug` over key material.

use super::config::KeySetting;
use foundry_auth::secret_box::{
    GeneratedKey, SealedSecret, SecretBox, SecretSlot, SecretsKey, INSTANCE_KEY_SETTING,
};
use foundry_store::integrations::{KeyCheck, KeyWrapper, NewSecretsKey, StoredSecretsKey};
use secrecy::SecretString;
use std::collections::BTreeSet;
use std::sync::Arc;
use uuid::Uuid;

/// Why this server holds no instance key (I-27 / I-31).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoKeyReason {
    /// No operator key, and the key file cannot be made (I-27).
    Unwritable,
    /// No operator key, and the key file cannot be read (I-31).
    Unreadable,
}

/// The key mode (ADR-IIS-001 decision 4): one closed type, derived at boot
/// and on every reload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMode {
    NoInstanceKey(NoKeyReason),
    /// No stored secrets key; the legacy name is in use or some stored value
    /// is sealed directly under a held instance key (I-2e). Today's
    /// behaviour: new credentials are sealed under the instance key.
    Direct,
    /// No stored secrets key, the new name or a file, no direct seals (I-3).
    AwaitingSecretsKey,
    /// The stored secrets key unwrapped under a held instance key (I-2).
    Sealed,
    /// A secrets key is stored and no held instance key unwraps it (I-26).
    Unopenable,
}

/// The instance key file as the key source reports it (ADR-IIS-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyFile {
    /// The file supplied a key.
    Held,
    /// The file cannot be made.
    Unwritable,
    /// The file exists but cannot be read.
    Unreadable,
}

/// The stored secrets key, as far as the held instance keys go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredKey {
    Absent,
    Unwraps,
    DoesNotUnwrap,
}

/// Everything the key mode depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyFacts {
    /// Which setting supplied the operator's key, if any.
    pub operator: Option<KeySetting>,
    /// The key file; consulted only without an operator key.
    pub key_file: KeyFile,
    pub stored_key: StoredKey,
    /// Some stored sealed value's key id is a held instance key's.
    pub direct_seals: bool,
}

impl KeyMode {
    /// The mode of `facts`. Precedence: `NoInstanceKey`, then a stored key
    /// (`Sealed` or `Unopenable`), then `Direct` or `AwaitingSecretsKey`.
    pub fn decide(facts: &KeyFacts) -> Self {
        let holds = facts.operator.is_some() || facts.key_file == KeyFile::Held;
        match (holds, facts.key_file, facts.stored_key) {
            (false, KeyFile::Unreadable, _) => Self::NoInstanceKey(NoKeyReason::Unreadable),
            (false, _, _) => Self::NoInstanceKey(NoKeyReason::Unwritable),
            (true, _, StoredKey::Unwraps) => Self::Sealed,
            (true, _, StoredKey::DoesNotUnwrap) => Self::Unopenable,
            (true, _, StoredKey::Absent)
                if facts.direct_seals || facts.operator == Some(KeySetting::LegacySecretsKey) =>
            {
                Self::Direct
            }
            (true, _, StoredKey::Absent) => Self::AwaitingSecretsKey,
        }
    }

    /// Whether new credentials can be sealed: only `Direct` and `Sealed`.
    pub fn seals(self) -> bool {
        matches!(self, Self::Direct | Self::Sealed)
    }

    /// Whether Generate applies (ADR-IIS-001 decision 5): only while awaiting
    /// a secrets key, and never while some stored value opens under no held
    /// key (I-4, `unopenable_values`).
    pub fn generates(self, unopenable_values: bool) -> bool {
        self == Self::AwaitingSecretsKey && !unopenable_values
    }

    /// Whether Move applies (ADR-IIS-001 r5): only while sealing directly,
    /// and never under I-4.
    pub fn moves(self, unopenable_values: bool) -> bool {
        self == Self::Direct && !unopenable_values
    }
}

/// What Generate stores: a new secrets key wrapped under the sealing
/// instance key, and the new key's check value. No `Debug`: it carries the
/// wrapped key and the new key itself, which Move seals with.
pub struct GeneratePlan {
    pub key: NewSecretsKey,
    pub canary: KeyCheck,
    secrets: SecretBox,
}

impl GeneratePlan {
    /// `sealed` in `slot`, opened under the held `instance` keys and sealed
    /// again under the new secrets key; `None` when no held instance key
    /// opens it (Move then changes nothing).
    pub fn reseal(
        &self,
        instance: &SecretBox,
        slot: &SecretSlot,
        sealed: &[u8],
    ) -> Option<Vec<u8>> {
        let opened = instance
            .open(&SealedSecret::from_bytes(sealed.to_vec()), slot)
            .ok()?;
        let resealed = self.secrets.seal(&opened, slot).ok()?;
        Some(resealed.as_bytes().to_vec())
    }
}

/// A new secrets key, wrapped under `instance`'s current key (decision 2:
/// the value wrapped is the key's base64url text), with its canary. `None`
/// when the random source or sealing fails.
pub fn plan_generate(
    instance: &SecretBox,
    wrapped_by: KeyWrapper,
    created_by: Option<Uuid>,
) -> Option<GeneratePlan> {
    let generated = SecretsKey::generate().ok()?;
    plan_of(instance, generated, wrapped_by, created_by)
}

fn plan_of(
    instance: &SecretBox,
    generated: GeneratedKey,
    wrapped_by: KeyWrapper,
    created_by: Option<Uuid>,
) -> Option<GeneratePlan> {
    let wrapped = instance.seal(&generated.text, &secrets_key_slot()).ok()?;
    let secrets = SecretBox::new(generated.key, Vec::new());
    let canary = super::key_check(&secrets)?;
    Some(GeneratePlan {
        key: NewSecretsKey {
            key_id: secrets.current_key_id().to_string(),
            wrapped_key: wrapped.as_bytes().to_vec(),
            wrapped_by_key_id: instance.current_key_id().to_string(),
            wrapped_by,
            created_by,
        },
        canary,
        secrets,
    })
}

/// Where the instance key file's key comes from: an injected capability, so
/// the boot function never touches the filesystem itself.
pub trait InstanceKeySource: Send + Sync {
    /// Read (or make) the key file.
    fn key_file(&self) -> KeyFileRead;
}

/// What a key source read. No `Debug`: it carries a key.
pub enum KeyFileRead {
    Held(Box<SecretsKey>),
    Unwritable,
    Unreadable,
}

/// No key file adapter (until ADR-IIS-002 lands): the file cannot be made.
pub struct NoKeyFile;

impl InstanceKeySource for NoKeyFile {
    fn key_file(&self) -> KeyFileRead {
        KeyFileRead::Unwritable
    }
}

/// The slot the stored secrets key is wrapped in (decision 2).
pub fn secrets_key_slot() -> SecretSlot {
    SecretSlot::new("integration_secrets_key", "1", "wrapped_key")
}

/// The stored secrets key, opened under the held instance keys and read back
/// as a key whose id is the stored one.
fn unwrap_secrets_key(instance: &SecretBox, stored: &StoredSecretsKey) -> Option<SecretBox> {
    let text = instance
        .open(
            &SealedSecret::from_bytes(stored.wrapped_key.clone()),
            &secrets_key_slot(),
        )
        .ok()?;
    let key = SecretsKey::parse(&text, INSTANCE_KEY_SETTING).ok()?;
    (key.key_id().to_string() == stored.key_id).then(|| SecretBox::new(key, Vec::new()))
}

/// The unwrapped secrets key (when any) and the mode. No `Debug`.
pub struct Keyring {
    /// Shared with the keyring a reload rebuilds, when the stored key id is
    /// unchanged (ADR-IIS-004 r6).
    secrets_key: Option<Arc<SecretBox>>,
    mode: KeyMode,
    opens_stored: bool,
}

impl Keyring {
    /// The keyring of the held `instance` keys, read from `operator` (or the
    /// key file), over the stored secrets key and the key ids of every stored
    /// sealed value.
    pub fn resolve(
        instance: Option<&SecretBox>,
        operator: Option<KeySetting>,
        key_file: KeyFile,
        stored: Option<&StoredSecretsKey>,
        stored_key_ids: &BTreeSet<String>,
    ) -> Self {
        Self::resolve_holding(None, instance, operator, key_file, stored, stored_key_ids)
    }

    /// [`Self::resolve`], keeping the `held` secrets key without unwrapping
    /// when the stored key id is still its id (ADR-IIS-004 r6): a re-wrap
    /// under an instance key this replica lacks never blinds it.
    pub fn resolve_holding(
        held_secrets_key: Option<&Arc<SecretBox>>,
        instance: Option<&SecretBox>,
        operator: Option<KeySetting>,
        key_file: KeyFile,
        stored: Option<&StoredSecretsKey>,
        stored_key_ids: &BTreeSet<String>,
    ) -> Self {
        let held_ids: BTreeSet<String> = instance
            .map(|keys| keys.key_ids().iter().map(ToString::to_string).collect())
            .unwrap_or_default();
        let kept = held_secrets_key.zip(stored).and_then(|(keys, row)| {
            (keys.current_key_id().to_string() == row.key_id).then(|| keys.clone())
        });
        let secrets_key = kept.or_else(|| {
            instance
                .zip(stored)
                .and_then(|(keys, row)| unwrap_secrets_key(keys, row))
                .map(Arc::new)
        });
        let stored_key = match (stored, &secrets_key) {
            (None, _) => StoredKey::Absent,
            (Some(_), Some(_)) => StoredKey::Unwraps,
            (Some(_), None) => StoredKey::DoesNotUnwrap,
        };
        let direct_seals = direct_seal_count(stored_key_ids, &held_ids) > 0;
        let opens_secrets_sealed = secrets_key.as_ref().is_some_and(|keys| {
            let id = keys.current_key_id().to_string();
            stored_key_ids.contains(&id)
        });
        let facts = KeyFacts {
            operator,
            key_file,
            stored_key,
            direct_seals,
        };
        Self {
            secrets_key,
            mode: KeyMode::decide(&facts),
            opens_stored: direct_seals || opens_secrets_sealed,
        }
    }

    pub fn mode(&self) -> KeyMode {
        self.mode
    }

    /// The unwrapped secrets key, for the next rebuild to keep.
    pub fn secrets_key(&self) -> Option<&Arc<SecretBox>> {
        self.secrets_key.as_ref()
    }

    /// Whether a held key opens some stored sealed value (by key id).
    pub fn opens_stored(&self) -> bool {
        self.opens_stored
    }

    /// The credential box over the held `instance` keys.
    pub fn credentials<'a>(&'a self, instance: Option<&'a SecretBox>) -> CredentialBox<'a> {
        CredentialBox {
            secrets_key: self.secrets_key.as_deref(),
            instance,
            mode: self.mode,
        }
    }
}

/// How many stored key ids are a held instance key's: the values sealed
/// directly, counted by key id with no stored flag (ADR-IIS-001 r3).
pub fn direct_seal_count(
    stored_key_ids: &BTreeSet<String>,
    held_key_ids: &BTreeSet<String>,
) -> usize {
    stored_key_ids.intersection(held_key_ids).count()
}

/// The credential box (decision 3): the current key is the secrets key when
/// `Sealed`, otherwise the instance key; every held instance key is a
/// previous key, so legacy direct seals open with no action.
pub struct CredentialBox<'a> {
    secrets_key: Option<&'a SecretBox>,
    instance: Option<&'a SecretBox>,
    mode: KeyMode,
}

impl<'a> CredentialBox<'a> {
    /// The box new credentials are sealed with: only in `Direct` or `Sealed`.
    pub fn sealing(&self) -> Option<&'a SecretBox> {
        match self.mode {
            KeyMode::Sealed => self.secrets_key,
            KeyMode::Direct => self.instance,
            _ => None,
        }
    }

    /// `sealed` opened under whichever held key sealed it.
    pub fn open(&self, sealed: &SealedSecret, slot: &SecretSlot) -> Option<SecretString> {
        self.boxes().find_map(|keys| keys.open(sealed, slot).ok())
    }

    /// Whether this server holds the key `key_id`.
    pub fn holds(&self, key_id: &str) -> bool {
        self.boxes()
            .flat_map(SecretBox::key_ids)
            .any(|id| id.to_string() == key_id)
    }

    /// Whether this server holds any key at all.
    pub fn holds_a_key(&self) -> bool {
        self.boxes().next().is_some()
    }

    /// The secrets key first, then the instance keys.
    fn boxes(&self) -> impl Iterator<Item = &'a SecretBox> {
        self.secrets_key.into_iter().chain(self.instance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use proptest::prelude::*;
    use secrecy::ExposeSecret;
    use KeySetting::{InstanceKey, LegacySecretsKey};

    fn key(byte: u8) -> SecretsKey {
        let text = SecretString::from(URL_SAFE_NO_PAD.encode([byte; 32]));
        SecretsKey::parse(&text, INSTANCE_KEY_SETTING).expect("32 bytes")
    }

    /// The operator's key 1 with previous keys 2 and 3.
    fn instance_box() -> SecretBox {
        SecretBox::new(key(1), vec![key(2), key(3)])
    }

    fn id(byte: u8) -> String {
        key(byte).key_id().to_string()
    }

    /// A secrets key, wrapped under `wrapper`'s current key.
    fn stored_under(wrapper: &SecretBox) -> (StoredSecretsKey, String) {
        let generated = SecretsKey::generate().expect("the system random source");
        let wrapped = wrapper
            .seal(&generated.text, &secrets_key_slot())
            .expect("wrap");
        let key_id = generated.key.key_id().to_string();
        let stored = StoredSecretsKey {
            key_id: key_id.clone(),
            wrapped_key: wrapped.as_bytes().to_vec(),
            wrapped_by_key_id: wrapper.current_key_id().to_string(),
            wrapped_by: foundry_store::integrations::KeyWrapper::Operator,
            rewrapped_from_path: None,
            created_by: None,
            created_at: time::OffsetDateTime::UNIX_EPOCH,
        };
        (stored, key_id)
    }

    /// The ADR's table, read row by row in its precedence order.
    fn adr_mode(facts: &KeyFacts) -> KeyMode {
        let holds = facts.operator.is_some() || facts.key_file == KeyFile::Held;
        if !holds && facts.key_file == KeyFile::Unreadable {
            return KeyMode::NoInstanceKey(NoKeyReason::Unreadable);
        }
        if !holds {
            return KeyMode::NoInstanceKey(NoKeyReason::Unwritable);
        }
        if facts.stored_key == StoredKey::Unwraps {
            return KeyMode::Sealed;
        }
        if facts.stored_key == StoredKey::DoesNotUnwrap {
            return KeyMode::Unopenable;
        }
        if facts.operator == Some(LegacySecretsKey) || facts.direct_seals {
            return KeyMode::Direct;
        }
        KeyMode::AwaitingSecretsKey
    }

    /// Every (operator key and name, file state, stored key and whether it
    /// unwraps, direct seals): the mode the ADR's table gives.
    #[test]
    fn every_combination_of_facts_takes_the_adr_mode() {
        let mut seen = BTreeSet::new();
        for operator in [None, Some(InstanceKey), Some(LegacySecretsKey)] {
            for key_file in [KeyFile::Held, KeyFile::Unwritable, KeyFile::Unreadable] {
                for stored_key in [
                    StoredKey::Absent,
                    StoredKey::Unwraps,
                    StoredKey::DoesNotUnwrap,
                ] {
                    for direct_seals in [false, true] {
                        let facts = KeyFacts {
                            operator,
                            key_file,
                            stored_key,
                            direct_seals,
                        };
                        let mode = KeyMode::decide(&facts);
                        assert_eq!(mode, adr_mode(&facts), "{facts:?}");
                        seen.insert(format!("{mode:?}"));
                    }
                }
            }
        }
        assert_eq!(seen.len(), 6, "every mode is reachable: {seen:?}");
    }

    /// Named rows of the table, the ones the existing suites rely on.
    #[rustfmt::skip]
    #[test]
    fn the_rows_existing_instances_rely_on() {
        use KeyMode::*;
        let facts = |operator, key_file, stored_key, direct_seals| KeyFacts { operator, key_file, stored_key, direct_seals };
        let table = [
            // no operator key, no key file adapter: today's K-18 / K-20
            (facts(None, KeyFile::Unwritable, StoredKey::Absent, false), NoInstanceKey(NoKeyReason::Unwritable)),
            // a stored key never makes a keyless server Sealed
            (facts(None, KeyFile::Unreadable, StoredKey::Unwraps, true), NoInstanceKey(NoKeyReason::Unreadable)),
            // the legacy name on a fresh instance: Matrix at once (KPI-IIS-G3)
            (facts(Some(LegacySecretsKey), KeyFile::Unwritable, StoredKey::Absent, false), Direct),
            // the same key renamed keeps a legacy instance direct
            (facts(Some(InstanceKey), KeyFile::Unwritable, StoredKey::Absent, true), Direct),
            (facts(Some(InstanceKey), KeyFile::Unwritable, StoredKey::Absent, false), AwaitingSecretsKey),
            // a stored key outranks the legacy name and direct seals
            (facts(Some(LegacySecretsKey), KeyFile::Unwritable, StoredKey::Unwraps, true), Sealed),
            (facts(Some(LegacySecretsKey), KeyFile::Unwritable, StoredKey::DoesNotUnwrap, true), Unopenable),
            (facts(None, KeyFile::Held, StoredKey::Absent, false), AwaitingSecretsKey),
        ];
        for (facts, mode) in table {
            assert_eq!(KeyMode::decide(&facts), mode, "{facts:?}");
        }
    }

    #[test]
    fn a_stored_secrets_key_unwraps_under_a_previous_instance_key_and_seals_new_credentials() {
        let instance = instance_box();
        let (stored, secrets_key_id) = stored_under(&SecretBox::new(key(3), vec![]));
        let keyring = Keyring::resolve(
            Some(&instance),
            Some(InstanceKey),
            KeyFile::Unwritable,
            Some(&stored),
            &BTreeSet::from([id(2)]),
        );
        assert_eq!(keyring.mode(), KeyMode::Sealed);
        let credentials = keyring.credentials(Some(&instance));
        let slot = || SecretSlot::new("integration_connections", "c", "sealed_credential");
        let sealed = credentials
            .sealing()
            .expect("Sealed seals")
            .seal(&SecretString::from("bot-token"), &slot())
            .expect("seal");
        assert_eq!(sealed.key_id().to_string(), secrets_key_id);
        let opened = credentials.open(&sealed, &slot()).expect("opens");
        assert_eq!(opened.expose_secret(), "bot-token");
        // a legacy direct seal under a previous instance key still opens
        let direct = SecretBox::new(key(2), vec![])
            .seal(&SecretString::from("old-token"), &slot())
            .expect("seal");
        let opened = credentials.open(&direct, &slot()).expect("opens");
        assert_eq!(opened.expose_secret(), "old-token");
        assert!(credentials.holds(&secrets_key_id) && credentials.holds(&id(1)));
        assert!(!credentials.holds(&id(9)));
    }

    #[test]
    fn a_secrets_key_wrapped_under_a_key_not_held_is_unopenable_and_seals_nothing() {
        let instance = instance_box();
        let (stored, secrets_key_id) = stored_under(&SecretBox::new(key(9), vec![]));
        let keyring = Keyring::resolve(
            Some(&instance),
            Some(LegacySecretsKey),
            KeyFile::Unwritable,
            Some(&stored),
            &BTreeSet::from([id(1)]),
        );
        assert_eq!(keyring.mode(), KeyMode::Unopenable);
        assert!(
            keyring.opens_stored(),
            "the direct seal under key 1 still opens"
        );
        let credentials = keyring.credentials(Some(&instance));
        assert!(credentials.sealing().is_none());
        assert!(!credentials.holds(&secrets_key_id));
    }

    #[test]
    fn a_stored_row_whose_key_id_is_not_the_wrapped_keys_does_not_unwrap() {
        let instance = instance_box();
        let (mut stored, _) = stored_under(&instance);
        stored.key_id = id(7);
        let keyring = Keyring::resolve(
            Some(&instance),
            Some(InstanceKey),
            KeyFile::Unwritable,
            Some(&stored),
            &BTreeSet::new(),
        );
        assert_eq!(keyring.mode(), KeyMode::Unopenable);
    }

    #[test]
    fn without_any_held_key_nothing_seals_or_opens() {
        let keyring = Keyring::resolve(
            None,
            None,
            KeyFile::Unwritable,
            None,
            &BTreeSet::from([id(1)]),
        );
        assert_eq!(
            keyring.mode(),
            KeyMode::NoInstanceKey(NoKeyReason::Unwritable)
        );
        assert!(!keyring.opens_stored());
        let credentials = keyring.credentials(None);
        assert!(credentials.sealing().is_none() && !credentials.holds_a_key());
        assert!(!credentials.holds(&id(1)));
    }

    /// The stored row a plan's key becomes.
    fn stored_of(plan: &GeneratePlan) -> StoredSecretsKey {
        StoredSecretsKey {
            key_id: plan.key.key_id.clone(),
            wrapped_key: plan.key.wrapped_key.clone(),
            wrapped_by_key_id: plan.key.wrapped_by_key_id.clone(),
            wrapped_by: plan.key.wrapped_by,
            rewrapped_from_path: None,
            created_by: plan.key.created_by,
            created_at: time::OffsetDateTime::UNIX_EPOCH,
        }
    }

    /// The Generate gate over every mode, with and without I-4: only
    /// `AwaitingSecretsKey` without I-4 generates.
    #[rustfmt::skip]
    #[test]
    fn generate_applies_only_while_awaiting_a_secrets_key_and_never_under_i4() {
        use KeyMode::*;
        let table = [
            (NoInstanceKey(NoKeyReason::Unwritable), false, false),
            (NoInstanceKey(NoKeyReason::Unwritable), true, false),
            (NoInstanceKey(NoKeyReason::Unreadable), false, false),
            (NoInstanceKey(NoKeyReason::Unreadable), true, false),
            (Direct, false, false),
            (Direct, true, false),
            (AwaitingSecretsKey, false, true),
            (AwaitingSecretsKey, true, false),
            (Sealed, false, false),
            (Sealed, true, false),
            (Unopenable, false, false),
            (Unopenable, true, false),
        ];
        for (mode, i4, generates) in table {
            assert_eq!(mode.generates(i4), generates, "{mode:?}, I-4 {i4}");
        }
    }

    /// The Move gate over every mode, with and without I-4: only `Direct`
    /// without I-4 moves.
    #[rustfmt::skip]
    #[test]
    fn move_applies_only_while_sealing_directly_and_never_under_i4() {
        use KeyMode::*;
        let table = [
            (NoInstanceKey(NoKeyReason::Unwritable), false, false),
            (NoInstanceKey(NoKeyReason::Unwritable), true, false),
            (NoInstanceKey(NoKeyReason::Unreadable), false, false),
            (NoInstanceKey(NoKeyReason::Unreadable), true, false),
            (Direct, false, true),
            (Direct, true, false),
            (AwaitingSecretsKey, false, false),
            (AwaitingSecretsKey, true, false),
            (Sealed, false, false),
            (Sealed, true, false),
            (Unopenable, false, false),
            (Unopenable, true, false),
        ];
        for (mode, i4, moves) in table {
            assert_eq!(mode.moves(i4), moves, "{mode:?}, I-4 {i4}");
        }
    }

    /// A value sealed directly under a held instance key reseals under the
    /// plan's new secrets key, which alone opens it then; one no held key
    /// opens is refused.
    #[test]
    fn a_move_reseals_what_a_held_instance_key_opens_and_refuses_the_rest() {
        let instance = instance_box();
        let plan = plan_generate(
            &instance,
            foundry_store::integrations::KeyWrapper::Operator,
            None,
        )
        .expect("a plan");
        let slot = secrets_key_slot();
        let token = SecretString::from("xoxb-direct");
        let direct = SecretBox::new(key(2), Vec::new())
            .seal(&token, &slot)
            .expect("seal");
        let resealed = plan
            .reseal(&instance, &slot, direct.as_bytes())
            .expect("a held key opens it");
        let opened = plan
            .secrets
            .open(&SealedSecret::from_bytes(resealed.clone()), &slot)
            .expect("the secrets key opens it");
        assert_eq!(opened.expose_secret(), "xoxb-direct");
        assert!(instance
            .open(&SealedSecret::from_bytes(resealed), &slot)
            .is_err());
        let foreign = SecretBox::new(key(9), Vec::new())
            .seal(&token, &slot)
            .expect("seal");
        assert!(plan.reseal(&instance, &slot, foreign.as_bytes()).is_none());
    }

    /// Held ids are keys 1–3 (the operator's and its previous keys); 4–6 are
    /// keys this server never held.
    fn stored_ids(mask: u8) -> BTreeSet<String> {
        (1..=6u8)
            .filter(|byte| mask & (1 << (byte - 1)) != 0)
            .map(id)
            .collect()
    }

    proptest! {
        /// Over arbitrary sets of stored key ids, a stored key or not, and
        /// either name: sealing is possible iff the mode is Direct or Sealed,
        /// and with no stored key under the new name the mode is Direct iff
        /// some stored id is a held instance key's.
        #[test]
        fn sealing_follows_the_mode_and_direct_seals_select_direct(
            mask in 0u8..64,
            stored in 0u8..3,
            legacy in any::<bool>(),
        ) {
            let instance = instance_box();
            let ids = stored_ids(mask);
            let wrapper = if stored == 1 { SecretBox::new(key(2), vec![]) } else { SecretBox::new(key(8), vec![]) };
            let row = (stored > 0).then(|| stored_under(&wrapper).0);
            let operator = Some(if legacy { LegacySecretsKey } else { InstanceKey });
            let keyring = Keyring::resolve(Some(&instance), operator, KeyFile::Unwritable, row.as_ref(), &ids);
            let mode = keyring.mode();
            let credentials = keyring.credentials(Some(&instance));
            prop_assert_eq!(credentials.sealing().is_some(), matches!(mode, KeyMode::Direct | KeyMode::Sealed));
            prop_assert_eq!(mode.seals(), credentials.sealing().is_some());
            let some_held = mask & 0b111 != 0;
            prop_assert_eq!(keyring.opens_stored(), some_held);
            if row.is_none() && !legacy {
                prop_assert_eq!(mode == KeyMode::Direct, some_held);
                prop_assert_eq!(mode == KeyMode::AwaitingSecretsKey, !some_held);
            }
            if row.is_none() && some_held {
                prop_assert_eq!(mode, KeyMode::Direct);
            }
        }
    }

    proptest! {
        /// Over arbitrary stored key ids, with keys 1–3 held: the direct-seal
        /// count is the number of stored ids among the held ones.
        #[test]
        fn the_direct_seal_count_is_the_stored_ids_a_held_key_has(mask in 0u8..64) {
            let held: BTreeSet<String> = instance_box().key_ids().iter().map(ToString::to_string).collect();
            let expected = (mask & 0b111).count_ones() as usize;
            prop_assert_eq!(direct_seal_count(&stored_ids(mask), &held), expected);
        }
    }

    proptest! {
        /// For any instance key: generate, wrap, unwrap and parse round-trip
        /// to the same key id; the canary is the new key's; the row unwraps
        /// to `Sealed` and seals under the new key.
        #[test]
        fn a_generated_key_wrapped_under_any_instance_key_unwraps_to_the_same_key_id(
            bytes in any::<[u8; 32]>(),
            created_by in any::<u128>(),
        ) {
            let text = SecretString::from(URL_SAFE_NO_PAD.encode(bytes));
            let instance = SecretBox::new(SecretsKey::parse(&text, INSTANCE_KEY_SETTING).expect("32 bytes"), vec![]);
            let actor = Some(Uuid::from_u128(created_by));
            let plan = plan_generate(&instance, KeyWrapper::Operator, actor).expect("the plan");
            prop_assert_eq!(&plan.key.wrapped_by_key_id, &instance.current_key_id().to_string());
            prop_assert_eq!(&plan.canary.key_id, &plan.key.key_id);
            prop_assert_eq!(plan.key.created_by, actor);
            let stored = stored_of(&plan);
            let unwrapped = unwrap_secrets_key(&instance, &stored).expect("unwraps");
            prop_assert_eq!(unwrapped.current_key_id().to_string(), plan.key.key_id.clone());
            let keyring = Keyring::resolve(Some(&instance), Some(InstanceKey), KeyFile::Unwritable, Some(&stored), &BTreeSet::new());
            prop_assert_eq!(keyring.mode(), KeyMode::Sealed);
            let sealing = keyring.credentials(Some(&instance)).sealing().map(|b| b.current_key_id().to_string());
            prop_assert_eq!(sealing, Some(plan.key.key_id.clone()));
        }
    }
}
