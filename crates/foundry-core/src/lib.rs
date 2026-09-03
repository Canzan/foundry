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

// ------------------------------------------------------------------- slugify
//
// instance-admin-project-rename (D2 / ADR-PROJECT-RENAME-001): the SINGLE
// production slug-derivation rule, moved verbatim from
// `foundry-app/src/projects.rs`. The create path and
// `admin_tokens::resolve_scope` call this; `cargo xtask check-arch` fails the
// build if any `fn slugify(` DEFINITION reappears under
// `crates/foundry-app/src` (use is fine; redefinition is the regression class
// behind the render-time re-derivation defect).

/// URL-safe slug derivation — minted ONCE at creation time (and used for the
/// D4 duplicate-name collision check); NEVER re-derived from a stored name at
/// render time (ADR-PROJECT-RENAME-001).
///
/// Rules (kept deliberately simple for slice 1):
/// - lower-case ASCII letters/digits are kept verbatim
/// - whitespace + every other run of non-alphanumeric input collapses
///   to a single hyphen
/// - leading/trailing hyphens are stripped
///
/// Examples:
/// - `"Auth v2"` → `"auth-v2"`
/// - `"  Hello, World!  "` → `"hello-world"`
pub fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_hyphen = true; // suppress leading hyphen
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            for c in ch.to_lowercase() {
                out.push(c);
            }
            last_was_hyphen = false;
        } else if !last_was_hyphen {
            out.push('-');
            last_was_hyphen = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// SCAFFOLD: true — board-lane-overflow-menu DISTILL (ADR-025). DELIVER
/// (slice 03) replaces the body; the SIGNATURE is the DESIGN contract
/// (`adr-board-lane-004-lane-slug-mint.md`, component-boundaries.md §5).
///
/// Lane slug derivation — the LANE sibling of [`slugify`], and NOT
/// interchangeable with it. `lanes.slug` is constrained
/// `CHECK (slug ~ '^[a-z][a-z0-9_]*$')`: **underscores, letter-anchored**.
/// `slugify` emits HYPHENS, so its output is rejected outright by the schema —
/// measured against a real Postgres 16, not assumed:
///
/// ```text
/// ERROR:  new row for relation "lanes" violates check constraint "lanes_slug_check"
/// DETAIL:  Failing row contains (…, in-progress, Hyphen, 99).
/// ```
///
/// Rules:
/// - lower-case ASCII letters/digits kept verbatim
/// - every run of other characters collapses to a single `_`
/// - leading/trailing `_` stripped
/// - a **digit-leading** result takes a `lane_` prefix (the `^[a-z]` anchor).
///   This is normalisation to satisfy a CHECK, not D7 collision-suffixing: the
///   label is preserved verbatim and the slug is never shown to the operator.
/// - **empty** result means "no usable characters" — the caller REFUSES inline
///   (D7); this function never invents a slug from nothing.
///
/// Examples:
/// - `"Staging"` → `"staging"`
/// - `"In Progress"` → `"in_progress"` (byte-equal to the shipped seed)
/// - `"Code Review!!"` → `"code_review"`
/// - `"2024 Review"` → `"lane_2024_review"`
/// - `"..."` → `""` (refused by the caller)
pub fn lane_slug(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + LANE_SLUG_PREFIX.len());
    let mut last_was_sep = true; // suppress a leading separator
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            for c in ch.to_lowercase() {
                out.push(c);
            }
            last_was_sep = false;
        } else if !last_was_sep {
            out.push('_');
            last_was_sep = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    // No usable characters: return empty and let the CALLER refuse (D7). This
    // function never invents a slug from nothing — a generated identity the
    // operator did not ask for is permanent, because slugs are immutable.
    if out.is_empty() {
        return out;
    }
    // The `^[a-z]` anchor. A digit-leading name ("2024 Review") is a perfectly
    // good LABEL; the prefix exists only so its slug satisfies the CHECK. The
    // label is preserved verbatim and the slug is never shown, which is why
    // this is normalisation rather than the D7 collision-suffixing that is
    // forbidden.
    if !out.starts_with(|c: char| c.is_ascii_lowercase()) {
        let mut prefixed = String::with_capacity(LANE_SLUG_PREFIX.len() + out.len());
        prefixed.push_str(LANE_SLUG_PREFIX);
        prefixed.push_str(&out);
        return prefixed;
    }
    out
}

/// Applied only when normalisation leaves a digit-leading slug.
const LANE_SLUG_PREFIX: &str = "lane_";

/// Does this string satisfy `lanes.slug`'s CHECK, `^[a-z][a-z0-9_]*$`? Kept
/// beside the minter so the rule has ONE statement in the codebase, and so the
/// property test below asserts the real constraint rather than a paraphrase.
pub fn is_valid_lane_slug(slug: &str) -> bool {
    let mut chars = slug.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[cfg(test)]
mod lane_slug_tests {
    use super::{is_valid_lane_slug, lane_slug, slugify};
    use proptest::prelude::*;

    #[test]
    fn mints_the_documented_examples() {
        assert_eq!(lane_slug("Staging"), "staging");
        assert_eq!(lane_slug("In Progress"), "in_progress");
        assert_eq!(lane_slug("Code Review!!"), "code_review");
        assert_eq!(lane_slug("2024 Review"), "lane_2024_review");
        assert_eq!(lane_slug("  Archive Box  "), "archive_box");
    }

    /// `is_valid_lane_slug` IS this codebase's statement of the DB CHECK
    /// `^[a-z][a-z0-9_]*$`, so its anchor needs testing directly — not only
    /// through `lane_slug`, which by construction never emits a bad first
    /// character. Mutation testing caught exactly that hole: replacing the
    /// first-character guard with `true` survived every other test here.
    #[test]
    fn validity_requires_a_lowercase_letter_first() {
        assert!(is_valid_lane_slug("staging"));
        assert!(is_valid_lane_slug("a"));
        assert!(is_valid_lane_slug("lane_2024_review"));

        assert!(
            !is_valid_lane_slug("2024_review"),
            "digit-leading is INVALID"
        );
        assert!(
            !is_valid_lane_slug("_leading"),
            "underscore-leading is INVALID"
        );
        assert!(
            !is_valid_lane_slug("Staging"),
            "uppercase-leading is INVALID"
        );
        assert!(!is_valid_lane_slug(""), "empty is INVALID");
    }

    /// The tail alphabet, tested for the same reason: it is the other half of
    /// the CHECK, and nothing else asserts what it excludes.
    #[test]
    fn validity_rejects_characters_the_check_forbids_after_the_first() {
        assert!(
            !is_valid_lane_slug("in-progress"),
            "hyphens are INVALID — this is why slugify cannot mint lane slugs"
        );
        assert!(!is_valid_lane_slug("in progress"), "spaces are INVALID");
        assert!(
            !is_valid_lane_slug("inProgress"),
            "uppercase is INVALID anywhere"
        );
        assert!(
            is_valid_lane_slug("in_progress_2"),
            "digits and underscores are fine after the first char"
        );
    }

    #[test]
    fn no_usable_characters_yields_empty_for_the_caller_to_refuse() {
        assert_eq!(lane_slug("..."), "");
        assert_eq!(lane_slug("!!!"), "");
        assert_eq!(lane_slug("   "), "");
        assert_eq!(lane_slug(""), "");
    }

    /// The reason this function exists at all. `slugify` is the PROJECT slug
    /// minter and emits hyphens, which `lanes_slug_check` rejects outright —
    /// verified against a real Postgres during DESIGN. If someone ever "tidies
    /// up" by pointing lane creation at `slugify`, this test says why not.
    #[test]
    fn is_not_interchangeable_with_slugify() {
        assert_eq!(slugify("In Progress"), "in-progress");
        assert_eq!(lane_slug("In Progress"), "in_progress");
        assert!(!is_valid_lane_slug(&slugify("In Progress")));
        assert!(is_valid_lane_slug(&lane_slug("In Progress")));
    }

    #[test]
    fn the_seeded_lane_slugs_round_trip_from_their_shipped_labels() {
        // The 0015 seed pairs. If minting ever drifted from them, an inserted
        // "In-Progress" would collide-or-not against the grandfathered lane
        // unpredictably.
        assert_eq!(lane_slug("Backlog"), "backlog");
        assert_eq!(lane_slug("Todo"), "todo");
        assert_eq!(lane_slug("In-Progress"), "in_progress");
        assert_eq!(lane_slug("Done"), "done");
        assert_eq!(lane_slug("Cancelled"), "cancelled");
    }

    proptest! {
        /// Output is always storable: either empty (the caller refuses) or a
        /// value the DB CHECK accepts. There is no third outcome, which is what
        /// makes "mint then insert" safe without a second validation.
        #[test]
        fn output_is_empty_or_a_valid_lane_slug(input in ".{0,80}") {
            let out = lane_slug(&input);
            prop_assert!(out.is_empty() || is_valid_lane_slug(&out));
        }

        /// A fixed point — mirroring `slugify_is_a_fixed_point`. Re-minting a
        /// slug must not mutate it, or a lane's identity could drift if any
        /// path ever minted twice.
        #[test]
        fn lane_slug_is_a_fixed_point(input in ".{0,80}") {
            let once = lane_slug(&input);
            prop_assume!(!once.is_empty());
            prop_assert_eq!(lane_slug(&once), once);
        }

        /// Never underscore-leading, never underscore-trailing, never doubled —
        /// the separator rules, stated as a property rather than by example.
        #[test]
        fn separators_are_single_and_interior(input in ".{0,80}") {
            let out = lane_slug(&input);
            prop_assume!(!out.is_empty());
            prop_assert!(!out.ends_with('_'));
            prop_assert!(!out.contains("__"));
        }
    }
}

#[cfg(test)]
mod slugify_tests {
    use super::slugify;
    use proptest::prelude::*;

    /// Migrated verbatim from `foundry-app/src/projects.rs::slug_tests`
    /// (02-01 / ADR-PROJECT-RENAME-001) — the example pins the exact rule
    /// the create path has always minted slugs with.
    #[test]
    fn slugifies_common_project_names() {
        assert_eq!(slugify("Auth v2"), "auth-v2");
        assert_eq!(slugify("  Hello, World!  "), "hello-world");
        assert_eq!(slugify("Sandbox"), "sandbox");
        assert_eq!(slugify(""), "");
    }

    proptest! {
        // Property (a): slugify is a fixed point — re-slugifying an already
        // minted slug never changes it. This is the invariant the whole D2
        // fix rests on: a stored slug fed back through the derivation rule
        // is stable, so "minted once at creation" is well-defined.
        #[test]
        fn slugify_is_a_fixed_point(input in ".{0,64}") {
            let once = slugify(&input);
            prop_assert_eq!(slugify(&once), once);
        }

        // Property (b): URL-safe charset — output contains ONLY lowercase
        // ASCII alphanumerics and single hyphens, with no leading/trailing
        // hyphen and no hyphen runs. Exactly what `projects.slug` (URL
        // identity, `UNIQUE (team_id, slug)`) may contain.
        #[test]
        fn slugify_output_is_url_safe(input in ".{0,64}") {
            let slug = slugify(&input);
            prop_assert!(
                slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "non-URL-safe char in {slug:?}"
            );
            prop_assert!(!slug.starts_with('-'), "leading hyphen in {slug:?}");
            prop_assert!(!slug.ends_with('-'), "trailing hyphen in {slug:?}");
            prop_assert!(!slug.contains("--"), "hyphen run in {slug:?}");
        }
    }
}
