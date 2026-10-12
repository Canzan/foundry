//! `TeamName` — the ONE team-name rule (team-lifecycle D6, OQ-2; DESIGN DDD-2,
//! DDD-3, DDD-4, ADR-TEAM-LIFECYCLE-001 decisions 1 and 3).
//!
//! The project rule at a cap of 64: the refused-character set is
//! [`crate::name_chars`]'s, and the uniqueness definition and the lowest-free
//! address search are the helpers `ProjectName` uses too, so the rules cannot
//! drift apart. The `Display` of each [`TeamNameError`] variant IS the D12 copy.

use thiserror::Error;

use crate::name_chars::is_refused_name_char;
use crate::project_name::{collides_with_sibling, lowest_free_slug};
use crate::slugify;

/// The longest team name accepted, counted in Unicode scalar values of the
/// trimmed input (OQ-2).
pub const TEAM_NAME_MAX_CHARS: usize = 64;

/// The address a team whose name derives none takes, before suffixing.
const TEAM_FALLBACK_SLUG: &str = "team";

/// Why a team name is not acceptable (D6). Flat and `Copy`, as
/// `ProjectNameError`; the `Display` of each variant is the D12 copy, byte for
/// byte. [`TeamName::try_new`] never yields `NotUnique`; only
/// [`TeamName::ensure_unique_among`] does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TeamNameError {
    #[error("Team name must not be empty")]
    Empty,
    #[error("Team name must not contain control characters")]
    ControlCharacter,
    #[error("Team name must be at most {TEAM_NAME_MAX_CHARS} characters")]
    TooLong,
    #[error("Team name must be unique within the workspace")]
    NotUnique,
}

/// A trimmed, non-empty team name of at most [`TEAM_NAME_MAX_CHARS`] scalars
/// containing no refused character. Constructed only via [`TeamName::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TeamName(String);

impl TeamName {
    /// Trim, then refuse in order: `Empty`, `ControlCharacter`, `TooLong`.
    pub fn try_new(raw: &str) -> Result<Self, TeamNameError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(TeamNameError::Empty);
        }
        if trimmed.chars().any(is_refused_name_char) {
            return Err(TeamNameError::ControlCharacter);
        }
        if trimmed.chars().count() > TEAM_NAME_MAX_CHARS {
            return Err(TeamNameError::TooLong);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The project rule's uniqueness definition over `(name, stored slug)`
    /// siblings (case-insensitive name, or a non-empty derived slug equal to a
    /// sibling's stored slug).
    pub fn ensure_unique_among(&self, siblings: &[(String, String)]) -> Result<(), TeamNameError> {
        if collides_with_sibling(&self.0, siblings) {
            Err(TeamNameError::NotUnique)
        } else {
            Ok(())
        }
    }
}

/// The address a create stores for a new team, tagged with how it was minted
/// (DDD-4). `Derived` is `slugify` of the name; `Fallback` is the lowest free of
/// `team`, `team-2`, … for a name that derives none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MintedTeamSlug {
    Derived(String),
    Fallback(String),
}

impl MintedTeamSlug {
    /// The address itself, whichever way it was minted.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Derived(slug) | Self::Fallback(slug) => slug,
        }
    }
}

/// The one team-address mint (DDD-4, ADR-TEAM-LIFECYCLE-001 decision 3). Total:
/// never refuses, never returns `""`. `Derived` when [`slugify`] of the name is
/// non-empty, used verbatim (the sibling check owns derived collisions); else
/// `Fallback` of the first of `team`, `team-2`, … not among `taken`, the
/// workspace's stored team addresses.
pub fn mint_team_slug(name: &TeamName, taken: &[String]) -> MintedTeamSlug {
    let derived = slugify(name.as_str());
    if derived.is_empty() {
        MintedTeamSlug::Fallback(lowest_free_slug(TEAM_FALLBACK_SLUG, taken))
    } else {
        MintedTeamSlug::Derived(derived)
    }
}

/// What a rename asks for once the store reads are in hand (DDD-14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeamRenameVerdict {
    /// The trimmed input is byte-equal to the current name: write nothing (D15).
    Unchanged,
    /// Store this name.
    Rename(TeamName),
}

/// The one rename classification (DDD-14): trim, then the no-op, then
/// [`TeamName::try_new`], then [`TeamName::ensure_unique_among`] over the
/// workspace's OTHER teams. `siblings_without_self` must exclude the team being
/// renamed, so a case-only change is a rename and never collides with itself.
pub fn classify_team_rename(
    raw: &str,
    current: &str,
    siblings_without_self: &[(String, String)],
) -> Result<TeamRenameVerdict, TeamNameError> {
    if raw.trim() == current {
        return Ok(TeamRenameVerdict::Unchanged);
    }
    let name = TeamName::try_new(raw)?;
    name.ensure_unique_among(siblings_without_self)?;
    Ok(TeamRenameVerdict::Rename(name))
}

#[cfg(test)]
mod team_name_rule {
    //! The four arms at their boundaries (64/65, padded, astral) and the D12
    //! copy. bypass: boundary and exact-string contracts are single examples;
    //! the quantifiable invariants are the properties below.
    use super::*;

    #[test]
    fn sixty_four_scalars_are_accepted_and_sixty_five_are_too_long() {
        let at_cap = "a".repeat(TEAM_NAME_MAX_CHARS);
        assert_eq!(TeamName::try_new(&at_cap).map(|n| n.0), Ok(at_cap.clone()));
        assert_eq!(
            TeamName::try_new(&format!("{at_cap}a")),
            Err(TeamNameError::TooLong)
        );
    }

    #[test]
    fn padding_is_trimmed_before_counting() {
        let at_cap = "a".repeat(TEAM_NAME_MAX_CHARS);
        assert_eq!(
            TeamName::try_new(&format!("  {at_cap}\t")).map(|n| n.0),
            Ok(at_cap)
        );
    }

    #[test]
    fn astral_scalars_count_once_each_not_by_bytes() {
        let at_cap = "\u{1D538}".repeat(TEAM_NAME_MAX_CHARS);
        assert!(TeamName::try_new(&at_cap).is_ok());
        assert_eq!(
            TeamName::try_new(&format!("{at_cap}\u{1D538}")),
            Err(TeamNameError::TooLong)
        );
    }

    #[test]
    fn blank_is_empty_and_a_refused_character_wins_over_length() {
        assert_eq!(TeamName::try_new(" \t "), Err(TeamNameError::Empty));
        let long_with_control = format!("{}\u{202E}", "a".repeat(80));
        assert_eq!(
            TeamName::try_new(&long_with_control),
            Err(TeamNameError::ControlCharacter)
        );
    }

    #[test]
    fn the_copy_is_d12_byte_for_byte() {
        assert_eq!(
            TeamNameError::Empty.to_string(),
            "Team name must not be empty"
        );
        assert_eq!(
            TeamNameError::ControlCharacter.to_string(),
            "Team name must not contain control characters"
        );
        assert_eq!(
            TeamNameError::TooLong.to_string(),
            "Team name must be at most 64 characters"
        );
        assert_eq!(
            TeamNameError::NotUnique.to_string(),
            "Team name must be unique within the workspace"
        );
    }

    #[test]
    fn uniqueness_is_case_insensitive_on_name_and_exact_on_a_non_empty_address() {
        let siblings = vec![
            ("Backend".to_string(), "backend".to_string()),
            ("インフラ".to_string(), "team".to_string()),
        ];
        let check = |raw: &str| {
            TeamName::try_new(raw)
                .unwrap()
                .ensure_unique_among(&siblings)
        };
        assert_eq!(check("BACKEND"), Err(TeamNameError::NotUnique));
        // "Backend!" derives `backend`, a sibling's stored address.
        assert_eq!(check("Backend!"), Err(TeamNameError::NotUnique));
        assert_eq!(check("Platform"), Ok(()));
        // "運用" derives no address, so it never matches "team" on the address arm.
        assert_eq!(check("運用"), Ok(()));
    }
}

#[cfg(test)]
mod team_name_properties {
    use super::*;
    use crate::{ProjectName, ProjectNameError, WorkspaceName, WorkspaceNameError};
    use proptest::prelude::*;

    proptest! {
        /// DESIGN Test Seams: for every scalar, the team rule's control verdict
        /// equals the project and workspace rules'.
        #[test]
        fn the_control_verdict_equals_project_and_workspace_rules(c in any::<char>()) {
            let raw = format!("a{c}b");
            let team = TeamName::try_new(&raw) == Err(TeamNameError::ControlCharacter);
            let project = ProjectName::try_new(&raw) == Err(ProjectNameError::ControlCharacter);
            let workspace =
                WorkspaceName::try_new(&raw) == Err(WorkspaceNameError::ControlCharacter);
            prop_assert_eq!(team, project);
            prop_assert_eq!(team, workspace);
        }

        /// The mint is total and never empty: the derived address when the name
        /// derives one, else the lowest free of `team`, `team-2`, ….
        #[test]
        fn the_mint_is_derived_or_the_lowest_free_fallback(
            raw in "[a-zA-Z0-9 \u{30A4}\u{30F3}\u{904B}\u{7528}-]{1,20}",
            suffixes in proptest::collection::btree_set(1u32..8, 0..6),
        ) {
            prop_assume!(TeamName::try_new(&raw).is_ok());
            let name = TeamName::try_new(&raw).unwrap();
            let taken: Vec<String> = suffixes
                .iter()
                .map(|n| if *n == 1 { "team".to_string() } else { format!("team-{n}") })
                .collect();
            let minted = mint_team_slug(&name, &taken);
            prop_assert!(!minted.as_str().is_empty());
            let derived = slugify(&raw);
            if derived.is_empty() {
                let lowest = (1u32..).find(|n| !suffixes.contains(n)).unwrap();
                let want = if lowest == 1 { "team".to_string() } else { format!("team-{lowest}") };
                prop_assert_eq!(minted.as_str(), want.as_str());
                prop_assert_eq!(minted, MintedTeamSlug::Fallback(want));
            } else {
                prop_assert_eq!(minted.as_str(), derived.as_str());
                prop_assert_eq!(minted, MintedTeamSlug::Derived(derived));
            }
        }
    }
}

#[cfg(test)]
mod team_rename_classification {
    //! DDD-14: no-op first, case-only is a rename, and a changed name gets the
    //! create rule's verdict (create/rename parity, KPI-TL-6).
    use super::*;
    use proptest::prelude::*;

    /// Other teams: digit names, so none collides with a letters-only name.
    fn others() -> impl Strategy<Value = Vec<(String, String)>> {
        proptest::collection::btree_set(100u32..999, 0..5).prop_map(|ns| {
            ns.into_iter()
                .map(|n| (n.to_string(), n.to_string()))
                .collect()
        })
    }

    proptest! {
        /// A raw that trims to the byte-equal current name is Unchanged before
        /// any refusal: even an over-cap legacy name, and even when a sibling
        /// would collide with it.
        #[test]
        fn a_byte_equal_name_is_unchanged_before_any_refusal(
            current in "[a-zA-Z][a-zA-Z ]{0,80}[a-zA-Z]",
            pad_left in "[ \t]{0,3}",
            pad_right in "[ \t]{0,3}",
            others in others(),
        ) {
            let mut siblings = others;
            siblings.push((current.to_uppercase(), slugify(&current)));
            let raw = format!("{pad_left}{current}{pad_right}");
            prop_assert_eq!(
                classify_team_rename(&raw, &current, &siblings),
                Ok(TeamRenameVerdict::Unchanged)
            );
        }

        /// A case-only change is never Unchanged and never collides with the
        /// team itself: it is a rename to exactly the typed name.
        #[test]
        fn a_case_only_change_is_a_rename(
            current in "[a-z][a-zA-Z ]{0,40}[a-zA-Z]",
            others in others(),
        ) {
            let typed: String = current
                .chars()
                .map(|c| if c.is_ascii_lowercase() { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() })
                .collect();
            prop_assert_ne!(&typed, &current);
            let verdict = classify_team_rename(&typed, &current, &others);
            prop_assert_eq!(verdict, Ok(TeamRenameVerdict::Rename(TeamName::try_new(&typed).unwrap())));
        }

        /// On a changed name the verdict is the create rule's: `try_new`, then
        /// uniqueness over the other teams.
        #[test]
        fn a_changed_name_gets_the_create_verdict(
            raw in "[ a-zA-Z0-9!\u{202E}]{0,70}",
            current in "[a-zA-Z]{1,10}",
            siblings in proptest::collection::vec(("[a-zA-Z0-9 ]{1,8}", "[a-z0-9-]{0,8}"), 0..5),
        ) {
            prop_assume!(raw.trim() != current);
            let create = TeamName::try_new(&raw)
                .and_then(|name| name.ensure_unique_among(&siblings).map(|()| name));
            prop_assert_eq!(
                classify_team_rename(&raw, &current, &siblings),
                create.map(TeamRenameVerdict::Rename)
            );
        }
    }
}
