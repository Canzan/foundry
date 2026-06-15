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

use crate::support::harness::{signed_in_post, InProcHarness};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
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
