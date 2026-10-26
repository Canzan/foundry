//! external-integration-plugins — gold tests for `SecretBox` and
//! `ProjectEmailStopToken` (DESIGN DDR-9, DDR-16, ADR-EIP-004, ADR-EIP-007;
//! DEVOPS eip-auth-unit). Pending stubs written by DISTILL 2026-10-21 (ADR-025
//! scaffolded RED): each is `#[ignore]`d with its DELIVER step id and panics
//! with the RED marker until DELIVER replaces its body. DELIVER moves a test
//! into `src/secret_box.rs` / the stop-token module if it prefers unit tests;
//! the names are the contract. DELIVER 01-03 and 05-01 replaced every body.

// SecretBox (slice 01): seal returns a value, open returns SecretString, no I/O.

use base64::Engine;
use foundry_auth::secret_box::{
    SealedSecret, SecretBox, SecretBoxError, SecretSlot, SecretsKey, SECRETS_KEY_PREVIOUS_SETTING,
    SECRETS_KEY_SETTING,
};
use proptest::prelude::*;
use secrecy::{ExposeSecret, SecretString};

const KEY_A: [u8; 32] = [7; 32];
const KEY_B: [u8; 32] = [42; 32];

fn b64url(bytes: &[u8]) -> SecretString {
    SecretString::from(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

fn secret_box(current: &[u8; 32], previous: &[&[u8; 32]]) -> SecretBox {
    let previous = previous
        .iter()
        .map(|key| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key))
        .collect::<Vec<_>>()
        .join(",");
    let previous = (!previous.is_empty()).then(|| SecretString::from(previous));
    SecretBox::from_settings(&b64url(current), previous.as_ref()).expect("well-formed keys")
}

fn slack_token_slot(row_id: &str) -> SecretSlot {
    SecretSlot::new("integration_connections", row_id, "sealed_credential")
}

fn token(text: &str) -> SecretString {
    SecretString::from(text.to_string())
}

proptest! {
    #[test]
    fn sealed_secret_opens_to_the_same_plaintext(plaintext in "\\PC{0,200}", row in "[a-f0-9-]{1,36}") {
        let sealer = secret_box(&KEY_A, &[]);
        let slot = slack_token_slot(&row);
        let sealed = sealer.seal(&token(&plaintext), &slot).expect("seal");
        let stored = SealedSecret::from_bytes(sealed.as_bytes().to_vec());
        let opened = sealer.open(&stored, &slot).expect("open under the current key");
        prop_assert_eq!(opened.expose_secret(), plaintext.as_str());
        prop_assert!(!contains(sealed.as_bytes(), plaintext.as_bytes()) || plaintext.len() < 4);
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn open_with_the_wrong_row_fails_closed() {
    let sealer = secret_box(&KEY_A, &[]);
    let sealed = sealer
        .seal(&token("xoxb-row-one"), &slack_token_slot("row-1"))
        .expect("seal");
    let elsewhere = [
        SecretSlot::new("integration_connections", "row-2", "sealed_credential"),
        SecretSlot::new("integration_webhook_secrets", "row-1", "sealed_credential"),
        SecretSlot::new("integration_connections", "row-1", "other_column"),
    ];
    for slot in &elsewhere {
        assert_eq!(
            sealer.open(&sealed, slot).map(|_| ()),
            Err(SecretBoxError::OpenRefused)
        );
    }
    assert!(sealer.open(&sealed, &slack_token_slot("row-1")).is_ok());
}

#[test]
fn open_with_the_wrong_key_fails_closed() {
    let sealed = secret_box(&KEY_A, &[])
        .seal(&token("xoxb-sealed-under-a"), &slack_token_slot("row-1"))
        .expect("seal");
    let foreign = secret_box(&KEY_B, &[]);
    let refusal = foreign
        .open(&sealed, &slack_token_slot("row-1"))
        .map(|_| ())
        .expect_err("a foreign key must not open");
    assert_eq!(refusal, SecretBoxError::OpenRefused);
    let text = format!("{refusal} {refusal:?}");
    for key in [KEY_A, KEY_B] {
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key);
        assert!(!text.contains(&encoded), "refusal names a key: {text}");
    }
    assert!(
        !text.contains(&sealed.key_id().to_string()),
        "refusal names a key id: {text}"
    );
    assert!(
        !text.contains("xoxb"),
        "refusal echoes the credential: {text}"
    );
}

proptest! {
    #[test]
    fn truncated_or_altered_ciphertext_fails_closed(
        cut in any::<prop::sample::Index>(),
        flip in any::<prop::sample::Index>(),
        bit in 0u8..8,
    ) {
        let sealer = secret_box(&KEY_A, &[]);
        let slot = slack_token_slot("row-1");
        let bytes = sealer.seal(&token("xoxb-altered"), &slot).expect("seal").as_bytes().to_vec();

        let truncated = bytes[..cut.index(bytes.len())].to_vec();
        prop_assert_eq!(
            sealer.open(&SealedSecret::from_bytes(truncated), &slot).map(|_| ()),
            Err(SecretBoxError::OpenRefused)
        );

        let mut altered = bytes.clone();
        altered[flip.index(bytes.len())] ^= 1 << bit;
        prop_assert_eq!(
            sealer.open(&SealedSecret::from_bytes(altered), &slot).map(|_| ()),
            Err(SecretBoxError::OpenRefused)
        );

        let mut extended = bytes;
        extended.push(0);
        prop_assert_eq!(
            sealer.open(&SealedSecret::from_bytes(extended), &slot).map(|_| ()),
            Err(SecretBoxError::OpenRefused)
        );
    }
}

#[test]
fn previous_key_opens_during_a_rotation() {
    let slot = slack_token_slot("row-1");
    let before = secret_box(&KEY_A, &[]);
    let sealed_under_old = before.seal(&token("xoxb-old"), &slot).expect("seal");

    let rotating = secret_box(&KEY_B, &[&KEY_A]);
    let opened = rotating
        .open(&sealed_under_old, &slot)
        .expect("previous key opens");
    assert_eq!(opened.expose_secret(), "xoxb-old");

    let resealed = rotating.seal(&token("xoxb-new"), &slot).expect("seal");
    assert_eq!(resealed.key_id(), rotating.current_key_id());
    assert_ne!(resealed.key_id(), sealed_under_old.key_id());
    assert_eq!(
        before.open(&resealed, &slot).map(|_| ()),
        Err(SecretBoxError::OpenRefused),
        "sealing must use the current key, never a previous one"
    );
    let after = secret_box(&KEY_B, &[]);
    assert_eq!(
        after.open(&resealed, &slot).expect("open").expose_secret(),
        "xoxb-new"
    );
    assert_eq!(
        after.open(&sealed_under_old, &slot).map(|_| ()),
        Err(SecretBoxError::OpenRefused)
    );
    assert_eq!(
        rotating.key_ids(),
        vec![after.current_key_id(), before.current_key_id()]
    );
}

/// R2-e (DELIVER review, part 1, non-blocking; fixed before release by the
/// user's decision of 2026-10-25): `FOUNDRY_SECRETS_KEY_PREVIOUS` is a
/// comma-separated list an operator types by hand. Spaces around an entry
/// are trimmed and blank entries are dropped, so " k1 , ,k2 " gives exactly
/// two retired keys, each opening what it sealed. RED today: the entries are
/// parsed untrimmed, so " k1 " is refused as malformed and Foundry will not
/// start.
///
/// CONTRACT_SHAPE: pure-function
#[test]
#[ignore = "RED until review fix R2-e"]
fn previous_keys_are_trimmed_and_blank_entries_dropped() {
    const KEY_C: [u8; 32] = [99; 32];
    let encode = |key: &[u8; 32]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key);
    let typed = SecretString::from(format!(" {} , ,{} ", encode(&KEY_B), encode(&KEY_C)));

    let rotating = SecretBox::from_settings(&b64url(&KEY_A), Some(&typed))
        .expect("entries with spaces around them and a blank entry are well-formed");

    let id_of = |key: &[u8; 32]| secret_box(key, &[]).current_key_id();
    assert_eq!(
        rotating.key_ids(),
        vec![id_of(&KEY_A), id_of(&KEY_B), id_of(&KEY_C)],
        "the current key, then exactly the two retired keys, in order"
    );
    let slot = slack_token_slot("row-1");
    for retired in [&KEY_B, &KEY_C] {
        let sealed = secret_box(retired, &[])
            .seal(&token("xoxb-retired"), &slot)
            .expect("seal");
        assert_eq!(
            rotating
                .open(&sealed, &slot)
                .expect("a trimmed retired key opens what it sealed")
                .expose_secret(),
            "xoxb-retired"
        );
    }
}

#[test]
fn key_accepts_base64url_with_or_without_padding() {
    // Bytes whose encoding uses both URL-safe characters ('-' and '_').
    let key: [u8; 32] = std::array::from_fn(|i| if i % 2 == 0 { 0xfb } else { 0xff });
    let unpadded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key);
    let padded = base64::engine::general_purpose::URL_SAFE.encode(key);
    assert!(padded.ends_with('='));
    assert!(unpadded.contains('-') && unpadded.contains('_'));

    let a = SecretsKey::parse(&SecretString::from(unpadded.clone()), SECRETS_KEY_SETTING)
        .expect("unpadded base64url");
    let b = SecretsKey::parse(&SecretString::from(padded), SECRETS_KEY_SETTING)
        .expect("padded base64url");
    assert_eq!(a.key_id(), b.key_id());

    let standard = base64::engine::general_purpose::STANDARD.encode(key);
    let malformed = [
        String::new(),
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([1u8; 31]),
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([1u8; 33]),
        base64::engine::general_purpose::URL_SAFE.encode([1u8; 64]),
        standard,
        format!("{unpadded}!"),
        "not a key at all".to_string(),
    ];
    for candidate in malformed {
        assert_eq!(
            SecretsKey::parse(&SecretString::from(candidate.clone()), SECRETS_KEY_SETTING)
                .map(|_| ()),
            Err(SecretBoxError::MalformedKey {
                setting: SECRETS_KEY_SETTING
            }),
            "{candidate:?} must be malformed"
        );
    }
}

#[test]
fn malformed_key_is_refused_without_echoing_it() {
    let short = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"sixteen-byte-key");
    let good = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(KEY_A);
    let cases = [
        (SecretString::from(short.clone()), None, SECRETS_KEY_SETTING),
        (
            SecretString::from(good.clone()),
            Some(SecretString::from(format!("{good},{short}"))),
            SECRETS_KEY_PREVIOUS_SETTING,
        ),
    ];
    for (current, previous, setting) in cases {
        let refusal = SecretBox::from_settings(&current, previous.as_ref())
            .map(|_| ())
            .expect_err("a key that is not 32 bytes is refused");
        assert_eq!(refusal, SecretBoxError::MalformedKey { setting });
        let text = format!("{refusal} {refusal:?}");
        assert!(
            text.contains(setting),
            "the refusal names the setting: {text}"
        );
        assert!(!text.contains(&short), "the refusal echoes the key: {text}");
        assert!(
            !text.contains("sixteen"),
            "the refusal echoes the key: {text}"
        );
    }
    assert!(SecretBox::from_settings(&SecretString::from(good.clone()), None).is_ok());
    assert!(
        SecretBox::from_settings(
            &SecretString::from(good),
            Some(&SecretString::from(" ".to_string()))
        )
        .is_ok(),
        "an empty previous-key setting means no previous key"
    );
}

/// Compiles only while `T` implements none of the probed traits: a second
/// blanket impl makes the call ambiguous (the `static_assertions` technique).
macro_rules! assert_not_impl {
    ($ty:ty, $($probe:path),+ $(,)?) => {{
        $({
            trait Ambiguous<A> {
                fn probe() {}
            }
            impl<T: ?Sized> Ambiguous<()> for T {}
            #[allow(dead_code)]
            struct Implemented;
            impl<T: ?Sized + $probe> Ambiguous<Implemented> for T {}
            <$ty as Ambiguous<_>>::probe();
        })+
    }};
}

#[test]
fn sealed_secret_has_no_debug_display_or_serialize() {
    assert_not_impl!(
        SealedSecret,
        std::fmt::Debug,
        std::fmt::Display,
        serde::Serialize
    );
    let sealed = secret_box(&KEY_A, &[])
        .seal(&token("xoxb-opaque"), &slack_token_slot("row-1"))
        .expect("seal");
    assert!(!contains(sealed.as_bytes(), b"xoxb-opaque"));
}

// ProjectEmailStopToken (slice 05).
use foundry_auth::stop_token::ProjectEmailStopToken;
use foundry_auth::UnsubscribeToken;

const STOP_SECRET: &str = "eip-stop-token-session-secret-at-least-32-bytes";
const EMAIL_EVENTS: [&str; 4] = [
    "issue.created",
    "issue.moved",
    "issue.deleted",
    "comment.added",
];

fn session_secret() -> SecretString {
    SecretString::from(STOP_SECRET)
}

fn uuid_from(bits: u128) -> uuid::Uuid {
    uuid::Uuid::from_u128(bits)
}

fn a_choice() -> impl Strategy<Value = (uuid::Uuid, uuid::Uuid, &'static str)> {
    (any::<u128>(), any::<u128>(), 0..EMAIL_EVENTS.len()).prop_map(|(user, project, event)| {
        (uuid_from(user), uuid_from(project), EMAIL_EVENTS[event])
    })
}

proptest! {
    /// The signed link names exactly one (member, project, event) choice: it
    /// verifies, carries its three fields back through its link parameters,
    /// and a token for any other member, project or event does not verify
    /// with its signature.
    #[test]
    fn stop_token_binds_member_project_and_event(
        (user, project, event) in a_choice(),
        (other_user, other_project, other_event) in a_choice(),
    ) {
        let secret = session_secret();
        let token = ProjectEmailStopToken::new(user, project, event, &secret).expect("sign");
        prop_assert!(token.verify(&secret).is_ok());
        let carried = ProjectEmailStopToken::from_params(&token.choice_param(), &token.signature)
            .expect("its own link parameters name a choice");
        prop_assert_eq!(&carried, &token);
        prop_assert!(carried.verify(&secret).is_ok());
        for (u, p, e) in [
            (other_user, project, event),
            (user, other_project, event),
            (user, project, other_event),
        ] {
            let moved = ProjectEmailStopToken {
                user_id: u,
                project_id: p,
                event: e.to_string(),
                signature: token.signature.clone(),
            };
            prop_assert_eq!(
                moved.verify(&secret).is_ok(),
                (u, p, e) == (user, project, event),
                "{:?}", moved
            );
        }
    }

    /// Any change to the signature fails verification, as does the right
    /// signature under another secret.
    #[test]
    fn stop_token_with_an_altered_signature_is_refused(
        (user, project, event) in a_choice(),
        flip in any::<prop::sample::Index>(),
        replacement in "[A-Za-z0-9_-]{0,50}",
    ) {
        let secret = session_secret();
        let token = ProjectEmailStopToken::new(user, project, event, &secret).expect("sign");
        let mut bytes = token.signature.clone().into_bytes();
        let at = flip.index(bytes.len());
        bytes[at] = if bytes[at] == b'A' { b'B' } else { b'A' };
        let flipped = ProjectEmailStopToken {
            signature: String::from_utf8(bytes).expect("ascii"),
            ..token.clone()
        };
        prop_assert!(flipped.verify(&secret).is_err());
        let replaced = ProjectEmailStopToken { signature: replacement.clone(), ..token.clone() };
        prop_assert_eq!(replaced.verify(&secret).is_ok(), replacement == token.signature);
        let other_secret = SecretString::from("another-session-secret-of-at-least-32-bytes");
        prop_assert!(token.verify(&other_secret).is_err());
    }

    /// DD-7: the token carries no time. Its signature is exactly the HMAC of
    /// `stop|v1|{user_id}|{project_id}|{event}` (spelled out here, not read
    /// from the module), so nothing it signs can lapse; one minted long ago is
    /// the token minted now: the same link, and it still verifies.
    ///
    /// CONTRACT_SHAPE: pure-function
    #[test]
    fn stop_token_never_expires((user, project, event) in a_choice()) {
        let secret = session_secret();
        let long_ago = ProjectEmailStopToken::new(user, project, event, &secret).expect("sign");
        let timeless = format!("stop|v1|{user}|{project}|{event}");
        prop_assert_eq!(
            &long_ago.signature,
            &foundry_auth::sign(&secret, timeless.as_bytes()).expect("sign the payload"),
            "the signature covers the choice and nothing else"
        );
        let now = ProjectEmailStopToken::new(user, project, event, &secret).expect("sign");
        prop_assert_eq!(&long_ago, &now);
        prop_assert!(long_ago.verify(&secret).is_ok());
    }

    /// Domain separation: a shipped workspace UnsubscribeToken signature
    /// never verifies as a stop token, nor a stop token's as an unsubscribe,
    /// even when the stop token's member id is spelled as the unsubscribe's
    /// address and its project as the workspace.
    #[test]
    fn stop_token_is_not_an_unsubscribe_token(
        (user, project, event) in a_choice(),
        email in "[a-z]{1,12}@[a-z]{1,8}\\.example",
    ) {
        let secret = session_secret();
        let unsubscribe = UnsubscribeToken::new(&email, project, &secret).expect("sign");
        let posing = ProjectEmailStopToken {
            user_id: user,
            project_id: project,
            event: event.to_string(),
            signature: unsubscribe.signature.clone(),
        };
        prop_assert!(posing.verify(&secret).is_err());
        let stop = ProjectEmailStopToken::new(user, project, event, &secret).expect("sign");
        prop_assert!(UnsubscribeToken::verify(&email, project, &stop.signature, &secret).is_err());
        prop_assert!(
            UnsubscribeToken::verify(&user.to_string(), project, &stop.signature, &secret).is_err()
        );
    }
}
