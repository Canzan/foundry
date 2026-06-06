//! "Remaining-Surfaces Templating" step definitions — the move-only follow-up
//! to Feature B that finishes templating the last inline `format!()` render
//! sites (US-R01..US-R06 + a north-star completion guard).
//!
//! RED-state contract (DISTILL, ADR-025 / Mandate 7):
//! These steps drive the SAME in-process axum harness (`InProcHarness` →
//! `build_router`) the rest of the browser suite uses, over real HTTP through
//! `reqwest`. The render contract is selector-and-substring-identical
//! (inherited from Feature B ADR-B02), so structural assertions go through
//! `scraper` (support::html_assertions) and copy assertions through
//! `body.contains`.
//!
//! This feature is a MOVE-ONLY refactor: the EXISTING project-create
//! (us_07), keyboard-modal fragment (us_12), attachment-listing (us_11),
//! claim/invite (us_05) scenarios already pass for the current `format!`
//! output and are the regression net (NFR-WEBB-COMPAT-01) — they are NOT
//! re-asserted here. These steps assert ONLY the genuine user-visible DELTAS
//! the render-contract flagged as GAP/PARTIAL, each of which fails RED for
//! MISSING_FUNCTIONALITY today:
//!   - US-R01/R02/R04/R05/R06 full-page surfaces emit a bare `<!doctype><head>`
//!     today with NO `<link>` stylesheet → the "links the vendored stylesheet
//!     via the base layout" assertions fail until DELIVER moves each into a
//!     template extending `base.html`.
//!   - US-R01/R03/R05 fragment-marker asserts pin the byte-stable
//!     `data-hx-fragment` / `data-state` markers the move must preserve.
//!   - US-R07 source-tree guard fails while ANY bare-`<head>` inline full page
//!     remains in the handler sources.
//!
//! Reused Givens (cucumber-rs requires globally-unique step text):
//!   - `a workspace "..." exists with admin "..."`               (us_06_signin)
//!   - `a member "..." belongs to the team "..."`                (us_07_project_create)
//!   - `a project "..." with key prefix "..." exists in the "..." team` (us_08_file_issue)
//!   - `the "..." project has issue ... titled "..." (in progress|in the backlog)` (feature_a)
//!   - `(\w+) is signed in as a Backend member`                  (feature_b_web_tier)
//!   - `(\w+) has no current browser session`                    (feature_b_web_tier)
//!     The two Feature-B signed-in/out Givens set `world.b_signed_in_email`;
//!     this module's When steps read that field (mirrored into `r_*` slots).
//!
//! What DELIVER must wire to flip these GREEN is enumerated in
//! `docs/feature/remaining-surfaces-templating/distill/step-skeletons.md`.

use crate::support::harness::InProcHarness;
use crate::support::html_assertions;
use crate::world::FoundryWorld;
use cucumber::{then, when};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use std::collections::HashMap;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
const MEMBER_PASSWORD: &str = "mei-correct-horse-battery-staple";
const ADMIN_PASSWORD: &str = "admin-password-from-bootstrap";

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

fn password_for(email: &str) -> &'static str {
    if email == "devansh@acme.com" {
        ADMIN_PASSWORD
    } else {
        MEMBER_PASSWORD
    }
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

/// The persona the reused Feature-B `is signed in as a Backend member` Given
/// recorded for this scenario, mirrored into the `r_*` slot.
fn signed_in_email(world: &FoundryWorld) -> Option<String> {
    world
        .r_signed_in_email
        .clone()
        .or_else(|| world.b_signed_in_email.clone())
}

/// Sign in as `email`/`password` over the real cookie path; return the
/// `foundry_session=...` cookie pair. (A small per-module helper — the
/// step modules link independently, so it is duplicated rather than
/// re-exported, matching the feature_b / us_10 idiom.)
async fn sign_in_and_capture_cookie(world: &FoundryWorld, email: &str, password: &str) -> String {
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
    form.insert("password", password.to_string());
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

/// Mint a CSRF token bound to the given session cookie; return
/// `(token, "session; foundry_csrf=token")`.
async fn csrf_for_session(world: &FoundryWorld, session_cookie: &str) -> (String, String) {
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let base = harness.base_url();
    let csrf_get = http
        .get(format!("{base}/sign-in"))
        .header(reqwest::header::COOKIE, session_cookie.to_string())
        .send()
        .await
        .expect("get csrf for session");
    let token = csrf_get
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .and_then(|s| s.strip_prefix("foundry_csrf="))
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();
    let combined = format!("{session_cookie}; foundry_csrf={token}");
    (token, combined)
}

/// Authenticated GET of `path`, returning `(status, body)`. Signs the recorded
/// persona in fresh (no cookie jar) and presents the session cookie.
async fn signed_in_get(world: &FoundryWorld, path: &str) -> (StatusCode, String) {
    let email = signed_in_email(world).expect("a persona is signed in");
    let password = password_for(&email);
    let cookie = sign_in_and_capture_cookie(world, &email, password).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let resp = http
        .get(format!("{base}{path}", base = harness.base_url()))
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("authenticated GET");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

/// Anonymous GET of `path` returning `(status, headers, body)` with redirects
/// disabled so the signed-out 303 Location is observable.
async fn anonymous_get(
    world: &FoundryWorld,
    path: &str,
) -> (StatusCode, reqwest::header::HeaderMap, String) {
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let resp = http
        .get(format!("{base}{path}", base = harness.base_url()))
        .send()
        .await
        .expect("anonymous GET");
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp.text().await.unwrap_or_default();
    (status, headers, body)
}

fn capture(world: &mut FoundryWorld, status: StatusCode, body: String) {
    world.r_last_status = Some(status);
    world.r_last_body = Some(body);
}

fn last_body(world: &FoundryWorld) -> String {
    world
        .r_last_body
        .clone()
        .expect("a surface GET/POST was captured by the When step")
}

// ==========================================================================
// When — US-R01 project-create
// ==========================================================================

#[when(regex = r#"^(\w+) opens the project-create form for the "([^"]+)" team$"#)]
async fn open_project_create_form(world: &mut FoundryWorld, _persona: String, team_name: String) {
    ensure_harness(world).await;
    let path = format!("/team/{slug}/projects/new", slug = slugify(&team_name));
    let (status, body) = signed_in_get(world, &path).await;
    capture(world, status, body);
}

#[when(
    regex = r#"^(\w+) submits the project-create form for "([^"]+)" with name "([^"]+)" and an empty key prefix$"#
)]
async fn submit_project_create_empty_key(
    world: &mut FoundryWorld,
    _persona: String,
    team_name: String,
    name: String,
) {
    ensure_harness(world).await;
    let email = signed_in_email(world).expect("signed in");
    let password = password_for(&email);
    let cookie = sign_in_and_capture_cookie(world, &email, password).await;
    let (csrf_token, combined) = csrf_for_session(world, &cookie).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/team/{slug}/projects",
        base = harness.base_url(),
        slug = slugify(&team_name)
    );
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("name", name);
    form.insert("key_prefix", String::new());
    form.insert("_csrf", csrf_token);
    let resp = http
        .post(&url)
        .header(reqwest::header::COOKIE, combined)
        .header("hx-request", "true")
        .form(&form)
        .send()
        .await
        .expect("post create project");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    capture(world, status, body);
}

// ==========================================================================
// When — US-R02 new-issue modal full-page fallback (no scripting → no hx-request)
// ==========================================================================

#[when(regex = r#"^(\w+) opens the new-issue page for "([^"]+)" without scripting$"#)]
async fn open_new_issue_full_page(
    world: &mut FoundryWorld,
    _persona: String,
    project_name: String,
) {
    ensure_harness(world).await;
    // No `hx-request` header ⇒ the handler returns the FULL-PAGE fallback.
    let path = format!(
        "/team/backend/project/{slug}/issues/new",
        slug = slugify(&project_name)
    );
    let (status, body) = signed_in_get(world, &path).await;
    capture(world, status, body);
}

// ==========================================================================
// When — US-R03 issue-create error + state chip
// ==========================================================================

#[when(regex = r#"^(\w+) files an issue on "([^"]+)" with an empty title$"#)]
async fn file_issue_empty_title(world: &mut FoundryWorld, _persona: String, project_name: String) {
    ensure_harness(world).await;
    let email = signed_in_email(world).expect("signed in");
    let password = password_for(&email);
    let cookie = sign_in_and_capture_cookie(world, &email, password).await;
    let (csrf_token, combined) = csrf_for_session(world, &cookie).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/team/backend/project/{slug}/issues",
        base = harness.base_url(),
        slug = slugify(&project_name)
    );
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("title", String::new());
    form.insert("_csrf", csrf_token);
    let resp = http
        .post(&url)
        .header(reqwest::header::COOKIE, combined)
        .header("hx-request", "true")
        .form(&form)
        .send()
        .await
        .expect("post create issue");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    capture(world, status, body);
}

#[when(regex = r#"^(\w+) moves "(\w+)-(\d+)" to the "([^"]+)" state from the board$"#)]
async fn change_issue_state(
    world: &mut FoundryWorld,
    _persona: String,
    _prefix: String,
    number: i32,
    new_state: String,
) {
    ensure_harness(world).await;
    let email = signed_in_email(world).expect("signed in");
    let password = password_for(&email);
    let cookie = sign_in_and_capture_cookie(world, &email, password).await;
    let (csrf_token, combined) = csrf_for_session(world, &cookie).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/team/backend/project/auth-v2/issues/{number}/state",
        base = harness.base_url()
    );
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("state", new_state);
    form.insert("_csrf", csrf_token);
    let resp = http
        .post(&url)
        .header(reqwest::header::COOKIE, combined)
        .header("hx-request", "true")
        .form(&form)
        .send()
        .await
        .expect("post state change");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    capture(world, status, body);
}

// ==========================================================================
// When — US-R04 dashboard landing + events sign-in page
// ==========================================================================

#[when(regex = r#"^(\w+) opens the dashboard landing$"#)]
async fn open_dashboard_landing(world: &mut FoundryWorld, _persona: String) {
    ensure_harness(world).await;
    match signed_in_email(world) {
        Some(_) => {
            let (status, body) = signed_in_get(world, "/").await;
            capture(world, status, body);
        }
        None => {
            let (status, headers, body) = anonymous_get(world, "/").await;
            world.r_last_headers = Some(headers);
            capture(world, status, body);
        }
    }
}

#[when(regex = r#"^(\w+) requests the events stream for "([^"]+)" without a session$"#)]
async fn request_events_anon(world: &mut FoundryWorld, _persona: String, project_name: String) {
    ensure_harness(world).await;
    let path = format!(
        "/team/backend/project/{slug}/events",
        slug = slugify(&project_name)
    );
    let (status, headers, body) = anonymous_get(world, &path).await;
    world.r_last_headers = Some(headers);
    capture(world, status, body);
}

// ==========================================================================
// When — US-R05 attachment surfaces
// ==========================================================================

#[when(regex = r#"^(\w+) submits an upload to "(\w+)-(\d+)" with no file attached$"#)]
async fn upload_no_file_part(
    world: &mut FoundryWorld,
    _persona: String,
    _prefix: String,
    number: i32,
) {
    // A multipart body with ONLY the `_csrf` text field — no part carries a
    // filename, so `extract_file_part` returns `UploadError::Missing`, which the
    // handler maps to the `attachment-upload-error` bad-request fragment.
    upload_attachment(world, number, None, Vec::new()).await;
}

#[when(regex = r#"^(\w+) uploads a file over the configured limit to "(\w+)-(\d+)"$"#)]
async fn upload_oversize_file(
    world: &mut FoundryWorld,
    _persona: String,
    _prefix: String,
    number: i32,
) {
    // 30 MB exceeds the configured 10 MB default cap (DEFAULT_FILE_UPLOAD_MAX_MB);
    // `extract_file_part` surfaces axum's PayloadTooLarge as the styled 413 page.
    let bytes = vec![0u8; 30 * 1024 * 1024];
    upload_attachment(world, number, Some("too-large.bin"), bytes).await;
}

/// POST a multipart upload. `filename = Some(name)` attaches a real file part;
/// `filename = None` sends ONLY the `_csrf` text field (no file part) so the
/// handler's missing-file path fires.
async fn upload_attachment(
    world: &mut FoundryWorld,
    issue_number: i32,
    filename: Option<&str>,
    bytes: Vec<u8>,
) {
    ensure_harness(world).await;
    let email = signed_in_email(world).expect("signed in");
    let password = password_for(&email);
    let cookie = sign_in_and_capture_cookie(world, &email, password).await;
    let (csrf_token, combined) = csrf_for_session(world, &cookie).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/team/backend/project/auth-v2/issues/{issue_number}/attachments",
        base = harness.base_url()
    );
    // The CSRF middleware reads the token from the `x-csrf-token` HEADER for
    // multipart bodies (it cannot parse a `_csrf` field out of multipart) — see
    // us_11_attachments::perform_upload. We mirror that idiom exactly.
    let mut form = reqwest::multipart::Form::new();
    if let Some(name) = filename {
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(name.to_string())
            .mime_str("application/octet-stream")
            .expect("mime");
        form = form.part("file", part);
    } else {
        // No file part: send a benign non-file text field so the multipart body
        // is well-formed but carries no part with a filename.
        form = form.text("note", "no file");
    }
    let resp = http
        .post(&url)
        .header(reqwest::header::COOKIE, combined)
        .header("x-csrf-token", csrf_token)
        .header("hx-request", "true")
        .multipart(form)
        .send()
        .await
        .expect("post attachment");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    capture(world, status, body);
}

// ==========================================================================
// When — US-R06 bootstrap dashboard + shared invalid_page
// ==========================================================================

#[when(regex = r#"^(\w+) opens the bootstrap dashboard$"#)]
async fn open_bootstrap_dashboard(world: &mut FoundryWorld, _persona: String) {
    ensure_harness(world).await;
    let (status, body) = signed_in_get(world, "/dashboard").await;
    capture(world, status, body);
}

#[when(regex = r#"^(\w+) opens the board for a team slug that does not exist$"#)]
async fn open_nonexistent_team(world: &mut FoundryWorld, _persona: String) {
    ensure_harness(world).await;
    let (status, body) = signed_in_get(world, "/team/no-such-team/project/whatever").await;
    capture(world, status, body);
}

// ==========================================================================
// When — US-R07 completion-check (source-tree scan)
// ==========================================================================

#[when(
    regex = r#"^the foundry-app handler sources are scanned for inline full-page HTML documents$"#
)]
async fn scan_handler_sources(world: &mut FoundryWorld) {
    // No HTTP — this is a source-tree contract (mirrors feature_b's on-disk
    // `vendored_htmx_files` count). The result is stashed as a pseudo-body so
    // the Then step can read it through the same world slot.
    let sites = inline_full_page_sites();
    world.r_last_body = Some(sites.join("\n"));
}

// ==========================================================================
// Then — full-page styling (links the vendored stylesheet via base layout)
// ==========================================================================

#[then(
    regex = r#"^the (project-create form|new-issue page|dashboard landing|events page|too-large page|bootstrap dashboard|not-found page) links the vendored stylesheet from the application's own static path$"#
)]
async fn surface_links_stylesheet(world: &mut FoundryWorld, surface: String) {
    let body = last_body(world);
    assert_links_local_stylesheet(&body, &surface);
}

#[then(
    regex = r#"^the (project-create form|new-issue page|dashboard landing) references no external origin$"#
)]
async fn surface_no_external_origin(world: &mut FoundryWorld, surface: String) {
    let body = last_body(world);
    assert_no_external_origin(&body, &surface);
}

// ==========================================================================
// Then — US-R01 project-create form fields + error fragment
// ==========================================================================

#[then(
    regex = r#"^the project-create form shows the project-name and key-prefix inputs and the hidden anti-forgery field$"#
)]
async fn project_create_fields(world: &mut FoundryWorld) {
    let body = last_body(world);
    html_assertions::assert_has(&body, r#"input[name="name"]"#);
    html_assertions::assert_has(&body, r#"input[name="key_prefix"]"#);
    html_assertions::assert_has(&body, r#"input[type="hidden"][name="_csrf"]"#);
}

#[then(regex = r#"^the project-create error fragment carries the marker "([^"]+)"$"#)]
async fn project_create_error_marker(world: &mut FoundryWorld, marker: String) {
    let body = last_body(world);
    html_assertions::assert_has(&body, &format!(r#"[data-hx-fragment="{marker}"]"#));
}

// ==========================================================================
// Then — US-R02 modal full-page dialog
// ==========================================================================

#[then(
    regex = r#"^the new-issue page carries the new-issue dialog with the autofocused title input and the hidden anti-forgery field$"#
)]
async fn new_issue_dialog(world: &mut FoundryWorld) {
    let body = last_body(world);
    html_assertions::assert_has(&body, r#"[data-modal="new-issue"]"#);
    html_assertions::assert_has(&body, r#"[role="dialog"]"#);
    html_assertions::assert_has(&body, r#"input[name="title"][autofocus]"#);
    html_assertions::assert_has(&body, r#"input[type="hidden"][name="_csrf"]"#);
}

// ==========================================================================
// Then — US-R03 issue-create error + state chip
// ==========================================================================

#[then(regex = r#"^the issue-create error fragment carries the marker "([^"]+)"$"#)]
async fn issue_create_error_marker(world: &mut FoundryWorld, marker: String) {
    let body = last_body(world);
    html_assertions::assert_has(&body, &format!(r#"[data-hx-fragment="{marker}"]"#));
}

#[then(regex = r#"^the issue-create error fragment shows the literal copy "([^"]+)"$"#)]
async fn issue_create_error_copy(world: &mut FoundryWorld, copy: String) {
    let body = last_body(world);
    assert!(
        body.contains(&copy),
        "issue-create error fragment is missing the literal copy {copy:?}; body:\n{body}"
    );
}

#[then(regex = r#"^the state chip carries the data-state value "([^"]+)"$"#)]
async fn state_chip_value(world: &mut FoundryWorld, value: String) {
    let body = last_body(world);
    html_assertions::assert_has(&body, &format!(r#"span.state[data-state="{value}"]"#));
}

// ==========================================================================
// Then — bare-fragment guard (fragments MUST NOT extend base.html)
// ==========================================================================

#[then(
    regex = r#"^the (project-create error fragment|issue-create error fragment|state chip|attachment upload-error fragment) is a bare fragment that is not wrapped in the base layout$"#
)]
async fn fragment_is_bare(world: &mut FoundryWorld, _which: String) {
    let body = last_body(world);
    // A bare fragment is htmx-swapped into an existing DOM — it must NOT be a
    // full document (no <html>/<head>/<!doctype>), or the swap double-wraps and
    // the page breaks (NFR-WEBB-COMPAT-02 fragment-vs-full-page rule).
    let lower = body.to_ascii_lowercase();
    assert!(
        !lower.contains("<html") && !lower.contains("<head") && !lower.contains("<!doctype"),
        "expected a BARE fragment (no <html>/<head>/<!doctype>) but the response is a full \
         document — extending base.html on a fragment double-wraps the htmx swap; body:\n{body}"
    );
}

// ==========================================================================
// Then — US-R04 signed-out landing redirect + events status/link
// ==========================================================================

#[then(regex = r#"^the dashboard landing redirects to the sign-in page with no body change$"#)]
async fn landing_redirects(world: &mut FoundryWorld) {
    let status = world.r_last_status.expect("status captured");
    assert_eq!(
        status.as_u16(),
        303,
        "signed-out landing must 303 SEE_OTHER, got {status}"
    );
    let headers = world.r_last_headers.as_ref().expect("headers captured");
    let location = headers
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        location, "/sign-in",
        "expected redirect to /sign-in, got {location:?}"
    );
    let body = world.r_last_body.clone().unwrap_or_default();
    assert!(
        body.trim().is_empty(),
        "signed-out 303 must have an empty body (handler control flow unchanged); body:\n{body}"
    );
}

#[then(regex = r#"^the events page is refused with a sign-in-required status$"#)]
async fn events_unauthorized(world: &mut FoundryWorld) {
    let status = world.r_last_status.expect("status captured");
    assert_eq!(
        status.as_u16(),
        401,
        "the events sign-in-required page must keep its 401 status, got {status}"
    );
}

#[then(regex = r#"^the events page offers a sign-in link$"#)]
async fn events_signin_link(world: &mut FoundryWorld) {
    let body = last_body(world);
    html_assertions::assert_has(&body, r#"a[href="/sign-in"]"#);
}

// ==========================================================================
// Then — US-R05 attachment markers + 413 page
// ==========================================================================

#[then(regex = r#"^the attachment upload-error fragment carries the marker "([^"]+)"$"#)]
async fn upload_error_marker(world: &mut FoundryWorld, marker: String) {
    let body = last_body(world);
    html_assertions::assert_has(&body, &format!(r#"[data-hx-fragment="{marker}"]"#));
}

#[then(regex = r#"^the upload is refused with an over-limit status$"#)]
async fn upload_over_limit(world: &mut FoundryWorld) {
    let status = world.r_last_status.expect("status captured");
    assert_eq!(
        status.as_u16(),
        413,
        "an over-limit upload must keep its 413 status, got {status}"
    );
}

#[then(regex = r#"^the (too-large page|bootstrap dashboard) shows the literal copy "([^"]+)"$"#)]
async fn full_page_copy(world: &mut FoundryWorld, _surface: String, copy: String) {
    let body = last_body(world);
    assert!(
        body.contains(&copy),
        "page is missing the literal copy {copy:?}; body:\n{body}"
    );
}

// ==========================================================================
// Then — US-R06 shared invalid_page shape
// ==========================================================================

#[then(
    regex = r#"^the not-found page shows a heading and a message in the shared error-page shape$"#
)]
async fn invalid_page_shape(world: &mut FoundryWorld) {
    let body = last_body(world);
    // The shared invalid_page is `<h1>{heading}</h1><p>{message}</p>`
    // (render-contract.md §US-R06). One structural assertion on this shape
    // covers every caller (~17 call sites) at once.
    html_assertions::assert_has(&body, "h1");
    html_assertions::assert_has(&body, "p");
}

// ==========================================================================
// Then — US-R07 completion-check
// ==========================================================================

#[then(regex = r#"^no handler emits a bare-head inline HTML document$"#)]
async fn no_inline_full_page(world: &mut FoundryWorld) {
    let captured = world.r_last_body.clone().unwrap_or_default();
    let sites: Vec<&str> = captured.lines().filter(|l| !l.is_empty()).collect();
    assert!(
        sites.is_empty(),
        "{n} foundry-app handler site(s) still emit a bare-<head> inline full-page HTML \
         document — the move is not complete (north-star KPI: 0). Offending sites:\n{list}",
        n = sites.len(),
        list = sites.join("\n")
    );
}

// ==========================================================================
// Internals — assertions + source-tree scan
// ==========================================================================

fn assert_links_local_stylesheet(body: &str, surface: &str) {
    let doc = html_assertions::parse(body);
    let links = html_assertions::select_all(&doc, r#"link[rel="stylesheet"]"#);
    // Inherited Feature B contract (ADR-B03): the hand-authored CSS is
    // cache-busted by a content-hash in its committed filename
    // (`/static/css/foundry.<hex>.css`); a full page rendered through base.html
    // links that hashed name. We assert the linked href is content-hashed, not
    // just any `/static/` href — identical to feature_b's check.
    let hashed = links.iter().any(|el| {
        el.value()
            .attr("href")
            .map(is_content_hashed_css_href)
            .unwrap_or(false)
    });
    assert!(
        hashed,
        "{surface} must link a CONTENT-HASHED vendored stylesheet \
         (/static/css/foundry.<hash>.css per ADR-B03, via base.html); today it emits a bare \
         <head> with no <link> — RED until DELIVER moves it into a template extending base.html. \
         links were {:?}; body:\n{body}",
        links
            .iter()
            .filter_map(|el| el.value().attr("href"))
            .collect::<Vec<_>>()
    );
}

fn is_content_hashed_css_href(href: &str) -> bool {
    let Some(stem) = href.strip_prefix("/static/css/foundry.") else {
        return false;
    };
    let Some(hash) = stem.strip_suffix(".css") else {
        return false;
    };
    !hash.is_empty() && hash.chars().all(|c| c.is_ascii_hexdigit())
}

fn assert_no_external_origin(body: &str, surface: &str) {
    let doc = html_assertions::parse(body);
    for css in ["script[src]", r#"link[rel="stylesheet"]"#] {
        for el in html_assertions::select_all(&doc, css) {
            let attr = if css.starts_with("script") {
                "src"
            } else {
                "href"
            };
            if let Some(v) = el.value().attr(attr) {
                let lower = v.to_ascii_lowercase();
                assert!(
                    !lower.starts_with("http://")
                        && !lower.starts_with("https://")
                        && !lower.starts_with("//"),
                    "{surface} references an external origin asset {v:?} (must be /static-local)"
                );
            }
        }
    }
}

/// Scan the foundry-app handler sources for the tell of an inline bare-`<head>`
/// FULL-PAGE HTML document emitted from Rust (a `<!doctype` literal). Returns
/// one `"file:line"` per offending site. Empty ⇒ the cut is complete.
///
/// Mirrors feature_b's on-disk `vendored_htmx_files` filesystem contract: a
/// real source-tree check, not a service call. Bare FRAGMENTS have no `<head>`
/// so the `<!doctype>` tell already excludes them (fragment-vs-full-page rule).
fn inline_full_page_sites() -> Vec<String> {
    let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../foundry-app/src");
    let mut hits = Vec::new();
    for entry in std::fs::read_dir(&src_dir).expect("read foundry-app/src") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let fname = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        for (idx, line) in text.lines().enumerate() {
            // The unambiguous tell of an inline FULL PAGE: a string literal
            // opening an HTML document. We match `<!doctype` case-insensitively
            // (every offending site today uses `<!doctype html>`).
            if line.to_ascii_lowercase().contains("<!doctype") {
                hits.push(format!("{fname}:{}", idx + 1));
            }
        }
    }
    hits
}
