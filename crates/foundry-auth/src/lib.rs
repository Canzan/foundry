//! foundry-auth — slice 1 surface.
//!
//! Slice 1 ships:
//! - [`BootstrapToken`] (US-01) — the admin-claim URL token.
//! - [`hash_password`] / [`verify_password`] (US-05) — argon2id at
//!   OWASP-recommended parameters.
//! - [`sign`] / [`verify`] — HMAC primitives used by [`InviteToken`].
//! - [`InviteToken`] (US-05) — signed shareable invite URL.

#![forbid(unsafe_code)]
#![deny(clippy::all)]

use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version};
use base64::Engine;
use hmac::{Hmac, Mac};
use password_hash::SaltString;
use rand::rngs::OsRng;
use rand::RngCore;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("hmac key invalid")]
    HmacKey,
    #[error("hmac signature mismatch")]
    SignatureMismatch,
    #[error("invalid base64 in signature")]
    BadEncoding(#[from] base64::DecodeError),
    #[error("password hashing failed: {0}")]
    PasswordHash(String),
}

/// A freshly-minted bootstrap token. The `raw` value goes into the
/// admin-claim URL printed on stdout; `hash` is what we persist.
pub struct BootstrapToken {
    pub raw: SecretString,
    pub hash: [u8; 32],
}

impl BootstrapToken {
    /// Generate 32 random bytes, encode URL-safe base64, and compute
    /// the SHA-256 hash for persistence.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        let mut hasher = Sha256::new();
        hasher.update(raw.as_bytes());
        let hash: [u8; 32] = hasher.finalize().into();
        Self {
            raw: SecretString::new(raw.into()),
            hash,
        }
    }
}

/// Sign a payload with the session secret. Used by US-05's invite-link
/// HMAC sigs; declared here in slice 1 so the crate API is shaped
/// correctly. Returns URL-safe base64.
pub fn sign(secret: &SecretString, payload: &[u8]) -> Result<String, AuthError> {
    let mut mac = HmacSha256::new_from_slice(secret.expose_secret().as_bytes())
        .map_err(|_| AuthError::HmacKey)?;
    mac.update(payload);
    let tag = mac.finalize().into_bytes();
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(tag))
}

/// Verify a signature in constant time.
pub fn verify(secret: &SecretString, payload: &[u8], signature: &str) -> Result<(), AuthError> {
    let provided = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(signature)?;
    let mut mac = HmacSha256::new_from_slice(secret.expose_secret().as_bytes())
        .map_err(|_| AuthError::HmacKey)?;
    mac.update(payload);
    let expected = mac.finalize().into_bytes();
    if provided.ct_eq(expected.as_slice()).into() {
        Ok(())
    } else {
        Err(AuthError::SignatureMismatch)
    }
}

/// Argon2id parameters per the OWASP-recommended minimum for 2024+.
/// 64 MiB memory, 3 iterations, 1 lane. Stored hashes encode their own
/// parameters so this constant can bump without invalidating old hashes.
fn argon2_params() -> Params {
    Params::new(64 * 1024, 3, 1, None).expect("OWASP-recommended argon2id params")
}

/// Hash a password with argon2id at OWASP-recommended parameters.
/// Returns the PHC-encoded hash string ready to persist in `users.password_hash`.
///
/// Runs the CPU work on a blocking thread because OWASP-grade argon2id
/// (64 MiB × 3 iterations) pins a CPU for 80–300ms; executing it on a
/// tokio worker would stall every other future scheduled on that
/// worker for the duration. The async lifecycle (allocate salt, build
/// hasher, encode result) is moved entirely inside `spawn_blocking`
/// so the caller never observes the cost on its own runtime thread.
pub async fn hash_password(password: &SecretString) -> Result<String, AuthError> {
    let bytes = password.expose_secret().as_bytes().to_vec();
    tokio::task::spawn_blocking(move || -> Result<String, AuthError> {
        let salt = SaltString::generate(&mut OsRng);
        let hasher = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2_params());
        let hash = hasher
            .hash_password(&bytes, &salt)
            .map_err(|e| AuthError::PasswordHash(e.to_string()))?;
        Ok(hash.to_string())
    })
    .await
    .map_err(|e| AuthError::PasswordHash(format!("spawn_blocking join: {e}")))?
}

/// Verify a password against a PHC-encoded hash. Constant-time.
///
/// Same blocking-thread discipline as [`hash_password`].
pub async fn verify_password(password: &SecretString, encoded: &str) -> Result<bool, AuthError> {
    let bytes = password.expose_secret().as_bytes().to_vec();
    let encoded = encoded.to_string();
    tokio::task::spawn_blocking(move || -> Result<bool, AuthError> {
        let parsed =
            PasswordHash::new(&encoded).map_err(|e| AuthError::PasswordHash(e.to_string()))?;
        let hasher = Argon2::default();
        Ok(hasher.verify_password(&bytes, &parsed).is_ok())
    })
    .await
    .map_err(|e| AuthError::PasswordHash(format!("spawn_blocking join: {e}")))?
}

/// A freshly-minted invite token. The shareable URL embeds both
/// `invite_id` (the database row PK) and `sig` (HMAC of
/// `invite_id || expires_at`). The DB row is the primary single-use
/// control; the HMAC is defense-in-depth so obviously-tampered URLs
/// can be rejected without a DB hit.
pub struct InviteToken {
    pub invite_id: uuid::Uuid,
    pub expires_at: time::OffsetDateTime,
    pub signature: String,
}

impl InviteToken {
    /// Sign a new invite. `secret` is the application's SESSION_SECRET.
    pub fn new(
        invite_id: uuid::Uuid,
        expires_at: time::OffsetDateTime,
        secret: &SecretString,
    ) -> Result<Self, AuthError> {
        let payload = invite_payload(invite_id, expires_at);
        let signature = sign(secret, payload.as_bytes())?;
        Ok(Self {
            invite_id,
            expires_at,
            signature,
        })
    }

    /// Verify a signature against the (invite_id, expires_at) pair.
    pub fn verify(
        invite_id: uuid::Uuid,
        expires_at: time::OffsetDateTime,
        signature: &str,
        secret: &SecretString,
    ) -> Result<(), AuthError> {
        let payload = invite_payload(invite_id, expires_at);
        verify(secret, payload.as_bytes(), signature)
    }
}

fn invite_payload(id: uuid::Uuid, expires_at: time::OffsetDateTime) -> String {
    format!("{}|{}", id, expires_at.unix_timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_token_round_trips_through_hash() {
        let token = BootstrapToken::generate();
        let mut hasher = Sha256::new();
        hasher.update(token.raw.expose_secret().as_bytes());
        let recomputed: [u8; 32] = hasher.finalize().into();
        assert_eq!(token.hash, recomputed);
    }

    #[test]
    fn sign_and_verify_round_trip() {
        let secret = SecretString::new("0123456789abcdef0123456789abcdef".to_string().into());
        let sig = sign(&secret, b"invite-id|expires-at").unwrap();
        assert!(verify(&secret, b"invite-id|expires-at", &sig).is_ok());
        assert!(verify(&secret, b"tampered", &sig).is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn password_hash_round_trip() {
        let pwd = SecretString::new("correct horse battery staple".to_string().into());
        let encoded = hash_password(&pwd).await.unwrap();
        assert!(verify_password(&pwd, &encoded).await.unwrap());
        let wrong = SecretString::new("wrong password".to_string().into());
        assert!(!verify_password(&wrong, &encoded).await.unwrap());
    }

    #[test]
    fn invite_token_round_trip() {
        let secret = SecretString::new("0123456789abcdef0123456789abcdef".to_string().into());
        let id = uuid::Uuid::now_v7();
        let exp = time::OffsetDateTime::now_utc() + time::Duration::days(7);
        let tok = InviteToken::new(id, exp, &secret).unwrap();
        assert!(InviteToken::verify(id, exp, &tok.signature, &secret).is_ok());
        assert!(InviteToken::verify(id, exp, "AAAA", &secret).is_err());
    }
}
