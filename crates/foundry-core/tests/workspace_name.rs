//! `foundry_core::WorkspaceName` — the one workspace-name rule
//! (instance-workspace-name-rule, DESIGN DDD-1/2/3/13, ADR-WORKSPACE-NAME-001).
//!
//! SCAFFOLD: true — written at DISTILL (2026-10-05) BEFORE the value object
//! exists. Every test is `#[ignore]`d and runs against the `scaffold` shim below,
//! whose bodies panic, so `cargo test -p foundry-core -- --include-ignored`
//! classifies each one RED (a panic in the code under test), never BROKEN (a
//! missing symbol). DELIVER slice 01:
//!   1. creates `WorkspaceName`, `WorkspaceNameError` and `WORKSPACE_NAME_MAX_CHARS`
//!      in `foundry-core` (lib.rs or a re-exported `workspace_name.rs`);
//!   2. deletes `mod scaffold` and swaps the `use` line for
//!      `use foundry_core::{WorkspaceName, WorkspaceNameError, WORKSPACE_NAME_MAX_CHARS};`;
//!   3. removes the `#[ignore]` attributes.
//!
//! The rule under test (D1, D4, D5, DDD-2): `try_new(raw)` trims (`str::trim`,
//! Unicode White_Space), then refuses Empty, then ControlCharacter (Cc, i.e.
//! `char::is_control`; U+202A-202E; U+2066-2069; U+2028; U+2029), then TooLong
//! (more than 24 Unicode scalars, `chars().count()`); on success it holds the
//! trimmed value. Every other format character (Cf) is allowed. `Display` of each
//! error is the D3 copy, byte for byte.
//!
//! Mutation targets (DoD 6, DDD-13): every boundary below is an exact example
//! pair, because a proptest range need not sample the boundary (the precedent's
//! lesson). The properties cover the space between the pairs.

use proptest::prelude::*;
use scaffold::{WorkspaceName, WorkspaceNameError, WORKSPACE_NAME_MAX_CHARS};

/// SCAFFOLD: true — the target API's shape, panicking. DELIVER deletes this module.
mod scaffold {
    pub const WORKSPACE_NAME_MAX_CHARS: usize = 24;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum WorkspaceNameError {
        Empty,
        ControlCharacter,
        TooLong,
    }

    impl std::fmt::Display for WorkspaceNameError {
        fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("SCAFFOLD: WorkspaceNameError's Display (the D3 copy) is not implemented yet")
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct WorkspaceName(String);

    impl WorkspaceName {
        pub fn try_new(_raw: &str) -> Result<Self, WorkspaceNameError> {
            panic!("SCAFFOLD: WorkspaceName::try_new is not implemented yet (DDD-2)")
        }

        pub fn as_str(&self) -> &str {
            &self.0
        }
    }
}

use WorkspaceNameError::{ControlCharacter, Empty, TooLong};

/// Assert one example: `raw` either parses to exactly `stored` or is refused with `err`.
fn check(raw: &str, expected: Result<&str, WorkspaceNameError>) {
    let got = WorkspaceName::try_new(raw);
    match expected {
        Ok(stored) => {
            let name = got.unwrap_or_else(|e| panic!("{raw:?} must be accepted, got {e:?}"));
            assert_eq!(
                name.as_str(),
                stored,
                "{raw:?} must be stored as {stored:?}"
            );
        }
        Err(err) => assert_eq!(got.map(|n| n.as_str().to_string()), Err(err), "{raw:?}"),
    }
}

fn interior(c: char) -> String {
    format!("Ops{c}Team")
}

// ---------------------------------------------------------------- exact pairs

#[test]
#[ignore = "SCAFFOLD: un-ignored by DELIVER slice 01 (instance-workspace-name-rule)"]
fn the_cap_is_24_scalars() {
    assert_eq!(WORKSPACE_NAME_MAX_CHARS, 24);
    check("Canzan Labs Platform Ops", Ok("Canzan Labs Platform Ops")); // 24 ASCII
    check("Canzan Labs Platform Team", Err(TooLong)); // 25 ASCII
    check("Ångström Øresund Société", Ok("Ångström Øresund Société")); // 24 scalars, 28 bytes
    check("Ångström Øresund Sociétés", Err(TooLong)); // 25 scalars
    check(
        "  Canzan Labs Platform Ops  ",
        Ok("Canzan Labs Platform Ops"),
    ); // padding not counted
    check("a", Ok("a"));
}

#[test]
#[ignore = "SCAFFOLD: un-ignored by DELIVER slice 01 (instance-workspace-name-rule)"]
fn the_c0_and_c1_control_boundaries() {
    check(&interior('\u{1F}'), Err(ControlCharacter));
    check(&interior('\u{20}'), Ok("Ops Team"));
    check(&interior('\u{7E}'), Ok("Ops~Team"));
    check(&interior('\u{7F}'), Err(ControlCharacter));
    check(&interior('\u{9F}'), Err(ControlCharacter));
    check(&interior('\u{A0}'), Ok("Ops\u{A0}Team")); // interior NBSP is allowed
    check(&interior('\0'), Err(ControlCharacter));
    check(&interior('\t'), Err(ControlCharacter));
    check(&interior('\n'), Err(ControlCharacter));
    check(&interior('\r'), Err(ControlCharacter));
    check(&interior('\u{1B}'), Err(ControlCharacter)); // escape
    check(&interior('\u{85}'), Err(ControlCharacter)); // NEL, interior
}

#[test]
#[ignore = "SCAFFOLD: un-ignored by DELIVER slice 01 (instance-workspace-name-rule)"]
fn the_line_and_paragraph_separators() {
    check(&interior('\u{2027}'), Ok("Ops\u{2027}Team"));
    check(&interior('\u{2028}'), Err(ControlCharacter));
    check(&interior('\u{2029}'), Err(ControlCharacter));
    check(&interior('\u{202A}'), Err(ControlCharacter));
}

#[test]
#[ignore = "SCAFFOLD: un-ignored by DELIVER slice 01 (instance-workspace-name-rule)"]
fn the_bidi_embedding_override_and_isolate_boundaries() {
    check(&interior('\u{2029}'), Err(ControlCharacter));
    check(&interior('\u{202A}'), Err(ControlCharacter));
    check(&interior('\u{202E}'), Err(ControlCharacter));
    check(&interior('\u{202F}'), Ok("Ops\u{202F}Team")); // narrow NBSP, interior
    check(&interior('\u{2065}'), Ok("Ops\u{2065}Team"));
    check(&interior('\u{2066}'), Err(ControlCharacter));
    check(&interior('\u{2069}'), Err(ControlCharacter));
    check(&interior('\u{206A}'), Ok("Ops\u{206A}Team")); // Cf, not in D4
    check("Ops\u{202E}gnikcatS", Err(ControlCharacter));
}

#[test]
#[ignore = "SCAFFOLD: un-ignored by DELIVER slice 01 (instance-workspace-name-rule)"]
fn other_format_characters_stay_allowed() {
    for c in [
        '\u{200B}', '\u{200C}', '\u{200D}', '\u{200E}', '\u{200F}', '\u{FEFF}', '\u{AD}',
    ] {
        let raw = interior(c);
        check(&raw, Ok(raw.as_str()));
    }
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} Bailey"; // 12 scalars
    check(family, Ok(family));
    check("Mehr\u{200C}dad", Ok("Mehr\u{200C}dad"));
}

#[test]
#[ignore = "SCAFFOLD: un-ignored by DELIVER slice 01 (instance-workspace-name-rule)"]
fn whitespace_controls_at_the_edges_are_trimmed_not_refused() {
    check("\tKitchen\n", Ok("Kitchen"));
    check("\r\nKitchen\u{0B}\u{0C}", Ok("Kitchen"));
    check("\u{2028}Ops", Ok("Ops"));
    check("Ops\u{2029}", Ok("Ops"));
    check("\u{85}Ops", Ok("Ops"));
    check("\u{A0}Ops\u{A0}", Ok("Ops"));
}

#[test]
#[ignore = "SCAFFOLD: un-ignored by DELIVER slice 01 (instance-workspace-name-rule)"]
fn non_whitespace_controls_at_the_edges_are_refused() {
    check("\u{1}Ops", Err(ControlCharacter));
    check("Ops\u{202E}", Err(ControlCharacter));
    check("\u{2066}Ops", Err(ControlCharacter));
    check("Ops\u{7F}", Err(ControlCharacter));
    check("\0Ops", Err(ControlCharacter));
}

#[test]
#[ignore = "SCAFFOLD: un-ignored by DELIVER slice 01 (instance-workspace-name-rule)"]
fn empty_comes_before_control_and_control_before_length() {
    check("", Err(Empty));
    check("   ", Err(Empty));
    check(" \t\n ", Err(Empty)); // trims to nothing: empty, not control
    let long_with_tab = "Canzan Labs Platform\tEngineering"; // 32 scalars
    assert_eq!(long_with_tab.chars().count(), 32);
    check(long_with_tab, Err(ControlCharacter));
    check("Canzan Labs Platform Engineering", Err(TooLong)); // 32 clean scalars
}

#[test]
#[ignore = "SCAFFOLD: un-ignored by DELIVER slice 01 (instance-workspace-name-rule)"]
fn each_refusal_reads_the_d3_copy_byte_for_byte() {
    assert_eq!(Empty.to_string(), "Workspace name must not be empty");
    assert_eq!(
        ControlCharacter.to_string(),
        "Workspace name must not contain control characters"
    );
    assert_eq!(
        TooLong.to_string(),
        "Workspace name must be at most 24 characters"
    );
}

// ----------------------------------------------------------------- properties

/// One character from the D4 refused set.
fn refused_char() -> impl Strategy<Value = char> {
    prop_oneof![
        0x00u32..=0x1F,
        0x7Fu32..=0x9F,
        0x202Au32..=0x202E,
        0x2066u32..=0x2069,
        Just(0x2028u32),
        Just(0x2029u32),
    ]
    .prop_map(|cp| char::from_u32(cp).expect("a scalar value"))
}

/// A name with no D4 character and no whitespace at either end, 2-24 scalars.
fn clean_name(max: usize) -> impl Strategy<Value = String> {
    let pattern = format!(
        "[a-zA-Z0-9éÅø][a-zA-Z0-9éÅø .'-]{{0,{}}}[a-zA-Z0-9éÅø]",
        max - 2
    );
    proptest::string::string_regex(&pattern).expect("a valid pattern")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    #[ignore = "SCAFFOLD: un-ignored by DELIVER slice 01 (instance-workspace-name-rule)"]
    fn whitespace_only_is_always_empty(raw in "[ \t\n\r\u{0B}\u{0C}\u{85}\u{A0}\u{2028}\u{2029}\u{3000}]{0,12}") {
        prop_assert_eq!(WorkspaceName::try_new(&raw).map(|n| n.as_str().to_string()), Err(Empty));
    }

    #[test]
    #[ignore = "SCAFFOLD: un-ignored by DELIVER slice 01 (instance-workspace-name-rule)"]
    fn a_refused_character_inside_a_clean_name_is_always_refused(
        name in clean_name(24),
        c in refused_char(),
        at in any::<prop::sample::Index>(),
    ) {
        let chars: Vec<char> = name.chars().collect();
        // An interior position: after the first scalar, before the last.
        let i = 1 + at.index(chars.len() - 1);
        let raw: String = chars[..i].iter().chain(std::iter::once(&c)).chain(&chars[i..]).collect();
        prop_assert_eq!(
            WorkspaceName::try_new(&raw).map(|n| n.as_str().to_string()),
            Err(ControlCharacter)
        );
    }

    #[test]
    #[ignore = "SCAFFOLD: un-ignored by DELIVER slice 01 (instance-workspace-name-rule)"]
    fn the_length_gate_sits_at_24_trimmed_scalars(name in clean_name(40), pad in "[ \t]{0,3}") {
        let raw = format!("{pad}{name}{pad}");
        let got = WorkspaceName::try_new(&raw).map(|n| n.as_str().to_string());
        if name.chars().count() <= WORKSPACE_NAME_MAX_CHARS {
            prop_assert_eq!(got, Ok(name));
        } else {
            prop_assert_eq!(got, Err(TooLong));
        }
    }

    #[test]
    #[ignore = "SCAFFOLD: un-ignored by DELIVER slice 01 (instance-workspace-name-rule)"]
    fn an_accepted_name_is_the_trimmed_input(chars in prop::collection::vec(any::<char>(), 0..30)) {
        let raw: String = chars.into_iter().collect();
        if let Ok(name) = WorkspaceName::try_new(&raw) {
            prop_assert_eq!(name.as_str(), raw.trim());
        }
    }
}
