//! `TeamName` — the ONE team-name rule (team-lifecycle D6, OQ-2; DESIGN DDD-2,
//! ADR-TEAM-LIFECYCLE-001 decision 1).
//!
//! SCAFFOLD: true — RED scaffold written by DISTILL (2026-10-11). The error enum
//! is real (the services and store gold tests name its variants); every
//! behaviour panics until DELIVER 01-01 lands the rule over the shared
//! `name_chars::is_refused_name_char` and the uniqueness helper extracted from
//! `ProjectName::ensure_unique_among`. The `Display` of each variant becomes the
//! D12 copy then (it is deliberately not written here: check-arch
//! `team-name-one-source` arrives with it).

use std::fmt;

/// The longest team name accepted, counted in Unicode scalar values of the
/// trimmed input (OQ-2).
pub const TEAM_NAME_MAX_CHARS: usize = 64;

/// Why a team name is not acceptable (D6). Flat and `Copy`, as
/// `ProjectNameError`: [`TeamName::try_new`] never yields `NotUnique`; only
/// [`TeamName::ensure_unique_among`] does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamNameError {
    Empty,
    ControlCharacter,
    TooLong,
    NotUnique,
}

impl fmt::Display for TeamNameError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("Not yet implemented -- RED scaffold (team-lifecycle 01-01: TeamNameError copy)")
    }
}

impl std::error::Error for TeamNameError {}

/// A trimmed, non-empty team name of at most [`TEAM_NAME_MAX_CHARS`] scalars
/// containing no refused character. Constructed only via [`TeamName::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TeamName(String);

impl TeamName {
    /// Trim, then refuse in order: `Empty`, `ControlCharacter`, `TooLong`.
    pub fn try_new(_raw: &str) -> Result<Self, TeamNameError> {
        panic!("Not yet implemented -- RED scaffold (team-lifecycle 01-01: TeamName::try_new)")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The project rule's uniqueness definition over `(name, stored slug)`
    /// siblings (case-insensitive name, or a non-empty derived slug equal to a
    /// sibling's stored slug).
    pub fn ensure_unique_among(&self, _siblings: &[(String, String)]) -> Result<(), TeamNameError> {
        panic!(
            "Not yet implemented -- RED scaffold (team-lifecycle 01-01: TeamName::ensure_unique_among)"
        )
    }
}
