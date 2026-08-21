//! In-process OpenID Connect provider double for the `keycloak-sso` feature.
//!
//! Extension Justification (quality-framework Mandate against Parallel
//! Implementations):
//!   WHY-NEW-FILE: crates/foundry-acceptance/src/support/oidc_issuer.rs
//!   CLOSEST-EXISTING: crates/foundry-acceptance/src/support/webhook_receiver.rs
//!     (the other in-process HTTP double standing in for an external party)
//!   EXTENSION-COST: webhook_receiver is a RECEIVER — it records inbound POSTs and
//!     asserts nothing about what it returns. This double is a RESPONDER that must
//!     mint correctly RS256-signed identities per request, publish a JWKS, and vary
//!     its answers per scenario (unpublished key, foreign issuer, lapsed validity,
//!     stale challenge). Folding a signing fixture and a discovery/JWKS/token
//!     surface into a request recorder would give one type two unrelated jobs.
//!   PARALLEL-RATIONALE: the production `foundry-oidc` client fetches discovery and
//!     JWKS and POSTs to the token endpoint over real `reqwest`, so the double must
//!     be a real socket serving real documents — the wire IS the behaviour under
//!     test. A canned-response stub (`wiremock`) cannot serve a COMPUTED, correctly
//!     signed identity, which is the whole point (DISTILL, OQ-2).
//!
//! Per the DISTILL harness boundary: external transports are IN-PROCESS TEST
//! DOUBLES — no request leaves the test process. This provider is a local axum
//! server bound on `127.0.0.1:0`. The RS256 crypto is REAL; only the key MATERIAL is
//! a fixture, mirroring the shipped machine-token test keypair.
//!
//! The real Keycloak is exercised by slice 03's cluster e2e, not here.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

/// `kid` the double publishes and signs with.
pub const SIGNING_KID: &str = "kc-test-1";

/// PEM of the key the double publishes in its JWKS (`kid = "kc-test-1"`).
pub const SIGNING_KEY_PEM: &str = "\
-----BEGIN PRIVATE KEY-----\n\
MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQDOkLdgBnwBgSh3\n\
AI7xU4eea4AVMH7oKq3Zh0vyxn0uPsEsbfERW+xpKj2419npvf/S5Jri1K/agrQn\n\
+CE8JAPRbj3RW7bedy7lGdyuoyAxzhmInFGymZR0gehfcpLeJppYq+EA6P9ZesRW\n\
KrUFv6XOLZEvWYJd8fZCebXkteB02Dk8AZUGU+khr7/qGWH8wPxuyL6UV32LtkDP\n\
Q5WCaDUJqlWSUTTktlCxu25bl1fOt3oK8zybfxGktd7GETHOcKlYTF9f7zrNFZRo\n\
rubV7oNyB+kHhE5MJgoUQ8+/1xBozx4oFaXZqNFtauSqXBWtH+MjftX5/Od86YwI\n\
YSnatJpBAgMBAAECggEAERw70vnZMem1hPKi+dqcragYw7M7lovEe0k2/AzqJ8IT\n\
JRTqm/C/QDB7d4Z1BTopCO/JOaKlpLtguhKBCZbadHGwhVlBaE47MEcoYxSIO+qR\n\
wwxfGGTFfzIXN2LJnMUEp3V9+Dc/QYrxskksK1CmPBIaXYTz+h1Vs/NveEVYKpgW\n\
sxVQf8EZ5CK/MvJXIz7MKxmI8ZaEbn3J2lknGJ6r+aVILsD8EX9wC/6HGlPcE/u3\n\
jLSXVjpXKRICjzHtbY9sy2iAZ4rEnHuwbveFygWDfG2u3yWoGXyi8ZoJzYxAJq21\n\
Y2jvNfNwr5yhU0wdW28qiX8X/Kl1tUUXjvzPVNo6fQKBgQDyNJHlhoRGaLbzwWMN\n\
vssZ7bI6tJ3/PBHD/1R72tQ0kX59ZsY0ySMEj75VlCgwCI5SVP9F/U0WiN2b9FWI\n\
xEWSqGEH4QYl9WImlegedQNQgjOqvirqq0YC8tdEvYf8ISMe8I0Wu70f9Th7GXey\n\
vyoS7iUNCAMuf+89/KSIZ9bO9QKBgQDaVIDT08FM+HyljR7/6nqBIdban2w/fhor\n\
Vzi4Vun0R6ZFLHNi/ZYwCKtVF8Lrf4MofBJq3NH+F36JhdBFNA0meCYsDrPBIy+0\n\
m5GOmK8ssvLAhVEb9SCI/exCN4ci8wrC5IE5aLi2kGTDngE8XStBmHdfkSKycy8r\n\
QmEYGRA2nQKBgCeFmIEBkDgFAkWIOueVSIL0nG6j5lwtqyB2W4zSSmpBi4he6tzW\n\
LVajNgW05VHhM4gPwo/jI18X+kFmf0aP8GJcA2lLuLsc7WUqdPPzWBUdCd1EprAg\n\
Po5gnevjmXr01UxJKFybSeMbGppLr5KFSxJHtdgIhKxjx+Avh5GSkCS5AoGAUWqH\n\
q/ZgNArJuJaag8Z1rmfnDhm7LSYiLh1VenB2x/BcEZmU4co80ma5NX8p4dXoHBXA\n\
bHcyG7W5KyFqXBQf/0N4wJ8u6wvrA0esDOflEx8cJSzR5UIQwuUl0D+Stja5wZmi\n\
krz5fKL14Hiwb0kzEz/+6/VcYf1QDqqvOGRIRoUCgYB3DPgJZ5v5lzURjAnjSHk9\n\
uGZiZWco3EiuTY63Jfawfz0J+JH05OyOVgakl1ySEVUA3TT4+flye8DHxVtW5cL1\n\
I/VumwvO49mtMcE5KahnFj5IhTGQYid+KuCwkrhF+R/8USnURh9AIv3glmy+TZcW\n\
mR7+nl4lx/XjALuCsCdM/Q==\n\
-----END PRIVATE KEY-----\n\
";

/// PEM of a well-formed key the double NEVER publishes — used to mint an
/// identity signed by a key the provider does not vouch for.
pub const ROGUE_KEY_PEM: &str = "\
-----BEGIN PRIVATE KEY-----\n\
MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQCmljGpyNxQzzyZ\n\
KmS4unwJhh3ORXhVlCX5pUZNRAq0E2e2rhAaCEgxFcLpbL6oG/9mrFqH6C2FSC7U\n\
uNWkEHOL/zZ5zJGR3s8CmFTPurAGBtsJY8stjkOy5TC0RhQBCfS2RMcHBsNUevEP\n\
/WmFyfVWvTxyjaRNwmL5Bp+crwiPbnE59XPz3tJEVVWeJ6NG+EYKkgSapyGR3WXf\n\
UFB98+G2k1ZxokeHVGZzDsZl0Vtz8xcS6R+cinFBRHcY6tdWZNjXZ5GbEJp3ysdb\n\
q6vkMxAZgRIssmj3WDneH8UReclGtdsDMzpqshIQiIfvrYdXozmBBV/TQahOxlZV\n\
VKuERKqNAgMBAAECggEAQzk/0EqQcYG3a/2aCJukezlmZLtO/IXcrzndPUfUY+ei\n\
xirGr6Kg80HbVlP+OhuWPIoXvFlaqTrAPzJZcCv9XMS/6HW/VpSJh/wN2Yf2CXCR\n\
yl+9eWQ7+ICZX14aK3Mnj7gActrtTvWPusqh3w3pFbgmoPl8kK59Pw3HsjRF6Y7a\n\
aVMv7jOLTCmZPpQrhxXMfWq6/yOIq9WpW3DaaZWJLLm3apJYKpnM4CWu511ouTL1\n\
+S3P243SNBOqidAyx7h2lBfFKTHc3glfnsqZVRjfLnox3Xn0sapbLHI6tItT6IYo\n\
wM9IgEZSfvD3acGEJBAjVkaHTPsMlmxGd+AYl0FtNwKBgQDa4bhRwAn9Rwe1Moa6\n\
r/rQy70KPz1sjuhI1yb+LVMkpOfST6luE3ps4wu0NhjCy4jOOEvsLtQQKSdGocZB\n\
do8Fq/pMUdLyuT7S5fv9J3VKMrScmfjx+LaaLLhCMl4SZxaDKcs/zxG9d5KAhOzv\n\
G/9TAK35gg+5SDPAaz1dbRzKrwKBgQDC1jKjWmCHG5BMSSKONzHFu1yqxSHCEPJS\n\
4B0RdIu01+k0dFm6hVkmXdckoE4hgHlaw+qu9woqigYT0CNHGtPBXOXEx9lgIhyt\n\
icZaC7aD58NVJ0dyv7OihXzLv717FQQm6adPx+2bg8+P+C+yaphGmX27Pm1ZL4Ud\n\
9tf9L+b9gwKBgFDR/eQ5u7aI7sCqWnM+nadRQ4kwFrcqAX077Ir4I3YpaewPPCmI\n\
CbGBGIY/X182FlrHEMmx9N3OxFDhVTpA08itWuupXvH/EsJ+50/vrPBrzqLwe6ql\n\
Qo+lKZhPzsqOxBJEcWcrR4qlRzQrYO1dcias3pB9xN6OWYWYU31W18XDAoGAIkim\n\
qG/ixGNpRMMpvXSg4XZSnAoMIqXwvfyJoOStIKlNc9l1YIjOYx3oGZ3LocGFmR8Y\n\
UKlPtKSM5TeevYhO8ptyIuo3qd7WxQKVUIr3FsbVbEp5HAv3hAWRLBkVMm9ER8Sd\n\
mEBJ4y+SenbljbOMEAA6S5R0kVj3R4qD/x1KvBkCgYABFKoN4fXaK23lFniDZxKY\n\
w5VAll7t4pM/5KqQngJPdeGDGUNzu6dGWWWLwIr2J3IPSjy3GsE4QhmE7EV3Ic29\n\
hS2GidnEw6EofWGstL0bqlv00dqq5ADnc2ddl6ZdSvVnAWkUBaHIxPrD2Lu2F0R2\n\
MEnArjSH0mfw67MluYzPuw==\n\
-----END PRIVATE KEY-----\n\
";

/// Base64url modulus of [`SIGNING_KEY_PEM`], as published in the JWKS.
pub const SIGNING_KEY_N: &str = "zpC3YAZ8AYEodwCO8VOHnmuAFTB-6Cqt2YdL8sZ9Lj7BLG3xEVvsaSo9uNfZ6b3_0uSa4tSv2oK0J_ghPCQD0W490Vu23ncu5RncrqMgMc4ZiJxRspmUdIHoX3KS3iaaWKvhAOj_WXrEViq1Bb-lzi2RL1mCXfH2Qnm15LXgdNg5PAGVBlPpIa-_6hlh_MD8bsi-lFd9i7ZAz0OVgmg1CapVklE05LZQsbtuW5dXzrd6CvM8m38RpLXexhExznCpWExfX-86zRWUaK7m1e6DcgfpB4ROTCYKFEPPv9cQaM8eKBWl2ajRbWrkqlwVrR_jI37V-fznfOmMCGEp2rSaQQ";
/// Base64url exponent (65537) of [`SIGNING_KEY_PEM`].
pub const SIGNING_KEY_E: &str = "AQAB";

/// How the double should behave for the NEXT identity it mints. One scenario sets
/// one variant; the happy path leaves [`Variant::Valid`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Variant {
    /// A correctly signed, in-date identity naming this double as issuer.
    #[default]
    Valid,
    /// Signed by a well-formed key the JWKS does not publish.
    UnpublishedKey,
    /// `iss` names a different provider.
    ForeignIssuer,
    /// `exp` is already in the past.
    Lapsed,
    /// `nonce` answers a challenge other than the one just begun.
    StaleNonce,
    /// `email_verified` is false.
    UnconfirmedEmail,
}

/// Per-scenario knobs, shared with the running server.
#[derive(Debug, Default)]
struct Inner {
    /// Subject/email the next minted identity describes.
    email: String,
    email_verified: bool,
    variant: Variant,
    /// Codes the double has already exchanged. An authorization code is
    /// single-use AT the provider — this is what actually refuses a replayed
    /// sign-in (feature-delta.md § Changed Assumptions, AC-3.5).
    spent_codes: Vec<String>,
    /// `nonce` carried by the most recent authorization request, echoed into the
    /// identity unless the scenario asks for [`Variant::StaleNonce`].
    last_nonce: Option<String>,
    /// Every token request the double received, for assertions.
    token_requests: Vec<HashMap<String, String>>,
}

#[derive(Clone)]
struct AppState2 {
    inner: Arc<Mutex<Inner>>,
    issuer: String,
    audience: String,
}

/// A bound, running provider double.
#[derive(Debug)]
pub struct OidcIssuerDouble {
    addr: SocketAddr,
    inner: Arc<Mutex<Inner>>,
    handle: tokio::task::JoinHandle<()>,
}

impl OidcIssuerDouble {
    /// Bind on an ephemeral loopback port and start serving.
    pub async fn start(audience: &str) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind oidc double");
        let addr = listener.local_addr().expect("local_addr");
        let inner = Arc::new(Mutex::new(Inner {
            email: "operator@example.test".to_string(),
            email_verified: true,
            ..Inner::default()
        }));
        let state = AppState2 {
            inner: Arc::clone(&inner),
            issuer: format!("http://{addr}"),
            audience: audience.to_string(),
        };
        let router = Router::new()
            .route("/.well-known/openid-configuration", get(discovery))
            .route("/jwks", get(jwks))
            .route("/authorize", get(authorize))
            .route("/token", post(token))
            .with_state(state);
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        Self {
            addr,
            inner,
            handle,
        }
    }

    /// Issuer URL foundry should be pointed at.
    pub fn issuer(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Describe the person the next identity will name.
    pub fn will_vouch_for(&self, email: &str, confirmed: bool) {
        let mut g = self.inner.lock().expect("lock");
        g.email = email.to_string();
        g.email_verified = confirmed;
    }

    /// Bend the next identity out of shape.
    pub fn will_mint(&self, variant: Variant) {
        self.inner.lock().expect("lock").variant = variant;
    }

    /// Token requests observed so far.
    pub fn token_request_count(&self) -> usize {
        self.inner.lock().expect("lock").token_requests.len()
    }

    /// Stop serving. Dropping the double without calling this is also fine — the
    /// task dies with the runtime.
    pub fn shutdown(&self) {
        self.handle.abort();
    }
}

impl Drop for OidcIssuerDouble {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

async fn discovery(State(st): State<AppState2>) -> Json<Value> {
    let iss = &st.issuer;
    Json(json!({
        "issuer": iss,
        "authorization_endpoint": format!("{iss}/authorize"),
        "token_endpoint": format!("{iss}/token"),
        "jwks_uri": format!("{iss}/jwks"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "code_challenge_methods_supported": ["S256"],
    }))
}

async fn jwks() -> Json<Value> {
    Json(json!({
        "keys": [{
            "kty": "RSA",
            "use": "sig",
            "alg": "RS256",
            "kid": SIGNING_KID,
            "n": SIGNING_KEY_N,
            "e": SIGNING_KEY_E,
        }]
    }))
}

/// Records the `nonce` of the authorization request so the minted identity can
/// echo it. Real Keycloak would render a login page here; the double does not,
/// because the scenarios never drive a browser through it.
async fn authorize(
    State(st): State<AppState2>,
    Query(q): Query<HashMap<String, String>>,
) -> StatusCode {
    if let Some(n) = q.get("nonce") {
        st.inner.lock().expect("lock").last_nonce = Some(n.clone());
    }
    StatusCode::OK
}

#[derive(Serialize)]
struct TokenResponse {
    access_token: String,
    id_token: String,
    token_type: &'static str,
    expires_in: u64,
}

#[derive(Serialize, Deserialize)]
struct Claims {
    iss: String,
    sub: String,
    aud: String,
    exp: i64,
    iat: i64,
    nonce: String,
    email: String,
    email_verified: bool,
}

async fn token(
    State(st): State<AppState2>,
    axum::extract::Form(form): axum::extract::Form<HashMap<String, String>>,
) -> Result<Json<TokenResponse>, StatusCode> {
    let mut g = st.inner.lock().expect("lock");
    g.token_requests.push(form.clone());

    // An authorization code is single-use AT the provider. This — not the state
    // cookie — is what refuses a replayed sign-in.
    let code = form.get("code").cloned().unwrap_or_default();
    if g.spent_codes.contains(&code) {
        return Err(StatusCode::BAD_REQUEST);
    }
    g.spent_codes.push(code);

    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let variant = g.variant;
    let nonce = match variant {
        Variant::StaleNonce => "a-challenge-nobody-issued".to_string(),
        _ => g.last_nonce.clone().unwrap_or_default(),
    };
    let claims = Claims {
        iss: match variant {
            Variant::ForeignIssuer => "https://someone-else.example".to_string(),
            _ => st.issuer.clone(),
        },
        sub: format!("sub-{}", g.email),
        aud: st.audience.clone(),
        exp: match variant {
            Variant::Lapsed => now - 3600,
            _ => now + 300,
        },
        iat: now - 5,
        nonce,
        email: g.email.clone(),
        email_verified: match variant {
            Variant::UnconfirmedEmail => false,
            _ => g.email_verified,
        },
    };
    drop(g);

    let pem = match variant {
        Variant::UnpublishedKey => ROGUE_KEY_PEM,
        _ => SIGNING_KEY_PEM,
    };
    let key = EncodingKey::from_rsa_pem(pem.as_bytes()).expect("fixture rsa pem parses");
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(SIGNING_KID.to_string());
    let id_token = jsonwebtoken::encode(&header, &claims, &key).expect("sign id token");

    Ok(Json(TokenResponse {
        access_token: "double-access-token".to_string(),
        id_token,
        token_type: "Bearer",
        expires_in: 300,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixture key material is only exercised at runtime (`from_rsa_pem`), so
    /// a typo in the embedded PEM would otherwise surface as a mid-scenario panic
    /// rather than a compile error. This pins both keypairs at unit-test speed.
    #[test]
    fn fixture_keys_parse_and_sign() {
        for pem in [SIGNING_KEY_PEM, ROGUE_KEY_PEM] {
            let key = EncodingKey::from_rsa_pem(pem.as_bytes()).expect("fixture pem parses");
            let claims = Claims {
                iss: "http://issuer.test".into(),
                sub: "sub-1".into(),
                aud: "foundry".into(),
                exp: 4_102_444_800,
                iat: 0,
                nonce: "n".into(),
                email: "a@b.test".into(),
                email_verified: true,
            };
            let mut header = Header::new(Algorithm::RS256);
            header.kid = Some(SIGNING_KID.to_string());
            let token = jsonwebtoken::encode(&header, &claims, &key).expect("sign");
            assert_eq!(token.split('.').count(), 3, "a JWS has three parts");
        }
    }

    /// The published JWKS must describe the key the double actually signs with —
    /// if these drift, every scenario fails for the wrong reason.
    #[test]
    fn published_jwks_matches_the_signing_key() {
        let key = jsonwebtoken::DecodingKey::from_rsa_components(SIGNING_KEY_N, SIGNING_KEY_E)
            .expect("published components form a usable key");
        let signing = EncodingKey::from_rsa_pem(SIGNING_KEY_PEM.as_bytes()).expect("pem");
        let claims = Claims {
            iss: "http://issuer.test".into(),
            sub: "sub-1".into(),
            aud: "foundry".into(),
            exp: 4_102_444_800,
            iat: 0,
            nonce: "n".into(),
            email: "a@b.test".into(),
            email_verified: true,
        };
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(SIGNING_KID.to_string());
        let token = jsonwebtoken::encode(&header, &claims, &signing).expect("sign");

        let mut validation = jsonwebtoken::Validation::new(Algorithm::RS256);
        validation.algorithms = vec![Algorithm::RS256];
        validation.set_audience(&["foundry"]);
        validation.set_issuer(&["http://issuer.test"]);
        jsonwebtoken::decode::<Claims>(&token, &key, &validation)
            .expect("the published JWKS verifies what the double signs");
    }
}
