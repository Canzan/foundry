//! The bound on how often a Foundry server reloads its integrations, read the
//! way a server reads it as it boots: through the public settings read, over
//! the whole text a deployment can give that setting.
//!
//! No page can observe this one. A setting a server refuses to start on leaves
//! no page to read, and the refusal happens where the settings are read,
//! before anything serves - so this is a crate-level check beside the public
//! read, not a scenario.
//!
//! `scripts/test-in-lima.sh cargo test -p foundry-app --test iis_secrets_key_bound`

use foundry_app::integrations::config::{
    ConfigError, IntegrationSettings, DEFAULT_RELOAD_MS, RELOAD_MS,
};
use proptest::prelude::*;
use std::time::Duration;

/// Any public address stands in here: this check reads one setting and the
/// address only keeps the read from falling back to nothing.
const PUBLIC_URL: &str = "https://foundry.canzan.dev";

/// The slowest reload a deployment may ask for: the envelope every server of
/// one Foundry must stay inside, so that no server's gap between two of its
/// own words outlives the time such a word counts as current for. The admitted
/// and refused sets below are worked out from this one number, never from what
/// the read answers.
const ENVELOPE_MS: u64 = 20_000;

/// What a deployment can have left in that setting.
#[derive(Debug, Clone)]
enum Given {
    /// Nothing at all, which reads as the default.
    Nothing,
    /// Something that is not a whole number of milliseconds.
    NotAWholeNumber(String),
    /// A whole number of milliseconds.
    Milliseconds(u64),
}

impl Given {
    /// The text a server would read, or nothing.
    fn text(&self) -> Option<String> {
        match self {
            Self::Nothing => None,
            Self::NotAWholeNumber(raw) => Some(raw.clone()),
            Self::Milliseconds(ms) => Some(ms.to_string()),
        }
    }
}

/// Everything a deployment can leave in that setting: nothing, something that
/// is not a whole number, and the whole numeric range - zero, one, every value
/// up to the envelope, the envelope itself, the first value past it, a minute,
/// and the largest number there is.
fn given_settings() -> impl Strategy<Value = Given> {
    prop_oneof![
        1 => Just(Given::Nothing),
        1 => prop_oneof![
            Just("abc".to_string()),
            Just("1.5".to_string()),
            Just("-1".to_string()),
            Just("20 s".to_string()),
        ].prop_map(Given::NotAWholeNumber),
        1 => Just(Given::Milliseconds(0)),
        1 => Just(Given::Milliseconds(1)),
        1 => Just(Given::Milliseconds(ENVELOPE_MS)),
        1 => Just(Given::Milliseconds(ENVELOPE_MS + 1)),
        1 => Just(Given::Milliseconds(60_000)),
        1 => Just(Given::Milliseconds(u64::MAX)),
        3 => (1..=ENVELOPE_MS).prop_map(Given::Milliseconds),
        3 => (ENVELOPE_MS + 1..=u64::MAX).prop_map(Given::Milliseconds),
    ]
}

/// The settings a server reads with that one setting given, and nothing else
/// set: the public read, through a lookup of this check's own.
fn read_with(given: &Given) -> Result<IntegrationSettings, ConfigError> {
    let text = given.text();
    IntegrationSettings::from_lookup(
        |name| match name == RELOAD_MS {
            true => text.clone(),
            false => None,
        },
        PUBLIC_URL,
    )
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    /// Every reload a deployment may ask for is admitted up to the envelope
    /// and refused above it, with the refusal naming the setting, the value
    /// read and the bound, so no Foundry ever serves with a reload so slow
    /// that its own servers look gone to one another.
    #[test]
    fn the_reload_setting_is_admitted_only_up_to_the_envelope(given in given_settings()) {
        let read = read_with(&given);
        match &given {
            Given::Nothing => {
                let settings = read.map_err(|e| TestCaseError::fail(format!(
                    "a Foundry with nothing in that setting starts and reloads at the default: \
                     the setting is optional, and a server that refuses to start without it \
                     cannot be deployed at all. Read an unset setting as the default; it was \
                     refused with {e}"
                )))?;
                prop_assert_eq!(
                    settings.reload,
                    Duration::from_millis(DEFAULT_RELOAD_MS),
                    "nothing in that setting reads as the default reload"
                );
            }
            Given::NotAWholeNumber(raw) => {
                let refusal = read.err().ok_or_else(|| TestCaseError::fail(format!(
                    "a Foundry given {raw:?} as that setting refuses to start and says the \
                     setting is not a whole number: a value nobody can read as a number silently \
                     becoming the default leaves the operator believing a reload they never got"
                )))?;
                prop_assert!(
                    matches!(&refusal, ConfigError::NotANumber { setting } if *setting == RELOAD_MS),
                    "a value that is not a whole number is refused as that and not as one out of \
                     range, so the operator is told what is actually wrong with it; it was \
                     refused with {}",
                    refusal
                );
            }
            Given::Milliseconds(ms) if *ms <= ENVELOPE_MS => {
                let settings = read.map_err(|e| TestCaseError::fail(format!(
                    "a Foundry asked to reload every {ms} ms starts: every reload up to the \
                     envelope is one a deployment may keep, and refusing one inside it stops a \
                     server that was serving correctly. Admit it; it was refused with {e}"
                )))?;
                prop_assert_eq!(
                    settings.reload,
                    Duration::from_millis((*ms).max(1)),
                    "an admitted reload is read as given, with nothing below one millisecond"
                );
            }
            Given::Milliseconds(ms) => {
                let refusal = read.err().ok_or_else(|| TestCaseError::fail(format!(
                    "a Foundry asked to reload every {ms} ms refuses to start: past the envelope \
                     a server leaves longer between two of its own words than the time such a \
                     word counts as current for, so its running servers look gone to one another \
                     and an admin is offered a way out that discards the credentials they are \
                     still serving with. Refuse the setting where it is read, with no settings \
                     value produced"
                )))?;
                let said = refusal.to_string();
                prop_assert!(
                    said.contains(RELOAD_MS),
                    "the refusal names the setting to change: {said}"
                );
                prop_assert!(
                    said.contains(&ms.to_string()),
                    "the refusal names the value it read, so the operator can find it: {said}"
                );
                prop_assert!(
                    said.contains(&ENVELOPE_MS.to_string()) || said.contains("20 s"),
                    "the refusal names the bound, so the operator knows what to set instead: \
                     {said}"
                );
            }
        }
    }
}
