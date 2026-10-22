//! external-integration-plugins — gold tests for `SecretBox` and
//! `ProjectEmailStopToken` (DESIGN DDR-9, DDR-16, ADR-EIP-004, ADR-EIP-007;
//! DEVOPS eip-auth-unit). Pending stubs written by DISTILL 2026-10-21 (ADR-025
//! scaffolded RED): each is `#[ignore]`d with its DELIVER step id and panics
//! with the RED marker until DELIVER replaces its body. DELIVER moves a test
//! into `src/secret_box.rs` / the stop-token module if it prefers unit tests;
//! the names are the contract.

macro_rules! pending {
    ($name:ident, $step:literal, $what:literal) => {
        #[test]
        #[ignore = concat!("pending: external-integration-plugins ", $step, " ", $what)]
        fn $name() {
            panic!(concat!(
                "Not yet implemented -- RED scaffold (external-integration-plugins ",
                $step,
                ": ",
                $what,
                ")"
            ));
        }
    };
}

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
pending!(
    stop_token_binds_member_project_and_event,
    "05-01",
    "the signed link names exactly one (member, project, event) choice"
);
pending!(
    stop_token_with_an_altered_signature_is_refused,
    "05-01",
    "any change to the token or its fields fails verification"
);
pending!(
    stop_token_never_expires,
    "05-01",
    "DD-7: a token minted long ago still verifies"
);
pending!(
    stop_token_is_not_an_unsubscribe_token,
    "05-01",
    "a shipped workspace UnsubscribeToken does not verify as a stop token, and the reverse"
);
