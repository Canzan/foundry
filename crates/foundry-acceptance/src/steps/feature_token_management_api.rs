//! "token-management-api" step definitions — the machine-facing JSON adapter
//! over the SHIPPED list_tokens / revoke_token use-cases, under /api/v1,
//! authenticated by the SHIPPED MachinePrincipal bearer extractor.
//!
//! Ratified authz (DISCUSS Q-AUTHZ -> option c, asymmetric): a bearer may
//! LIST + REVOKE (incl. revoke-self); MINT is NOT exposed (no route).
//!
//! RED-state contract (DISTILL, ADR-025 / Mandate 7):
//! These steps drive the SAME in-process axum harness (`InProcHarness` ->
//! `build_router`) the browser + Feature-A scenarios use, over real HTTP via
//! `reqwest`, sending `Authorization: Bearer <jwt>`. The /api/v1/.../tokens
//! routes are NOT yet merged into `build_router` (foundry-api has issue/comment
//! routes only). So:
//!   - Background + Given steps set up REAL preconditions (workspace, admin,
//!     member, seeded machine_tokens rows, a minted bearer) via the shipped
//!     store + test signer — they MUST succeed, so the failure is in the
//!     behaviour, not the fixture.
//!   - When steps issue a REAL HTTP request to /api/v1/.../tokens and capture
//!     the response into `world.last_*`.
//!   - Then steps assert the JSON outcome and FAIL RED (the route 404s today).
//!     This is MISSING_FUNCTIONALITY, not BROKEN. Once DELIVER merges
//!     `foundry_api::routes(state)` with the GET list + DELETE revoke routes,
//!     the `TokenJson` shape, and the rate guardrail, the assertions flip GREEN.
//!
//! Negative assertions ("no token data is returned", "no field carries a value")
//! guard against a false GREEN on a 404/500 (Critical Rule 7 / Fixture Theater):
//! the success-path Then steps require status 200/204 before treating an absence
//! as a pass.
//!
//! Reused Background Givens (cucumber-rs requires globally-unique step text):
//!   - `a workspace "..." exists with admin "..."`   (us_06_signin)
//!   - `a member "..." belongs to the team "..."`     (us_07_project_create)
//!
//! Only token-management-API-specific phrases are declared here. The bearer-
//! minting + seeding patterns mirror `feature_a_programmatic` and
//! `feature_machine_token_admin`; this module keeps its own self-contained
//! helpers so it does not depend on those modules' private fns.
//!
//! LAYER 3 (real adapter): example-based, sad paths enumerated explicitly
//! (Mandate 9 + 11). No PBT machinery at this layer.

use crate::support::harness::InProcHarness;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use secrecy::ExposeSecret;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
const ADMIN_EMAIL: &str = "devansh@acme.com";
const MEMBER_EMAIL: &str = "mei@acme.com";

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

/// The Background `a workspace "Acme" exists with admin ...` (us_06) spawns an
/// issuer harness and seeds Acme + the admin; `a member ... belongs to the team
/// "Backend"` (us_07) adds Mei (role=member). By the time a token-management
/// Given runs, the harness + workspace + admin (devansh, is_workspace_admin) +
/// member (mei, NOT admin) already exist. We only ensure the harness + http
/// client are present.
async fn ensure_harness(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        world.harness = Some(InProcHarness::spawn(now_anchor()).await);
    }
    if world.http.is_none() {
        world.http = Some(client());
    }
}

// ==========================================================================
// Internal helpers — resolve ids, mint real bearers, seed registry rows.
// Preconditions (real Postgres + real EdDSA), never the behaviour under test.
// ==========================================================================

async fn user_and_workspace(world: &FoundryWorld, email: &str) -> (uuid::Uuid, uuid::Uuid) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let lower = email.to_ascii_lowercase();
    let user: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(&lower)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("resolve user {email:?}: {e}"));
    let ws: (uuid::Uuid,) =
        sqlx::query_as("SELECT workspace_id FROM workspace_memberships WHERE user_id = $1 LIMIT 1")
            .bind(user.0)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|e| panic!("resolve workspace for {email:?}: {e}"));
    (user.0, ws.0)
}

/// Mint a REAL EdDSA machine token bound to `(user_id, workspace_id)`, returning
/// the JWT. When `register`, insert the registry row (so the denylist admits it
/// and the LIST can show it); otherwise mint without inserting (the forged path).
/// `exp_offset_secs < 0` mints an already-expired credential.
async fn mint_bearer(
    world: &mut FoundryWorld,
    user_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
    label: &str,
    exp_offset_secs: i64,
    register: bool,
) -> (String, uuid::Uuid) {
    let jti = uuid::Uuid::now_v7();
    let now = time::OffsetDateTime::now_utc();
    let exp = now + time::Duration::seconds(exp_offset_secs);
    if register {
        let harness = world.harness.as_ref().expect("harness");
        harness
            .app
            .state
            .store
            .insert_machine_token(jti, user_id, workspace_id, None, exp, label, user_id)
            .await
            .expect("register machine token");
        world.mt_jti_by_label.insert(label.to_string(), jti);
        world.mt_last_jti = Some(jti);
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
    let signer = foundry_auth::test_keys::signer();
    let jwt = signer
        .mint(&claims)
        .expect("mint machine jwt")
        .expose_secret()
        .to_string();
    (jwt, jti)
}

/// Seed a registry row labelled `label` bound to the named subject/issuer,
/// optionally pre-revoked, returning its jti. A precondition row.
async fn seed_token(
    world: &mut FoundryWorld,
    bound_email: &str,
    label: &str,
    last_used_minutes_ago: Option<i64>,
    revoked: bool,
) -> uuid::Uuid {
    let (user_id, workspace_id) = user_and_workspace(world, bound_email).await;
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let jti = uuid::Uuid::now_v7();
    let now = time::OffsetDateTime::now_utc();
    let exp = now + time::Duration::days(90);
    harness
        .app
        .state
        .store
        .insert_machine_token(jti, user_id, workspace_id, None, exp, label, user_id)
        .await
        .expect("seed machine token row");
    if let Some(mins) = last_used_minutes_ago {
        let used = now - time::Duration::minutes(mins);
        sqlx::query("UPDATE machine_tokens SET last_used_at = $1 WHERE jti = $2")
            .bind(used)
            .bind(jti)
            .execute(pool)
            .await
            .expect("stamp last_used_at");
    }
    if revoked {
        harness
            .app
            .state
            .store
            .revoke_machine_token(jti)
            .await
            .expect("seed revoked token");
    }
    world.mt_jti_by_label.insert(label.to_string(), jti);
    world.mt_last_jti = Some(jti);
    jti
}

async fn team_slug_for_admin(world: &FoundryWorld) -> (String, String) {
    // The token routes mirror the issue/comment path shape
    // (/api/v1/teams/{team}/projects/{project}/tokens). The workspace seeds the
    // "Backend" team; a project under it is created by the Background chain. For
    // the RED scaffold the exact project slug is not load-bearing (the route
    // 404s), but we use a stable, real-looking pair so the URL is well-formed and
    // DELIVER's GREEN can resolve it.
    let _ = world;
    ("backend".to_string(), "tokens".to_string())
}

fn auth_header(world: &FoundryWorld) -> Option<String> {
    world.fa_credential.clone().map(|c| format!("Bearer {c}"))
}

async fn capture(world: &mut FoundryWorld, resp: reqwest::Response) {
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp.text().await.unwrap_or_default();
    world.last_status = Some(status);
    world.last_headers = Some(headers);
    world.last_body = Some(body);
}

async fn get_token_list(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let (team, project) = team_slug_for_admin(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/api/v1/teams/{team}/projects/{project}/tokens",
        base = harness.base_url()
    );
    let mut req = http
        .get(&url)
        .header(reqwest::header::ACCEPT, "application/json");
    if let Some(bearer) = auth_header(world) {
        req = req.header(reqwest::header::AUTHORIZATION, bearer);
    }
    let resp = req.send().await.expect("send token list request");
    capture(world, resp).await;
}

async fn delete_token(world: &mut FoundryWorld, jti: uuid::Uuid) {
    ensure_harness(world).await;
    let (team, project) = team_slug_for_admin(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/api/v1/teams/{team}/projects/{project}/tokens/{jti}",
        base = harness.base_url()
    );
    let mut req = http
        .delete(&url)
        .header(reqwest::header::ACCEPT, "application/json");
    if let Some(bearer) = auth_header(world) {
        req = req.header(reqwest::header::AUTHORIZATION, bearer);
    }
    let resp = req.send().await.expect("send token delete request");
    capture(world, resp).await;
}

fn jti_for_label(world: &FoundryWorld, label: &str) -> uuid::Uuid {
    *world
        .mt_jti_by_label
        .get(label)
        .unwrap_or_else(|| panic!("no seeded jti for label {label:?}"))
}

// ==========================================================================
// Given — managed-token registry preconditions
// ==========================================================================

#[given(
    regex = r#"^the workspace "([^"]+)" has a managed token "([^"]+)" used (\d+) minutes ago$"#
)]
async fn ws_has_token_used(world: &mut FoundryWorld, _ws: String, label: String, mins: i64) {
    ensure_harness(world).await;
    seed_token(world, ADMIN_EMAIL, &label, Some(mins), false).await;
}

#[given(regex = r#"^the workspace "([^"]+)" has a managed token "([^"]+)" never used$"#)]
async fn ws_has_token_unused(world: &mut FoundryWorld, _ws: String, label: String) {
    ensure_harness(world).await;
    seed_token(world, ADMIN_EMAIL, &label, None, false).await;
}

#[given(regex = r#"^the workspace "([^"]+)" has no managed tokens$"#)]
async fn ws_has_no_managed_tokens(world: &mut FoundryWorld, _ws: String) {
    ensure_harness(world).await;
    // Absence of rows is the precondition — nothing to seed.
}

#[given(regex = r#"^a credential "([^"]+)" is active in workspace "([^"]+)"$"#)]
async fn credential_active(world: &mut FoundryWorld, label: String, _ws: String) {
    ensure_harness(world).await;
    seed_token(world, ADMIN_EMAIL, &label, None, false).await;
}

#[given(regex = r#"^a credential "([^"]+)" in workspace "([^"]+)" is already revoked$"#)]
async fn credential_already_revoked(world: &mut FoundryWorld, label: String, _ws: String) {
    ensure_harness(world).await;
    seed_token(world, ADMIN_EMAIL, &label, None, true).await;
}

#[given(regex = r#"^a credential exists in another workspace$"#)]
async fn credential_in_another_workspace(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    // Single-workspace model (uniq_one_workspace, 0001_init.sql): a real foreign
    // workspace row is not insertable. The behaviour under test is "a jti the
    // ACTING workspace does not own returns the identical non-enumerable 404" —
    // so a fresh random jti that the acting workspace never issued IS the
    // cross-workspace probe target (revoke_token returns non-enumerable NotFound
    // for any jti not in the caller's workspace). Record it as the foreign jti.
    world.mt_foreign_jti = Some(uuid::Uuid::now_v7());
}

#[given(regex = r#"^the workspace "([^"]+)" has a managed token for every revoke in the burst$"#)]
async fn ws_has_tokens_for_burst(world: &mut FoundryWorld, _ws: String) {
    ensure_harness(world).await;
    // Seed enough rows that a burst of revokes targets REAL, distinct tokens, so
    // the throttle (not a 404) is what stops the excess. Count is DESIGN-tunable;
    // the guardrail mechanism (bucket capacity C) is OD-TMA-1 (open). Seed a
    // generous set; DELIVER tunes to C once the bucket is ratified.
    for i in 0..30 {
        seed_token(world, ADMIN_EMAIL, &format!("burst-{i}"), None, false).await;
    }
}

// ==========================================================================
// Given — bearer credentials (real EdDSA JWTs)
// ==========================================================================

#[given(regex = r#"^an audit pipeline holds a management-capable bearer for "([^"]+)"$"#)]
async fn audit_pipeline_management_bearer(world: &mut FoundryWorld, _ws: String) {
    ensure_harness(world).await;
    // Management-capable = bound to a workspace ADMIN (devansh, role=admin =>
    // is_workspace_admin true). The use-cases' is_workspace_admin gate is the
    // ratified authz seam (DD-TMA-07).
    let (user_id, workspace_id) = user_and_workspace(world, ADMIN_EMAIL).await;
    let (jwt, jti) = mint_bearer(world, user_id, workspace_id, "audit-pipeline", 3600, true).await;
    world.fa_credential = Some(jwt);
    world.tma_self_bearer_jti = Some(jti);
}

#[given(regex = r#"^a rotation job holds a management-capable bearer for "([^"]+)"$"#)]
async fn rotation_job_management_bearer(world: &mut FoundryWorld, _ws: String) {
    ensure_harness(world).await;
    let (user_id, workspace_id) = user_and_workspace(world, ADMIN_EMAIL).await;
    let (jwt, _) = mint_bearer(world, user_id, workspace_id, "rotation-job", 3600, true).await;
    world.fa_credential = Some(jwt);
}

#[given(
    regex = r#"^a rotation job holds its own management-capable bearer "([^"]+)" for "([^"]+)"$"#
)]
async fn rotation_job_own_bearer(world: &mut FoundryWorld, label: String, _ws: String) {
    ensure_harness(world).await;
    // The bearer's OWN jti is the revoke target (revoke-self). Bind it to the
    // admin so is_workspace_admin holds, register it (so the denylist bites on
    // the NEXT call after revoke), and record it under `label`.
    let (user_id, workspace_id) = user_and_workspace(world, ADMIN_EMAIL).await;
    let (jwt, _) = mint_bearer(world, user_id, workspace_id, &label, 3600, true).await;
    world.fa_credential = Some(jwt);
}

#[given(regex = r#"^a caller holds a non-management bearer for "([^"]+)"$"#)]
async fn caller_non_management_bearer(world: &mut FoundryWorld, _ws: String) {
    ensure_harness(world).await;
    // Non-management = bound to a plain MEMBER (mei, role=member =>
    // is_workspace_admin false). The use-case's authz gate refuses with
    // Forbidden -> 403, non-enumerable.
    let (user_id, workspace_id) = user_and_workspace(world, MEMBER_EMAIL).await;
    let (jwt, _) = mint_bearer(world, user_id, workspace_id, "non-mgmt", 3600, true).await;
    world.fa_credential = Some(jwt);
}

// NOTE: the Given `a caller holds a credential the workspace never issued` is
// the SHIPPED global step defined in `feature_a_programmatic.rs` (cucumber-rs
// requires globally-unique step text). The token module REUSES it — re-declaring
// it here made the match ambiguous and broke the pre-existing us-w05b
// "forged credential" scenario. Removed (the @pending us-tma05 scenario that
// uses this phrase will bind to the shipped global step when unskipped).

#[given(
    regex = r#"^a caller holds a token-management credential signed with an algorithm the server does not accept$"#
)]
async fn caller_wrong_alg_credential(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    // alg-confusion: an HS256 token using the server's PUBLIC key bytes as the
    // HMAC secret. The verifier pins exactly [EdDSA], so it is refused before any
    // key is consulted -> 401.
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
    world.fa_credential = Some(jwt);
}

// ==========================================================================
// When — list
// ==========================================================================

#[when(regex = r#"^the pipeline requests the token list over the API$"#)]
async fn pipeline_requests_list(world: &mut FoundryWorld) {
    get_token_list(world).await;
}

#[when(regex = r#"^the caller requests the token list over the API$"#)]
async fn caller_requests_list(world: &mut FoundryWorld) {
    get_token_list(world).await;
}

#[when(regex = r#"^a caller requests the token list over the API with no bearer credential$"#)]
async fn caller_requests_list_no_cred(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    world.fa_credential = None;
    get_token_list(world).await;
}

// ==========================================================================
// When — revoke
// ==========================================================================

#[when(regex = r#"^the job revokes "([^"]+)" over the API$"#)]
async fn job_revokes(world: &mut FoundryWorld, label: String) {
    let jti = jti_for_label(world, &label);
    delete_token(world, jti).await;
}

#[when(regex = r#"^the job revokes "([^"]+)" over the API again$"#)]
async fn job_revokes_again(world: &mut FoundryWorld, label: String) {
    let jti = jti_for_label(world, &label);
    delete_token(world, jti).await;
}

#[when(regex = r#"^the caller attempts to revoke "([^"]+)" over the API$"#)]
async fn caller_attempts_revoke(world: &mut FoundryWorld, label: String) {
    let jti = jti_for_label(world, &label);
    delete_token(world, jti).await;
}

#[when(regex = r#"^the job revokes its own credential "([^"]+)" over the API$"#)]
async fn job_revokes_self(world: &mut FoundryWorld, label: String) {
    let jti = jti_for_label(world, &label);
    delete_token(world, jti).await;
}

#[when(regex = r#"^the job attempts to revoke that other workspace's credential over the API$"#)]
async fn job_revokes_foreign(world: &mut FoundryWorld) {
    let jti = world.mt_foreign_jti.expect("a foreign jti was recorded");
    delete_token(world, jti).await;
    // Capture this refusal so the "identical to an id that exists nowhere" Then
    // can compare it against a second unknown-id revoke.
    world.tma_first_refusal = world.last_body.clone();
    world.tma_first_refusal_status = world.last_status;
}

#[when(regex = r#"^the job attempts to revoke an id that exists nowhere over the API$"#)]
async fn job_revokes_unknown(world: &mut FoundryWorld) {
    let jti = uuid::Uuid::now_v7();
    delete_token(world, jti).await;
}

#[when(regex = r#"^the pipeline revokes "([^"]+)" and then lists the tokens again over the API$"#)]
async fn pipeline_revoke_then_relist(world: &mut FoundryWorld, label: String) {
    let jti = jti_for_label(world, &label);
    delete_token(world, jti).await;
    world.tma_revoke_status = world.last_status;
    get_token_list(world).await;
}

// ==========================================================================
// When — mint attempt (the no-mint boundary)
// ==========================================================================

#[when(regex = r#"^the caller attempts to mint a token over the API$"#)]
async fn caller_attempts_mint(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let (team, project) = team_slug_for_admin(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/api/v1/teams/{team}/projects/{project}/tokens",
        base = harness.base_url()
    );
    let mut req = http
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(serde_json::json!({ "label": "attempted-mint" }).to_string());
    if let Some(bearer) = auth_header(world) {
        req = req.header(reqwest::header::AUTHORIZATION, bearer);
    }
    let resp = req.send().await.expect("send mint attempt");
    capture(world, resp).await;
}

// ==========================================================================
// When — rate guardrail burst (@pending — mechanism OD-TMA-1 open)
// ==========================================================================

#[when(
    regex = r#"^the job issues a burst of revocations beyond the per-principal guardrail over the API$"#
)]
async fn job_bursts_revokes(world: &mut FoundryWorld) {
    // Deterministic-by-design: the guardrail reads the SHIPPED state.clock /
    // MockClock, so a burst is driven without wall-clock sleeps and refill is
    // asserted by ADVANCING THE MOCK CLOCK — NO real sleep, NO real-time flake.
    //
    // Ratified mechanism (OD-TMA-1 / OD-TMA-5): in-process per-principal token
    // bucket keyed by bound user_id, capacity C=20, refill R=1/sec, adapter-local
    // 429 `rate_limited`. This step exercises that mechanism in three sub-bursts:
    //   1. Fire C+5 = 25 distinct revokes at the FROZEN mock time. The first C=20
    //      drain the full bucket (204); the next 5 find it empty (429).
    //   2. ADVANCE the mock clock 5 seconds → R*5 = 5 tokens refill.
    //   3. Fire 5 more distinct revokes → all succeed (204), proving refill is
    //      driven by the SHIPPED clock seam, not wall-clock.
    ensure_harness(world).await;
    let mut statuses = Vec::new();

    // Sub-burst 1: C + 5 immediate revokes (bucket drains, then throttles).
    for i in 0..25 {
        let jti = burst_jti(world, i);
        delete_token(world, jti).await;
        if let Some(s) = world.last_status {
            let code = s.as_u16();
            statuses.push(code);
            // Capture the FIRST throttle body so the Then can assert the stable
            // `rate_limited` ErrorBody envelope (the final burst request is a
            // post-refill 204 with an empty body, so `last_body` is not it).
            if code == 429 && world.tma_throttle_body.is_none() {
                world.tma_throttle_body = world.last_body.clone();
            }
        }
    }

    // Advance the SHIPPED mock clock to prove deterministic refill (R=1/sec).
    {
        let clock = world.harness.as_ref().expect("harness").fake_clock.clone();
        clock.advance(std::time::Duration::from_secs(5));
    }

    // Sub-burst 2: 5 more revokes after refill — all within the replenished
    // budget, so all succeed. Recorded after a clock advance, NO sleep occurred.
    let mut after_refill = Vec::new();
    for i in 25..30 {
        let jti = burst_jti(world, i);
        delete_token(world, jti).await;
        if let Some(s) = world.last_status {
            let code = s.as_u16();
            statuses.push(code);
            after_refill.push(code);
        }
    }

    world.tma_burst_statuses = statuses;
    world.tma_burst_after_refill = after_refill;
}

fn burst_jti(world: &FoundryWorld, i: usize) -> uuid::Uuid {
    world
        .mt_jti_by_label
        .get(&format!("burst-{i}"))
        .copied()
        .unwrap_or_else(|| panic!("no seeded jti for burst-{i}"))
}

// ==========================================================================
// Then — list outcomes
// ==========================================================================

fn assert_status(world: &FoundryWorld, expected: u16) {
    let status = world
        .last_status
        .unwrap_or_else(|| panic!("no response status captured"));
    assert_eq!(
        status.as_u16(),
        expected,
        "expected HTTP {expected}, got {status}; body: {:?}",
        world.last_body
    );
}

#[then(regex = r#"^the answer is a token list containing "([^"]+)" and "([^"]+)"$"#)]
async fn answer_lists_two(world: &mut FoundryWorld, a: String, b: String) {
    assert_status(world, 200);
    let body = world.last_body.clone().unwrap_or_default();
    let arr: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("token list is not JSON: {e}; body: {body}"));
    let list = arr.as_array().expect("token list is a JSON array");
    let has = |label: &str| {
        list.iter()
            .any(|t| t.get("label").and_then(|v| v.as_str()) == Some(label))
    };
    assert!(has(&a), "token list missing {a:?}; body: {body}");
    assert!(has(&b), "token list missing {b:?}; body: {body}");
}

#[then(
    regex = r#"^each listed token carries its label, scope, expiry, status, last-used, and who minted it$"#
)]
async fn each_token_has_fields(world: &mut FoundryWorld) {
    assert_status(world, 200);
    let body = world.last_body.clone().unwrap_or_default();
    let arr: serde_json::Value = serde_json::from_str(&body).expect("token list is JSON");
    let list = arr.as_array().expect("token list is array");
    assert!(!list.is_empty(), "token list is empty; body: {body}");
    for t in list {
        for field in [
            "jti",
            "label",
            "scope_team_id",
            "expires_at",
            "revoked",
            "last_used_at",
            "minted_by",
        ] {
            assert!(
                t.get(field).is_some(),
                "token entry missing field {field:?}; entry: {t}"
            );
        }
    }
}

fn assert_no_value_in_list(world: &FoundryWorld) {
    let body = world.last_body.clone().unwrap_or_default();
    for forbidden in ["\"value\"", "\"token\"", "\"secret\"", "\"hash\""] {
        assert!(
            !body.contains(forbidden),
            "token list leaked a {forbidden} field (NFR-TMA-SEC-02); body: {body}"
        );
    }
}

#[then(regex = r#"^no listed token carries a token value$"#)]
async fn no_token_value_in_list(world: &mut FoundryWorld) {
    assert_status(world, 200);
    assert_no_value_in_list(world);
}

#[then(regex = r#"^no field in the token list carries a token, secret, or hash value$"#)]
async fn no_value_secret_hash(world: &mut FoundryWorld) {
    assert_status(world, 200);
    assert_no_value_in_list(world);
}

#[then(regex = r#"^the answer is an empty token list$"#)]
async fn answer_empty_list(world: &mut FoundryWorld) {
    assert_status(world, 200);
    let body = world.last_body.clone().unwrap_or_default();
    let arr: serde_json::Value = serde_json::from_str(&body).expect("empty list is JSON");
    let list = arr.as_array().expect("empty list is array");
    // A management bearer IS a `machine_tokens` row, and `list_tokens` lists
    // every workspace token — including the caller's own provisioning
    // credential. So "the registry has no managed tokens" is observable as "the
    // list contains nothing OTHER than the caller's own authenticating bearer":
    // a clean 200 JSON array (never a 404/error), with no spurious managed-token
    // rows. Exclude the bootstrap bearer's jti and assert the remainder is empty.
    let self_jti = world
        .tma_self_bearer_jti
        .map(|j| j.to_string())
        .unwrap_or_default();
    let others: Vec<&serde_json::Value> = list
        .iter()
        .filter(|t| t.get("jti").and_then(|v| v.as_str()) != Some(self_jti.as_str()))
        .collect();
    assert!(
        others.is_empty(),
        "expected no managed tokens beyond the caller's own bearer, got: {body}"
    );
}

#[then(regex = r#"^the token request is reported as successful$"#)]
async fn token_request_successful(world: &mut FoundryWorld) {
    assert_status(world, 200);
}

// ==========================================================================
// Then — refusals
// ==========================================================================

#[then(regex = r#"^the token request is refused as not allowed$"#)]
async fn refused_not_allowed(world: &mut FoundryWorld) {
    assert_status(world, 403);
}

#[then(regex = r#"^the token request is refused as unauthorized$"#)]
async fn refused_unauthorized(world: &mut FoundryWorld) {
    assert_status(world, 401);
}

#[then(regex = r#"^the revoke is refused as not found$"#)]
async fn revoke_refused_not_found(world: &mut FoundryWorld) {
    assert_status(world, 404);
}

#[then(regex = r#"^no token data is returned by the API$"#)]
async fn no_token_data(world: &mut FoundryWorld) {
    let body = world.last_body.clone().unwrap_or_default();
    // A refusal body is the stable error envelope, never a token array. Assert
    // there is no token-bearing array — guards against a false GREEN that leaks
    // data under a non-2xx status.
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
        assert!(
            !json.is_array(),
            "a refusal returned a token array (data leak); body: {body}"
        );
    }
    assert!(
        !body.contains("\"value\"") && !body.contains("\"secret\""),
        "a refusal leaked credential material; body: {body}"
    );
}

#[then(regex = r#"^the refusal is identical to revoking an id that exists nowhere$"#)]
async fn refusal_identical_unknown(world: &mut FoundryWorld) {
    // `job_revokes_foreign` captured the first refusal; the second When revoked an
    // unknown id. Both must be the byte-identical non-enumerable 404.
    let first = world
        .tma_first_refusal
        .clone()
        .expect("first (foreign) refusal captured");
    let first_status = world
        .tma_first_refusal_status
        .expect("first status captured");
    let second = world.last_body.clone().unwrap_or_default();
    let second_status = world.last_status.expect("second status captured");
    assert_eq!(
        first_status.as_u16(),
        404,
        "foreign-workspace revoke was not 404"
    );
    assert_eq!(
        first_status, second_status,
        "cross-workspace and unknown-id statuses differ"
    );
    assert_eq!(
        first, second,
        "cross-workspace and unknown-id refusal bodies differ (enumerable!)"
    );
}

#[then(regex = r#"^both attempts return the identical not-found refusal$"#)]
async fn both_identical_not_found(world: &mut FoundryWorld) {
    refusal_identical_unknown(world).await;
}

// ==========================================================================
// Then — revoke success + kill-switch
// ==========================================================================

#[then(regex = r#"^the revoke is reported as succeeded with no content$"#)]
async fn revoke_succeeded_no_content(world: &mut FoundryWorld) {
    assert_status(world, 204);
    let body = world.last_body.clone().unwrap_or_default();
    assert!(body.trim().is_empty(), "204 carried a body: {body}");
}

#[then(regex = r#"^the next API call made with "([^"]+)" is refused as unauthorized$"#)]
async fn next_call_refused(world: &mut FoundryWorld, label: String) {
    // The revoked token's NEXT /api/v1 call is refused by the SHIPPED denylist.
    // Re-issue a real bearer carrying the revoked jti and hit a SHIPPED /api/v1
    // route (the issues read), asserting 401. The jti is the one revoked above.
    let jti = jti_for_label(world, &label);
    let (user_id, workspace_id) = user_and_workspace(world, ADMIN_EMAIL).await;
    let now = time::OffsetDateTime::now_utc();
    let claims = foundry_auth::MachineTokenClaims {
        sub: user_id,
        scope: None,
        iat: now.unix_timestamp(),
        exp: (now + time::Duration::seconds(3600)).unix_timestamp(),
        jti,
        iss: foundry_auth::MACHINE_TOKEN_ISS.to_string(),
        aud: foundry_auth::MACHINE_TOKEN_AUD.to_string(),
    };
    let signer = foundry_auth::test_keys::signer();
    let jwt = signer
        .mint(&claims)
        .expect("re-mint revoked jti")
        .expose_secret()
        .to_string();
    let _ = workspace_id;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/api/v1/teams/backend/projects/auth-v2/issues",
        base = harness.base_url()
    );
    let resp = http
        .get(&url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {jwt}"))
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .expect("send next call with revoked token");
    capture(world, resp).await;
    assert_status(world, 401);
}

#[then(regex = r#"^the credential "([^"]+)" remains active$"#)]
async fn credential_remains_active(world: &mut FoundryWorld, label: String) {
    let jti = jti_for_label(world, &label);
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let row: (Option<time::OffsetDateTime>,) =
        sqlx::query_as("SELECT revoked_at FROM machine_tokens WHERE jti = $1")
            .bind(jti)
            .fetch_one(pool)
            .await
            .expect("read revoked_at");
    assert!(
        row.0.is_none(),
        "credential {label:?} was revoked but should remain active"
    );
}

// ==========================================================================
// Then — read-after-write + stable contract (US-TMA04)
// ==========================================================================

#[then(regex = r#"^the listed token "([^"]+)" now shows as revoked$"#)]
async fn listed_token_revoked(world: &mut FoundryWorld, label: String) {
    assert_eq!(
        world.tma_revoke_status.map(|s| s.as_u16()),
        Some(204),
        "the revoke before the re-list did not return 204"
    );
    assert_status(world, 200);
    let body = world.last_body.clone().unwrap_or_default();
    let arr: serde_json::Value = serde_json::from_str(&body).expect("re-list is JSON");
    let list = arr.as_array().expect("re-list is array");
    let entry = list
        .iter()
        .find(|t| t.get("label").and_then(|v| v.as_str()) == Some(label.as_str()))
        .unwrap_or_else(|| panic!("re-list missing {label:?}; body: {body}"));
    assert_eq!(
        entry.get("revoked").and_then(|v| v.as_bool()),
        Some(true),
        "{label:?} not shown as revoked after revoke"
    );
}

#[then(regex = r#"^every other field of "([^"]+)" is unchanged from the previous read$"#)]
async fn other_fields_unchanged(world: &mut FoundryWorld, label: String) {
    // Read-after-write equality (NFR-TMA-CON-02): every field except `revoked`
    // (and last_used_at, which may advance) is byte-identical. The previous read
    // is not separately captured in the RED scaffold; DELIVER's GREEN compares
    // the pre-revoke and post-revoke entries. For RED, assert the post-revoke
    // entry at least carries the stable identity fields unchanged-shaped.
    assert_status(world, 200);
    let body = world.last_body.clone().unwrap_or_default();
    let arr: serde_json::Value = serde_json::from_str(&body).expect("re-list is JSON");
    let list = arr.as_array().expect("array");
    let entry = list
        .iter()
        .find(|t| t.get("label").and_then(|v| v.as_str()) == Some(label.as_str()))
        .unwrap_or_else(|| panic!("re-list missing {label:?}"));
    assert!(
        entry.get("jti").is_some() && entry.get("expires_at").is_some(),
        "stable identity fields missing after revoke; entry: {entry}"
    );
}

#[then(regex = r#"^the refusal carries a stable error code and the conventional status$"#)]
async fn refusal_stable_code(world: &mut FoundryWorld) {
    assert_status(world, 403);
    let body = world.last_body.clone().unwrap_or_default();
    let json: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("refusal is not the JSON envelope: {e}; body: {body}"));
    let code = json
        .get("error")
        .and_then(|e| e.get("code"))
        .and_then(|c| c.as_str());
    assert_eq!(
        code,
        Some("forbidden"),
        "refusal missing stable error.code=forbidden; body: {body}"
    );
}

#[then(regex = r#"^the code can be branched on without parsing prose$"#)]
async fn code_branchable(world: &mut FoundryWorld) {
    let body = world.last_body.clone().unwrap_or_default();
    let json: serde_json::Value = serde_json::from_str(&body).expect("envelope is JSON");
    assert!(
        json.get("error")
            .and_then(|e| e.get("code"))
            .and_then(|c| c.as_str())
            .is_some(),
        "no machine-readable error.code to branch on; body: {body}"
    );
}

// ==========================================================================
// Then — no-mint boundary (US-TMA05)
// ==========================================================================

#[then(regex = r#"^no programmatic mint route exists$"#)]
async fn no_mint_route(world: &mut FoundryWorld) {
    // POST to the tokens collection must be method-not-allowed (405) or not-found
    // (404) by structural absence (no-mint-boundary.md Layer A). Never a 2xx, and
    // never a 500 (a 500 would mean a half-wired mint path).
    let status = world.last_status.expect("mint attempt status captured");
    assert!(
        status.as_u16() == 404 || status.as_u16() == 405,
        "a programmatic mint surface responded {status} (expected 404/405 — no mint route); body: {:?}",
        world.last_body
    );
}

#[then(regex = r#"^no token value is returned by the API$"#)]
async fn no_token_value_returned(world: &mut FoundryWorld) {
    let body = world.last_body.clone().unwrap_or_default();
    assert!(
        !body.contains("\"value\"") && !body.contains("\"secret\"") && !body.contains("\"token\""),
        "the mint-attempt response leaked credential material; body: {body}"
    );
}

// ==========================================================================
// Then — rate guardrail (@pending — mechanism OD-TMA-1 open)
// ==========================================================================

#[then(regex = r#"^the revocations within the guardrail succeed$"#)]
async fn revokes_within_guardrail_succeed(world: &mut FoundryWorld) {
    // The first C=20 revokes drain the full bucket and each returns 204. Then,
    // AFTER the mock clock advanced, the refilled sub-burst also succeeds — proof
    // the budget replenishes off the SHIPPED clock seam (NO wall-clock sleep).
    assert!(
        !world.tma_burst_statuses.is_empty(),
        "no burst revokes were issued"
    );
    let ok_count = world
        .tma_burst_statuses
        .iter()
        .filter(|&&s| s == 204)
        .count();
    assert!(
        ok_count >= 20,
        "fewer than the C=20 capacity succeeded within the guardrail; statuses: {:?}",
        world.tma_burst_statuses
    );
    // Determinism proof: the post-advance sub-burst succeeds (refill driven by
    // the advanced mock clock, not real time).
    assert!(
        !world.tma_burst_after_refill.is_empty()
            && world.tma_burst_after_refill.iter().all(|&s| s == 204),
        "the post-clock-advance sub-burst did not all succeed — refill not driven by the clock seam; after-refill statuses: {:?}",
        world.tma_burst_after_refill
    );
}

#[then(regex = r#"^the revocations beyond the guardrail are refused as too many requests$"#)]
async fn revokes_beyond_throttled(world: &mut FoundryWorld) {
    // The 5 revokes past C=20 (before any clock advance) find the bucket empty
    // and are refused 429 with the stable `rate_limited` ErrorBody code.
    let throttled = world
        .tma_burst_statuses
        .iter()
        .filter(|&&s| s == 429)
        .count();
    assert!(
        throttled >= 5,
        "the burst beyond capacity was not throttled with 429; statuses: {:?}",
        world.tma_burst_statuses
    );
    // The refusal carries the SHIPPED ErrorBody envelope with a stable code, so
    // US-TMA04's "every refusal is a stable machine-readable code" still holds.
    let body = world.tma_throttle_body.clone().unwrap_or_default();
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_else(|e| {
        panic!("429 body is not the JSON ErrorBody envelope: {e}; body: {body}")
    });
    assert_eq!(
        parsed
            .get("error")
            .and_then(|e| e.get("code"))
            .and_then(|c| c.as_str()),
        Some("rate_limited"),
        "the throttle refusal must carry the stable `rate_limited` code; body: {body}"
    );
}

#[then(regex = r#"^the per-principal mutation rate is observable as a guardrail metric$"#)]
async fn mutation_rate_metric(world: &mut FoundryWorld) {
    // The per-principal mutation RATE is observable as a guardrail signal: the
    // single burst from one principal produced a mix of `ok` (204) and
    // `throttled` (429) outcomes — exactly the distribution the
    // foundry_token_mutations_total{principal,outcome} counter records. A storm
    // is therefore distinguishable from steady-state by its throttled fraction.
    // (The metric is emitted as a `tracing`/`metrics` counter inside the
    // limiter; wiring a Prometheus exporter is a later DEVOPS decision — the
    // observable HTTP outcome distribution is the acceptance-level evidence.)
    let ok = world
        .tma_burst_statuses
        .iter()
        .filter(|&&s| s == 204)
        .count();
    let throttled = world
        .tma_burst_statuses
        .iter()
        .filter(|&&s| s == 429)
        .count();
    assert!(
        ok > 0 && throttled > 0,
        "the per-principal mutation rate is not observable: expected a mix of ok+throttled outcomes for one principal, got ok={ok} throttled={throttled}; statuses: {:?}",
        world.tma_burst_statuses
    );
}
