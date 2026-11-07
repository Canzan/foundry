//! The generated-key test seam (instance-integration-setup DISCUSS D3,
//! DESIGN D8, DISTILL D6): acceptance reads the secrets key Foundry
//! generated as a planted string, for the "never shown" sweep and the
//! database-dump check (KPI-IIS-G1).
//!
//! Nothing but a `test-support` build declares this module
//! (`integrations/mod.rs`): check-arch `integrations-test-seam` lists it, and
//! the Dockerfile refuses a binary carrying [`SECRETS_KEY_SEAM_MARKER`]. It
//! takes a pool, not a `Store` (check-arch `integrations-store-capability`),
//! and reads the row through the store's own reader (check-arch
//! `integrations-instance-tables`). It never logs the key.

use super::keys::secrets_key_slot;
use foundry_auth::secret_box::{SealedSecret, SecretBox, SecretsKey, INSTANCE_KEY_SETTING};
use foundry_store::integrations::StoredSecretsKey;
use secrecy::{ExposeSecret, SecretString};

/// The seam's unique marker: present in a binary only if the seam is.
pub const SECRETS_KEY_SEAM_MARKER: &str = "foundry-test-seam:integrations-secrets-key-v1";

/// The secrets key stored in the database `pool` reaches, unwrapped with `instance_key` (the
/// operator's setting value, or the Foundry-made file's contents), as its
/// base64url text. `None` when no secrets key is stored or `instance_key`
/// does not unwrap it.
pub async fn generated_secrets_key(pool: &sqlx::PgPool, instance_key: &str) -> Option<String> {
    tracing::trace!(
        seam = SECRETS_KEY_SEAM_MARKER,
        "integrations.secrets_key.read"
    );
    let stored = StoredSecretsKey::read_for_test_seam(pool).await.ok()??;
    let instance = SecretsKey::parse(
        &SecretString::from(instance_key.to_string()),
        INSTANCE_KEY_SETTING,
    )
    .ok()?;
    let text = SecretBox::new(instance, Vec::new())
        .open(
            &SealedSecret::from_bytes(stored.wrapped_key),
            &secrets_key_slot(),
        )
        .ok()?;
    let key = SecretsKey::parse(&text, INSTANCE_KEY_SETTING).ok()?;
    (key.key_id().to_string() == stored.key_id).then(|| text.expose_secret().to_string())
}
