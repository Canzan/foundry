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
pending!(
    sealed_secret_opens_to_the_same_plaintext,
    "01-03",
    "seal then open is the identity under the current key"
);
pending!(open_with_the_wrong_row_fails_closed, "01-03", "associated data bound to table, row and column: a ciphertext moved to another row does not open");
pending!(
    open_with_the_wrong_key_fails_closed,
    "01-03",
    "a value sealed under another key id does not open and names no key"
);
pending!(
    truncated_or_altered_ciphertext_fails_closed,
    "01-03",
    "any truncation or bit flip is refused, never a panic"
);
pending!(previous_key_opens_during_a_rotation, "01-03", "FOUNDRY_SECRETS_KEY_PREVIOUS opens values sealed under it; sealing always uses the current key");
pending!(
    key_accepts_base64url_with_or_without_padding,
    "01-03",
    "U-D5: 32 bytes as base64url, padded or not; anything else is malformed"
);
pending!(
    malformed_key_is_refused_without_echoing_it,
    "01-03",
    "a key that does not decode to 32 bytes is an error whose text never contains the key"
);
pending!(
    sealed_secret_has_no_debug_display_or_serialize,
    "01-03",
    "compile-time: SealedSecret implements none of Debug, Display, Serialize (static assertion)"
);

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
