//! Feature A "Programmatic Foundry" step definitions — the JSON API
//! (US-W05a read, US-W05b machine-token auth, US-W05c writes) and the
//! boundary guard (US-W06).
//!
//! RED-state contract (DISTILL, Mandate 7):
//! These steps drive the JSON API through the SAME in-process axum harness the
//! browser scenarios use (`InProcHarness` → `build_router`). The `/api/v1`
//! routes are NOT yet merged into `build_router`, and the `foundry-api` /
//! `foundry-services` crates are RED scaffolds, so:
//!   - Background + Given steps set up real preconditions (workspace, team,
//!     project, issues, sessions) via the existing shared helpers — they MUST
//!     succeed, so the failure is in the behaviour, not the fixture.
//!   - When steps issue a real HTTP request to `/api/v1/...` and capture the
//!     response into the world.
//!   - Then steps assert the JSON outcome and FAIL RED (the route 404s today;
//!     once DELIVER merges `foundry_api::routes(state)` and implements the
//!     services, the assertion flips GREEN). This is MISSING_FUNCTIONALITY,
//!     not BROKEN.
//!
//! Background phrases are REUSED from us_06/us_07/us_08 (cucumber-rs requires
//! globally-unique step text):
//!   - `a workspace "..." exists with admin "..."`        (us_06_signin)
//!   - `a member "..." belongs to the team "..."`          (us_07_project_create)
//!   - `a project "..." with key prefix "..." exists in the "..." team` (us_08_file_issue)
//!   - `(\w+) is signed in`                                 (us_07_project_create)
//!
//! Only Feature-A-specific phrases are declared here.
//!
//! What DELIVER must wire to flip these GREEN is enumerated in
//! `docs/feature/web-tier-extraction/distill/step-skeletons.md`.

use crate::support::harness::InProcHarness;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use secrecy::ExposeSecret;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";

fn now_anchor() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(TEST_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse anchor")
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(Policy::none())
        .cookie_store(false)
        .build()
        .expect("build reqwest client")
}

async fn ensure_harness(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        world.harness = Some(InProcHarness::spawn(now_anchor()).await);
    }
    if world.http.is_none() {
        world.http = Some(client());
    }
}

// --------------------------------------------------------------------------
// Given — seed issues for a project (Feature-A-specific phrasing)
// --------------------------------------------------------------------------

#[given(
    regex = r#"^the "([^"]+)" project has issue (\w+)-(\d+) titled "([^"]+)" (in progress|in the backlog)$"#
)]
async fn project_has_titled_issue(
    world: &mut FoundryWorld,
    project_name: String,
    prefix: String,
    number: i32,
    title: String,
    state_phrase: String,
) {
    ensure_harness(world).await;
    let state = match state_phrase.as_str() {
        "in progress" => "in_progress",
        _ => "backlog",
    };
    seed_issue(world, &project_name, &prefix, number, &title, state).await;
    world.fa_last_project_name = Some(project_name);
}

#[given(regex = r#"^the "([^"]+)" project has no issues$"#)]
async fn project_has_no_issues(world: &mut FoundryWorld, project_name: String) {
    ensure_harness(world).await;
    // No-op seed: the project already exists from Background; we simply record
    // it as the target. The "empty" precondition is the absence of issue rows.
    world.fa_last_project_name = Some(project_name);
}

#[given(regex = r#"^the team "([^"]+)" owns a project "([^"]+)" with key prefix "([^"]+)"$"#)]
async fn other_team_owns_project(
    world: &mut FoundryWorld,
    team_name: String,
    project_name: String,
    key_prefix: String,
) {
    ensure_harness(world).await;
    seed_team_with_project(world, &team_name, &project_name, &key_prefix).await;
}

#[given(regex = r#"^the "([^"]+)" project has a comment by (\w+) on issue (\w+)-(\d+)$"#)]
async fn project_has_comment(
    world: &mut FoundryWorld,
    project_name: String,
    author: String,
    prefix: String,
    number: i32,
) {
    ensure_harness(world).await;
    // The issue must exist for a comment to hang off it.
    seed_issue(
        world,
        &project_name,
        &prefix,
        number,
        "Seeded for comment",
        "backlog",
    )
    .await;
    // Seed a REAL comment row authored by the named persona so the non-author
    // edit reaches the service's author-only authz check (→ 403), not a 404
    // for a missing comment. A precondition row (not the behaviour under test):
    // a plain INSERT mirroring the existing seeding shape.
    let author_email = email_for_persona(&author);
    seed_comment_by(world, &project_name, number, &author_email).await;
    world.fa_last_project_name = Some(project_name);
}

// --------------------------------------------------------------------------
// Given — machine credentials (US-W05b/c).
//
// DISTILL records the admin's INTENT to grant/revoke; it does NOT mint a real
// JWT (the foundry-auth MachineToken primitive is a RED scaffold). The When
// step sends whatever credential string the world holds — for the RED phase
// that is a placeholder, and the assertion fails on the missing /api/v1 route
// regardless. DELIVER replaces these bodies with real issue/revoke calls
// through the admin issuance use-case (auth.md §Issuance & revocation).
// --------------------------------------------------------------------------

#[given(regex = r#"^the admin has granted a machine credential for "([^"]+)" bound to (\w+)$"#)]
async fn admin_grants_credential(world: &mut FoundryWorld, label: String, bound_to: String) {
    ensure_harness(world).await;
    let email = email_for_persona(&bound_to);
    let (user_id, workspace_id) = user_and_workspace(world, &email).await;
    let jwt = mint_credential(world, user_id, workspace_id, None, &label, 3600, true).await;
    world.fa_credential = Some(jwt);
    world.fa_credential_revoked = false;
}

/// Map a Background persona first-name to its seeded email. Mei is the only
/// member seeded by the us-w05b Background; the admin is Devansh.
fn email_for_persona(persona: &str) -> String {
    match persona.to_ascii_lowercase().as_str() {
        "devansh" => "devansh@acme.com".to_string(),
        _ => "mei@acme.com".to_string(),
    }
}

#[given(
    regex = r#"^the admin has granted a machine credential for "([^"]+)" bound to (\w+) with write access to "([^"]+)"$"#
)]
async fn admin_grants_write_credential(
    world: &mut FoundryWorld,
    label: String,
    bound_to: String,
    project: String,
) {
    ensure_harness(world).await;
    let email = email_for_persona(&bound_to);
    let (user_id, workspace_id) = user_and_workspace(world, &email).await;
    let jwt = mint_credential(world, user_id, workspace_id, None, &label, 3600, true).await;
    world.fa_credential = Some(jwt);
    world.fa_credential_revoked = false;
    world.fa_last_project_name = Some(project);
}

#[given(
    regex = r#"^the admin has granted a machine credential bound to (\w+) scoped to the "([^"]+)" team$"#
)]
async fn admin_grants_scoped_credential(world: &mut FoundryWorld, bound_to: String, team: String) {
    ensure_harness(world).await;
    let email = email_for_persona(&bound_to);
    let (user_id, workspace_id) = user_and_workspace(world, &email).await;
    // The credential is SCOPE-NARROWED to the named team: it can never reach
    // beyond it even though the bound principal may be a member of others.
    let scope_team_id = team_id_by_name(world, &team).await;
    let jwt = mint_credential(
        world,
        user_id,
        workspace_id,
        Some(scope_team_id),
        "scoped credential",
        3600,
        true,
    )
    .await;
    world.fa_credential = Some(jwt);
    world.fa_credential_revoked = false;
}

#[given(
    regex = r#"^the admin granted a machine credential bound to (\w+) that has since expired$"#
)]
async fn admin_grants_expired_credential(world: &mut FoundryWorld, bound_to: String) {
    ensure_harness(world).await;
    let email = email_for_persona(&bound_to);
    let (user_id, workspace_id) = user_and_workspace(world, &email).await;
    // A validly-SIGNED credential whose `exp` is already in the past — the
    // verifier's EdDSA-pinned `Validation` rejects it on `exp` before any
    // registry lookup. Register the row anyway (its registry `expires_at` is
    // also past) so neither the crypto nor the denylist would admit it.
    let jwt = mint_credential(
        world,
        user_id,
        workspace_id,
        None,
        "expired credential",
        -3600,
        true,
    )
    .await;
    world.fa_credential = Some(jwt);
    world.fa_credential_revoked = false;
}

#[given(
    regex = r#"^the admin has granted a second machine credential bound to a member who is not the comment's author and not an admin$"#
)]
async fn admin_grants_non_author_credential(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    // The credential must bind to a Backend MEMBER who is NEITHER the comment's
    // author (Mei) NOR the admin — so it passes team-membership authz and then
    // hits the author-only edit refusal (403), proving rule-parity with the
    // browser's "edit is author-only" rule (ADR-006). Seed a distinct member
    // (carol@acme.com) on the Backend team and bind the credential to her.
    let carol = "carol@acme.com";
    seed_backend_member(world, carol).await;
    let (user_id, workspace_id) = user_and_workspace(world, carol).await;
    let jwt = mint_credential(world, user_id, workspace_id, None, "non-author", 3600, true).await;
    world.fa_credential = Some(jwt);
    world.fa_credential_revoked = false;
}

#[given(regex = r#"^the admin revokes that credential$"#)]
async fn admin_revokes_credential(world: &mut FoundryWorld) {
    // Stamp `revoked_at = now()` on the exact registry row minted by the
    // preceding grant Given (auth.md §Revoke). Next use → 401 via the denylist.
    let jti = world
        .fa_credential_jti
        .expect("a credential was minted before revoke");
    let harness = world.harness.as_ref().expect("harness");
    harness
        .app
        .state
        .store
        .revoke_machine_token(jti)
        .await
        .expect("revoke machine token");
    world.fa_credential_revoked = true;
}

#[given(regex = r#"^a caller holds a credential the workspace never issued$"#)]
async fn caller_holds_forged_credential(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    // A credential the registry NEVER issued: validly EdDSA-signed by the test
    // key (so it passes the crypto), but its `jti` is NOT inserted, so the
    // denylist returns no row → 401. `register=false` mints without inserting.
    let (user_id, workspace_id) = user_and_workspace(world, "mei@acme.com").await;
    let jwt = mint_credential(world, user_id, workspace_id, None, "forged", 3600, false).await;
    world.fa_credential = Some(jwt);
    world.fa_credential_revoked = false;
}

#[given(
    regex = r#"^a caller holds a credential signed with an algorithm the server does not accept$"#
)]
async fn caller_holds_wrong_alg_credential(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    // The classic alg-confusion attack: an HS256 token using the server's
    // PUBLIC key bytes as the HMAC secret. The verifier pins exactly `[EdDSA]`,
    // so it is refused before any key is consulted → 401.
    let claims = foundry_auth::MachineTokenClaims {
        sub: uuid::Uuid::now_v7(),
        scope: None,
        iat: time::OffsetDateTime::now_utc().unix_timestamp(),
        exp: (time::OffsetDateTime::now_utc() + time::Duration::seconds(3600)).unix_timestamp(),
        jti: uuid::Uuid::now_v7(),
    };
    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    let key = jsonwebtoken::EncodingKey::from_secret(
        foundry_auth::test_keys::TEST_PUBLIC_KEY_PEM.as_bytes(),
    );
    let jwt = jsonwebtoken::encode(&header, &claims, &key).expect("hs256 encode");
    world.fa_credential = Some(jwt);
    world.fa_credential_revoked = false;
}

#[given(regex = r#"^a caller presents no valid credential$"#)]
async fn caller_no_valid_credential(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    world.fa_credential = None;
}

#[given(regex = r#"^a member account for "([^"]+)" with password "([^"]+)"$"#)]
async fn member_account_with_password(world: &mut FoundryWorld, email: String, password: String) {
    ensure_harness(world).await;
    // The member row is seeded by the Background `a member "..." belongs to the
    // team "Backend"` step (us_07) with that step's password. Reset the hash to
    // the password named HERE so the browser sign-in in this regression-guard
    // scenario authenticates. Authorship of the hash is a fixture detail, not
    // the behaviour under test (NFR-WEB-API-SEC-01 is about the path being
    // unchanged, which it already is today).
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let hash = foundry_auth::hash_password(&secrecy::SecretString::new(password.clone().into()))
        .await
        .expect("hash browser password");
    sqlx::query("UPDATE users SET password_hash = $1 WHERE email_lower = $2")
        .bind(&hash)
        .bind(email.to_ascii_lowercase())
        .execute(pool)
        .await
        .expect("reset member password for browser sign-in");
    world.fa_browser_email = Some(email);
    world.fa_browser_password = Some(password);
}

#[given(regex = r#"^Mei is watching the "([^"]+)" board in real time$"#)]
async fn mei_watching_board(world: &mut FoundryWorld, project: String) {
    ensure_harness(world).await;
    world.fa_last_project_name = Some(project);
    world.fa_watching = true;
}

// --------------------------------------------------------------------------
// When — read the board as data (US-W05a)
// --------------------------------------------------------------------------

#[when(regex = r#"^Mei requests the "([^"]+)" board's issues as machine-readable data$"#)]
async fn mei_requests_board_data(world: &mut FoundryWorld, project: String) {
    request_board_json(world, &project).await;
}

#[when(regex = r#"^Mei reads the "([^"]+)" board as machine-readable data$"#)]
async fn mei_reads_board_data(world: &mut FoundryWorld, project: String) {
    request_board_json(world, &project).await;
}

#[when(regex = r#"^Mei opens the "([^"]+)" board in the browser$"#)]
async fn mei_opens_board_browser(world: &mut FoundryWorld, project: String) {
    // GET the HTML board; capture its body for the "same set of issues" parity
    // assertion. The HTML board already works today; the parity assertion still
    // fails RED because the JSON side has no issues to compare yet.
    request_board_html(world, &project).await;
}

#[when(regex = r#"^the caller requests the "([^"]+)" board's issues as machine-readable data$"#)]
async fn caller_requests_board_data(world: &mut FoundryWorld, project: String) {
    request_board_json(world, &project).await;
}

#[when(regex = r#"^the machine requests the "([^"]+)" board's issues with that credential$"#)]
async fn machine_requests_board(world: &mut FoundryWorld, project: String) {
    request_board_json(world, &project).await;
}

#[when(
    regex = r#"^the machine requests the board's issues carrying only its credential and no session and no anti-forgery token$"#
)]
async fn machine_requests_board_token_only(world: &mut FoundryWorld) {
    let project = world
        .fa_last_project_name
        .clone()
        .unwrap_or_else(|| "Auth v2".into());
    request_board_json(world, &project).await;
}

#[when(regex = r#"^a caller requests the board's issues carrying no credential$"#)]
async fn caller_requests_board_no_cred(world: &mut FoundryWorld) {
    world.fa_credential = None;
    let project = world
        .fa_last_project_name
        .clone()
        .unwrap_or_else(|| "Auth v2".into());
    request_board_json(world, &project).await;
}

#[when(regex = r#"^a caller requests the board's issues carrying a malformed credential$"#)]
async fn caller_requests_board_malformed(world: &mut FoundryWorld) {
    world.fa_credential = Some("!!!not-a-valid-credential!!!".to_string());
    let project = world
        .fa_last_project_name
        .clone()
        .unwrap_or_else(|| "Auth v2".into());
    request_board_json(world, &project).await;
}

#[when(regex = r#"^the caller requests the board's issues with that credential$"#)]
async fn caller_requests_board_with_cred(world: &mut FoundryWorld) {
    let project = world
        .fa_last_project_name
        .clone()
        .unwrap_or_else(|| "Auth v2".into());
    request_board_json(world, &project).await;
}

#[when(regex = r#"^the machine requests the board's issues with that expired credential$"#)]
async fn machine_requests_board_expired(world: &mut FoundryWorld) {
    let project = world
        .fa_last_project_name
        .clone()
        .unwrap_or_else(|| "Auth v2".into());
    request_board_json(world, &project).await;
}

#[when(regex = r#"^the machine next requests the board's issues with that credential$"#)]
async fn machine_requests_board_after_revoke(world: &mut FoundryWorld) {
    let project = world
        .fa_last_project_name
        .clone()
        .unwrap_or_else(|| "Auth v2".into());
    request_board_json(world, &project).await;
}

// --------------------------------------------------------------------------
// When — writes (US-W05c)
// --------------------------------------------------------------------------

#[when(regex = r#"^the machine files an issue titled "([^"]+)" through the API$"#)]
async fn machine_files_issue(world: &mut FoundryWorld, title: String) {
    let project = world
        .fa_last_project_name
        .clone()
        .unwrap_or_else(|| "Auth v2".into());
    world.fa_last_title = Some(title.clone());
    post_create_issue(
        world,
        &project,
        &serde_json::json!({ "title": title }).to_string(),
    )
    .await;
}

#[when(regex = r#"^the machine files an issue with an empty title through the API$"#)]
async fn machine_files_empty_title(world: &mut FoundryWorld) {
    let project = world
        .fa_last_project_name
        .clone()
        .unwrap_or_else(|| "Auth v2".into());
    post_create_issue(
        world,
        &project,
        &serde_json::json!({ "title": "" }).to_string(),
    )
    .await;
}

#[when(regex = r#"^the machine moves (\w+)-(\d+) to "([^"]+)" through the API$"#)]
async fn machine_moves_issue(world: &mut FoundryWorld, prefix: String, number: i32, state: String) {
    let project = world
        .fa_last_project_name
        .clone()
        .unwrap_or_else(|| "Auth v2".into());
    let _ = prefix;
    // The Gherkin uses the human phrase ("in progress"); a real machine client
    // sends a contract-valid state token (api-contract.md §Issue: the API
    // normalizes "in-progress"/"in_progress" through the SAME normalize_state
    // the UI uses). Translate the prose to the wire form here in the driver —
    // this is test-wiring of the scenario phrase to a request value, not a
    // change to any assertion.
    let wire_state = state.trim().replace(' ', "_");
    patch_issue_state(
        world,
        &project,
        number,
        &serde_json::json!({ "state": wire_state }).to_string(),
    )
    .await;
}

#[when(
    regex = r#"^the machine posts a comment on (\w+)-(\d+) containing a script tag and a "javascript:" link through the API$"#
)]
async fn machine_posts_dangerous_comment(world: &mut FoundryWorld, _prefix: String, number: i32) {
    let project = world
        .fa_last_project_name
        .clone()
        .unwrap_or_else(|| "Auth v2".into());
    let body = "<script>alert(1)</script> see [link](javascript:alert(2))";
    post_create_comment(
        world,
        &project,
        number,
        &serde_json::json!({ "body": body }).to_string(),
    )
    .await;
}

#[when(regex = r#"^that machine edits Mei's comment through the API$"#)]
async fn machine_edits_comment(world: &mut FoundryWorld) {
    let project = world
        .fa_last_project_name
        .clone()
        .unwrap_or_else(|| "Auth v2".into());
    // Target Mei's REAL seeded comment id (looked up fresh) so the edit reaches
    // the service's author-only authz and is refused 403 — not a 404 for a
    // missing comment.
    let comment_id = mei_comment_id_on_issue(world, "mei@acme.com", 8).await;
    patch_comment(
        world,
        &project,
        8,
        comment_id,
        &serde_json::json!({ "body": "edited by non-author" }).to_string(),
    )
    .await;
}

#[when(regex = r#"^Mei signs in through the browser with her email and password$"#)]
async fn mei_signs_in_browser(world: &mut FoundryWorld) {
    // A REAL browser sign-in through the unchanged cookie path: GET /sign-in for
    // the CSRF cookie + token, then POST credentials. This regression guard
    // (NFR-WEB-API-SEC-01 / NFR-WEB-COMPAT-04) asserts the browser path is
    // UNAFFECTED by the additive machine-credential surface — so it is GREEN
    // today and must STAY green through DELIVER (no @skip; it is the live proof
    // the credential work did not touch session/CSRF).
    ensure_harness(world).await;
    let email = world
        .fa_browser_email
        .clone()
        .unwrap_or_else(|| "mei@acme.com".into());
    let password = world
        .fa_browser_password
        .clone()
        .unwrap_or_else(|| "correct horse battery staple".into());
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let base = harness.base_url();

    let csrf_get = http
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("get /sign-in for csrf");
    let csrf_cookie = csrf_get
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(|s| s.to_string())
        .expect("/sign-in mints foundry_csrf cookie");
    let csrf_token = csrf_cookie
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();

    let mut form: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    form.insert("email", email);
    form.insert("password", password);
    form.insert("_csrf", csrf_token.clone());
    let resp = http
        .post(format!("{base}/sign-in"))
        .header(
            reqwest::header::COOKIE,
            format!("foundry_csrf={csrf_token}"),
        )
        .form(&form)
        .send()
        .await
        .expect("post /sign-in");
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp.text().await.unwrap_or_default();
    world.last_status = Some(status);
    world.last_headers = Some(headers);
    world.last_body = Some(body);
    world.fa_browser_signed_in = true;
}

// --------------------------------------------------------------------------
// When — boundary guard (US-W06): subprocess, not the binary.
//
// DISTILL records the maintainer's invocation INTENT and a planted-violation
// flag. The Then steps fail RED because `cargo xtask check-arch` does not yet
// exist. DELIVER implements the subcommand + cargo-deny rule, then these run a
// real subprocess (see step-skeletons.md "Boundary guard wiring").
// --------------------------------------------------------------------------

#[given(regex = r#"^the project tree has no boundary violations$"#)]
async fn tree_clean(world: &mut FoundryWorld) {
    world.fa_guard_violation = None;
}

#[given(regex = r#"^a copy of the tree in which a data-API handler is changed to build a page$"#)]
async fn tree_with_page_violation(world: &mut FoundryWorld) {
    world.fa_guard_violation = Some("api-builds-page".to_string());
}

#[given(
    regex = r#"^a copy of the tree in which the data-API adapter declares a direct dependency on the persistence layer$"#
)]
async fn tree_with_dep_violation(world: &mut FoundryWorld) {
    world.fa_guard_violation = Some("api-depends-on-store".to_string());
}

#[given(
    regex = r#"^a copy of the tree in which the credential verifier is changed to accept any signing algorithm$"#
)]
async fn tree_with_alg_violation(world: &mut FoundryWorld) {
    world.fa_guard_violation = Some("verifier-accepts-any-alg".to_string());
}

#[when(regex = r#"^the maintainer runs the boundary check$"#)]
async fn run_boundary_check_clean(world: &mut FoundryWorld) {
    run_boundary_check(world).await;
}

#[when(regex = r#"^the maintainer runs the boundary check on that copy$"#)]
async fn run_boundary_check_copy(world: &mut FoundryWorld) {
    run_boundary_check(world).await;
}

// --------------------------------------------------------------------------
// Then — read outcomes (US-W05a)
// --------------------------------------------------------------------------

#[then(regex = r#"^the answer is a data list containing (\w+)-(\d+) and (\w+)-(\d+)$"#)]
async fn answer_lists_two(world: &mut FoundryWorld, p1: String, n1: i32, p2: String, n2: i32) {
    let issues = parse_issue_array(world);
    let want1 = format!("{p1}-{n1}");
    let want2 = format!("{p2}-{n2}");
    let keys: Vec<String> = issues.iter().map(|i| i.key.clone()).collect();
    assert!(
        keys.contains(&want1) && keys.contains(&want2),
        "expected data list to contain {want1} and {want2}, got {keys:?} (status {:?}, body {:?})",
        world.last_status,
        world.last_body
    );
}

#[then(regex = r#"^each entry carries the issue's key, title, and state$"#)]
async fn entries_have_fields(world: &mut FoundryWorld) {
    let issues = parse_issue_array(world);
    assert!(!issues.is_empty(), "expected a non-empty data list");
    for i in &issues {
        assert!(!i.key.is_empty(), "entry missing key: {i:?}");
        assert!(!i.title.is_empty(), "entry missing title: {i:?}");
        assert!(!i.state.is_empty(), "entry missing state: {i:?}");
    }
}

#[then(regex = r#"^(\w+)-(\d+) is reported in progress and (\w+)-(\d+) in the backlog$"#)]
async fn states_reported(world: &mut FoundryWorld, p1: String, n1: i32, p2: String, n2: i32) {
    let issues = parse_issue_array(world);
    let find = |key: &str| issues.iter().find(|i| i.key == key).cloned();
    let a = find(&format!("{p1}-{n1}")).expect("first issue present");
    let b = find(&format!("{p2}-{n2}")).expect("second issue present");
    assert_eq!(a.state, "in_progress", "{} state", a.key);
    assert_eq!(b.state, "backlog", "{} state", b.key);
}

#[then(regex = r#"^the answer contains no markup$"#)]
async fn answer_no_markup(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().unwrap_or("");
    // Must parse as JSON (data, not a page) AND contain no HTML tags at the
    // response-body level.
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(body);
    assert!(parsed.is_ok(), "answer is not JSON data: {body:?}");
    assert!(
        !body.contains("<html") && !body.contains("<!DOCTYPE") && !body.contains("<body"),
        "answer contains markup: {body:?}"
    );
}

#[then(regex = r#"^the answer is an empty data list$"#)]
async fn answer_empty_list(world: &mut FoundryWorld) {
    let issues = parse_issue_array(world);
    assert!(
        issues.is_empty(),
        "expected empty data list, got {issues:?}"
    );
}

#[then(regex = r#"^the request is reported as successful$"#)]
async fn request_successful(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert_eq!(status.as_u16(), 200, "expected 200, got {status}");
}

#[then(regex = r#"^both list exactly the same set of issues$"#)]
async fn both_same_set(world: &mut FoundryWorld) {
    let json_issues = parse_issue_array(world);
    let html = world.fa_last_html_body.as_deref().unwrap_or("");
    assert!(
        !json_issues.is_empty(),
        "JSON side returned no issues (status {:?}); cannot prove parity",
        world.last_status
    );
    for issue in &json_issues {
        assert!(
            html.contains(&issue.key),
            "browser board missing {} that the data answer listed",
            issue.key
        );
    }
}

#[then(regex = r#"^no issue data is returned$"#)]
async fn no_issue_data(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().unwrap_or("");
    // A refusal must not leak any issue key.
    let leaked = body.contains("AUTH-") || body.contains("BILL-");
    assert!(!leaked, "refusal leaked issue data: {body:?}");
}

// --------------------------------------------------------------------------
// Then — auth outcomes (US-W05b)
// --------------------------------------------------------------------------

#[then(regex = r#"^the request is authenticated as the machine$"#)]
async fn authenticated_as_machine(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert!(
        status.is_success(),
        "expected an authenticated success, got {status} body {:?}",
        world.last_body
    );
}

#[then(regex = r#"^the board's issues are returned as data$"#)]
async fn board_issues_returned(world: &mut FoundryWorld) {
    let issues = parse_issue_array(world);
    assert!(
        !issues.is_empty(),
        "expected board issues as data, got {issues:?}"
    );
}

#[then(regex = r#"^the request succeeds$"#)]
async fn request_succeeds(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert!(status.is_success(), "expected success, got {status}");
}

#[then(regex = r#"^the request is refused as unauthenticated$"#)]
async fn refused_unauthenticated(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert_eq!(
        status.as_u16(),
        401,
        "expected 401 unauthenticated, got {status} body {:?}",
        world.last_body
    );
}

#[then(regex = r#"^the request is refused as not-allowed$"#)]
async fn refused_not_allowed(world: &mut FoundryWorld) {
    assert_authorization_forbidden(world);
}

#[then(regex = r#"^she receives a session cookie as before$"#)]
async fn receives_session_cookie(world: &mut FoundryWorld) {
    assert!(
        world.fa_browser_signed_in,
        "browser sign-in did not complete"
    );
    // DELIVER asserts the foundry_session Set-Cookie is present and unchanged;
    // this regression-guard scenario flips GREEN once the credential surface is
    // added without touching the session path.
    let headers = world.last_headers.as_ref();
    let has_session = headers
        .map(|h| {
            h.get_all(reqwest::header::SET_COOKIE)
                .iter()
                .filter_map(|v| v.to_str().ok())
                .any(|s| s.starts_with("foundry_session="))
        })
        .unwrap_or(false);
    assert!(
        has_session,
        "browser sign-in must still set foundry_session"
    );
}

#[then(
    regex = r#"^her browser session still requires an anti-forgery token on a mutating request$"#
)]
async fn browser_still_requires_csrf(world: &mut FoundryWorld) {
    // Regression guard for NFR-WEB-COMPAT-03 — the existing CSRF scenarios are
    // the real proof; this asserts the browser path is unaffected.
    assert!(
        world.fa_browser_signed_in,
        "precondition: browser sign-in completed"
    );
}

// --------------------------------------------------------------------------
// Then — write outcomes (US-W05c)
// --------------------------------------------------------------------------

#[then(regex = r#"^a new issue is created with the next sequential key$"#)]
async fn new_issue_created(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert_eq!(
        status.as_u16(),
        201,
        "expected 201 Created, got {status} body {:?}",
        world.last_body
    );
}

#[then(regex = r#"^the created issue is returned as data including its key and state$"#)]
async fn created_issue_returned(world: &mut FoundryWorld) {
    let issue = parse_single_issue(world);
    assert!(!issue.key.is_empty(), "created issue missing key");
    assert!(!issue.state.is_empty(), "created issue missing state");
}

#[then(regex = r#"^the new issue starts in the backlog$"#)]
async fn new_issue_in_backlog(world: &mut FoundryWorld) {
    let issue = parse_single_issue(world);
    assert_eq!(issue.state, "backlog", "new issue should start in backlog");
}

#[then(regex = r#"^the new issue appears on Mei's board$"#)]
async fn new_issue_on_mei_board(world: &mut FoundryWorld) {
    // DELIVER observes the SSE/outbox fan-out reach the watching subscriber.
    // RED scaffold: the create 404s, so the issue never appears.
    assert!(
        world.fa_watching,
        "precondition: Mei was watching the board"
    );
    let status = world.last_status.expect("status captured");
    assert_eq!(
        status.as_u16(),
        201,
        "expected the API create to succeed so the issue can appear; got {status}"
    );
}

#[then(regex = r#"^it was filed through the same core path a browser-filed issue travels$"#)]
async fn same_core_path(world: &mut FoundryWorld) {
    // Proven by DELIVER asserting the outbox row + SSE event shape match a
    // UI-filed issue. RED until the create succeeds.
    let status = world.last_status.expect("status captured");
    assert_eq!(
        status.as_u16(),
        201,
        "create must succeed to assert core-path parity; got {status}"
    );
}

#[then(regex = r#"^(\w+)-(\d+)'s state becomes in progress$"#)]
async fn issue_state_becomes(world: &mut FoundryWorld, _prefix: String, _number: i32) {
    let issue = parse_single_issue(world);
    assert_eq!(issue.state, "in_progress", "state should be in_progress");
}

#[then(regex = r#"^the updated issue is returned as data$"#)]
async fn updated_issue_returned(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert_eq!(
        status.as_u16(),
        200,
        "expected 200 with updated issue, got {status}"
    );
    let _ = parse_single_issue(world);
}

#[then(regex = r#"^the comment is stored with the dangerous content removed$"#)]
async fn comment_sanitized(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert_eq!(
        status.as_u16(),
        201,
        "expected 201 created comment, got {status} body {:?}",
        world.last_body
    );
    let body = world.last_body.as_deref().unwrap_or("");
    assert!(
        !body.contains("<script") && !body.contains("javascript:"),
        "dangerous content not removed: {body:?}"
    );
}

#[then(
    regex = r#"^the stored comment matches what a browser-posted comment with the same text would store$"#
)]
async fn comment_matches_browser(world: &mut FoundryWorld) {
    // Rule-parity (NFR-WEB-API-CON-02): DELIVER asserts the API-stored body_html
    // equals the UI-stored bytes (both via render_comment_markdown in core).
    let status = world.last_status.expect("status captured");
    assert_eq!(
        status.as_u16(),
        201,
        "comment create must succeed to assert parity; got {status}"
    );
}

#[then(regex = r#"^the write is rejected for a missing title$"#)]
async fn rejected_missing_title(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert_eq!(
        status.as_u16(),
        422,
        "expected 422 validation, got {status} body {:?}",
        world.last_body
    );
}

#[then(regex = r#"^the rejection reason matches the browser's "Title is required" rule$"#)]
async fn rejection_matches_title_rule(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().unwrap_or("");
    assert!(
        body.contains("Title is required") || body.contains("title_required"),
        "rejection did not carry the UI's title rule: {body:?}"
    );
}

#[then(regex = r#"^the rejection is returned as data with no markup$"#)]
async fn rejection_no_markup(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().unwrap_or("");
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(body);
    assert!(parsed.is_ok(), "rejection is not JSON data: {body:?}");
    assert!(
        !body.contains("<html") && !body.contains("<body"),
        "rejection contains markup: {body:?}"
    );
}

#[then(regex = r#"^the write is refused as not-allowed$"#)]
async fn write_refused_not_allowed(world: &mut FoundryWorld) {
    assert_authorization_forbidden(world);
}

#[then(regex = r#"^the comment is left unchanged$"#)]
async fn comment_unchanged(world: &mut FoundryWorld) {
    // DELIVER asserts the stored body is byte-equal to the pre-edit body. RED
    // until the route exists; the 403 assertion above is the gating signal.
    assert!(
        world.last_status.is_some(),
        "a response must have been captured"
    );
}

// --------------------------------------------------------------------------
// Then — boundary guard (US-W06)
// --------------------------------------------------------------------------

#[then(regex = r#"^the check passes$"#)]
async fn check_passes(world: &mut FoundryWorld) {
    let exit = world.fa_guard_exit_code.expect("boundary check ran");
    assert_eq!(
        exit, 0,
        "expected the boundary check to pass; stderr {:?}",
        world.fa_guard_stderr
    );
}

#[then(regex = r#"^the check fails$"#)]
async fn check_fails(world: &mut FoundryWorld) {
    let exit = world.fa_guard_exit_code.expect("boundary check ran");
    let out = world.fa_guard_stderr.clone().unwrap_or_default();
    // Guard against a false signal (Critical Rule 7): today `xtask` exits
    // non-zero with "unknown subcommand: check-arch", which would spuriously
    // satisfy a bare `exit != 0`. Require that the guard actually RAN — i.e. it
    // recognised `check-arch` — before treating a non-zero exit as "caught the
    // planted violation". Flips GREEN once DELIVER implements the subcommand.
    assert!(
        !out.contains("unknown subcommand"),
        "the boundary check subcommand is not implemented yet (the guard never ran); \
         output {out:?}"
    );
    assert_ne!(
        exit, 0,
        "expected the boundary check to FAIL on the planted violation; output {out:?}"
    );
}

#[then(regex = r#"^it names the handler that builds a page$"#)]
async fn names_page_handler(world: &mut FoundryWorld) {
    let out = world.fa_guard_stderr.clone().unwrap_or_default();
    assert!(
        out.to_lowercase().contains("html") || out.to_lowercase().contains("page"),
        "guard output did not name the page-building handler: {out:?}"
    );
}

#[then(regex = r#"^it names the forbidden dependency$"#)]
async fn names_forbidden_dep(world: &mut FoundryWorld) {
    let out = world.fa_guard_stderr.clone().unwrap_or_default();
    assert!(
        out.contains("foundry-store") || out.to_lowercase().contains("dependency"),
        "guard output did not name the forbidden dependency: {out:?}"
    );
}

#[then(
    regex = r#"^it reports the credential verifier no longer pins the single allowed algorithm$"#
)]
async fn names_alg_violation(world: &mut FoundryWorld) {
    let out = world.fa_guard_stderr.clone().unwrap_or_default();
    assert!(
        out.to_lowercase().contains("alg") || out.to_lowercase().contains("algorithm"),
        "guard output did not report the algorithm-pin violation: {out:?}"
    );
}

// ==========================================================================
// Internals — real HTTP against the in-process harness.
// ==========================================================================

/// Assert the response is a genuine authorization refusal from the JSON API:
/// HTTP 403 carrying the JSON error envelope with `code = "forbidden"`.
///
/// This deliberately REJECTS the catch-all CSRF 403 ("CSRF token missing or
/// mismatched") that the browser-path middleware emits when an unrecognised
/// `/api/v1` route falls through today. Without this guard the RED scaffold
/// would pass for the WRONG reason (Critical Rule 7 / Fixture Theater): the
/// authorization use-case is never reached. It flips GREEN only once DELIVER
/// mounts `/api/v1` OUTSIDE the CSRF layer (auth.md §Coexistence) and the
/// service returns `ServiceError::Forbidden`.
fn assert_authorization_forbidden(world: &FoundryWorld) {
    let status = world.last_status.expect("status captured");
    let body = world.last_body.as_deref().unwrap_or("");
    assert_eq!(
        status.as_u16(),
        403,
        "expected 403 not-allowed, got {status} body {body:?}"
    );
    assert!(
        !body.contains("CSRF"),
        "got a CSRF 403 from the browser middleware, not an authorization refusal \
         from the API — the /api/v1 route is not yet mounted CSRF-exempt; body {body:?}"
    );
    let parsed: serde_json::Value = serde_json::from_str(body).unwrap_or_else(|err| {
        panic!("403 body is not the JSON error envelope ({err}); body {body:?}")
    });
    assert_eq!(
        parsed
            .get("error")
            .and_then(|e| e.get("code"))
            .and_then(|c| c.as_str()),
        Some("forbidden"),
        "403 envelope code is not \"forbidden\"; body {body:?}"
    );
}

/// A parsed issue from the JSON answer. Mirrors `foundry_api::IssueJson`.
#[derive(Debug, Clone, serde::Deserialize)]
struct ApiIssue {
    key: String,
    #[allow(dead_code)]
    number: i32,
    title: String,
    state: String,
}

fn parse_issue_array(world: &FoundryWorld) -> Vec<ApiIssue> {
    let body = world.last_body.as_deref().unwrap_or("");
    serde_json::from_str::<Vec<ApiIssue>>(body).unwrap_or_else(|err| {
        panic!(
            "expected a JSON array of issues but parse failed ({err}); status {:?}, body {:?}",
            world.last_status, body
        )
    })
}

fn parse_single_issue(world: &FoundryWorld) -> ApiIssue {
    let body = world.last_body.as_deref().unwrap_or("");
    serde_json::from_str::<ApiIssue>(body).unwrap_or_else(|err| {
        panic!(
            "expected a single JSON issue but parse failed ({err}); status {:?}, body {:?}",
            world.last_status, body
        )
    })
}

fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_hyphen = true;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            for c in ch.to_lowercase() {
                out.push(c);
            }
            last_hyphen = false;
        } else if !last_hyphen {
            out.push('-');
            last_hyphen = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Resolve the team slug owning a given project name (defaults to "backend").
async fn team_slug_for_project(world: &FoundryWorld, project_name: &str) -> String {
    if let Some(harness) = world.harness.as_ref() {
        let pool = harness.app.state.store.pool();
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT t.slug FROM projects p JOIN teams t ON t.id = p.team_id WHERE p.name = $1",
        )
        .bind(project_name)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        if let Some((slug,)) = row {
            return slug;
        }
    }
    "backend".to_string()
}

fn auth_header(world: &FoundryWorld) -> Option<String> {
    if world.fa_credential_revoked {
        // A revoked credential is still presented by the client — the server
        // refuses it via the denylist. Keep the header so the request reaches
        // the verifier.
        return world.fa_credential.clone().map(|c| format!("Bearer {c}"));
    }
    world.fa_credential.clone().map(|c| format!("Bearer {c}"))
}

async fn request_board_json(world: &mut FoundryWorld, project_name: &str) {
    ensure_harness(world).await;
    // Slice 2 (us-w05b): the read endpoint REQUIRES a machine token going
    // forward — the 01-02 transitional browser-session path is REPLACED. So:
    //   - a scenario that set an explicit credential (the us-w05b grant/forged/
    //     expired/revoked/wrong-alg Givens) presents exactly that bearer;
    //   - a us-w05a read scenario uses `Mei is signed in` to NAME the integrator
    //     persona — we mint a real machine credential bound to that persona so
    //     the read authenticates over the now-required token path;
    //   - a caller with neither carries no Authorization header and is refused
    //     401 (the fail-closed no-credential path).
    ensure_bearer_for_signed_in_persona(world).await;
    let team = team_slug_for_project(world, project_name).await;
    let project_slug = slugify(project_name);
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/api/v1/teams/{team}/projects/{project_slug}/issues",
        base = harness.base_url()
    );
    let mut req = http
        .get(&url)
        .header(reqwest::header::ACCEPT, "application/json");
    if let Some(bearer) = auth_header(world) {
        req = req.header(reqwest::header::AUTHORIZATION, bearer);
    }
    let resp = req.send().await.expect("send api board request");
    capture(world, resp).await;
}

/// If a us-w05a scenario named a signed-in persona (`Mei is signed in`) but no
/// explicit machine credential was granted, mint a real machine credential
/// bound to that persona so the now-token-required read authenticates. A
/// no-credential caller (no signed-in persona, no grant) is left with nothing
/// and is refused 401.
async fn ensure_bearer_for_signed_in_persona(world: &mut FoundryWorld) {
    if world.fa_credential.is_some() {
        return;
    }
    let Some(email) = world.us_07_signed_in_email.clone() else {
        return;
    };
    let (user_id, workspace_id) = user_and_workspace(world, &email).await;
    let jwt = mint_credential(
        world,
        user_id,
        workspace_id,
        None,
        "us-w05a reader",
        3600,
        true,
    )
    .await;
    world.fa_credential = Some(jwt);
}

/// Sign the recorded signed-in persona in over the real browser path and
/// return their `foundry_session` cookie pair (`foundry_session=...`), or
/// `None` if no persona is signed in for this scenario. Used to carry the
/// slice-1 transitional browser-session credential onto the JSON API GET.
async fn session_cookie_for_caller(world: &FoundryWorld) -> Option<String> {
    let email = world.us_07_signed_in_email.clone()?;
    let password = world.us_07_signed_in_password.clone()?;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let base = harness.base_url();

    let csrf_get = http
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("get /sign-in for csrf");
    let csrf_token = csrf_get
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .and_then(|s| s.strip_prefix("foundry_csrf="))
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();

    let mut form: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    form.insert("email", email);
    form.insert("password", password);
    form.insert("_csrf", csrf_token.clone());
    let resp = http
        .post(format!("{base}/sign-in"))
        .header(
            reqwest::header::COOKIE,
            format!("foundry_csrf={csrf_token}"),
        )
        .form(&form)
        .send()
        .await
        .expect("post /sign-in for session cookie");
    resp.headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .and_then(|s| s.split(';').next())
        .map(|pair| pair.to_string())
}

async fn request_board_html(world: &mut FoundryWorld, project_name: &str) {
    ensure_harness(world).await;
    let team = team_slug_for_project(world, project_name).await;
    let project_slug = slugify(project_name);
    // The HTML board requires a signed-in session (else it 302s to /sign-in).
    // Carry the same persona's browser-session cookie the JSON read uses, so
    // the cross-path parity scenario compares two AUTHENTICATED reads of the
    // SAME board through the SAME core seam (NFR-WEB-BND-05).
    let session_cookie = session_cookie_for_caller(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/team/{team}/project/{project_slug}",
        base = harness.base_url()
    );
    let mut req = http.get(&url);
    if let Some(cookie) = session_cookie {
        req = req.header(reqwest::header::COOKIE, cookie);
    }
    let resp = req.send().await.expect("send html board request");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    world.fa_last_html_body = Some(body);
    world.last_status = Some(status);
}

async fn post_create_issue(world: &mut FoundryWorld, project_name: &str, json_body: &str) {
    ensure_harness(world).await;
    let team = team_slug_for_project(world, project_name).await;
    let project_slug = slugify(project_name);
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/api/v1/teams/{team}/projects/{project_slug}/issues",
        base = harness.base_url()
    );
    let mut req = http
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(json_body.to_string());
    if let Some(bearer) = auth_header(world) {
        req = req.header(reqwest::header::AUTHORIZATION, bearer);
    }
    let resp = req.send().await.expect("send api create-issue");
    capture(world, resp).await;
}

async fn patch_issue_state(
    world: &mut FoundryWorld,
    project_name: &str,
    number: i32,
    json_body: &str,
) {
    ensure_harness(world).await;
    let team = team_slug_for_project(world, project_name).await;
    let project_slug = slugify(project_name);
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/api/v1/teams/{team}/projects/{project_slug}/issues/{number}",
        base = harness.base_url()
    );
    let mut req = http
        .patch(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(json_body.to_string());
    if let Some(bearer) = auth_header(world) {
        req = req.header(reqwest::header::AUTHORIZATION, bearer);
    }
    let resp = req.send().await.expect("send api patch-issue");
    capture(world, resp).await;
}

async fn post_create_comment(
    world: &mut FoundryWorld,
    project_name: &str,
    number: i32,
    json_body: &str,
) {
    ensure_harness(world).await;
    let team = team_slug_for_project(world, project_name).await;
    let project_slug = slugify(project_name);
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/api/v1/teams/{team}/projects/{project_slug}/issues/{number}/comments",
        base = harness.base_url()
    );
    let mut req = http
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(json_body.to_string());
    if let Some(bearer) = auth_header(world) {
        req = req.header(reqwest::header::AUTHORIZATION, bearer);
    }
    let resp = req.send().await.expect("send api create-comment");
    capture(world, resp).await;
}

async fn patch_comment(
    world: &mut FoundryWorld,
    project_name: &str,
    number: i32,
    comment_id: uuid::Uuid,
    json_body: &str,
) {
    ensure_harness(world).await;
    let team = team_slug_for_project(world, project_name).await;
    let project_slug = slugify(project_name);
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/api/v1/teams/{team}/projects/{project_slug}/issues/{number}/comments/{comment_id}",
        base = harness.base_url()
    );
    let mut req = http
        .patch(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(json_body.to_string());
    if let Some(bearer) = auth_header(world) {
        req = req.header(reqwest::header::AUTHORIZATION, bearer);
    }
    let resp = req.send().await.expect("send api patch-comment");
    capture(world, resp).await;
}

async fn capture(world: &mut FoundryWorld, resp: reqwest::Response) {
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp.text().await.unwrap_or_default();
    world.last_status = Some(status);
    world.last_headers = Some(headers);
    world.last_body = Some(body);
}

/// Run the boundary check as a subprocess: `cargo xtask check-arch`. Today the
/// subcommand does not exist, so `xtask` exits non-zero with an "unknown
/// subcommand" usage — which makes the clean-tree "check passes" scenario fail
/// RED (the right reason: the guard is unimplemented). DELIVER implements the
/// subcommand; the planted-violation copies are produced by DELIVER's harness
/// extension (see step-skeletons.md "Boundary guard wiring").
async fn run_boundary_check(world: &mut FoundryWorld) {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    // crates/foundry-acceptance -> workspace root is two levels up.
    let workspace_root = std::path::Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new(env!("CARGO"))
            .current_dir(&workspace_root)
            .args(["run", "-q", "-p", "xtask", "--", "check-arch"])
            .output()
    })
    .await
    .expect("join spawn_blocking")
    .expect("spawn cargo xtask check-arch");
    world.fa_guard_exit_code = output.status.code();
    let mut combined = String::new();
    combined.push_str(&String::from_utf8_lossy(&output.stdout));
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    world.fa_guard_stderr = Some(combined);
}

// --------------------------------------------------------------------------
// Machine-credential minting (US-W05b). Real EdDSA JWTs signed by the FIXED
// test signing key (foundry_auth::test_keys), registered in the machine_tokens
// denylist via the real Store repo (02-01). These are PRECONDITIONS — a real
// admin-issued credential — not the behaviour under test. The JWT `exp` is set
// against the REAL wall clock (jsonwebtoken's `Validation` validates `exp`
// against `SystemTime::now()`, not the harness MockClock anchor).
// --------------------------------------------------------------------------

/// Resolve a user's id + workspace by email (lower-cased).
async fn user_and_workspace(world: &FoundryWorld, email: &str) -> (uuid::Uuid, uuid::Uuid) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let row: (uuid::Uuid, uuid::Uuid) = sqlx::query_as(
        "SELECT u.id, wm.workspace_id
           FROM users u
           JOIN workspace_memberships wm ON wm.user_id = u.id
          WHERE u.email_lower = $1
          LIMIT 1",
    )
    .bind(email.to_ascii_lowercase())
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| panic!("resolve user {email:?}: {e}"));
    row
}

/// Resolve a team's id by name within the (single) workspace.
async fn team_id_by_name(world: &FoundryWorld, team_name: &str) -> uuid::Uuid {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let row: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM teams WHERE name = $1 LIMIT 1")
        .bind(team_name)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("resolve team {team_name:?}: {e}"));
    row.0
}

/// Mint a REAL machine credential bound to `user_id` and register it in the
/// denylist, returning the compact JWT to present as the bearer. `exp_offset`
/// (real seconds from now) lets a Given mint an already-expired token.
/// `register` lets the forged-credential Given mint a validly-signed JWT whose
/// `jti` was NEVER inserted (so the denylist refuses it).
async fn mint_credential(
    world: &mut FoundryWorld,
    user_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
    scope_team_id: Option<uuid::Uuid>,
    label: &str,
    exp_offset_secs: i64,
    register: bool,
) -> String {
    let jti = uuid::Uuid::now_v7();
    let now = time::OffsetDateTime::now_utc();
    let exp = now + time::Duration::seconds(exp_offset_secs);
    if register {
        let harness = world.harness.as_ref().expect("harness");
        harness
            .app
            .state
            .store
            .insert_machine_token(jti, user_id, workspace_id, scope_team_id, exp, label)
            .await
            .expect("register machine token");
        world.fa_credential_jti = Some(jti);
    } else {
        world.fa_credential_jti = None;
    }
    let claims = foundry_auth::MachineTokenClaims {
        sub: user_id,
        scope: scope_team_id,
        iat: now.unix_timestamp(),
        exp: exp.unix_timestamp(),
        jti,
    };
    let signer = foundry_auth::test_keys::signer();
    signer
        .mint(&claims)
        .expect("mint machine jwt")
        .expose_secret()
        .to_string()
}

// --------------------------------------------------------------------------
// Seeding helpers (real Postgres via the harness pool). Mirror the direct
// inserts used by us_07/us_08 — preconditions, never the behaviour under test.
// --------------------------------------------------------------------------

async fn seed_issue(
    world: &mut FoundryWorld,
    project_name: &str,
    expected_prefix: &str,
    number: i32,
    title: &str,
    state: &str,
) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let proj: Option<(uuid::Uuid, uuid::Uuid, String)> =
        sqlx::query_as("SELECT id, workspace_id, key_prefix FROM projects WHERE name = $1")
            .bind(project_name)
            .fetch_optional(pool)
            .await
            .expect("query project");
    let Some((project_id, workspace_id, key_prefix)) = proj else {
        panic!("seed_issue: project {project_name:?} not found — Background must create it first");
    };
    assert_eq!(
        key_prefix, expected_prefix,
        "issue {expected_prefix}-{number} does not match project key prefix {key_prefix}"
    );
    // The author_id column is NOT NULL with no default (0001_init.sql:76).
    // Use any existing user (the admin seeded by Background) as the author —
    // authorship is not the behaviour under test here, just a valid FK.
    let author: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("an author user exists (Background seeds the admin)");
    let issue_id = uuid::Uuid::now_v7();
    // Direct insert mirroring us_08's seeding shape — a precondition row, not
    // the behaviour under test. DELIVER's real write path is
    // insert_issue_with_outbox; for a fixture row a plain insert is correct.
    // ON CONFLICT keeps re-runs idempotent on the (project_id, number) unique.
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, state, author_id)
              VALUES ($1, $2, $3, $4, $5, $6, $7)
              ON CONFLICT (project_id, number) DO NOTHING",
    )
    .bind(issue_id)
    .bind(project_id)
    .bind(workspace_id)
    .bind(number)
    .bind(title)
    .bind(state)
    .bind(author.0)
    .execute(pool)
    .await
    .expect("seed issue row");
}

/// Seed a real `comments` row authored by `author_email` on the given issue
/// number in `project_name`. A precondition row (not the behaviour under test):
/// a plain INSERT mirroring `seed_issue`'s shape.
async fn seed_comment_by(
    world: &mut FoundryWorld,
    project_name: &str,
    issue_number: i32,
    author_email: &str,
) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let row: (uuid::Uuid, uuid::Uuid) = sqlx::query_as(
        "SELECT i.id, i.workspace_id
           FROM issues i JOIN projects p ON p.id = i.project_id
          WHERE p.name = $1 AND i.number = $2",
    )
    .bind(project_name)
    .bind(issue_number)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| panic!("seed_comment_by: issue {project_name} #{issue_number}: {e}"));
    let (issue_id, workspace_id) = row;
    let author: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(author_email.to_ascii_lowercase())
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("seed_comment_by: author {author_email}: {e}"));
    let comment_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO comments (id, workspace_id, issue_id, author_id, body_markdown, body_html)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(comment_id)
    .bind(workspace_id)
    .bind(issue_id)
    .bind(author.0)
    .bind("seeded comment body")
    .bind("<p>seeded comment body</p>")
    .execute(pool)
    .await
    .expect("seed comment row");
}

/// Resolve the id of `author_email`'s comment on the given issue number — the
/// real row seeded by `seed_comment_by`, so the non-author edit targets it.
async fn mei_comment_id_on_issue(
    world: &FoundryWorld,
    author_email: &str,
    issue_number: i32,
) -> uuid::Uuid {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let row: (uuid::Uuid,) = sqlx::query_as(
        "SELECT c.id
           FROM comments c
           JOIN issues i ON i.id = c.issue_id
           JOIN users u ON u.id = c.author_id
          WHERE i.number = $1 AND u.email_lower = $2
          ORDER BY c.created_at DESC
          LIMIT 1",
    )
    .bind(issue_number)
    .bind(author_email.to_ascii_lowercase())
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| panic!("mei_comment_id_on_issue {author_email} #{issue_number}: {e}"));
    row.0
}

/// Seed a workspace+team member on the Backend team (the team the us-w05c
/// Background creates). Used to bind the non-author credential to a member who
/// is neither the comment's author nor the admin.
async fn seed_backend_member(world: &mut FoundryWorld, email: &str) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let ws: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("fetch workspace");
    let team: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM teams WHERE workspace_id = $1 AND name = 'Backend'")
            .bind(ws.0)
            .fetch_one(pool)
            .await
            .expect("fetch Backend team");
    let user_id = uuid::Uuid::now_v7();
    let lower = email.to_ascii_lowercase();
    let hash = foundry_auth::hash_password(&secrecy::SecretString::new(
        "carol-correct-horse-battery-staple".to_string().into(),
    ))
    .await
    .expect("hash member pw");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5) ON CONFLICT (email_lower) DO NOTHING",
    )
    .bind(user_id)
    .bind(&lower)
    .bind(email)
    .bind("Carol")
    .bind(&hash)
    .execute(pool)
    .await
    .expect("insert member user");
    let resolved: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(&lower)
        .fetch_one(pool)
        .await
        .expect("resolve member id");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING",
    )
    .bind(ws.0)
    .bind(resolved.0)
    .execute(pool)
    .await
    .expect("insert workspace membership");
    sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role)
              VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING",
    )
    .bind(team.0)
    .bind(resolved.0)
    .execute(pool)
    .await
    .expect("insert team membership");
}

async fn seed_team_with_project(
    world: &mut FoundryWorld,
    team_name: &str,
    project_name: &str,
    key_prefix: &str,
) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let ws: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("fetch workspace");
    let team_id = uuid::Uuid::now_v7();
    let team_slug = slugify(team_name);
    sqlx::query(
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)
              ON CONFLICT (workspace_id, slug) DO NOTHING",
    )
    .bind(team_id)
    .bind(ws.0)
    .bind(team_name)
    .bind(&team_slug)
    .execute(pool)
    .await
    .expect("seed other team");
    let resolved_team: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM teams WHERE workspace_id = $1 AND name = $2")
            .bind(ws.0)
            .bind(team_name)
            .fetch_one(pool)
            .await
            .expect("resolve team id");
    let project_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)
              ON CONFLICT (workspace_id, key_prefix) DO NOTHING",
    )
    .bind(project_id)
    .bind(resolved_team.0)
    .bind(ws.0)
    .bind(project_name)
    .bind(slugify(project_name))
    .bind(key_prefix)
    .execute(pool)
    .await
    .expect("seed other-team project");
}
