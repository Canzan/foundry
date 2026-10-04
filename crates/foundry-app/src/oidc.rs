//! Federated sign-in: `GET /auth/oidc/start` and `GET /auth/oidc/callback`.
//!
//! ADDITIVE. The password door in `signin.rs` is untouched and remains the
//! break-glass route — Keycloak, LLDAP and foundry share a cluster, so an
//! SSO-only tracker is unreachable exactly when the operator most needs the issue
//! describing how to fix it.
//!
//! Both handlers end in `signin::establish_session`, the SAME seam the password
//! flow uses, so a federated session is indistinguishable downstream and the
//! fail-closed no-workspace branch cannot drift between the two doors.
//!
//! NON-ENUMERABLE. Every refusal returns exactly what a wrong password returns
//! (`GENERIC_SIGNIN_ERROR`, 401). The callback is publicly reachable, so a
//! specific message would make foundry an account-existence oracle for the whole
//! Keycloak realm.
//!
//! THE ONE-TIME CHALLENGE rides in a signed cookie, not a pre-auth session row:
//! `/auth/oidc/start` is reachable signed-out, so a session row per click would be
//! an unauthenticated unbounded INSERT on a public endpoint. The cookie is
//! stateless, short-lived, and signed with the shipped `foundry_auth::sign` HMAC
//! over SESSION_SECRET — the same primitive `InviteToken` and `UnsubscribeToken`
//! already use (ADR-OIDC-002).

use crate::signin::{
    ensure_csrf_cookie, establish_session, render_signin_form, response_with_optional_cookie,
    GENERIC_SIGNIN_ERROR,
};
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::header::{HeaderMap, SET_COOKIE};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use base64::Engine as _;
use foundry_oidc::{AuthRequest, IdentityClaims};
use foundry_store::FederatedProvisionOutcome;
use serde::Deserialize;
use tower_sessions::Session;

/// Where the sign-in page's control points. Moving this moves the redirect URI
/// registered with Keycloak, in the same change.
pub const START_PATH: &str = "/auth/oidc/start";
pub const CALLBACK_PATH: &str = "/auth/oidc/callback";

const CHALLENGE_COOKIE: &str = "foundry_oidc";
/// Long enough for a human to authenticate, short enough that an outstanding
/// challenge is not a durable credential. It cannot be revoked server-side (the
/// cost of statelessness), so it expires quickly instead.
const CHALLENGE_TTL_SECONDS: i64 = 600;

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub state: String,
}

/// The generic refusal. One function, so the federated and password paths
/// physically cannot diverge on refusal shape.
fn refuse(state: &AppState, headers: &HeaderMap, why: &str) -> Response {
    tracing::info!(reason = %why, "oidc sign-in refused");
    let (token, set_cookie) = ensure_csrf_cookie(state, headers);
    let body = render_signin_form(&token, Some(GENERIC_SIGNIN_ERROR));
    let mut resp = response_with_optional_cookie(
        StatusCode::UNAUTHORIZED,
        Html(body).into_response(),
        set_cookie,
    );
    // Clear the challenge on every refusal as well as on success: single-use.
    if let Ok(v) = clear_cookie(state).parse() {
        resp.headers_mut().append(SET_COOKIE, v);
    }
    resp
}

fn cookie_attrs(state: &AppState) -> String {
    let secure = if state.session_cookie_secure {
        "; Secure"
    } else {
        ""
    };
    format!("; Path=/; HttpOnly; SameSite=Lax{secure}")
}

fn clear_cookie(state: &AppState) -> String {
    format!(
        "{CHALLENGE_COOKIE}={}",
        format_args!("; Max-Age=0{}", cookie_attrs(state))
    )
}

/// `<base64url(json)>.<hmac>` — the payload is not secret (it is the client's own
/// challenge), but it must not be client-CHOSEN, which is what the signature buys.
fn seal(state: &AppState, req: &AuthRequest) -> Option<String> {
    let json = serde_json::json!({
        "state": req.state,
        "nonce": req.nonce,
        "verifier": req.code_verifier,
    })
    .to_string();
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes());
    let sig = foundry_auth::sign(&state.session_secret, payload.as_bytes()).ok()?;
    Some(format!("{payload}.{sig}"))
}

fn unseal(state: &AppState, raw: &str) -> Option<AuthRequest> {
    let (payload, sig) = raw.split_once('.')?;
    foundry_auth::verify(&state.session_secret, payload.as_bytes(), sig).ok()?;
    let json = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&json).ok()?;
    Some(AuthRequest {
        state: v.get("state")?.as_str()?.to_string(),
        nonce: v.get("nonce")?.as_str()?.to_string(),
        code_verifier: v.get("verifier")?.as_str()?.to_string(),
    })
}

fn read_challenge(state: &AppState, headers: &HeaderMap) -> Option<AuthRequest> {
    let raw = headers
        .get(axum::http::header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|c| c.trim().split_once('='))
        .find(|(k, _)| *k == CHALLENGE_COOKIE)
        .map(|(_, v)| v.to_string())?;
    unseal(state, &raw)
}

/// Begin: mint a fresh challenge, remember it in the cookie, hand off to Keycloak.
pub async fn start(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(provider) = state.oidc.clone() else {
        // Not configured: refuse exactly as any other failed sign-in would, never
        // a 500 and never a stack trace (AC-5.3).
        return refuse(&state, &headers, "oidc is not configured");
    };
    let req = AuthRequest::generate();
    let url = match provider.authorization_url(&req).await {
        Ok(u) => u,
        Err(err) => return refuse(&state, &headers, &format!("authorization_url: {err}")),
    };
    let Some(sealed) = seal(&state, &req) else {
        return refuse(&state, &headers, "could not seal the challenge");
    };

    let cookie = format!(
        "{CHALLENGE_COOKIE}={sealed}; Max-Age={CHALLENGE_TTL_SECONDS}{}",
        cookie_attrs(&state)
    );
    let mut resp = Response::builder()
        .status(StatusCode::FOUND)
        .header(axum::http::header::LOCATION, url)
        .body(axum::body::Body::empty())
        .expect("redirect response builds");
    if let Ok(v) = cookie.parse() {
        resp.headers_mut().append(SET_COOKIE, v);
    }
    resp
}

/// Finish: verify the challenge, exchange the code, link the identity, sign in.
pub async fn callback(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Query(q): Query<CallbackQuery>,
) -> Response {
    let Some(provider) = state.oidc.clone() else {
        return refuse(&state, &headers, "oidc is not configured");
    };
    // No challenge means this callback was never started here.
    let Some(req) = read_challenge(&state, &headers) else {
        return refuse(&state, &headers, "no challenge cookie");
    };
    if q.state.is_empty() || q.state != req.state {
        return refuse(&state, &headers, "state mismatch");
    }
    if q.code.is_empty() {
        return refuse(&state, &headers, "no authorization code");
    }

    // Exchanges the code (single-use AT the provider — this is what actually
    // refuses a replay), validates the ID token RS256-pinned against the
    // published JWKS, and checks the nonce.
    let identity = match provider.exchange_code(&q.code, &req).await {
        Ok(c) => c,
        Err(err) => return refuse(&state, &headers, &format!("exchange: {err}")),
    };

    if !identity.email_verified {
        return refuse(&state, &headers, "provider has not confirmed the email");
    }

    // DDD-15 find-or-provision. (1) An existing account links unless provisioning
    // created it and it no longer holds the provision role (DDD-28) —
    // users.email_lower is UNIQUE, so the match is unambiguous.
    let email_lower = identity.email.trim().to_lowercase();
    let user_id = match state.store.find_user_by_email(&email_lower).await {
        Ok(Some(u)) => match judge_returning(u.provisioned, provider.provision_role(), &identity) {
            Ok(()) => u.id,
            Err(why) => return refuse(&state, &headers, why),
        },
        Ok(None) => match provision(&state, &provider, &identity, &email_lower).await {
            Ok(id) => id,
            Err(ProvisionFailure::Refused(why)) => return refuse(&state, &headers, why),
            Err(ProvisionFailure::Internal) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
            }
        },
        Err(err) => {
            tracing::error!(%err, "find_user_by_email failed during oidc callback");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };

    let mut resp = establish_session(&state, &session, &headers, user_id).await;
    if let Ok(v) = clear_cookie(&state).parse() {
        resp.headers_mut().append(SET_COOKIE, v);
    }
    resp
}

/// Refusal reasons the find-or-provision order adds (DDD-15/DDD-20). They ride the
/// existing `refuse()` log line; the response is the generic refusal regardless.
const NO_ACCOUNT: &str = "no foundry account for this identity";
const LACKS_PROVISION_ROLE: &str = "identity lacks provision role";
const PROVISIONED_LACKS_PROVISION_ROLE: &str = "provisioned account lacks provision role";
const NO_WORKSPACE: &str = "no workspace to provision into";

/// `users.display_name CHECK (length BETWEEN 1 AND 64)`.
const MAX_DISPLAY_NAME_CHARS: usize = 64;

/// Why `provision` yielded no account: a logged refusal (the generic refusal
/// page) or an internal fault (500).
enum ProvisionFailure {
    Refused(&'static str),
    Internal,
}

/// DDD-15 steps (2)–(5) for an identity with no foundry account: gate on the
/// provision role, then create a password-less member in the original workspace.
async fn provision(
    state: &AppState,
    provider: &foundry_oidc::OidcProvider,
    identity: &IdentityClaims,
    email_lower: &str,
) -> Result<uuid::Uuid, ProvisionFailure> {
    judge_newcomer(provider.provision_role(), identity).map_err(ProvisionFailure::Refused)?;
    // Unreachable in practice (RFC 5321 caps a local-part at 64); refusing beats a
    // CHECK-violation 500 and never truncates (OD-7).
    let display_name = greeting_name(identity).ok_or(ProvisionFailure::Refused(NO_ACCOUNT))?;
    let email_display = identity.email.trim();
    match state
        .store
        .provision_federated_member(email_lower, email_display, &display_name, state.clock.now())
        .await
    {
        Ok(FederatedProvisionOutcome::Created {
            user_id,
            workspace_id,
        }) => {
            log_provisioned(user_id, workspace_id);
            Ok(user_id)
        }
        Ok(FederatedProvisionOutcome::Existing { user_id }) => Ok(user_id),
        Ok(FederatedProvisionOutcome::NoWorkspace) => Err(ProvisionFailure::Refused(NO_WORKSPACE)),
        Err(err) => {
            tracing::error!(%err, "provision_federated_member failed during oidc callback");
            Err(ProvisionFailure::Internal)
        }
    }
}

/// DDD-15 steps (2)–(3) for an identity with no foundry account. `None` role means
/// link-only (exactly D3); the role match is exact and case-sensitive.
fn judge_newcomer(
    provision_role: Option<&str>,
    identity: &IdentityClaims,
) -> Result<(), &'static str> {
    let role = provision_role.ok_or(NO_ACCOUNT)?;
    if identity.has_realm_role(role) {
        Ok(())
    } else {
        Err(LACKS_PROVISION_ROLE)
    }
}

/// DDD-28 step (1) / DDD-29 for an identity whose account already exists. Only an
/// account provisioning created depends on the role, and only while provisioning is
/// on (OD-13); the role match is exact and case-sensitive, read at sign-in (D11).
fn judge_returning(
    provisioned: bool,
    provision_role: Option<&str>,
    identity: &IdentityClaims,
) -> Result<(), &'static str> {
    match provision_role {
        Some(role) if provisioned && !identity.has_realm_role(role) => {
            Err(PROVISIONED_LACKS_PROVISION_ROLE)
        }
        _ => Ok(()),
    }
}

/// DDD-18 greeting-name chain: the first of `name` → `preferred_username` → email
/// local-part that is non-blank once trimmed and fits the 64-character column.
/// A longer candidate is skipped, never truncated (OD-7).
fn greeting_name(identity: &IdentityClaims) -> Option<String> {
    let local_part = identity.email.trim().split('@').next();
    [
        identity.name.as_deref(),
        identity.preferred_username.as_deref(),
        local_part,
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .find(|c| !c.is_empty() && c.chars().count() <= MAX_DISPLAY_NAME_CHARS)
    .map(str::to_string)
}

/// DDD-20: the one info line a successful provision emits — ids only, no claims.
fn log_provisioned(user_id: uuid::Uuid, workspace_id: uuid::Uuid) {
    tracing::info!(%user_id, %workspace_id, "oidc identity provisioned");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (configured provision role, realm roles held, verdict)
    type RoleCase<'a> = (Option<&'a str>, &'a [&'a str], Result<(), &'a str>);
    /// (name, preferred_username, email, expected greeting)
    type NameCase<'a> = (Option<&'a str>, Option<&'a str>, &'a str, Option<&'a str>);

    fn identity(
        email: &str,
        name: Option<&str>,
        username: Option<&str>,
        roles: &[&str],
    ) -> IdentityClaims {
        IdentityClaims {
            subject: "sub-1".to_string(),
            email: email.to_string(),
            email_verified: true,
            realm_roles: roles.iter().map(|r| r.to_string()).collect(),
            name: name.map(str::to_string),
            preferred_username: username.map(str::to_string),
        }
    }

    /// DDD-15: provisioning off refuses as today (D3); a missing role — including
    /// a differently-capitalised one — refuses with its own reason; the exact role
    /// lets provisioning proceed.
    #[test]
    fn a_newcomer_is_refused_unless_provisioning_is_on_and_the_role_is_held() {
        let cases: [RoleCase; 5] = [
            (None, &["foundry-user"], Err(NO_ACCOUNT)),
            (Some("foundry-user"), &[], Err(LACKS_PROVISION_ROLE)),
            (
                Some("foundry-user"),
                &["some-other-role"],
                Err(LACKS_PROVISION_ROLE),
            ),
            (
                Some("foundry-user"),
                &["Foundry-User"],
                Err(LACKS_PROVISION_ROLE),
            ),
            (Some("foundry-user"), &["other", "foundry-user"], Ok(())),
        ];
        for (role, held, expected) in cases {
            let who = identity("nia@example.test", Some("Nia"), None, held);
            assert_eq!(
                judge_newcomer(role, &who),
                expected,
                "provision role {role:?}, held {held:?}"
            );
        }
    }

    /// (account was provisioned, configured provision role, realm roles held, verdict)
    type ReturningCase<'a> = (bool, Option<&'a str>, &'a [&'a str], Result<(), &'a str>);

    /// DDD-28 step (1) / DDD-29 / AC-7.6: an account that already exists is
    /// refused only when provisioning created it AND provisioning is on AND the
    /// identity lacks the exact role. Never provisioned → link whatever the roles
    /// (1a, D3b); provisioning off → link (1b, OD-13); role held → link (1c); role
    /// missing, differently capitalised or replaced → refuse (1d, D11 / OD-9) with
    /// a reason of its own, distinct from a newcomer's (D12).
    #[test]
    fn a_returning_account_is_refused_only_when_provisioned_and_the_role_is_missing() {
        let refuse = Err(PROVISIONED_LACKS_PROVISION_ROLE);
        let cases: [ReturningCase; 10] = [
            (false, Some("foundry-user"), &[], Ok(())),
            (false, Some("foundry-user"), &["some-other-role"], Ok(())),
            (false, None, &[], Ok(())),
            (true, None, &[], Ok(())),
            (true, None, &["some-other-role"], Ok(())),
            (true, Some("foundry-user"), &["foundry-user"], Ok(())),
            (
                true,
                Some("foundry-user"),
                &["other", "foundry-user"],
                Ok(()),
            ),
            (true, Some("foundry-user"), &[], refuse),
            (true, Some("foundry-user"), &["some-other-role"], refuse),
            (true, Some("foundry-user"), &["Foundry-User"], refuse),
        ];
        for (provisioned, role, held, expected) in cases {
            let who = identity("nia@example.test", Some("Nia"), None, held);
            assert_eq!(
                judge_returning(provisioned, role, &who),
                expected,
                "provisioned {provisioned}, provision role {role:?}, held {held:?}"
            );
        }
        assert_ne!(
            PROVISIONED_LACKS_PROVISION_ROLE, LACKS_PROVISION_ROLE,
            "the operator must be able to tell a withdrawn member from a stranger (D12)"
        );
    }

    /// DDD-18 / OD-7: first non-blank candidate of name → preferred_username →
    /// email local-part that fits in 64 characters; longer ones are skipped, never
    /// truncated; the chosen candidate is trimmed.
    #[test]
    fn the_greeting_is_the_first_usable_candidate() {
        let sixty_four = "a".repeat(64);
        let sixty_five = "é".repeat(65);
        let cases: [NameCase; 8] = [
            (
                Some("Nia Newcomer"),
                Some("nia"),
                "nia.newcomer@x.test",
                Some("Nia Newcomer"),
            ),
            (Some("  Nia  "), None, "nia.newcomer@x.test", Some("Nia")),
            (None, Some("nia"), "nia.newcomer@x.test", Some("nia")),
            (Some("   "), Some("nia"), "nia.newcomer@x.test", Some("nia")),
            (
                Some(""),
                Some("\t"),
                "nia.newcomer@x.test",
                Some("nia.newcomer"),
            ),
            (
                Some(&sixty_five),
                Some("nia"),
                "nia.newcomer@x.test",
                Some("nia"),
            ),
            (
                Some(&sixty_four),
                Some("nia"),
                "nia.newcomer@x.test",
                Some(&sixty_four),
            ),
            (None, None, &format!("{}@x.test", "b".repeat(65)), None),
        ];
        for (name, username, email, expected) in cases {
            assert_eq!(
                greeting_name(&identity(email, name, username, &[])).as_deref(),
                expected,
                "name {name:?}, username {username:?}, email {email:?}"
            );
        }
    }

    /// DDD-20: a provision logs one info line naming the user and workspace.
    #[test]
    fn a_provision_logs_the_user_and_workspace() {
        #[derive(Clone, Default)]
        struct Buf(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
        impl std::io::Write for Buf {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.lock().expect("buf").extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let buf = Buf::default();
        let sink = buf.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || sink.clone())
            .with_ansi(false)
            .finish();
        let (user, workspace) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());

        tracing::subscriber::with_default(subscriber, || log_provisioned(user, workspace));

        let out = String::from_utf8(buf.0.lock().expect("buf").clone()).expect("utf8");
        assert_eq!(out.lines().count(), 1, "exactly one line: {out}");
        assert!(out.contains("INFO"), "{out}");
        assert!(out.contains("oidc identity provisioned"), "{out}");
        assert!(out.contains(&user.to_string()), "{out}");
        assert!(out.contains(&workspace.to_string()), "{out}");
    }
}
