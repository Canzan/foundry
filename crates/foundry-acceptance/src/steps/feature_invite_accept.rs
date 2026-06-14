//! invite-accept-flow (US-01/02/03) step definitions.
//!
//! A NEW PUBLIC web driving adapter — the `/invites/accept` GET+POST route pair
//! that turns the emitted (today DEAD) invite link into a live claim-your-account
//! vertical: verify the signed `InviteToken` → render a set-password form naming
//! the workspace → atomically consume the single-use invite + write the argon2id
//! password in ONE tx → auto sign-in → land on the workspace. (architecture.md
//! C4-L3; ADR-001 consume-tx, ADR-004 password policy.)
//!
//! Step 01-01 implements ONLY the `@walking_skeleton` scenario: a first-admin with
//! a live signed invite opens the GET, sets a policy-passing password, and lands on
//! her workspace signed in — seeing only her tenant, with the invite consumed
//! exactly once. The thinnest cut proving the NEW public route wires through the
//! SHIPPED session + double-submit CSRF layers to the NEW one-TX consume+write and
//! back to an auto-signed-in landing.
//!
//! Driving adapter: the in-process axum router served by foundry-app over real
//! HTTP (the `InProcHarness`), under the SHIPPED session + CSRF layers — mirrors
//! `feature_web_provisioning_flow`. The Background seeds a REAL invite by running
//! the SHIPPED `Store::provision_workspace` tx (which creates the workspace + the
//! first-admin user + the `invites` row with `used_at`/`used_by` defaulting NULL)
//! and minting the InviteToken signature with the harness `session_secret` — so
//! the token under test is genuine, not synthesised.
//!
//! LAYER 3 (real adapter + real HTTP, @real-io @wiring_e2e): real Postgres via
//! testcontainers + per-scenario schema; the real tower-sessions Postgres store;
//! the real double-submit CSRF middleware; the SHIPPED `InviteToken::verify` +
//! `hash_password` + `resolve_active_workspace`; the NEW
//! `Store::set_first_admin_password_and_consume` + `check_password_policy`.
//! Example-based (Mandates 9 + 11) — no PBT at this layer; assertions are
//! traditional, over port-exposed web observables (rendered form substrings, the
//! 303 redirect + auto-sign-in session cookie, the landed tenant, and the
//! post-consume `invites.used_at` set exactly once).

use crate::support::harness::InProcHarness;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use foundry_store::Store;
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use secrecy::SecretString;
use std::sync::Arc;

/// The password Priya chooses — meets the min-12 length-first policy (ADR-004).
const PRIYA_PASSWORD: &str = "northwind-secure-pass";
/// Priya's first-admin email (the invite's `invitee_email` / the user row).
const PRIYA_EMAIL: &str = "priya@northwind.example";

fn harness(world: &FoundryWorld) -> &InProcHarness {
    world
        .ia_harness
        .as_ref()
        .expect("the invite-accept Background must have spawned the ia harness")
}

fn http(world: &mut FoundryWorld) -> reqwest::Client {
    if world.http.is_none() {
        world.http = Some(
            reqwest::Client::builder()
                .redirect(Policy::none())
                .cookie_store(false)
                .build()
                .expect("build reqwest client"),
        );
    }
    world.http.as_ref().expect("http client").clone()
}

// ---------------------------------------------------------------------------
// Background
// ---------------------------------------------------------------------------

/// `Given a super-admin provisioned the "Northwind" workspace` — spawn the
/// in-process harness and provision the workspace + first-admin + invite row via
/// the SHIPPED `Store::provision_workspace` tx (the same seam the web/CLI provision
/// legs drive). The invite row lands with `used_at`/`used_by` NULL — exactly the
/// "unconsumed" state the consume guard requires.
#[given(regex = r#"^a super-admin provisioned the "([^"]+)" workspace$"#)]
async fn super_admin_provisioned_workspace(world: &mut FoundryWorld, ws_name: String) {
    let harness = InProcHarness::spawn(time::OffsetDateTime::now_utc()).await;
    let store: Arc<Store> = harness.app.state.store.clone();
    let now = harness.app.state.clock.now();

    let workspace_id = uuid::Uuid::now_v7();
    let admin_user_id = uuid::Uuid::now_v7();
    let invite_id = uuid::Uuid::now_v7();
    let expires_at = now + time::Duration::days(7);

    // A throwaway initial credential — Priya has never seen it; the accept flow is
    // the only way she sets a real one (mirrors the provisioning leg's behaviour).
    let throwaway_hash = foundry_auth::hash_password(&SecretString::new(
        "never-seen-initial-credential".to_string().into(),
    ))
    .await
    .expect("hash throwaway initial credential");

    store
        .provision_workspace(
            workspace_id,
            &ws_name,
            admin_user_id,
            PRIYA_EMAIL,
            PRIYA_EMAIL,
            "Priya Nair",
            &throwaway_hash,
            invite_id,
            expires_at,
        )
        .await
        .expect("provision the Northwind workspace + first-admin + invite row");

    world.ia_workspace_ids.insert(ws_name, workspace_id);
    world.ia_invite_id = Some(invite_id);
    world.ia_admin_user_id = Some(admin_user_id);
    world.ia_harness = Some(harness);
    let _ = http(world);
}

/// `And Priya Nair was seeded as its first-admin with a live invite link valid for
/// 7 days` — mint the InviteToken HMAC signature over `invite_id|expires_at` with
/// the harness `session_secret` (the SAME secret the GET/POST handlers verify),
/// recovering `expires_at` from the seeded invite row. This is the genuine signed
/// `sig` the accept URL carries.
#[given(
    regex = r#"^Priya Nair was seeded as its first-admin with a live invite link valid for 7 days$"#
)]
async fn priya_seeded_with_live_invite(world: &mut FoundryWorld) {
    let invite_id = world.ia_invite_id.expect("invite seeded in the Background");
    let store = harness(world).app.state.store.clone();
    let expires_at = store
        .invite_expires_at(invite_id)
        .await
        .expect("read seeded invite expiry")
        .expect("the seeded invite row exists");
    let secret = harness(world).app.state.session_secret.clone();
    let token = foundry_auth::InviteToken::new(invite_id, expires_at, &secret)
        .expect("mint the live invite signature");
    world.ia_invite_sig = Some(token.signature);
}

// ---------------------------------------------------------------------------
// Walking skeleton (step 01-01)
// ---------------------------------------------------------------------------

/// `Given Priya has opened her live invite for "Northwind" and seen the
/// set-password form` — drive the NEW public GET `/invites/accept?id=&sig=` over
/// real HTTP. The handler verifies the signature + advisory liveness, mints the
/// CSRF cookie, and renders the set-password form NAMING the workspace. Capture
/// the rendered page so the form/workspace-name observables are asserted, and the
/// CSRF cookie so the subsequent POST carries a matching double-submit pair.
#[given(
    regex = r#"^Priya has opened her live invite for "([^"]+)" and seen the set-password form$"#
)]
async fn priya_opened_live_invite(world: &mut FoundryWorld, ws_name: String) {
    let invite_id = world.ia_invite_id.expect("invite seeded");
    let sig = world
        .ia_invite_sig
        .clone()
        .expect("invite signature minted");
    let base = harness(world).base_url();
    let client = http(world);

    let resp = client
        .get(format!(
            "{base}/invites/accept?id={invite_id}&sig={sig}",
            sig = urlencoding::encode(&sig)
        ))
        .send()
        .await
        .expect("GET /invites/accept");
    let status = resp.status();
    // Capture the CSRF cookie minted on the GET (D4/adr-003 — public-route seam).
    let csrf_cookie = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(|s| s.to_string());
    let body = resp.text().await.unwrap_or_default();

    assert_eq!(
        status,
        StatusCode::OK,
        "the GET accept page for a live invite must render a 200 set-password form; body = {body:?}"
    );
    assert!(
        body.contains(r#"action="/invites/accept""#) && body.contains(r#"name="password""#),
        "the GET must render a set-password form posting to /invites/accept; got {body:?}"
    );
    assert!(
        body.contains(&ws_name),
        "the set-password form must NAME the {ws_name:?} workspace; got {body:?}"
    );

    world.last_body = Some(body);
    world.ia_session_cookie = None;
    world.session_cookie_header = csrf_cookie;
}

/// `When she sets a password meeting the strength policy and confirms it` — drive
/// the NEW public POST `/invites/accept` over real HTTP carrying the double-submit
/// `_csrf` (cookie + form field), the token, and a policy-passing password +
/// matching confirm. The SHIPPED `csrf_middleware` screens the token; the handler
/// re-verifies, runs the policy, performs the one-TX consume+write, establishes a
/// session, and 303-redirects. Capture the 303, the Location, and the auto-sign-in
/// session cookie.
#[when(regex = r#"^she sets a password meeting the strength policy and confirms it$"#)]
async fn she_sets_a_valid_password(world: &mut FoundryWorld) {
    let invite_id = world.ia_invite_id.expect("invite seeded");
    let sig = world
        .ia_invite_sig
        .clone()
        .expect("invite signature minted");
    let csrf_cookie = world
        .session_cookie_header
        .clone()
        .expect("the GET minted a foundry_csrf cookie");
    let csrf_token = csrf_cookie
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();
    let base = harness(world).base_url();
    let client = http(world);

    let form = [
        ("id", invite_id.to_string()),
        ("sig", sig),
        ("password", PRIYA_PASSWORD.to_string()),
        ("confirm", PRIYA_PASSWORD.to_string()),
        ("_csrf", csrf_token.clone()),
    ];
    let resp = client
        .post(format!("{base}/invites/accept"))
        .header(
            reqwest::header::COOKIE,
            format!("foundry_csrf={csrf_token}"),
        )
        .form(&form)
        .send()
        .await
        .expect("POST /invites/accept");
    let status = resp.status();
    let location = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let session_cookie = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .and_then(|s| s.split(';').next())
        .map(str::to_string);

    world.ia_post_status = Some(status);
    world.ia_post_location = location;
    world.ia_session_cookie = session_cookie;
}

/// `Then she is signed in without a separate login step` — the POST answered with
/// a 303 SEE_OTHER (auto sign-in, decision 3) AND issued a `foundry_session`
/// cookie. The presence of the session cookie on the accept POST response is the
/// port-exposed observable that no separate `/sign-in` round-trip occurred.
#[then(regex = r#"^she is signed in without a separate login step$"#)]
async fn she_is_signed_in_without_login(world: &mut FoundryWorld) {
    assert_eq!(
        world.ia_post_status,
        Some(StatusCode::SEE_OTHER),
        "the accept POST must 303 SEE_OTHER on success (auto sign-in); got {:?}",
        world.ia_post_status
    );
    assert!(
        world.ia_session_cookie.is_some(),
        "the accept POST must establish a session (issue a foundry_session cookie) — \
         proving auto sign-in with no separate login step; got no session cookie"
    );
}

/// `And she lands on the "Northwind" workspace dashboard` — follow the 303 with
/// the auto-sign-in session cookie to the landing page. The page renders the
/// signed-in dashboard (200, "signed in"), AND the session's RESOLVED active
/// workspace is the provisioned tenant (DB-observable via the SHIPPED
/// `resolve_active_workspace` seam) — confirming she landed ON Northwind.
#[then(regex = r#"^she lands on the "([^"]+)" workspace dashboard$"#)]
async fn she_lands_on_workspace_dashboard(world: &mut FoundryWorld, ws_name: String) {
    let location = world
        .ia_post_location
        .clone()
        .expect("the accept POST set a Location to follow");
    let session_cookie = world
        .ia_session_cookie
        .clone()
        .expect("the accept POST issued a session cookie");
    let base = harness(world).base_url();
    let client = http(world);

    let resp = client
        .get(format!("{base}{location}"))
        .header(reqwest::header::COOKIE, session_cookie)
        .send()
        .await
        .expect("GET the landing page with the auto-sign-in session");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(
        status,
        StatusCode::OK,
        "the signed-in landing page must render 200; body = {body:?}"
    );
    assert!(
        body.to_ascii_lowercase().contains("signed in"),
        "the landing page must show she is signed in; got {body:?}"
    );

    // DB-observable: her session's RESOLVED active workspace is Northwind.
    let expected_ws = *world
        .ia_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} provisioned in the Background"));
    let admin_id = world.ia_admin_user_id.expect("first-admin id seeded");
    let resolved = harness(world)
        .app
        .state
        .store
        .resolve_active_workspace(admin_id)
        .await
        .expect("resolve the first-admin active workspace")
        .expect("the first-admin belongs to the provisioned workspace");
    assert_eq!(
        resolved.0, expected_ws,
        "the first-admin must land on the {ws_name:?} workspace ({expected_ws}); resolved {resolved:?}"
    );
    world.ia_landing_body = Some(body);
}

/// `And she sees no data from any other workspace` — the first-admin's ONLY
/// membership is the provisioned tenant: `resolve_active_workspace` returns exactly
/// Northwind and the membership table holds exactly one membership for her. There
/// is no path by which her signed-in session is scoped to a foreign tenant.
#[then(regex = r#"^she sees no data from any other workspace$"#)]
async fn she_sees_only_her_tenant(world: &mut FoundryWorld) {
    let admin_id = world.ia_admin_user_id.expect("first-admin id seeded");
    let pool = harness(world).app.state.store.pool().clone();
    let (membership_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM workspace_memberships WHERE user_id = $1")
            .bind(admin_id)
            .fetch_one(&pool)
            .await
            .expect("count the first-admin memberships");
    assert_eq!(
        membership_count, 1,
        "the first-admin must belong to EXACTLY her own tenant (no foreign membership); \
         found {membership_count} memberships"
    );
}

/// `And her invite is recorded as used exactly once` — the DB-observable single-use
/// outcome: the invite row's `used_at` is set (the consume guard fired) and exactly
/// ONE such consumed row exists for this id, with `used_by` = the first-admin (the
/// `created_by` the guarded-UPDATE returned). Reads the REAL per-scenario Postgres.
#[then(regex = r#"^her invite is recorded as used exactly once$"#)]
async fn invite_recorded_used_exactly_once(world: &mut FoundryWorld) {
    let invite_id = world.ia_invite_id.expect("invite seeded");
    let admin_id = world.ia_admin_user_id.expect("first-admin id seeded");
    let pool = harness(world).app.state.store.pool().clone();
    let (consumed_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NOT NULL AND used_by = $2",
    )
    .bind(invite_id)
    .bind(admin_id)
    .fetch_one(&pool)
    .await
    .expect("count the consumed invite row");
    assert_eq!(
        consumed_rows, 1,
        "the invite must be recorded as used EXACTLY ONCE (used_at set, used_by = the \
         first-admin); found {consumed_rows} consumed rows"
    );
}
