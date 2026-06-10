//! multi-workspace-tenancy — Slice 2 (web-tier boundary) step definitions.
//!
//! Generalises the isolation boundary slice 1 proved on the JSON API leg to the
//! htmx WEB tier — the surface with the most read/write paths. A member/admin of
//! "Acme" sees and manages ONLY Acme on the web; a crafted/stale link to a
//! "Globex" resource is refused IDENTICALLY to a never-existed one; an Acme admin
//! cannot manage Globex; and a multi-membership contractor acts on exactly one
//! workspace at a time, switching changing which tenant's data the web shows.
//! Proven with REAL Acme/Globex fixtures (slice-02-web-tier-boundary.md).
//!
//! Driving adapter: the htmx web tier (foundry-app over real HTTP) under the
//! production session + double-submit CSRF layers, reached at:
//!   GET  /team/{team}/project/{project}                   (board read)
//!   GET  /team/{team}/project/{project}/issues/{n}         (issue detail read)
//!   POST /team/{team}/project/{project}/issues             (file-issue write)
//!   POST /admin/tokens/{jti}/revoke                        (admin action, gated)
//! authenticated by a real signed-in `foundry_session` cookie (sign_in_and_capture
//! mirrors feature_b_web_tier + the shipped `signed_in_post` CSRF helper).
//!
//! Refusal-status contract (ADR-003 / OD-MWT-D6): a CROSS-tenant resource on the
//! web is the SAME 404 not-found response (status + page shape) as a never-existed
//! id — generalising the shipped `find_*_in_workspace → None` idiom. Cross-tenant
//! access never 403s (a 403-vs-404 difference is an existence oracle). The shipped
//! `/admin/tokens` surface already collapses non-admin / missing / foreign jti to
//! the SAME non-enumerable 404 (admin_tokens.rs:48); slice 2 proves it under a
//! genuinely-coexisting second workspace.
//!
//! RED-state contract (DISTILL, ADR-025 / Mandate 7): the crate COMPILES (not
//! BROKEN). Runtime RED is MISSING_FUNCTIONALITY:
//!   1. The Background's SECOND `INSERT INTO workspaces` fails on
//!      `uniq_one_workspace` (0001_init.sql:15) until DELIVER ships `0002`.
//!   2. Once two workspaces coexist, the web session resolves its acting
//!      workspace via `first_workspace()` (signin.rs:140), which picks an
//!      ARBITRARY row — so a member of Acme is not reliably scoped to Acme until
//!      ADR-005 membership resolution + the switcher are wired by DELIVER. The
//!      switch step targets a `/workspace/switch` route DELIVER adds (absent today
//!      → the switch + the post-switch read red for the right reason).
//!
//! LAYER 3 → example-based (Mandates 9/11), no PBT; traditional assertions over
//! port-exposed WEB observables (rendered page substrings, HTTP refusal status,
//! post-write workspace-scoped DB row presence). Step text is NEW + globally
//! unique; the Background workspace/member/issue SEEDS reuse the slice-1
//! `feature_mwt_slice_01_coexist` step text verbatim (cucumber-rs requires
//! globally-unique text — slice 1 owns those, slice 2 adds only web phrases).

use crate::support::harness::{signed_in_post, InProcHarness};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use secrecy::SecretString;
use std::collections::HashMap;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
/// Password every slice-2 web persona signs in with. The seeds below set this
/// hash on each user so the real cookie sign-in path authenticates.
const WEB_PASSWORD: &str = "slice-02-correct-horse-battery-staple";

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

/// Spawn the harness ONCE per scenario WITHOUT resetting it on subsequent calls
/// (the Background seeds two workspaces; a reset would discard the first).
/// Mirrors `feature_mwt_slice_01_coexist::ensure_harness`.
async fn ensure_harness(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        world.harness = Some(InProcHarness::spawn(now_anchor()).await);
        world.mwt_workspace_ids.clear();
        world.mwt_project_route.clear();
        world.mwt2_web_email = None;
        world.mwt2_web_password = None;
        world.mwt2_acting_ws = None;
        world.mwt2_last_body = None;
        world.mwt2_last_status = None;
        world.mwt2_first_refusal_body = None;
        world.mwt2_first_refusal_status = None;
        world.mwt2_credential_jti_by_label.clear();
        world.mwt2_credential_revoked_before = None;
    }
    if world.http.is_none() {
        world.http = Some(client());
    }
}

fn pool(world: &FoundryWorld) -> sqlx::PgPool {
    world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool()
        .clone()
}

/// Reset the named user's password to `WEB_PASSWORD` so the slice-2 web sign-in
/// can authenticate any seeded persona (the slice-1 seeds use a different,
/// internal password constant).
async fn set_web_password(world: &FoundryWorld, email: &str) {
    let hash = foundry_auth::hash_password(&SecretString::new(WEB_PASSWORD.to_string().into()))
        .await
        .expect("hash web password");
    sqlx::query("UPDATE users SET password_hash = $1 WHERE email_lower = $2")
        .bind(&hash)
        .bind(email.to_ascii_lowercase())
        .execute(&pool(world))
        .await
        .expect("set web password");
}

/// Sign in over the real cookie path and capture the `foundry_session=...` pair.
/// Mirrors `feature_b_web_tier::sign_in_and_capture_cookie`.
async fn sign_in_cookie(world: &FoundryWorld, email: &str) -> String {
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

    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("email", email.to_string());
    form.insert("password", WEB_PASSWORD.to_string());
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
    resp.headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .and_then(|s| s.split(';').next())
        .map(|p| p.to_string())
        .expect("sign-in issues a foundry_session cookie")
}

/// Authenticated web GET of `path` as the currently signed-in slice-2 persona.
async fn web_get(world: &FoundryWorld, path: &str) -> (StatusCode, String) {
    let email = world
        .mwt2_web_email
        .clone()
        .expect("a slice-2 web persona is signed in");
    let cookie = sign_in_cookie(world, &email).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let resp = http
        .get(format!("{base}{path}", base = harness.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("authenticated web GET");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

/// Resolve the (team_slug, project_slug) recorded for a workspace-scoped project
/// so a When step can target the right tenant's route.
fn project_route(world: &FoundryWorld, ws: &str, project: &str) -> (String, String) {
    world
        .mwt_project_route
        .get(&(ws.to_string(), project.to_string()))
        .cloned()
        .unwrap_or_else(|| panic!("project route for {ws:?}/{project:?} not seeded"))
}

// --------------------------------------------------------------------------
// Given — cross-membership seed + signed-in web personas (NEW slice-2 text)
// --------------------------------------------------------------------------

/// Add an ADDITIONAL workspace membership (+ team + project) for an existing or
/// new user — the multi-membership precondition (OD-2/ADR-005). Delegates the
/// member+team+project seed to the slice-1 step so the route is recorded
/// workspace-scoped, then ensures the user's web password is set.
#[given(
    regex = r#"^"([^"]+)" is also a member of "([^"]+)" in team "([^"]+)" with project "([^"]+)" prefix "([^"]+)"$"#
)]
async fn also_member_of(
    world: &mut FoundryWorld,
    email: String,
    ws: String,
    team: String,
    project: String,
    prefix: String,
) {
    ensure_harness(world).await;
    crate::steps::feature_mwt_slice_01_coexist::workspace_has_member_team_project(
        world,
        ws,
        email.clone(),
        team,
        project,
        prefix,
    )
    .await;
    set_web_password(world, &email).await;
}

/// Sign a member in on the web; record the acting workspace. The acting workspace
/// is the session's ACTIVE workspace (ADR-005). Today sign-in resolves it via
/// `first_workspace()` (signin.rs:140) — arbitrary under two workspaces — so this
/// step records the INTENDED acting workspace; DELIVER's membership resolution +
/// switcher make the session actually act on it (the RED edge).
#[given(regex = r#"^"([^"]+)" is signed in on the web acting on workspace "([^"]+)"$"#)]
async fn signed_in_acting_on(world: &mut FoundryWorld, email: String, ws: String) {
    ensure_harness(world).await;
    set_web_password(world, &email).await;
    world.mwt2_web_email = Some(email);
    world.mwt2_web_password = Some(WEB_PASSWORD.to_string());
    world.mwt2_acting_ws = Some(ws);
}

/// Seed a workspace-scoped admin credential (a `machine_tokens` row) into a NAMED
/// workspace, addressed later by label. Used by the admin-cannot-cross scenario:
/// an Acme admin attempts to revoke THIS Globex credential.
#[given(regex = r#"^the "([^"]+)" workspace has an admin credential "([^"]+)"$"#)]
async fn workspace_has_credential(world: &mut FoundryWorld, ws: String, label: String) {
    ensure_harness(world).await;
    let workspace_id = *world
        .mwt_workspace_ids
        .get(&ws)
        .unwrap_or_else(|| panic!("workspace {ws:?} must be seeded first"));
    // Bind the credential to the workspace's admin user (any member of it).
    let (admin_id,): (uuid::Uuid,) = sqlx::query_as(
        "SELECT user_id FROM workspace_memberships WHERE workspace_id = $1 AND role = 'admin' LIMIT 1",
    )
    .bind(workspace_id)
    .fetch_one(&pool(world))
    .await
    .unwrap_or_else(|e| panic!("resolve {ws:?} admin: {e}"));
    let jti = uuid::Uuid::now_v7();
    let exp = time::OffsetDateTime::now_utc() + time::Duration::seconds(3600);
    world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .insert_machine_token(jti, admin_id, workspace_id, None, exp, &label, admin_id)
        .await
        .expect("seed workspace credential");
    world.mwt2_credential_jti_by_label.insert(label, jti);
}

// --------------------------------------------------------------------------
// When — web reads / writes / admin action / switch (NEW slice-2 text)
// --------------------------------------------------------------------------

#[when(regex = r#"^the member opens the "([^"]+)" project "([^"]+)" board on the web$"#)]
async fn open_board(world: &mut FoundryWorld, ws: String, project: String) {
    let (team_slug, project_slug) = project_route(world, &ws, &project);
    let (status, body) = web_get(world, &format!("/team/{team_slug}/project/{project_slug}")).await;
    world.mwt2_last_status = Some(status);
    world.mwt2_last_body = Some(body);
}

#[when(
    regex = r#"^the member opens the "([^"]+)" project "([^"]+)" board on the web by its real address$"#
)]
async fn open_board_foreign(world: &mut FoundryWorld, ws: String, project: String) {
    // The FOREIGN board's REAL route (its own team/project slugs). The acting
    // workspace's scoped lookup must not resolve it → non-enumerable 404.
    let (team_slug, project_slug) = project_route(world, &ws, &project);
    let (status, body) = web_get(world, &format!("/team/{team_slug}/project/{project_slug}")).await;
    world.mwt2_first_refusal_status = Some(status);
    world.mwt2_first_refusal_body = Some(body);
}

#[when(regex = r#"^the member opens a project board that never existed on the web$"#)]
async fn open_board_never_existed(world: &mut FoundryWorld) {
    let (status, body) = web_get(world, "/team/no-such-team/project/no-such-project").await;
    world.mwt2_last_status = Some(status);
    world.mwt2_last_body = Some(body);
}

#[when(
    regex = r#"^the member opens issue (\w+-\d+) in the "([^"]+)" project "([^"]+)" on the web$"#
)]
async fn open_issue(world: &mut FoundryWorld, key: String, ws: String, project: String) {
    let (team_slug, project_slug) = project_route(world, &ws, &project);
    let number = issue_number(&key);
    let (status, body) = web_get(
        world,
        &format!("/team/{team_slug}/project/{project_slug}/issues/{number}"),
    )
    .await;
    // For the foreign-issue scenario this is the FIRST (foreign) refusal; for the
    // own-workspace read it is the page under test. Record both slots so the
    // matching Then can read whichever it needs.
    world.mwt2_first_refusal_status = Some(status);
    world.mwt2_first_refusal_body = Some(body.clone());
    world.mwt2_last_status = Some(status);
    world.mwt2_last_body = Some(body);
}

#[when(regex = r#"^the member opens an issue that never existed on the web$"#)]
async fn open_issue_never_existed(world: &mut FoundryWorld) {
    // A never-existed issue under the acting workspace's own (real) project, with
    // an absurd number — so the ONLY difference from the foreign reach is
    // existence, not the route shape.
    let ws = world.mwt2_acting_ws.clone().expect("acting workspace");
    let project = first_project_in(world, &ws);
    let (team_slug, project_slug) = project_route(world, &ws, &project);
    let (status, body) = web_get(
        world,
        &format!("/team/{team_slug}/project/{project_slug}/issues/999999"),
    )
    .await;
    world.mwt2_last_status = Some(status);
    world.mwt2_last_body = Some(body);
}

#[when(
    regex = r#"^the member files issue "([^"]+)" in the "([^"]+)" project "([^"]+)" on the web$"#
)]
async fn file_issue(world: &mut FoundryWorld, title: String, ws: String, project: String) {
    let (team_slug, project_slug) = project_route(world, &ws, &project);
    let email = world.mwt2_web_email.clone().expect("web persona");
    let url = format!("/team/{team_slug}/project/{project_slug}/issues");
    let outcome = signed_in_post(
        world.harness.as_ref().expect("harness"),
        world.http.as_ref().expect("http"),
        &email,
        WEB_PASSWORD,
        &url,
        &[("title", title.as_str())],
    )
    .await;
    world.mwt2_last_status = Some(outcome.status);
    world.mwt2_last_body = Some(outcome.body);
}

#[when(regex = r#"^the member files issue "([^"]+)" in a project that never existed on the web$"#)]
async fn file_issue_never_existed(world: &mut FoundryWorld, title: String) {
    let email = world.mwt2_web_email.clone().expect("web persona");
    let outcome = signed_in_post(
        world.harness.as_ref().expect("harness"),
        world.http.as_ref().expect("http"),
        &email,
        WEB_PASSWORD,
        "/team/no-such-team/project/no-such-project/issues",
        &[("title", title.as_str())],
    )
    .await;
    // The foreign-write captured first into mwt2_first_refusal_*; this is the
    // never-existed comparator.
    world.mwt2_last_status = Some(outcome.status);
    world.mwt2_last_body = Some(outcome.body);
}

#[when(
    regex = r#"^the "([^"]+)" admin tries to revoke the "([^"]+)" credential "([^"]+)" on the web$"#
)]
async fn admin_revoke_foreign(
    world: &mut FoundryWorld,
    _admin_ws: String,
    foreign_ws: String,
    label: String,
) {
    let jti = *world
        .mwt2_credential_jti_by_label
        .get(&label)
        .unwrap_or_else(|| panic!("credential {label:?} must be seeded"));
    // Snapshot the foreign credential's revoked state BEFORE the attempt.
    let (revoked_before,): (bool,) =
        sqlx::query_as("SELECT (revoked_at IS NOT NULL) FROM machine_tokens WHERE jti = $1")
            .bind(jti)
            .fetch_one(&pool(world))
            .await
            .unwrap_or_else(|e| panic!("snapshot {foreign_ws:?} credential: {e}"));
    world.mwt2_credential_revoked_before = Some(revoked_before);

    let email = world.mwt2_web_email.clone().expect("web admin persona");
    let outcome = signed_in_post(
        world.harness.as_ref().expect("harness"),
        world.http.as_ref().expect("http"),
        &email,
        WEB_PASSWORD,
        &format!("/admin/tokens/{jti}/revoke"),
        &[],
    )
    .await;
    world.mwt2_first_refusal_status = Some(outcome.status);
    world.mwt2_first_refusal_body = Some(outcome.body);
}

#[when(regex = r#"^the member switches their active workspace to "([^"]+)"$"#)]
async fn switch_workspace(world: &mut FoundryWorld, ws: String) {
    // DELIVER adds the switcher route (`POST /workspace/switch`, ADR-005) that
    // re-stamps `SessionUser.workspace_id` after verifying membership. Absent
    // today → this POST 404s, reding the post-switch read for the right reason.
    let workspace_id = *world
        .mwt_workspace_ids
        .get(&ws)
        .unwrap_or_else(|| panic!("workspace {ws:?} must be seeded first"));
    let email = world.mwt2_web_email.clone().expect("web persona");
    let outcome = signed_in_post(
        world.harness.as_ref().expect("harness"),
        world.http.as_ref().expect("http"),
        &email,
        WEB_PASSWORD,
        "/workspace/switch",
        &[("workspace_id", workspace_id.to_string().as_str())],
    )
    .await;
    world.mwt2_last_status = Some(outcome.status);
    world.mwt2_last_body = Some(outcome.body);
    world.mwt2_acting_ws = Some(ws);
}

// --------------------------------------------------------------------------
// Then — port-exposed observable assertions (LAYER 3, traditional)
// --------------------------------------------------------------------------

fn body_now(world: &FoundryWorld) -> &str {
    world
        .mwt2_last_body
        .as_deref()
        .expect("a web body captured")
}

fn prefix_for(ws: &str) -> &'static str {
    match ws {
        "Acme" => "ACME-",
        "Globex" => "GLOBEX-",
        other => panic!("unexpected workspace {other:?}"),
    }
}

#[then(regex = r#"^only "([^"]+)" data appears on the web$"#)]
async fn only_ws_data(world: &mut FoundryWorld, ws: String) {
    assert_eq!(
        world.mwt2_last_status,
        Some(StatusCode::OK),
        "expected 200 rendering own-workspace data; body = {:?}",
        world.mwt2_last_body
    );
    let body = body_now(world);
    assert!(
        body.contains(prefix_for(&ws)),
        "expected {ws} data in the web page: {body:?}"
    );
}

#[then(regex = r#"^no "([^"]+)" data appears on the web$"#)]
async fn no_ws_data(world: &mut FoundryWorld, ws: String) {
    let body = body_now(world);
    assert!(
        !body.contains(prefix_for(&ws)),
        "{ws} data leaked into the web page: {body:?}"
    );
}

#[then(regex = r#"^the new issue appears only in "([^"]+)" on the web$"#)]
async fn new_issue_only_in(world: &mut FoundryWorld, ws: String) {
    // Port-observable WRITE outcome: the row exists scoped to the acting workspace
    // and does NOT exist in the sibling. Asserted via a workspace-scoped count.
    let acting = *world
        .mwt_workspace_ids
        .get(&ws)
        .unwrap_or_else(|| panic!("workspace {ws:?}"));
    let (count_here,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM issues WHERE workspace_id = $1 AND title = 'Rotate signing keys'",
    )
    .bind(acting)
    .fetch_one(&pool(world))
    .await
    .expect("count issues in acting workspace");
    assert_eq!(
        count_here, 1,
        "the new issue should exist exactly once in {ws}"
    );
    let (count_foreign,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM issues WHERE workspace_id <> $1 AND title = 'Rotate signing keys'",
    )
    .bind(acting)
    .fetch_one(&pool(world))
    .await
    .expect("count issues outside acting workspace");
    assert_eq!(
        count_foreign, 0,
        "the write must not affect any other workspace"
    );
}

#[then(regex = r#"^the member now sees only "([^"]+)" data on the web$"#)]
async fn now_sees_only(world: &mut FoundryWorld, ws: String) {
    only_ws_data(world, ws).await;
}

/// The non-enumerability core: the foreign-id refusal and the never-existed-id
/// refusal are observationally identical (same status; same page shape).
#[then(regex = r#"^the two web responses are refused identically$"#)]
async fn refused_identically(world: &mut FoundryWorld) {
    let foreign_status = world
        .mwt2_first_refusal_status
        .expect("a foreign-id refusal status captured");
    let nonexistent_status = world
        .mwt2_last_status
        .expect("a never-existed refusal status captured");
    assert_eq!(
        foreign_status,
        StatusCode::NOT_FOUND,
        "cross-tenant access must be a non-enumerable 404 (ADR-003), got {foreign_status}"
    );
    assert_eq!(
        foreign_status, nonexistent_status,
        "foreign-id and never-existed-id must share the SAME status (no oracle)"
    );
    let foreign_body = world
        .mwt2_first_refusal_body
        .as_deref()
        .expect("foreign-id refusal body");
    let nonexistent_body = world.mwt2_last_body.as_deref().expect("never-existed body");
    assert_eq!(
        foreign_body, nonexistent_body,
        "foreign-id and never-existed-id must share the SAME body shape (no oracle)"
    );
}

#[then(regex = r#"^the web request is refused identically to a never-existed credential$"#)]
async fn admin_refused_non_enumerably(world: &mut FoundryWorld) {
    let status = world
        .mwt2_first_refusal_status
        .expect("a refusal status captured");
    // Cross-tenant admin reach collapses to the SAME non-enumerable 404 as a
    // never-existed jti (admin_tokens.rs:48, NFR-MT-SEC-03 / ADR-003) — never a
    // 403/200 that would confirm the foreign credential exists.
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "an Acme admin reaching a Globex credential must get a non-enumerable 404, got {status}"
    );
}

#[then(regex = r#"^nothing on the web reveals the "([^"]+)" board exists$"#)]
async fn nothing_reveals_board(world: &mut FoundryWorld, ws: String) {
    let body = world.mwt2_first_refusal_body.as_deref().unwrap_or_default();
    assert!(
        !body.contains(prefix_for(&ws)),
        "{ws} data leaked into the refusal page: {body:?}"
    );
}

#[then(regex = r#"^nothing on the web reveals the "([^"]+)" issue exists$"#)]
async fn nothing_reveals_issue(world: &mut FoundryWorld, ws: String) {
    nothing_reveals_board(world, ws).await;
}

#[then(regex = r#"^no "([^"]+)" membership or credential is changed$"#)]
async fn no_foreign_change(world: &mut FoundryWorld, _ws: String) {
    let before = world
        .mwt2_credential_revoked_before
        .expect("recorded foreign credential state");
    let jti = *world
        .mwt2_credential_jti_by_label
        .values()
        .next()
        .expect("a seeded foreign credential");
    let (after,): (bool,) =
        sqlx::query_as("SELECT (revoked_at IS NOT NULL) FROM machine_tokens WHERE jti = $1")
            .bind(jti)
            .fetch_one(&pool(world))
            .await
            .expect("re-read foreign credential");
    assert_eq!(
        before, after,
        "the foreign workspace's credential must be unchanged by the cross-tenant attempt"
    );
    assert!(!after, "the foreign credential must NOT have been revoked");
}

// --------------------------------------------------------------------------
// Local helpers
// --------------------------------------------------------------------------

fn issue_number(key: &str) -> i32 {
    key.rsplit('-')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("issue key {key:?} must end in -<n>"))
}

/// The first project name recorded for a workspace (for the never-existed-issue
/// comparator, which uses the acting workspace's own real project route).
fn first_project_in(world: &FoundryWorld, ws: &str) -> String {
    world
        .mwt_project_route
        .keys()
        .find(|(w, _)| w == ws)
        .map(|(_, p)| p.clone())
        .unwrap_or_else(|| panic!("no project recorded for {ws:?}"))
}
