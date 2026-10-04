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
    /// Realm role whose holders get a foundry account on their first sign-in
    /// (`FOUNDRY_OIDC_PROVISION_ROLE`). `None` = link-only: an identity with no
    /// foundry account is refused, exactly as before provisioning existed.
    pub provision_role: Option<String>,
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
        .map(|cfg| cfg.map(|c| c.with_provision_role(read("FOUNDRY_OIDC_PROVISION_ROLE"))))
    }

    /// Opt in to role-gated provisioning. Blank means off: an operator who sets
    /// the variable to `""` gets today's link-only behaviour, never a gate that
    /// matches a role literally named "".
    pub fn with_provision_role(mut self, role: Option<String>) -> Self {
        self.provision_role = role.map(|r| r.trim().to_string()).filter(|r| !r.is_empty());
        self
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
            provision_role: None,
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

    /// Every field of an issued challenge is a fresh, non-empty random value.
    /// One with an empty field was never issued by [`Self::generate`].
    fn ensure_issued(&self) -> Result<(), OidcError> {
        if self.state.is_empty() || self.nonce.is_empty() || self.code_verifier.is_empty() {
            return Err(OidcError::Untrusted(
                "the sign-in answers no challenge we issued".to_string(),
            ));
        }
        Ok(())
    }

    /// S256 PKCE challenge for [`Self::code_verifier`].
    pub fn code_challenge(&self) -> String {
        let digest = Sha256::digest(self.code_verifier.as_bytes());
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
    }
}

/// OIDC Core §3.1.3.7(11): a nonce was sent, so the ID token's nonce claim must
/// be present AND equal. Two empty values are not a match.
fn ensure_nonce(expected: &str, presented: &str) -> Result<(), OidcError> {
    if expected.is_empty() || presented.is_empty() || presented != expected {
        return Err(OidcError::Untrusted(
            "the identity answers a challenge we did not issue".to_string(),
        ));
    }
    Ok(())
}

fn random_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// What foundry actually needs from a validated identity.
///
/// Claims only, never decisions (DDD-14): whether an identity may be provisioned
/// is foundry-app's call. The roles and names default to empty, so a token that
/// carries none of them validates exactly as it did before provisioning existed.
#[derive(Clone, Debug)]
pub struct IdentityClaims {
    pub subject: String,
    pub email: String,
    pub email_verified: bool,
    /// Keycloak realm roles (`realm_access.roles`). Client roles
    /// (`resource_access.*.roles`) are deliberately NOT read (OD-9).
    pub realm_roles: Vec<String>,
    /// The `name` profile claim, verbatim.
    pub name: Option<String>,
    /// The `preferred_username` profile claim, verbatim.
    pub preferred_username: Option<String>,
}

impl IdentityClaims {
    fn from_id_token(claims: IdTokenClaims) -> Self {
        Self {
            subject: claims.sub,
            email: claims.email,
            email_verified: claims.email_verified,
            realm_roles: claims.realm_access.map(|r| r.roles).unwrap_or_default(),
            name: claims.name,
            preferred_username: claims.preferred_username,
        }
    }

    /// Does the identity hold this realm role? Exact, case-sensitive (OD-9).
    pub fn has_realm_role(&self, role: &str) -> bool {
        self.realm_roles.iter().any(|held| held == role)
    }
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
    #[serde(default)]
    realm_access: Option<RealmAccess>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    preferred_username: Option<String>,
}

/// Keycloak's realm-role mapper shape: `"realm_access": {"roles": [...]}`.
#[derive(Deserialize)]
struct RealmAccess {
    #[serde(default)]
    roles: Vec<String>,
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

    /// The realm role that opts an unknown identity into provisioning; `None`
    /// means link-only. Read-only: foundry-app decides, this crate only carries it.
    pub fn provision_role(&self) -> Option<&str> {
        self.config.provision_role.as_deref()
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
    /// cooperates. The `nonce` check below is a second layer, but not an
    /// independent one: the expected nonce comes from the same challenge cookie
    /// that carries `state`. So both sides of it must be present — a challenge
    /// with an empty field is refused before any provider call, and an identity
    /// whose nonce is absent or different answers no challenge we issued.
    pub async fn exchange_code(
        &self,
        code: &str,
        req: &AuthRequest,
    ) -> Result<IdentityClaims, OidcError> {
        req.ensure_issued()?;
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
        ensure_nonce(&req.nonce, &claims.nonce)?;
        if claims.email.trim().is_empty() {
            return Err(OidcError::Untrusted(
                "the identity carries no email".to_string(),
            ));
        }
        Ok(IdentityClaims::from_id_token(claims))
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

    /// The identity an ID-token payload of this shape yields.
    fn identity_from(payload: serde_json::Value) -> IdentityClaims {
        let claims: IdTokenClaims =
            serde_json::from_value(payload).expect("payload deserialises as ID-token claims");
        IdentityClaims::from_id_token(claims)
    }

    fn keycloak_payload() -> serde_json::Value {
        serde_json::json!({
            "sub": "kc-user-1",
            "nonce": "n",
            "email": "ada@example.com",
            "email_verified": true,
            "realm_access": { "roles": ["foundry-member", "offline_access"] },
            "resource_access": { "foundry": { "roles": ["foundry-admin"] } },
            "name": "Ada Lovelace",
            "preferred_username": "ada",
        })
    }

    #[test]
    fn a_keycloak_token_exposes_its_realm_roles_and_profile_names() {
        let id = identity_from(keycloak_payload());
        assert_eq!(id.realm_roles, ["foundry-member", "offline_access"]);
        assert_eq!(id.name.as_deref(), Some("Ada Lovelace"));
        assert_eq!(id.preferred_username.as_deref(), Some("ada"));
        assert_eq!(id.subject, "kc-user-1");
        assert_eq!(id.email, "ada@example.com");
        assert!(id.email_verified);
    }

    #[test]
    fn a_token_without_roles_or_names_reads_exactly_as_before() {
        for payload in [
            serde_json::json!({ "sub": "s", "email": "a@example.com", "email_verified": true }),
            serde_json::json!({
                "sub": "s", "email": "a@example.com", "email_verified": true,
                "realm_access": null, "name": null, "preferred_username": null,
            }),
            serde_json::json!({
                "sub": "s", "email": "a@example.com", "email_verified": true,
                "realm_access": {},
            }),
        ] {
            let id = identity_from(payload.clone());
            assert!(id.realm_roles.is_empty(), "{payload}");
            assert_eq!(id.name, None, "{payload}");
            assert_eq!(id.preferred_username, None, "{payload}");
            assert_eq!(id.email, "a@example.com", "{payload}");
        }
    }

    #[test]
    fn only_an_exact_realm_role_counts_never_a_client_role() {
        let id = identity_from(keycloak_payload());
        assert!(id.has_realm_role("foundry-member"));
        for not_held in [
            "foundry-admin", // a client role (resource_access) never counts
            "Foundry-Member",
            "FOUNDRY-MEMBER",
            "foundry-member ",
            "foundry",
            "",
        ] {
            assert!(!id.has_realm_role(not_held), "{not_held:?} must not match");
        }
    }

    fn provider_with_provision_role(role: Option<&str>) -> OidcProvider {
        let cfg = OidcConfig::from_parts(
            some("https://kc.example/realms/x"),
            some("foundry"),
            some("s3cret"),
            some("https://foundry.example/auth/oidc/callback"),
        )
        .expect("complete config is Ok")
        .expect("complete config is Some")
        .with_provision_role(role.map(str::to_string));
        OidcProvider::new(cfg).expect("provider builds")
    }

    #[test]
    fn a_blank_or_unset_provision_role_means_provisioning_off() {
        for role in [None, Some(""), Some("   "), Some("\t\n")] {
            assert_eq!(
                provider_with_provision_role(role).provision_role(),
                None,
                "{role:?} must leave provisioning off"
            );
        }
    }

    /// A provider whose issuer refuses every connection: any request that reaches
    /// the network comes back as `Transport`, never `Untrusted`.
    fn provider_at_a_dead_issuer() -> OidcProvider {
        let cfg = OidcConfig::from_parts(
            some("http://127.0.0.1:9/realms/x"),
            some("foundry"),
            some("s3cret"),
            some("https://foundry.example/auth/oidc/callback"),
        )
        .expect("complete config is Ok")
        .expect("complete config is Some");
        OidcProvider::new(cfg).expect("provider builds")
    }

    #[tokio::test]
    async fn an_unissued_challenge_is_refused_before_the_exchange() {
        let provider = provider_at_a_dead_issuer();
        let issued = AuthRequest::generate();
        let unissued = [
            (
                "state",
                AuthRequest {
                    state: String::new(),
                    ..issued.clone()
                },
            ),
            (
                "nonce",
                AuthRequest {
                    nonce: String::new(),
                    ..issued.clone()
                },
            ),
            (
                "code_verifier",
                AuthRequest {
                    code_verifier: String::new(),
                    ..issued.clone()
                },
            ),
        ];
        for (blank, req) in unissued {
            let outcome = provider.exchange_code("a-code", &req).await;
            assert!(
                matches!(outcome, Err(OidcError::Untrusted(_))),
                "an empty {blank} must be refused as untrusted before any provider call, got {outcome:?}"
            );
        }
    }

    #[test]
    fn only_a_present_and_matching_nonce_is_answered() {
        for (expected, presented, answered) in [
            ("", "", false),
            ("n", "", false),
            ("", "n", false),
            ("n", "m", false),
            ("n", "n", true),
        ] {
            assert_eq!(
                ensure_nonce(expected, presented).is_ok(),
                answered,
                "expected {expected:?}, presented {presented:?}"
            );
        }
    }

    #[test]
    fn an_identity_with_no_nonce_claim_answers_no_challenge() {
        let mut payload = keycloak_payload();
        payload.as_object_mut().expect("object").remove("nonce");
        let claims: IdTokenClaims =
            serde_json::from_value(payload).expect("a missing nonce still deserialises");
        let issued = AuthRequest::generate();
        assert!(matches!(
            ensure_nonce(&issued.nonce, &claims.nonce),
            Err(OidcError::Untrusted(_))
        ));
    }

    #[test]
    fn a_provision_role_is_carried_through_the_provider() {
        assert_eq!(
            provider_with_provision_role(Some("foundry-member")).provision_role(),
            Some("foundry-member")
        );
    }
}
