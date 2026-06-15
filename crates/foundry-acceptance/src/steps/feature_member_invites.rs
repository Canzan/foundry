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
