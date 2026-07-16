//! Feature B "htmx Web Tier" step definitions — the templated, styled web
//! surfaces (US-B01 board, US-B02 vendored assets, US-B03 comment partial +
//! the OOB-affordance bug fix, US-B04 sign-in layout, US-B05 htmx 2).
//!
//! RED-state contract (DISTILL, ADR-025 / Mandate 7):
//! These steps drive the SAME in-process axum harness (`InProcHarness` →
//! `build_router`) the rest of the browser suite uses, over real HTTP through
//! `reqwest`. The render contract is selector-and-substring-identical
//! (DESIGN ADR-B02), so structural assertions go through `scraper`
//! (support::html_assertions) and copy assertions through `body.contains`.
//!
//! This feature is a MOVE-ONLY refactor: the EXISTING board / comment /
//! sign-in scenarios already pass for the current `format!` output and are the
//! regression net (NFR-WEBB-COMPAT-01) — they are NOT re-asserted here. These
//! steps assert ONLY the genuine user-visible DELTAS, each of which fails RED
//! for MISSING_FUNCTIONALITY today:
//!   - US-B01/B04: the board / sign-in page now REFERENCE the vendored
//!     `/static` assets — today `render_board` / `render_signin_form` emit no
//!     `<link>`/`<script>` and there is no `/static` route, so the
//!     asset-reference assertion fails.
//!   - US-B02/B05: `GET /static/...` 404s (static/ empty + route unmounted),
//!     so the served-asset assertions fail.
//!   - US-B03: the live (htmx OOB) comment card omits Edit/Delete
//!     (`comments.rs::render_comment_card_oob` ~:828-858), so the
//!     live-vs-reloaded structural-parity assertion fails — this is the bug fix
//!     made observable.
//!
//! Background phrases are REUSED from us_06/us_07/us_08/feature_a (cucumber-rs
//! requires globally-unique step text):
//!   - `a workspace "..." exists with admin "..."`               (us_06_signin)
//!   - `a member "..." belongs to the team "..."`                (us_07_project_create)
//!   - `a member "..." is registered with password "..."`        (us_06_signin)
//!   - `a project "..." with key prefix "..." exists in the "..." team` (us_08_file_issue)
//!   - `the "..." project has issue ... titled "..." (in progress|in the backlog)` (feature_a)
//!   - `the "..." project has no issues`                         (feature_a)
//!
//! Only Feature-B-specific phrases are declared below.
//!
//! What DELIVER must wire to flip these GREEN is enumerated in
//! `docs/feature/htmx-web-tier/distill/step-skeletons.md`.

use crate::support::harness::InProcHarness;
use crate::support::html_assertions;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use std::collections::HashMap;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
/// Member password seeded by `a member "..." belongs to the team "..."`
/// (us_07_project_create::MEMBER_PASSWORD). Mirrored here so the Feature-B
/// authenticated GETs can sign the member in.
const MEMBER_PASSWORD: &str = "mei-correct-horse-battery-staple";
/// Admin password seeded by `a workspace "..." exists with admin "..."`
/// (us_06_signin::workspace_with_admin).
const ADMIN_PASSWORD: &str = "admin-password-from-bootstrap";
/// Password Mei registers with via the US-B04 Background
/// `a member "..." is registered with password "correct horse battery staple"`.
const SIGNIN_PASSWORD: &str = "correct horse battery staple";

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

fn email_for(persona: &str) -> String {
    match persona.to_ascii_lowercase().as_str() {
        "devansh" => "devansh@acme.com".to_string(),
        "hiroshi" => "hiroshi@acme.com".to_string(),
        _ => "mei@acme.com".to_string(),
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

/// Sign in as `email`/`password` over the real cookie path; return the
/// `foundry_session=...` cookie pair. Mirrors the per-module helper in
/// us_10_comments (the modules link independently, so the small helper is
/// duplicated rather than re-exported).
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

/// Resolve the password for a signed-in persona.
fn password_for(email: &str) -> &'static str {
    if email == "devansh@acme.com" {
        ADMIN_PASSWORD
    } else {
        MEMBER_PASSWORD
    }
}

/// Authenticated GET of `path`, returning the body text. Signs the recorded
/// Feature-B persona in fresh (no cookie jar) and presents the session cookie.
async fn signed_in_get(world: &FoundryWorld, path: &str) -> (StatusCode, String) {
    let email = world
        .b_signed_in_email
        .clone()
        .expect("a Feature-B persona is signed in");
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

fn board_path(project_name: &str) -> String {
    // Slice-1/2 fixtures all live under the Backend team.
    format!("/team/backend/project/{slug}", slug = slugify(project_name))
}

// ==========================================================================
// Given — signed-in personas (Feature-B-specific phrasing)
// ==========================================================================

#[given(regex = r#"^(\w+) is signed in as a Backend member$"#)]
async fn signed_in_member(world: &mut FoundryWorld, persona: String) {
    ensure_harness(world).await;
    world.b_signed_in_email = Some(email_for(&persona));
    world.b_signed_in_password = Some(MEMBER_PASSWORD.to_string());
}

#[given(regex = r#"^(\w+) is signed in as the workspace admin$"#)]
async fn signed_in_admin(world: &mut FoundryWorld, persona: String) {
    ensure_harness(world).await;
    world.b_signed_in_email = Some(email_for(&persona));
    world.b_signed_in_password = Some(ADMIN_PASSWORD.to_string());
}

#[given(regex = r#"^(\w+) has no current browser session$"#)]
async fn no_browser_session(world: &mut FoundryWorld, _persona: String) {
    ensure_harness(world).await;
    world.b_signed_in_email = None;
}

#[given(regex = r#"^the foundry binary is running$"#)]
async fn binary_running(world: &mut FoundryWorld) {
    ensure_harness(world).await;
}

#[given(regex = r#"^the workspace admin (\w+) also belongs to the Backend team$"#)]
async fn admin_joins_backend(world: &mut FoundryWorld, persona: String) {
    ensure_harness(world).await;
    // The admin user already exists (seeded by the workspace Background); we
    // only add a Backend team membership so the admin can VIEW the issue page
    // (team-membership-gated) and exercise the admin delete affordance. A pure
    // precondition row, not the behaviour under test.
    let email = email_for(&persona);
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let user_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(email.to_ascii_lowercase())
        .fetch_one(pool)
        .await
        .expect("admin user exists");
    let team_id: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM teams WHERE slug = 'backend' LIMIT 1")
            .fetch_one(pool)
            .await
            .expect("backend team exists");
    sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')
             ON CONFLICT DO NOTHING",
    )
    .bind(team_id.0)
    .bind(user_id.0)
    .execute(pool)
    .await
    .expect("add admin to backend team");
}

#[given(regex = r#"^the board template is configured to fail rendering$"#)]
async fn board_template_fails(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    // DELIVER (step 01-03): flip the real test-only AppState seam
    // (`force_board_render_failure`, parallel to `db_unreachable`). The flag
    // is an `Arc<AtomicBool>` shared with the live router's AppState clone, so
    // flipping it here forces the board view's `render_board` to return `Err`,
    // which the handler maps to a CLEAN 500 (never a half-page). The
    // `b_force_template_failure` World bool stays as a record of intent.
    //
    // Phase-4 FIX 2 — isolation: each cucumber scenario gets a FRESH
    // `FoundryWorld` (`#[world(init = Self::default)]`), so `ensure_harness`
    // spawns a brand-new `InProcHarness` with a fresh `AppState` whose
    // `force_board_render_failure`/`db_unreachable` flags start `false`; the
    // flag therefore cannot leak across scenarios. To be robust to any
    // WITHIN-scenario reuse (a board-failure step followed by a board-success
    // step on the same harness), we first reset both test seams to a known
    // baseline, then arm only the render-failure seam. `reset_test_seams`
    // makes the arming idempotent and self-documenting.
    reset_test_seams(world);
    world.b_force_template_failure = true;
    let harness = world.harness.as_ref().expect("harness");
    harness
        .app
        .state
        .force_board_render_failure
        .store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Reset the test-only `AppState` seams (`force_board_render_failure` and
/// `db_unreachable`) on the live harness back to `false`. Per-scenario fresh
/// Worlds already isolate these, but resetting before arming the render-failure
/// seam guards against any within-scenario harness reuse leaving a stale flag
/// that would spuriously 500 a later board GET (Phase-4 FIX 2).
fn reset_test_seams(world: &FoundryWorld) {
    use std::sync::atomic::Ordering::SeqCst;
    let harness = world.harness.as_ref().expect("harness");
    harness
        .app
        .state
        .force_board_render_failure
        .store(false, SeqCst);
    harness.app.state.db_unreachable.store(false, SeqCst);
}

#[given(regex = r#"^(\w+) has posted the comment "([^"]+)" on (\w+)-(\d+)$"#)]
async fn has_posted_comment(
    world: &mut FoundryWorld,
    persona: String,
    body: String,
    _prefix: String,
    number: i32,
) {
    ensure_harness(world).await;
    let email = email_for(&persona);
    post_comment(world, &email, MEMBER_PASSWORD, number, &body).await;
}

// ==========================================================================
// When — board / issue / sign-in / asset GETs
// ==========================================================================

#[when(regex = r#"^(\w+) opens the "([^"]+)" board in her browser$"#)]
async fn open_board(world: &mut FoundryWorld, _persona: String, project_name: String) {
    let path = board_path(&project_name);
    let (status, body) = signed_in_get(world, &path).await;
    world.b_last_status = Some(status);
    world.b_last_body = Some(body);
}

#[when(regex = r#"^(\w+) opens the (\w+)-(\d+) issue page$"#)]
async fn open_issue_page(world: &mut FoundryWorld, _persona: String, _prefix: String, number: i32) {
    let path = format!("/team/backend/project/auth-v2/issues/{number}");
    let (status, body) = signed_in_get(world, &path).await;
    world.b_last_status = Some(status);
    world.b_last_body = Some(body);
}

#[when(regex = r#"^(\w+) reopens the (\w+)-(\d+) issue page after a full reload$"#)]
async fn reopen_issue_page(
    world: &mut FoundryWorld,
    _persona: String,
    _prefix: String,
    number: i32,
) {
    let path = format!("/team/backend/project/auth-v2/issues/{number}");
    let (_status, body) = signed_in_get(world, &path).await;
    world.b_reloaded_page = Some(body);
}

#[when(regex = r#"^(\w+) posts the comment "([^"]+)" on (\w+)-(\d+)$"#)]
async fn posts_comment(
    world: &mut FoundryWorld,
    persona: String,
    body: String,
    _prefix: String,
    number: i32,
) {
    ensure_harness(world).await;
    let email = email_for(&persona);
    post_comment(world, &email, MEMBER_PASSWORD, number, &body).await;
    // The OOB fragment returned by the post IS the live card.
    world.b_live_fragment = world.b_last_body.clone();
}

#[when(regex = r#"^(\w+) edits her comment on (\w+)-(\d+) to read "([^"]+)"$"#)]
async fn edits_comment(
    world: &mut FoundryWorld,
    persona: String,
    _prefix: String,
    number: i32,
    new_body: String,
) {
    ensure_harness(world).await;
    let email = email_for(&persona);
    edit_comment(world, &email, MEMBER_PASSWORD, number, &new_body).await;
}

#[when(regex = r#"^(\w+) files an issue on "([^"]+)" titled "([^"]+)"$"#)]
async fn files_issue(
    world: &mut FoundryWorld,
    persona: String,
    project_name: String,
    title: String,
) {
    ensure_harness(world).await;
    let email = email_for(&persona);
    file_issue(world, &email, MEMBER_PASSWORD, &project_name, &title).await;
    world.b_live_fragment = world.b_last_body.clone();
}

// ---- US-B04 sign-in / forgot ----

#[when(regex = r#"^(\w+) opens the sign-in page$"#)]
async fn open_signin(world: &mut FoundryWorld, _persona: String) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let resp = http
        .get(format!("{base}/sign-in", base = harness.base_url()))
        .send()
        .await
        .expect("get /sign-in");
    world.b_last_status = Some(resp.status());
    world.b_last_headers = Some(resp.headers().clone());
    world.b_last_body = Some(resp.text().await.unwrap_or_default());
}

#[when(regex = r#"^(\w+) opens the forgot-password page$"#)]
async fn open_forgot(world: &mut FoundryWorld, _persona: String) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    // The forgot-password route path mirrors the existing signin.rs handler.
    let resp = http
        .get(format!("{base}/forgot-password", base = harness.base_url()))
        .send()
        .await
        .expect("get /forgot-password");
    world.b_last_status = Some(resp.status());
    world.b_last_body = Some(resp.text().await.unwrap_or_default());
}

#[when(regex = r#"^(\w+) submits valid credentials on the sign-in page$"#)]
async fn submit_valid_signin(world: &mut FoundryWorld, _persona: String) {
    submit_signin(world, "mei@acme.com", SIGNIN_PASSWORD).await;
}

#[when(regex = r#"^(\w+) submits a wrong password on the sign-in page$"#)]
async fn submit_wrong_password(world: &mut FoundryWorld, _persona: String) {
    submit_signin(world, "mei@acme.com", "definitely-the-wrong-password").await;
}

#[when(regex = r#"^an unknown visitor submits an unregistered email on the sign-in page$"#)]
async fn submit_unknown_email(world: &mut FoundryWorld) {
    submit_signin(world, "nobody@acme.com", "whatever-password").await;
}

#[when(regex = r#"^a sign-in is submitted without a valid anti-forgery token$"#)]
async fn submit_no_csrf(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("email", "mei@acme.com".to_string());
    form.insert("password", SIGNIN_PASSWORD.to_string());
    // No `_csrf` field and no cookie — the double-submit check must refuse.
    let resp = http
        .post(format!("{base}/sign-in", base = harness.base_url()))
        .form(&form)
        .send()
        .await
        .expect("post /sign-in without csrf");
    world.b_last_status = Some(resp.status());
    world.b_last_body = Some(resp.text().await.unwrap_or_default());
}

// ---- US-B02 / US-B05 static assets ----

#[when(regex = r#"^a browser requests the vendored htmx script from the static path$"#)]
async fn request_htmx_asset(world: &mut FoundryWorld) {
    request_static(world, "/static/vendor/htmx.min.js").await;
}

#[when(regex = r#"^a browser requests the vendored Foundry stylesheet from the static path$"#)]
async fn request_css_asset(world: &mut FoundryWorld) {
    // ADR-B03: the CSS is served under a content-hashed name. We discover the
    // exact name from the on-disk vendored file rather than pinning a literal,
    // so a future CSS edit (new hash) does not break this step.
    request_static(world, &content_hashed_css_path()).await;
}

/// Resolve the served path of the content-hashed CSS by inspecting the
/// committed `static/css/` directory (`foundry.<hash>.css`). Mirrors what
/// `base.html` references; keeps the WS asset GET name-agnostic.
fn content_hashed_css_path() -> String {
    let css_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../foundry-app/static/css");
    let name = std::fs::read_dir(&css_dir)
        .expect("read static/css dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .find(|n| n.starts_with("foundry.") && n.ends_with(".css") && n != "foundry.css")
        .expect("a content-hashed foundry.<hash>.css exists (ADR-B03)");
    format!("/static/css/{name}")
}

#[when(regex = r#"^a browser requests a stylesheet that was never vendored$"#)]
async fn request_missing_asset(world: &mut FoundryWorld) {
    request_static(world, "/static/css/does-not-exist.css").await;
}

#[when(
    regex = r#"^a browser tries to reach a file outside the static directory through the static path$"#
)]
async fn request_traversal(world: &mut FoundryWorld) {
    // ServeDir must refuse traversal; the raw `..` segment is what an attacker
    // would send. We send it un-normalized so the route (not reqwest) decides.
    request_static(world, "/static/../Cargo.toml").await;
}

// ==========================================================================
// Then — board (US-B01) + render-contract markers (US-B05)
// ==========================================================================

#[then(regex = r#"^the board still shows the columns "([^"]+)", "([^"]+)", "([^"]+)", "([^"]+)"$"#)]
async fn board_shows_columns(world: &mut FoundryWorld, a: String, b: String, c: String, d: String) {
    let body = board_body(world);
    for label in [&a, &b, &c, &d] {
        assert!(
            body.contains(label.as_str()),
            "board is missing column label {label:?}; body was:\n{body}"
        );
    }
}

#[then(
    regex = r#"^the board still shows the cards for (\w+)-(\d+) and (\w+)-(\d+) in their columns$"#
)]
async fn board_shows_cards(world: &mut FoundryWorld, p1: String, n1: i32, p2: String, n2: i32) {
    let body = board_body(world);
    for key in [format!("{p1}-{n1}"), format!("{p2}-{n2}")] {
        html_assertions::assert_has(&body, &format!(r#"[data-issue-key="{key}"]"#));
    }
}

#[then(
    regex = r#"^the board links the vendored stylesheet from the application's own static path$"#
)]
async fn board_links_stylesheet(world: &mut FoundryWorld) {
    let body = board_body(world);
    assert_links_local_stylesheet(&body, "board");
}

#[then(
    regex = r#"^the board loads the vendored htmx script from the application's own static path$"#
)]
async fn board_loads_scripts(world: &mut FoundryWorld) {
    let body = board_body(world);
    assert_loads_local_scripts(&body, "board");
}

#[then(regex = r#"^the board references no external origin$"#)]
async fn board_no_external_origin(world: &mut FoundryWorld) {
    let body = board_body(world);
    assert_no_external_origin(&body, "board");
}

#[then(regex = r#"^the board shows guidance explaining how to file the first issue$"#)]
async fn board_empty_guidance(world: &mut FoundryWorld) {
    let body = board_body(world);
    // The asserted contract is only "guidance explaining how to file" — keep a
    // recognizable empty-state element (render-contract.md §Board). The `c`
    // shortcut is the in-product way to file.
    let lower = body.to_lowercase();
    assert!(
        lower.contains("press c") || lower.contains("file the first"),
        "empty board shows no file-the-first-issue guidance; body was:\n{body}"
    );
}

#[then(
    regex = r#"^the board carries the keyboard-navigation list with (\w+)-(\d+) before (\w+)-(\d+)$"#
)]
async fn board_kb_order(world: &mut FoundryWorld, p1: String, n1: i32, p2: String, n2: i32) {
    let body = board_body(world);
    // The hidden #kb-items carrier is ASC-sorted by issue number
    // (render-contract.md §Board); the US-12 ordering check reads
    // [data-issue-key] in document order under it.
    let keys =
        html_assertions::collect_attributes(&body, "#kb-items [data-issue-key]", "data-issue-key");
    let want1 = format!("{p1}-{n1}");
    let want2 = format!("{p2}-{n2}");
    let i1 = keys.iter().position(|k| k == &want1);
    let i2 = keys.iter().position(|k| k == &want2);
    assert!(
        matches!((i1, i2), (Some(a), Some(b)) if a < b),
        "keyboard-nav order wrong: expected {want1} before {want2}, got {keys:?}"
    );
}

#[then(regex = r#"^the board responds with a clean server error$"#)]
async fn board_clean_500(world: &mut FoundryWorld) {
    let status = world.b_last_status.expect("status captured");
    assert_eq!(
        status.as_u16(),
        500,
        "expected a clean 500 from a failed render, got {status}"
    );
}

#[then(regex = r#"^the response is not a partially rendered page$"#)]
async fn board_not_half_page(world: &mut FoundryWorld) {
    let body = world.b_last_body.clone().unwrap_or_default();
    // A clean error is not a half-emitted board: it must not contain a
    // dangling open board structure with no closing document.
    assert!(
        !body.contains("<section class=\"column\"") || body.contains("</html>"),
        "render error emitted a partial board page; body was:\n{body}"
    );
}

#[then(regex = r#"^the board carries the column marker for the backlog column$"#)]
async fn board_column_marker(world: &mut FoundryWorld) {
    let body = board_body(world);
    html_assertions::assert_has(&body, "[data-column='backlog']");
}

#[then(regex = r#"^the board carries the issue-key markers on its cards$"#)]
async fn board_issue_key_markers(world: &mut FoundryWorld) {
    let body = board_body(world);
    html_assertions::assert_has(&body, "[data-issue-key]");
}

#[then(regex = r#"^the issue page carries the comment-list marker$"#)]
async fn issue_comment_list_marker(world: &mut FoundryWorld) {
    let body = world.b_last_body.clone().expect("issue page fetched");
    html_assertions::assert_has(&body, "[data-comment-list]");
}

// ==========================================================================
// Then — comments (US-B03) — the live-vs-reloaded affordance fix
// ==========================================================================

#[then(
    regex = r#"^the live-appended comment card and the reloaded comment card are structurally identical$"#
)]
async fn live_matches_reloaded(world: &mut FoundryWorld) {
    let live = world
        .b_live_fragment
        .clone()
        .expect("a live OOB comment fragment was captured");
    let reloaded = world
        .b_reloaded_page
        .clone()
        .expect("the reloaded issue page was captured");
    // Both must carry the SAME comment-card structure including the action
    // affordances. Today render_comment_card_oob (comments.rs:841) omits the
    // .comment-actions buttons in the live fragment, so the live card lacks an
    // edit affordance the reloaded card has — divergence → RED.
    let live_doc = html_assertions::parse(&live);
    let live_has_actions = !html_assertions::select_all(&live_doc, ".comment-actions").is_empty();
    let reloaded_doc = html_assertions::parse(&reloaded);
    let reloaded_has_actions =
        !html_assertions::select_all(&reloaded_doc, ".comment-actions").is_empty();
    assert!(
        live_has_actions && reloaded_has_actions,
        "live card actions={live_has_actions}, reloaded card actions={reloaded_has_actions}; \
         the live OOB card must carry the SAME .comment-actions as the reloaded card. \
         live fragment:\n{live}\nreloaded page:\n{reloaded}"
    );
    let live_has_edit = !html_assertions::select_all(&live_doc, ".comment-edit-button").is_empty();
    let reloaded_has_edit =
        !html_assertions::select_all(&reloaded_doc, ".comment-edit-button").is_empty();
    assert_eq!(
        live_has_edit, reloaded_has_edit,
        "live card edit affordance ({live_has_edit}) must match reloaded card ({reloaded_has_edit})"
    );
}

#[then(regex = r#"^the live-appended comment card offers (\w+) the edit affordance$"#)]
async fn live_offers_edit(world: &mut FoundryWorld, _persona: String) {
    let live = world
        .b_live_fragment
        .clone()
        .expect("live fragment captured");
    html_assertions::assert_has(&live, ".comment-edit-button");
}

#[then(regex = r#"^the live-appended comment card offers (\w+) the delete affordance$"#)]
async fn live_offers_delete(world: &mut FoundryWorld, _persona: String) {
    let live = world
        .b_live_fragment
        .clone()
        .expect("live fragment captured");
    html_assertions::assert_has(&live, ".comment-delete-button");
}

#[then(regex = r#"^the comment card by (\w+) shows (?:her|him|them) as the author$"#)]
async fn comment_shows_author(world: &mut FoundryWorld, persona: String) {
    let body = world.b_last_body.clone().expect("issue page fetched");
    let email = email_for(&persona);
    assert!(
        html_assertions::comment_section_by_author(&body, &email).is_some(),
        "no comment card authored by {email} on the issue page; body:\n{body}"
    );
}

#[then(regex = r#"^the comment card by (\w+) shows the rendered comment body "([^"]+)"$"#)]
async fn comment_shows_body(world: &mut FoundryWorld, persona: String, expected: String) {
    let body = world.b_last_body.clone().expect("issue page fetched");
    let email = email_for(&persona);
    html_assertions::assert_comment_has_element_with_text(
        &body,
        &email,
        ".comment-body",
        &expected,
    );
}

#[then(regex = r#"^the comment card by (\w+) shows the edited marker$"#)]
async fn comment_shows_edited(world: &mut FoundryWorld, persona: String) {
    let body = world.b_last_body.clone().expect("issue page fetched");
    let email = email_for(&persona);
    let Some(section) = html_assertions::comment_section_by_author(&body, &email) else {
        panic!("no comment by {email} on issue page;\n{body}");
    };
    assert!(
        !html_assertions::select_all(&section, ".comment-edited-marker").is_empty(),
        "comment by {email} is missing the (edited) marker;\n{body}"
    );
}

#[then(regex = r#"^the comment card by (\w+) offers (\w+) no edit affordance$"#)]
async fn comment_no_edit(world: &mut FoundryWorld, author: String, _viewer: String) {
    let body = world.b_last_body.clone().expect("issue page fetched");
    let email = email_for(&author);
    html_assertions::assert_comment_has_no_element(&body, &email, ".comment-edit-button");
}

#[then(regex = r#"^the comment card by (\w+) offers (\w+) no delete affordance$"#)]
async fn comment_no_delete(world: &mut FoundryWorld, author: String, _viewer: String) {
    let body = world.b_last_body.clone().expect("issue page fetched");
    let email = email_for(&author);
    html_assertions::assert_comment_has_no_element(&body, &email, ".comment-delete-button");
}

#[then(regex = r#"^the comment card by (\w+) offers (\w+) the delete affordance$"#)]
async fn comment_has_delete(world: &mut FoundryWorld, author: String, _viewer: String) {
    let body = world.b_last_body.clone().expect("issue page fetched");
    let email = email_for(&author);
    let Some(section) = html_assertions::comment_section_by_author(&body, &email) else {
        panic!("no comment by {email} on issue page;\n{body}");
    };
    assert!(
        !html_assertions::select_all(&section, ".comment-delete-button").is_empty(),
        "comment by {email} should offer a delete affordance to the viewer;\n{body}"
    );
}

#[then(regex = r#"^the comment card by (\w+) contains no script element$"#)]
async fn comment_no_script(world: &mut FoundryWorld, persona: String) {
    let body = world.b_last_body.clone().expect("issue page fetched");
    let email = email_for(&persona);
    html_assertions::assert_comment_has_no_element(&body, &email, "script");
}

#[then(regex = r#"^the comment card by (\w+) contains no javascript link$"#)]
async fn comment_no_js_link(world: &mut FoundryWorld, persona: String) {
    let body = world.b_last_body.clone().expect("issue page fetched");
    let email = email_for(&persona);
    let Some(section) = html_assertions::comment_section_by_author(&body, &email) else {
        panic!("no comment by {email} on issue page;\n{body}");
    };
    for el in html_assertions::select_all(&section, "a") {
        let href = el.value().attr("href").unwrap_or("");
        assert!(
            !href
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("javascript:"),
            "comment by {email} carries a javascript: link ({href:?}); sanitization failed"
        );
    }
}

// ==========================================================================
// Then — issue-file fragment (US-B05 regression)
// ==========================================================================

#[then(regex = r#"^the returned fragment appends the new card to the backlog column$"#)]
async fn fragment_appends_backlog(world: &mut FoundryWorld) {
    let body = world
        .b_live_fragment
        .clone()
        .expect("issue-file fragment captured");
    assert!(
        body.contains("hx-swap-oob") && body.to_lowercase().contains("backlog"),
        "issue-file fragment does not OOB-append to the Backlog column; fragment:\n{body}"
    );
}

#[then(regex = r#"^the new card carries the issue key$"#)]
async fn fragment_carries_key(world: &mut FoundryWorld) {
    let body = world
        .b_live_fragment
        .clone()
        .expect("issue-file fragment captured");
    html_assertions::assert_has(&body, "[data-issue-key]");
}

// ==========================================================================
// Then — sign-in layout (US-B04)
// ==========================================================================

#[then(
    regex = r#"^the sign-in page links the vendored stylesheet from the application's own static path$"#
)]
async fn signin_links_stylesheet(world: &mut FoundryWorld) {
    let body = world.b_last_body.clone().expect("sign-in page fetched");
    assert_links_local_stylesheet(&body, "sign-in");
}

#[then(
    regex = r#"^the forgot-password page links the vendored stylesheet from the application's own static path$"#
)]
async fn forgot_links_stylesheet(world: &mut FoundryWorld) {
    let body = world.b_last_body.clone().expect("forgot page fetched");
    assert_links_local_stylesheet(&body, "forgot-password");
}

#[then(regex = r#"^the sign-in page renders from the shared base layout$"#)]
async fn signin_uses_base_layout(world: &mut FoundryWorld) {
    let body = world.b_last_body.clone().expect("sign-in page fetched");
    assert_uses_base_layout(&body, "sign-in");
}

#[then(regex = r#"^the forgot-password page renders from the shared base layout$"#)]
async fn forgot_uses_base_layout(world: &mut FoundryWorld) {
    let body = world.b_last_body.clone().expect("forgot page fetched");
    assert_uses_base_layout(&body, "forgot-password");
}

#[then(regex = r#"^(\w+) is signed in and lands on the dashboard$"#)]
async fn signed_in_lands_dashboard(world: &mut FoundryWorld, _persona: String) {
    let status = world.b_last_status.expect("status captured");
    assert!(
        status.is_redirection() || status.is_success(),
        "sign-in should succeed (redirect/200), got {status}"
    );
}

#[then(
    regex = r#"^her browser holds a session cookie that is HttpOnly and Secure and SameSite=Lax and valid for 30 days$"#
)]
async fn signin_session_cookie(world: &mut FoundryWorld) {
    let headers = world.b_last_headers.as_ref().expect("headers captured");
    let cookie = headers
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .map(|s| s.to_string())
        .expect("sign-in sets foundry_session");
    let lower = cookie.to_ascii_lowercase();
    assert!(
        lower.contains("httponly"),
        "session cookie missing HttpOnly: {cookie}"
    );
    assert!(
        lower.contains("secure"),
        "session cookie missing Secure: {cookie}"
    );
    assert!(
        lower.contains("samesite=lax"),
        "session cookie missing SameSite=Lax: {cookie}"
    );
    assert!(
        cookie.contains("Max-Age=2592000") || lower.contains("max-age=2592000"),
        "session cookie not 30 days: {cookie}"
    );
}

#[then(regex = r#"^the styled sign-in form shows "([^"]+)"$"#)]
async fn signin_shows_error(world: &mut FoundryWorld, expected: String) {
    let body = world
        .b_last_body
        .clone()
        .expect("sign-in response captured");
    assert!(
        body.contains(&expected),
        "sign-in form does not show {expected:?}; body:\n{body}"
    );
}

#[then(regex = r#"^the sign-in page sets an anti-forgery cookie$"#)]
async fn signin_sets_csrf_cookie(world: &mut FoundryWorld) {
    let headers = world.b_last_headers.as_ref().expect("headers captured");
    let has = headers
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .any(|s| s.starts_with("foundry_csrf="));
    assert!(has, "GET /sign-in must set the foundry_csrf cookie");
}

#[then(regex = r#"^the sign-in form carries a matching hidden anti-forgery field$"#)]
async fn signin_hidden_csrf_field(world: &mut FoundryWorld) {
    let body = world.b_last_body.clone().expect("sign-in page fetched");
    html_assertions::assert_has(&body, r#"input[type="hidden"][name="_csrf"]"#);
}

#[then(regex = r#"^the sign-in submission is refused$"#)]
async fn signin_csrf_refused(world: &mut FoundryWorld) {
    let status = world.b_last_status.expect("status captured");
    assert_eq!(
        status.as_u16(),
        403,
        "a sign-in without a valid CSRF token must be refused 403, got {status}"
    );
}

// ==========================================================================
// Then — static assets (US-B02 / US-B05)
// ==========================================================================

#[then(regex = r#"^the response is delivered successfully with a JavaScript content type$"#)]
async fn asset_ok_js(world: &mut FoundryWorld) {
    assert_asset_ok(world, &["javascript", "ecmascript"]);
}

#[then(regex = r#"^the response is delivered successfully with a stylesheet content type$"#)]
async fn asset_ok_css(world: &mut FoundryWorld) {
    assert_asset_ok(world, &["css"]);
}

#[then(regex = r#"^the response carries a long-lived cache header$"#)]
async fn asset_cache_header(world: &mut FoundryWorld) {
    let cc = world
        .b_asset_cache_control
        .clone()
        .unwrap_or_default()
        .to_ascii_lowercase();
    assert!(
        cc.contains("max-age") || cc.contains("immutable"),
        "static asset is missing a long-lived cache header; Cache-Control was {cc:?}"
    );
}

#[then(regex = r#"^the response body is a non-empty script$"#)]
async fn asset_nonempty(world: &mut FoundryWorld) {
    let body = world.b_asset_body.clone().unwrap_or_default();
    assert!(
        body.trim().len() > 100,
        "vendored htmx script is empty or stub-sized ({} bytes)",
        body.len()
    );
}

#[then(regex = r#"^the request is refused as not found$"#)]
async fn asset_not_found(world: &mut FoundryWorld) {
    let status = world.b_asset_status.expect("asset status captured");
    assert_eq!(
        status.as_u16(),
        404,
        "a missing vendored asset must 404, got {status}"
    );
}

#[then(regex = r#"^the request is refused and no file outside the static directory is served$"#)]
async fn asset_traversal_refused(world: &mut FoundryWorld) {
    let status = world.b_asset_status.expect("asset status captured");
    let body = world.b_asset_body.clone().unwrap_or_default();
    assert!(
        status.is_client_error(),
        "path traversal must be refused (4xx), got {status}"
    );
    assert!(
        !body.contains("[package]") && !body.contains("foundry-app"),
        "path traversal leaked a file outside static/: {body}"
    );
}

#[then(regex = r#"^the vendored htmx script reports a version in the 2 series$"#)]
async fn htmx_is_v2(world: &mut FoundryWorld) {
    let body = world.b_asset_body.clone().unwrap_or_default();
    // The htmx blob records its version near the top of the file
    // (e.g. `htmx.org@2.0.4` / `version:"2.x"`); the served bytes must report 2.x.
    assert!(
        body.contains("2.0.") || body.contains("\"2.") || body.contains("@2."),
        "served htmx asset does not report a 2.x version (it is unvendored/htmx-1 today)"
    );
}

#[then(regex = r#"^exactly one htmx file is vendored under the static path$"#)]
async fn one_htmx_file(world: &mut FoundryWorld) {
    // Two halves of the contract, both verified here:
    // (1) the served-side proof — the htmx GET succeeded (not a 404), and
    let status = world.b_asset_status.expect("asset status captured");
    assert!(
        status.is_success(),
        "the single vendored htmx file must be served (got {status})"
    );
    // (2) the on-disk count — enumerate `static/vendor/htmx*.js` and assert
    //     EXACTLY ONE exists. A second htmx blob (e.g. a leftover htmx-1
    //     `htmx.min.js` alongside a `htmx2.min.js`) would make `base.html`'s
    //     single `<script src=".../htmx.min.js">` ambiguous and risk shipping
    //     two htmx runtimes. (The previously-cited "asset-resolution probe"
    //     xtask does not exist; this filesystem check is the real contract.)
    let htmx_files = vendored_htmx_files();
    assert_eq!(
        htmx_files.len(),
        1,
        "expected exactly one vendored htmx file under static/vendor/, found {}: {:?}",
        htmx_files.len(),
        htmx_files
    );
}

/// Enumerate vendored htmx JS blobs under `crates/foundry-app/static/vendor/`
/// (`htmx*.js`). Used to enforce the "exactly one htmx file" on-disk contract.
fn vendored_htmx_files() -> Vec<String> {
    let vendor_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../foundry-app/static/vendor");
    std::fs::read_dir(&vendor_dir)
        .expect("read static/vendor dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("htmx") && n.ends_with(".js"))
        .collect()
}

// ==========================================================================
// Internals — real HTTP against the in-process harness.
// ==========================================================================

fn board_body(world: &FoundryWorld) -> String {
    world
        .b_last_body
        .clone()
        .expect("a board/page GET was captured by the When step")
}

async fn request_static(world: &mut FoundryWorld, path: &str) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let resp = http
        .get(format!("{base}{path}", base = harness.base_url()))
        .send()
        .await
        .expect("GET static asset");
    let status = resp.status();
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let cache_control = resp
        .headers()
        .get(reqwest::header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let body = resp.text().await.unwrap_or_default();
    world.b_asset_status = Some(status);
    world.b_asset_content_type = content_type;
    world.b_asset_cache_control = cache_control;
    world.b_asset_body = Some(body);
}

fn assert_asset_ok(world: &FoundryWorld, type_fragments: &[&str]) {
    let status = world.b_asset_status.expect("asset status captured");
    assert!(
        status.is_success(),
        "vendored asset was not served (got {status}); static/ is empty + /static \
         route unmounted today — RED until DELIVER vendors the blobs and mounts ServeDir"
    );
    let ct = world
        .b_asset_content_type
        .clone()
        .unwrap_or_default()
        .to_ascii_lowercase();
    assert!(
        type_fragments.iter().any(|f| ct.contains(f)),
        "asset content type {ct:?} is none of {type_fragments:?}"
    );
}

fn assert_links_local_stylesheet(body: &str, surface: &str) {
    let doc = html_assertions::parse(body);
    let links = html_assertions::select_all(&doc, r#"link[rel="stylesheet"]"#);
    // ADR-B03 (assets.md §Decision #4, option 4a): the hand-authored CSS is
    // cache-busted by a content-hash in its COMMITTED filename
    // (`/static/css/foundry.<8-hex>.css`), so the blanket `immutable` long-cache
    // on `/static` is safe — a CSS edit changes the hash, changes the URL, and
    // misses the cache correctly. A mutable name (`foundry.css`) served
    // `immutable` would pin stale CSS for a year. We assert the linked href is
    // a content-hashed name, NOT just any `/static/` href.
    let hashed = links.iter().any(|el| {
        el.value()
            .attr("href")
            .map(is_content_hashed_css_href)
            .unwrap_or(false)
    });
    assert!(
        hashed,
        "{surface} page must link a CONTENT-HASHED vendored stylesheet \
         (/static/css/foundry.<hash>.css per ADR-B03) so the immutable cache is \
         safe on a mutable name; links were {:?}; body:\n{body}",
        links
            .iter()
            .filter_map(|el| el.value().attr("href"))
            .collect::<Vec<_>>()
    );
}

/// True iff `href` is the content-hashed vendored CSS path
/// (`/static/css/foundry.<hex>.css`, hash segment non-empty, lowercase hex).
fn is_content_hashed_css_href(href: &str) -> bool {
    let Some(stem) = href.strip_prefix("/static/css/foundry.") else {
        return false;
    };
    let Some(hash) = stem.strip_suffix(".css") else {
        return false;
    };
    !hash.is_empty() && hash.chars().all(|c| c.is_ascii_hexdigit())
}

/// AMENDED by keyboard-shortcut-bindings step 01-03 (user-ratified — see
/// docs/feature/keyboard-shortcut-bindings/deliver/upstream-issues.md UI-1).
/// This asserted `has_htmx && has_alpine`; the Alpine half is gone with the
/// framework (ADR-001), which had zero runtime consumers. htmx is asserted
/// exactly as before — it is the live one, driving every fragment swap on the
/// board.
fn assert_loads_local_scripts(body: &str, surface: &str) {
    let doc = html_assertions::parse(body);
    let scripts = html_assertions::select_all(&doc, "script[src]");
    let local_srcs: Vec<String> = scripts
        .iter()
        .filter_map(|el| el.value().attr("src").map(|s| s.to_string()))
        .filter(|s| s.starts_with("/static/"))
        .collect();
    assert!(
        local_srcs.iter().any(|s| s.contains("htmx")),
        "{surface} page does not load htmx from /static; local script srcs were {local_srcs:?}"
    );
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

fn assert_uses_base_layout(body: &str, surface: &str) {
    // The shared base layout is the single source of head/asset boilerplate
    // (NFR-WEBB-MAINT-01). Its tell is: a full <html> document that links the
    // vendored /static stylesheet (the thing a bare format! head lacks today).
    // We treat "extends base" observably as "is a full styled page", since the
    // template internals are not user-observable.
    assert!(
        body.contains("<html") && body.contains("</html>"),
        "{surface} is not a full HTML document; body:\n{body}"
    );
    assert_links_local_stylesheet(body, surface);
}

async fn submit_signin(world: &mut FoundryWorld, email: &str, password: &str) {
    ensure_harness(world).await;
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
    world.b_last_status = Some(resp.status());
    world.b_last_headers = Some(resp.headers().clone());
    world.b_last_body = Some(resp.text().await.unwrap_or_default());
}

/// Sign in as the author, mint a CSRF token bound to the session, then POST a
/// comment over the real htmx path. Captures the OOB fragment into b_last_body.
async fn post_comment(
    world: &mut FoundryWorld,
    email: &str,
    password: &str,
    issue_number: i32,
    body: &str,
) {
    let cookie = sign_in_and_capture_cookie(world, email, password).await;
    let (csrf_token, combined) = csrf_for_session(world, &cookie).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/team/backend/project/auth-v2/issues/{issue_number}/comments",
        base = harness.base_url()
    );
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("body", body.to_string());
    form.insert("_csrf", csrf_token);
    let resp = http
        .post(&url)
        .header(reqwest::header::COOKIE, combined)
        .header("hx-request", "true")
        .form(&form)
        .send()
        .await
        .expect("post comment");
    world.b_last_status = Some(resp.status());
    world.b_last_body = Some(resp.text().await.unwrap_or_default());
}

/// Edit Mei's most-recent comment on the issue over the htmx PATCH path.
async fn edit_comment(
    world: &mut FoundryWorld,
    email: &str,
    password: &str,
    issue_number: i32,
    new_body: &str,
) {
    let comment_id = latest_comment_id_by_author(world, email, issue_number).await;
    let cookie = sign_in_and_capture_cookie(world, email, password).await;
    let (csrf_token, combined) = csrf_for_session(world, &cookie).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/team/backend/project/auth-v2/issues/{issue_number}/comments/{comment_id}",
        base = harness.base_url()
    );
    let mut form: HashMap<&str, String> = HashMap::new();
    // The edit handler's EditCommentForm expects `body_markdown` (comments.rs:48),
    // not `body` (the create form's field). The edit re-submits the raw markdown.
    form.insert("body_markdown", new_body.to_string());
    form.insert("_csrf", csrf_token);
    let resp = http
        .patch(&url)
        .header(reqwest::header::COOKIE, combined)
        .header("hx-request", "true")
        .form(&form)
        .send()
        .await
        .expect("patch comment");
    world.b_last_status = Some(resp.status());
    world.b_last_body = Some(resp.text().await.unwrap_or_default());
}

/// File an issue over the real htmx create path; capture the OOB fragment.
async fn file_issue(
    world: &mut FoundryWorld,
    email: &str,
    password: &str,
    project_name: &str,
    title: &str,
) {
    let cookie = sign_in_and_capture_cookie(world, email, password).await;
    let (csrf_token, combined) = csrf_for_session(world, &cookie).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let slug = slugify(project_name);
    let url = format!(
        "{base}/team/backend/project/{slug}/issues",
        base = harness.base_url()
    );
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("title", title.to_string());
    form.insert("_csrf", csrf_token);
    let resp = http
        .post(&url)
        .header(reqwest::header::COOKIE, combined)
        .header("hx-request", "true")
        .form(&form)
        .send()
        .await
        .expect("post create issue");
    world.b_last_status = Some(resp.status());
    world.b_last_body = Some(resp.text().await.unwrap_or_default());
}

/// Mint a CSRF token bound to the given session cookie; return
/// (token, "session; foundry_csrf=token") combined cookie header.
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

/// Look up the most-recent comment id authored by `email` on the given issue
/// number (direct read-model SELECT; a precondition lookup, not the behaviour
/// under test).
async fn latest_comment_id_by_author(
    world: &FoundryWorld,
    email: &str,
    issue_number: i32,
) -> uuid::Uuid {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let row: (uuid::Uuid,) = sqlx::query_as(
        "SELECT c.id FROM comments c
           JOIN issues i ON i.id = c.issue_id
           JOIN users u ON u.id = c.author_id
          WHERE u.email_lower = $1 AND i.number = $2
          ORDER BY c.created_at DESC
          LIMIT 1",
    )
    .bind(email.to_ascii_lowercase())
    .bind(issue_number)
    .fetch_one(pool)
    .await
    .expect("find latest comment by author");
    row.0
}
