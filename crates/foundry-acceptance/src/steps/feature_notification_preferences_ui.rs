//! notification-preferences-ui — step definitions for the signed-in settings surface
//! that makes the SHIPPED per-workspace notification mute/subscribe backend reachable
//! and self-service from the app UI.
//!
//! HARNESS BOUNDARY: the app + Postgres are REAL (the shipped in-process axum harness
//! `support::harness::InProcHarness::spawn` + testcontainers, `@real-io`), driven through
//! the production composition root (`build_router`) — Pillar 3. Every scenario enters
//! through a DRIVING PORT: the shared sidebar rail on an authed page, `GET
//! /account/settings`, the NEW `POST /account/settings/mute`, or the SHIPPED `POST
//! /account/notifications/resubscribe` — never an internal function (Mandate 1). No
//! email delivery is observed (that is the shipped recipient-unsubscribe feature), so no
//! recording provider double is needed; the observables are the rendered surface + the
//! `notification_unsubscribes` opt-out state read at the store boundary.
//!
//! DELIVERED (all 12 scenarios GREEN): the NEW production seams
//! (`GET /account/settings` + `POST /account/settings/mute` in
//! `unsubscribe.rs`, the `sidebar__user` `<a href="/account/settings">`, the
//! `SettingsPage`/`SettingsRow` views + `templates/settings.html`, and the two
//! `build_router` route mounts under `session_layer` + `csrf_middleware`) are
//! wired; every step below asserts a real observable — the rendered `data-status`
//! marker, the HTTP status, or a `Store::is_unsubscribed` read at the store
//! boundary. Run this feature's lane with
//! `FOUNDRY_ACCEPTANCE_TAGS=notification-preferences-ui`.
//!
//! REUSED, per the globally-unique-phrase rule (cucumber-rs panics on duplicate step
//! registration) — DO NOT redefine here (defined in `feature_navigation_bar`):
//!   - `(\w+) opens the authenticated page "…"`
//!   - `the "…" navigation item is marked as the current page`
//!   - `exactly one primary navigation item is marked as the current page`
//!
//! The reused `opens the authenticated page` step reads `us_07_signed_in_email` /
//! `us_07_signed_in_password` (which the "Nadia is signed in …" Given below sets) and
//! writes `last_status` / `last_headers` / `last_body`, so the reused nav Then steps and
//! the NEW `sidebar footer offers a link …` Then compose over the SAME response body.
//! All NEW phrases below use the persona "Nadia" + the "… on the settings surface" /
//! "… notification settings surface" wording, globally distinct from the recipient
//! feature's "Maria"/"Sam" + "notification settings page" vocabulary.

use crate::support::harness::{
    establish_session, get_with_cookie, post_with_cookie, signed_in_get, signed_in_post,
    InProcHarness,
};
use crate::support::html_assertions as html;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use secrecy::SecretString;

const NPUI_NOW: &str = "2026-01-15T12:00:00Z";
/// The signed-in member persona these scenarios target — a real, sign-in-able user
/// (already lower-cased so `email_lower` matches the 0014 opt-out key). Globally
/// distinct from the recipient feature's Maria/Sam so no seed collides in `users`.
const NADIA_EMAIL: &str = "nadia@northwind.example";
const NADIA_PASSWORD: &str = "npui-correct-horse-battery-staple";
/// The dedicated signed-in settings surface (OD-1: pinned canonical URL).
const SETTINGS_PATH: &str = "/account/settings";
/// The NEW signed-in per-workspace mute action (OD-3: pinned path).
const MUTE_PATH: &str = "/account/settings/mute";
/// The SHIPPED signed-in resubscribe the surface reuses (FR-5 backwards-compat).
const RESUBSCRIBE_PATH: &str = "/account/notifications/resubscribe";

fn now_anchor() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(NPUI_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse anchor")
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(Policy::none())
        .cookie_store(false)
        .build()
        .expect("build reqwest client")
}

/// Ensure the per-scenario REAL harness + client exist (spawned once per scenario).
async fn ensure_harness(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        world.harness = Some(InProcHarness::spawn(now_anchor()).await);
    }
    if world.http.is_none() {
        world.http = Some(client());
    }
}

/// Seed one real, sign-in-able member (`email`/`password`) plus a `member` membership in
/// each named workspace, returning `name → workspace_id`. Mirrors the recipient feature's
/// Maria seed but for this feature's persona; the membership makes each workspace resolve
/// on sign-in and appear in the settings surface's `workspaces_for_member` list.
async fn seed_member_workspaces(
    harness: &InProcHarness,
    email: &str,
    password: &str,
    names: &[String],
) -> Vec<(String, uuid::Uuid)> {
    let pool = harness.app.state.store.pool();
    let hash = foundry_auth::hash_password(&SecretString::new(password.to_string().into()))
        .await
        .expect("hash member pw");
    let user_id = uuid::Uuid::now_v7();
    let lower = email.to_ascii_lowercase();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(&lower)
    .bind(email)
    .bind("Nadia Member")
    .bind(&hash)
    .execute(pool)
    .await
    .expect("insert member user");
    let mut out = Vec::with_capacity(names.len());
    for name in names {
        let workspace_id = uuid::Uuid::now_v7();
        sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
            .bind(workspace_id)
            .bind(name)
            .execute(pool)
            .await
            .expect("insert workspace");
        sqlx::query(
            "INSERT INTO workspace_memberships (workspace_id, user_id, role)
                  VALUES ($1, $2, 'member')",
        )
        .bind(workspace_id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("insert membership");
        out.push((name.clone(), workspace_id));
    }
    out
}

/// Seed a SECOND recipient (`email`) who is the sole `member` of a NEW `workspace` the
/// persona does not belong to. Returns the new workspace id. Used by the foreign-injection
/// scenario: proving a crafted foreign `workspace_id` cannot write this other recipient's
/// opt-out state.
async fn seed_other_recipient_workspace(
    harness: &InProcHarness,
    workspace: &str,
    email: &str,
) -> uuid::Uuid {
    let pool = harness.app.state.store.pool();
    let hash = foundry_auth::hash_password(&SecretString::new(NADIA_PASSWORD.to_string().into()))
        .await
        .expect("hash other-recipient pw");
    let user_id = uuid::Uuid::now_v7();
    let workspace_id = uuid::Uuid::now_v7();
    let lower = email.to_ascii_lowercase();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(workspace_id)
        .bind(workspace)
        .execute(pool)
        .await
        .expect("insert foreign workspace");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(&lower)
    .bind(email)
    .bind("Vic Other")
    .bind(&hash)
    .execute(pool)
    .await
    .expect("insert other-recipient user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert other-recipient membership");
    workspace_id
}

/// Resolve the id of a workspace the persona belongs to. PANICS if she does not — the
/// falsifiable precondition every mute/resubscribe step relies on.
fn nadia_ws_id(world: &FoundryWorld, name: &str) -> uuid::Uuid {
    world
        .npui_workspaces
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, id)| *id)
        .unwrap_or_else(|| panic!("Nadia must belong to {name:?}"))
}

fn creds(world: &FoundryWorld) -> (String, String) {
    world
        .npui_creds
        .clone()
        .expect("the persona's credentials were recorded by the sign-in Given")
}

fn body_of(world: &FoundryWorld) -> String {
    world.last_body.clone().unwrap_or_default()
}

/// Re-open the settings surface freshly as the signed-in persona, returning the rendered
/// `(status, body)`. Used by the "shown as muted/subscribed on the settings surface" Then
/// steps so they observe the CURRENT server state through the real GET, independent of
/// whatever the preceding mute/resubscribe POST returned.
async fn open_settings_signed_in(world: &FoundryWorld) -> (StatusCode, String) {
    let (email, password) = creds(world);
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let out = signed_in_get(harness, http, &email, &password, SETTINGS_PATH).await;
    (out.status, out.body)
}

/// Assert the settings surface renders `name` in some per-workspace status row (the
/// shipped `data-status` marker — the prose-immune oracle).
fn assert_listed(body: &str, name: &str) {
    let doc = html::parse(body);
    let found = html::select_all(&doc, "[data-status]")
        .into_iter()
        .any(|el| el.text().collect::<String>().contains(name));
    assert!(
        found,
        "the settings surface must list {name:?} with a mute status; body:\n{body}"
    );
}

/// Assert `name` renders with the given `status` marker (`muted`/`subscribed`).
fn assert_row_status(body: &str, name: &str, status: &str) {
    let doc = html::parse(body);
    let selector = format!(r#"[data-status="{status}"]"#);
    let found = html::select_all(&doc, &selector)
        .into_iter()
        .any(|el| el.text().collect::<String>().contains(name));
    assert!(
        found,
        "the settings surface must show {name:?} as {status}; body:\n{body}"
    );
}

// ============================================================================
// Given — a signed-in member with per-workspace state
// ============================================================================

#[given(regex = r#"^Nadia is signed in and belongs to "([^"]+)", "([^"]+)", and "([^"]+)"$"#)]
async fn nadia_signed_in_belongs_to(
    world: &mut FoundryWorld,
    ws_a: String,
    ws_b: String,
    ws_c: String,
) {
    ensure_harness(world).await;
    let workspaces = {
        let harness = world.harness.as_ref().expect("harness");
        seed_member_workspaces(harness, NADIA_EMAIL, NADIA_PASSWORD, &[ws_a, ws_b, ws_c]).await
    };
    world.npui_workspaces = workspaces;
    world.npui_creds = Some((NADIA_EMAIL.to_string(), NADIA_PASSWORD.to_string()));
    // Feed the reused navigation-bar `opens the authenticated page` glue (Pillar 2/3).
    world.us_07_signed_in_email = Some(NADIA_EMAIL.to_string());
    world.us_07_signed_in_password = Some(NADIA_PASSWORD.to_string());
}

#[given(regex = r#"^Nadia has muted "([^"]+)"$"#)]
async fn nadia_has_muted(world: &mut FoundryWorld, workspace: String) {
    let workspace_id = nadia_ws_id(world, &workspace);
    let harness = world.harness.as_ref().expect("harness");
    harness
        .app
        .state
        .store
        .insert_unsubscribe(&NADIA_EMAIL.to_ascii_lowercase(), workspace_id)
        .await
        .expect("seed the muted precondition at the store boundary");
}

// ============================================================================
// When — driving-port actions (sidebar link, settings GET, mute/resubscribe POST)
// ============================================================================

#[when(
    regex = r#"^Nadia opens an authenticated page and follows the settings link in the sidebar$"#
)]
async fn nadia_follows_settings_link(world: &mut FoundryWorld) {
    let (email, password) = creds(world);
    let session = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        establish_session(harness, http, &email, &password).await
    };
    // (1) Land on an authed page and read the sidebar footer's settings link.
    let home = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        get_with_cookie(harness, http, "/", &session).await
    };
    let href = html::collect_attributes(&home.body, ".sidebar__user a", "href")
        .into_iter()
        .find(|h| h.starts_with(SETTINGS_PATH))
        .unwrap_or_else(|| {
            panic!(
                "the sidebar footer must offer a settings link; body:\n{}",
                home.body
            )
        });
    // (2) Follow it end-to-end to the settings surface.
    let settings = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        get_with_cookie(harness, http, &href, &session).await
    };
    world.dash_session_cookie = Some(session);
    world.last_status = Some(settings.status);
    world.last_headers = Some(settings.headers);
    world.last_body = Some(settings.body);
}

#[when(regex = r#"^Nadia opens an authenticated page$"#)]
async fn nadia_opens_authenticated_page(world: &mut FoundryWorld) {
    let (email, password) = creds(world);
    let out = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        signed_in_get(harness, http, &email, &password, "/").await
    };
    world.last_status = Some(out.status);
    world.last_headers = Some(out.headers);
    world.last_body = Some(out.body);
}

#[when(regex = r#"^Nadia opens the notification settings surface$"#)]
async fn nadia_opens_settings_surface(world: &mut FoundryWorld) {
    let (email, password) = creds(world);
    let out = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        signed_in_get(harness, http, &email, &password, SETTINGS_PATH).await
    };
    world.last_status = Some(out.status);
    world.last_headers = Some(out.headers);
    world.last_body = Some(out.body);
}

#[when(regex = r#"^Nadia mutes "([^"]+)" from the settings surface$"#)]
async fn nadia_mutes_workspace(world: &mut FoundryWorld, workspace: String) {
    let workspace_id = nadia_ws_id(world, &workspace);
    world.npui_last_target = Some(workspace_id);
    let (email, password) = creds(world);
    let ws = workspace_id.to_string();
    let out = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        signed_in_post(
            harness,
            http,
            &email,
            &password,
            MUTE_PATH,
            &[("workspace_id", ws.as_str())],
        )
        .await
    };
    world.last_status = Some(out.status);
    world.last_headers = Some(out.headers);
    world.last_body = Some(out.body);
}

#[when(regex = r#"^Nadia resubscribes to "([^"]+)" from the settings surface$"#)]
async fn nadia_resubscribes_workspace(world: &mut FoundryWorld, workspace: String) {
    let workspace_id = nadia_ws_id(world, &workspace);
    world.npui_last_target = Some(workspace_id);
    let (email, password) = creds(world);
    let ws = workspace_id.to_string();
    let out = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        signed_in_post(
            harness,
            http,
            &email,
            &password,
            RESUBSCRIBE_PATH,
            &[("workspace_id", ws.as_str())],
        )
        .await
    };
    world.last_status = Some(out.status);
    world.last_headers = Some(out.headers);
    world.last_body = Some(out.body);
}

#[when(regex = r#"^Nadia tries to mute "([^"]+)" without a valid request token$"#)]
async fn nadia_mutes_without_token(world: &mut FoundryWorld, workspace: String) {
    let workspace_id = nadia_ws_id(world, &workspace);
    world.npui_last_target = Some(workspace_id);
    let (email, password) = creds(world);
    // A real signed-in session, but the POST presents NO `foundry_csrf` cookie and NO
    // `_csrf` field, so the shipped `csrf_middleware` refuses it before the handler runs.
    let session = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        establish_session(harness, http, &email, &password).await
    };
    let ws = workspace_id.to_string();
    let out = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        post_with_cookie(
            harness,
            http,
            MUTE_PATH,
            &session,
            &[("workspace_id", ws.as_str())],
        )
        .await
    };
    world.last_status = Some(out.status);
    world.last_headers = Some(out.headers);
    world.last_body = Some(out.body);
}

#[when(regex = r#"^Nadia mutes "([^"]+)" twice from a stale surface$"#)]
async fn nadia_mutes_twice(world: &mut FoundryWorld, workspace: String) {
    let workspace_id = nadia_ws_id(world, &workspace);
    world.npui_last_target = Some(workspace_id);
    let (email, password) = creds(world);
    let ws = workspace_id.to_string();
    let first = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        signed_in_post(
            harness,
            http,
            &email,
            &password,
            MUTE_PATH,
            &[("workspace_id", ws.as_str())],
        )
        .await
    };
    world.npui_prior_body = Some(first.body.clone());
    let second = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        signed_in_post(
            harness,
            http,
            &email,
            &password,
            MUTE_PATH,
            &[("workspace_id", ws.as_str())],
        )
        .await
    };
    world.last_status = Some(second.status);
    world.last_headers = Some(second.headers);
    world.last_body = Some(second.body);
}

#[when(regex = r#"^Nadia tries to mute a workspace she does not belong to$"#)]
async fn nadia_mutes_unknown_workspace(world: &mut FoundryWorld) {
    // A well-formed id for a workspace that either does not exist or she is not a member
    // of — the handler must resolve membership from her SESSION and refuse non-enumerably.
    let workspace_id = uuid::Uuid::now_v7();
    world.npui_last_target = Some(workspace_id);
    let (email, password) = creds(world);
    let ws = workspace_id.to_string();
    let out = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        signed_in_post(
            harness,
            http,
            &email,
            &password,
            MUTE_PATH,
            &[("workspace_id", ws.as_str())],
        )
        .await
    };
    world.last_status = Some(out.status);
    world.last_headers = Some(out.headers);
    world.last_body = Some(out.body);
}

#[when(regex = r#"^a crafted request tries to mute a workspace belonging to another recipient$"#)]
async fn crafted_mute_foreign_workspace(world: &mut FoundryWorld) {
    let other_email = "vic@umbrella.example";
    let workspace_id = {
        let harness = world.harness.as_ref().expect("harness");
        seed_other_recipient_workspace(harness, "Umbrella", other_email).await
    };
    world.npui_foreign = Some((other_email.to_string(), workspace_id));
    world.npui_last_target = Some(workspace_id);
    // Nadia is signed in but supplies a foreign workspace id she does not belong to. Her
    // identity comes from the SESSION (never the request), so this can neither write her
    // row for a workspace she has no membership in NOR the other recipient's row.
    let (email, password) = creds(world);
    let ws = workspace_id.to_string();
    let out = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        signed_in_post(
            harness,
            http,
            &email,
            &password,
            MUTE_PATH,
            &[("workspace_id", ws.as_str())],
        )
        .await
    };
    world.last_status = Some(out.status);
    world.last_headers = Some(out.headers);
    world.last_body = Some(out.body);
}

#[when(regex = r#"^a signed-out visitor opens the notification settings surface$"#)]
async fn signed_out_opens_settings(world: &mut FoundryWorld) {
    let out = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        get_with_cookie(harness, http, SETTINGS_PATH, "").await
    };
    world.last_status = Some(out.status);
    world.last_headers = Some(out.headers);
    world.last_body = Some(out.body);
}

#[when(regex = r#"^a signed-out visitor tries to mute a workspace$"#)]
async fn signed_out_mutes_workspace(world: &mut FoundryWorld) {
    // Target one of the persona's real workspaces so a leaked write would be observable —
    // the point is that a signed-out (no cookie, no CSRF) POST changes nothing.
    let workspace_id = world
        .npui_workspaces
        .first()
        .map(|(_, id)| *id)
        .expect("a workspace was seeded");
    world.npui_last_target = Some(workspace_id);
    let ws = workspace_id.to_string();
    let out = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        post_with_cookie(
            harness,
            http,
            MUTE_PATH,
            "",
            &[("workspace_id", ws.as_str())],
        )
        .await
    };
    world.last_status = Some(out.status);
    world.last_headers = Some(out.headers);
    world.last_body = Some(out.body);
}

// ============================================================================
// Then — observable outcomes on the surface + at the store boundary
// ============================================================================

#[then(regex = r#"^the sidebar footer offers a link to the notification settings surface$"#)]
async fn sidebar_offers_settings_link(world: &mut FoundryWorld) {
    // Asserted by href (OD-2: the label may be "Settings" or "Notifications"), in the
    // footer user block — NOT as a third primary rail item (NFR-3).
    html::assert_has(
        &body_of(world),
        r#".sidebar__user a[href="/account/settings"]"#,
    );
}

#[then(
    regex = r#"^Nadia sees the notification settings surface listing "([^"]+)", "([^"]+)", and "([^"]+)"$"#
)]
async fn nadia_sees_listing(world: &mut FoundryWorld, ws_a: String, ws_b: String, ws_c: String) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the settings surface must render (200), got {:?}",
        world.last_status
    );
    let body = body_of(world);
    for name in [&ws_a, &ws_b, &ws_c] {
        assert_listed(&body, name);
    }
}

#[then(regex = r#"^"([^"]+)" is shown as muted on the settings surface$"#)]
async fn shown_as_muted(world: &mut FoundryWorld, workspace: String) {
    let (status, body) = open_settings_signed_in(world).await;
    assert!(
        status.is_success(),
        "the settings surface must render (2xx) to show a mute status, got {status}; body:\n{body}"
    );
    assert_row_status(&body, &workspace, "muted");
}

#[then(regex = r#"^"([^"]+)" is shown as subscribed on the settings surface$"#)]
async fn shown_as_subscribed(world: &mut FoundryWorld, workspace: String) {
    let (status, body) = open_settings_signed_in(world).await;
    assert!(
        status.is_success(),
        "the settings surface must render (2xx) to show a mute status, got {status}; body:\n{body}"
    );
    assert_row_status(&body, &workspace, "subscribed");
}

#[then(regex = r#"^the notification settings surface is not shown$"#)]
async fn settings_surface_not_shown(world: &mut FoundryWorld) {
    let body = body_of(world);
    let doc = html::parse(&body);
    assert!(
        html::select_all(&doc, "[data-status]").is_empty(),
        "a signed-out caller must NOT see the per-workspace notification listing; body:\n{body}"
    );
    assert_eq!(
        world.last_status,
        Some(StatusCode::NOT_FOUND),
        "a signed-out settings request must be the uniform non-enumerable 404, got {:?}",
        world.last_status
    );
}

#[then(regex = r#"^the mute is refused and Nadia's notification state is unchanged$"#)]
async fn mute_refused_state_unchanged(world: &mut FoundryWorld) {
    let status = world.last_status.expect("a response status was captured");
    assert!(
        !status.is_success(),
        "a mute without a valid request token must be refused, got {status}"
    );
    let workspace_id = world.npui_last_target.expect("a mute target was recorded");
    let harness = world.harness.as_ref().expect("harness");
    let muted = harness
        .app
        .state
        .store
        .is_unsubscribed(&NADIA_EMAIL.to_ascii_lowercase(), workspace_id)
        .await
        .expect("read the persona's opt-out state");
    assert!(
        !muted,
        "a refused mute must not change the persona's notification state"
    );
}

#[then(regex = r#"^Nadia sees the same mute confirmation both times with no error$"#)]
async fn same_mute_confirmation_both_times(world: &mut FoundryWorld) {
    let status = world.last_status.expect("a response status was captured");
    assert!(
        status.is_success(),
        "a repeated (idempotent) mute must still succeed, got {status}"
    );
    let first = world
        .npui_prior_body
        .clone()
        .expect("the first mute body was captured");
    let second = body_of(world);
    assert_eq!(
        first, second,
        "confirming a mute twice must render the same confirmation with no error"
    );
    let workspace_id = world.npui_last_target.expect("a mute target was recorded");
    let harness = world.harness.as_ref().expect("harness");
    let muted = harness
        .app
        .state
        .store
        .is_unsubscribed(&NADIA_EMAIL.to_ascii_lowercase(), workspace_id)
        .await
        .expect("read the persona's opt-out state");
    assert!(muted, "after a double mute the workspace must be muted");
}

#[then(regex = r#"^the mute is refused without revealing whether the workspace exists$"#)]
async fn mute_refused_non_enumerable(world: &mut FoundryWorld) {
    let status = world.last_status.expect("a response status was captured");
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a mute for an unknown / non-member workspace must be the uniform non-enumerable 404, got {status}"
    );
}

#[then(regex = r#"^the other recipient's notification state is unchanged$"#)]
async fn other_recipient_state_unchanged(world: &mut FoundryWorld) {
    let (other_email, workspace_id) = world
        .npui_foreign
        .clone()
        .expect("a foreign recipient + workspace were seeded");
    let harness = world.harness.as_ref().expect("harness");
    let muted = harness
        .app
        .state
        .store
        .is_unsubscribed(&other_email.to_ascii_lowercase(), workspace_id)
        .await
        .expect("read the other recipient's opt-out state");
    assert!(
        !muted,
        "a crafted foreign-workspace mute must not change another recipient's notification state"
    );
}

#[then(regex = r#"^the mute is refused and no notification state changes$"#)]
async fn mute_refused_no_state_change(world: &mut FoundryWorld) {
    let status = world.last_status.expect("a response status was captured");
    assert!(
        !status.is_success(),
        "a signed-out mute must be refused, got {status}"
    );
    let workspace_id = world.npui_last_target.expect("a mute target was recorded");
    let harness = world.harness.as_ref().expect("harness");
    let muted = harness
        .app
        .state
        .store
        .is_unsubscribed(&NADIA_EMAIL.to_ascii_lowercase(), workspace_id)
        .await
        .expect("read the persona's opt-out state");
    assert!(
        !muted,
        "a signed-out mute must not change any notification state"
    );
}
