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

    // Snapshot the first-admin's throwaway password hash BEFORE any accept, so the
    // "Opening the accept page consumes nothing" scenario can prove the GET wrote no
    // password by comparing the post-GET hash against this baseline.
    let (seeded_hash,): (String,) = sqlx::query_as("SELECT password_hash FROM users WHERE id = $1")
        .bind(admin_user_id)
        .fetch_one(store.pool())
        .await
        .expect("read the seeded first-admin password hash");

    world.ia_workspace_ids.insert(ws_name, workspace_id);
    world.ia_invite_id = Some(invite_id);
    world.ia_admin_user_id = Some(admin_user_id);
    world.ia_seeded_password_hash = Some(seeded_hash);
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
// Scenario 2 (step 01-02) — a live invite renders a set-password form naming
// the workspace. Splits the chained walking-skeleton Given into an explicit
// arrival narrative (precondition → GET → form observable → workspace-name
// observable), so the GET render path is asserted in its own right. Green by
// inheritance from the 01-01 `show_accept_form` GET handler + template.
// ---------------------------------------------------------------------------

/// `Given Priya's invite has not expired and has not been used` — the Background
/// seeded a live invite (7-day expiry, `used_at`/`used_by` NULL). Confirm that
/// precondition holds against the REAL per-scenario Postgres before the GET, so
/// the "live" claim under test is grounded in observable invite state, not assumed.
#[given(regex = r#"^Priya's invite has not expired and has not been used$"#)]
async fn priya_invite_is_live(world: &mut FoundryWorld) {
    let invite_id = world.ia_invite_id.expect("invite seeded in the Background");
    let now = harness(world).app.state.clock.now();
    let pool = harness(world).app.state.store.pool().clone();
    let (live_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL AND expires_at > $2",
    )
    .bind(invite_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("count the live (unused, unexpired) invite row");
    assert_eq!(
        live_rows, 1,
        "the invite under test must be live (unused and unexpired) before the GET; \
         found {live_rows} live rows"
    );
}

/// `When Priya opens her invite link` — drive the NEW public GET
/// `/invites/accept?id=&sig=` over real HTTP with the genuine signed token. The
/// handler verifies the signature + advisory liveness and renders the set-password
/// form naming the workspace. Capture the status + rendered body so the form and
/// workspace-name observables are asserted by the following Thens.
#[when(regex = r#"^Priya opens her invite link$"#)]
async fn priya_opens_her_invite_link(world: &mut FoundryWorld) {
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
    world.ia_post_status = Some(resp.status());
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

/// `Then she sees a set-password form` — the GET for a live invite rendered a 200
/// page carrying a password form posting back to `/invites/accept` (the
/// port-exposed observable that the set-password form was served, not a refusal).
#[then(regex = r#"^she sees a set-password form$"#)]
async fn she_sees_a_set_password_form(world: &mut FoundryWorld) {
    assert_eq!(
        world.ia_post_status,
        Some(StatusCode::OK),
        "the GET accept page for a live invite must render a 200 set-password form; got {:?}",
        world.ia_post_status
    );
    let body = world
        .last_body
        .clone()
        .expect("the GET captured a rendered body");
    assert!(
        body.contains(r#"action="/invites/accept""#) && body.contains(r#"name="password""#),
        "the GET must render a set-password form posting to /invites/accept; got {body:?}"
    );
}

/// `And the form names the "Northwind" workspace` — the rendered form NAMES the
/// workspace, proving the GET resolved the invite's workspace (via the SHIPPED
/// `invite_accept_view` read) before rendering. The workspace-name substring is
/// the observable that distinguishes "named the right tenant" from a blank form.
#[then(regex = r#"^the form names the "([^"]+)" workspace$"#)]
async fn the_form_names_the_workspace(world: &mut FoundryWorld, ws_name: String) {
    let body = world
        .last_body
        .clone()
        .expect("the GET captured a rendered body");
    assert!(
        body.contains(&ws_name),
        "the set-password form must NAME the {ws_name:?} workspace; got {body:?}"
    );
}

// ---------------------------------------------------------------------------
// Scenario 4 (step 01-04) — an invite opened just INSIDE its expiry window is
// accepted. Pins the INCLUSIVE side of the expiry boundary (just-inside =
// accepted), complementing scenario 6 (just-past = refused). Green by
// inheritance from the SHIPPED `expires_at > $now` guard, which is enforced
// IDENTICALLY in the GET advisory liveness check (`invite_is_acceptable`) AND
// the authoritative consume TX (`set_first_admin_password_and_consume`): a
// still-future expiry (now + 1s) satisfies `expires_at > now`, so the GET
// renders the form and the POST consumes + writes + signs in.
//
// Setup re-points the seeded invite's `expires_at` to ~1s in the future against
// the REAL per-scenario Postgres and RE-MINTS the HMAC signature over the new
// `expires_at` (the token binds expires_at — the tamper oracle). This is test
// PRECONDITION setup, not production logic; no store method is added.
//
// Falsifiability litmus: tightening EITHER guard to reject a not-yet-expired
// invite (e.g. `expires_at > now + 1 hour`, or flipping `>` to a check that
// excludes the near-boundary) REDs this — the GET would refuse (no form) or the
// POST would refuse (no 303 / no session).
// ---------------------------------------------------------------------------

/// `Given Priya's invite is one second away from expiring and has not been used`
/// — re-point the seeded invite's `expires_at` to one second in the future (just
/// INSIDE the window) against the REAL per-scenario Postgres, and RE-MINT the
/// signed token over the new `expires_at` (the HMAC binds it). The invite stays
/// unused (`used_at` NULL). Asserts the re-pointed row is live (`expires_at >
/// now`, unused) so the "one second away from expiring" precondition is grounded
/// in observable invite state, not assumed.
#[given(regex = r#"^Priya's invite is one second away from expiring and has not been used$"#)]
async fn priya_invite_one_second_from_expiring(world: &mut FoundryWorld) {
    let invite_id = world.ia_invite_id.expect("invite seeded in the Background");
    let now = harness(world).app.state.clock.now();
    let expires_at = now + time::Duration::seconds(1);
    let pool = harness(world).app.state.store.pool().clone();

    sqlx::query("UPDATE invites SET expires_at = $2 WHERE id = $1 AND used_at IS NULL")
        .bind(invite_id)
        .bind(expires_at)
        .execute(&pool)
        .await
        .expect("re-point the seeded invite to one second from expiring");

    // Re-mint the signed token over the new expires_at (the HMAC binds it — a
    // stale signature over the 7-day expiry would fail the tamper oracle).
    let secret = harness(world).app.state.session_secret.clone();
    let token = foundry_auth::InviteToken::new(invite_id, expires_at, &secret)
        .expect("re-mint the near-expiry invite signature");
    world.ia_invite_sig = Some(token.signature);

    let (live_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL AND expires_at > $2",
    )
    .bind(invite_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("count the live (unused, just-inside-expiry) invite row");
    assert_eq!(
        live_rows, 1,
        "the invite under test must be live (unused, expiry just in the future) before \
         the accept; found {live_rows} live rows"
    );
}

/// `When Priya opens her invite link and sets a valid password` — drive the full
/// accept against the just-inside-expiry invite: the GET renders the form + mints
/// the CSRF cookie (proving the advisory `expires_at > now` admits the boundary),
/// then the POST carries the double-submit `_csrf` + token + a policy-passing
/// password through the SHIPPED CSRF middleware to the authoritative consume TX
/// (which re-enforces `expires_at > now`). Capture the 303, Location, and
/// auto-sign-in session cookie for the Then.
#[when(regex = r#"^Priya opens her invite link and sets a valid password$"#)]
async fn priya_opens_link_and_sets_valid_password(world: &mut FoundryWorld) {
    let invite_id = world.ia_invite_id.expect("invite seeded");
    let sig = world
        .ia_invite_sig
        .clone()
        .expect("invite signature minted");
    let base = harness(world).base_url();
    let client = http(world);

    // GET — render the form + mint the CSRF cookie (advisory liveness admits the
    // just-inside-expiry invite: a 200 form, not a refusal).
    let get_resp = client
        .get(format!(
            "{base}/invites/accept?id={invite_id}&sig={sig}",
            sig = urlencoding::encode(&sig)
        ))
        .send()
        .await
        .expect("GET /invites/accept");
    let get_status = get_resp.status();
    let csrf_cookie = get_resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(|s| s.to_string());
    let get_body = get_resp.text().await.unwrap_or_default();
    assert_eq!(
        get_status,
        StatusCode::OK,
        "the GET for a just-inside-expiry invite must render the form (advisory \
         expires_at > now admits it); body = {get_body:?}"
    );

    let csrf_cookie = csrf_cookie.expect("the GET minted a foundry_csrf cookie");
    let csrf_token = csrf_cookie
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();

    // POST — consume + write + sign in through the SHIPPED CSRF middleware and the
    // authoritative consume TX.
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

/// `Then she is signed in on the "Northwind" workspace` — the accept of the
/// just-inside-expiry invite succeeded end-to-end: the POST 303-redirected with
/// an auto-sign-in `foundry_session` cookie (no separate login), and her session's
/// RESOLVED active workspace is the provisioned tenant (DB-observable via the
/// SHIPPED `resolve_active_workspace` seam). Together: the consume guard admitted
/// the not-yet-expired invite (boundary inclusive side) and signed her in ON
/// Northwind. Tightening either guard to reject the near-boundary REDs this.
#[then(regex = r#"^she is signed in on the "([^"]+)" workspace$"#)]
async fn she_is_signed_in_on_workspace(world: &mut FoundryWorld, ws_name: String) {
    assert_eq!(
        world.ia_post_status,
        Some(StatusCode::SEE_OTHER),
        "the accept POST for a just-inside-expiry invite must 303 SEE_OTHER on success \
         (auto sign-in); got {:?}",
        world.ia_post_status
    );
    assert!(
        world.ia_session_cookie.is_some(),
        "the accept POST must establish a session (issue a foundry_session cookie), \
         proving the not-yet-expired invite was admitted and she was signed in; got none"
    );

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
        "she must be signed in ON the {ws_name:?} workspace ({expected_ws}); \
         resolved {resolved:?}"
    );
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

// ---------------------------------------------------------------------------
// Scenario 3 (step 01-03) — Opening the accept page consumes nothing.
//
// Reuses the walking-skeleton arrival Given (the GET that renders the form),
// then proves the GET was NON-COMMITTAL against the REAL per-scenario Postgres:
// the first-admin's password is still the seeded throwaway (no password was
// written) and the invite row is still live (used_at NULL, unexpired). Green by
// inheritance from the non-committal `show_accept_form` GET handler, which reads
// via `invite_accept_view` and renders without writing. The falsifiability
// litmus: a GET that wrote `used_at` (or the chosen password) reds BOTH Thens.
// ---------------------------------------------------------------------------

/// `Then no password has yet been set on her account` — after the non-committal
/// GET, the first-admin's `password_hash` is byte-identical to the throwaway
/// credential snapshotted at seed time (before any accept). A GET that wrote the
/// chosen password would change the hash and red this — the falsifiability bind.
#[then(regex = r#"^no password has yet been set on her account$"#)]
async fn no_password_set_yet(world: &mut FoundryWorld) {
    let admin_id = world.ia_admin_user_id.expect("first-admin id seeded");
    let seeded_hash = world
        .ia_seeded_password_hash
        .clone()
        .expect("the Background snapshotted the seeded throwaway password hash");
    let pool = harness(world).app.state.store.pool().clone();
    let (current_hash,): (String,) =
        sqlx::query_as("SELECT password_hash FROM users WHERE id = $1")
            .bind(admin_id)
            .fetch_one(&pool)
            .await
            .expect("read the first-admin password hash after the GET");
    assert_eq!(
        current_hash, seeded_hash,
        "opening the accept page must write NO password — the first-admin's \
         password_hash must equal the seeded throwaway credential; it changed, so \
         the GET consumed/wrote something"
    );
}

// ---------------------------------------------------------------------------
// Scenario 5 (step 02-01) — an EXPIRED invite is refused without leaking
// existence. The CANONICAL refusal arm: scenarios 6/7/8 assert byte-identity
// AGAINST the (status + full body) captured here. (D3/adr-002, OD-3 = 200 OK.)
//
// Setup re-points the seeded invite's `expires_at` to one day in the PAST
// against the REAL per-scenario Postgres and RE-MINTS the HMAC signature over
// the new `expires_at` (the token binds expires_at — the tamper oracle stays
// satisfied, so ONLY the liveness check fails; this is the genuine "expired"
// arm, not a tampered-signature arm). Test PRECONDITION setup, no store method.
//
// Falsifiability litmus (proven at DELIVER): making the expired path LEAK —
// rendering the workspace name, the invitee email, or any reason-distinct copy
// instead of the uniform refusal — REDs the no-leak Then; returning a 4xx/5xx
// status (instead of the ratified 200 OK) REDs the standard-page Then.
// ---------------------------------------------------------------------------

/// `Given Priya's invite expired one day ago` — re-point the seeded invite's
/// `expires_at` to one day in the past against the REAL per-scenario Postgres,
/// and RE-MINT the signed token over the new (past) `expires_at` so the HMAC
/// tamper oracle still verifies (only the liveness check fails — the canonical
/// expired arm). Asserts the row is now expired-but-unused so the "expired one
/// day ago" precondition is grounded in observable invite state, not assumed.
#[given(regex = r#"^Priya's invite expired one day ago$"#)]
async fn priya_invite_expired_one_day_ago(world: &mut FoundryWorld) {
    let invite_id = world.ia_invite_id.expect("invite seeded in the Background");
    let now = harness(world).app.state.clock.now();
    let expires_at = now - time::Duration::days(1);
    let pool = harness(world).app.state.store.pool().clone();

    sqlx::query("UPDATE invites SET expires_at = $2 WHERE id = $1 AND used_at IS NULL")
        .bind(invite_id)
        .bind(expires_at)
        .execute(&pool)
        .await
        .expect("re-point the seeded invite to one day past expiry");

    // Re-mint the signed token over the new (past) expires_at so the HMAC
    // verifies — isolating the failure to the liveness check (the canonical
    // expired arm), not the tamper oracle.
    let secret = harness(world).app.state.session_secret.clone();
    let token = foundry_auth::InviteToken::new(invite_id, expires_at, &secret)
        .expect("re-mint the expired invite signature");
    world.ia_invite_sig = Some(token.signature);

    let (expired_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL AND expires_at <= $2",
    )
    .bind(invite_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("count the expired (unused, past-expiry) invite row");
    assert_eq!(
        expired_rows, 1,
        "the invite under test must be expired (unused, expiry in the past) before \
         the GET; found {expired_rows} expired rows"
    );
}

/// `Then she sees the standard "invite is no longer valid" page` — the GET for
/// an expired invite rendered the uniform refusal at the ratified 200 OK
/// (OD-3 — no status-code oracle) carrying the journey's "no longer valid" copy.
/// CAPTURES the status + full body into the canonical refusal slots so scenarios
/// 6/7/8 can assert byte-identity against this arm.
#[then(regex = r#"^she sees the standard "invite is no longer valid" page$"#)]
async fn she_sees_standard_refusal_page(world: &mut FoundryWorld) {
    assert_eq!(
        world.ia_post_status,
        Some(StatusCode::OK),
        "the expired-invite refusal must be the ratified 200 OK (OD-3, no status \
         oracle); got {:?}",
        world.ia_post_status
    );
    let body = world
        .last_body
        .clone()
        .expect("the GET captured a rendered body");
    assert!(
        body.to_ascii_lowercase().contains("no longer valid"),
        "the refusal must render the standard \"invite is no longer valid\" page; \
         got {body:?}"
    );
    // Capture the canonical refusal (status + full body) for the byte-identity
    // comparison helper scenarios 6/7/8 reuse.
    world.ia_refusal_status = world.ia_post_status;
    world.ia_refusal_body = Some(body);
}

/// `And the page reveals nothing about whether any account or workspace exists`
/// — the uniform refusal leaks NONE of: the workspace name ("Northwind"), the
/// invitee email, or any account/invite-state identifier. This is the
/// non-enumerability guarantee (NFR-3): a prober learns nothing. Making the
/// expired path render the workspace name or the invitee email REDs this.
#[then(regex = r#"^the page reveals nothing about whether any account or workspace exists$"#)]
async fn refusal_leaks_no_existence(world: &mut FoundryWorld) {
    let body = world
        .ia_refusal_body
        .clone()
        .or_else(|| world.last_body.clone())
        .expect("the refusal captured a rendered body");
    assert!(
        !body.contains("Northwind"),
        "the refusal must NOT reveal the workspace name; got {body:?}"
    );
    assert!(
        !body.contains(PRIYA_EMAIL),
        "the refusal must NOT reveal the invitee email; got {body:?}"
    );
}

/// `And the page advises asking the instance administrator to re-issue the
/// invite` — the journey's universal next action (the only "reason" a legitimate
/// recipient gets, by design): ask the instance administrator to re-issue /
/// re-provision. Asserts the advisory copy is present (admin + re-issue intent).
#[then(regex = r#"^the page advises asking the instance administrator to re-issue the invite$"#)]
async fn refusal_advises_admin_reissue(world: &mut FoundryWorld) {
    let body = world
        .ia_refusal_body
        .clone()
        .or_else(|| world.last_body.clone())
        .expect("the refusal captured a rendered body");
    let lower = body.to_ascii_lowercase();
    assert!(
        lower.contains("administrator")
            && (lower.contains("re-issue") || lower.contains("reissue")),
        "the refusal must advise asking the instance administrator to re-issue the \
         invite; got {body:?}"
    );
}

/// `And her invite is still live and unconsumed` — after the GET, the invite row
/// is still live: `used_at` is NULL and it has not expired (exactly the same
/// liveness the pre-GET precondition asserted). A GET that consumed the invite
/// (set `used_at`) would drop the live count to 0 and red this.
#[then(regex = r#"^her invite is still live and unconsumed$"#)]
async fn invite_still_live_and_unconsumed(world: &mut FoundryWorld) {
    let invite_id = world.ia_invite_id.expect("invite seeded");
    let now = harness(world).app.state.clock.now();
    let pool = harness(world).app.state.store.pool().clone();
    let (live_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL AND expires_at > $2",
    )
    .bind(invite_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("count the live (unused, unexpired) invite row after the GET");
    assert_eq!(
        live_rows, 1,
        "opening the accept page must consume NOTHING — the invite must still be \
         live (used_at NULL, unexpired) after the GET; found {live_rows} live rows"
    );
}

// ---------------------------------------------------------------------------
// Scenario 6 (step 02-02) — an invite opened just PAST its expiry window is
// refused, byte-identically to the canonical expired arm. Pins the EXCLUSIVE
// side of the expiry boundary (just-past = refused), complementing scenario 4
// (just-inside = accepted). Green by inheritance from the SHIPPED
// `invite_is_acceptable` guard (`used_at.is_none() && expires_at > now`): an
// invite re-pointed to `now - 1s` fails `expires_at > now`, so the GET renders
// the uniform `invite_refusal_page()` — the SAME page the expired-one-day arm
// (02-01) renders, because the refusal is non-committal on the reason.
//
// Byte-identity proof (D3/adr-002, the security crux): the just-past refusal
// (captured by the reused "standard page" Then into `ia_refusal_*`) is asserted
// (status + FULL body) against an in-scenario RECOMPUTE of the canonical
// expired-one-day arm — re-pointing the SAME invite to `now - 1 day`, re-minting
// the HMAC, and GETting again. Recomputing the canonical control in-scenario
// (rather than reading a cross-scenario slot — each cucumber scenario gets a
// fresh harness) mirrors the proven web-provisioning non-enumerability pattern.
//
// Falsifiability litmus (proven at DELIVER): loosening `invite_is_acceptable` to
// admit a recently-expired invite (e.g. `expires_at > now - 2h`) makes the GET
// render the 200 set-password FORM for the just-past invite instead of the
// refusal — RED-ing BOTH the reused "no longer valid" Then and the byte-identity
// assertion (form body != refusal body).
// ---------------------------------------------------------------------------

/// `Given Priya's invite expired one second ago` — re-point the seeded invite's
/// `expires_at` to one second in the PAST (just OUTSIDE the window) against the
/// REAL per-scenario Postgres, and RE-MINT the signed token over the new (past)
/// `expires_at` so the HMAC tamper oracle still verifies (only the liveness
/// check fails — the genuine "just-past expired" arm, not a tampered-signature
/// arm). Asserts the row is now expired-but-unused so the "expired one second
/// ago" precondition is grounded in observable invite state, not assumed.
#[given(regex = r#"^Priya's invite expired one second ago$"#)]
async fn priya_invite_expired_one_second_ago(world: &mut FoundryWorld) {
    let invite_id = world.ia_invite_id.expect("invite seeded in the Background");
    let now = harness(world).app.state.clock.now();
    let expires_at = now - time::Duration::seconds(1);
    let pool = harness(world).app.state.store.pool().clone();

    sqlx::query("UPDATE invites SET expires_at = $2 WHERE id = $1 AND used_at IS NULL")
        .bind(invite_id)
        .bind(expires_at)
        .execute(&pool)
        .await
        .expect("re-point the seeded invite to one second past expiry");

    // Re-mint the signed token over the new (past) expires_at so the HMAC
    // verifies — isolating the failure to the liveness check (the just-past
    // expired arm), not the tamper oracle.
    let secret = harness(world).app.state.session_secret.clone();
    let token = foundry_auth::InviteToken::new(invite_id, expires_at, &secret)
        .expect("re-mint the just-past-expiry invite signature");
    world.ia_invite_sig = Some(token.signature);

    let (expired_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL AND expires_at <= $2",
    )
    .bind(invite_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("count the expired (unused, just-past-expiry) invite row");
    assert_eq!(
        expired_rows, 1,
        "the invite under test must be expired (unused, expiry one second in the \
         past) before the GET; found {expired_rows} expired rows"
    );
}

/// `And the response is byte-identical to the expired-invite refusal` — the
/// EXCLUSIVE-side boundary + non-enumerability core. The just-past refusal
/// (captured by the reused "standard page" Then into `ia_refusal_*`) is held,
/// then the CANONICAL expired-one-day arm is RECOMPUTED in this same scenario
/// (re-point the SAME invite to `now - 1 day`, re-mint the HMAC, GET again) and
/// the two responses are asserted byte-identical (status AND full body).
/// Asserting the FULL body (not merely same-status) is what makes the litmus
/// bite: a guard that admitted the just-past invite would render the
/// set-password FORM (a divergent body) and re-RED here.
#[then(regex = r#"^the response is byte-identical to the expired-invite refusal$"#)]
async fn response_byte_identical_to_expired_refusal(world: &mut FoundryWorld) {
    // The just-past refusal captured by the reused "standard page" Then.
    let just_past_status = world
        .ia_refusal_status
        .expect("the just-past refusal status was captured by the standard-page Then");
    let just_past_body = world
        .ia_refusal_body
        .clone()
        .expect("the just-past refusal body was captured by the standard-page Then");
    world.ia_just_past_refusal_status = Some(just_past_status);
    world.ia_just_past_refusal_body = Some(just_past_body.clone());

    // Recompute the CANONICAL expired-one-day arm in-scenario: re-point the SAME
    // invite to one day past, re-mint the HMAC over the new expires_at, and GET.
    let invite_id = world.ia_invite_id.expect("invite seeded");
    let now = harness(world).app.state.clock.now();
    let canonical_expires_at = now - time::Duration::days(1);
    let pool = harness(world).app.state.store.pool().clone();
    sqlx::query("UPDATE invites SET expires_at = $2 WHERE id = $1 AND used_at IS NULL")
        .bind(invite_id)
        .bind(canonical_expires_at)
        .execute(&pool)
        .await
        .expect("re-point the invite to one day past expiry for the canonical control");
    let secret = harness(world).app.state.session_secret.clone();
    let canonical_token = foundry_auth::InviteToken::new(invite_id, canonical_expires_at, &secret)
        .expect("re-mint the canonical expired-one-day signature");
    let canonical_sig = canonical_token.signature;
    let base = harness(world).base_url();
    let client = http(world);
    let resp = client
        .get(format!(
            "{base}/invites/accept?id={invite_id}&sig={sig}",
            sig = urlencoding::encode(&canonical_sig)
        ))
        .send()
        .await
        .expect("GET /invites/accept for the canonical expired-one-day control");
    let canonical_status = resp.status();
    let canonical_body = resp.text().await.unwrap_or_default();

    // Byte-identity: status AND full body across the two expiry-boundary arms.
    assert_eq!(
        just_past_status, canonical_status,
        "the just-past-expiry refusal status ({just_past_status}) must be \
         byte-identical to the canonical expired-one-day refusal status \
         ({canonical_status}) — a status oracle on the expiry boundary"
    );
    assert_eq!(
        just_past_body, canonical_body,
        "the just-past-expiry refusal body must be byte-identical to the canonical \
         expired-one-day refusal body — a body oracle would reveal HOW LONG ago the \
         invite expired. just-past = {just_past_body:?}, canonical = {canonical_body:?}"
    );
}

// ---------------------------------------------------------------------------
// Scenario 7 (step 02-03) — a TAMPERED signature is refused IDENTICALLY to an
// expired link. The HMAC tamper oracle (`InviteToken::verify`, called FIRST in
// the SHIPPED `invite_is_acceptable`) rejects an altered `sig` BEFORE any
// liveness check or DB-state mutation; the GET then renders the SAME uniform
// `invite_refusal_page()` an expired link renders, because the refusal is
// non-committal on the reason. Green by inheritance from the SHIPPED verify →
// refusal path. (D3/adr-002, the security crux; E3.)
//
// The invite under test stays LIVE (7-day expiry, unused) — ONLY the signature
// is altered, so the failure is isolated to the tamper oracle, not liveness
// (this is the genuine "tampered-signature" arm, NOT an expired arm). The
// scenario then REUSES scenario 2's `When Priya opens her invite link` (GET with
// the now-tampered `ia_invite_sig`), scenario 5's `Then she sees the standard
// "invite is no longer valid" page` (captures the refusal into `ia_refusal_*`),
// and scenario 6's `And the response is byte-identical to the expired-invite
// refusal` (recomputes the canonical expired-one-day arm in-scenario and asserts
// status + FULL body byte-identity). The ONLY new step is the tamper Given.
//
// Falsifiability litmus (proven at DELIVER): a verify path that distinguished a
// bad signature — returning a DIFFERENT message ("invalid signature") or a
// DIFFERENT status (a 4xx tamper oracle) instead of the uniform 200 refusal —
// REDs BOTH the reused "no longer valid" Then (divergent copy/status) AND the
// byte-identity assertion (the tampered-arm response would differ from the
// canonical expired-arm response). Asserting the FULL body (not merely
// same-status) is what makes the litmus bite.
// ---------------------------------------------------------------------------

/// `Given Priya's invite is live but the signature in the link has been altered
/// by one character` — keep the Background's live invite (7-day expiry, `used_at`
/// NULL) UNTOUCHED, and corrupt the genuine minted `ia_invite_sig` by flipping a
/// single character. Confirms the invite is still live against the REAL
/// per-scenario Postgres (so the failure under test is isolated to the tamper
/// oracle, NOT liveness) and that the tampered sig genuinely DIFFERS from the
/// authentic one (so the corruption actually took). The corrupted sig is stored
/// back into `ia_invite_sig`, which the reused GET step then carries.
#[given(
    regex = r#"^Priya's invite is live but the signature in the link has been altered by one character$"#
)]
async fn priya_invite_signature_tampered(world: &mut FoundryWorld) {
    let invite_id = world.ia_invite_id.expect("invite seeded in the Background");
    let now = harness(world).app.state.clock.now();
    let pool = harness(world).app.state.store.pool().clone();

    // The invite stays LIVE (unused + unexpired) — only the signature is altered,
    // so the refusal under test fires on the tamper oracle, not liveness.
    let (live_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL AND expires_at > $2",
    )
    .bind(invite_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("count the live (unused, unexpired) invite row");
    assert_eq!(
        live_rows, 1,
        "the invite under test must be live (unused and unexpired) so the refusal \
         fires on the tampered signature, not liveness; found {live_rows} live rows"
    );

    // Corrupt the genuine signature by flipping a single character (the
    // base64url alphabet is large; pick a replacement that differs from the
    // original char so the corruption is guaranteed to take).
    let authentic = world
        .ia_invite_sig
        .clone()
        .expect("the Background minted the genuine invite signature");
    let mut chars: Vec<char> = authentic.chars().collect();
    assert!(
        !chars.is_empty(),
        "the genuine invite signature must be non-empty to tamper with"
    );
    let original = chars[0];
    // base64url uses A-Z a-z 0-9 - _ ; flip to a different alphabet character.
    chars[0] = if original == 'A' { 'B' } else { 'A' };
    let tampered: String = chars.into_iter().collect();
    assert_ne!(
        tampered, authentic,
        "the tampered signature must differ from the genuine one (the corruption \
         must actually take); both = {authentic:?}"
    );
    world.ia_invite_sig = Some(tampered);
}

/// `When Priya opens the tampered link` — drive the NEW public GET
/// `/invites/accept?id=&sig=` over real HTTP carrying the now-tampered
/// `ia_invite_sig`. Identical GET path to scenario 2's `Priya opens her invite
/// link` (distinct phrasing for the tampered narrative): the SHIPPED
/// `invite_is_acceptable` calls `InviteToken::verify` FIRST, the altered HMAC
/// fails the tamper oracle, and the handler renders the uniform
/// `invite_refusal_page()` — captured (status + full body) for the byte-identity
/// Then. Capturing the same status + body slots the reused refusal/byte-identity
/// Thens read.
#[when(regex = r#"^Priya opens the tampered link$"#)]
async fn priya_opens_the_tampered_link(world: &mut FoundryWorld) {
    priya_opens_her_invite_link(world).await;
}
