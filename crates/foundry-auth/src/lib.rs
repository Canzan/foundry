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
use jsonwebtoken::{
    decode, encode, Algorithm as JwtAlgorithm, DecodingKey, EncodingKey, Header, Validation,
};
use password_hash::SaltString;
use rand::rngs::OsRng;
use rand::RngCore;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
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
    #[error("machine-token signing key invalid")]
    MachineTokenSigningKey,
    #[error("machine-token public key invalid")]
    MachineTokenPublicKey,
    #[error("machine-token verifier needs at least one public key")]
    MachineTokenNoPublicKeys,
    #[error("machine-token mint failed")]
    MachineTokenMint,
    #[error("machine-token verification failed")]
    MachineTokenVerify,
    #[error("machine-token key probe failed: keypair does not round-trip")]
    MachineTokenKeyProbe,
}

/// The v1 machine-token claim set (US-W05b, ADR-W02). Carries the bound
/// principal (`sub`), an optional team-narrowing `scope`, the unique
/// token id (`jti`, the denylist/registry key), and the issued-at /
/// expiry timestamps. These serialize 1:1 to the signed JWT body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineTokenClaims {
    /// The bound principal — a `user_id`. Authorization is computed from this.
    pub sub: uuid::Uuid,
    /// Team-narrowing filter. `None` = workspace-wide (still bounded by
    /// `sub`'s membership, enforced downstream by the service layer).
    pub scope: Option<uuid::Uuid>,
    /// Issued-at (unix seconds).
    pub iat: i64,
    /// Expiry (unix seconds). The EdDSA-pinned `Validation` rejects an
    /// expired token before any registry lookup.
    pub exp: i64,
    /// Unique token id — the per-request `jti` denylist lookup key.
    pub jti: uuid::Uuid,
}

/// Holds the Ed25519 signing (private) key, ready to mint compact JWTs.
/// Built only on a binary configured to ISSUE tokens
/// (`MACHINE_TOKEN_SIGNING_KEY`). The minted JWT is wrapped in
/// `SecretString` so it can never be `Debug`/`Display`-logged — same
/// posture as [`BootstrapToken::raw`].
pub struct MachineTokenSigner {
    encoding_key: EncodingKey,
}

impl MachineTokenSigner {
    /// Parse the Ed25519 private key from a PKCS#8 PEM string (the
    /// `MACHINE_TOKEN_SIGNING_KEY` env value), wrapped in `SecretString`
    /// like `SESSION_SECRET`.
    pub fn from_pkcs8_pem(secret: &SecretString) -> Result<Self, AuthError> {
        let encoding_key = EncodingKey::from_ed_pem(secret.expose_secret().as_bytes())
            .map_err(|_| AuthError::MachineTokenSigningKey)?;
        Ok(Self { encoding_key })
    }

    /// EdDSA-sign the claims into a compact JWT, wrapped in `SecretString`.
    pub fn mint(&self, claims: &MachineTokenClaims) -> Result<SecretString, AuthError> {
        let header = Header::new(JwtAlgorithm::EdDSA);
        let jwt =
            encode(&header, claims, &self.encoding_key).map_err(|_| AuthError::MachineTokenMint)?;
        Ok(SecretString::new(jwt.into()))
    }
}

/// Holds the set of Ed25519 verifying (public) keys plus the EdDSA-pinned
/// `Validation`. Built on every binary that VERIFIES
/// (`MACHINE_TOKEN_PUBLIC_KEYS`, comma-separated, newest first). To
/// support overlapping-key rotation it holds a SMALL SET (≤2 in practice)
/// and tries each — a token verifies if ANY configured key accepts it.
///
/// The `Validation` allow-lists EXACTLY `[EdDSA]`, so a token presenting
/// any other `alg` (RS256, HS256, …) or `alg: none` is rejected before any
/// key is consulted — closing the alg-confusion / `none` footgun.
pub struct MachineTokenVerifier {
    decoding_keys: Vec<DecodingKey>,
    validation: Validation,
}

impl MachineTokenVerifier {
    /// Build a verifier from one-or-more Ed25519 public keys in SPKI PEM
    /// form. Returns an error on an empty set or any malformed key (the
    /// boot path turns this into a refuse-to-start).
    pub fn from_public_keys(keys: &[String]) -> Result<Self, AuthError> {
        if keys.is_empty() {
            return Err(AuthError::MachineTokenNoPublicKeys);
        }
        let mut decoding_keys = Vec::with_capacity(keys.len());
        for key in keys {
            let decoding_key = DecodingKey::from_ed_pem(key.as_bytes())
                .map_err(|_| AuthError::MachineTokenPublicKey)?;
            decoding_keys.push(decoding_key);
        }
        // Pin the algorithm allow-list to EXACTLY [EdDSA]. jsonwebtoken
        // rejects `alg: none` and any non-listed alg during decode.
        let mut validation = Validation::new(JwtAlgorithm::EdDSA);
        validation.algorithms = vec![JwtAlgorithm::EdDSA];
        // `exp` is validated by default; we do not require iss/aud here
        // (the single-issuer Feature-A case validates them downstream).
        validation.validate_exp = true;
        Ok(Self {
            decoding_keys,
            validation,
        })
    }

    /// Verify a compact JWT. Tries each configured public key; succeeds if
    /// ANY key accepts the signature AND the EdDSA-pinned `Validation`
    /// passes (alg = EdDSA, not expired). Returns the recovered claims.
    pub fn verify(&self, jwt: &str) -> Result<MachineTokenClaims, AuthError> {
        for key in &self.decoding_keys {
            if let Ok(data) = decode::<MachineTokenClaims>(jwt, key, &self.validation) {
                return Ok(data.claims);
            }
        }
        Err(AuthError::MachineTokenVerify)
    }

    /// Earned-Trust key probe (wire-then-probe-then-use): sign a throwaway
    /// claim set with `signer` and verify it under THIS verifier's public
    /// keys, proving the keypair round-trips in this environment. A
    /// malformed or mismatched key makes this fail so the boot path can
    /// refuse to start with `health.startup.refused {reason:'machine_token_key'}`.
    pub fn self_test(&self, signer: &MachineTokenSigner) -> Result<(), AuthError> {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let probe_claims = MachineTokenClaims {
            sub: uuid::Uuid::now_v7(),
            scope: None,
            iat: now,
            exp: now + 60,
            jti: uuid::Uuid::now_v7(),
        };
        let jwt = signer.mint(&probe_claims)?;
        let recovered = self
            .verify(jwt.expose_secret())
            .map_err(|_| AuthError::MachineTokenKeyProbe)?;
        if recovered == probe_claims {
            Ok(())
        } else {
            Err(AuthError::MachineTokenKeyProbe)
        }
    }
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

/// Feature A (US-W05b) — a FIXED test Ed25519 keypair + constructors,
/// mirroring the fixed test `SESSION_SECRET`. Gated behind `test-support`
/// so release builds never carry it. The acceptance harness uses this at
/// its ~4 AppState construction sites so 02-03/W05c can mint+verify real
/// machine tokens against a deterministic keypair.
#[cfg(feature = "test-support")]
pub mod test_keys {
    use super::*;

    /// Fixed Ed25519 private key (PKCS#8 PEM). Test-only.
    pub const TEST_SIGNING_KEY_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIK82aJdks5jZYdUK6HN6tiWmq7nU4yrILbQSibZdvMYK\n-----END PRIVATE KEY-----\n";

    /// Fixed Ed25519 public key (SPKI PEM) matching [`TEST_SIGNING_KEY_PEM`].
    pub const TEST_PUBLIC_KEY_PEM: &str = "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAwtFPs8Jcuncc+E7dXqG/oolI3P6Hamrpd8zVKPvRmg0=\n-----END PUBLIC KEY-----\n";

    /// Build a verifier from the fixed test public key.
    pub fn verifier() -> MachineTokenVerifier {
        MachineTokenVerifier::from_public_keys(&[TEST_PUBLIC_KEY_PEM.to_string()])
            .expect("fixed test public key is valid")
    }

    /// Build a signer from the fixed test private key.
    pub fn signer() -> MachineTokenSigner {
        MachineTokenSigner::from_pkcs8_pem(&SecretString::new(
            TEST_SIGNING_KEY_PEM.to_string().into(),
        ))
        .expect("fixed test signing key is valid")
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
mod machine_token_tests {
    use super::*;
    use proptest::prelude::*;

    // Fixed test Ed25519 keypair (PKCS#8 / SPKI PEM). Generated once with
    // `openssl genpkey -algorithm ed25519`; matches the fixed test
    // SESSION_SECRET posture — deterministic so 02-03/W05c can mint+verify.
    const TEST_PRIV_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIK82aJdks5jZYdUK6HN6tiWmq7nU4yrILbQSibZdvMYK\n-----END PRIVATE KEY-----\n";
    const TEST_PUB_PEM: &str = "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAwtFPs8Jcuncc+E7dXqG/oolI3P6Hamrpd8zVKPvRmg0=\n-----END PUBLIC KEY-----\n";

    // A SECOND, independent keypair — used to prove a token signed by a
    // non-matching key never verifies under the first public key.
    const OTHER_PRIV_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIPhxOnEN0yQC3YZR4Eh1lOaanhB8VioOGewFTymkIWBB\n-----END PRIVATE KEY-----\n";

    fn signer() -> MachineTokenSigner {
        MachineTokenSigner::from_pkcs8_pem(&SecretString::new(TEST_PRIV_PEM.to_string().into()))
            .expect("build signer from fixed test key")
    }

    fn verifier() -> MachineTokenVerifier {
        MachineTokenVerifier::from_public_keys(&[TEST_PUB_PEM.to_string()])
            .expect("build verifier from fixed test public key")
    }

    fn now_unix() -> i64 {
        time::OffsetDateTime::now_utc().unix_timestamp()
    }

    // Strategy: a well-formed claim set with an arbitrary principal,
    // optional scope, and a future expiry.
    fn arb_claims() -> impl Strategy<Value = MachineTokenClaims> {
        (any::<u128>(), any::<u128>(), any::<bool>(), any::<u128>()).prop_map(
            |(sub_bits, jti_bits, has_scope, scope_bits)| {
                let iat = now_unix();
                MachineTokenClaims {
                    sub: uuid::Uuid::from_u128(sub_bits),
                    jti: uuid::Uuid::from_u128(jti_bits),
                    scope: has_scope.then(|| uuid::Uuid::from_u128(scope_bits)),
                    iat,
                    exp: iat + 3600,
                }
            },
        )
    }

    proptest! {
        // Property (a): sign -> verify round-trip. Any well-formed claim
        // set, minted under the signing key, verifies under the matching
        // public key and returns the SAME claims.
        #[test]
        fn sign_verify_round_trip(claims in arb_claims()) {
            let jwt = signer().mint(&claims).expect("mint");
            let recovered = verifier()
                .verify(jwt.expose_secret())
                .expect("verify under matching key");
            prop_assert_eq!(recovered.sub, claims.sub);
            prop_assert_eq!(recovered.jti, claims.jti);
            prop_assert_eq!(recovered.scope, claims.scope);
            prop_assert_eq!(recovered.exp, claims.exp);
            prop_assert_eq!(recovered.iat, claims.iat);
        }

        // Property (b): a token signed by a NON-matching key never
        // verifies under the configured public key.
        #[test]
        fn non_matching_key_never_verifies(claims in arb_claims()) {
            let other = MachineTokenSigner::from_pkcs8_pem(&SecretString::new(
                OTHER_PRIV_PEM.to_string().into(),
            ))
            .expect("build other signer");
            let jwt = other.mint(&claims).expect("mint with other key");
            prop_assert!(verifier().verify(jwt.expose_secret()).is_err());
        }
    }

    #[test]
    fn expired_token_is_rejected() {
        let iat = now_unix() - 7200;
        let claims = MachineTokenClaims {
            sub: uuid::Uuid::now_v7(),
            jti: uuid::Uuid::now_v7(),
            scope: None,
            iat,
            exp: iat + 3600, // expired one hour ago
        };
        let jwt = signer().mint(&claims).expect("mint expired");
        assert!(verifier().verify(jwt.expose_secret()).is_err());
    }

    #[test]
    fn non_eddsa_algorithm_is_rejected() {
        // Forge a token with HS256 (the classic public-key-as-HMAC-secret
        // alg-confusion attack). The EdDSA-pinned Validation must reject it.
        use jsonwebtoken::{encode, EncodingKey, Header};
        let claims = MachineTokenClaims {
            sub: uuid::Uuid::now_v7(),
            jti: uuid::Uuid::now_v7(),
            scope: None,
            iat: now_unix(),
            exp: now_unix() + 3600,
        };
        let header = Header::new(jsonwebtoken::Algorithm::HS256);
        let forged = encode(&header, &claims, &EncodingKey::from_secret(b"sekrit"))
            .expect("forge HS256 token");
        assert!(verifier().verify(&forged).is_err());
    }

    #[test]
    fn alg_none_is_rejected() {
        // Hand-craft an `alg: none` token (header.payload. with empty sig).
        use base64::Engine;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let header = b64.encode(br#"{"alg":"none","typ":"JWT"}"#);
        let payload = b64.encode(
            format!(
                r#"{{"sub":"{}","jti":"{}","scope":null,"iat":{},"exp":{}}}"#,
                uuid::Uuid::now_v7(),
                uuid::Uuid::now_v7(),
                now_unix(),
                now_unix() + 3600
            )
            .as_bytes(),
        );
        let none_token = format!("{header}.{payload}.");
        assert!(verifier().verify(&none_token).is_err());
    }

    #[test]
    fn self_test_round_trips_matched_keypair() {
        // Earned-Trust key probe: signer+verifier built from a MATCHED
        // keypair must self-test OK.
        assert!(verifier().self_test(&signer()).is_ok());
    }

    #[test]
    fn self_test_rejects_mismatched_keypair() {
        // A verifier whose public keys do NOT correspond to the signer's
        // private key must fail the self-test (refuse startup upstream).
        let other_signer = MachineTokenSigner::from_pkcs8_pem(&SecretString::new(
            OTHER_PRIV_PEM.to_string().into(),
        ))
        .expect("build other signer");
        assert!(verifier().self_test(&other_signer).is_err());
    }

    #[test]
    fn malformed_public_key_is_refused() {
        assert!(MachineTokenVerifier::from_public_keys(&["not a pem".to_string()]).is_err());
    }
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
