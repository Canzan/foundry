//! `foundry_core::ProjectName` and `foundry_core::mint_project_slug` — the one
//! project-name rule and the one create-time address mint (project-name-rule,
//! DESIGN DDD-1/2/3/4/9/14, ADR-PROJECT-NAME-001/002).
//!
//! SCAFFOLD: partial — DISTILL 2026-10-06; DELIVER 01-01 landed the rule, copy,
//! uniqueness check and derived address in `foundry-core` and un-ignored slice 01.
//! The mint still runs against the `mod scaffold` shim below, whose body panics.
//! DELIVER: create the mint in `foundry-core` (slice 04), delete `mod scaffold`,
//! replace `use scaffold::…` with `use foundry_core::…`, and remove the remaining
//! ignores slice by slice.
//!
//! The rule under test (D2, D3, D5, DDD-2): `try_new(raw)` trims (`str::trim`),
//! then refuses Empty, then ControlCharacter (exactly `WorkspaceName`'s set,
//! through ONE shared predicate, DDD-1), then TooLong (more than 256 Unicode
//! scalars). The sibling check (D6, DDD-3) refuses NotUnique when a sibling's
//! name is equal under `to_lowercase`, or when the name's derived address
//! (`slugify` of it) is NON-EMPTY and equal to a sibling's stored address.
//! `Display` of each error is the D4 copy, byte for byte. The mint (D15, DDD-9)
//! returns `Derived(slugify(name))` when that is non-empty, else
//! `KeyFallback(k)` with `k` the first of `key.lower()`, `key.lower()-2`,
//! `key.lower()-3`, … not among the team's stored addresses.
//!
//! Mutation targets (DoD 6): every boundary is an exact example pair, because a
//! proptest range need not sample the boundary. The properties cover the space
//! between the pairs. The names the crafter gives the uniqueness method and the
//! derived-address accessor are the crafter's; the behaviour is the contract.

use proptest::prelude::*;

use foundry_core::{
    ProjectKey, ProjectName, ProjectNameError, WorkspaceName, WorkspaceNameError,
    PROJECT_NAME_MAX_CHARS,
};
use scaffold::{mint_project_slug, MintedSlug};

use ProjectNameError::{ControlCharacter, Empty, NotUnique, TooLong};

const SCAFFOLD_MINT: &str =
    "SCAFFOLD: foundry_core::mint_project_slug is not implemented yet (DDD-9)";

/// The remaining target API (slice 04 mint), as DESIGN fixed it. DELIVER 04-01
/// deletes this module.
mod scaffold {
    #![allow(dead_code)]
    use super::SCAFFOLD_MINT;
    use foundry_core::{ProjectKey, ProjectName};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum MintedSlug {
        Derived(String),
        KeyFallback(String),
    }

    impl MintedSlug {
        pub fn as_str(&self) -> &str {
            match self {
                Self::Derived(s) | Self::KeyFallback(s) => s,
            }
        }
    }

    pub fn mint_project_slug(
        _name: &ProjectName,
        _key: &ProjectKey,
        _team_slugs: &[String],
    ) -> MintedSlug {
        panic!("{SCAFFOLD_MINT}")
    }
}

/// Assert one example: `raw` either parses to exactly `stored` or is refused with `err`.
fn check(raw: &str, expected: Result<&str, ProjectNameError>) {
    let got = ProjectName::try_new(raw);
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
    format!("Ops{c}Board")
}

fn name(raw: &str) -> ProjectName {
    ProjectName::try_new(raw).unwrap_or_else(|e| panic!("{raw:?} must be a valid name: {e:?}"))
}

fn key(raw: &str) -> ProjectKey {
    ProjectKey::try_new(raw).expect("valid key prefix")
}

fn siblings(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(n, s)| ((*n).to_string(), (*s).to_string()))
        .collect()
}

fn taken(slugs: &[&str]) -> Vec<String> {
    slugs.iter().map(|s| (*s).to_string()).collect()
}

// ---------------------------------------------------------------- the rule: exact pairs

#[test]
fn the_cap_is_256_scalars() {
    assert_eq!(PROJECT_NAME_MAX_CHARS, 256);
    let a256 = "a".repeat(256);
    check(&a256, Ok(&a256));
    check(&"a".repeat(257), Err(TooLong));
    let cjk256 = "日".repeat(256); // 768 bytes: the count is in scalars, not bytes
    check(&cjk256, Ok(&cjk256));
    check(&"日".repeat(257), Err(TooLong));
    check(&format!(" {a256} "), Ok(&a256)); // padding not counted
    check(&format!("\t{a256}\n"), Ok(&a256));
    check("a", Ok("a"));
}

#[test]
fn the_c0_and_c1_control_boundaries() {
    check(&interior('\u{1F}'), Err(ControlCharacter));
    check(&interior('\u{20}'), Ok("Ops Board"));
    check(&interior('\u{7E}'), Ok("Ops~Board"));
    check(&interior('\u{7F}'), Err(ControlCharacter));
    check(&interior('\u{9F}'), Err(ControlCharacter));
    check(&interior('\u{A0}'), Ok("Ops\u{A0}Board"));
    check(&interior('\0'), Err(ControlCharacter));
    check(&interior('\t'), Err(ControlCharacter));
    check(&interior('\n'), Err(ControlCharacter));
}

#[test]
fn the_line_and_paragraph_separators() {
    check(&interior('\u{2027}'), Ok("Ops\u{2027}Board"));
    check(&interior('\u{2028}'), Err(ControlCharacter));
    check(&interior('\u{2029}'), Err(ControlCharacter));
}

#[test]
fn the_bidi_embedding_override_and_isolate_boundaries() {
    check(&interior('\u{202A}'), Err(ControlCharacter));
    check(&interior('\u{202E}'), Err(ControlCharacter));
    check(&interior('\u{202F}'), Ok("Ops\u{202F}Board"));
    check(&interior('\u{2065}'), Ok("Ops\u{2065}Board"));
    check(&interior('\u{2066}'), Err(ControlCharacter));
    check(&interior('\u{2069}'), Err(ControlCharacter));
    check(&interior('\u{206A}'), Ok("Ops\u{206A}Board"));
}

#[test]
fn other_format_characters_stay_allowed() {
    for c in ['\u{200B}', '\u{200C}', '\u{200D}', '\u{FEFF}', '\u{00AD}'] {
        let raw = interior(c);
        check(&raw, Ok(&raw));
    }
    check(
        "Café Roadmap 👨\u{200D}👩\u{200D}👧",
        Ok("Café Roadmap 👨\u{200D}👩\u{200D}👧"),
    );
}

#[test]
fn whitespace_at_the_edges_is_trimmed_and_other_edge_controls_are_refused() {
    check("\tSandbox Experiments\n", Ok("Sandbox Experiments"));
    check("\u{1}Sandbox", Err(ControlCharacter));
    check("Sandbox\u{202E}", Err(ControlCharacter));
}

#[test]
fn empty_comes_before_control_and_control_before_length() {
    check("", Err(Empty));
    check("   ", Err(Empty));
    check(" \t\n ", Err(Empty));
    let long_with_tab = format!("{}\t{}", "a".repeat(150), "b".repeat(150));
    check(&long_with_tab, Err(ControlCharacter));
    check(&"a".repeat(300), Err(TooLong));
}

#[test]
fn each_refusal_reads_the_d4_copy_byte_for_byte() {
    assert_eq!(Empty.to_string(), "Project name must not be empty");
    assert_eq!(
        ControlCharacter.to_string(),
        "Project name must not contain control characters"
    );
    assert_eq!(
        TooLong.to_string(),
        "Project name must be at most 256 characters"
    );
    assert_eq!(
        NotUnique.to_string(),
        "Project name must be unique within the team"
    );
}

#[test]
fn the_seeded_sandbox_project_name_passes_the_rule() {
    assert_eq!(name("Sandbox").as_str(), "Sandbox");
}

#[test]
#[ignore = "SCAFFOLD: slice 04 (D13, DDD-14 mint half)"]
fn the_seeded_sandbox_project_mints_its_shipped_address() {
    assert_eq!(
        mint_project_slug(&name("Sandbox"), &key("GEN"), &[]),
        MintedSlug::Derived("sandbox".to_string()),
        "Store::seed_initial_workspace's constant address must equal the mint"
    );
}

// ---------------------------------------------------------------- uniqueness (D6, DDD-3)

#[test]
fn a_case_insensitive_name_match_alone_is_not_unique() {
    // The renamed sibling keeps its old address: only the name arm can see it.
    let team = siblings(&[("Identity Platform", "auth-v2"), ("Sandbox", "sandbox")]);
    assert_eq!(
        name("identity platform").ensure_unique_among(&team),
        Err(NotUnique)
    );
    assert_eq!(
        name("IDENTITY PLATFORM").ensure_unique_among(&team),
        Err(NotUnique)
    );
}

#[test]
fn a_derived_address_match_alone_is_not_unique() {
    let team = siblings(&[("Identity Platform", "auth-v2")]);
    assert_eq!(name("Auth V2!").ensure_unique_among(&team), Err(NotUnique));
    // An allowed invisible separator still derives the same address (D7).
    assert_eq!(
        name("Auth\u{200B}v2").ensure_unique_among(&team),
        Err(NotUnique)
    );
}

#[test]
fn an_empty_derived_address_is_never_an_address_match() {
    let legacy = siblings(&[("Ωμέγα", ""), ("Identity Platform", "auth-v2")]);
    assert_eq!(name("🚀").ensure_unique_among(&legacy), Ok(()));
    assert_eq!(name("日本語ボード").ensure_unique_among(&legacy), Ok(()));
    // The skip sits on the EMPTY case only: a non-empty match is still refused.
    assert_eq!(
        name("Auth V2!").ensure_unique_among(&legacy),
        Err(NotUnique)
    );
    // And the name arm still applies to names without an address.
    assert_eq!(name("ωμέγα").ensure_unique_among(&legacy), Err(NotUnique));
}

#[test]
fn distinct_names_and_addresses_are_unique_and_no_siblings_is_unique() {
    let team = siblings(&[("Identity Platform", "auth-v2"), ("Sandbox", "sandbox")]);
    assert_eq!(name("Homelab Ops").ensure_unique_among(&team), Ok(()));
    assert_eq!(
        name("Identity Platforms").ensure_unique_among(&team),
        Ok(())
    );
    assert_eq!(name("Sandbox").ensure_unique_among(&[]), Ok(()));
}

#[test]
fn the_derived_address_is_slugify_of_the_name() {
    assert_eq!(name("Ωmega 2").derived_slug(), "mega-2");
    assert_eq!(name("Homelab Ops").derived_slug(), "homelab-ops");
    assert_eq!(name("日本語ボード").derived_slug(), "");
    assert_eq!(name("🚀").derived_slug(), "");
}

// ---------------------------------------------------------------- the mint (D15, DDD-9)

#[test]
#[ignore = "SCAFFOLD: slice 04 (DDD-9)"]
fn a_name_with_its_own_address_keeps_it() {
    assert_eq!(
        mint_project_slug(&name("Ωmega 2"), &key("OMG"), &taken(&["mega-2"])),
        MintedSlug::Derived("mega-2".to_string()),
        "a derived address is used verbatim (the sibling check owns its collisions)"
    );
    assert_eq!(
        mint_project_slug(&name("Homelab Ops"), &key("OPS"), &[]),
        MintedSlug::Derived("homelab-ops".to_string())
    );
}

#[test]
#[ignore = "SCAFFOLD: slice 04 (DDD-9)"]
fn a_name_without_an_address_takes_the_key_prefix_in_lower_case() {
    let jp = name("日本語ボード");
    let k = key("JP");
    assert_eq!(
        mint_project_slug(&jp, &k, &[]),
        MintedSlug::KeyFallback("jp".into())
    );
    assert_eq!(
        mint_project_slug(&jp, &k, &taken(&["jp"])),
        MintedSlug::KeyFallback("jp-2".into())
    );
    assert_eq!(
        mint_project_slug(&jp, &k, &taken(&["jp", "jp-2"])),
        MintedSlug::KeyFallback("jp-3".into())
    );
    assert_eq!(
        mint_project_slug(&jp, &k, &taken(&["jp-2"])),
        MintedSlug::KeyFallback("jp".into()),
        "the lowest free address, not the one after the highest"
    );
    assert_eq!(
        mint_project_slug(&jp, &k, &taken(&["jp", "jp-1"])),
        MintedSlug::KeyFallback("jp-2".into()),
        "the suffix starts at 2, never 1"
    );
    assert_eq!(
        mint_project_slug(&jp, &k, &taken(&["jp", "jp-3"])),
        MintedSlug::KeyFallback("jp-2".into())
    );
}

#[test]
#[ignore = "SCAFFOLD: slice 04 (DDD-9)"]
fn a_six_letter_key_and_a_legacy_empty_address_are_handled() {
    assert_eq!(
        mint_project_slug(&name("🚀"), &key("AUTHWS"), &[]),
        MintedSlug::KeyFallback("authws".into())
    );
    assert_eq!(
        mint_project_slug(&name("🚀"), &key("RKT"), &taken(&["", "sandbox"])),
        MintedSlug::KeyFallback("rkt".into()),
        "a legacy empty address neither blocks nor is minted"
    );
    assert_eq!(
        mint_project_slug(&name("🛠"), &key("OPS"), &taken(&["ops", "auth-v2"])),
        MintedSlug::KeyFallback("ops-2".into())
    );
}

// ---------------------------------------------------------------- properties

fn clean_name(max: usize) -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop::char::any().prop_filter("not a D3 character and not white space", |c| {
            !c.is_control()
                && !c.is_whitespace()
                && !matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
        }),
        1..=max,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

fn slug_token() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(String::new()),
        "[a-z]{2,6}",
        "[a-z]{2,6}-[1-9]",
        "[a-z]{2,6}-[1-9][0-9]",
    ]
}

proptest! {
    /// DDD-1: one predicate. For every scalar, the workspace rule and the project
    /// rule agree on whether it is a control character (at an interior index).
    #[test]
    fn the_project_and_workspace_rules_refuse_exactly_the_same_characters(c in any::<char>()) {
        let raw = format!("a{c}b");
        let workspace = WorkspaceName::try_new(&raw)
            .err()
            .is_some_and(|e| e == WorkspaceNameError::ControlCharacter);
        let project = ProjectName::try_new(&raw)
            .err()
            .is_some_and(|e| e == ControlCharacter);
        prop_assert_eq!(workspace, project, "{:?} (U+{:04X})", c, c as u32);
    }

    /// DDD-4: `try_new` never answers NotUnique — only the sibling check does.
    #[test]
    fn the_rule_alone_never_says_not_unique(chars in prop::collection::vec(any::<char>(), 0..300)) {
        let raw: String = chars.into_iter().collect();
        prop_assert_ne!(ProjectName::try_new(&raw).err(), Some(NotUnique));
    }

    /// D2: an accepted name is the trimmed input, unchanged.
    #[test]
    fn an_accepted_name_is_the_trimmed_input(chars in prop::collection::vec(any::<char>(), 0..300)) {
        let raw: String = chars.into_iter().collect();
        if let Ok(n) = ProjectName::try_new(&raw) {
            prop_assert_eq!(n.as_str(), raw.trim());
        }
    }

    /// The length gate sits at 256 trimmed scalars for any clean name.
    #[test]
    fn the_length_gate_sits_at_256_trimmed_scalars(n in clean_name(300), pad in "[ \t]{0,3}") {
        let raw = format!("{pad}{n}{pad}");
        let got = ProjectName::try_new(&raw);
        if n.chars().count() <= PROJECT_NAME_MAX_CHARS {
            prop_assert!(got.is_ok(), "{} scalars must be accepted: {:?}", n.chars().count(), got);
        } else {
            prop_assert_eq!(got.err(), Some(TooLong));
        }
    }

    /// D15: a fallback address is never empty, never already taken, never `-1`,
    /// and is the LOWEST free candidate (every lower candidate is taken).
    #[test]
    #[ignore = "SCAFFOLD: slice 04 (DDD-9)"]
    fn a_fallback_address_is_the_lowest_free_candidate(
        k in "[A-Z]{2,6}",
        taken_slugs in prop::collection::vec(slug_token(), 0..8),
        suffixes in prop::collection::btree_set(2u32..8, 0..6),
        base_taken in any::<bool>(),
    ) {
        let lower = k.to_lowercase();
        let mut team: Vec<String> = taken_slugs;
        if base_taken {
            team.push(lower.clone());
        }
        team.extend(suffixes.iter().map(|n| format!("{lower}-{n}")));
        let minted = mint_project_slug(&name("日本語ボード"), &key(&k), &team);
        let MintedSlug::KeyFallback(slug) = minted else {
            panic!("a name without an address must take the fallback");
        };
        prop_assert!(!slug.is_empty());
        prop_assert!(!team.contains(&slug), "{} is already taken", slug);
        prop_assert_ne!(slug.clone(), format!("{lower}-1"));
        let mut candidates = std::iter::once(lower.clone())
            .chain((2..).map(|n| format!("{lower}-{n}")));
        let lowest_free = candidates.find(|c| !team.contains(c)).expect("some free");
        prop_assert_eq!(slug, lowest_free);
    }
}
