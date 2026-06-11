//! multi-workspace-tenancy — Slice 4 (cross-tenant NON-ENUMERABILITY hardening)
//! step definitions.
//!
//! Slices 2-3 proved the isolation boundary PER SURFACE (the web tier; the JSON
//! /api/v1 with machine-token + session resolution), each on the highest-traffic
//! representative path. This slice does NOT add a new surface — it UNIFIES and
//! PROVES, with an explicit ADVERSARIAL MATRIX, that the cross-tenant refusal is
//! OBSERVATIONALLY IDENTICAL to a never-existed id on EVERY remaining
//! tenant-scoped surface: same status, same body, no 403-vs-404 oracle, no
//! id/slug echo (US-MWT05 / NFR-MWT-SEC-02 / ADR-003).
//!
//! NEW surfaces this slice sweeps (not individually exercised by slices 2-3):
//!   - Web comment write     POST /team/{t}/project/{p}/issues/{n}/comments
//!   - Web state-change      POST /team/{t}/project/{p}/issues/{n}/state
//!   - Web attachment upload POST /team/{t}/project/{p}/issues/{n}/attachments
//!   - Web attachment dl     GET  /team/{t}/project/{p}/issues/{n}/attachments/{id}
//!   - API state-change      PATCH /api/v1/teams/{t}/projects/{p}/issues/{n}
//!   - API comment write     POST  /api/v1/teams/{t}/projects/{p}/issues/{n}/comments
//!
//! Plus the oracle-hunt no-403 / no-echo invariants gathered across surfaces.
//!
//! All of these funnel through the SHIPPED `find_team_by_slug(workspace_id, ..)`
//! -> `find_project_by_slug(workspace_id, ..)` scoping chain (and
//! `find_attachment_for_requester` for the download surface), which returns
//! `None`/`NotFound` IDENTICALLY for a foreign-workspace id and a never-existed
//! id (ADR-003 option (b)). The web handler renders the SAME 404 page; the API
//! handler returns the SAME `status_for` 404 JSON envelope.
//!
//! Timing (ADR-003): the foreign-id and missing-id paths run the SAME
//! `WHERE id AND workspace_id` query, sharing a timing profile BY CONSTRUCTION.
//! Asserted STRUCTURALLY (status + body identity => the same None path) — NOT by
//! flaky wall-clock measurement (which would be unstable under @all load).
//!
//! RED-state contract (DISTILL, ADR-025 / Mandate 7): the crate COMPILES (no
//! import/collection error => not BROKEN). At runtime against real testcontainers
//! PG16 the genuine RED is MISSING_FUNCTIONALITY (the `0002` guard drop, shared
//! with slices 1-3); a matrix cell that reds for a REAL oracle (a 403, a body
//! echo, a shape diff) is flagged in distill/slice-04-upstream-issues.md.
//!
//! Step-text reuse (cucumber-rs requires globally-unique step text — a reused
//! step is bound by matching its exact registered regex, NOT re-declared here).
//!
//! REUSED from slice 1: the two-workspace SEED Background; the bearer-bind
//! `a machine credential is bound to ...` Given.
//!
//! REUSED from slice 2: `"<email>" is signed in on the web acting on workspace
//! "<ws>"`; the foreign-vs-missing web board/issue/file Whens; the
//! admin-revoke-foreign When + `the "<ws>" workspace has an admin credential
//! "<label>"`; `the two web responses are refused identically` (reads
//! `mwt2_first_refusal_*` vs `mwt2_last_*` — this module's NEW web Whens populate
//! EXACTLY those slots, so the slice-2 assertion is reused verbatim);
//! `no "<ws>" data appears on the web`; `no "<ws>" membership or credential is
//! changed`.
//!
//! REUSED from slice 3: `a managed token "<label>" exists in workspace "<ws>"`;
//! `the Acme-bound credential lists the "<p>" project's issues over the API by
//! its real address`; the API token-revoke Whens; `the two API responses are
//! refused identically` (reads `mwt3_first_refusal_*` vs `mwt_last_*`); `the two
//! API revoke responses are refused identically as not found`; `the "<ws>" token
//! "<label>" remains active`.
//!
//! NEW slice-4 text: the comment/state/attachment web writes; the API
//! state/comment writes; the attachment-download reads; the never-existed-
//! credential web revoke comparator; the no-foreign-identifier / no-foreign-
//! mutation asserts; the oracle-hunt no-403 / all-404 asserts.
//!
//! LAYER 3 (real adapter): example-based, adversarial paths enumerated
//! explicitly (Mandates 9 + 11). No PBT machinery at this layer.

use crate::support::harness::{signed_in_post, InProcHarness};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use reqwest::StatusCode;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
/// Same web password the slice-2 personas sign in with (the slice-2 seeds set
/// this hash on each persona; this module reuses the slice-2 sign-in path).
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

/// Additive harness guard (mirrors slices 1-3): spawn ONCE per scenario, never
/// reset on subsequent calls (the Background seeds two workspaces; a reset would
/// discard the first).
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

fn base_url(world: &FoundryWorld) -> String {
    world.harness.as_ref().expect("harness").base_url()
}

fn project_route(world: &FoundryWorld, ws: &str, project: &str) -> (String, String) {
    world
        .mwt_project_route
        .get(&(ws.to_string(), project.to_string()))
        .cloned()
        .unwrap_or_else(|| panic!("project route for {ws:?}/{project:?} not seeded"))
}

fn issue_number(key: &str) -> i32 {
    key.rsplit('-')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("issue key {key:?} must end in -<n>"))
}

fn acme_bearer(world: &FoundryWorld) -> Option<String> {
    world
        .mwt_bearer_by_email
        .get("marco@acme.com")
        .cloned()
        .or_else(|| world.mwt_bearer_by_email.get("ops@acme.com").cloned())
}

/// Record a foreign identifier that MUST NOT leak into any refusal body (the
/// oracle-hunt no-echo assertion). De-duplicated.
fn note_foreign_identifier(world: &mut FoundryWorld, s: &str) {
    let s = s.to_string();
    if !world.mwt4_foreign_identifiers.contains(&s) {
        world.mwt4_foreign_identifiers.push(s);
    }
}

/// Push a cross-tenant refusal status into the oracle-hunt accumulator (no-403 /
/// all-404). Only the FOREIGN reach feeds this — the never-existed comparator is
/// definitionally a 404 and is not a cross-tenant refusal.
fn note_refusal_status(world: &mut FoundryWorld, status: Option<StatusCode>) {
    if let Some(s) = status {
        world.mwt4_refusal_statuses.push(s);
    }
}

// ==========================================================================
// Web sign-in (mirrors slice-2's cookie path; self-contained so this module
// does not depend on slice-2's private helpers).
// ==========================================================================

async fn sign_in_cookie(world: &FoundryWorld, email: &str) -> String {
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let base = harness.base_url();

    let csrf_get = http
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("get /sign-in for csrf");
    let csrf_token = extract_csrf(&csrf_get);

    let mut form: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
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

fn extract_csrf(resp: &reqwest::Response) -> String {
    resp.headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .and_then(|s| s.strip_prefix("foundry_csrf="))
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string()
}

fn web_email(world: &FoundryWorld) -> String {
    world
        .mwt2_web_email
        .clone()
        .expect("a slice-4 web persona is signed in (reused slice-2 Given)")
}

async fn web_get(world: &FoundryWorld, path: &str) -> (StatusCode, String) {
    let email = web_email(world);
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

// ==========================================================================
// __SCAFFOLD__ — slice-4 matrix steps.
//
// SCAFFOLD: true
//
// These NEW step definitions exercise REAL HTTP against the in-process harness
// (no stubbed seam), so they are RED-not-BROKEN by construction: the crate
// compiles and the steps run, but the genuine MISSING_FUNCTIONALITY (the `0002`
// guard drop, shared with slices 1-3) makes the Background's second workspace
// insert fail until DELIVER ships it. Once `0002` + the shipped scoping resolve
// the member to Acme, each cell proves the SHIPPED non-enumerable 404 holds
// uniformly. There is no production Rust module for this slice to stub — the
// behaviour under test is shipped + proven; this slice asserts UNIFORMITY.
// ==========================================================================

// --------------------------------------------------------------------------
// When — web comment write (foreign vs never-existed). Populates the slice-2
// refusal slots (`mwt2_first_refusal_*` for the foreign reach, `mwt2_last_*` for
// the never-existed comparator) so the REUSED slice-2 Then
// `the two web responses are refused identically` asserts them.
// --------------------------------------------------------------------------

async fn web_comment(
    world: &mut FoundryWorld,
    ws: &str,
    project: &str,
    number: i32,
) -> (StatusCode, String) {
    let (team_slug, project_slug) = project_route(world, ws, project);
    let email = web_email(world);
    let url = format!("/team/{team_slug}/project/{project_slug}/issues/{number}/comments");
    let outcome = signed_in_post(
        world.harness.as_ref().expect("harness"),
        world.http.as_ref().expect("http"),
        &email,
        WEB_PASSWORD,
        &url,
        &[("body", "leak attempt")],
    )
    .await;
    (outcome.status, outcome.body)
}

#[when(
    regex = r#"^the member comments on issue (\w+-\d+) in the "([^"]+)" project "([^"]+)" on the web$"#
)]
async fn web_comment_foreign(world: &mut FoundryWorld, key: String, ws: String, project: String) {
    ensure_harness(world).await;
    world.mwt4_foreign_comment_count_before = Some(comment_count_in_workspace(world, &ws).await);
    note_foreign_identifier(world, &key);
    note_foreign_identifier(world, &project);
    let (status, body) = web_comment(world, &ws, &project, issue_number(&key)).await;
    world.mwt2_first_refusal_status = Some(status);
    world.mwt2_first_refusal_body = Some(body.clone());
    world.mwt4_first_refusal_status = Some(status);
    world.mwt4_first_refusal_body = Some(body);
    note_refusal_status(world, world.mwt2_first_refusal_status);
}

#[when(regex = r#"^the member comments on an issue that never existed on the web$"#)]
async fn web_comment_missing(world: &mut FoundryWorld) {
    let ws = world.mwt2_acting_ws.clone().expect("acting workspace");
    let project = first_project_in(world, &ws);
    let (status, body) = web_comment(world, &ws, &project, 999_999).await;
    world.mwt2_last_status = Some(status);
    world.mwt2_last_body = Some(body);
}

// --------------------------------------------------------------------------
// When — web state-change write (foreign vs never-existed)
// --------------------------------------------------------------------------

async fn web_state_change(
    world: &mut FoundryWorld,
    ws: &str,
    project: &str,
    number: i32,
) -> (StatusCode, String) {
    let (team_slug, project_slug) = project_route(world, ws, project);
    let email = web_email(world);
    let url = format!("/team/{team_slug}/project/{project_slug}/issues/{number}/state");
    let outcome = signed_in_post(
        world.harness.as_ref().expect("harness"),
        world.http.as_ref().expect("http"),
        &email,
        WEB_PASSWORD,
        &url,
        &[("state", "in_progress")],
    )
    .await;
    (outcome.status, outcome.body)
}

#[when(
    regex = r#"^the member changes the state of issue (\w+-\d+) in the "([^"]+)" project "([^"]+)" on the web$"#
)]
async fn web_state_foreign(world: &mut FoundryWorld, key: String, ws: String, project: String) {
    ensure_harness(world).await;
    world.mwt4_foreign_issue_state_before = Some(issue_state(world, &ws, &key).await);
    note_foreign_identifier(world, &key);
    note_foreign_identifier(world, &project);
    let (status, body) = web_state_change(world, &ws, &project, issue_number(&key)).await;
    world.mwt2_first_refusal_status = Some(status);
    world.mwt2_first_refusal_body = Some(body.clone());
    world.mwt4_first_refusal_status = Some(status);
    world.mwt4_first_refusal_body = Some(body);
    note_refusal_status(world, world.mwt2_first_refusal_status);
}

#[when(regex = r#"^the member changes the state of an issue that never existed on the web$"#)]
async fn web_state_missing(world: &mut FoundryWorld) {
    let ws = world.mwt2_acting_ws.clone().expect("acting workspace");
    let project = first_project_in(world, &ws);
    let (status, body) = web_state_change(world, &ws, &project, 999_999).await;
    world.mwt2_last_status = Some(status);
    world.mwt2_last_body = Some(body);
}

// --------------------------------------------------------------------------
// When — web attachment upload write (foreign vs never-existed)
// --------------------------------------------------------------------------

async fn web_upload(
    world: &mut FoundryWorld,
    ws: &str,
    project: &str,
    number: i32,
) -> (StatusCode, String) {
    let (team_slug, project_slug) = project_route(world, ws, project);
    let email = web_email(world);
    let cookie = sign_in_cookie(world, &email).await;
    let base = base_url(world);
    let http = world.http.as_ref().expect("http").clone();
    let csrf = extract_csrf(
        &http
            .get(format!("{base}/sign-in"))
            .send()
            .await
            .expect("csrf get"),
    );
    let part = reqwest::multipart::Part::bytes(b"leak-bytes".to_vec())
        .file_name("probe.txt")
        .mime_str("text/plain")
        .expect("mime");
    let form = reqwest::multipart::Form::new()
        .text("_csrf", csrf.clone())
        .part("file", part);
    let url = format!("{base}/team/{team_slug}/project/{project_slug}/issues/{number}/attachments");
    let resp = http
        .post(&url)
        .header(
            reqwest::header::COOKIE,
            format!("{cookie}; foundry_csrf={csrf}"),
        )
        .multipart(form)
        .send()
        .await
        .expect("upload");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

#[when(
    regex = r#"^the member uploads a file to issue (\w+-\d+) in the "([^"]+)" project "([^"]+)" on the web$"#
)]
async fn web_upload_foreign(world: &mut FoundryWorld, key: String, ws: String, project: String) {
    ensure_harness(world).await;
    world.mwt4_foreign_attachment_count_before =
        Some(attachment_count_in_workspace(world, &ws).await);
    note_foreign_identifier(world, &key);
    note_foreign_identifier(world, &project);
    let (status, body) = web_upload(world, &ws, &project, issue_number(&key)).await;
    world.mwt2_first_refusal_status = Some(status);
    world.mwt2_first_refusal_body = Some(body.clone());
    world.mwt4_first_refusal_status = Some(status);
    world.mwt4_first_refusal_body = Some(body);
    note_refusal_status(world, world.mwt2_first_refusal_status);
}

#[when(regex = r#"^the member uploads a file to an issue that never existed on the web$"#)]
async fn web_upload_missing(world: &mut FoundryWorld) {
    let ws = world.mwt2_acting_ws.clone().expect("acting workspace");
    let project = first_project_in(world, &ws);
    let (status, body) = web_upload(world, &ws, &project, 999_999).await;
    world.mwt2_last_status = Some(status);
    world.mwt2_last_body = Some(body);
}

// --------------------------------------------------------------------------
// Given + When — web attachment download read (foreign vs never-existed)
// --------------------------------------------------------------------------

#[given(regex = r#"^the "([^"]+)" project "([^"]+)" issue (\w+-\d+) has an attachment$"#)]
async fn seed_attachment(world: &mut FoundryWorld, ws: String, project: String, key: String) {
    ensure_harness(world).await;
    let workspace_id = *world
        .mwt_workspace_ids
        .get(&ws)
        .unwrap_or_else(|| panic!("workspace {ws:?} must be seeded first"));
    let number = issue_number(&key);
    let (issue_id,): (uuid::Uuid,) = sqlx::query_as(
        "SELECT i.id FROM issues i
              JOIN projects p ON p.id = i.project_id
              WHERE i.workspace_id = $1 AND p.name = $2 AND i.number = $3",
    )
    .bind(workspace_id)
    .bind(&project)
    .bind(number)
    .fetch_one(&pool(world))
    .await
    .unwrap_or_else(|e| panic!("resolve issue {key:?} in {ws:?}/{project:?}: {e}"));
    let (uploader_id,): (uuid::Uuid,) =
        sqlx::query_as("SELECT user_id FROM workspace_memberships WHERE workspace_id = $1 LIMIT 1")
            .bind(workspace_id)
            .fetch_one(&pool(world))
            .await
            .unwrap_or_else(|e| panic!("resolve uploader in {ws:?}: {e}"));
    let id = uuid::Uuid::now_v7();
    world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .insert_attachment(
            id,
            issue_id,
            workspace_id,
            uploader_id,
            "secret.txt",
            "text/plain",
            "0000000000000000000000000000000000000000000000000000000000000000",
            b"globex-secret",
        )
        .await
        .expect("seed foreign attachment");
    world
        .mwt4_attachment_id_by_label
        .insert(format!("{ws}:{key}"), id);
    note_foreign_identifier(world, &id.to_string());
    note_foreign_identifier(world, "secret.txt");
}

async fn web_download(
    world: &mut FoundryWorld,
    ws: &str,
    project: &str,
    number: i32,
    id: uuid::Uuid,
) -> (StatusCode, String) {
    let (team_slug, project_slug) = project_route(world, ws, project);
    web_get(
        world,
        &format!("/team/{team_slug}/project/{project_slug}/issues/{number}/attachments/{id}"),
    )
    .await
}

#[when(regex = r#"^the member downloads the "([^"]+)" attachment on the web$"#)]
async fn web_download_foreign(world: &mut FoundryWorld, ws: String) {
    ensure_harness(world).await;
    let label = format!("{ws}:GLOBEX-1");
    let id = *world
        .mwt4_attachment_id_by_label
        .get(&label)
        .unwrap_or_else(|| panic!("no seeded attachment for {label:?}"));
    let (status, body) = web_download(world, &ws, "Core", 1, id).await;
    world.mwt2_first_refusal_status = Some(status);
    world.mwt2_first_refusal_body = Some(body.clone());
    world.mwt4_first_refusal_status = Some(status);
    world.mwt4_first_refusal_body = Some(body);
    note_refusal_status(world, world.mwt2_first_refusal_status);
}

#[when(regex = r#"^the member downloads an attachment that never existed on the web$"#)]
async fn web_download_missing(world: &mut FoundryWorld) {
    let ws = world.mwt2_acting_ws.clone().expect("acting workspace");
    let project = first_project_in(world, &ws);
    let (status, body) = web_download(world, &ws, &project, 1, uuid::Uuid::now_v7()).await;
    world.mwt2_last_status = Some(status);
    world.mwt2_last_body = Some(body);
}

// --------------------------------------------------------------------------
// When — web admin revoke of a never-existed credential (the missing-id
// comparator for the REUSED slice-2 admin-revoke-foreign When, which captured
// the foreign refusal into `mwt2_first_refusal_*`).
// --------------------------------------------------------------------------

#[when(
    regex = r#"^the "([^"]+)" admin tries to revoke a credential that never existed on the web$"#
)]
async fn web_admin_revoke_missing(world: &mut FoundryWorld, _admin_ws: String) {
    ensure_harness(world).await;
    // The slice-2 admin-revoke-foreign When already populated
    // `mwt2_first_refusal_*` with the foreign-credential refusal. Mirror it into
    // the oracle-hunt accumulator, then attempt a never-existed jti so the REUSED
    // `the two web responses are refused identically` compares the two.
    note_refusal_status(world, world.mwt2_first_refusal_status);
    let email = world.mwt2_web_email.clone().expect("web admin persona");
    let jti = uuid::Uuid::now_v7();
    let outcome = signed_in_post(
        world.harness.as_ref().expect("harness"),
        world.http.as_ref().expect("http"),
        &email,
        WEB_PASSWORD,
        &format!("/admin/tokens/{jti}/revoke"),
        &[],
    )
    .await;
    world.mwt2_last_status = Some(outcome.status);
    world.mwt2_last_body = Some(outcome.body);
}

// --------------------------------------------------------------------------
// When — API state-change PATCH (foreign vs never-existed). Populates the
// slice-3 refusal slots (`mwt3_first_refusal_*` for foreign, `mwt_last_*` for
// the comparator) so the REUSED slice-3 Then
// `the two API responses are refused identically` asserts them.
// --------------------------------------------------------------------------

async fn api_patch_state(
    world: &mut FoundryWorld,
    team_slug: &str,
    project_slug: &str,
    number: i32,
) {
    let bearer = acme_bearer(world);
    let url = format!(
        "{}/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}",
        base_url(world)
    );
    let http = world.http.as_ref().expect("http");
    let mut req = http
        .patch(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::ACCEPT, "application/json")
        .body(serde_json::json!({ "state": "in_progress" }).to_string());
    if let Some(b) = bearer {
        req = req.header(reqwest::header::AUTHORIZATION, format!("Bearer {b}"));
    }
    let resp = req.send().await.expect("send state PATCH");
    world.mwt_last_status = Some(resp.status());
    world.mwt_last_body = Some(resp.text().await.unwrap_or_default());
}

#[when(
    regex = r#"^the Acme-bound credential changes the state of issue (\w+-\d+) in the "([^"]+)" project over the API by its real address$"#
)]
async fn api_state_foreign(world: &mut FoundryWorld, key: String, project: String) {
    ensure_harness(world).await;
    let ws = "Globex";
    world.mwt4_foreign_issue_state_before = Some(issue_state(world, ws, &key).await);
    note_foreign_identifier(world, &key);
    let (team_slug, project_slug) = project_route(world, ws, &project);
    api_patch_state(world, &team_slug, &project_slug, issue_number(&key)).await;
    world.mwt3_first_refusal_status = world.mwt_last_status;
    world.mwt3_first_refusal_body = world.mwt_last_body.clone();
    world.mwt4_first_refusal_status = world.mwt_last_status;
    world.mwt4_first_refusal_body = world.mwt_last_body.clone();
    note_refusal_status(world, world.mwt_last_status);
}

#[when(
    regex = r#"^the Acme-bound credential changes the state of an issue that never existed over the API$"#
)]
async fn api_state_missing(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    api_patch_state(world, "no-such-team", "no-such-project", 999_999).await;
}

// --------------------------------------------------------------------------
// When — API comment POST (foreign vs never-existed)
// --------------------------------------------------------------------------

async fn api_comment(world: &mut FoundryWorld, team_slug: &str, project_slug: &str, number: i32) {
    let bearer = acme_bearer(world);
    let url = format!(
        "{}/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}/comments",
        base_url(world)
    );
    let http = world.http.as_ref().expect("http");
    let mut req = http
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::ACCEPT, "application/json")
        .body(serde_json::json!({ "body": "leak attempt" }).to_string());
    if let Some(b) = bearer {
        req = req.header(reqwest::header::AUTHORIZATION, format!("Bearer {b}"));
    }
    let resp = req.send().await.expect("send comment POST");
    world.mwt_last_status = Some(resp.status());
    world.mwt_last_body = Some(resp.text().await.unwrap_or_default());
}

#[when(
    regex = r#"^the Acme-bound credential comments on issue (\w+-\d+) in the "([^"]+)" project over the API by its real address$"#
)]
async fn api_comment_foreign(world: &mut FoundryWorld, key: String, project: String) {
    ensure_harness(world).await;
    let ws = "Globex";
    world.mwt4_foreign_comment_count_before = Some(comment_count_in_workspace(world, ws).await);
    note_foreign_identifier(world, &key);
    let (team_slug, project_slug) = project_route(world, ws, &project);
    api_comment(world, &team_slug, &project_slug, issue_number(&key)).await;
    world.mwt3_first_refusal_status = world.mwt_last_status;
    world.mwt3_first_refusal_body = world.mwt_last_body.clone();
    world.mwt4_first_refusal_status = world.mwt_last_status;
    world.mwt4_first_refusal_body = world.mwt_last_body.clone();
    note_refusal_status(world, world.mwt_last_status);
}

#[when(
    regex = r#"^the Acme-bound credential comments on an issue that never existed over the API$"#
)]
async fn api_comment_missing(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    api_comment(world, "no-such-team", "no-such-project", 999_999).await;
}

// --------------------------------------------------------------------------
// When — oracle-hunt cross-surface probe (web read + API read in ONE step), so
// BOTH cross-tenant refusal statuses land in `mwt4_refusal_statuses` for the
// no-403 / all-404 assertions. Self-contained (not a reused slice-2/3 When) so
// the accumulator is populated for the oracle-hunt scenario.
// --------------------------------------------------------------------------

#[when(
    regex = r#"^the member probes the "([^"]+)" issue (\w+-\d+) in project "([^"]+)" across the web and the API$"#
)]
async fn oracle_hunt_probe(world: &mut FoundryWorld, ws: String, key: String, project: String) {
    ensure_harness(world).await;
    let (team_slug, project_slug) = project_route(world, &ws, &project);
    let number = issue_number(&key);

    // Web leg: open the foreign issue detail by its REAL address.
    let (web_status, _web_body) = web_get(
        world,
        &format!("/team/{team_slug}/project/{project_slug}/issues/{number}"),
    )
    .await;
    note_refusal_status(world, Some(web_status));

    // API leg: list the foreign project's issues with the Acme-bound token.
    let bearer = acme_bearer(world);
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
    let resp = req.send().await.expect("send oracle-hunt API read");
    note_refusal_status(world, Some(resp.status()));
}

// --------------------------------------------------------------------------
// Then — no foreign-identifier echo (oracle-hunt no-echo). Reads the slice-4
// first-refusal body slot (set by whichever foreign When ran).
// --------------------------------------------------------------------------

#[then(regex = r#"^the web refusal reveals no foreign identifier$"#)]
async fn web_no_foreign_identifier(world: &mut FoundryWorld) {
    assert_no_foreign_echo(world, "web");
}

#[then(regex = r#"^the API refusal reveals no foreign identifier$"#)]
async fn api_no_foreign_identifier(world: &mut FoundryWorld) {
    assert_no_foreign_echo(world, "API");
}

fn assert_no_foreign_echo(world: &FoundryWorld, surface: &str) {
    // Prefer the slice-4 first-refusal slot (set by this module's foreign Whens);
    // fall back to the slice-2 foreign-refusal slot when the reused slice-2 web
    // read When (scenarios 1-2) populated it instead.
    let body = world
        .mwt4_first_refusal_body
        .clone()
        .or_else(|| world.mwt2_first_refusal_body.clone())
        .unwrap_or_default();
    // The known Globex foreign identifiers must NEVER appear in a refusal body —
    // checked unconditionally so the assertion is never vacuous, regardless of
    // which (reused or new) When produced the refusal. The issue-key prefix, the
    // foreign project + team slugs, and the foreign workspace name are all
    // existence-revealing identifiers.
    let always_forbidden = ["GLOBEX-", "core", "platform", "Globex"];
    let dynamic: Vec<&str> = world
        .mwt4_foreign_identifiers
        .iter()
        .map(String::as_str)
        .collect();
    for id in always_forbidden.iter().copied().chain(dynamic) {
        assert!(
            !body.contains(id),
            "the {surface} refusal body echoed a foreign identifier {id:?} (enumeration oracle): {body:?}"
        );
    }
}

// --------------------------------------------------------------------------
// Then — no foreign mutation occurred (write surfaces)
// --------------------------------------------------------------------------

#[then(regex = r#"^no comment was created in "([^"]+)"$"#)]
async fn no_comment_created(world: &mut FoundryWorld, ws: String) {
    let before = world
        .mwt4_foreign_comment_count_before
        .expect("comment count snapshotted before the cross-tenant write");
    let after = comment_count_in_workspace(world, &ws).await;
    assert_eq!(
        before, after,
        "a cross-tenant comment write leaked into {ws} (before={before}, after={after})"
    );
}

#[then(regex = r#"^no attachment was created in "([^"]+)"$"#)]
async fn no_attachment_created(world: &mut FoundryWorld, ws: String) {
    let before = world
        .mwt4_foreign_attachment_count_before
        .expect("attachment count snapshotted before the cross-tenant upload");
    let after = attachment_count_in_workspace(world, &ws).await;
    assert_eq!(
        before, after,
        "a cross-tenant upload leaked into {ws} (before={before}, after={after})"
    );
}

#[then(regex = r#"^no "([^"]+)" issue changed state$"#)]
async fn no_issue_changed_state(world: &mut FoundryWorld, _ws: String) {
    let before = world
        .mwt4_foreign_issue_state_before
        .clone()
        .expect("foreign issue state snapshotted before the cross-tenant change");
    let after = issue_state(world, "Globex", "GLOBEX-1").await;
    assert_eq!(
        before, after,
        "a cross-tenant state change mutated the foreign issue (before={before:?}, after={after:?})"
    );
}

// --------------------------------------------------------------------------
// Then — oracle hunt (no 403 anywhere; all 404)
// --------------------------------------------------------------------------

#[then(regex = r#"^no cross-tenant refusal in this scenario is a 403$"#)]
async fn no_refusal_is_403(world: &mut FoundryWorld) {
    assert!(
        !world.mwt4_refusal_statuses.is_empty(),
        "the oracle-hunt scenario captured no cross-tenant refusal to assert on"
    );
    for s in &world.mwt4_refusal_statuses {
        assert_ne!(
            *s,
            StatusCode::FORBIDDEN,
            "a cross-tenant refusal was 403 — a 403-vs-404 difference is an existence oracle"
        );
    }
}

#[then(regex = r#"^every cross-tenant refusal in this scenario is a non-enumerable 404$"#)]
async fn every_refusal_is_404(world: &mut FoundryWorld) {
    assert!(
        !world.mwt4_refusal_statuses.is_empty(),
        "the oracle-hunt scenario captured no cross-tenant refusal to assert on"
    );
    for s in &world.mwt4_refusal_statuses {
        assert_eq!(
            *s,
            StatusCode::NOT_FOUND,
            "every cross-tenant refusal must be a uniform non-enumerable 404, got {s}"
        );
    }
}

// --------------------------------------------------------------------------
// Local helpers — workspace-scoped counts / state snapshots
// --------------------------------------------------------------------------

async fn comment_count_in_workspace(world: &FoundryWorld, ws: &str) -> i64 {
    let workspace_id = *world
        .mwt_workspace_ids
        .get(ws)
        .unwrap_or_else(|| panic!("workspace {ws:?} not seeded"));
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM comments c
              JOIN issues i ON i.id = c.issue_id
              WHERE i.workspace_id = $1",
    )
    .bind(workspace_id)
    .fetch_one(&pool(world))
    .await
    .expect("count comments in workspace");
    count
}

async fn attachment_count_in_workspace(world: &FoundryWorld, ws: &str) -> i64 {
    let workspace_id = *world
        .mwt_workspace_ids
        .get(ws)
        .unwrap_or_else(|| panic!("workspace {ws:?} not seeded"));
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM issue_attachments WHERE workspace_id = $1")
            .bind(workspace_id)
            .fetch_one(&pool(world))
            .await
            .expect("count attachments in workspace");
    count
}

async fn issue_state(world: &FoundryWorld, ws: &str, key: &str) -> String {
    let workspace_id = *world
        .mwt_workspace_ids
        .get(ws)
        .unwrap_or_else(|| panic!("workspace {ws:?} not seeded"));
    let number = issue_number(key);
    let (state,): (String,) =
        sqlx::query_as("SELECT state FROM issues WHERE workspace_id = $1 AND number = $2")
            .bind(workspace_id)
            .bind(number)
            .fetch_one(&pool(world))
            .await
            .unwrap_or_else(|e| panic!("read state of {key:?} in {ws:?}: {e}"));
    state
}

fn first_project_in(world: &FoundryWorld, ws: &str) -> String {
    world
        .mwt_project_route
        .keys()
        .find(|(w, _)| w == ws)
        .map(|(_, p)| p.clone())
        .unwrap_or_else(|| panic!("no project recorded for {ws:?}"))
}
