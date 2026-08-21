//! Step definitions for `keycloak-sso` — signing in to foundry with the cluster
//! identity (`tests/features/keycloak-sso.feature`, 23 scenarios, all `@pending`).
//!
//! RED, not BROKEN (Mandate 7, this project's variant): NO new production
//! panic-stub is committed. The shipped `build_router`, `signin.rs` and templates
//! are untouched, and the two new endpoints are referenced ONLY as the path string
//! literals below. This module therefore COMPILES against current production; an
//! unskipped scenario fails at an ASSERTION (404 where a redirect was expected, an
//! absent control, an absent session), which is the correct RED.
//!
//! The identity provider is `support::oidc_issuer` — an in-process axum double on
//! `127.0.0.1:0` signing with a FIXED RSA test keypair. Real RS256 crypto, fixture
//! key material, mirroring the shipped machine-token keypair. Postgres is REAL
//! (shared testcontainer, per-scenario schema).
//!
//! KNOWN RED GAP, deliberate: `InProcHarness` has no `spawn_with_oidc` constructor
//! because `AppState` has no `oidc` field yet. The "foundry is connected to the
//! cluster identity provider" Given therefore starts the double and RECORDS the
//! issuer, but cannot yet point foundry at it. DELIVER adds the field, the
//! composition-root wiring, and the constructor, then replaces the `todo` marker in
//! `connect_provider` below. Until then every OIDC scenario fails on the 404 from
//! the unmounted route — the right failure for the right reason.
//!
//! Reused Givens (cucumber-rs requires globally-unique step text — every step
//! phrase in this module is scoped to "cluster identity" wording to avoid
//! colliding with the shipped sign-in and bootstrap modules).

use crate::support::harness::InProcHarness;
use crate::support::oidc_issuer::{OidcIssuerDouble, Variant};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use reqwest::StatusCode;

/// DESIGN OD-3 pinned these. If DELIVER moves them, the Keycloak client's redirect
/// URI in the homelab repo moves in the same change.
const START_PATH: &str = "/auth/oidc/start";
const CALLBACK_PATH: &str = "/auth/oidc/callback";
const SIGN_IN_PATH: &str = "/sign-in";

const TEST_NOW: &str = "2026-01-15T12:00:00Z";

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
    ensure_harness(world).await;
    let double = OidcIssuerDouble::start("foundry").await;
    world.kc_issuer_url = Some(double.issuer());
    world.kc_issuer = Some(double);
    // DELIVER: replace this harness with `InProcHarness::spawn_with_oidc(now(),
    // &issuer, "foundry", "test-secret")` once `AppState.oidc` exists. Until then
    // foundry is NOT actually pointed at the double, so the scenarios fail on the
    // unmounted route — RED for MISSING_FUNCTIONALITY.
    world.kc_provider_configured = true;
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
    let email = "operator@example.test".to_string();
    if let Some(d) = world.kc_issuer.as_ref() {
        d.will_vouch_for(&email, true);
    }
    world.kc_subject_email = Some(email);
    world.kc_account_exists = true;
}

#[given("a person known to the identity provider has no foundry account")]
async fn no_account(world: &mut FoundryWorld) {
    let email = "stranger@example.test".to_string();
    if let Some(d) = world.kc_issuer.as_ref() {
        d.will_vouch_for(&email, true);
    }
    world.kc_subject_email = Some(email);
    world.kc_account_exists = false;
}

#[given("the operator has a foundry account for an address the provider has not confirmed")]
async fn account_unconfirmed(world: &mut FoundryWorld) {
    let email = "operator@example.test".to_string();
    if let Some(d) = world.kc_issuer.as_ref() {
        d.will_vouch_for(&email, false);
        d.will_mint(Variant::UnconfirmedEmail);
    }
    world.kc_subject_email = Some(email);
    world.kc_account_exists = true;
}

#[given("a person has a foundry account but belongs to no workspace")]
async fn account_without_workspace(world: &mut FoundryWorld) {
    let email = "orphan@example.test".to_string();
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
    ensure_harness(world).await;
    let url = format!("{}{}", base(world), START_PATH);
    let resp = world
        .http
        .as_ref()
        .expect("client")
        .get(&url)
        .send()
        .await
        .expect("start");
    world.kc_start_status = Some(resp.status());
    record(world, resp).await;
}

#[given("the operator has signed in with their cluster identity")]
async fn has_signed_in(world: &mut FoundryWorld) {
    begun_signin(world).await;
    complete_with_provider(world).await;
}

// ------------------------------------------------------------------- Whens

#[when("the operator chooses to sign in with their cluster identity")]
async fn choose_cluster_identity(world: &mut FoundryWorld) {
    begun_signin(world).await;
}

#[when("they authenticate with the identity provider")]
async fn complete_with_provider(world: &mut FoundryWorld) {
    let code = format!("code-{}", uuid::Uuid::new_v4());
    world.kc_last_code = Some(code.clone());
    let url = format!(
        "{}{}?code={}&state=from-cookie",
        base(world),
        CALLBACK_PATH,
        code
    );
    let resp = world
        .http
        .as_ref()
        .expect("client")
        .get(&url)
        .send()
        .await
        .expect("callback");
    record(world, resp).await;
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
    begun_signin(world).await;
    let first = world.last_headers.clone();
    begun_signin(world).await;
    world.kc_first_start_headers = first;
}

#[when("they file an issue")]
async fn file_issue_as_federated(world: &mut FoundryWorld) {
    world.kc_filed_issue = true;
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

#[when("they arrive answering a different challenge")]
async fn arrive_wrong_state(world: &mut FoundryWorld) {
    let url = format!(
        "{}{}?code=c&state=a-challenge-nobody-issued",
        base(world),
        CALLBACK_PATH
    );
    let resp = world
        .http
        .as_ref()
        .expect("client")
        .get(&url)
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
    // Replay the GENUINE code. Refusal comes from the provider accepting an
    // authorization code once — NOT from the challenge cookie having been
    // cleared (feature-delta.md § Changed Assumptions, AC-3.5).
    let code = world.kc_last_code.clone().expect("a completed sign-in");
    let url = format!(
        "{}{}?code={}&state=from-cookie",
        base(world),
        CALLBACK_PATH,
        code
    );
    let resp = world
        .http
        .as_ref()
        .expect("client")
        .get(&url)
        .send()
        .await
        .expect("callback");
    record(world, resp).await;
}

#[when("the operator tries to sign in with their cluster identity")]
async fn try_sign_in_unreachable(world: &mut FoundryWorld) {
    begun_signin(world).await;
    complete_with_provider(world).await;
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
        seen.push((
            world.last_status.expect("status"),
            world.last_body.clone().unwrap_or_default(),
        ));
    }
    world.kc_refusals = seen;
}

#[when("they sign in with their foundry password")]
async fn password_sign_in(world: &mut FoundryWorld) {
    world.kc_password_path_used = true;
}

#[when("the first operator claims the instance")]
async fn claim_instance(world: &mut FoundryWorld) {
    world.kc_claimed_instance = true;
}

#[when("they sign out and sign in again with their foundry password")]
async fn switch_doors(world: &mut FoundryWorld) {
    world.kc_password_path_used = true;
}

#[when("someone asks to sign in with a cluster identity")]
async fn ask_when_unconfigured(world: &mut FoundryWorld) {
    begun_signin(world).await;
}

#[when("foundry starts")]
async fn foundry_starts(world: &mut FoundryWorld) {
    world.kc_start_attempted = true;
}

// ------------------------------------------------------------------- Thens

#[then("they arrive at their board signed in as themselves")]
async fn arrive_signed_in(world: &mut FoundryWorld) {
    let status = world.last_status.expect("a response was captured");
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "expected a redirect onto the board; got {status}"
    );
    let loc = world
        .last_headers
        .as_ref()
        .and_then(|h| h.get(reqwest::header::LOCATION))
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert_eq!(loc, "/", "expected to land on the board, got {loc:?}");
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
    let first = world
        .kc_first_start_headers
        .as_ref()
        .and_then(|h| h.get(reqwest::header::LOCATION))
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let second = world
        .last_headers
        .as_ref()
        .and_then(|h| h.get(reqwest::header::LOCATION))
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        !first.is_empty(),
        "the first attempt did not hand off to the provider"
    );
    assert_ne!(first, second, "two sign-in attempts reused one challenge");
}

#[then("the issue is recorded as authored by them")]
async fn issue_authored_by_them(world: &mut FoundryWorld) {
    assert!(world.kc_filed_issue, "no issue was filed");
    let email = world.kc_subject_email.clone().unwrap_or_default();
    panic!("authorship for {email} is not yet observable — DELIVER wires the federated session");
}

#[then("no challenge remains held by their browser")]
async fn challenge_cleared(world: &mut FoundryWorld) {
    let cleared = world
        .last_headers
        .as_ref()
        .map(|h| {
            h.get_all(reqwest::header::SET_COOKIE)
                .iter()
                .filter_map(|v| v.to_str().ok())
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
    let body = world.last_body.clone().unwrap_or_default();
    assert!(
        !body.to_lowercase().contains("no such account")
            && !body.to_lowercase().contains("not found"),
        "the refusal names why it refused — that is an account-existence oracle"
    );
    let set_cookie = world
        .last_headers
        .as_ref()
        .map(|h| h.get_all(reqwest::header::SET_COOKIE).iter().count())
        .unwrap_or(0);
    let _ = set_cookie;
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
    panic!("session durability across a replay is not yet observable — DELIVER wires it");
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
    let first = &seen[0];
    for (i, other) in seen.iter().enumerate().skip(1) {
        assert_eq!(
            first, other,
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
    panic!("cross-path refusal comparison needs the federated path — DELIVER wires it");
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
