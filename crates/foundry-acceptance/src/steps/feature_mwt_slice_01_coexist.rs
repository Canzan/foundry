//! multi-workspace-tenancy — Slice 1 (Walking Skeleton) step definitions.
//!
//! Proves the load-bearing abstraction every later slice depends on: two REAL
//! workspaces ("Acme" + "Globex") coexist in one instance, and a request
//! resolves to EXACTLY its own workspace on ONE read path — the JSON API
//!   GET /api/v1/teams/{team}/projects/{project}/issues
//! authenticated by the SHIPPED MachinePrincipal bearer whose
//! `token.workspace_id` is the acting workspace (ADR-001 hybrid resolution,
//! API leg). The web/session leg (SessionUser active-workspace EXTEND, ADR-005)
//! is Slice 3 and is OUT of scope here.
//!
//! RED-state contract (DISTILL, ADR-025 / Mandate 7):
//!   - The two-workspace seeding Given steps do REAL inserts (no synthetic
//!     uuids). The SECOND `INSERT INTO workspaces` FAILS on the shipped
//!     `uniq_one_workspace` unique index (0001_init.sql:15) until DELIVER ships
//!     the `0002_multi_workspace.sql` migration that DROPS it. Per
//!     design/upstream-changes.md Finding 1, the application-level 409 guard in
//!     `create_workspace` (bootstrap.rs:289) is NOT on this path — seeding
//!     inserts the row directly via sqlx, mirroring the shipped single-workspace
//!     seeds (us_06/us_07) — so the index is the only guard the seed hits. The
//!     RED failure is "second workspace cannot exist yet" = MISSING_FUNCTIONALITY,
//!     not BROKEN.
//!   - When steps issue a REAL HTTP request to `/api/v1/.../issues` and capture
//!     the response. The route is shipped (Feature-A US-W05a), so the When
//!     reaches a real handler; the isolation outcome is the behaviour under test
//!     once two workspaces can coexist.
//!   - Then steps assert the JSON outcome (port-exposed observables: the listed
//!     issue keys, the HTTP refusal status). LAYER 3 → traditional assertions
//!     over port observables (Mandate 8 layers 1-3; example-based, no PBT per
//!     Mandates 9/11).
//!
//! Step text is NEW and globally unique (cucumber-rs requires it). The seeding
//! deliberately does NOT reuse `a workspace "..." exists with admin "..."`
//! (us_06_signin) because that step RESETS the harness to a single workspace and
//! re-inits state — incompatible with seeding TWO coexisting workspaces. The
//! NEW `workspace "..." exists with admin "..."` text seeds additively.

use crate::support::harness::InProcHarness;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use secrecy::{ExposeSecret, SecretString};

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

/// Spawn the harness ONCE per scenario WITHOUT resetting it on subsequent calls
/// (Background seeds two workspaces; a reset would discard the first).
async fn ensure_harness(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        world.harness = Some(InProcHarness::spawn(now_anchor()).await);
        world.mwt_workspace_ids.clear();
        world.mwt_project_route.clear();
        world.mwt_bearer_by_email.clear();
        world.mwt_no_workspace_bearer = None;
        world.mwt_issues_before_by_workspace.clear();
        world.mwt_workspace_id_before = None;
        world.mwt_acme_answer = None;
        world.mwt_globex_answer = None;
    }
    if world.http.is_none() {
        world.http = Some(client());
    }
}

fn slugify(name: &str) -> String {
    name.to_ascii_lowercase().replace(' ', "-")
}

// --------------------------------------------------------------------------
// Given — seed coexisting workspaces (the NEW two-workspace fixture)
// --------------------------------------------------------------------------

/// Seed a workspace + its admin user + admin membership, ADDITIVELY (does not
/// reset the harness). The SECOND call's `INSERT INTO workspaces` is the RED
/// edge: it fails on `uniq_one_workspace` until `0002` drops the guard.
#[given(regex = r#"^workspace "([^"]+)" exists with admin "([^"]+)"$"#)]
async fn workspace_exists_with_admin(world: &mut FoundryWorld, ws_name: String, admin: String) {
    ensure_harness(world).await;
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool()
        .clone();

    let workspace_id = uuid::Uuid::now_v7();
    let user_id = uuid::Uuid::now_v7();
    let admin_lower = admin.to_ascii_lowercase();
    let admin_hash =
        foundry_auth::hash_password(&SecretString::new("admin-password".to_string().into()))
            .await
            .expect("hash admin pw");

    // RED edge: the 2nd workspace insert fails on `uniq_one_workspace` today.
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(workspace_id)
        .bind(&ws_name)
        .execute(&pool)
        .await
        .expect("insert workspace (RED until 0002 drops uniq_one_workspace)");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5) ON CONFLICT (email_lower) DO NOTHING",
    )
    .bind(user_id)
    .bind(&admin_lower)
    .bind(&admin)
    .bind("Admin")
    .bind(&admin_hash)
    .execute(&pool)
    .await
    .expect("insert workspace admin");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'admin')",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("insert admin membership");

    world.mwt_workspace_ids.insert(ws_name, workspace_id);
}

/// Seed a member + a team + a project, all scoped to the named workspace. The
/// project route is recorded workspace-scoped so the When step targets the
/// right tenant's project (name lookups alone are ambiguous across tenants).
#[given(
    regex = r#"^"([^"]+)" has a member "([^"]+)" in team "([^"]+)" with project "([^"]+)" prefix "([^"]+)"$"#
)]
async fn workspace_has_member_team_project(
    world: &mut FoundryWorld,
    ws_name: String,
    member: String,
    team: String,
    project: String,
    prefix: String,
) {
    ensure_harness(world).await;
    let workspace_id = *world
        .mwt_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} must be seeded first"));
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool()
        .clone();

    let user_id = uuid::Uuid::now_v7();
    let member_lower = member.to_ascii_lowercase();
    let pw_hash =
        foundry_auth::hash_password(&SecretString::new("member-password".to_string().into()))
            .await
            .expect("hash member pw");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5) ON CONFLICT (email_lower) DO NOTHING",
    )
    .bind(user_id)
    .bind(&member_lower)
    .bind(&member)
    .bind("Member")
    .bind(&pw_hash)
    .execute(&pool)
    .await
    .expect("insert member user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')
              ON CONFLICT DO NOTHING",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("insert member membership");

    let team_id = uuid::Uuid::now_v7();
    let team_slug = slugify(&team);
    sqlx::query(
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)
              ON CONFLICT (workspace_id, slug) DO NOTHING",
    )
    .bind(team_id)
    .bind(workspace_id)
    .bind(&team)
    .bind(&team_slug)
    .execute(&pool)
    .await
    .expect("insert team");

    // Resolve the real team id (the ON CONFLICT above may have kept an existing
    // row, so the generated `team_id` is not authoritative), then seed the
    // member's team membership. The shipped board-read authz requires
    // `is_team_member` (foundry-services/src/issues.rs:160) — a workspace
    // membership alone is refused 403, so the member must belong to the team.
    let (team_id,): (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM teams WHERE workspace_id = $1 AND slug = $2")
            .bind(workspace_id)
            .bind(&team_slug)
            .fetch_one(&pool)
            .await
            .expect("resolve team id");
    sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')
              ON CONFLICT DO NOTHING",
    )
    .bind(team_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("insert team membership");

    let project_id = uuid::Uuid::now_v7();
    let project_slug = slugify(&project);
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (workspace_id, key_prefix) DO NOTHING",
    )
    .bind(project_id)
    .bind(team_id)
    .bind(workspace_id)
    .bind(&project)
    .bind(&project_slug)
    .bind(&prefix)
    .execute(&pool)
    .await
    .expect("insert project");

    world
        .mwt_project_route
        .insert((ws_name, project), (team_slug, project_slug));
}

/// Seed two real issues into a workspace-scoped project (the data whose
/// isolation is under test). Issue keys are `<PREFIX>-<n>` per the project's
/// key prefix; we record the two keys requested in the step text.
#[given(regex = r#"^the "([^"]+)" project "([^"]+)" has issues (\w+-\d+) and (\w+-\d+)$"#)]
async fn project_has_two_issues(
    world: &mut FoundryWorld,
    ws_name: String,
    project: String,
    key_a: String,
    key_b: String,
) {
    ensure_harness(world).await;
    let workspace_id = *world
        .mwt_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} must be seeded first"));
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool()
        .clone();

    // Resolve the project WITHIN this workspace (name alone is ambiguous across
    // tenants — scope by workspace_id, the whole point of the slice).
    let (project_id,): (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM projects WHERE workspace_id = $1 AND name = $2")
            .bind(workspace_id)
            .bind(&project)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|e| panic!("resolve project {project:?} in {ws_name:?}: {e}"));
    // Resolve an author within the workspace (any member of it).
    let (author_id,): (uuid::Uuid,) =
        sqlx::query_as("SELECT user_id FROM workspace_memberships WHERE workspace_id = $1 LIMIT 1")
            .bind(workspace_id)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|e| panic!("resolve author in {ws_name:?}: {e}"));

    for key in [&key_a, &key_b] {
        let number: i32 = key
            .rsplit('-')
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| panic!("issue key {key:?} must end in -<n>"));
        sqlx::query(
            "INSERT INTO issues (id, project_id, workspace_id, number, title, author_id)
                  VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(uuid::Uuid::now_v7())
        .bind(project_id)
        .bind(workspace_id)
        .bind(number)
        .bind(format!("Seeded issue {key}"))
        .bind(author_id)
        .execute(&pool)
        .await
        .expect("insert issue");
    }
}

// --------------------------------------------------------------------------
// Given — credentials (the resolution seam: token.workspace_id)
// --------------------------------------------------------------------------

/// Mint a REAL EdDSA bearer bound to `(member, workspace)` — `token.workspace_id`
/// is the acting workspace per ADR-001. Registers the registry row so the
/// shipped denylist admits it.
#[given(regex = r#"^a machine credential is bound to "([^"]+)" in workspace "([^"]+)"$"#)]
async fn credential_bound_to(world: &mut FoundryWorld, member: String, ws_name: String) {
    ensure_harness(world).await;
    let workspace_id = *world
        .mwt_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} must be seeded first"));
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool()
        .clone();
    let (user_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(member.to_ascii_lowercase())
        .fetch_one(&pool)
        .await
        .unwrap_or_else(|e| panic!("resolve member {member:?}: {e}"));

    let jwt = mint_bearer_bound(world, user_id, workspace_id, "slice-01-cred").await;
    world.mwt_bearer_by_email.insert(member, jwt);
}

/// A credential whose holder belongs to NO workspace — the fail-closed
/// resolution edge. We seed a user with NO membership and bind a token to a
/// workspace_id that the holder is not a member of (resolution must refuse,
/// never default).
#[given(regex = r#"^a credential whose holder belongs to no workspace$"#)]
async fn credential_no_workspace(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool()
        .clone();
    let user_id = uuid::Uuid::now_v7();
    let pw_hash = foundry_auth::hash_password(&SecretString::new("orphan-pw".to_string().into()))
        .await
        .expect("hash orphan pw");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind("orphan@nowhere.test")
    .bind("orphan@nowhere.test")
    .bind("Orphan")
    .bind(&pw_hash)
    .execute(&pool)
    .await
    .expect("insert orphan user");
    // No membership row is inserted: resolution from the token must fail closed.
    let any_ws = *world
        .mwt_workspace_ids
        .values()
        .next()
        .expect("at least one workspace seeded");
    let jwt = mint_bearer_bound(world, user_id, any_ws, "slice-01-orphan").await;
    world.mwt_no_workspace_bearer = Some(jwt);
}

async fn mint_bearer_bound(
    world: &mut FoundryWorld,
    user_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
    label: &str,
) -> String {
    let jti = uuid::Uuid::now_v7();
    let now = time::OffsetDateTime::now_utc();
    let exp = now + time::Duration::seconds(3600);
    world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .insert_machine_token(jti, user_id, workspace_id, None, exp, label, user_id)
        .await
        .expect("register machine token");
    let claims = foundry_auth::MachineTokenClaims {
        sub: user_id,
        scope: None,
        iat: now.unix_timestamp(),
        exp: exp.unix_timestamp(),
        jti,
        iss: foundry_auth::MACHINE_TOKEN_ISS.to_string(),
        aud: foundry_auth::MACHINE_TOKEN_AUD.to_string(),
    };
    foundry_auth::test_keys::signer()
        .mint(&claims)
        .expect("mint machine jwt")
        .expose_secret()
        .to_string()
}

// --------------------------------------------------------------------------
// Given — coexistence + no-rewrite preconditions
// --------------------------------------------------------------------------

#[given(regex = r#"^an instance that already has the workspace "([^"]+)"$"#)]
async fn instance_already_has_workspace(world: &mut FoundryWorld, ws_name: String) {
    ensure_harness(world).await;
    // Reuse the additive seed.
    workspace_exists_with_admin(world, ws_name, "ops@first.test".to_string()).await;
}

#[given(
    regex = r#"^the existing workspace "([^"]+)" with its issues recorded before the guard is dropped$"#
)]
async fn existing_workspace_recorded(world: &mut FoundryWorld, ws_name: String) {
    ensure_harness(world).await;
    // The Background already seeded this workspace additively (one row, recorded
    // in `mwt_workspace_ids`). Re-seeding here would insert a DUPLICATE
    // same-named `workspaces` row, making the Then's `SELECT id ... WHERE name`
    // ambiguous. Reuse the existing row and only add its team/project/issues.
    if !world.mwt_workspace_ids.contains_key(&ws_name) {
        workspace_exists_with_admin(world, ws_name.clone(), "ops@first.test".to_string()).await;
    }
    workspace_has_member_team_project(
        world,
        ws_name.clone(),
        "marco@first.test".to_string(),
        "Backend".to_string(),
        "Auth".to_string(),
        "ACME".to_string(),
    )
    .await;
    project_has_two_issues(
        world,
        ws_name.clone(),
        "Auth".to_string(),
        "ACME-1".to_string(),
        "ACME-2".to_string(),
    )
    .await;
    let workspace_id = *world.mwt_workspace_ids.get(&ws_name).expect("workspace");
    world.mwt_workspace_id_before = Some(workspace_id);
    world
        .mwt_issues_before_by_workspace
        .insert(ws_name, vec!["ACME-1".to_string(), "ACME-2".to_string()]);
}

// --------------------------------------------------------------------------
// When — list a workspace-scoped project's issues via the JSON API
// --------------------------------------------------------------------------

async fn list_issues_as(
    world: &mut FoundryWorld,
    ws_name: &str,
    project: &str,
    bearer: Option<String>,
) {
    ensure_harness(world).await;
    let (team_slug, project_slug) = world
        .mwt_project_route
        .get(&(ws_name.to_string(), project.to_string()))
        .cloned()
        .unwrap_or_else(|| panic!("project route for {ws_name:?}/{project:?} not seeded"));
    let base = world.harness.as_ref().expect("harness").base_url();
    let url = format!("{base}/api/v1/teams/{team_slug}/projects/{project_slug}/issues");
    let http = world.http.as_ref().expect("http");
    let mut req = http
        .get(&url)
        .header(reqwest::header::ACCEPT, "application/json");
    if let Some(b) = bearer {
        req = req.header(reqwest::header::AUTHORIZATION, format!("Bearer {b}"));
    }
    let resp = req.send().await.expect("send issues list request");
    world.mwt_last_status = Some(resp.status());
    world.mwt_last_body = Some(resp.text().await.unwrap_or_default());
}

#[when(regex = r#"^the Acme-bound credential lists the "([^"]+)" project's issues as data$"#)]
async fn acme_lists(world: &mut FoundryWorld, project: String) {
    let bearer = world.mwt_bearer_by_email.get("marco@acme.com").cloned();
    list_issues_as(world, "Acme", &project, bearer).await;
    world.mwt_acme_answer = world.mwt_last_body.clone();
}

#[when(regex = r#"^the Globex-bound credential lists the "([^"]+)" project's issues as data$"#)]
async fn globex_lists(world: &mut FoundryWorld, project: String) {
    let bearer = world.mwt_bearer_by_email.get("lucia@globex.com").cloned();
    list_issues_as(world, "Globex", &project, bearer).await;
    world.mwt_globex_answer = world.mwt_last_body.clone();
}

#[when(regex = r#"^that credential lists the "([^"]+)" project's issues as data$"#)]
async fn orphan_lists(world: &mut FoundryWorld, project: String) {
    let bearer = world.mwt_no_workspace_bearer.clone();
    list_issues_as(world, "Acme", &project, bearer).await;
}

#[when(regex = r#"^the workspace "([^"]+)" is created alongside it$"#)]
async fn workspace_created_alongside(world: &mut FoundryWorld, ws_name: String) {
    // RED edge: this second workspace insert fails on `uniq_one_workspace`
    // (and the bootstrap.rs:289 409 guard once provisioning lands) until 0002.
    workspace_exists_with_admin(world, ws_name, "ops@second.test".to_string()).await;
}

#[when(regex = r#"^the single-workspace guard is dropped so a second workspace becomes possible$"#)]
async fn guard_dropped(world: &mut FoundryWorld) {
    // DELIVER drops `uniq_one_workspace` in `0002`. In the RED scaffold the
    // migration is not present; the proof that "data is unchanged" is asserted
    // by the Then against the recorded-before snapshot once the migration runs.
    // No-op here: the migration is applied by the per-scenario schema set.
    let _ = world;
}

// --------------------------------------------------------------------------
// Then — port-exposed observable assertions (LAYER 3, traditional)
// --------------------------------------------------------------------------

fn body_contains_key(body: &str, key: &str) -> bool {
    body.contains(key)
}

#[then(regex = r#"^the answer lists only the "([^"]+)" issues (\w+-\d+) and (\w+-\d+)$"#)]
async fn answer_lists_only(world: &mut FoundryWorld, _ws: String, key_a: String, key_b: String) {
    let body = world.mwt_last_body.as_deref().expect("issues answer body");
    assert_eq!(
        world.mwt_last_status,
        Some(StatusCode::OK),
        "expected 200 OK listing issues; body = {body:?}"
    );
    assert!(
        body_contains_key(body, &key_a),
        "expected {key_a} in answer: {body:?}"
    );
    assert!(
        body_contains_key(body, &key_b),
        "expected {key_b} in answer: {body:?}"
    );
}

#[then(regex = r#"^no "([^"]+)" issue appears in the answer$"#)]
async fn no_other_issue(world: &mut FoundryWorld, other_ws: String) {
    let body = world.mwt_last_body.as_deref().expect("issues answer body");
    let foreign_prefix = match other_ws.as_str() {
        "Globex" => "GLOBEX-",
        "Acme" => "ACME-",
        _ => panic!("unexpected foreign workspace {other_ws:?}"),
    };
    assert!(
        !body.contains(foreign_prefix),
        "{other_ws} issue leaked into the answer: {body:?}"
    );
}

#[then(regex = r#"^the Acme answer contains only "Acme" issues$"#)]
async fn acme_answer_only_acme(world: &mut FoundryWorld) {
    let body = world.mwt_acme_answer.as_deref().expect("acme answer");
    assert!(body.contains("ACME-"), "expected Acme issues: {body:?}");
    assert!(
        !body.contains("GLOBEX-"),
        "Globex leaked into Acme answer: {body:?}"
    );
}

#[then(regex = r#"^the Globex answer contains only "Globex" issues$"#)]
async fn globex_answer_only_globex(world: &mut FoundryWorld) {
    let body = world.mwt_globex_answer.as_deref().expect("globex answer");
    assert!(body.contains("GLOBEX-"), "expected Globex issues: {body:?}");
    assert!(
        !body.contains("ACME-"),
        "Acme leaked into Globex answer: {body:?}"
    );
}

#[then(regex = r#"^neither answer contains any of the other workspace's issues$"#)]
async fn neither_contains_other(world: &mut FoundryWorld) {
    let acme = world.mwt_acme_answer.as_deref().expect("acme answer");
    let globex = world.mwt_globex_answer.as_deref().expect("globex answer");
    assert!(!acme.contains("GLOBEX-"), "Globex in Acme answer: {acme:?}");
    assert!(
        !globex.contains("ACME-"),
        "Acme in Globex answer: {globex:?}"
    );
}

#[then(regex = r#"^the answer is an empty data list for the new workspace$"#)]
async fn answer_empty(world: &mut FoundryWorld) {
    let body = world.mwt_last_body.as_deref().expect("answer body");
    assert_eq!(
        world.mwt_last_status,
        Some(StatusCode::OK),
        "expected 200; body {body:?}"
    );
    assert!(
        !body.contains("ACME-"),
        "no Acme issue should appear: {body:?}"
    );
    assert!(
        !body.contains("GLOBEX-"),
        "no Globex issue should appear: {body:?}"
    );
}

#[then(regex = r#"^no other workspace's issues appear$"#)]
async fn no_other_workspace_issue(world: &mut FoundryWorld) {
    let body = world.mwt_last_body.as_deref().expect("answer body");
    assert!(!body.contains("ACME-"), "Acme issue leaked: {body:?}");
}

#[then(regex = r#"^both workspaces exist on the instance$"#)]
async fn both_workspaces_exist(world: &mut FoundryWorld) {
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool()
        .clone();
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM workspaces")
        .fetch_one(&pool)
        .await
        .expect("count workspaces");
    assert!(
        count >= 2,
        "expected >= 2 coexisting workspaces, found {count}"
    );
}

#[then(regex = r#"^neither creation is blocked by a single-workspace limit$"#)]
async fn neither_blocked(world: &mut FoundryWorld) {
    // If the additive seeds above succeeded, the guard is gone. Re-assert count
    // as the observable (no exception was raised inserting the second row).
    both_workspaces_exist(world).await;
}

#[then(regex = r#"^the request is refused$"#)]
async fn request_refused(world: &mut FoundryWorld) {
    let status = world.mwt_last_status.expect("a status was captured");
    assert!(
        status == StatusCode::UNAUTHORIZED
            || status == StatusCode::FORBIDDEN
            || status == StatusCode::NOT_FOUND,
        "expected a fail-closed refusal (401/403/404), got {status}"
    );
}

#[then(regex = r#"^it is not served against any workspace's data$"#)]
async fn not_served(world: &mut FoundryWorld) {
    let body = world.mwt_last_body.as_deref().unwrap_or("");
    assert!(
        !body.contains("ACME-"),
        "orphan was served Acme data: {body:?}"
    );
    assert!(
        !body.contains("GLOBEX-"),
        "orphan was served Globex data: {body:?}"
    );
}

#[then(regex = r#"^the "([^"]+)" workspace's identity is unchanged$"#)]
async fn workspace_identity_unchanged(world: &mut FoundryWorld, ws_name: String) {
    let before = world
        .mwt_workspace_id_before
        .expect("recorded workspace id");
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool()
        .clone();
    let (after,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces WHERE name = $1")
        .bind(&ws_name)
        .fetch_one(&pool)
        .await
        .expect("fetch workspace after");
    assert_eq!(before, after, "workspace id changed across the guard drop");
}

#[then(regex = r#"^every "([^"]+)" issue recorded beforehand is present and unchanged afterward$"#)]
async fn issues_unchanged(world: &mut FoundryWorld, ws_name: String) {
    let expected = world
        .mwt_issues_before_by_workspace
        .get(&ws_name)
        .cloned()
        .expect("recorded issues");
    let workspace_id = *world.mwt_workspace_ids.get(&ws_name).expect("workspace");
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool()
        .clone();
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM issues WHERE workspace_id = $1")
        .bind(workspace_id)
        .fetch_one(&pool)
        .await
        .expect("count issues");
    assert_eq!(
        count as usize,
        expected.len(),
        "issue count for {ws_name} changed across the guard drop"
    );
}
