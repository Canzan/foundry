//! `ProjectName` — the ONE project-name rule (project-name-rule D2/D3/D4/D5/D6,
//! DESIGN DDD-1/2/3/4, ADR-PROJECT-NAME-001).
//!
//! Every path that accepts a project name constructs one of these. The
//! refused-character set is shared with `WorkspaceName` through
//! [`crate::name_chars`]; there is no second statement of it.

use std::fmt;
use thiserror::Error;

use crate::name_chars::is_refused_name_char;
use crate::{slugify, ProjectKey};

/// The longest project name accepted, counted in Unicode scalar values of the
/// trimmed input (`chars().count()`), never in bytes.
pub const PROJECT_NAME_MAX_CHARS: usize = 256;

/// Why a name is not acceptable. Flat and `Copy`: the `Display` of each variant
/// IS the operator-facing copy (D4), byte for byte. [`ProjectName::try_new`]
/// never yields `NotUnique`; only [`ProjectName::ensure_unique_among`] does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ProjectNameError {
    #[error("Project name must not be empty")]
    Empty,
    #[error("Project name must not contain control characters")]
    ControlCharacter,
    #[error("Project name must be at most {PROJECT_NAME_MAX_CHARS} characters")]
    TooLong,
    #[error("Project name must be unique within the team")]
    NotUnique,
}

/// Domain value object: a trimmed, non-empty project name of at most
/// [`PROJECT_NAME_MAX_CHARS`] scalars containing no refused character.
/// Constructed only via [`ProjectName::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectName(String);

impl ProjectName {
    /// Trim (`str::trim`), then refuse in order (D5/DDD-2): `Empty`,
    /// `ControlCharacter`, `TooLong`. On success holds the trimmed value.
    pub fn try_new(raw: &str) -> Result<Self, ProjectNameError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ProjectNameError::Empty);
        }
        if trimmed.chars().any(is_refused_name_char) {
            return Err(ProjectNameError::ControlCharacter);
        }
        if trimmed.chars().count() > PROJECT_NAME_MAX_CHARS {
            return Err(ProjectNameError::TooLong);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The address this name derives: [`slugify`] of it. May be empty.
    pub fn derived_slug(&self) -> String {
        slugify(&self.0)
    }

    /// The one sibling uniqueness check (D6, DDD-3). `siblings` are the team's
    /// other projects as `(name, stored slug)`. Refuses `NotUnique` when a
    /// sibling's name equals this one under `to_lowercase`, or when this name's
    /// derived address equals a sibling's stored address. An empty derived
    /// address never matches on the address arm (D6): a name without an address
    /// of its own is not a duplicate of a legacy project stored with none.
    ///
    /// Self-exclusion is the caller's job: pass the siblings from
    /// `list_team_sibling_projects` with the project's own id excluded.
    pub fn ensure_unique_among(
        &self,
        siblings: &[(String, String)],
    ) -> Result<(), ProjectNameError> {
        let lowered = self.0.to_lowercase();
        let derived_slug = self.derived_slug();
        let has_address = !derived_slug.is_empty();
        let collides = siblings.iter().any(|(name, slug)| {
            name.to_lowercase() == lowered || (has_address && *slug == derived_slug)
        });
        if collides {
            Err(ProjectNameError::NotUnique)
        } else {
            Ok(())
        }
    }
}

impl fmt::Display for ProjectName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The address a create stores for a new project, tagged with how it was minted
/// (DDD-9). `Derived` is `slugify` of the name — today's address, the one every
/// Latin-lettered name keeps. `KeyFallback` is the key-prefix address a name
/// without one of its own takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MintedSlug {
    Derived(String),
    KeyFallback(String),
}

impl MintedSlug {
    /// The address itself, whichever way it was minted.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Derived(slug) | Self::KeyFallback(slug) => slug,
        }
    }
}

/// The one create-time address mint (D15, DDD-9, ADR-PROJECT-NAME-002). Total:
/// never refuses, never returns `""`.
///
/// `Derived(name.derived_slug())` when that is non-empty, used verbatim (the
/// sibling check owns derived collisions). Otherwise `KeyFallback` of the first
/// of `k`, `k-2`, `k-3`, … (`k` = the key lower-cased) not among `team_slugs`,
/// the team's stored addresses. The suffix starts at 2, never 1, and the lowest
/// free candidate wins, not the one after the highest.
pub fn mint_project_slug(
    name: &ProjectName,
    key: &ProjectKey,
    team_slugs: &[String],
) -> MintedSlug {
    let derived = name.derived_slug();
    if !derived.is_empty() {
        return MintedSlug::Derived(derived);
    }
    let base = key.as_str().to_ascii_lowercase();
    let is_free = |candidate: &String| !team_slugs.contains(candidate);
    let fallback = std::iter::once(base.clone())
        .chain((2u32..).map(|n| format!("{base}-{n}")))
        .find(is_free)
        .expect("a finite team leaves some candidate free");
    MintedSlug::KeyFallback(fallback)
}
