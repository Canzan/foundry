//! OpenID Connect relying-party protocol for foundry.
//!
//! WHY ITS OWN CRATE (ADR-OIDC-001). `xtask check-arch::check_jwt_alg_pin` scans
//! `crates/foundry-auth/src` and fails the build unless every `jsonwebtoken`
//! `Validation` there pins `algorithms` to EdDSA and nothing else. Keycloak signs
//! ID tokens RS256, so this code cannot live in foundry-auth without either
//! failing CI or weakening the pin that protects machine tokens — and
//! `pins_algorithms_to_eddsa` reads only the FIRST `algorithms` list in a file, so
//! one file-scoped rule cannot express "EdDSA here, RS256 there". Two crates, two
//! independent per-credential-class pins, enforced by `check_oidc_alg_pin` over
//! THIS directory.
//!
//! This crate is pure protocol: claims in, validated claims out. It depends on
//! neither foundry-auth nor foundry-store. Binding an identity to a `users` row
//! happens in foundry-app, where the tenancy rules already live; `deny.toml` bans
//! any other crate from depending on this one.
//!
//! Zero new runtime dependencies: `reqwest` and `jsonwebtoken` are already
//! workspace members, and `DecodingKey::from_jwk` covers JWKS→key directly.

use base64::Engine as _;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a fetched key set is trusted before it is re-fetched.
const JWKS_TTL: Duration = Duration::from_secs(15 * 60);
/// Floor between JWKS refreshes triggered by an unknown `kid`, so a stream of
/// forged tokens cannot turn the provider into our own rate-limit problem.
const JWKS_REFRESH_FLOOR: Duration = Duration::from_secs(30);
/// Budget for every outbound call. A hung provider must refuse a sign-in, not
/// hold a request handler open.
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum OidcError {
    #[error("OIDC is configured incompletely: {0}")]
    Config(String),
    #[error("the identity provider could not be reached: {0}")]
    Transport(String),
    #[error("the identity provider answered unusably: {0}")]
    Protocol(String),
    #[error("the identity could not be trusted: {0}")]
    Untrusted(String),
}

/// Shape-validated configuration. Built at boot; see [`OidcConfig::from_env`].
#[derive(Clone, Debug)]
pub struct OidcConfig {
    /// Issuer base URL, no trailing slash.
    pub issuer: String,
    pub client_id: String,
    pub client_secret: SecretString,
    /// Absolute callback URL registered with the provider.
    pub redirect_uri: String,
}

impl OidcConfig {
    /// Read the four settings from the environment.
    ///
    /// Returns `Ok(None)` when NONE are set — the feature is simply off, which is
    /// how a contributor's `run.sh` and `cargo xtask ci` run with no identity
    /// provider. Returns `Err` when SOME are set: a half-configured provider
    /// renders a sign-in control that then fails at the callback, which reads as
    /// a broken deploy rather than an unconfigured one (ADR-OIDC-003, AC-5.5).
    ///
    /// SHAPE ONLY. Nothing here touches the network: discovery and JWKS are
    /// fetched lazily, so foundry starts and serves while Keycloak is down — the
    /// exact case the retained password path exists for.
    pub fn from_env() -> Result<Option<Self>, OidcError> {
        let read = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        Self::from_parts(
            read("OIDC_ISSUER_URL"),
            read("OIDC_CLIENT_ID"),
            read("OIDC_CLIENT_SECRET"),
            read("OIDC_REDIRECT_URL"),
        )
    }

    /// The whole decision, as a pure function.
    ///
    /// `from_env` is a thin adapter over this so the rules can be tested without
    /// touching the process environment. That is not tidiness: `std::env::set_var`
    /// is process-global while Rust runs tests in parallel threads, so env-based
    /// tests race each other and fail for reasons unrelated to the rule under
    /// test. Keeping the logic pure removes the shared mutable state entirely.
    pub fn from_parts(
        issuer: Option<String>,
        client_id: Option<String>,
        client_secret: Option<String>,
        redirect_uri: Option<String>,
    ) -> Result<Option<Self>, OidcError> {
        let named = [
            ("OIDC_ISSUER_URL", &issuer),
            ("OIDC_CLIENT_ID", &client_id),
            ("OIDC_CLIENT_SECRET", &client_secret),
            ("OIDC_REDIRECT_URL", &redirect_uri),
        ];
        let present = named.iter().filter(|(_, v)| v.is_some()).count();
        if present == 0 {
            return Ok(None);
        }
        if present < named.len() {
            let missing: Vec<&str> = named
                .iter()
                .filter(|(_, v)| v.is_none())
                .map(|(n, _)| *n)
                .collect();
            return Err(OidcError::Config(format!("missing {}", missing.join(", "))));
        }

        let issuer = issuer
            .expect("checked present")
            .trim_end_matches('/')
            .to_string();
        let redirect_uri = redirect_uri.expect("checked present");
        for (name, value) in [
            ("OIDC_ISSUER_URL", &issuer),
            ("OIDC_REDIRECT_URL", &redirect_uri),
        ] {
            if !(value.starts_with("https://") || value.starts_with("http://")) {
                return Err(OidcError::Config(format!(
                    "{name} must be an absolute http(s) URL"
                )));
            }
        }

        Ok(Some(Self {
            issuer,
            client_id: client_id.expect("checked present"),
            client_secret: SecretString::new(client_secret.expect("checked present").into()),
            redirect_uri,
        }))
    }
}

/// The one-time secrets of a single sign-in attempt. Minted at `/auth/oidc/start`,
/// carried in a signed cookie, and compared at the callback.
#[derive(Clone, Debug)]
pub struct AuthRequest {
    pub state: String,
    pub nonce: String,
    pub code_verifier: String,
}

impl AuthRequest {
    pub fn generate() -> Self {
        Self {
            state: random_token(),
            nonce: random_token(),
            code_verifier: random_token(),
        }
    }

    /// S256 PKCE challenge for [`Self::code_verifier`].
    pub fn code_challenge(&self) -> String {
        let digest = Sha256::digest(self.code_verifier.as_bytes());
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
    }
}

fn random_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// What foundry actually needs from a validated identity.
#[derive(Clone, Debug)]
pub struct IdentityClaims {
    pub subject: String,
    pub email: String,
    pub email_verified: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    id_token: String,
}

#[derive(Deserialize)]
struct IdTokenClaims {
    sub: String,
    #[serde(default)]
    nonce: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    email_verified: bool,
}

struct CachedJwks {
    keys: jsonwebtoken::jwk::JwkSet,
    fetched_at: Instant,
}

pub struct OidcProvider {
    config: OidcConfig,
    http: reqwest::Client,
    discovery: Mutex<Option<Discovery>>,
    jwks: Mutex<Option<CachedJwks>>,
}

impl std::fmt::Debug for OidcProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcProvider")
            .field("issuer", &self.config.issuer)
            .field("client_id", &self.config.client_id)
            .finish_non_exhaustive()
    }
}

impl OidcProvider {
    pub fn new(config: OidcConfig) -> Result<Self, OidcError> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|e| OidcError::Config(format!("could not build an HTTP client: {e}")))?;
        Ok(Self {
            config,
            http,
            discovery: Mutex::new(None),
            jwks: Mutex::new(None),
        })
    }

    pub fn config(&self) -> &OidcConfig {
        &self.config
    }

    async fn discovery(&self) -> Result<Discovery, OidcError> {
        if let Some(d) = self.discovery.lock().expect("discovery lock").clone() {
            return Ok(d);
        }
        let url = format!("{}/.well-known/openid-configuration", self.config.issuer);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| OidcError::Transport(format!("discovery: {e}")))?;
        if !resp.status().is_success() {
            return Err(OidcError::Protocol(format!(
                "discovery returned {}",
                resp.status()
            )));
        }
        let d: Discovery = resp
            .json()
            .await
            .map_err(|e| OidcError::Protocol(format!("discovery body: {e}")))?;
        if d.issuer.trim_end_matches('/') != self.config.issuer {
            return Err(OidcError::Untrusted(format!(
                "discovery names issuer {:?} but we are configured for {:?}",
                d.issuer, self.config.issuer
            )));
        }
        *self.discovery.lock().expect("discovery lock") = Some(d.clone());
        Ok(d)
    }

    /// Where to send the browser to authenticate.
    pub async fn authorization_url(&self, req: &AuthRequest) -> Result<String, OidcError> {
        let d = self.discovery().await?;
        let sep = if d.authorization_endpoint.contains('?') {
            '&'
        } else {
            '?'
        };
        Ok(format!(
            "{}{sep}response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&nonce={}&code_challenge={}&code_challenge_method=S256",
            d.authorization_endpoint,
            urlencode(&self.config.client_id),
            urlencode(&self.config.redirect_uri),
            urlencode("openid email profile"),
            urlencode(&req.state),
            urlencode(&req.nonce),
            urlencode(&req.code_challenge()),
        ))
    }

    async fn jwks(&self, force_refresh: bool) -> Result<jsonwebtoken::jwk::JwkSet, OidcError> {
        {
            let guard = self.jwks.lock().expect("jwks lock");
            if let Some(c) = guard.as_ref() {
                let fresh = c.fetched_at.elapsed() < JWKS_TTL;
                let too_soon = c.fetched_at.elapsed() < JWKS_REFRESH_FLOOR;
                if (fresh && !force_refresh) || (force_refresh && too_soon) {
                    return Ok(c.keys.clone());
                }
            }
        }
        let d = self.discovery().await?;
        let resp = self
            .http
            .get(&d.jwks_uri)
            .send()
            .await
            .map_err(|e| OidcError::Transport(format!("jwks: {e}")))?;
        if !resp.status().is_success() {
            return Err(OidcError::Protocol(format!(
                "jwks returned {}",
                resp.status()
            )));
        }
        let keys: jsonwebtoken::jwk::JwkSet = resp
            .json()
            .await
            .map_err(|e| OidcError::Protocol(format!("jwks body: {e}")))?;
        *self.jwks.lock().expect("jwks lock") = Some(CachedJwks {
            keys: keys.clone(),
            fetched_at: Instant::now(),
        });
        Ok(keys)
    }

    /// Exchange the authorization code and return the identity it vouches for.
    ///
    /// The code is single-use AT the provider, which is what actually refuses a
    /// replayed callback — clearing our own cookie only helps if the client
    /// cooperates. The `nonce` comparison below is the independent second layer.
    pub async fn exchange_code(
        &self,
        code: &str,
        req: &AuthRequest,
    ) -> Result<IdentityClaims, OidcError> {
        let d = self.discovery().await?;
        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", self.config.redirect_uri.as_str()),
            ("client_id", self.config.client_id.as_str()),
            ("client_secret", self.config.client_secret.expose_secret()),
            ("code_verifier", req.code_verifier.as_str()),
        ];
        let resp = self
            .http
            .post(&d.token_endpoint)
            .form(&params)
            .send()
            .await
            .map_err(|e| OidcError::Transport(format!("token exchange: {e}")))?;
        if !resp.status().is_success() {
            return Err(OidcError::Untrusted(format!(
                "token endpoint returned {} (a replayed or expired code looks like this)",
                resp.status()
            )));
        }
        let body: TokenResponse = resp
            .json()
            .await
            .map_err(|e| OidcError::Protocol(format!("token body: {e}")))?;

        let claims = self.validate_id_token(&body.id_token).await?;
        if claims.nonce != req.nonce {
            return Err(OidcError::Untrusted(
                "the identity answers a challenge we did not issue".to_string(),
            ));
        }
        if claims.email.trim().is_empty() {
            return Err(OidcError::Untrusted(
                "the identity carries no email".to_string(),
            ));
        }
        Ok(IdentityClaims {
            subject: claims.sub,
            email: claims.email,
            email_verified: claims.email_verified,
        })
    }

    async fn validate_id_token(&self, token: &str) -> Result<IdTokenClaims, OidcError> {
        let header = jsonwebtoken::decode_header(token)
            .map_err(|e| OidcError::Untrusted(format!("unreadable token header: {e}")))?;
        let kid = header
            .kid
            .ok_or_else(|| OidcError::Untrusted("token names no signing key".to_string()))?;

        // An unknown kid is the normal signal that the provider rotated keys, so
        // refresh once (rate-limited) before refusing.
        let mut set = self.jwks(false).await?;
        if set.find(&kid).is_none() {
            set = self.jwks(true).await?;
        }
        let jwk = set
            .find(&kid)
            .ok_or_else(|| OidcError::Untrusted(format!("no published key for kid {kid}")))?;
        let key = jsonwebtoken::DecodingKey::from_jwk(jwk)
            .map_err(|e| OidcError::Protocol(format!("published key unusable: {e}")))?;

        // ALGORITHM PIN. RS256 and nothing else — `check_oidc_alg_pin` fails the
        // build if any other algorithm token appears in this list, which is what
        // keeps the alg-confusion footgun a loud build error instead of a silent
        // authentication of the wrong person.
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.algorithms = vec![jsonwebtoken::Algorithm::RS256];
        validation.set_audience(&[self.config.client_id.as_str()]);
        validation.set_issuer(&[self.config.issuer.as_str()]);
        validation.validate_exp = true;

        jsonwebtoken::decode::<IdTokenClaims>(token, &key, &validation)
            .map(|data| data.claims)
            .map_err(|e| OidcError::Untrusted(format!("token rejected: {e}")))
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some(v: &str) -> Option<String> {
        Some(v.to_string())
    }

    #[test]
    fn absent_config_is_off_not_an_error() {
        assert!(OidcConfig::from_parts(None, None, None, None)
            .expect("absent is Ok")
            .is_none());
    }

    #[test]
    fn partial_config_is_refused_and_names_what_is_missing() {
        let err = OidcConfig::from_parts(some("https://kc.example/realms/x"), None, None, None)
            .expect_err("partial config must refuse");
        let msg = err.to_string();
        for expected in ["OIDC_CLIENT_ID", "OIDC_CLIENT_SECRET", "OIDC_REDIRECT_URL"] {
            assert!(msg.contains(expected), "should name {expected}: {msg}");
        }
    }

    #[test]
    fn a_relative_issuer_is_refused() {
        let err = OidcConfig::from_parts(
            some("kc.example/realms/x"),
            some("foundry"),
            some("s3cret"),
            some("https://foundry.example/auth/oidc/callback"),
        )
        .expect_err("a relative issuer must be refused");
        assert!(err.to_string().contains("absolute"), "{err}");
    }

    #[test]
    fn a_complete_config_normalises_the_trailing_slash() {
        let cfg = OidcConfig::from_parts(
            some("https://kc.example/realms/x/"),
            some("foundry"),
            some("s3cret"),
            some("https://foundry.example/auth/oidc/callback"),
        )
        .expect("complete config is Ok")
        .expect("complete config is Some");
        assert_eq!(cfg.issuer, "https://kc.example/realms/x");
    }

    #[test]
    fn pkce_challenge_is_the_s256_of_the_verifier() {
        let req = AuthRequest::generate();
        let expected = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(Sha256::digest(req.code_verifier.as_bytes()));
        assert_eq!(req.code_challenge(), expected);
        assert_ne!(req.state, req.nonce, "state and nonce must be independent");
    }

    #[test]
    fn two_attempts_never_share_a_challenge() {
        let a = AuthRequest::generate();
        let b = AuthRequest::generate();
        assert_ne!(a.state, b.state);
        assert_ne!(a.nonce, b.nonce);
        assert_ne!(a.code_verifier, b.code_verifier);
    }
}
