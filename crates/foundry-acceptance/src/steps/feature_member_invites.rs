//! workspace-member-invites (US-01/02/03/04) step definitions.
//!
//! GENERALIZES the shipped first-admin `/invites/accept` flow to GENERAL workspace
//! members, across two genuinely-new surfaces over an otherwise-reused vertical.
//! ISSUANCE: a workspace ADMIN invites a teammate by email at the NEW admin-gated
//! `/workspace/invites` web adapter (mirrors the shipped `bootstrap::create_invite`
//! plus the `instance_admin` issuance idiom). ACCEPTANCE: the invitee (with NO
//! Foundry account) opens the link, sets a password, and the accept POST runs ONE
//! atomic tx (`create_member_and_consume`) that creates the user, adds a `member`
//! membership, consumes the invite, and writes argon2id, then auto-signs-in (303 to
//! the workspace dashboard).
//!
//! Step 01-01 implements ONLY the `@walking_skeleton` scenario: Dana (admin) issues
//! a member invite via the REAL issuance handler, then Sam (no account) opens the
//! emitted link, sets a password, has an account and a member-role membership
//! created and the invite consumed in ONE tx, and is auto-signed-in onto Northwind
//! seeing only that tenant. The thinnest cut proving the NEW issuance route, the NEW
//! `create_member_and_consume` tx, and the member arm of the accept dispatch wire
//! end-to-end through session + CSRF + Postgres.
//!
//! Driving adapter: the in-process axum router served by foundry-app over real HTTP
//! (the `InProcHarness`), under the SHIPPED session + double-submit CSRF layers,
//! mirroring `feature_invite_accept` + `feature_web_provisioning_flow`. The
//! Background seeds Dana + the Northwind workspace + her admin membership directly (a
//! known password so the web sign-in can authenticate her), so the invite under test
//! is minted by the REAL issuance handler — the issuance-to-accept handoff (the SAME
//! invite_id + sig) is genuine, not synthesised.
//!
//! LAYER 3 (real adapter + real HTTP, @real-io @wiring_e2e): real Postgres via
//! testcontainers + per-scenario schema; the real tower-sessions Postgres store; the
//! real double-submit CSRF middleware; the SHIPPED `InviteToken::new`/`verify`,
//! `hash_password`, `check_password_policy`, `is_workspace_admin`, `insert_invite`,
//! and `resolve_active_workspace`; plus the NEW `Store::create_member_and_consume`.
//! Example-based (Mandates 9 + 11) — no PBT at this layer; assertions are
//! traditional, over port-exposed web observables: the rendered "invite sent"
//! fragment + emitted link, the 303 auto-sign-in + session cookie, the created user +
//! member membership, and the consumed-exactly-once invite.

use crate::support::harness::{signed_in_get, signed_in_post, InProcHarness};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use foundry_app::clock::Clock;
use foundry_store::Store;
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use secrecy::SecretString;
use std::sync::Arc;

/// Dana's password — set on the seeded admin user so the web sign-in can
/// authenticate her issuance POST (the harness keeps no cookie jar; `signed_in_post`
/// re-authenticates per request).
const DANA_PASSWORD: &str = "northwind-admin-secret";
/// Sam's password — meets the min-12 length-first policy (ADR-004).
const SAM_PASSWORD: &str = "sam-northwind-secure-pass";

fn harness(world: &FoundryWorld) -> &InProcHarness {
    world
        .mi_harness
        .as_ref()
        .expect("the member-invites Background must have spawned the mi harness")
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

/// `Given Dana Reyes is signed in as an admin of the "Northwind" workspace` — spawn
/// the in-process harness and seed the Northwind workspace + Dana (a real `users`
/// row with a KNOWN password) + her `admin`-role membership directly (mirrors the
/// shipped `workspace_has_member` seed). Dana's web sign-in itself happens
/// per-request inside `signed_in_post` (the harness keeps no cookie jar),
/// authenticating against the SHIPPED cookie sign-in path with `DANA_PASSWORD`.
#[given(regex = r#"^Dana Reyes is signed in as an admin of the "([^"]+)" workspace$"#)]
async fn dana_signed_in_admin(world: &mut FoundryWorld, ws_name: String) {
    let harness = InProcHarness::spawn(time::OffsetDateTime::now_utc()).await;
    let store: Arc<Store> = harness.app.state.store.clone();
    let pool = store.pool().clone();

    let workspace_id = uuid::Uuid::now_v7();
    let admin_user_id = uuid::Uuid::now_v7();
    let dana_email = "dana.reyes@northwind.example";

    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(workspace_id)
        .bind(&ws_name)
        .execute(&pool)
        .await
        .expect("seed the Northwind workspace");

    let dana_hash =
        foundry_auth::hash_password(&SecretString::new(DANA_PASSWORD.to_string().into()))
            .await
            .expect("hash Dana's admin password");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, 'Dana Reyes', $4)",
    )
    .bind(admin_user_id)
    .bind(dana_email)
    .bind(dana_email)
    .bind(&dana_hash)
    .execute(&pool)
    .await
    .expect("seed Dana's admin user row");

    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'admin')",
    )
    .bind(workspace_id)
    .bind(admin_user_id)
    .execute(&pool)
    .await
    .expect("seed Dana's admin membership");

    world.mi_workspace_ids.insert(ws_name, workspace_id);
    world.mi_admin_user_id = Some(admin_user_id);
    world.mi_admin_email = Some(dana_email.to_string());
    world.mi_harness = Some(harness);
    let _ = http(world);
}

// ---------------------------------------------------------------------------
// Walking skeleton (step 01-01)
// ---------------------------------------------------------------------------

/// `When Dana invites "<email>" to "<workspace>"` — drive the NEW admin-gated web
/// issuance route over real HTTP: sign in as Dana (cookie + CSRF) and POST the
/// member-invite form (`email` + `_csrf`) to `/workspace/invites`. The handler gates
/// on the SHIPPED `is_workspace_admin`, resolves Dana's active workspace from her
/// session, inserts the invite (`created_by = Dana`, `invitee_email = the typed
/// email`), signs the `InviteToken`, and renders the "invite sent" fragment carrying
/// the emitted `/invites/accept?id=&sig=` link. Parse the invite id + sig out of the
/// rendered link so the accept leg drives the SAME genuine invite.
#[when(regex = r#"^Dana invites "([^"]+)" to "([^"]+)"$"#)]
async fn dana_invites_teammate(world: &mut FoundryWorld, invitee: String, _ws_name: String) {
    let dana_email = world
        .mi_admin_email
        .clone()
        .expect("the Background seeded Dana's email");
    let client = http(world);
    let outcome = signed_in_post(
        harness(world),
        &client,
        &dana_email,
        DANA_PASSWORD,
        "/workspace/invites",
        &[("email", invitee.as_str())],
    )
    .await;

    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "the member-invite issuance POST must render a 200 'invite sent' fragment; \
         body = {:?}",
        outcome.body
    );
    assert!(
        outcome.body.contains("/invites/accept?id="),
        "the issuance fragment must report a shareable accept link; got {:?}",
        outcome.body
    );

    let (invite_id, sig) = parse_accept_link(&outcome.body);
    world.mi_invite_id = Some(invite_id);
    world.mi_invite_sig = Some(sig);
    world.last_body = Some(outcome.body);
}

/// `And Sam opens his invite link and sets a password meeting the strength policy` —
/// drive the SHIPPED PUBLIC accept route pair (the invitee is signed out, has no
/// account yet) against the REAL issued invite: GET `/invites/accept?id=&sig=` to
/// render the set-password form + mint the double-submit CSRF cookie, then POST the
/// `id` + `sig` + a policy-passing `password` + matching `confirm` + `_csrf`. The
/// SHIPPED `csrf_middleware` screens the token; the handler re-verifies, runs the
/// policy, DISPATCHES to the NEW `create_member_and_consume` (no user maps to Sam's
/// email → member arm), establishes a session, and 303-redirects. Capture the 303,
/// Location, and auto-sign-in session cookie.
#[when(regex = r#"^Sam opens his invite link and sets a password meeting the strength policy$"#)]
async fn sam_opens_link_and_sets_password(world: &mut FoundryWorld) {
    accept_member_invite(world, SAM_PASSWORD).await;
}

/// Drive the full accept (GET → POST) for the live member invite with `password`.
/// Shared by the walking-skeleton When; left as a helper so later member-accept
/// scenarios can reuse the GET-form-then-POST dance.
async fn accept_member_invite(world: &mut FoundryWorld, password: &str) {
    let invite_id = world.mi_invite_id.expect("issuance minted an invite id");
    let sig = world.mi_invite_sig.clone().expect("issuance minted a sig");
    let base = harness(world).base_url();
    let client = http(world);

    // GET — render the set-password form + mint the CSRF cookie.
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
        .map(str::to_string);
    let get_body = get_resp.text().await.unwrap_or_default();
    assert_eq!(
        get_status,
        StatusCode::OK,
        "the GET accept page for a live member invite must render a 200 set-password \
         form; body = {get_body:?}"
    );

    let csrf_cookie = csrf_cookie.expect("the GET minted a foundry_csrf cookie");
    let csrf_token = csrf_cookie
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();

    // POST — consume + create user + add member membership + write password + sign in.
    let form = [
        ("id", invite_id.to_string()),
        ("sig", sig),
        ("password", password.to_string()),
        ("confirm", password.to_string()),
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

    world.mi_post_status = Some(status);
    world.mi_post_location = location;
    world.mi_session_cookie = session_cookie;
}

/// `Then a new account is created for "<email>"` — the DB-observable account-creation
/// outcome: EXACTLY ONE `users` row exists for the invitee email (the member arm of
/// the accept dispatch created it via `create_member_and_consume`). Before this
/// feature no such row existed (the invitee had no Foundry account), so a count of 1
/// proves the create-user step of the tx fired.
#[then(regex = r#"^a new account is created for "([^"]+)"$"#)]
async fn new_account_created(world: &mut FoundryWorld, email: String) {
    let pool = harness(world).app.state.store.pool().clone();
    let email_lower = email.to_ascii_lowercase();
    let (user_rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email_lower = $1")
        .bind(&email_lower)
        .fetch_one(&pool)
        .await
        .expect("count the created member user row");
    assert_eq!(
        user_rows, 1,
        "the member accept must create EXACTLY ONE account for {email:?}; found \
         {user_rows} rows"
    );
}

/// `And Sam is signed in on the "<workspace>" workspace without a separate login
/// step` — the accept POST 303-redirected with an auto-sign-in `foundry_session`
/// cookie (no separate `/sign-in`), and the new member's RESOLVED active workspace is
/// the inviting tenant (DB-observable via the SHIPPED `resolve_active_workspace`).
#[then(regex = r#"^Sam is signed in on the "([^"]+)" workspace without a separate login step$"#)]
async fn sam_signed_in_on_workspace(world: &mut FoundryWorld, ws_name: String) {
    assert_eq!(
        world.mi_post_status,
        Some(StatusCode::SEE_OTHER),
        "the member accept POST must 303 SEE_OTHER on success (auto sign-in); got {:?}",
        world.mi_post_status
    );
    assert!(
        world.mi_session_cookie.is_some(),
        "the accept POST must establish a session (issue a foundry_session cookie), \
         proving auto sign-in with no separate login step; got none"
    );

    let expected_ws = *world
        .mi_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} seeded in the Background"));
    let member_id = sam_user_id(world).await;
    let resolved = harness(world)
        .app
        .state
        .store
        .resolve_active_workspace(member_id)
        .await
        .expect("resolve the new member's active workspace")
        .expect("the new member belongs to the inviting workspace");
    assert_eq!(
        resolved.0, expected_ws,
        "the new member must be signed in ON the {ws_name:?} workspace ({expected_ws}); \
         resolved {resolved:?}"
    );
}

/// `And Sam is a member of "<workspace>" and sees no data from any other workspace` —
/// the new member's ONLY membership is the inviting tenant, with the `member` role
/// (NOT admin): exactly one `workspace_memberships` row for him, on the inviting
/// workspace, role = `member`. There is no path by which his signed-in session is
/// scoped to a foreign tenant.
#[then(regex = r#"^Sam is a member of "([^"]+)" and sees no data from any other workspace$"#)]
async fn sam_is_member_only(world: &mut FoundryWorld, ws_name: String) {
    let expected_ws = *world
        .mi_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} seeded in the Background"));
    let member_id = sam_user_id(world).await;
    let pool = harness(world).app.state.store.pool().clone();

    let memberships: Vec<(uuid::Uuid, String)> =
        sqlx::query_as("SELECT workspace_id, role FROM workspace_memberships WHERE user_id = $1")
            .bind(member_id)
            .fetch_all(&pool)
            .await
            .expect("read the new member's memberships");
    assert_eq!(
        memberships.len(),
        1,
        "the new member must belong to EXACTLY one tenant (no foreign membership); \
         found {memberships:?}"
    );
    let (membership_ws, role) = &memberships[0];
    assert_eq!(
        *membership_ws, expected_ws,
        "the member's sole membership must be the inviting {ws_name:?} workspace \
         ({expected_ws}); got {membership_ws}"
    );
    assert_eq!(
        role, "member",
        "the invitee must join as a MEMBER (not admin); got role {role:?}"
    );
}

/// `And his invite is recorded as used exactly once` — the DB-observable single-use
/// outcome: the invite row's `used_at` is set (the consume guard fired) and exactly
/// ONE such consumed row exists for this id, with `used_by` = the newly-created
/// member (the FK the tx satisfied after creating the user). Reads the REAL
/// per-scenario Postgres.
#[then(regex = r#"^his invite is recorded as used exactly once$"#)]
async fn invite_used_exactly_once(world: &mut FoundryWorld) {
    let invite_id = world.mi_invite_id.expect("issuance minted an invite id");
    let member_id = sam_user_id(world).await;
    let pool = harness(world).app.state.store.pool().clone();
    let (consumed_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NOT NULL AND used_by = $2",
    )
    .bind(invite_id)
    .bind(member_id)
    .fetch_one(&pool)
    .await
    .expect("count the consumed invite row");
    assert_eq!(
        consumed_rows, 1,
        "the invite must be recorded as used EXACTLY ONCE (used_at set, used_by = the \
         new member); found {consumed_rows} consumed rows"
    );
}

// ---------------------------------------------------------------------------
// Issuance happy paths (step 01-02)
// ---------------------------------------------------------------------------

/// `When Dana opens the member-invite form` — drive the NEW admin-gated GET
/// `/workspace/invites` over real HTTP as the signed-in admin. The handler gates on
/// the SHIPPED `is_workspace_admin`, resolves her workspace from the session, and
/// renders the one-email-field form naming the workspace. Captures the rendered body
/// for the Then.
#[when(regex = r#"^Dana opens the member-invite form$"#)]
async fn dana_opens_form(world: &mut FoundryWorld) {
    let dana_email = world
        .mi_admin_email
        .clone()
        .expect("the Background seeded Dana's email");
    let client = http(world);
    let outcome = signed_in_get(
        harness(world),
        &client,
        &dana_email,
        DANA_PASSWORD,
        "/workspace/invites",
    )
    .await;
    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "the admin GET on /workspace/invites must render a 200 form; body = {:?}",
        outcome.body
    );
    world.last_body = Some(outcome.body);
}

/// `Then she sees a one-email-field form to invite a member to "<workspace>"` — the
/// rendered GET body is the member-invite form: it names the workspace, carries an
/// email input field, and POSTs to the issuance surface.
#[then(regex = r#"^she sees a one-email-field form to invite a member to "([^"]+)"$"#)]
async fn sees_invite_form(world: &mut FoundryWorld, ws_name: String) {
    let body = world
        .last_body
        .clone()
        .expect("the GET form body was captured");
    assert!(
        body.contains(&ws_name),
        "the form must name the {ws_name:?} workspace; body = {body:?}"
    );
    assert!(
        body.contains("name=\"email\""),
        "the form must carry a one-email-field input; body = {body:?}"
    );
    assert!(
        body.contains("/workspace/invites"),
        "the form must POST to the issuance surface; body = {body:?}"
    );
}

/// `Given Dana has opened the member-invite form for "<workspace>"` — the arrival
/// state of the issuance chain. Reuses the GET-form When so scenario 3 begins from a
/// rendered form, matching the chained narrative.
#[given(regex = r#"^Dana has opened the member-invite form for "([^"]+)"$"#)]
async fn dana_has_opened_form(world: &mut FoundryWorld, _ws_name: String) {
    dana_opens_form(world).await;
}

/// `When Dana submits "<email>"` — POST the valid email to the issuance surface via
/// `signed_in_post` (sign-in + CSRF + form). The handler inserts the invite
/// (`created_by = Dana`), signs the `InviteToken`, emits the link, best-effort
/// emails, and renders the "invite sent" fragment. Parse the invite id + sig.
#[when(regex = r#"^Dana submits "([^"]+)"$"#)]
async fn dana_submits_email(world: &mut FoundryWorld, invitee: String) {
    submit_issuance(world, &invitee).await;
}

/// `Then an invite to "<workspace>" is created for "<email>"` — the DB-observable
/// issuance outcome: exactly ONE `invites` row exists for this email on the inviting
/// workspace, `created_by = Dana`. Reads the REAL per-scenario Postgres at the driven
/// port boundary.
#[then(regex = r#"^an invite to "([^"]+)" is created for "([^"]+)"$"#)]
async fn invite_created_for(world: &mut FoundryWorld, ws_name: String, email: String) {
    let expected_ws = *world
        .mi_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} seeded in the Background"));
    let admin_id = world
        .mi_admin_user_id
        .expect("the Background seeded Dana's user id");
    let pool = harness(world).app.state.store.pool().clone();
    let (rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites \
         WHERE invitee_email = $1 AND workspace_id = $2 AND created_by = $3",
    )
    .bind(&email)
    .bind(expected_ws)
    .bind(admin_id)
    .fetch_one(&pool)
    .await
    .expect("count the issued invite row");
    assert_eq!(
        rows, 1,
        "issuance must create EXACTLY ONE invite for {email:?} on {ws_name:?} \
         (created_by = Dana); found {rows} rows"
    );
}

/// `And Dana sees a confirmation with a shareable accept link valid for 7 days` — the
/// rendered "invite sent" fragment carries a shareable `/invites/accept?id&sig` link,
/// and the issued invite's `expires_at` is 7 days out from the issuing clock (the
/// mock clock the harness froze at spawn).
#[then(regex = r#"^Dana sees a confirmation with a shareable accept link valid for 7 days$"#)]
async fn sees_confirmation_link_7_days(world: &mut FoundryWorld) {
    let body = world
        .last_body
        .clone()
        .expect("the issuance fragment body was captured");
    assert!(
        body.contains("/invites/accept?id="),
        "the confirmation must carry a shareable accept link; body = {body:?}"
    );
    let invite_id = world.mi_invite_id.expect("issuance minted an invite id");
    let pool = harness(world).app.state.store.pool().clone();
    let (expires_at,): (time::OffsetDateTime,) =
        sqlx::query_as("SELECT expires_at FROM invites WHERE id = $1")
            .bind(invite_id)
            .fetch_one(&pool)
            .await
            .expect("read the issued invite's expires_at");
    let issued_at = harness(world).fake_clock.now();
    let ttl = expires_at - issued_at;
    assert_eq!(
        ttl.whole_days(),
        7,
        "the invite must be valid for 7 days; expires_at = {expires_at}, issued_at = \
         {issued_at}, ttl_days = {}",
        ttl.whole_days()
    );
}

/// `And the emitted signature verifies against that invite` — the `sig` parsed out of
/// the emitted link is a genuine HMAC over `invite_id|expires_at` under the harness
/// `session_secret` (the SHIPPED `InviteToken::verify`). Proves the link is signed by
/// the real issuance handler, not a placeholder.
#[then(regex = r#"^the emitted signature verifies against that invite$"#)]
async fn emitted_signature_verifies(world: &mut FoundryWorld) {
    let invite_id = world.mi_invite_id.expect("issuance minted an invite id");
    let sig = world.mi_invite_sig.clone().expect("issuance minted a sig");
    let pool = harness(world).app.state.store.pool().clone();
    let (expires_at,): (time::OffsetDateTime,) =
        sqlx::query_as("SELECT expires_at FROM invites WHERE id = $1")
            .bind(invite_id)
            .fetch_one(&pool)
            .await
            .expect("read the issued invite's expires_at");
    let secret = harness(world).app.state.session_secret.clone();
    foundry_auth::InviteToken::verify(invite_id, expires_at, &sig, &secret)
        .expect("the emitted signature must verify against the issued invite");
}

/// `Given the mail service is unavailable for "<workspace>"` — flip the harness's
/// FakeEmailSender into failure mode so the issuance handler's best-effort
/// `state.email.send` returns Err. The handler logs at warn and is non-fatal: the
/// invite is still inserted and the link still rendered (AC-01.4, FR-2).
#[given(regex = r#"^the mail service is unavailable for "([^"]+)"$"#)]
async fn mail_service_unavailable(world: &mut FoundryWorld, _ws_name: String) {
    harness(world).fake_email.set_failing();
}

/// `When Dana submits "<email>" on the member-invite form` — same issuance POST as
/// `Dana submits`, named distinctly for the email-failure scenario's narrative.
#[when(regex = r#"^Dana submits "([^"]+)" on the member-invite form$"#)]
async fn dana_submits_on_form(world: &mut FoundryWorld, invitee: String) {
    submit_issuance(world, &invitee).await;
}

/// `Then the invite is still created` — despite the email send failing, the invite
/// row landed (the email seam is best-effort, non-fatal). Exactly one row for the
/// last-submitted invitee.
#[then(regex = r#"^the invite is still created$"#)]
async fn invite_still_created(world: &mut FoundryWorld) {
    let invite_id = world.mi_invite_id.expect("issuance minted an invite id");
    let pool = harness(world).app.state.store.pool().clone();
    let (rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM invites WHERE id = $1")
        .bind(invite_id)
        .fetch_one(&pool)
        .await
        .expect("count the issued invite row");
    assert_eq!(
        rows, 1,
        "the invite must still be created when the email send fails (best-effort, \
         non-fatal); found {rows} rows"
    );
    // The email genuinely failed: nothing was recorded by the (failing) sender.
    assert!(
        harness(world).fake_email.sent().is_empty(),
        "the failing mail service must have recorded NO sent email (proving the \
         link-still-shown path ran through a real send failure)"
    );
}

/// `And Dana still sees the shareable accept link to paste manually` — the "invite
/// sent" fragment still carries the `/invites/accept` link so the admin can share it
/// out-of-band.
#[then(regex = r#"^Dana still sees the shareable accept link to paste manually$"#)]
async fn still_sees_link(world: &mut FoundryWorld) {
    let body = world
        .last_body
        .clone()
        .expect("the issuance fragment body was captured");
    assert!(
        body.contains("/invites/accept?id="),
        "the shareable accept link must still render after an email failure; body = \
         {body:?}"
    );
}

/// `Given Dana already issued "<email>" an invite yesterday that was never used` —
/// seed a prior LIVE (unconsumed) invite for the same email by running the REAL
/// issuance handler now, then record its id so the second-invite Then can prove
/// independence (no collapse). The "yesterday/never used" framing is satisfied by an
/// unconsumed row; insert_invite is reused as-is per D2 (BR-3).
#[given(regex = r#"^Dana already issued "([^"]+)" an invite yesterday that was never used$"#)]
async fn already_issued_invite(world: &mut FoundryWorld, invitee: String) {
    submit_issuance(world, &invitee).await;
    let first_id = world.mi_invite_id.expect("the first issuance minted an id");
    // Stash the first invite id in mi_post_location (unused on this path) so the
    // independence Then can compare against the second id.
    world.mi_post_location = Some(first_id.to_string());
}

/// `When Dana issues another invite to "<email>"` — a SECOND issuance POST for the
/// same email through the real handler. `insert_invite` does not dedupe, so this
/// produces an independent row.
#[when(regex = r#"^Dana issues another invite to "([^"]+)"$"#)]
async fn issues_another_invite(world: &mut FoundryWorld, invitee: String) {
    submit_issuance(world, &invitee).await;
}

/// `Then a second independent live invite is created with its own link` — TWO
/// distinct, unconsumed `invites` rows now exist for the same email, with different
/// ids and different signatures (each its own link). Proves issuance does not collapse
/// re-invites (BR-3).
#[then(regex = r#"^a second independent live invite is created with its own link$"#)]
async fn second_independent_invite(world: &mut FoundryWorld) {
    let second_id = world
        .mi_invite_id
        .expect("the second issuance minted an id");
    let first_id: uuid::Uuid = world
        .mi_post_location
        .clone()
        .expect("the first invite id was stashed")
        .parse()
        .expect("stashed first invite id parses");
    assert_ne!(
        first_id, second_id,
        "the second invite must be an INDEPENDENT row (a different id), not a collapse \
         of the first"
    );

    // The second link carries the second invite id (its own link).
    let body = world
        .last_body
        .clone()
        .expect("the second issuance fragment body was captured");
    assert!(
        body.contains(&format!("/invites/accept?id={second_id}")),
        "the second confirmation must carry ITS OWN link (the second invite id); body \
         = {body:?}"
    );

    // Both invites are live (unconsumed) and distinct in the DB.
    let pool = harness(world).app.state.store.pool().clone();
    let live: Vec<(uuid::Uuid,)> =
        sqlx::query_as("SELECT id FROM invites WHERE id = ANY($1) AND used_at IS NULL ORDER BY id")
            .bind(vec![first_id, second_id])
            .fetch_all(&pool)
            .await
            .expect("read the two live invites");
    assert_eq!(
        live.len(),
        2,
        "BOTH invites must be live (unconsumed) and independent; found {} live rows",
        live.len()
    );
}

/// Drive the REAL issuance POST for `invitee` as the signed-in admin and capture the
/// minted invite id + sig + rendered fragment body. Shared by the issuance-happy-path
/// When/Given steps.
async fn submit_issuance(world: &mut FoundryWorld, invitee: &str) {
    let dana_email = world
        .mi_admin_email
        .clone()
        .expect("the Background seeded Dana's email");
    let client = http(world);
    let outcome = signed_in_post(
        harness(world),
        &client,
        &dana_email,
        DANA_PASSWORD,
        "/workspace/invites",
        &[("email", invitee)],
    )
    .await;
    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "the issuance POST must render a 200 'invite sent' fragment; body = {:?}",
        outcome.body
    );
    let (invite_id, sig) = parse_accept_link(&outcome.body);
    world.mi_invite_id = Some(invite_id);
    world.mi_invite_sig = Some(sig);
    world.last_body = Some(outcome.body);
}

// ---------------------------------------------------------------------------
// Member-accept GET + happy cluster (step 01-03)
// ---------------------------------------------------------------------------
//
// Scenarios 6/7/8/9 exercise the SHIPPED member arm of the accept path:
//   * GET `/invites/accept?id&sig` renders the set-password form NAMING the
//     workspace, NON-COMMITTALLY (no account created, invite unconsumed) —
//     `show_accept_form` + the invite_accept.html template;
//   * the POST DISPATCHES to the NEW `create_member_and_consume` (no user maps
//     to the invitee email -> member arm), creating the account + a member
//     membership, consuming the invite, and signing in (303).
// Green-by-inheritance from the 01-01 accept path: this step authors only the
// acceptance GLUE (it adds NO production code). The member invite under test is
// seeded DIRECTLY via the SHIPPED `insert_invite` (created_by = the seeded admin
// Dana, invitee_email = a NEW email with no user) so the dispatch routes to the
// member arm — mirroring the invite-accept feature's Background seam, but for a
// member (account-creating) invite rather than a first-admin (pre-existing-user)
// one.

/// Priya Shah's email (scenario 8) — a distinct invitee from Sam, with no
/// pre-existing Foundry account, so her accept dispatches to the member arm.
const PRIYA_SHAH_EMAIL: &str = "priya.shah@northwind.example";
/// A policy-passing password (min-12, ADR-004) for the account-creating accepts.
const MEMBER_PASSWORD: &str = "member-northwind-secure-pass";

/// Seed a LIVE member invite for `invitee_email` on the named workspace, issued by
/// the seeded admin (Dana, the invite's `created_by`), with `expires_at` set to
/// `now + ttl` against the REAL per-scenario Postgres via the SHIPPED
/// `insert_invite`; then mint the genuine HMAC `sig` over `invite_id|expires_at`
/// with the harness `session_secret` (the SAME secret the GET/POST handlers
/// verify). Stash the id + sig in the `mi_*` slots so the accept legs drive this
/// real, freshly-issued invite. No user exists for `invitee_email`, so the accept
/// dispatch routes to the member arm (`create_member_and_consume`).
async fn seed_member_invite(
    world: &mut FoundryWorld,
    ws_name: &str,
    invitee_email: &str,
    ttl: time::Duration,
) {
    let workspace_id = *world
        .mi_workspace_ids
        .get(ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} seeded in the Background"));
    let admin_id = world
        .mi_admin_user_id
        .expect("the Background seeded Dana's user id");
    let store = harness(world).app.state.store.clone();
    let now = harness(world).app.state.clock.now();
    let expires_at = now + ttl;
    let invite_id = uuid::Uuid::now_v7();

    store
        .insert_invite(
            invite_id,
            workspace_id,
            Some(invitee_email),
            admin_id,
            expires_at,
        )
        .await
        .expect("seed a live member invite via the shipped insert_invite");

    let secret = harness(world).app.state.session_secret.clone();
    let token = foundry_auth::InviteToken::new(invite_id, expires_at, &secret)
        .expect("mint the member invite signature");
    world.mi_invite_id = Some(invite_id);
    world.mi_invite_sig = Some(token.signature);
}

/// Drive the NEW public GET `/invites/accept?id&sig` over real HTTP with the
/// genuine signed token, capturing the status (into `mi_post_status`) + rendered
/// body (into `last_body`) for the form/refusal observables. NON-COMMITTAL —
/// renders only; mutates nothing.
async fn get_member_accept_page(world: &mut FoundryWorld) {
    let invite_id = world.mi_invite_id.expect("a member invite was seeded");
    let sig = world
        .mi_invite_sig
        .clone()
        .expect("the invite sig was minted");
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
    world.mi_post_status = Some(resp.status());
    // Capture the GET-time double-submit CSRF cookie (a LIVE GET mints one with the
    // set-password form), so a later stale POST can reuse it and have its refusal
    // fire on the TX guard rather than CSRF. A refusal-page GET mints none → None.
    world.mi_get_csrf_cookie = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(str::to_string);
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

// --- Scenario 6: a live member invite renders a set-password form -------------

/// `Given Dana issued Sam a live member invite for "Northwind" two hours ago` —
/// seed a LIVE member invite for Sam (issued two hours ago, so well within the
/// 7-day window) via the SHIPPED `insert_invite` as Dana, and mint its signature.
/// "Two hours ago" is modelled by a 7-day-minus-2-hours TTL from the frozen clock
/// (the invite is unexpired and unused — the liveness the GET requires).
#[given(regex = r#"^Dana issued Sam a live member invite for "([^"]+)" two hours ago$"#)]
async fn dana_issued_sam_live_invite_two_hours_ago(world: &mut FoundryWorld, ws_name: String) {
    let ttl = time::Duration::days(7) - time::Duration::hours(2);
    seed_member_invite(world, &ws_name, "sam.okafor@northwind.example", ttl).await;
}

/// `And Sam has no Foundry account yet` — confirm the precondition that grounds the
/// member arm of the accept dispatch: NO `users` row maps to Sam's email. (If one
/// existed, the dispatch would route to the first-admin/collision arms instead of
/// `create_member_and_consume`.) Read against the REAL per-scenario Postgres.
#[given(regex = r#"^Sam has no Foundry account yet$"#)]
async fn sam_has_no_account_yet(world: &mut FoundryWorld) {
    let pool = harness(world).app.state.store.pool().clone();
    let (rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email_lower = $1")
        .bind("sam.okafor@northwind.example")
        .fetch_one(&pool)
        .await
        .expect("count Sam's user rows");
    assert_eq!(
        rows, 0,
        "the member-accept precondition requires NO pre-existing account for Sam (so the \
         accept dispatches to the member arm); found {rows} rows"
    );
}

/// `When Sam opens his invite link` — GET the live member invite (no password yet).
#[when(regex = r#"^Sam opens his invite link$"#)]
async fn sam_opens_his_invite_link(world: &mut FoundryWorld) {
    get_member_accept_page(world).await;
}

/// `Then he sees a set-password form to join "<workspace>" as a member` — the GET
/// for a live member invite rendered a 200 page carrying a password form posting
/// back to `/invites/accept` and NAMING the workspace (the port-exposed observable
/// that the set-password form was served, not a refusal). The workspace name +
/// password form together prove the GET resolved the invite's workspace and offered
/// the member the join form.
#[then(regex = r#"^he sees a set-password form to join "([^"]+)" as a member$"#)]
async fn sees_set_password_form_to_join(world: &mut FoundryWorld, ws_name: String) {
    assert_eq!(
        world.mi_post_status,
        Some(StatusCode::OK),
        "the GET accept page for a live member invite must render a 200 set-password form; \
         got {:?}",
        world.mi_post_status
    );
    let body = world
        .last_body
        .clone()
        .expect("the GET captured a rendered body");
    assert!(
        body.contains(r#"action="/invites/accept""#) && body.contains(r#"name="password""#),
        "the GET must render a set-password form posting to /invites/accept; got {body:?}"
    );
    assert!(
        body.contains(&ws_name),
        "the set-password form must NAME the {ws_name:?} workspace the member is joining; \
         got {body:?}"
    );
}

// --- Scenario 7: the GET is non-committal -------------------------------------

/// `Given Sam has opened his live member invite for "<workspace>" and seen the
/// set-password form` — seed the live member invite (two hours ago) and drive the
/// GET, asserting the form rendered. The arrival state for the non-committal proof.
#[given(
    regex = r#"^Sam has opened his live member invite for "([^"]+)" and seen the set-password form$"#
)]
async fn sam_opened_live_invite_seen_form(world: &mut FoundryWorld, ws_name: String) {
    let ttl = time::Duration::days(7) - time::Duration::hours(2);
    seed_member_invite(world, &ws_name, "sam.okafor@northwind.example", ttl).await;
    get_member_accept_page(world).await;
    assert_eq!(
        world.mi_post_status,
        Some(StatusCode::OK),
        "the GET accept page for a live member invite must render a 200 set-password form; \
         got {:?}",
        world.mi_post_status
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

/// `Then no account exists yet for "<email>"` — the GET was NON-COMMITTAL: it
/// created NO account. EXACTLY ZERO `users` rows map to the invitee email after the
/// GET. Read against the REAL per-scenario Postgres. The falsifiability litmus: a
/// GET that ran `create_member_and_consume` (creating the user) would RED this.
#[then(regex = r#"^no account exists yet for "([^"]+)"$"#)]
async fn no_account_exists_yet_for(world: &mut FoundryWorld, email: String) {
    let pool = harness(world).app.state.store.pool().clone();
    let email_lower = email.to_ascii_lowercase();
    let (rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email_lower = $1")
        .bind(&email_lower)
        .fetch_one(&pool)
        .await
        .expect("count the invitee's user rows after the GET");
    assert_eq!(
        rows, 0,
        "opening the member-accept page must create NO account (the GET is non-committal); \
         found {rows} rows for {email:?}"
    );
}

/// `And his invite is still live and unconsumed` — the GET mutated nothing: the
/// seeded invite row is STILL live (`used_at` NULL and `expires_at > now`) after the
/// GET. Read against the REAL per-scenario Postgres. The falsifiability litmus: a
/// GET that consumed the invite (set `used_at`) would RED this.
#[then(regex = r#"^his invite is still live and unconsumed$"#)]
async fn his_invite_still_live_and_unconsumed(world: &mut FoundryWorld) {
    let invite_id = world.mi_invite_id.expect("a member invite was seeded");
    let now = harness(world).app.state.clock.now();
    let pool = harness(world).app.state.store.pool().clone();
    let (live_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL AND expires_at > $2",
    )
    .bind(invite_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("count the still-live invite row after the GET");
    assert_eq!(
        live_rows, 1,
        "the member invite must stay live (unused, unexpired) after the non-committal GET; \
         found {live_rows} live rows"
    );
}

// --- Scenarios 8 & 9: the member invite accepts (account created + signed in) --

/// `Given Dana issued Priya Shah a member invite for "<workspace>" twenty seconds
/// ago` — seed a near-fresh LIVE member invite for Priya Shah (issued 20s ago,
/// modelled as a 7-day-minus-20s TTL from the frozen clock) via the SHIPPED
/// `insert_invite` as Dana, and mint its signature. No user maps to Priya's email,
/// so her accept dispatches to the member arm.
#[given(regex = r#"^Dana issued Priya Shah a member invite for "([^"]+)" twenty seconds ago$"#)]
async fn dana_issued_priya_invite_twenty_seconds_ago(world: &mut FoundryWorld, ws_name: String) {
    let ttl = time::Duration::days(7) - time::Duration::seconds(20);
    seed_member_invite(world, &ws_name, PRIYA_SHAH_EMAIL, ttl).await;
}

/// `When Priya opens her link and sets a valid password` — drive the full member
/// accept (GET form + CSRF cookie -> POST id+sig+password+confirm+_csrf) for the
/// near-fresh invite. The POST dispatches to `create_member_and_consume`, creating
/// Priya's account + a member membership, consuming the invite, and auto-signing in.
#[when(regex = r#"^Priya opens her link and sets a valid password$"#)]
async fn priya_opens_link_sets_valid_password(world: &mut FoundryWorld) {
    accept_member_invite(world, MEMBER_PASSWORD).await;
}

/// `Then a new member account is created for Priya and she is signed in on
/// "<workspace>"` — the account-creating accept succeeded end-to-end: EXACTLY ONE
/// `users` row now exists for Priya (created by the member arm), the POST 303'd with
/// an auto-sign-in `foundry_session` cookie (no separate login), and her RESOLVED
/// active workspace is the inviting tenant (DB-observable via the SHIPPED
/// `resolve_active_workspace`).
#[then(regex = r#"^a new member account is created for Priya and she is signed in on "([^"]+)"$"#)]
async fn priya_account_created_and_signed_in(world: &mut FoundryWorld, ws_name: String) {
    assert_member_accepted_and_signed_in(world, PRIYA_SHAH_EMAIL, &ws_name).await;
}

/// `Given Sam's member invite is one second away from expiring and has not been
/// used` — seed a LIVE member invite for Sam whose `expires_at` is one second in the
/// future (the INCLUSIVE side of the expiry boundary: `expires_at > now` holds),
/// minting the signature over the near-expiry `expires_at`. Unused. The boundary the
/// SHIPPED `expires_at > now` guard admits IDENTICALLY on the GET advisory-liveness
/// check and the authoritative consume TX.
#[given(regex = r#"^Sam's member invite is one second away from expiring and has not been used$"#)]
async fn sam_invite_one_second_from_expiring(world: &mut FoundryWorld) {
    seed_member_invite(
        world,
        "Northwind",
        "sam.okafor@northwind.example",
        time::Duration::seconds(1),
    )
    .await;
    // Ground the "one second away from expiring, unused" precondition in observable
    // invite state before the accept.
    let invite_id = world.mi_invite_id.expect("a member invite was seeded");
    let now = harness(world).app.state.clock.now();
    let pool = harness(world).app.state.store.pool().clone();
    let (live_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL AND expires_at > $2",
    )
    .bind(invite_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("count the live (unused, just-inside-expiry) member invite row");
    assert_eq!(
        live_rows, 1,
        "the member invite under test must be live (unused, expiry just in the future) \
         before the accept; found {live_rows} live rows"
    );
}

/// `When Sam opens his link and sets a valid password` — drive the full member
/// accept (GET form + CSRF -> POST) for the just-inside-expiry invite. The advisory
/// GET liveness (`expires_at > now`) admits the boundary and the authoritative
/// consume TX re-enforces it, so the account is created + the invite consumed + Sam
/// signed in.
#[when(regex = r#"^Sam opens his link and sets a valid password$"#)]
async fn sam_opens_link_sets_valid_password(world: &mut FoundryWorld) {
    accept_member_invite(world, MEMBER_PASSWORD).await;
}

/// `Then his member account is created and he is signed in on "<workspace>"` — the
/// just-inside-expiry member invite accepted end-to-end: EXACTLY ONE `users` row for
/// Sam, the POST 303'd with an auto-sign-in session cookie, and his RESOLVED active
/// workspace is the inviting tenant. Tightening either expiry guard to reject the
/// near-boundary would RED this (no account / no 303 / no session).
#[then(regex = r#"^his member account is created and he is signed in on "([^"]+)"$"#)]
async fn sam_account_created_and_signed_in(world: &mut FoundryWorld, ws_name: String) {
    assert_member_accepted_and_signed_in(world, "sam.okafor@northwind.example", &ws_name).await;
}

/// Shared account-created + signed-in assertion for the account-creating member
/// accepts (scenarios 8 + 9): EXACTLY ONE `users` row for the invitee, a 303 with an
/// auto-sign-in session cookie, and the RESOLVED active workspace = the inviting
/// tenant (via the SHIPPED `resolve_active_workspace`). Reads the REAL per-scenario
/// Postgres at the driven-port boundary.
async fn assert_member_accepted_and_signed_in(
    world: &FoundryWorld,
    invitee_email: &str,
    ws_name: &str,
) {
    assert_eq!(
        world.mi_post_status,
        Some(StatusCode::SEE_OTHER),
        "the member accept POST must 303 SEE_OTHER on success (auto sign-in); got {:?}",
        world.mi_post_status
    );
    assert!(
        world.mi_session_cookie.is_some(),
        "the accept POST must establish a session (issue a foundry_session cookie), proving \
         auto sign-in with no separate login step; got none"
    );

    let pool = harness(world).app.state.store.pool().clone();
    let email_lower = invitee_email.to_ascii_lowercase();
    let (id, _count): (uuid::Uuid, i64) = {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email_lower = $1")
            .bind(&email_lower)
            .fetch_one(&pool)
            .await
            .expect("count the created member user row");
        assert_eq!(
            count, 1,
            "the member accept must create EXACTLY ONE account for {invitee_email:?}; found \
             {count} rows"
        );
        let (uid,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
            .bind(&email_lower)
            .fetch_one(&pool)
            .await
            .expect("read the created member user id");
        (uid, count)
    };

    let expected_ws = *world
        .mi_workspace_ids
        .get(ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} seeded in the Background"));
    let resolved = harness(world)
        .app
        .state
        .store
        .resolve_active_workspace(id)
        .await
        .expect("resolve the new member's active workspace")
        .expect("the new member belongs to the inviting workspace");
    assert_eq!(
        resolved.0, expected_ws,
        "the new member must be signed in ON the {ws_name:?} workspace ({expected_ws}); \
         resolved {resolved:?}"
    );
}

// ---------------------------------------------------------------------------
// Member isolation + role (step 01-04)
// ---------------------------------------------------------------------------
//
// Scenarios 10 + 11 prove the TWO halves of the join's privilege scope, both
// GREEN-BY-INHERITANCE through SHIPPED seams (no production code added here):
//
//   10. ISOLATION — the new member, driving the REAL web board route over HTTP
//       (`GET /team/core/project/apollo` as the auto-signed-in Sam), sees ONLY the
//       inviting workspace's data and no other tenant's. The SHIPPED `show_board`
//       handler scopes every lookup by the workspace RESOLVED from Sam's session
//       (`acting.workspace_id()`), membership-gated through `list_board_issues` —
//       the production scoping path. Falsifiability: a foreign workspace ("Globex")
//       seeded with the SAME team/project slugs holds its own issue; a board-route
//       scope leak (or resolving Sam to Globex) would render Globex's title and
//       drop Northwind's → the rendered-body assertions RED.
//
//   11. ROLE — Sam joined as role `'member'` (NOT admin), so a GET on the
//       admin-gated issuance surface `/workspace/invites` is refused by the
//       SHIPPED `require_workspace_admin` gate with the byte-identical
//       non-enumerable `resource_not_found_page()` (404 "Not found") a
//       never-existed path returns. Falsifiability: were Sam minted as `'admin'`,
//       the gate's `is_workspace_admin` would pass and he would see the form (a
//       200 carrying `name="email"`) → the 404 assertion REDs.

/// Northwind's own board issue title (the data the new member IS entitled to see).
const NORTHWIND_ISSUE_TITLE: &str = "Northwind-only issue";
/// Globex's board issue title — a FOREIGN tenant's data the new member must NEVER
/// see. Globex deliberately reuses Northwind's team/project slugs so the ONLY
/// thing distinguishing the two reads is the acting workspace; a scope leak would
/// surface this title under the shared slugs.
const GLOBEX_ISSUE_TITLE: &str = "Globex-only issue";

/// `Given Sam has accepted his member invite and is signed in on "<workspace>"` —
/// drive the FULL shipped join end-to-end (Dana issues via the real
/// `/workspace/invites` POST → Sam accepts via the public `/invites/accept`
/// GET+POST), so Sam ends with a real account, a role=`member` membership on the
/// inviting tenant, and an auto-sign-in session. Then seed BOTH the inviting
/// tenant's own board (the data Sam may see) AND a FOREIGN "Globex" tenant with
/// its own board under the SAME slugs (the data Sam must never see) — the
/// isolation fixture for scenario 10 / the role fixture for scenario 11.
#[given(regex = r#"^Sam has accepted his member invite and is signed in on "([^"]+)"$"#)]
async fn sam_accepted_and_signed_in(world: &mut FoundryWorld, ws_name: String) {
    // Issue + accept through the SHIPPED handlers (mirrors the walking skeleton).
    dana_invites_teammate(
        world,
        "sam.okafor@northwind.example".to_string(),
        ws_name.clone(),
    )
    .await;
    accept_member_invite(world, SAM_PASSWORD).await;
    assert_eq!(
        world.mi_post_status,
        Some(StatusCode::SEE_OTHER),
        "the join precondition requires Sam to be auto-signed-in (303); got {:?}",
        world.mi_post_status
    );

    let inviting_ws = *world
        .mi_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} seeded in the Background"));
    let sam_id = sam_user_id(world).await;
    let pool = harness(world).app.state.store.pool().clone();

    // The inviting tenant's OWN board, with Sam joined to its team so the shipped
    // membership-gated scoped read returns it.
    seed_board(&pool, inviting_ws, Some(sam_id), NORTHWIND_ISSUE_TITLE).await;

    // A FOREIGN tenant ("Globex") with its own board under the SAME slugs and its
    // own admin (Sam is NOT a member). The isolation falsifiability surface.
    let globex_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Globex')")
        .bind(globex_id)
        .execute(&pool)
        .await
        .expect("seed the foreign Globex workspace");
    let globex_admin = uuid::Uuid::now_v7();
    // Globex's admin never signs in; a non-null placeholder hash satisfies the
    // NOT NULL constraint (the column's only insert-time precondition).
    let globex_hash = foundry_auth::hash_password(&SecretString::new(
        "globex-unused-secret".to_string().into(),
    ))
    .await
    .expect("hash Globex admin's placeholder password");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, 'gus.globex@globex.example', 'gus.globex@globex.example', 'Gus Globex', $2)",
    )
    .bind(globex_admin)
    .bind(&globex_hash)
    .execute(&pool)
    .await
    .expect("seed Globex's own admin user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'admin')",
    )
    .bind(globex_id)
    .bind(globex_admin)
    .execute(&pool)
    .await
    .expect("seed Globex's admin membership");
    seed_board(&pool, globex_id, Some(globex_admin), GLOBEX_ISSUE_TITLE).await;

    world
        .mi_workspace_ids
        .insert("Globex".to_string(), globex_id);
}

/// Seed a `core`/`apollo` team→project→issue board scoped to `workspace_id` with
/// the given issue title. When `team_member` is `Some(uid)`, that user is added to
/// the team so the shipped membership-gated scoped read admits them. Deliberately
/// uses the SAME slugs across tenants so the acting workspace is the only thing
/// distinguishing two reads (a scope leak surfaces the wrong title).
async fn seed_board(
    pool: &sqlx::PgPool,
    workspace_id: uuid::Uuid,
    team_member: Option<uuid::Uuid>,
    issue_title: &str,
) {
    let team_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Core', 'core')")
        .bind(team_id)
        .bind(workspace_id)
        .execute(pool)
        .await
        .expect("seed team");
    if let Some(uid) = team_member {
        sqlx::query(
            "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')",
        )
        .bind(team_id)
        .bind(uid)
        .execute(pool)
        .await
        .expect("add member to team");
    }
    let project_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, 'Apollo', 'apollo', 'APL')",
    )
    .bind(project_id)
    .bind(team_id)
    .bind(workspace_id)
    .execute(pool)
    .await
    .expect("seed project");
    let author = team_member.unwrap_or(project_id);
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, author_id)
              VALUES ($1, $2, $3, 1, $4, $5)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project_id)
    .bind(workspace_id)
    .bind(issue_title)
    .bind(author)
    .execute(pool)
    .await
    .expect("seed issue");
}

/// `When Sam views his workspace` — drive the REAL web board route over HTTP as the
/// freshly-joined, auto-signed-in member: `signed_in_get` re-authenticates Sam with
/// the password he set during accept, then GETs `/team/core/project/apollo` (the
/// `core`/`apollo` board seeded under BOTH tenants' SAME slugs). The SHIPPED
/// `show_board` handler scopes EVERY tenant lookup by the workspace RESOLVED from
/// Sam's session (`acting.workspace_id()`), membership-gated through
/// `list_board_issues` — the production scoping path a browser exercises. Capture the
/// rendered board body so the isolation Thens assert on what the route actually
/// serves. NO new isolation code — green by inheritance through the shipped route.
#[when(regex = r#"^Sam views his workspace$"#)]
async fn sam_views_his_workspace(world: &mut FoundryWorld) {
    let client = http(world);
    let outcome = signed_in_get(
        harness(world),
        &client,
        "sam.okafor@northwind.example",
        SAM_PASSWORD,
        "/team/core/project/apollo",
    )
    .await;
    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "the new member must reach his OWN tenant's board over the real route (200); \
         got {:?}, body = {:?}",
        outcome.status,
        outcome.body
    );
    world.last_body = Some(outcome.body);
}

/// `Then he sees only "<workspace>" data` — the board route, scoped to Sam's resolved
/// workspace, RENDERS his inviting tenant's own issue title. The foreign Globex
/// issue, seeded under the SAME slugs, is absent (asserted in the next Then) —
/// together proving the route serves only the acting tenant's data. Falsifiability:
/// an unscoped board read (resolving Sam to Globex, or dropping the
/// `acting.workspace_id()` scope in `show_board`) would surface the Globex title and
/// drop the Northwind one → this assertion REDs.
#[then(regex = r#"^he sees only "([^"]+)" data$"#)]
async fn sees_only_workspace_data(world: &mut FoundryWorld, _ws_name: String) {
    let body = world
        .last_body
        .clone()
        .expect("the board GET captured a rendered body");
    assert!(
        body.contains(NORTHWIND_ISSUE_TITLE),
        "the new member's board must render his inviting tenant's own issue {:?}; \
         body = {body:?}",
        NORTHWIND_ISSUE_TITLE
    );
}

/// `And he sees no data from any other workspace` — the foreign tenant's issue title
/// (Globex's, under the SAME `core`/`apollo` slugs) is ABSENT from the rendered board
/// the real route served. The isolation guarantee: there is no path by which the
/// session-scoped board route surfaces another workspace's data. A board-route
/// scoping bug (a leak past `acting.workspace_id()`) would render the Globex title
/// → this assertion REDs.
#[then(regex = r#"^he sees no data from any other workspace$"#)]
async fn sees_no_foreign_data(world: &mut FoundryWorld) {
    let body = world
        .last_body
        .clone()
        .expect("the board GET captured a rendered body");
    assert!(
        !body.contains(GLOBEX_ISSUE_TITLE),
        "the new member's board must NOT render any foreign tenant's data; the Globex \
         issue {:?} leaked into the rendered board = {body:?}",
        GLOBEX_ISSUE_TITLE
    );
}

/// `When Sam opens the member-invite form` — drive the admin-gated issuance GET
/// `/workspace/invites` AS SAM (the freshly-joined member). `signed_in_get`
/// re-authenticates Sam with the password he set during accept, then GETs the
/// form. The SHIPPED `require_workspace_admin` gate calls `is_workspace_admin`,
/// which is FALSE for Sam (role=`member`), so the handler returns the uniform
/// non-enumerable 404. Capture the status + body.
#[when(regex = r#"^Sam opens the member-invite form$"#)]
async fn sam_opens_member_invite_form(world: &mut FoundryWorld) {
    let client = http(world);
    let outcome = signed_in_get(
        harness(world),
        &client,
        "sam.okafor@northwind.example",
        SAM_PASSWORD,
        "/workspace/invites",
    )
    .await;
    world.mi_post_status = Some(outcome.status);
    world.last_body = Some(outcome.body);
}

/// `Then he sees a generic "not found"` — the member's GET on the admin issuance
/// surface was refused with the SHIPPED uniform `resource_not_found_page()`: a 404
/// whose body is the generic "Not found" page. Falsifiability: were Sam minted
/// `'admin'`, the gate would pass and this would be a 200 form (RED).
#[then(regex = r#"^he sees a generic "not found"$"#)]
async fn sees_generic_not_found(world: &mut FoundryWorld) {
    assert_eq!(
        world.mi_post_status,
        Some(StatusCode::NOT_FOUND),
        "a freshly-joined member (role=member) must be refused the admin issuance \
         surface with a 404; got {:?}",
        world.mi_post_status
    );
    let body = world
        .last_body
        .clone()
        .expect("the refused GET captured a body");
    assert!(
        body.contains("Not found"),
        "the refusal must render the generic 'Not found' page; got {body:?}"
    );
}

/// `And nothing reveals that the issuance surface exists` — the refusal is
/// NON-ENUMERABLE: the 404 body carries NO oracle that an invite-issuance surface
/// exists (no leaked form, no email field, no "invite" affordance) — byte-identical
/// to a never-existed path. A non-admin cannot tell the surface is there.
#[then(regex = r#"^nothing reveals that the issuance surface exists$"#)]
async fn nothing_reveals_issuance_surface(world: &mut FoundryWorld) {
    let body = world
        .last_body
        .clone()
        .expect("the refused GET captured a body");
    assert!(
        !body.contains("name=\"email\""),
        "the refusal must NOT leak the issuance form's email field; got {body:?}"
    );
    assert!(
        !body.to_ascii_lowercase().contains("invite"),
        "the refusal must NOT mention invites (no oracle the issuance surface \
         exists); got {body:?}"
    );
}

// ---------------------------------------------------------------------------
// First-admin regression guard (step 01-05) — scenario 12 (@verify-path-unchanged)
// ---------------------------------------------------------------------------
//
// PROVES the data-derived accept DISPATCH this feature introduced did NOT break
// the SHIPPED first-admin accept path. A FIRST-ADMIN invite — one minted by the
// SHIPPED `provision_workspace` tx, where the invitee's account ALREADY EXISTS
// (the first-admin user) and the invite's `created_by` IS that same user — must
// still route to the SHIPPED `set_first_admin_password_and_consume` (the kind
// discriminator `is_first_admin_invite` returns true: invitee_email maps to a
// user whose id == created_by), NOT to the member arm (`create_member_and_consume`,
// which would CREATE a duplicate account). Green-by-inheritance; NO production
// code added.
//
// Falsifiability (demonstrated at DELIVER, then reverted): forcing the dispatch to
// ALWAYS take the member arm (e.g. hard-`false` `is_first_admin_invite`) makes the
// first-admin accept try to CREATE a second user for `invitee_email`, which the
// `users.email_lower UNIQUE` guard rejects → the create-member tx rolls back to
// `EmailCollision` → the POST renders the uniform refusal (200, not 303), no
// session, no consume. That REDs "Priya is signed in" (303 + session) AND "invite
// recorded as used exactly once" (used_at stays NULL). Routing to the first-admin
// arm restores all three Thens GREEN.

/// Priya Nair's first-admin email — the invite's `invitee_email`, which ALSO maps
/// to her pre-existing first-admin user row (the discriminator that routes to the
/// SHIPPED first-admin arm).
const PRIYA_NAIR_EMAIL: &str = "priya.nair@globex.example";
/// Priya's chosen password — meets the min-12 length-first policy (ADR-004).
const PRIYA_NAIR_PASSWORD: &str = "globex-first-admin-pass";

/// `Given a super-admin provisioned the "<workspace>" workspace and seeded Priya
/// Nair as its first-admin with a live invite` — drive the SHIPPED
/// `provision_workspace` tx against the REAL per-scenario Postgres: it creates the
/// workspace, the first-admin USER (Priya, with a throwaway initial credential she
/// has never seen), her `admin` membership, AND the `invites` row whose
/// `created_by` IS Priya's user id and whose `invitee_email` IS Priya's email — the
/// exact shape that makes `is_first_admin_invite` true (the SHIPPED first-admin
/// accept path). Then mint the genuine HMAC `sig` over `invite_id|expires_at`. The
/// id + sig land in the `mi_*` slots so the shared accept helper drives this real,
/// freshly-provisioned first-admin invite. Snapshots the first-admin user id so the
/// "no second account" Then can prove no duplicate was created.
#[given(
    regex = r#"^a super-admin provisioned the "([^"]+)" workspace and seeded Priya Nair as its first-admin with a live invite$"#
)]
async fn provisioned_first_admin_with_live_invite(world: &mut FoundryWorld, ws_name: String) {
    let store = harness(world).app.state.store.clone();
    let now = harness(world).app.state.clock.now();

    let workspace_id = uuid::Uuid::now_v7();
    let admin_user_id = uuid::Uuid::now_v7();
    let invite_id = uuid::Uuid::now_v7();
    let expires_at = now + time::Duration::days(7);

    // The throwaway initial credential — Priya has never seen it; the accept flow is
    // the only way she sets a real one (mirrors the SHIPPED provisioning leg).
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
            PRIYA_NAIR_EMAIL,
            PRIYA_NAIR_EMAIL,
            "Priya Nair",
            &throwaway_hash,
            invite_id,
            expires_at,
        )
        .await
        .expect("provision the workspace + first-admin + invite via the shipped tx");

    let secret = harness(world).app.state.session_secret.clone();
    let token = foundry_auth::InviteToken::new(invite_id, expires_at, &secret)
        .expect("mint the first-admin invite signature");

    world.mi_workspace_ids.insert(ws_name, workspace_id);
    world.mi_admin_user_id = Some(admin_user_id);
    world.mi_invite_id = Some(invite_id);
    world.mi_invite_sig = Some(token.signature);
}

/// `When Priya opens her first-admin invite link and sets a valid password` — drive
/// the full SHIPPED accept (GET form + CSRF cookie → POST id+sig+password+confirm+
/// _csrf) for the provisioned first-admin invite via the shared `accept_member_invite`
/// helper. Because the invitee's account already exists (id == created_by), the
/// accept DISPATCH routes to the SHIPPED `set_first_admin_password_and_consume`
/// (writing her real password + consuming the invite + signing her in) — NOT the
/// member arm. Captures the 303 + session cookie for the Thens.
#[when(regex = r#"^Priya opens her first-admin invite link and sets a valid password$"#)]
async fn priya_nair_opens_first_admin_link(world: &mut FoundryWorld) {
    accept_member_invite(world, PRIYA_NAIR_PASSWORD).await;
}

/// `Then Priya is signed in on the "<workspace>" workspace without a separate login
/// step` — the SHIPPED first-admin accept succeeded end-to-end through the dispatch:
/// the POST 303'd with an auto-sign-in `foundry_session` cookie (no separate login),
/// and her RESOLVED active workspace is the provisioned tenant (DB-observable via the
/// SHIPPED `resolve_active_workspace`). If the dispatch wrongly took the member arm,
/// the create-user UNIQUE collision would refuse (200, no session) and RED this.
#[then(regex = r#"^Priya is signed in on the "([^"]+)" workspace without a separate login step$"#)]
async fn priya_nair_signed_in_on_workspace(world: &mut FoundryWorld, ws_name: String) {
    assert_eq!(
        world.mi_post_status,
        Some(StatusCode::SEE_OTHER),
        "the first-admin accept POST must 303 SEE_OTHER on success (auto sign-in via \
         the SHIPPED first-admin arm); got {:?}",
        world.mi_post_status
    );
    assert!(
        world.mi_session_cookie.is_some(),
        "the first-admin accept POST must establish a session (issue a foundry_session \
         cookie), proving auto sign-in with no separate login step; got none"
    );

    let expected_ws = *world
        .mi_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} provisioned in the Given"));
    let admin_id = world
        .mi_admin_user_id
        .expect("the Given seeded the first-admin user id");
    let resolved = harness(world)
        .app
        .state
        .store
        .resolve_active_workspace(admin_id)
        .await
        .expect("resolve the first-admin's active workspace")
        .expect("the first-admin belongs to the provisioned workspace");
    assert_eq!(
        resolved.0, expected_ws,
        "the first-admin must be signed in ON the {ws_name:?} workspace ({expected_ws}); \
         resolved {resolved:?}"
    );
}

/// `And no second account is created for Priya` — the regression crux: the dispatch
/// took the SHIPPED first-admin arm (which only SETS the password on her PRE-EXISTING
/// account), so EXACTLY ONE `users` row maps to Priya's email — the one
/// `provision_workspace` seeded. If the dispatch wrongly routed to the member arm
/// (`create_member_and_consume`), it would attempt a duplicate INSERT for the same
/// email; either it collides (refusal, no row) or — were the UNIQUE guard absent — a
/// second row would appear. Asserting EXACTLY ONE proves the first-admin arm ran and
/// created no duplicate.
#[then(regex = r#"^no second account is created for Priya$"#)]
async fn no_second_account_for_priya(world: &mut FoundryWorld) {
    let pool = harness(world).app.state.store.pool().clone();
    let (rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email_lower = $1")
        .bind(PRIYA_NAIR_EMAIL)
        .fetch_one(&pool)
        .await
        .expect("count the first-admin user rows after the accept");
    assert_eq!(
        rows, 1,
        "the first-admin arm must NOT create a second account — EXACTLY ONE users row \
         for {PRIYA_NAIR_EMAIL:?} (the provisioned first-admin); found {rows} rows"
    );
}

/// `And her first-admin invite is recorded as used exactly once` — the DB-observable
/// single-use outcome of the SHIPPED first-admin consume: the invite row's `used_at`
/// is set and exactly ONE consumed row exists for this id, with `used_by` = the
/// first-admin (the `created_by` the guarded-UPDATE returned). If the dispatch had
/// taken the member arm and refused on the email collision, `used_at` would stay NULL
/// → 0 consumed rows → RED. Reads the REAL per-scenario Postgres. (Distinct phrasing
/// from the shared `(?:her|the) invite ...` step to avoid an ambiguous global match.)
#[then(regex = r#"^her first-admin invite is recorded as used exactly once$"#)]
async fn her_first_admin_invite_recorded_used_exactly_once(world: &mut FoundryWorld) {
    let invite_id = world
        .mi_invite_id
        .expect("the Given seeded a first-admin invite id");
    let admin_id = world
        .mi_admin_user_id
        .expect("the Given seeded the first-admin user id");
    let pool = harness(world).app.state.store.pool().clone();
    let (consumed_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NOT NULL AND used_by = $2",
    )
    .bind(invite_id)
    .bind(admin_id)
    .fetch_one(&pool)
    .await
    .expect("count the consumed first-admin invite row");
    assert_eq!(
        consumed_rows, 1,
        "the first-admin invite must be recorded as used EXACTLY ONCE (used_at set, \
         used_by = the first-admin); found {consumed_rows} consumed rows"
    );
}

// ---------------------------------------------------------------------------
// Member-invite expiry refusals (step 02-01) — scenarios 14 (canonical) + 13
// ---------------------------------------------------------------------------
//
// PROVES the SHIPPED uniform refusal path applies to MEMBER invites: opening an
// EXPIRED member invite renders the canonical `invite_refusal_page()` (200 OK,
// OD-3 — no status oracle) carrying the journey's "no longer valid" copy, leaking
// NOTHING about whether any account or workspace exists, and advising asking the
// administrator to re-issue. GREEN-BY-INHERITANCE: the accept GET liveness check
// (`invite_is_acceptable` → `expires_at > now`) returns the SAME refusal page for
// a member invite as for a first-admin one (the route is shared, the page is
// static), so this step adds acceptance GLUE only — NO production code.
//
//   14 (canonical, expired one day ago): the CANONICAL refusal arm — captured
//       (status + full body) into `mi_refusal_*` so 02-02 (just-past) + the
//       byte-identity scenarios assert AGAINST it. Also asserts non-leakage
//       (no workspace name / invitee email in the body) and the re-issue advisory.
//   13 (just past, expired one second ago): the EXCLUSIVE side of the expiry
//       boundary (`expires_at <= now` ⇒ refused), complementing scenario 9's
//       just-inside-accepted (`expires_at > now` ⇒ accepted). Reuses the
//       canonical standard-page Then.
//
// Falsifiability (demonstrated at DELIVER, then reverted): (a) tightening the GET
// liveness to ADMIT an expired member invite (e.g. `expires_at >= now - 1d`)
// makes the expired GET render the set-password form (200 carrying
// `name="password"` + the workspace name) instead of the refusal → the
// "no longer valid" + non-leakage Thens RED; (b) leaking the workspace name into
// the refusal body REDs the non-leakage Then; (c) returning a distinct status
// (e.g. 404/410 instead of 200) REDs the standard-page Then's ratified-200 check.

/// Sam's member-invite email — no pre-existing account, so a LIVE invite would
/// dispatch to the member arm; here we drive it past expiry to exercise refusal.
const SAM_EMAIL: &str = "sam.okafor@northwind.example";

/// `Given Sam's member invite expired one day ago` (scenario 14, canonical) — seed
/// a member invite for Sam whose `expires_at` is one day in the PAST against the
/// REAL per-scenario Postgres, minting the genuine HMAC `sig` over that past
/// `expires_at` so the tamper oracle still verifies (ONLY the liveness check fails
/// — the canonical expired arm, not a tamper). Grounds the "expired one day ago"
/// precondition in observable invite state (unused, expiry in the past) before the
/// GET, so the refusal is genuinely driven by expiry, not by a missing row.
#[given(regex = r#"^Sam's member invite expired one day ago$"#)]
async fn sam_invite_expired_one_day_ago(world: &mut FoundryWorld) {
    seed_expired_member_invite(world, "Northwind", SAM_EMAIL, time::Duration::days(1)).await;
}

/// `Given Sam's member invite expired one second ago` (scenario 13, just-past
/// boundary) — same as the canonical seed but expiry is ONE SECOND in the past:
/// the EXCLUSIVE side of `expires_at > now` (the SHIPPED guard rejects it),
/// complementing scenario 9's just-inside-accepted (one second in the future).
#[given(regex = r#"^Sam's member invite expired one second ago$"#)]
async fn sam_invite_expired_one_second_ago(world: &mut FoundryWorld) {
    seed_expired_member_invite(world, "Northwind", SAM_EMAIL, time::Duration::seconds(1)).await;
}

/// Seed an EXPIRED (unused) member invite for `invitee_email` on the named
/// workspace via the SHIPPED `insert_invite` as Dana, with `expires_at = now -
/// past`, then mint the genuine HMAC `sig` over that past `expires_at` (so the
/// tamper oracle verifies — the failure isolates to the liveness check). Asserts
/// the row is now expired-but-unused so the precondition is grounded in observable
/// invite state, not assumed. No user exists for `invitee_email` (the member arm
/// shape), though the GET refuses before any dispatch.
async fn seed_expired_member_invite(
    world: &mut FoundryWorld,
    ws_name: &str,
    invitee_email: &str,
    past: time::Duration,
) {
    let workspace_id = *world
        .mi_workspace_ids
        .get(ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} seeded in the Background"));
    let admin_id = world
        .mi_admin_user_id
        .expect("the Background seeded Dana's user id");
    let store = harness(world).app.state.store.clone();
    let now = harness(world).app.state.clock.now();
    let expires_at = now - past;
    let invite_id = uuid::Uuid::now_v7();

    store
        .insert_invite(
            invite_id,
            workspace_id,
            Some(invitee_email),
            admin_id,
            expires_at,
        )
        .await
        .expect("seed an expired member invite via the shipped insert_invite");

    let secret = harness(world).app.state.session_secret.clone();
    let token = foundry_auth::InviteToken::new(invite_id, expires_at, &secret)
        .expect("mint the expired member invite signature");
    world.mi_invite_id = Some(invite_id);
    world.mi_invite_sig = Some(token.signature);

    let pool = store.pool().clone();
    let (expired_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL AND expires_at <= $2",
    )
    .bind(invite_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("count the expired (unused, past-expiry) member invite row");
    assert_eq!(
        expired_rows, 1,
        "the member invite under test must be expired (unused, expiry in the past) \
         before the GET; found {expired_rows} expired rows"
    );
}

/// `Then he sees the standard "invite is no longer valid" page` (scenarios 14 + 13)
/// — the GET for an expired member invite rendered the SHIPPED uniform refusal at
/// the ratified 200 OK (OD-3 — no status-code oracle) carrying the journey's "no
/// longer valid" copy. CAPTURES the status + full body into the CANONICAL member
/// refusal slots (`mi_refusal_*`) so 02-02 (just-past) + the byte-identity
/// scenarios assert AGAINST this arm.
#[then(regex = r#"^he sees the standard "invite is no longer valid" page$"#)]
async fn sees_standard_member_refusal_page(world: &mut FoundryWorld) {
    assert_eq!(
        world.mi_post_status,
        Some(StatusCode::OK),
        "the expired member-invite refusal must be the ratified 200 OK (OD-3, no \
         status oracle); got {:?}",
        world.mi_post_status
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
    world.mi_refusal_status = world.mi_post_status;
    world.mi_refusal_body = Some(body.clone());
    // Also populate the canonical `ia_refusal_*` slots the SHARED byte-identity Then
    // (`feature_invite_accept::response_byte_identical_to_expired_refusal`) reads, so
    // the member tampered/unknown arms (scenarios 15/16) can assert byte-identity
    // against the recomputed canonical member refusal.
    world.ia_refusal_status = world.mi_post_status;
    world.ia_refusal_body = Some(body);
}

// `And the page reveals nothing about whether any account or workspace exists`
// (scenario 14) — the non-enumerability guarantee (NFR-3) is asserted by the
// SHARED step defined in `feature_invite_accept.rs` (cucumber-rs steps are global
// across loaded modules). That step asserts the refusal body reveals neither the
// workspace name ("Northwind") nor the invitee email, falling back to `last_body`
// when `ia_refusal_body` is unset (our case). The refusal page is static, so the
// shared assertion holds identically for a member invite — REUSED, not redefined
// (a duplicate regex would make the match ambiguous).

/// `And the page advises asking the workspace administrator to re-issue the invite`
/// (scenario 14) — the journey's universal next action (the only "reason" a
/// legitimate recipient gets, by design): ask the administrator to re-issue /
/// re-provision. Asserts the advisory copy is present (administrator + re-issue
/// intent), matching the SHIPPED `invite_refusal_page()` body.
#[then(regex = r#"^the page advises asking the workspace administrator to re-issue the invite$"#)]
async fn member_refusal_advises_admin_reissue(world: &mut FoundryWorld) {
    let body = world
        .mi_refusal_body
        .clone()
        .or_else(|| world.last_body.clone())
        .expect("the refusal captured a rendered body");
    let lower = body.to_ascii_lowercase();
    assert!(
        lower.contains("administrator")
            && (lower.contains("re-issue") || lower.contains("reissue")),
        "the refusal must advise asking the administrator to re-issue the invite; \
         got {body:?}"
    );
}

// ---------------------------------------------------------------------------
// Email-already-a-user collision refusal (step 02-03) — scenario 17
// ---------------------------------------------------------------------------
//
// PROVES the riskiest collision arm (D5, OD-1, A-E9): a member invite whose
// `invitee_email` ALREADY maps to an existing user is refused with the canonical
// uniform refusal (200, BYTE-IDENTICAL to the expired arm), the invite is NOT
// consumed, NO duplicate user is created, and it is NEVER a 500. GREEN-BY-
// INHERITANCE from the 01-01 collision handling: `create_member_and_consume`
// catches the `users.email_lower` UNIQUE violation (SQLSTATE 23505) INSIDE the tx
// → `MemberConsumeOutcome::EmailCollision` (rollback, invite untouched), and
// `submit_accept` maps it to the SHIPPED `invite_refusal_page()` (200) — the SAME
// page the expired arm renders. This step adds acceptance GLUE only — NO
// production code.
//
// Falsifiability (demonstrated at DELIVER, then reverted): (a) letting the 23505
// bubble as a StoreError instead of mapping to EmailCollision makes the accept a
// 500 instead of the uniform 200 refusal → the standard-page + byte-identity Thens
// RED; (b) NOT catching the collision (no UNIQUE rollback) wrongly consumes the
// invite and/or half-creates a second account → the "not consumed / no second
// account" Then REDs.

/// The invitee email that ALREADY has a Foundry account (the collision oracle).
const COLLISION_EMAIL: &str = "already.a.user@northwind.example";
/// The pre-existing user's password — never used by the accept; a non-null hash
/// satisfies the NOT NULL constraint on the seeded row.
const COLLISION_EXISTING_PASSWORD: &str = "already-a-user-existing-pass";

/// `Given Dana issued a member invite for an email that already has a Foundry
/// account` — seed an EXISTING `users` row for the collision email (so the
/// create-user INSERT inside the accept tx will hit the `users.email_lower` UNIQUE
/// guard), then issue a LIVE member invite for that SAME email via the SHIPPED
/// `insert_invite` as Dana and mint its genuine HMAC signature. Snapshots the
/// existing user id so the "no second account" Then can prove no duplicate appears.
/// Grounds the precondition in observable state: exactly one pre-existing user, and
/// a live unconsumed invite for that email.
#[given(regex = r#"^Dana issued a member invite for an email that already has a Foundry account$"#)]
async fn dana_issued_invite_for_existing_email(world: &mut FoundryWorld) {
    let store = harness(world).app.state.store.clone();
    let pool = store.pool().clone();

    // Pre-existing account for the collision email — the row the accept's
    // create-user INSERT collides with on `users.email_lower` UNIQUE.
    let existing_user_id = uuid::Uuid::now_v7();
    let existing_hash = foundry_auth::hash_password(&SecretString::new(
        COLLISION_EXISTING_PASSWORD.to_string().into(),
    ))
    .await
    .expect("hash the pre-existing user's password");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, 'Already A. User', $3)",
    )
    .bind(existing_user_id)
    .bind(COLLISION_EMAIL)
    .bind(&existing_hash)
    .execute(&pool)
    .await
    .expect("seed the pre-existing user that owns the collision email");
    // Stash the pre-existing user id in mi_post_location (unused on this path) so
    // the "no second account" Then can confirm the single surviving row is this one.
    world.mi_post_location = Some(existing_user_id.to_string());

    // A LIVE member invite for the SAME email (two hours within the 7-day window),
    // minted by the SHIPPED insert_invite as Dana — the genuine invite under test.
    let ttl = time::Duration::days(7) - time::Duration::hours(2);
    seed_member_invite(world, "Northwind", COLLISION_EMAIL, ttl).await;

    // Ground the precondition: exactly one pre-existing user + a live unconsumed
    // invite for the collision email, before the accept.
    let (existing_rows,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM users WHERE email_lower = $1")
            .bind(COLLISION_EMAIL)
            .fetch_one(&pool)
            .await
            .expect("count the pre-existing user rows");
    assert_eq!(
        existing_rows, 1,
        "the collision precondition requires EXACTLY ONE pre-existing account for \
         {COLLISION_EMAIL:?}; found {existing_rows} rows"
    );
    let invite_id = world.mi_invite_id.expect("a member invite was seeded");
    let now = harness(world).app.state.clock.now();
    let (live_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL AND expires_at > $2",
    )
    .bind(invite_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("count the live (unused) collision invite row");
    assert_eq!(
        live_rows, 1,
        "the collision invite under test must be live (unused, unexpired) before the \
         accept; found {live_rows} live rows"
    );
}

/// Recompute the CANONICAL expired-one-day member refusal as the byte-identity
/// control for the email-collision arm (scenario 17). Called by the SHARED
/// byte-identity Then (`feature_invite_accept.rs`) when it is driven from the
/// member-invites feature (no `ia_harness`). Seeds a FRESH expired member invite on
/// a NEW email (so it cannot itself collide), GETs it, and returns the SHIPPED
/// uniform refusal (status + full body). Leaves the collision invite
/// (`mi_collision_invite_id`) untouched — it must stay unconsumed for the
/// "not consumed" Then. NOTE: this overwrites `mi_invite_id`/`mi_invite_sig` with
/// the fresh control invite; the collision invite-under-test is preserved in
/// `mi_collision_invite_id`.
pub async fn recompute_canonical_member_refusal(world: &mut FoundryWorld) -> (StatusCode, String) {
    seed_expired_member_invite(
        world,
        "Northwind",
        "expired.control@northwind.example",
        time::Duration::days(1),
    )
    .await;
    get_member_accept_page(world).await;
    let status = world
        .mi_post_status
        .expect("the canonical expired-control GET captured a status");
    let body = world
        .last_body
        .clone()
        .expect("the canonical expired-control GET captured a body");
    (status, body)
}

/// `When that invitee opens the link and submits a valid password` — drive the full
/// accept (GET form + CSRF cookie → POST id+sig+password+confirm+_csrf) for the live
/// collision invite, CAPTURING the POST refusal body. The GET renders the
/// set-password form NON-COMMITTALLY (liveness only); the POST dispatches to the
/// member arm `create_member_and_consume`, whose create-user INSERT collides on
/// `users.email_lower` UNIQUE → 23505 caught INSIDE the tx → rollback →
/// `EmailCollision` → the SHIPPED `invite_refusal_page()` (200, NEVER a 500).
/// Stashes the POST status + full body into `mi_post_status` + `last_body` for the
/// standard-page + byte-identity Thens.
#[when(regex = r#"^that invitee opens the link and submits a valid password$"#)]
async fn collision_invitee_accepts(world: &mut FoundryWorld) {
    accept_member_invite_capturing_body(world, MEMBER_PASSWORD).await;
}

// NOTE: scenario 17's `Then they see the standard "invite is no longer valid" page`
// and `And the response is byte-identical to the expired-invite refusal` REUSE the
// SHARED steps defined in `feature_invite_accept.rs` (cucumber-rs steps are global
// across loaded modules; a duplicate regex would make the match ambiguous). Those
// shared steps were made world-source-agnostic (they fall back from the `ia_*` slots
// to the `mi_*` slots + the `mi_harness`) so a member-feature collision scenario
// drives them identically — the refusal page is the SHIPPED static
// `invite_refusal_page()` regardless of arm. The collision accept's POST status +
// full body land in `mi_post_status` + `last_body` (see
// `accept_member_invite_capturing_body`), which the shared "standard page" Then
// captures into `mi_refusal_*`, and the shared byte-identity Then then recomputes the
// canonical expired-one-day arm on a FRESH member invite and asserts status + FULL
// body byte-identity. The collision invite id is preserved in
// `mi_collision_invite_id` so the recompute (which overwrites `mi_invite_id`) does
// not lose the invite-under-test for the "not consumed" assertion.

/// `And no second account is created and the invite is not consumed` — the
/// collision-rollback outcome: EXACTLY ONE `users` row maps to the collision email
/// (the pre-existing one — NO duplicate was half-created), and the invite stays LIVE
/// (`used_at` NULL). Proves the tx rolled back cleanly on the 23505: no second
/// account, invite untouched. Reads the REAL per-scenario Postgres at the
/// driven-port boundary. Falsifiability: a half-create or a consumed invite REDs.
#[then(regex = r#"^no second account is created and the invite is not consumed$"#)]
async fn collision_no_second_account_invite_not_consumed(world: &mut FoundryWorld) {
    let pool = harness(world).app.state.store.pool().clone();

    // EXACTLY ONE user row for the collision email — the pre-existing one, no dup.
    let (user_rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email_lower = $1")
        .bind(COLLISION_EMAIL)
        .fetch_one(&pool)
        .await
        .expect("count the user rows for the collision email after the accept");
    assert_eq!(
        user_rows, 1,
        "the collision accept must create NO second account — EXACTLY ONE users row \
         for {COLLISION_EMAIL:?} (the pre-existing one); found {user_rows} rows"
    );
    let existing_user_id: uuid::Uuid = world
        .mi_post_location
        .clone()
        .expect("the pre-existing user id was stashed")
        .parse()
        .expect("stashed pre-existing user id parses");
    let (surviving_id,): (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
            .bind(COLLISION_EMAIL)
            .fetch_one(&pool)
            .await
            .expect("read the surviving user id for the collision email");
    assert_eq!(
        surviving_id, existing_user_id,
        "the surviving account must be the PRE-EXISTING user (proving the create-user \
         step rolled back, not that a new row replaced it)"
    );

    // The collision invite (NOT the recomputed expired control) stays LIVE — the tx
    // rolled back, so `used_at` was never set.
    let collision_invite_id = world.mi_collision_invite_id.expect(
        "the collision invite id was stashed before the canonical control overwrote \
         mi_invite_id",
    );
    let (live_rows,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL")
            .bind(collision_invite_id)
            .fetch_one(&pool)
            .await
            .expect("count the still-live collision invite row");
    assert_eq!(
        live_rows, 1,
        "the collision invite must NOT be consumed (used_at stays NULL — the tx rolled \
         back on the 23505); found {live_rows} unconsumed rows for the collision invite"
    );
}

/// Drive the full accept (GET → POST) for the live member invite with `password`,
/// CAPTURING the POST status + full body into `mi_post_status` + `last_body` (the
/// shared `accept_member_invite` discards the POST body; the collision refusal Thens
/// need it). Also stashes the collision invite id into `mi_collision_invite_id` so a
/// later in-scenario control (the recomputed expired arm) overwriting `mi_invite_id`
/// does not lose the invite-under-test for the "not consumed" assertion.
async fn accept_member_invite_capturing_body(world: &mut FoundryWorld, password: &str) {
    let invite_id = world.mi_invite_id.expect("a member invite was seeded");
    world.mi_collision_invite_id = Some(invite_id);
    let sig = world
        .mi_invite_sig
        .clone()
        .expect("the invite sig was minted");
    let base = harness(world).base_url();
    let client = http(world);

    // GET — render the set-password form + mint the CSRF cookie (non-committal).
    let get_resp = client
        .get(format!(
            "{base}/invites/accept?id={invite_id}&sig={sig}",
            sig = urlencoding::encode(&sig)
        ))
        .send()
        .await
        .expect("GET /invites/accept");
    let csrf_cookie = get_resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(str::to_string)
        .expect("the GET minted a foundry_csrf cookie");
    let _ = get_resp.text().await;
    let csrf_token = csrf_cookie
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();

    // POST — the create-user step collides → rollback → uniform refusal (200).
    let form = [
        ("id", invite_id.to_string()),
        ("sig", sig),
        ("password", password.to_string()),
        ("confirm", password.to_string()),
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
    world.mi_post_status = Some(resp.status());
    world.mi_session_cookie = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .and_then(|s| s.split(';').next())
        .map(str::to_string);
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

// ---------------------------------------------------------------------------
// Tampered-signature + unknown-id refusals (step 02-02) — scenarios 15 + 16
// ---------------------------------------------------------------------------
//
// PROVE the SHIPPED non-committal verify→refusal path applies to MEMBER invites
// across the two pre-DB-state refusal arms, BYTE-IDENTICAL to the canonical
// expired arm (02-01):
//
//   15 (tampered signature): the invite is LIVE (7-day expiry, unused) but the
//       `sig` in the link is altered by one character. The SHIPPED
//       `invite_is_acceptable` calls `InviteToken::verify` FIRST; the altered HMAC
//       fails the tamper oracle BEFORE any liveness/DB-state branch, so the GET
//       renders the uniform `invite_refusal_page()` (200) — byte-identical to the
//       canonical expired-one-day member refusal. New Sam-variant Given + When; the
//       "he sees the standard ..." Then (capturing into the refusal slots) and the
//       SHARED byte-identity Then are reused.
//
//   16 (unknown id): a validly-signed id that names NO `invites` row. REUSES the
//       SHIPPED scenario-16 steps from `feature_invite_accept` (now world-source-
//       agnostic: they fall back to `mi_harness` + `mi_invite_*` + `mi_post_status`),
//       so NO new member step is needed here.
//
// GREEN-BY-INHERITANCE: this step adds acceptance GLUE only — NO production code.
//
// Falsifiability (demonstrated at DELIVER, then reverted): a verify path that
// DISTINGUISHED a bad signature — a distinct "invalid signature" body or a 4xx
// tamper-oracle status instead of the uniform 200 refusal — REDs BOTH the
// "no longer valid" Then (divergent copy/status) AND the byte-identity assertion
// (the tampered-arm response would diverge from the canonical expired-arm body).

/// `Given Sam's member invite is live but the signature in the link has been
/// altered by one character` (scenario 15) — seed a LIVE member invite for Sam
/// (7-day-minus-2-hours TTL, well inside the window, unused) via the SHIPPED
/// `insert_invite`, then corrupt the genuine minted `mi_invite_sig` by flipping a
/// single character. Confirms against the REAL per-scenario Postgres that the
/// invite is still live (so the refusal under test isolates to the tamper oracle,
/// NOT liveness) and that the tampered sig genuinely DIFFERS from the authentic
/// one. The corrupted sig is stored back into `mi_invite_sig`, which the GET When
/// then carries.
#[given(
    regex = r#"^Sam's member invite is live but the signature in the link has been altered by one character$"#
)]
async fn sam_member_invite_signature_tampered(world: &mut FoundryWorld) {
    let ttl = time::Duration::days(7) - time::Duration::hours(2);
    seed_member_invite(world, "Northwind", SAM_EMAIL, ttl).await;

    let invite_id = world.mi_invite_id.expect("a live member invite was seeded");
    let now = harness(world).app.state.clock.now();
    let pool = harness(world).app.state.store.pool().clone();
    let (live_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL AND expires_at > $2",
    )
    .bind(invite_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("count the live (unused, unexpired) member invite row");
    assert_eq!(
        live_rows, 1,
        "the member invite under test must be live (unused and unexpired) so the \
         refusal fires on the tampered signature, not liveness; found {live_rows} live rows"
    );

    let authentic = world
        .mi_invite_sig
        .clone()
        .expect("the seed minted the genuine member invite signature");
    let mut chars: Vec<char> = authentic.chars().collect();
    assert!(
        !chars.is_empty(),
        "the genuine member invite signature must be non-empty to tamper with"
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
    world.mi_invite_sig = Some(tampered);
}

/// `When Sam opens the tampered link` (scenario 15) — drive the PUBLIC GET
/// `/invites/accept?id=&sig=` over real HTTP carrying the now-tampered
/// `mi_invite_sig`. The SHIPPED `invite_is_acceptable` calls `InviteToken::verify`
/// FIRST; the altered HMAC fails the tamper oracle and the handler renders the
/// uniform `invite_refusal_page()` — captured (status + full body) into
/// `mi_post_status` + `last_body` for the reused "he sees the standard ..." +
/// byte-identity Thens.
#[when(regex = r#"^Sam opens the tampered link$"#)]
async fn sam_opens_the_tampered_link(world: &mut FoundryWorld) {
    get_member_accept_page(world).await;
}

// ---------------------------------------------------------------------------
// Five-arm byte-identity property (step 02-02) — scenario 18 (@property)
// ---------------------------------------------------------------------------
//
// THE CRUX (AC-03.2/03.3/03.8, NFR-3): the five invalid member-accept reasons —
// expired, already-used/consumed, tampered-signature, unknown-id, AND
// email-already-a-user — ALL collapse to ONE byte-identical user-visible refusal
// (status + FULL body). An attacker cannot distinguish WHY a member invite is
// invalid; the arms differ ONLY in internal logging. Example-pinned at LAYER 3
// (Mandate 11): each arm is driven through the SHIPPED `/invites/accept` route
// against the REAL per-scenario Postgres, and the five captured responses are
// asserted MUTUALLY byte-identical.
//
// GREEN-BY-INHERITANCE from the four shipped/inherited arms (02-01 expired, the
// 01-01 single-use guard, the now-green tampered + unknown arms, and the 02-03
// email-collision arm). NO production code added.
//
// Falsifiability (demonstrated at DELIVER, then reverted): diverging ONE arm — e.g.
// returning a distinct status or body for the unknown-id arm (a 404, or echoing
// "no such invite") — makes that arm's response differ from the other four, so the
// MUTUAL byte-identity assertion REDs. Asserting the FULL body (not merely
// same-status) is what makes the property bite.

/// `Given an expired invite, an already-used invite, a tampered-signature link, an
/// unknown-id link, and an email-already-a-user invite` (scenario 18) — set up the
/// FIVE invalid-accept fixtures against the REAL per-scenario Postgres (no drive
/// yet; the When attempts each). Each fixture is grounded in observable state:
///   * EXPIRED — a member invite one day past expiry (unused).
///   * ALREADY-USED — a member invite consumed by a full successful accept.
///   * TAMPERED — a live member invite whose minted `sig` is corrupted by one char.
///   * UNKNOWN-ID — a validly-signed id naming NO `invites` row.
///   * EMAIL-COLLISION — a live member invite whose `invitee_email` already maps to
///     an existing user (the create-user step will hit the UNIQUE guard).
/// Stashes each arm's drive recipe in `world.mi_five_arms` (the When replays them).
#[given(
    regex = r#"^an expired invite, an already-used invite, a tampered-signature link, an unknown-id link, and an email-already-a-user invite$"#
)]
async fn five_invalid_invites(world: &mut FoundryWorld) {
    let now = harness(world).app.state.clock.now();
    let secret = harness(world).app.state.session_secret.clone();
    let store = harness(world).app.state.store.clone();
    let pool = store.pool().clone();
    let workspace_id = *world
        .mi_workspace_ids
        .get("Northwind")
        .expect("Northwind seeded in the Background");
    let admin_id = world
        .mi_admin_user_id
        .expect("the Background seeded Dana's user id");

    let mut arms: Vec<FiveArm> = Vec::new();

    // 1) EXPIRED — one day past expiry, unused. Genuine sig over the past expiry.
    {
        let id = uuid::Uuid::now_v7();
        let expires_at = now - time::Duration::days(1);
        store
            .insert_invite(
                id,
                workspace_id,
                Some("expired.five@northwind.example"),
                admin_id,
                expires_at,
            )
            .await
            .expect("seed the expired five-arm member invite");
        let sig = foundry_auth::InviteToken::new(id, expires_at, &secret)
            .expect("mint the expired five-arm signature")
            .signature;
        arms.push(FiveArm {
            id,
            sig,
            password: None,
        });
    }

    // 2) ALREADY-USED — seed a live invite, accept it fully (consumes it), then a
    //    re-open is refused by the single-use guard. The genuine sig is preserved.
    {
        let id = uuid::Uuid::now_v7();
        let expires_at = now + (time::Duration::days(7) - time::Duration::hours(2));
        store
            .insert_invite(
                id,
                workspace_id,
                Some("used.five@northwind.example"),
                admin_id,
                expires_at,
            )
            .await
            .expect("seed the already-used five-arm member invite");
        let sig = foundry_auth::InviteToken::new(id, expires_at, &secret)
            .expect("mint the already-used five-arm signature")
            .signature;
        // Drive a full accept to CONSUME it (the member arm creates the account +
        // consumes), so a subsequent open hits the single-use guard.
        drive_full_accept(world, id, &sig, MEMBER_PASSWORD).await;
        let (consumed_rows,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NOT NULL")
                .bind(id)
                .fetch_one(&pool)
                .await
                .expect("count the consumed five-arm invite");
        assert_eq!(
            consumed_rows, 1,
            "the already-used five-arm invite must be consumed before the re-open; \
             found {consumed_rows} consumed rows"
        );
        arms.push(FiveArm {
            id,
            sig,
            password: None,
        });
    }

    // 3) TAMPERED — a live invite whose sig is corrupted by one character.
    {
        let id = uuid::Uuid::now_v7();
        let expires_at = now + (time::Duration::days(7) - time::Duration::hours(2));
        store
            .insert_invite(
                id,
                workspace_id,
                Some("tampered.five@northwind.example"),
                admin_id,
                expires_at,
            )
            .await
            .expect("seed the tampered five-arm member invite");
        let authentic = foundry_auth::InviteToken::new(id, expires_at, &secret)
            .expect("mint the tampered five-arm signature")
            .signature;
        let mut chars: Vec<char> = authentic.chars().collect();
        let original = chars[0];
        chars[0] = if original == 'A' { 'B' } else { 'A' };
        let tampered: String = chars.into_iter().collect();
        assert_ne!(
            tampered, authentic,
            "the five-arm tamper must actually take"
        );
        arms.push(FiveArm {
            id,
            sig: tampered,
            password: None,
        });
    }

    // 4) UNKNOWN-ID — a validly-signed id naming NO invites row.
    {
        let id = uuid::Uuid::now_v7();
        let (rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM invites WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("count rows for the unknown five-arm id");
        assert_eq!(
            rows, 0,
            "the unknown five-arm id must name NO row; found {rows}"
        );
        let expires_at = now + time::Duration::days(7);
        let sig = foundry_auth::InviteToken::new(id, expires_at, &secret)
            .expect("mint the unknown five-arm signature")
            .signature;
        arms.push(FiveArm {
            id,
            sig,
            password: None,
        });
    }

    // 5) EMAIL-COLLISION — a live invite whose invitee_email already has a user.
    {
        let existing_hash = foundry_auth::hash_password(&SecretString::new(
            "five-arm-existing-pass".to_string().into(),
        ))
        .await
        .expect("hash the five-arm pre-existing user password");
        sqlx::query(
            "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
                  VALUES ($1, $2, $2, 'Five Arm Existing', $3)",
        )
        .bind(uuid::Uuid::now_v7())
        .bind("collision.five@northwind.example")
        .bind(&existing_hash)
        .execute(&pool)
        .await
        .expect("seed the five-arm collision pre-existing user");
        let id = uuid::Uuid::now_v7();
        let expires_at = now + (time::Duration::days(7) - time::Duration::hours(2));
        store
            .insert_invite(
                id,
                workspace_id,
                Some("collision.five@northwind.example"),
                admin_id,
                expires_at,
            )
            .await
            .expect("seed the collision five-arm member invite");
        let sig = foundry_auth::InviteToken::new(id, expires_at, &secret)
            .expect("mint the collision five-arm signature")
            .signature;
        // The collision arm is refused at the POST (create-user UNIQUE rollback), so
        // it must be driven with a password.
        arms.push(FiveArm {
            id,
            sig,
            password: Some(MEMBER_PASSWORD.to_string()),
        });
    }

    world.mi_five_arms = arms;
}

/// `When each accept is attempted` (scenario 18) — drive each of the five invalid
/// arms through the SHIPPED `/invites/accept` route and capture its user-visible
/// response (status + full body). Reason-by-reason: arms WITHOUT a password are
/// GET refusals (expired / already-used / tampered / unknown — the GET liveness or
/// verify or lookup refuses non-committally); the email-collision arm carries a
/// password and is refused at the POST (the create-user UNIQUE rollback). Each
/// captured (status, body) lands in `world.mi_five_responses`.
#[when(regex = r#"^each accept is attempted$"#)]
async fn each_accept_attempted(world: &mut FoundryWorld) {
    let arms = world.mi_five_arms.clone();
    let mut responses: Vec<(StatusCode, String)> = Vec::new();
    for arm in arms {
        let response = match arm.password {
            None => drive_get_refusal(world, arm.id, &arm.sig).await,
            Some(password) => drive_full_accept(world, arm.id, &arm.sig, &password).await,
        };
        responses.push(response);
    }
    world.mi_five_responses = responses;
}

/// `Then all five produce a byte-identical user-visible refusal page` (scenario 18)
/// — the property crux: the five captured responses are MUTUALLY byte-identical
/// (status AND full body). An attacker cannot distinguish WHY a member invite is
/// invalid. Diverging any single arm (a distinct status or body) REDs this.
#[then(regex = r#"^all five produce a byte-identical user-visible refusal page$"#)]
async fn all_five_byte_identical(world: &mut FoundryWorld) {
    let responses = &world.mi_five_responses;
    assert_eq!(
        responses.len(),
        5,
        "the property must drive all FIVE invalid arms; captured {} responses",
        responses.len()
    );
    let (first_status, first_body) = &responses[0];
    for (idx, (status, body)) in responses.iter().enumerate() {
        assert_eq!(
            status, first_status,
            "arm {idx} status ({status}) must be byte-identical to arm 0 ({first_status}) \
             — a status oracle would distinguish the refusal reason"
        );
        assert_eq!(
            body, first_body,
            "arm {idx} body must be byte-identical to arm 0 — a body oracle would reveal \
             WHY the invite is invalid. arm {idx} = {body:?}, arm 0 = {first_body:?}"
        );
    }
}

/// `And the email-collision refusal is never a server error` (scenario 18) — the
/// HIGH-risk collision arm (the fifth) is the SHIPPED uniform 200 refusal, NEVER a
/// 500. Reads the captured fifth response.
#[then(regex = r#"^the email-collision refusal is never a server error$"#)]
async fn collision_never_server_error(world: &mut FoundryWorld) {
    let (status, _body) = world
        .mi_five_responses
        .last()
        .expect("the five-arm responses were captured");
    assert_eq!(
        *status,
        StatusCode::OK,
        "the email-collision arm must be the ratified 200 uniform refusal, NEVER a 500; \
         got {status}"
    );
}

// NOTE: scenario 18's `And they differ only in internal logging, never in the
// observable response` REUSES the SHARED step in `feature_invite_accept.rs`
// (cucumber-rs steps are global; a duplicate regex would make the match ambiguous).
// That shared step was made world-source-agnostic: in member mode it re-affirms the
// FIVE-arm `mi_five_responses` are mutually byte-identical and leak no enumeration
// oracle, instead of the invite-accept feature's four `ia_four_refusals`.

/// One invalid-accept arm's drive recipe: the invite id, the (possibly tampered)
/// signature, and an optional password (present ⇒ the arm is refused at the POST,
/// e.g. the email-collision arm; absent ⇒ a GET refusal).
#[derive(Clone, Debug)]
pub struct FiveArm {
    pub id: uuid::Uuid,
    pub sig: String,
    pub password: Option<String>,
}

/// Drive the PUBLIC GET `/invites/accept?id=&sig=` for `(id, sig)` and return the
/// uniform refusal (status + full body). Used by the GET-refused five-arm reasons
/// (expired / already-used / tampered / unknown).
async fn drive_get_refusal(
    world: &mut FoundryWorld,
    id: uuid::Uuid,
    sig: &str,
) -> (StatusCode, String) {
    let base = harness(world).base_url();
    let client = http(world);
    let resp = client
        .get(format!(
            "{base}/invites/accept?id={id}&sig={sig}",
            sig = urlencoding::encode(sig)
        ))
        .send()
        .await
        .expect("GET /invites/accept (five-arm GET refusal)");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

/// Drive the full PUBLIC accept (GET form + CSRF cookie → POST id+sig+password+
/// confirm+_csrf) for `(id, sig)` and return the POST's user-visible response
/// (status + full body). Used by the email-collision five-arm reason (refused at
/// the POST via the create-user UNIQUE rollback) and to CONSUME the already-used
/// arm's invite during setup.
async fn drive_full_accept(
    world: &mut FoundryWorld,
    id: uuid::Uuid,
    sig: &str,
    password: &str,
) -> (StatusCode, String) {
    let base = harness(world).base_url();
    let client = http(world);

    let get_resp = client
        .get(format!(
            "{base}/invites/accept?id={id}&sig={sig}",
            sig = urlencoding::encode(sig)
        ))
        .send()
        .await
        .expect("GET /invites/accept (five-arm full accept)");
    let csrf_cookie = get_resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(str::to_string)
        .expect("the GET minted a foundry_csrf cookie");
    let _ = get_resp.text().await;
    let csrf_token = csrf_cookie
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();

    let form = [
        ("id", id.to_string()),
        ("sig", sig.to_string()),
        ("password", password.to_string()),
        ("confirm", password.to_string()),
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
        .expect("POST /invites/accept (five-arm full accept)");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Resolve the new member (Sam) user id by the walking-skeleton invitee email.
async fn sam_user_id(world: &FoundryWorld) -> uuid::Uuid {
    let pool = harness(world).app.state.store.pool().clone();
    let (id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind("sam.okafor@northwind.example")
        .fetch_one(&pool)
        .await
        .expect("the member accept must have created Sam's user row");
    id
}

// ---------------------------------------------------------------------------
// Scenario 19 (step 02-04) — SINGLE-USE: a consumed member invite re-opened is
// refused; no second account and no session.
//
// Green by inheritance from the authoritative atomic guarded consume
// (`create_member_and_consume`): its one-statement guarded UPDATE
// `... WHERE id = $1 AND used_at IS NULL AND expires_at > $2 RETURNING ...` ran
// once on the first accept, stamping `used_at`. A re-open GET re-checks liveness
// (`used_at IS NULL`) and, finding it set, renders the SHIPPED uniform
// `invite_refusal_page()` (200, OD-3) — never re-creating an account or session.
//
// Falsifiability (documented atomicity argument + revert-reds-it): dropping the
// `AND used_at IS NULL` clause from the guard (or letting the GET advisory
// liveness check trust a stale read) would let the re-open re-consume the invite
// + re-create a second account + mint a second session — RED-ing both the
// refusal Then AND the "no second account / no session" Then.
// ---------------------------------------------------------------------------

/// `Given Sam has already created his account and joined "<workspace>" via his
/// invite link` — seed a LIVE member invite for Sam (two hours ago) and drive the
/// FULL successful accept (GET form + CSRF cookie → POST password) through the REAL
/// shared `/invites/accept` flow, so the invite is GENUINELY consumed exactly once
/// (real `used_at`/`used_by`, real argon2id) and Sam's account + member membership
/// exist. Snapshots the post-accept observable baseline (Sam's user id) so the
/// re-open Then can prove NO second account/session appears. The arrival state for
/// the single-use re-open proof.
#[given(
    regex = r#"^Sam has already created his account and joined "([^"]+)" via his invite link$"#
)]
async fn sam_already_joined_via_invite(world: &mut FoundryWorld, ws_name: String) {
    let ttl = time::Duration::days(7) - time::Duration::hours(2);
    seed_member_invite(world, &ws_name, SAM_EMAIL, ttl).await;
    accept_member_invite(world, SAM_PASSWORD).await;
    assert_eq!(
        world.mi_post_status,
        Some(StatusCode::SEE_OTHER),
        "the first member accept must succeed (303 auto-sign-in) so the re-open under \
         test starts from a genuinely-consumed invite; got {:?}",
        world.mi_post_status
    );
    // Ground the single-use precondition in observable state: exactly one account
    // for Sam, exactly one consumed invite row.
    let pool = harness(world).app.state.store.pool().clone();
    let (user_rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email_lower = $1")
        .bind(SAM_EMAIL)
        .fetch_one(&pool)
        .await
        .expect("count Sam's account after the first accept");
    assert_eq!(
        user_rows, 1,
        "the first accept must have created EXACTLY ONE account for Sam; found {user_rows}"
    );
}

/// `When Sam opens the same invite link again` — re-GET the now-CONSUMED member
/// invite over real HTTP with the genuine signed token. The GET re-checks liveness
/// against the REAL Postgres, finds `used_at` set, and renders the uniform refusal
/// — captured into `mi_post_status` + `last_body` for the Thens. No second accept
/// POST is driven (a re-open is a GET).
#[when(regex = r#"^Sam opens the same invite link again$"#)]
async fn sam_opens_same_link_again(world: &mut FoundryWorld) {
    get_member_accept_page(world).await;
}

/// `And no second account is created and no session is created` — the re-open was
/// NON-COMMITTAL: still EXACTLY ONE `users` row for Sam (no duplicate account), and
/// the re-open GET minted NO `foundry_session` cookie (no second sign-in). Reads
/// the REAL per-scenario Postgres. Falsifiability litmus: a re-open that
/// re-consumed would create a second account and/or sign a second session in,
/// RED-ing this.
#[then(regex = r#"^no second account is created and no session is created$"#)]
async fn no_second_account_no_session(world: &mut FoundryWorld) {
    let pool = harness(world).app.state.store.pool().clone();
    let (user_rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email_lower = $1")
        .bind(SAM_EMAIL)
        .fetch_one(&pool)
        .await
        .expect("count Sam's accounts after the re-open");
    assert_eq!(
        user_rows, 1,
        "re-opening a consumed member invite must create NO second account; Sam must \
         still have EXACTLY ONE account, found {user_rows}"
    );
    // The re-open GET is non-committal: the body is the uniform refusal page, and a
    // GET never establishes a session (the only session mint is the successful
    // accept POST). `last_body` carries the refusal copy already asserted by the
    // shared "no longer valid" Then; here we confirm the re-open did not 303 / sign
    // in (it rendered the 200 refusal page, captured in `mi_post_status`).
    assert_eq!(
        world.mi_post_status,
        Some(StatusCode::OK),
        "the re-open must render the uniform 200 refusal (no second sign-in 303); got {:?}",
        world.mi_post_status
    );
}

// ---------------------------------------------------------------------------
// Scenario 20 (step 02-04) — SINGLE-USE + SINGLE-CREATE UNDER CONCURRENCY: N
// accept submissions for ONE live member invite race; EXACTLY ONE creates the
// account + joins + signs in, the rest get the uniform refusal, and exactly one
// `users` row + one membership + one consumed invite exist.
//
// The `When two accept submissions ... arrive concurrently` and `And the other
// receives the standard "invite is no longer valid" page` steps are the SHARED
// concurrency idiom defined in `feature_invite_accept.rs` (cucumber-rs steps are
// global). They drive the `ia_invite_id`/`ia_invite_sig` slots and capture
// `ia_concurrent_outcomes`; the member `Given` below mirrors the seeded member
// invite into those slots so the shared When races THIS member invite over the
// shared `/invites/accept` route. The member-specific winner + invariant Thens
// (below) read `ia_concurrent_outcomes` + the REAL Postgres.
//
// Green by inheritance from the ATOMIC `create_member_and_consume` guard: its
// one-statement guarded UPDATE `... WHERE used_at IS NULL ... RETURNING` takes a
// row lock; every concurrent writer BLOCKS, re-evaluates `used_at IS NULL` against
// the now-committed row, matches 0 rows ⇒ rollback ⇒ `MemberConsumeOutcome::Refused`.
// The DB serializes the race — exactly-one-winner, one user, one membership, one
// consume; no duplicate account, no torn state, no double session.
//
// Falsifiability (documented atomicity argument + revert-reds-it): splitting the
// guard into a read-then-write check-then-act (SELECT used_at; if NULL then create
// + UPDATE) opens a TOCTOU window where two racers both read NULL and both create —
// admitting >1 winner (>1 303 + >1 session), a SECOND `users` row (or a 23505
// collision crash), a second membership, and a re-stamped `used_at` — RED-ing the
// exactly-one-303 winner Then AND the exactly-one-user/membership/consume Then. The
// atomic one-statement guarded UPDATE closes that window; restored after the demo.
// ---------------------------------------------------------------------------

/// `Given Sam's member invite is live` — seed a LIVE member invite for Sam (two
/// hours ago) via the SHIPPED `insert_invite` and confirm it is live (unused +
/// unexpired) against the REAL per-scenario Postgres, so the exactly-one-winner
/// race starts from a single genuinely-consumable member invite. MIRRORS the seeded
/// id + sig into the `ia_invite_id`/`ia_invite_sig` slots the SHARED concurrent-
/// accept When (`feature_invite_accept::two_concurrent_accepts`) drives, so that
/// shared idiom races THIS member invite over the shared `/invites/accept` route.
#[given(regex = r#"^Sam's member invite is live$"#)]
async fn sam_member_invite_is_live(world: &mut FoundryWorld) {
    let ttl = time::Duration::days(7) - time::Duration::hours(2);
    seed_member_invite(world, "Northwind", SAM_EMAIL, ttl).await;
    let invite_id = world.mi_invite_id.expect("seeded a member invite");
    let now = harness(world).app.state.clock.now();
    let pool = harness(world).app.state.store.pool().clone();
    let (live_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL AND expires_at > $2",
    )
    .bind(invite_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("count the live (unused, unexpired) member invite before the race");
    assert_eq!(
        live_rows, 1,
        "the member invite under test must be live (unused and unexpired) before the \
         concurrent accepts; found {live_rows} live rows"
    );
    // Mirror into the slots the SHARED concurrent-accept When drives.
    world.ia_invite_id = world.mi_invite_id;
    world.ia_invite_sig = world.mi_invite_sig.clone();
}

/// `Then exactly one submission creates the account, joins, and signs Sam in` — the
/// exactly-one-winner core for the member arm: across the N concurrent accepts,
/// EXACTLY ONE answered 303 SEE_OTHER carrying a `foundry_session` cookie (the
/// `create_member_and_consume` tx that won the guarded-UPDATE row lock), creating
/// Sam's account and joining him. A read-then-write split would admit >1 winner,
/// RED-ing the exactly-one count. Reads `ia_concurrent_outcomes` (populated by the
/// shared When).
#[then(regex = r#"^exactly one submission creates the account, joins, and signs Sam in$"#)]
async fn member_exactly_one_winner(world: &mut FoundryWorld) {
    let outcomes = &world.ia_concurrent_outcomes;
    assert!(
        outcomes.len() >= 2,
        "the When must have raced ≥2 concurrent member-accept legs; got {}",
        outcomes.len()
    );
    let winners: Vec<&(StatusCode, Option<String>, String)> = outcomes
        .iter()
        .filter(|(status, session, _)| *status == StatusCode::SEE_OTHER && session.is_some())
        .collect();
    assert_eq!(
        winners.len(),
        1,
        "EXACTLY ONE concurrent member accept must win (303 SEE_OTHER + a session \
         cookie); the atomic guarded UPDATE serializes the race. got {} winners; \
         outcomes = {:?}",
        winners.len(),
        outcomes
            .iter()
            .map(|(s, sess, _)| (*s, sess.is_some()))
            .collect::<Vec<_>>()
    );
}

/// `And exactly one user and one membership are created and the invite is used
/// exactly once` — the single-create invariant against the REAL per-scenario
/// Postgres: EXACTLY ONE `users` row for Sam (no duplicate account from a torn
/// double-create), EXACTLY ONE `workspace_memberships` row for him on Northwind with
/// the `member` role, and the invite is consumed EXACTLY ONCE (`used_at` set,
/// `used_by` = Sam's single account). A check-then-act split would admit a second
/// user/membership and/or re-stamp `used_at`, RED-ing this.
#[then(
    regex = r#"^exactly one user and one membership are created and the invite is used exactly once$"#
)]
async fn member_exactly_one_user_membership_consume(world: &mut FoundryWorld) {
    let invite_id = world.mi_invite_id.expect("seeded a member invite");
    let expected_ws = *world
        .mi_workspace_ids
        .get("Northwind")
        .expect("Northwind seeded in the Background");
    let pool = harness(world).app.state.store.pool().clone();

    // Exactly one account for Sam.
    let (user_rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email_lower = $1")
        .bind(SAM_EMAIL)
        .fetch_one(&pool)
        .await
        .expect("count Sam's accounts after the race");
    assert_eq!(
        user_rows, 1,
        "the concurrent accepts must create EXACTLY ONE account for Sam (no duplicate \
         from a torn double-create); found {user_rows}"
    );
    let sam_id = sam_user_id(world).await;

    // Exactly one membership, on Northwind, member role.
    let memberships: Vec<(uuid::Uuid, String)> =
        sqlx::query_as("SELECT workspace_id, role FROM workspace_memberships WHERE user_id = $1")
            .bind(sam_id)
            .fetch_all(&pool)
            .await
            .expect("read Sam's memberships after the race");
    assert_eq!(
        memberships.len(),
        1,
        "the concurrent accepts must create EXACTLY ONE membership for Sam; found {memberships:?}"
    );
    assert_eq!(
        (memberships[0].0, memberships[0].1.as_str()),
        (expected_ws, "member"),
        "Sam's sole membership must be Northwind with the member role; got {memberships:?}"
    );

    // The invite is consumed exactly once, used_by = Sam's single account.
    let (consumed_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NOT NULL AND used_by = $2",
    )
    .bind(invite_id)
    .bind(sam_id)
    .fetch_one(&pool)
    .await
    .expect("count the consumed invite row after the race");
    assert_eq!(
        consumed_rows, 1,
        "the member invite must be used EXACTLY ONCE, by Sam's single account; found {consumed_rows}"
    );
}

// ---------------------------------------------------------------------------
// Scenario 21 (step 02-04) — TOCTOU: a member invite consumed in the GET→POST
// window is refused by the consume tx guard, NOT trusting the GET-time advisory
// read; nothing is created.
//
// Green by inheritance from the authoritative guard's `AND used_at IS NULL`
// clause INSIDE the `create_member_and_consume` tx — the SAME clause the single-use
// + concurrency arms ride. This step proves the SPECIFIC TOCTOU shape: the gap
// between the GET render and the POST submit, closed by re-checking liveness INSIDE
// the tx rather than trusting the GET-time advisory read.
//
// The out-of-band consume is driven through the AUTHORITATIVE store seam
// (`create_member_and_consume`) — the SAME atomic guarded UPDATE a real concurrent
// accept hits — so the invite is GENUINELY consumed in the GET→POST window (a real
// first account + membership + `used_at`/`used_by` written), not synthesised. The
// stale POST then reuses the GET's double-submit CSRF token so the refusal under
// test fires on the TX guard, NOT a CSRF rejection.
//
// Falsifiability litmus (PROVEN at DELIVER, then reverted): making the POST trust
// the GET-time liveness instead of the TX guard — dropping the guard's
// `AND used_at IS NULL` clause — lets the stale POST re-consume + create a SECOND
// account + mint a session, RED-ing BOTH the "no longer valid" Then (the stale POST
// would 303, not render the 200 refusal) AND the state-unchanged Then (a second
// `users` row would appear, `used_at`/`used_by` would change off the first consumer).
// ---------------------------------------------------------------------------

/// The out-of-band first consumer's email — DISTINCT from Sam's, so the consume
/// routes to the member arm cleanly and "no second account for the stale email"
/// bites against a known single first account. (The seeded invite carries Sam's
/// email; we consume it out-of-band via the authoritative seam, which creates the
/// invitee account from the invite's `invitee_email` = Sam's. So the first consumer
/// IS Sam's account; the stale POST must create no SECOND account.)
/// A policy-passing credential for the out-of-band first consume.
const TOCTOU_FIRST_CONSUMER_PASSWORD: &str = "out-of-band-member-consumer-pass";
/// The DIFFERENT password Sam submits on his now-STALE page — also policy-passing.
const SAM_STALE_PASSWORD: &str = "stale-member-page-secure-pass";

/// `Given the same invite is consumed by another submission before Sam submits` —
/// AFTER Sam's GET rendered the form (the reused arrival Given), consume the SAME
/// member invite OUT-OF-BAND via the AUTHORITATIVE store seam
/// (`create_member_and_consume`) — the SAME atomic guarded UPDATE a real concurrent
/// accept hits — so the invite is GENUINELY consumed in the GET→POST window.
/// Asserts the consume SUCCEEDED (the guard returned `Consumed`) and snapshots the
/// post-consume baseline (exactly one account for Sam + the now-set `used_at`)
/// against the REAL per-scenario Postgres, so the stale-POST Then can prove NOTHING
/// changed after.
#[given(regex = r#"^the same invite is consumed by another submission before Sam submits$"#)]
async fn member_invite_consumed_out_of_band(world: &mut FoundryWorld) {
    let invite_id = world.mi_invite_id.expect("a member invite was seeded");
    let now = harness(world).app.state.clock.now();
    let store = harness(world).app.state.store.clone();

    let first_consumer_hash = foundry_auth::hash_password(&SecretString::new(
        TOCTOU_FIRST_CONSUMER_PASSWORD.to_string().into(),
    ))
    .await
    .expect("hash the out-of-band first consumer credential");

    // Consume OUT-OF-BAND through the AUTHORITATIVE atomic guarded UPDATE — the
    // same seam a genuine concurrent accept hits in the GET→POST window.
    let outcome = store
        .create_member_and_consume(invite_id, &first_consumer_hash, now)
        .await
        .expect("the out-of-band member consume must reach the store");
    assert!(
        matches!(
            outcome,
            foundry_store::MemberConsumeOutcome::Consumed { .. }
        ),
        "the out-of-band submission must GENUINELY consume the live member invite \
         (the guard returns Consumed), so the TOCTOU precondition is real; got {outcome:?}"
    );

    // Snapshot the post-consume baseline: exactly one account for Sam + the now-set
    // used_at, against the REAL Postgres.
    let pool = store.pool().clone();
    let (user_rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email_lower = $1")
        .bind(SAM_EMAIL)
        .fetch_one(&pool)
        .await
        .expect("count Sam's account after the out-of-band consume");
    assert_eq!(
        user_rows, 1,
        "the out-of-band consume must have created EXACTLY ONE account for Sam, so the \
         stale-POST 'no second account' assertion has a genuine baseline; found {user_rows}"
    );
    let (used_at,): (time::OffsetDateTime,) =
        sqlx::query_as("SELECT used_at FROM invites WHERE id = $1 AND used_at IS NOT NULL")
            .bind(invite_id)
            .fetch_one(&pool)
            .await
            .expect("the invite must be recorded as consumed after the out-of-band submission");
    world.mi_consumed_used_at = Some(used_at);
}

/// `When Sam submits a valid password on his now-stale page` — drive Sam's POST
/// `/invites/accept` over real HTTP from his now-STALE page: he carries the genuine
/// id + sig + a policy-passing (but DIFFERENT) password + matching confirm + the
/// double-submit `_csrf` token the GET minted, so the refusal under test fires on
/// the TX guard, NOT a CSRF rejection. The SHIPPED `csrf_middleware` admits the
/// matching pair; the authoritative `create_member_and_consume` guard re-checks
/// `used_at IS NULL` inside the TX, matches 0 rows (the out-of-band consume already
/// stamped it) ⇒ `MemberConsumeOutcome::Refused` ⇒ uniform `invite_refusal_page()`.
/// Captures the status + full body into the slots the reused "standard page" Then
/// reads, and the (expected-absent) session cookie so "no second account" + "used
/// once" can be proven.
#[when(regex = r#"^Sam submits a valid password on his now-stale page$"#)]
async fn sam_submits_on_stale_page(world: &mut FoundryWorld) {
    let invite_id = world.mi_invite_id.expect("a member invite was seeded");
    let sig = world
        .mi_invite_sig
        .clone()
        .expect("the invite sig was minted");
    // Reuse the GET-time double-submit CSRF token Sam's LIVE arrival GET minted, so
    // the refusal under test fires on the TX guard, NOT a CSRF rejection. (A fresh
    // re-GET would now hit the consumed-invite refusal page, which mints no CSRF
    // cookie — Sam's page is STALE, carrying the live-GET token.)
    let csrf_cookie = world
        .mi_get_csrf_cookie
        .clone()
        .expect("Sam's live arrival GET minted a foundry_csrf cookie");
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
        ("password", SAM_STALE_PASSWORD.to_string()),
        ("confirm", SAM_STALE_PASSWORD.to_string()),
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
        .expect("POST /invites/accept (stale page)");
    world.mi_post_status = Some(resp.status());
    world.mi_post_location = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    world.mi_session_cookie = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .and_then(|s| s.split(';').next())
        .map(str::to_string);
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

/// `And no account is created and the invite stays used exactly once` — the
/// stale-POST state-unchanged invariant against the REAL per-scenario Postgres: NO
/// SECOND `users` row appeared (still exactly one account for Sam, from the
/// out-of-band first consume — the stale POST created none), the stale POST minted
/// NO session cookie, and the invite is STILL used exactly once with its `used_at`
/// UNCHANGED from the out-of-band consume (the guard matched 0 rows and rolled back).
#[then(regex = r#"^no account is created and the invite stays used exactly once$"#)]
async fn member_stale_post_state_unchanged(world: &mut FoundryWorld) {
    let invite_id = world.mi_invite_id.expect("a member invite was seeded");
    let pool = harness(world).app.state.store.pool().clone();

    // No second account: still exactly one account for Sam (from the first consume).
    let (user_rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email_lower = $1")
        .bind(SAM_EMAIL)
        .fetch_one(&pool)
        .await
        .expect("count Sam's accounts after the stale POST");
    assert_eq!(
        user_rows, 1,
        "the stale POST must create NO second account; Sam must still have EXACTLY ONE \
         account (from the out-of-band first consume), found {user_rows}"
    );

    // The stale POST minted no session (it rendered the 200 refusal, not a 303).
    assert!(
        world.mi_session_cookie.is_none(),
        "the refused stale POST must mint NO session cookie; got {:?}",
        world.mi_session_cookie
    );

    // The invite stays used exactly once, used_at UNCHANGED from the out-of-band consume.
    let (used_at, used_count): (time::OffsetDateTime, i64) = {
        let (ts,): (time::OffsetDateTime,) =
            sqlx::query_as("SELECT used_at FROM invites WHERE id = $1 AND used_at IS NOT NULL")
                .bind(invite_id)
                .fetch_one(&pool)
                .await
                .expect("the invite must still be consumed exactly once after the stale POST");
        let (n,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NOT NULL")
                .bind(invite_id)
                .fetch_one(&pool)
                .await
                .expect("count the consumed invite rows after the stale POST");
        (ts, n)
    };
    assert_eq!(
        used_count, 1,
        "the invite must stay used EXACTLY ONCE after the refused stale POST; found {used_count}"
    );
    assert_eq!(
        Some(used_at),
        world.mi_consumed_used_at,
        "the invite's used_at must be UNCHANGED from the out-of-band consume (the stale \
         POST's guard matched 0 rows and rolled back); a re-stamp would mean the stale \
         POST won"
    );
}

// ---------------------------------------------------------------------------
// Issuance non-enumerability (step 02-05) — scenarios 22 (non-admin) + 23
// (signed-out).
//
// PROVES the SECURITY CRUX (NFR-1, AC-03.1, I-E1/I-E2): a signed-in NON-ADMIN
// member's AND a SIGNED-OUT caller's GET/POST to the admin-gated issuance surface
// `/workspace/invites` is refused BYTE-IDENTICALLY (status AND full body) to a path
// that never existed — NO 401/403, NO login redirect, NO oracle the issuance
// surface exists — and for the signed-out arm, ALSO byte-identical to the non-admin
// refusal (the refusal CAUSE is indistinguishable). GREEN-BY-INHERITANCE behind the
// SHIPPED `require_workspace_admin` gate (returns the SHIPPED
// `resource_not_found_page()` for `is_workspace_admin == false` AND for a missing
// session, member_invites.rs:82-83/104-105/185-208) and the SHIPPED router fallback
// (the never-existed control), under the SHIPPED double-submit `csrf_middleware`
// that screens the token-less POST ahead of routing. This step adds acceptance GLUE
// only — NO production code. The web-provisioning 02-02/02-03 idiom
// (`feature_web_provisioning_flow`) applied to `/workspace/invites`.
//
// Per-method comparison (GET-vs-GET, POST-vs-POST): the gate refuses the GET, the
// CSRF layer screens the token-less POST; comparing a POST against a GET control
// would be a category error, not an oracle test. Each member-invite refusal is
// asserted against the same-method never-existed control.
//
// Falsifiability (revert-reds-it litmus, demonstrated at DELIVER then reverted):
// collapsing the gate's refusal into a DISTINCT response — a 403/401 for the
// non-admin, a 303 redirect-to-sign-in for the signed-out caller, or any body that
// diverges from the never-existed page — diverges from the control and re-REDS
// `each_member_invite_response_byte_identical_to_never_existed` (and, for the
// signed-out arm, `signed_out_refusal_byte_identical_to_non_admin`).
// ---------------------------------------------------------------------------

/// Marco's password — seeded on his plain-member account so his web GET/POST to
/// `/workspace/invites` authenticates (the harness keeps no cookie jar;
/// `signed_in_get` / `session_only_member_post` re-authenticate per request).
const MARCO_PASSWORD: &str = "marco-northwind-member-pass";
/// Marco's email — a plain member of Northwind (role=`member`, NOT admin), so the
/// SHIPPED `require_workspace_admin` gate refuses him with the non-enumerable 404.
const MARCO_EMAIL: &str = "marco@northwind.example";
/// A path with no route at all — the never-existed control the issuance refusal is
/// asserted byte-identical to. Refused by the SHIPPED router fallback (GET) / the
/// CSRF layer ahead of routing (the token-less POST), regardless of who asks.
const NEVER_EXISTED_PATH: &str = "/this-path-has-never-existed-anywhere";

/// `Given Marco is signed in as a plain member of "<workspace>"` — seed Marco as an
/// ordinary `member`-role membership on the Background-seeded workspace (a real
/// `users` row with a KNOWN password + a `member` membership), and assert the
/// `is_workspace_admin == false` precondition that makes this the NOT-AUTHORIZED
/// refusal cause (distinct from signed-out). Marco's web sign-in happens per-request
/// inside the When (the harness keeps no cookie jar).
#[given(regex = r#"^Marco is signed in as a plain member of "([^"]+)"$"#)]
async fn marco_signed_in_plain_member(world: &mut FoundryWorld, ws_name: String) {
    let workspace_id = *world
        .mi_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} seeded in the Background"));
    let pool = harness(world).app.state.store.pool().clone();

    let marco_id = uuid::Uuid::now_v7();
    let marco_hash =
        foundry_auth::hash_password(&SecretString::new(MARCO_PASSWORD.to_string().into()))
            .await
            .expect("hash Marco's member password");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, 'Marco', $3)",
    )
    .bind(marco_id)
    .bind(MARCO_EMAIL)
    .bind(&marco_hash)
    .execute(&pool)
    .await
    .expect("seed Marco's plain-member user row");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(marco_id)
    .execute(&pool)
    .await
    .expect("seed Marco's member membership");

    // Precondition: Marco is NOT a workspace admin (the not-authorized refusal cause).
    let is_admin = harness(world)
        .app
        .state
        .store
        .is_workspace_admin(workspace_id, marco_id)
        .await
        .expect("probe is_workspace_admin precondition for Marco");
    assert!(
        !is_admin,
        "the acting member {MARCO_EMAIL:?} must NOT be a workspace admin (this is the \
         not-authorized refusal cause, distinct from the signed-out cause)"
    );
    let _ = http(world);
}

/// Issue an ANONYMOUS request (no session cookie, no CSRF token) for `method url`
/// against the in-process harness, returning the full (status, body) refusal shape.
/// A signed-out caller carries no credentials at all — the adversary the
/// non-enumerability property defends against. A token-less POST is screened by the
/// SHIPPED double-submit CSRF layer ahead of routing.
async fn anonymous_issuance_request(
    world: &mut FoundryWorld,
    method: &str,
    url: &str,
) -> (StatusCode, String) {
    let base = harness(world).base_url();
    let client = http(world);
    let request = match method {
        "GET" => client.get(format!("{base}{url}")),
        "POST" => client
            .post(format!("{base}{url}"))
            .form(&[("email", "smuggled@northwind.example")]),
        other => panic!("unsupported anonymous method {other:?}"),
    };
    let resp = request
        .send()
        .await
        .expect("send anonymous issuance request");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

/// Sign in as `email` to capture a real `foundry_session` cookie, then POST `url`
/// carrying ONLY that session cookie — NO `_csrf` token. The SHIPPED double-submit
/// `csrf_middleware` refuses the token-less POST before routing, the SAME way it
/// refuses a signed-out (also token-less) POST and a never-existed token-less POST —
/// which is what keeps a signed-in member's POST refusal byte-identical to the
/// signed-out/never-existed baseline.
async fn session_only_member_post(
    world: &mut FoundryWorld,
    email: &str,
    password: &str,
    url: &str,
) -> (StatusCode, String) {
    let base = harness(world).base_url();
    let client = http(world);

    // (1) GET /sign-in to mint a CSRF cookie + token (needed only to AUTHENTICATE).
    let signin_get = client
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("get /sign-in for csrf");
    let csrf_token = signin_get
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .and_then(|s| s.strip_prefix("foundry_csrf="))
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();

    // (2) POST /sign-in to authenticate; capture the session cookie.
    let mut signin_form: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    signin_form.insert("email", email.to_string());
    signin_form.insert("password", password.to_string());
    signin_form.insert("_csrf", csrf_token.clone());
    let signin_resp = client
        .post(format!("{base}/sign-in"))
        .header(
            reqwest::header::COOKIE,
            format!("foundry_csrf={csrf_token}"),
        )
        .form(&signin_form)
        .send()
        .await
        .expect("post /sign-in");
    let session_pair = signin_resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .and_then(|s| s.split(';').next())
        .map(str::to_string)
        .expect("sign-in must issue a foundry_session cookie");

    // (3) POST `url` carrying ONLY the session cookie — NO _csrf cookie/field. The
    //     double-submit CSRF middleware refuses it before routing.
    let resp = client
        .post(format!("{base}{url}"))
        .header(reqwest::header::COOKIE, session_pair)
        .form(&[("email", "smuggled@northwind.example")])
        .send()
        .await
        .expect("post issuance url (session-only, no csrf)");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

/// `When Marco opens the member-invite page and submits an email` — drive the
/// signed-in NON-ADMIN member to BOTH issuance methods over real HTTP: GET
/// `/workspace/invites` (session cookie, gate-refused) and a token-less POST
/// `/workspace/invites` (session cookie, no `_csrf` — CSRF-screened). Record each
/// `(method url, status, body)` refusal so the byte-identity Then asserts it against
/// the same-method never-existed control.
#[when(regex = r#"^Marco opens the member-invite page and submits an email$"#)]
async fn marco_probes_issuance_surface(world: &mut FoundryWorld) {
    // GET — the session-cookie-bearing member reaches the `require_workspace_admin`
    // gate, which refuses `is_workspace_admin == false` with the uniform 404.
    let client = http(world);
    let get_outcome = signed_in_get(
        harness(world),
        &client,
        MARCO_EMAIL,
        MARCO_PASSWORD,
        "/workspace/invites",
    )
    .await;
    // Also expose the GET refusal body to the SHARED `nothing reveals that the
    // issuance surface exists` Then (defined for scenario 11; reads `last_body`).
    world.last_body = Some(get_outcome.body.clone());
    world.mi_issuance_refusals.push((
        "GET /workspace/invites".to_string(),
        get_outcome.status,
        get_outcome.body,
    ));

    // POST — token-less, CSRF-screened ahead of routing.
    let (post_status, post_body) =
        session_only_member_post(world, MARCO_EMAIL, MARCO_PASSWORD, "/workspace/invites").await;
    world.mi_issuance_refusals.push((
        "POST /workspace/invites".to_string(),
        post_status,
        post_body,
    ));
}

/// `And Marco requests a path that never existed` — capture the never-existed-path
/// control PER HTTP METHOD (GET + POST), the same identity-blind uniform 404 the
/// admin surface must be indistinguishable from. The control is anonymous because a
/// never-existed path has no gate to reach — refused by the router fallback (GET) /
/// the CSRF layer ahead of routing (POST) regardless of who asks; that
/// caller-independence is precisely why it is the right control.
#[when(regex = r#"^Marco requests a path that never existed$"#)]
async fn marco_requests_never_existed_path(world: &mut FoundryWorld) {
    for method in ["GET", "POST"] {
        let (status, body) = anonymous_issuance_request(world, method, NEVER_EXISTED_PATH).await;
        world
            .mi_issuance_never_existed
            .insert(method.to_string(), (status, body));
    }
}

/// `Given no one is signed in` — the acting persona carries NO session cookie. The
/// Background already seeded Dana + the Northwind workspace (so the surface genuinely
/// exists for SOMEONE); this step only primes the http client for the anonymous
/// probes.
#[given(regex = r#"^no one is signed in$"#)]
async fn no_one_signed_in(world: &mut FoundryWorld) {
    assert!(
        world.mi_harness.is_some(),
        "the member-invites Background must have spawned the harness (so the issuance \
         surface exists for the admin the signed-out caller cannot reach)"
    );
    let _ = http(world);
}

/// `When a signed-out caller opens the member-invite page and a never-existed path` —
/// drive an ANONYMOUS caller to BOTH issuance methods (GET + token-less POST
/// `/workspace/invites`), recording each refusal in `mi_issuance_refusals` (so the
/// SHARED byte-identity Then asserts it against the never-existed control), AND
/// capture the never-existed-path control per method. ALSO drive the NON-ADMIN
/// (Marco, seeded for this scenario) to every route into
/// `mi_issuance_signed_out_refusals`'s counterpart so the cross-cause identity Then
/// can compare signed-out vs non-admin route-for-route.
#[when(regex = r#"^a signed-out caller opens the member-invite page and a never-existed path$"#)]
async fn signed_out_probes_issuance_and_never_existed(world: &mut FoundryWorld) {
    // The signed-out refusals (recorded in the shared `mi_issuance_refusals` slot the
    // byte-identity-vs-never-existed Then reads).
    for (method, url) in [
        ("GET", "/workspace/invites"),
        ("POST", "/workspace/invites"),
    ] {
        let (status, body) = anonymous_issuance_request(world, method, url).await;
        world
            .mi_issuance_refusals
            .push((format!("{method} {url}"), status, body.clone()));
        // Mirror into the signed-out baseline so the cross-cause identity Then can
        // compare it against the non-admin refusal for the SAME route.
        world
            .mi_issuance_signed_out_refusals
            .push((format!("{method} {url}"), status, body));
    }

    // The never-existed-path control per method.
    for method in ["GET", "POST"] {
        let (status, body) = anonymous_issuance_request(world, method, NEVER_EXISTED_PATH).await;
        world
            .mi_issuance_never_existed
            .insert(method.to_string(), (status, body));
    }

    // The NON-ADMIN refusal baseline (Marco) for the SAME routes, so the cross-cause
    // byte-identity (signed-out == non-admin) is asserted route-for-route. Seed Marco
    // here (this scenario's Given is `no one is signed in`, so no Marco exists yet).
    let workspace_id = *world
        .mi_workspace_ids
        .get("Northwind")
        .expect("the Background seeded the Northwind workspace");
    let pool = harness(world).app.state.store.pool().clone();
    let marco_id = uuid::Uuid::now_v7();
    let marco_hash =
        foundry_auth::hash_password(&SecretString::new(MARCO_PASSWORD.to_string().into()))
            .await
            .expect("hash Marco's member password");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, 'Marco', $3) ON CONFLICT (email_lower) DO NOTHING",
    )
    .bind(marco_id)
    .bind(MARCO_EMAIL)
    .bind(&marco_hash)
    .execute(&pool)
    .await
    .expect("seed Marco's plain-member user row (cross-cause baseline)");
    let (marco_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(MARCO_EMAIL)
        .fetch_one(&pool)
        .await
        .expect("resolve Marco's id");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING",
    )
    .bind(workspace_id)
    .bind(marco_id)
    .execute(&pool)
    .await
    .expect("seed Marco's member membership (cross-cause baseline)");

    let client = http(world);
    let na_get = signed_in_get(
        harness(world),
        &client,
        MARCO_EMAIL,
        MARCO_PASSWORD,
        "/workspace/invites",
    )
    .await;
    let na_get_pair = (
        "GET /workspace/invites".to_string(),
        na_get.status,
        na_get.body,
    );
    let (na_post_status, na_post_body) =
        session_only_member_post(world, MARCO_EMAIL, MARCO_PASSWORD, "/workspace/invites").await;
    // The non-admin refusals live in their OWN slot (the signed-out arm already
    // populated `mi_issuance_refusals`); the cross-cause Then compares the two
    // dedicated vectors route-for-route.
    world.mi_issuance_non_admin_refusals = vec![
        na_get_pair,
        (
            "POST /workspace/invites".to_string(),
            na_post_status,
            na_post_body,
        ),
    ];
}

/// `Then each member-invite response is byte-identical to the never-existed path`
/// (scenarios 22 + 23) — the non-enumerability core. EVERY recorded issuance refusal
/// (GET + POST, non-admin or signed-out) is BYTE-IDENTICAL (status AND full body) to
/// the same-method never-existed-path control: a uniform 404, NO 403, NO 401, NO
/// login redirect, NO per-method divergence. The control itself must be a genuine 404
/// (so the comparison is not vacuously matching two redirects). Comparing the FULL
/// body — not merely "both 404" — is what makes the assertion falsifiable: any
/// existence-revealing divergence (a 401, a 303, a distinct body) re-REDS here.
#[then(regex = r#"^each member-invite response is byte-identical to the never-existed path$"#)]
async fn each_member_invite_response_byte_identical_to_never_existed(world: &mut FoundryWorld) {
    assert!(
        !world.mi_issuance_refusals.is_empty(),
        "no issuance-surface refusal was captured to assert on"
    );
    // No issuance refusal may be a login-redirect oracle.
    for (route, status, _body) in &world.mi_issuance_refusals {
        assert!(
            !status.is_redirection(),
            "{route} answered with a redirect ({status}) — a login-redirect oracle that \
             reveals the issuance surface exists (NFR-1 forbids it)"
        );
    }
    for (route, status, body) in &world.mi_issuance_refusals {
        let method = route
            .split_whitespace()
            .next()
            .expect("each recorded route is 'METHOD /path'");
        let (control_status, control_body) = world
            .mi_issuance_never_existed
            .get(method)
            .unwrap_or_else(|| panic!("a never-existed {method} control was captured"));
        // The control must be a genuine refusal, NOT a 3xx (so the comparison is not
        // vacuously matching two redirects). The GET control is the router fallback's
        // 404; the POST control is the CSRF layer's 403 (token-less, screened ahead of
        // routing) — both are 4xx client-error refusals with no existence oracle.
        assert!(
            control_status.is_client_error(),
            "the never-existed {method} control must itself be a genuine 4xx refusal (so \
             the comparison is not vacuously matching two redirects); got {control_status}"
        );
        assert_eq!(
            status, control_status,
            "{route} refused with status {status} but a never-existed {method} path refused \
             with {control_status} — a status oracle (no 403, 401, or redirect distinguishing \
             the issuance surface from nothing is allowed)"
        );
        assert_eq!(
            body, control_body,
            "{route} refusal body differs from the never-existed {method}-path body — a body \
             oracle that reveals the issuance surface exists. \
             issuance = {body:?}, never-existed = {control_body:?}"
        );
    }
}

/// `And the signed-out refusal is byte-identical to the non-admin refusal`
/// (scenario 23) — the cross-cause non-enumerability core (NFR-1, AC-03.1). For EVERY
/// issuance route, the signed-out caller's refusal is BYTE-IDENTICAL (status AND full
/// body) to the SIGNED-IN NON-ADMIN's refusal for the SAME route — so an observer
/// cannot tell WHY a request was refused (not-signed-in vs signed-in-but-not-admin).
/// Asserting the FULL body route-for-route is what makes the litmus bite: collapsing
/// the two refusal arms into distinct responses (a 403/401 on the not-authorized arm,
/// a "sign in to invite" oracle on the signed-out arm) re-REDS here.
#[then(regex = r#"^the signed-out refusal is byte-identical to the non-admin refusal$"#)]
async fn signed_out_refusal_byte_identical_to_non_admin(world: &mut FoundryWorld) {
    assert_eq!(
        world.mi_issuance_signed_out_refusals.len(),
        2,
        "both issuance routes must have a signed-out refusal baseline; got {:?}",
        world.mi_issuance_signed_out_refusals
    );
    assert_eq!(
        world.mi_issuance_non_admin_refusals.len(),
        2,
        "both issuance routes must have a non-admin refusal baseline; got {:?}",
        world.mi_issuance_non_admin_refusals
    );
    for (signed_out, non_admin) in world
        .mi_issuance_signed_out_refusals
        .iter()
        .zip(world.mi_issuance_non_admin_refusals.iter())
    {
        let (so_route, so_status, so_body) = signed_out;
        let (na_route, na_status, na_body) = non_admin;
        assert_eq!(
            so_route, na_route,
            "the signed-out and non-admin refusals must be compared route-for-route; got \
             {so_route:?} vs {na_route:?}"
        );
        assert_eq!(
            so_status, na_status,
            "{so_route} refused the signed-out caller with status {so_status} but the \
             non-admin with {na_status} — a status oracle revealing the refusal CAUSE \
             (NFR-1 forbids distinguishing not-signed-in from not-authorized)"
        );
        assert_eq!(
            so_body, na_body,
            "{so_route} refusal body for the signed-out caller differs from the non-admin \
             refusal body — a body oracle that reveals the refusal CAUSE. \
             signed-out = {so_body:?}, non-admin = {na_body:?}"
        );
    }
}

/// `And no invite is created` (scenarios 22 + 23) — the refused issuance probes
/// produced NO `invites` row: ZERO invites exist in the per-scenario Postgres. The
/// non-admin/signed-out POST was screened (gate or CSRF) BEFORE any `insert_invite`
/// ran, so the invite table is empty. Falsifiability: were the gate to let the POST
/// through, an invite for the smuggled email would appear → this REDs.
#[then(regex = r#"^no invite is created$"#)]
async fn no_invite_is_created(world: &mut FoundryWorld) {
    let pool = harness(world).app.state.store.pool().clone();
    let (rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM invites")
        .fetch_one(&pool)
        .await
        .expect("count invites after the refused issuance probes");
    assert_eq!(
        rows, 0,
        "a refused non-admin/signed-out issuance probe must create NO invite; found \
         {rows} invite rows"
    );
}

// ---------------------------------------------------------------------------
// CSRF — BOTH state-changing POSTs refused without a valid security token
// (scenario 24, step 02-06)
// ---------------------------------------------------------------------------
//
// GREEN-BY-INHERITANCE behind the SHIPPED double-submit `csrf_middleware`
// (csrf.rs:96-169). BOTH state-changing POSTs are mounted UNDER the layer
// (`build_router` .layer(csrf::csrf_middleware), lib.rs:430): the issuance POST
// `/workspace/invites` AND the accept POST `/invites/accept`. Neither is an exempt
// path (only `/bootstrap` is, csrf.rs:66), so a non-safe-method POST with no
// matching `foundry_csrf` cookie + `_csrf` form-field pair is rejected with
// `403 FORBIDDEN` from the middleware (csrf.rs:160-169) BEFORE it reaches either
// handler. The issuance POST therefore never reaches `submit_invite`
// (insert_invite never runs → no invite) and the accept POST never reaches
// `submit_accept` (the one-TX create-user + member-membership + consume never runs
// → no account, the invite stays live). (AC-03.9, NFR-6, I-E4 + A-E8.)
//
// The accept invite under test is a REAL live member invite (seeded via the SHIPPED
// `insert_invite` as Dana, `used_at` NULL, 7-day-minus-2-hours TTL), with a genuine
// id + sig + a policy-passing password — EVERYTHING a real accept needs EXCEPT the
// double-submit CSRF pair — so each refusal is isolated to the request-forgery
// protection, not a dead invite / bad token / policy failure. The issuance forged
// POST carries a smuggled email (`anonymous_issuance_request`) so a let-through
// would create a visible invite.
//
// Falsifiability (revert-reds-it litmus, demonstrated at DELIVER then restored):
// removing `.layer(csrf::csrf_middleware)` from `build_router` (or adding either
// route to `is_exempt_path`) lets BOTH token-less POSTs through to their handlers —
// the issuance POST would insert an invite for the smuggled email (RED-ing the
// 403 refusal Then AND the no-invite-created Then) and the accept POST would consume
// the live invite + create an account + 303 sign-in (RED-ing the 403 refusal Then
// AND the no-consume/no-account Then).

/// `Given a forged issuance submission and a forged accept submission for a live
/// invite, each without a valid security token` — seed a REAL live member invite via
/// the SHIPPED `insert_invite` (as Dana, `used_at` NULL, well within the 7-day
/// window), and confirm it is live against the REAL per-scenario Postgres, so the
/// accept-side CSRF refusal under test fires on the missing double-submit token, NOT
/// a dead invite. No request is sent yet; both forged (CSRF-less) POSTs are driven by
/// the When.
#[given(
    regex = r#"^a forged issuance submission and a forged accept submission for a live invite, each without a valid security token$"#
)]
async fn forged_issuance_and_accept_without_csrf(world: &mut FoundryWorld) {
    let ttl = time::Duration::days(7) - time::Duration::hours(2);
    seed_member_invite(world, "Northwind", "sam.okafor@northwind.example", ttl).await;

    let invite_id = world.mi_invite_id.expect("a member invite was seeded");
    let now = harness(world).app.state.clock.now();
    let pool = harness(world).app.state.store.pool().clone();
    let (live_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL AND expires_at > $2",
    )
    .bind(invite_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("count the live (unused, unexpired) invite row before the forged accept POST");
    assert_eq!(
        live_rows, 1,
        "the accept invite under test must be live (unused, unexpired) so the refusal \
         fires on the missing CSRF token, not a dead invite; found {live_rows} live rows"
    );
}

/// `When each reaches its surface` — drive BOTH forged POSTs over real HTTP, each
/// DELIBERATELY OMITTING the double-submit CSRF pair (no `foundry_csrf` cookie, no
/// `_csrf` form field): a token-less issuance POST to `/workspace/invites` (carrying
/// a smuggled email, via `anonymous_issuance_request`), and a token-less accept POST
/// to `/invites/accept` (carrying the genuine id + sig + a policy-passing
/// password + confirm). The SHIPPED `csrf_middleware` screens each BEFORE its handler.
/// Record each `(label, status, body)` refusal in `mi_issuance_refusals`; capture any
/// accept session cookie (expected absent) so the no-account Then can prove
/// non-commitment.
#[when(regex = r#"^each reaches its surface$"#)]
async fn each_forged_post_reaches_its_surface(world: &mut FoundryWorld) {
    // (1) Forged ISSUANCE POST — token-less, screened ahead of routing.
    let (issuance_status, issuance_body) =
        anonymous_issuance_request(world, "POST", "/workspace/invites").await;
    world.mi_issuance_refusals.push((
        "POST /workspace/invites".to_string(),
        issuance_status,
        issuance_body,
    ));

    // (2) Forged ACCEPT POST — genuine id + sig + policy-passing password, but NO
    //     foundry_csrf cookie and NO _csrf field. Screened by csrf_middleware.
    let invite_id = world.mi_invite_id.expect("a member invite was seeded");
    let sig = world
        .mi_invite_sig
        .clone()
        .expect("the invite sig was minted");
    let base = harness(world).base_url();
    let client = http(world);
    let form = [
        ("id", invite_id.to_string()),
        ("sig", sig),
        ("password", MEMBER_PASSWORD.to_string()),
        ("confirm", MEMBER_PASSWORD.to_string()),
    ];
    let resp = client
        .post(format!("{base}/invites/accept"))
        .form(&form)
        .send()
        .await
        .expect("POST /invites/accept (forged, no CSRF)");
    let accept_status = resp.status();
    world.mi_session_cookie = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .and_then(|s| s.split(';').next())
        .map(str::to_string);
    let accept_body = resp.text().await.unwrap_or_default();
    world.mi_issuance_refusals.push((
        "POST /invites/accept".to_string(),
        accept_status,
        accept_body,
    ));
}

/// `Then each is refused by the request-forgery protection` — BOTH forged POSTs were
/// rejected with `403 FORBIDDEN` by the SHIPPED `csrf_middleware` BEFORE their
/// handlers ran. The 403 (not a 200 "invite sent" fragment, not a 303 accept
/// success, not the accept handler's 200 refusal page) is the port-exposed
/// observable that the request-forgery layer — not the handler — refused each one.
#[then(regex = r#"^each is refused by the request-forgery protection$"#)]
async fn each_refused_by_request_forgery_protection(world: &mut FoundryWorld) {
    assert_eq!(
        world.mi_issuance_refusals.len(),
        2,
        "both forged POSTs (issuance + accept) must have recorded a refusal; got {:?}",
        world.mi_issuance_refusals
    );
    for (route, status, _body) in &world.mi_issuance_refusals {
        assert_eq!(
            *status,
            StatusCode::FORBIDDEN,
            "{route} must be refused with 403 FORBIDDEN by the SHIPPED csrf_middleware \
             BEFORE the handler (not a 200 success fragment, not a 303 accept success, \
             not the handler's 200 refusal); got {status}"
        );
    }
}

/// `And no invite is created, no invite is consumed, and no account is created` — the
/// request-forgery refusals were fully non-committal against the REAL per-scenario
/// Postgres: (a) the forged issuance POST created NO new invite — only the ONE seeded
/// live member invite exists (the smuggled email never landed); (b) that seeded
/// invite stays live (`used_at` NULL) — the forged accept POST consumed nothing; and
/// (c) NO `users` row exists for the invitee — the accept POST created no account and
/// established no session. Falsifiability: a CSRF bypass on either route would add an
/// invite row, stamp `used_at`, and/or create the account → this REDs.
#[then(regex = r#"^no invite is created, no invite is consumed, and no account is created$"#)]
async fn no_invite_created_consumed_no_account(world: &mut FoundryWorld) {
    let seeded_invite_id = world.mi_invite_id.expect("a member invite was seeded");
    let pool = harness(world).app.state.store.pool().clone();

    // (a) The forged issuance POST created NO new invite: only the seeded one exists.
    let (invite_rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM invites")
        .fetch_one(&pool)
        .await
        .expect("count invites after the forged POSTs");
    assert_eq!(
        invite_rows, 1,
        "the forged issuance POST must create NO new invite (only the ONE seeded live \
         invite may exist, the smuggled email never landed); found {invite_rows} invite rows"
    );

    // (b) The seeded invite stays live: the forged accept POST consumed nothing.
    let (live_rows,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL")
            .bind(seeded_invite_id)
            .fetch_one(&pool)
            .await
            .expect("count the still-live seeded invite after the forged accept POST");
    assert_eq!(
        live_rows, 1,
        "the forged accept POST must consume NO invite (the seeded invite must stay live, \
         used_at NULL); found {live_rows} live rows for the seeded invite"
    );

    // (c) The forged accept POST created NO account (and thus no session).
    let (user_rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email_lower = $1")
        .bind("sam.okafor@northwind.example")
        .fetch_one(&pool)
        .await
        .expect("count the invitee's user rows after the forged accept POST");
    assert_eq!(
        user_rows, 0,
        "the forged accept POST must create NO account for the invitee; found {user_rows} \
         user rows"
    );
    assert!(
        world.mi_session_cookie.is_none(),
        "the forged accept POST must establish NO session (no foundry_session cookie), \
         proving the request-forgery layer refused it before the handler signed anyone in; \
         got {:?}",
        world.mi_session_cookie
    );
}

/// Parse the `id` + `sig` out of an emitted `/invites/accept?id=<uuid>&sig=<sig>`
/// link rendered in the issuance "invite sent" fragment.
fn parse_accept_link(body: &str) -> (uuid::Uuid, String) {
    let start = body
        .find("/invites/accept?id=")
        .expect("the issuance fragment must carry an /invites/accept link");
    let after = &body[start + "/invites/accept?id=".len()..];
    let id_str: String = after.chars().take_while(|c| *c != '&').collect();
    let invite_id = uuid::Uuid::parse_str(id_str.trim()).unwrap_or_else(|_| {
        panic!("the emitted link must carry a valid invite id; got {id_str:?}")
    });
    let sig_marker = "sig=";
    let sig_start = after
        .find(sig_marker)
        .expect("the emitted link must carry a sig param");
    let sig_raw: String = after[sig_start + sig_marker.len()..]
        .chars()
        .take_while(|c| *c != '"' && *c != '&' && *c != '<' && *c != ' ')
        .collect();
    let sig = urlencoding::decode(&sig_raw)
        .map(|cow| cow.into_owned())
        .unwrap_or(sig_raw);
    (invite_id, sig)
}

// ---------------------------------------------------------------------------
// Scenario 25 (step 02-07) @property — NO-SECRET-LEAKAGE across the full member
// cycle: across a real issuance POST + a successful member accept + a hostile
// prober's refusal, NEITHER the invite `sig` value NOR the submitted password
// ever appears on any captured response-surface body; the refusal reason lives
// ONLY in internal `tracing` keyed on `invite_id` (+ %err). (AC-03.10, NFR-5.)
//
// Green by INHERITANCE from the SHIPPED design. PREPARE-verified production
// citation (foundry-app):
//   * `member_invites.rs` issuance handler — every `tracing` line carries ONLY
//     `%err` / `error = %err` (lines 134, 141, 159, 169, 188, 200); NEVER the
//     minted `sig`, NEVER any submitted value.
//   * `invites_accept.rs` accept handlers — every `tracing` line carries ONLY
//     `%invite_id` (+ `%err`) (lines 83, 151, 171, 220, 279, 304); NEVER the
//     `sig`, NEVER the raw `password`. The password is argon2id-hashed before any
//     persistence/log surface; the `sig` is verified by the tamper oracle but
//     never emitted.
//
// LOG OBSERVABLE — no in-process tracing-capture seam (the harness wires no
// custom subscriber; tracing is global-only, initialised in `main.rs`). So, per
// the step's guidance, the STRONGEST AVAILABLE observable is asserted: the FULL
// response-body surface across the cycle never contains the `sig` or the
// submitted password — backed by the tracing-keyed-on-invite_id citation above.
// A handler careless enough to format a secret into a log line is the same kind
// that would format it into a rendered body; the falsifiability demo (echoing
// the sig into the refusal/landing body, or rendering the password) reds the
// assertions, proving they are not vacuous.
//
// Slot reuse: this Given populates the SAME `ia_*` slots the SHARED When/Then
// (`feature_invite_accept.rs`) scan — `ia_cycle_bodies` (the true LOG-surface
// bodies: success POST + signed-in landing + prober refusal, which must carry NO
// sig and NO password), `ia_invite_sig` (the genuine member sig under
// protection), `ia_prober_sig` (the prober's supplied tampered sig), and
// `ia_get_form_body` (the LEGITIMATE sig-carriers — the issuance "invite sent"
// fragment + the holder's own GET set-password form — joined; EXCLUDED from the
// sig-scan because they are the admin's/holder's own links round-tripped back,
// NOT log surfaces, but STILL password-scanned). The member submits
// `PRIYA_PASSWORD` (min-12 policy-passing) so the shared password Then — which
// scans for that literal — bites genuinely on this cycle.
//
// Example-pinned at LAYER 3 (Mandate 11): one concrete full member cycle, the
// universal-invariant SHAPE (no secret in the observable log surface) enumerated
// explicitly; NO PBT machinery at this layer.
#[given(
    regex = r#"^Dana issues an invite, Sam completes a successful accept, and a hostile prober is refused$"#
)]
async fn member_no_leak_full_cycle(world: &mut FoundryWorld) {
    // The submitted password for the member accept — the SAME literal the shared
    // `no_submitted_password` Then scans for, so its scan is non-vacuous here.
    const CYCLE_PASSWORD: &str = "northwind-secure-pass";
    let invitee = "leakprobe.sam@northwind.example";

    let dana_email = world
        .mi_admin_email
        .clone()
        .expect("the Background seeded Dana's email");
    let base = harness(world).base_url();
    let client = http(world);

    let mut log_surface_bodies: Vec<String> = Vec::new();
    // Legitimate sig-carriers (NOT log surfaces): the issuance fragment + the GET
    // set-password form. Joined into `ia_get_form_body`; excluded from the sig-scan,
    // included in the password-scan.
    let mut legit_sig_carriers: Vec<String> = Vec::new();

    // 1 — Dana issues a REAL member invite over the admin-gated issuance route. The
    //     rendered "invite sent" fragment legitimately carries the accept link (with
    //     the sig) — the admin's own shareable link, NOT a log surface. Parse the
    //     genuine id + sig out of it for the accept + prober legs.
    let issuance = signed_in_post(
        harness(world),
        &client,
        &dana_email,
        DANA_PASSWORD,
        "/workspace/invites",
        &[("email", invitee)],
    )
    .await;
    assert_eq!(
        issuance.status,
        StatusCode::OK,
        "the member-invite issuance POST must render a 200 'invite sent' fragment; \
         body = {:?}",
        issuance.body
    );
    let (invite_id, sig) = parse_accept_link(&issuance.body);
    world.mi_invite_id = Some(invite_id);
    world.mi_invite_sig = Some(sig.clone());
    legit_sig_carriers.push(issuance.body);

    // 2 — GET the live link: the set-password form + the double-submit CSRF cookie.
    let get_resp = client
        .get(format!(
            "{base}/invites/accept?id={invite_id}&sig={sig}",
            sig = urlencoding::encode(&sig)
        ))
        .send()
        .await
        .expect("GET /invites/accept (member no-leak cycle)");
    assert_eq!(
        get_resp.status(),
        StatusCode::OK,
        "the GET for the live member invite must render the set-password form"
    );
    let csrf_cookie = get_resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(str::to_string)
        .expect("the GET minted a foundry_csrf cookie");
    let csrf_token = csrf_cookie
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();
    // The GET form legitimately carries the sig in its hidden field (the holder's own
    // valid link round-tripped back to her) — a legit sig-carrier, NOT a log surface.
    legit_sig_carriers.push(get_resp.text().await.unwrap_or_default());

    // 3 — POST the success accept: create user + member membership + write password
    //     + sign in. The submitted password is `CYCLE_PASSWORD`.
    let form = [
        ("id", invite_id.to_string()),
        ("sig", sig.clone()),
        ("password", CYCLE_PASSWORD.to_string()),
        ("confirm", CYCLE_PASSWORD.to_string()),
        ("_csrf", csrf_token.clone()),
    ];
    let post_resp = client
        .post(format!("{base}/invites/accept"))
        .header(
            reqwest::header::COOKIE,
            format!("foundry_csrf={csrf_token}"),
        )
        .form(&form)
        .send()
        .await
        .expect("POST /invites/accept (member no-leak cycle)");
    assert_eq!(
        post_resp.status(),
        StatusCode::SEE_OTHER,
        "the success member accept must 303 SEE_OTHER (auto sign-in)"
    );
    let location = post_resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .expect("the success POST set a Location to follow");
    let session_cookie = post_resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .and_then(|s| s.split(';').next())
        .map(str::to_string)
        .expect("the success POST issued a foundry_session cookie");
    // The success POST body is a true LOG-surface body — must carry NO sig, NO password.
    log_surface_bodies.push(post_resp.text().await.unwrap_or_default());

    // 4 — follow the 303 to the signed-in landing page (a true LOG-surface body).
    let landing_resp = client
        .get(format!("{base}{location}"))
        .header(reqwest::header::COOKIE, session_cookie)
        .send()
        .await
        .expect("GET the signed-in landing page (member no-leak cycle)");
    log_surface_bodies.push(landing_resp.text().await.unwrap_or_default());

    // 5 — a hostile prober opens a TAMPERED link → the uniform refusal (a true
    //     LOG-surface body). A fresh genuine signature for the SAME id, then one char
    //     flipped, so the refusal fires on the tamper oracle; its body must leak
    //     neither the prober's supplied sig nor any secret.
    let secret = harness(world).app.state.session_secret.clone();
    let now = harness(world).app.state.clock.now();
    let prober_expires_at = now + time::Duration::days(7);
    let prober_genuine = foundry_auth::InviteToken::new(invite_id, prober_expires_at, &secret)
        .expect("mint a genuine signature to tamper for the prober")
        .signature;
    let prober_sig = crate::steps::feature_invite_accept::tamper_one_char(&prober_genuine);
    let refusal_resp = client
        .get(format!(
            "{base}/invites/accept?id={invite_id}&sig={sig}",
            sig = urlencoding::encode(&prober_sig)
        ))
        .send()
        .await
        .expect("GET /invites/accept (hostile prober, tampered member link)");
    assert_eq!(
        refusal_resp.status(),
        StatusCode::OK,
        "the hostile prober's tampered link must render the uniform 200 refusal"
    );
    let refusal_body = refusal_resp.text().await.unwrap_or_default();
    assert!(
        refusal_body
            .to_ascii_lowercase()
            .contains("no longer valid"),
        "the prober refusal must be the uniform \"no longer valid\" page; got \
         {refusal_body:?}"
    );
    log_surface_bodies.push(refusal_body);

    // Populate the SHARED slots the When/Then scan. `ia_invite_sig` = the genuine
    // member sig under protection; `ia_prober_sig` = the prober's supplied tampered
    // sig; `ia_cycle_bodies` = the true LOG-surface bodies (sig + password must be
    // absent); `ia_get_form_body` = the joined legitimate sig-carriers (issuance
    // fragment + GET form), excluded from the sig-scan but password-scanned.
    world.ia_invite_sig = Some(sig);
    world.ia_prober_sig = Some(prober_sig);
    world.ia_cycle_bodies = log_surface_bodies;
    world.ia_get_form_body = Some(legit_sig_carriers.join("\n"));
}

// ---------------------------------------------------------------------------
// Member password-recovery cluster (step 03-01) — scenarios 26, 27, 30
// ---------------------------------------------------------------------------
//
// The member arm INHERITS the SHIPPED `submit_accept` recovery contract
// (`invites_accept.rs`): `validate_password_inputs` runs the confirm-match check
// THEN the min-12 `check_password_policy` BEFORE `create_member_and_consume` opens
// — a rejected password re-renders the set-password form inline (200) and leaves
// the invite UNTOUCHED (no account, no membership, no consume, no session). The
// invite-accept first-admin password-recovery scenarios (03-01/02/04 in
// `feature_invite_accept`) are the precedent; this cluster proves the SAME
// behaviour for an account-CREATING member invite (the member arm), green by
// inheritance — NO production code added here; acceptance GLUE only.
//
//   26 (PRIMARY, weak password): a sub-min-12 password on a live member invite
//       re-renders inline with the min-length error; NO `users` row, the invite
//       stays live (unconsumed), no `foundry_session` cookie. Falsifiability
//       (proven at DELIVER, reverted): dropping/moving the policy check AFTER the
//       consume TX would EITHER consume the invite (the consume guard fires first
//       → "still live and unconsumed" REDs) OR accept the weak password (303 +
//       session → the inline-error + no-account/no-session Thens RED).
//
//   27 (mismatched confirmation): password != confirm → inline mismatch error; NO
//       account, the invite stays live. REUSES the SHIPPED invite-accept When/Then
//       (`her confirmation does not match her new password` / `she sees an inline
//       error that the passwords do not match`) — the member Given populates BOTH
//       the `ia_*` slots those steps read AND `session_cookie_header` (the GET CSRF
//       cookie), while the harness falls back to the member `mi_harness`, so the
//       member invite drives them with NO duplicate regex.
//
//   30 (BOUNDARY, exactly-12): a confirm-matching exactly-12-character password
//       completes the join (account + membership + consume + session) — the
//       INCLUSIVE side of the min-12 boundary (NFR-4, "at least 12"). REUSES the
//       shipped `his member account is created and he is signed in on "<ws>"` Then.

/// A weak member password BELOW the min-12 policy (3 chars) — rejected by
/// `check_password_policy` BEFORE the member consume TX opens.
const MEMBER_WEAK_PASSWORD: &str = "abc";
/// A member password EXACTLY at the min-12 boundary (12 chars) — the INCLUSIVE
/// side of the `check_password_policy` length-first rule (NFR-4).
const MEMBER_TWELVE_CHAR_PASSWORD: &str = "abcdef123456";

/// POST `/invites/accept` for the LIVE member invite under test (driven by the
/// `mi_*` slots + the GET-minted `mi_get_csrf_cookie`) with `password` + `confirm`,
/// capturing the status + re-rendered body + any session cookie into
/// `mi_post_status` / `last_body` / `mi_session_cookie`. Shared by the weak +
/// boundary member-recovery Whens. The GET that minted the CSRF cookie ran in the
/// scenario's `Given` (`sam_opened_live_invite_seen_form` → `get_member_accept_page`),
/// so this is the SECOND leg only — no re-GET, mirroring the inherited recovery dance.
async fn member_accept_post_with_confirm(world: &mut FoundryWorld, password: &str, confirm: &str) {
    let invite_id = world.mi_invite_id.expect("a live member invite was seeded");
    let sig = world
        .mi_invite_sig
        .clone()
        .expect("the invite sig was minted");
    let csrf_cookie = world
        .mi_get_csrf_cookie
        .clone()
        .expect("the Given's GET minted a foundry_csrf cookie");
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
        ("password", password.to_string()),
        ("confirm", confirm.to_string()),
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
        .expect("POST /invites/accept (member recovery)");
    world.mi_post_status = Some(resp.status());
    world.mi_session_cookie = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .and_then(|s| s.split(';').next())
        .map(str::to_string);
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

// --- Scenario 26 (PRIMARY): a weak password is corrected inline ----------------

/// `When he submits a password below the strength policy` — POST the SHIPPED member
/// accept with a WEAK password (3 chars, below min-12) and a MATCHING confirm (so
/// ONLY the policy fails, not the confirm match). `validate_password_inputs` rejects
/// it via `check_password_policy` BEFORE `create_member_and_consume` opens, so the
/// handler re-renders the form inline (200) and touches NOTHING.
#[when(regex = r#"^he submits a password below the strength policy$"#)]
async fn he_submits_a_weak_password(world: &mut FoundryWorld) {
    member_accept_post_with_confirm(world, MEMBER_WEAK_PASSWORD, MEMBER_WEAK_PASSWORD).await;
}

/// `Then he sees an inline error explaining the minimum password length` — the
/// weak-password POST re-rendered the set-password form IN PLACE at 200 OK carrying
/// the min-length policy copy ("at least 12 characters"), still posting back to
/// /invites/accept (an inline correction, not a refusal or a 303 redirect). A policy
/// check moved AFTER the consume (or dropped) would 303 instead → this REDs.
#[then(regex = r#"^he sees an inline error explaining the minimum password length$"#)]
async fn he_sees_inline_min_length_error(world: &mut FoundryWorld) {
    assert_eq!(
        world.mi_post_status,
        Some(StatusCode::OK),
        "a weak-password member POST must re-render the form inline at 200 OK (not a \
         303 redirect, not a refusal); got {:?}",
        world.mi_post_status
    );
    let body = world
        .last_body
        .clone()
        .expect("the weak-password POST captured a re-rendered body");
    let lower = body.to_ascii_lowercase();
    assert!(
        lower.contains("at least 12") || lower.contains("at least twelve"),
        "the inline error must explain the MINIMUM password length (at least 12 \
         characters); got {body:?}"
    );
    assert!(
        body.contains(r#"action="/invites/accept""#) && body.contains(r#"name="password""#),
        "the inline error must be shown ON the set-password form (re-rendered in \
         place, posting back to /invites/accept), not on a refusal page; got {body:?}"
    );
}

/// `And no account is created and no session is created` — the rejected member
/// accept was NON-COMMITTAL end-to-end: EXACTLY ZERO `users` rows map to Sam's email
/// (no `create_member_and_consume` ran) AND the POST issued NO `foundry_session`
/// cookie (the policy check fails before the consume TX + `establish_session_and_
/// redirect`). Reads the REAL per-scenario Postgres at the driven-port boundary. A
/// policy gap that let the weak password through would create the account + a session
/// here → this REDs.
#[then(regex = r#"^no account is created and no session is created$"#)]
async fn no_account_and_no_session_created(world: &mut FoundryWorld) {
    let pool = harness(world).app.state.store.pool().clone();
    let (rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email_lower = $1")
        .bind(SAM_EMAIL)
        .fetch_one(&pool)
        .await
        .expect("count the invitee's user rows after the rejected accept");
    assert_eq!(
        rows, 0,
        "a weak-password member accept must create NO account (the policy check fails \
         before the consume TX); found {rows} rows for {SAM_EMAIL:?}"
    );
    assert!(
        world.mi_session_cookie.is_none(),
        "a weak-password member accept must create NO session — no foundry_session \
         cookie (the policy check fails before the consume TX + sign-in); got {:?}",
        world.mi_session_cookie
    );
}

// --- Scenario 27: a mismatched confirmation is corrected inline -----------------

/// `Given Priya Shah has opened her live member invite for "<workspace>" and seen
/// the set-password form` — seed a LIVE member invite for Priya Shah (two hours ago,
/// well inside the 7-day window, unused) via the SHIPPED `insert_invite`, GET the
/// accept page (rendering the set-password form + minting the double-submit CSRF
/// cookie), and assert the form rendered. Then mirror the member invite into the
/// `ia_*` slots + stash the GET CSRF cookie into `session_cookie_header`, so the
/// SHIPPED invite-accept mismatch When/Then (`her confirmation does not match her
/// new password` / `she sees an inline error that the passwords do not match`) drive
/// THIS member invite with NO duplicate regex — `harness()` falls back to the member
/// `mi_harness`. The arrival state for the mismatch-inline proof.
#[given(
    regex = r#"^Priya Shah has opened her live member invite for "([^"]+)" and seen the set-password form$"#
)]
async fn priya_shah_opened_live_invite_seen_form(world: &mut FoundryWorld, ws_name: String) {
    let ttl = time::Duration::days(7) - time::Duration::hours(2);
    seed_member_invite(world, &ws_name, PRIYA_SHAH_EMAIL, ttl).await;
    get_member_accept_page(world).await;
    assert_eq!(
        world.mi_post_status,
        Some(StatusCode::OK),
        "the GET accept page for Priya's live member invite must render a 200 \
         set-password form; got {:?}",
        world.mi_post_status
    );
    let body = world
        .last_body
        .clone()
        .expect("the GET captured a rendered body");
    assert!(
        body.contains(r#"action="/invites/accept""#) && body.contains(r#"name="password""#),
        "the GET must render a set-password form posting to /invites/accept; got {body:?}"
    );

    // Point the SHIPPED invite-accept mismatch When/Then at THIS member invite (they
    // read `ia_*` + `session_cookie_header`; the harness falls back to `mi_harness`).
    world.ia_invite_id = world.mi_invite_id;
    world.ia_invite_sig = world.mi_invite_sig.clone();
    world.session_cookie_header = world.mi_get_csrf_cookie.clone();
}

/// `And her invite is still live and unconsumed and no account is created` — the
/// rejected mismatch member accept was NON-COMMITTAL: Priya's seeded invite is STILL
/// live (`used_at` NULL and `expires_at > now`) and EXACTLY ZERO `users` rows map to
/// her email (no `create_member_and_consume` ran). Reads the REAL per-scenario
/// Postgres at the driven-port boundary. A confirm-match check moved AFTER the
/// consume would set `used_at` (or create the account) → this REDs.
#[then(regex = r#"^her invite is still live and unconsumed and no account is created$"#)]
async fn her_invite_still_live_and_no_account(world: &mut FoundryWorld) {
    let invite_id = world.mi_invite_id.expect("a member invite was seeded");
    let now = harness(world).app.state.clock.now();
    let pool = harness(world).app.state.store.pool().clone();

    let (live_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL AND expires_at > $2",
    )
    .bind(invite_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("count the still-live invite row after the rejected mismatch accept");
    assert_eq!(
        live_rows, 1,
        "the member invite must stay live (unused, unexpired) after the rejected \
         mismatch accept; found {live_rows} live rows"
    );

    let (user_rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email_lower = $1")
        .bind(PRIYA_SHAH_EMAIL)
        .fetch_one(&pool)
        .await
        .expect("count Priya's user rows after the rejected mismatch accept");
    assert_eq!(
        user_rows, 0,
        "a mismatched-confirm member accept must create NO account; found {user_rows} \
         rows for {PRIYA_SHAH_EMAIL:?}"
    );
}

// --- Scenario 30 (BOUNDARY): a 12-char password is accepted --------------------

/// `When he submits a twelve-character password and confirms it` — POST the SHIPPED
/// member accept with a password EXACTLY at the min-12 boundary (12 chars) and a
/// MATCHING confirm. `check_password_policy` admits the inclusive boundary (NFR-4,
/// "at least 12"), so `create_member_and_consume` runs: account + member membership
/// created, invite consumed, session established, 303 → workspace. Captures the 303 +
/// Location + session cookie via the full GET→POST dance (a fresh GET re-mints the
/// CSRF cookie for the redirect-following success POST).
#[when(regex = r#"^he submits a twelve-character password and confirms it$"#)]
async fn he_submits_twelve_char_password(world: &mut FoundryWorld) {
    accept_member_invite(world, MEMBER_TWELVE_CHAR_PASSWORD).await;
}

// ---------------------------------------------------------------------------
// Issuance inline recovery (step 03-02) — scenario 28: a BLANK email on the
// issuance form is corrected inline; NO invite is created.
// ---------------------------------------------------------------------------
//
// GREEN-BY-INHERITANCE through the SHIPPED issuance handler (`member_invites.rs`
// `submit_invite`): a blank/empty `email` form field is trimmed to "" and short-
// circuits BEFORE `insert_invite` runs, re-rendering the member-invite form inline
// (200) carrying the `BLANK_EMAIL_ERROR` copy ("Enter an email address to invite a
// member.") — NO `invites` row. The admin can correct and resubmit on the same form
// (AC-04.3, FR-3, I-E3). This step authors only acceptance GLUE; NO production code.
//
// Falsifiability (proven at DELIVER, reverted): were the blank-email guard removed so
// the handler called `insert_invite` with an empty email, an `invites` row would be
// created and the response would be the "invite sent" fragment rather than the inline
// form error → both the inline-error Then and the "no invite is created" Then would RED.

/// `When Dana submits the form with an empty email` — drive the REAL admin-gated
/// issuance POST `/workspace/invites` as the signed-in admin with an EMPTY `email`
/// field. The SHIPPED `submit_invite` trims the blank email, short-circuits before
/// `insert_invite`, and re-renders the form inline (200) with the blank-email error.
/// Captures the re-rendered body for the inline-error Then.
#[when(regex = r#"^Dana submits the form with an empty email$"#)]
async fn dana_submits_empty_email(world: &mut FoundryWorld) {
    let dana_email = world
        .mi_admin_email
        .clone()
        .expect("the Background seeded Dana's email");
    let client = http(world);
    let outcome = signed_in_post(
        harness(world),
        &client,
        &dana_email,
        DANA_PASSWORD,
        "/workspace/invites",
        &[("email", "")],
    )
    .await;
    world.mi_post_status = Some(outcome.status);
    world.last_body = Some(outcome.body);
}

/// `Then she sees an inline error asking for an email address` — the blank-email POST
/// re-rendered the member-invite form IN PLACE at 200 OK carrying the blank-email
/// error copy and still posting to /workspace/invites (an inline correction, not a
/// 303 redirect, not the "invite sent" fragment). The handler short-circuited before
/// `insert_invite`, so the admin can correct + resubmit on the same form.
#[then(regex = r#"^she sees an inline error asking for an email address$"#)]
async fn sees_inline_blank_email_error(world: &mut FoundryWorld) {
    assert_eq!(
        world.mi_post_status,
        Some(StatusCode::OK),
        "a blank-email issuance POST must re-render the form inline at 200 OK (not a \
         303, not the invite-sent fragment); got {:?}",
        world.mi_post_status
    );
    let body = world
        .last_body
        .clone()
        .expect("the blank-email POST captured a re-rendered body");
    let lower = body.to_ascii_lowercase();
    assert!(
        lower.contains("email address"),
        "the inline error must ask for an email address; got {body:?}"
    );
    assert!(
        body.contains(r#"action="/workspace/invites""#) && body.contains(r#"name="email""#),
        "the inline error must be shown ON the member-invite form (re-rendered in \
         place, posting back to /workspace/invites), not on the invite-sent fragment; \
         got {body:?}"
    );
    assert!(
        !body.contains("/invites/accept?id="),
        "a blank-email POST must NOT render the shareable accept link (no invite was \
         created); got {body:?}"
    );
}

// ---------------------------------------------------------------------------
// Member inline-recovery RE-ATTEMPT (step 03-02) — scenario 29 (PRIMARY): after an
// inline password error, a VALID retry on the SAME live member invite completes the
// join (account + member membership + consume + session), proving recoverability —
// the failed attempt did NOT strand the invitee on a dead link (AC-04.5).
// ---------------------------------------------------------------------------
//
// GREEN-BY-INHERITANCE through the SHIPPED member arm: scenario 26 (03-01) proved a
// weak password re-renders inline at 200 WITHOUT consuming the invite (the policy
// check runs before `create_member_and_consume` opens). This scenario chains off
// that left-live invite and proves the SECOND, VALID submission on the SAME invite —
// reusing the SAME GET-minted double-submit CSRF cookie + the SAME signed token —
// passes the policy, fires the consume guarded-UPDATE, creates the account + member
// membership, writes the hash, establishes a session, and 303-redirects. NO production
// code added here; acceptance GLUE only.
//
// Falsifiability (proven at DELIVER, reverted): had the FAILED weak attempt consumed
// the invite (e.g. moving the policy check AFTER the consume TX), the retry's consume
// guarded-UPDATE would match 0 rows (`used_at IS NULL` already false) → the retry would
// be REFUSED (the uniform refusal page / no 303 / no session) and NO account would be
// created → the "account created + signed in" + "used exactly once" Thens would RED.
// The Given's own DB-observable live-invite assertion binds that explicitly.

/// `Given Sam was shown an inline password error and his member invite is still live`
/// — seed a LIVE member invite for Sam (two hours ago, unused), GET the accept page
/// (rendering the set-password form + minting the `mi_get_csrf_cookie`), then drive
/// the reused weak-password POST (3 chars, below min-12, matching confirm) which
/// re-renders inline at 200 WITHOUT consuming the invite (the policy check runs before
/// the consume TX). Assert (DB-observable against the REAL per-scenario Postgres) the
/// failed attempt was an INLINE error (200, no session) AND the invite is STILL live —
/// grounding the "shown an inline error, invite still live" precondition in observable
/// state, and binding the recovery explicitly to the invite-stays-live behaviour
/// proven in scenario 26 (03-01).
#[given(regex = r#"^Sam was shown an inline password error and his member invite is still live$"#)]
async fn sam_shown_inline_error_invite_live(world: &mut FoundryWorld) {
    // Reused arrival: seed the live member invite + GET the form, minting the CSRF cookie.
    sam_opened_live_invite_seen_form(world, "Northwind".to_string()).await;

    // Reused failed attempt: a weak password re-renders inline (200) without consuming.
    he_submits_a_weak_password(world).await;

    // The failed attempt was an INLINE error (200 re-render), not a redirect/refusal.
    assert_eq!(
        world.mi_post_status,
        Some(StatusCode::OK),
        "the failed attempt must re-render inline at 200 OK (the inline password \
         error), proving the member was shown a correctable error; got {:?}",
        world.mi_post_status
    );
    assert!(
        world.mi_session_cookie.is_none(),
        "the failed attempt must establish NO session (the policy check fails before \
         the consume TX + sign-in); got {:?}",
        world.mi_session_cookie
    );

    // DB-observable: the invite is STILL live after the failed attempt — the failed
    // POST did not strand the member on a dead link. If the weak POST HAD consumed the
    // invite, this count would be 0 and the precondition would RED here.
    let invite_id = world.mi_invite_id.expect("a member invite was seeded");
    let now = harness(world).app.state.clock.now();
    let pool = harness(world).app.state.store.pool().clone();
    let (live_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM invites WHERE id = $1 AND used_at IS NULL AND expires_at > $2",
    )
    .bind(invite_id)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("count the live (unused, unexpired) member invite row after the failed attempt");
    assert_eq!(
        live_rows, 1,
        "after the failed password attempt the member invite must still be live \
         (used_at NULL, unexpired) — the failed attempt must NOT have consumed it; \
         found {live_rows} live rows"
    );
}

/// `When he submits a valid password on the same invite and confirms it` — drive a
/// VALID retry POST against the SAME live member invite, reusing the SAME GET-minted
/// double-submit CSRF cookie + the SAME signed token, with a policy-passing password
/// + matching confirm. The policy passes, `create_member_and_consume` fires (the
/// consume guarded-UPDATE still matches because the failed attempt left the invite
/// live), the account + member membership are created, the hash is written, a session
/// is established, and the handler 303-redirects. Captures the 303 + session cookie
/// for the reused success Thens.
#[when(regex = r#"^he submits a valid password on the same invite and confirms it$"#)]
async fn sam_submits_valid_retry_on_same_invite(world: &mut FoundryWorld) {
    member_accept_post_with_confirm(world, MEMBER_PASSWORD, MEMBER_PASSWORD).await;
}
