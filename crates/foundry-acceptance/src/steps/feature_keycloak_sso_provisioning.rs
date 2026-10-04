//! Step definitions for `keycloak-sso-provisioning.feature` — role-gated, opt-in
//! provisioning of a foundry account on a cluster identity's first sign-in (D3a).
//!
//! These scenarios drive the WHOLE federated round-trip against the shipped code:
//! foundry is built with the real `foundry_oidc::OidcProvider` pointed at the
//! in-process RS256 double, `/auth/oidc/start` mints the challenge, the double's
//! `/authorize` records the nonce, and `/auth/oidc/callback` is answered with the
//! `state` from the start redirect and the sealed challenge cookie from the start
//! response. The password door is driven through `POST /sign-in`. Postgres is read
//! only in Thens, to observe which accounts and memberships exist.
//!
//! Cookies are carried by hand rather than by a cookie jar: foundry marks them
//! `Secure` while the harness speaks plain HTTP on loopback, so a spec-following
//! jar would silently drop them and every scenario would fail for the wrong reason.
//!
//! "Turned away exactly as a wrong password is" is checked against a REAL
//! wrong-password answer for an unknown address, byte for byte once the per-request
//! CSRF token is masked (D7), not just by searching for the refusal copy.
//!
//! The round-trip, the password door, the cookie helpers and the protocol
//! constants are `pub(crate)`: `feature_keycloak_sso` drives the same flow
//! link-only, so both features exercise one copy of the browser's behaviour.

use crate::steps::feature_keycloak_sso::{csrf_for_session, seed_team_project, session_user_id};
use crate::steps::us_06_signin::submit_forgot_password;
use crate::support::harness::InProcHarness;
use crate::support::oidc_issuer::OidcIssuerDouble;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::header::HeaderMap;
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use secrecy::SecretString;
use std::collections::HashMap;

/// DESIGN OD-3 pinned the two OIDC paths. If DELIVER moves them, the Keycloak
/// client's redirect URI in the homelab repo moves in the same change.
pub(crate) const START_PATH: &str = "/auth/oidc/start";
pub(crate) const CALLBACK_PATH: &str = "/auth/oidc/callback";
pub(crate) const SIGN_IN_PATH: &str = "/sign-in";
const CLIENT_ID: &str = "foundry";
pub(crate) const CSRF_COOKIE: &str = "foundry_csrf";
pub(crate) const SESSION_COOKIE: &str = "foundry_session";
pub(crate) const CHALLENGE_COOKIE: &str = "foundry_oidc";
/// Local-part `nia.newcomer` deliberately differs from the username `nia`, so the
/// display-name fallback chain's last two links are distinguishable.
const NEWCOMER_EMAIL: &str = "nia.newcomer@example.test";
/// A pre-existing member, stored lower-case; the provider vouches for the same
/// address differently capitalised.
const MEMBER_EMAIL: &str = "pat@example.test";
const MEMBER_IDP_EMAIL: &str = "Pat@Example.test";
const MEMBER_PASSWORD: &str = "pat-own-password-long-enough";
/// An address with no account anywhere — the baseline every refusal is compared to.
pub(crate) const UNKNOWN_EMAIL: &str = "nobody-at-all@example.test";
/// The one refusal copy every failed sign-in shows (D7).
const GENERIC_REFUSAL: &str = "Invalid email or password";
pub(crate) const TEST_NOW: &str = "2026-01-15T12:00:00Z";
const ORIGINAL_WORKSPACE_CREATED: &str = "2025-06-01T00:00:00Z";
const NEWER_WORKSPACE_CREATED: &str = "2025-12-01T00:00:00Z";

pub(crate) fn ts(raw: &str) -> time::OffsetDateTime {
    time::OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339)
        .expect("timestamp parses")
}

pub(crate) fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(Policy::none())
        .cookie_store(false)
        .build()
        .expect("client builds")
}

fn harness(world: &FoundryWorld) -> &InProcHarness {
    world
        .harness
        .as_ref()
        .expect("a provisioning Given spawned foundry")
}

fn issuer(world: &FoundryWorld) -> &OidcIssuerDouble {
    world
        .kc_issuer
        .as_ref()
        .expect("a provisioning Given started the provider")
}

fn subject_email(world: &FoundryWorld) -> String {
    world
        .kc_subject_email
        .clone()
        .expect("a Given described who is signing in")
}

/// `name=value` of the first `Set-Cookie` for `name`, or `None`.
pub(crate) fn set_cookie_pair(headers: &HeaderMap, name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    headers
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|c| c.starts_with(&prefix))
        .and_then(|c| c.split(';').next())
        .map(str::to_string)
}

/// The value half of a `name=value` pair from [`set_cookie_pair`].
pub(crate) fn cookie_value(pair: &str, name: &str) -> String {
    pair.trim_start_matches(&format!("{name}=")).to_string()
}

/// Mask the per-request CSRF token so two refusals rendered for two different
/// requests compare byte-for-byte on everything else.
pub(crate) fn mask_csrf(body: &str) -> String {
    const MARK: &str = r#"name="_csrf" value=""#;
    let Some(at) = body.find(MARK) else {
        return body.to_string();
    };
    let rest = &body[at + MARK.len()..];
    let token = &rest[..rest.find('"').unwrap_or(0)];
    if token.is_empty() {
        return body.to_string();
    }
    body.replace(token, "<csrf>")
}

/// Keep a response as the scenario's latest answer: status, headers and body.
pub(crate) async fn record(world: &mut FoundryWorld, resp: reqwest::Response) {
    world.last_status = Some(resp.status());
    world.last_headers = Some(resp.headers().clone());
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

/// One password-door attempt, exactly as a browser makes it: fetch the form for
/// its CSRF cookie, then post. Returns status, headers and body.
pub(crate) async fn password_attempt(
    world: &FoundryWorld,
    email: &str,
    password: &str,
) -> (StatusCode, HeaderMap, String) {
    let base = harness(world).base_url();
    let http = client();
    let form_page = http
        .get(format!("{base}{SIGN_IN_PATH}"))
        .send()
        .await
        .expect("sign-in page");
    let csrf = set_cookie_pair(form_page.headers(), CSRF_COOKIE).expect("csrf cookie");
    let form = HashMap::from([
        ("email", email.to_string()),
        ("password", password.to_string()),
        ("_csrf", cookie_value(&csrf, CSRF_COOKIE)),
    ]);
    let resp = http
        .post(format!("{base}{SIGN_IN_PATH}"))
        .header(reqwest::header::COOKIE, csrf)
        .form(&form)
        .send()
        .await
        .expect("password sign-in");
    let status = resp.status();
    let headers = resp.headers().clone();
    (status, headers, resp.text().await.unwrap_or_default())
}

/// A password-door attempt kept as the scenario's latest answer, with the session
/// it established (if any) kept as the scenario's session.
pub(crate) async fn sign_in_with_password(world: &mut FoundryWorld, email: &str, password: &str) {
    let (status, headers, body) = password_attempt(world, email, password).await;
    world.kc_session_cookie = set_cookie_pair(&headers, SESSION_COOKIE);
    world.last_status = Some(status);
    world.last_headers = Some(headers);
    world.last_body = Some(body);
}

/// Wall-clock of one password-door attempt (form fetch included on both arms).
async fn timed_password_attempt_ms(world: &FoundryWorld, email: &str) -> u64 {
    let started = std::time::Instant::now();
    let _ = password_attempt(world, email, "a-guess-at-twelve-or-more").await;
    started.elapsed().as_millis() as u64
}

/// Start the provider double, then foundry pointed at it. When `seed_workspace` is
/// set, seed the instance's ORIGINAL workspace — the one a provisioned newcomer
/// joins; otherwise the instance is unclaimed.
pub(crate) async fn spawn_foundry(
    world: &mut FoundryWorld,
    provision_role: Option<String>,
    seed_workspace: bool,
) {
    let double = OidcIssuerDouble::start(CLIENT_ID).await;
    let config = foundry_oidc::OidcConfig::from_parts(
        Some(double.issuer()),
        Some(CLIENT_ID.to_string()),
        Some("test-client-secret".to_string()),
        Some(format!("http://127.0.0.1{CALLBACK_PATH}")),
    )
    .expect("complete oidc config")
    .expect("oidc config present")
    .with_provision_role(provision_role);
    let harness = InProcHarness::spawn_with_oidc(ts(TEST_NOW), config).await;

    if seed_workspace {
        world.kc_workspace_id =
            Some(insert_workspace(&harness, "Cluster", ORIGINAL_WORKSPACE_CREATED).await);
    }

    world.kc_issuer = Some(double);
    world.harness = Some(harness);
    world.http = Some(client());
}

/// Seed a workspace created at `created_at`; creation order decides which one is
/// the instance's ORIGINAL workspace.
pub(crate) async fn insert_workspace(
    harness: &InProcHarness,
    name: &str,
    created_at: &str,
) -> uuid::Uuid {
    let workspace_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name, created_at) VALUES ($1, $2, $3)")
        .bind(workspace_id)
        .bind(name)
        .bind(ts(created_at))
        .execute(harness.app.state.store.pool())
        .await
        .expect("seed a workspace");
    workspace_id
}

/// The provider will vouch for the newcomer's address with this profile.
fn describe_newcomer(
    world: &mut FoundryWorld,
    confirmed: bool,
    name: Option<&str>,
    username: Option<&str>,
) {
    issuer(world).will_vouch_for(NEWCOMER_EMAIL, confirmed);
    issuer(world).will_name(name, username);
    world.kc_subject_email = Some(NEWCOMER_EMAIL.to_string());
}

// ------------------------------------------------------------------ Givens

#[given(regex = r#"^foundry provisions holders of the "([^"]+)" realm role$"#)]
async fn provisions_role_holders(world: &mut FoundryWorld, role: String) {
    spawn_foundry(world, Some(role), true).await;
}

#[given(
    regex = r#"^foundry provisions holders of the "([^"]+)" realm role on an instance nobody has claimed$"#
)]
async fn provisions_role_holders_unclaimed(world: &mut FoundryWorld, role: String) {
    spawn_foundry(world, Some(role), false).await;
}

#[given("foundry provisions nobody from the cluster identity provider")]
async fn provisions_nobody(world: &mut FoundryWorld) {
    spawn_foundry(world, None, true).await;
}

#[given(regex = r#"^the instance also has a newer workspace named "([^"]+)"$"#)]
async fn newer_workspace(world: &mut FoundryWorld, name: String) {
    insert_workspace(harness(world), &name, NEWER_WORKSPACE_CREATED).await;
}

#[given(regex = r#"^a newcomer named "([^"]+)" is confirmed by the identity provider$"#)]
async fn newcomer_confirmed(world: &mut FoundryWorld, name: String) {
    describe_newcomer(world, true, Some(&name), Some("nia"));
}

/// A blank cell means the provider sends no such claim at all.
#[given(
    regex = r#"^a newcomer is confirmed by the identity provider as "([^"]*)" with username "([^"]*)"$"#
)]
async fn newcomer_confirmed_profile(world: &mut FoundryWorld, name: String, username: String) {
    let present = |s: &str| (!s.is_empty()).then(|| s.to_string());
    describe_newcomer(
        world,
        true,
        present(&name).as_deref(),
        present(&username).as_deref(),
    );
}

#[given(
    regex = r#"^a newcomer named "([^"]+)" is known to the identity provider at an unconfirmed address$"#
)]
async fn newcomer_unconfirmed(world: &mut FoundryWorld, name: String) {
    describe_newcomer(world, false, Some(&name), Some("nia"));
}

#[given(regex = r#"^a member named "([^"]+)" already has a foundry account with a password$"#)]
async fn member_with_password(world: &mut FoundryWorld, name: String) {
    let workspace_id = world
        .kc_workspace_id
        .expect("the original workspace was seeded");
    let pool = harness(world).app.state.store.pool();
    let hash = foundry_auth::hash_password(&SecretString::new(MEMBER_PASSWORD.to_string().into()))
        .await
        .expect("hash the member's password");
    let user_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash) \
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(MEMBER_EMAIL)
    .bind(MEMBER_EMAIL)
    .bind(&name)
    .bind(&hash)
    .execute(pool)
    .await
    .expect("seed the member");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("seed the member's membership");
    world.kc_member_password = Some(MEMBER_PASSWORD.to_string());
}

#[given(
    regex = r#"^the identity provider confirms the member under a differently capitalised address as "([^"]+)"$"#
)]
async fn member_confirmed_other_case(world: &mut FoundryWorld, idp_name: String) {
    issuer(world).will_vouch_for(MEMBER_IDP_EMAIL, true);
    issuer(world).will_name(Some(&idp_name), Some("pat"));
    world.kc_subject_email = Some(MEMBER_EMAIL.to_string());
}

#[given(
    regex = r#"^the identity provider grants (?:the newcomer|the member) the "([^"]+)" realm role$"#
)]
async fn grants_realm_role(world: &mut FoundryWorld, role: String) {
    issuer(world).will_grant_realm_roles(&[&role]);
}

#[given(regex = r"^the identity provider grants (?:the newcomer|the member) no realm roles$")]
async fn grants_no_realm_roles(world: &mut FoundryWorld) {
    issuer(world).will_grant_realm_roles(&[]);
}

#[given(
    regex = r#"^the identity provider no longer grants the newcomer the "([^"]+)" realm role$"#
)]
async fn role_withdrawn(world: &mut FoundryWorld, _role: String) {
    issuer(world).will_grant_realm_roles(&[]);
}

/// The account the sign-in created is remembered by the person its session
/// belongs to, so a later sign-in can be checked to be the same account (AC-7.3).
#[given("the newcomer has been given an account through the identity provider")]
async fn newcomer_already_provisioned(world: &mut FoundryWorld) {
    signs_in_through_provider(world).await;
    arrives_signed_in(world).await;
    let session = world.kc_session_cookie.clone().expect("a session cookie");
    world.kc_federated_user_id = session_user_id(world, &session).await;
}

// ------------------------------------------------------------------- Whens

/// The full browser round-trip, minus the browser.
#[when(regex = r"^(?:the newcomer|the member) signs in through the identity provider$")]
async fn signs_in_through_provider(world: &mut FoundryWorld) {
    begin_federated_sign_in(world).await;
    finish_federated_sign_in(world).await;
}

/// The first leg: ask foundry to start a federated sign-in. The start response is
/// recorded, so its redirect and challenge cookie are what the second leg answers.
pub(crate) async fn begin_federated_sign_in(world: &mut FoundryWorld) {
    let base = harness(world).base_url();
    let http = world.http.clone().expect("client");
    let start = http
        .get(format!("{base}{START_PATH}"))
        .send()
        .await
        .expect("start");
    record(world, start).await;
}

/// The second leg: visit the provider's `/authorize` named by the recorded start
/// redirect, then answer foundry's callback with that redirect's `state` and the
/// sealed challenge cookie the start response set.
pub(crate) async fn finish_federated_sign_in(world: &mut FoundryWorld) {
    let base = harness(world).base_url();
    let http = world.http.clone().expect("client");
    assert_eq!(
        world.last_status,
        Some(StatusCode::FOUND),
        "foundry did not hand off to the identity provider"
    );
    let start = world.last_headers.clone().expect("the start response");
    let authorize = start
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("start redirect names the provider")
        .to_string();
    let challenge =
        set_cookie_pair(&start, CHALLENGE_COOKIE).expect("start sets the challenge cookie");
    let state = reqwest::Url::parse(&authorize)
        .expect("authorize url parses")
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.into_owned())
        .expect("authorize url carries state");

    // The provider records the nonce here, as Keycloak would while the person
    // authenticates.
    http.get(&authorize).send().await.expect("authorize");

    let code = format!("code-{}", uuid::Uuid::new_v4());
    let resp = http
        .get(format!("{base}{CALLBACK_PATH}"))
        .query(&[("code", code.as_str()), ("state", state.as_str())])
        .header(reqwest::header::COOKIE, challenge)
        .send()
        .await
        .expect("callback");
    world.kc_session_cookie = set_cookie_pair(resp.headers(), SESSION_COOKIE);
    world.kc_last_code = Some(code);
    record(world, resp).await;
}

#[when(regex = r#"^the newcomer tries the password form with "([^"]*)"$"#)]
async fn newcomer_tries_password(world: &mut FoundryWorld, password: String) {
    let email = subject_email(world);
    sign_in_with_password(world, &email, &password).await;
}

/// Follow the reset link the forgot-password request emailed, as the person would:
/// open it (which mints the form's CSRF cookie), then submit the new password.
#[when(regex = r#"^the newcomer chooses the password "([^"]+)" through the emailed reset link$"#)]
async fn newcomer_resets_password(world: &mut FoundryWorld, password: String) {
    let email = subject_email(world);
    let delivery = harness(world)
        .fake_email
        .last_to(&email)
        .expect("a reset email was delivered to the newcomer");
    let link = delivery
        .body
        .split_whitespace()
        .find(|w| w.contains("/reset-password?token="))
        .expect("the reset email carries a reset link");
    let token = reqwest::Url::parse(link)
        .expect("the reset link parses")
        .query_pairs()
        .find(|(k, _)| k == "token")
        .map(|(_, v)| v.into_owned())
        .expect("the reset link carries a token");

    let base = harness(world).base_url();
    let http = client();
    let page = http
        .get(format!("{base}/reset-password"))
        .query(&[("token", token.as_str())])
        .send()
        .await
        .expect("open the reset link");
    assert_eq!(page.status(), StatusCode::OK, "the reset link did not open");
    let csrf = set_cookie_pair(page.headers(), CSRF_COOKIE).expect("reset form csrf cookie");
    let form = HashMap::from([
        ("token", token),
        ("password", password.clone()),
        ("confirm", password),
        ("_csrf", cookie_value(&csrf, CSRF_COOKIE)),
    ]);
    let resp = http
        .post(format!("{base}/reset-password"))
        .header(reqwest::header::COOKIE, csrf)
        .form(&form)
        .send()
        .await
        .expect("submit the new password");
    assert_eq!(
        resp.status(),
        StatusCode::SEE_OTHER,
        "the reset link did not accept the new password"
    );
}

/// Interleaved sampling, as the `us-06` timing oracle does: both arms see the same
/// contention, one warm-up pair is discarded, and medians absorb spikes.
#[when(
    regex = r"^password sign-in latency is sampled over (\d+) interleaved attempts for the newcomer and for an unknown address$"
)]
async fn sample_password_latency(world: &mut FoundryWorld, pairs: usize) {
    let newcomer = subject_email(world);
    let _ = timed_password_attempt_ms(world, &newcomer).await;
    let _ = timed_password_attempt_ms(world, UNKNOWN_EMAIL).await;
    let mut newcomer_ms = Vec::with_capacity(pairs);
    let mut unknown_ms = Vec::with_capacity(pairs);
    for _ in 0..pairs {
        newcomer_ms.push(timed_password_attempt_ms(world, &newcomer).await);
        unknown_ms.push(timed_password_attempt_ms(world, UNKNOWN_EMAIL).await);
    }
    world.kc_newcomer_latencies_ms = newcomer_ms;
    world.kc_unknown_latencies_ms = unknown_ms;
}

// ------------------------------------------------------------------- Thens

#[then(regex = r"^(?:the newcomer|the member) arrives signed in to the board$")]
async fn arrives_signed_in(world: &mut FoundryWorld) {
    let status = world.last_status.expect("a response was captured");
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "expected a redirect onto the board; got {status}: {}",
        world.last_body.clone().unwrap_or_default()
    );
    let location = world
        .last_headers
        .as_ref()
        .and_then(|h| h.get(reqwest::header::LOCATION))
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert_eq!(location, "/", "expected to land on the board");
    assert!(
        world.kc_session_cookie.is_some(),
        "the sign-in established no session"
    );
}

#[then(regex = r#"^(?:the newcomer|the member) is greeted as "([^"]+)"$"#)]
async fn greeted_as(world: &mut FoundryWorld, name: String) {
    let base = harness(world).base_url();
    let session = world.kc_session_cookie.clone().expect("a session cookie");
    let resp = client()
        .get(format!("{base}/"))
        .header(reqwest::header::COOKIE, session)
        .send()
        .await
        .expect("board");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "the session does not open the board"
    );
    let body = resp.text().await.unwrap_or_default();
    let greeting = format!("Welcome back, {name}</p>");
    assert!(
        body.contains(&greeting),
        "the board does not greet them as {name:?}"
    );
}

#[then("the newcomer is an ordinary member of the instance's original workspace")]
async fn ordinary_member_of_original(world: &mut FoundryWorld) {
    let workspace_id = world.kc_workspace_id.expect("seeded original workspace");
    let memberships: Vec<(uuid::Uuid, String)> = sqlx::query_as(
        "SELECT m.workspace_id, m.role FROM workspace_memberships m \
           JOIN users u ON u.id = m.user_id WHERE u.email_lower = $1",
    )
    .bind(subject_email(world))
    .fetch_all(harness(world).app.state.store.pool())
    .await
    .expect("read memberships");
    assert_eq!(
        memberships,
        vec![(workspace_id, "member".to_string())],
        "a provisioned newcomer must hold exactly one least-privileged membership, \
         in the original workspace"
    );
}

#[then("the newcomer is turned away exactly as a wrong password is")]
async fn turned_away(world: &mut FoundryWorld) {
    let status = world.last_status.expect("a response was captured");
    let body = world.last_body.clone().unwrap_or_default();
    let session = world
        .last_headers
        .as_ref()
        .and_then(|h| set_cookie_pair(h, SESSION_COOKIE));
    assert!(session.is_none(), "a refusal must not establish a session");
    assert!(
        body.contains(GENERIC_REFUSAL),
        "the refusal is not the generic wrong-password refusal; got {status}"
    );

    let (baseline_status, _, baseline_body) =
        password_attempt(world, UNKNOWN_EMAIL, "wrong-password").await;
    assert_eq!(
        status, baseline_status,
        "the refusal's status differs from a wrong password's"
    );
    assert_eq!(
        mask_csrf(&body),
        mask_csrf(&baseline_body),
        "the refusal is distinguishable from a wrong password's"
    );
}

#[then("no foundry account exists for the newcomer")]
async fn no_account(world: &mut FoundryWorld) {
    let found: Option<(uuid::Uuid,)> =
        sqlx::query_as("SELECT id FROM users WHERE lower(email_lower) = $1")
            .bind(subject_email(world))
            .fetch_optional(harness(world).app.state.store.pool())
            .await
            .expect("users lookup");
    assert!(found.is_none(), "a refused newcomer was given an account");
}

#[then(regex = r"^(?:the newcomer|the member) still has exactly one foundry account$")]
async fn still_one_account(world: &mut FoundryWorld) {
    let (count,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM users WHERE lower(email_lower) = $1")
            .bind(subject_email(world))
            .fetch_one(harness(world).app.state.store.pool())
            .await
            .expect("users count");
    assert_eq!(count, 1, "signing in created a second account for them");
}

#[then("the member's own password still lets them in")]
async fn member_password_still_works(world: &mut FoundryWorld) {
    let password = world
        .kc_member_password
        .clone()
        .expect("the member has a password");
    let (status, headers, body) = password_attempt(world, &subject_email(world), &password).await;
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "the member's password no longer opens the door: {body}"
    );
    assert!(
        set_cookie_pair(&headers, SESSION_COOKIE).is_some(),
        "the member's password established no session"
    );
}

#[then(
    regex = r"^the median newcomer latency is within (\d+)ms of the median unknown-address latency$"
)]
async fn newcomer_timing_within(world: &mut FoundryWorld, budget_ms: u64) {
    fn median(mut samples: Vec<u64>) -> i64 {
        samples.sort_unstable();
        samples[samples.len() / 2] as i64
    }
    let newcomer = world.kc_newcomer_latencies_ms.clone();
    let unknown = world.kc_unknown_latencies_ms.clone();
    assert!(
        !newcomer.is_empty() && newcomer.len() == unknown.len(),
        "expected equal non-empty sample arms, got newcomer={newcomer:?} unknown={unknown:?}"
    );
    let (m_newcomer, m_unknown) = (median(newcomer.clone()), median(unknown.clone()));
    let delta = (m_newcomer - m_unknown).abs();
    assert!(
        delta <= budget_ms as i64,
        "timing side-channel: a provisioned account's refusal differs from an unknown \
         address's by {delta}ms (budget {budget_ms}ms). median(newcomer)={m_newcomer}ms \
         median(unknown)={m_unknown}ms; newcomer={newcomer:?} unknown={unknown:?}"
    );
}

// ------------------------------------------------- US-07 (D3b) role withdrawal
//
// DISTILL 2026-10-04. Every phrase below drives an API that ships today (the
// browser round-trip, the password door, the reset link, the store's provisioning
// write); nothing here waits on DELIVER except the behaviour itself, so the
// scenarios that need D3b fail at an assertion, never at a missing step.

/// Where the provisioned newcomer files the work that must survive the withdrawal.
const NEWCOMER_TEAM_SLUG: &str = "cluster";
const NEWCOMER_PROJECT_SLUG: &str = "nia-work";
const NEWCOMER_ISSUE_TITLE: &str = "Filed before the provision role was withdrawn";
const NEWCOMER_COMMENT_BODY: &str = "Commented before the provision role was withdrawn";
/// The display name `newcomer_confirmed` and the provisioning store write give her.
const NEWCOMER_NAME: &str = "Nia Newcomer";

/// The newcomer's account id, read at the store boundary by address.
async fn newcomer_account_id(world: &FoundryWorld) -> uuid::Uuid {
    let (id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(subject_email(world))
        .fetch_one(harness(world).app.state.store.pool())
        .await
        .expect("the newcomer has an account");
    id
}

/// A form post made through the newcomer's own session, exactly as a browser makes
/// it: the CSRF pair is minted for that session first.
async fn post_as_newcomer(world: &FoundryWorld, path: &str, fields: &[(&str, &str)]) -> StatusCode {
    let base = harness(world).base_url();
    let session = world
        .kc_session_cookie
        .clone()
        .expect("the newcomer's sign-in established a session");
    let http = client();
    let csrf = csrf_for_session(&http, &base, &session).await;
    let mut form: HashMap<&str, String> =
        fields.iter().map(|(k, v)| (*k, (*v).to_string())).collect();
    form.insert("_csrf", cookie_value(&csrf, CSRF_COOKIE));
    http.post(format!("{base}{path}"))
        .header(reqwest::header::COOKIE, format!("{session}; {csrf}"))
        .header("hx-request", "true")
        .form(&form)
        .send()
        .await
        .expect("post as the newcomer")
        .status()
}

/// AC-7.2's "authored work": one issue and one comment on it, both made through
/// the newcomer's session — the team and project they are filed in are the
/// precondition, seeded at the store boundary.
#[given("the newcomer has filed an issue and commented on it")]
async fn newcomer_authored_work(world: &mut FoundryWorld) {
    let workspace_id = world
        .kc_workspace_id
        .expect("the original workspace was seeded");
    let user_id = newcomer_account_id(world).await;
    seed_team_project(
        harness(world).app.state.store.pool(),
        workspace_id,
        user_id,
        NEWCOMER_TEAM_SLUG,
        NEWCOMER_PROJECT_SLUG,
    )
    .await;
    let issues = format!("/team/{NEWCOMER_TEAM_SLUG}/project/{NEWCOMER_PROJECT_SLUG}/issues");
    let filed = post_as_newcomer(world, &issues, &[("title", NEWCOMER_ISSUE_TITLE)]).await;
    assert!(
        filed.is_success() || filed.is_redirection(),
        "the newcomer could not file an issue: {filed}"
    );
    let commented = post_as_newcomer(
        world,
        &format!("{issues}/1/comments"),
        &[("body", NEWCOMER_COMMENT_BODY)],
    )
    .await;
    assert!(
        commented.is_success() || commented.is_redirection(),
        "the newcomer could not comment on their issue: {commented}"
    );
}

/// Only `role` is in the next ID token's realm roles — the provision role is gone.
#[given(regex = r#"^the identity provider now grants the newcomer only the "([^"]+)" realm role$"#)]
async fn now_grants_only(world: &mut FoundryWorld, role: String) {
    issuer(world).will_grant_realm_roles(&[&role]);
}

/// Scenario 11's When and its first Then, composed as the precondition of the
/// re-grant (Pillar 2).
#[given("the newcomer has been turned away through the identity provider")]
async fn newcomer_already_turned_away(world: &mut FoundryWorld) {
    signs_in_through_provider(world).await;
    turned_away(world).await;
}

/// Scenario 10's forgot-password request and reset link, composed as a
/// precondition: the provisioned newcomer now has a password of their own.
#[given(regex = r#"^the newcomer has chosen the password "([^"]+)" through a reset$"#)]
async fn newcomer_has_chosen_password(world: &mut FoundryWorld, password: String) {
    let email = subject_email(world);
    submit_forgot_password(world, email).await;
    newcomer_resets_password(world, password).await;
}

/// The account was created by provisioning while it was still switched on —
/// through the store's own provisioning write, the one the callback calls — and
/// foundry has since been started with provisioning off (OD-13). The harness
/// cannot restart foundry over the same database mid-scenario, so the write that
/// sign-in performed earlier is made directly; what is under test is the later
/// sign-in, not the provisioning.
///
/// DELIVER: DDD-25 adds a `now` parameter to `provision_federated_member`; pass
/// `harness(world).app.state.clock.now()` (or the harness's equivalent) here when
/// the signature changes.
#[given("the newcomer was given an account while provisioning was still switched on")]
async fn newcomer_provisioned_earlier(world: &mut FoundryWorld) {
    let email = subject_email(world);
    let outcome = harness(world)
        .app
        .state
        .store
        .provision_federated_member(&email, &email, NEWCOMER_NAME)
        .await
        .expect("the provisioning write succeeds");
    assert!(
        matches!(
            outcome,
            foundry_store::FederatedProvisionOutcome::Created { .. }
        ),
        "the newcomer was not given a new account: {outcome:?}"
    );
}

/// The browser still holds the session it was given before the withdrawal; the
/// newcomer opens the board with it. The answer is kept as the latest response.
#[when("the newcomer comes back to the board in the session they already have")]
async fn newcomer_returns_with_session(world: &mut FoundryWorld) {
    let base = harness(world).base_url();
    let session = world
        .kc_session_cookie
        .clone()
        .expect("the newcomer was signed in before the withdrawal");
    let resp = client()
        .get(format!("{base}/"))
        .header(reqwest::header::COOKIE, session)
        .send()
        .await
        .expect("board");
    record(world, resp).await;
}

/// D8: the refusal deleted nothing — the display name, the issue and the comment
/// the newcomer authored are all still theirs, unedited and not tombstoned.
#[then("the newcomer's name and the work they authored are unchanged")]
async fn newcomer_work_unchanged(world: &mut FoundryWorld) {
    let user_id = newcomer_account_id(world).await;
    let pool = harness(world).app.state.store.pool();
    let (name,): (String,) = sqlx::query_as("SELECT display_name FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("read the newcomer's name");
    assert_eq!(name, NEWCOMER_NAME, "the newcomer's display name changed");
    let issues: Vec<(String,)> = sqlx::query_as("SELECT title FROM issues WHERE author_id = $1")
        .bind(user_id)
        .fetch_all(pool)
        .await
        .expect("read the newcomer's issues");
    assert_eq!(
        issues,
        vec![(NEWCOMER_ISSUE_TITLE.to_string(),)],
        "the issues the newcomer authored changed"
    );
    let comments: Vec<(String, bool)> = sqlx::query_as(
        "SELECT body_markdown, deleted_at IS NULL FROM comments WHERE author_id = $1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .expect("read the newcomer's comments");
    assert_eq!(
        comments,
        vec![(NEWCOMER_COMMENT_BODY.to_string(), true)],
        "the comments the newcomer authored changed"
    );
}

/// AC-7.3: the session the re-granted sign-in established belongs to the very
/// account provisioning created — same id, no repair step.
#[then("the newcomer is signed in as the same account they were given")]
async fn same_account_as_given(world: &mut FoundryWorld) {
    let given = world
        .kc_federated_user_id
        .expect("the account provisioning gave the newcomer was remembered");
    let session = world.kc_session_cookie.clone().expect("a session cookie");
    assert_eq!(
        session_user_id(world, &session).await,
        Some(given),
        "the newcomer was signed in as a different account than the one they were given"
    );
}
