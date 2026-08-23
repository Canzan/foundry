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

/// The fixed `iss` (issuer) claim for the single-issuer Feature-A case
/// (auth.md §Claims). Pinned at mint time and enforced at verify time.
pub const MACHINE_TOKEN_ISS: &str = "foundry";

/// The fixed `aud` (audience) claim for the single-issuer Feature-A case
/// (auth.md §Claims). Pinned at mint time and enforced at verify time.
pub const MACHINE_TOKEN_AUD: &str = "foundry-api";

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
    /// Issuer — pinned to [`MACHINE_TOKEN_ISS`] for the single-issuer Feature-A
    /// case (auth.md §Claims). Defaults to the constant so callers that build
    /// claims need not repeat it; [`MachineTokenSigner::mint`] always stamps it
    /// and [`MachineTokenVerifier`] enforces it.
    #[serde(default = "default_iss")]
    pub iss: String,
    /// Audience — pinned to [`MACHINE_TOKEN_AUD`] (auth.md §Claims). Same
    /// defaulting/enforcement posture as [`MachineTokenClaims::iss`].
    #[serde(default = "default_aud")]
    pub aud: String,
}

fn default_iss() -> String {
    MACHINE_TOKEN_ISS.to_string()
}

fn default_aud() -> String {
    MACHINE_TOKEN_AUD.to_string()
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
    /// The `iss`/`aud` registered claims are always stamped to the pinned
    /// single-issuer constants ([`MACHINE_TOKEN_ISS`]/[`MACHINE_TOKEN_AUD`]),
    /// regardless of what the caller passed, so the minting path can never
    /// produce a token the verifier will reject on iss/aud.
    pub fn mint(&self, claims: &MachineTokenClaims) -> Result<SecretString, AuthError> {
        let header = Header::new(JwtAlgorithm::EdDSA);
        let claims = MachineTokenClaims {
            iss: MACHINE_TOKEN_ISS.to_string(),
            aud: MACHINE_TOKEN_AUD.to_string(),
            ..claims.clone()
        };
        let jwt = encode(&header, &claims, &self.encoding_key)
            .map_err(|_| AuthError::MachineTokenMint)?;
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
        // `exp` is validated by default. `nbf` defaults to OFF in jsonwebtoken
        // 9.x — turn it on so a not-yet-valid token is refused (defense-in-
        // depth, FIX 2). Pin the single-issuer `iss`/`aud` to the Feature-A
        // constants (auth.md §Claims, FIX 1) so a validly-signed token with the
        // wrong issuer or audience is rejected before any registry lookup.
        validation.validate_exp = true;
        validation.validate_nbf = true;
        validation.set_issuer(&[MACHINE_TOKEN_ISS]);
        validation.set_audience(&[MACHINE_TOKEN_AUD]);
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
            iss: MACHINE_TOKEN_ISS.to_string(),
            aud: MACHINE_TOKEN_AUD.to_string(),
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

/// A signed, self-contained unsubscribe token (recipient-notification-preferences,
/// ADR-001). Unlike [`InviteToken`] it binds NO database row and has NO expiry: the
/// signature covers a domain-separated, versioned payload over `(email_lower,
/// workspace_id)` only, so the link works logged-out and indefinitely. Built on the
/// reused constant-time [`sign`] / [`verify`] primitives keyed on `SESSION_SECRET`.
/// The `unsub` prefix is domain separation — an [`InviteToken`] signature can never
/// be replayed as an unsubscribe, and vice-versa; `v1` is a rotation seam.
pub struct UnsubscribeToken {
    pub signature: String,
}

impl UnsubscribeToken {
    /// Sign a new unsubscribe token binding `(email_lower, workspace_id)`. `secret`
    /// is the application's `SESSION_SECRET`.
    pub fn new(
        email_lower: &str,
        workspace_id: uuid::Uuid,
        secret: &SecretString,
    ) -> Result<Self, AuthError> {
        let payload = unsubscribe_payload(email_lower, workspace_id);
        let signature = sign(secret, payload.as_bytes())?;
        Ok(Self { signature })
    }

    /// Verify a signature against the `(email_lower, workspace_id)` pair (constant
    /// time, inherited from [`verify`]). Any failure → the uniform refusal upstream.
    pub fn verify(
        email_lower: &str,
        workspace_id: uuid::Uuid,
        signature: &str,
        secret: &SecretString,
    ) -> Result<(), AuthError> {
        let payload = unsubscribe_payload(email_lower, workspace_id);
        verify(secret, payload.as_bytes(), signature)
    }
}

/// The domain-separated, versioned unsubscribe payload: `unsub|v1|{email_lower}|{workspace_id}`.
fn unsubscribe_payload(email_lower: &str, workspace_id: uuid::Uuid) -> String {
    format!("unsub|v1|{email_lower}|{workspace_id}")
}

/// The minimum password length (ADR-004 / NFR-4, NIST 800-63B length-first).
/// Operator-lowered 2026-08-22 from the original min-12 to min-6 (homelab
/// convenience; single-operator instance). The length-first shape (no
/// composition rules) is unchanged.
pub const MIN_PASSWORD_LENGTH: usize = 6;

/// Why a password failed [`check_password_policy`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PolicyError {
    #[error("password must be at least {min} characters")]
    TooShort { min: usize },
}

/// The app-wide password-strength policy (ADR-004): length-first, minimum 12
/// characters, no composition rule. Pure (no I/O), the single source of the
/// policy — the accept POST is its first caller; bootstrap / sign-up / reset can
/// import it unchanged. Returns `Err(PolicyError::TooShort)` below the minimum.
pub fn check_password_policy(password: &SecretString) -> Result<(), PolicyError> {
    // Length-first (NIST 800-63B): count Unicode scalar values, not bytes, so a
    // 12-character password of multi-byte characters is not mis-rejected.
    if password.expose_secret().chars().count() < MIN_PASSWORD_LENGTH {
        return Err(PolicyError::TooShort {
            min: MIN_PASSWORD_LENGTH,
        });
    }
    Ok(())
}

#[cfg(test)]
mod password_policy_tests {
    use super::*;

    /// ADR-004 / NFR-4 — the length-first policy boundary (min-6 since the
    /// 2026-08-22 operator lowering). A pure driving-port test on
    /// `check_password_policy`: passwords of exactly the minimum length (6)
    /// and longer are accepted; anything shorter is rejected with
    /// `PolicyError::TooShort`. The boundary is the observable contract the
    /// accept POST enforces BEFORE opening the consume TX.
    #[test]
    fn enforces_min_six_length_boundary() {
        for len in [0_usize, 1, 5] {
            let pwd = SecretString::new("a".repeat(len).into());
            assert!(
                matches!(
                    check_password_policy(&pwd),
                    Err(PolicyError::TooShort { min: 6 })
                ),
                "a {len}-char password must be rejected as too short (min 6)"
            );
        }
        for len in [6_usize, 7, 12, 64] {
            let pwd = SecretString::new("a".repeat(len).into());
            assert!(
                check_password_policy(&pwd).is_ok(),
                "a {len}-char password (>= 6) must satisfy the policy"
            );
        }
    }
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
                    iss: MACHINE_TOKEN_ISS.to_string(),
                    aud: MACHINE_TOKEN_AUD.to_string(),
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
            iss: MACHINE_TOKEN_ISS.to_string(),
            aud: MACHINE_TOKEN_AUD.to_string(),
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
            iss: MACHINE_TOKEN_ISS.to_string(),
            aud: MACHINE_TOKEN_AUD.to_string(),
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

    // FIX 1 (auth.md §Claims): the single-issuer Feature-A contract pins
    // `iss = MACHINE_TOKEN_ISS` and `aud = MACHINE_TOKEN_AUD`. A validly-signed,
    // unexpired token whose `iss` OR `aud` does not match the pinned constants
    // MUST be refused. We forge such tokens with the MATCHING signing key (so the
    // signature is genuine) but a wrong registered claim, proving rejection is on
    // the claim, not the signature/alg/exp.
    fn signed_with_claims(raw_iss: &str, raw_aud: &str) -> String {
        // Serialize a claim set carrying an arbitrary iss/aud, sign it with the
        // genuine test key so only the iss/aud check can reject it.
        #[derive(serde::Serialize)]
        struct ForgedClaims {
            sub: uuid::Uuid,
            scope: Option<uuid::Uuid>,
            iat: i64,
            exp: i64,
            jti: uuid::Uuid,
            iss: String,
            aud: String,
        }
        let now = now_unix();
        let forged = ForgedClaims {
            sub: uuid::Uuid::now_v7(),
            scope: None,
            iat: now,
            exp: now + 3600,
            jti: uuid::Uuid::now_v7(),
            iss: raw_iss.to_string(),
            aud: raw_aud.to_string(),
        };
        let key = EncodingKey::from_ed_pem(TEST_PRIV_PEM.as_bytes()).expect("ed key");
        encode(&Header::new(JwtAlgorithm::EdDSA), &forged, &key).expect("encode forged")
    }

    #[test]
    fn wrong_issuer_is_rejected() {
        let jwt = signed_with_claims("evil-issuer", MACHINE_TOKEN_AUD);
        assert!(
            verifier().verify(&jwt).is_err(),
            "a token with the wrong iss must be refused"
        );
    }

    #[test]
    fn wrong_audience_is_rejected() {
        let jwt = signed_with_claims(MACHINE_TOKEN_ISS, "some-other-api");
        assert!(
            verifier().verify(&jwt).is_err(),
            "a token with the wrong aud must be refused"
        );
    }

    #[test]
    fn correct_issuer_and_audience_verifies() {
        let jwt = signed_with_claims(MACHINE_TOKEN_ISS, MACHINE_TOKEN_AUD);
        assert!(
            verifier().verify(&jwt).is_ok(),
            "a token with the pinned iss+aud and a genuine signature must verify"
        );
    }

    // FIX 1 corollary (serde defaults): a genuinely-signed token whose JSON body
    // OMITS the `iss`/`aud` claims must still verify, because `#[serde(default)]`
    // supplies the pinned single-issuer constants on decode. If the defaulting
    // functions returned `""` or some other string, the issuer/audience check
    // would reject the token — so this exercises the default_iss/default_aud path
    // through the `verify` driving port (observable: Ok vs Err).
    #[test]
    fn token_omitting_iss_aud_verifies_via_serde_defaults() {
        #[derive(serde::Serialize)]
        struct ClaimsWithoutIssAud {
            sub: uuid::Uuid,
            scope: Option<uuid::Uuid>,
            iat: i64,
            exp: i64,
            jti: uuid::Uuid,
        }
        let now = now_unix();
        let forged = ClaimsWithoutIssAud {
            sub: uuid::Uuid::now_v7(),
            scope: None,
            iat: now,
            exp: now + 3600,
            jti: uuid::Uuid::now_v7(),
        };
        let key = EncodingKey::from_ed_pem(TEST_PRIV_PEM.as_bytes()).expect("ed key");
        let jwt =
            encode(&Header::new(JwtAlgorithm::EdDSA), &forged, &key).expect("encode no-iss/aud");
        let recovered = verifier()
            .verify(&jwt)
            .expect("token without iss/aud must verify via serde defaults");
        // The recovered claims must carry the pinned defaults, not empty strings.
        assert_eq!(recovered.iss, MACHINE_TOKEN_ISS);
        assert_eq!(recovered.aud, MACHINE_TOKEN_AUD);
    }

    // FIX 2 (defense-in-depth): a not-yet-valid token (`nbf` in the future) must
    // be refused even though it is genuinely signed and unexpired.
    #[test]
    fn not_yet_valid_token_is_rejected() {
        #[derive(serde::Serialize)]
        struct NbfClaims {
            sub: uuid::Uuid,
            scope: Option<uuid::Uuid>,
            iat: i64,
            exp: i64,
            jti: uuid::Uuid,
            iss: String,
            aud: String,
            nbf: i64,
        }
        let now = now_unix();
        let forged = NbfClaims {
            sub: uuid::Uuid::now_v7(),
            scope: None,
            iat: now,
            exp: now + 7200,
            jti: uuid::Uuid::now_v7(),
            iss: MACHINE_TOKEN_ISS.to_string(),
            aud: MACHINE_TOKEN_AUD.to_string(),
            nbf: now + 3600, // valid only an hour from now
        };
        let key = EncodingKey::from_ed_pem(TEST_PRIV_PEM.as_bytes()).expect("ed key");
        let jwt = encode(&Header::new(JwtAlgorithm::EdDSA), &forged, &key).expect("encode nbf");
        assert!(
            verifier().verify(&jwt).is_err(),
            "a token whose nbf is in the future must be refused"
        );
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

    // recipient-notification-preferences (ADR-001): the UnsubscribeToken signature
    // must round-trip AND bind to BOTH (email_lower, workspace_id). A signature
    // minted for one pair must never verify a different email, a different
    // workspace, or a tampered signature — else an attacker could redirect an
    // unsubscribe onto a different recipient or workspace under a genuine signature.
    #[test]
    fn unsubscribe_token_round_trips_and_binds_email_and_workspace() {
        let secret = SecretString::new("0123456789abcdef0123456789abcdef".to_string().into());
        let email = "sam@northwind.example";
        let ws = uuid::Uuid::now_v7();

        let tok = UnsubscribeToken::new(email, ws, &secret).unwrap();
        assert!(
            UnsubscribeToken::verify(email, ws, &tok.signature, &secret).is_ok(),
            "a freshly-minted token must verify against its own pair"
        );
        // A tampered/garbage signature is refused.
        assert!(UnsubscribeToken::verify(email, ws, "AAAA", &secret).is_err());
        // Binds to the email: a different recipient does not verify.
        assert!(
            UnsubscribeToken::verify("mallory@northwind.example", ws, &tok.signature, &secret)
                .is_err(),
            "signature must not validate a different email"
        );
        // Binds to the workspace: a different workspace does not verify.
        let other_ws = uuid::Uuid::now_v7();
        assert!(
            UnsubscribeToken::verify(email, other_ws, &tok.signature, &secret).is_err(),
            "signature must not validate a different workspace"
        );
    }

    // Domain separation (ADR-001): the `unsub|v1|…` payload prefix means an
    // InviteToken signature can never be replayed as an unsubscribe. A raw `sign`
    // over the invite payload must NOT verify as an unsubscribe for the same ids.
    #[test]
    fn unsubscribe_token_is_domain_separated_from_invite_token() {
        let secret = SecretString::new("0123456789abcdef0123456789abcdef".to_string().into());
        let ws = uuid::Uuid::now_v7();
        let email = "sam@northwind.example";
        // A signature over a NON-unsub payload (here the bare email|ws, no domain
        // prefix) must not verify as an unsubscribe token.
        let foreign = sign(&secret, format!("{email}|{ws}").as_bytes()).unwrap();
        assert!(
            UnsubscribeToken::verify(email, ws, &foreign, &secret).is_err(),
            "a signature over a differently-domained payload must not verify"
        );
    }

    // The invite signature must BIND to the (invite_id, expires_at) pair. If the
    // signed payload ignored its inputs (a constant), a signature minted for one
    // invite would validate a DIFFERENT invite — letting an attacker swap the id
    // or extend the expiry under a genuine signature. Verify the signature does
    // NOT cross-validate across a different id or a different expiry.
    #[test]
    fn invite_signature_binds_to_id_and_expiry() {
        let secret = SecretString::new("0123456789abcdef0123456789abcdef".to_string().into());
        let id_a = uuid::Uuid::now_v7();
        let id_b = uuid::Uuid::now_v7();
        let exp_a = time::OffsetDateTime::now_utc() + time::Duration::days(7);
        let exp_b = exp_a + time::Duration::days(1);

        let tok = InviteToken::new(id_a, exp_a, &secret).unwrap();

        // Same signature must NOT verify against a different invite id.
        assert!(
            InviteToken::verify(id_b, exp_a, &tok.signature, &secret).is_err(),
            "signature must not validate a different invite id"
        );
        // Same signature must NOT verify against a different expiry.
        assert!(
            InviteToken::verify(id_a, exp_b, &tok.signature, &secret).is_err(),
            "signature must not validate a different expiry"
        );
    }
}
