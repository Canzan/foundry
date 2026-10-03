//! Step definitions for `keycloak-sso` — signing in to foundry with the cluster
//! identity (`tests/features/keycloak-sso.feature`; un-pended scenario by scenario in DELIVER).
//!
//! Foundry is spawned with `InProcHarness::spawn_with_oidc`, built through the
//! provisioning module's `spawn_foundry` with NO provision role — link-only, D3
//! exactly. The sign-in is the real browser round-trip shared with that module:
//! `/auth/oidc/start`, the double's `/authorize` (which records the nonce), then
//! the callback answered with the start redirect's `state` and the sealed
//! challenge cookie. Cookies are carried by hand: foundry marks them `Secure`
//! while the harness speaks plain HTTP on loopback.
//!
//! The identity provider is `support::oidc_issuer` — an in-process axum double on
//! `127.0.0.1:0` signing with a FIXED RSA test keypair. Real RS256 crypto, fixture
//! key material, mirroring the shipped machine-token keypair. Postgres is REAL
//! (shared testcontainer, per-scenario schema).
//!
//! Reused Givens (cucumber-rs requires globally-unique step text — every step
//! phrase in this module is scoped to "cluster identity" wording to avoid
//! colliding with the shipped sign-in and bootstrap modules).

use crate::steps::feature_keycloak_sso_provisioning::{
    begin_federated_sign_in, client, cookie_value, finish_federated_sign_in, insert_workspace,
    set_cookie_pair, spawn_foundry,
};
use crate::support::harness::InProcHarness;
use crate::support::oidc_issuer::Variant;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use foundry_app::Clock;
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use secrecy::SecretString;
use std::collections::HashMap;

/// DESIGN OD-3 pinned these. If DELIVER moves them, the Keycloak client's redirect
/// URI in the homelab repo moves in the same change.
const START_PATH: &str = "/auth/oidc/start";
const CALLBACK_PATH: &str = "/auth/oidc/callback";
const SIGN_IN_PATH: &str = "/sign-in";

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
const CHALLENGE_COOKIE: &str = "foundry_oidc";
const CSRF_COOKIE: &str = "foundry_csrf";

/// The operator's existing foundry account — the one a cluster identity links to.
const OPERATOR_EMAIL: &str = "operator@example.test";
const OPERATOR_NAME: &str = "Olu Operator";
const OPERATOR_PASSWORD: &str = "operator-own-password-long-enough";
const OPERATOR_WORKSPACE: &str = "Operations";
const OPERATOR_WORKSPACE_CREATED: &str = "2025-06-01T00:00:00Z";
/// Where the federated operator files an issue.
const TEAM_SLUG: &str = "ops";
const PROJECT_SLUG: &str = "homelab";
const FILED_TITLE: &str = "Filed through a cluster identity";
/// Known to the provider, unknown to foundry.
const STRANGER_EMAIL: &str = "stranger@example.test";
/// Has an account, belongs to no workspace.
const ORPHAN_EMAIL: &str = "orphan@example.test";
const ORPHAN_NAME: &str = "Orla Orphan";
const SESSION_COOKIE: &str = "foundry_session";
/// An address with no account anywhere — the wrong-password baseline.
const UNKNOWN_EMAIL: &str = "nobody-at-all@example.test";
/// The first operator of a fresh instance, claiming it through the bootstrap link.
const CLAIM_TOKEN: &str = "keycloak-sso-claim-token";
const CLAIMANT_EMAIL: &str = "first-operator@example.test";
const CLAIMANT_NAME: &str = "Fen First";
const CLAIMANT_PASSWORD: &str = "first-operator-password-long";
const CLAIMANT_WORKSPACE: &str = "Homelab";

fn now() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(TEST_NOW, &time::format_description::well_known::Rfc3339)
        .expect("TEST_NOW parses")
}

/// A client that does NOT follow redirects — every assertion here is about the
/// redirect itself (where it points, whether it happened at all).
fn no_redirect_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(Policy::none())
        .cookie_store(true)
        .build()
        .expect("client builds")
}

async fn ensure_harness(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        world.harness = Some(InProcHarness::spawn(now()).await);
    }
    if world.http.is_none() {
        world.http = Some(no_redirect_client());
    }
}

fn base(world: &FoundryWorld) -> String {
    world
        .harness
        .as_ref()
        .expect("harness spawned by a Given")
        .base_url()
}

async fn record(world: &mut FoundryWorld, resp: reqwest::Response) {
    world.last_status = Some(resp.status());
    world.last_headers = Some(resp.headers().clone());
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

// ------------------------------------------------------------------ Givens

#[given("foundry is connected to the cluster identity provider")]
async fn connect_provider(world: &mut FoundryWorld) {
    ensure_connected(world).await;
}

/// Start the provider double and foundry pointed at it, link-only (no provision
/// role: D3 exactly). A no-op once a harness exists, so Givens that open a
/// scenario on their own can call it without double-spawning.
async fn ensure_connected(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        spawn_foundry(world, None, false).await;
        world.kc_provider_configured = true;
    }
}

#[given("foundry is not connected to any cluster identity provider")]
async fn no_provider(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    world.kc_provider_configured = false;
}

#[given("foundry is given a provider address but no credential for it")]
async fn half_configured(world: &mut FoundryWorld) {
    world.kc_partial_config = true;
}

#[given(regex = r"^the operator has a foundry account for a confirmed address$")]
async fn account_confirmed(world: &mut FoundryWorld) {
    ensure_connected(world).await;
    seed_operator_account(world).await;
    if let Some(d) = world.kc_issuer.as_ref() {
        d.will_vouch_for(OPERATOR_EMAIL, true);
    }
    world.kc_subject_email = Some(OPERATOR_EMAIL.to_string());
    world.kc_account_exists = true;
}

/// A real account with a password and exactly one `member` membership. Idempotent:
/// a second call within the scenario leaves the first account as it is.
async fn seed_operator_account(world: &mut FoundryWorld) {
    if world.kc_workspace_id.is_some() {
        return;
    }
    let harness = world.harness.as_ref().expect("harness");
    let workspace_id =
        insert_workspace(harness, OPERATOR_WORKSPACE, OPERATOR_WORKSPACE_CREATED).await;
    seed_account(world, OPERATOR_EMAIL, OPERATOR_NAME).await;
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool();
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) \
              SELECT $1, id, 'member' FROM users WHERE email_lower = $2",
    )
    .bind(workspace_id)
    .bind(OPERATOR_EMAIL)
    .execute(pool)
    .await
    .expect("seed the operator's membership");
    world.kc_workspace_id = Some(workspace_id);
}

/// A real account with a password and NO workspace membership.
async fn seed_account(world: &FoundryWorld, email: &str, name: &str) {
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool();
    let hash =
        foundry_auth::hash_password(&SecretString::new(OPERATOR_PASSWORD.to_string().into()))
            .await
            .expect("hash a password");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash) \
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(email)
    .bind(email)
    .bind(name)
    .bind(&hash)
    .execute(pool)
    .await
    .expect("seed an account");
}

/// The operator's user id, read at the store boundary.
async fn operator_id(world: &FoundryWorld) -> uuid::Uuid {
    let (id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(OPERATOR_EMAIL)
        .fetch_one(
            world
                .harness
                .as_ref()
                .expect("harness")
                .app
                .state
                .store
                .pool(),
        )
        .await
        .expect("the operator's account");
    id
}

/// The instance is in use — the operator's workspace and account exist — so a
/// refusal here is the link-only gate, not the unclaimed-instance guard.
#[given("a person known to the identity provider has no foundry account")]
async fn no_account(world: &mut FoundryWorld) {
    ensure_connected(world).await;
    seed_operator_account(world).await;
    let email = STRANGER_EMAIL.to_string();
    if let Some(d) = world.kc_issuer.as_ref() {
        d.will_vouch_for(&email, true);
    }
    world.kc_subject_email = Some(email);
    world.kc_account_exists = false;
}

#[given("the operator has a foundry account for an address the provider has not confirmed")]
async fn account_unconfirmed(world: &mut FoundryWorld) {
    ensure_connected(world).await;
    seed_operator_account(world).await;
    let email = OPERATOR_EMAIL.to_string();
    if let Some(d) = world.kc_issuer.as_ref() {
        d.will_vouch_for(&email, false);
        d.will_mint(Variant::UnconfirmedEmail);
    }
    world.kc_subject_email = Some(email);
    world.kc_account_exists = true;
}

#[given("a person has a foundry account but belongs to no workspace")]
async fn account_without_workspace(world: &mut FoundryWorld) {
    ensure_connected(world).await;
    // A workspace exists — the person simply is not in it — so failing closed is
    // observable rather than the only thing an empty instance could do.
    seed_operator_account(world).await;
    seed_account(world, ORPHAN_EMAIL, ORPHAN_NAME).await;
    let email = ORPHAN_EMAIL.to_string();
    if let Some(d) = world.kc_issuer.as_ref() {
        d.will_vouch_for(&email, true);
    }
    world.kc_subject_email = Some(email);
    world.kc_account_exists = true;
    world.kc_has_workspace = false;
}

#[given("the identity provider cannot be reached")]
async fn provider_unreachable(world: &mut FoundryWorld) {
    // Shutting the double down leaves foundry pointed at a dead loopback port —
    // a genuine connection failure, not a simulated one.
    if let Some(d) = world.kc_issuer.as_ref() {
        d.shutdown();
    }
    world.kc_provider_reachable = false;
}

#[given("the operator has begun signing in with their cluster identity")]
async fn begun_signin(world: &mut FoundryWorld) {
    account_confirmed(world).await;
    start_sign_in(world).await;
}

#[given("the operator has signed in with their cluster identity")]
async fn has_signed_in(world: &mut FoundryWorld) {
    begun_signin(world).await;
    // Kept so a replay can re-present this sign-in's own state and challenge.
    world.kc_first_start_headers = world.last_headers.clone();
    complete_with_provider(world).await;
}

/// Ask foundry to start a federated sign-in; the start response is recorded.
async fn start_sign_in(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    begin_federated_sign_in(world).await;
    world.kc_start_status = world.last_status;
}

// ------------------------------------------------------------------- Whens

#[when("the operator chooses to sign in with their cluster identity")]
async fn choose_cluster_identity(world: &mut FoundryWorld) {
    start_sign_in(world).await;
}

/// Authenticating is the whole round-trip: when no sign-in has been begun in this
/// scenario, it begins one first.
#[when("they authenticate with the identity provider")]
async fn complete_with_provider(world: &mut FoundryWorld) {
    if world.kc_start_status.is_none() {
        start_sign_in(world).await;
    }
    finish_federated_sign_in(world).await;
}

#[when("a visitor opens the sign-in page")]
async fn open_sign_in(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let url = format!("{}{}", base(world), SIGN_IN_PATH);
    let resp = world
        .http
        .as_ref()
        .expect("client")
        .get(&url)
        .send()
        .await
        .expect("sign-in");
    record(world, resp).await;
}

#[when("the operator begins signing in with their cluster identity twice")]
async fn begin_twice(world: &mut FoundryWorld) {
    start_sign_in(world).await;
    let first = world.last_headers.clone();
    start_sign_in(world).await;
    world.kc_first_start_headers = first;
}

#[when("they file an issue")]
async fn file_issue_as_federated(world: &mut FoundryWorld) {
    seed_operator_project(world).await;
    let session = world
        .kc_session_cookie
        .clone()
        .expect("the cluster-identity sign-in established a session");
    let base = base(world);
    let http = client();
    // The CSRF token is minted for this session exactly as a browser receives it.
    let page = http
        .get(format!("{base}{SIGN_IN_PATH}"))
        .header(reqwest::header::COOKIE, session.clone())
        .send()
        .await
        .expect("csrf for the session");
    let csrf = set_cookie_pair(page.headers(), CSRF_COOKIE).expect("csrf cookie");
    let form = HashMap::from([
        ("title", FILED_TITLE.to_string()),
        ("_csrf", cookie_value(&csrf, CSRF_COOKIE)),
    ]);
    let resp = http
        .post(format!(
            "{base}/team/{TEAM_SLUG}/project/{PROJECT_SLUG}/issues"
        ))
        .header(reqwest::header::COOKIE, format!("{session}; {csrf}"))
        .header("hx-request", "true")
        .form(&form)
        .send()
        .await
        .expect("file an issue");
    world.kc_filed_issue = resp.status().is_success();
    record(world, resp).await;
}

/// Precondition for filing: a team the operator belongs to, holding a project, in
/// the operator's workspace.
async fn seed_operator_project(world: &FoundryWorld) {
    let workspace_id = world.kc_workspace_id.expect("the operator's workspace");
    let user_id = operator_id(world).await;
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool();
    let team_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Ops', $3)")
        .bind(team_id)
        .bind(workspace_id)
        .bind(TEAM_SLUG)
        .execute(pool)
        .await
        .expect("seed a team");
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')")
        .bind(team_id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("seed the team membership");
    let project_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix) \
              VALUES ($1, $2, $3, 'Homelab', $4, 'HL')",
    )
    .bind(project_id)
    .bind(team_id)
    .bind(workspace_id)
    .bind(PROJECT_SLUG)
    .execute(pool)
    .await
    .expect("seed a project");
    crate::support::harness::seed_lanes_for_project(pool, project_id).await;
}

#[when("someone arrives claiming to have signed in, having never begun")]
async fn arrive_without_starting(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let url = format!(
        "{}{}?code=fabricated&state=fabricated",
        base(world),
        CALLBACK_PATH
    );
    let resp = no_redirect_client()
        .get(&url)
        .send()
        .await
        .expect("callback");
    record(world, resp).await;
}

/// The genuine round-trip — the start's sealed challenge cookie carried by hand,
/// the provider's `/authorize` visited so the identity it mints answers the real
/// nonce — with exactly one thing wrong: the `state` the arrival answers. Only the
/// state comparison stands between this arrival and a session.
#[when("they arrive answering a different challenge")]
async fn arrive_wrong_state(world: &mut FoundryWorld) {
    let start = world.last_headers.clone().expect("the start response");
    let challenge =
        set_cookie_pair(&start, CHALLENGE_COOKIE).expect("start sets the challenge cookie");
    let authorize = start
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("start redirect names the provider")
        .to_string();
    let http = world.http.clone().expect("client");
    http.get(&authorize).send().await.expect("authorize");

    let code = format!("code-{}", uuid::Uuid::new_v4());
    let resp = http
        .get(format!("{}{}", base(world), CALLBACK_PATH))
        .query(&[
            ("code", code.as_str()),
            ("state", "a-challenge-nobody-issued"),
        ])
        .header(reqwest::header::COOKIE, challenge)
        .send()
        .await
        .expect("callback");
    record(world, resp).await;
}

#[when("the identity provider vouches for them against an earlier challenge")]
async fn provider_stale_nonce(world: &mut FoundryWorld) {
    if let Some(d) = world.kc_issuer.as_ref() {
        d.will_mint(Variant::StaleNonce);
    }
    complete_with_provider(world).await;
}

#[when("an identity signed by a key the provider does not publish arrives")]
async fn identity_unpublished_key(world: &mut FoundryWorld) {
    if let Some(d) = world.kc_issuer.as_ref() {
        d.will_mint(Variant::UnpublishedKey);
    }
    complete_with_provider(world).await;
}

#[when("an identity naming a different provider arrives")]
async fn identity_foreign_issuer(world: &mut FoundryWorld) {
    if let Some(d) = world.kc_issuer.as_ref() {
        d.will_mint(Variant::ForeignIssuer);
    }
    complete_with_provider(world).await;
}

#[when("an identity whose validity has already lapsed arrives")]
async fn identity_lapsed(world: &mut FoundryWorld) {
    if let Some(d) = world.kc_issuer.as_ref() {
        d.will_mint(Variant::Lapsed);
    }
    complete_with_provider(world).await;
}

#[when("that same sign-in is presented a second time")]
async fn replay_completed_signin(world: &mut FoundryWorld) {
    // Replay the GENUINE code, state AND challenge cookie of the completed sign-in.
    // Everything foundry checks before the exchange still matches, so the refusal
    // can only come from the provider accepting an authorization code once — NOT
    // from the challenge cookie having been cleared (feature-delta.md § Changed
    // Assumptions, AC-3.5). The session it established rides along, as a browser
    // would send it, so a refusal that disturbs it is observable.
    let code = world.kc_last_code.clone().expect("a completed sign-in");
    let start = world
        .kc_first_start_headers
        .clone()
        .expect("the completed sign-in's start response");
    let challenge =
        set_cookie_pair(&start, CHALLENGE_COOKIE).expect("start sets the challenge cookie");
    let state = challenge_params(Some(&start))
        .map(|(state, _)| state)
        .expect("start redirect carries state");
    let session = world
        .kc_session_cookie
        .clone()
        .expect("the completed sign-in established a session");
    let resp = client()
        .get(format!("{}{}", base(world), CALLBACK_PATH))
        .query(&[("code", code.as_str()), ("state", state.as_str())])
        .header(reqwest::header::COOKIE, format!("{challenge}; {session}"))
        .send()
        .await
        .expect("callback");
    record(world, resp).await;
}

/// The provider is already unreachable, so the start itself may refuse — that
/// refusal (no hand-off at all) is the answer the operator gets.
#[when("the operator tries to sign in with their cluster identity")]
async fn try_sign_in_unreachable(world: &mut FoundryWorld) {
    begun_signin(world).await;
    if world.last_status == Some(StatusCode::FOUND) {
        finish_federated_sign_in(world).await;
    }
}

#[when("each way of being turned away is attempted in turn")]
async fn every_refusal(world: &mut FoundryWorld) {
    let mut seen = Vec::new();
    for variant in [
        Variant::UnconfirmedEmail,
        Variant::UnpublishedKey,
        Variant::ForeignIssuer,
        Variant::Lapsed,
        Variant::StaleNonce,
    ] {
        if let Some(d) = world.kc_issuer.as_ref() {
            d.will_mint(variant);
        }
        begun_signin(world).await;
        complete_with_provider(world).await;
        seen.push(last_answer(world));
    }

    // A well-formed identity that foundry itself turns away: no account (refused
    // before any provisioning, D3), and an account in no workspace (refused where
    // the session would be established — a separately rendered branch).
    if let Some(d) = world.kc_issuer.as_ref() {
        d.will_mint(Variant::Valid);
    }
    seed_account(world, ORPHAN_EMAIL, ORPHAN_NAME).await;
    for email in [STRANGER_EMAIL, ORPHAN_EMAIL] {
        begun_signin(world).await;
        if let Some(d) = world.kc_issuer.as_ref() {
            d.will_vouch_for(email, true);
        }
        complete_with_provider(world).await;
        seen.push(last_answer(world));
    }

    // Arrivals refused before the provider is ever asked.
    arrive_without_starting(world).await;
    seen.push(last_answer(world));
    begun_signin(world).await;
    arrive_wrong_state(world).await;
    seen.push(last_answer(world));

    world.kc_refusals = seen;
}

fn last_answer(world: &FoundryWorld) -> (StatusCode, String) {
    (
        world.last_status.expect("status"),
        world.last_body.clone().unwrap_or_default(),
    )
}

#[when("they sign in with their foundry password")]
async fn password_sign_in(world: &mut FoundryWorld) {
    world.kc_password_path_used = true;
    let resp = password_door(world, OPERATOR_EMAIL, OPERATOR_PASSWORD).await;
    world.kc_session_cookie = set_cookie_pair(resp.headers(), SESSION_COOKIE);
    record(world, resp).await;
}

/// The real claim: a bootstrap token minted at the store boundary (as the startup
/// hook mints one), then the claim form posted to `/bootstrap`, signed out.
#[when("the first operator claims the instance")]
async fn claim_instance(world: &mut FoundryWorld) {
    world.kc_claimed_instance = true;
    let harness = world.harness.as_ref().expect("harness");
    let expires_at = harness.fake_clock.now() + time::Duration::minutes(30);
    harness
        .app
        .state
        .store
        .insert_bootstrap_token(uuid::Uuid::now_v7(), &sha256(CLAIM_TOKEN), expires_at)
        .await
        .expect("mint a bootstrap token");
    let form = HashMap::from([
        ("email", CLAIMANT_EMAIL),
        ("password", CLAIMANT_PASSWORD),
        ("display_name", CLAIMANT_NAME),
        ("workspace_name", CLAIMANT_WORKSPACE),
    ]);
    let resp = client()
        .post(format!("{}/bootstrap?token={CLAIM_TOKEN}", base(world)))
        .form(&form)
        .send()
        .await
        .expect("claim the instance");
    world.kc_session_cookie = set_cookie_pair(resp.headers(), SESSION_COOKIE);
    record(world, resp).await;
}

fn sha256(s: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(s.as_bytes()).into()
}

/// Through the door the person actually uses: the federated session's person is
/// noted, the footer sign-out is posted as a browser posts it, then the password
/// form is submitted.
#[when("they sign out and sign in again with their foundry password")]
async fn switch_doors(world: &mut FoundryWorld) {
    let federated = world
        .kc_session_cookie
        .clone()
        .expect("the cluster-identity sign-in established a session");
    world.kc_federated_user_id = Some(
        session_user_id(world, &federated)
            .await
            .expect("the federated session belongs to no user"),
    );
    sign_out(world, &federated).await;
    password_sign_in(world).await;
}

async fn sign_out(world: &FoundryWorld, session: &str) {
    let base = base(world);
    let http = client();
    let page = http
        .get(format!("{base}{SIGN_IN_PATH}"))
        .header(reqwest::header::COOKIE, session)
        .send()
        .await
        .expect("csrf for the session");
    let csrf = set_cookie_pair(page.headers(), CSRF_COOKIE).expect("csrf cookie");
    let form = HashMap::from([("_csrf", cookie_value(&csrf, CSRF_COOKIE))]);
    let resp = http
        .post(format!("{base}/sign-out"))
        .header(reqwest::header::COOKIE, format!("{session}; {csrf}"))
        .form(&form)
        .send()
        .await
        .expect("sign out");
    assert_eq!(
        resp.status(),
        StatusCode::SEE_OTHER,
        "signing out of the federated session was refused"
    );
    assert_eq!(
        session_user_id(world, session).await,
        None,
        "the federated session survived signing out"
    );
}

/// The user a session belongs to, read at the store boundary: the session row the
/// cookie names holds its user id. `None` once the session is gone.
async fn session_user_id(world: &FoundryWorld, session: &str) -> Option<uuid::Uuid> {
    let id = cookie_value(session, SESSION_COOKIE);
    let found: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT u.id FROM session s JOIN users u \
              ON position(convert_to(u.id::text, 'UTF8') IN s.data) > 0 \
          WHERE s.id = $1",
    )
    .bind(id)
    .fetch_optional(
        world
            .harness
            .as_ref()
            .expect("harness")
            .app
            .state
            .store
            .pool(),
    )
    .await
    .expect("read the session's user");
    found.map(|(id,)| id)
}

#[when("someone asks to sign in with a cluster identity")]
async fn ask_when_unconfigured(world: &mut FoundryWorld) {
    start_sign_in(world).await;
}

#[when("foundry starts")]
async fn foundry_starts(world: &mut FoundryWorld) {
    world.kc_start_attempted = true;
}

// ------------------------------------------------------------------- Thens

/// The door a scenario came in through, read from what its When recorded.
#[derive(Debug, Clone, Copy)]
enum Door {
    ClusterIdentity,
    Password,
    InstanceClaim,
}

impl Door {
    fn used(world: &FoundryWorld) -> Self {
        if world.kc_claimed_instance {
            Door::InstanceClaim
        } else if world.kc_password_path_used {
            Door::Password
        } else {
            Door::ClusterIdentity
        }
    }

    /// Each door's documented landing: the claim lands on the workspace dashboard
    /// (bootstrap.rs, pinned by us_05_bootstrap); both sign-in doors on the board.
    fn landing(self) -> &'static str {
        match self {
            Door::InstanceClaim => "/dashboard",
            Door::ClusterIdentity | Door::Password => "/",
        }
    }

    fn person(self) -> &'static str {
        match self {
            Door::InstanceClaim => CLAIMANT_NAME,
            Door::ClusterIdentity | Door::Password => OPERATOR_NAME,
        }
    }
}

#[then("they arrive at their board signed in as themselves")]
async fn arrive_signed_in(world: &mut FoundryWorld) {
    let door = Door::used(world);
    let status = world.last_status.expect("a response was captured");
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "expected the {door:?} door to redirect onto the board; got {status}"
    );
    let loc = world
        .last_headers
        .as_ref()
        .and_then(|h| h.get(reqwest::header::LOCATION))
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        loc,
        door.landing(),
        "the {door:?} door landed somewhere other than its documented landing"
    );
    if let Door::InstanceClaim = door {
        assert_landing_signed_in(world, door.landing()).await;
    }
    assert_session_opens_board_as(world, door.person(), "the session does not open the board")
        .await;
    if let Some(federated) = world.kc_federated_user_id {
        let session = world.kc_session_cookie.clone().expect("a session");
        assert_eq!(
            session_user_id(world, &session).await,
            Some(federated),
            "the password door reached a different person than the cluster identity did"
        );
    }
}

/// Follow the redirect with the session cookie: the landing serves a signed-in page.
async fn assert_landing_signed_in(world: &FoundryWorld, landing: &str) {
    let session = world
        .kc_session_cookie
        .clone()
        .expect("the claim established no session");
    let resp = client()
        .get(format!("{}{landing}", base(world)))
        .header(reqwest::header::COOKIE, session)
        .send()
        .await
        .expect("landing");
    assert_eq!(resp.status(), StatusCode::OK, "{landing} did not open");
    let body = resp.text().await.unwrap_or_default();
    assert!(
        body.contains("Signed in: true"),
        "{landing} does not see the claimant signed in"
    );
}

/// The session the sign-in established opens the board, signed in as the operator.
async fn assert_session_opens_board(world: &FoundryWorld, failure: &str) {
    assert_session_opens_board_as(world, OPERATOR_NAME, failure).await;
}

/// The session the sign-in established opens the board, greeting `name`.
async fn assert_session_opens_board_as(world: &FoundryWorld, name: &str, failure: &str) {
    let session = world
        .kc_session_cookie
        .clone()
        .expect("the sign-in established no session");
    let resp = client()
        .get(format!("{}/", base(world)))
        .header(reqwest::header::COOKIE, session)
        .send()
        .await
        .expect("board");
    assert_eq!(resp.status(), StatusCode::OK, "{failure}");
    let body = resp.text().await.unwrap_or_default();
    let greeting = format!("Welcome back, {name}</p>");
    assert!(
        body.contains(&greeting),
        "the board is not signed in as {name} (expected {greeting:?})"
    );
}

#[then("they are offered a way to sign in with their cluster identity")]
async fn offered_cluster_identity(world: &mut FoundryWorld) {
    let body = world.last_body.clone().unwrap_or_default();
    assert!(
        body.contains(START_PATH),
        "the sign-in page offers no cluster-identity control (expected a link to {START_PATH})"
    );
}

#[then("they are not offered a way to sign in with a cluster identity")]
async fn not_offered_cluster_identity(world: &mut FoundryWorld) {
    let body = world.last_body.clone().unwrap_or_default();
    assert!(
        !body.contains(START_PATH),
        "the sign-in page offers a cluster-identity control while none is configured"
    );
}

#[then("each attempt carries a different challenge")]
async fn challenges_differ(world: &mut FoundryWorld) {
    let first = challenge_params(world.kc_first_start_headers.as_ref());
    let second = challenge_params(world.last_headers.as_ref());
    let (Some((first_state, first_nonce)), Some((second_state, second_nonce))) = (first, second)
    else {
        panic!("an attempt did not hand off to the provider with a state and a nonce");
    };
    assert_ne!(
        first_state, second_state,
        "two sign-in attempts reused one state"
    );
    assert_ne!(
        first_nonce, second_nonce,
        "two sign-in attempts reused one nonce"
    );
}

/// The `state` and `nonce` a start redirect hands to the provider — the challenge
/// itself, as opposed to the PKCE parameters that ride alongside it.
fn challenge_params(headers: Option<&reqwest::header::HeaderMap>) -> Option<(String, String)> {
    let location = headers?.get(reqwest::header::LOCATION)?.to_str().ok()?;
    let url = reqwest::Url::parse(location).ok()?;
    let param = |name: &str| {
        url.query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
    };
    Some((param("state")?, param("nonce")?))
}

#[then("the issue is recorded as authored by them")]
async fn issue_authored_by_them(world: &mut FoundryWorld) {
    assert!(
        world.kc_filed_issue,
        "filing through the federated session was refused: {:?} {}",
        world.last_status,
        world.last_body.clone().unwrap_or_default()
    );
    let authors: Vec<(uuid::Uuid,)> =
        sqlx::query_as("SELECT author_id FROM issues WHERE title = $1")
            .bind(FILED_TITLE)
            .fetch_all(
                world
                    .harness
                    .as_ref()
                    .expect("harness")
                    .app
                    .state
                    .store
                    .pool(),
            )
            .await
            .expect("read the filed issue");
    assert_eq!(
        authors,
        vec![(operator_id(world).await,)],
        "the issue is not recorded as authored by the operator"
    );
}

#[then("no challenge remains held by their browser")]
async fn challenge_cleared(world: &mut FoundryWorld) {
    let prefix = format!("{CHALLENGE_COOKIE}=");
    let cleared = world
        .last_headers
        .as_ref()
        .map(|h| {
            h.get_all(reqwest::header::SET_COOKIE)
                .iter()
                .filter_map(|v| v.to_str().ok())
                .filter(|c| c.starts_with(&prefix))
                .any(|c| c.contains("Max-Age=0") || c.contains("expires=Thu, 01 Jan 1970"))
        })
        .unwrap_or(false);
    assert!(cleared, "the one-time challenge cookie was not cleared");
}

#[then("they are returned to the sign-in page and told nothing more")]
async fn refused_uniformly(world: &mut FoundryWorld) {
    let status = world.last_status.expect("a response was captured");
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "expected the generic refusal; got {status}"
    );
    let session = world
        .last_headers
        .as_ref()
        .and_then(|h| set_cookie_pair(h, SESSION_COOKIE));
    assert!(session.is_none(), "a refusal must not establish a session");
    let body = world.last_body.clone().unwrap_or_default();
    // D7: byte-identical (CSRF token aside) to a real wrong-password refusal.
    let (baseline_status, baseline_body) = wrong_password_refusal(world).await;
    assert_eq!(
        status, baseline_status,
        "the refusal's status differs from a wrong password's"
    );
    assert_eq!(
        mask_csrf(&body),
        mask_csrf(&baseline_body),
        "the refusal is distinguishable from a wrong password's — an account-existence oracle"
    );
}

/// A real wrong-password attempt at the password door, made as a browser makes it.
async fn wrong_password_refusal(world: &FoundryWorld) -> (StatusCode, String) {
    let resp = password_door(world, UNKNOWN_EMAIL, "a-wrong-password-long-enough").await;
    let status = resp.status();
    (status, resp.text().await.unwrap_or_default())
}

/// The password door as a browser uses it: open `GET /sign-in` (which mints the
/// CSRF cookie), then submit the form with it.
async fn password_door(world: &FoundryWorld, email: &str, password: &str) -> reqwest::Response {
    let base = base(world);
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
    http.post(format!("{base}{SIGN_IN_PATH}"))
        .header(reqwest::header::COOKIE, csrf)
        .form(&form)
        .send()
        .await
        .expect("password sign-in")
}

/// Mask the per-request CSRF token so two refusals rendered for two requests
/// compare byte-for-byte on everything else.
fn mask_csrf(body: &str) -> String {
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

#[then("no foundry account has been created for them")]
async fn no_account_created(world: &mut FoundryWorld) {
    let email = world.kc_subject_email.clone().expect("a subject email");
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool()
        .clone();
    let found: Option<(uuid::Uuid,)> =
        sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
            .bind(email.to_lowercase())
            .fetch_optional(&pool)
            .await
            .expect("users lookup");
    assert!(
        found.is_none(),
        "the federated path created a foundry account for {email} — it must provision nothing"
    );
}

#[then("their original session is untouched")]
async fn original_session_untouched(world: &mut FoundryWorld) {
    assert!(
        world.kc_last_code.is_some(),
        "no completed sign-in to replay"
    );
    let reissued = world
        .last_headers
        .as_ref()
        .and_then(|h| set_cookie_pair(h, SESSION_COOKIE));
    assert!(
        reissued.is_none(),
        "the refused replay touched the session cookie: {reissued:?}"
    );
    assert_session_opens_board(world, "the refused replay ended the original session").await;
}

#[then("foundry keeps serving every other page")]
async fn still_serving(world: &mut FoundryWorld) {
    let url = format!("{}/healthz", base(world));
    let resp = no_redirect_client()
        .get(&url)
        .send()
        .await
        .expect("healthz");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "foundry stopped serving because the identity provider was unreachable"
    );
}

#[then("every one of them is answered identically")]
async fn refusals_identical(world: &mut FoundryWorld) {
    let seen = &world.kc_refusals;
    assert!(seen.len() >= 2, "fewer than two refusals were collected");
    // Each refusal carries its own freshly minted CSRF token; everything else
    // must match byte for byte.
    let (first_status, first_body) = &seen[0];
    for (i, (status, body)) in seen.iter().enumerate().skip(1) {
        assert_eq!(
            (first_status, mask_csrf(first_body)),
            (status, mask_csrf(body)),
            "refusal {i} differs from refusal 0 — the branches are distinguishable"
        );
    }
}

#[then("a wrong password is answered identically too")]
async fn password_refusal_identical(world: &mut FoundryWorld) {
    assert!(
        !world.kc_refusals.is_empty(),
        "no federated refusals collected"
    );
    let (baseline_status, baseline_body) = wrong_password_refusal(world).await;
    for (i, (status, body)) in world.kc_refusals.iter().enumerate() {
        assert_eq!(
            (*status, mask_csrf(body)),
            (baseline_status, mask_csrf(&baseline_body)),
            "refusal {i} is distinguishable from a wrong password's — an account-existence oracle"
        );
    }
}

#[then("foundry reports itself healthy and ready")]
async fn healthy_and_ready(world: &mut FoundryWorld) {
    for path in ["/healthz", "/readyz"] {
        let url = format!("{}{}", base(world), path);
        let resp = no_redirect_client().get(&url).send().await.expect("probe");
        assert_eq!(resp.status(), StatusCode::OK, "{path} did not answer OK");
    }
}

#[then("it refuses to start and names the missing credential")]
async fn refuses_to_start(world: &mut FoundryWorld) {
    assert!(
        world.kc_partial_config,
        "the scenario did not half-configure foundry"
    );
    assert!(world.kc_start_attempted, "foundry was never started");
    panic!("startup refusal on partial OIDC config is not yet implemented — DELIVER adds it");
}
