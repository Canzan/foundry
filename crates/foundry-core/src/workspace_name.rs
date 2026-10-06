//! `WorkspaceName` — the ONE workspace-name rule (instance-workspace-name-rule,
//! D1/D3/D4/D5, DESIGN DDD-1/2/3, ADR-WORKSPACE-NAME-001).
//!
//! Every path that accepts a workspace name (onboarding, the instance-admin
//! rename, the CLI) constructs one of these; there is no second statement of
//! the rule anywhere in the codebase.

use std::fmt;
use thiserror::Error;

/// The longest workspace name accepted, counted in Unicode scalar values of the
/// trimmed input (`chars().count()`), never in bytes.
pub const WORKSPACE_NAME_MAX_CHARS: usize = 24;

/// Why a raw string is not a workspace name. Flat and `Copy`: the `Display` of
/// each variant IS the operator-facing copy (D3), byte for byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum WorkspaceNameError {
    #[error("Workspace name must not be empty")]
    Empty,
    #[error("Workspace name must not contain control characters")]
    ControlCharacter,
    #[error("Workspace name must be at most {WORKSPACE_NAME_MAX_CHARS} characters")]
    TooLong,
}

/// Domain value object: a trimmed, non-empty workspace name of at most
/// [`WORKSPACE_NAME_MAX_CHARS`] scalars containing no refused character (D4).
/// Constructed only via [`WorkspaceName::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkspaceName(String);

impl WorkspaceName {
    /// Trim (Unicode White_Space), then refuse in order (D5/DDD-2):
    /// `Empty`, `ControlCharacter`, `TooLong`. On success holds the trimmed value.
    pub fn try_new(raw: &str) -> Result<Self, WorkspaceNameError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(WorkspaceNameError::Empty);
        }
        if trimmed.chars().any(is_refused) {
            return Err(WorkspaceNameError::ControlCharacter);
        }
        if trimmed.chars().count() > WORKSPACE_NAME_MAX_CHARS {
            return Err(WorkspaceNameError::TooLong);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkspaceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The D4 refused set: Cc (`char::is_control`), the bidi embeddings and
/// overrides (U+202A-202E), the bidi isolates (U+2066-2069), and the line and
/// paragraph separators (U+2028, U+2029). Every other format character (ZWJ,
/// ZWNJ, ZWSP, LRM, RLM, soft hyphen, U+FEFF, ...) is allowed.
fn is_refused(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{2028}' | '\u{2029}'
        )
}
