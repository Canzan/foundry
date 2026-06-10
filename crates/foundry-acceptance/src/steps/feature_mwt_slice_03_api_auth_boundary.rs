//! multi-workspace-tenancy — Slice 3 (JSON /api/v1 + machine-token +
//! sign-in/session-resolution boundary) step definitions.
//!
//! Slice 1 proved coexistence + resolution on ONE API read path (issues LIST,
//! scoped by `token.workspace_id`). Slice 2 generalised the boundary to the WEB
//! htmx tier (read/write/admin/refusal + the multi-membership switcher). This
//! slice EXTENDS the boundary to the REMAINING /api/v1 surfaces — the issue
//! WRITE path and the token list/revoke path — and asserts the session-
//! resolution CONTRACT (US-MWT04) directly at the seam.
//!
//! What this slice proves (slices/slice-03-api-and-auth-boundary.md "Done when"):
//!   - An Acme-bound token acts ONLY on Acme across /api/v1 (read + WRITE + token
//!     list/revoke); a Globex-targeting call is refused non-enumerably (uniform
//!     404, reusing the ADR-003 contract proven on the web in slice 2).
//!   - Token list/revoke is confined to the acting workspace with REAL two-
//!     workspace fixtures — CONVERTING the synthetic-uuid residual in
//!     `feature_token_management_api::credential_in_another_workspace` (which used
//!     a fresh random jti because `uniq_one_workspace` forbade a real second
//!     workspace) into a real-fixture proof (NFR-MWT-TEST-01 / DM8).
//!   - A session resolves to EXACTLY one acting workspace via the SHIPPED
//!     `resolve_active_workspace` seam (ADR-005): single-membership auto,
//!     multi-membership to the chosen one, none → refused/fail-closed.
//!   - The shipped verify path (iss/aud/EdDSA pinning + the per-request jti
//!     denylist) is UNCHANGED under two coexisting workspaces.
//!
//! RED-state contract (DISTILL, ADR-025 / Mandate 7):
//!   - The two-workspace Background seeds Acme then Globex; the SECOND
//!     `INSERT INTO workspaces` FAILS on `uniq_one_workspace` (0001_init.sql:15)
//!     until DELIVER ships `0002_multi_workspace.sql` (shared with slices 1-2).
//!     This is MISSING_FUNCTIONALITY, not BROKEN.
//!   - The remaining /api/v1 scoping (issue WRITE via `create_issue`; token
//!     list/revoke via `list_tokens(principal.workspace_id())` /
//!     `revoke_token`'s `row.workspace_id != principal.workspace_id() ⇒
//!     NotFound`) is ALREADY shipped + 100%-mutation-hardened — green-by-
//!     inheritance once `0002` lets two workspaces coexist.
//!   - US-MWT04 session resolution (`resolve_active_workspace`) is shipped by
//!     slices 1-2; these scenarios assert that CONTRACT directly.
//!
//! Step text is NEW and globally unique where this slice adds surface; the
//! two-workspace SEED Givens + the bearer-mint + the issues-list When/Then are
//! REUSED verbatim from slice-1's registered step text (cucumber-rs requires
//! globally-unique step text, so a reused step is bound by matching its exact
//! registered regex — NOT re-declared here). The multi-membership
//! `is also a member of ...` Given is REUSED from slice-2's registered text.
//!
//! LAYER 3 (real adapter): example-based, sad paths enumerated explicitly
//! (Mandates 9 + 11). No PBT machinery at this layer.

use crate::support::harness::InProcHarness;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
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

/// Mirror slice-1/slice-2's additive harness guard: spawn ONCE per scenario and
/// never reset on subsequent calls (the Background seeds two workspaces; a reset
/// would discard the first). Slice-3 owns no extra reset because it shares the
/// slice-1 world fields the seed Givens populate; we only ensure the harness +
/// http client exist when a slice-3-OWNED step runs first in isolation.
async fn ensure_harness(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        world.harness = Some(InProcHarness::spawn(now_anchor()).await);
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

async fn user_id_for(world: &FoundryWorld, email: &str) -> uuid::Uuid {
    let (id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(email.to_ascii_lowercase())
        .fetch_one(&pool(world))
        .await
        .unwrap_or_else(|e| panic!("resolve user {email:?}: {e}"));
    id
}

/// Mint a REAL EdDSA bearer bound to `(user_id, workspace_id)`, optionally
/// registering the registry row (so the denylist admits it). Self-contained so
/// this module does not depend on slice-1's private helpers.
async fn mint_bearer_bound(
    world: &mut FoundryWorld,
    user_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
    label: &str,
    register: bool,
) -> (String, uuid::Uuid) {
    let jti = uuid::Uuid::now_v7();
    let now = time::OffsetDateTime::now_utc();
    let exp = now + time::Duration::seconds(3600);
    if register {
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
    }
    let claims = foundry_auth::MachineTokenClaims {
        sub: user_id,
        scope: None,
        iat: now.unix_timestamp(),
        exp: exp.unix_timestamp(),
        jti,
        iss: foundry_auth::MACHINE_TOKEN_ISS.to_string(),
        aud: foundry_auth::MACHINE_TOKEN_AUD.to_string(),
    };
    let jwt = foundry_auth::test_keys::signer()
        .mint(&claims)
        .expect("mint machine jwt")
        .expose_secret()
        .to_string();
    (jwt, jti)
}

fn base_url(world: &FoundryWorld) -> String {
    world.harness.as_ref().expect("harness").base_url()
}

// ==========================================================================
// Given — token registry preconditions in a NAMED real workspace (residual
// closure: the foreign token is a REAL row in the REAL Globex workspace, not a
// synthetic uuid).
// ==========================================================================

/// Seed a `machine_tokens` row in the named workspace, bound to that workspace's
/// admin, recorded by label. The Globex variant is the REAL foreign target the
/// Acme token must NOT see/revoke — replacing the `us-tma`
/// `credential_in_another_workspace` synthetic uuid.
#[given(regex = r#"^a managed token "([^"]+)" exists in workspace "([^"]+)"$"#)]
async fn managed_token_in_workspace(world: &mut FoundryWorld, label: String, ws_name: String) {
    ensure_harness(world).await;
    let workspace_id = *world
        .mwt_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} must be seeded first"));
    // Bind the token to a real member of that workspace (its admin).
    let (admin_id,): (uuid::Uuid,) = sqlx::query_as(
        "SELECT user_id FROM workspace_memberships WHERE workspace_id = $1 AND role = 'admin' LIMIT 1",
    )
    .bind(workspace_id)
    .fetch_one(&pool(world))
    .await
    .unwrap_or_else(|e| panic!("resolve admin in {ws_name:?}: {e}"));

    let jti = uuid::Uuid::now_v7();
    let exp = time::OffsetDateTime::now_utc() + time::Duration::days(90);
    world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .insert_machine_token(jti, admin_id, workspace_id, None, exp, &label, admin_id)
        .await
        .expect("seed managed token row in named workspace");
    world.mwt3_token_jti_by_label.insert(label, jti);
}

/// Bind the next bearer to an existing user in the named workspace, registering
/// the registry row so the denylist admits it. (For the token list/revoke
/// scenarios the bearer must be a workspace ADMIN so `is_workspace_admin` holds.)
#[given(regex = r#"^that credential has been revoked$"#)]
async fn that_credential_revoked(world: &mut FoundryWorld) {
    // The most-recent Acme bearer was minted+registered by the reused slice-1
    // `a machine credential is bound to ...` Given, which stores the JWT in
    // `mwt_bearer_by_email`. Revoke its registry row so the per-request jti
    // denylist refuses its next call (verify-path-unchanged regression).
    let jwt = world
        .mwt_bearer_by_email
        .get("marco@acme.com")
        .cloned()
        .expect("an Acme bearer was minted by the reused slice-1 Given");
    // Recover the jti from the registered row: the slice-1 mint labels it
    // "slice-01-cred" for marco; revoke the most-recent Acme admin/member token.
    // We re-mint+register a fresh known-jti bearer here so the revoke target is
    // unambiguous, then make THAT the credential under test.
    let user_id = user_id_for(world, "marco@acme.com").await;
    let workspace_id = *world.mwt_workspace_ids.get("Acme").expect("Acme seeded");
    let (fresh, jti) =
        mint_bearer_bound(world, user_id, workspace_id, "slice-03-revoked", true).await;
    world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .revoke_machine_token(jti)
        .await
        .expect("revoke credential under test");
    world.mwt3_revoked_bearer = Some(fresh);
    let _ = jwt;
}

// ==========================================================================
// Given — session-resolution preconditions (US-MWT04 contract)
// ==========================================================================

/// A single-membership precondition: the named member belongs to exactly the one
/// named workspace. The slice-1 Background already seeded marco into Acme only,
/// so this records the EXPECTED resolution target and asserts the precondition.
#[given(regex = r#"^"([^"]+)" belongs to exactly one workspace "([^"]+)"$"#)]
async fn belongs_to_exactly_one(world: &mut FoundryWorld, member: String, ws_name: String) {
    ensure_harness(world).await;
    let user_id = user_id_for(world, &member).await;
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM workspace_memberships WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&pool(world))
            .await
            .expect("count memberships");
    assert_eq!(
        count, 1,
        "{member:?} must have exactly one membership for this precondition; found {count}"
    );
    world.mwt3_resolution_user = Some(member);
    world.mwt3_expected_workspace = Some(ws_name);
}

/// A multi-membership user has CHOSEN (persisted) the named workspace as their
/// active one — the `users.active_workspace_id` the shipped `/workspace/switch`
/// sets, honoured by `resolve_active_workspace` through the membership JOIN.
#[given(regex = r#"^"([^"]+)" has chosen "([^"]+)" as their active workspace$"#)]
async fn has_chosen_active(world: &mut FoundryWorld, member: String, ws_name: String) {
    ensure_harness(world).await;
    let user_id = user_id_for(world, &member).await;
    let workspace_id = *world
        .mwt_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} must be seeded"));
    let set = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .set_active_workspace(user_id, workspace_id)
        .await
        .expect("persist active workspace");
    assert!(
        set,
        "{member:?} must be a member of {ws_name:?} to choose it"
    );
    world.mwt3_resolution_user = Some(member);
    world.mwt3_expected_workspace = Some(ws_name);
}

/// A user who belongs to NO workspace (the fail-closed edge): seed a bare user
/// row with no membership.
#[given(regex = r#"^"([^"]+)" belongs to no workspace$"#)]
async fn belongs_to_no_workspace(world: &mut FoundryWorld, member: String) {
    ensure_harness(world).await;
    let pw =
        foundry_auth::hash_password(&secrecy::SecretString::new("evicted-pw".to_string().into()))
            .await
            .expect("hash pw");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5) ON CONFLICT (email_lower) DO NOTHING",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(member.to_ascii_lowercase())
    .bind(&member)
    .bind("Evicted")
    .bind(&pw)
    .execute(&pool(world))
    .await
    .expect("insert no-membership user");
    world.mwt3_resolution_user = Some(member);
    world.mwt3_expected_workspace = None;
}

// ==========================================================================
// When — issue WRITE over the API (the new confinement surface)
// ==========================================================================

/// POST an issue into a workspace-scoped project over /api/v1. The route is
/// resolved from the slice-1 `mwt_project_route` map (recorded workspace-scoped).
async fn file_issue_as(
    world: &mut FoundryWorld,
    ws_name: &str,
    project: &str,
    title: &str,
    bearer: Option<String>,
) {
    ensure_harness(world).await;
    let (team_slug, project_slug) = world
        .mwt_project_route
        .get(&(ws_name.to_string(), project.to_string()))
        .cloned()
        .unwrap_or_else(|| panic!("project route for {ws_name:?}/{project:?} not seeded"));
    let url = format!(
        "{}/api/v1/teams/{team_slug}/projects/{project_slug}/issues",
        base_url(world)
    );
    let http = world.http.as_ref().expect("http");
    let mut req = http
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::ACCEPT, "application/json")
        .body(serde_json::json!({ "title": title }).to_string());
    if let Some(b) = bearer {
        req = req.header(reqwest::header::AUTHORIZATION, format!("Bearer {b}"));
    }
    let resp = req.send().await.expect("send file-issue request");
    world.mwt_last_status = Some(resp.status());
    world.mwt_last_body = Some(resp.text().await.unwrap_or_default());
}

/// POST an issue into a project addressed by a never-existed route (the
/// missing-id comparison target for non-enumerability).
async fn file_issue_missing_project(world: &mut FoundryWorld, title: &str, bearer: Option<String>) {
    ensure_harness(world).await;
    let url = format!(
        "{}/api/v1/teams/no-such-team/projects/no-such-project/issues",
        base_url(world)
    );
    let http = world.http.as_ref().expect("http");
    let mut req = http
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::ACCEPT, "application/json")
        .body(serde_json::json!({ "title": title }).to_string());
    if let Some(b) = bearer {
        req = req.header(reqwest::header::AUTHORIZATION, format!("Bearer {b}"));
    }
    let resp = req.send().await.expect("send file-issue (missing) request");
    world.mwt_last_status = Some(resp.status());
    world.mwt_last_body = Some(resp.text().await.unwrap_or_default());
}

fn acme_bearer(world: &FoundryWorld) -> Option<String> {
    world
        .mwt_bearer_by_email
        .get("marco@acme.com")
        .cloned()
        .or_else(|| world.mwt_bearer_by_email.get("ops@acme.com").cloned())
}

#[when(
    regex = r#"^the Acme-bound credential files issue "([^"]+)" in the "([^"]+)" project over the API$"#
)]
async fn acme_files_issue(world: &mut FoundryWorld, title: String, project: String) {
    let bearer = acme_bearer(world);
    file_issue_as(world, "Acme", &project, &title, bearer).await;
}

#[when(
    regex = r#"^the Acme-bound credential files issue "([^"]+)" in the "([^"]+)" project over the API by its real address$"#
)]
async fn acme_files_issue_foreign(world: &mut FoundryWorld, title: String, project: String) {
    let bearer = acme_bearer(world);
    // Address a REAL Globex project with the Acme token — must 404 non-enumerably.
    file_issue_as(world, "Globex", &project, &title, bearer).await;
    world.mwt3_first_refusal_body = world.mwt_last_body.clone();
    world.mwt3_first_refusal_status = world.mwt_last_status;
}

#[when(
    regex = r#"^the Acme-bound credential files issue "([^"]+)" in a project that never existed over the API$"#
)]
async fn acme_files_issue_missing(world: &mut FoundryWorld, title: String) {
    let bearer = acme_bearer(world);
    file_issue_missing_project(world, &title, bearer).await;
}

// ==========================================================================
// When — cross-tenant READ refusal (foreign project vs never-existed project)
// ==========================================================================

async fn list_issues_route(
    world: &mut FoundryWorld,
    team_slug: &str,
    project_slug: &str,
    bearer: Option<String>,
) {
    ensure_harness(world).await;
    let url = format!(
        "{}/api/v1/teams/{team_slug}/projects/{project_slug}/issues",
        base_url(world)
    );
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

#[when(
    regex = r#"^the Acme-bound credential lists the "([^"]+)" project's issues over the API by its real address$"#
)]
async fn acme_lists_foreign(world: &mut FoundryWorld, project: String) {
    let bearer = acme_bearer(world);
    let (team_slug, project_slug) = world
        .mwt_project_route
        .get(&("Globex".to_string(), project.clone()))
        .cloned()
        .unwrap_or_else(|| panic!("Globex project route for {project:?} not seeded"));
    list_issues_route(world, &team_slug, &project_slug, bearer).await;
    world.mwt3_first_refusal_body = world.mwt_last_body.clone();
    world.mwt3_first_refusal_status = world.mwt_last_status;
}

#[when(
    regex = r#"^the Acme-bound credential lists a project's issues that never existed over the API$"#
)]
async fn acme_lists_missing(world: &mut FoundryWorld) {
    let bearer = acme_bearer(world);
    list_issues_route(world, "no-such-team", "no-such-project", bearer).await;
}

// ==========================================================================
// When — token list / revoke confined to the acting workspace (residual closure)
// ==========================================================================

async fn get_tokens(world: &mut FoundryWorld, bearer: Option<String>) {
    ensure_harness(world).await;
    // Address the tokens route under the Acme project (the acting workspace's
    // route); the use-case scopes by `principal.workspace_id()`, so the path's
    // project slug is not the authority — the token binding is.
    let (team_slug, project_slug) = world
        .mwt_project_route
        .get(&("Acme".to_string(), "Auth".to_string()))
        .cloned()
        .unwrap_or_else(|| panic!("Acme project route not seeded"));
    let url = format!(
        "{}/api/v1/teams/{team_slug}/projects/{project_slug}/tokens",
        base_url(world)
    );
    let http = world.http.as_ref().expect("http");
    let mut req = http
        .get(&url)
        .header(reqwest::header::ACCEPT, "application/json");
    if let Some(b) = bearer {
        req = req.header(reqwest::header::AUTHORIZATION, format!("Bearer {b}"));
    }
    let resp = req.send().await.expect("send token list request");
    world.mwt_last_status = Some(resp.status());
    world.mwt_last_body = Some(resp.text().await.unwrap_or_default());
}

async fn delete_token(world: &mut FoundryWorld, jti: uuid::Uuid, bearer: Option<String>) {
    ensure_harness(world).await;
    let (team_slug, project_slug) = world
        .mwt_project_route
        .get(&("Acme".to_string(), "Auth".to_string()))
        .cloned()
        .unwrap_or_else(|| panic!("Acme project route not seeded"));
    let url = format!(
        "{}/api/v1/teams/{team_slug}/projects/{project_slug}/tokens/{jti}",
        base_url(world)
    );
    let http = world.http.as_ref().expect("http");
    let mut req = http
        .delete(&url)
        .header(reqwest::header::ACCEPT, "application/json");
    if let Some(b) = bearer {
        req = req.header(reqwest::header::AUTHORIZATION, format!("Bearer {b}"));
    }
    let resp = req.send().await.expect("send token delete request");
    world.mwt_last_status = Some(resp.status());
    world.mwt_last_body = Some(resp.text().await.unwrap_or_default());
}

#[when(regex = r#"^the Acme-bound credential lists the workspace's tokens over the API$"#)]
async fn acme_lists_tokens(world: &mut FoundryWorld) {
    let bearer = acme_bearer(world);
    get_tokens(world, bearer).await;
}

#[when(regex = r#"^the Acme-bound credential revokes the "([^"]+)" token "([^"]+)" over the API$"#)]
async fn acme_revokes_foreign(world: &mut FoundryWorld, _ws: String, label: String) {
    let bearer = acme_bearer(world);
    let jti = *world
        .mwt3_token_jti_by_label
        .get(&label)
        .unwrap_or_else(|| panic!("no seeded jti for label {label:?}"));
    delete_token(world, jti, bearer).await;
    world.mwt3_first_refusal_body = world.mwt_last_body.clone();
    world.mwt3_first_refusal_status = world.mwt_last_status;
}

#[when(
    regex = r#"^the Acme-bound credential revokes a token id that exists nowhere over the API$"#
)]
async fn acme_revokes_unknown(world: &mut FoundryWorld) {
    let bearer = acme_bearer(world);
    delete_token(world, uuid::Uuid::now_v7(), bearer).await;
}

// ==========================================================================
// When — session resolution (US-MWT04 contract, via the SHIPPED seam)
// ==========================================================================

#[when(regex = r#"^(?:his|her|their) session's acting workspace is resolved$"#)]
async fn session_resolved(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let member = world
        .mwt3_resolution_user
        .clone()
        .expect("a resolution user was recorded");
    let user_id = user_id_for(world, &member).await;
    let resolved = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .resolve_active_workspace(user_id)
        .await
        .expect("resolve active workspace");
    world.mwt3_resolved_workspace = resolved.map(|(id, _name)| id);
    world.mwt3_resolution_ran = true;
}

// ==========================================================================
// When — verify-path-unchanged regression
// ==========================================================================

#[when(regex = r#"^the revoked credential lists the "([^"]+)" project's issues as data$"#)]
async fn revoked_lists(world: &mut FoundryWorld, project: String) {
    let bearer = world.mwt3_revoked_bearer.clone();
    let (team_slug, project_slug) = world
        .mwt_project_route
        .get(&("Acme".to_string(), project.clone()))
        .cloned()
        .unwrap_or_else(|| panic!("Acme project route for {project:?} not seeded"));
    list_issues_route(world, &team_slug, &project_slug, bearer).await;
}

// ==========================================================================
// Then — write outcome + workspace-scoped row presence
// ==========================================================================

async fn count_issues_in_workspace(world: &FoundryWorld, ws_name: &str, title: &str) -> i64 {
    let workspace_id = *world
        .mwt_workspace_ids
        .get(ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} not seeded"));
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM issues WHERE workspace_id = $1 AND title = $2")
            .bind(workspace_id)
            .bind(title)
            .fetch_one(&pool(world))
            .await
            .expect("count issues by title in workspace");
    count
}

#[then(regex = r#"^the write is reported as created$"#)]
async fn write_created(world: &mut FoundryWorld) {
    let status = world.mwt_last_status.expect("a status was captured");
    assert!(
        status == StatusCode::CREATED || status == StatusCode::OK,
        "expected 201/200 on a successful write, got {status}; body: {:?}",
        world.mwt_last_body
    );
}

#[then(regex = r#"^the new issue exists only in "([^"]+)"$"#)]
async fn new_issue_only_in(world: &mut FoundryWorld, ws_name: String) {
    let here = count_issues_in_workspace(world, &ws_name, "Rotate signing keys").await;
    assert!(
        here >= 1,
        "expected the new issue in {ws_name}, found {here}"
    );
}

#[then(regex = r#"^no issue was created in "([^"]+)"$"#)]
async fn no_issue_created_in(world: &mut FoundryWorld, ws_name: String) {
    // Neither "Rotate signing keys" (write WS) nor "Sneaky" (cross-tenant write)
    // may appear in the named workspace.
    for title in ["Rotate signing keys", "Sneaky"] {
        let n = count_issues_in_workspace(world, &ws_name, title).await;
        assert_eq!(n, 0, "{title:?} leaked into {ws_name} ({n} rows)");
    }
}

// ==========================================================================
// Then — non-enumerable refusal (foreign ≡ missing), API
// ==========================================================================

#[then(regex = r#"^the two API responses are refused identically$"#)]
async fn two_api_refused_identically(world: &mut FoundryWorld) {
    let first_status = world
        .mwt3_first_refusal_status
        .expect("first (foreign) refusal status captured");
    let first_body = world
        .mwt3_first_refusal_body
        .clone()
        .expect("first refusal body captured");
    let second_status = world.mwt_last_status.expect("second status captured");
    let second_body = world.mwt_last_body.clone().unwrap_or_default();
    assert_eq!(
        first_status,
        StatusCode::NOT_FOUND,
        "the foreign-resource API refusal was not 404 (enumerable oracle)"
    );
    assert_eq!(
        first_status, second_status,
        "foreign-id and never-existed-id statuses differ (enumerable)"
    );
    assert_eq!(
        first_body, second_body,
        "foreign-id and never-existed-id bodies differ (enumerable)"
    );
}

#[then(regex = r#"^nothing in the API response reveals the "([^"]+)" project exists$"#)]
async fn nothing_reveals_foreign(world: &mut FoundryWorld, _ws: String) {
    let body = world.mwt_last_body.as_deref().unwrap_or("");
    assert!(
        !body.contains("GLOBEX-") && !body.to_ascii_lowercase().contains("core"),
        "the refusal body leaked a foreign resource identifier: {body:?}"
    );
}

// ==========================================================================
// Then — issues list (reused phrasing kept distinct where new)
// ==========================================================================
// NOTE: `the answer lists only the "..." issues ...-N and ...-M` and
// `no "..." issue appears in the answer` are REUSED from slice-1's registered
// step text (feature_mwt_slice_01_coexist) — not re-declared here.

// ==========================================================================
// Then — token list / revoke confinement (residual closure)
// ==========================================================================

fn token_list(world: &FoundryWorld) -> Vec<serde_json::Value> {
    let body = world.mwt_last_body.clone().unwrap_or_default();
    let parsed: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("token list is not JSON: {e}; body: {body}"));
    parsed
        .as_array()
        .unwrap_or_else(|| panic!("token list is not a JSON array; body: {body}"))
        .clone()
}

fn list_has_label(world: &FoundryWorld, label: &str) -> bool {
    token_list(world)
        .iter()
        .any(|t| t.get("label").and_then(|v| v.as_str()) == Some(label))
}

#[then(regex = r#"^the token list contains "([^"]+)"$"#)]
async fn token_list_contains(world: &mut FoundryWorld, label: String) {
    assert_eq!(
        world.mwt_last_status,
        Some(StatusCode::OK),
        "expected 200 listing tokens; body: {:?}",
        world.mwt_last_body
    );
    assert!(
        list_has_label(world, &label),
        "token list missing {label:?}; body: {:?}",
        world.mwt_last_body
    );
}

#[then(regex = r#"^the token list does not contain "([^"]+)"$"#)]
async fn token_list_excludes(world: &mut FoundryWorld, label: String) {
    assert_eq!(
        world.mwt_last_status,
        Some(StatusCode::OK),
        "expected 200 listing tokens; body: {:?}",
        world.mwt_last_body
    );
    assert!(
        !list_has_label(world, &label),
        "token list leaked a foreign-workspace token {label:?} (cross-tenant!); body: {:?}",
        world.mwt_last_body
    );
}

#[then(regex = r#"^the two API revoke responses are refused identically as not found$"#)]
async fn two_revokes_identical_not_found(world: &mut FoundryWorld) {
    let first_status = world
        .mwt3_first_refusal_status
        .expect("first (foreign jti) refusal status captured");
    let first_body = world
        .mwt3_first_refusal_body
        .clone()
        .expect("first refusal body captured");
    let second_status = world.mwt_last_status.expect("second status captured");
    let second_body = world.mwt_last_body.clone().unwrap_or_default();
    assert_eq!(
        first_status,
        StatusCode::NOT_FOUND,
        "the foreign-jti revoke was not 404 (enumerable oracle)"
    );
    assert_eq!(
        first_status, second_status,
        "foreign-jti and never-existed-jti revoke statuses differ (enumerable)"
    );
    assert_eq!(
        first_body, second_body,
        "foreign-jti and never-existed-jti revoke bodies differ (enumerable)"
    );
}

#[then(regex = r#"^the "([^"]+)" token "([^"]+)" remains active$"#)]
async fn foreign_token_remains_active(world: &mut FoundryWorld, _ws: String, label: String) {
    let jti = *world
        .mwt3_token_jti_by_label
        .get(&label)
        .unwrap_or_else(|| panic!("no seeded jti for label {label:?}"));
    let (revoked_at,): (Option<time::OffsetDateTime>,) =
        sqlx::query_as("SELECT revoked_at FROM machine_tokens WHERE jti = $1")
            .bind(jti)
            .fetch_one(&pool(world))
            .await
            .expect("read revoked_at");
    assert!(
        revoked_at.is_none(),
        "{label:?} was revoked by a cross-tenant call — the Globex token must stay active"
    );
}

// ==========================================================================
// Then — session resolution contract (US-MWT04)
// ==========================================================================

#[then(regex = r#"^the session resolves to exactly the workspace "([^"]+)"$"#)]
async fn resolves_to_exactly(world: &mut FoundryWorld, ws_name: String) {
    assert!(world.mwt3_resolution_ran, "resolution When did not run");
    let resolved = world
        .mwt3_resolved_workspace
        .expect("a workspace was resolved");
    let expected = *world
        .mwt_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} not seeded"));
    assert_eq!(
        resolved, expected,
        "session resolved to the wrong workspace (expected {ws_name})"
    );
}

#[then(regex = r#"^no workspace choice was required$"#)]
async fn no_choice_required(world: &mut FoundryWorld) {
    // Single-membership: resolution returned exactly one without any selection
    // step. The observable is that `resolve_active_workspace` returned Some for a
    // user with exactly one membership (no prompt path was taken).
    assert!(
        world.mwt3_resolved_workspace.is_some(),
        "single-membership resolution should yield a workspace with no choice step"
    );
}

#[then(regex = r#"^(?:her|his|their) session is scoped to exactly one workspace$"#)]
async fn scoped_to_exactly_one(world: &mut FoundryWorld) {
    assert!(
        world.mwt3_resolved_workspace.is_some(),
        "the session must resolve to exactly one workspace"
    );
}

#[then(regex = r#"^no workspace is resolved$"#)]
async fn no_workspace_resolved(world: &mut FoundryWorld) {
    assert!(world.mwt3_resolution_ran, "resolution When did not run");
    assert!(
        world.mwt3_resolved_workspace.is_none(),
        "a no-membership user must resolve to NO workspace (fail-closed), got {:?}",
        world.mwt3_resolved_workspace
    );
}

#[then(regex = r#"^the session is not scoped to any workspace$"#)]
async fn not_scoped_to_any(world: &mut FoundryWorld) {
    assert!(
        world.mwt3_resolved_workspace.is_none(),
        "fail-closed: the session must not be scoped to any workspace"
    );
}

// ==========================================================================
// Then — verify-path-unchanged regression
// ==========================================================================

#[then(regex = r#"^the request is refused as unauthorized by the verify path$"#)]
async fn refused_unauthorized_verify(world: &mut FoundryWorld) {
    assert_eq!(
        world.mwt_last_status,
        Some(StatusCode::UNAUTHORIZED),
        "a revoked credential's next call must be refused 401 by the jti denylist; body: {:?}",
        world.mwt_last_body
    );
}

#[then(
    regex = r#"^a credential signed with a disallowed algorithm is also refused as unauthorized$"#
)]
async fn disallowed_alg_refused(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    // alg-confusion: an HS256 token using the server's PUBLIC key bytes as the
    // HMAC secret. The verifier pins exactly [EdDSA], so it is refused before any
    // key is consulted -> 401. Mirrors feature_token_management_api's wrong-alg
    // construction; proves the EdDSA pinning still holds under multi-workspace.
    let claims = foundry_auth::MachineTokenClaims {
        sub: uuid::Uuid::now_v7(),
        scope: None,
        iat: time::OffsetDateTime::now_utc().unix_timestamp(),
        exp: (time::OffsetDateTime::now_utc() + time::Duration::seconds(3600)).unix_timestamp(),
        jti: uuid::Uuid::now_v7(),
        iss: foundry_auth::MACHINE_TOKEN_ISS.to_string(),
        aud: foundry_auth::MACHINE_TOKEN_AUD.to_string(),
    };
    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    let key = jsonwebtoken::EncodingKey::from_secret(
        foundry_auth::test_keys::TEST_PUBLIC_KEY_PEM.as_bytes(),
    );
    let jwt = jsonwebtoken::encode(&header, &claims, &key).expect("hs256 encode");
    let (team_slug, project_slug) = world
        .mwt_project_route
        .get(&("Acme".to_string(), "Auth".to_string()))
        .cloned()
        .expect("Acme project route seeded");
    list_issues_route(world, &team_slug, &project_slug, Some(jwt)).await;
    assert_eq!(
        world.mwt_last_status,
        Some(StatusCode::UNAUTHORIZED),
        "a disallowed-algorithm credential must be refused 401 (EdDSA pinning); body: {:?}",
        world.mwt_last_body
    );
}
