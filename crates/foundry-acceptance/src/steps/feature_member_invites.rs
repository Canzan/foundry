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
//   10. ISOLATION — the new member, reading through the SHIPPED resolution +
//       scoped-read seam (`resolve_active_workspace(sam)` →
//       `find_team_by_slug(acting_ws, …)` → `is_team_member` →
//       `find_project_by_slug` → `list_issues_by_project`, exactly the chain
//       `list_board_issues` walks), sees ONLY the inviting workspace's data and
//       no other tenant's. Falsifiability: a foreign workspace ("Globex") seeded
//       with the SAME team/project slugs holds its own issue; resolving Sam to
//       Globex (or reading unscoped) would surface Globex's issue → RED. This is
//       the slice-06 isolation idiom (`board_titles_scoped`) applied to the
//       member-invite join.
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

/// `When Sam views his workspace` — drive the SHIPPED scoped-read seam exactly as
/// `list_board_issues` does: resolve Sam's acting workspace from his sole
/// membership (`resolve_active_workspace`), then read the `core`/`apollo` board
/// scoped to THAT workspace, membership-gated. Stash the titles he is permitted to
/// see. NO new isolation code — green by inheritance through the shipped seam.
#[when(regex = r#"^Sam views his workspace$"#)]
async fn sam_views_his_workspace(world: &mut FoundryWorld) {
    let store = harness(world).app.state.store.clone();
    let sam_id = sam_user_id(world).await;

    let (acting_ws, _name) = store
        .resolve_active_workspace(sam_id)
        .await
        .expect("resolve Sam's active workspace")
        .expect("the new member resolves to his inviting workspace");

    let titles = member_board_titles_scoped(&store, acting_ws, sam_id).await;
    world.mi_seen_titles = titles;
}

/// The SHIPPED scoped-read chain (`find_team_by_slug(acting_ws, "core")` →
/// `is_team_member` → `find_project_by_slug("apollo")` → `list_issues_by_project`),
/// extracted so the read is driven with the RESOLVED acting workspace. A foreign
/// acting workspace (the isolation falsifiability mutation) surfaces the other
/// tenant's issue.
async fn member_board_titles_scoped(
    store: &foundry_store::Store,
    acting_workspace_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Vec<String> {
    let Some(team) = store
        .find_team_by_slug(acting_workspace_id, "core")
        .await
        .expect("find team by slug scoped to acting workspace")
    else {
        return Vec::new();
    };
    if !store
        .is_team_member(team.id, user_id)
        .await
        .expect("team membership gate")
    {
        return Vec::new();
    }
    let Some(project) = store
        .find_project_by_slug(team.id, "apollo")
        .await
        .expect("find project by slug")
    else {
        return Vec::new();
    };
    store
        .list_issues_by_project(project.id)
        .await
        .expect("scoped issue read")
        .into_iter()
        .map(|row| row.title)
        .collect()
}

/// `Then he sees only "<workspace>" data` — the scoped read returns EXACTLY the
/// inviting tenant's own issue (and nothing else). The foreign Globex issue, seeded
/// under the SAME slugs, does NOT appear — proving the read is scoped to Sam's
/// resolved workspace, not leaking across tenants.
#[then(regex = r#"^he sees only "([^"]+)" data$"#)]
async fn sees_only_workspace_data(world: &mut FoundryWorld, _ws_name: String) {
    assert_eq!(
        world.mi_seen_titles,
        vec![NORTHWIND_ISSUE_TITLE.to_string()],
        "the new member must see ONLY his inviting tenant's data; saw {:?}",
        world.mi_seen_titles
    );
}

/// `And he sees no data from any other workspace` — the foreign tenant's issue
/// (Globex's, under the SAME slugs) is absent from the new member's scoped read.
/// The isolation guarantee: there is no path by which his resolution-scoped read
/// surfaces another workspace's data.
#[then(regex = r#"^he sees no data from any other workspace$"#)]
async fn sees_no_foreign_data(world: &mut FoundryWorld) {
    assert!(
        !world
            .mi_seen_titles
            .contains(&GLOBEX_ISSUE_TITLE.to_string()),
        "the new member must NOT see any foreign tenant's data; the Globex issue \
         leaked into {:?}",
        world.mi_seen_titles
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
