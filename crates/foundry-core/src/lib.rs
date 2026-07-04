//! foundry-core — domain types, no I/O.
//!
//! Slice 1 placeholders. Real aggregates land as later user stories
//! (US-05 workspace/user, US-07 project, US-08 issue) drive them.

#![forbid(unsafe_code)]
#![deny(clippy::all)]

use std::fmt;
use thiserror::Error;
use uuid::Uuid;

pub mod markdown;
pub use markdown::{render_comment_markdown, SanitizedHtml};

/// Marker IDs. Strong-typed wrappers around UUIDs to prevent
/// accidental cross-aggregate ID mix-ups at the type system level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkspaceId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TeamId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProjectId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IssueId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(pub Uuid);

/// Domain error placeholder. Concrete variants land with aggregates.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("validation failed: {0}")]
    Validation(String),
}

// ---------------------------------------------------------------- ProjectKey
//
// Invariant I-P3 (`design/domain-model.md`): project key prefix matches
// `^[A-Z]{2,6}$`. Enforced here as the single domain construction point
// AND as a Postgres CHECK constraint at the data layer (defence in
// depth). Acceptance suite drives this via the US-07 property outline.

/// Reasons a string fails to be a valid `ProjectKey`.
///
/// Variants are flat (no `String` payload) so callers can map them to
/// localized HTML error messages without losing the original input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ProjectKeyError {
    #[error("project key must not be empty")]
    Empty,
    #[error("project key must be 2-6 characters")]
    WrongLength,
    #[error("project key must contain only uppercase A-Z characters")]
    InvalidCharacters,
}

/// Domain value object: the uppercase prefix used to build issue keys
/// (e.g. `AUTH-1`, `AUTH-2`). Constructed only via [`ProjectKey::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectKey(String);

impl ProjectKey {
    /// Try to construct a `ProjectKey` enforcing invariant I-P3.
    ///
    /// Rules (in order):
    /// 1. Empty -> `Empty`.
    /// 2. Length outside 2..=6 (counted as bytes; ASCII-only is checked
    ///    next so byte-length == char-length on success) -> `WrongLength`.
    /// 3. Any character outside `A`..=`Z` -> `InvalidCharacters`.
    pub fn try_new(raw: &str) -> Result<Self, ProjectKeyError> {
        if raw.is_empty() {
            return Err(ProjectKeyError::Empty);
        }
        if raw.len() < 2 || raw.len() > 6 {
            // We check length in bytes first to short-circuit huge inputs;
            // the character validation below catches non-ASCII content
            // even if its byte-length happens to land inside 2..=6.
            return Err(ProjectKeyError::WrongLength);
        }
        if !raw.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(ProjectKeyError::InvalidCharacters);
        }
        Ok(Self(raw.to_string()))
    }

    /// Borrow the underlying string. Used by the persistence layer to
    /// bind the value into a SQL parameter.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProjectKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ------------------------------------------------------------------ IssueKey
//
// Invariant (`design/domain-model.md` Issue aggregate): the user-facing
// issue key is `{ProjectKey}-{number}` where `number >= 1`. The domain
// is the single construction point so handlers, templates, and queries
// share a Display impl rather than duplicating the format string.

/// Reasons a `(ProjectKey, number)` pair fails to be a valid `IssueKey`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum IssueKeyError {
    #[error("issue number must be >= 1")]
    NonPositiveNumber,
}

/// Domain value object: the user-visible identifier for an issue
/// (e.g. `AUTH-1`, `AUTH-42`). Constructed only via [`IssueKey::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IssueKey {
    prefix: String,
    number: u32,
}

impl IssueKey {
    /// Try to construct an `IssueKey` from a validated project key and a
    /// per-project sequential number.
    pub fn try_new(project_key: &ProjectKey, number: u32) -> Result<Self, IssueKeyError> {
        if number == 0 {
            return Err(IssueKeyError::NonPositiveNumber);
        }
        Ok(Self {
            prefix: project_key.as_str().to_string(),
            number,
        })
    }

    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    pub fn number(&self) -> u32 {
        self.number
    }
}

impl fmt::Display for IssueKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.prefix, self.number)
    }
}

#[cfg(test)]
mod issue_key_tests {
    use super::*;

    // Behaviour 1 — accepted (prefix, number) pairs render `{PREFIX}-{N}`
    // across the typical range (boundary, mid, large).
    #[test]
    fn renders_prefix_dash_number_across_range() {
        let key = ProjectKey::try_new("AUTH").expect("valid prefix");
        for (n, expected) in [(1u32, "AUTH-1"), (7, "AUTH-7"), (100, "AUTH-100")] {
            let ik = IssueKey::try_new(&key, n).expect("accepted number");
            assert_eq!(format!("{ik}"), expected);
            assert_eq!(ik.prefix(), "AUTH");
            assert_eq!(ik.number(), n);
        }
    }

    // Behaviour 2 — number 0 is rejected (per-project numbering starts at 1).
    #[test]
    fn rejects_zero_number() {
        let key = ProjectKey::try_new("AUTH").expect("valid prefix");
        assert_eq!(
            IssueKey::try_new(&key, 0),
            Err(IssueKeyError::NonPositiveNumber)
        );
    }

    // Behaviour 3 — Display preserves the project key prefix verbatim
    // (no case mangling) for a different prefix shape.
    #[test]
    fn preserves_prefix_verbatim() {
        let key = ProjectKey::try_new("WEB").expect("valid prefix");
        let ik = IssueKey::try_new(&key, 1).expect("valid");
        assert_eq!(format!("{ik}"), "WEB-1");
    }
}

#[cfg(test)]
mod project_key_tests {
    use super::*;

    // Behaviour 1 — accepted keys produce a value carrying the original
    // text (3 input variations covering min boundary / typical / max boundary).
    #[test]
    fn accepts_uppercase_keys_within_length_range() {
        for raw in ["AU", "AUTH", "AUTHWS"] {
            let key = ProjectKey::try_new(raw).expect("expected accepted key");
            assert_eq!(key.as_str(), raw, "value object preserves input");
        }
    }

    // Behaviour 2 — empty input is rejected with the dedicated variant.
    #[test]
    fn rejects_empty_input() {
        assert_eq!(ProjectKey::try_new(""), Err(ProjectKeyError::Empty));
    }

    // Behaviour 3 — too short / too long input is rejected with WrongLength.
    #[test]
    fn rejects_inputs_outside_length_range() {
        for raw in ["A", "AUTHWORD2", "AUTHWORD"] {
            // Note: "AUTHWORD2" mixes a digit but length (9) exceeds 6
            // so the length check fires first — intentional ordering so
            // the user sees the most actionable error first.
            assert_eq!(
                ProjectKey::try_new(raw),
                Err(ProjectKeyError::WrongLength),
                "raw={raw:?}"
            );
        }
    }

    // Behaviour 4 — lowercase / digits / punctuation within the length
    // window are rejected with InvalidCharacters.
    #[test]
    fn rejects_non_uppercase_ascii_when_length_is_in_range() {
        for raw in ["auth", "Auth", "AU1", "AUTH-X", "AU TH"] {
            assert_eq!(
                ProjectKey::try_new(raw),
                Err(ProjectKeyError::InvalidCharacters),
                "raw={raw:?}"
            );
        }
    }

    // Behaviour 5 — Display renders exactly the stored string (used by
    // template rendering + log lines).
    #[test]
    fn display_renders_the_underlying_string() {
        let key = ProjectKey::try_new("AUTH").expect("valid");
        assert_eq!(format!("{key}"), "AUTH");
    }
}
