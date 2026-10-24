//! external-integration-plugins (DD-2, U-D8, ADR-EIP-004 decision 4) — the
//! boot secrets probe below the acceptance lane: the check value Foundry
//! records for its current key proves that key, and a stored check value
//! sealed under any other key leaves the secrets check degraded.

use foundry_app::integrations::config::IntegrationSettings;
use foundry_app::integrations::{credential_slot, Integrations};
use foundry_auth::secret_box::SecretBox;
use foundry_store::integrations::KeyCheck;
use secrecy::SecretString;

const KEY: &str = "ZWlwLXNlbnRpbmVsLXNlY3JldHMta2V5LTMyYnl0ZXM";
const OTHER_KEY: &str = "ZWlwLXNlbnRpbmVsLW5ldy1zZWNyZXRzLWtleS0zMmI";
/// Version byte, then the 8-byte key id (`foundry_auth::secret_box`).
const KEY_ID_BYTES: std::ops::Range<usize> = 1..9;

fn integrations(key: &str) -> Integrations {
    let env = [("FOUNDRY_SECRETS_KEY", key.to_string())];
    Integrations::new(
        IntegrationSettings::from_lookup(
            |name| env.iter().find(|(k, _)| *k == name).map(|(_, v)| v.clone()),
            "http://foundry.test",
        )
        .expect("settings with a secrets key"),
    )
}

fn secrets(integrations: &Integrations) -> &SecretBox {
    integrations.settings().secrets.as_ref().expect("a key")
}

/// One stored credential, sealed under `integrations`' current key.
fn stored_credential(integrations: &Integrations) -> Vec<u8> {
    secrets(integrations)
        .seal(
            &SecretString::from("xoxb-eip-probe-token".to_string()),
            &credential_slot(uuid::Uuid::now_v7()),
        )
        .expect("seal")
        .as_bytes()
        .to_vec()
}

#[test]
fn the_recorded_check_value_proves_the_current_key() {
    let foundry = integrations(KEY);
    let check = foundry
        .key_check()
        .expect("a Foundry with a secrets key has a check value to record");
    assert_eq!(
        check.key_id,
        secrets(&foundry).current_key_id().to_string(),
        "the check value is filed under the current key's id"
    );

    let verdict = foundry.check_secrets(&[stored_credential(&foundry)], &[check]);

    assert!(
        !verdict.degraded(),
        "the key's own check value opens: {verdict:?}"
    );
}

#[test]
fn a_check_value_sealed_under_another_key_degrades_the_secrets_check() {
    let foundry = integrations(KEY);
    let own = foundry.key_check().expect("own check value");
    // The other key's check value, relabelled as this key's: same key id in
    // the row and in the sealed header, but the ciphertext is another key's.
    let mut forged = integrations(OTHER_KEY)
        .key_check()
        .expect("other check value")
        .sealed_canary;
    forged[KEY_ID_BYTES].copy_from_slice(&own.sealed_canary[KEY_ID_BYTES]);
    let forged = KeyCheck {
        key_id: own.key_id.clone(),
        sealed_canary: forged,
    };

    let verdict = foundry.check_secrets(&[stored_credential(&foundry)], &[forged]);

    assert!(verdict.degraded(), "the forged check value does not open");
    assert_eq!(verdict.missing_key_ids, vec![own.key_id]);
}
