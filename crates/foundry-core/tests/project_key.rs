//! The project key rule on the pure core: which raw keys are accepted (L1),
//! that the widening takes nothing away (L2), and how an issue key reads (L4).
//!
//! The expected verdict is restated here from the rule itself — count the
//! characters, classify each one — never read back from the construction point
//! under test. A failure prints the seed that replays it.

use foundry_core::{IssueKey, ProjectKey};
use proptest::prelude::*;

/// The rule, restated: 2 to 10 characters, every one a capital letter A to Z.
fn the_rule_accepts(raw: &str) -> bool {
    let characters = raw.chars().count();
    (2..=10).contains(&characters) && raw.chars().all(|c| c.is_ascii_uppercase())
}

/// The earlier rule, restated: 2 to 6 capital letters A to Z.
fn the_earlier_rule_accepted(raw: &str) -> bool {
    let characters = raw.chars().count();
    (2..=6).contains(&characters) && raw.chars().all(|c| c.is_ascii_uppercase())
}

/// What an operator can type or paste into the key field: capitals, lower-case
/// letters, digits, punctuation, a space and characters outside ASCII. Keys made
/// only of capitals are drawn by construction, at every length from none to
/// fourteen, so neither side of the rule is left to chance.
fn a_typed_key() -> impl Strategy<Value = String> {
    let mixed_characters = prop_oneof![
        30 => prop::char::range('A', 'Z'),
        10 => prop::char::range('a', 'z'),
        6 => prop::char::range('0', '9'),
        4 => prop::sample::select(vec!['-', '_', '.', ' ', '#', '/', '+']),
        4 => prop::sample::select(vec!['é', 'Ä', 'ß', '日', '語', 'ボ', '🔑']),
    ];
    prop_oneof![
        1 => prop::collection::vec(prop::char::range('A', 'Z'), 0..=14)
            .prop_map(|cs| cs.into_iter().collect::<String>()),
        1 => prop::collection::vec(mixed_characters, 0..=14)
            .prop_map(|cs| cs.into_iter().collect::<String>()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// L1: a raw key is accepted exactly when it is 2 to 10 characters long and
    /// every character is a capital letter A to Z, and an accepted key keeps
    /// the text that was typed.
    #[test]
    fn a_key_is_accepted_exactly_when_it_is_two_to_ten_capital_letters(raw in a_typed_key()) {
        let verdict = ProjectKey::try_new(&raw);
        if the_rule_accepts(&raw) {
            let accepted = verdict.map_err(|e| e.to_string());
            prop_assert!(
                accepted.is_ok(),
                "WHAT: the key {:?} of {} capital letters was refused with {:?}. \
                 WHY: a key of 2 to 10 capital letters A to Z is one an operator may use. \
                 HOW: accept it.",
                raw, raw.chars().count(), accepted.as_ref().err()
            );
            let accepted = accepted.expect("an accepted key");
            prop_assert_eq!(
                accepted.as_str(), raw.as_str(),
                "WHAT: the accepted key reads differently from the text that was typed. \
                 WHY: a key the operator typed must be stored and shown as she typed it. \
                 HOW: keep the text unchanged."
            );
        } else {
            prop_assert!(
                verdict.is_err(),
                "WHAT: the key {:?} was accepted although it is not 2 to 10 capital letters \
                 A to Z. WHY: it would reach the stored rule and every issue key of that \
                 project. HOW: refuse it.",
                raw
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    /// L2: every key the earlier 2 to 6 rule accepted is still accepted, so the
    /// widening takes nothing away.
    #[test]
    fn every_key_the_earlier_rule_accepted_is_still_accepted(raw in "[A-Z]{2,6}") {
        prop_assert!(
            the_earlier_rule_accepted(&raw),
            "the earlier rule is restated wrongly for {:?}", raw
        );
        prop_assert!(
            ProjectKey::try_new(&raw).is_ok(),
            "WHAT: {:?} was accepted under the earlier rule and is refused now. \
             WHY: an operator who could use a key before must still be able to use it. \
             HOW: keep every shorter key acceptable.",
            raw
        );
    }

    /// L4: an issue key reads as the project key verbatim, a hyphen, then the
    /// issue number. The expected text is assembled here from the key and the
    /// number, not from the rendering under test.
    #[test]
    fn an_issue_key_reads_as_the_project_key_then_the_number(
        raw in "[A-Z]{2,10}",
        number in 1u32..=1000,
    ) {
        let key = ProjectKey::try_new(&raw).map_err(|e| e.to_string());
        prop_assert!(
            key.is_ok(),
            "WHAT: the key {:?} cannot be built, so no issue key can carry it ({:?}). \
             WHY: an issue of a project keyed with 2 to 10 capital letters must carry that \
             key. HOW: accept the key.",
            raw, key.as_ref().err()
        );
        let key = key.expect("an accepted key");
        let issue = IssueKey::try_new(&key, number).expect("an issue number of at least one");
        prop_assert_eq!(
            issue.to_string(), format!("{raw}-{number}"),
            "WHAT: the issue key does not read as the project key, a hyphen and the number. \
             WHY: a truncated, padded or re-lettered prefix gives the operator a key that \
             points at nothing. HOW: render the key verbatim."
        );
    }
}

/// The keys the rule is owed an answer on, each with the answer the rule itself
/// gives: the two ends of the range, the key just past the long end, a single
/// letter, nothing at all, lower case, a hyphen, a digit, and four characters
/// outside ASCII that take more than ten bytes.
#[test]
fn the_named_keys_get_the_answer_the_rule_gives() {
    let owed = [
        ("AU", true),
        ("AUTHWS", true),
        ("PLATFORMOP", true),
        ("PLATFORMOPS", false),
        ("A", false),
        ("", false),
        ("auth", false),
        ("AUTH-X", false),
        ("AU1", false),
        ("日本語語", false),
    ];
    for (raw, accepted) in owed {
        assert_eq!(
            the_rule_accepts(raw),
            accepted,
            "the rule restated here disagrees with the owed answer for {raw:?}"
        );
        assert_eq!(
            ProjectKey::try_new(raw).is_ok(),
            accepted,
            "WHAT: {raw:?} is {got} where the rule says it is {want}. \
             WHY: these are the keys the rule is owed an answer on, the ends of the range \
             and one of each refused class. HOW: answer {raw:?} as the rule does.",
            got = if accepted { "refused" } else { "accepted" },
            want = if accepted { "accepted" } else { "refused" },
        );
    }
}

/// L4, the named case: the first issue of a project keyed PLATFORMOP.
#[test]
fn the_first_issue_of_a_ten_letter_key_project_reads_in_full() {
    let key = ProjectKey::try_new("PLATFORMOP");
    assert!(
        key.is_ok(),
        "WHAT: the ten-letter key PLATFORMOP cannot be built. WHY: its issues must carry it. \
         HOW: accept it."
    );
    let issue = IssueKey::try_new(&key.expect("an accepted key"), 1).expect("issue number one");
    assert_eq!(issue.to_string(), "PLATFORMOP-1");
}
